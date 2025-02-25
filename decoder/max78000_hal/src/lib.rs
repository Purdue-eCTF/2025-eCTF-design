#![no_std]

mod committed_array;
pub mod flash;
pub mod gcr;
pub mod gpio;
pub mod i2c;
pub mod led;
pub mod mpu;
pub mod prelude;
pub mod timer;
pub mod trng;
pub mod uart;

use mpu::Mpu;
use thiserror_no_std::Error;

pub use flash::Flash;
pub use gcr::Gcr;
pub use gpio::Gpio;
pub use i2c::{ClientI2c, MasterI2c, UninitializedI2c};
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

/// Contains various peripheralls of the max78000 device.
pub struct Peripherals {
    pub i2c: UninitializedI2c,
    pub trng: Trng,
    pub mpu: Mpu,
}

impl Peripherals {
    /// Initializes all peripherals and returns them.
    pub fn take() -> Option<Peripherals> {
        let cortex_m::peripheral::Peripherals { SYST, MPU, .. } =
            cortex_m::peripheral::Peripherals::take()?;

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
        Flash::init(FLC);

        timer::init(SYST);
        led::init();

        Some(Peripherals {
            i2c: UninitializedI2c::new(I2C1),
            trng: Trng::new(TRNG),
            mpu: Mpu::new(MPU),
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
