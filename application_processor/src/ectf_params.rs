#![allow(unused)]

use design_utils::crypto::EncryptedData;

use crate::ap_driver::ProvisionedComponent;

pub struct ComponentKey {
    pub build_id: u32,
    pub pubkey: [u8; 32],
}

include!(concat!(env!("OUT_DIR"), "/ectf_params.rs"));

/// Returns the key index corresponding to the given build id if it exists
pub fn build_id_to_key_index(build_id: u32) -> Option<usize> {
    COMPONENT_KEYS.iter()
        .enumerate()
        .filter(|(_, key)| key.build_id == build_id)
        .map(|(i, _)| i)
        .next()
}

pub fn get_key_for_build_id(build_id: u32) -> Option<&'static [u8; 32]> {
    Some(&COMPONENT_KEYS[build_id_to_key_index(build_id)?].pubkey)
}
