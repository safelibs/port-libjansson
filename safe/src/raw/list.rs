use std::ptr::null_mut;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawListNode {
    pub prev: *mut RawListNode,
    pub next: *mut RawListNode,
}

impl RawListNode {
    pub const fn new() -> Self {
        Self {
            prev: null_mut(),
            next: null_mut(),
        }
    }
}

#[inline]
pub unsafe fn init(node: *mut RawListNode) {
    unsafe {
        (*node).prev = node;
        (*node).next = node;
    }
}

#[inline]
pub unsafe fn is_empty(head: *const RawListNode) -> bool {
    unsafe { std::ptr::eq((*head).next.cast_const(), head) }
}

#[inline]
pub unsafe fn insert_before(position: *mut RawListNode, node: *mut RawListNode) {
    unsafe {
        (*node).next = position;
        (*node).prev = (*position).prev;
        (*(*position).prev).next = node;
        (*position).prev = node;
    }
}

#[inline]
pub unsafe fn remove(node: *mut RawListNode) {
    unsafe {
        (*(*node).prev).next = (*node).next;
        (*(*node).next).prev = (*node).prev;
        init(node);
    }
}
