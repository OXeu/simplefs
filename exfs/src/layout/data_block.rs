use std::mem::size_of;

use crate::config::BLOCK_SIZE;
use crate::typ::file_name::FileName;

pub type DataBlock = [u8; BLOCK_SIZE];

/// 目录项 64 字节
/// name: 文件名, 56字节以内
/// inode: Inode 号
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DirEntry {
    pub name: FileName,
    // 56 bytes
    pub inode: u64,     // 8 bytes
}

impl DirEntry {
    pub fn valid(&self) -> bool {
        // println!("Entry valid:{:?}", self);
        !self.name.is_empty() && self.inode != 0
    }
}

pub const DIR_ENTRY_SIZE: usize = size_of::<DirEntry>();
