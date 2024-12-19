use core::cell::RefCell;

use cortex_m::interrupt::{Mutex, self};
use max78000_device::{gcr::clkctrl::SYSCLK_SEL_A, GCR, LPGCR};

use crate::{ERTCO_FREQUENCY, EXTCLK_FREQUECNY, IBRO_FREQUENCY, INRO_FREQUENCY, IPO_FREQUENCY, ISO_FREQUENCY};

/// Stores gcr used by all peripherals
static GCR: Mutex<RefCell<Option<Gcr>>> = Mutex::new(RefCell::new(None));

/// Global configuration registers.
/// 
/// Used for controlling certain global features of the device and enabling other peripherals and such.
pub struct Gcr {
    regs: GCR,
    low_power_regs: LPGCR,
}

impl Gcr {
    /// Executes the given closure and gives it exclusive access to the gcr
    /// 
    /// # Panics
    /// 
    /// panics if the gcr is not initialized
    pub fn with<T>(f: impl FnOnce(&mut Gcr) -> T) -> T {
        interrupt::free(|token| {
            let mut gcr = GCR.borrow(token).borrow_mut();
            f(gcr.as_mut().expect("gcr not initialized"))
        })
    }

    /// Initialize the gcr
    /// 
    /// # Panics
    /// 
    /// panics if the gcr is already initialized
    pub fn init(gcr: GCR, lpgcr: LPGCR) {
        interrupt::free(|token| {
            let mut global_gcr = GCR.borrow(token).borrow_mut();
            assert!(global_gcr.is_none(), "gcr already initialized");

            *global_gcr = Some(Gcr {
                regs: gcr,
                low_power_regs: lpgcr,
            });
        })
    }

    /// Gets the frequency of system clock in ticks per second.
    pub fn get_sysclock_frequency(&self) -> u32 {
        let clock_source = self.regs.clkctrl().read().sysclk_sel().variant()
            .expect("invalid system clock selected");

        // TODO: maybe specify certain clock to use
        let frequency = match clock_source {
            SYSCLK_SEL_A::ISO => ISO_FREQUENCY,
            SYSCLK_SEL_A::INRO => INRO_FREQUENCY,
            SYSCLK_SEL_A::IPO => IPO_FREQUENCY,
            SYSCLK_SEL_A::IBRO => IBRO_FREQUENCY,
            SYSCLK_SEL_A::ERTCO => ERTCO_FREQUENCY,
            SYSCLK_SEL_A::EXTCLK => EXTCLK_FREQUECNY,
        };

        let clock_divide = self.regs.clkctrl().read().sysclk_div().bits();

        frequency >> clock_divide
    }

    /// Gets the frequency of the clock used for many peripherals in ticks per second.
    pub fn get_peripheral_clock_frequency(&self) -> u32 {
        self.get_sysclock_frequency() / 2
    }

    pub fn reset_uart0(&mut self) {
        self.regs.rst0().write(|rst0| rst0.uart0().set_bit());

        while self.regs.rst0().read().uart0().bit() {}
    }

    pub fn reset_i2c1(&mut self) {
        self.regs.rst1().write(|rst1| rst1.i2c1().set_bit());

        while self.regs.rst1().read().i2c1().bit() {}
    }

    /// Enables the IBRO (Internal Baurd Rate Oscillator) clock.
    /// 
    /// This clock is used by the uart for timing purposes.
    pub fn enable_ibro_clock(&self) {
        self.regs.clkctrl().modify(|_, clckctrl| {
            // this might not be necessary
            // msdk enables ibro, but manual says it is always enabled
            clckctrl.ibro_en().set_bit()
        });

        // wait for ibro to be ready
        while self.regs.clkctrl().read().ibro_rdy().bit_is_clear() {}
    }

    pub fn set_uart0_clock_enabled(&mut self, enabled: bool) {
        self.regs.pclkdis0().modify(|_, clock| {
            clock.uart0().bit(!enabled)
        });
    }

    pub fn set_i2c1_clock_enabled(&mut self, enabled: bool) {
        self.regs.pclkdis0().modify(|_, clock| {
            clock.i2c1().bit(!enabled)
        });
    }

    pub fn set_gpio0_clock_enabled(&mut self, enabled: bool) {
        self.regs.pclkdis0().modify(|_, clock| {
            clock.gpio0().bit(!enabled)
        });
    }

    pub fn set_gpio2_clock_enabled(&mut self, enabled: bool) {
        self.low_power_regs.pclkdis().modify(|_, clock| {
            clock.gpio2().bit(!enabled)
        });
    }

    pub fn set_trng_clock_enabled(&mut self, enabled: bool) {
        self.regs.pclkdis1().modify(|_, clock| {
            clock.trng().bit(!enabled)
        });
    }

    /// Flushes the instruction cache, and perhaps some other caches.
    pub fn flush_cache(&mut self) {
        self.regs.sysctrl().modify(|_, sysctrl| {
            sysctrl.icc0_flush().set_bit()
        });

        while self.regs.sysctrl().read().icc0_flush().bit_is_set() {}
    }
}
