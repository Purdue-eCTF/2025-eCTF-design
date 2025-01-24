use core::cmp::min;
use max78000_hal::uart::uart;
use thiserror_no_std::Error;

// TODO: add error types
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("Unknown error")]
    Unknown,
}

/*
class Opcode(IntEnum):
    """Enum class for use in device output processing."""

    DECODE = 0x44  # D
    SUBSCRIBE = 0x53  # S
    LIST = 0x4C  # L
    ACK = 0x41  # A
    DEBUG = 0x47  # G
    ERROR = 0x45  # E
*/
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opcode {
    Decode,
    Subscribe,
    List,
    Ack,
    Debug,
    Error,
}

impl TryFrom<u8> for Opcode {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, ()> {
        match value {
            0x44 => Ok(Self::Decode),
            0x53 => Ok(Self::Subscribe),
            0x4c => Ok(Self::List),
            0x41 => Ok(Self::Ack),
            0x47 => Ok(Self::Debug),
            0x45 => Ok(Self::Error),
            _ => Err(()),
        }
    }
}

impl Into<u8> for Opcode {
    fn into(self) -> u8 {
        match self {
            Self::Decode => 0x44,
            Self::Subscribe => 0x53,
            Self::List => 0x4c,
            Self::Ack => 0x41,
            Self::Debug => 0x47,
            Self::Error => 0x45,
        }
    }
}

impl Opcode {
    #[allow(unused)]
    fn name(&self) -> &'static str {
        match self {
            Self::Decode => "Decode",
            Self::Subscribe => "Subscribe",
            Self::List => "List",
            Self::Ack => "Ack",
            Self::Debug => "Debug",
            Self::Error => "Error",
        }
    }
}

pub const MAGIC: u8 = '%' as u8;
pub const MAX_BODY_SIZE: usize = 1024;
const CHUNK_SIZE: usize = 256;
const NACKS: [Opcode; 2] = [Opcode::Debug, Opcode::Ack];

pub struct Message {
    pub opcode: Opcode,
    pub length: u16,
    pub body: [u8; MAX_BODY_SIZE],
}

impl Message {
    pub fn new(opcode: Opcode, length: u16, body: [u8; MAX_BODY_SIZE]) -> Self {
        Self {
            opcode,
            length,
            body,
        }
    }

    // TODO: better error handling
    #[inline]
    pub fn read_header() -> Result<Self, ()> {
        let reader = uart();
        let magic = reader.read_byte();
        if magic != MAGIC {
            return Err(());
        }
        let Ok(opcode) = Opcode::try_from(reader.read_byte()) else {
            return Err(());
        };
        let mut length = [0, 0];
        for b in length.iter_mut() {
            // reader.read_bytes() special-cases newlines, which we don't want
            *b = reader.read_byte();
        }
        let length = u16::from_le_bytes(length);
        Ok(Self {
            opcode,
            length,
            body: [0; MAX_BODY_SIZE],
        })
    }

    pub fn read() -> Result<Self, ()> {
        let reader = uart();
        let mut message = Self::read_header()?;

        Self::send_ack();

        if message.length != 0 {
            for chunk in message.body[..message.length.into()].chunks_mut(CHUNK_SIZE) {
                for b in chunk.iter_mut() {
                    *b = reader.read_byte();
                }
                Self::send_ack();
            }
        }

        Ok(message)
    }

    pub fn send_ack() {
        let ack = Self::ack();
        ack.write_header();
    }

    pub fn read_ack() -> Result<(), ()> {
        let message = Self::read_header()?;

        if message.length != 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn write_header(&self) {
        let writer = uart();

        writer.write_byte(MAGIC);
        writer.write_byte(self.opcode.into());
        for b in self.length.to_le_bytes() {
            writer.write_byte(b);
        }
    }

    pub fn write(&self) -> Result<(), ()> {
        self.write_header();
        if !NACKS.contains(&self.opcode) {
            Self::read_ack()?;
        }
        let writer = uart();

        if self.length != 0 {
            for chunk in self.body[..self.length.into()].chunks(CHUNK_SIZE) {
                for b in chunk.into_iter().copied() {
                    writer.write_byte(b);
                }
                if !NACKS.contains(&self.opcode) {
                    Self::read_ack()?;
                }
            }
        }
        Ok(())
    }

    pub const fn ack() -> Self {
        Self {
            opcode: Opcode::Ack,
            length: 0,
            body: [0; MAX_BODY_SIZE],
        }
    }

    pub fn debug(text: &str) -> Self {
        // need to use length of bytes because utf8 could mess up length
        let text_bytes = text.as_bytes();

        let trunc_len = min(text_bytes.len(), MAX_BODY_SIZE);
        let text_trunc = &text_bytes[..trunc_len];
        let mut body = [0; MAX_BODY_SIZE];
        body[..text_trunc.len()].copy_from_slice(text_trunc);
        Self {
            opcode: Opcode::Debug,
            length: text_trunc.len() as u16,
            body,
        }
    }
}
