use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

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

pub const IMMORTAL_REFCOUNT: usize = usize::MAX;

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

#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum json_error_code {
    json_error_unknown = 0,
    json_error_out_of_memory = 1,
    json_error_stack_overflow = 2,
    json_error_cannot_open_file = 3,
    json_error_invalid_argument = 4,
    json_error_invalid_utf8 = 5,
    json_error_premature_end_of_input = 6,
    json_error_end_of_input_expected = 7,
    json_error_invalid_syntax = 8,
    json_error_invalid_format = 9,
    json_error_wrong_type = 10,
    json_error_null_character = 11,
    json_error_null_value = 12,
    json_error_null_byte_in_key = 13,
    json_error_duplicate_key = 14,
    json_error_numeric_overflow = 15,
    json_error_item_not_found = 16,
    json_error_index_out_of_range = 17,
}

#[inline]
pub fn bool_to_c_int(value: bool) -> c_int {
    if value {
        1
    } else {
        0
    }
}

#[inline]
pub unsafe fn is_type(json: *const json_t, kind: json_type) -> bool {
    !json.is_null() && unsafe { (*json).type_ == kind }
}

#[inline]
pub unsafe fn is_object(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_OBJECT) }
}

#[inline]
pub unsafe fn is_array(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_ARRAY) }
}

#[inline]
pub unsafe fn is_string(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_STRING) }
}

#[inline]
pub unsafe fn is_integer(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_INTEGER) }
}

#[inline]
pub unsafe fn is_real(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_REAL) }
}

#[inline]
pub unsafe fn is_true(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_TRUE) }
}

#[inline]
pub unsafe fn is_false(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_FALSE) }
}

#[inline]
pub unsafe fn is_null(json: *const json_t) -> bool {
    unsafe { is_type(json, JSON_NULL) }
}

#[inline]
pub unsafe fn is_number(json: *const json_t) -> bool {
    unsafe { is_integer(json) || is_real(json) }
}

#[inline]
pub unsafe fn type_of(json: *const json_t) -> Option<json_type> {
    if json.is_null() {
        None
    } else {
        Some(unsafe { (*json).type_ })
    }
}

#[inline]
unsafe fn atomic_refcount(json: *mut json_t) -> *const AtomicUsize {
    unsafe { ptr::addr_of!((*json).refcount).cast::<AtomicUsize>() }
}

#[inline]
pub unsafe fn incref(json: *mut json_t) -> *mut json_t {
    if !json.is_null() && unsafe { (*json).refcount } != IMMORTAL_REFCOUNT {
        let refcount = unsafe { &*atomic_refcount(json) };
        refcount.fetch_add(1, Ordering::Acquire);
    }

    json
}

#[inline]
pub unsafe fn decref(json: *mut json_t) {
    if json.is_null() || unsafe { (*json).refcount } == IMMORTAL_REFCOUNT {
        return;
    }

    let refcount = unsafe { &*atomic_refcount(json) };
    if refcount.fetch_sub(1, Ordering::Release) == 1 {
        crate::scalar::json_delete(json);
    }
}
