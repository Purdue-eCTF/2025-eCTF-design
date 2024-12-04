use core::time::Duration;

use max78000_hal::timer::{timeout, sleep};
use design_utils::crypto::{sign, verify_signature};
use tinyvec::ArrayVec;
use design_utils::{multi_if, MAX_POST_BOOT_MESSAGE_SIZE};
use design_utils::messages::{PostBootMessage, PostBootMessageStart, SignedPostBootMessage};

use crate::ectf_params::{AP_PUBKEY, COMPONENT_ID, COMPONENT_PRIVKEY};
use crate::ComponentError;
use crate::component_driver::ComponentDriver;

pub fn secure_send(driver: &mut ComponentDriver, message: &[u8]) -> Result<(), ComponentError> {
    // TODO: send ap error if serialization fails
    let post_boot_request: PostBootMessageStart = driver.recv_struct()?;

    // send ap error if it tried to send when we were expecting it to receive
    let PostBootMessageStart::ApToComponentNonce(nonce) = post_boot_request else {
        driver.send_error()?;
        return Err(ComponentError::InvalidPostBootAction);
    };

    let mut message_buf = ArrayVec::new();
    message_buf.extend_from_slice(message);
    let message = PostBootMessage {
        component_id: COMPONENT_ID,
        nonce,
        message: message_buf,
    };

    let signature_buffer = message.get_bytes_to_sign();
    let signature = sign(signature_buffer.as_slice(), &COMPONENT_PRIVKEY);

    driver.send_struct(SignedPostBootMessage {
        message,
        signature: signature.into(),
    })?;

    Ok(())
}

pub fn secure_receive(
    driver: &mut ComponentDriver,
    recv_buf: &mut [u8; MAX_POST_BOOT_MESSAGE_SIZE]
) -> Result<usize, ComponentError> {
    // TODO: send ap error if serialization fails
    let post_boot_request: PostBootMessageStart = driver.recv_struct()?;

    // send ap error if it tried to recieve when we were expecting it to send
    let PostBootMessageStart::RequestComponentNonce = post_boot_request else {
        driver.send_error()?;
        return Err(ComponentError::InvalidPostBootAction);
    };

    // make pulling rng samples more annoying
    sleep(Duration::from_millis(300));

    let nonce = driver.gen_nonce();

    driver.send_struct(nonce)?;
    let SignedPostBootMessage {
        message,
        signature,
    } = timeout(|| driver.recv_struct(), Duration::from_secs(1))??;

    multi_if!(
        message.component_id == COMPONENT_ID && message.nonce == nonce,
        (),
        {
            driver.send_error()?;
            return Err(ComponentError::SuspiciousActivity);
        },
        driver.get_chacha(),
    );

    let signature_buffer = message.get_bytes_to_sign();

    // TODO: use the jump table variant of multi if
    multi_if!(
        verify_signature(signature_buffer.as_slice(), &signature.into(), &AP_PUBKEY),
        (),
        {
            driver.send_error()?;
            return Err(ComponentError::SuspiciousActivity);
        },
        driver.get_chacha(),
    );

    // send ack to ap
    driver.send_struct(())?;

    let message_len = message.message.len();
    recv_buf[..message_len].copy_from_slice(message.message.as_slice());

    Ok(message_len)
}