#![no_std]
#![feature(generic_const_exprs)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]

use thiserror_no_std::Error;

pub mod crypto;
pub mod messages;
pub mod str;
pub mod anti_hardware;
mod libc_functions;

// reesoirt for const time macro
pub use subtle;
use max78000_hal::i2c::I2cAddr;

/// The maximum size of a message the c post boot code will send
pub const MAX_POST_BOOT_MESSAGE_SIZE: usize = 64;

/// I²C frequency to be used in hertz
pub const I2C_FREQUENCY: u32 = 100000;

const COMPONENT_ADDR_MASK: u32 = 0x000000FF;

pub type ComponentId = u32;

/// Converts a `u32` component ID to its corresponding I²C address.
///
/// # Arguments
/// * `component_id` - The component ID to convert.
///
/// Returns the I²C address, as a `u8`.
pub fn component_id_to_i2c_addr(component_id: ComponentId) -> I2cAddr {
    (component_id & COMPONENT_ADDR_MASK) as I2cAddr
}

#[derive(Debug, Error)]
pub enum DesignUtilsError {
    #[error("There was an error encrypting or decrypting")]
    ChaChaError(#[from] chacha20poly1305::Error),
    #[error("There was an error with serializing or deserializing")]
    SerializeError(#[from] postcard::Error),
    #[error("The signature was invalid")]
    InvalidSignature,
    #[error("Insufficient capacity to hold the encrypted data")]
    InsuficientCapacity,
}
