// 全局单例与函数
// 放在一个文件夹便于快速查找 API

use crate::layout::inode::Inode;

pub mod block_cache_manager;
pub(crate) mod error_code;
pub mod file_system;
pub mod interface;

pub struct DirEntryDetail {
    pub name: String,
    pub inode_id: usize,
    pub offset: usize,
    pub inode: Inode,
}
