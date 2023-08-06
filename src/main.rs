use simplefs::fs::{FS, humanity_size};

const MB_BLOCK: u64 = 256; // 4KB * 256 = 1MB
fn main() {
    // println!("Hello, world!{}", 8 * MB_BLOCK);
    let fs = FS::connect("test.fs");
    // let fs = FS::mkfs("test.fs", 3 * 1024 * MB_BLOCK);
    fs.ls("/");
    // fs.mkdir("/", "hello");
    // fs.mkdir("/", "ash");
    // fs.mkdir("/", "mock");
    // fs.mkdir("/", "mock");
    // fs.ls("/");
    // fs.ls("/mock");
    // fs.mkdir("/mock", "mock2");
    // fs.ls("/mock");

    // let content = "hello world".as_bytes();
    // fs.write("/", "hello", content);
    // fs.write("/", "hello.txt", content);
    // println!("{}",fs.read("/hello").string());
    // println!("{}",fs.read("/hello.txt").string());
    fs.ls("/");
    let new_content = r#"RMP is a pure Rust MessagePack implementation of an efficient binary serialization format. This crate provides low-level core functionality, writers and readers for primitive values with direct mapping between binary MessagePack format.
    RMP 是高效二进制序列化格式的纯 Rust MessagePack 实现。该包提供低级核心功能、原始值的编写器和读取器以及二进制 MessagePack 格式之间的直接映射。"#.repeat(1024);
    println!("Size:{}", humanity_size(new_content.len() as u64));
    fs.write("/", "hello.txt", new_content.as_bytes());
    // println!("{}",fs.read("/hello.txt").string());
    fs.ls("/");
}