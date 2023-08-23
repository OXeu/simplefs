use crate::config::BLOCK_SIZE;
use crate::layout::bitmap::AllocError::{NoEnoughSpace, Nothing};
use crate::layout::super_block::SuperBlock;
use crate::manager::block_cache_manager::BlockCacheDevice;

type BitmapBlock = [u8; BLOCK_SIZE];

#[derive(Eq, PartialEq)]
pub enum AllocError {
    NoEnoughSpace,
    Nothing,
}

impl BlockCacheDevice {
    /// @return Option<usize> data_block_id
    /// 返回逻辑地址
    pub fn alloc_block(&mut self, is_inode: bool) -> Option<usize> {
        let mut super_block = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        // println!("Alloc Super Block: {:?}", super_block);
        let bitmap_blocks = super_block.bitmap_blocks;
        let data_end_blk = super_block.data_block_last - bitmap_blocks - 1;
        let inode_end_blk = super_block.inode_block_last - bitmap_blocks - 1;
        let inode_start_blk = super_block.inode_block_first - bitmap_blocks - 1;
        let free = inode_end_blk - data_end_blk;
        let range: Box<dyn Iterator<Item=usize>> = if is_inode { Box::new((inode_end_blk..=inode_start_blk).rev()) } else { Box::new(0..=data_end_blk) };
        // println!("Range: {}..={}", if is_inode { inode_start_blk } else { 0 }, if is_inode { inode_end_blk } else { data_end_blk + 1 });
        for blk_id in range {
            if !self.used(blk_id) {
                let inode_id = if is_inode { inode_start_blk - blk_id } else { blk_id };
                // println!("Alloc {}: {},{}", if is_inode { "Inode" } else { "Data" }, inode_id, blk_id);
                self.set(blk_id, true);
                return Some(inode_id);
            }
        }
        // 没有回收的块，分配新块
        if free > 0 {
            let range: Box<dyn Iterator<Item=usize>> = if is_inode {
                Box::new((data_end_blk..inode_end_blk).rev())
            } else {
                Box::new(data_end_blk + 1..inode_end_blk)
            };
            for blk_id in range {
                if !self.used(blk_id) {
                    let inode_id = if is_inode { inode_start_blk - blk_id } else { blk_id };
                    // println!("Alloc {}: {},{}", if is_inode { "Inode" } else { "Data" }, inode_id, blk_id);
                    // 更新 Super Block
                    if is_inode {
                        self.super_block
                            .lock()
                            .unwrap()
                            .modify(0, |sb: &mut SuperBlock| {
                                sb.inode_block_last = blk_id + bitmap_blocks + 1
                            });
                    } else {
                        self.super_block
                            .lock()
                            .unwrap()
                            .modify(0, |sb: &mut SuperBlock| {
                                sb.data_block_last = blk_id + bitmap_blocks + 1
                            });
                    }
                    self.set(blk_id, true);
                    return Some(inode_id);
                }
            }
        }
        None
    }

    pub fn free_block(&mut self, id: usize, is_inode: bool) {
        let mut super_block = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        // let phy_id = super_block.data_block_id(id);
        let actual_id = if is_inode {
            super_block.inode_block_last - id
        } else {
            id
        };
        if self.used(actual_id) {
            self.set(actual_id, false);
            // 不对物理块清理
            return;
        }
        // 重复释放
        println!("Try to release the free {} block({})!", if is_inode { "Inode" } else { "Data" }, id)
    }

    pub fn used(&mut self, data_blk_id: usize) -> bool {
        let bitmap_blk_id = data_blk_id / (BLOCK_SIZE * 8);
        let bitmap_offset = data_blk_id % (BLOCK_SIZE * 8);
        let bitmap_offset_u8 = bitmap_offset / 8;
        let bitmap_offset_bit = bitmap_offset % 8;
        let mut super_block = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        // 边界检查
        if bitmap_blk_id > super_block.bitmap_blocks {
            println!("{} / {}", bitmap_blk_id, super_block.bitmap_blocks);
            panic!("out of boundaty")
        }
        let mut check = false;
        self.block_cache(bitmap_blk_id + 1)
            .lock()
            .unwrap()
            .read(bitmap_offset_u8, |byte: &u8| {
                check = byte & 1 << bitmap_offset_bit > 0
            });
        check
    }

    fn set(&mut self, data_blk_id: usize, v: bool) {
        let bitmap_blk_id = data_blk_id / (BLOCK_SIZE * 8);
        let bitmap_offset = data_blk_id % (BLOCK_SIZE * 8);
        let bitmap_offset_u8 = bitmap_offset / 8;
        let bitmap_offset_bit = bitmap_offset % 8;
        let mut super_block = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        // 边界检查
        if bitmap_blk_id > super_block.bitmap_blocks {
            println!("{} / {}", bitmap_blk_id, super_block.bitmap_blocks);
            panic!("out of boundaty")
        }
        self.block_cache(bitmap_blk_id + 1)
            .lock()
            .unwrap()
            .modify(bitmap_offset_u8, |byte: &mut u8| {
                if v {
                    *byte |= 1 << bitmap_offset_bit
                } else {
                    *byte &= !(1 << bitmap_offset_bit)
                }
            });
    }

    fn clear(&mut self) {
        let mut super_block = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        let bitmap_blocks = super_block.bitmap_blocks;
        for blk_id in 0..bitmap_blocks {
            self.block_cache(blk_id + 1)
                .lock()
                .unwrap()
                .modify(0, |bytes: &mut BitmapBlock| {
                    bytes.iter_mut().for_each(|v| {
                        *v = 0;
                    })
                })
        }
    }

    pub fn print(&mut self) {
        let mut super_block = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        let bitmap_blocks = super_block.bitmap_blocks;
        let data_end_blk = super_block.data_block_last;
        let inode_end_blk = super_block.inode_block_last;
        let inode_start_blk = super_block.inode_block_first;
        let free = inode_end_blk - data_end_blk;
        let mut used = Vec::new();
        for blk_id in 0..data_end_blk - bitmap_blocks {
            if self.used(blk_id) {
                used.push(blk_id)
            }
        }
        // println!("Super Block: {:?}", super_block);
        for blk_id in inode_end_blk..=inode_start_blk {
            if self.used(blk_id - bitmap_blocks - 1) {
                used.push(blk_id)
            }
        }
        println!("Used Blocks: {:#?}", used);
    }
}
