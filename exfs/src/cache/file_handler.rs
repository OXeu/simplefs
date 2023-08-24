use std::sync::{Arc, Mutex};

use libc::{c_int, ENOSPC};

use crate::config::BLOCK_SIZE;
use crate::layout::inode::Inode;
use crate::manager::block_cache_manager::BlockCacheDevice;

#[derive(Debug, Copy, Clone)]
pub struct FileHandler {
    pub inode: usize,
    pub offset: usize,
    pub flags: u16,
}

impl FileHandler {
    pub fn inode(&self, device: Arc<Mutex<BlockCacheDevice>>) -> Inode {
        device.lock().unwrap().inode(self.inode)
    }

    pub fn write(&mut self, device: &mut BlockCacheDevice, buf: &[u8]) -> Result<(), c_int> {
        device.write_inner(self.offset, self.inode, buf)
    }

    pub fn read(&mut self, device: &mut BlockCacheDevice, buf: &mut [u8]) -> usize {
        let data = device.inode_data_blk_list(self.inode);
        let start_offset = self.offset;
        let blk = start_offset / BLOCK_SIZE;
        let off = start_offset % BLOCK_SIZE;
        let len = buf.len();
        let read_times = (data.len() - blk).min((off + len + BLOCK_SIZE - 1) / BLOCK_SIZE);
        for i in 0..read_times {
            let offset = self.offset % BLOCK_SIZE;
            let end = BLOCK_SIZE.min(buf.len() - offset);
            // 需要写几块
            device.data(data[blk + i], 0, |data: &[u8; BLOCK_SIZE]| {
                buf[i * BLOCK_SIZE..len.min((i + 1) * BLOCK_SIZE)]
                    .copy_from_slice(&data[offset..end]);
            });
            let length = end - offset;
            self.offset += length
        }
        self.offset - start_offset
    }
}

impl BlockCacheDevice {
    pub fn write_inner(&mut self, offset: usize, inode: usize, buf: &[u8]) -> Result<(), c_int> {
        let mut data = self.inode_data_blk_list(inode);
        let blk = offset / BLOCK_SIZE;
        let len = buf.len();
        let need_blk = (offset + len + BLOCK_SIZE - 1) / BLOCK_SIZE;
        if need_blk > data.len() {
            // 申请空间
            for _ in data.len()..need_blk {
                match self.alloc_block(false) {
                    None => return Err(ENOSPC),
                    Some(data_id) => data.push(data_id),
                }
            }
        }
        let offset_part = offset % BLOCK_SIZE;
        let mut offset_mut = offset;
        let write_times = (offset_part + len + BLOCK_SIZE - 1) / BLOCK_SIZE;
        for i in 0..write_times {
            let offset = offset_mut % BLOCK_SIZE;
            let end = BLOCK_SIZE.min(len - i * BLOCK_SIZE - offset);
            // println!("Verbose: {}..{},{},{},{}", offset, end, len, i * BLOCK_SIZE, len.min((i + 1) * BLOCK_SIZE));
            // 需要写几块
            let length = end - offset;
            self.modify_data(data[blk + i], |data: &mut [u8; BLOCK_SIZE]| {
                data[offset..end]
                    .copy_from_slice(&buf[i * BLOCK_SIZE..len.min((i + 1) * BLOCK_SIZE)]);
            });
            offset_mut += length
        }
        self.make_index_part(inode, data, 0)
    }
    // 末尾直接截断
    pub fn write_all(&mut self, offset: usize, inode: usize, buf: &[u8]) -> Result<(), c_int> {
        let mut data = self.inode_data_blk_list(inode);
        let blk = offset / BLOCK_SIZE;
        let len = buf.len();
        let need_blk = (offset + len + BLOCK_SIZE - 1) / BLOCK_SIZE;
        if need_blk > data.len() {
            // 申请空间
            for _ in data.len()..need_blk {
                match self.alloc_block(false) {
                    None => return Err(ENOSPC),
                    Some(data_id) => data.push(data_id),
                }
            }
        } else if need_blk < data.len() {
            for _ in need_blk..data.len() {
                self.free_block(data.pop().unwrap(),false,true);
            }
        }
        let offset_part = offset % BLOCK_SIZE;
        let mut offset_mut = offset;
        let write_times = (offset_part + len + BLOCK_SIZE - 1) / BLOCK_SIZE;
        for i in 0..write_times {
            let offset = offset_mut % BLOCK_SIZE;
            let end = BLOCK_SIZE.min(len - i * BLOCK_SIZE - offset);
            // println!("Verbose: {}..{},{},{},{}", offset, end, len, i * BLOCK_SIZE, len.min((i + 1) * BLOCK_SIZE));
            // 需要写几块
            let length = end - offset;
            self.modify_data(data[blk + i], |data: &mut [u8; BLOCK_SIZE]| {
                data[offset..end]
                    .copy_from_slice(&buf[i * BLOCK_SIZE..len.min((i + 1) * BLOCK_SIZE)]);
            });
            offset_mut += length
        }
        self.make_index_part(inode, data, 0)
    }
}
