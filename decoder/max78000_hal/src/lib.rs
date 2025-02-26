#![no_std]

pub mod flash;
pub mod gcr;
pub mod gpio;
pub mod led;
pub mod mpu;
pub mod prelude;
pub mod timer;
pub mod uart;

use mpu::Mpu;
use thiserror_no_std::Error;

pub use flash::Flash;
pub use gcr::Gcr;
pub use gpio::Gpio;
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
}

/// Contains various peripheralls of the max78000 device.
pub struct Peripherals {
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
            LPGCR,
            GPIO0,
            GPIO2,
            UART,
            ..
        } = max78000_device::Peripherals::take()?;

        Gcr::init(GCR, LPGCR);
        Gpio::init(GPIO0, GPIO2);
        Uart::init(UART);
        Flash::init(FLC);

        timer::init(SYST);
        led::init();

        Some(Peripherals {
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
