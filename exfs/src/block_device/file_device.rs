use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::io::Read;
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
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start((block * BLOCK_SIZE) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.read(buf).unwrap(), BLOCK_SIZE, "Not a complete block!");
    }

    fn write(&self, block: usize, buf: &[u8]) {
        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start((block * BLOCK_SIZE) as u64))
            .expect("Error when seeking!");
        assert_eq!(file.write(buf).unwrap(), BLOCK_SIZE, "Not a complete block!");
    }

    fn sync(&self) {
        let _ = self.file.lock().unwrap().sync_all();
    }
}
