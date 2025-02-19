use bytemuck::{AnyBitPattern, NoUninit};
use ed25519_dalek::VerifyingKey;

use crate::decoder_context::{KeySubtree, SubscriptionEntry};
use crate::ectf_params::{SUBSCRIPTION_ENC_KEY, SUBSCRIPTION_PUBLIC_KEY};
use crate::message::{Message, Opcode};
use crate::utils::{Cursor, CursorError};
use crate::{crypto::decrypt_decoder_payload, decoder_context::DecoderContext, DecoderError};

fn read_subscription(data: &[u8]) -> Result<SubscriptionEntry, DecoderError> {
    let mut data_cursor = Cursor::new(data);

    let channel_public_key: [u8; 32] = read_value(&mut data_cursor)?;

    let start_time: u64 = read_value(&mut data_cursor)?;
    let end_time: u64 = read_value(&mut data_cursor)?;
    assert!(start_time <= end_time);

    let channel_id: u32 = read_value(&mut data_cursor)?;
    assert!(channel_id <= 8);

    let subtree_count: u8 = read_value(&mut data_cursor)?;
    assert!(subtree_count <= 128);

    let mut subtrees = [KeySubtree::default(); 128];

    /*
    Start and end timestamp have already been sent. Since the nodes must be a continuous range,
    we can omit sending highest_timestamp, as well as the lowest_timestamp for the first node.
    This saves serial bandwidth, which seems to be taking too long.
    */

    let mut lowest_timestamps = [0u64; 128];
    lowest_timestamps[0] = start_time;
    data_cursor.read_into(bytemuck::cast_slice_mut(
        &mut lowest_timestamps[1..subtree_count as usize],
    ))?;
    for i in 0..subtree_count {
        let lowest_timestamp = lowest_timestamps[i as usize];
        let highest_timestamp = if i == subtree_count - 1 {
            end_time
        } else {
            lowest_timestamps[(i + 1) as usize] - 1
        };
        assert!(lowest_timestamp <= highest_timestamp);

        let key = read_value(&mut data_cursor)?;
        let subtree = KeySubtree {
            lowest_timestamp,
            highest_timestamp,
            key,
        };
        subtrees[i as usize] = subtree;
    }

    let subscription = SubscriptionEntry {
        start_time,
        end_time,
        channel_id,
        public_key: channel_public_key,
        subtrees,
        subtree_count: subtree_count as u32,
    };

    // make sure the whole valid range is covered
    let active = subscription.active_subtrees();
    assert!(active[0].lowest_timestamp == start_time);
    assert!(active.last().unwrap().highest_timestamp == end_time);
    assert!(active[..active.len() - 1]
        .iter()
        .zip(active[1..].iter())
        .all(|(lower, higher)| lower.highest_timestamp == higher.lowest_timestamp - 1));

    Ok(subscription)
}

pub fn subscribe(
    context: &mut DecoderContext,
    subscribe_data: &mut [u8],
) -> Result<(), DecoderError> {
    let subscription_public_key = VerifyingKey::from_bytes(&SUBSCRIPTION_PUBLIC_KEY)
        .expect("decoder loaded with invalid public key");

    let subscription_data = decrypt_decoder_payload(
        subscribe_data,
        0,
        &SUBSCRIPTION_ENC_KEY,
        &subscription_public_key,
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
