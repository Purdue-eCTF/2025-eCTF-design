use bytemuck::{bytes_of, Pod, Zeroable};
use core::marker::PhantomData;
use thiserror_no_std::Error;

use max78000_hal::flash::FLASH_PAGE_SIZE;
use max78000_hal::{flash::PAGE_MASK, Flash, Peripherals};

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tinyvec::ArrayVec;

use crate::ectf_params::{FLASH_DATA_ADDRS, MAX_SUBSCRIPTIONS};

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
        // make sure T is not too big
        const {
            assert!(size_of::<T>() <= FLASH_PAGE_SIZE - 16);
        }

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
        // I think this should be volatile, since underlying flash can change by writing to unrelated address
        // so it kind of acts like io memory that will change without specific interaction from the compiler pov
        unsafe { core::ptr::read_volatile(ptr) }
    }

    pub fn has_object(&self) -> bool {
        self.status() == FLASH_ENTRY_MAGIC
    }

    pub fn get(&self) -> Option<&T> {
        if self.has_object() {
            // trait bound AnyBitPattern ensures flash data valid for any bits
            unsafe { (self.address as *const T).as_ref() }
        } else {
            None
        }
    }

    pub fn set(&mut self, object: &T) {
        let flash = Flash::get();

        // convert object to bytes
        let data = bytes_of(object);

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
    pub start_time: u64,
    /// End of subscription (inclusive)
    pub end_time: u64,
    /// Id of channel subscription is for
    pub channel_id: u32,
    /// Number of internal nodes in subtree for deriving frame keys
    // bigger than needed for padding
    pub subtree_count: u32,
    /// Public key for channel
    pub public_key: [u8; 32],
    /// Internal nodes used to reconstruct frame keys
    // 126 is I believe worse case scenario for how many subtrees we need
    pub subtrees: [KeySubtree; 128],
}

impl SubscriptionEntry {
    pub fn active_subtrees(&self) -> &[KeySubtree] {
        &self.subtrees[..self.subtree_count as usize]
    }
}

/// Represents information about one internal node in the key tree for a channel.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
pub struct KeySubtree {
    /// The lowest timestamp that this subtree can provide
    pub lowest_timestamp: u64,
    /// The highest timestamp that this subtree can provide
    pub highest_timestamp: u64,
    /// Value of internal node used to derive keys
    pub key: [u8; 32],
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
    pub last_decoded_timestamp: Option<u64>,
    /// PRNG used for random operations to help prevent glitching
    chacha: ChaCha20Rng,
}

impl DecoderContext {
    pub fn new() -> Self {
        let Peripherals { mut trng, .. } =
            Peripherals::take().expect("could not initialize peripherals");

        let chacha = ChaCha20Rng::from_seed(trng.gen_nonce());

        // safety: FLASH_DATA_ADDRS generated at build time are verified to be correct
        // and made to not overlap with anything else
        let subscriptions = unsafe {
            [
                FlashEntry::new(FLASH_DATA_ADDRS[0]),
                FlashEntry::new(FLASH_DATA_ADDRS[1]),
                FlashEntry::new(FLASH_DATA_ADDRS[2]),
                FlashEntry::new(FLASH_DATA_ADDRS[3]),
                FlashEntry::new(FLASH_DATA_ADDRS[4]),
                FlashEntry::new(FLASH_DATA_ADDRS[5]),
                FlashEntry::new(FLASH_DATA_ADDRS[6]),
                FlashEntry::new(FLASH_DATA_ADDRS[7]),
            ]
        };

        DecoderContext {
            subscriptions,
            last_decoded_timestamp: None,
            chacha,
        }
    }

    pub fn get_subscription_for_channel(&self, channel_id: u32) -> Option<&SubscriptionEntry> {
        self.subscriptions
            .iter()
            .filter_map(|flash_entry| flash_entry.get())
            .find(|subscription| subscription.channel_id == channel_id)
    }

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
