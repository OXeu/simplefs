use std::ops::Range;

use crate::config::BLOCK_SIZE;
use crate::layout::inode::Inode;
use crate::layout::super_block::SuperBlock;
use crate::manager::block_cache_manager::BlockCacheDevice;

#[derive(Eq, PartialEq)]
pub enum AllocError {
    NoEnoughSpace,
    Nothing,
}

impl BlockCacheDevice {
    /// @return Option<usize> data_block_id
    /// 返回逻辑地址
    pub fn alloc_block(&mut self, is_inode: bool) -> Option<usize> {
        let super_block = self.super_block();
        let size = if is_inode { super_block.inode_blocks } else { super_block.data_blocks };
        for index in 0..size {
            if !self.used(index, is_inode) {
                self.set(index, is_inode, true);
                let id = if is_inode { index + 1 } else { index };
                println!("[Alloc{}] {}", if is_inode { "Inode" } else { "Data" }, id);
                return Some(id);
            }
        };
        None
    }

    pub fn free_block(&mut self, id: usize, is_inode: bool, free_block: bool) {
        println!("free block: {}, is_inode:{}", id, is_inode);
        let index = if is_inode { id - 1 } else { id };
        if self.used(index, is_inode) {
            self.set(index, is_inode, false);
            if free_block {
                // 对物理块清理
                if is_inode {
                    let (blk_id, offset) = self.inode_block(id);
                    println!("free inode block: {}, offset:{}", blk_id, offset);
                    self.block_cache(blk_id)
                        .lock()
                        .unwrap()
                        .modify(offset, |ino: &mut Inode| {
                            *ino = Inode::nil()
                        });
                } else {
                    let blk_id = self.data_block(id);
                    self.block_cache(blk_id)
                        .lock()
                        .unwrap()
                        .free()
                }
            }
            return;
        }
        // 重复释放
        println!(
            "Try to release the free {} block({})!",
            if is_inode { "Inode" } else { "Data" },
            id
        )
    }

    pub fn bitmap_range(&self, is_inode: bool) -> Range<usize> {
        let super_block = self.super_block();
        if is_inode {
            1..1 + super_block.inode_bitmap_blocks
        } else {
            1 + super_block.inode_bitmap_blocks
                ..1 + super_block.inode_bitmap_blocks + super_block.bitmap_blocks
        }
    }

    pub fn super_block(&self) -> SuperBlock {
        let mut super_block = SuperBlock::default();
        self.super_block
            .lock()
            .unwrap()
            .read(0, |sb: &SuperBlock| super_block = *sb);
        super_block
    }

    /// @return (blk_id usize, bytes_offset:usize, bit_offset:usize)
    fn bitmap_offset(&self, index: usize, is_inode: bool) -> (usize, usize, usize) {
        let range = self.bitmap_range(is_inode);
        let super_block = self.super_block();
        if is_inode {
            if index > super_block.inode_blocks {
                panic!("out of inode blocks bit range");
            }
        } else {
            if index > super_block.data_blocks {
                panic!("out of data blocks bit range");
            }
        }
        let blk_id = index / (BLOCK_SIZE * 8);
        if blk_id > range.len() {
            // out of bounds
            panic!("out of bounds");
        }
        let blk_offset = index % (BLOCK_SIZE * 8);
        let blk_bytes_offset = blk_offset / 8;
        let blk_bit_offset = blk_offset % 8;
        (blk_id + range.start, blk_bytes_offset, blk_bit_offset)
    }

    pub fn used(&mut self, index: usize, is_inode: bool) -> bool {
        let (blk_id, bytes_offset, bit_offset) = self.bitmap_offset(index, is_inode);
        let mut check = false;
        self.block_cache(blk_id)
            .lock()
            .unwrap()
            .read(bytes_offset, |byte: &u8| {
                let mask = 1 << bit_offset;
                check = byte & mask > 0;
            });
        check
    }

    fn set(&mut self, id: usize, is_inode: bool, v: bool) {
        let (blk_id, bytes_offset, bit_offset) = self.bitmap_offset(id, is_inode);
        self.block_cache(blk_id)
            .lock()
            .unwrap()
            .modify(bytes_offset, |byte: &mut u8| {
                if v {
                    *byte |= 1 << bit_offset
                } else {
                    *byte &= !(1 << bit_offset)
                }
            });
    }

    pub fn clear_bitmap(&mut self) {
        let super_block = self.super_block();
        let bitmap_blocks = super_block.bitmap_blocks + super_block.inode_bitmap_blocks;
        for blk_id in 0..bitmap_blocks {
            self.block_cache(blk_id + 1)
                .lock()
                .unwrap()
                .modify(0, |bytes: &mut [u8; BLOCK_SIZE]| {
                    bytes.iter_mut().for_each(|v| {
                        *v = 0;
                    })
                })
        }
    }

    pub fn print(&mut self) {
        let super_block = self.super_block();
        let size = super_block.inode_blocks;
        let mut used = Vec::new();
        for id in 0..size {
            if self.used(id, true) {
                used.push(id + 1)
            }
        };
        println!("Used Inode Blocks: {:?}", used);
        let size = super_block.data_blocks;
        let mut used = Vec::new();
        for id in 0..size {
            if self.used(id, false) {
                used.push(id)
            }
        };
        println!("Used Data Blocks: {:?}", used);
    }
}
