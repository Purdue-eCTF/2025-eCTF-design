use core::ptr;

use max78000_device::{interrupt, FLC};

use crate::{align_down, Gcr, HalError};

pub const FLASH_PAGE_SIZE: usize = 0x2000;
const ADDR_ALIGN: usize = 0x10;

const ADDR_MASK: usize = !(ADDR_ALIGN - 1);
pub const PAGE_MASK: usize = !(FLASH_PAGE_SIZE - 1);

pub const FLASH_BASE_ADDR: usize = 0x10000000;
pub const FLASH_SIZE: usize = 0x80000;

pub struct Flash {
    regs: FLC,
}

impl Flash {
    pub(crate) fn new(regs: FLC) -> Self {
        Flash {
            regs,
        }
    }

    fn await_not_busy(&self) {
        let mut ctrl = self.regs.ctrl().read();

        while ctrl.pend().bit_is_set() || ctrl.wr().bit_is_set() || ctrl.me().bit_is_set() || ctrl.pge().bit_is_set() {
            ctrl = self.regs.ctrl().read();
        }
    }

    fn start_flash_operation(&mut self) {
        self.await_not_busy();

        // msdk sets clkdiv everytime
        let sysclock = Gcr::with(|gcr| gcr.get_sysclock_frequency());
        self.regs.clkdiv().write(|clckdiv| {
            clckdiv.clkdiv().variant((sysclock / 1000000) as u8)
        });

        // clear old errors
        self.regs.intr().modify(|_, intr| {
            intr.af().clear_bit()
        });

        // unlock flash controller
        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.unlock().unlocked()
        });
    }

    fn lock_flash(&mut self) {
        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.unlock().locked()
        });
    }

    fn get_and_clear_error(&mut self) -> Result<(), HalError> {
        let mut error = Ok(());

        self.regs.intr().modify(|val, intr| {
            if val.af().bit_is_set() {
                error = Err(HalError::FlashError);
            }

            intr.af().clear_bit()
        });

        error
    }

    fn flush_line_fill_buffer() {
        //Gcr::with(|gcr| gcr.flush_cache());

        // perform 2 reads from different pages to flush line fill buffer
        unsafe {
            let _ = ptr::read_volatile(FLASH_BASE_ADDR as *const usize);
            let _ = ptr::read_volatile((FLASH_BASE_ADDR + FLASH_PAGE_SIZE) as *const usize);
        }
    }

    fn set_address(&mut self, address: usize) {
        assert!(
            address >= FLASH_BASE_ADDR && address < FLASH_BASE_ADDR + FLASH_SIZE,
            "address does not correspond to flash memory",
        );

        // convert address so that 0 is the start of flash memory
        let flash_address = address & (FLASH_SIZE - 1);

        self.regs.addr().write(|addr| {
            // safety: flash_address has been verified to be a valid flash address
            unsafe { addr.bits(flash_address.try_into().unwrap()) }
        });
    }

    pub unsafe fn erase_page(&mut self, address: usize) -> Result<(), HalError> {
        assert_eq!(address & PAGE_MASK, address, "address not page aligned");

        self.start_flash_operation();

        self.set_address(address);

        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.erase_code().erase_page()
        });

        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.pge().start()
        });

        self.await_not_busy();

        let result = self.get_and_clear_error();

        self.lock_flash();
        Self::flush_line_fill_buffer();

        result
    }

    /// Writes 16 bytes of data to a 16 byte aligned address
    pub unsafe fn write16(&mut self, address: usize, data: &[u8; 16]) -> Result<(), HalError> {
        assert_eq!(address & ADDR_MASK, address, "address not 128 byte aligned");

        self.start_flash_operation();

        self.set_address(address);

        for (i, data) in data.chunks_exact(4).enumerate() {
            self.regs.data(i).write(|data_register| {
                let num = u32::from_le_bytes(data.try_into().unwrap());

                // safety: data register can have any bits written to it
                unsafe { data_register.bits(num) }
            });
        }

        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.wr().start()
        });

        self.await_not_busy();

        let result = self.get_and_clear_error();

        self.lock_flash();
        Self::flush_line_fill_buffer();

        result
    }

    /// Writes the bytes to the given address
    /// 
    /// Panics if the address i not 16 byte aligned
    /// if the length is not 16 byte aligned, the extra bytes are filled with 0s
    pub unsafe fn write(&mut self, address: usize, data: &[u8]) -> Result<(), HalError> {
        assert_eq!(address & ADDR_MASK, address, "address not 128 byte aligned");

        let chunks = data.chunks_exact(16);

        for (i, chunk) in chunks.clone().enumerate() {
            self.write16(address + 16 * i, chunk.try_into().unwrap())?;
        }

        let mut buf = [0; 16];
        let remainder_len = chunks.remainder().len();
        if remainder_len == 0 {
            return Ok(())
        }

        buf[..remainder_len].copy_from_slice(&data[(data.len() - remainder_len)..]);
        buf[remainder_len..].fill(0);

        let last_chunk_addr = align_down(address + data.len(), ADDR_ALIGN);
        self.write16(last_chunk_addr, &buf)
    }
}

// for some reason this is needed for flash to work
#[allow(non_snake_case)]
#[interrupt]
fn FLASH_CONTROLLER() {}