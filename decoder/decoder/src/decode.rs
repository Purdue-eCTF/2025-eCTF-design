use bytemuck::{Pod, Zeroable};

use crate::crypto::get_decoder_payload_associated_data;
use crate::{decoder_context::DecoderContext, DecoderError};

/// Non encrypted associated data sent with frame.
///
/// Needed because we need to know channel and timestamp for deriving encryption key.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FrameAssociatedData {
    channel_number: u32,
    timestamp: u64,
}

pub fn decode(context: &mut DecoderContext, subscribe_data: &mut [u8]) -> Result<(), DecoderError> {
    let frame_info: &FrameAssociatedData = get_decoder_payload_associated_data(subscribe_data)?;

    // TODO: decode

    Ok(())
}
