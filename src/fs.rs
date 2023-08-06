use std::cmp::min;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::{fs::File, io::Write};

use bit_set::BitSet;
use prettytable::{row, Table};
use serde::Serialize;

use crate::meta::{FSFolder, FSMeta};
use crate::time::{timestamp, format_timestamp};

pub struct FS {
    pub file: File,
    pub size: u64,
    pub skip: u64,
}

impl FS {
    pub fn connect(path: &str) -> FS {
        let mut buffer = vec![0u8; 8];
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .expect("Unable to open filesystem");
        file.read_exact(&mut buffer).expect("Read Error");
        let size = u64::from_be_bytes(buffer.try_into().expect("Wrong Size")); // 读取文件系统大小
        let bitset_size = size / 8; // 计算 bitset 大小
        let skip = 8 + bitset_size; // 跳过文件系统大小和 bitset
        FS {
            file: file,
            size,
            skip,
        }
    }

    // 创建文件系统
    // 固定格式：
    // 8 字节：文件系统块数量
    // (块 / 8) 字节：文件块使用情况
    // 块 * 4KB：文件块
    // 第一个文件块存放根目录的节点信息
    pub fn mkfs(path: &str, size: u64) -> FS {
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .expect("Unable to open filesystem");
        let bitset_size = size / 8; // 计算 bitset 大小
        let skip = 8 + bitset_size; // 跳过文件系统大小和 bitset
                                    // println!("Skip:{}", skip);
        let mut buffer = vec![0u8; 8];
        size.to_be_bytes().iter().enumerate().for_each(|(i, b)| {
            buffer[i] = *b;
        });
        let fs = FS {
            file: file,
            size,
            skip,
        };
        let mut file = &fs.file;
        file.write_all(&buffer).expect("Unable to write");

        // 写入大小为 size 的 bitset
        let mut buffer = vec![0u8; size as usize / 8];
        buffer[0] = 0b10000000; // 第一个块被占用
        file.write_all(&buffer).expect("Unable to write");

        // 写入 size 个 4KB 空块，将第一个块作为根目录节点列表
        let buffer = vec![0u8; 4096];
        for _ in 0..size {
            file.write_all(&buffer).expect("Unable to write");
        }

        // 创建根目录节点
        let root = FSFolder { 0: Vec::new() };
        let mut buf = Vec::new();
        root.serialize(&mut rmps::Serializer::new(&mut buf).with_struct_map())
            .unwrap();
        // println!("Buffer:[{:?}]", buf);

        let block_ids = fs.write_blocks(buf);
        // 将 block_ids 写入第一个块
        let mut buffer = vec![0u8; 8];
        block_ids
            .iter()
            .map(|b| b.to_be_bytes())
            .flatten()
            .enumerate()
            .for_each(|(i, b)| {
                buffer[i] = b;
            });
        file.seek(SeekFrom::Start(skip)).expect("Unable to seek");
        file.write_all(&buffer).expect("Unable to write");
        fs
    }

    pub fn mkdir(&self, path: &str, name: &str) {

        // 寻找父目录
        let mut folder = self.ls_folder(path).expect("No such file or directory");
        // println!("Folder:[{:?}]", folder);
        let mut is_dir = false;
        // 检查是否存在同名文件
        if folder.0.iter().any(|f| {
            let result = f.name == name;
            if result{
                is_dir = f.is_dir;
            }
            result
        }) {
            println!("存在同名文件{}: {}", if is_dir { "夹" } else { "" },name);
            return;
        }

        let new_dir_data = FSFolder { 0: Vec::new() };
        let mut buf = Vec::new();
        // 序列化新目录节点
        new_dir_data
            .serialize(&mut rmps::Serializer::new(&mut buf).with_struct_map())
            .unwrap();
        // 写入
        // println!("Buffer:[{:?}]", buf);
        let block_ids = self.write_blocks(buf);
        // 创建新目录元数据
        let new_meta = FSMeta {
            name: String::from(name),
            is_dir: true,
            size: 0,
            created: timestamp(),
            modified: timestamp(),
            block_ids: block_ids,
        };
        let mut buf = Vec::new();
        // 序列化目录节点
        new_meta
            .serialize(&mut rmps::Serializer::new(&mut buf).with_struct_map())
            .unwrap();
        

        // 存入父目录节点列表
        folder.0.push(new_meta);

        // 更新父目录节点列表
        let mut buf = Vec::new();
        // println!("Folder:[{:?}]", folder);
        folder
            .serialize(&mut rmps::Serializer::new(&mut buf).with_struct_map())
            .unwrap();

        // println!("Buffer:[{:?}]", buf);
        self.update_file(path, buf);
    }

    pub fn ls(self, path: &str) {
        let folder = self.ls_folder(path).expect("No such file or directory");
        let mut table = Table::new();
        table.add_row(row!["名称", "类型", "实际大小","磁盘大小", "创建时间", "修改时间"]);
        folder.0.iter().for_each(|child| {
            table.add_row(row!(
                child.name,
                if child.is_dir { "文件夹" } else { "文件" },
                humanity_size(child.size),
                humanity_size(child.block_ids.len() as u64 * 4096),
                format_timestamp(child.created),
                format_timestamp(child.modified)
            ));
        });
        table.printstd();
    }

    // 读取文件系统中的文件/文件夹的块地址
    fn get_block_ids(&self, path: &str) -> Option<Vec<u64>> {
        let path = path.split("/").filter(|v| !v.is_empty());
        let mut file = &self.file;
        let mut buffer: Vec<u8> = vec![0u8; 4096];
        file.seek(SeekFrom::Start(self.skip)).expect("Seek Error"); // 跳过文件系统大小和 bitset
        file.read_exact(&mut buffer).expect("Read Error"); // 读取第一个块
                                                           // 将第一个块每 8 字节分割为一个节点
        let mut nodes = Vec::new();
        for i in 0..(4096 / 8) {
            let mut node = vec![0u8; 8];
            node.copy_from_slice(&buffer[i * 8..(i + 1) * 8]);
            let node = u64::from_be_bytes(node.try_into().expect("Wrong Size"));
            if node == 0 {
                continue;
            }
            nodes.push(node);
        }
        // 根节点
        let mut block_ids: Vec<u64> = nodes;
        for folder_name in path {
            let data = self.read_blocks(block_ids);
            let folder: FSFolder = rmps::from_slice(&data).unwrap();
            let data = folder
                .0
                .iter()
                .find(|child| child.name == folder_name && child.is_dir);
            match data {
                Some(child) => {
                    block_ids = child.block_ids.clone();
                }
                None => {
                    return None;
                }
            }
        }
        Some(block_ids)
    }

    pub fn ls_folder(&self, path: &str) -> Option<FSFolder> {
        let block_ids = self.get_block_ids(path).expect("No such file or directory");
        // println!("block_ids:[{:?}]", block_ids);
        let data = self.read_blocks(block_ids);
        let folder: FSFolder = rmps::from_slice(&data).unwrap();
        Some(folder)
    }

    fn read_blocks(&self, blocks: Vec<u64>) -> Vec<u8> {
        let mut file = &self.file;
        let mut buffer = vec![0u8; 4096];
        let mut data = Vec::new();
        blocks.iter().for_each(|block| {
            file.seek(SeekFrom::Start(block * 4096 + self.skip))
                .expect("Seek Error");
            file.read_exact(&mut buffer).expect("Read Error");
            data.append(&mut buffer);
        });

        // println!("Read data original:[{:?}]", data);
        if data.len() == 0 {
            return data;
        }
        // 删除末尾的 0
        let mut i = data.len() - 1; // 从后往前找
        while i > 0 && data[i] == 0 {
            i -= 1;
        }
        data.truncate(i + 1);
        // println!("Read data size:[{:?}]", data.len());
        return data;
    }

    fn update_file(&self, path: &str, buf: Vec<u8>) {
        let mut block_ids = self.get_block_ids(path).expect("No such file or directory"); // 获取原有文件节点
                                                                                  // println!("Update Old Block_ids:[{:?}]", block_ids);
        let mut file = &self.file;
        // 判断是否需要扩容或缩容
        let need_blocks = buf.len() / 4096 + 1; // 4KB * Blocks
        if block_ids.len() < need_blocks {
            // 扩容
            // println!("扩容 Need Blocks:[{}]", need_blocks);
            let new_blocks = self.alloc_new_blocks(need_blocks - block_ids.len());
            block_ids.append(&mut new_blocks.clone());
            let mut i = 0;
            while i < block_ids.len() {
                file.seek(SeekFrom::Start(new_blocks[i] * 4096 + self.skip))
                    .expect("Seek Error");
                file.write_all(&buf[i * 4096..min((i + 1) * 4096, buf.len())])
                    .expect("Write Error");
                i += 1;
            }
        } else {
            // 缩容或不变
            let (_, recycle_node) = block_ids.split_at(need_blocks);
            // println!("缩容 Need Blocks:[{}]", need_blocks);
            // println!("Recycle Node:[{:?}]", recycle_node);
            self.free_blocks(recycle_node.to_vec());
            block_ids.truncate(need_blocks);
            block_ids.iter().enumerate().for_each(|(i, block)| {
                let skip = *block * 4096 + self.skip;
                file.seek(SeekFrom::Start(skip)).expect("Seek Error");
                file.write_all(&buf[i * 4096..min((i + 1) * 4096, buf.len())])
                    .expect("Write Error");
            });
        }

        // 寻找父目录，更新父目录节点列表
        // 去除路径最后一个元素
        let child_path = Path::new(&path);
        let parent_path = child_path.parent();
        match parent_path {
            Some(parent_path) => {
                let parent_path_str = parent_path.to_str().expect("Wrong Path");
                let parent_block_ids = self
                    .get_block_ids(parent_path_str)
                    .expect("No such file or directory");
                let data = self.read_blocks(parent_block_ids);
                let mut folder: FSFolder = rmps::from_slice(&data).unwrap();
                let child = folder
                    .0
                    .iter_mut()
                    .find(|child| child.name == child_path.file_name().unwrap().to_str().unwrap())
                    .expect("No such file or directory");
                child.block_ids = block_ids;
                let data = rmps::to_vec(&folder).unwrap();
                self.update_file(parent_path_str, data);
            }
            None => {
                // 根目录
                if block_ids.len() * 8 > 4096 {
                    // 根目录满，报错
                    panic!("Root Folder is full");
                }
                let mut buffer = Vec::new();
                // 把 新的 block_ids 写入
                block_ids.iter().for_each(|block| {
                    // u64 转为 8 个 u8
                    let block_bytes = block.to_be_bytes();
                    buffer.append(&mut block_bytes.to_vec());
                });
                file.seek(SeekFrom::Start(self.skip as u64))
                    .expect("Seek Error");
                file.write_all(&buffer).expect("Write Error");
            }
        }
    }

    fn alloc_new_blocks(&self, need_size: usize) -> Vec<u64> {
        if need_size == 0 {
            return Vec::new();
        }
        let mut free_blocks = Vec::new();
        let mut buffer = vec![0u8; self.size as usize / 8];
        let mut file = &self.file;
        file.seek(SeekFrom::Start(8)).expect("Seek Error");
        file.read_exact(&mut buffer).expect("Read Error");
        // buffer 取反
        buffer.iter_mut().for_each(|byte| {
            *byte = !*byte;
        });
        let mut bitset = BitSet::from_bytes(&buffer);
        bitset.iter().for_each(|block| {
            free_blocks.push(block as u64);
        });
        if free_blocks.len() < need_size {
            panic!("No enough space");
        }
        free_blocks.truncate(need_size);
        // 更新 bitset
        free_blocks.iter().for_each(|block| {
            bitset.remove(*block as usize);
        });
        let mut buffer = bitset.into_bit_vec().to_bytes();
        buffer.iter_mut().for_each(|byte| {
            *byte = !*byte;
        });
        // println!("Alloc new blocks:{:?}", buffer);
        file.seek(SeekFrom::Start(8)).expect("Seek Error");
        file.write_all(&buffer).expect("Write Error");
        free_blocks
    }

    fn free_blocks(&self, blocks: Vec<u64>) {
        let mut buffer = vec![0u8; self.size as usize / 8];
        let mut file = &self.file;

        file.seek(SeekFrom::Start(8)).expect("Seek Error");
        file.read_exact(&mut buffer).expect("Read Error");
        buffer.iter_mut().for_each(|byte| {
            *byte = !*byte;
        });
        let mut bitset = BitSet::from_bytes(&buffer);
        blocks.iter().for_each(|block| {
            bitset.insert(*block as usize);
        });
        let mut buffer = bitset.into_bit_vec().to_bytes();
        buffer.iter_mut().for_each(|byte| {
            *byte = !*byte;
        });
        file.seek(SeekFrom::Start(8)).expect("Seek Error");
        file.write_all(&buffer).expect("Write Error");
    }

    // 写入数据块，返回块号
    fn write_blocks(&self, blocks: Vec<u8>) -> Vec<u64> {
        if blocks.len() == 0 {
            return Vec::new();
        }
        // 寻找空闲块
        let need_size = blocks.len() / 4096 + 1; // 4KB * Blocks
        if need_size == 0 {
            return Vec::new();
        }
        let free_blocks = self.alloc_new_blocks(need_size); // 分配空闲块
                                                            // println!("free blocks: {:?} -> {:?}", free_blocks, blocks);
        let mut file = &self.file;

        // 将数据写入块
        for i in 0..need_size {
            let start = i * 4096;
            let end = min((i + 1) * 4096, blocks.len());
            file.seek(SeekFrom::Start(free_blocks[i] * 4096 + self.skip))
                .expect("Seek Error");
            file.write_all(&blocks[start..end]).expect("Write Error");
        }
        free_blocks
    }
}

// 将字节大小转换为人类可读的大小
fn humanity_size(size: u64) -> String {
    let mut size = size as f64;
    let mut unit = "B";
    if size > 1024.0 {
        size /= 1024.0;
        unit = "KB";
    }
    if size > 1024.0 {
        size /= 1024.0;
        unit = "MB";
    }
    if size > 1024.0 {
        size /= 1024.0;
        unit = "GB";
    }
    if size > 1024.0 {
        size /= 1024.0;
        unit = "TB";
    }
    format!("{:.2}{}", size, unit)
}
