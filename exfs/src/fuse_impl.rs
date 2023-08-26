use fuser::{
    FileAttr,
    Request,
};

use crate::config::BLOCK_SIZE;
use crate::layout::inode::InodeWithId;
use crate::manager::error_code::ErrorCode;
use crate::typ::file_type::FileType;
use crate::typ::request::Req;
use crate::utils::time::system_time_from_time;

fn reply_<T>(data: Result<T, ErrorCode>, error: impl FnOnce(ErrorCode), f: impl FnOnce(T)) {
    match data {
        Ok(v) => f(v),
        Err(e) => error(e),
    }
}
/*
impl Filesystem for BlockCacheDevice {
    fn lookup(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEntry) {
        let ttl = Duration::new(60, 0);
        reply_(
            self.lookup(&_req.into(), cast(_parent), _name.into()),
            |e| reply.error(e),
            |entry| reply.entry(&ttl, &entry, 0),
        )
    }

    fn getattr(&mut self, _req: &Request, _ino: u64, reply: ReplyAttr) {
        let ttl = Duration::new(60, 0);
        reply_(
            self.getattr(&_req.into(), cast(_ino)),
            |e| reply.error(e),
            |data| {
                reply.attr(&ttl, &data);
            },
        )
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
        reply_(
            self.setattr(
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
                _flags,
            ),
            |e| reply.error(e),
            |attr| reply.attr(&ttl, &attr),
        );
    }

    fn readlink(&mut self, _req: &Request, _ino: u64, reply: ReplyData) {
        // debug!("ReadLink: {}", _ino)
        let buf = self.read_all(cast(_ino));
        reply.data(buf.as_ref())
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
        let file = self.make_node_internal(_name.to_str().unwrap(), cast(_parent), _mode as u16);
        let ttl = Duration::new(60, 0);
        match file {
            Ok(v) => {
                let inode = self.inode(v);
                let attr = inode.with_id(v).into();
                debug!("Mknod: v:{}, {:#?}", v, attr);
                reply.entry(&ttl, &attr, 0)
            }
            Err(e) => {
                debug!("Mknod error: {:?}", e);
                reply.error(e)
            }
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
        let folder = self.make_node_internal(
            _name.to_str().unwrap(),
            cast(_parent),
            FileType::Dir << 12 | _mode as u16,
        );
        let ttl = Duration::new(60, 0);
        match folder {
            Ok(v) => {
                let inode = self.inode(v);
                let attr = inode.with_id(v).into();
                // debug!("Mkdir: v:{}, {:#?}", v, attr);
                reply.entry(&ttl, &attr, 0)
            }
            Err(e) => {
                debug!("Mkdir error: {:?}", e);
                reply.error(e)
            }
        }
    }

    fn unlink(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEmpty) {
        let parent_id = cast(_parent);
        match self.rm(parent_id, _name.into()) {
            Ok(_) => reply.ok(),
            Err(e) => reply.error(e),
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
        // debug!("SymLink: {:?}", _name)
        let symbol = self.make_node_internal(
            _name.to_str().unwrap(),
            cast(_parent),
            FileType::SymbolLink << 12 | 0o744u16,
        );
        let ttl = Duration::new(60, 0);
        match symbol {
            Ok(v) => {
                let buf = _link.to_str().unwrap();
                let inode = self.inode(v);
                if let Err(e) = self.write_all(0, v, buf.as_ref(), true) {
                    reply.error(e);
                } else {
                    let attr = inode.with_id(v).into();
                    reply.entry(&ttl, &attr, 0);
                }
            }
            Err(e) => {
                debug!("Symbol link error: {:?}", e);
                reply.error(e)
            }
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
        let parent = cast(_parent);
        let new_parent = cast(_newparent);
        match self.rename_internal(parent, _name.into(), new_parent, _newname.into()) {
            Ok(_) => reply.ok(),
            Err(e) => reply.error(e),
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
        println!("FFFFFFFFFF");
        let inode_id = cast(_ino);
        let parent_id = cast(_newparent);
        let mut dirs = self.dir_list(parent_id);
        match self.lookup_internal(parent_id, _newname.into()) {
            Err(ENOENT) => {
                dirs.push(DirEntry {
                    name: _newname.into(),
                    inode: inode_id as u64,
                });
                let buf: Vec<u8> = vec2slice(dirs);
                if let Err(e) = self.write_all(0, parent_id, &buf, true) {
                    debug!("link:235 error: {}", e);
                    reply.error(e);
                    return;
                }
                let inode = self.inode(inode_id);
                let ttl = Duration::new(60, 0);
                reply.entry(&ttl, &inode.with_id(_ino as usize).into(), 0)
            }
            Ok(_) => reply.error(EEXIST),
            Err(e) => {
                reply.error(e);
            }
        }
    }

    fn open(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        let fh = self.open_inner(cast(_ino), 0, _flags as u16);
        reply.opened(fh, _flags as u32)
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
        FileHandler {
            inode: cast(_ino),
            offset: _offset as usize,
            flags: 0,
        }
        .read(self, &mut buf);
        debug!("Read {}: 【{:?}】", _ino, trim_zero(buf.clone()));
        reply.data(&buf)
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
        match self.write_inner(_offset as usize, cast(_ino), _data) {
            Ok(_) => reply.written(_data.len() as u32),
            Err(e) => reply.error(e),
        }
    }

    fn flush(&mut self, _req: &Request, _ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        self.sync();
        reply.ok()
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
        match self.close_inner(_fh) {
            Ok(_) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn opendir(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        // debug!("OpenDir: {}", _ino);
        reply.opened(_ino, O_RDWR as u32);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        reply: ReplyDirectory,
    ) {
        let mut r = reply;
        if _offset != 0 {
            r.ok();
            return;
        }
        match self.ls_(cast(_ino)) {
            Ok(v) => {
                let _ = r.add(
                    _ino,
                    1,
                    fuser::FileType::Directory,
                    OsString::from_str("..").unwrap(),
                );
                for entry in v {
                    debug!("DirEntry: {:?} ({})", entry.name, entry.inode);
                    let inode = self.inode(entry.inode as usize);
                    let _ = r.add(
                        entry.inode,
                        1,
                        inode.file_type().into(),
                        OsString::from(entry.name).as_os_str(),
                    );
                }
            }
            Err(e) => {
                debug!("ReadDir error: {:?}", e)
            }
        }
        r.ok()
    }

    fn getxattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _name: &OsStr,
        _size: u32,
        reply: ReplyXattr,
    ) {
        // debug!("GetXAttr: {}", _ino);
        reply.error(ENOTSUP);
    }

    fn access(&mut self, _req: &Request, _ino: u64, _mask: i32, reply: ReplyEmpty) {
        // debug!("Access: {}", _ino);
        reply.ok()
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
        let file = self.make_node_internal(
            _name.to_str().unwrap(),
            cast(_parent),
            FileType::File << 12 | _mode as u16,
        );
        let ttl = Duration::new(60, 0);
        match file {
            Ok(v) => {
                let inode = self.inode(v);
                let attr = inode.with_id(v).into();
                let fh = self.open_inner(v, 0, _flags as u16);
                // debug!("Create: v:{}, {:#?}", v, attr);
                reply.created(&ttl, &attr, 0, fh, _flags as u32);
            }
            Err(e) => {
                debug!("Create error: {:?}", e);
                reply.error(e)
            }
        }
    }
}
*/
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
        FileAttr {
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
        }
    }
}

impl<'a> Into<Req> for &Request<'a> {
    fn into(self) -> Req {
        Req {
            uid: self.uid(),
            gid: self.gid(),
            pid: self.pid(),
        }
    }
}

fn cast(ino_: u64) -> usize {
    (ino_) as usize
}
