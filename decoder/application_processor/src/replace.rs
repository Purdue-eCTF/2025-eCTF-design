use core::time::Duration;

use max78000_hal::prelude::*;
use max78000_hal::timer::sleep;
use design_utils::crypto::hash;
use design_utils::const_time_equal_or_error_jump_table;

use crate::{ApError, try_get_component_id, recv_input_with_message};
use crate::ap_driver::ApDriver;
use crate::ectf_params::{TOKEN_HASH, TOKEN_SALT};

const TOKEN_LEN: usize = 16;

fn perform_replace(driver: &mut ApDriver) -> Result<(), ApError> {
    let mut flash_data = driver.get_flash_data();

    let Some(new_component_id) = try_get_component_id("Component ID In: ") else {
        return Err(ApError::InvalidInput);
    };

    // verify we are not provisioned with the new component
    if flash_data.get_provisioned_component(new_component_id).is_some() {
        return Err(ApError::InvalidInput);
    }

    let Some(old_component_id) = try_get_component_id("Component ID Out: ") else {
        return Err(ApError::InvalidInput);
    };

    // retern error if we are not provisioned for the old component
    let component = flash_data.get_provisioned_component(old_component_id)
        .ok_or(ApError::InvalidInput)?;

    component.component_id = new_component_id;

    driver.save_flash_data(flash_data);

    uprintln_success!("Replace");

    Ok(())
}

// Replace a component if the PIN is correct
fn attempt_replace(driver: &mut ApDriver) -> Result<(), ApError> {
    let mut token_buf = [0; TOKEN_LEN];
    let token = recv_input_with_message("Enter token: ", &mut token_buf)
        .ok_or(ApError::InvalidInput)?;

    let hash = hash(token.as_bytes(), &TOKEN_SALT, 8);

    const_time_equal_or_error_jump_table!(
        hash.as_slice(),
        TOKEN_HASH.as_slice(),
        fn(&mut ApDriver) -> Result<(), ApError>,
        perform_replace,
        (driver,),
        Err(ApError::InvalidInput),
        driver.get_chacha(),
    )
}

pub fn replace(driver: &mut ApDriver) -> Result<(), ApError> {
    if let Err(err) = attempt_replace(driver) {
        sleep(Duration::from_secs(5));
        Err(err)
    } else {
        Ok(())
    }
}
