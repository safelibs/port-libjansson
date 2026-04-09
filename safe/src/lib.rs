#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]

pub mod abi;
pub mod array;
pub mod error;
pub mod object;
pub mod raw {
    pub mod alloc;
    pub mod buf;
    pub mod list;
    pub mod table;
}
pub mod scalar;
pub mod utf;
pub mod version;

use std::ffi::{c_char, c_int, c_void};
use std::ptr::null_mut;

use libc::FILE;

pub use abi::{
    json_dump_callback_t, json_error_t, json_free_t, json_int_t, json_load_callback_t,
    json_malloc_t, json_t, json_type, JANSSON_MAJOR_VERSION, JANSSON_MICRO_VERSION,
    JANSSON_MINOR_VERSION, JSON_ARRAY, JSON_ERROR_SOURCE_LENGTH, JSON_ERROR_TEXT_LENGTH,
    JSON_FALSE, JSON_INTEGER, JSON_NULL, JSON_OBJECT, JSON_REAL, JSON_STRING, JSON_TRUE,
};

#[no_mangle]
pub extern "C" fn json_dumps(json: *const json_t, flags: usize) -> *mut c_char {
    let _ = (json, flags);
    null_mut()
}

#[no_mangle]
pub extern "C" fn json_dumpb(
    json: *const json_t,
    buffer: *mut c_char,
    size: usize,
    flags: usize,
) -> usize {
    let _ = (json, buffer, size, flags);
    0
}

#[no_mangle]
pub extern "C" fn json_dumpf(json: *const json_t, output: *mut FILE, flags: usize) -> c_int {
    let _ = (json, output, flags);
    -1
}

#[no_mangle]
pub extern "C" fn json_dumpfd(json: *const json_t, output: c_int, flags: usize) -> c_int {
    let _ = (json, output, flags);
    -1
}

#[no_mangle]
pub extern "C" fn json_dump_file(json: *const json_t, path: *const c_char, flags: usize) -> c_int {
    let _ = (json, path, flags);
    -1
}

#[no_mangle]
pub extern "C" fn json_dump_callback(
    json: *const json_t,
    callback: json_dump_callback_t,
    data: *mut c_void,
    flags: usize,
) -> c_int {
    let _ = (json, callback, data, flags);
    -1
}

#[no_mangle]
pub extern "C" fn json_loads(
    input: *const c_char,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    let _ = (input, flags, error);
    null_mut()
}

#[no_mangle]
pub extern "C" fn json_loadb(
    buffer: *const c_char,
    buflen: usize,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    let _ = (buffer, buflen, flags, error);
    null_mut()
}

#[no_mangle]
pub extern "C" fn json_loadf(
    input: *mut FILE,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    let _ = (input, flags, error);
    null_mut()
}

#[no_mangle]
pub extern "C" fn json_loadfd(input: c_int, flags: usize, error: *mut json_error_t) -> *mut json_t {
    let _ = (input, flags, error);
    null_mut()
}

#[no_mangle]
pub extern "C" fn json_load_file(
    path: *const c_char,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    let _ = (path, flags, error);
    null_mut()
}

#[no_mangle]
pub extern "C" fn json_load_callback(
    callback: json_load_callback_t,
    data: *mut c_void,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    let _ = (callback, data, flags, error);
    null_mut()
}
