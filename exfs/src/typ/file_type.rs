use std::ops::{Shl, Shr};

#[derive(Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum FileType {
    Socket = 0b1100,
    SymbolLink = 0b1010,
    File = 0b1000,
    BlockDevice = 0b0110,
    Dir = 0b0100,
    CharDevice = 0b0010,
    FIFO = 0b1,
    UNK = 0,
}

impl Shr<u16> for FileType {
    type Output = u16;

    fn shr(self, rhs: u16) -> Self::Output {
        (self as u16) >> rhs
    }
}
impl Shl<u16> for FileType {
    type Output = u16;

    fn shl(self, lhs: u16) -> Self::Output {
        (self as u16) << lhs
    }
}

impl From<u16> for FileType {
    fn from(value: u16) -> Self {
        match value {
            0b1100 => FileType::Socket,
            0b1010 => FileType::SymbolLink,
            0b1000 => FileType::File,
            0b0110 => FileType::BlockDevice,
            0b0100 => FileType::Dir,
            0b0010 => FileType::CharDevice,
            0b1 => FileType::FIFO,
            _ => FileType::UNK,
        }
    }
}
