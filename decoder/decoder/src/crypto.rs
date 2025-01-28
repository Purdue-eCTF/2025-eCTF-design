use bytemuck::{from_bytes, Pod, Zeroable};
use chacha20poly1305::{AeadInPlace, KeyInit, XChaCha20Poly1305};
use ed25519_dalek::{Signature, Verifier, VerifyingKey, SIGNATURE_LENGTH};

use crate::DecoderError;

/// Header of a decoder payload
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct DecoderPayloadHeader {
    signature: [u8; SIGNATURE_LENGTH],
    chacha_nonce: [u8; 24],
    poly1305_tag: [u8; 16],
}

/// Verifies and decrypts any payload the decoder recieves.
///
/// This includes both satelite frames and subscription data.
/// Decrypted data overwrites ciphertext in payload and a reference to this data is returned.
///
/// # Payload Format
///
/// |-----------------------------------------------|
/// | Ed25519 Signature: 64 bytes                   |
/// |-----------------------------------------------|
/// | XChaCha20Poly1305 Nonce: 24 bytes             |
/// |-----------------------------------------------|
/// | Poly1305 Tag: 16 bytes                        |
/// |-----------------------------------------------|
/// | Ciphertext: variable amount bytes             |
/// |-----------------------------------------------|
/// | Associated Data: `associated_data_size` bytes |
/// |-----------------------------------------------|
pub fn decrypt_decoder_payload<'a>(
    payload: &'a mut [u8],
    associated_data_size: usize,
    symmetric_key: &[u8; 32],
    public_key: &VerifyingKey,
) -> Result<&'a [u8], DecoderError> {
    if payload.len() < size_of::<DecoderPayloadHeader>() + associated_data_size {
        return Err(DecoderError::InvalidEncoderPayload);
    }

    let header: DecoderPayloadHeader = *from_bytes(&payload[..size_of::<DecoderPayloadHeader>()]);

    // first verify signature
    // signaute should include chacha nonce and tag, otherwise attacker can alter nonce and get invalid frame
    // decode for scenario 5 if they have the key
    let message_to_verify = &payload[SIGNATURE_LENGTH..];
    let Ok(_) = public_key.verify(message_to_verify, &Signature::from_bytes(&header.signature))
    else {
        return Err(DecoderError::InvalidEncoderPayload);
    };

    // retrieve ciphertext and associated data
    let body = &mut payload[size_of::<DecoderPayloadHeader>()..];
    let (ciphertext, associated_data) = body.split_at_mut(body.len() - associated_data_size);

    // then decrypt message
    let cipher = XChaCha20Poly1305::new(symmetric_key.into());
    let Ok(_) = cipher.decrypt_in_place_detached(
        &header.chacha_nonce.into(),
        associated_data,
        ciphertext,
        &header.poly1305_tag.into(),
    ) else {
        return Err(DecoderError::InvalidEncoderPayload);
    };

    Ok(ciphertext)
}

/// Gets a reference to the associated data of the given decoder payload.
pub fn get_decoder_payload_associated_data<T: Pod>(payload: &[u8]) -> Result<&T, DecoderError> {
    if payload.len() < size_of::<DecoderPayloadHeader>() + size_of::<T>() {
        return Err(DecoderError::InvalidEncoderPayload);
    } else {
        let associated_data = &payload[payload.len() - size_of::<T>()..];
        Ok(from_bytes(associated_data))
    }
}
