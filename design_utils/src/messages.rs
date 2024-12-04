use core::marker::PhantomData;

use max78000_hal::i2c::MAX_I2C_MESSAGE_LEN;
use serde::{Serialize, Deserialize};
use tinyvec::ArrayVec;

use crate::{DesignUtilsError, MAX_POST_BOOT_MESSAGE_SIZE};
use crate::crypto::{decrypt, encrypt, sign, verify_signature, EncryptedData};
use ed25519::Signature;

pub type Nonce = u64;

/// A signature that can be serialized with `serde`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub struct SerialSignature {
    pub r_bytes: [u8; 32],
    pub s_bytes: [u8; 32],
}

impl From<Signature> for SerialSignature {
    fn from(signature: Signature) -> Self {
        Self {
            r_bytes: *signature.r_bytes(),
            s_bytes: *signature.s_bytes(),
        }
    }
}

impl From<SerialSignature> for Signature {
    fn from(value: SerialSignature) -> Self {
        Self::from_components(value.r_bytes, value.s_bytes)
    }
}

/// A message with a signature and byte data that can be serialized / deserialized to type `T`.
#[derive(Serialize, Deserialize, Debug)]
pub struct SignedMessage<T> {
    pub message_data: ArrayVec<[u8; MAX_I2C_MESSAGE_LEN]>,
    signature: SerialSignature,
    _marker: PhantomData<T>
}

impl<T: Serialize> SignedMessage<T> {
    pub fn new_signed(message: T, key_bytes: &[u8; 32]) -> Result<Self, DesignUtilsError> {
        let mut buf = [0; MAX_I2C_MESSAGE_LEN];
        let serialized_data = postcard::to_slice(&message, &mut buf)?;

        let mut message_data = ArrayVec::new();
        message_data.extend_from_slice(serialized_data);

        let signature = sign(serialized_data, key_bytes);

        Ok(SignedMessage {
            message_data,
            signature: signature.into(),
            _marker: PhantomData,
        })
    }
}

impl<T> SignedMessage<T> {
    pub fn signature(&self) -> Signature {
        self.signature.into()
    }
}

impl<'de, T: Deserialize<'de>> SignedMessage<T> {
    pub fn verify(&self, verify_key_bytes: &[u8; 32]) -> bool {
        verify_signature(self.message_data.as_slice(), &self.signature.into(), verify_key_bytes)
    }

    pub fn get_data_verified(&'de self, verify_key_bytes: &[u8; 32]) -> Result<T, DesignUtilsError> {
        if self.verify(verify_key_bytes) {
            let message = postcard::from_bytes(self.message_data.as_slice())?;
            Ok(message)
        } else {
            Err(DesignUtilsError::InvalidSignature)
        }
    }
}

/// A message with encrypted data that can be decrypted and serialized / deserialized
/// to type `T`.
#[derive(Serialize, Deserialize, Debug)]
pub struct EncryptedMessage<T> {
    encrypted_data: EncryptedData<MAX_I2C_MESSAGE_LEN>,
    _marker: PhantomData<T>,
}

impl<T: Serialize> EncryptedMessage<T> {
    /// Encrypts a message of type `T` into an `EncryptedMessage` using the provided key and nonce.
    ///
    /// # Arguments
    /// * `message` - The message to encrypt.
    /// * `key` - The key to use.
    /// * `nonce` - The nonce to use.
    ///
    /// Returns the encrypted message.
    pub fn new_encrypted(message: T, key: &[u8; 32], nonce: [u8; 12]) -> Result<Self, DesignUtilsError> {
        let mut serialized_data = [0; MAX_I2C_MESSAGE_LEN];
        let serialized_data = postcard::to_slice(&message, &mut serialized_data)?;

        let encrypted_data = encrypt(serialized_data, key, nonce)?;
        Ok(EncryptedMessage {
            encrypted_data,
            _marker: PhantomData,
        })
    }
}

impl<'de, T: Deserialize<'de>> EncryptedMessage<T> {
    /// Gets the decrypted data from this encrypted message using the provided key.
    ///
    /// # Arguments
    /// * `key` - The key to use.
    ///
    /// Returns `Ok(T)` on success, or an appropriate error.
    ///
    /// Note: this can only be called once, subsequent calls will return error
    pub fn get_decrypted_data(&'de mut self, key: &[u8; 32]) -> Result<T, DesignUtilsError> {
        let decrypted_data = decrypt(&mut self.encrypted_data, key)?;
        Ok(postcard::from_bytes(decrypted_data.as_slice())?)
    }
}


/// The first message sent to the component to specify the type of protocol to start.
#[derive(Serialize, Deserialize, Debug)]
pub enum StartProtocolMessage {
    ScanId,
    Attest,
    Boot(EncryptedMessage<BootMessageStart>),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct ProtocolError;

// scan messages
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct ScanIdResponse {
    pub component_id: u32
}

// attest messages
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct AttestationReqMessage {
    pub component_id: u32,
    pub nonce: Nonce,
    pub hmac: [u8; 32],
}

pub type AttestationRespMessage = EncryptedData<256>;

// boot messages
/// This is a byte which indicates which stage of the boot protocol the message is a part of
#[repr(u8)]
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
pub enum MessageStage {
    M1,
    M2,
    M3,
}

/// The firt message in the chain of boot messages, sent by ap to component
#[derive(Serialize, Deserialize, Debug)]
pub struct BootMessageStart {
    pub m: MessageStage,
    pub component_id: u32,
    pub nonce: Nonce,
}

/// Sent by component to ap to verify it is authentic
#[derive(Serialize, Deserialize, Debug)]
pub struct BootMessageComponentReply {
    pub m: MessageStage,
    pub component_id: u32,
    pub build_id: u32,
    pub start_nonce_plus_one: u64,
    pub reply_nonce: u64,
}

/// Sent by ap to component to verify aps authenticity and to tell component to boot
#[derive(Serialize, Deserialize, Debug)]
pub struct BootMessageFinalize {
    pub m: MessageStage,
    pub component_id: u32,
    pub reply_nonce_plus_one: u64,
}

// post boot messages
#[derive(Serialize, Deserialize, Debug)]
pub enum PostBootMessageStart {
    // the ap is going to receive a message from the component
    // it has first sent a nonce to the component
    ApToComponentNonce(Nonce),
    // the ap is going to send a message to the component
    // it is first requesting the component which nonce to use
    RequestComponentNonce,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PostBootMessage {
    pub component_id: u32,
    pub nonce: Nonce,
    pub message: ArrayVec<[u8; MAX_POST_BOOT_MESSAGE_SIZE]>,
}

impl PostBootMessage {
    pub fn get_bytes_to_sign(&self) -> ArrayVec<[u8; MAX_I2C_MESSAGE_LEN]> {
        let mut signature_buffer = ArrayVec::<[u8; MAX_I2C_MESSAGE_LEN]>::new();
        signature_buffer.extend_from_slice(&self.component_id.to_le_bytes());
        signature_buffer.extend_from_slice(&self.nonce.to_le_bytes());
        signature_buffer.extend_from_slice(&self.message.as_slice());

        signature_buffer
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SignedPostBootMessage {
    pub message: PostBootMessage,
    pub signature: SerialSignature,
}
