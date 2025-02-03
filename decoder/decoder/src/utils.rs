use core::{
    fmt::{self, Display, Write},
    time::Duration,
};

use max78000_hal::{
    led::{led_off, led_on, Led},
    timer::sleep,
};

use crate::message::{Message, MessageError, Opcode, MAX_BODY_SIZE};

pub struct Cursor<'a> {
    buf: &'a mut [u8],
    pub offset: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Cursor { buf, offset: 0 }
    }
    /// Read bytes from this cursor into a buffer.
    /// If there is not enough bytes remaining to do so, return an error with how many bytes are left
    pub fn read_into(&mut self, other: &mut [u8]) -> Result<(), usize> {
        let remainder = &self.buf[self.offset..];
        if remainder.len() < other.len() {
            Err(remainder.len())
        } else {
            other.copy_from_slice(&remainder[..other.len()]);
            self.offset += other.len();
            Ok(())
        }
    }
}

impl<'a> fmt::Write for Cursor<'a> {
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
    const DELAY_TIME: u64 = 100;
    for _ in 0..n {
        led_on(Led::Red);
        sleep(Duration::from_millis(DELAY_TIME));
        led_off(Led::Red);
        sleep(Duration::from_millis(DELAY_TIME));
    }
}

#[allow(unused)]
pub fn write_debug_message(message: &str) -> Result<(), MessageError> {
    let message_bytes = message.as_bytes();
    for chunk in message_bytes.chunks(MAX_BODY_SIZE) {
        Message::send_data(Opcode::Debug, chunk)?;
    }
    Ok(())
}

/// Sends the given `message` bytes as the body of an error packet to the host tools.
pub fn write_error_bytes(message: &[u8]) -> Result<(), MessageError> {
    // error can't be split across blocks I think
    // otherwise we might have a situation where host tools reports error for next command also
    // Message::send_data checks body length beforehand.
    Message::send_data(Opcode::Error, message)
}

/// Sends the error message for an error back to the host tools in an error packet.
pub fn write_error<E: Display>(error: &E) -> Result<(), MessageError> {
    let mut message_buf = [0; MAX_BODY_SIZE];
    let mut writer = Cursor::new(&mut message_buf);

    if write!(writer, "{}", error).is_ok() {
        let error_len = writer.offset;
        write_error_bytes(&message_buf[..error_len])
    } else {
        write_error_bytes(b"Error occured (error too long to send)")
    }
}
