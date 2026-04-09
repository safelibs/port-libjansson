use std::ffi::c_char;
use std::ptr::{self, null_mut};

use crate::raw::alloc::{jsonp_free, jsonp_malloc};

const STRBUFFER_MIN_SIZE: usize = 16;
const STRBUFFER_FACTOR: usize = 2;
const STRBUFFER_SIZE_MAX: usize = usize::MAX;

#[repr(C)]
pub struct RawBuf {
    size: usize,
    length: usize,
    value: *mut c_char,
}

impl RawBuf {
    pub const fn new() -> Self {
        Self {
            size: 0,
            length: 0,
            value: null_mut(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    #[inline]
    pub fn value(&self) -> *const c_char {
        self.value.cast_const()
    }

    #[inline]
    pub fn value_mut(&mut self) -> *mut c_char {
        self.value
    }

    pub unsafe fn init(&mut self) -> i32 {
        self.size = STRBUFFER_MIN_SIZE;
        self.length = 0;
        self.value = unsafe { jsonp_malloc(self.size) }.cast::<c_char>();
        if self.value.is_null() {
            self.size = 0;
            return -1;
        }

        unsafe {
            *self.value = 0;
        }
        0
    }

    pub unsafe fn close(&mut self) {
        if !self.value.is_null() {
            unsafe {
                jsonp_free(self.value.cast());
            }
        }

        self.size = 0;
        self.length = 0;
        self.value = null_mut();
    }

    pub unsafe fn clear(&mut self) {
        self.length = 0;
        if !self.value.is_null() {
            unsafe {
                *self.value = 0;
            }
        }
    }

    pub unsafe fn steal_value(&mut self) -> *mut c_char {
        let result = self.value;
        self.value = null_mut();
        self.length = 0;
        self.size = 0;
        result
    }

    pub unsafe fn append_byte(&mut self, byte: c_char) -> i32 {
        unsafe { self.append_bytes((&byte as *const c_char).cast::<u8>(), 1) }
    }

    pub unsafe fn append_bytes(&mut self, data: *const u8, size: usize) -> i32 {
        if data.is_null() && size != 0 {
            return -1;
        }

        if size >= self.size.saturating_sub(self.length) {
            if self.size > STRBUFFER_SIZE_MAX / STRBUFFER_FACTOR
                || size > STRBUFFER_SIZE_MAX - 1
                || self.length > STRBUFFER_SIZE_MAX - 1 - size
            {
                return -1;
            }

            let doubled_size = self.size * STRBUFFER_FACTOR;
            let min_size = self.length + size + 1;
            let new_size = doubled_size.max(min_size);
            let new_value = unsafe { jsonp_malloc(new_size) }.cast::<c_char>();
            if new_value.is_null() {
                return -1;
            }

            if self.length != 0 {
                unsafe {
                    ptr::copy_nonoverlapping(self.value, new_value, self.length);
                }
            }

            unsafe {
                jsonp_free(self.value.cast());
            }
            self.value = new_value;
            self.size = new_size;
        }

        if size != 0 {
            unsafe {
                ptr::copy_nonoverlapping(data.cast::<c_char>(), self.value.add(self.length), size);
            }
        }
        self.length += size;
        unsafe {
            *self.value.add(self.length) = 0;
        }

        0
    }

    pub unsafe fn pop(&mut self) -> c_char {
        if self.length == 0 {
            return 0;
        }

        self.length -= 1;
        let byte = unsafe { *self.value.add(self.length) };
        unsafe {
            *self.value.add(self.length) = 0;
        }
        byte
    }
}

pub unsafe fn dup_bytes(data: *const u8, len: usize) -> *mut c_char {
    let Some(alloc_len) = len.checked_add(1) else {
        return null_mut();
    };

    let out = unsafe { jsonp_malloc(alloc_len) }.cast::<c_char>();
    if out.is_null() {
        return null_mut();
    }

    if len != 0 {
        unsafe {
            ptr::copy_nonoverlapping(data.cast::<c_char>(), out, len);
        }
    }
    unsafe {
        *out.add(len) = 0;
    }

    out
}

#[inline]
pub unsafe fn dup_cstr(data: *const c_char, len: usize) -> *mut c_char {
    unsafe { dup_bytes(data.cast::<u8>(), len) }
}
