#![cfg(test)]

use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};

use crate::block_device::file_device::FileDevice;
use crate::layout::inode::DIR;
use crate::manager::block_cache_manager::BlockCacheDevice;

#[test]
fn test() {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("fs.img")
        .unwrap();
    let mut fs = BlockCacheDevice::new(Arc::new(FileDevice { file: Arc::new(Mutex::new(file)) }));
    fs.mkfs(1024);
    ls(&mut fs, "/");
    fs.mk_file("hello", 0, DIR << 12).unwrap();
    ls(&mut fs, "/");
    fs.mk_file("hello2", 0, DIR << 12).unwrap();
    ls(&mut fs, "/");
    fs.mk_file("hello3333", 1, DIR << 12).unwrap();
    ls(&mut fs, "/hello");
    fs.mk_file("hello44444", 1, DIR << 12).unwrap();
    ls(&mut fs, "/hello");
    ls(&mut fs, "/hello/hello3333");
}

fn ls(fs: &mut BlockCacheDevice, path: &str) {
    println!("ls: {}", path);
    match fs.ls(path) {
        Ok(iter) => {
            if iter.len() == 0 {
                println!("[Empty]")
            } else {
                iter.iter().for_each(|v| println!("{}    {:?}     {}", v.name, v.inode.file_type(), v.inode_id));
            }
        }
        Err(e) => println!("[error] {:?}",e)
    }
}

// 有个问题,为什么 inode 是 6 -> 4 -> 2