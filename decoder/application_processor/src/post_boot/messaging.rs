use core::time::Duration;

use design_utils::crypto::{verify_signature, sign};
use design_utils::messages::{Nonce, PostBootMessage, SignedPostBootMessage, PostBootMessageStart};
use design_utils::{multi_if, MAX_POST_BOOT_MESSAGE_SIZE};
use max78000_hal::i2c::I2cAddr;
use max78000_hal::timer::timeout;
use tinyvec::ArrayVec;

use crate::ectf_params::{AP_PRIVKEY, COMPONENT_KEYS};
use crate::ApError;
use crate::ap_driver::ApDriver;

pub fn secure_send(driver: &mut ApDriver, address: I2cAddr, message: &[u8]) -> Result<(), ApError> {
    let flash_data = driver.get_flash_data();
    let component = flash_data.get_component_for_i2c_addr(address)
        .ok_or(ApError::InvalidComponentError)?;

    // TODO: notify component of deserialization error
    let nonce: Nonce = driver.send_and_receive_struct(address, PostBootMessageStart::RequestComponentNonce)?;

    let mut message_buf = ArrayVec::new();
    message_buf.extend_from_slice(message);
    let message = PostBootMessage {
        component_id: component.component_id,
        nonce,
        message: message_buf,
    };

    let signature_buffer = message.get_bytes_to_sign();
    let signature = sign(signature_buffer.as_slice(), &AP_PRIVKEY);

    let _ack: () = driver.send_and_receive_struct(address, SignedPostBootMessage {
        message,
        signature: signature.into(),
    })?;

    Ok(())
}

pub fn secure_receive(
    driver: &mut ApDriver,
    address: I2cAddr,
    recv_buf: &mut [u8; MAX_POST_BOOT_MESSAGE_SIZE]
) -> Result<usize, ApError> {
    let flash_data = driver.get_flash_data();
    let component = flash_data.get_component_for_i2c_addr(address)
        .ok_or(ApError::InvalidComponentError)?;

    let nonce = driver.gen_nonce();

    let SignedPostBootMessage {
        message,
        signature,
    } = timeout(
        || driver.send_and_receive_struct(address, PostBootMessageStart::ApToComponentNonce(nonce)),
        Duration::from_secs(1),
    )??;

    multi_if!(
        component.component_id == message.component_id && message.nonce == nonce,
        (),
        { return Err(ApError::SuspiciousActivity) },
        driver.get_chacha(),
    );

    let signature_buffer = message.get_bytes_to_sign();
    let component_key = &COMPONENT_KEYS[component.key_index].pubkey;

    // TODO: use the jump table variant of multi if
    multi_if!(
        verify_signature(signature_buffer.as_slice(), &signature.into(), component_key),
        (),
        { return Err(ApError::SuspiciousActivity) },
        driver.get_chacha(),
    );

    let message_len = message.message.len();
    recv_buf[..message_len].copy_from_slice(message.message.as_slice());

    Ok(message_len)
}
