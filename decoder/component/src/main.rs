#![no_std]
#![no_main]

#![feature(effects)]
#![feature(const_trait_impl)]

use core::panic::PanicInfo;
use core::time::Duration;

use cortex_m::interrupt;
use cortex_m_rt::entry;
use design_utils::anti_hardware::check_or_error_jump_table;
use design_utils::{multi_if, DesignUtilsError};
use thiserror_no_std::Error;
use design_utils::const_time_equal_or_error;
use design_utils::crypto::hmac;
use design_utils::messages::{
    AttestationReqMessage, BootMessageComponentReply, BootMessageFinalize, BootMessageStart, EncryptedMessage, MessageStage, ScanIdResponse, SignedMessage, StartProtocolMessage
};
use design_utils::str::concat;
use max78000_hal::prelude::*;
use max78000_hal::HalError;
use max78000_hal::timer::{sleep, timeout};
use max78000_hal::led::{led_on, led_off, Led};

use ectf_params::{
    AP_PUBKEY,
    COMPONENT_ID,
    HMAC_KEY,
    BUILD_ID,
    COMPONENT_PRIVKEY,
    BOOT_CR_KEY,
    encrypted_attestation_data,
    encrypted_boot_message
};
use crate::component_driver::ComponentDriver;

mod component_driver;
mod ectf_params;
mod post_boot;

#[derive(Debug, Error)]
pub enum ComponentError {
    #[error("Error in the HAL: {0}")]
    HalError(#[from] HalError),
    #[error("Error: Suspicious activity detected")]
    SuspiciousActivity,
    #[error("Error: ap reported an error occurred")]
    ProtocolError,
    #[error("Error while serializing or deserializing message: {0}")]
    PostcardError(#[from] postcard::Error),
    #[error("Error in design utils: {0}")]
    DecryptError(#[from] DesignUtilsError),
    #[error("Post boot code sent or received when it was not supposed to")]
    InvalidPostBootAction,
}

#[entry]
fn main() -> ! {
    unsafe {
        interrupt::enable();
    }

    let mut driver = Some(ComponentDriver::new());
    sleep(Duration::from_millis(950));

    led_on(Led::Green);

    loop {
        let Ok(command) = driver.as_mut().unwrap().recv_struct() else {
            continue;
        };

        let result = match command {
            StartProtocolMessage::ScanId => process_scan(driver.as_mut().unwrap()),
            StartProtocolMessage::Attest => process_attest(driver.as_mut().unwrap()),
            StartProtocolMessage::Boot(mut boot_data) => process_boot(&mut driver, &mut boot_data),
        };

        if let Err(error) = result {
            let driver = driver.as_mut().unwrap();

            // FIXME: figure out how to handle this error
            // i2c state will be all messed up if this fails
            // FIXME: not all errors require sending component an error
            let _ = driver.send_error();
            sleep(Duration::from_secs(5));

            uprint_error!("{error}");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    /*led_on(Led::Red);
    led_off(Led::Blue);
    led_off(Led::Green);

    uprintln_error!("{info}");*/

    loop {}
}

fn process_scan(driver: &mut ComponentDriver) -> Result<(), ComponentError> {
    driver.send_struct(&ScanIdResponse {
        component_id: COMPONENT_ID,
    })?;

    Ok(())
}

fn process_attest(driver: &mut ComponentDriver) -> Result<(), ComponentError> {
    // I think this should make getting rng samples a little harder
    // this amount of sleeping leaves a 20 milliseconds extra under 3 seconds for the whole attest process
    sleep(Duration::from_millis(55));

    let nonce = driver.gen_nonce();
    driver.send_struct(nonce)?;

    let request: AttestationReqMessage = timeout(|| driver.recv_struct(), Duration::from_millis(100))??;

    if request.component_id != COMPONENT_ID || request.nonce != nonce {
        return Err(ComponentError::SuspiciousActivity);
    }

    let message = concat(request.component_id.to_le_bytes(), request.nonce.to_le_bytes());
    let hmac = hmac(&message, &HMAC_KEY);

    const_time_equal_or_error!(
        request.hmac.as_slice(),
        hmac.as_slice(),
        ComponentError::SuspiciousActivity,
        driver.get_chacha(),
    );

    driver.send_struct(encrypted_attestation_data())?;

    Ok(())
}

fn process_boot(
    driver_option: &mut Option<ComponentDriver>,
    encrypted_message: &mut EncryptedMessage<BootMessageStart>
) -> Result<(), ComponentError> {
    let driver = driver_option.as_mut().unwrap();

    // received enc(m1 || cid || ra)
    let message = encrypted_message.get_decrypted_data(&BOOT_CR_KEY)?;

    multi_if!(
        message.m == MessageStage::M1 && message.component_id == COMPONENT_ID,
        (),
        { return Err(ComponentError::SuspiciousActivity) },
        driver.get_chacha(),
    );

    // reply enc(m2 || cid || bid || ra + 1 || rb || signature)
    let component_nonce = driver.gen_nonce();

    let reply = BootMessageComponentReply {
        m: MessageStage::M2,
        component_id: COMPONENT_ID,
        build_id: BUILD_ID,
        start_nonce_plus_one: message.nonce + 1,
        reply_nonce: component_nonce,
    };

    let signed_reply = SignedMessage::new_signed(reply, &COMPONENT_PRIVKEY)?;
    let encrypted_reply = EncryptedMessage::new_encrypted(signed_reply, &BOOT_CR_KEY, driver.gen_bytes())?;
    driver.send_struct(encrypted_reply)?;

    // received enc(m3 || cid || rb + 1 || signature)
    let mut encrypted_message: EncryptedMessage<SignedMessage<BootMessageFinalize>> = 
        timeout(|| driver.recv_struct(), Duration::from_secs(3))??;

    let signed_message = encrypted_message.get_decrypted_data(&BOOT_CR_KEY)?;
    let message: BootMessageFinalize = postcard::from_bytes(&signed_message.message_data)?;

    check_or_error_jump_table!(
        {
            message.m == MessageStage::M3
                && message.component_id == COMPONENT_ID
                && message.reply_nonce_plus_one == component_nonce + 1
                && signed_message.verify(&AP_PUBKEY)
        },
        fn(&mut Option<ComponentDriver>) -> Result<(), ComponentError>,
        boot_finish,
        (driver_option,),
        Err(ComponentError::SuspiciousActivity),
        driver.get_chacha(),
    )
}

fn boot_finish(
    driver_option: &mut Option<ComponentDriver>,
) -> Result<(), ComponentError> {
    let mut driver = driver_option.take().unwrap();

    // reply with encrypted boot message
    driver.send_struct(encrypted_boot_message())?;

    post_boot::boot(driver);
}