#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]

pub mod abi;
pub mod error;
pub mod raw {
    pub mod alloc;
}
pub mod scalar;
pub mod utf;
pub mod version;

use std::ffi::{c_char, c_int, c_void};
use std::ptr::{null, null_mut};

use libc::FILE;

pub use abi::{
    json_dump_callback_t, json_error_t, json_free_t, json_int_t, json_load_callback_t,
    json_malloc_t, json_t, json_type, JANSSON_MAJOR_VERSION, JANSSON_MICRO_VERSION,
    JANSSON_MINOR_VERSION, JSON_ARRAY, JSON_ERROR_SOURCE_LENGTH, JSON_ERROR_TEXT_LENGTH,
    JSON_FALSE, JSON_INTEGER, JSON_NULL, JSON_OBJECT, JSON_REAL, JSON_STRING, JSON_TRUE,
};

#[no_mangle]
pub extern "C" fn json_object_iter_key(iter: *mut c_void) -> *const c_char {
    let _ = iter;
    null()
}

#[no_mangle]
pub extern "C" fn json_object_iter_key_len(iter: *mut c_void) -> usize {
    let _ = iter;
    0
}

#[no_mangle]
pub extern "C" fn json_object_seed(seed: usize) {
    let _ = seed;
}

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

macro_rules! stub_json_ptr {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) ;)+) => {
        $(
            #[no_mangle]
            pub extern "C" fn $name($($arg: $ty),*) -> *mut json_t {
                let _ = ($($arg),*);
                null_mut()
            }
        )+
    };
}

macro_rules! stub_void_ptr {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) ;)+) => {
        $(
            #[no_mangle]
            pub extern "C" fn $name($($arg: $ty),*) -> *mut c_void {
                let _ = ($($arg),*);
                null_mut()
            }
        )+
    };
}

macro_rules! stub_usize {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) ;)+) => {
        $(
            #[no_mangle]
            pub extern "C" fn $name($($arg: $ty),*) -> usize {
                let _ = ($($arg),*);
                0
            }
        )+
    };
}

macro_rules! stub_int {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) ;)+) => {
        $(
            #[no_mangle]
            pub extern "C" fn $name($($arg: $ty),*) -> c_int {
                let _ = ($($arg),*);
                -1
            }
        )+
    };
}

stub_json_ptr! {
    fn json_array();
    fn json_array_get(array: *const json_t, index: usize);
    fn json_object();
    fn json_object_get(object: *const json_t, key: *const c_char);
    fn json_object_getn(object: *const json_t, key: *const c_char, key_len: usize);
    fn json_object_iter_value(iter: *mut c_void);
}

stub_void_ptr! {
    fn json_object_iter(object: *mut json_t);
    fn json_object_iter_at(object: *mut json_t, key: *const c_char);
    fn json_object_iter_next(object: *mut json_t, iter: *mut c_void);
    fn json_object_key_to_iter(key: *const c_char);
}

stub_usize! {
    fn json_array_size(array: *const json_t);
    fn json_object_size(object: *const json_t);
}

stub_int! {
    fn json_array_set_new(array: *mut json_t, index: usize, value: *mut json_t);
    fn json_array_append_new(array: *mut json_t, value: *mut json_t);
    fn json_array_insert_new(array: *mut json_t, index: usize, value: *mut json_t);
    fn json_array_remove(array: *mut json_t, index: usize);
    fn json_array_clear(array: *mut json_t);
    fn json_array_extend(array: *mut json_t, other: *mut json_t);
    fn json_object_set_new(object: *mut json_t, key: *const c_char, value: *mut json_t);
    fn json_object_setn_new(
        object: *mut json_t,
        key: *const c_char,
        key_len: usize,
        value: *mut json_t
    );
    fn json_object_set_new_nocheck(object: *mut json_t, key: *const c_char, value: *mut json_t);
    fn json_object_setn_new_nocheck(
        object: *mut json_t,
        key: *const c_char,
        key_len: usize,
        value: *mut json_t
    );
    fn json_object_del(object: *mut json_t, key: *const c_char);
    fn json_object_deln(object: *mut json_t, key: *const c_char, key_len: usize);
    fn json_object_clear(object: *mut json_t);
    fn json_object_update(object: *mut json_t, other: *mut json_t);
    fn json_object_update_existing(object: *mut json_t, other: *mut json_t);
    fn json_object_update_missing(object: *mut json_t, other: *mut json_t);
    fn json_object_update_recursive(object: *mut json_t, other: *mut json_t);
    fn json_object_iter_set_new(object: *mut json_t, iter: *mut c_void, value: *mut json_t);
}
