use std::ffi::{c_char, c_int, c_void};
use std::mem::size_of;
use std::ptr::{self, null_mut};
use std::slice;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::abi::{decref, incref, is_object, json_t, JSON_OBJECT};
use crate::raw::alloc::{alloc, free, jsonp_free, jsonp_malloc};
use crate::raw::list::{self, RawListNode};
use crate::raw::table::{PointerSet, RawBucket, RawTable};
use crate::scalar::init_json;
use crate::utf;

static HASH_SEED: AtomicU32 = AtomicU32::new(0);
static HASH_SEED_INITIALIZING: AtomicBool = AtomicBool::new(false);

#[repr(C)]
pub(crate) struct JsonObject {
    json: json_t,
    table: RawTable,
    ordered: RawListNode,
}

#[repr(C)]
struct ObjectEntry {
    bucket_link: RawListNode,
    order_link: RawListNode,
    hash: usize,
    value: *mut json_t,
    key_len: usize,
}

const ORDER_LINK_OFFSET: usize = size_of::<RawListNode>();
const KEY_OFFSET: usize = size_of::<ObjectEntry>();

#[inline]
unsafe fn as_object_ptr(json: *const json_t) -> *const JsonObject {
    json.cast::<JsonObject>()
}

#[inline]
unsafe fn as_object_mut(json: *mut json_t) -> *mut JsonObject {
    json.cast::<JsonObject>()
}

#[inline]
unsafe fn entry_from_bucket_link(link: *mut RawListNode) -> *mut ObjectEntry {
    link.cast::<ObjectEntry>()
}

#[inline]
unsafe fn entry_from_order_link(link: *mut RawListNode) -> *mut ObjectEntry {
    unsafe { link.cast::<u8>().sub(ORDER_LINK_OFFSET).cast::<ObjectEntry>() }
}

#[inline]
unsafe fn entry_from_key(key: *const c_char) -> *mut ObjectEntry {
    unsafe { key.cast_mut().cast::<u8>().sub(KEY_OFFSET).cast::<ObjectEntry>() }
}

#[inline]
unsafe fn entry_key_ptr(entry: *const ObjectEntry) -> *const c_char {
    unsafe { entry.cast::<u8>().add(KEY_OFFSET).cast::<c_char>() }
}

#[inline]
unsafe fn entry_key_bytes<'a>(entry: *const ObjectEntry) -> &'a [u8] {
    unsafe { slice::from_raw_parts(entry_key_ptr(entry).cast::<u8>(), (*entry).key_len) }
}

#[inline]
unsafe fn ordered_first(object: *mut JsonObject) -> *mut RawListNode {
    let head = unsafe { ptr::addr_of_mut!((*object).ordered) };
    let next = unsafe { (*head).next };
    if next == head {
        null_mut()
    } else {
        next
    }
}

#[inline]
unsafe fn ordered_next(object: *mut JsonObject, iter: *mut RawListNode) -> *mut RawListNode {
    let head = unsafe { ptr::addr_of_mut!((*object).ordered) };
    let next = unsafe { (*iter).next };
    if next == head {
        null_mut()
    } else {
        next
    }
}

#[inline]
fn rot(x: u32, k: u32) -> u32 {
    x.rotate_left(k)
}

#[inline]
fn mix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *a = a.wrapping_sub(*c);
    *a ^= rot(*c, 4);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= rot(*a, 6);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= rot(*b, 8);
    *b = b.wrapping_add(*a);
    *a = a.wrapping_sub(*c);
    *a ^= rot(*c, 16);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= rot(*a, 19);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= rot(*b, 4);
    *b = b.wrapping_add(*a);
}

#[inline]
fn final_mix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *c ^= *b;
    *c = c.wrapping_sub(rot(*b, 14));
    *a ^= *c;
    *a = a.wrapping_sub(rot(*c, 11));
    *b ^= *a;
    *b = b.wrapping_sub(rot(*a, 25));
    *c ^= *b;
    *c = c.wrapping_sub(rot(*b, 16));
    *a ^= *c;
    *a = a.wrapping_sub(rot(*c, 4));
    *b ^= *a;
    *b = b.wrapping_sub(rot(*a, 14));
    *c ^= *b;
    *c = c.wrapping_sub(rot(*b, 24));
}

fn hashlittle(bytes: &[u8], seed: u32) -> u32 {
    let mut a = 0xdeadbeef_u32
        .wrapping_add(bytes.len() as u32)
        .wrapping_add(seed);
    let mut b = a;
    let mut c = a;

    let mut index = 0usize;
    while bytes.len().saturating_sub(index) > 12 {
        a = a.wrapping_add(u32::from_le_bytes([
            bytes[index],
            bytes[index + 1],
            bytes[index + 2],
            bytes[index + 3],
        ]));
        b = b.wrapping_add(u32::from_le_bytes([
            bytes[index + 4],
            bytes[index + 5],
            bytes[index + 6],
            bytes[index + 7],
        ]));
        c = c.wrapping_add(u32::from_le_bytes([
            bytes[index + 8],
            bytes[index + 9],
            bytes[index + 10],
            bytes[index + 11],
        ]));
        mix(&mut a, &mut b, &mut c);
        index += 12;
    }

    let tail = &bytes[index..];
    match tail.len() {
        12 => {
            c = c.wrapping_add(u32::from_le_bytes([tail[8], tail[9], tail[10], tail[11]]));
            b = b.wrapping_add(u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]));
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        11 => {
            c = c.wrapping_add((tail[10] as u32) << 16);
            c = c.wrapping_add((tail[9] as u32) << 8);
            c = c.wrapping_add(tail[8] as u32);
            b = b.wrapping_add(u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]));
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        10 => {
            c = c.wrapping_add((tail[9] as u32) << 8);
            c = c.wrapping_add(tail[8] as u32);
            b = b.wrapping_add(u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]));
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        9 => {
            c = c.wrapping_add(tail[8] as u32);
            b = b.wrapping_add(u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]));
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        8 => {
            b = b.wrapping_add(u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]));
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        7 => {
            b = b.wrapping_add((tail[6] as u32) << 16);
            b = b.wrapping_add((tail[5] as u32) << 8);
            b = b.wrapping_add(tail[4] as u32);
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        6 => {
            b = b.wrapping_add((tail[5] as u32) << 8);
            b = b.wrapping_add(tail[4] as u32);
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        5 => {
            b = b.wrapping_add(tail[4] as u32);
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        4 => {
            a = a.wrapping_add(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]));
        }
        3 => {
            a = a.wrapping_add((tail[2] as u32) << 16);
            a = a.wrapping_add((tail[1] as u32) << 8);
            a = a.wrapping_add(tail[0] as u32);
        }
        2 => {
            a = a.wrapping_add((tail[1] as u32) << 8);
            a = a.wrapping_add(tail[0] as u32);
        }
        1 => {
            a = a.wrapping_add(tail[0] as u32);
        }
        0 => return c,
        _ => unreachable!(),
    }

    final_mix(&mut a, &mut b, &mut c);
    c
}

fn seed_from_urandom() -> Option<u32> {
    #[cfg(unix)]
    {
        let mut data = [0u8; 4];
        let file = std::fs::File::open("/dev/urandom").ok()?;
        let mut reader = std::io::BufReader::new(file);
        use std::io::Read;
        reader.read_exact(&mut data).ok()?;
        Some(u32::from_be_bytes(data))
    }

    #[cfg(not(unix))]
    {
        None
    }
}

fn seed_from_timestamp_and_pid() -> u32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut seed = (now.as_secs() as u32) ^ now.subsec_micros();

    #[cfg(unix)]
    {
        unsafe {
            seed ^= libc::getpid() as u32;
        }
    }

    #[cfg(windows)]
    {
        seed ^= std::process::id();
    }

    seed
}

fn generate_seed() -> u32 {
    let mut seed = seed_from_urandom().unwrap_or_else(seed_from_timestamp_and_pid);
    if seed == 0 {
        seed = 1;
    }
    seed
}

fn current_seed() -> u32 {
    HASH_SEED.load(Ordering::Acquire)
}

fn ensure_seed(explicit_seed: Option<u32>) {
    if current_seed() != 0 {
        return;
    }

    if HASH_SEED_INITIALIZING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let mut seed = explicit_seed.unwrap_or(0);
        if seed == 0 {
            seed = generate_seed();
        }
        HASH_SEED.store(seed, Ordering::Release);
        HASH_SEED_INITIALIZING.store(false, Ordering::Release);
        return;
    }

    while current_seed() == 0 {
        thread::yield_now();
    }
}

#[cfg(test)]
pub(crate) fn reset_seed_state_for_tests() {
    HASH_SEED.store(0, Ordering::Release);
    HASH_SEED_INITIALIZING.store(false, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn current_seed_for_tests() -> u32 {
    current_seed()
}

#[inline]
fn hash_key_bytes(bytes: &[u8]) -> usize {
    let seed = current_seed();
    hashlittle(bytes, if seed == 0 { 1 } else { seed }) as usize
}

unsafe fn hash_key(key: *const c_char, key_len: usize) -> usize {
    unsafe { hash_key_bytes(slice::from_raw_parts(key.cast::<u8>(), key_len)) }
}

unsafe fn entry_matches(entry: *const ObjectEntry, key: *const c_char, key_len: usize, hash: usize) -> bool {
    if unsafe { (*entry).hash != hash || (*entry).key_len != key_len } {
        return false;
    }

    unsafe { entry_key_bytes(entry) == slice::from_raw_parts(key.cast::<u8>(), key_len) }
}

unsafe fn find_entry(
    object: *mut JsonObject,
    key: *const c_char,
    key_len: usize,
    hash: usize,
) -> (*mut RawBucket, *mut ObjectEntry) {
    let bucket = unsafe { (*object).table.bucket_mut(hash) };
    let node = unsafe {
        (*object)
            .table
            .find_in_bucket(bucket, |candidate| entry_matches(entry_from_bucket_link(candidate), key, key_len, hash))
    };

    (bucket, if node.is_null() { null_mut() } else { unsafe { entry_from_bucket_link(node) } })
}

unsafe fn allocate_entry(
    key: *const c_char,
    key_len: usize,
    hash: usize,
    value: *mut json_t,
) -> *mut ObjectEntry {
    let Some(total_size) = KEY_OFFSET.checked_add(key_len).and_then(|n| n.checked_add(1)) else {
        return null_mut();
    };

    let entry = unsafe { jsonp_malloc(total_size) }.cast::<ObjectEntry>();
    if entry.is_null() {
        return null_mut();
    }

    unsafe {
        (*entry).bucket_link = RawListNode::new();
        (*entry).order_link = RawListNode::new();
        list::init(ptr::addr_of_mut!((*entry).bucket_link));
        list::init(ptr::addr_of_mut!((*entry).order_link));
        (*entry).hash = hash;
        (*entry).value = value;
        (*entry).key_len = key_len;
        if key_len != 0 {
            ptr::copy_nonoverlapping(key, entry_key_ptr(entry).cast_mut(), key_len);
        }
        *entry_key_ptr(entry).cast_mut().add(key_len) = 0;
    }

    entry
}

unsafe fn free_entry(entry: *mut ObjectEntry) {
    unsafe {
        decref((*entry).value);
        jsonp_free(entry.cast());
    }
}

unsafe fn getn_inner(object: *mut json_t, key: *const c_char, key_len: usize) -> *mut json_t {
    let object_ptr = unsafe { as_object_mut(object) };
    let hash = unsafe { hash_key(key, key_len) };
    let (_, entry) = unsafe { find_entry(object_ptr, key, key_len, hash) };
    if entry.is_null() {
        null_mut()
    } else {
        unsafe { (*entry).value }
    }
}

unsafe fn set_new_nocheck_inner(
    object: *mut json_t,
    key: *const c_char,
    key_len: usize,
    value: *mut json_t,
) -> c_int {
    if value.is_null() {
        return -1;
    }

    if key.is_null() || !unsafe { is_object(object) } || object == value {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let object_ptr = unsafe { as_object_mut(object) };
    if unsafe {
        (*object_ptr)
            .table
            .ensure_capacity(|node| (*entry_from_bucket_link(node)).hash)
    } != 0
    {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let hash = unsafe { hash_key(key, key_len) };
    let (bucket, existing) = unsafe { find_entry(object_ptr, key, key_len, hash) };
    if !existing.is_null() {
        unsafe {
            decref((*existing).value);
            (*existing).value = value;
        }
        return 0;
    }

    let entry = unsafe { allocate_entry(key, key_len, hash, value) };
    if entry.is_null() {
        unsafe {
            decref(value);
        }
        return -1;
    }

    unsafe {
        (*object_ptr)
            .table
            .insert_into_bucket(bucket, ptr::addr_of_mut!((*entry).bucket_link));
        list::insert_before(
            ptr::addr_of_mut!((*object_ptr).ordered),
            ptr::addr_of_mut!((*entry).order_link),
        );
    }
    0
}

unsafe fn set_borrowed_nocheck(
    object: *mut json_t,
    key: *const c_char,
    key_len: usize,
    value: *mut json_t,
) -> c_int {
    let borrowed = unsafe { incref(value) };
    unsafe { set_new_nocheck_inner(object, key, key_len, borrowed) }
}

unsafe fn set_borrowed_cstr_nocheck(
    object: *mut json_t,
    key: *const c_char,
    value: *mut json_t,
) -> c_int {
    let key_len = unsafe { libc::strlen(key) };
    unsafe { set_borrowed_nocheck(object, key, key_len, value) }
}

unsafe fn delete_inner(object: *mut JsonObject) {
    unsafe {
        (*object).table.close(|node| {
            let entry = entry_from_bucket_link(node);
            list::remove(ptr::addr_of_mut!((*entry).order_link));
            free_entry(entry);
        });
        free(object);
    }
}

pub(crate) unsafe fn delete_object(json: *mut json_t) {
    unsafe {
        delete_inner(as_object_mut(json));
    }
}

pub(crate) unsafe fn equal_object(object1: *const json_t, object2: *const json_t) -> bool {
    if json_object_size(object1) != json_object_size(object2) {
        return false;
    }

    let object1_ptr = unsafe { as_object_ptr(object1).cast_mut() };
    let mut iter = unsafe { ordered_first(object1_ptr) };
    while !iter.is_null() {
        let entry = unsafe { entry_from_order_link(iter) };
        let value2 = unsafe { json_object_get(object2, entry_key_ptr(entry)) };
        if crate::scalar::json_equal(unsafe { (*entry).value }.cast_const(), value2.cast_const()) == 0 {
            return false;
        }
        iter = unsafe { ordered_next(object1_ptr, iter) };
    }

    true
}

pub(crate) unsafe fn copy_object(object: *mut json_t) -> *mut json_t {
    let result = json_object();
    if result.is_null() {
        return null_mut();
    }

    let object_ptr = unsafe { as_object_mut(object) };
    let mut iter = unsafe { ordered_first(object_ptr) };
    while !iter.is_null() {
        let entry = unsafe { entry_from_order_link(iter) };
        if unsafe { set_borrowed_cstr_nocheck(result, entry_key_ptr(entry), (*entry).value) } != 0 {
            unsafe {
                decref(result);
            }
            return null_mut();
        }
        iter = unsafe { ordered_next(object_ptr, iter) };
    }

    result
}

pub(crate) unsafe fn deep_copy_object(
    object: *const json_t,
    parents: &mut PointerSet,
) -> *mut json_t {
    if unsafe { parents.contains(object.cast()) } {
        return null_mut();
    }
    if unsafe { parents.insert(object.cast()) } != 0 {
        return null_mut();
    }

    let result = json_object();
    if result.is_null() {
        unsafe {
            parents.remove(object.cast());
        }
        return null_mut();
    }

    let object_ptr = unsafe { as_object_ptr(object).cast_mut() };
    let mut iter = unsafe { ordered_first(object_ptr) };
    let mut copied = result;
    while !iter.is_null() {
        let entry = unsafe { entry_from_order_link(iter) };
        let deep = unsafe { crate::scalar::do_deep_copy((*entry).value.cast_const(), parents) };
        if deep.is_null() || unsafe { json_object_set_new_nocheck(copied, entry_key_ptr(entry), deep) } != 0 {
            unsafe {
                decref(copied);
                copied = null_mut();
            }
            break;
        }
        iter = unsafe { ordered_next(object_ptr, iter) };
    }

    unsafe {
        parents.remove(object.cast());
    }
    copied
}

unsafe fn update_recursive_inner(
    object: *mut json_t,
    other: *mut json_t,
    parents: &mut PointerSet,
) -> c_int {
    if !unsafe { is_object(object) } || !unsafe { is_object(other) } {
        return -1;
    }

    if unsafe { parents.contains(other.cast()) } {
        return -1;
    }
    if unsafe { parents.insert(other.cast()) } != 0 {
        return -1;
    }

    let other_ptr = unsafe { as_object_mut(other) };
    let mut iter = unsafe { ordered_first(other_ptr) };
    let mut result = 0;
    while !iter.is_null() {
        let entry = unsafe { entry_from_order_link(iter) };
        let current = unsafe { getn_inner(object, entry_key_ptr(entry), (*entry).key_len) };
        if unsafe { is_object(current) } && unsafe { is_object((*entry).value) } {
            if unsafe { update_recursive_inner(current, (*entry).value, parents) } != 0 {
                result = -1;
                break;
            }
        } else if unsafe {
            set_borrowed_nocheck(object, entry_key_ptr(entry), (*entry).key_len, (*entry).value)
        } != 0
        {
            result = -1;
            break;
        }
        iter = unsafe { ordered_next(other_ptr, iter) };
    }

    unsafe {
        parents.remove(other.cast());
    }
    result
}

#[no_mangle]
pub extern "C" fn json_object() -> *mut json_t {
    let object = unsafe { alloc::<JsonObject>() };
    if object.is_null() {
        return null_mut();
    }

    ensure_seed(None);

    unsafe {
        init_json(ptr::addr_of_mut!((*object).json), JSON_OBJECT);
        (*object).table = RawTable::new();
        (*object).ordered = RawListNode::new();
        list::init(ptr::addr_of_mut!((*object).ordered));
    }

    if unsafe { (*object).table.init() } != 0 {
        unsafe {
            free(object);
        }
        return null_mut();
    }

    unsafe { ptr::addr_of_mut!((*object).json) }
}

#[no_mangle]
pub extern "C" fn json_object_seed(seed: usize) {
    ensure_seed(Some(seed as u32));
}

#[no_mangle]
pub extern "C" fn json_object_size(object: *const json_t) -> usize {
    if !unsafe { is_object(object) } {
        return 0;
    }

    unsafe { (*as_object_ptr(object)).table.size }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_get(object: *const json_t, key: *const c_char) -> *mut json_t {
    if key.is_null() {
        return null_mut();
    }

    let key_len = unsafe { libc::strlen(key) };
    unsafe { json_object_getn(object, key, key_len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_getn(
    object: *const json_t,
    key: *const c_char,
    key_len: usize,
) -> *mut json_t {
    if key.is_null() || !unsafe { is_object(object) } {
        return null_mut();
    }

    unsafe { getn_inner(object.cast_mut(), key, key_len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_set_new(
    object: *mut json_t,
    key: *const c_char,
    value: *mut json_t,
) -> c_int {
    if key.is_null() {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let key_len = unsafe { libc::strlen(key) };
    unsafe { json_object_setn_new(object, key, key_len, value) }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_setn_new(
    object: *mut json_t,
    key: *const c_char,
    key_len: usize,
    value: *mut json_t,
) -> c_int {
    if key.is_null() || !unsafe { utf::validate_ptr(key, key_len) } {
        unsafe {
            decref(value);
        }
        return -1;
    }

    unsafe { json_object_setn_new_nocheck(object, key, key_len, value) }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_set_new_nocheck(
    object: *mut json_t,
    key: *const c_char,
    value: *mut json_t,
) -> c_int {
    if key.is_null() {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let key_len = unsafe { libc::strlen(key) };
    unsafe { json_object_setn_new_nocheck(object, key, key_len, value) }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_setn_new_nocheck(
    object: *mut json_t,
    key: *const c_char,
    key_len: usize,
    value: *mut json_t,
) -> c_int {
    unsafe { set_new_nocheck_inner(object, key, key_len, value) }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_del(object: *mut json_t, key: *const c_char) -> c_int {
    if key.is_null() {
        return -1;
    }

    let key_len = unsafe { libc::strlen(key) };
    unsafe { json_object_deln(object, key, key_len) }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_deln(
    object: *mut json_t,
    key: *const c_char,
    key_len: usize,
) -> c_int {
    if key.is_null() || !unsafe { is_object(object) } {
        return -1;
    }

    let object_ptr = unsafe { as_object_mut(object) };
    let hash = unsafe { hash_key(key, key_len) };
    let (bucket, entry) = unsafe { find_entry(object_ptr, key, key_len, hash) };
    if entry.is_null() {
        return -1;
    }

    unsafe {
        (*object_ptr)
            .table
            .remove_from_bucket(bucket, ptr::addr_of_mut!((*entry).bucket_link));
        list::remove(ptr::addr_of_mut!((*entry).order_link));
        free_entry(entry);
    }

    0
}

#[no_mangle]
pub extern "C" fn json_object_clear(object: *mut json_t) -> c_int {
    if !unsafe { is_object(object) } {
        return -1;
    }

    let object_ptr = unsafe { as_object_mut(object) };
    unsafe {
        (*object_ptr).table.clear(|node| {
            let entry = entry_from_bucket_link(node);
            list::remove(ptr::addr_of_mut!((*entry).order_link));
            free_entry(entry);
        });
        list::init(ptr::addr_of_mut!((*object_ptr).ordered));
    }

    0
}

#[no_mangle]
pub extern "C" fn json_object_update(object: *mut json_t, other: *mut json_t) -> c_int {
    if !unsafe { is_object(object) } || !unsafe { is_object(other) } {
        return -1;
    }

    let other_ptr = unsafe { as_object_mut(other) };
    let mut iter = unsafe { ordered_first(other_ptr) };
    while !iter.is_null() {
        let entry = unsafe { entry_from_order_link(iter) };
        if unsafe { set_borrowed_cstr_nocheck(object, entry_key_ptr(entry), (*entry).value) } != 0 {
            return -1;
        }
        iter = unsafe { ordered_next(other_ptr, iter) };
    }

    0
}

#[no_mangle]
pub extern "C" fn json_object_update_existing(object: *mut json_t, other: *mut json_t) -> c_int {
    if !unsafe { is_object(object) } || !unsafe { is_object(other) } {
        return -1;
    }

    let other_ptr = unsafe { as_object_mut(other) };
    let mut iter = unsafe { ordered_first(other_ptr) };
    while !iter.is_null() {
        let entry = unsafe { entry_from_order_link(iter) };
        if !unsafe { getn_inner(object, entry_key_ptr(entry), (*entry).key_len) }.is_null()
            && unsafe {
                set_borrowed_nocheck(object, entry_key_ptr(entry), (*entry).key_len, (*entry).value)
            } != 0
        {
            return -1;
        }
        iter = unsafe { ordered_next(other_ptr, iter) };
    }

    0
}

#[no_mangle]
pub extern "C" fn json_object_update_missing(object: *mut json_t, other: *mut json_t) -> c_int {
    if !unsafe { is_object(object) } || !unsafe { is_object(other) } {
        return -1;
    }

    let other_ptr = unsafe { as_object_mut(other) };
    let mut iter = unsafe { ordered_first(other_ptr) };
    while !iter.is_null() {
        let entry = unsafe { entry_from_order_link(iter) };
        if unsafe { json_object_get(object.cast_const(), entry_key_ptr(entry)) }.is_null()
            && unsafe { set_borrowed_cstr_nocheck(object, entry_key_ptr(entry), (*entry).value) } != 0
        {
            return -1;
        }
        iter = unsafe { ordered_next(other_ptr, iter) };
    }

    0
}

#[no_mangle]
pub extern "C" fn json_object_update_recursive(object: *mut json_t, other: *mut json_t) -> c_int {
    let mut parents = PointerSet::new();
    if unsafe { parents.init() } != 0 {
        return -1;
    }

    let result = unsafe { update_recursive_inner(object, other, &mut parents) };
    unsafe {
        parents.close();
    }
    result
}

#[no_mangle]
pub extern "C" fn json_object_iter(object: *mut json_t) -> *mut c_void {
    if !unsafe { is_object(object) } {
        return null_mut();
    }

    unsafe { ordered_first(as_object_mut(object)).cast::<c_void>() }
}

#[no_mangle]
pub unsafe extern "C" fn json_object_iter_at(
    object: *mut json_t,
    key: *const c_char,
) -> *mut c_void {
    if key.is_null() || !unsafe { is_object(object) } {
        return null_mut();
    }

    let key_len = unsafe { libc::strlen(key) };
    let hash = unsafe { hash_key(key, key_len) };
    let (_, entry) = unsafe { find_entry(as_object_mut(object), key, key_len, hash) };
    if entry.is_null() {
        null_mut()
    } else {
        unsafe { ptr::addr_of_mut!((*entry).order_link).cast::<c_void>() }
    }
}

#[no_mangle]
pub extern "C" fn json_object_iter_next(object: *mut json_t, iter: *mut c_void) -> *mut c_void {
    if !unsafe { is_object(object) } || iter.is_null() {
        return null_mut();
    }

    unsafe { ordered_next(as_object_mut(object), iter.cast::<RawListNode>()).cast::<c_void>() }
}

#[no_mangle]
pub extern "C" fn json_object_iter_key(iter: *mut c_void) -> *const c_char {
    if iter.is_null() {
        return std::ptr::null();
    }

    unsafe { entry_key_ptr(entry_from_order_link(iter.cast::<RawListNode>())) }
}

#[no_mangle]
pub extern "C" fn json_object_iter_key_len(iter: *mut c_void) -> usize {
    if iter.is_null() {
        return 0;
    }

    unsafe { (*entry_from_order_link(iter.cast::<RawListNode>())).key_len }
}

#[no_mangle]
pub extern "C" fn json_object_iter_value(iter: *mut c_void) -> *mut json_t {
    if iter.is_null() {
        return null_mut();
    }

    unsafe { (*entry_from_order_link(iter.cast::<RawListNode>())).value }
}

#[no_mangle]
pub extern "C" fn json_object_iter_set_new(
    object: *mut json_t,
    iter: *mut c_void,
    value: *mut json_t,
) -> c_int {
    if !unsafe { is_object(object) } || iter.is_null() || value.is_null() {
        unsafe {
            decref(value);
        }
        return -1;
    }

    let entry = unsafe { entry_from_order_link(iter.cast::<RawListNode>()) };
    unsafe {
        let old = (*entry).value;
        (*entry).value = value;
        decref(old);
    }

    0
}

#[no_mangle]
pub extern "C" fn json_object_key_to_iter(key: *const c_char) -> *mut c_void {
    if key.is_null() {
        return null_mut();
    }

    unsafe { ptr::addr_of_mut!((*entry_from_key(key)).order_link).cast::<c_void>() }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::abi::decref;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn container_seed_contract_explicit_seed_only_before_first_object() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_seed_state_for_tests();

        json_object_seed(1234);
        assert_eq!(current_seed_for_tests(), 1234);

        let object = json_object();
        assert!(!object.is_null());
        json_object_seed(5678);
        assert_eq!(current_seed_for_tests(), 1234);

        unsafe {
            decref(object);
        }
    }

    #[test]
    fn container_seed_contract_autoseeds() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_seed_state_for_tests();

        let object = json_object();
        assert!(!object.is_null());
        assert_ne!(current_seed_for_tests(), 0);

        unsafe {
            decref(object);
        }
    }
}
