use core::cell::RefCell;

use cortex_m::interrupt::{self, Mutex};
use max78000_device::{GPIO0, GPIO2};

use crate::gcr::Gcr;

/// Specifies which GPIO controller to use.
#[derive(Debug, Clone, Copy)]
pub enum GpioType {
    // GPIO0
    Gpio0,
    // GPIO2
    Gpio2,
}

/// Specifies function of the gpio pin.
#[derive(Debug, Clone, Copy)]
pub enum GpioPinFunction {
    /// Gpio pin is used for input.
    Input,
    /// Gpio pin is used for output.
    Output,
    /// Gpio pin is used for alternative function and used by some other device.
    Alternate1,
}

/// Controls voltage source for gpio pins.
#[derive(Debug, Clone, Copy)]
pub enum GpioPinVoltage {
    /// Regular voltage.
    Vddio,
    /// High voltage.
    Vddioh,
}

/// Controls the configuration of pullup and pulldown resisters and such for gpio pins.
#[derive(Debug, Clone, Copy)]
pub enum GpioPadConfig {
    /// No resisters.
    None,
    /// Pullup resister will be used.
    PullUp,
}

/// Specifies configuration options for a GPIO pins.
#[derive(Debug, Clone, Copy)]
pub struct ConfigureIoOptions {
    pub gpio_type: GpioType,
    /// pin mask is a bitmask of pins, where each set bit indicates a pin to be configured.
    pub pin_mask: u32,
    pub function: GpioPinFunction,
    pub pad: GpioPadConfig,
    pub voltage: GpioPinVoltage,
}

static GPIO: Mutex<RefCell<Option<Gpio>>> = Mutex::new(RefCell::new(None));

/// Used for interacting with gpio pin controllers.
pub struct Gpio {
    gpio0: GPIO0,
    gpio2: GPIO2,
}

impl Gpio {
    /// Create a new GPIO instance.
    fn new(gpio0: GPIO0, gpio2: GPIO2) -> Self {
        Gcr::with(|gcr| {
            gcr.set_gpio0_clock_enabled(true);
            gcr.set_gpio2_clock_enabled(true);
        });

        Gpio {
            gpio0,
            gpio2,
        }
    }

    /// Initializes the global `Gpio` instance.
    pub fn init(gpio0: GPIO0, gpio2: GPIO2) {
        interrupt::free(|token| {
            let mut gpio = GPIO.borrow(token).borrow_mut();
            assert!(gpio.is_none(), "gpio already initialized");

            *gpio = Some(Self::new(gpio0, gpio2));
        })
    }

    /// Get mutable access to the Gpio instance and call the given closure with it.
    pub fn with(f: impl FnOnce(&mut Gpio)) {
        interrupt::free(|token| {
            let mut gpio = GPIO.borrow(token).borrow_mut();
            f(gpio.as_mut().expect("gpio not initialized"));
        })
    }
}

macro_rules! make_configure_io {
    ($regs:expr, $options:expr) => {{
        $regs.inen().modify(|val, inen| {
            // safety: any bits in the inem register can be set
            unsafe { inen.bits(val.bits() | $options.pin_mask) }
        });

        $regs.en0_set().write(|en0| {
            // safety: any bits in the en0_set register can be set
            unsafe { en0.bits($options.pin_mask) }
        });

        // only 2 of the functions we need are supported
        match $options.function {
            GpioPinFunction::Input => {
                $regs.outen_clr().write(|outen_clr| {
                    // safety: any bits in the outen_clr register can be set
                    unsafe { outen_clr.bits($options.pin_mask) }
                });

                $regs.en0_set().write(|en0_clr| {
                    // safety: any bits in the en0_set register can be set
                    unsafe { en0_clr.bits($options.pin_mask) }
                });

                $regs.en1_clr().write(|en1_clr| {
                    // safety: any bits in the en1_clr register can be set
                    unsafe { en1_clr.bits($options.pin_mask) }
                });

                $regs.en2_clr().write(|en2_clr| {
                    // safety: any bits in the en2_clr register can be set
                    unsafe { en2_clr.bits($options.pin_mask) }
                });
            },
            GpioPinFunction::Output => {
                $regs.outen_set().write(|outen_set| {
                    // safety: any bits in the outen_set register can be set
                    unsafe { outen_set.bits($options.pin_mask) }
                });

                $regs.en0_set().write(|en0_clr| {
                    // safety: any bits in the en0_set register can be set
                    unsafe { en0_clr.bits($options.pin_mask) }
                });

                $regs.en1_clr().write(|en1_clr| {
                    // safety: any bits in the en1_clr register can be set
                    unsafe { en1_clr.bits($options.pin_mask) }
                });

                $regs.en2_clr().write(|en2_clr| {
                    // safety: any bits in the en2_clr register can be set
                    unsafe { en2_clr.bits($options.pin_mask) }
                });
            }
            GpioPinFunction::Alternate1 => {
                $regs.en2_clr().write(|en2_clr| {
                    // safety: any bits in the en2_clr register can be set
                    unsafe { en2_clr.bits($options.pin_mask) }
                });
        
                $regs.en1_clr().write(|en1_clr| {
                    // safety: any bits in the en1_clr register can be set
                    unsafe { en1_clr.bits($options.pin_mask) }
                });

                $regs.en0_clr().write(|en0_clr| {
                    // safety: any bits in the en0_clr register can be set
                    unsafe { en0_clr.bits($options.pin_mask) }
                });
            },
        }

        // only 2 of the pad modes supported, theo only ones we need
        match $options.pad {
            GpioPadConfig::None => {
                $regs.padctrl0().modify(|val, padctrl| {
                    // safety: any bits in the padctrl0 register can be set
                    unsafe { padctrl.bits(val.bits() & !$options.pin_mask) }
                });
        
                $regs.padctrl1().modify(|val, padctrl| {
                    // safety: any bits in the padctrl1 register can be set
                    unsafe { padctrl.bits(val.bits() & !$options.pin_mask) }
                });
            },
            GpioPadConfig::PullUp => {
                $regs.padctrl0().modify(|val, padctrl| {
                    // safety: any bits in the padctrl0 register can be set
                    unsafe { padctrl.bits(val.bits() | $options.pin_mask) }
                });
        
                $regs.padctrl1().modify(|val, padctrl| {
                    // safety: any bits in the padctrl1 register can be set
                    unsafe { padctrl.bits(val.bits() & !$options.pin_mask) }
                });

                $regs.ps().modify(|val, ps| {
                    // safety: any bits in the ps register can be set
                    unsafe { ps.bits(val.bits() | $options.pin_mask) }
                });
            },
        }

        $regs.vssel().modify(|val, vssel| {
            let new_bits = match $options.voltage {
                // unset pin bits for vddio
                GpioPinVoltage::Vddio => val.bits() & !$options.pin_mask,
                // set pin bits for vddioh
                GpioPinVoltage::Vddioh => val.bits() | $options.pin_mask,
            };

            // safety: any bits in vssel register can be set
            unsafe { vssel.bits(new_bits) }
        });
    }};
}

impl Gpio {
    /// Configure pins based on the given configuration options.
    pub fn configure_io(&mut self, options: ConfigureIoOptions) {
        match options.gpio_type {
            GpioType::Gpio0 => make_configure_io!(self.gpio0, options),
            GpioType::Gpio2 => make_configure_io!(self.gpio2, options),
        }
    }

    /// Sets the output of all pins in the bitmask given by `pins` on the given controller `gpio_type` to high.
    pub fn output_set(&mut self, gpio_type: GpioType, pins: u32) {
        match gpio_type {
            GpioType::Gpio0 => {
                self.gpio0.out_set().write(|out_set| {
                    // safety: any bits in the out_set register can be set
                    unsafe { out_set.bits(pins) }
                })
            },
            GpioType::Gpio2 => {
                self.gpio2.out_set().write(|out_set| {
                    // safety: any bits in the out_set register can be set
                    unsafe { out_set.bits(pins) }
                })
            },
        }
    }

    /// Sets the output of all pins in the bitmask given by `pins` on the given controller `gpio_type` to low.
    pub fn output_clear(&mut self, gpio_type: GpioType, pins: u32) {
        match gpio_type {
            GpioType::Gpio0 => {
                self.gpio0.out_clr().write(|out_clr| {
                    // safety: any bits in the out_clr register can be set
                    unsafe { out_clr.bits(pins) }
                })
            },
            GpioType::Gpio2 => {
                self.gpio2.out_clr().write(|out_clr| {
                    // safety: any bits in the out_clr register can be set
                    unsafe { out_clr.bits(pins) }
                })
            },
        }
    }

    /// Toggles the output of all pins in the bitmask given by `pins` on the given controller `gpio_type`.
    pub fn output_toggle(&mut self, gpio_type: GpioType, pins: u32) {
        match gpio_type {
            GpioType::Gpio0 => {
                self.gpio0.out().modify(|val, out| {
                    // safety: any bits in the out register can be set
                    unsafe { out.bits(val.bits() ^ pins) }
                });
            },
            GpioType::Gpio2 => {
                self.gpio2.out().modify(|val, out| {
                    // safety: any bits in the out register can be set
                    unsafe { out.bits(val.bits() ^ pins) }
                });
            },
        }
    }
}

impl Drop for Gpio {
    fn drop(&mut self) {
        Gcr::with(|gcr| gcr.set_gpio0_clock_enabled(false));
    }
}
