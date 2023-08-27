use std::collections::BTreeMap;
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use libc::{c_int, EBADF, ENOSPC};
use lru::LruCache;

use crate::block_device::block_device::BlockDevice;
use crate::cache::block_cache::CacheBlock;
use crate::cache::file_handler::FileHandler;
use crate::config::BLOCK_SIZE;
use crate::layout::data_block::DataBlock;
use crate::layout::index_node::IndexNode;
use crate::layout::inode::{Inode, InodeWithId};
use crate::layout::super_block::SuperBlock;
use crate::manager::error_code::ErrorCode;
use crate::typ::file_type::FileType;
use crate::utils::slice::vec2slice;

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

    pub fn fh(&self, fh: u32, pid: u32) -> Option<&FileHandler> {
        let key = (pid as u64) << 32 | fh as u64;
        println!("fh: {:x},pid:{:x},key:{:x},kv:{:x?}", fh, pid, key, self.file_handlers.keys());
        let fh = self.file_handlers.get(&key);
        if fh.is_none() { panic!("get fh bad descriptor") }
        fh
    }

    pub fn open_internal(
        &mut self,
        inode: usize,
        offset: usize,
        flags: i32,
        pid: u32,
    ) -> Result<u32, ErrorCode> {
        let key = if self.recycled_fh.iter().filter(|&v| (*v >> 32) as u32 == pid).count() <= 0 {
            self.file_handlers.keys().filter(|&v| (*v >> 32) as u32 == pid).max().unwrap_or(&((pid as u64) << 32)) + 1
        } else {
            self.recycled_fh.pop().unwrap()
        };
        FileHandler::new(inode, self, offset, flags).map(|v| {
            println!("open pid:{:x},key:{:x}", pid, key);
            self.file_handlers.insert(key, v);
            key as u32
        })
    }

    pub fn close_internal(&mut self, fh: u32, pid: u32, flush: bool) -> Result<(), c_int> {
        let key = (pid as u64) << 32 | fh as u64;
        match self.file_handlers.get(&key) {
            Some(_) => {
                self.file_handlers.remove(&key).map(|fh| {
                    if flush {
                        fh.flush(self);
                    }
                });
                self.recycled_fh.push(key);
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
        let (blk_id, offset) = self.super_block().inode_block(id);
        println!("[Inode Block] {} -> {}({})", id, blk_id, offset);
        (blk_id, offset)
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
        let mut inode: Inode = Inode::nil();
        self.block_cache(blk_id)
            .lock()
            .unwrap()
            .read(offset, |i: &Inode| {
                inode = *i;
            });
        inode
    }

    /// inode 数据存储所在的 数据块 id 列表
    /// 非物理块
    pub fn inode_data_blk_list(&mut self, inode: &Inode) -> Vec<usize> {
        inode.index_node.list(self, inode.index_level)
    }

    /// id: inode id
    pub fn read_all(&mut self, id: usize) -> Vec<u8> {
        let inode = self.inode(id);
        let mut data_ = Vec::new();
        // debug!("dir list: {:?},{}", inode.index_node, inode.index_level);
        inode
            .index_node
            .list(self, inode.index_level)
            .iter()
            .for_each(|v| {
                // debug!("data blocks: {}", v);
                self.block_cache(self.data_block(*v))
                    .lock()
                    .unwrap()
                    .read(0, |data: &[u8; BLOCK_SIZE]| {
                        data.iter().for_each(|v| data_.push(*v))
                    });
            });
        data_.truncate(inode.size as usize);
        data_
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
        inode: &InodeWithId,
        data_blocks: Vec<usize>,
        data_level: u8,
    ) -> Result<(), c_int> {
        let new_index = IndexNode::from(data_blocks.clone());
        // debug!("Index node：{:?},data:{:?}", new_index, data_blocks);
        if new_index.len() <= 1 {
            // top level,save to inode
            self.modify_inode(inode.inode, |ino| {
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
        let mut index_blk = inode.inode().index_node.list_level_blk(
            self,
            inode.inode().index_level,
            data_level + 1,
        );
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
        self.make_index_part(inode, index_blk, data_level + 1)
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
    pub fn flush_internal(&mut self, inode: &InodeWithId) {
        let mut data_blocks = self.inode_data_blk_list(inode.inode());
        data_blocks = data_blocks.iter().map(|data_id| {
            self.data_block(*data_id)
        }).collect();
        let (inode_blk, _) = self.inode_block(inode.inode);
        data_blocks.push(inode_blk);
        self.caches
            .iter()
            .filter(|(id, _)| {
                data_blocks.contains(*id)
            })
            .for_each(|(_, c)| c.lock().unwrap().sync());
        self.device.sync();
    }
    fn mk_root(&mut self) {
        let inode = self.alloc_block(true).unwrap();
        self.modify_inode(inode, |root| {
            *root = Inode::new((FileType::Dir as u16) << 12 | 0b111101101, 0, 0);
            root.size = BLOCK_SIZE as u64
        })
    }
}
