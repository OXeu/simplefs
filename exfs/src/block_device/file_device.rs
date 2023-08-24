use std::fs::File;
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::sync::{Arc, Mutex};

use crate::block_device::block_device::BlockDevice;
use crate::config::BLOCK_SIZE;

pub struct FileDevice {
    pub file: Arc<Mutex<File>>,
}

impl BlockDevice for FileDevice {
    fn id(&self) -> usize {
        return 0x92101221;
    }

    fn read(&self, block: usize, buf: &mut [u8]) {
        if block > 1000000 {
            println!("block:{}", block);
        }
        self.file
            .lock()
            .unwrap()
            .read_at(buf, block as u64 * BLOCK_SIZE as u64)
            .unwrap();
    }

    fn write(&self, block: usize, data: &[u8]) {
        self.file
            .lock()
            .unwrap()
            .write_at(data, block as u64 * BLOCK_SIZE as u64)
            .unwrap();
        let _= self.file.lock().unwrap().flush();
    }
}
