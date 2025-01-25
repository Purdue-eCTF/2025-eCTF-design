use core::{
    fmt::{self},
    time::Duration,
};

use max78000_hal::{
    led::{led_off, led_on, Led},
    timer::sleep,
};

use crate::message::{Message, MessageError, Opcode, MAX_BODY_SIZE};

pub struct SliceWriteWrapper<'a> {
    buf: &'a mut [u8],
    pub offset: usize,
}

impl<'a> SliceWriteWrapper<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        SliceWriteWrapper { buf, offset: 0 }
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

#[allow(unused)]
pub fn flash_red(n: usize) {
    for _ in 0..n {
        led_on(Led::Red);
        sleep(Duration::from_millis(250));
        led_off(Led::Red);
        sleep(Duration::from_millis(250));
    }
}

#[allow(unused)]
pub fn write_debug_message(message: &str) -> Result<(), MessageError> {
    let message_bytes = message.as_bytes();
    for chunk in message_bytes.chunks(MAX_BODY_SIZE) {
        let mut buf = [0; MAX_BODY_SIZE];
        buf[..chunk.len()].copy_from_slice(chunk);
        let debug_message = Message::new(Opcode::Debug, chunk.len() as u16, buf);
        debug_message.write()?;
    }
    Ok(())
}