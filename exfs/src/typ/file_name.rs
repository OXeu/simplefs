use crate::manager::block_cache_manager::trim_zero;
use std::cmp::min;
use std::ffi::{OsStr, OsString};
use std::ops::Deref;
use std::os::unix::ffi::OsStringExt;

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct FileName([u8; 56]);

impl Deref for FileName {
    type Target = [u8; 56];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Into<FileName> for [u8; 56] {
    fn into(self) -> FileName {
        FileName { 0: self }
    }
}

impl From<FileName> for OsString {
    fn from(value: FileName) -> Self {
        OsString::from_vec(trim_zero(value.to_vec()))
    }
}

impl From<FileName> for String {
    fn from(value: FileName) -> Self {
        String::from_utf8(trim_zero(value.to_vec())).unwrap()
    }
}

impl Into<FileName> for &OsStr {
    fn into(self) -> FileName {
        let mut file_name = [0u8; 56];
        let name_str = self.to_str().unwrap();
        let len = min(name_str.len(), 56);
        file_name[..len].copy_from_slice(name_str[..len].as_bytes());
        file_name.into()
    }
}
