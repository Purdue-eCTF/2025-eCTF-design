use ed25519_dalek::VerifyingKey;

use crate::ectf_params::{SUBSCRIPTION_ENC_KEY, SUBSCRIPTION_PUBLIC_KEY};
use crate::message::{Message, Opcode};
use crate::{crypto::decrypt_decoder_payload, decoder_context::DecoderContext, DecoderError};

pub fn subscribe(
    context: &mut DecoderContext,
    subscribe_data: &mut [u8],
) -> Result<(), DecoderError> {
    let public_key = VerifyingKey::from_bytes(&SUBSCRIPTION_PUBLIC_KEY)
        .expect("decoder loaded with invalid public key");

    let subscription_data =
        decrypt_decoder_payload(subscribe_data, 0, &SUBSCRIPTION_ENC_KEY, &public_key)?;

    // TODO: parse subscription and update data in flash

    let response = Message::new(Opcode::Subscribe, 0, [0; 1024]);
    // TODO: handle error
    response.write().unwrap();

    Ok(())
}
