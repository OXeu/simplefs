use std::ffi::{OsStr, OsString};
use std::io::SeekFrom;
use std::path::Path;
use std::time::{Duration, SystemTime};

use fuser::{FileAttr, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, ReplyXattr, Request, TimeOrNow};
use libc::ENOTSUP;

use crate::config::BLOCK_SIZE;
use crate::layout::inode::InodeWithId;
use crate::manager::block_cache_manager::BlockCacheDevice;
use crate::manager::error_code::EBADF;
use crate::typ::file_type::FileType;
use crate::typ::request::Req;
use crate::utils::time::system_time_from_time;

impl Filesystem for BlockCacheDevice {
    fn lookup(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEntry) {
        let ttl = Duration::new(60, 0);
        let guard = self.lookup_guard(&_req.into(), cast(_parent), _name.into());
        println!("Lookup: {:?}", guard);
        match guard {
            Err(e) => reply.error(e),
            Ok(entry) => reply.entry(&ttl, &entry.into(), 0),
        }
    }
    fn getattr(&mut self, _req: &Request, _ino: u64, reply: ReplyAttr) {
        let ttl = Duration::new(60, 0);
        let res = self.getattr_guard(&_req.into(), cast(_ino));
        match res {
            Err(e) => reply.error(e),
            Ok(data) => {
                reply.attr(&ttl, &data.into());
            }
        }
    }
    fn setattr(
        &mut self,
        _req: &Request,
        _ino: u64,
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
        reply: ReplyAttr,
    ) {
        let ttl = Duration::new(60, 0);
        match self.setattr_guard(
            &_req.into(),
            _ino as usize,
            _mode,
            _uid,
            _gid,
            _size,
            _atime,
            _mtime,
            _crtime,
            _fh,
            _crtime,
            _chgtime,
            _bkuptime,
            _flags, )
        {
            Err(e) => reply.error(e),
            Ok(attr) => reply.attr(&ttl, &attr.into())
        }
    }
    fn readlink(&mut self, _req: &Request, _ino: u64, reply: ReplyData) {
        // debug!("ReadLink: {}", _ino)
        match self.readlink_guard(&_req.into(), _ino as usize) {
            Err(e) => reply.error(e),
            Ok(buf) => reply.data(buf.as_ref())
        }
    }
    fn mknod(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        let ttl = Duration::new(60, 0);
        match self.mknod_guard(&_req.into(), _parent as usize, _name.into(), _mode, _umask, _rdev) {
            Err(e) => reply.error(e),
            Ok(buf) => reply.entry(&ttl, &buf.into(), 0)
        }
    }
    fn mkdir(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let ttl = Duration::new(60, 0);
        match self.mkdir_guard(&_req.into(), _parent as usize, _name.into(), _mode, _umask) {
            Err(e) => reply.error(e),
            Ok(buf) => reply.entry(&ttl, &buf.into(), 0)
        }
    }

    fn unlink(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEmpty) {
        match self.unlink_guard(&_req.into(), _parent as usize, _name.into()) {
            Err(e) => reply.error(e),
            Ok(_) => reply.ok()
        }
    }

    fn rmdir(&mut self, _req: &Request<'_>, _parent: u64, _name: &OsStr, reply: ReplyEmpty) {
        match self.rmdir_guard(&_req.into(), _parent as usize, _name.into()) {
            Ok(_) => {
                reply.ok()
            }
            Err(e) => {
                reply.error(e)
            }
        }
    }

    fn symlink(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _link: &Path,
        reply: ReplyEntry,
    ) {
        let ttl = Duration::new(60, 0);
        match self.symlink_guard(&_req.into(), _parent as usize, _name.into(), _link) {
            Err(e) => reply.error(e),
            Ok(buf) => reply.entry(&ttl, &buf.into(), 0)
        }
    }

    fn rename(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _newparent: u64,
        _newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        match self.move_guard(&_req.into(), _parent as usize, _name.into(), _newparent as usize, _newname.into(), _flags) {
            Err(e) => reply.error(e),
            Ok(_) => reply.ok()
        }
    }

    fn link(
        &mut self,
        _req: &Request,
        _ino: u64,
        _newparent: u64,
        _newname: &OsStr,
        reply: ReplyEntry,
    ) {
        let ttl = Duration::new(60, 0);
        match self.link_guard(&_req.into(), _ino as usize, _newparent as usize, _newname.into()) {
            Err(e) => reply.error(e),
            Ok(buf) => reply.entry(&ttl, &buf.into(), 0)
        }
    }

    fn open(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        match self.open_guard(&_req.into(), _ino as usize, _flags) {
            Err(e) => reply.error(e),
            Ok(fh) => reply.opened(fh as u64, _flags as u32)
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        _size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let mut buf: Vec<u8> = Vec::new();
        for _ in 0.._size {
            buf.push(0)
        }

        match self.read_guard(&_req.into(), _fh as u32, SeekFrom::Start(_offset as u64), &mut buf) {
            Err(e) => reply.error(e),
            Ok(_) => reply.data(&buf)
        }
    }

    fn write(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        _data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        match self.write_guard(&_req.into(), _fh as u32, SeekFrom::Start(_offset as u64), _data) {
            Err(e) => reply.error(e),
            Ok(len) => reply.written(len as u32)
        }
    }

    fn flush(&mut self, _req: &Request, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        match self.flush_guard(&_req.into(), _fh as u32) {
            Err(e) => reply.error(e),
            Ok(_) => reply.ok()
        }
    }

    fn release(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        match self.release_guard(&_req.into(), _fh as u32, _flush) {
            Err(e) => reply.error(e),
            Ok(_) => reply.ok()
        }
    }

    fn opendir(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        println!("Open {}", _ino);
        match self.opendir_guard(&_req.into(), _ino as usize, _flags) {
            Err(e) => {
                println!("open dir error -> {}", e);
                reply.error(e)
            }
            Ok(fh) => {
                println!("open dir -> {}", fh);
                reply.opened(fh as u64, _flags as u32)
            }
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        mut reply: ReplyDirectory,
    ) {
        match self.readdir_guard(&_req.into(), _fh as u32, _offset as usize) {
            Err(e) => reply.error(e),
            Ok(buf) => {
                buf.iter()
                    .for_each(|dir| {
                        let _ = reply.add(dir.inode_id as u64, dir.offset as i64, dir.inode.file_type().into(), OsString::from(dir.name.clone()));
                    });
                reply.ok()
            }
        }
    }

    fn getxattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _name: &OsStr,
        _size: u32,
        reply: ReplyXattr,
    ) {
        match self.getxattr_guard(&_req.into(), _ino as usize, _name.into(), _size) {
            Err(e) => reply.error(e),
            Ok(_) => reply.error(ENOTSUP)
        }
    }

    fn access(&mut self, _req: &Request, _ino: u64, _mask: i32, reply: ReplyEmpty) {
        match self.access_guard(&_req.into(), _ino as usize, _mask) {
            Err(e) => reply.error(e),
            Ok(_) => reply.ok()
        }
    }

    fn create(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let req = _req.into();
        let ttl = Duration::new(60, 0);
        match self.create_guard(&req, _parent as usize, _name.into(), _mode, _umask, _flags) {
            Err(e) => reply.error(e),
            Ok(fh) => {
                match self.fh(fh, req.pid) {
                    None => reply.error(EBADF),
                    Some(handler) => {
                        let inode = handler.inode_with_id();
                        reply.created(&ttl, &inode.into(), 0, fh as u64, _flags as u32)
                    }
                }
            }
        }
    }
}

impl Into<fuser::FileType> for FileType {
    fn into(self) -> fuser::FileType {
        match self {
            FileType::Socket => fuser::FileType::Socket,
            FileType::SymbolLink => fuser::FileType::Symlink,
            FileType::File => fuser::FileType::RegularFile,
            FileType::BlockDevice => fuser::FileType::BlockDevice,
            FileType::Dir => fuser::FileType::Directory,
            FileType::CharDevice => fuser::FileType::CharDevice,
            FileType::FIFO => fuser::FileType::NamedPipe,
            FileType::UNK => fuser::FileType::RegularFile,
        }
    }
}

impl Into<FileAttr> for InodeWithId {
    fn into(self) -> FileAttr {
        let attr = FileAttr {
            ino: self.inode as u64,
            size: self.data.size,
            blocks: self.blocks(),
            atime: system_time_from_time(self.data.modified as i64, 0),
            mtime: system_time_from_time(self.data.modified as i64, 0),
            ctime: system_time_from_time(self.data.created as i64, 0),
            crtime: system_time_from_time(self.data.created as i64, 0),
            kind: FileType::from(self.data.mode).into(),
            perm: self.permission(),
            nlink: self.data.link_count,
            uid: self.data.uid,
            gid: self.data.gid,
            rdev: 0,
            blksize: (self.blocks() as usize * BLOCK_SIZE) as u32,
            padding: 0,
            flags: 0,
        };
        // debug!("Attr:{:?}",attr);
        attr
    }
}

impl<'a> Into<Req> for &Request<'a> {
    fn into(self) -> Req {
        Req {
            uid: self.uid(),
            gid: self.gid(),
            pid: 0,
        }
    }
}

fn cast(ino_: u64) -> usize {
    (ino_) as usize
}
