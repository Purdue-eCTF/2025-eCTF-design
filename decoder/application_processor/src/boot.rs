use core::str;
use core::time::Duration;

use design_utils::component_id_to_i2c_addr;
use design_utils::crypto::{decrypt, EncryptedData};
use design_utils::anti_hardware::{check_or_error_jump_table, create_mutations};
use design_utils::multi_if;
use design_utils::messages::{
    BootMessageStart,
    BootMessageComponentReply,
    BootMessageFinalize,
    EncryptedMessage,
    MessageStage,
    Nonce,
    SignedMessage,
    StartProtocolMessage,
};
use max78000_hal::prelude::*;
use max78000_hal::i2c::{I2cAddr, MAX_I2C_MESSAGE_LEN};
use max78000_hal::timer::{timeout, sleep};
use tinyvec::ArrayVec;

use crate::ApError;
use crate::ectf_params::{
    build_id_to_key_index,
    AP_BOOT_MSG,
    AP_PRIVKEY,
    BOOT_CR_KEY,
    BOOT_DATA_ENC_KEY,
    COMPONENT_KEYS
};
use crate::ap_driver::ApDriver;
use crate::post_boot;

const COMPONENT_COUNT: usize = 2;

#[derive(Default)]
struct ComponentBootState {
    component_id: u32,
    protocol_in_progress: bool,
    expected_reply_nonce: Nonce,
    key_index: Option<usize>,
    boot_message: Option<ArrayVec<[u8; MAX_I2C_MESSAGE_LEN]>>,
}

impl ComponentBootState {
    pub fn i2c_addr(&self) -> I2cAddr {
        component_id_to_i2c_addr(self.component_id)
    }
}

// sends final message telling components to boot
fn send_m3_and_boot(
    driver_option: &mut Option<ApDriver>,
    state: &mut [ComponentBootState; COMPONENT_COUNT],
)  -> Result<(), ApError> {
    let driver = driver_option.as_mut().unwrap();

    for component in state.iter_mut() {
        // send enc(m3 || cid || rb + 1 || signature)
        let message = BootMessageFinalize {
            m: MessageStage::M3,
            component_id: component.component_id,
            reply_nonce_plus_one: component.expected_reply_nonce,
        };

        let signed_message = SignedMessage::new_signed(message, &AP_PRIVKEY)?;
        let enc_message = EncryptedMessage::new_encrypted(signed_message, &BOOT_CR_KEY, driver.gen_bytes())?;

        let mut encrypted_boot_message: EncryptedData<MAX_I2C_MESSAGE_LEN> = timeout(
            || driver.send_and_receive_struct(component.i2c_addr(), enc_message),
            Duration::from_secs(1),
        )??;
        // recieved encrypted boot message
        component.protocol_in_progress = false;

        component.boot_message = Some(
            decrypt(&mut encrypted_boot_message, &BOOT_DATA_ENC_KEY)?.clone()
        );
    }

    // print boot messages only after both messages have been decrypted
    for component in state.iter_mut() {
        let boot_message = str::from_utf8(
            component.boot_message.as_ref().unwrap().as_slice()
        )?;

        uprintln_info!("0x{:x}>{}", component.component_id, boot_message);
    }

    uprintln_info!("AP>{}", AP_BOOT_MSG);

    let mut flash_data = driver.get_flash_data();
    // set key indexes for post boot code to use
    for (i, component) in state.iter().enumerate() {
        flash_data.components[i].key_index = component.key_index.unwrap();
    }
    driver.save_flash_data(flash_data);

    uprintln_success!("Boot");

    post_boot::boot(driver_option.take().unwrap());
}

fn boot_components(driver_option: &mut Option<ApDriver>, state: &mut [ComponentBootState; COMPONENT_COUNT]) -> Result<(), ApError> {
    let mut verifier: i32 = 0;
    create_mutations!(28);
    
    let driver = driver_option.as_mut().unwrap();

    // verify components by sending m1 and verifiying their m2 response
    for component in state.iter_mut() {
        // send enc(m1 || cid || ra)
        let ap_nonce = driver.gen_nonce();
        let message = BootMessageStart {
            m: MessageStage::M1,
            component_id: component.component_id,
            nonce: ap_nonce,
        };

        let req = StartProtocolMessage::Boot(
            EncryptedMessage::new_encrypted(message, &BOOT_CR_KEY, driver.gen_bytes())?,
        );

        let mut enc_response: EncryptedMessage<SignedMessage<BootMessageComponentReply>> = timeout(
            || driver.send_and_receive_struct(component.i2c_addr(), req),
            Duration::from_secs(1),
        )??;
        component.protocol_in_progress = true;

        // received enc(m2 || cid || bid || ra + 1 || rb || signature)
        let signed_response = enc_response.get_decrypted_data(&BOOT_CR_KEY)?;
        let response: BootMessageComponentReply = postcard::from_bytes(&signed_response.message_data)?;

        // glitch protect these checks
        multi_if!(
            { mutate(&mut verifier); response.m == MessageStage::M2
                && response.component_id == component.component_id
                && response.start_nonce_plus_one == ap_nonce + 1 },
            (),
            { return Err(ApError::SuspiciousActivity) },
            driver.get_chacha(),
        );

        let key_index = build_id_to_key_index(response.build_id)
            .ok_or(ApError::InvalidBuildId)?;
        let pubkey = &COMPONENT_KEYS[key_index].pubkey;

        multi_if!(
            { mutate(&mut verifier); signed_response.verify(pubkey) },
            (),
            { return Err(ApError::SuspiciousActivity) },
            driver.get_chacha(),
        );

        component.key_index = Some(key_index);
        component.expected_reply_nonce = response.reply_nonce + 1;
    }

    // verify components are using different build ids then tell components to boot and boot
    check_or_error_jump_table!(
        state[0].key_index != state[1].key_index && verifier == VERIFIED_VALUE ,
        fn(&mut Option<ApDriver>, &mut [ComponentBootState; COMPONENT_COUNT]) -> Result<(), ApError>,
        send_m3_and_boot,
        (driver_option, state),
        Err(ApError::SuspiciousActivity),
        driver.get_chacha(),
    )
}

// Boot the components and board if the components validate
pub fn attempt_boot(driver_option: &mut Option<ApDriver>) -> Result<(), ApError> {
    let driver = driver_option.as_mut().unwrap();

    let flash_data = driver.get_flash_data();
    if flash_data.components_len != COMPONENT_COUNT {
        // not enough components to boot
        return Err(ApError::InvalidBootConditions);
    }

    let mut boot_state = [ComponentBootState::default(), ComponentBootState::default()];
    for (i, component) in flash_data.components.iter().enumerate() {
        boot_state[i].component_id = component.component_id;
    }

    // make sure we are booting with distinct component ids (with glitch hardened check)
    let result = check_or_error_jump_table!(
        boot_state[0].component_id != boot_state[1].component_id,
        fn(&mut Option<ApDriver>, &mut [ComponentBootState; COMPONENT_COUNT]) -> Result<(), ApError>,
        boot_components,
        (driver_option, &mut boot_state),
        Err(ApError::SuspiciousActivity),
        driver.get_chacha(),
    );

    let driver = driver_option.as_mut().unwrap();
    if let Err(err) = result {
        // send error message to components which were still expecting a message
        for component in boot_state {
            if component.protocol_in_progress {
                let _ = driver.send_error(component.i2c_addr());
            }
        }

        sleep(Duration::from_secs(5));
        Err(err)
    } else {
        // if boot succeeds, it will run post boot which runs forever
        unreachable!()
    }
}
