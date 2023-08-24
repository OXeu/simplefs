use std::collections::BTreeMap;
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use libc::{c_int, EBADF, EEXIST, ENOENT, ENOSPC, ENOTDIR};
use lru::LruCache;

use crate::block_device::block_device::BlockDevice;
use crate::cache::block_cache::CacheBlock;
use crate::cache::file_handler::FileHandler;
use crate::config::BLOCK_SIZE;
use crate::layout::data_block::{DataBlock, DirEntry, FileName, DIR_ENTRY_SIZE};
use crate::layout::inode::{IndexNode, Inode, DIR};
use crate::layout::super_block::SuperBlock;
use crate::manager::DirEntryDetail;
use crate::utils::slice::{align, vec2slice};

/// 块设备缓存管理器
pub struct BlockCacheDevice {
    device: Arc<dyn BlockDevice>,
    caches: LruCache<usize, Arc<Mutex<CacheBlock>>>,
    file_handlers: BTreeMap<u64, FileHandler>,
    recycled_fh: Vec<u64>,
    pub super_block: Arc<Mutex<CacheBlock>>,
}

impl BlockCacheDevice {
    pub fn new(device: Arc<dyn BlockDevice>) -> Self {
        let cache_blk = Arc::new(Mutex::new(CacheBlock::new(device.clone(), 0)));
        Self {
            device,
            caches: LruCache::new(NonZeroUsize::new(128).unwrap()),
            file_handlers: BTreeMap::new(),
            recycled_fh: Vec::new(),
            super_block: cache_blk,
        }
    }

    pub fn fh(&self, fh: u64) -> Option<&FileHandler> {
        self.file_handlers.get(&fh)
    }

    pub fn open_inner(&mut self, inode: usize, offset: usize, flags: u16) -> u64 {
        let key = if self.recycled_fh.is_empty() {
            self.file_handlers.keys().max().unwrap_or(&0) + 1
        } else {
            self.recycled_fh.pop().unwrap()
        };
        self.file_handlers.insert(
            key,
            FileHandler {
                inode,
                offset,
                flags,
            },
        );
        key
    }

    pub fn close_inner(&mut self, fh: u64) -> Result<(), c_int> {
        match self.file_handlers.get(&fh) {
            Some(_) => {
                self.file_handlers.remove(&fh);
                self.recycled_fh.push(fh);
                Ok(())
            }
            None => Err(EBADF),
        }
    }

    pub fn block_cache(&mut self, block: usize) -> Arc<Mutex<CacheBlock>> {
        match self.caches.get(&block) {
            Some(cache) => cache.clone(),
            None => {
                let cache = Arc::new(Mutex::new(CacheBlock::new(self.device.clone(), block)));
                let _ = self.caches.push(block, cache.clone());
                cache
            }
        }
    }

    pub fn data_block(&self, id: usize) -> usize {
        let mut super_block: SuperBlock = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        super_block.data_block(id)
    }

    /// 通过 inode 号计算实际物理块地址与偏移量
    /// inode 块是倒序存储的,内部是顺序存储的
    /// @return block_id(物理),offset
    pub fn inode_block(&self, id: usize) -> (usize, usize) {
        self.super_block().inode_block(id)
    }

    pub fn data<T>(&mut self, id: usize, offset: usize, f: impl FnOnce(&T)) {
        let blk_id = self.data_block(id);
        self.block_cache(blk_id).lock().unwrap().read(offset, f);
    }

    pub fn modify_data<V>(&mut self, id: usize, f: impl FnOnce(&mut DataBlock) -> V) {
        let blk_id = self.data_block(id);
        self.block_cache(blk_id).lock().unwrap().modify(0, f);
    }

    pub fn inode(&mut self, id: usize) -> Inode {
        let (blk_id, offset) = self.inode_block(id);
        let mut inode: Inode = Inode::new(0);
        self.block_cache(blk_id)
            .lock()
            .unwrap()
            .read(offset, |i: &Inode| {
                inode = *i;
            });
        inode
    }

    pub fn inode_data_blk_list(&mut self, id: usize) -> Vec<usize> {
        let inode = self.inode(id);
        inode.index_node.list(self, inode.index_level)
    }

    /// id: inode id
    pub fn dir_list(&mut self, id: usize) -> Vec<DirEntry> {
        let inode = self.inode(id);
        let mut entries = Vec::new();
        // println!("dir list: {:?},{}", inode.index_node, inode.index_level);
        inode
            .index_node
            .list(self, inode.index_level)
            .iter()
            .for_each(|v| {
                // println!("data blocks: {}", v);
                self.block_cache(self.data_block(*v)).lock().unwrap().read(
                    0,
                    |dirs: &[DirEntry; BLOCK_SIZE / DIR_ENTRY_SIZE]| {
                        // println!("dir entries: {:?}", dirs);
                        dirs.iter()
                            .filter(|v| !v.name.is_empty() && v.inode != 0)
                            .for_each(|dir| entries.push(*dir))
                    },
                );
            });
        entries
    }

    pub fn modify_inode<V>(&mut self, id: usize, f: impl FnOnce(&mut Inode) -> V) -> V {
        let (blk_id, offset) = self.inode_block(id);
        self.block_cache(blk_id).lock().unwrap().modify(offset, f)
    }

    /// 写入完整数据,并自动为其创建完整的索引节点,返回根节点和 level
    pub fn write_data(&mut self, buf: &[u8], level: u8) -> (IndexNode, u8) {
        let len = buf.len();
        if len == 0 {
            return (IndexNode::default(), 0);
        }
        let blocks_need = (len + BLOCK_SIZE - 1) / BLOCK_SIZE;
        let mut blks = Vec::new();
        for i in 0..blocks_need {
            let blk_id = self.alloc_block(false).expect("No enough space left");

            self.block_cache(self.data_block(blk_id))
                .lock()
                .unwrap()
                .modify(0, |v: &mut [u8; BLOCK_SIZE]| {
                    let range = i * BLOCK_SIZE..len.min((i + 1) * BLOCK_SIZE);
                    let slice = &buf[range];
                    v[..slice.len()].copy_from_slice(slice)
                });
            blks.push(blk_id);
        }

        self.make_indexes(blks, level + 1)
    }

    // 保留已有节点，仅连接新增子节点
    /// data_level: 数据块为 0
    ///  new_data_blocks: 现在的所有数据块，包含原有块，若不包含则表示删除数据块，将会缩减索引
    pub fn make_index_part(
        &mut self,
        inode_id: usize,
        data_blocks: Vec<usize>,
        data_level: u8,
    ) -> Result<(), c_int> {
        let new_index = IndexNode::from(data_blocks.clone());
        // println!("Index node：{:?},data:{:?}", new_index, data_blocks);
        if new_index.len() <= 1 {
            // top level,save to inode
            self.modify_inode(inode_id, |ino| {
                if new_index.is_empty() {
                    ino.index_node = IndexNode::default();
                    ino.index_level = 0;
                } else {
                    ino.index_node = *new_index.first().unwrap();
                    ino.index_level = data_level + 1;
                }
            });
            return Ok(());
        }
        // 将索引转换为字节存储
        let buf: Vec<u8> = vec2slice(new_index);
        let need_blk_num = (buf.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;
        let inode = self.inode(inode_id);
        let mut index_blk =
            inode
                .index_node
                .list_level_blk(self, inode.index_level, data_level + 1);
        if need_blk_num > index_blk.len() {
            // 扩容
            for _ in index_blk.len()..need_blk_num {
                match self.alloc_block(false) {
                    None => return Err(ENOSPC),
                    Some(data_id) => index_blk.push(data_id),
                }
            }
        } else if need_blk_num < index_blk.len() {
            // 缩容
            for _ in need_blk_num..index_blk.len() {
                self.free_block(index_blk.pop().unwrap(), false, true);
            }
        }
        let mut offset = 0;
        for i in 0..need_blk_num {
            // 需要写几块
            self.modify_data(index_blk[i], |data| {
                let end = BLOCK_SIZE.min(buf.len() - offset);
                data[..end]
                    .copy_from_slice(&buf[i * BLOCK_SIZE..buf.len().min((i + 1) * BLOCK_SIZE)]);
            });
            offset += BLOCK_SIZE
        }
        self.make_index_part(inode_id, index_blk, data_level + 1)
    }

    pub fn make_indexes(&mut self, data_blocks: Vec<usize>, level: u8) -> (IndexNode, u8) {
        let index_node_list = IndexNode::from(data_blocks.clone());
        if index_node_list.is_empty() {
            panic!("Empty data blocks![{:?}]", data_blocks);
            // return (IndexNode::default(),0);
        }
        if index_node_list.len() > 1 {
            let buf: Vec<u8> = index_node_list
                .iter()
                .map(|v| unsafe {
                    std::slice::from_raw_parts(
                        v as *const IndexNode as *const u8,
                        size_of::<IndexNode>(),
                    )
                })
                .flatten()
                .cloned()
                .collect();
            self.write_data(buf.as_slice(), level);
        }
        return (index_node_list[0], level);
    }
}

impl From<Arc<dyn BlockDevice>> for BlockCacheDevice {
    fn from(value: Arc<dyn BlockDevice>) -> Self {
        BlockCacheDevice::new(value)
    }
}

/// 初始化接口
impl BlockCacheDevice {
    pub fn mkfs(&mut self, block_size: usize) {
        // 清空磁盘所有块
        for blk_id in 0..block_size {
            self.block_cache(blk_id).lock().unwrap().free()
        }
        // 初始化超级块
        self.super_block
            .lock()
            .unwrap()
            .modify(0, |sb: &mut SuperBlock| {
                *sb = SuperBlock::new(block_size);
            });
        self.super_block.lock().unwrap().sync();
        self.clear_bitmap();
        // self.print();
        // 创建根节点
        self.mk_root();
        // 同步至磁盘
        self.sync();
    }

    pub fn sync(&mut self) {
        self.caches
            .iter()
            .for_each(|(_, c)| c.lock().unwrap().sync());
        self.device.sync();
    }

    fn mk_root(&mut self) {
        let inode = self.alloc_block(true).unwrap();
        self.modify_inode(inode, |root| {
            *root = Inode::new(DIR << 12 | 0b111101101);
            root.size = BLOCK_SIZE as u64
        })
    }
}

/// 上层接口
impl BlockCacheDevice {
    pub fn mk_file(
        &mut self,
        file_name: &str,
        parent_inode: usize,
        mode: u16,
    ) -> Result<usize, c_int> {
        let mut name = [0u8; 56];
        name[..file_name.len()].copy_from_slice(file_name.as_bytes());
        // println!("mk_file: {:?}, parent: {}", name, parent_inode);
        let parent = self.inode(parent_inode);
        println!("parent Inode({}):{:?}", parent_inode, parent);
        if parent.exist() {
            return if parent.is_dir() {
                let mut dirs = self.dir_list(parent_inode);
                // println!("dirs0: {:?}", dirs);
                if dirs.iter().any(|v| v.name.as_slice() == name) {
                    return Err(EEXIST);
                }
                let inode_opt = self.alloc_block(true);
                match inode_opt {
                    None => return Err(ENOSPC),
                    Some(inode_id) => {
                        // println!("Allocated Inode: {}", inode_id);
                        self.print();
                        self.modify_inode(inode_id, |inode| *inode = Inode::new(mode));
                        dirs.push(DirEntry {
                            name,
                            inode: inode_id as u64,
                        });
                        // println!("dirs: {:?}", dirs);
                        let buf: Vec<u8> = vec2slice(dirs);
                        if let Err(e) = self.write_all(0, parent_inode, &buf, true) {
                            println!("mk_file:339 error: {}", e);
                            return Err(e);
                        }
                        // let (index_node, level) = self.write_data(buf.as_slice(), 0);
                        // // println!("free {}({})", 5, self.check(5));
                        // parent.index_node.delete(self, parent.index_level, false); // 删除原数据
                        // self.modify_inode(parent_inode, |p| {
                        //     p.index_node = index_node;
                        //     p.index_level = level;
                        //     p.size = buf.len() as u64;
                        // });
                        // let parent = self.inode(parent_inode);
                        // println!("parent ({}) -> {:?}", parent_inode, parent);
                        return Ok(inode_id);
                    }
                }
            } else {
                Err(ENOTDIR)
            };
        }
        println!("mk_file:end error: NotExist");
        Err(ENOENT)
    }

    pub fn ls_(&mut self, inode_id: usize) -> Result<Vec<DirEntry>, c_int> {
        // println!("ls_ {}",inode_id);
        let inode = self.inode(inode_id);
        if inode.exist() {
            if inode.is_dir() {
                Ok(self.dir_list(inode_id))
            } else {
                Err(ENOTDIR)
            }
        } else {
            Err(ENOENT)
        }
    }

    pub fn lookup(&mut self, parent: usize, name: FileName) -> Result<DirEntry, c_int> {
        match self.ls_(parent) {
            Ok(v) => match v.iter().find(|entry| name == entry.name) {
                None => Err(ENOENT),
                Some(e) => Ok(e.clone()),
            },
            Err(e) => Err(e),
        }
    }

    pub fn rm(&mut self, parent: usize, name: FileName, keep_file: bool) -> Result<(), c_int> {
        match self.ls_(parent) {
            Ok(mut v) => {
                for (id, entry) in v.clone().iter().enumerate() {
                    if entry.name == name {
                        v.remove(id);
                        if !keep_file {
                            let ino_id = entry.inode as usize;
                            let inode = self.inode(ino_id);
                            inode.index_node.delete(self, inode.index_level, true);
                            self.free_block(ino_id, true, true);
                        }
                        let mut buf = vec2slice(v);
                        align(&mut buf, BLOCK_SIZE);
                        return match self.write_all(0, parent, &buf, true) {
                            Ok(_) => Ok(()),
                            Err(e) => Err(e),
                        };
                    }
                }
                Err(ENOENT)
            }
            Err(e) => Err(e),
        }
    }

    pub fn rename_inner(
        &mut self,
        parent: usize,
        name: FileName,
        new_parent: usize,
        new_name: FileName,
    ) -> Result<(), c_int> {
        let old = self.lookup(parent, name);
        if parent == new_parent && name == new_name {
            return Ok(());
        }
        match old {
            Ok(entry) => match self.rm(new_parent, new_name, false) {
                Ok(_) | Err(ENOENT) => {
                    let mut new_dirs = self.dir_list(new_parent);
                    new_dirs.push(DirEntry {
                        name: new_name,
                        inode: entry.inode,
                    });
                    let mut buf = vec2slice(new_dirs);
                    align(&mut buf, BLOCK_SIZE);
                    if let Err(e) = self.write_all(0, new_parent, &buf, true) {
                        return Err(e);
                    }
                    match self.rm(parent, name, true) {
                        Ok(_) => Ok(()),
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        }
    }

    pub fn ls(&mut self, path: &str) -> Result<Vec<DirEntryDetail>, c_int> {
        let path_split = path.split("/").filter(|p| !p.is_empty());
        let mut parent_inode = 0;
        for p in path_split {
            match self
                .dir_list(parent_inode)
                .iter()
                .find(|v| to_str(v.name) == p)
            {
                None => return Err(ENOENT),
                Some(v) => parent_inode = v.inode as usize,
            }
        }
        self.ls_(parent_inode).map(|vec| {
            vec.iter()
                .map(|v| DirEntryDetail {
                    name: to_str(v.name),
                    inode_id: v.inode as usize,
                    inode: self.inode(v.inode as usize),
                })
                .collect()
        })
    }
}

pub fn trim_zero(data: Vec<u8>) -> Vec<u8> {
    let mut trimmed_data = data.clone();
    while let Some(&last) = trimmed_data.last() {
        if last == 0 {
            trimmed_data.pop();
        } else {
            break;
        }
    }
    trimmed_data
}

pub fn to_str(name: FileName) -> String {
    unsafe { String::from_utf8_unchecked(trim_zero(name.to_vec())) }
}
