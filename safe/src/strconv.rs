use std::ffi::c_char;
use std::ptr;

use crate::raw::buf::RawBuf;

const DEFAULT_REAL_PRECISION: i32 = 17;
const DTOSTR_FMT: &[u8] = b"%.*g\0";

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
unsafe fn errno_location() -> *mut libc::c_int {
    unsafe { libc::__error() }
}

#[cfg(windows)]
unsafe fn errno_location() -> *mut libc::c_int {
    unsafe { libc::_errno() }
}

#[inline]
unsafe fn set_errno(value: libc::c_int) {
    unsafe {
        *errno_location() = value;
    }
}

#[inline]
unsafe fn get_errno() -> libc::c_int {
    unsafe { *errno_location() }
}

fn locale_decimal_point() -> Option<u8> {
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", target_os = "freebsd")))]
    {
        None
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos", target_os = "freebsd"))]
    unsafe {
        let conv = libc::localeconv();
        if conv.is_null() || (*conv).decimal_point.is_null() {
            return None;
        }

        let point = *(*conv).decimal_point.cast::<u8>();
        if point == b'.' {
            None
        } else {
            Some(point)
        }
    }
}

pub unsafe fn strtod(strbuffer: &mut RawBuf) -> Result<f64, ()> {
    if let Some(point) = locale_decimal_point() {
        let dot = unsafe { libc::strchr(strbuffer.value(), b'.' as i32) };
        if !dot.is_null() {
            unsafe {
                *dot = point as c_char;
            }
        }
    }

    let mut end = ptr::null_mut();
    unsafe {
        set_errno(0);
    }
    let value = unsafe { libc::strtod(strbuffer.value(), &mut end) };

    if end != unsafe { strbuffer.value_mut().add(strbuffer.len()) } {
        return Err(());
    }

    if let Some(point) = locale_decimal_point() {
        let decimal = unsafe { libc::strchr(strbuffer.value(), point as i32) };
        if !decimal.is_null() {
            unsafe {
                *decimal = b'.' as c_char;
            }
        }
    }

    let errno = unsafe { get_errno() };
    if errno == libc::ERANGE && value.is_infinite() {
        return Err(());
    }

    Ok(value)
}

fn strip_positive_exponent_sign_and_zeroes(buffer: &mut [u8], mut length: usize) -> usize {
    let Some(exp_index) = buffer[..length].iter().position(|&byte| byte == b'e') else {
        return length;
    };

    let mut start = exp_index + 1;
    let mut end = start + 1;

    if buffer.get(start) == Some(&b'-') {
        start += 1;
    }

    while end < length && buffer[end] == b'0' {
        end += 1;
    }

    if end != start {
        let move_len = length - end + 1;
        buffer.copy_within(end..end + move_len, start);
        length -= end - start;
    }

    length
}

pub fn dtostr(buffer: &mut [u8], value: f64, precision: i32) -> Result<usize, ()> {
    let precision = if precision == 0 {
        DEFAULT_REAL_PRECISION
    } else {
        precision
    };

    let written = unsafe {
        libc::snprintf(
            buffer.as_mut_ptr().cast::<c_char>(),
            buffer.len(),
            DTOSTR_FMT.as_ptr().cast::<c_char>(),
            precision,
            value,
        )
    };
    if written < 0 {
        return Err(());
    }

    let mut length = written as usize;
    if length >= buffer.len() {
        return Err(());
    }

    if let Some(point) = locale_decimal_point() {
        if let Some(index) = buffer[..length].iter().position(|&byte| byte == point) {
            buffer[index] = b'.';
        }
    }

    if !buffer[..length].contains(&b'.') && !buffer[..length].contains(&b'e') {
        if length + 3 >= buffer.len() {
            return Err(());
        }

        buffer[length] = b'.';
        buffer[length + 1] = b'0';
        buffer[length + 2] = 0;
        length += 2;
    }

    Ok(strip_positive_exponent_sign_and_zeroes(buffer, length))
}
