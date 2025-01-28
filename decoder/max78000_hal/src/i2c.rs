use core::cell::RefCell;
use core::marker::PhantomData;
use core::slice;

use cortex_m::interrupt::{free as interrupt_free, Mutex};
use cortex_m::peripheral::NVIC;
use max78000_device::{interrupt, Interrupt, I2C1};

use crate::committed_array::{CommittedArray, CommittedArrayError};
use crate::gcr::Gcr;
use crate::gpio::{
    ConfigureIoOptions, Gpio, GpioPadConfig, GpioPinFunction, GpioPinVoltage, GpioType,
};
use crate::HalError;

pub const MAX_I2C_MESSAGE_LEN: usize = 256;

const I2C_FASTPLUS_SPEED: u32 = 1000000;
const RX_FIFO_LEN: u8 = 8;
const TX_FIFO_LEN: u8 = 8;

const INTFL0_MASK: u32 = 0x00ffffff;
const INTFL1_MASK: u32 = 0x00000007;
// error mask for interrupt flags0
const ERROR_MASK: u32 = 0x7f00;

const RX_THD: u32 = 1 << 4;
const TX_THD: u32 = 1 << 5;
const TX_LOCKOUT: u32 = 1 << 15;
const RD_ADDR_MATCH: u32 = 1 << 22;
const WR_ADDR_MATCH: u32 = 1 << 23;

pub type I2cAddr = u8;

struct I2cInner {
    regs: I2C1,
}

impl I2cInner {
    fn clear_rx_fifo(&self) {
        self.regs
            .rxctrl0()
            .modify(|_, rxctrl0| rxctrl0.flush().set_bit());

        while self.regs.rxctrl0().read().flush().bit_is_set() {
            //core::hint::spin_loop();
        }
    }

    fn clear_tx_fifo(&self) {
        self.regs
            .txctrl0()
            .modify(|_, rxctrl0| rxctrl0.flush().set_bit());

        while self.regs.txctrl0().read().flush().bit_is_set() {
            //core::hint::spin_loop();
        }
    }

    fn set_rx_threshold(&self, n: u8) {
        assert!(n <= RX_FIFO_LEN);

        self.regs
            .rxctrl0()
            .modify(|_, rxctrl0| rxctrl0.thd_lvl().variant(n));
    }

    fn set_tx_threshold(&self, n: u8) {
        assert!(n <= TX_FIFO_LEN);

        self.regs
            .txctrl0()
            .modify(|_, txctrl0| txctrl0.thd_val().variant(n));
    }

    fn set_rx_threshold_int_enabled(&self, enabled: bool) {
        self.regs
            .inten0()
            .modify(|_, inten0| inten0.rx_thd().bit(enabled));
    }

    fn set_tx_threshold_int_enabled(&self, enabled: bool) {
        self.regs
            .inten0()
            .modify(|_, inten0| inten0.tx_thd().bit(enabled));
    }

    fn is_rx_fifo_empty(&self) -> bool {
        self.regs.status().read().rx_em().bit_is_set()
    }

    fn is_tx_fifo_full(&self) -> bool {
        self.regs.status().read().tx_full().bit_is_set()
    }

    fn read_rx_fifo(&self, buf: &mut [u8]) -> usize {
        let mut i = 0;

        while i < buf.len() && !self.is_rx_fifo_empty() {
            buf[i] = self.regs.fifo().read().data().bits();
            i += 1;
        }

        i
    }

    /// Writes the data into the txfifo
    ///
    /// # Returns
    ///
    /// Returns the number of bytes written
    fn write_tx_fifo(&self, data: &[u8]) -> usize {
        for (i, byte) in data.iter().enumerate() {
            if self.is_tx_fifo_full() {
                return i;
            }

            self.regs.fifo().write(|fifo| {
                // safety: writing any bit into fifo is ok
                unsafe { fifo.data().bits(*byte) }
            });
        }

        data.len()
    }

    fn clear_flags(&self, flags0: u32, flags1: u32) {
        self.regs.intfl0().write(|intfl0| {
            // safety: it is ok to write any bytes to flags 0
            unsafe { intfl0.bits(flags0) }
        });

        self.regs.intfl1().write(|intfl1| {
            // safety: it is ok to write any bytes to flags 1
            unsafe { intfl1.bits(flags1) }
        });
    }

    fn clear_tx_lockout(&self) {
        // TODO: determine if this check is even need, probably can be removed
        if self.regs.intfl0().read().tx_lockout().bit_is_set() {
            self.clear_flags(TX_LOCKOUT, 0);
        }
    }

    fn has_error(&self) -> bool {
        self.regs.intfl0().read().bits() & ERROR_MASK != 0
    }

    fn set_frequency(&self, hz: u32) {
        if hz > I2C_FASTPLUS_SPEED {
            unimplemented!("i2c fastplus speed not implemented");
        }

        let peripheral_clock = Gcr::with(|gcr| gcr.get_peripheral_clock_frequency());

        let ticks_total = peripheral_clock / hz;
        let ticks_per_hi_low = (ticks_total >> 1) - 1;

        if ticks_per_hi_low > 0x1ff || ticks_per_hi_low == 0 {
            panic!("invalid clock speed");
        }

        self.regs
            .clkhi()
            .write(|clkhi| clkhi.hi().variant(ticks_per_hi_low as u16));

        self.regs
            .clklo()
            .write(|clklo| clklo.lo().variant(ticks_per_hi_low as u16));
    }
}

pub struct UninitializedI2c(I2cInner);

impl UninitializedI2c {
    pub(crate) fn new(regs: I2C1) -> Self {
        UninitializedI2c(I2cInner { regs })
    }

    fn init_common(&self) {
        Gcr::with(|gcr| {
            // first shutdown everything, this is what msdk does
            gcr.set_i2c1_clock_enabled(false);
            gcr.reset_i2c1();

            gcr.set_i2c1_clock_enabled(true);
        });

        Gpio::with(|gpio| {
            gpio.configure_io(ConfigureIoOptions {
                gpio_type: GpioType::Gpio0,
                pin_mask: 1 << 16 | 1 << 17,
                function: GpioPinFunction::Alternate1,
                pad: GpioPadConfig::None,
                voltage: GpioPinVoltage::Vddio,
            });
        });

        self.0.clear_rx_fifo();
        self.0.clear_tx_fifo();
        self.0.set_rx_threshold(6);
        self.0.set_tx_threshold(2);

        // enable i2c controller
        self.0.regs.ctrl().modify(|_, ctrl| ctrl.en().set_bit());
    }

    pub fn init_master(self, frequency_hz: u32) -> MasterI2c {
        self.init_common();
        self.0
            .regs
            .ctrl()
            .modify(|_, ctrl| ctrl.mst_mode().set_bit());

        self.0.set_frequency(frequency_hz);

        MasterI2c(self.0)
    }

    pub fn init_client(self, frequency_hz: u32, address: I2cAddr) -> ClientI2c {
        self.init_common();

        // TODO: review this more, msdk does these operations but im not sure if they are right
        self.0.regs.slave0().write(|slave| {
            // safety: any bits in slave register can be set
            unsafe { slave.bits(0) }
        });

        if address > 0b1111111 {
            self.0.regs.slave0().write(|slave| {
                // set extended address
                unsafe { slave.bits(1) }
            });
        }

        self.0
            .regs
            .slave0()
            .modify(|_, slave| unsafe { slave.bits(address.into()) });

        self.0.set_frequency(frequency_hz);

        self.0.regs.inten0().write(|inten0| {
            inten0.wr_addr_match().set_bit();
            inten0.rd_addr_match().set_bit()
        });

        interrupt_free(|token| {
            let mut handler_state = HANDLER_STATE.borrow(token).borrow_mut();
            assert!(
                handler_state.is_none(),
                "i2c handler state already initialized"
            );

            *handler_state = Some(I2cHandlerState {
                i2c: self.0,
                i2c_mode: I2cTransactionMode::ReadyReceive,
                data_index: 0,
                data: [0; MAX_I2C_MESSAGE_LEN],
                data_len: 0,
            });
        });

        // safety: i2c1 interrupt has not yet ran, so it is not relying on any interrupt critical sections
        unsafe {
            NVIC::unmask(Interrupt::I2C1);
        }

        ClientI2c(PhantomData)
    }
}

pub struct MasterI2c(I2cInner);

impl MasterI2c {
    fn start(&self) {
        self.0
            .regs
            .mstctrl()
            .modify(|_, mstctrl| mstctrl.start().set_bit());
    }

    fn stop(&self) {
        self.0
            .regs
            .mstctrl()
            .modify(|_, mstctrl| mstctrl.stop().set_bit());

        while self.0.regs.mstctrl().read().stop().bit_is_set() {
            //core::hint::spin_loop();
        }
    }

    fn await_transaction_is_done(&self) {
        while self.0.regs.intfl0().read().done().bit_is_clear() {
            //core::hint::spin_loop();
        }
    }

    /// Writes bytes to the given i2c address
    ///
    /// # Returns
    ///
    /// Returns the number of bytes written
    pub fn send(&mut self, address: I2cAddr, data: &[u8]) -> Result<usize, HalError> {
        assert!(data.len() <= MAX_I2C_MESSAGE_LEN);
        if data.len() == 0 {
            return Ok(0);
        }

        // flags have to be cleared first, because if tx lockout flag is set,
        // clear tx fifo will be stuck in an infinite loop
        self.0.clear_flags(INTFL0_MASK, INTFL1_MASK);
        self.0.clear_tx_fifo();
        self.0.clear_rx_fifo();

        let mut write_len = 0;

        // write slave address with read bit cleared (write mode)
        self.0.write_tx_fifo(&[(address << 1) & !1]);

        // prefill fifo with data bytes
        write_len += self.0.write_tx_fifo(data);

        self.start();

        while write_len < data.len() {
            if self.0.regs.intfl0().read().tx_thd().bit_is_set() {
                write_len += self.0.write_tx_fifo(&data[write_len..]);

                self.0.clear_flags(TX_THD, 0);
            }

            if self.0.has_error() {
                self.stop();
                return Err(HalError::I2cConnectionError);
            }
        }

        self.stop();
        self.await_transaction_is_done();

        // this check is needed in case all bytes are prefilled, the other check in the loop will not run
        if self.0.has_error() {
            Err(HalError::I2cConnectionError)
        } else {
            Ok(write_len)
        }
    }

    /// Receives bytes from the given device with the given address.
    ///
    /// Recieves exactly `buffer.len()` bytes.
    pub fn recv(&mut self, address: I2cAddr, buffer: &mut [u8]) -> Result<(), HalError> {
        assert!(buffer.len() <= MAX_I2C_MESSAGE_LEN);
        if buffer.len() == 0 {
            return Ok(());
        }

        // flags have to be cleared first, because if tx lockout flag is set,
        // clear tx fifo will be stuck in an infinite loop
        self.0.clear_flags(INTFL0_MASK, INTFL1_MASK);
        self.0.clear_tx_fifo();
        self.0.clear_rx_fifo();

        let mut recv_len = 0;

        // set number of bytes to receive
        self.0.regs.rxctrl1().modify(|_, rxctrl1| {
            let write_len = if buffer.len() == MAX_I2C_MESSAGE_LEN {
                0
            } else {
                buffer.len() as u8
            };

            rxctrl1.cnt().variant(write_len)
        });

        // write slave address with read bit set
        self.0.write_tx_fifo(&[(address << 1) | 1]);

        self.start();

        while recv_len < buffer.len() {
            let flags = self.0.regs.intfl0().read();
            if flags.rx_thd().bit_is_set() || flags.done().bit_is_set() {
                recv_len += self.0.read_rx_fifo(&mut buffer[recv_len..]);

                self.0.clear_flags(RX_THD, 0);
            }

            if self.0.has_error() {
                self.stop();
                return Err(HalError::I2cConnectionError);
            }

            if flags.done().bit_is_set() {
                // TODO: support restart for larger transactions
                break;
            }
        }

        self.stop();
        self.await_transaction_is_done();

        if self.0.has_error() {
            Err(HalError::I2cConnectionError)
        } else {
            Ok(())
        }
    }
}

static CLIENT_SEND_BUFFER: CommittedArray = CommittedArray::new();
static CLIENT_RECEIVE_BUFFER: CommittedArray = CommittedArray::new();
pub struct ClientI2c(PhantomData<()>);

impl ClientI2c {
    pub fn send(&self, data: &[u8]) -> Result<(), HalError> {
        loop {
            match CLIENT_SEND_BUFFER.try_commit(data) {
                Ok(_) => return Ok(()),
                Err(CommittedArrayError::Busy) => (),
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub fn recv<'a>(&self, buf: &'a mut [u8]) -> Result<&'a [u8], HalError> {
        loop {
            match CLIENT_RECEIVE_BUFFER.try_take(buf) {
                Ok(data) => {
                    // get around limitation of borrow checker
                    let len = data.len();
                    return Ok(&buf[..len]);
                }
                Err(CommittedArrayError::Busy) => (),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cTransactionMode {
    /// The component is waiting to receive a command
    ReadyReceive,
    /// Component is currently receiving the length of the message it is to receive
    ReceivingLength,
    /// The component is currently receiving a command
    Receiving,
    /// The component is waiting for a command to finish processing
    ///
    /// Ap should repeatedly pull the length until it is non-zero
    WaitSend,
    /// The component is ready to send a response
    ReadySend,
    /// The component is currently sending a response
    Sending,
}

struct I2cHandlerState {
    i2c: I2cInner,
    i2c_mode: I2cTransactionMode,
    data_index: usize,
    data: [u8; MAX_I2C_MESSAGE_LEN],
    data_len: usize,
}

impl I2cHandlerState {
    // resets state that is not preserved across transaction
    fn reset_non_preserved_state(&mut self) {
        self.data_index = 0;
        self.data_len = 0;
    }

    fn read_bytes(&mut self) {
        if self.i2c_mode == I2cTransactionMode::ReceivingLength {
            let mut len = 0;
            if self.i2c.read_rx_fifo(slice::from_mut(&mut len)) == 0 {
                return;
            }

            self.data_len = len.into();
            self.i2c_mode = I2cTransactionMode::Receiving;
        }

        if self.i2c_mode == I2cTransactionMode::Receiving {
            let read_len = self
                .i2c
                .read_rx_fifo(&mut self.data[self.data_index..self.data_len]);
            self.data_index += read_len;

            if self.data_index == self.data_len {
                // ignore error, nothing we can do about it
                let _ = CLIENT_RECEIVE_BUFFER.try_commit(&self.data[..self.data_len]);

                self.i2c.set_rx_threshold_int_enabled(false);

                self.reset_non_preserved_state();
                self.i2c_mode = I2cTransactionMode::WaitSend;
            }
        }
    }

    fn write_bytes(&mut self) {
        if self.i2c_mode == I2cTransactionMode::Sending {
            let write_len = self
                .i2c
                .write_tx_fifo(&self.data[self.data_index..self.data_len]);
            self.data_index += write_len;

            if self.data_index == self.data_len {
                self.i2c.set_tx_threshold_int_enabled(false);

                self.reset_non_preserved_state();
                self.i2c_mode = I2cTransactionMode::ReadyReceive;
            }
        }
    }

    fn handle_send(&mut self) {
        self.i2c.clear_tx_lockout();

        match self.i2c_mode {
            I2cTransactionMode::WaitSend => {
                let take_result = CLIENT_SEND_BUFFER.try_take(&mut self.data);

                match take_result {
                    Ok(data) => {
                        // avoid borrow error
                        let data_len = data.len();
                        self.data_len = data_len;

                        self.i2c.write_tx_fifo(&[data_len.try_into().unwrap()]);

                        self.i2c_mode = I2cTransactionMode::ReadySend;
                    }
                    Err(_) => {
                        self.i2c.write_tx_fifo(&[0]);
                    }
                }
            }
            I2cTransactionMode::ReadySend => {
                self.i2c_mode = I2cTransactionMode::Sending;
                self.i2c.set_tx_threshold_int_enabled(true);

                self.write_bytes();
            }
            _ => (),
        }
    }

    fn handle_receive(&mut self) {
        if self.i2c_mode == I2cTransactionMode::ReadyReceive {
            self.i2c_mode = I2cTransactionMode::ReceivingLength;
            self.i2c.set_rx_threshold_int_enabled(true);

            self.read_bytes();
        }
    }

    fn handle_interrupt(&mut self) {
        // make sure to handle receive before send, send can occur directly after receive,
        // but receive cannot occur directly after a send (because we have not yet sent the bytes)
        if self.i2c.regs.intfl0().read().rd_addr_match().bit_is_set() {
            self.handle_receive();

            self.i2c.clear_flags(RD_ADDR_MATCH, 0);
        }

        if self.i2c.regs.intfl0().read().rx_thd().bit_is_set()
            && (self.i2c_mode == I2cTransactionMode::Receiving
                || self.i2c_mode == I2cTransactionMode::ReceivingLength)
        {
            self.read_bytes();

            self.i2c.clear_flags(RX_THD, 0);
        }

        if self.i2c.regs.intfl0().read().wr_addr_match().bit_is_set() {
            // read any remaining bytes
            self.read_bytes();
            self.handle_send();

            self.i2c.clear_flags(WR_ADDR_MATCH, 0);
        }

        if self.i2c.regs.intfl0().read().tx_thd().bit_is_set()
            && self.i2c_mode == I2cTransactionMode::Sending
        {
            self.i2c.clear_tx_lockout();

            self.write_bytes();

            self.i2c.clear_flags(TX_THD, 0);
        }
    }
}

static HANDLER_STATE: Mutex<RefCell<Option<I2cHandlerState>>> = Mutex::new(RefCell::new(None));

#[allow(non_snake_case)]
#[interrupt]
fn I2C1() {
    interrupt_free(|token| {
        if let Some(state) = HANDLER_STATE.borrow(token).borrow_mut().as_mut() {
            state.handle_interrupt();
        }
    });
}
