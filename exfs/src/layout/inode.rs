use std::mem::size_of;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::BLOCK_SIZE;
use crate::layout::index_node::IndexNode;
use crate::typ::file_type::FileType;

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

// 64 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Inode {
    // 1 索引等级,最小为 0,直接指向数据块,当当前等级的索引无法满足上限后将索引升一级,最高 255 级
    pub index_level: u8,
    pub extra: [u8; 9],
    pub mode: u16,
    pub link_count: u32,
    pub created: u64,
    pub modified: u64,
    pub size: u64,
    pub uid: u32,
    pub gid: u32,
    pub index_node: IndexNode,
}

impl Inode {
    pub fn new(mode: u16) -> Self {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        Self {
            index_level: 0,
            extra: [0u8; 9],
            mode,
            link_count: 1,
            created: since_the_epoch,
            modified: since_the_epoch,
            size: 0,
            uid: 0,
            gid: 0,
            index_node: Default::default(),
        }
    }
    pub fn nil() -> Self {
        Self {
            index_level: 0,
            extra: [0u8; 9],
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
        self.file_type() != FileType::UNK
    }
    pub fn is_dir(&self) -> bool {
        self.file_type() == FileType::Dir
    }
    pub fn file_type(&self) -> FileType {
        (self.mode >> 12).into()
    }
}

pub const INODE_SIZE: usize = size_of::<Inode>();

#[derive(Copy, Clone, Debug)]
pub struct InodeWithId {
    pub inode: usize,
    // 1 索引等级,最小为 0,直接指向数据块,当当前等级的索引无法满足上限后将索引升一级,最高 255 级
    pub data: Inode,
}

impl Inode {
    pub fn with_id(&self, ino: usize) -> InodeWithId {
        InodeWithId {
            inode: ino,
            data: self.clone(),
        }
    }
}

impl InodeWithId {
    pub fn file_type(&self) -> FileType {
        (self.data.mode >> 12).into()
    }
    pub fn blocks(&self) -> u64 {
        return (self.data.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;
    }
    pub fn permission(&self) -> u16 {
        self.data.mode & 0o777
    }
    pub fn inode(&self) -> &Inode {
        &self.data
    }
}
