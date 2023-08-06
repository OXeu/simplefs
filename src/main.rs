use crate::fs::FS;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rmp_serde as rmps;
mod fs;
mod meta;
mod time;
const MB_BLOCK: u64 = 256; // 4KB * 256 = 1MB
fn main() {
    // println!("Hello, world!{}", 8 * MB_BLOCK);
    // let fs = FS::connect("test.fs");
    let fs = FS::mkfs("test.fs",1*MB_BLOCK);
    fs.mkdir("/", "hello");
    fs.mkdir("/", "ash");
    fs.mkdir("/", "mock");
    fs.mkdir("/", "mock");
    fs.ls("/");
}
