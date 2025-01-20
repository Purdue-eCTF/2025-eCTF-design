#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::str::Utf8Error;
use core::time::Duration;

use cortex_m_rt::entry;
use bytemuck::checked::CheckedCastError;
use thiserror_no_std::Error;
use max78000_hal::prelude::*;
use max78000_hal::uart::uart;
use max78000_hal::HalError;
use max78000_hal::led::{led_on, led_off, Led};
use max78000_hal::timer::sleep;
use design_utils::{ComponentId, DesignUtilsError};
use design_utils::messages::ProtocolError;

use decoder_driver::DecoderDriver;

mod decoder_driver;
mod ectf_params;

#[derive(Debug, Error)]
pub enum ApError {
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


#[entry]
fn main() -> ! {
    // safety: no critical sections depending on interrupts are currently held
    unsafe {
        cortex_m::interrupt::enable();
    }

    let mut driver = Some(DecoderDriver::new());
    sleep(Duration::from_millis(950));

    led_on(Led::Blue);

    let mut command_buf = [0u8; 16];

    loop {
        // let Some(command) = recv_input_with_message("Enter Command: ", &mut command_buf) else {
        //     uprintln_error!("Unrecognized command");
        //     continue;
        // };

        // let command = command.trim();

        // uprintln_debug!("running command: {command}");

        // let result = match command {
        //     "list" => list::scan_components(driver.as_mut().unwrap()),
        //     "attest" => attest::attest(driver.as_mut().unwrap()),
        //     "replace" => replace::replace(driver.as_mut().unwrap()),
        //     "boot" => boot::attempt_boot(&mut driver),
        //     &_ => Err(ApError::InvalidCommand),
        // };

        // if let Err(_error) = result {
        //     //uprintln_error!("{command}: {error}");
        //     uprintln_error!("{command}");
        // }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    led_on(Led::Red);
    led_off(Led::Green);
    led_off(Led::Blue);

    uprintln_error!("Panic: {info}");

    loop {}
}

/// Prints out a message than waits for a response
/// 
/// # Returns
/// 
/// A string to the data which was actually received, or None if invalid utf8
fn recv_input_with_message<'a>(msg: &str, buf: &'a mut [u8]) -> Option<&'a str> {
    uprintln_debug!("{msg}");
    uprintln!("%ack%");
    let input = uart().recv_input(buf);
    uprintln!();
    input
}

/// Attempts to read in a component id from the user, returning None if there is an error
fn try_get_component_id(msg: &str) -> Option<ComponentId> {
    let mut buf = [0u8; 16];
    let mut input = recv_input_with_message(msg, &mut buf)?.trim();

    if input.starts_with("0x") {
        input = &input[2..];
    }

    ComponentId::from_str_radix(input, 16).ok()
}
