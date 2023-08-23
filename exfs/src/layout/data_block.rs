use std::mem::size_of;

use crate::config::BLOCK_SIZE;

pub type DataBlock = [u8; BLOCK_SIZE];

/// 目录项 64 字节
/// name: 文件名, 56字节以内
/// inode: Inode 号
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DirEntry {
    pub name: FileName, // 56 bytes
    pub inode: u64,     // 8 bytes
}

pub type FileName = [u8; 56];

pub const DIR_ENTRY_SIZE: usize = size_of::<DirEntry>();
