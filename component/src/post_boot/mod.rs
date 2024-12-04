use core::ffi::{c_int, c_uint};
use core::slice;

use max78000_hal::led::{Led, led_on, led_off, led_toggle};
use design_utils::MAX_POST_BOOT_MESSAGE_SIZE;

use crate::component_driver::ComponentDriver;

mod messaging;

// definition of c post boot function
extern "C" {
    pub fn post_boot();
}

// FIXME: don't use static mut
// Can't use critical section cause interrupts need to be turned on while
// a mutable reference to this is needed
static mut COMPONENT_DRIVER: Option<ComponentDriver> = None;

unsafe fn with_driver<T>(f: impl FnOnce(&mut ComponentDriver) -> T) -> T {
    unsafe {
        f(COMPONENT_DRIVER.as_mut().unwrap())
    }
}

pub fn boot(driver: ComponentDriver) -> ! {
    // safety: no other code has a reference to the component driver at this time
    unsafe {
        COMPONENT_DRIVER = Some(driver);
    }

    unsafe { post_boot(); }

    loop {}
}

#[no_mangle]
extern "C" fn secure_send(buffer: *const u8, len: u8) {
    assert!((len as usize) <= MAX_POST_BOOT_MESSAGE_SIZE);

    // safety: post boot c code is supposed to give us a valid buffer for reading len bytes from
    let message = unsafe {
        slice::from_raw_parts(buffer, len.into())
    };

    // safety: no other code is using the component driver at this time
    unsafe { 
        with_driver(|driver| messaging::secure_send(driver, message))
            .expect("secure send failed");
    }
}

#[no_mangle]
extern "C" fn secure_receive(buffer: *mut u8) -> c_int {
    // safety: lets just hope post boot code gives a buffer of this size
    let recv_buf = unsafe {
        (buffer as *mut [u8; MAX_POST_BOOT_MESSAGE_SIZE]).as_mut().unwrap()
    };

    // messaging::secure_recieve ensrues recv_len does not exceed MAX_POST_BOOT_MESSAGE_SIZE
    // safety: no other code is using the component driver at this time
    let recv_len = unsafe {
        with_driver(|driver| messaging::secure_receive(driver, recv_buf))
            .expect("secure receive failed")
    };

    recv_len.try_into().unwrap()
}

#[no_mangle]
extern "C" fn LED_On(idx: c_uint) {
    let Some(led) = Led::from_index(idx) else {
        return
    };

    led_on(led);
}

#[no_mangle]
extern "C" fn LED_Off(idx: c_uint) {
    let Some(led) = Led::from_index(idx) else {
        return
    };

    led_off(led);
}

#[no_mangle]
extern "C" fn LED_Toggle(idx: c_uint) {
    let Some(led) = Led::from_index(idx) else {
        return
    };

    led_toggle(led);
}
