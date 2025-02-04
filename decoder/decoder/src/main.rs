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
use utils::{write_error, CursorError};

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
    #[error("Error: invalid payload recieved")]
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

fn list_channels(context: &mut DecoderContext) -> Result<(), DecoderError> {
    let channel_info = context.list_channels();
    let channel_info_bytes = must_cast_slice(channel_info.as_slice());

    let mut data = [0; message::MAX_BODY_SIZE];

    // first 4 bytes is number of channels
    data[0..4].copy_from_slice(&(channel_info.len() as u32).to_le_bytes());
    // next bytes is info about channels
    data[4..(4 + channel_info_bytes.len())].copy_from_slice(channel_info_bytes);

    let response = Message::new(Opcode::List, (channel_info_bytes.len() + 4) as u16, data);
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
            let opcode = message.opcode;
            println!("got message: {opcode:?}");
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
    let _ = utils::write_error(info);
    loop {}
}
