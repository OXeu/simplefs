# SimpleFS
## 格式化
```rust
let fs = FS::mkfs("test.fs",1 * MB_BLOCK);
```
## 连接已有文件系统
```rust
let fs = FS::connect("test.fs");
```

## 创建文件夹
```rust
fs.mkdir("/hello", "ash");
```

## 文件系统组织形式
1. 前 8 字节为文件系统块数量(单个块大小为 4KB)  
2. 随后为 块数量 / 8 字节的bitset，用于标记块是否被使用  
3. 后续为文件系统的所有块，每 4KB 为一个块  


块 0 规定存储 根目录 / 的 block_ids，每8个字节为一个 u64 id  
初始化时默认创建 / 文件夹，存储在块 1  
文件夹也是文件，内部存储子文件的元数据(FSMeta)列表  
通过元数据FSMeta 中的 is_dir 字段判断是否为文件夹
