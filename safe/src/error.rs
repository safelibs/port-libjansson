use std::ffi::{c_char, c_int, CStr};
use std::ptr;

use crate::abi::{json_error_t, JSON_ERROR_SOURCE_LENGTH, JSON_ERROR_TEXT_LENGTH};

#[no_mangle]
pub unsafe extern "C" fn jsonp_error_init(error: *mut json_error_t, source: *const c_char) {
    if error.is_null() {
        return;
    }

    unsafe {
        (*error).text[0] = 0;
        (*error).line = -1;
        (*error).column = -1;
        (*error).position = 0;
    }

    if source.is_null() {
        unsafe {
            (*error).source[0] = 0;
        }
    } else {
        unsafe {
            jsonp_error_set_source(error, source);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn jsonp_error_set_source(error: *mut json_error_t, source: *const c_char) {
    if error.is_null() || source.is_null() {
        return;
    }

    let source_bytes = unsafe { CStr::from_ptr(source).to_bytes() };
    let dest = unsafe { &mut (*error).source };

    if source_bytes.len() < JSON_ERROR_SOURCE_LENGTH {
        unsafe {
            ptr::copy_nonoverlapping(
                source_bytes.as_ptr().cast::<c_char>(),
                dest.as_mut_ptr(),
                source_bytes.len(),
            );
        }
        dest[source_bytes.len()] = 0;
        return;
    }

    let extra = source_bytes.len() - JSON_ERROR_SOURCE_LENGTH + 4;
    dest[0] = b'.' as c_char;
    dest[1] = b'.' as c_char;
    dest[2] = b'.' as c_char;

    let suffix = &source_bytes[extra..];
    unsafe {
        ptr::copy_nonoverlapping(
            suffix.as_ptr().cast::<c_char>(),
            dest.as_mut_ptr().add(3),
            suffix.len(),
        );
    }
    dest[JSON_ERROR_SOURCE_LENGTH - 1] = 0;
}

#[no_mangle]
pub unsafe extern "C" fn jsonp_error_vformat(
    error: *mut json_error_t,
    line: c_int,
    column: c_int,
    position: usize,
    code: c_int,
    text: *const c_char,
) {
    if error.is_null() {
        return;
    }

    if unsafe { (*error).text[0] } != 0 {
        return;
    }

    unsafe {
        (*error).line = line;
        (*error).column = column;
        (*error).position = position as c_int;
    }

    let text_bytes = if text.is_null() {
        &[][..]
    } else {
        unsafe { CStr::from_ptr(text).to_bytes() }
    };

    let dest = unsafe { &mut (*error).text };
    let copy_len = text_bytes.len().min(JSON_ERROR_TEXT_LENGTH - 2);

    if copy_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(
                text_bytes.as_ptr().cast::<c_char>(),
                dest.as_mut_ptr(),
                copy_len,
            );
        }
    }

    dest[copy_len] = 0;
    dest[JSON_ERROR_TEXT_LENGTH - 2] = 0;
    dest[JSON_ERROR_TEXT_LENGTH - 1] = code as c_char;
}
