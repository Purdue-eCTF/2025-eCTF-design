use core::{marker::PhantomData, slice};
use core::time::Duration;
use bytemuck::{must_cast_slice, AnyBitPattern, Pod, Zeroable};

use max78000_hal::flash::FLASH_PAGE_SIZE;
use serde::{Serialize, Deserialize};
use max78000_hal::{flash::PAGE_MASK, i2c::{I2cAddr, MAX_I2C_MESSAGE_LEN}, Flash, MasterI2c, Peripherals, Trng, timer::sleep};
use design_utils::{component_id_to_i2c_addr, messages::ProtocolError, I2C_FREQUENCY};

use rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

use crate::ectf_params::FLASH_DATA_ADDR;
use crate::DecoderError;

const FLASH_ENTRY_MAGIC: u32 = 0x11aa0055;

/// Stores an object on a page of flash
pub struct FlashEntry<T: Pod> {
    address: usize,
    _marker: PhantomData<T>,
}

impl<T: Pod> FlashEntry<T> {
    /// Creates a new `FlashEntry`.
    /// 
    /// # Safety
    /// 
    /// `address` must be the address of the start of a flash page which is not in use by anything else (references to it, code on it, etc).
    unsafe fn new(address: usize) -> Self {
        assert_eq!(address & PAGE_MASK, address, "address is not flash page aligned");

        FlashEntry {
            address,
            _marker: PhantomData,
        }
    }

    fn status_address(&self) -> usize {
        self.address + FLASH_PAGE_SIZE - 16
    }

    fn status(&self) -> u32 {
        let ptr = self.status_address() as *const u32;

        // safety: address assumes to point to valid flash page, so this address is also on that page, and is a valid u32
        unsafe {
            core::ptr::read(ptr)
        }
    }

    pub fn has_object(&self) -> bool {
        self.status() == FLASH_ENTRY_MAGIC
    }

    pub fn get(&self) -> Option<T> {
        if self.has_object() {
            // trait bound AnyBitPattern ensures flash data valid for any bits
            let object = unsafe {
                core::ptr::read(self.address as *const T)
            };

            Some(object)
        } else {
            None
        }
    }

    pub fn set(&mut self, object: &T) {
        let flash = Flash::get();

        // safety: u8 slice is valid for any pattern of bits (technically not true for this, since uninit bytes, but probably fine right :))
        // TODO: fix this issue
        let data = must_cast_slice(slice::from_ref(object));
        /*let data = unsafe {
            slice::from_raw_parts(object as *const T as *const u8, size_of::<T>())
        };*/

        // 16 bytes for status at end
        assert!(data.len() < FLASH_PAGE_SIZE - 16);

        unsafe {
            // erase page first
            // safety: no references should be at this page, since no references are returned on get
            flash.erase_page(self.address)
                .expect("failed to erase flash page");

            // write data
            // safety: write is to address which is assumed valid when constructing FlashEntry
            flash.write(self.address, data)
                .expect("failed to write object data to flash");

            // write status after whole object written
            // safety: status resides within this flash page
            flash.write(self.status_address(), &FLASH_ENTRY_MAGIC.to_ne_bytes())
                .expect("failed to write status to flash");
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SubscriptionEntry {
    start_time: u64,
    end_time: u64,
    channel_id: u32,
    subtree_count: u8,
    _padding1: u8,
    _padding2: u16,
    public_key: [u8; 32],
    // 126 is I beleive worse case scenario for how many subtrees we need
    subtrees: [KeySubtree; 128],
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
pub struct KeySubtree {
    mask: u64,
    shift: u64,
    key: [u8; 32],
}

pub struct DecoderContext {
    subscriptions: [FlashEntry<SubscriptionEntry>; 8],
    last_decoded_timestamp: u64,
    chacha: ChaCha20Rng,
}

impl DecoderContext {
    pub fn new() -> Self {
        let Peripherals {
            mut trng,
            ..
        } = Peripherals::take().expect("could not initialize peripherals");

        let chacha = ChaCha20Rng::from_seed(trng.gen_nonce());

        // safety: FLASH_PAGE_SIZE generated at build time is verified to be correct
        let subscriptions = unsafe {[
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 0),
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 1),
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 2),
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 3),
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 4),
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 5),
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 6),
            FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 7),
        ]};

        DecoderContext {
            subscriptions,
            last_decoded_timestamp: 0,
            chacha,
        }
    }

    pub fn get_subscription_for_channel(&self, channel_id: u32) -> Option<SubscriptionEntry> {
        self.subscriptions.iter()
            .filter_map(|flash_entry| flash_entry.get())
            .filter(|subscription| subscription.channel_id == channel_id)
            .next()
    }

    pub fn update_subscription(&mut self, subscription: &SubscriptionEntry) {
        // update subscription if it already exists
        for flash_entry in self.subscriptions.iter_mut() {
            let Some(subscription_old) = flash_entry.get() else {
                continue;
            };

            if subscription_old.channel_id == subscription.channel_id {
                flash_entry.set(subscription);
                return;
            }
        }

        // find empty slot if no subcription for given channel exists
        for flash_entry in self.subscriptions.iter_mut() {
            if !flash_entry.has_object() {
                flash_entry.set(subscription);
                return;
            }
        }

        // TODO: return error instead
        panic!("too many subscriptions");
    }

    /// Gets rng for random operations.
    pub fn get_chacha(&mut self) -> &mut ChaCha20Rng {
        &mut self.chacha
    }
}