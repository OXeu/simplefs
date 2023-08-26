#![cfg(test)]

use log::debug;

use crate::manager::block_cache_manager::BlockCacheDevice;

fn ls(fs: &mut BlockCacheDevice, path: &str) {
    debug!("ls: {}", path);
    match fs.ls(path) {
        Ok(iter) => {
            if iter.len() == 0 {
                debug!("[Empty]")
            } else {
                iter.iter().for_each(|v| debug!("{}    {:?}     {}", v.name, v.inode.file_type(), v.inode_id));
            }
        }
        Err(e) => debug!("[error] {:?}",e)
    }
}

// 有个问题,为什么 inode 是 6 -> 4 -> 2