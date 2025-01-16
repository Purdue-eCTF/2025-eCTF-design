use max78000_hal::i2c::MAX_I2C_MESSAGE_LEN;
use serde::{Serialize, Deserialize};
use design_utils::{I2C_FREQUENCY, component_id_to_i2c_addr};
use design_utils::messages::ProtocolError;
use max78000_hal::{ClientI2c, Peripherals, Trng};

use crate::ComponentError;
use crate::ectf_params::COMPONENT_ID;

use rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

pub struct ComponentDriver {
    trng: Trng,
    i2c: ClientI2c,
    chacha: ChaCha20Rng,
    i2c_receive_buffer: [u8; MAX_I2C_MESSAGE_LEN],
}

impl ComponentDriver {
    pub fn new() -> Self {
        let Peripherals {
            i2c,
            mut trng,
            ..
        } = Peripherals::take().expect("could not initialize peripherals");

        let i2c = i2c.init_client(I2C_FREQUENCY, component_id_to_i2c_addr(COMPONENT_ID));
        let chacha = ChaCha20Rng::from_seed(trng.gen_nonce());

        ComponentDriver {
            trng,
            i2c,
            chacha,
            i2c_receive_buffer: [0; MAX_I2C_MESSAGE_LEN],
        }
    }

    fn send_serialized<T: Serialize>(i2c: &mut ClientI2c, data: &T) -> Result<(), ComponentError> {
        let mut send_buf = [0; MAX_I2C_MESSAGE_LEN];
        let serialized_data = postcard::to_slice(data, &mut send_buf)?;

        i2c.send(serialized_data)?;

        Ok(())
    }

    /// Send a successful response to the ap
    pub fn send_struct<T: Serialize>(&mut self, data: T) -> Result<(), ComponentError> {
        Self::send_serialized::<Result<T, ProtocolError>>(&mut self.i2c, &Ok(data))
    }

    /// Tell the ap an error occurred
    pub fn send_error(&mut self) -> Result<(), ComponentError> {
        Self::send_serialized::<Result<(), ProtocolError>>(&mut self.i2c, &Err(ProtocolError))
    }

    /// Attempts to receive a message from the ap
    pub fn recv_struct<'de, T: Deserialize<'de>>(&'de mut self) -> Result<T, ComponentError> {
        let ComponentDriver {
            i2c,
            i2c_receive_buffer,
            ..
        } = self;

        let recv_data = i2c.recv(i2c_receive_buffer.as_mut_slice())?;

        let response: Result<T, ProtocolError> = postcard::from_bytes(recv_data)?;
        match response {
            Ok(data) => Ok(data),
            Err(ProtocolError) => {
                // we need to update or i2c state to be ready for a receive by doing another send to match the earlier
                // receive also informs the ap we have acknowledged an error occurred
                Self::send_serialized::<Result<(), ProtocolError>>(i2c, &Err(ProtocolError))?;
                Err(ComponentError::ProtocolError)
            },
        }
    }

    pub fn get_chacha(&mut self) -> &mut ChaCha20Rng {
        &mut self.chacha
    } 

    pub fn gen_bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut nonce = [0u8; N];
        self.chacha.fill_bytes(&mut nonce);
        nonce
    }

    pub fn gen_nonce(&mut self) -> u64 {
        self.chacha.next_u64()
    }
}
