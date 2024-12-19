use core::ptr;

use max78000_device::{interrupt, FLC};

use crate::{align_down, Gcr, HalError};

/// Size in bytes of a flash page on the max78000 board.
pub const FLASH_PAGE_SIZE: usize = 0x2000;

/// The required alignmant of each write to the max78000 flash memory.
const ADDR_ALIGN: usize = 0x10;

const ADDR_MASK: usize = !(ADDR_ALIGN - 1);
pub const PAGE_MASK: usize = !(FLASH_PAGE_SIZE - 1);

/// Start of flash memory in address space.
pub const FLASH_BASE_ADDR: usize = 0x10000000;
/// Size of flash memory.
pub const FLASH_SIZE: usize = 0x80000;

/// Used to interact with the max78000 flash memory.
/// 
/// Performs various fuctionality such as writing to and clearing flash.
pub struct Flash {
    regs: FLC,
}

impl Flash {
    /// Creates a new Flash instance from the flash controller registers.
    pub(crate) fn new(regs: FLC) -> Self {
        Flash {
            regs,
        }
    }

    /// Busy waits until the flash controller reports all pending operations have finished.
    fn await_not_busy(&self) {
        let mut ctrl = self.regs.ctrl().read();

        while ctrl.pend().bit_is_set() || ctrl.wr().bit_is_set() || ctrl.me().bit_is_set() || ctrl.pge().bit_is_set() {
            ctrl = self.regs.ctrl().read();
        }
    }

    /// Starts a flash operation by waiting for all other operations to finish, clearing errors, and unlocking controller.
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

    /// Locks flash controller.
    /// 
    /// This means a cpu exception will be raised if any flash operations
    /// are attempted to be performed while the controller is locked.
    fn lock_flash(&mut self) {
        self.regs.ctrl().modify(|_, ctrl| {
            ctrl.unlock().locked()
        });
    }

    /// Checks if an error has occured with the flash controller, and clears the error if present.
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

    /// Set address to perform next flash operation on.
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

    /// Erases the page at the given address.
    /// 
    /// # Panics
    /// 
    /// Panics if address is not flash page aligned.
    /// 
    /// # Safety
    /// 
    /// Must not erase any page with executable code, or any page that a refrence currently points to.
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
    ///
    /// # Safety
    /// 
    /// Must not write to any bytes with executable code, or any bytes that a refrence currently points to.
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

    /// Writes the bytes to the given address.
    /// 
    /// If the length is not 16 byte aligned, the extra bytes are filled with 0s
    /// 
    /// # Panics
    /// 
    /// Panics if the address i not 16 byte aligned
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

    pub fn lock_page(&mut self, page_address: usize) {
        assert_eq!(page_address & PAGE_MASK, page_address, "address not page aligned");
        assert!(
            page_address >= FLASH_BASE_ADDR && page_address < FLASH_BASE_ADDR + FLASH_SIZE,
            "address does not correspond to flash memory",
        );

        // this will be between 0-64 because of earlier checks
        // device has 64 flash pages
        let page_number = (page_address - FLASH_BASE_ADDR) / FLASH_PAGE_SIZE;

        // flash lock registers have 2 32 bit registers, 1 for first 32 pages, 1 for next 32 pages
        // least significant bit of each register is lowest page, then next bit next page, and so on

        // calculate bit to set in register
        let flash_lock_bit = 1u32 << (page_number % 32);

        if page_number >= 32 {
            // safety: any bit in welr0 register can be written to to lock a flash page
            self.regs.welr0().write(|welr0| unsafe {
                welr0.bits(flash_lock_bit)
            });
        } else {
            // safety: any bit in welr0 register can be written to to lock a flash page
            self.regs.welr1().write(|welr1| unsafe {
                welr1.bits(flash_lock_bit)
            });
        }
    }
}

// for some reason this is needed for flash to work
// iirc hal macros expected interrupt handler to exist when compiling, but we don't need to handle flash interrupts ever
#[allow(non_snake_case)]
#[interrupt]
fn FLASH_CONTROLLER() {}