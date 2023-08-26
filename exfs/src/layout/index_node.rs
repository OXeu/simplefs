use std::mem::size_of;
use crate::config::BLOCK_SIZE;
use crate::manager::block_cache_manager::BlockCacheDevice;

/// 多级索引项
/// 8 bytes / 16 bytes
/// 每个文件快可存储 512 / 256 索引项
/// 每个索引块至少可表示一个文件块
/// 至少可表示 256^n * 4KB 大小的文件(理论无上限,只需要扩充 index_level 级数)
/// 但是n级索引占用空间为 256^(n-1) + ... + 256^1 + 1 ≈ 256 ^ (n-1) * 4KB
/// 即索引会最多占用文件大小的 1 / 256 ≈ 0.39 %, 在可接受范围内
/// 主要受制于文件系统碎片化程度与文件系统大小
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct IndexNode {
    start_blk: usize,
    // inclusive
    len: usize, // exclusive
}


pub const INDEX_NODE_SIZE: usize = size_of::<IndexNode>();

impl IndexNode {
    pub fn is_valid(&self) -> bool {
        self.len != 0
    }
    pub fn list(&self, device: &mut BlockCacheDevice, level: u8) -> Vec<usize> {
        let mut vec = Vec::new();
        for blk_id in self.start_blk..(self.start_blk + self.len) {
            // debug!("blk_id {}", blk_id);
            if level <= 1 {
                // 一级索引直接将块 id 返回
                vec.push(blk_id)
            } else {
                device.block_cache(blk_id).lock().unwrap().read(
                    0,
                    |data: &[IndexNode; BLOCK_SIZE / INDEX_NODE_SIZE]| {
                        data.iter().for_each(|v| {
                            // vec.push(v.clone());
                            v.list(device, level - 1)
                                .iter()
                                .for_each(|data| vec.push(data.clone()));
                        })
                    },
                )
            }
        }
        vec
    }
    pub fn list_level_blk(&self, device: &mut BlockCacheDevice, level: u8, need: u8) -> Vec<usize> {
        let mut vec = Vec::new();
        for blk_id in self.start_blk..(self.start_blk + self.len) {
            if level <= need + 1 {
                // 所需级索引直接将块 id 返回
                if blk_id != 0 {
                    vec.push(blk_id)
                }
            } else {
                device.block_cache(blk_id).lock().unwrap().read(
                    0,
                    |data: &[IndexNode; BLOCK_SIZE / INDEX_NODE_SIZE]| {
                        data.iter().for_each(|v| {
                            // vec.push(v.clone());
                            v.list(device, level - 1)
                                .iter()
                                .for_each(|data| vec.push(data.clone()));
                        })
                    },
                )
            }
        }
        vec
    }
    /// 删除当前索引节点以及下属索引节点
    /// keep_data: 是否保留 DataBlock
    pub fn delete(&self, device: &mut BlockCacheDevice, level: u8, keep_data: bool) {
        for blk_id in self.start_blk..(self.start_blk + self.len) {
            // debug!("{},{}->{}",blk_id,self.start_blk,self.start_blk +self.len);
            if level <= 1 {
                // 数据块 id
                if !keep_data {
                    // 删除数据块
                    device
                        .block_cache(device.data_block(blk_id))
                        .lock()
                        .unwrap()
                        .free();
                    device.free_block(blk_id, false, true);
                }
            } else {
                device
                    .block_cache(device.data_block(blk_id))
                    .lock()
                    .unwrap()
                    .modify(0, |data: &mut [IndexNode; BLOCK_SIZE / INDEX_NODE_SIZE]| {
                        data.iter_mut().for_each(|v| {
                            // vec.push(v.clone());
                            if v.is_valid() {
                                v.delete(device, level - 1, keep_data);
                            }
                            *v = IndexNode::default();
                        });
                    });
                device.free_block(blk_id, false, true)
            }
        }
    }
}

impl IndexNode {
    /// 给定数据块 id,生成紧凑的 IndexNode 列表
    pub fn from(blocks: Vec<usize>) -> Vec<Self> {
        let mut value = blocks;
        let mut nodes = Vec::new();
        value.sort();
        let mut last_id = 0;
        let mut node = IndexNode::default();
        value.iter().enumerate().for_each(|(id, v)| {
            if v - last_id == 1 && id != 0 {
                // 连续,长度 ＋1
                node.len += 1;
            } else {
                // 不连续,将之前的推进去,并创建新节点
                if node.len != 0 {
                    nodes.push(node.clone());
                }
                node.start_blk = *v;
                node.len = 1;
            }
            last_id = *v;
        });
        if node.len != 0 {
            nodes.push(node.clone());
        }
        nodes
    }
}
