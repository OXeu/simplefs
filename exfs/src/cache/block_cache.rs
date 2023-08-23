// 物理块缓存层

use crate::block_device::block_device::BlockDevice;
use crate::config::BLOCK_SIZE;
use crate::manager::block_cache_manager::trim_zero;
use libc::sync;
use std::any::Any;
use std::any::TypeId;
use std::fmt::Debug;
use std::mem::size_of;
use std::sync::Arc;
use crate::utils::slice::SliceExt;

pub struct CacheBlock {
    block: usize,
    data: [u8; BLOCK_SIZE],
    device: Arc<dyn BlockDevice>,
    dirty: bool,
}

impl CacheBlock {
    pub fn new(device: Arc<dyn BlockDevice>, block: usize) -> Self {
        let mut buf = [0u8; BLOCK_SIZE];
        device.read(block, &mut buf);
        Self {
            block,
            data: buf,
            device,
            dirty: false,
        }
    }

    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.data[offset] as *const _ as usize
    }
    fn get_ref<T: Sized>(&self, offset: usize) -> &T {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SIZE);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }

    pub fn get_mut<T: Sized>(&mut self, offset: usize) -> &mut T {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SIZE);
        self.dirty = true;
        self.sync();
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }
}

impl CacheBlock {
    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    pub fn modify<T: Debug + 'static, V>(
        &mut self,
        offset: usize,
        f: impl FnOnce(&mut T) -> V,
    ) -> V {
        let blk = self.block;
        let data: &mut T = self.get_mut(offset);
        let data_slice =
            unsafe { std::slice::from_raw_parts(data as *const T as *const u8, size_of::<T>()) };
        let data_mm = data_slice.trim();
        if data_slice.len() < BLOCK_SIZE {
            println!("({}->{})【Before】{:?}", blk, offset, &data);
        } else if data_mm.len() > 0 {
            // let d = unsafe { &mut *(data_mm.as_ptr() as *mut T) };
            println!("({}->{})【Before】{:?}", blk, offset, data_mm);
        }
        let v = f(data);
        let data_mm = data_slice.trim();
        if data_slice.len() < BLOCK_SIZE {
            println!("({}->{})【After】{:?}", blk, offset, &data);
        } else if data_mm.len() > 0 {
            // let d = unsafe { &mut *(data_mm.as_ptr() as *mut T) };
            println!("({}->{})【After】{:?}", blk, offset, data_mm);
        }
        self.sync(); // 关缓存
        v
    }
    pub fn free(&mut self) {
        self.modify(0, |data: &mut [u8; BLOCK_SIZE]| {
            for byte in data.iter_mut() {
                *byte = 0;
            }
        });
        self.device.write(self.block, &self.data)
    }

    pub fn sync(&mut self) {
        if self.dirty {
            self.dirty = false;
            self.device.write(self.block, &self.data)
        }
    }
}

impl Drop for CacheBlock {
    fn drop(&mut self) {
        self.sync()
    }
}
