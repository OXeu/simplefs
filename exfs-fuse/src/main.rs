use std::env;
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};

use exfs::block_device::file_device::FileDevice;
use exfs::manager::block_cache_manager::BlockCacheDevice;
use fuser::MountOption;

fn main() {
    println!("Hello, world!");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        // .create(true)
        // .truncate(true)
        .open("fs.img")
        .unwrap();
    file.set_len(1024 * 4096 * 2).unwrap();
    let mut fs = BlockCacheDevice::new(Arc::new(FileDevice { file: Arc::new(Mutex::new(file)) }));
    fs.mkfs(1024);
    fs.print();
    env_logger::init();
    let mountpoint = env::args_os().nth(1).unwrap();
    println!("mount point: {:?}", mountpoint);
    // let options = ["-o", "fsname=exfs"]
    //     .iter()
    //     .map(|o| o.as_ref())
    //     .collect::<Vec<&OsStr>>();
    let mut options = vec![MountOption::RW, MountOption::FSName("hello".to_string())];
    options.push(MountOption::AutoUnmount);
    // options.push(MountOption::AllowRoot);
    fuser::mount2(fs, &mountpoint, &options).unwrap();
}


#[test]
fn mkfs() {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open("fs.img")
        .unwrap();
    file.set_len(1024 * 4096 * 2).unwrap();
    let mut fs = BlockCacheDevice::new(Arc::new(FileDevice { file: Arc::new(Mutex::new(file)) }));
    fs.mkfs(1024);
    fs.print();
}