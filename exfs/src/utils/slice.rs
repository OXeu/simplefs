use std::mem::size_of;

pub fn vec2slice<T: Sized>(data: Vec<T>) -> Vec<u8> {
    data.iter()
        .map(|v| unsafe { std::slice::from_raw_parts(v as *const T as *const u8, size_of::<T>()) })
        .flatten()
        .cloned()
        .collect()
}

pub fn slice<T: Sized>(data: &T) -> &'static [u8] {
    unsafe { std::slice::from_raw_parts(data as *const T as *const u8, size_of::<T>()) }
}

pub trait SliceExt {
    fn trim(&self) -> &Self;
}

impl SliceExt for [u8] {
    fn trim(&self) -> &[u8] {
        fn is_whitespace(c: &u8) -> bool {
            *c == b'\t' || *c == b' ' || *c == 0
        }

        fn is_not_whitespace(c: &u8) -> bool {
            !is_whitespace(c)
        }

        if let Some(first) = self.iter().position(is_not_whitespace) {
            if let Some(last) = self.iter().rposition(is_not_whitespace) {
                &self[first..last + 1]
            } else {
                unreachable!();
            }
        } else {
            &[]
        }
    }
}

pub fn empty_u8(len: usize) -> Vec<u8> {
    let mut u = Vec::new();
    for _ in 0..len {
        u.push(0u8)
    }
    u
}

pub fn align(buf: &mut Vec<u8>, align: usize) {
    let target = (buf.len() + align - 1) / align * align;
    for _ in buf.len()..target {
        buf.push(0)
    }
}
