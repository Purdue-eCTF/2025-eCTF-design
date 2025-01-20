use core::slice;
use core::time::Duration;
use bytemuck::{must_cast_slice, Pod, Zeroable};

use serde::{Serialize, Deserialize};
use max78000_hal::{flash::PAGE_MASK, i2c::{I2cAddr, MAX_I2C_MESSAGE_LEN}, Flash, MasterI2c, Peripherals, Trng, timer::sleep};
use design_utils::{component_id_to_i2c_addr, messages::ProtocolError, I2C_FREQUENCY};

use rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

use crate::ectf_params::FLASH_DATA_ADDR;
use crate::ApError;

pub const FLASH_MAGIC: u32 = 0xdeadbeef;
pub const FLASH_DATA: *const FlashData = FLASH_DATA_ADDR as *const FlashData;

pub struct DecoderDriver {
    flash: Flash,
    i2c: MasterI2c,
    trng: Trng,
    chacha: ChaCha20Rng,
    flash_data: Option<FlashData>,
    i2c_recv_buffer: [u8; MAX_I2C_MESSAGE_LEN],
}

impl DecoderDriver {
    pub fn new() -> Self {
        let Peripherals {
            flash,
            i2c,
            mut trng,
        } = Peripherals::take().expect("could not initialize peripherals");

        let i2c = i2c.init_master(I2C_FREQUENCY);
        let chacha = ChaCha20Rng::from_seed(trng.gen_nonce());

        DecoderDriver {
            flash,
            i2c,
            trng,
            chacha,
            flash_data: None,
            i2c_recv_buffer: [0; MAX_I2C_MESSAGE_LEN],
        }
    }

    pub fn get_flash_data(&mut self) -> FlashData {
        if let Some(flash_data) = self.flash_data {
            flash_data
        } else {
            // read data from flash if it is not yet read
            // safety: flash data should be valid for any pattern of bytes
            let mut flash_data = unsafe {
                core::ptr::read(FLASH_DATA)
            };

            // if flash is not initialized, component ids we are provisioned for
            if flash_data.flash_magic != FLASH_MAGIC {
                flash_data.flash_magic = FLASH_MAGIC;
                //flash_data.components_len = COMPONENTS.len();
                //flash_data.components[..COMPONENTS.len()].copy_from_slice(COMPONENTS.as_slice());
            }

            self.flash_data = Some(flash_data);
            flash_data
        }
    }

    pub fn save_flash_data(&mut self, flash_data: FlashData) {
        // safety: nothing else is present at the flash address, linker script only uses bottom half of flash
        unsafe {
            self.flash.erase_page(FLASH_DATA_ADDR & PAGE_MASK)
                .expect("could not erase flash page");

            self.flash.write(FLASH_DATA_ADDR, must_cast_slice(slice::from_ref(&flash_data)))
                .expect("could not save data to flash");
        }

        self.flash_data = Some(flash_data);
    }

    pub fn get_chacha(&mut self) -> &mut ChaCha20Rng {
        &mut self.chacha
    }

    fn send_packet(&mut self, address: I2cAddr, packet: &[u8]) -> Result<(), ApError> {
        let mut send_packet = [0; MAX_I2C_MESSAGE_LEN];
        send_packet[0] = packet.len().try_into().expect("i2c send message to big");
        send_packet[1..(packet.len() + 1)].copy_from_slice(packet);

        self.i2c.send(address, &send_packet[..(packet.len() + 1)])?;

        Ok(())
    }

    fn recieve_packet(&mut self, address: I2cAddr) -> Result<&[u8], ApError> {
        let mut recv_len = 0;

        sleep(Duration::from_millis(3));

        loop {
            self.i2c.recv(address, slice::from_mut(&mut recv_len))?;

            // delay to allow component to get work done while requesting response, and delay is needed between next read
            sleep(Duration::from_millis(5));

            if recv_len != 0 {
                self.i2c.recv(address, &mut self.i2c_recv_buffer[..recv_len.into()])?;

                return Ok(&self.i2c_recv_buffer[..recv_len.into()]);
            }
        }
    }

    pub fn send_struct<M: Serialize>(&mut self, address: I2cAddr, message: M) -> Result<(), ApError> {
        let mut send_buf = [0; MAX_I2C_MESSAGE_LEN];
        let serialized_message = postcard::to_slice::<Result<M, ProtocolError>>(&Ok(message), &mut send_buf)?;

        self.send_packet(address, serialized_message)
    }

    pub fn receive_struct<'de, R: Deserialize<'de>>(&'de mut self, address: I2cAddr) -> Result<R, ApError> {
        let response_bytes = self.recieve_packet(address)?;

        let response: Result<R, ProtocolError> = postcard::from_bytes(response_bytes)?;
        Ok(response?)
    }

    pub fn send_and_receive_struct<'de, M: Serialize, R: Deserialize<'de>>(
        &'de mut self,
        address: I2cAddr, message: M
    ) -> Result<R, ApError> {
        self.send_struct(address, message)?;
        self.receive_struct(address)
    }

    pub fn send_error(&mut self, address: I2cAddr) -> Result<(), ApError> {
        let mut send_buf = [0; MAX_I2C_MESSAGE_LEN];
        let serialized_message = postcard::to_slice::<Result<(), ProtocolError>>(&Err(ProtocolError), &mut send_buf)?;

        self.send_packet(address, serialized_message)?;
        let response_bytes = self.recieve_packet(address)?;

        // this response should be an error, but we don't really need to check if it is
        let _response: Result<(), ProtocolError> = postcard::from_bytes(response_bytes)?;

        Ok(())
    }

    pub fn gen_bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut bytes = [0u8; N];
        self.chacha.fill_bytes(&mut bytes);
        bytes
    }

    pub fn gen_nonce(&mut self) -> u64 {
        self.chacha.next_u64()
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
pub struct ProvisionedComponent {
    pub component_id: u32,
    pub key_index: usize,
}

/// Datatype for information stored in flash
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
pub struct FlashData {
    pub(crate) components_len: usize,
    pub(crate) components: [ProvisionedComponent; 2],
    pub(crate) flash_magic: u32,
}

impl FlashData {
    pub fn get_provisioned_component(&mut self, component_id: u32) -> Option<&mut ProvisionedComponent> {
        for i in 0..self.components_len {
            if self.components[i].component_id == component_id {
                return Some(&mut self.components[i]);
            }
        }

        None
    }

    pub fn get_component_for_i2c_addr(&self, i2c_addr: I2cAddr) -> Option<&ProvisionedComponent> {
        for i in 0..self.components_len {
            if component_id_to_i2c_addr(self.components[i].component_id) == i2c_addr {
                return Some(&self.components[i]);
            }
        }

        None
    }
}
