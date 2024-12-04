use design_utils::messages::{StartProtocolMessage, ScanIdResponse};
use max78000_hal::{prelude::*, HalError};

use crate::ApError;
use crate::ap_driver::ApDriver;

pub fn scan_components(driver: &mut ApDriver) -> Result<(), ApError> {
    let flash_data = driver.get_flash_data();

    // Print out provisioned component IDs
    for i in 0..(flash_data.components_len as usize) {
        uprintln_info!("P>0x{:08x}", flash_data.components[i].component_id);
    }

    // Scan command to each component
    for addr in 0x8..0x78 {
        // I2C Blacklist:
        // 0x18, 0x28, and 0x36 conflict with separate devices on MAX78000FTHR
        if addr == 0x18 || addr == 0x28 || addr == 0x36 {
            continue;
        }

        let result = driver.send_and_receive_struct(addr, &StartProtocolMessage::ScanId);
        let response: ScanIdResponse = match result {
            // could not reach this i2c address, go to next one
            Err(ApError::HalError(HalError::I2cConnectionError)) => continue,
            Err(error) => return Err(error),
            Ok(response) => response,
        };

        uprintln_info!("F>0x{:08x}", response.component_id);
    }

    uprintln_success!("List");

    Ok(())
}
