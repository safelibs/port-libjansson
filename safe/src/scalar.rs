use std::cell::UnsafeCell;
use std::ffi::{c_char, c_double, c_int};
use std::ptr::{self, null, null_mut};
use std::slice;

use crate::abi::{
    bool_to_c_int, json_int_t, json_t, type_of, IMMORTAL_REFCOUNT, JSON_FALSE, JSON_INTEGER,
    JSON_NULL, JSON_REAL, JSON_STRING, JSON_TRUE,
};
use crate::raw::alloc::{alloc, free, jsonp_free, jsonp_strndup};
use crate::utf;

#[repr(C)]
struct json_string_t {
    json: json_t,
    value: *mut c_char,
    length: usize,
}

#[repr(C)]
struct json_integer_t {
    json: json_t,
    value: json_int_t,
}

#[repr(C)]
struct json_real_t {
    json: json_t,
    value: c_double,
}

#[repr(transparent)]
struct JsonSingleton(UnsafeCell<json_t>);

unsafe impl Sync for JsonSingleton {}

static JSON_TRUE_SINGLETON: JsonSingleton = JsonSingleton(UnsafeCell::new(json_t {
    type_: JSON_TRUE,
    refcount: IMMORTAL_REFCOUNT,
}));
static JSON_FALSE_SINGLETON: JsonSingleton = JsonSingleton(UnsafeCell::new(json_t {
    type_: JSON_FALSE,
    refcount: IMMORTAL_REFCOUNT,
}));
static JSON_NULL_SINGLETON: JsonSingleton = JsonSingleton(UnsafeCell::new(json_t {
    type_: JSON_NULL,
    refcount: IMMORTAL_REFCOUNT,
}));

#[inline]
fn singleton_ptr(singleton: &'static JsonSingleton) -> *mut json_t {
    singleton.0.get()
}

#[inline]
unsafe fn init_json(json: *mut json_t, kind: c_int) {
    unsafe {
        (*json).type_ = kind;
        (*json).refcount = 1;
    }
}

#[inline]
unsafe fn as_string_ptr(json: *const json_t) -> *const json_string_t {
    json.cast::<json_string_t>()
}

#[inline]
unsafe fn as_string_mut(json: *mut json_t) -> *mut json_string_t {
    json.cast::<json_string_t>()
}

#[inline]
unsafe fn as_integer_ptr(json: *const json_t) -> *const json_integer_t {
    json.cast::<json_integer_t>()
}

#[inline]
unsafe fn as_integer_mut(json: *mut json_t) -> *mut json_integer_t {
    json.cast::<json_integer_t>()
}

#[inline]
unsafe fn as_real_ptr(json: *const json_t) -> *const json_real_t {
    json.cast::<json_real_t>()
}

#[inline]
unsafe fn as_real_mut(json: *mut json_t) -> *mut json_real_t {
    json.cast::<json_real_t>()
}

unsafe fn string_create(value: *const c_char, len: usize, own: bool) -> *mut json_t {
    if value.is_null() {
        return null_mut();
    }

    let dup = if own {
        value.cast_mut()
    } else {
        unsafe { jsonp_strndup(value, len) }
    };

    if dup.is_null() {
        return null_mut();
    }

    let string = unsafe { alloc::<json_string_t>() };
    if string.is_null() {
        unsafe {
            jsonp_free(dup.cast());
        }
        return null_mut();
    }

    unsafe {
        init_json(ptr::addr_of_mut!((*string).json), JSON_STRING);
        (*string).value = dup;
        (*string).length = len;
        ptr::addr_of_mut!((*string).json)
    }
}

unsafe fn string_clone(json: *const json_t) -> *mut json_t {
    let string = unsafe { &*as_string_ptr(json) };
    unsafe { json_stringn_nocheck(string.value.cast_const(), string.length) }
}

unsafe fn integer_clone(json: *const json_t) -> *mut json_t {
    unsafe { json_integer((*as_integer_ptr(json)).value) }
}

unsafe fn real_clone(json: *const json_t) -> *mut json_t {
    unsafe { json_real((*as_real_ptr(json)).value) }
}

unsafe fn delete_string(json: *mut json_t) {
    let string = unsafe { as_string_mut(json) };
    unsafe {
        jsonp_free((*string).value.cast());
        free(string);
    }
}

unsafe fn delete_integer(json: *mut json_t) {
    unsafe {
        free(as_integer_mut(json));
    }
}

unsafe fn delete_real(json: *mut json_t) {
    unsafe {
        free(as_real_mut(json));
    }
}

#[no_mangle]
pub extern "C" fn json_delete(json: *mut json_t) {
    if json.is_null() {
        return;
    }

    match unsafe { (*json).type_ } {
        JSON_STRING => unsafe { delete_string(json) },
        JSON_INTEGER => unsafe { delete_integer(json) },
        JSON_REAL => unsafe { delete_real(json) },
        _ => {}
    }
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
pub unsafe extern "C" fn jsonp_stringn_nocheck_own(
    value: *const c_char,
    len: usize,
) -> *mut json_t {
    unsafe { string_create(value, len, true) }
}

#[no_mangle]
pub unsafe extern "C" fn jsonp_sprintf_string_own(value: *mut c_char, len: usize) -> *mut json_t {
    if value.is_null() {
        return null_mut();
    }

    let bytes = unsafe { slice::from_raw_parts(value.cast::<u8>(), len) };
    if !utf::validate(bytes) {
        unsafe {
            jsonp_free(value.cast());
        }
        return null_mut();
    }

    unsafe { jsonp_stringn_nocheck_own(value.cast_const(), len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_string(value: *const c_char) -> *mut json_t {
    if value.is_null() {
        return null_mut();
    }

    let len = unsafe { libc::strlen(value) };
    unsafe { json_stringn(value, len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_stringn(value: *const c_char, len: usize) -> *mut json_t {
    if value.is_null() || !unsafe { utf::validate_ptr(value, len) } {
        return null_mut();
    }

    unsafe { json_stringn_nocheck(value, len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_string_nocheck(value: *const c_char) -> *mut json_t {
    if value.is_null() {
        return null_mut();
    }

    let len = unsafe { libc::strlen(value) };
    unsafe { json_stringn_nocheck(value, len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_stringn_nocheck(value: *const c_char, len: usize) -> *mut json_t {
    unsafe { string_create(value, len, false) }
}

#[no_mangle]
pub extern "C" fn json_string_value(string: *const json_t) -> *const c_char {
    if !unsafe { crate::abi::is_string(string) } {
        return null();
    }

    unsafe { (*as_string_ptr(string)).value.cast_const() }
}

#[no_mangle]
pub extern "C" fn json_string_length(string: *const json_t) -> usize {
    if !unsafe { crate::abi::is_string(string) } {
        return 0;
    }

    unsafe { (*as_string_ptr(string)).length }
}

#[no_mangle]
pub unsafe extern "C" fn json_string_set(string: *mut json_t, value: *const c_char) -> c_int {
    if value.is_null() {
        return -1;
    }

    let len = unsafe { libc::strlen(value) };
    unsafe { json_string_setn(string, value, len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_string_setn(
    string: *mut json_t,
    value: *const c_char,
    len: usize,
) -> c_int {
    if value.is_null() || !unsafe { utf::validate_ptr(value, len) } {
        return -1;
    }

    unsafe { json_string_setn_nocheck(string, value, len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_string_set_nocheck(
    string: *mut json_t,
    value: *const c_char,
) -> c_int {
    if value.is_null() {
        return -1;
    }

    let len = unsafe { libc::strlen(value) };
    unsafe { json_string_setn_nocheck(string, value, len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_string_setn_nocheck(
    string: *mut json_t,
    value: *const c_char,
    len: usize,
) -> c_int {
    if !unsafe { crate::abi::is_string(string) } || value.is_null() {
        return -1;
    }

    let dup = unsafe { jsonp_strndup(value, len) };
    if dup.is_null() {
        return -1;
    }

    let string_value = unsafe { &mut *as_string_mut(string) };
    let old_value = string_value.value;
    string_value.value = dup;
    string_value.length = len;

    unsafe {
        jsonp_free(old_value.cast());
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn json_integer(value: json_int_t) -> *mut json_t {
    let integer = unsafe { alloc::<json_integer_t>() };
    if integer.is_null() {
        return null_mut();
    }

    unsafe {
        init_json(ptr::addr_of_mut!((*integer).json), JSON_INTEGER);
        (*integer).value = value;
        ptr::addr_of_mut!((*integer).json)
    }
}

#[no_mangle]
pub extern "C" fn json_integer_value(integer: *const json_t) -> json_int_t {
    if !unsafe { crate::abi::is_integer(integer) } {
        return 0;
    }

    unsafe { (*as_integer_ptr(integer)).value }
}

#[no_mangle]
pub unsafe extern "C" fn json_integer_set(integer: *mut json_t, value: json_int_t) -> c_int {
    if !unsafe { crate::abi::is_integer(integer) } {
        return -1;
    }

    unsafe {
        (*as_integer_mut(integer)).value = value;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn json_real(value: c_double) -> *mut json_t {
    if !value.is_finite() {
        return null_mut();
    }

    let real = unsafe { alloc::<json_real_t>() };
    if real.is_null() {
        return null_mut();
    }

    unsafe {
        init_json(ptr::addr_of_mut!((*real).json), JSON_REAL);
        (*real).value = value;
        ptr::addr_of_mut!((*real).json)
    }
}

#[no_mangle]
pub extern "C" fn json_real_value(real: *const json_t) -> c_double {
    if !unsafe { crate::abi::is_real(real) } {
        return 0.0;
    }

    unsafe { (*as_real_ptr(real)).value }
}

#[no_mangle]
pub unsafe extern "C" fn json_real_set(real: *mut json_t, value: c_double) -> c_int {
    if !unsafe { crate::abi::is_real(real) } || !value.is_finite() {
        return -1;
    }

    unsafe {
        (*as_real_mut(real)).value = value;
    }
    0
}

#[no_mangle]
pub extern "C" fn json_number_value(json: *const json_t) -> c_double {
    if unsafe { crate::abi::is_integer(json) } {
        json_integer_value(json) as c_double
    } else if unsafe { crate::abi::is_real(json) } {
        json_real_value(json)
    } else {
        0.0
    }
}

#[no_mangle]
pub extern "C" fn json_equal(value1: *const json_t, value2: *const json_t) -> c_int {
    if value1.is_null() || value2.is_null() {
        return 0;
    }

    let Some(type1) = (unsafe { type_of(value1) }) else {
        return 0;
    };
    let Some(type2) = (unsafe { type_of(value2) }) else {
        return 0;
    };

    if type1 != type2 {
        return 0;
    }

    if value1 == value2 {
        return 1;
    }

    let equal = match type1 {
        JSON_STRING => {
            let string1 = unsafe { &*as_string_ptr(value1) };
            let string2 = unsafe { &*as_string_ptr(value2) };
            if string1.length != string2.length {
                false
            } else {
                let bytes1 =
                    unsafe { slice::from_raw_parts(string1.value.cast::<u8>(), string1.length) };
                let bytes2 =
                    unsafe { slice::from_raw_parts(string2.value.cast::<u8>(), string2.length) };
                bytes1 == bytes2
            }
        }
        JSON_INTEGER => json_integer_value(value1) == json_integer_value(value2),
        JSON_REAL => json_real_value(value1) == json_real_value(value2),
        _ => false,
    };

    bool_to_c_int(equal)
}

#[no_mangle]
pub extern "C" fn json_copy(value: *mut json_t) -> *mut json_t {
    if value.is_null() {
        return null_mut();
    }

    match unsafe { (*value).type_ } {
        JSON_STRING => unsafe { string_clone(value.cast_const()) },
        JSON_INTEGER => unsafe { integer_clone(value.cast_const()) },
        JSON_REAL => unsafe { real_clone(value.cast_const()) },
        JSON_TRUE | JSON_FALSE | JSON_NULL => value,
        _ => null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn json_deep_copy(value: *const json_t) -> *mut json_t {
    if value.is_null() {
        return null_mut();
    }

    match unsafe { (*value).type_ } {
        JSON_STRING => unsafe { string_clone(value) },
        JSON_INTEGER => unsafe { integer_clone(value) },
        JSON_REAL => unsafe { real_clone(value) },
        JSON_TRUE | JSON_FALSE | JSON_NULL => value.cast_mut(),
        _ => null_mut(),
    }
}
