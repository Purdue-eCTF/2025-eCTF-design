use max78000_hal::uart::uart;
use thiserror_no_std::Error;

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("Unknown error")]
    Unknown,
    #[error("Unexpected opcode: {0:x}")]
    UnexpectedOpcode(u8),
    #[error("Incorrect magic: {0:x}")]
    IncorrectMagic(u8),
    #[error("Nonzero body length on ACK packet")]
    AckError,
    #[error("Body too long")]
    BodyLengthError,
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

impl From<Opcode> for u8 {
    fn from(opcode: Opcode) -> u8 {
        match opcode {
            Opcode::Decode => 0x44,
            Opcode::Subscribe => 0x53,
            Opcode::List => 0x4c,
            Opcode::Ack => 0x41,
            Opcode::Debug => 0x47,
            Opcode::Error => 0x45,
        }
    }
}

/// Value required at the start of every message
pub const MAGIC: u8 = b'%';
/// Maximum body size for a message: 4.5 KiB
pub const MAX_BODY_SIZE: usize = 4608;
/// Messages get sent in 256-byte chunks, with an ack for each chunk
const CHUNK_SIZE: usize = 256;

/// Opcodes that don't require an ack response.
const NACKS: [Opcode; 2] = [Opcode::Debug, Opcode::Ack];

#[derive(Debug)]
pub struct Message {
    pub opcode: Opcode,
    pub length: u16,
    pub body: [u8; MAX_BODY_SIZE],
}

impl Message {
    /// Creates a new message from the given opcode, length, and body.
    pub fn new(opcode: Opcode, length: u16, body: [u8; MAX_BODY_SIZE]) -> Self {
        Self {
            opcode,
            length,
            body,
        }
    }

    /// Creates a new message from the given opcode and byte slice.
    #[inline]
    pub fn from_data(opcode: Opcode, data: &[u8]) -> Self {
        let mut body = [0; MAX_BODY_SIZE];
        body[..data.len()].copy_from_slice(data);

        Self::new(opcode, data.len() as u16, body)
    }

    /// Gets the message body as a mutable byte slice.
    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.body[..self.length.into()]
    }

    /// Reads a message header from UART without reading the body.
    #[inline]
    pub fn read_header() -> Result<Self, MessageError> {
        let reader = uart();

        let magic = reader.read_byte();
        if magic != MAGIC {
            return Err(MessageError::IncorrectMagic(magic));
        }

        let opcode_byte = reader.read_byte();
        let Ok(opcode) = Opcode::try_from(opcode_byte) else {
            return Err(MessageError::UnexpectedOpcode(opcode_byte));
        };

        let mut length = [0, 0];
        reader.read_bytes(&mut length);
        let length = u16::from_le_bytes(length);
        Ok(Self {
            opcode,
            length,
            body: [0; MAX_BODY_SIZE],
        })
    }

    /// Reads a complete message from UART.
    pub fn read() -> Result<Self, MessageError> {
        let reader = uart();
        let mut message = Self::read_header()?;

        Self::send_ack();

        if message.length != 0 {
            for chunk in message.body[..message.length.into()].chunks_mut(CHUNK_SIZE) {
                reader.read_bytes(chunk);
                Self::send_ack();
            }
        }

        Ok(message)
    }

    /// Writes an ack to UART.
    pub fn send_ack() {
        let ack = Self::ack();
        ack.write_header();
    }

    /// Reads an ack from UART.
    pub fn read_ack() -> Result<(), MessageError> {
        let message = Self::read_header()?;

        if message.length != 0 {
            Err(MessageError::AckError)
        } else {
            Ok(())
        }
    }

    /// Writes this message's header to UART.
    pub fn write_header(&self) {
        let writer = uart();

        writer.write_byte(MAGIC);
        writer.write_byte(self.opcode.into());
        writer.write_bytes(&self.length.to_le_bytes());
    }

    /// Writes this message to UART.
    pub fn write(&self) -> Result<(), MessageError> {
        self.write_header();
        if !NACKS.contains(&self.opcode) {
            Self::read_ack()?;
        }
        let writer = uart();

        if self.length != 0 {
            for chunk in self.body[..self.length.into()].chunks(CHUNK_SIZE) {
                writer.write_bytes(chunk);
                if !NACKS.contains(&self.opcode) {
                    Self::read_ack()?;
                }
            }
        }
        Ok(())
    }

    /// Convenience method to immediately create and send a message from the given
    /// opcode and byte slice.
    pub fn send_data(opcode: Opcode, data: &[u8]) -> Result<(), MessageError> {
        if data.len() > MAX_BODY_SIZE {
            return Err(MessageError::BodyLengthError);
        }
        let msg = Message::from_data(opcode, data);
        msg.write()
    }

    /// Static method to generate an ack message.
    pub const fn ack() -> Self {
        Self {
            opcode: Opcode::Ack,
            length: 0,
            body: [0; MAX_BODY_SIZE],
        }
    }
}
