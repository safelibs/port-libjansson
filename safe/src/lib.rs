#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]

use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::ffi::{c_char, c_double, c_int, c_void};
use std::ptr::{null, null_mut};
use std::sync::RwLock;

use libc::FILE;

pub const JANSSON_MAJOR_VERSION: c_int = 2;
pub const JANSSON_MINOR_VERSION: c_int = 14;
pub const JANSSON_MICRO_VERSION: c_int = 0;
pub const JSON_ERROR_TEXT_LENGTH: usize = 160;
pub const JSON_ERROR_SOURCE_LENGTH: usize = 80;

pub type json_type = c_int;

pub const JSON_OBJECT: json_type = 0;
pub const JSON_ARRAY: json_type = 1;
pub const JSON_STRING: json_type = 2;
pub const JSON_INTEGER: json_type = 3;
pub const JSON_REAL: json_type = 4;
pub const JSON_TRUE: json_type = 5;
pub const JSON_FALSE: json_type = 6;
pub const JSON_NULL: json_type = 7;

pub type json_int_t = libc::c_longlong;
pub type json_load_callback_t =
    Option<unsafe extern "C" fn(buffer: *mut c_void, buflen: usize, data: *mut c_void) -> usize>;
pub type json_dump_callback_t =
    Option<unsafe extern "C" fn(buffer: *const c_char, size: usize, data: *mut c_void) -> c_int>;
pub type json_malloc_t = Option<unsafe extern "C" fn(size: usize) -> *mut c_void>;
pub type json_free_t = Option<unsafe extern "C" fn(ptr: *mut c_void)>;

#[repr(C)]
pub struct json_t {
    pub type_: json_type,
    pub refcount: usize,
}

#[repr(C)]
pub struct json_error_t {
    pub line: c_int,
    pub column: c_int,
    pub position: c_int,
    pub source: [c_char; JSON_ERROR_SOURCE_LENGTH],
    pub text: [c_char; JSON_ERROR_TEXT_LENGTH],
}

const IMMORTAL_REFCOUNT: usize = usize::MAX;
const VERSION_CSTR: &[u8] = b"2.14\0";
const EMPTY_CSTR: &[u8] = b"\0";
const DEFAULT_ALLOC_FNS: AllocFns = AllocFns {
    malloc_fn: Some(libc::malloc),
    free_fn: Some(libc::free),
};

#[repr(transparent)]
struct JsonCell(UnsafeCell<json_t>);

unsafe impl Sync for JsonCell {}

#[derive(Clone, Copy)]
struct AllocFns {
    malloc_fn: json_malloc_t,
    free_fn: json_free_t,
}

static JSON_TRUE_SINGLETON: JsonCell = JsonCell(UnsafeCell::new(json_t {
    type_: JSON_TRUE,
    refcount: IMMORTAL_REFCOUNT,
}));
static JSON_FALSE_SINGLETON: JsonCell = JsonCell(UnsafeCell::new(json_t {
    type_: JSON_FALSE,
    refcount: IMMORTAL_REFCOUNT,
}));
static JSON_NULL_SINGLETON: JsonCell = JsonCell(UnsafeCell::new(json_t {
    type_: JSON_NULL,
    refcount: IMMORTAL_REFCOUNT,
}));
static ALLOC_FNS: RwLock<AllocFns> = RwLock::new(DEFAULT_ALLOC_FNS);

#[inline]
fn empty_cstr() -> *const c_char {
    EMPTY_CSTR.as_ptr().cast()
}

#[inline]
fn version_cstr() -> *const c_char {
    VERSION_CSTR.as_ptr().cast()
}

#[inline]
fn bool_to_c_int(value: bool) -> c_int {
    if value {
        1
    } else {
        0
    }
}

#[inline]
fn version_ordering(major: c_int, minor: c_int, micro: c_int) -> Ordering {
    (
        JANSSON_MAJOR_VERSION,
        JANSSON_MINOR_VERSION,
        JANSSON_MICRO_VERSION,
    )
        .cmp(&(major, minor, micro))
}

#[inline]
fn singleton_ptr(cell: &'static JsonCell) -> *mut json_t {
    cell.0.get()
}

#[no_mangle]
pub extern "C" fn json_delete(json: *mut json_t) {
    let _ = json;
}

#[no_mangle]
pub extern "C" fn json_true() -> *mut json_t {
    singleton_ptr(&JSON_TRUE_SINGLETON)
}

#[no_mangle]
pub extern "C" fn json_false() -> *mut json_t {
    singleton_ptr(&JSON_FALSE_SINGLETON)
}

#[no_mangle]
pub extern "C" fn json_null() -> *mut json_t {
    singleton_ptr(&JSON_NULL_SINGLETON)
}

#[no_mangle]
pub extern "C" fn json_string_value(string: *const json_t) -> *const c_char {
    let _ = string;
    empty_cstr()
}

#[no_mangle]
pub extern "C" fn json_string_length(string: *const json_t) -> usize {
    let _ = string;
    0
}

#[no_mangle]
pub extern "C" fn json_integer_value(integer: *const json_t) -> json_int_t {
    let _ = integer;
    0
}

#[no_mangle]
pub extern "C" fn json_real_value(real: *const json_t) -> c_double {
    let _ = real;
    0.0
}

#[no_mangle]
pub extern "C" fn json_number_value(json: *const json_t) -> c_double {
    let _ = json;
    0.0
}

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
pub extern "C" fn json_equal(value1: *const json_t, value2: *const json_t) -> c_int {
    bool_to_c_int(value1 == value2)
}

#[no_mangle]
pub extern "C" fn json_copy(value: *mut json_t) -> *mut json_t {
    match value {
        ptr if ptr == json_true() => json_true(),
        ptr if ptr == json_false() => json_false(),
        ptr if ptr == json_null() => json_null(),
        _ => null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn json_deep_copy(value: *const json_t) -> *mut json_t {
    match value {
        ptr if ptr == json_true().cast_const() => json_true(),
        ptr if ptr == json_false().cast_const() => json_false(),
        ptr if ptr == json_null().cast_const() => json_null(),
        _ => null_mut(),
    }
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

#[no_mangle]
pub extern "C" fn json_set_alloc_funcs(malloc_fn: json_malloc_t, free_fn: json_free_t) {
    if let Ok(mut guard) = ALLOC_FNS.write() {
        *guard = AllocFns { malloc_fn, free_fn };
    }
}

#[no_mangle]
pub unsafe extern "C" fn json_get_alloc_funcs(
    malloc_fn: *mut json_malloc_t,
    free_fn: *mut json_free_t,
) {
    let current = ALLOC_FNS
        .read()
        .map(|guard| *guard)
        .unwrap_or(DEFAULT_ALLOC_FNS);

    if !malloc_fn.is_null() {
        unsafe {
            *malloc_fn = current.malloc_fn;
        }
    }

    if !free_fn.is_null() {
        unsafe {
            *free_fn = current.free_fn;
        }
    }
}

#[no_mangle]
pub extern "C" fn jansson_version_str() -> *const c_char {
    version_cstr()
}

#[no_mangle]
pub extern "C" fn jansson_version_cmp(major: c_int, minor: c_int, micro: c_int) -> c_int {
    match version_ordering(major, minor, micro) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
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
    fn json_string(value: *const c_char);
    fn json_stringn(value: *const c_char, len: usize);
    fn json_string_nocheck(value: *const c_char);
    fn json_stringn_nocheck(value: *const c_char, len: usize);
    fn json_integer(value: json_int_t);
    fn json_real(value: c_double);
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
    fn json_string_set(string: *mut json_t, value: *const c_char);
    fn json_string_setn(string: *mut json_t, value: *const c_char, len: usize);
    fn json_string_set_nocheck(string: *mut json_t, value: *const c_char);
    fn json_string_setn_nocheck(string: *mut json_t, value: *const c_char, len: usize);
    fn json_integer_set(integer: *mut json_t, value: json_int_t);
    fn json_real_set(real: *mut json_t, value: c_double);
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
