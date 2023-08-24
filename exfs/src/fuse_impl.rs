use std::cmp::min;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::str::FromStr;

use fuse::{
    FileAttr, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, ReplyXattr, Request,
};
use libc::O_RDWR;
use time::Timespec;

use crate::cache::file_handler::FileHandler;
use crate::layout::data_block::FileName;
use crate::layout::inode::{DIR, FILE, FileType, Inode};
use crate::manager::block_cache_manager::{BlockCacheDevice, trim_zero};

impl Filesystem for BlockCacheDevice {
    fn read(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        _size: u32,
        reply: ReplyData,
    ) {
        let mut buf: Vec<u8> = Vec::new();
        for _ in 0.._size {
            buf.push(0)
        }
        FileHandler {
            inode: ino_id(_ino),
            offset: _offset as usize,
            flags: 0,
        }
            .read(self, &mut buf);
        println!("Read {}: 【{:?}】", _ino, trim_zero(buf.clone()));
        reply.data(&buf)
    }

    fn write(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _offset: i64,
        _data: &[u8],
        _flags: u32,
        reply: ReplyWrite,
    ) {
        match self.write_inner(_offset as usize, ino_id(_ino), _data) {
            Ok(_) => {
                self.modify_inode(ino_id(_ino), |ino| ino.size = _data.len() as u64);
                reply.written(_data.len() as u32)
            }
            Err(e) => reply.error(e),
        }
    }
    fn open(&mut self, _req: &Request, _ino: u64, _flags: u32, reply: ReplyOpen) {
        let fh = self.open_inner(ino_id(_ino), 0, _flags as u16);
        reply.opened(fh, _flags)
    }

    fn forget(&mut self, _req: &Request, _ino: u64, _nlookup: u64) {
        // println!("Forget: {}", _ino)
    }

    fn getxattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _name: &OsStr,
        _size: u32,
        reply: ReplyXattr,
    ) {
        // println!("GetXAttr: {}", _ino);
        reply.size(0)
    }

    fn setattr(
        &mut self,
        _req: &Request,
        _ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<Timespec>,
        _mtime: Option<Timespec>,
        _fh: Option<u64>,
        _crtime: Option<Timespec>,
        _chgtime: Option<Timespec>,
        _bkuptime: Option<Timespec>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let inode = self.modify_inode(ino_id(_ino), |ino| {
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
            // if let Some(_) = _atime {
            //     // ino. = v
            // }
            if let Some(v) = _mtime {
                ino.modified = v.sec as u64
            }
            if let Some(v) = _crtime {
                ino.created = v.sec as u64
            }
            ino.clone()
        });
        let ttl = Timespec::new(60, 0);
        reply.attr(&ttl, &file_attr(inode, _ino))
    }

    fn readlink(&mut self, _req: &Request, _ino: u64, _reply: ReplyData) {
        // println!("ReadLink: {}", _ino)
    }

    fn mknod(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        let file = self.mk_file(
            _name.to_str().unwrap(),
            ino_id(_parent),
            FILE << 12 | _mode as u16,
        );
        let ttl = Timespec::new(60, 0);
        match file {
            Ok(v) => {
                let inode = self.inode(v);
                let attr = file_attr(inode, id_ino(v));
                println!("Mknod: v:{}, {:#?}", v, attr);
                reply.entry(&ttl, &attr, 0)
            }
            Err(e) => {
                println!("Mknod error: {:?}", e);
                reply.error(e)
            }
        }
    }

    fn create(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        _flags: u32,
        reply: ReplyCreate,
    ) {
        let file = self.mk_file(
            _name.to_str().unwrap(),
            ino_id(_parent),
            FILE << 12 | _mode as u16,
        );
        let ttl = Timespec::new(60, 0);
        match file {
            Ok(v) => {
                let inode = self.inode(v);
                let attr = file_attr(inode, id_ino(v));
                let fh = self.open_inner(v, 0, _flags as u16);
                // println!("Create: v:{}, {:#?}", v, attr);
                reply.created(&ttl, &attr, 0, fh, _flags);
            }
            Err(e) => {
                println!("Create error: {:?}", e);
                reply.error(e)
            }
        }
    }

    fn unlink(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEmpty) {
        let parent_id = ino_id(_parent);
        match self.rm(parent_id, name2(_name), false) {
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
        _reply: ReplyEntry,
    ) {
        // println!("Link: {}", _ino)
    }

    fn rmdir(&mut self, _req: &Request, _parent: u64, _name: &OsStr, _reply: ReplyEmpty) {
        // println!("RmDir: {:?}", _name)
    }

    fn rename(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _newparent: u64,
        _newname: &OsStr,
        reply: ReplyEmpty,
    ) {
        let parent = ino_id(_parent);
        let new_parent = ino_id(_newparent);
        match self.rename_inner(parent, name2(_name), new_parent, name2(_newname)) {
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
        _reply: ReplyEntry,
    ) {
        // println!("SymLink: {:?}", _name)
    }

    fn access(&mut self, _req: &Request, _ino: u64, _mask: u32, reply: ReplyEmpty) {
        // println!("Access: {}", _ino);
        reply.ok()
    }

    fn opendir(&mut self, _req: &Request, _ino: u64, _flags: u32, reply: ReplyOpen) {
        // println!("OpenDir: {}", _ino);
        reply.opened(_ino, O_RDWR as u32);
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
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        match self.close_inner(_fh) {
            Ok(_) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, _reply: ReplyStatfs) {
        // println!("StatsFS: {}", _ino)
    }

    fn lookup(&mut self, _req: &Request, _parent: u64, _name: &OsStr, reply: ReplyEntry) {
        // println!("parent:{},req:{:?},name:{:?}", _parent, _req, _name);
        let ttl = Timespec::new(60, 0);
        let r = reply;
        match self.lookup(ino_id(_parent), name2(_name)) {
            Ok(entry) => {
                let inode = self.inode(entry.inode as usize);
                r.entry(&ttl, &file_attr(inode, id_ino(entry.inode as usize)), 0);
            }
            Err(e) => {
                println!("Lookup error: {:?}", e);
                r.error(e);
            }
        }
    }

    fn getattr(&mut self, _req: &Request, _ino: u64, reply: ReplyAttr) {
        let ttl = Timespec::new(60, 0);
        let inode_id = ino_id(_ino);
        let inode = self.inode(inode_id);
        let attr = file_attr(inode, _ino);
        reply.attr(&ttl, &attr)
    }

    fn mkdir(
        &mut self,
        _req: &Request,
        _parent: u64,
        _name: &OsStr,
        _mode: u32,
        reply: ReplyEntry,
    ) {
        let folder = self.mk_file(
            _name.to_str().unwrap(),
            ino_id(_parent),
            DIR << 12 | _mode as u16,
        );
        let ttl = Timespec::new(60, 0);
        match folder {
            Ok(v) => {
                let inode = self.inode(v);
                let attr = file_attr(inode, id_ino(v));
                // println!("Mkdir: v:{}, {:#?}", v, attr);
                reply.entry(&ttl, &attr, 0)
            }
            Err(e) => {
                println!("Mkdir error: {:?}", e);
                reply.error(e)
            }
        }
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
        match self.ls_(ino_id(_ino)) {
            Ok(v) => {
                r.add(
                    _ino,
                    1,
                    fuse::FileType::Directory,
                    OsString::from_str("..").unwrap(),
                );
                for entry in v {
                    println!("DirEntry: {:?} ({})", name(entry.name), entry.inode);
                    let inode = self.inode(entry.inode as usize);
                    r.add(
                        entry.inode,
                        1,
                        file_type(inode.file_type()),
                        name(entry.name).as_os_str(),
                    );
                }
            }
            Err(e) => {
                println!("ReadDir error: {:?}", e)
            }
        }
        r.ok()
    }
}

fn file_type(typ: FileType) -> fuse::FileType {
    match typ {
        FileType::Socket => fuse::FileType::Socket,
        FileType::SymbolLink => fuse::FileType::Symlink,
        FileType::File => fuse::FileType::RegularFile,
        FileType::BlockDevice => fuse::FileType::BlockDevice,
        FileType::Dir => fuse::FileType::Directory,
        FileType::CharDevice => fuse::FileType::CharDevice,
        FileType::FIFO => fuse::FileType::NamedPipe,
        FileType::UNK => fuse::FileType::RegularFile,
    }
}

fn name(name: FileName) -> OsString {
    OsString::from_vec(trim_zero(name.to_vec()))
}

fn name2(name: &OsStr) -> FileName {
    let mut file_name = [0u8; 56];
    let name_str = name.to_str().unwrap();
    let len = min(name_str.len(), 56);
    file_name[..len].copy_from_slice(name_str[..len].as_bytes());
    file_name
}

fn file_attr(inode: Inode, _ino: u64) -> FileAttr {
    let mode = inode.mode & ((1 << 9) - 1);
    // println!(
    //     "FMode: {:b},{:o}, Type: {:?}",
    //     inode.mode,
    //     mode,
    //     inode.file_type()
    // );
    FileAttr {
        ino: _ino,
        size: inode.size,
        blocks: inode.blocks(),
        atime: Timespec::new(inode.modified as i64, 0),
        mtime: Timespec::new(inode.modified as i64, 0),
        ctime: Timespec::new(inode.created as i64, 0),
        crtime: Timespec::new(inode.created as i64, 0),
        perm: mode,
        kind: file_type(inode.file_type()),
        nlink: inode.link_count,
        uid: inode.uid,
        gid: inode.gid,
        rdev: 0,
        flags: 0,
    }
}

fn ino_id(ino_: u64) -> usize {
    (ino_) as usize
}

fn id_ino(inode_id: usize) -> u64 {
    (inode_id) as u64
}
