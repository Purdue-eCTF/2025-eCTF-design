use bytemuck::{must_cast_slice, Pod, Zeroable};
use core::{marker::PhantomData, slice};
use thiserror_no_std::Error;

use max78000_hal::flash::FLASH_PAGE_SIZE;
use max78000_hal::{flash::PAGE_MASK, Flash, Peripherals};

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tinyvec::ArrayVec;

use crate::ectf_params::{FLASH_DATA_ADDR, MAX_SUBSCRIPTIONS};

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
        assert_eq!(
            address & PAGE_MASK,
            address,
            "address is not flash page aligned"
        );

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
        unsafe { core::ptr::read(ptr) }
    }

    pub fn has_object(&self) -> bool {
        self.status() == FLASH_ENTRY_MAGIC
    }

    pub fn get(&self) -> Option<T> {
        if self.has_object() {
            // trait bound AnyBitPattern ensures flash data valid for any bits
            let object = unsafe { core::ptr::read(self.address as *const T) };

            Some(object)
        } else {
            None
        }
    }

    pub fn set(&mut self, object: &T) {
        let flash = Flash::get();

        // convert object to bytes
        let data = must_cast_slice(slice::from_ref(object));

        // 16 bytes for status at end
        assert!(data.len() < FLASH_PAGE_SIZE - 16);

        unsafe {
            // erase page first
            // safety: no references should be at this page, since no references are returned on get
            flash
                .erase_page(self.address)
                .expect("failed to erase flash page");

            // write data
            // safety: write is to address which is assumed valid when constructing FlashEntry
            flash
                .write(self.address, data)
                .expect("failed to write object data to flash");

            // write status after whole object written
            // safety: status resides within this flash page
            flash
                .write(self.status_address(), &FLASH_ENTRY_MAGIC.to_ne_bytes())
                .expect("failed to write status to flash");
        }
    }
}

/// Data stored on flash for each subscription.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SubscriptionEntry {
    /// Start of subscription (inclusive)
    start_time: u64,
    /// End of subscription (exlusive? TODO: figure out if inclusive or exclusive)
    end_time: u64,
    /// Id of channel subscription is for
    channel_id: u32,
    /// Number of internal nodes in subtree for deriving frame keys
    // bigger than needed for padding
    subtree_count: u32,
    /// Public key for channel
    public_key: [u8; 32],
    /// Internal nodes used to reconstruct frame keys
    // 126 is I beleive worse case scenario for how many subtrees we need
    subtrees: [KeySubtree; 128],
}

/// Represents information about one internal node in the key tree for a channel.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
pub struct KeySubtree {
    /// If timeatamp & mask == timestamp_value, timestamp is in this node
    timestamp_value: u64,
    mask: u64,
    /// Amount of bits to shift timestamp to the left after a match to start deriving key
    // bigger than needed for padding
    shift: u64,
    /// Value of internal node used to derive keys
    key: [u8; 32],
}

#[derive(Debug, Error)]
pub enum DecoderContextError {
    #[error("Too many subscriptions!")]
    TooManySubscriptions,
}

/// Information about channel sent back to tv for list channels
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct DecoderChannelInfo {
    channel_id: u32,
    start_time: u64,
    end_time: u64,
}

/// Stores state of decoder.
pub struct DecoderContext {
    /// Data for all subscriptions
    subscriptions: [FlashEntry<SubscriptionEntry>; MAX_SUBSCRIPTIONS],
    /// Timestamp of last decoded frame (starts at 0)
    pub last_decoded_timestamp: u64,
    /// PRNG used for random operations to help prevent glitching
    chacha: ChaCha20Rng,
}

impl DecoderContext {
    pub fn new() -> Self {
        let Peripherals { mut trng, .. } =
            Peripherals::take().expect("could not initialize peripherals");

        let chacha = ChaCha20Rng::from_seed(trng.gen_nonce());

        // safety: FLASH_PAGE_SIZE generated at build time is verified to be correct
        let subscriptions = unsafe {
            [
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 0),
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 1),
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 2),
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 3),
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 4),
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 5),
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 6),
                FlashEntry::new(FLASH_DATA_ADDR + FLASH_PAGE_SIZE * 7),
            ]
        };

        DecoderContext {
            subscriptions,
            last_decoded_timestamp: 0,
            chacha,
        }
    }

    #[allow(unused)]
    pub fn get_subscription_for_channel(&self, channel_id: u32) -> Option<SubscriptionEntry> {
        self.subscriptions
            .iter()
            .filter_map(|flash_entry| flash_entry.get())
            .find(|subscription| subscription.channel_id == channel_id)
    }

    #[allow(unused)]
    pub fn update_subscription(
        &mut self,
        subscription: &SubscriptionEntry,
    ) -> Result<(), DecoderContextError> {
        // update subscription if it already exists
        for flash_entry in self.subscriptions.iter_mut() {
            let Some(subscription_old) = flash_entry.get() else {
                continue;
            };

            if subscription_old.channel_id == subscription.channel_id {
                flash_entry.set(subscription);
                return Ok(());
            }
        }

        // find empty slot if no subcription for given channel exists
        for flash_entry in self.subscriptions.iter_mut() {
            if !flash_entry.has_object() {
                flash_entry.set(subscription);
                return Ok(());
            }
        }

        Err(DecoderContextError::TooManySubscriptions)
    }

    /// Returns a list of info about all subscribed channels.
    ///
    /// Used for the list functionality with tv.
    pub fn list_channels(&self) -> ArrayVec<[DecoderChannelInfo; MAX_SUBSCRIPTIONS]> {
        let mut out = ArrayVec::new();

        for flash_entry in self.subscriptions.iter() {
            if let Some(subscription) = flash_entry.get() {
                out.push(DecoderChannelInfo {
                    channel_id: subscription.channel_id,
                    start_time: subscription.start_time,
                    end_time: subscription.end_time,
                });
            }
        }

        out
    }

    #[allow(unused)]
    /// Gets rng for random operations.
    pub fn get_chacha(&mut self) -> &mut ChaCha20Rng {
        &mut self.chacha
    }
}
