use core::fmt::{self};
pub struct SliceWriteWrapper<'a> {
    buf: &'a mut [u8],
    pub offset: usize,
}

impl<'a> SliceWriteWrapper<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        SliceWriteWrapper {
            buf: buf,
            offset: 0,
        }
    }
}

impl<'a> fmt::Write for SliceWriteWrapper<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remainder = &mut self.buf[self.offset..];
        if remainder.len() < bytes.len() {
            return Err(fmt::Error);
        }
        let remainder = &mut remainder[..bytes.len()];
        remainder.copy_from_slice(bytes);
        self.offset += bytes.len();

        Ok(())
    }
}
