use std::cmp::Ordering;
use std::ffi::{c_char, c_int, c_void};
use std::ptr::{self, null_mut};
use std::slice;

use libc::FILE;

use crate::abi::{json_dump_callback_t, json_t};
use crate::raw::alloc::jsonp_strdup;
use crate::raw::buf::RawBuf;
use crate::raw::table::PointerSet;
use crate::strconv;
use crate::utf;

const JSON_MAX_INDENT: usize = 0x1F;
const JSON_COMPACT: usize = 0x20;
const JSON_ENSURE_ASCII: usize = 0x40;
const JSON_SORT_KEYS: usize = 0x80;
const JSON_ENCODE_ANY: usize = 0x200;
const JSON_ESCAPE_SLASH: usize = 0x400;
const JSON_EMBED: usize = 0x10000;

const MODE_WRITE_TEXT: &[u8] = b"w\0";
const WHITESPACE: &[u8] = b"                                ";

#[derive(Clone, Copy)]
struct BufferSink {
    size: usize,
    used: usize,
    data: *mut c_char,
}

unsafe extern "C" fn dump_to_strbuffer(
    buffer: *const c_char,
    size: usize,
    data: *mut c_void,
) -> c_int {
    let output = unsafe { &mut *data.cast::<RawBuf>() };
    unsafe { output.append_bytes(buffer.cast::<u8>(), size) }
}

unsafe extern "C" fn dump_to_buffer(
    buffer: *const c_char,
    size: usize,
    data: *mut c_void,
) -> c_int {
    let sink = unsafe { &mut *data.cast::<BufferSink>() };
    if sink.used + size <= sink.size && !sink.data.is_null() {
        unsafe {
            ptr::copy_nonoverlapping(buffer, sink.data.add(sink.used), size);
        }
    }
    sink.used += size;
    0
}

unsafe extern "C" fn dump_to_file(
    buffer: *const c_char,
    size: usize,
    data: *mut c_void,
) -> c_int {
    let output = data.cast::<FILE>();
    if output.is_null() {
        return -1;
    }

    if unsafe { libc::fwrite(buffer.cast::<c_void>(), size, 1, output) } == 1 {
        0
    } else {
        -1
    }
}

unsafe extern "C" fn dump_to_fd(
    buffer: *const c_char,
    size: usize,
    data: *mut c_void,
) -> c_int {
    let fd = unsafe { *data.cast::<c_int>() };
    if unsafe { libc::write(fd, buffer.cast::<c_void>(), size) } == size as isize {
        0
    } else {
        -1
    }
}

#[inline]
fn indent_flags(flags: usize) -> usize {
    flags & JSON_MAX_INDENT
}

#[inline]
fn precision_flags(flags: usize) -> i32 {
    ((flags >> 11) & 0x1F) as i32
}

unsafe fn emit_bytes(
    callback: json_dump_callback_t,
    data: *mut c_void,
    bytes: &[u8],
) -> c_int {
    let Some(callback) = callback else {
        return -1;
    };

    unsafe { callback(bytes.as_ptr().cast::<c_char>(), bytes.len(), data) }
}

unsafe fn dump_indent(
    flags: usize,
    depth: usize,
    space: bool,
    callback: json_dump_callback_t,
    data: *mut c_void,
) -> c_int {
    let indent = indent_flags(flags);
    if indent > 0 {
        if unsafe { emit_bytes(callback, data, b"\n") } != 0 {
            return -1;
        }

        let mut remaining = depth.saturating_mul(indent);
        while remaining > 0 {
            let chunk = remaining.min(WHITESPACE.len());
            if unsafe { emit_bytes(callback, data, &WHITESPACE[..chunk]) } != 0 {
                return -1;
            }
            remaining -= chunk;
        }

        0
    } else if space && flags & JSON_COMPACT == 0 {
        unsafe { emit_bytes(callback, data, b" ") }
    } else {
        0
    }
}

unsafe fn dump_string(
    string: *const c_char,
    len: usize,
    callback: json_dump_callback_t,
    data: *mut c_void,
    flags: usize,
) -> c_int {
    if unsafe { emit_bytes(callback, data, b"\"") } != 0 {
        return -1;
    }

    let bytes = unsafe { slice::from_raw_parts(string.cast::<u8>(), len) };
    let mut pos = 0usize;
    let mut safe_start = 0usize;

    while pos < len {
        let (codepoint, width) = match utf::iterate(bytes, pos) {
            Ok(value) => value,
            Err(_) => return -1,
        };

        let must_escape = codepoint == b'\\' as u32
            || codepoint == b'"' as u32
            || codepoint < 0x20
            || (flags & JSON_ESCAPE_SLASH != 0 && codepoint == b'/' as u32)
            || (flags & JSON_ENSURE_ASCII != 0 && codepoint > 0x7F);

        if must_escape {
            if safe_start < pos
                && unsafe { emit_bytes(callback, data, &bytes[safe_start..pos]) } != 0
            {
                return -1;
            }

            let mut escape = [0u8; 12];
            let emitted = match codepoint {
                x if x == b'\\' as u32 => {
                    escape[..2].copy_from_slice(br"\\");
                    2
                }
                x if x == b'"' as u32 => {
                    escape[..2].copy_from_slice(br#"\""#);
                    2
                }
                x if x == b'\x08' as u32 => {
                    escape[..2].copy_from_slice(br"\b");
                    2
                }
                x if x == b'\x0c' as u32 => {
                    escape[..2].copy_from_slice(br"\f");
                    2
                }
                x if x == b'\n' as u32 => {
                    escape[..2].copy_from_slice(br"\n");
                    2
                }
                x if x == b'\r' as u32 => {
                    escape[..2].copy_from_slice(br"\r");
                    2
                }
                x if x == b'\t' as u32 => {
                    escape[..2].copy_from_slice(br"\t");
                    2
                }
                x if x == b'/' as u32 => {
                    escape[..2].copy_from_slice(br"\/");
                    2
                }
                x if x < 0x10000 => {
                    let text = format!("\\u{:04X}", x);
                    escape[..6].copy_from_slice(text.as_bytes());
                    6
                }
                x => {
                    let value = x - 0x10000;
                    let first = 0xD800 | ((value & 0xFFC00) >> 10);
                    let last = 0xDC00 | (value & 0x003FF);
                    let text = format!("\\u{:04X}\\u{:04X}", first, last);
                    escape.copy_from_slice(text.as_bytes());
                    12
                }
            };

            if unsafe { emit_bytes(callback, data, &escape[..emitted]) } != 0 {
                return -1;
            }
            safe_start = pos + width;
        }

        pos += width;
    }

    if safe_start < len && unsafe { emit_bytes(callback, data, &bytes[safe_start..]) } != 0 {
        return -1;
    }

    unsafe { emit_bytes(callback, data, b"\"") }
}

struct ParentGuard {
    parents: *mut PointerSet,
    pointer: *const c_void,
    active: bool,
}

impl Drop for ParentGuard {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                (*self.parents).remove(self.pointer);
            }
        }
    }
}

unsafe fn enter_parent(parents: &mut PointerSet, pointer: *const c_void) -> Result<ParentGuard, ()> {
    if unsafe { parents.contains(pointer) } || unsafe { parents.insert(pointer) } != 0 {
        return Err(());
    }

    Ok(ParentGuard {
        parents: parents as *mut PointerSet,
        pointer,
        active: true,
    })
}

#[derive(Clone, Copy)]
struct KeyLen {
    key: *const c_char,
    len: usize,
}

fn compare_keys(left: &KeyLen, right: &KeyLen) -> Ordering {
    let left_bytes = unsafe { slice::from_raw_parts(left.key.cast::<u8>(), left.len) };
    let right_bytes = unsafe { slice::from_raw_parts(right.key.cast::<u8>(), right.len) };
    left_bytes.cmp(right_bytes).then(left.len.cmp(&right.len))
}

unsafe fn do_dump(
    json: *const json_t,
    flags: usize,
    depth: usize,
    parents: &mut PointerSet,
    callback: json_dump_callback_t,
    data: *mut c_void,
) -> c_int {
    let embed = flags & JSON_EMBED != 0;
    let child_flags = flags & !JSON_EMBED;

    if json.is_null() {
        return -1;
    }

    match unsafe { (*json).type_ } {
        crate::abi::JSON_NULL => unsafe { emit_bytes(callback, data, b"null") },
        crate::abi::JSON_TRUE => unsafe { emit_bytes(callback, data, b"true") },
        crate::abi::JSON_FALSE => unsafe { emit_bytes(callback, data, b"false") },
        crate::abi::JSON_INTEGER => {
            let text = crate::scalar::json_integer_value(json).to_string();
            unsafe { emit_bytes(callback, data, text.as_bytes()) }
        }
        crate::abi::JSON_REAL => {
            let mut buffer = [0u8; 100];
            let len = match strconv::dtostr(
                &mut buffer,
                crate::scalar::json_real_value(json),
                precision_flags(flags),
            ) {
                Ok(len) => len,
                Err(()) => return -1,
            };
            unsafe { emit_bytes(callback, data, &buffer[..len]) }
        }
        crate::abi::JSON_STRING => unsafe {
            dump_string(
                crate::scalar::json_string_value(json),
                crate::scalar::json_string_length(json),
                callback,
                data,
                flags,
            )
        },
        crate::abi::JSON_ARRAY => {
            let _guard = match unsafe { enter_parent(parents, json.cast::<c_void>()) } {
                Ok(guard) => guard,
                Err(()) => return -1,
            };

            let size = crate::array::json_array_size(json);
            if !embed && unsafe { emit_bytes(callback, data, b"[") } != 0 {
                return -1;
            }
            if size == 0 {
                return if embed {
                    0
                } else {
                    unsafe { emit_bytes(callback, data, b"]") }
                };
            }
            if unsafe { dump_indent(flags, depth + 1, false, callback, data) } != 0 {
                return -1;
            }

            let mut index = 0usize;
            while index < size {
                if unsafe {
                    do_dump(
                        crate::array::json_array_get(json, index),
                        child_flags,
                        depth + 1,
                        parents,
                        callback,
                        data,
                    )
                } != 0
                {
                    return -1;
                }

                if index + 1 < size {
                    if unsafe { emit_bytes(callback, data, b",") } != 0
                        || unsafe { dump_indent(flags, depth + 1, true, callback, data) } != 0
                    {
                        return -1;
                    }
                } else if unsafe { dump_indent(flags, depth, false, callback, data) } != 0 {
                    return -1;
                }

                index += 1;
            }

            if embed {
                0
            } else {
                unsafe { emit_bytes(callback, data, b"]") }
            }
        }
        crate::abi::JSON_OBJECT => {
            let _guard = match unsafe { enter_parent(parents, json.cast::<c_void>()) } {
                Ok(guard) => guard,
                Err(()) => return -1,
            };

            let separator: &[u8] = if flags & JSON_COMPACT != 0 { b":" } else { b": " };
            let mut iter = crate::object::json_object_iter(json.cast_mut());

            if !embed && unsafe { emit_bytes(callback, data, b"{") } != 0 {
                return -1;
            }
            if iter.is_null() {
                return if embed {
                    0
                } else {
                    unsafe { emit_bytes(callback, data, b"}") }
                };
            }
            if unsafe { dump_indent(flags, depth + 1, false, callback, data) } != 0 {
                return -1;
            }

            if flags & JSON_SORT_KEYS != 0 {
                let size = crate::object::json_object_size(json);
                let mut keys = Vec::with_capacity(size);
                while !iter.is_null() {
                    keys.push(KeyLen {
                        key: crate::object::json_object_iter_key(iter),
                        len: crate::object::json_object_iter_key_len(iter),
                    });
                    iter = crate::object::json_object_iter_next(json.cast_mut(), iter);
                }
                keys.sort_by(compare_keys);

                for (index, key) in keys.iter().enumerate() {
                    let value = unsafe { crate::object::json_object_getn(json, key.key, key.len) };
                    if value.is_null()
                        || unsafe { dump_string(key.key, key.len, callback, data, flags) } != 0
                        || unsafe { emit_bytes(callback, data, separator) } != 0
                        || unsafe {
                            do_dump(value, child_flags, depth + 1, parents, callback, data)
                        } != 0
                    {
                        return -1;
                    }

                    if index + 1 < keys.len() {
                        if unsafe { emit_bytes(callback, data, b",") } != 0
                            || unsafe { dump_indent(flags, depth + 1, true, callback, data) } != 0
                        {
                            return -1;
                        }
                    } else if unsafe { dump_indent(flags, depth, false, callback, data) } != 0 {
                        return -1;
                    }
                }
            } else {
                while !iter.is_null() {
                    let next = crate::object::json_object_iter_next(json.cast_mut(), iter);
                    let key = crate::object::json_object_iter_key(iter);
                    let key_len = crate::object::json_object_iter_key_len(iter);
                    let value = crate::object::json_object_iter_value(iter);

                    if unsafe { dump_string(key, key_len, callback, data, flags) } != 0
                        || unsafe { emit_bytes(callback, data, separator) } != 0
                        || unsafe {
                            do_dump(value, child_flags, depth + 1, parents, callback, data)
                        } != 0
                    {
                        return -1;
                    }

                    if !next.is_null() {
                        if unsafe { emit_bytes(callback, data, b",") } != 0
                            || unsafe { dump_indent(flags, depth + 1, true, callback, data) } != 0
                        {
                            return -1;
                        }
                    } else if unsafe { dump_indent(flags, depth, false, callback, data) } != 0 {
                        return -1;
                    }

                    iter = next;
                }
            }

            if embed {
                0
            } else {
                unsafe { emit_bytes(callback, data, b"}") }
            }
        }
        _ => -1,
    }
}

#[no_mangle]
pub extern "C" fn json_dumps(json: *const json_t, flags: usize) -> *mut c_char {
    let mut output = RawBuf::new();
    if unsafe { output.init() } != 0 {
        return null_mut();
    }

    let result = if json_dump_callback(
        json,
        Some(dump_to_strbuffer),
        ptr::addr_of_mut!(output).cast::<c_void>(),
        flags,
    ) != 0
    {
        null_mut()
    } else {
        unsafe { jsonp_strdup(output.value()) }
    };

    unsafe {
        output.close();
    }
    result
}

#[no_mangle]
pub extern "C" fn json_dumpb(
    json: *const json_t,
    buffer: *mut c_char,
    size: usize,
    flags: usize,
) -> usize {
    let mut sink = BufferSink {
        size,
        used: 0,
        data: buffer,
    };

    if json_dump_callback(
        json,
        Some(dump_to_buffer),
        ptr::addr_of_mut!(sink).cast::<c_void>(),
        flags,
    ) != 0
    {
        0
    } else {
        sink.used
    }
}

#[no_mangle]
pub extern "C" fn json_dumpf(json: *const json_t, output: *mut FILE, flags: usize) -> c_int {
    if output.is_null() {
        return -1;
    }

    json_dump_callback(json, Some(dump_to_file), output.cast::<c_void>(), flags)
}

#[no_mangle]
pub extern "C" fn json_dumpfd(json: *const json_t, output: c_int, flags: usize) -> c_int {
    let mut fd = output;
    json_dump_callback(
        json,
        Some(dump_to_fd),
        ptr::addr_of_mut!(fd).cast::<c_void>(),
        flags,
    )
}

#[no_mangle]
pub extern "C" fn json_dump_file(json: *const json_t, path: *const c_char, flags: usize) -> c_int {
    if path.is_null() {
        return -1;
    }

    let output = unsafe { libc::fopen(path, MODE_WRITE_TEXT.as_ptr().cast::<c_char>()) };
    if output.is_null() {
        return -1;
    }

    let result = json_dumpf(json, output, flags);
    if unsafe { libc::fclose(output) } != 0 {
        return -1;
    }

    result
}

#[no_mangle]
pub extern "C" fn json_dump_callback(
    json: *const json_t,
    callback: json_dump_callback_t,
    data: *mut c_void,
    flags: usize,
) -> c_int {
    if json.is_null() || callback.is_none() {
        return -1;
    }

    if flags & JSON_ENCODE_ANY == 0
        && !unsafe { crate::abi::is_array(json) }
        && !unsafe { crate::abi::is_object(json) }
    {
        return -1;
    }

    let mut parents = PointerSet::new();
    if unsafe { parents.init() } != 0 {
        return -1;
    }

    let result = unsafe { do_dump(json, flags, 0, &mut parents, callback, data) };
    unsafe {
        parents.close();
    }
    result
}
