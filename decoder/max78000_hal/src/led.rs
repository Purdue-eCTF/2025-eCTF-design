use crate::gpio::{Gpio, ConfigureIoOptions, GpioPadConfig, GpioPinFunction, GpioPinVoltage, GpioType};

/// GPIO configurations for each color of led
const LED_GPIO_PINS: [ConfigureIoOptions; 3] = [
    // red
    ConfigureIoOptions {
        gpio_type: GpioType::Gpio2,
        pin_mask: 0b1,
        function: GpioPinFunction::Output,
        pad: GpioPadConfig::None,
        voltage: GpioPinVoltage::Vddioh,
    },
    // green
    ConfigureIoOptions {
        gpio_type: GpioType::Gpio2,
        pin_mask: 0b10,
        function: GpioPinFunction::Output,
        pad: GpioPadConfig::None,
        voltage: GpioPinVoltage::Vddioh,
    },
    // blue
    ConfigureIoOptions {
        gpio_type: GpioType::Gpio2,
        pin_mask: 0b100,
        function: GpioPinFunction::Output,
        pad: GpioPadConfig::None,
        voltage: GpioPinVoltage::Vddioh,
    },
];

/// Represents a certain color of led.
#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum Led {
    Red,
    Green,
    Blue,
}

impl Led {
    /// Converts integer index to specified led.
    /// 
    /// Mainly used for functions that c code calls.
    pub fn from_index(index: u32) -> Option<Led> {
        match index {
            0 => Some(Led::Red),
            1 => Some(Led::Blue),
            2 => Some(Led::Green),
            _ => None,
        }
    }
}

/// Turns on the given led.
pub fn led_on(led: Led) {
    let config = LED_GPIO_PINS[led as usize];
    Gpio::with(|gpio| {
        gpio.output_clear(config.gpio_type, config.pin_mask);
    });
}

/// Turns off the given led.
pub fn led_off(led: Led) {
    let config = LED_GPIO_PINS[led as usize];
    Gpio::with(|gpio| {
        gpio.output_set(config.gpio_type, config.pin_mask);
    });
}

/// Toggles the given led.
pub fn led_toggle(led: Led) {
    let config = LED_GPIO_PINS[led as usize];
    Gpio::with(|gpio| {
        gpio.output_toggle(config.gpio_type, config.pin_mask);
    });
}

/// Initializes led gpio pins.
pub(crate) fn init() {
    Gpio::with(|gpio| {
        for config in LED_GPIO_PINS {
            gpio.configure_io(config);
            gpio.output_set(config.gpio_type, config.pin_mask);
        }
    });
}
