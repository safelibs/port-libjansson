use std::ffi::{c_char, c_double, c_int, c_void, CStr};
use std::fmt::{self, Write as _};
use std::ptr::{null, null_mut};
use std::slice;

use crate::abi::{decref, incref, json_error_code, json_error_t, json_int_t, json_t};
use crate::array;
use crate::error::{jsonp_error_init, jsonp_error_set_source, jsonp_error_vformat};
use crate::object;
use crate::raw::alloc::jsonp_free;
use crate::raw::buf::RawBuf;
use crate::scalar;
use crate::utf;

const SOURCE_FORMAT: &[u8] = b"<format>\0";
const SOURCE_ARGS: &[u8] = b"<args>\0";
const SOURCE_INTERNAL: &[u8] = b"<internal>\0";

const PACK_ARG_CSTR: c_int = 1;
const PACK_ARG_INT: c_int = 2;
const PACK_ARG_SIZE: c_int = 3;
const PACK_ARG_DOUBLE: c_int = 4;
const PACK_ARG_JSON: c_int = 5;

#[repr(C)]
pub struct jsonp_pack_arg {
    pub kind: c_int,
    pub ptr: *const c_void,
    pub size: usize,
    pub integer: json_int_t,
    pub real: c_double,
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
    fmt: &'a [u8],
    index: usize,
    prev_token: Token,
    token: Token,
    next_token: Token,
    line: c_int,
    column: c_int,
    pos: usize,
    has_error: bool,
}

struct PackArgs<'a> {
    args: &'a [jsonp_pack_arg],
    index: usize,
    mismatch_reported: bool,
}

struct ErrorTextBuf {
    buf: [u8; crate::abi::JSON_ERROR_TEXT_LENGTH - 1],
    len: usize,
}

enum PackedString {
    Borrowed(*const c_char, usize),
    Owned(*mut c_char, usize),
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
    fn new(error: *mut json_error_t, fmt: &'a [u8]) -> Self {
        Self {
            error,
            fmt,
            index: 0,
            prev_token: Token::default(),
            token: Token::default(),
            next_token: Token::default(),
            line: 1,
            column: 0,
            pos: 0,
            has_error: false,
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
}

impl<'a> PackArgs<'a> {
    fn new(args: &'a [jsonp_pack_arg]) -> Self {
        Self {
            args,
            index: 0,
            mismatch_reported: false,
        }
    }

    fn next(&mut self, kind: c_int, scanner: &mut Scanner<'_>) -> Option<&'a jsonp_pack_arg> {
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
        scanner.has_error = true;
    }
}

impl PackedString {
    fn as_ptr_len(&self) -> (*const c_char, usize) {
        match *self {
            Self::Borrowed(ptr, len) => (ptr, len),
            Self::Owned(ptr, len) => (ptr.cast_const(), len),
        }
    }

    fn free(self) {
        if let Self::Owned(ptr, _) = self {
            unsafe {
                jsonp_free(ptr.cast::<c_void>());
            }
        }
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

unsafe fn next_cstr(args: &mut PackArgs<'_>, scanner: &mut Scanner<'_>) -> *const c_char {
    args.next(PACK_ARG_CSTR, scanner)
        .map(|arg| arg.ptr.cast::<c_char>())
        .unwrap_or(null())
}

unsafe fn next_int(args: &mut PackArgs<'_>, scanner: &mut Scanner<'_>) -> c_int {
    args.next(PACK_ARG_INT, scanner)
        .map(|arg| arg.integer as c_int)
        .unwrap_or_default()
}

unsafe fn next_size(args: &mut PackArgs<'_>, scanner: &mut Scanner<'_>) -> usize {
    args.next(PACK_ARG_SIZE, scanner)
        .map(|arg| arg.size)
        .unwrap_or_default()
}

unsafe fn next_double(args: &mut PackArgs<'_>, scanner: &mut Scanner<'_>) -> c_double {
    args.next(PACK_ARG_DOUBLE, scanner)
        .map(|arg| arg.real)
        .unwrap_or_default()
}

unsafe fn next_json(args: &mut PackArgs<'_>, scanner: &mut Scanner<'_>) -> *mut json_t {
    args.next(PACK_ARG_JSON, scanner)
        .map(|arg| arg.ptr.cast_mut().cast::<json_t>())
        .unwrap_or(null_mut())
}

unsafe fn validate_utf8(ptr: *const c_char, len: usize) -> bool {
    !ptr.is_null() && unsafe { utf::validate_ptr(ptr, len) }
}

unsafe fn read_string(
    scanner: &mut Scanner<'_>,
    args: &mut PackArgs<'_>,
    purpose: &str,
    optional: bool,
) -> Option<PackedString> {
    scanner.next_token();
    let modifier = scanner.current_token();
    scanner.prev_token();

    if modifier != b'#' && modifier != b'%' && modifier != b'+' {
        let value = unsafe { next_cstr(args, scanner) };
        if value.is_null() {
            if !optional {
                set_scanner_error(
                    scanner,
                    SOURCE_ARGS,
                    json_error_code::json_error_null_value,
                    format_args!("NULL {purpose}"),
                );
                scanner.has_error = true;
            }
            return None;
        }

        let len = unsafe { libc::strlen(value) };
        if !unsafe { validate_utf8(value, len) } {
            set_scanner_error(
                scanner,
                SOURCE_ARGS,
                json_error_code::json_error_invalid_utf8,
                format_args!("Invalid UTF-8 {purpose}"),
            );
            scanner.has_error = true;
            return None;
        }

        return Some(PackedString::Borrowed(value, len));
    }

    if optional {
        set_scanner_error(
            scanner,
            SOURCE_FORMAT,
            json_error_code::json_error_invalid_format,
            format_args!("Cannot use '{}' on optional strings", modifier as char),
        );
        scanner.has_error = true;
        return None;
    }

    let mut buffer = RawBuf::new();
    if unsafe { buffer.init() } != 0 {
        set_scanner_error(
            scanner,
            SOURCE_INTERNAL,
            json_error_code::json_error_out_of_memory,
            format_args!("Out of memory"),
        );
        scanner.has_error = true;
        return None;
    }

    loop {
        let value = unsafe { next_cstr(args, scanner) };
        if value.is_null() {
            set_scanner_error(
                scanner,
                SOURCE_ARGS,
                json_error_code::json_error_null_value,
                format_args!("NULL {purpose}"),
            );
            scanner.has_error = true;
        }

        scanner.next_token();
        let len = match scanner.current_token() {
            b'#' => unsafe { next_int(args, scanner) as usize },
            b'%' => unsafe { next_size(args, scanner) },
            _ => {
                scanner.prev_token();
                if scanner.has_error {
                    0
                } else {
                    unsafe { libc::strlen(value) }
                }
            }
        };

        if !scanner.has_error && unsafe { buffer.append_bytes(value.cast::<u8>(), len) } != 0 {
            set_scanner_error(
                scanner,
                SOURCE_INTERNAL,
                json_error_code::json_error_out_of_memory,
                format_args!("Out of memory"),
            );
            scanner.has_error = true;
        }

        scanner.next_token();
        if scanner.current_token() != b'+' {
            scanner.prev_token();
            break;
        }
    }

    if scanner.has_error {
        unsafe {
            buffer.close();
        }
        return None;
    }

    if !unsafe { validate_utf8(buffer.value(), buffer.len()) } {
        set_scanner_error(
            scanner,
            SOURCE_ARGS,
            json_error_code::json_error_invalid_utf8,
            format_args!("Invalid UTF-8 {purpose}"),
        );
        unsafe {
            buffer.close();
        }
        scanner.has_error = true;
        return None;
    }

    let len = buffer.len();
    let value = unsafe { buffer.steal_value() };
    Some(PackedString::Owned(value, len))
}

unsafe fn pack_integer(scanner: &mut Scanner<'_>, value: json_int_t) -> *mut json_t {
    let json = unsafe { scalar::json_integer(value) };
    if json.is_null() {
        set_scanner_error(
            scanner,
            SOURCE_INTERNAL,
            json_error_code::json_error_out_of_memory,
            format_args!("Out of memory"),
        );
        scanner.has_error = true;
    }
    json
}

unsafe fn pack_real(scanner: &mut Scanner<'_>, value: c_double) -> *mut json_t {
    let json = unsafe { scalar::json_real(0.0) };
    if json.is_null() {
        set_scanner_error(
            scanner,
            SOURCE_INTERNAL,
            json_error_code::json_error_out_of_memory,
            format_args!("Out of memory"),
        );
        scanner.has_error = true;
        return null_mut();
    }

    if unsafe { scalar::json_real_set(json, value) } != 0 {
        unsafe {
            decref(json);
        }
        set_scanner_error(
            scanner,
            SOURCE_ARGS,
            json_error_code::json_error_numeric_overflow,
            format_args!("Invalid floating point value"),
        );
        scanner.has_error = true;
        return null_mut();
    }

    json
}

unsafe fn pack_object_inter(
    scanner: &mut Scanner<'_>,
    args: &mut PackArgs<'_>,
    need_incref: bool,
) -> *mut json_t {
    scanner.next_token();
    let next = scanner.current_token();
    if next != b'?' && next != b'*' {
        scanner.prev_token();
    }

    let json = unsafe { next_json(args, scanner) };
    if !json.is_null() {
        return if need_incref {
            unsafe { incref(json) }
        } else {
            json
        };
    }

    match next {
        b'?' => scalar::json_null(),
        b'*' => null_mut(),
        _ => {
            set_scanner_error(
                scanner,
                SOURCE_ARGS,
                json_error_code::json_error_null_value,
                format_args!("NULL object"),
            );
            scanner.has_error = true;
            null_mut()
        }
    }
}

unsafe fn pack_string(scanner: &mut Scanner<'_>, args: &mut PackArgs<'_>) -> *mut json_t {
    scanner.next_token();
    let token = scanner.current_token();
    let optional = token == b'?' || token == b'*';
    if !optional {
        scanner.prev_token();
    }

    let Some(string) = (unsafe { read_string(scanner, args, "string", optional) }) else {
        return if token == b'?' && !scanner.has_error {
            scalar::json_null()
        } else {
            null_mut()
        };
    };

    if scanner.has_error {
        string.free();
        return null_mut();
    }

    let (value, len) = string.as_ptr_len();
    match string {
        PackedString::Borrowed(_, _) => unsafe { scalar::json_stringn_nocheck(value, len) },
        PackedString::Owned(ptr, _) => unsafe { scalar::jsonp_stringn_nocheck_own(ptr, len) },
    }
}

unsafe fn pack_object(scanner: &mut Scanner<'_>, args: &mut PackArgs<'_>) -> *mut json_t {
    let object = object::json_object();
    if object.is_null() {
        return null_mut();
    }

    scanner.next_token();
    while scanner.current_token() != b'}' {
        if scanner.current_token() == 0 {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!("Unexpected end of format string"),
            );
            unsafe {
                decref(object);
            }
            return null_mut();
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
                decref(object);
            }
            return null_mut();
        }

        let key = unsafe { read_string(scanner, args, "object key", false) };
        let (key_ptr, key_len) = key
            .as_ref()
            .map(PackedString::as_ptr_len)
            .unwrap_or((null(), 0));

        scanner.next_token();
        scanner.next_token();
        let value_optional = scanner.current_token();
        scanner.prev_token();

        let value = unsafe { pack(scanner, args) };
        if value.is_null() {
            if let Some(key) = key {
                key.free();
            }

            if value_optional != b'*' {
                set_scanner_error(
                    scanner,
                    SOURCE_ARGS,
                    json_error_code::json_error_null_value,
                    format_args!("NULL object value"),
                );
                scanner.has_error = true;
            }

            scanner.next_token();
            continue;
        }

        if scanner.has_error {
            unsafe {
                decref(value);
            }
        }

        if !scanner.has_error
            && unsafe { object::json_object_setn_new_nocheck(object, key_ptr, key_len, value) } != 0
        {
            let key_text = unsafe {
                String::from_utf8_lossy(slice::from_raw_parts(key_ptr.cast::<u8>(), key_len))
            };
            set_scanner_error(
                scanner,
                SOURCE_INTERNAL,
                json_error_code::json_error_out_of_memory,
                format_args!("Unable to add key \"{key_text}\""),
            );
            scanner.has_error = true;
        }

        if let Some(key) = key {
            key.free();
        }

        scanner.next_token();
    }

    if scanner.has_error {
        unsafe {
            decref(object);
        }
        null_mut()
    } else {
        object
    }
}

unsafe fn pack_array(scanner: &mut Scanner<'_>, args: &mut PackArgs<'_>) -> *mut json_t {
    let array = array::json_array();
    if array.is_null() {
        return null_mut();
    }

    scanner.next_token();
    while scanner.current_token() != b']' {
        if scanner.current_token() == 0 {
            set_scanner_error(
                scanner,
                SOURCE_FORMAT,
                json_error_code::json_error_invalid_format,
                format_args!("Unexpected end of format string"),
            );
            unsafe {
                decref(array);
            }
            return null_mut();
        }

        scanner.next_token();
        let value_optional = scanner.current_token();
        scanner.prev_token();

        let value = unsafe { pack(scanner, args) };
        if value.is_null() {
            if value_optional != b'*' {
                scanner.has_error = true;
            }

            scanner.next_token();
            continue;
        }

        if scanner.has_error {
            unsafe {
                decref(value);
            }
        }

        if !scanner.has_error && array::json_array_append_new(array, value) != 0 {
            set_scanner_error(
                scanner,
                SOURCE_INTERNAL,
                json_error_code::json_error_out_of_memory,
                format_args!("Unable to append to array"),
            );
            scanner.has_error = true;
        }

        scanner.next_token();
    }

    if scanner.has_error {
        unsafe {
            decref(array);
        }
        null_mut()
    } else {
        array
    }
}

unsafe fn pack(scanner: &mut Scanner<'_>, args: &mut PackArgs<'_>) -> *mut json_t {
    match scanner.current_token() {
        b'{' => unsafe { pack_object(scanner, args) },
        b'[' => unsafe { pack_array(scanner, args) },
        b's' => unsafe { pack_string(scanner, args) },
        b'n' => scalar::json_null(),
        b'b' => {
            if unsafe { next_int(args, scanner) } != 0 {
                scalar::json_true()
            } else {
                scalar::json_false()
            }
        }
        b'i' => unsafe {
            let value = next_int(args, scanner) as json_int_t;
            pack_integer(scanner, value)
        },
        b'I' => unsafe {
            let value = args
                .next(PACK_ARG_INT, scanner)
                .map(|arg| arg.integer)
                .unwrap_or_default();
            pack_integer(scanner, value)
        },
        b'f' => unsafe {
            let value = next_double(args, scanner);
            pack_real(scanner, value)
        },
        b'O' => unsafe { pack_object_inter(scanner, args, true) },
        b'o' => unsafe { pack_object_inter(scanner, args, false) },
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
            scanner.has_error = true;
            null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn jsonp_pack_marshaled(
    error: *mut json_error_t,
    _flags: usize,
    fmt: *const c_char,
    args: *const jsonp_pack_arg,
    args_len: usize,
) -> *mut json_t {
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
        return null_mut();
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
        return null_mut();
    }

    unsafe {
        jsonp_error_init(error, null());
    }

    let mut scanner = Scanner::new(error, fmt_bytes);
    let marshaled = if args.is_null() || args_len == 0 {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(args, args_len) }
    };
    let mut args = PackArgs::new(marshaled);

    scanner.next_token();
    let value = unsafe { pack(&mut scanner, &mut args) };
    if value.is_null() {
        return null_mut();
    }

    scanner.next_token();
    if scanner.current_token() != 0 {
        unsafe {
            decref(value);
        }
        set_scanner_error(
            &mut scanner,
            SOURCE_FORMAT,
            json_error_code::json_error_invalid_format,
            format_args!("Garbage after format string"),
        );
        return null_mut();
    }

    value
}
