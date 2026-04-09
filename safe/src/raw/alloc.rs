use std::ffi::{c_char, c_void};
use std::mem::size_of;
use std::ptr::{self, null_mut};
use std::sync::RwLock;

use crate::abi::{json_free_t, json_malloc_t};

#[derive(Clone, Copy)]
pub struct AllocFns {
    pub malloc_fn: json_malloc_t,
    pub free_fn: json_free_t,
}

const DEFAULT_ALLOC_FNS: AllocFns = AllocFns {
    malloc_fn: Some(libc::malloc),
    free_fn: Some(libc::free),
};

static ALLOC_FNS: RwLock<AllocFns> = RwLock::new(DEFAULT_ALLOC_FNS);

fn read_alloc_fns() -> AllocFns {
    match ALLOC_FNS.read() {
        Ok(guard) => *guard,
        Err(poisoned) => *poisoned.into_inner(),
    }
}

#[no_mangle]
pub extern "C" fn json_set_alloc_funcs(malloc_fn: json_malloc_t, free_fn: json_free_t) {
    match ALLOC_FNS.write() {
        Ok(mut guard) => {
            *guard = AllocFns { malloc_fn, free_fn };
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            *guard = AllocFns { malloc_fn, free_fn };
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn json_get_alloc_funcs(
    malloc_fn: *mut json_malloc_t,
    free_fn: *mut json_free_t,
) {
    let current = read_alloc_fns();

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

pub unsafe fn jsonp_malloc(size: usize) -> *mut c_void {
    if size == 0 {
        return null_mut();
    }

    match read_alloc_fns().malloc_fn {
        Some(malloc_fn) => unsafe { malloc_fn(size) },
        None => null_mut(),
    }
}

pub unsafe fn jsonp_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }

    if let Some(free_fn) = read_alloc_fns().free_fn {
        unsafe {
            free_fn(ptr);
        }
    }
}

pub unsafe fn jsonp_strdup(value: *const c_char) -> *mut c_char {
    unsafe { jsonp_strndup(value, libc::strlen(value)) }
}

pub unsafe fn jsonp_strndup(value: *const c_char, len: usize) -> *mut c_char {
    let alloc_len = match len.checked_add(1) {
        Some(alloc_len) => alloc_len,
        None => return null_mut(),
    };

    let new_str = unsafe { jsonp_malloc(alloc_len) }.cast::<c_char>();
    if new_str.is_null() {
        return null_mut();
    }

    unsafe {
        ptr::copy_nonoverlapping(value, new_str, len);
        *new_str.add(len) = 0;
    }
    new_str
}

pub unsafe fn alloc<T>() -> *mut T {
    unsafe { jsonp_malloc(size_of::<T>()) }.cast::<T>()
}

pub unsafe fn free<T>(ptr: *mut T) {
    unsafe {
        jsonp_free(ptr.cast::<c_void>());
    }
}
