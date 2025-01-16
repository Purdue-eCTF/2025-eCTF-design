use core::str;
use core::time::Duration;

use max78000_hal::prelude::*;
use max78000_hal::i2c::MAX_I2C_MESSAGE_LEN;
use design_utils::component_id_to_i2c_addr;
use design_utils::crypto::{decrypt, hmac, hash, EncryptedData};
use design_utils::messages::{StartProtocolMessage, AttestationReqMessage, Nonce};
use design_utils::str::concat;
use design_utils::const_time_equal_or_error_jump_table;
use max78000_hal::timer::sleep;

use crate::{recv_input_with_message, try_get_component_id, ApError};
use crate::ectf_params::{PIN_HASH, PIN_SALT, HMAC_KEY, ADATA_ENC_KEY};
use crate::ap_driver::ApDriver;

const PIN_LEN: usize = 6;

fn perform_attest(driver: &mut ApDriver) -> Result<(), ApError> {
    let component_id = try_get_component_id("Component ID: ")
        .ok_or(ApError::InvalidInput)?;

    // check that we are provisioned for the component we are requested to attest
    if driver.get_flash_data().get_provisioned_component(component_id).is_none() {
        return Err(ApError::InvalidComponentError);
    }

    let address = component_id_to_i2c_addr(component_id);
    let nonce: Nonce = driver.send_and_receive_struct(address, StartProtocolMessage::Attest)?;

    let message = concat(component_id.to_le_bytes(), nonce.to_le_bytes());
    let hmac = hmac(&message, &HMAC_KEY);

    let req = AttestationReqMessage {
        component_id,
        nonce,
        hmac,
    };

    // recieve encrypted attestation data
    let mut response: EncryptedData<MAX_I2C_MESSAGE_LEN> = driver.send_and_receive_struct(address, &req)?;

    let message = decrypt(&mut response, &ADATA_ENC_KEY)?;
    let message = str::from_utf8(message.as_slice())?;

    // Print out attestation data
    uprintln_info!("C>0x{:08x}", component_id);
    uprintln_info!("{message}");

    uprintln_success!("Attest");
    Ok(())
}

fn attempt_attest(driver: &mut ApDriver) -> Result<(), ApError> {
    let mut buf = [0; PIN_LEN];
    let pin= recv_input_with_message("Enter pin: ", &mut buf)
        .ok_or(ApError::InvalidInput)?;

    let hash = hash(pin.as_bytes(), &PIN_SALT, 8);

    const_time_equal_or_error_jump_table!(
        hash.as_slice(),
        PIN_HASH.as_slice(),
        fn(&mut ApDriver) -> Result<(), ApError>,
        perform_attest,
        (driver,),
        Err(ApError::InvalidInput),
        driver.get_chacha(),
    )
}

pub fn attest(driver: &mut ApDriver) -> Result<(), ApError> {
    if let Err(err) = attempt_attest(driver) {
        sleep(Duration::from_secs(5));
        Err(err)
    } else {
        Ok(())
    }
}
