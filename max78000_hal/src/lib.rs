#![no_std]

mod committed_array;
pub mod flash;
pub mod gcr;
pub mod gpio;
pub mod i2c;
pub mod led;
pub mod timer;
pub mod trng;
pub mod uart;
pub mod prelude;

use thiserror_no_std::Error;

pub use flash::Flash;
pub use gcr::Gcr;
pub use gpio::Gpio;
pub use i2c::{UninitializedI2c, MasterI2c, ClientI2c};
pub use trng::Trng;
pub use uart::Uart;

// frequency of various clocks on the board
const ISO_FREQUENCY: u32 = 60000000;
const INRO_FREQUENCY: u32 = 30000;
const IPO_FREQUENCY: u32 = 100000000;
const IBRO_FREQUENCY: u32 = 7372800;
const ERTCO_FREQUENCY: u32 = 32768;

// NOTE: not correct, this varies, this is just default value msdk uses
const EXTCLK_FREQUECNY: u32 = 75000000;

#[derive(Debug, Error)]
pub enum HalError {
    #[error("Error writing to flash")]
    FlashError,
    #[error("Error with i2c connection")]
    I2cConnectionError,
    #[error("Error: timeout occured")]
    Timeout,
    #[error("Error with committed array: {0}")]
    CommittedArrayError(#[from] committed_array::CommittedArrayError),
}

pub struct Peripherals {
    pub flash: Flash,
    pub i2c: UninitializedI2c,
    pub trng: Trng,
}

impl Peripherals {
    pub fn take() -> Option<Peripherals> {
        let cortex_m::peripheral::Peripherals {
            SYST,
            ..
        } = cortex_m::peripheral::Peripherals::take()?;

        let max78000_device::Peripherals {
            FLC,
            GCR,
            I2C1,
            LPGCR,
            GPIO0,
            GPIO2,
            UART,
            TRNG,
            ..
        } = max78000_device::Peripherals::take()?;

        Gcr::init(GCR, LPGCR);
        Gpio::init(GPIO0, GPIO2);
        Uart::init(UART);

        timer::init(SYST);
        led::init();

        Some(Peripherals {
            flash: Flash::new(FLC),
            i2c: UninitializedI2c::new(I2C1),
            trng: Trng::new(TRNG),
        })
    }
}

/// Aligns `addr` up to the power 2 alignment `align`
/// `align` must be a power of 2
pub const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Aligns `addr` down to the power 2 alignment `align`
/// `align` must be a power of 2
pub const fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}
