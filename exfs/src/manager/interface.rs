use log::debug;

use crate::config::BLOCK_SIZE;
use crate::layout::data_block::{DIR_ENTRY_SIZE, DirEntry};
use crate::layout::inode::{Inode, InodeWithId};
use crate::manager::block_cache_manager::BlockCacheDevice;
use crate::manager::DirEntryDetail;
use crate::manager::error_code::*;
use crate::typ::file_name::FileName;
use crate::utils::slice::{align, vec2slice};

/// 功能接口
/// 无权限管理
impl BlockCacheDevice {
    pub fn lookup_internal(
        &mut self,
        parent: &Inode,
        name: FileName,
    ) -> Result<InodeWithId, ErrorCode> {
        self.ls_internal(&parent)
            .and_then(|v| match v.iter().find(|entry| name == entry.name) {
                None => Err(ENOENT),
                Some(e) => Ok(self.inode(e.inode as usize).with_id(e.inode as usize)),
            })
    }
    pub fn make_node_internal(
        &mut self,
        file_name: &str,
        parent: &InodeWithId,
        mode: u16,
    ) -> Result<usize, ErrorCode> {
        let mut name = [0u8; 56];
        name[..file_name.len()].copy_from_slice(file_name.as_bytes());
        if parent.data.exist() {
            return if parent.data.is_dir() {
                self.ls_internal(parent.inode()).and_then(|mut dirs| {
                    if dirs.iter().any(|v| v.name.as_slice() == name) {
                        return Err(EEXIST);
                    }
                    let inode_opt = self.alloc_block(true);
                    return match inode_opt {
                        None => Err(ENOSPC),
                        Some(inode_id) => {
                            self.print();
                            self.modify_inode(inode_id, |inode| *inode = Inode::new(mode));
                            dirs.push(DirEntry {
                                name: name.into(),
                                inode: inode_id as u64,
                            });
                            let buf: Vec<u8> = vec2slice(dirs);
                            if let Err(e) = self.write_system(0, parent, &buf, true) {
                                debug!("mk_file:339 error: {}", e);
                                return Err(e);
                            }
                            Ok(inode_id)
                        }
                    };
                })
            } else {
                Err(ENOTDIR)
            };
        }
        debug!("mk_file:end error: NotExist");
        Err(ENOENT)
    }
    pub fn ls_internal(&mut self, inode: &Inode) -> Result<Vec<DirEntry>, ErrorCode> {
        if inode.exist() {
            if inode.is_dir() {
                let mut entries = Vec::new();
                // debug!("dir list: {:?},{}", inode.index_node, inode.index_level);
                inode
                    .index_node
                    .list(self, inode.index_level)
                    .iter()
                    .for_each(|v| {
                        // debug!("data blocks: {}", v);
                        self.block_cache(self.data_block(*v)).lock().unwrap().read(
                            0,
                            |dirs: &[DirEntry; BLOCK_SIZE / DIR_ENTRY_SIZE]| {
                                // debug!("dir entries: {:?}", dirs);
                                dirs.iter()
                                    .filter(|v| !v.name.is_empty() && v.inode != 0)
                                    .for_each(|dir| entries.push(*dir))
                            },
                        );
                    });
                Ok(entries)
            } else {
                Err(ENOTDIR)
            }
        } else {
            Err(ENOENT)
        }
    }
    pub fn unlink_internal(
        &mut self,
        parent: &InodeWithId,
        name: FileName,
    ) -> Result<(), ErrorCode> {
        self.ls_internal(parent.inode()).and_then(|mut v| {
            for (id, entry) in v.clone().iter().enumerate() {
                if entry.name == name {
                    v.remove(id);
                    let ino_id = entry.inode as usize;
                    let inode = self.modify_inode(ino_id, |inode| {
                        inode.link_count -= 1;
                        inode.clone()
                    });
                    if inode.link_count == 0 {
                        inode.index_node.delete(self, inode.index_level, true);
                        self.free_block(ino_id, true, true);
                    }
                    let mut buf = vec2slice(v);
                    align(&mut buf, BLOCK_SIZE);
                    return self.write_system(0, parent, &buf, true).map(|_| ());
                }
            }
            Err(ENOENT)
        })
    }
    pub fn rename_internal(
        &mut self,
        parent: &InodeWithId,
        name: FileName,
        new_parent: &InodeWithId,
        new_name: FileName,
    ) -> Result<(), ErrorCode> {
        if parent.inode == new_parent.inode && name == new_name {
            return Ok(());
        }
        self.lookup_internal(parent.inode(), name)
            .and_then(|entry| match self.unlink_internal(new_parent, new_name) {
                Ok(_) | Err(ENOENT) => {
                    self.ls_internal(new_parent.inode()).and_then(|mut new_dirs| {
                        new_dirs.push(DirEntry {
                            name: new_name,
                            inode: entry.inode as u64,
                        });
                        let mut buf = vec2slice(new_dirs);
                        align(&mut buf, BLOCK_SIZE);
                        self.write_system(0, new_parent, &buf, true)
                            .and_then(|_| self.unlink_internal(parent, name))
                    })
                }
                Err(e) => Err(e),
            })
    }
    pub fn ls(&mut self, path: &str) -> Result<Vec<DirEntryDetail>, ErrorCode> {
        let path_split = path.split("/").filter(|p| !p.is_empty());
        let mut parent_inode = self.inode(0);
        for p in path_split {
            let dirs_result = self
                .ls_internal(&parent_inode);
            match dirs_result {
                Ok(dirs) => {
                    match dirs.iter().find(|v| String::from(v.name) == p.to_string())
                    {
                        None => return Err(ENOENT),
                        Some(v) => parent_inode = self.inode(v.inode as usize),
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        self.ls_internal(&parent_inode).map(|vec| {
            vec.iter()
                .map(|v| DirEntryDetail {
                    name: String::from(v.name),
                    inode_id: v.inode as usize,
                    inode: self.inode(v.inode as usize),
                })
                .collect()
        })
    }
}
