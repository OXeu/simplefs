use std::io::SeekFrom;
use std::sync::{Arc, Mutex};

use libc::{c_int, ENOSPC, O_APPEND, O_TRUNC};

use crate::cache::block_cache::CacheBlock;
use crate::config::BLOCK_SIZE;
use crate::layout::inode::{Inode, InodeWithId};
use crate::manager::block_cache_manager::BlockCacheDevice;
use crate::manager::error_code::{ENOENT, ErrorCode};


pub struct FileHandler {
    inode_id: usize,
    inode_block: Arc<Mutex<CacheBlock>>,
    inode_offset: usize,
    offset: usize,
    flags: i32,
}

impl Clone for FileHandler {
    fn clone(&self) -> Self {
        Self {
            inode_id: self.inode_id,
            inode_block: self.inode_block.clone(),
            inode_offset: self.inode_offset,
            offset: self.offset,
            flags: self.flags,
        }
    }
}

impl FileHandler {
    pub fn new(
        inode_id: usize,
        device: &mut BlockCacheDevice,
        offset: usize,
        flags: i32,
    ) -> Result<Self, ErrorCode> {
        let (blk_id, inode_offset) = device.inode_block(inode_id);
        let blk = device.block_cache(blk_id);
        let ino = device.inode(inode_id);
        if !ino.exist() {
            return Err(ENOENT);
        }
        let fh = Self {
            inode_id,
            inode_block: blk,
            inode_offset,
            offset,
            flags,
        };
        if (flags & O_TRUNC) > 0 {
            if let Err(e) = device.write_system(0, &fh.inode_with_id(), &mut vec![], true) {
                return Err(e);
            }
        }
        Ok(fh)
    }
}

impl FileHandler {
    pub fn inode_with_id(&self) -> InodeWithId {
        self.inode_block.lock().unwrap().read(self.inode_offset, |i: &Inode| i.with_id(self.inode_id)).clone()
    }

    pub fn write(&mut self, device: &mut BlockCacheDevice, buf: &[u8]) -> Result<usize, c_int> {
        let inode_with_id = &self.inode_with_id();
        let offset = if self.is_append() {
            inode_with_id.data.size as usize
        } else {
            self.offset
        };
        device.write_internal(offset, inode_with_id, buf)
    }

    pub fn seek(&mut self, offset: SeekFrom) {
        match offset {
            SeekFrom::Start(v) => self.offset = v as usize,
            SeekFrom::End(v) => self.offset = (self.inode_with_id().data.size as i64 + v) as usize,
            SeekFrom::Current(v) => self.offset = (self.offset as i64 + v) as usize
        }
    }

    pub fn read(&mut self, device: &mut BlockCacheDevice, buf: &mut [u8]) -> usize {
        device.read_internal(self, buf)
    }

    pub fn read_block<T, V>(&mut self, device: &mut BlockCacheDevice, blk_id: usize, offset: usize, f: impl FnOnce(&T) -> V) -> Option<V> {
        let data = device.inode_data_blk_list(&self.inode_with_id().data);
        println!("read block {}, but {:?}", blk_id, data);
        if blk_id >= data.len() { return None }
        Some(device.block_cache(device.data_block(data[blk_id]))
            .lock()
            .unwrap()
            .read(offset, f))
    }

    fn is_append(&self) -> bool {
        (self.flags & O_APPEND) > 0
    }

    pub fn flush(&self, device: &mut BlockCacheDevice) {
        device.flush_internal(&self.inode_with_id())
    }
}

impl BlockCacheDevice {
    pub fn write_internal(
        &mut self,
        offset: usize,
        inode: &InodeWithId,
        buf: &[u8],
    ) -> Result<usize, c_int> {
        self.write_system(offset, inode, buf, false)
    }

    /// truncate: 是否根据 buf 和 offset 重新调整大小
    /// 内部使用，外部进程使用 fh 读写
    pub(crate) fn write_system(
        &mut self,
        offset: usize,
        inode_with_id: &InodeWithId,
        buf: &[u8],
        truncate: bool,
    ) -> Result<usize, ErrorCode> {
        let mut data = self.inode_data_blk_list(inode_with_id.inode());
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
        } else if need_blk < data.len() && truncate {
            for _ in need_blk..data.len() {
                self.free_block(data.pop().unwrap(), false, true);
            }
        }
        let offset_part = offset % BLOCK_SIZE;
        let mut offset_mut = offset;
        let write_times = (offset_part + len + BLOCK_SIZE - 1) / BLOCK_SIZE;
        for i in 0..write_times {
            let offset = offset_mut % BLOCK_SIZE;
            let end = BLOCK_SIZE.min(len - i * BLOCK_SIZE + offset);
            // debug!("Verbose: {}..{},{},{},{}", offset, end, len, i * BLOCK_SIZE, len.min((i + 1) * BLOCK_SIZE));
            // 需要写几块
            let length = end - offset;
            self.modify_data(data[blk + i], |data: &mut [u8; BLOCK_SIZE]| {
                data[offset..end]
                    .copy_from_slice(&buf[i * BLOCK_SIZE..len.min((i + 1) * BLOCK_SIZE)]);
            });
            offset_mut += length
        }
        self.modify_inode(inode_with_id.inode, |ino| {
            ino.size = if truncate {
                offset_mut as u64
            } else {
                (offset_mut as u64).max(ino.size)
            }
        });
        self.make_index_part(inode_with_id, data, 0).map(|_| offset_mut)
    }

    pub fn read_internal(&mut self, fh: &mut FileHandler, buf: &mut [u8]) -> usize {
        let data = self.inode_data_blk_list(&fh.inode_with_id().inode());
        let start_offset = fh.offset;
        let blk = start_offset / BLOCK_SIZE;
        let off = start_offset % BLOCK_SIZE;
        let len = buf.len();
        let read_times = (data.len() - blk).min((off + len + BLOCK_SIZE - 1) / BLOCK_SIZE);
        for i in 0..read_times {
            let offset = fh.offset % BLOCK_SIZE;
            let end = BLOCK_SIZE.min(buf.len() - offset);
            // 需要读几块
            self.data(data[blk + i], 0, |data: &[u8; BLOCK_SIZE]| {
                buf[i * BLOCK_SIZE..len.min((i + 1) * BLOCK_SIZE)]
                    .copy_from_slice(&data[offset..end]);
            });
            let length = end - offset;
            fh.offset += length
        }
        fh.offset - start_offset
    }

}
