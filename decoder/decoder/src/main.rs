#![no_std]
#![no_main]

use bytemuck::PodCastError;
use bytemuck::{checked::CheckedCastError, must_cast_slice};
use core::panic::PanicInfo;
use core::time::Duration;
use cortex_m_rt::entry;
use decoder_context::{DecoderContext, DecoderContextError};
use max78000_hal::led::{led_off, led_on, Led};
use max78000_hal::timer::sleep;
use max78000_hal::HalError;
use message::{Message, MessageError, Opcode};
use thiserror_no_std::Error;
use utils::{write_error, Cursor, CursorError};

mod crypto;
mod decode;
mod decoder_context;
mod ectf_params;
mod message;
mod subscribe;
mod utils;

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("Error in the HAL: {0}")]
    HalError(#[from] HalError),
    #[error("Error interpreting bytes: {0}")]
    CastError(#[from] CheckedCastError),
    #[error("Error casting bytes: {0}")]
    PodCastError(#[from] PodCastError),
    #[error("Error: Suspicious activity detected")]
    SuspiciousActivity,
    #[error("Error: timestamp not found in subtrees")]
    NoTimestampFound,
    #[error("Error: timestamp is out of bounds")]
    InvalidTimestamp,
    #[error("Error: non-monotonic timestamp")]
    NonMonotonicTimestamp,
    #[error("Error: invalid payload received")]
    InvalidEncoderPayload,
    #[error("Error: subscription is not valid for decoding the given frame")]
    InvalidSubscription,
    #[error("Messaging error: {0}")]
    MessagingError(#[from] MessageError),
    #[error("Cursor error: {0}")]
    CursorError(#[from] CursorError),
    #[error("Decoder context error: {0}")]
    DecoderContextError(#[from] DecoderContextError),
}

/// Performs the list channels functionality required by host tools.
fn list_channels(context: &mut DecoderContext) -> Result<(), DecoderError> {
    let channel_info = context.list_channels();

    let mut data = [0; message::MAX_BODY_SIZE];
    let mut data_cursor = Cursor::new(&mut data);
    // first 4 bytes is number of channels
    data_cursor.read_from(&(channel_info.len() as u32).to_le_bytes())?;

    // next bytes are info about channels
    data_cursor.read_from(must_cast_slice(channel_info.as_slice()))?;
    let data = data_cursor.written();

    let response = Message::from_data(Opcode::List, data);
    response.write()?;

    Ok(())
}

#[entry]
fn main() -> ! {
    let mut context = DecoderContext::new();
    // there is a 1 second power up limit
    sleep(Duration::from_millis(900));
    led_on(Led::Green);

    loop {
        if let Ok(mut message) = Message::read() {
            //let opcode = message.opcode;
            //println!("got message: {opcode:?}");
            let result = match message.opcode {
                Opcode::List => list_channels(&mut context),
                Opcode::Subscribe => subscribe::subscribe(&mut context, message.data_mut()),
                Opcode::Decode => decode::decode(&mut context, message.data_mut()),
                _ => Ok(()),
            };

            if let Err(error) = result {
                write_error(&error).expect("Failed to report error");
            }
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    led_on(Led::Red);
    led_off(Led::Blue);
    led_off(Led::Green);
    let _ = write_error(info);
    loop {}
}
