use bytemuck::{AnyBitPattern, NoUninit, Pod, Zeroable};

use crate::crypto::get_decoder_payload_associated_data;
use crate::decoder_context::CompressedSubscriptionEntry;
use crate::ectf_params::{DECODER_ID, EMERGENCY_CHANNEL_ID, SUBSCRIPTION_ENC_KEY};
use crate::message::{Message, Opcode};
use crate::utils::{Cursor, CursorError};
use crate::{crypto::decrypt_decoder_payload, decoder_context::DecoderContext, DecoderError};

/// Parses subscription entry from subecription data plaintext.
fn read_subscription(data: &[u8]) -> Result<CompressedSubscriptionEntry, DecoderError> {
    let mut data_cursor = Cursor::new(data);

    let public_key: [u8; 32] = read_value(&mut data_cursor)?;

    let start_time: u64 = read_value(&mut data_cursor)?;

    let channel_id: u32 = read_value(&mut data_cursor)?;
    assert!(channel_id != EMERGENCY_CHANNEL_ID);

    let subtree_count: u8 = read_value(&mut data_cursor)?;
    let subtree_count = u32::from(subtree_count);
    assert!(subtree_count <= 128);

    let mut depths = [0u8; 128];
    data_cursor.read_into(&mut depths[..subtree_count as usize])?;

    let mut node_keys = [[0u8; 32]; 128];
    data_cursor.read_into(bytemuck::cast_slice_mut(
        &mut node_keys[..subtree_count as usize],
    ))?;
    /*
    To save time on flash writes, we store the subscription entry "compressed" (using depths instead of start/end) in flash
    and then decompress later
    */

    let subscription = CompressedSubscriptionEntry {
        public_key,
        start_time,
        depths,
        node_keys,
        channel_id,
        subtree_count,
    };

    // // make sure the whole valid range is covered
    // let active = subscription.active_subtrees();
    // assert!(active[0].lowest_timestamp == start_time);
    // assert!(active.last().unwrap().highest_timestamp == end_time);
    // assert!(active[..active.len() - 1]
    //     .iter()
    //     .zip(active[1..].iter())
    //     .all(|(lower, higher)| lower.highest_timestamp == higher.lowest_timestamp - 1));

    Ok(subscription)
}

/// Non-encrypted associated data sent with subscription.
///
/// Probably not needed but signing plaintext decoder id ensures only 1 possible symmetric
/// key can be used to decrypt payload, just in case.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SubscriptionAssociatedData {
    decoder_id: u32,
}

/// Performs subscribe functionality required by host tools.
pub fn subscribe(
    context: &mut DecoderContext,
    subscribe_data: &mut [u8],
) -> Result<(), DecoderError> {
    let associated_data: SubscriptionAssociatedData =
        get_decoder_payload_associated_data(subscribe_data)?;
    if associated_data.decoder_id != DECODER_ID {
        return Err(DecoderError::InvalidSubscription);
    }

    let subscription_public_key = &context.subscription_public_key;

    let subscription_data = decrypt_decoder_payload(
        subscribe_data,
        size_of::<SubscriptionAssociatedData>(),
        &SUBSCRIPTION_ENC_KEY,
        subscription_public_key,
    )?;
    let entry = read_subscription(subscription_data)?;

    context.update_subscription(&entry)?;

    Message::send_data(Opcode::Subscribe, &[])?;

    Ok(())
}

fn read_value<T>(cursor: &mut Cursor<&[u8]>) -> Result<T, CursorError>
where
    T: Default + NoUninit + AnyBitPattern,
{
    let mut data = T::default();

    cursor.read_into(bytemuck::bytes_of_mut(&mut data))?;
    Ok(data)
}
