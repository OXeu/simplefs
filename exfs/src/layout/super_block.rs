use crate::config::BLOCK_SIZE;
use crate::layout::inode::INODE_SIZE;

const MAGIC: usize = 0x0aca_baca_01a7_88cc;
/// 磁盘布局
/// | SuperBlock | Bitmap | Data Blocks ->| Free Space |<- Inode Blocks |
/// |     1块    |   n块   |      x块      |     **     |       y块      |
///             |bm_blocks|           db_last      ib_last        ib_first
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct SuperBlock {
    magic: usize,
    pub bitmap_blocks: usize,
    pub data_block_last: usize,
    pub inode_block_last: usize,
    pub inode_block_first: usize,
}

impl SuperBlock {
    pub fn new(blocks: usize) -> Self {
        assert!(blocks > 2); // 设备至少有三个可分配的块
        let bitmap_blocks = (blocks - 2) / (BLOCK_SIZE * 8 + 1) + 1;
        Self {
            magic: MAGIC,
            bitmap_blocks,
            data_block_last: bitmap_blocks + 1,
            inode_block_last: blocks - 1,
            inode_block_first: blocks-1,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC
    }

    // 通过数据块id计算物理块地址
    // id 最小值为 1,id为 0 时表示无效地址
    pub fn data_block(&self, id: usize) -> usize {
        // assert!(id > 0);
        1 + self.bitmap_blocks + id
    }

    /// 通过 inode 号计算实际物理块地址与偏移量
    /// inode 块是倒序存储的,内部是顺序存储的
    /// @return block_id(物理),offset
    pub fn inode_block(&self, id: usize) -> (usize, usize) {
        let block_cap = BLOCK_SIZE / INODE_SIZE;
        let blk_index = id / block_cap;
        (self.inode_block_first - blk_index, (id % block_cap)*INODE_SIZE)
    }
}
