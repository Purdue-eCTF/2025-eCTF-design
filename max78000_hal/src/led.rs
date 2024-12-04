use crate::gpio::{Gpio, ConfigureIoOptions, GpioPadConfig, GpioPinFunction, GpioPinVoltage, GpioType};

const LED_GPIO_PINS: [ConfigureIoOptions; 3] = [
    ConfigureIoOptions {
        gpio_type: GpioType::Gpio2,
        pin_mask: 0b1,
        function: GpioPinFunction::Output,
        pad: GpioPadConfig::None,
        voltage: GpioPinVoltage::Vddioh,
    },
    ConfigureIoOptions {
        gpio_type: GpioType::Gpio2,
        pin_mask: 0b10,
        function: GpioPinFunction::Output,
        pad: GpioPadConfig::None,
        voltage: GpioPinVoltage::Vddioh,
    },
    ConfigureIoOptions {
        gpio_type: GpioType::Gpio2,
        pin_mask: 0b100,
        function: GpioPinFunction::Output,
        pad: GpioPadConfig::None,
        voltage: GpioPinVoltage::Vddioh,
    },
];


#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum Led {
    Red,
    Green,
    Blue,
}

impl Led {
    pub fn from_index(index: u32) -> Option<Led> {
        match index {
            0 => Some(Led::Red),
            1 => Some(Led::Blue),
            2 => Some(Led::Green),
            _ => None,
        }
    }
}

pub fn led_on(led: Led) {
    let config = LED_GPIO_PINS[led as usize];
    Gpio::with(|gpio| {
        gpio.output_clear(config.gpio_type, config.pin_mask);
    });
}

pub fn led_off(led: Led) {
    let config = LED_GPIO_PINS[led as usize];
    Gpio::with(|gpio| {
        gpio.output_set(config.gpio_type, config.pin_mask);
    });
}

pub fn led_toggle(led: Led) {
    let config = LED_GPIO_PINS[led as usize];
    Gpio::with(|gpio| {
        gpio.output_toggle(config.gpio_type, config.pin_mask);
    });
}

pub(crate) fn init() {
    Gpio::with(|gpio| {
        for config in LED_GPIO_PINS {
            gpio.configure_io(config);
            gpio.output_set(config.gpio_type, config.pin_mask);
        }
    });
}
