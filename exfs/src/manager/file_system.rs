use std::io::SeekFrom;
use std::path::Path;
use std::time::SystemTime;

use fuser::TimeOrNow;
use libc::ENOTSUP;

use crate::config::BLOCK_SIZE;
use crate::layout::data_block::DirEntry;
use crate::layout::inode::InodeWithId;
use crate::manager::block_cache_manager::BlockCacheDevice;
use crate::manager::DirEntryDetail;
use crate::manager::error_code::{EBADF, EEXIST, EIO, ENOENT, ENOTDIR, EPERM, ErrorCode};
use crate::typ::file_name::FileName;
use crate::typ::file_type::FileType;
use crate::typ::request::{Mask, Req};
use crate::utils::slice::{slice2vec, vec2slice};
use crate::utils::time::{time_sec, time_sys};

/// 上层接口，实现了权限管理
impl BlockCacheDevice {
    pub fn lookup(
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

    pub fn getattr(&mut self, req: &Req, inode_id: usize) -> Result<InodeWithId, ErrorCode> {
        let inode = self.inode(inode_id);
        inode.access_guard(req, Mask::R, inode.with_id(inode_id))
    }

    pub fn setattr(
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

    fn readlink(&mut self, req: &Req, inode_id: usize) -> Result<Vec<u8>, ErrorCode> {
        // debug!("ReadLink: {}", _ino)
        let inode = self.inode(inode_id);
        inode.access_guard(req, Mask::R, self.read_all(inode_id))
    }

    fn mknod(
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
            )
                .map(|v| self.inode(v).with_id(v))
        })
    }

    fn mkdir(
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
            )
                .map(|v| self.inode(v).with_id(v))
        })
    }

    fn unlink(&mut self, req: &Req, _parent: usize, _name: FileName) -> Result<(), ErrorCode> {
        let parent = self.inode(_parent);
        parent.access_guard_f(req, Mask::WX, || {
            self.unlink_internal(&parent.with_id(_parent), _name.into())
        })
    }

    fn symlink(
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
            )
                .and_then(|v| {
                    let buf = _link.to_str().unwrap();
                    let inode = self.inode(v);
                    self.write_system(0, &inode.with_id(v), buf.as_ref(), true)
                        .map(|_| inode.with_id(v))
                })
        })
    }

    fn mv(
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

    fn link(
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
                        let buf: Vec<u8> = vec2slice(dirs);
                        self.write_system(0, &new_parent.with_id(_new_parent), &buf, true)
                            .map(|_| self.inode(_ino).with_id(_ino))
                    }
                    Ok(_) => Err(EEXIST),
                    Err(e) => Err(e),
                }
            })
        })
    }

    fn open(&mut self, req: &Req, _ino: usize, _flags: i32) -> Result<u32, ErrorCode> {
        let inode = self.inode(_ino);
        Mask::from_flag(_flags).map_or(Err(EIO), |mask| {
            inode.access_guard_f(req, mask, || {
                self.open_internal(_ino, 0, _flags, req.pid)
            })
        })
    }

    fn read(
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

    fn write(
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

    fn flush(&mut self, req: &Req, fh: u32) -> Result<(), ErrorCode> {
        match self.fh(fh, req.pid) {
            None => Err(EBADF),
            Some(fh) => {
                let fh = fh.clone();
                fh.flush(self);
                Ok(())
            }
        }
    }

    fn release(
        &mut self,
        req: &Req,
        fh: u32,
        flush: bool,
    ) -> Result<(), ErrorCode> {
        self.close_internal(fh, req.pid, flush)
    }

    fn opendir(&mut self, req: &Req, _ino: usize, _flags: i32) -> Result<u32, ErrorCode> {
        // debug!("OpenDir: {}", _ino);
        let inode = self.inode(_ino);
        if inode.is_dir() {
            self.open(req, _ino, _flags)
        } else {
            Err(ENOTDIR)
        }
    }

    fn readdir(&mut self, req: &Req, fh: u32, offset: usize) -> Result<Vec<DirEntryDetail>, ErrorCode> {
        match self.fh(fh, req.pid) {
            None => Err(EBADF),
            Some(fh) => {
                let mut fh = fh.clone();
                let mut buf = [0u8; BLOCK_SIZE];
                fh.seek(SeekFrom::Start(offset as u64));
                fh.read(self, &mut buf);
                let dirs: Vec<&DirEntry> = slice2vec(&buf);
                Ok(dirs.iter().filter_map(|&dir| {
                    if dir.valid() {
                        let inode = self.inode(dir.inode as usize);
                        Some(DirEntryDetail {
                            name: String::from(dir.name),
                            inode_id: dir.inode as usize,
                            inode,
                        })
                    } else {
                        None
                    }
                }).collect())
            }
        }
    }

    fn getxattr(
        &mut self,
        _req: &Req,
        _ino: u64,
        _name: FileName,
        _size: u32,
    ) -> Result<(), ErrorCode> {
        // debug!("GetXAttr: {}", _ino);
        Err(ENOTSUP)
    }

    fn access(&mut self, req: &Req, _ino: usize, _mask: i32) -> Result<(), ErrorCode> {
        // debug!("Access: {}", _ino);
        let inode = self.inode(_ino);
        inode.access_guard(req, Mask::from_mask(_mask), ())
    }

    fn create(
        &mut self,
        req: &Req,
        _parent: usize,
        name: FileName,
        mode: u32,
        _umask: u32,
        flags: i32,
    ) -> Result<u32, ErrorCode> {
        let parent = self.inode(_parent);
        self.make_node_internal(
            String::from(name).as_str(),
            &parent.with_id(_parent),
            FileType::File << 12 | mode as u16,
        ).and_then(|v| {
            self.open(req, v, flags)
        })
    }
}
