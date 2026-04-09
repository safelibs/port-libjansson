use std::ffi::{c_char, c_double, c_int, c_void, CStr};
use std::fmt::{self, Write as _};
use std::ptr::{null, null_mut};
use std::slice;

use crate::abi::{
    is_array, is_false, is_integer, is_null, is_number, is_object, is_real, is_string, is_true,
    json_error_code, json_error_t, json_int_t, json_t, type_of, JSON_ARRAY, JSON_FALSE,
    JSON_INTEGER, JSON_NULL, JSON_OBJECT, JSON_REAL, JSON_STRING, JSON_TRUE,
};
use crate::array;
use crate::error::{jsonp_error_init, jsonp_error_set_source, jsonp_error_vformat};
use crate::object;
use crate::raw::buf::RawBuf;
use crate::scalar;

const SOURCE_ROOT: &[u8] = b"<root>\0";
const SOURCE_FORMAT: &[u8] = b"<format>\0";
const SOURCE_ARGS: &[u8] = b"<args>\0";
const SOURCE_VALIDATION: &[u8] = b"<validation>\0";
const SOURCE_INTERNAL: &[u8] = b"<internal>\0";

const JSON_VALIDATE_ONLY: usize = 0x1;
const JSON_STRICT: usize = 0x2;

const UNPACK_ARG_KEY: c_int = 1;
const UNPACK_ARG_STRING: c_int = 2;
const UNPACK_ARG_SIZE: c_int = 3;
const UNPACK_ARG_INT: c_int = 4;
const UNPACK_ARG_JSON_INT: c_int = 5;
const UNPACK_ARG_DOUBLE: c_int = 6;
const UNPACK_ARG_JSON: c_int = 7;

const UNPACK_VALUE_STARTERS: &[u8] = b"{[siIbfFOon";

#[repr(C)]
pub struct jsonp_unpack_arg {
    pub kind: c_int,
    pub ptr: *mut c_void,
}

#[derive(Clone, Copy, Default)]
struct Token {
    line: c_int,
    column: c_int,
    pos: usize,
    token: u8,
}

struct Scanner<'a> {
    error: *mut json_error_t,
    flags: usize,
    fmt: &'a [u8],
    index: usize,
    prev_token: Token,
    token: Token,
    next_token: Token,
    line: c_int,
    column: c_int,
    pos: usize,
}

struct UnpackArgs<'a> {
    args: &'a [jsonp_unpack_arg],
    index: usize,
    mismatch_reported: bool,
}

struct ErrorTextBuf {
    buf: [u8; crate::abi::JSON_ERROR_TEXT_LENGTH - 1],
    len: usize,
}

impl ErrorTextBuf {
    fn new() -> Self {
        Self {
            buf: [0; crate::abi::JSON_ERROR_TEXT_LENGTH - 1],
            len: 0,
        }
    }

    fn as_c_str_ptr(&self) -> *const c_char {
        self.buf.as_ptr().cast::<c_char>()
    }
}

impl fmt::Write for ErrorTextBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.len >= self.buf.len().saturating_sub(1) {
            return Ok(());
        }

        let remaining = self.buf.len() - 1 - self.len;
        let bytes = s.as_bytes();
        let count = bytes.len().min(remaining);
        self.buf[self.len..self.len + count].copy_from_slice(&bytes[..count]);
        self.len += count;
        self.buf[self.len] = 0;
        Ok(())
    }
}

impl<'a> Scanner<'a> {
    fn new(error: *mut json_error_t, flags: usize, fmt: &'a [u8]) -> Self {
        Self {
            error,
            flags,
            fmt,
            index: 0,
            prev_token: Token::default(),
            token: Token::default(),
            next_token: Token::default(),
            line: 1,
            column: 0,
            pos: 0,
        }
    }

    fn current_token(&self) -> u8 {
        self.token.token
    }

    fn next_token(&mut self) {
        self.prev_token = self.token;

        if self.next_token.line != 0 {
            self.token = self.next_token;
            self.next_token = Token::default();
            return;
        }

        if self.current_token() == 0 && self.index >= self.fmt.len() {
            return;
        }

        let mut idx = self.index;
        self.column += 1;
        self.pos += 1;

        while idx < self.fmt.len() {
            match self.fmt[idx] {
                b' ' | b'\t' | b',' | b':' => {
                    self.column += 1;
                    self.pos += 1;
                    idx += 1;
                }
                b'\n' => {
                    self.line += 1;
                    self.column = 1;
                    self.pos += 1;
                    idx += 1;
                }
                _ => break,
            }
        }

        let token = if idx < self.fmt.len() {
            self.fmt[idx]
        } else {
            0
        };
        self.token = Token {
            line: self.line,
            column: self.column,
            pos: self.pos,
            token,
        };

        if token != 0 {
            idx += 1;
        }
        self.index = idx;
    }

    fn prev_token(&mut self) {
        self.next_token = self.token;
        self.token = self.prev_token;
    }

    fn validate_only(&self) -> bool {
        (self.flags & JSON_VALIDATE_ONLY) != 0
    }
}

impl<'a> UnpackArgs<'a> {
    fn new(args: &'a [jsonp_unpack_arg]) -> Self {
        Self {
            args,
            index: 0,
            mismatch_reported: false,
        }
    }

    fn next(&mut self, kind: c_int, scanner: &mut Scanner<'_>) -> Option<&'a jsonp_unpack_arg> {
        let Some(arg) = self.args.get(self.index) else {
            self.report_mismatch(scanner);
            return None;
        };
        self.index += 1;

        if arg.kind != kind {
            self.report_mismatch(scanner);
            return None;
        }

        Some(arg)
    }

    fn report_mismatch(&mut self, scanner: &mut Scanner<'_>) {
        if self.mismatch_reported {
            return;
        }
        self.mismatch_reported = true;
        set_error_at(
            scanner.error,
            scanner.token.line,
            scanner.token.column,
            scanner.token.pos,
            SOURCE_INTERNAL,
            json_error_code::json_error_invalid_argument,
            format_args!("Argument marshaling mismatch"),
        );
    }
}

fn set_error_at(
    error: *mut json_error_t,
    line: c_int,
    column: c_int,
    position: usize,
    source: &[u8],
    code: json_error_code,
    args: fmt::Arguments<'_>,
) {
    let mut text = ErrorTextBuf::new();
    let _ = text.write_fmt(args);

    unsafe {
        jsonp_error_vformat(
            error,
            line,
            column,
            position,
            code as c_int,
            text.as_c_str_ptr(),
        );
        jsonp_error_set_source(error, source.as_ptr().cast::<c_char>());
    }
}

fn set_scanner_error(
    scanner: &mut Scanner<'_>,
    source: &[u8],
    code: json_error_code,
    args: fmt::Arguments<'_>,
) {
    set_error_at(
        scanner.error,
        scanner.token.line,
        scanner.token.column,
        scanner.token.pos,
        source,
        code,
        args,
    );
}

fn type_name(json: *const json_t) -> &'static str {
    match unsafe { type_of(json) } {
        Some(JSON_OBJECT) => "object",
        Some(JSON_ARRAY) => "array",
        Some(JSON_STRING) => "string",
        Some(JSON_INTEGER) => "integer",
        Some(JSON_REAL) => "real",
        Some(JSON_TRUE) => "true",
        Some(JSON_FALSE) => "false",
        Some(JSON_NULL) => "null",
        _ => "unknown",
    }
}

unsafe fn next_key<'a>(args: &'a mut UnpackArgs<'_>, scanner: &mut Scanner<'_>) -> *const c_char {
    args.next(UNPACK_ARG_KEY, scanner)
        .map(|arg| arg.ptr.cast::<c_char>().cast_const())
        .unwrap_or(null())
}

unsafe fn next_string_target<'a>(
    args: &'a mut UnpackArgs<'_>,
    scanner: &mut Scanner<'_>,
) -> *mut *const c_char {
    args.next(UNPACK_ARG_STRING, scanner)
        .map(|arg| arg.ptr.cast::<*const c_char>())
        .unwrap_or(null_mut())
}

unsafe fn next_size_target<'a>(
    args: &'a mut UnpackArgs<'_>,
    scanner: &mut Scanner<'_>,
) -> *mut usize {
    args.next(UNPACK_ARG_SIZE, scanner)
        .map(|arg| arg.ptr.cast::<usize>())
        .unwrap_or(null_mut())
}

unsafe fn next_int_target<'a>(
    args: &'a mut UnpackArgs<'_>,
    scanner: &mut Scanner<'_>,
) -> *mut c_int {
    args.next(UNPACK_ARG_INT, scanner)
        .map(|arg| arg.ptr.cast::<c_int>())
        .unwrap_or(null_mut())
}

unsafe fn next_json_int_target<'a>(
    args: &'a mut UnpackArgs<'_>,
    scanner: &mut Scanner<'_>,
) -> *mut json_int_t {
    args.next(UNPACK_ARG_JSON_INT, scanner)
        .map(|arg| arg.ptr.cast::<json_int_t>())
        .unwrap_or(null_mut())
}

unsafe fn next_double_target<'a>(
    args: &'a mut UnpackArgs<'_>,
    scanner: &mut Scanner<'_>,
) -> *mut c_double {
    args.next(UNPACK_ARG_DOUBLE, scanner)
        .map(|arg| arg.ptr.cast::<c_double>())
        .unwrap_or(null_mut())
}

unsafe fn next_json_target<'a>(
    args: &'a mut UnpackArgs<'_>,
    scanner: &mut Scanner<'_>,
) -> *mut *mut json_t {
    args.next(UNPACK_ARG_JSON, scanner)
        .map(|arg| arg.ptr.cast::<*mut json_t>())
        .unwrap_or(null_mut())
}

unsafe fn remember_key(key_set: *mut json_t, key: *const c_char, key_len: usize) -> c_int {
    unsafe { object::json_object_setn_new_nocheck(key_set, key, key_len, scalar::json_null()) }
}

unsafe fn append_unrecognized_key(
    buffer: &mut RawBuf,
    key: *const c_char,
    key_len: usize,
) -> c_int {
    unsafe { buffer.append_bytes(key.cast::<u8>(), key_len) }
}

unsafe fn unpack(scanner: &mut Scanner<'_>, root: *mut json_t, args: &mut UnpackArgs<'_>) -> c_int {
    match scanner.current_token() {
        b'{' => unsafe { unpack_object(scanner, root, args) },
        b'[' => unsafe { unpack_array(scanner, root, args) },
        b's' => unsafe { unpack_string(scanner, root, args) },
        b'i' => unsafe { unpack_integer(scanner, root, args) },
        b'I' => unsafe { unpack_json_int(scanner, root, args) },
        b'b' => unsafe { unpack_boolean(scanner, root, args) },
        b'f' => unsafe { unpack_real(scanner, root, args) },
        b'F' => unsafe { unpack_number(scanner, root, args) },
        b'O' => unsafe { unpack_object_ref(scanner, root, args, true) },
        b'o' => unsafe { unpack_object_ref(scanner, root, args, false) },
        b'n' => {
            if !root.is_null() && !unsafe { is_null(root) } {
                set_scanner_error(
                    scanner,
                    SOURCE_VALIDATION,
                    json_error_code::json_error_wrong_type,
                    format_args!("Expected null, got {}", type_name(root)),
                );
                return -1;
            }
            0
        }
        _ => {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!(
                    "Unexpected format character '{}'",
                    scanner.current_token() as char
                ),
            );
            -1
        }
    }
}

unsafe fn unpack_object(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    let key_set = object::json_object();
    if key_set.is_null() {
        set_scanner_error(
            scanner,
            SOURCE_INTERNAL,
            json_error_code::json_error_out_of_memory,
            format_args!("Out of memory"),
        );
        return -1;
    }

    if !root.is_null() && !unsafe { is_object(root) } {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected object, got {}", type_name(root)),
        );
        unsafe {
            crate::abi::decref(key_set);
        }
        return -1;
    }

    let mut strict = 0;
    scanner.next_token();
    while scanner.current_token() != b'}' {
        if strict != 0 {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!(
                    "Expected '}}' after '{}', got '{}'",
                    if strict == 1 { '!' } else { '*' },
                    scanner.current_token() as char
                ),
            );
            unsafe {
                crate::abi::decref(key_set);
            }
            return -1;
        }

        if scanner.current_token() == 0 {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!("Unexpected end of format string"),
            );
            unsafe {
                crate::abi::decref(key_set);
            }
            return -1;
        }

        if scanner.current_token() == b'!' || scanner.current_token() == b'*' {
            strict = if scanner.current_token() == b'!' {
                1
            } else {
                -1
            };
            scanner.next_token();
            continue;
        }

        if scanner.current_token() != b's' {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!(
                    "Expected format 's', got '{}'",
                    scanner.current_token() as char
                ),
            );
            unsafe {
                crate::abi::decref(key_set);
            }
            return -1;
        }

        let key = unsafe { next_key(args, scanner) };
        if key.is_null() {
            set_scanner_error(
                scanner,
                SOURCE_ARGS,
                json_error_code::json_error_null_value,
                format_args!("NULL object key"),
            );
            unsafe {
                crate::abi::decref(key_set);
            }
            return -1;
        }

        let key_len = unsafe { libc::strlen(key) };

        scanner.next_token();
        let mut optional = false;
        if scanner.current_token() == b'?' {
            optional = true;
            scanner.next_token();
        }

        let value = if root.is_null() {
            null_mut()
        } else {
            let value = unsafe { object::json_object_getn(root, key, key_len) };
            if value.is_null() && !optional {
                let key_text = unsafe {
                    std::str::from_utf8_unchecked(slice::from_raw_parts(key.cast::<u8>(), key_len))
                };
                set_scanner_error(
                    scanner,
                    SOURCE_VALIDATION,
                    json_error_code::json_error_item_not_found,
                    format_args!("Object item not found: {key_text}"),
                );
                unsafe {
                    crate::abi::decref(key_set);
                }
                return -1;
            }
            value
        };

        if unsafe { unpack(scanner, value, args) } != 0 {
            unsafe {
                crate::abi::decref(key_set);
            }
            return -1;
        }

        let _ = unsafe { remember_key(key_set, key, key_len) };
        scanner.next_token();
    }

    if strict == 0 && (scanner.flags & JSON_STRICT) != 0 {
        strict = 1;
    }

    if !root.is_null() && strict == 1 {
        let mut unpacked = 0isize;
        let mut keys_buffer = RawBuf::new();
        let mut keys_state = 1i32;
        let mut iter = object::json_object_iter(root);
        while !iter.is_null() {
            let key = object::json_object_iter_key(iter);
            let key_len = object::json_object_iter_key_len(iter);
            if unsafe { object::json_object_getn(key_set, key, key_len) }.is_null() {
                unpacked += 1;

                if keys_state == 1 {
                    keys_state = unsafe { keys_buffer.init() };
                } else if keys_state == 0 {
                    keys_state = unsafe { keys_buffer.append_bytes(b", ".as_ptr(), 2) };
                }

                if keys_state == 0 {
                    keys_state = unsafe { append_unrecognized_key(&mut keys_buffer, key, key_len) };
                }
            }

            iter = object::json_object_iter_next(root, iter);
        }

        if unpacked != 0 {
            let unknown = b"<unknown>";
            let unknown_str = unsafe { std::str::from_utf8_unchecked(unknown) };
            let listed = if keys_state == 0 {
                unsafe {
                    std::str::from_utf8_unchecked(slice::from_raw_parts(
                        keys_buffer.value().cast::<u8>(),
                        keys_buffer.len(),
                    ))
                }
            } else {
                unknown_str
            };

            set_scanner_error(
                scanner,
                SOURCE_VALIDATION,
                json_error_code::json_error_end_of_input_expected,
                format_args!("{unpacked} object item(s) left unpacked: {listed}"),
            );
            unsafe {
                keys_buffer.close();
                crate::abi::decref(key_set);
            }
            return -1;
        }

        unsafe {
            keys_buffer.close();
        }
    }

    unsafe {
        crate::abi::decref(key_set);
    }
    0
}

unsafe fn unpack_array(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    if !root.is_null() && !unsafe { is_array(root) } {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected array, got {}", type_name(root)),
        );
        return -1;
    }

    let mut index = 0usize;
    let mut strict = 0;

    scanner.next_token();
    while scanner.current_token() != b']' {
        if strict != 0 {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!(
                    "Expected ']' after '{}', got '{}'",
                    if strict == 1 { '!' } else { '*' },
                    scanner.current_token() as char
                ),
            );
            return -1;
        }

        if scanner.current_token() == 0 {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!("Unexpected end of format string"),
            );
            return -1;
        }

        if scanner.current_token() == b'!' || scanner.current_token() == b'*' {
            strict = if scanner.current_token() == b'!' {
                1
            } else {
                -1
            };
            scanner.next_token();
            continue;
        }

        if !UNPACK_VALUE_STARTERS.contains(&scanner.current_token()) {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!(
                    "Unexpected format character '{}'",
                    scanner.current_token() as char
                ),
            );
            return -1;
        }

        let value = if root.is_null() {
            null_mut()
        } else {
            let value = array::json_array_get(root, index);
            if value.is_null() {
                set_scanner_error(
                    scanner,
                    SOURCE_VALIDATION,
                    json_error_code::json_error_index_out_of_range,
                    format_args!("Array index {} out of range", index),
                );
                return -1;
            }
            value
        };

        if unsafe { unpack(scanner, value, args) } != 0 {
            return -1;
        }

        scanner.next_token();
        index += 1;
    }

    if strict == 0 && (scanner.flags & JSON_STRICT) != 0 {
        strict = 1;
    }

    if !root.is_null() && strict == 1 && index != array::json_array_size(root) {
        let diff = array::json_array_size(root) as isize - index as isize;
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_end_of_input_expected,
            format_args!("{diff} array item(s) left unpacked"),
        );
        return -1;
    }

    0
}

unsafe fn unpack_string(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    if !root.is_null() && !unsafe { is_string(root) } {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected string, got {}", type_name(root)),
        );
        return -1;
    }

    if !scanner.validate_only() {
        let target = unsafe { next_string_target(args, scanner) };
        if target.is_null() {
            set_scanner_error(
                scanner,
                SOURCE_ARGS,
                json_error_code::json_error_null_value,
                format_args!("NULL string argument"),
            );
            return -1;
        }

        scanner.next_token();
        let len_target = if scanner.current_token() == b'%' {
            let len_target = unsafe { next_size_target(args, scanner) };
            if len_target.is_null() {
                set_scanner_error(
                    scanner,
                    SOURCE_ARGS,
                    json_error_code::json_error_null_value,
                    format_args!("NULL string length argument"),
                );
                return -1;
            }
            len_target
        } else {
            scanner.prev_token();
            null_mut()
        };

        if !root.is_null() {
            unsafe {
                *target = scalar::json_string_value(root);
                if !len_target.is_null() {
                    *len_target = scalar::json_string_length(root);
                }
            }
        }
    }

    0
}

unsafe fn unpack_integer(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    if !root.is_null() && !unsafe { is_integer(root) } {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected integer, got {}", type_name(root)),
        );
        return -1;
    }

    if !scanner.validate_only() {
        let target = unsafe { next_int_target(args, scanner) };
        if !root.is_null() {
            unsafe {
                *target = scalar::json_integer_value(root) as c_int;
            }
        }
    }

    0
}

unsafe fn unpack_json_int(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    if !root.is_null() && !unsafe { is_integer(root) } {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected integer, got {}", type_name(root)),
        );
        return -1;
    }

    if !scanner.validate_only() {
        let target = unsafe { next_json_int_target(args, scanner) };
        if !root.is_null() {
            unsafe {
                *target = scalar::json_integer_value(root);
            }
        }
    }

    0
}

unsafe fn unpack_boolean(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    if !root.is_null() && !(unsafe { is_true(root) } || unsafe { is_false(root) }) {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected true or false, got {}", type_name(root)),
        );
        return -1;
    }

    if !scanner.validate_only() {
        let target = unsafe { next_int_target(args, scanner) };
        if !root.is_null() {
            unsafe {
                *target = if is_true(root) { 1 } else { 0 };
            }
        }
    }

    0
}

unsafe fn unpack_real(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    if !root.is_null() && !unsafe { is_real(root) } {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected real, got {}", type_name(root)),
        );
        return -1;
    }

    if !scanner.validate_only() {
        let target = unsafe { next_double_target(args, scanner) };
        if !root.is_null() {
            unsafe {
                *target = scalar::json_real_value(root);
            }
        }
    }

    0
}

unsafe fn unpack_number(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
) -> c_int {
    if !root.is_null() && !unsafe { is_number(root) } {
        set_scanner_error(
            scanner,
            SOURCE_VALIDATION,
            json_error_code::json_error_wrong_type,
            format_args!("Expected real or integer, got {}", type_name(root)),
        );
        return -1;
    }

    if !scanner.validate_only() {
        let target = unsafe { next_double_target(args, scanner) };
        if !root.is_null() {
            unsafe {
                *target = scalar::json_number_value(root);
            }
        }
    }

    0
}

unsafe fn unpack_object_ref(
    scanner: &mut Scanner<'_>,
    root: *mut json_t,
    args: &mut UnpackArgs<'_>,
    incref_root: bool,
) -> c_int {
    if incref_root && !root.is_null() && !scanner.validate_only() {
        unsafe {
            crate::abi::incref(root);
        }
    }

    if !scanner.validate_only() {
        let target = unsafe { next_json_target(args, scanner) };
        if !root.is_null() {
            unsafe {
                *target = root;
            }
        }
    }

    0
}

#[no_mangle]
pub unsafe extern "C" fn jsonp_unpack_marshaled(
    root: *mut json_t,
    error: *mut json_error_t,
    flags: usize,
    fmt: *const c_char,
    args: *const jsonp_unpack_arg,
    args_len: usize,
) -> c_int {
    if root.is_null() {
        unsafe {
            jsonp_error_init(error, SOURCE_ROOT.as_ptr().cast::<c_char>());
        }
        set_error_at(
            error,
            -1,
            -1,
            0,
            SOURCE_ROOT,
            json_error_code::json_error_null_value,
            format_args!("NULL root value"),
        );
        return -1;
    }

    if fmt.is_null() {
        unsafe {
            jsonp_error_init(error, SOURCE_FORMAT.as_ptr().cast::<c_char>());
        }
        set_error_at(
            error,
            -1,
            -1,
            0,
            SOURCE_FORMAT,
            json_error_code::json_error_invalid_argument,
            format_args!("NULL or empty format string"),
        );
        return -1;
    }

    let fmt_bytes = unsafe { CStr::from_ptr(fmt).to_bytes() };
    if fmt_bytes.is_empty() {
        unsafe {
            jsonp_error_init(error, SOURCE_FORMAT.as_ptr().cast::<c_char>());
        }
        set_error_at(
            error,
            -1,
            -1,
            0,
            SOURCE_FORMAT,
            json_error_code::json_error_invalid_argument,
            format_args!("NULL or empty format string"),
        );
        return -1;
    }

    unsafe {
        jsonp_error_init(error, null());
    }

    let mut scanner = Scanner::new(error, flags, fmt_bytes);
    let marshaled = if args.is_null() || args_len == 0 {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(args, args_len) }
    };
    let mut args = UnpackArgs::new(marshaled);

    scanner.next_token();
    if unsafe { unpack(&mut scanner, root, &mut args) } != 0 {
        return -1;
    }

    scanner.next_token();
    if scanner.current_token() != 0 {
        set_scanner_error(
            &mut scanner,
            SOURCE_FORMAT,
            json_error_code::json_error_invalid_format,
            format_args!("Garbage after format string"),
        );
        return -1;
    }

    0
}
