#![no_std]
#![no_main]

use bytemuck::{checked::CheckedCastError, must_cast_slice};
use core::fmt::Write;
use core::panic::PanicInfo;
use core::str::Utf8Error;
use core::time::Duration;
use cortex_m_rt::entry;
use decoder_context::DecoderContext;
use design_utils::messages::ProtocolError;
use design_utils::DesignUtilsError;
use max78000_hal::led::{led_off, led_on, Led};
use max78000_hal::timer::sleep;
use max78000_hal::uart::uart;
use max78000_hal::HalError;
use message::{Message, Opcode};
use thiserror_no_std::Error;
use utils::SliceWriteWrapper;
mod decoder_context;
mod ectf_params;
mod message;
mod utils;

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("Error in the HAL: {0}")]
    HalError(#[from] HalError),
    #[error("Error interpreting bytes: {0}")]
    CastError(#[from] CheckedCastError),
    #[error("An invalid component was detected")]
    InvalidComponentError,
    #[error("An invalid pin, secret key, or component id was entered")]
    InvalidInput,
    #[error("Command not recognized")]
    InvalidCommand,
    #[error("Message with invalid utf8 received: {0}")]
    InvalidUtf8(#[from] Utf8Error),
    #[error("Error in design utils: {0}")]
    DesignUtilsError(#[from] DesignUtilsError),
    #[error("Invalid challenge response for component {0}")]
    InvalidChallengeResponse(usize),
    #[error("Error while serializing or deserializing message: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("Component reported en error occurred")]
    ProtocolError(#[from] ProtocolError),
    #[error("Error: Suspicious activity detected")]
    SuspiciousActivity,
    #[error("Error: invalid build id")]
    InvalidBuildId,
    #[error("Error: conditions for boot are not met")]
    InvalidBootConditions,
}

fn list_channels(context: &mut DecoderContext) {
    let channel_info = context.list_channels();
    let channel_info_bytes = must_cast_slice(channel_info.as_slice());

    let mut data = [0; message::MAX_BODY_SIZE];

    // first 4 bytes is number of channels
    data[0..4].copy_from_slice(&(channel_info.len() as u32).to_le_bytes());
    // next bytes is info about channels
    data[4..(4 + channel_info_bytes.len())].copy_from_slice(channel_info_bytes);

    let response = Message::new(Opcode::List, (channel_info_bytes.len() + 4) as u16, data);
    // TODO: handle error
    response.write().unwrap();
}

#[entry]
fn main() -> ! {
    // safety: no critical sections depending on interrupts are currently held
    unsafe {
        cortex_m::interrupt::enable();
    }

    let mut context = DecoderContext::new();
    sleep(Duration::from_millis(100));
    led_on(Led::Green);

    loop {
        if let Ok(message) = Message::read() {
            match message.opcode {
                Opcode::List => list_channels(&mut context),
                _ => (),
            }
            // utils::write_debug_message("does it work??");
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    led_on(Led::Red);
    led_off(Led::Blue);
    led_off(Led::Green);
    let writer = uart();
    let mut panic_msg = [0u8; 2048];
    let mut wrapper = SliceWriteWrapper::new(panic_msg.as_mut_slice());
    if write!(wrapper, "{}", info).is_ok() {
        let num_bytes = wrapper.offset;
        writer.write_byte(message::MAGIC);
        writer.write_byte(b'E');
        for b in (num_bytes as u16).to_le_bytes() {
            writer.write_byte(b);
        }
        for b in &panic_msg[..num_bytes] {
            writer.write_byte(*b);
        }
        loop {}
    }
    let panic_msg = b"%E\x16\x00panic message too long";

    for b in panic_msg {
        writer.write_byte(*b);
    }
    loop {}
}
