// 全局单例与函数
// 放在一个文件夹便于快速查找 API

use crate::layout::inode::Inode;

pub mod block_cache_manager;

pub struct DirEntryDetail {
    pub name: String,
    pub inode_id: usize,
    pub inode: Inode,
}
