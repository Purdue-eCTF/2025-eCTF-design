use core::cell::RefCell;
use core::ops::Add;
use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;

use cortex_m::interrupt::{self, Mutex};
use cortex_m::peripheral::{syst::SystClkSource, SYST};
use cortex_m_rt::exception;

use crate::{Gcr, HalError};

const SYSTICK_RELOAD_VAL: u32 = 0xffffff;

/// Represents an instant in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant {
    time_since_boot: Duration,
}

impl Instant {
    /// Get an instant for the current time.
    pub fn now() -> Instant {
        let (current_tick, wrap_count) = interrupt::free(|token| {
            let mut systick_ref = SYSTICK.borrow(token).borrow_mut();

            let systick = systick_ref.as_mut().expect("timer not initialized");

            // first read current tick
            let current_tick = SYST::get_current();

            if systick.has_wrapped() {
                // a wrap has occured, use tick count of 0 and new value for wrap count
                let wrap_count = WRAP_COUNT.fetch_add(1, Ordering::Relaxed) + 1;

                (current_tick, wrap_count)
            } else {
                // no wrap occured, current tick count and wrap count are accurate
                (current_tick, WRAP_COUNT.load(Ordering::Relaxed))
            }
        });

        // current tick subtracted from reaload val because it counts down
        let total_ticks = (wrap_count as u64 * SYSTICK_RELOAD_VAL as u64)
            + (SYSTICK_RELOAD_VAL - current_tick) as u64;

        let sysclock_freq = Gcr::with(|gcr| gcr.get_sysclock_frequency()) as u64;

        // calculate seconds and microseconds seperately to avoid
        // potential overflow when ticks are multiplied by 1_000_000
        let seconds = total_ticks / sysclock_freq;
        let remaining_ticks = total_ticks % sysclock_freq;

        let remaining_microseconds = (remaining_ticks * 1_000_000) / sysclock_freq;
        let total_microseconds = (seconds * 1_000_000) + remaining_microseconds;

        Instant {
            time_since_boot: Duration::from_micros(total_microseconds),
        }
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Instant {
        Instant {
            time_since_boot: self.time_since_boot + rhs,
        }
    }
}

/// Spins until the given duration has fully ellapsed.
pub fn sleep(duration: Duration) {
    let start = Instant::now();
    let end = start + duration;

    while Instant::now() < end {}
}

/// Runs the function, returns the result or none if it took too long
pub fn timeout<T>(f: impl FnOnce() -> T, timeout_len: Duration) -> Result<T, HalError> {
    let start = Instant::now();
    let result = f();
    let end = Instant::now();

    if start + timeout_len < end {
        Err(HalError::Timeout)
    } else {
        Ok(result)
    }
}

/// Initializes the timer
pub(crate) fn init(mut systick: SYST) {
    interrupt::free(|token| {
        systick.set_reload(SYSTICK_RELOAD_VAL);
        systick.clear_current();
        systick.enable_counter();
        systick.set_clock_source(SystClkSource::Core);
        systick.enable_interrupt();

        let mut systick_ref = SYSTICK.borrow(token).borrow_mut();
        *systick_ref = Some(systick);
    })
}

static SYSTICK: Mutex<RefCell<Option<SYST>>> = Mutex::new(RefCell::new(None));
static WRAP_COUNT: AtomicU32 = AtomicU32::new(0);

#[exception]
fn SysTick() {
    interrupt::free(|token| {
        let mut systick_ref = SYSTICK.borrow(token).borrow_mut();

        let systick = systick_ref.as_mut().expect("timer not initialized");

        if systick.has_wrapped() {
            WRAP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    })
}
