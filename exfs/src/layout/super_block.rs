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
    pub inode_bitmap_blocks: usize,
    pub bitmap_blocks: usize,
    pub inode_blocks: usize, // inode 所占据的物理块的数量
    pub data_blocks: usize,
}

impl SuperBlock {
    // 实际存在的可分配 inode 数量和
    pub fn inode_size(&self) -> usize {
        self.inode_blocks * BLOCK_SIZE / INODE_SIZE
    }
    pub fn new(blocks: usize) -> Self {
        assert!(blocks > 10); // 设备至少有 10 个可分配的块(随意指定的一个数量确保绝大部分 fs 都大于且足够划分空间)
        // data:n bitmap:y=(n+BS*8-1)/(BS*8) inode:n * INODE_SIZE/BS inode_bitmap:
        let data = blocks;
        let bitmap_blocks = (data + BLOCK_SIZE * 8 - 1) / (BLOCK_SIZE * 8);
        let inode = data;
        let inode_blocks = inode * INODE_SIZE / BLOCK_SIZE;
        let inode_bitmap = (inode + BLOCK_SIZE * 8 - 1) / (BLOCK_SIZE * 8);
        let all_blocks = 1 + data + bitmap_blocks + inode_bitmap + inode_blocks;
        let scale = blocks as f64 / all_blocks as f64;

        let data = (blocks as f64 * scale) as usize;
        let bitmap_blocks = (data + BLOCK_SIZE * 8 - 1) / (BLOCK_SIZE * 8);
        let left = blocks - 1 - data - bitmap_blocks;
        let inode_bitmap_blocks = (left + BLOCK_SIZE * 8) / (BLOCK_SIZE * 8 + 1);
        let inode_blocks = left - inode_bitmap_blocks;
        Self {
            magic: MAGIC,
            inode_bitmap_blocks,
            bitmap_blocks,
            inode_blocks: inode_blocks,
            data_blocks: data,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC
    }

    // 通过数据块id计算物理块地址
    // id 最小值为 1,id为 0 时表示无效地址
    pub fn data_block(&self, id: usize) -> usize {
        1 + self.inode_bitmap_blocks + self.bitmap_blocks + self.inode_blocks + id
    }

    /// 通过 inode 号计算实际物理块地址与偏移量
    /// inode 块是倒序存储的,内部是顺序存储的
    /// @return block_id(物理),offset
    pub fn inode_block(&self, id: usize) -> (usize, usize) {
        assert!(id > 0);
        let id = id - 1;
        let block_cap = BLOCK_SIZE / INODE_SIZE;
        let blk_index = id / block_cap;
        let inode_blk = 1 + self.inode_bitmap_blocks + self.bitmap_blocks + blk_index;
        (inode_blk, (id % block_cap) * INODE_SIZE)
    }
}
