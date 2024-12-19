use core::fmt::{self, Write};
use core::cmp::min;
use core::str;

use once_cell::sync::OnceCell;
use max78000_device::UART;
use crate::IBRO_FREQUENCY;
use crate::gcr::Gcr;
use crate::gpio::{ConfigureIoOptions, Gpio, GpioPadConfig, GpioPinFunction, GpioPinVoltage, GpioType};

const MAX_CLOCK_DIVISOR: u32 = (1 << 20) - 1;

static UART: OnceCell<Uart> = OnceCell::new();

/// Gets a reference to the uart.
pub fn uart() -> &'static Uart {
    UART.get().expect("uart not yet initialized")
}

#[derive(Debug)]
pub struct Uart {
    regs: UART,
}

impl Uart {
    pub(crate) fn init(uart: UART) {
        let mut uart = Uart { regs: uart };
        uart.setup_uart();
        UART.set(uart).expect("could not set uart global");
    }

    fn setup_uart(&mut self) {
        Gcr::with(|gcr| {
            // disable first, this is what msdk does
            gcr.reset_uart0();
            gcr.set_uart0_clock_enabled(false);

            // now enable everything
            gcr.enable_ibro_clock();
        });

        Gpio::with(|gpio| {
            gpio.configure_io(ConfigureIoOptions {
                gpio_type: GpioType::Gpio0,
                pin_mask: 0b11,
                function: GpioPinFunction::Alternate1,
                pad: GpioPadConfig::None,
                voltage: GpioPinVoltage::Vddio,
            });
        });

        Gcr::with(|gcr| {
            gcr.set_uart0_clock_enabled(true);
        });

        self.regs.ctrl().modify(|_, ctrl| {
            // this bit is required to be set
            //ctrl.ucagm().set_bit();

            // turn of error checking parity bit
            ctrl.par_en().clear_bit();

            // 1 stop bit
            ctrl.stopbits().clear_bit();

            // 8 bit character size
            ctrl.char_size()._8bits();

            // safety: a value of 1 is allowed
            // 1 bit receive threshold before generating interrupt
            unsafe { ctrl.rx_thd_val().bits(1) }
        });

        self.set_frequency(115200);
    }

    fn set_frequency(&mut self, buad_rate: u32) {
        self.regs.osr().write(|osr| {
            // safety: 5 is a valid setting for the osr
            unsafe { osr.bits(5) }
        });

        self.regs.ctrl().modify(|_, ctrl| {
            // select internal buad rate clock
            ctrl.bclksrc().clk2()
        });

        let mut clock_divide = IBRO_FREQUENCY / buad_rate;
        let clock_mod = IBRO_FREQUENCY % buad_rate;

        if clock_divide == 0 || clock_mod > (buad_rate / 2) {
            clock_divide += 1;
        }

        clock_divide = min(clock_divide, MAX_CLOCK_DIVISOR);

        self.regs.clkdiv().write(|clkdiv| unsafe { clkdiv.bits(clock_divide) });

        // enable boad clock and wait for it to be ready
        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.bclken().set_bit()
        });

        while self.regs.ctrl().read().bclken().bit_is_clear() {}
    }

    fn is_transmit_full(&self) -> bool {
        self.regs.status().read().tx_full().bit()
    }

    fn is_receive_empty(&self) -> bool {
        self.regs.status().read().rx_em().bit()
    }

    pub fn write_byte(&self, byte: u8) {
        while self.is_transmit_full() {}

        self.regs.fifo().write(|fifo| {
            // safety: writing 1 bytes to the data register is intended usage of uart
            unsafe {
                fifo.data().bits(byte)
            }
        });
    }

    pub fn read_byte(&self) -> u8 {
        while self.is_receive_empty() {}

        self.regs.fifo().read().data().bits()
    }

    /// Reads in bytes to the buffer, and returns a slice to the section which was successfully read
    // behavior of echoing bytes back and crlf emulates msdk's behavior with _read and _write
    pub fn read_bytes<'a>(&self, buffer: &'a mut [u8]) -> &'a [u8] {
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte = self.read_byte();
            self.write_byte(*byte);

            if *byte == b'\r' {
                *byte = b'\n';
                return &buffer[..(i + 1)];
            }
        }

        buffer
    }

    pub fn write_bytes(&self, buffer: &[u8]) {
        for byte in buffer {
            if *byte == b'\n' {
                self.write_byte(b'\r');
            }

            self.write_byte(*byte);
        }
    }

    pub fn flush_uart_receive(&self) {
        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.rx_flush().set_bit()
        });

        while !self.is_receive_empty() {}
    }

    fn flush_until_newline(&self) {
        let mut buf = [0];
        while self.read_bytes(&mut buf)[0] != b'\n' {}
    }

    /// Reads a string from uart until newline encountered
    pub fn recv_input<'a>(&self, buf: &'a mut [u8]) -> Option<&'a str> {
        let input = self.read_bytes(buf);

        // panic safety: read reads in at least 1 byte
        if *input.last().unwrap() != b'\n' {
            self.flush_until_newline();
        }

        str::from_utf8(input).ok()
    }
}

// new type required for write because write requires mutable reference
struct UartWriter;

impl Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        uart().write_bytes(s.as_bytes());

        Ok(())
    }
}

// The uart uses just atomic reads and writes to addresses, so no unsafety will be caused
// The only issue is it may be possible for characters to be skipped printing
// (if for example the is txfifo not full returns true but then someone else fills it)
unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

#[doc(hidden)]
pub fn _uprint(args: fmt::Arguments) {
    UartWriter.write_fmt(args).unwrap();
}

/// Prints to the uart port
#[macro_export]
macro_rules! uprint {
    ($($arg:tt)*) => ($crate::uart::_uprint(format_args!($($arg)*)));
}

/// Prints to the uart port
#[macro_export]
macro_rules! uprintln {
    () => ($crate::uprint!("\n"));
    ($($arg:tt)*) => ($crate::uprint!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! uprint_debug {
    ($($arg:tt)*) => {{
        $crate::uprint!("%debug: {}%", format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! uprintln_debug {
    ($($arg:tt)*) => ($crate::uprint_debug!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! uprint_info {
    ($($arg:tt)*) => {{
        $crate::uprint!("%info: {}%", format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! uprintln_info {
    ($($arg:tt)*) => ($crate::uprint_info!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! uprint_success {
    ($($arg:tt)*) => {{
        $crate::uprint!("%success: {}%", format_args!($($arg)*));
        $crate::uart::uart().flush_uart_receive();
    }};
}

#[macro_export]
macro_rules! uprintln_success {
    ($($arg:tt)*) => ($crate::uprint_success!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! uprint_error {
    ($($arg:tt)*) => {{
        $crate::uprint!("%error: {}%", format_args!($($arg)*));
        $crate::uart::uart().flush_uart_receive();
    }};
}

#[macro_export]
macro_rules! uprintln_error {
    ($($arg:tt)*) => ($crate::uprint_error!("{}\n", format_args!($($arg)*)));
}
