//! Various functions libc expects to exist when linking with it for post boot code

use core::ffi::{c_char, c_int, c_long, c_void};
use core::slice;

use max78000_hal::uart::uart;

const STDIN: c_int = 0;
const STDOUT: c_int = 1;
const STDERR: c_int = 2;

// isize is used instead of intptr_t
#[no_mangle]
extern "C" fn _sbrk(_increment: isize) -> *mut c_void {
    usize::MAX as *mut c_void
}

#[no_mangle]
extern "C" fn _open(_filename: *const c_char, _flags: c_int, _mode: c_int) -> c_int {
    -1
}

#[no_mangle]
extern "C" fn _close(_fd: c_int) -> c_int {
    -1
}

#[no_mangle]
extern "C" fn _isatty(_fd: c_int) -> c_int {
    -1
}

// TODO: find out if c_long is correct for _offset, which has type off_t
#[no_mangle]
extern "C" fn _lseek(_fd: c_int, _offset: c_long, _whence: c_int) -> c_int {
    -1
}

// void * is used instead of struct stat *
#[no_mangle]
extern "C" fn _fstat(_fd: c_int, _stat_data: *mut c_void) -> c_int {
    -1
}

// TODO: find out if these types are correct for read
#[no_mangle]
unsafe extern "C" fn _read(fd: c_int, buf: *mut c_char, len: c_int) -> c_int {
    if fd == STDIN {
        // safety: _read must be called with valid buf and len
        let buffer = unsafe {
            slice::from_raw_parts_mut(buf as *mut u8, len.try_into().unwrap())
        };

        uart().read_bytes(buffer).len().try_into().unwrap()
    } else {
        -1
    }
}

// TODO: find out if these types are correct for write
#[no_mangle]
unsafe extern "C" fn _write(fd: c_int, buf: *const c_char, len: c_int) -> c_int {
    if fd == STDOUT || fd == STDERR {
        // safety: _write must be called with valid buf and len
        let buffer = unsafe {
            slice::from_raw_parts(buf as *const u8, len.try_into().unwrap())
        };

        uart().write_bytes(buffer);

        buffer.len().try_into().unwrap()
    } else {
        -1
    }
}
