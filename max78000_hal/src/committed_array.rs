use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU8, Ordering};

use thiserror_no_std::Error;

use crate::i2c::MAX_I2C_MESSAGE_LEN;

const COMMITTED_ARRAY_CAPACITY: usize = MAX_I2C_MESSAGE_LEN;

// various statuses for committed array
const EMPTY: u8 = 0;
const BUSY: u8 = 1;
const FULL: u8 = 2;

#[derive(Debug, Error)]
pub enum CommittedArrayError {
    #[error("The committed array is currently being written somewhere else")]
    Busy,
    #[error("The input to the committed array was too big")]
    InputTooBig,
    #[error("The output buffer was not large enough to hold the data in the committed array")]
    OutputBufferTooSmall,
}

pub struct CommittedArray {
    status: AtomicU8,
    inner: UnsafeCell<CommittedArrayData>,
}

struct CommittedArrayData {
    data_len: usize,
    data: [u8; COMMITTED_ARRAY_CAPACITY],
}

impl CommittedArray {
    pub const fn new() -> Self {
        CommittedArray {
            status: AtomicU8::new(EMPTY),
            inner: UnsafeCell::new(CommittedArrayData {
                data_len: 0,
                data: [0; COMMITTED_ARRAY_CAPACITY],
            }),
        }
    }

    unsafe fn inner(&self) -> &mut CommittedArrayData {
        unsafe {
            self.inner.get().as_mut().unwrap()
        }
    }

    pub fn try_commit(&self, data: &[u8]) -> Result<(), CommittedArrayError> {
        if data.len() > COMMITTED_ARRAY_CAPACITY {
            return Err(CommittedArrayError::InputTooBig);
        }

        self.status.compare_exchange(
            EMPTY,
            BUSY,
            Ordering::Acquire,
            Ordering::Relaxed
        ).or(Err(CommittedArrayError::Busy))?;

        // safety: the committed queue is currently in the busy state, no one else can access the inner data
        let inner = unsafe { self.inner() };

        inner.data_len = data.len();
        inner.data[..data.len()].copy_from_slice(data);

        // release synchronizes with the compare exchange acquire
        self.status.store(FULL, Ordering::Release);

        Ok(())
    }

    pub fn try_take<'a>(&self, buf: &'a mut [u8]) -> Result<&'a [u8], CommittedArrayError> {
        self.status.compare_exchange(
            FULL,
            BUSY,
            Ordering::Acquire,
            Ordering::Relaxed
        ).or(Err(CommittedArrayError::Busy))?;

        // safety: the committed queue is currently in the busy state, no one else can access the inner data
        let inner = unsafe { self.inner() };

        if buf.len() < inner.data_len {
            self.status.store(FULL, Ordering::Release);
            return Err(CommittedArrayError::OutputBufferTooSmall);
        }

        let out = &mut buf[..inner.data_len];
        out.copy_from_slice(&inner.data[..inner.data_len]);
        inner.data_len = 0;

        // release synchronizes with the compare exchange acquire
        self.status.store(EMPTY, Ordering::Release);

        Ok(out)
    }
}

// safety: atomics synchronize access to unsafe cell
unsafe impl Send for CommittedArray {}
unsafe impl Sync for CommittedArray {}
