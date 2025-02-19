use core::{
    fmt::{self, Display, Write},
    time::Duration,
};

use max78000_hal::{
    led::{led_off, led_on, Led},
    timer::sleep,
};
use thiserror_no_std::Error;

use crate::message::{Message, MessageError, Opcode, MAX_BODY_SIZE};

pub struct Cursor<T> {
    buf: T,
    pub offset: usize,
}
#[derive(Debug, Error)]
pub enum CursorError {
    #[error("Too many bytes: only {0} remaining")]
    OversizeError(usize),
}

impl<T> Cursor<T> {
    pub fn new(buf: T) -> Self {
        Cursor { buf, offset: 0 }
    }
}

impl<T> Cursor<T>
where
    T: AsRef<[u8]>,
{
    /// Read bytes from this cursor into a buffer.
    /// If there is not enough bytes remaining to do so, return an error with how many bytes are left
    pub fn read_into(&mut self, other: &mut [u8]) -> Result<(), CursorError> {
        let remainder = &self.buf.as_ref()[self.offset..];
        if remainder.len() < other.len() {
            Err(CursorError::OversizeError(remainder.len()))
        } else {
            other.copy_from_slice(&remainder[..other.len()]);
            self.offset += other.len();
            Ok(())
        }
    }
}
impl<T> Cursor<T>
where
    T: AsMut<[u8]>,
{
    /// Read bytes from this cursor into a buffer.
    /// If there is not enough bytes remaining to do so, return an error with how many bytes are left
    pub fn read_from(&mut self, other: &[u8]) -> Result<(), CursorError> {
        let remainder = &mut self.buf.as_mut()[self.offset..];
        if remainder.len() < other.len() {
            Err(CursorError::OversizeError(remainder.len()))
        } else {
            let () = &mut remainder[..other.len()].copy_from_slice(other);
            self.offset += other.len();
            Ok(())
        }
    }

    pub fn written(&mut self) -> &mut [u8] {
        &mut self.buf.as_mut()[..self.offset]
    }
}

impl<T> fmt::Write for Cursor<T>
where
    T: AsMut<[u8]>,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remainder = &mut self.buf.as_mut()[self.offset..];
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

    if write!(writer, "{error}").is_ok() {
        let error_len = writer.offset;
        write_error_bytes(&message_buf[..error_len])
    } else {
        write_error_bytes(b"Error occured (error too long to send)")
    }
}

/// Called internally by print and println macros.
///
/// Prints formatted info as debug messages.
pub fn write_debug_format(args: fmt::Arguments) {
    let mut message_buf = [0; MAX_BODY_SIZE];

    let mut cursor = Cursor::new(&mut message_buf);
    cursor.write_fmt(args).unwrap();
    let message_len = cursor.offset;

    Message::send_data(Opcode::Debug, &message_buf[..message_len]).unwrap();
}

/// Prints to the uart port
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::utils::write_debug_format(format_args!($($arg)*)));
}

/// Prints to the uart port
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
