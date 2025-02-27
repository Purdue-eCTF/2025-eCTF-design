#![allow(unused)]

include!(concat!(env!("OUT_DIR"), "/ectf_params.rs"));

pub const MAX_SUBSCRIPTIONS: usize = 8;
pub const EMERGENCY_CHANNEL_ID: u32 = 0;
