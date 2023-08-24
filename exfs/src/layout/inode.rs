use std::mem::size_of;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::BLOCK_SIZE;
use crate::layout::inode::FileType::{BlockDevice, CharDevice, Dir, File, Socket, SymbolLink, UNK};
use crate::manager::block_cache_manager::BlockCacheDevice;

///
/// Inode 文件索引节点
/// 每个块可以存放 BLK_SZ / INODE_SIZE = 64 个 INODE
/// Mode: 7 + 9
/// socket         1100 ___
/// symbol link    1010 ___
/// file           1000 ___
/// block device   0110 ___
/// dir            0100 ___
/// char device    0010 ___
/// fifo           0001 ___
///
/// set uid        ____ 1__ 任何用户执行该文件时，它将以文件所有者的权限运行
/// set gid        ____ _1_ 新创建的文件将继承目录的组所有权
/// sticky bit     ____ __1 只有文件所有者和超级用户才能删除该目录中的文件

pub const SOCKET: u16 = 0b1100;
pub const SYMBOL: u16 = 0b1010;
pub const FILE: u16 = 0b1000;
pub const BLOCK_DEVICE: u16 = 0b0110;
pub const DIR: u16 = 0b0100;
pub const CHAR_DEVICE: u16 = 0b0010;
pub const FIFO: u16 = 0b1;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Inode {
    // 64 bytes
    pub index_level: u8,
    // 1 索引等级,最小为 0,直接指向数据块,当当前等级的索引无法满足上限后将索引升一级,最高 255 级
    pub extra: [u8; 9],
    // 9
    pub mode: u16,
    // 2 bytes,
    pub link_count: u32,
    // 4
    pub created: u64,
    // 8
    pub modified: u64,
    // 8
    pub size: u64,
    // 8
    pub uid: u32,
    // 8
    pub gid: u32,
    // 8
    pub index_node: IndexNode, // 8,索引地址
}

impl Inode {
    pub fn new(mode: u16) -> Self {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards").as_secs();
        Self {
            index_level: 0,
            extra: [0u8;9],
            mode,
            link_count: 0,
            created: since_the_epoch,
            modified: since_the_epoch,
            size: 0,
            uid: 0,
            gid: 0,
            index_node: Default::default(),
        }
    }
    pub fn nil() -> Self {
        Self{
            index_level: 0,
            extra: [0u8;9],
            mode: 0,
            link_count: 0,
            created: 0,
            modified: 0,
            size: 0,
            uid: 0,
            gid: 0,
            index_node: Default::default(),
        }
    }
}

impl Inode {
    pub fn exist(&self) -> bool {
        self.file_type() != UNK
    }
    pub fn is_dir(&self) -> bool {
        self.file_type() == Dir
    }
    pub fn file_type(&self) -> FileType {
        match self.mode >> 12 {
            SOCKET => Socket,
            SYMBOL => SymbolLink,
            FILE => File,
            BLOCK_DEVICE => BlockDevice,
            DIR => Dir,
            CHAR_DEVICE => CharDevice,
            FIFO => FileType::FIFO,
            _ => UNK,
        }
    }
    pub fn blocks(&self) -> u64 {
        return (self.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;
    }
}

pub type IndexBlock = [IndexNode; BLOCK_SIZE / INDEX_NODE_SIZE];

/// 多级索引项
/// 8 bytes / 16 bytes
/// 每个文件快可存储 512 / 256 索引项
/// 每个索引块至少可表示一个文件块
/// 至少可表示 256^n * 4KB 大小的文件(理论无上限,只需要扩充 index_level 级数)
/// 但是n级索引占用空间为 256^(n-1) + ... + 256^1 + 1 ≈ 256 ^ (n-1) * 4KB
/// 即索引会最多占用文件大小的 1 / 256 ≈ 0.39 %, 在可接受范围内
/// 主要受制于文件系统碎片化程度与文件系统大小
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct IndexNode {
    start_blk: usize,
    // inclusive
    len: usize,       // exclusive
}

impl IndexNode {
    pub fn is_valid(&self) -> bool {
        self.len != 0
    }
    pub fn list(&self, device: &mut BlockCacheDevice, level: u8) -> Vec<usize> {
        let mut vec = Vec::new();
        for blk_id in self.start_blk..(self.start_blk + self.len) {
            // debug!("blk_id {}", blk_id);
            if level <= 1 {
                // 一级索引直接将块 id 返回
                vec.push(blk_id)
            } else {
                device.block_cache(blk_id).lock().unwrap().read(
                    0,
                    |data: &[IndexNode; BLOCK_SIZE / INDEX_NODE_SIZE]| {
                        data.iter().for_each(|v| {
                            // vec.push(v.clone());
                            v.list(device, level - 1)
                                .iter()
                                .for_each(|data| vec.push(data.clone()));
                        })
                    },
                )
            }
        }
        vec
    }
    pub fn list_level_blk(&self, device: &mut BlockCacheDevice, level: u8, need: u8) -> Vec<usize> {
        let mut vec = Vec::new();
        for blk_id in self.start_blk..(self.start_blk + self.len) {
            if level <= need + 1 {
                // 所需级索引直接将块 id 返回
                if blk_id != 0 {
                    vec.push(blk_id)
                }
            } else {
                device.block_cache(blk_id).lock().unwrap().read(
                    0,
                    |data: &[IndexNode; BLOCK_SIZE / INDEX_NODE_SIZE]| {
                        data.iter().for_each(|v| {
                            // vec.push(v.clone());
                            v.list(device, level - 1)
                                .iter()
                                .for_each(|data| vec.push(data.clone()));
                        })
                    },
                )
            }
        }
        vec
    }
    /// 删除当前索引节点以及下属索引节点
    /// keep_data: 是否保留 DataBlock
    pub fn delete(&self, device: &mut BlockCacheDevice, level: u8, keep_data: bool) {
        for blk_id in self.start_blk..(self.start_blk + self.len) {
            // debug!("{},{}->{}",blk_id,self.start_blk,self.start_blk +self.len);
            if level <= 1 {
                // 数据块 id
                if !keep_data {
                    // 删除数据块
                    device
                        .block_cache(device.data_block(blk_id))
                        .lock()
                        .unwrap()
                        .free();
                    device.free_block(blk_id, false,true);
                }
            } else {
                device
                    .block_cache(device.data_block(blk_id))
                    .lock()
                    .unwrap()
                    .modify(0, |data: &mut [IndexNode; BLOCK_SIZE / INDEX_NODE_SIZE]| {
                        data.iter_mut().for_each(|v| {
                            // vec.push(v.clone());
                            if v.is_valid() {
                                v.delete(device, level - 1, keep_data);
                            }
                            *v = IndexNode::default();
                        });
                    });
                device.free_block(blk_id, false,true)
            }
        }
    }
}

impl IndexNode {
    /// 给定数据块 id,生成紧凑的 IndexNode 列表
    pub fn from(blocks: Vec<usize>) -> Vec<Self> {
        let mut value = blocks;
        let mut nodes = Vec::new();
        value.sort();
        let mut last_id = 0;
        let mut node = IndexNode::default();
        value.iter().enumerate().for_each(|(id, v)| {
            if v - last_id == 1 && id != 0 {
                // 连续,长度 ＋1
                node.len += 1;
            } else {
                // 不连续,将之前的推进去,并创建新节点
                if node.len != 0 {
                    nodes.push(node.clone());
                }
                node.start_blk = *v;
                node.len = 1;
            }
            last_id = *v;
        });
        if node.len != 0 {
            nodes.push(node.clone());
        }
        nodes
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum FileType {
    Socket,
    SymbolLink,
    File,
    BlockDevice,
    Dir,
    CharDevice,
    FIFO,
    UNK,
}

pub const INDEX_NODE_SIZE: usize = size_of::<IndexNode>();
pub const INODE_SIZE: usize = size_of::<Inode>();
