use core::ffi::c_int;
use core::ptr;
use core::slice;
use core::time::Duration;

use design_utils::MAX_POST_BOOT_MESSAGE_SIZE;
use max78000_hal::i2c::I2cAddr;
use max78000_hal::timer::sleep;

use crate::ap_driver::ApDriver;

mod messaging;

// return codes used by the c code
const SUCCESS_RETURN: c_int = 0;

// definition of c post boot function
extern "C" {
    pub fn post_boot();
}

// FIXME: don't use static mut
// Can't use critical section cause interrupts need to be turned on while
// a mutable reference to this is needed
static mut AP_DRIVER: Option<ApDriver> = None;

unsafe fn with_driver<T>(f: impl FnOnce(&mut ApDriver) -> T) -> T {
    unsafe {
        let driver = AP_DRIVER.as_mut()
            .expect("ap driver not initialized");

        f(driver)
    }
}

pub fn boot(driver: ApDriver) -> ! {
    // safety: no other code is using this ap driver at this time
    unsafe {
        AP_DRIVER = Some(driver);
    }

    unsafe { post_boot(); }

    loop {}
}

#[no_mangle]
extern "C" fn secure_send(address: I2cAddr, buf: *const u8, len: u8) -> c_int {
    assert!((len as usize) <= MAX_POST_BOOT_MESSAGE_SIZE);

    // safety: post boot c code is supposed to give us a valid buffer for reading len bytes from
    let message = unsafe {
        slice::from_raw_parts(buf, len.into())
    };

    // safety: no other code is using the ap driver at this time
    unsafe {
        with_driver(|driver| messaging::secure_send(driver, address, message))
            .expect("could not send message to component");
    }

    SUCCESS_RETURN
}

#[no_mangle]
extern "C" fn secure_receive(address: I2cAddr, buffer: *mut u8) -> c_int {
    // safety: lets just hope post boot code gives a buffer of this size
    let recv_buf = unsafe {
        (buffer as *mut [u8; MAX_POST_BOOT_MESSAGE_SIZE]).as_mut().unwrap()
    };

    // messaging::secure_recieve ensrues recv_len does not exceed MAX_POST_BOOT_MESSAGE_SIZE
    // safety: no other code is using the ap driver at this time
    unsafe {
        with_driver(|driver| messaging::secure_receive(driver, address, recv_buf))
            .expect("could not recieve message from component")
            .try_into()
            .unwrap()
    }
}

#[no_mangle]
extern "C" fn get_provisioned_ids(buffer: *mut u32) -> c_int {
    // safety: no other code is using the ap driver at this time
    let flash_data = unsafe {
        with_driver(|driver| driver.get_flash_data())
    };

    for i in 0..flash_data.components_len {
        // safety: post boot c code is presumably supposed to give us an aligned buffer for at least 2 u32?
        unsafe {
            let ptr = buffer.add(i);
            ptr::write(ptr, flash_data.components[i].component_id);
        }
    }

    flash_data.components_len.try_into().unwrap()
}

#[no_mangle]
extern "C" fn MXC_Delay(microseconds: u32) {
    sleep(Duration::from_micros(microseconds.into()));
}
