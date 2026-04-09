use std::ffi::{c_char, c_int};

use crate::abi::{JANSSON_MAJOR_VERSION, JANSSON_MICRO_VERSION, JANSSON_MINOR_VERSION};

const VERSION_CSTR: &[u8] = b"2.14\0";

#[no_mangle]
pub extern "C" fn jansson_version_str() -> *const c_char {
    VERSION_CSTR.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn jansson_version_cmp(major: c_int, minor: c_int, micro: c_int) -> c_int {
    let mut diff = JANSSON_MAJOR_VERSION - major;
    if diff != 0 {
        return diff;
    }

    diff = JANSSON_MINOR_VERSION - minor;
    if diff != 0 {
        return diff;
    }

    JANSSON_MICRO_VERSION - micro
}
