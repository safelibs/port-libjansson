use std::ffi::{c_int, c_void};
use std::ptr::{self, null_mut};

use crate::raw::alloc::{jsonp_free, jsonp_malloc};
use crate::raw::list::{self, RawListNode};

const INITIAL_TABLE_ORDER: usize = 3;

#[repr(C)]
pub struct RawBucket {
    pub first: *mut RawListNode,
    pub last: *mut RawListNode,
}

#[repr(C)]
pub struct RawTable {
    pub size: usize,
    pub order: usize,
    pub buckets: *mut RawBucket,
    pub list: RawListNode,
}

impl RawTable {
    pub const fn new() -> Self {
        Self {
            size: 0,
            order: 0,
            buckets: null_mut(),
            list: RawListNode::new(),
        }
    }

    #[inline]
    pub fn bucket_count(&self) -> usize {
        if self.order == 0 {
            0
        } else {
            1usize << self.order
        }
    }

    #[inline]
    fn hashmask(order: usize) -> usize {
        (1usize << order) - 1
    }

    unsafe fn reset_buckets(&mut self) {
        let head = ptr::addr_of_mut!(self.list);
        let count = self.bucket_count();
        let mut index = 0usize;
        while index < count {
            let bucket = unsafe { self.buckets.add(index) };
            unsafe {
                (*bucket).first = head;
                (*bucket).last = head;
            }
            index += 1;
        }
    }

    #[inline]
    pub unsafe fn bucket_is_empty(&self, bucket: *const RawBucket) -> bool {
        let head = ptr::addr_of!(self.list);
        unsafe {
            std::ptr::eq((*bucket).first.cast_const(), head)
                && std::ptr::eq((*bucket).last.cast_const(), head)
        }
    }

    pub unsafe fn init(&mut self) -> c_int {
        self.size = 0;
        self.order = INITIAL_TABLE_ORDER;
        let bucket_count = self.bucket_count();
        let Some(alloc_size) = bucket_count.checked_mul(std::mem::size_of::<RawBucket>()) else {
            self.order = 0;
            return -1;
        };

        self.buckets = unsafe { jsonp_malloc(alloc_size) }.cast::<RawBucket>();
        if self.buckets.is_null() {
            self.order = 0;
            return -1;
        }

        unsafe {
            list::init(ptr::addr_of_mut!(self.list));
            self.reset_buckets();
        }

        0
    }

    pub unsafe fn bucket_mut(&mut self, hash: usize) -> *mut RawBucket {
        unsafe { self.buckets.add(hash & Self::hashmask(self.order)) }
    }

    pub unsafe fn find_in_bucket<F>(
        &self,
        bucket: *const RawBucket,
        mut matches: F,
    ) -> *mut RawListNode
    where
        F: FnMut(*mut RawListNode) -> bool,
    {
        if unsafe { self.bucket_is_empty(bucket) } {
            return null_mut();
        }

        let mut node = unsafe { (*bucket).first };
        loop {
            if matches(node) {
                return node;
            }

            if node == unsafe { (*bucket).last } {
                break;
            }

            node = unsafe { (*node).next };
        }

        null_mut()
    }

    pub unsafe fn insert_into_bucket(&mut self, bucket: *mut RawBucket, node: *mut RawListNode) {
        let head = ptr::addr_of_mut!(self.list);
        if unsafe { self.bucket_is_empty(bucket) } {
            unsafe {
                list::insert_before(head, node);
                (*bucket).first = node;
                (*bucket).last = node;
            }
        } else {
            unsafe {
                list::insert_before((*bucket).first, node);
                (*bucket).first = node;
            }
        }

        self.size += 1;
    }

    pub unsafe fn remove_from_bucket(&mut self, bucket: *mut RawBucket, node: *mut RawListNode) {
        let head = ptr::addr_of_mut!(self.list);
        if node == unsafe { (*bucket).first } && node == unsafe { (*bucket).last } {
            unsafe {
                (*bucket).first = head;
                (*bucket).last = head;
            }
        } else if node == unsafe { (*bucket).first } {
            unsafe {
                (*bucket).first = (*node).next;
            }
        } else if node == unsafe { (*bucket).last } {
            unsafe {
                (*bucket).last = (*node).prev;
            }
        }

        unsafe {
            list::remove(node);
        }
        self.size -= 1;
    }

    pub unsafe fn ensure_capacity<F>(&mut self, hash_of: F) -> c_int
    where
        F: FnMut(*mut RawListNode) -> usize,
    {
        if self.size < self.bucket_count() {
            return 0;
        }

        unsafe { self.rehash(hash_of) }
    }

    unsafe fn rehash<F>(&mut self, mut hash_of: F) -> c_int
    where
        F: FnMut(*mut RawListNode) -> usize,
    {
        let new_order = self.order + 1;
        let new_bucket_count = 1usize << new_order;
        let Some(alloc_size) = new_bucket_count.checked_mul(std::mem::size_of::<RawBucket>()) else {
            return -1;
        };

        let new_buckets = unsafe { jsonp_malloc(alloc_size) }.cast::<RawBucket>();
        if new_buckets.is_null() {
            return -1;
        }

        unsafe {
            jsonp_free(self.buckets.cast());
        }
        self.buckets = new_buckets;
        self.order = new_order;
        unsafe {
            self.reset_buckets();
        }

        let head = ptr::addr_of_mut!(self.list);
        let mut node = unsafe { (*head).next };
        unsafe {
            list::init(head);
        }

        while node != head {
            let next = unsafe { (*node).next };
            let hash = hash_of(node);
            let bucket = unsafe { self.bucket_mut(hash) };
            unsafe {
                self.insert_into_bucket(bucket, node);
            }
            self.size -= 1;
            node = next;
        }

        0
    }

    pub unsafe fn clear<F>(&mut self, mut drop_node: F)
    where
        F: FnMut(*mut RawListNode),
    {
        let head = ptr::addr_of_mut!(self.list);
        let mut node = unsafe { (*head).next };
        while node != head {
            let next = unsafe { (*node).next };
            drop_node(node);
            node = next;
        }

        if !self.buckets.is_null() {
            unsafe {
                self.reset_buckets();
            }
        }
        unsafe {
            list::init(head);
        }
        self.size = 0;
    }

    pub unsafe fn close<F>(&mut self, drop_node: F)
    where
        F: FnMut(*mut RawListNode),
    {
        unsafe {
            self.clear(drop_node);
        }

        if !self.buckets.is_null() {
            unsafe {
                jsonp_free(self.buckets.cast());
            }
        }
        self.buckets = null_mut();
        self.order = 0;
    }
}

#[repr(C)]
struct PointerEntry {
    link: RawListNode,
    pointer: *const c_void,
}

#[inline]
unsafe fn pointer_entry_from_link(link: *mut RawListNode) -> *mut PointerEntry {
    link.cast::<PointerEntry>()
}

#[inline]
fn pointer_hash(pointer: *const c_void) -> usize {
    let mut hash = pointer as usize;
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51afd7ed558ccd_u64 as usize);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xc4ceb9fe1a85ec53_u64 as usize);
    hash ^ (hash >> 33)
}

pub struct PointerSet {
    table: RawTable,
}

impl PointerSet {
    pub const fn new() -> Self {
        Self {
            table: RawTable::new(),
        }
    }

    pub unsafe fn init(&mut self) -> c_int {
        unsafe { self.table.init() }
    }

    pub unsafe fn close(&mut self) {
        unsafe {
            self.table.close(|node| {
                let entry = pointer_entry_from_link(node);
                jsonp_free(entry.cast());
            });
        }
    }

    pub unsafe fn contains(&mut self, pointer: *const c_void) -> bool {
        let hash = pointer_hash(pointer);
        let bucket = unsafe { self.table.bucket_mut(hash) };
        !unsafe {
            self.table.find_in_bucket(bucket, |node| {
                let entry = pointer_entry_from_link(node);
                (*entry).pointer == pointer
            })
        }
        .is_null()
    }

    pub unsafe fn insert(&mut self, pointer: *const c_void) -> c_int {
        if unsafe { self.contains(pointer) } {
            return 0;
        }

        if unsafe {
            self.table
                .ensure_capacity(|node| pointer_hash((*pointer_entry_from_link(node)).pointer))
        } != 0
        {
            return -1;
        }

        let entry = unsafe { jsonp_malloc(std::mem::size_of::<PointerEntry>()) }.cast::<PointerEntry>();
        if entry.is_null() {
            return -1;
        }

        unsafe {
            (*entry).link = RawListNode::new();
            list::init(ptr::addr_of_mut!((*entry).link));
            (*entry).pointer = pointer;
        }

        let hash = pointer_hash(pointer);
        let bucket = unsafe { self.table.bucket_mut(hash) };
        unsafe {
            self.table
                .insert_into_bucket(bucket, ptr::addr_of_mut!((*entry).link));
        }

        0
    }

    pub unsafe fn remove(&mut self, pointer: *const c_void) -> bool {
        let hash = pointer_hash(pointer);
        let bucket = unsafe { self.table.bucket_mut(hash) };
        let node = unsafe {
            self.table.find_in_bucket(bucket, |candidate| {
                let entry = pointer_entry_from_link(candidate);
                (*entry).pointer == pointer
            })
        };

        if node.is_null() {
            return false;
        }

        unsafe {
            self.table.remove_from_bucket(bucket, node);
            jsonp_free(pointer_entry_from_link(node).cast());
        }

        true
    }
}
