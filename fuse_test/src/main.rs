use std::env;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::mem::size_of;
use std::sync::{Arc, Mutex};
use exfs::block_device::file_device::FileDevice;
use exfs::manager::block_cache_manager::BlockCacheDevice;

fn main() {
    println!("Hello, world!");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("fs.img")
        .unwrap();
    let mut fs = BlockCacheDevice::new(Arc::new(FileDevice { file:Arc::new(Mutex::new(file)) }));
    fs.mkfs(102400);
    fs.print();

    env_logger::init();
    let mountpoint = env::args_os().nth(1).unwrap();
    println!("mount point: {:?}",mountpoint);
    let options = ["-o", "fsname=exfs"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(fs, &mountpoint, &options).unwrap();
}


#[repr(packed)]
#[derive(Default, Copy, Clone, Debug)]
pub struct Inode {
    // 64 bytes
    pub index_level: u8, // 1 索引等级,最小为 0,直接指向数据块,当当前等级的索引无法满足上限后将索引升一级,最高 255 级
    pub extra: [u8; 9],  // 9
    pub mode: u16,       // 2 bytes,
    pub link_count: u32, // 4
    pub created: u64,    // 8
    pub modified: u64,   // 8
    pub size: u64,       // 8
    pub uid: u32,        // 8
    pub gid: u32,        // 8
    pub index_node: IndexNode, // 16,索引地址
}
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct IndexNode {
    start_blk: usize, // inclusive
    len: usize,       // exclusive
}

pub const INDEX_NODE_SIZE: usize = size_of::<IndexNode>();
pub const INODE_SIZE: usize = size_of::<Inode>();

#[test]
fn test(){
    assert_eq!(INODE_SIZE,64);
    assert_eq!(INDEX_NODE_SIZE,16);
}