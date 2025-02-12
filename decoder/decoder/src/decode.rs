use bytemuck::{try_from_bytes, Pod, Zeroable};
use ed25519_dalek::VerifyingKey;

use crate::crypto::{
    compute_chacha_block, decrypt_decoder_payload, get_decoder_payload_associated_data,
};
use crate::decoder_context::SubscriptionEntry;
use crate::ectf_params::{CHANNEL0_ENC_KEY, CHANNEL0_PUBLIC_KEY};
use crate::message::{Message, Opcode};
use crate::println;
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

/// Data in encoded frames.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct FrameData {
    /// Number of bytes in frame.
    frame_len: u8,
    /// Data of frame.
    ///
    /// Extra bytes are zeroed out.
    frame_data: [u8; 64],
}

/// Performs all funcitonality related to decoding frames.
pub fn decode(context: &mut DecoderContext, encoded_frame: &mut [u8]) -> Result<(), DecoderError> {
    let frame_info: FrameAssociatedData = *get_decoder_payload_associated_data(encoded_frame)?;

    // check frame we are decoding is monotonically increasing for security requirement 3
    if context
        .last_decoded_timestamp
        .is_some_and(|last_decoded| frame_info.timestamp <= last_decoded)
    {
        return Err(DecoderError::NonMonotonicTimestamp);
    }

    let (symmetric_key, public_key) =
        get_keys_for_channel(context, frame_info.channel_number, frame_info.timestamp)?;

    // frame data has 1 byte at the start indicating how long it is
    // and 64 bytes after containing the data itself
    // this is to not leak length of frame (probably doesn't matter at all)
    let frame_data = decrypt_decoder_payload(
        encoded_frame,
        size_of::<FrameAssociatedData>(),
        &symmetric_key,
        &public_key,
    )?;

    let frame_data: &FrameData = try_from_bytes(frame_data)?;

    // decoding succeeded, update last decoded timestamp
    context.last_decoded_timestamp = Some(frame_info.timestamp);

    let message = Message::from_data(
        Opcode::Decode,
        &frame_data.frame_data[..frame_data.frame_len as usize],
    );
    message.write()?;

    Ok(())
}

/// Retrieve the public and symmetric keys for a frame on channel `channel_number` encoded with timestamp `timestamp`.
fn get_keys_for_channel(
    context: &DecoderContext,
    channel_number: u32,
    timestamp: u64,
) -> Result<([u8; 32], VerifyingKey), DecoderError> {
    if channel_number == 0 {
        // channel 0 keys are hardcoded
        Ok((
            CHANNEL0_ENC_KEY,
            VerifyingKey::from_bytes(&CHANNEL0_PUBLIC_KEY).expect("Invalid public key bytes"),
        ))
    } else {
        // other channel keys are derived from subscription data
        let Some(subscription) = context.get_subscription_for_channel(channel_number) else {
            return Err(DecoderError::InvalidSubscription);
        };

        // this check is not necessary since deriving the key should fail,
        // but we do it just in case
        if timestamp < subscription.start_time || timestamp > subscription.end_time {
            println!(
                "timestamp was {timestamp}; should be {}..={}",
                subscription.start_time, subscription.end_time
            );
            return Err(DecoderError::InvalidTimestamp);
        }

        // derive symmetric key based on subscription data and timestamp
        let symmetric_key = derive_decoder_key_for_timestamp(subscription, timestamp)?;

        Ok((
            symmetric_key,
            VerifyingKey::from_bytes(&subscription.public_key).expect("Invalid public key bytes"),
        ))
    }
}

/// Derives a symmetric key for the given `timestamp` using subscription data.
///
/// This uses the GGM key tree discussed in design doc.
fn derive_decoder_key_for_timestamp(
    subscription: &SubscriptionEntry,
    timestamp: u64,
) -> Result<[u8; 32], DecoderError> {
    // locate subtree root containing the key for the timestamp we are interested in
    let subtree = subscription
        .active_subtrees()
        .iter()
        .find(|tree| tree.lowest_timestamp <= timestamp && timestamp <= tree.highest_timestamp)
        .ok_or_else(|| {
            println!("Failed to find correct subtree");
            println!("{:?}", subscription.active_subtrees());
            DecoderError::NoTimestampFound
        })?;
    assert!(subtree.lowest_timestamp <= timestamp && timestamp <= subtree.highest_timestamp);

    let mut lower = subtree.lowest_timestamp;
    let mut upper = subtree.highest_timestamp;

    let mut key = subtree.key;
    // shrink upper and lower bounds until we have found the key
    while lower != upper {
        let expanded_key = compute_chacha_block(key);

        let lower_midsection = (lower + upper) / 2;
        let upper_midsection = lower_midsection + 1;

        if timestamp <= lower_midsection {
            key.copy_from_slice(&expanded_key[..32]);
            upper = lower_midsection;
        } else {
            key.copy_from_slice(&expanded_key[32..]);
            lower = upper_midsection;
        }
    }

    Ok(key)
}
