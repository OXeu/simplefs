use crate::layout::inode::Inode;
use crate::manager::error_code::{EPERM, ErrorCode};

#[derive(Debug)]
pub struct Req {
    pub uid: u32,
    pub gid: u32,
    pub pid: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Mask {
    F = 0,
    // 存在性检验
    R = 0b100,
    W = 0b010,
    X = 0b001,
    RW = 0b110,
    RX = 0b101,
    WX = 0b011,
    RWX = 0b111,
}

impl Mask {
    pub fn from_flag(flag: i32) -> Option<Mask> {
        match flag & 0b11 {
            0 => Some(Mask::R),
            1 => Some(Mask::W),
            2 => Some(Mask::RW),
            _ => None,
        }
    }
    pub fn from_mask(mask: i32) -> Mask {
        match mask & 0o111 {
            0b100 => Mask::R,
            0b010 => Mask::W,
            0b001 => Mask::X,
            0b110 => Mask::RW,
            0b101 => Mask::RX,
            0b011 => Mask::WX,
            0b111 => Mask::RWX,
            _ => Mask::F,
        }
    }
}

impl Inode {
    pub fn access(&self, uid: u32, gid: u32, mask: Mask) -> bool {
        if mask == Mask::F { return true; }
        let permission = if self.uid == 0 || self.uid == uid {
            self.mode >> 6 & 0o7
        } else if self.gid == 0 || self.gid == gid {
            self.mode >> 3 & 0o7
        } else {
            self.mode & 0o7
        };
        (permission & mask as u16) > 0
    }

    pub fn access_guard<T>(&self, req: &Req, mask: Mask, value: T) -> Result<T, ErrorCode> {
        if self.access(req.uid, req.gid, mask) {
            Ok(value)
        } else {
            println!("No permission: inode{:?} req:{:?} mask:{:?}", self, req, mask);
            Err(EPERM)
        }
    }
    pub fn access_guard_f<T>(
        &self,
        req: &Req,
        mask: Mask,
        value: impl FnOnce() -> Result<T, ErrorCode>,
    ) -> Result<T, ErrorCode> {
        if self.access(req.uid, req.gid, mask) {
            value()
        } else {
            println!("No permission function: inode{:?} req:{:?} mask:{:?}", self, req, mask);
            Err(EPERM)
        }
    }
}
