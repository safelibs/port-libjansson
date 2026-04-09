use std::cmp::max;
use std::ffi::c_int;
use std::mem::size_of;
use std::ptr::{self, null_mut};

use crate::abi::{decref, incref, is_array, json_t, JSON_ARRAY};
use crate::raw::alloc::{alloc, free, jsonp_free, jsonp_malloc};
use crate::raw::table::PointerSet;
use crate::scalar::init_json;

const INITIAL_ARRAY_CAPACITY: usize = 8;

#[repr(C)]
struct JsonArray {
    json: json_t,
    size: usize,
    entries: usize,
    table: *mut *mut json_t,
}

#[inline]
unsafe fn as_array_ptr(json: *const json_t) -> *const JsonArray {
    json.cast::<JsonArray>()
}

#[inline]
unsafe fn as_array_mut(json: *mut json_t) -> *mut JsonArray {
    json.cast::<JsonArray>()
}

unsafe fn array_move(array: *mut JsonArray, dest: usize, src: usize, count: usize) {
    if count == 0 {
        return;
    }

    unsafe {
        ptr::copy((*array).table.add(src), (*array).table.add(dest), count);
    }
}

unsafe fn array_copy(
    dest: *mut *mut json_t,
    dpos: usize,
    src: *mut *mut json_t,
    spos: usize,
    count: usize,
) {
    if count == 0 {
        return;
    }

    unsafe {
        ptr::copy_nonoverlapping(src.add(spos), dest.add(dpos), count);
    }
}

unsafe fn array_grow(array: *mut JsonArray, amount: usize, copy_existing: bool) -> *mut *mut json_t {
    let Some(required_entries) = (unsafe { (*array).entries.checked_add(amount) }) else {
        return null_mut();
    };

    if required_entries <= unsafe { (*array).size } {
        return unsafe { (*array).table };
    }

    let Some(grown_size) = (unsafe { (*array).size.checked_add(amount) }) else {
        return null_mut();
    };
    let Some(doubled_size) = (unsafe { (*array).size.checked_mul(2) }) else {
        return null_mut();
    };
    let new_size = max(grown_size, doubled_size);
    let Some(alloc_size) = new_size.checked_mul(size_of::<*mut json_t>()) else {
        return null_mut();
    };

    let old_table = unsafe { (*array).table };
    let new_table = unsafe { jsonp_malloc(alloc_size) }.cast::<*mut json_t>();
    if new_table.is_null() {
        return null_mut();
    }

    unsafe {
        (*array).size = new_size;
        (*array).table = new_table;
    }

    if copy_existing {
        unsafe {
            array_copy(new_table, 0, old_table, 0, (*array).entries);
            jsonp_free(old_table.cast());
        }
        new_table
    } else {
        old_table
    }
}

unsafe fn append_borrowed(array: *mut json_t, value: *mut json_t) -> c_int {
    let borrowed = unsafe { incref(value) };
    json_array_append_new(array, borrowed)
}

pub(crate) unsafe fn delete_array(json: *mut json_t) {
    let array = unsafe { as_array_mut(json) };
    let mut index = 0usize;
    while index < unsafe { (*array).entries } {
        unsafe {
            decref(*(*array).table.add(index));
        }
        index += 1;
    }

    unsafe {
        jsonp_free((*array).table.cast());
        free(array);
    }
}

pub(crate) unsafe fn equal_array(array1: *const json_t, array2: *const json_t) -> bool {
    let size = json_array_size(array1);
    if size != json_array_size(array2) {
        return false;
    }

    let mut index = 0usize;
    while index < size {
        let value1 = json_array_get(array1, index);
        let value2 = json_array_get(array2, index);
        if crate::scalar::json_equal(value1.cast_const(), value2.cast_const()) == 0 {
            return false;
        }
        index += 1;
    }

    true
}

pub(crate) unsafe fn copy_array(array: *mut json_t) -> *mut json_t {
    let result = json_array();
    if result.is_null() {
        return null_mut();
    }

    let mut index = 0usize;
    let size = json_array_size(array.cast_const());
    while index < size {
        if unsafe { append_borrowed(result, json_array_get(array.cast_const(), index)) } != 0 {
            unsafe {
                decref(result);
            }
            return null_mut();
        }
        index += 1;
    }

    result
}

pub(crate) unsafe fn deep_copy_array(array: *const json_t, parents: &mut PointerSet) -> *mut json_t {
    if unsafe { parents.contains(array.cast()) } {
        return null_mut();
    }
    if unsafe { parents.insert(array.cast()) } != 0 {
        return null_mut();
    }

    let result = json_array();
    if result.is_null() {
        unsafe {
            parents.remove(array.cast());
        }
        return null_mut();
    }

    let mut copied = result;
    let mut index = 0usize;
    let size = json_array_size(array);
    while index < size {
        let item = json_array_get(array, index);
        let deep = unsafe { crate::scalar::do_deep_copy(item.cast_const(), parents) };
        if deep.is_null() || json_array_append_new(copied, deep) != 0 {
            unsafe {
                decref(copied);
                copied = null_mut();
            }
            break;
        }
        index += 1;
    }

    unsafe {
        parents.remove(array.cast());
    }
    copied
}

#[no_mangle]
pub extern "C" fn json_array() -> *mut json_t {
    let array = unsafe { alloc::<JsonArray>() };
    if array.is_null() {
        return null_mut();
    }

    unsafe {
        init_json(ptr::addr_of_mut!((*array).json), JSON_ARRAY);
        (*array).entries = 0;
        (*array).size = INITIAL_ARRAY_CAPACITY;
    }

    let Some(alloc_size) = INITIAL_ARRAY_CAPACITY.checked_mul(size_of::<*mut json_t>()) else {
        unsafe {
            free(array);
        }
        return null_mut();
    };
    let table = unsafe { jsonp_malloc(alloc_size) }.cast::<*mut json_t>();
    if table.is_null() {
        unsafe {
            free(array);
        }
        return null_mut();
    }

    unsafe {
        (*array).table = table;
        ptr::addr_of_mut!((*array).json)
    }
}

#[no_mangle]
pub extern "C" fn json_array_size(array: *const json_t) -> usize {
    if !unsafe { is_array(array) } {
        return 0;
    }

    unsafe { (*as_array_ptr(array)).entries }
}

#[no_mangle]
pub extern "C" fn json_array_get(array: *const json_t, index: usize) -> *mut json_t {
    if !unsafe { is_array(array) } {
        return null_mut();
    }

    let array = unsafe { as_array_ptr(array) };
    if index >= unsafe { (*array).entries } {
        return null_mut();
    }

    unsafe { *(*array).table.add(index) }
}

#[no_mangle]
pub extern "C" fn json_array_set_new(array: *mut json_t, index: usize, value: *mut json_t) -> c_int {
    if value.is_null() {
        return -1;
    }

    if !unsafe { is_array(array) } || array == value {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let array_ptr = unsafe { as_array_mut(array) };
    if index >= unsafe { (*array_ptr).entries } {
        unsafe {
            decref(value);
        }
        return -1;
    }

    unsafe {
        decref(*(*array_ptr).table.add(index));
        *(*array_ptr).table.add(index) = value;
    }

    0
}

#[no_mangle]
pub extern "C" fn json_array_append_new(array: *mut json_t, value: *mut json_t) -> c_int {
    if value.is_null() {
        return -1;
    }

    if !unsafe { is_array(array) } || array == value {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let array_ptr = unsafe { as_array_mut(array) };
    if unsafe { array_grow(array_ptr, 1, true) }.is_null() {
        unsafe {
            decref(value);
        }
        return -1;
    }

    unsafe {
        *(*array_ptr).table.add((*array_ptr).entries) = value;
        (*array_ptr).entries += 1;
    }
    0
}

#[no_mangle]
pub extern "C" fn json_array_insert_new(
    array: *mut json_t,
    index: usize,
    value: *mut json_t,
) -> c_int {
    if value.is_null() {
        return -1;
    }

    if !unsafe { is_array(array) } || array == value {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let array_ptr = unsafe { as_array_mut(array) };
    if index > unsafe { (*array_ptr).entries } {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let old_table = unsafe { array_grow(array_ptr, 1, false) };
    if old_table.is_null() {
        unsafe {
            decref(value);
        }
        return -1;
    }

    if old_table != unsafe { (*array_ptr).table } {
        unsafe {
            array_copy((*array_ptr).table, 0, old_table, 0, index);
            array_copy(
                (*array_ptr).table,
                index + 1,
                old_table,
                index,
                (*array_ptr).entries - index,
            );
            jsonp_free(old_table.cast());
        }
    } else {
        unsafe {
            array_move(array_ptr, index + 1, index, (*array_ptr).entries - index);
        }
    }

    unsafe {
        *(*array_ptr).table.add(index) = value;
        (*array_ptr).entries += 1;
    }

    0
}

#[no_mangle]
pub extern "C" fn json_array_remove(array: *mut json_t, index: usize) -> c_int {
    if !unsafe { is_array(array) } {
        return -1;
    }

    let array_ptr = unsafe { as_array_mut(array) };
    if index >= unsafe { (*array_ptr).entries } {
        return -1;
    }

    unsafe {
        decref(*(*array_ptr).table.add(index));
        if index < (*array_ptr).entries - 1 {
            array_move(array_ptr, index, index + 1, (*array_ptr).entries - index - 1);
        }
        (*array_ptr).entries -= 1;
        *(*array_ptr).table.add((*array_ptr).entries) = null_mut();
    }

    0
}

#[no_mangle]
pub extern "C" fn json_array_clear(array: *mut json_t) -> c_int {
    if !unsafe { is_array(array) } {
        return -1;
    }

    let array_ptr = unsafe { as_array_mut(array) };
    let mut index = 0usize;
    while index < unsafe { (*array_ptr).entries } {
        unsafe {
            decref(*(*array_ptr).table.add(index));
            *(*array_ptr).table.add(index) = null_mut();
        }
        index += 1;
    }

    unsafe {
        (*array_ptr).entries = 0;
    }
    0
}

#[no_mangle]
pub extern "C" fn json_array_extend(array: *mut json_t, other: *mut json_t) -> c_int {
    if !unsafe { is_array(array) } || !unsafe { is_array(other) } {
        return -1;
    }

    let array_ptr = unsafe { as_array_mut(array) };
    let other_ptr = unsafe { as_array_mut(other) };
    if unsafe { array_grow(array_ptr, (*other_ptr).entries, true) }.is_null() {
        return -1;
    }

    let start = unsafe { (*array_ptr).entries };
    let mut index = 0usize;
    while index < unsafe { (*other_ptr).entries } {
        unsafe {
            let value = *(*other_ptr).table.add(index);
            incref(value);
            *(*array_ptr).table.add(start + index) = value;
        }
        index += 1;
    }

    unsafe {
        (*array_ptr).entries += (*other_ptr).entries;
    }
    0
}
