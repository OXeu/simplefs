use std::io::SeekFrom;
use std::path::Path;
use std::time::SystemTime;

use fuser::TimeOrNow;
use libc::ENOTSUP;

use crate::config::BLOCK_SIZE;
use crate::layout::data_block::{DIR_ENTRY_SIZE, DirEntry};
use crate::layout::inode::InodeWithId;
use crate::manager::block_cache_manager::BlockCacheDevice;
use crate::manager::DirEntryDetail;
use crate::manager::error_code::{EBADF, EEXIST, EIO, ENOENT, ENOTDIR, EPERM, ErrorCode};
use crate::typ::file_name::FileName;
use crate::typ::file_type::FileType;
use crate::typ::request::{Mask, Req};
use crate::utils::slice::vec2slice;
use crate::utils::time::{time_sec, time_sys};

/// 上层接口，实现了权限管理
impl BlockCacheDevice {
    pub fn lookup_guard(
        &mut self,
        req: &Req,
        _parent: usize,
        name: FileName,
    ) -> Result<InodeWithId, ErrorCode> {
        let parent = self.inode(_parent);
        parent.access_guard_f(req, Mask::RX, || match self.ls_internal(&parent) {
            Ok(v) => match v.iter().find(|entry| name == entry.name) {
                None => Err(ENOENT),
                Some(e) => Ok(self.inode(e.inode as usize).with_id(e.inode as usize)),
            },
            Err(e) => Err(e),
        })
    }

    pub fn getattr_guard(&mut self, req: &Req, inode_id: usize) -> Result<InodeWithId, ErrorCode> {
        let inode = self.inode(inode_id);
        inode.access_guard(req, Mask::R, inode.with_id(inode_id))
    }

    pub fn setattr_guard(
        &mut self,
        req: &Req,
        inode_id: usize,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
    ) -> Result<InodeWithId, ErrorCode> {
        self.modify_inode(inode_id, |ino| {
            if ino.access(req.uid, req.gid, Mask::W) {
                if let Some(v) = _mode {
                    ino.mode = v as u16
                }
                if let Some(v) = _uid {
                    ino.uid = v
                }
                if let Some(v) = _gid {
                    ino.gid = v
                }
                if let Some(v) = _size {
                    ino.size = v
                }
                if let Some(v) = _mtime {
                    ino.modified = time_sec(v)
                }
                if let Some(v) = _crtime {
                    ino.created = time_sys(v)
                }
                Ok(ino.with_id(inode_id))
            } else {
                Err(EPERM)
            }
        })
    }

    pub fn readlink_guard(&mut self, req: &Req, inode_id: usize) -> Result<Vec<u8>, ErrorCode> {
        let inode = self.inode(inode_id);
        inode.access_guard(req, Mask::R, self.read_all(inode_id))
    }

    pub fn mknod_guard(
        &mut self,
        req: &Req,
        _parent: usize,
        _name: FileName,
        _mode: u32,
        _umask: u32,
        _rdev: u32,
    ) -> Result<InodeWithId, ErrorCode> {
        let parent = self.inode(_parent);
        parent.access_guard_f(req, Mask::WX, || {
            self.make_node_internal(
                String::from(_name).as_str(),
                &parent.with_id(_parent),
                _mode as u16,
                req.uid,
                req.gid,
            )
                .map(|v| self.inode(v).with_id(v))
        })
    }

    pub fn mkdir_guard(
        &mut self,
        req: &Req,
        _parent: usize,
        _name: FileName,
        _mode: u32,
        _umask: u32,
    ) -> Result<InodeWithId, ErrorCode> {
        let parent = self.inode(_parent);
        parent.access_guard_f(req, Mask::WX, || {
            self.make_node_internal(
                String::from(_name).as_str(),
                &parent.with_id(_parent),
                FileType::Dir << 12 | _mode as u16,
                req.uid,
                req.gid,
            )
                .map(|v| self.inode(v).with_id(v))
        })
    }

    pub fn unlink_guard(&mut self, req: &Req, _parent: usize, _name: FileName) -> Result<(), ErrorCode> {
        let parent = self.inode(_parent);
        parent.access_guard_f(req, Mask::WX, || {
            self.unlink_internal(&parent.with_id(_parent), _name.into())
        })
    }

    pub fn rmdir_guard(&mut self, req: &Req, _parent: usize, _name: FileName) -> Result<(), ErrorCode> {
        let parent = self.inode(_parent).with_id(_parent);
        self.lookup_guard(req, _parent, _name)
            .and_then(|v| {
                self.remove_dir_internal(&v).and_then(|_| {
                    // 删除自己
                    self.unlink_internal(&parent, _name)
                })
            })
    }

    pub fn symlink_guard(
        &mut self,
        req: &Req,
        _parent: usize,
        _name: FileName,
        _link: &Path,
    ) -> Result<InodeWithId, ErrorCode> {
        // debug!("SymLink: {:?}", _name)
        let parent = self.inode(_parent);
        parent.access_guard_f(req, Mask::WX, || {
            self.make_node_internal(
                String::from(_name).as_str(),
                &parent.with_id(_parent),
                FileType::SymbolLink << 12 | 0o744u16,
                req.uid,
                req.gid,
            )
                .and_then(|v| {
                    let buf = _link.to_str().unwrap();
                    let inode = self.inode(v);
                    self.write_system(0, &inode.with_id(v), buf.as_ref(), true)
                        .map(|_| inode.with_id(v))
                })
        })
    }

    pub fn move_guard(
        &mut self,
        req: &Req,
        _parent: usize,
        _name: FileName,
        _new_parent: usize,
        _new_name: FileName,
        _flags: u32,
    ) -> Result<(), ErrorCode> {
        let parent = self.inode(_parent);
        let new_parent = self.inode(_new_parent);
        parent.access_guard_f(req, Mask::WX, || {
            new_parent.access_guard_f(req, Mask::WX, || {
                self.rename_internal(
                    &parent.with_id(_parent),
                    _name.into(),
                    &new_parent.with_id(_new_parent),
                    _new_name.into(),
                )
            })
        })
    }

    pub fn link_guard(
        &mut self,
        req: &Req,
        _ino: usize,
        _new_parent: usize,
        _new_name: FileName,
    ) -> Result<InodeWithId, ErrorCode> {
        let new_parent = self.inode(_new_parent);
        new_parent.access_guard_f(req, Mask::WX, || {
            self.ls_internal(&new_parent).and_then(|mut dirs| {
                match self.lookup_internal(&new_parent, _new_name.into()) {
                    Err(ENOENT) => {
                        dirs.push(DirEntry {
                            name: _new_name.into(),
                            inode: _ino as u64,
                        });
                        let inode = self.modify_inode(_ino, |ino| {
                            ino.link_count += 1;
                            ino.clone()
                        }).with_id(_ino);
                        let buf: Vec<u8> = vec2slice(dirs);
                        self.write_system(0, &new_parent.with_id(_new_parent), &buf, true)
                            .map(|_| inode)
                    }
                    Ok(_) => Err(EEXIST),
                    Err(e) => Err(e),
                }
            })
        })
    }

    pub fn open_guard(&mut self, req: &Req, _ino: usize, _flags: i32) -> Result<u32, ErrorCode> {
        let inode = self.inode(_ino);
        Mask::from_flag(_flags).map_or(Err(EIO), |mask| {
            inode.access_guard_f(req, mask, || {
                self.open_internal(_ino, 0, _flags, req.pid)
            })
        })
    }

    pub fn read_guard(
        &mut self,
        req: &Req,
        fh: u32,
        offset: SeekFrom,
        buf: &mut [u8],
        // _lock_owner: Option<u64>, 文件锁，暂时不实现
    ) -> Result<usize, ErrorCode> {
        match self.fh(fh, req.pid) {
            None => Err(EBADF),
            Some(fh) => {
                let mut fh = fh.clone();
                fh.seek(offset);
                Ok(fh.read(self, buf))
            }
        }
    }

    pub fn write_guard(
        &mut self,
        req: &Req,
        fh: u32,
        offset: SeekFrom,
        data: &[u8],
        //_lock_owner: Option<u64>,
    ) -> Result<usize, ErrorCode> {
        match self.fh(fh, req.pid) {
            None => Err(EBADF),
            Some(fh) => {
                let mut fh = fh.clone();
                fh.seek(offset);
                fh.write(self, data)
            }
        }
    }

    pub fn flush_guard(&mut self, req: &Req, fh: u32) -> Result<(), ErrorCode> {
        match self.fh(fh, req.pid) {
            None => Err(EBADF),
            Some(fh) => {
                let fh = fh.clone();
                fh.flush(self);
                Ok(())
            }
        }
    }

    pub fn release_guard(
        &mut self,
        req: &Req,
        fh: u32,
        flush: bool,
    ) -> Result<(), ErrorCode> {
        self.close_internal(fh, req.pid, flush)
    }

    pub fn opendir_guard(&mut self, req: &Req, _ino: usize, _flags: i32) -> Result<u32, ErrorCode> {
        let inode = self.inode(_ino);
        if inode.is_dir() {
            self.open_guard(req, _ino, _flags)
        } else {
            Err(ENOTDIR)
        }
    }

    pub fn readdir_guard(&mut self, req: &Req, fh: u32, offset: usize) -> Result<Vec<DirEntryDetail>, ErrorCode> {
        let blk_id = offset / (BLOCK_SIZE / DIR_ENTRY_SIZE);
        let blk_offset = offset % (BLOCK_SIZE / DIR_ENTRY_SIZE);
        println!("offset: {}", offset);
        let mut offset_id = offset + 1;
        match self.fh(fh, req.pid) {
            None => Err(EBADF),
            Some(fh) => {
                // println!("!!!1");
                let mut fh = fh.clone();
                // println!("!!!2");
                let mut vec = Vec::new();
                fh.read_block(self, blk_id, 0, |dirs: &[DirEntry; BLOCK_SIZE / DIR_ENTRY_SIZE]| {
                    dirs.iter().skip(blk_offset).for_each(|dir| {
                        if dir.valid() {
                            vec.push(dir.clone())
                        }
                    })
                });
                // println!("dir_entry: {:?}",vec);
                Ok(vec.iter().map(|dir| {
                    let inode = self.inode(dir.inode as usize);
                    let id = offset_id;
                    offset_id += 1;
                    DirEntryDetail {
                        name: String::from(dir.name),
                        inode_id: dir.inode as usize,
                        offset: id,
                        inode,
                    }
                }).collect())
            }
        }
    }

    pub fn getxattr_guard(
        &mut self,
        _req: &Req,
        _ino: usize,
        _name: FileName,
        _size: u32,
    ) -> Result<(), ErrorCode> {
        // debug!("GetXAttr: {}", _ino);
        Err(ENOTSUP)
    }

    pub fn access_guard(&mut self, req: &Req, _ino: usize, _mask: i32) -> Result<(), ErrorCode> {
        // debug!("Access: {}", _ino);
        let inode = self.inode(_ino);
        inode.access_guard(req, Mask::from_mask(_mask), ())
    }

    pub fn create_guard(
        &mut self,
        req: &Req,
        _parent: usize,
        name: FileName,
        mode: u32,
        _umask: u32,
        flags: i32,
    ) -> Result<u32, ErrorCode> {
        println!("Create guard: parent:{} mode:{} umask:{} flags:{}", _parent, mode, _umask, flags);
        let parent = self.inode(_parent);
        self.make_node_internal(
            String::from(name).as_str(),
            &parent.with_id(_parent),
            FileType::File << 12 | mode as u16,
            req.uid,
            req.gid,
        ).and_then(|v| {
            self.open_guard(req, v, flags)
        })
    }
}
