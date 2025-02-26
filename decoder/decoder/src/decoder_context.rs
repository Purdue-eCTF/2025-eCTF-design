use bytemuck::{bytes_of, Pod, Zeroable};
use max78000_hal::mpu::{MemoryCacheType, MpuPerms, MpuRegionSize};
use core::marker::PhantomData;
use ed25519_dalek::VerifyingKey;
use thiserror_no_std::Error;

use max78000_hal::flash::{FLASH_BASE_ADDR, FLASH_PAGE_SIZE, FLASH_SIZE, PAGE_MASK};
use max78000_hal::{Flash, Peripherals};

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tinyvec::ArrayVec;

use crate::ectf_params::{FLASH_DATA_ADDRS, MAX_SUBSCRIPTIONS, SUBSCRIPTION_PUBLIC_KEY, CHANNEL0_PUBLIC_KEY};

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
    /// `address` must be the address of the start of a flash page which is not in use by anything
    /// else (references to it, code on it, etc.).
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

    /// Every flash entry stores a status section at the end of the page indicating if it contains valid data or not.
    /// 
    /// This returns address of status section.
    fn status_address(&self) -> usize {
        self.address + FLASH_PAGE_SIZE - 16
    }

    /// Retreive status indicating if `FlashEntry` contains data or not.
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
    /// Public key for channel
    pub public_key: [u8; 32],
    /// Start of subscription (inclusive)
    pub start_time: u64,
    /// End of subscription (inclusive)
    pub end_time: u64,
    // Channel id subscription is for
    pub channel_id: u32,
    /// Number of internal nodes in subtree for deriving frame keys
    // bigger than needed for padding
    pub subtree_count: u32,
    /// Internal nodes used to reconstruct frame keys
    // 126 is I believe worse case scenario for how many subtrees we need
    pub subtrees: [KeySubtree; 128],
}

impl SubscriptionEntry {
    /// Gets subtrees which are in use for the subscription.
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

impl KeySubtree {
    /// Checks if the leaf key node corresponding to `timestamp` lies within this subtree.
    pub fn contains(&self, timestamp: u64) -> bool {
        self.lowest_timestamp <= timestamp && timestamp <= self.highest_timestamp
    }
}

/// Cached information about channel to speed up decoding performance.
#[derive(Debug, Clone, Copy)]
pub struct ChannelCache {
    /// Parsed public key used to verify frames
    pub public_key: VerifyingKey,
    /// Caches the last used key for each level in the channel key tree.
    ///
    /// Timestamps close together will likely share most of the same keys,
    /// so the cache can be used instead of recomputing keys.
    ///
    /// The first index in the cache is the biggest entry, and so on until the leaf.
    pub cache_entries: ArrayVec<[KeySubtree; 64]>,
}

impl ChannelCache {
    fn new(public_key: &[u8; 32]) -> Self {
        ChannelCache {
            public_key: VerifyingKey::from_bytes(public_key)
                .expect("Invalid public key for subscription"),
            cache_entries: ArrayVec::new(),
        }
    }
}

/// Contains info needed to decode frames for a channel.
struct ChannelInfo {
    /// Contains all persistant data related to channel stored in flash.
    flash_entry: FlashEntry<SubscriptionEntry>,
    /// Contains cached info about channel.
    /// 
    /// None if there is no subscription for channel.
    cache: Option<ChannelCache>,
}

impl ChannelInfo {
    /// Constructs new `ChannelInfo` object by reading from flash which may or may not contain subscription data.
    /// 
    /// # Safety
    /// 
    /// `flash_data_addr` must be the address of the start of a flash page used for storing subscription entries.
    /// 
    /// It cannot point to code for example.
    unsafe fn new(flash_data_addr: usize) -> Self {
        let flash_entry: FlashEntry<SubscriptionEntry> = unsafe {
            FlashEntry::new(flash_data_addr)
        };

        let cache = if let Some(subscription) = flash_entry.get() {
            Some(ChannelCache::new(&subscription.public_key))
        } else {
            None
        };

        ChannelInfo {
            flash_entry,
            cache,
        }
    }

    /// Gets the channel id for this ChannelInfo, or `None` if it is not subscribed to any channel.
    fn channel_id(&self) -> Option<u32> {
        Some(self.flash_entry.get()?.channel_id)
    }

    /// Updates the subscription for this channel cache
    fn set_subscription(&mut self, subscription: &SubscriptionEntry) {
        self.flash_entry.set(subscription);
        self.cache = Some(ChannelCache::new(&subscription.public_key));
    }
}

#[derive(Debug, Error)]
pub enum DecoderContextError {
    #[error("Too many subscriptions!")]
    TooManySubscriptions,
}

/// Format of information about channel sent back to tv host tools for list channels command.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct DecoderChannelInfoResult {
    channel_id: u32,
    start_time: u64,
    end_time: u64,
}

/// Stores state of decoder.
pub struct DecoderContext {
    /// Data for all subscriptions, indexed by channel id
    subscriptions: [ChannelInfo; MAX_SUBSCRIPTIONS],
    /// Timestamp of last decoded frame (starts at 0)
    pub last_decoded_timestamp: Option<u64>,
    /// PRNG used for random operations to help prevent glitching
    chacha: ChaCha20Rng,
    /// Verifying public key for subscriptions
    pub subscription_public_key: VerifyingKey,
    /// Verifying public key for frames on the emergency channel
    pub emergency_channel_public_key: VerifyingKey,
}

impl DecoderContext {
    /// Initialize decoder state and setup all necessary peripherals.
    pub fn new() -> Self {
        let Peripherals { mut trng, mut mpu, .. } =
            Peripherals::take().expect("could not initialize peripherals");

        let chacha = ChaCha20Rng::from_seed(trng.gen_nonce());

        // lock all flash pages not used for storing subscription data
        for page_address in
            (FLASH_BASE_ADDR..(FLASH_BASE_ADDR + FLASH_SIZE)).step_by(FLASH_PAGE_SIZE)
        {
            if !FLASH_DATA_ADDRS.contains(&page_address) {
                Flash::get().lock_page(page_address);
            }
        }

        // set up memory protections
        unsafe {
            // make flash executable
            mpu.set_region(
                0,
                0x1000_0000,
                MpuRegionSize::KibiByte512, // ends 0x1008_0000
                0,
                MpuPerms {
                    read: true,
                    write: false,
                    execute: true,
                },
                MemoryCacheType::StronglyOrdered,
            );

            // make ram read write
            mpu.set_region(
                1,
                0x2000_0000,
                MpuRegionSize::KibiByte128, // ends 0x2002_0000
                0,
                MpuPerms {
                    read: true,
                    write: true,
                    execute: false,
                },
                MemoryCacheType::StronglyOrdered,
            );

            // make peripheral memory read write
            mpu.set_region(
                2,
                0x4000_0000,
                MpuRegionSize::MibiByte512,
                0,
                MpuPerms {
                    read: true,
                    write: true,
                    execute: false,
                },
                MemoryCacheType::StronglyOrdered,
            );

            mpu.clear_region(3);
            mpu.clear_region(4);
            mpu.clear_region(5);
            mpu.clear_region(6);
            mpu.clear_region(7);

            mpu.enable();
        }

        let subscriptions = unsafe {
            [
                ChannelInfo::new(FLASH_DATA_ADDRS[0]),
                ChannelInfo::new(FLASH_DATA_ADDRS[1]),
                ChannelInfo::new(FLASH_DATA_ADDRS[2]),
                ChannelInfo::new(FLASH_DATA_ADDRS[3]),
                ChannelInfo::new(FLASH_DATA_ADDRS[4]),
                ChannelInfo::new(FLASH_DATA_ADDRS[5]),
                ChannelInfo::new(FLASH_DATA_ADDRS[6]),
                ChannelInfo::new(FLASH_DATA_ADDRS[7]),
            ]
        };

        let subscription_public_key = VerifyingKey::from_bytes(&SUBSCRIPTION_PUBLIC_KEY)
            .expect("decoder loaded with invalid public key");

        let emergency_channel_public_key = VerifyingKey::from_bytes(&CHANNEL0_PUBLIC_KEY)
            .expect("decoder loaded with invaid public key");

        DecoderContext {
            subscriptions,
            last_decoded_timestamp: None,
            chacha,
            subscription_public_key,
            emergency_channel_public_key,
        }
    }

    fn get_channel_info_for_id(&mut self, channel_id: u32) -> Option<&mut ChannelInfo> {
        self.subscriptions.iter_mut()
            .find(|channel_info| matches!(channel_info.channel_id(), Some(cid) if cid == channel_id))
    }

    fn find_empty_channel_info(&mut self) -> Option<&mut ChannelInfo> {
        self.subscriptions.iter_mut()
            .find(|channel_info| channel_info.channel_id().is_none())
    }

    /// Retreives bot nonvalatile and volatile cached information about a subscription on the given `channel_id`.
    /// 
    /// Returns `None` if no subscription exists for the given channel.
    pub fn get_subscription_for_channel(
        &mut self,
        channel_id: u32,
    ) -> Option<(&SubscriptionEntry, &mut ChannelCache)> {
        let ChannelInfo {
            flash_entry,
            cache,
        } = self.get_channel_info_for_id(channel_id)?;

        Some((flash_entry.get().unwrap(), cache.as_mut().unwrap()))
    }

    /// Updates subscription information using provided `subscription`.
    /// 
    /// If a subscription with the same channel id already exists, it is overwritten.
    /// If no such subscription exists, a new slot is used to store the subscription.
    /// If all 8 subscription slots have been taken, `update_subscription` will return an error.
    pub fn update_subscription(
        &mut self,
        subscription: &SubscriptionEntry,
    ) -> Result<(), DecoderContextError> {
        if let Some(channel_info) = self.get_channel_info_for_id(subscription.channel_id) {
            channel_info.set_subscription(subscription);
            Ok(())
        } else if let Some(channel_info) = self.find_empty_channel_info()  {
            channel_info.set_subscription(subscription);
            Ok(())
        } else {
            Err(DecoderContextError::TooManySubscriptions)
        }
    }

    /// Returns a list of info about all subscribed channels.
    ///
    /// Used for the list functionality with tv.
    pub fn list_channels(&self) -> ArrayVec<[DecoderChannelInfoResult; MAX_SUBSCRIPTIONS]> {
        let mut out = ArrayVec::new();

        for channel_info in &self.subscriptions {
            if let Some(subscription) = channel_info.flash_entry.get() {
                out.push(DecoderChannelInfoResult {
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
