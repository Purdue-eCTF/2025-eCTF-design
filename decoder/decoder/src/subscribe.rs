use bytemuck::{AnyBitPattern, NoUninit};
use ed25519_dalek::VerifyingKey;

use crate::decoder_context::{KeySubtree, SubscriptionEntry};
use crate::ectf_params::{SUBSCRIPTION_ENC_KEY, SUBSCRIPTION_PUBLIC_KEY};
use crate::message::{Message, Opcode};
use crate::utils::{Cursor, CursorError};
use crate::{crypto::decrypt_decoder_payload, decoder_context::DecoderContext, DecoderError};
use core::{slice, u64};

// TODO (sebastian): can I do unit tests?
fn read_subscription(data: &[u8]) -> Result<SubscriptionEntry, DecoderError> {
    let mut data_cursor = Cursor::new(data);

    let channel_public_key: [u8; 32] = read_value(&mut data_cursor)?;

    let start_time: u64 = read_value(&mut data_cursor)?;
    let end_time: u64 = read_value(&mut data_cursor)?;
    assert!(start_time <= end_time);

    let channel_id: u32 = read_value(&mut data_cursor)?;
    assert!(channel_id <= 8);

    let subtree_count: u32 = read_value(&mut data_cursor)?;
    // TODO (sebastian): verify the maximum number of subtrees
    assert!(subtree_count <= 128);

    let mut subtrees = [Default::default(); 128];
    for i in 0u32..subtree_count {
        let lowest_timestamp = read_value(&mut data_cursor)?;
        let highest_timestamp = read_value(&mut data_cursor)?;

        let key = read_value(&mut data_cursor)?;
        let subtree = KeySubtree {
            lowest_timestamp,
            highest_timestamp,
            key,
        };
        subtrees[i as usize] = subtree;
    }

    Ok(SubscriptionEntry {
        start_time,
        end_time,
        channel_id,
        public_key: channel_public_key,
        subtrees,
        subtree_count,
    })
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
    cursor.read_into(bytemuck::cast_slice_mut(slice::from_mut(&mut data)))?;
    Ok(data)
}
