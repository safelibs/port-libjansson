use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr::{self, null_mut};
use std::slice;

use libc::FILE;

use crate::abi::{
    json_error_code, json_error_t, json_int_t, json_load_callback_t, json_t,
};
use crate::error::{jsonp_error_init, jsonp_error_vformat};
use crate::raw::alloc::jsonp_free;
use crate::raw::buf::RawBuf;
use crate::strconv;
use crate::utf;

const STREAM_STATE_OK: c_int = 0;
const STREAM_STATE_EOF: c_int = -1;
const STREAM_STATE_ERROR: c_int = -2;

const TOKEN_INVALID: c_int = -1;
const TOKEN_EOF: c_int = 0;
const TOKEN_STRING: c_int = 256;
const TOKEN_INTEGER: c_int = 257;
const TOKEN_REAL: c_int = 258;
const TOKEN_TRUE: c_int = 259;
const TOKEN_FALSE: c_int = 260;
const TOKEN_NULL: c_int = 261;

const JSON_REJECT_DUPLICATES: usize = 0x1;
const JSON_DISABLE_EOF_CHECK: usize = 0x2;
const JSON_DECODE_ANY: usize = 0x4;
const JSON_DECODE_INT_AS_REAL: usize = 0x8;
const JSON_ALLOW_NUL: usize = 0x10;

const JSON_PARSER_MAX_DEPTH: usize = 2048;
const MAX_CALLBACK_BUF_LEN: usize = 1024;

const SOURCE_STRING: &[u8] = b"<string>\0";
const SOURCE_BUFFER: &[u8] = b"<buffer>\0";
const SOURCE_STREAM: &[u8] = b"<stream>\0";
const SOURCE_STDIN: &[u8] = b"<stdin>\0";
const SOURCE_CALLBACK: &[u8] = b"<callback>\0";
const MODE_READ_BINARY: &[u8] = b"rb\0";

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn errno_location() -> *mut c_int {
    unsafe { libc::__errno_location() }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
unsafe fn errno_location() -> *mut c_int {
    unsafe { libc::__error() }
}

#[cfg(windows)]
unsafe fn errno_location() -> *mut c_int {
    unsafe { libc::_errno() }
}

#[inline]
unsafe fn set_errno(value: c_int) {
    unsafe {
        *errno_location() = value;
    }
}

#[inline]
unsafe fn get_errno() -> c_int {
    unsafe { *errno_location() }
}

#[inline]
fn is_upper(c: u8) -> bool {
    c.is_ascii_uppercase()
}

#[inline]
fn is_lower(c: u8) -> bool {
    c.is_ascii_lowercase()
}

#[inline]
fn is_alpha(c: u8) -> bool {
    is_upper(c) || is_lower(c)
}

#[inline]
fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

#[inline]
fn is_xdigit(c: u8) -> bool {
    c.is_ascii_hexdigit()
}

#[derive(Clone, Copy)]
struct Stream {
    get: unsafe fn(*mut c_void) -> c_int,
    data: *mut c_void,
    buffer: [u8; 4],
    buffer_len: usize,
    buffer_pos: usize,
    state: c_int,
    line: c_int,
    column: c_int,
    last_column: c_int,
    position: usize,
}

impl Stream {
    fn new(get: unsafe fn(*mut c_void) -> c_int, data: *mut c_void) -> Self {
        Self {
            get,
            data,
            buffer: [0; 4],
            buffer_len: 0,
            buffer_pos: 0,
            state: STREAM_STATE_OK,
            line: 1,
            column: 0,
            last_column: 0,
            position: 0,
        }
    }
}

struct Lexer {
    stream: Stream,
    saved_text: RawBuf,
    flags: usize,
    token: c_int,
    string_val: *mut c_char,
    string_len: usize,
    integer: json_int_t,
    real: f64,
}

impl Lexer {
    unsafe fn init(
        get: unsafe fn(*mut c_void) -> c_int,
        flags: usize,
        data: *mut c_void,
    ) -> Option<Self> {
        let mut saved_text = RawBuf::new();
        if unsafe { saved_text.init() } != 0 {
            return None;
        }

        Some(Self {
            stream: Stream::new(get, data),
            saved_text,
            flags,
            token: TOKEN_INVALID,
            string_val: null_mut(),
            string_len: 0,
            integer: 0,
            real: 0.0,
        })
    }

    unsafe fn close(&mut self) {
        if self.token == TOKEN_STRING {
            unsafe {
                self.free_string();
            }
        }
        unsafe {
            self.saved_text.close();
        }
    }

    unsafe fn free_string(&mut self) {
        unsafe {
            jsonp_free(self.string_val.cast());
        }
        self.string_val = null_mut();
        self.string_len = 0;
    }

    fn saved_text_bytes(&self) -> &[u8] {
        if self.saved_text.len() == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(self.saved_text.value().cast::<u8>(), self.saved_text.len()) }
        }
    }

    fn saved_text_display(&self) -> String {
        if self.saved_text.value().is_null() {
            return String::new();
        }

        unsafe { CStr::from_ptr(self.saved_text.value()) }
            .to_string_lossy()
            .into_owned()
    }

    unsafe fn error_set(
        &self,
        error: *mut json_error_t,
        mut code: json_error_code,
        message: String,
    ) {
        if error.is_null() {
            return;
        }

        let mut result = message;
        let line = self.stream.line;
        let column = self.stream.column;
        let position = self.stream.position;

        let saved_bytes = self.saved_text_bytes();
        if !saved_bytes.is_empty() && saved_bytes[0] != 0 {
            if self.saved_text.len() <= 20 {
                result = format!("{} near '{}'", result, self.saved_text_display());
            }
        } else {
            if code == json_error_code::json_error_invalid_syntax {
                code = json_error_code::json_error_premature_end_of_input;
            }

            if self.stream.state != STREAM_STATE_ERROR {
                result = format!("{} near end of file", result);
            }
        }

        if let Ok(text) = CString::new(result) {
            unsafe {
                jsonp_error_vformat(error, line, column, position, code as c_int, text.as_ptr());
            }
        }
    }

    unsafe fn plain_error_set(
        error: *mut json_error_t,
        code: json_error_code,
        message: String,
    ) {
        if error.is_null() {
            return;
        }

        if let Ok(text) = CString::new(message) {
            unsafe {
                jsonp_error_vformat(error, -1, -1, 0, code as c_int, text.as_ptr());
            }
        }
    }

    unsafe fn oom(&self, error: *mut json_error_t) {
        unsafe {
            self.error_set(error, json_error_code::json_error_out_of_memory, "out of memory".into());
        }
    }

    unsafe fn get_char(&mut self, error: *mut json_error_t) -> c_int {
        if self.stream.state != STREAM_STATE_OK {
            return self.stream.state;
        }

        if self.stream.buffer_pos >= self.stream.buffer_len {
            let c = unsafe { (self.stream.get)(self.stream.data) };
            if c == libc::EOF {
                self.stream.state = STREAM_STATE_EOF;
                return STREAM_STATE_EOF;
            }

            let byte = c as u8;
            let count = utf::check_first(byte);
            if count == 0 {
                self.stream.state = STREAM_STATE_ERROR;
                unsafe {
                    self.error_set(
                        error,
                        json_error_code::json_error_invalid_utf8,
                        format!("unable to decode byte 0x{:x}", byte),
                    );
                }
                return STREAM_STATE_ERROR;
            }

            self.stream.buffer[0] = byte;
            self.stream.buffer_len = count;
            self.stream.buffer_pos = 0;

            if count > 1 {
                let mut index = 1usize;
                while index < count {
                    let next = unsafe { (self.stream.get)(self.stream.data) };
                    if next == libc::EOF {
                        self.stream.state = STREAM_STATE_ERROR;
                        unsafe {
                            self.error_set(
                                error,
                                json_error_code::json_error_invalid_utf8,
                                format!("unable to decode byte 0x{:x}", byte),
                            );
                        }
                        return STREAM_STATE_ERROR;
                    }

                    self.stream.buffer[index] = next as u8;
                    index += 1;
                }

                if utf::check_full(&self.stream.buffer[..count], count).is_none() {
                    self.stream.state = STREAM_STATE_ERROR;
                    unsafe {
                        self.error_set(
                            error,
                            json_error_code::json_error_invalid_utf8,
                            format!("unable to decode byte 0x{:x}", byte),
                        );
                    }
                    return STREAM_STATE_ERROR;
                }
            }
        }

        let c = self.stream.buffer[self.stream.buffer_pos];
        self.stream.buffer_pos += 1;
        self.stream.position += 1;

        if c == b'\n' {
            self.stream.line += 1;
            self.stream.last_column = self.stream.column;
            self.stream.column = 0;
        } else if utf::check_first(c) != 0 {
            self.stream.column += 1;
        }

        c as c_int
    }

    unsafe fn unget_char(&mut self, c: c_int) {
        if matches!(c, STREAM_STATE_EOF | STREAM_STATE_ERROR) {
            return;
        }

        self.stream.position -= 1;
        if c == b'\n' as c_int {
            self.stream.line -= 1;
            self.stream.column = self.stream.last_column;
        } else if utf::check_first(c as u8) != 0 {
            self.stream.column -= 1;
        }

        self.stream.buffer_pos -= 1;
    }

    unsafe fn save_char(&mut self, c: c_int) {
        let byte = c as c_char;
        unsafe {
            self.saved_text.append_byte(byte);
        }
    }

    unsafe fn get_and_save(&mut self, error: *mut json_error_t) -> c_int {
        let c = unsafe { self.get_char(error) };
        if !matches!(c, STREAM_STATE_EOF | STREAM_STATE_ERROR) {
            unsafe {
                self.save_char(c);
            }
        }
        c
    }

    unsafe fn unget_and_unsave(&mut self, c: c_int) {
        if matches!(c, STREAM_STATE_EOF | STREAM_STATE_ERROR) {
            return;
        }

        unsafe {
            self.unget_char(c);
            self.saved_text.pop();
        }
    }

    unsafe fn save_cached(&mut self) {
        while self.stream.buffer_pos < self.stream.buffer_len {
            let byte = self.stream.buffer[self.stream.buffer_pos];
            unsafe {
                self.saved_text.append_byte(byte as c_char);
            }
            self.stream.buffer_pos += 1;
            self.stream.position += 1;
        }
    }

    unsafe fn steal_string(&mut self) -> Option<(*mut c_char, usize)> {
        if self.token != TOKEN_STRING {
            return None;
        }

        let result = (self.string_val, self.string_len);
        self.string_val = null_mut();
        self.string_len = 0;
        Some(result)
    }

    unsafe fn scan_string(&mut self, error: *mut json_error_t) {
        self.string_val = null_mut();
        self.string_len = 0;
        self.token = TOKEN_INVALID;

        let mut c = unsafe { self.get_and_save(error) };
        while c != b'"' as c_int {
            if c == STREAM_STATE_ERROR {
                return;
            }
            if c == STREAM_STATE_EOF {
                unsafe {
                    self.error_set(
                        error,
                        json_error_code::json_error_premature_end_of_input,
                        "premature end of input".into(),
                    );
                }
                return;
            }
            if (0..=0x1F).contains(&c) {
                unsafe {
                    self.unget_and_unsave(c);
                    if c == b'\n' as c_int {
                        self.error_set(
                            error,
                            json_error_code::json_error_invalid_syntax,
                            "unexpected newline".into(),
                        );
                    } else {
                        self.error_set(
                            error,
                            json_error_code::json_error_invalid_syntax,
                            format!("control character 0x{:x}", c),
                        );
                    }
                }
                return;
            }
            if c == b'\\' as c_int {
                c = unsafe { self.get_and_save(error) };
                if c == b'u' as c_int {
                    c = unsafe { self.get_and_save(error) };
                    for _ in 0..4 {
                        if c == STREAM_STATE_ERROR || !is_xdigit(c as u8) {
                            unsafe {
                                self.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "invalid escape".into(),
                                );
                            }
                            return;
                        }
                        c = unsafe { self.get_and_save(error) };
                    }
                } else if matches!(
                    c,
                    x if x == b'"' as c_int
                        || x == b'\\' as c_int
                        || x == b'/' as c_int
                        || x == b'b' as c_int
                        || x == b'f' as c_int
                        || x == b'n' as c_int
                        || x == b'r' as c_int
                        || x == b't' as c_int
                ) {
                    c = unsafe { self.get_and_save(error) };
                } else {
                    unsafe {
                        self.error_set(
                            error,
                            json_error_code::json_error_invalid_syntax,
                            "invalid escape".into(),
                        );
                    }
                    return;
                }
            } else {
                c = unsafe { self.get_and_save(error) };
            }
        }

        let out = unsafe { crate::raw::alloc::jsonp_malloc(self.saved_text.len() + 1) }.cast::<c_char>();
        if out.is_null() {
            unsafe {
                self.oom(error);
            }
            return;
        }

        let saved = self.saved_text_bytes();
        let mut source = 1usize;
        let mut dest = 0usize;

        while source < saved.len().saturating_sub(1) {
            let byte = saved[source];
            if byte == b'\\' {
                source += 1;
                match saved[source] {
                    b'u' => {
                        let value = match decode_unicode_escape(&saved[source..]) {
                            Some(value) => value,
                            None => {
                                unsafe {
                                    jsonp_free(out.cast());
                                    self.error_set(
                                        error,
                                        json_error_code::json_error_invalid_syntax,
                                        format!(
                                            "invalid Unicode escape '{}'",
                                            String::from_utf8_lossy(
                                                &saved[source.saturating_sub(1)
                                                    ..(source + 5).min(saved.len())]
                                            )
                                        ),
                                    );
                                }
                                return;
                            }
                        };
                        source += 5;

                        let scalar = if (0xD800..=0xDBFF).contains(&value) {
                            if source + 6 <= saved.len()
                                && saved[source] == b'\\'
                                && saved[source + 1] == b'u'
                            {
                                let value2 = match decode_unicode_escape(&saved[source + 1..]) {
                                    Some(value2) => value2,
                                    None => {
                                        unsafe {
                                            jsonp_free(out.cast());
                                            self.error_set(
                                                error,
                                                json_error_code::json_error_invalid_syntax,
                                                format!(
                                                    "invalid Unicode escape '{}'",
                                                    String::from_utf8_lossy(
                                                        &saved[source..(source + 6).min(saved.len())]
                                                    )
                                                ),
                                            );
                                        }
                                        return;
                                    }
                                };
                                source += 6;

                                if !(0xDC00..=0xDFFF).contains(&value2) {
                                    unsafe {
                                        jsonp_free(out.cast());
                                        self.error_set(
                                            error,
                                            json_error_code::json_error_invalid_syntax,
                                            format!(
                                                "invalid Unicode '\\u{:04X}\\u{:04X}'",
                                                value, value2
                                            ),
                                        );
                                    }
                                    return;
                                }

                                (((value - 0xD800) as u32) << 10)
                                    + (value2 - 0xDC00) as u32
                                    + 0x10000
                            } else {
                                unsafe {
                                    jsonp_free(out.cast());
                                    self.error_set(
                                        error,
                                        json_error_code::json_error_invalid_syntax,
                                        format!("invalid Unicode '\\u{:04X}'", value),
                                    );
                                }
                                return;
                            }
                        } else if (0xDC00..=0xDFFF).contains(&value) {
                            unsafe {
                                jsonp_free(out.cast());
                                self.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    format!("invalid Unicode '\\u{:04X}'", value),
                                );
                            }
                            return;
                        } else {
                            value as u32
                        };

                        let mut utf8 = [0u8; 4];
                        let width = utf::encode(scalar, &mut utf8).unwrap_or(0);
                        unsafe {
                            ptr::copy_nonoverlapping(
                                utf8.as_ptr().cast::<c_char>(),
                                out.add(dest),
                                width,
                            );
                        }
                        dest += width;
                    }
                    b'"' | b'\\' | b'/' => {
                        unsafe {
                            *out.add(dest) = saved[source] as c_char;
                        }
                        dest += 1;
                        source += 1;
                    }
                    b'b' => {
                        unsafe {
                            *out.add(dest) = b'\x08' as c_char;
                        }
                        dest += 1;
                        source += 1;
                    }
                    b'f' => {
                        unsafe {
                            *out.add(dest) = b'\x0c' as c_char;
                        }
                        dest += 1;
                        source += 1;
                    }
                    b'n' => {
                        unsafe {
                            *out.add(dest) = b'\n' as c_char;
                        }
                        dest += 1;
                        source += 1;
                    }
                    b'r' => {
                        unsafe {
                            *out.add(dest) = b'\r' as c_char;
                        }
                        dest += 1;
                        source += 1;
                    }
                    b't' => {
                        unsafe {
                            *out.add(dest) = b'\t' as c_char;
                        }
                        dest += 1;
                        source += 1;
                    }
                    _ => unreachable!(),
                }
            } else {
                unsafe {
                    *out.add(dest) = byte as c_char;
                }
                dest += 1;
                source += 1;
            }
        }

        unsafe {
            *out.add(dest) = 0;
        }
        self.string_val = out;
        self.string_len = dest;
        self.token = TOKEN_STRING;
    }

    unsafe fn scan_number(&mut self, mut c: c_int, error: *mut json_error_t) -> c_int {
        self.token = TOKEN_INVALID;

        if c == b'-' as c_int {
            c = unsafe { self.get_and_save(error) };
        }

        if c == b'0' as c_int {
            c = unsafe { self.get_and_save(error) };
            if c >= 0 && is_digit(c as u8) {
                unsafe {
                    self.unget_and_unsave(c);
                }
                return -1;
            }
        } else if c >= 0 && is_digit(c as u8) {
            loop {
                c = unsafe { self.get_and_save(error) };
                if !(c >= 0 && is_digit(c as u8)) {
                    break;
                }
            }
        } else {
            unsafe {
                self.unget_and_unsave(c);
            }
            return -1;
        }

        if self.flags & JSON_DECODE_INT_AS_REAL == 0
            && c != b'.' as c_int
            && c != b'E' as c_int
            && c != b'e' as c_int
        {
            unsafe {
                self.unget_and_unsave(c);
            }

            let saved = self.saved_text.value();
            let mut end = ptr::null_mut();
            unsafe {
                set_errno(0);
            }
            let value = unsafe { libc::strtoll(saved, &mut end, 10) };
            if unsafe { get_errno() } == libc::ERANGE {
                unsafe {
                    self.error_set(
                        error,
                        json_error_code::json_error_numeric_overflow,
                        if value < 0 {
                            "too big negative integer".into()
                        } else {
                            "too big integer".into()
                        },
                    );
                }
                return -1;
            }

            self.integer = value as json_int_t;
            self.token = TOKEN_INTEGER;
            return 0;
        }

        if c == b'.' as c_int {
            c = unsafe { self.get_char(error) };
            if !(c >= 0 && is_digit(c as u8)) {
                unsafe {
                    self.unget_char(c);
                }
                return -1;
            }
            unsafe {
                self.save_char(c);
            }

            loop {
                c = unsafe { self.get_and_save(error) };
                if !(c >= 0 && is_digit(c as u8)) {
                    break;
                }
            }
        }

        if c == b'E' as c_int || c == b'e' as c_int {
            c = unsafe { self.get_and_save(error) };
            if c == b'+' as c_int || c == b'-' as c_int {
                c = unsafe { self.get_and_save(error) };
            }
            if !(c >= 0 && is_digit(c as u8)) {
                unsafe {
                    self.unget_and_unsave(c);
                }
                return -1;
            }

            loop {
                c = unsafe { self.get_and_save(error) };
                if !(c >= 0 && is_digit(c as u8)) {
                    break;
                }
            }
        }

        unsafe {
            self.unget_and_unsave(c);
        }

        match unsafe { strconv::strtod(&mut self.saved_text) } {
            Ok(value) => {
                self.real = value;
                self.token = TOKEN_REAL;
                0
            }
            Err(()) => {
                unsafe {
                    self.error_set(
                        error,
                        json_error_code::json_error_numeric_overflow,
                        "real number overflow".into(),
                    );
                }
                -1
            }
        }
    }

    unsafe fn scan(&mut self, error: *mut json_error_t) -> c_int {
        unsafe {
            self.saved_text.clear();
        }

        if self.token == TOKEN_STRING {
            unsafe {
                self.free_string();
            }
        }

        let mut c;
        loop {
            c = unsafe { self.get_char(error) };
            if !matches!(c, x if x == b' ' as c_int || x == b'\t' as c_int || x == b'\n' as c_int || x == b'\r' as c_int) {
                break;
            }
        }

        if c == STREAM_STATE_EOF {
            self.token = TOKEN_EOF;
            return self.token;
        }
        if c == STREAM_STATE_ERROR {
            self.token = TOKEN_INVALID;
            return self.token;
        }

        unsafe {
            self.save_char(c);
        }

        self.token = match c {
            x if matches!(
                x,
                y if y == b'{' as c_int
                    || y == b'}' as c_int
                    || y == b'[' as c_int
                    || y == b']' as c_int
                    || y == b':' as c_int
                    || y == b',' as c_int
            ) => x,
            x if x == b'"' as c_int => {
                unsafe {
                    self.scan_string(error);
                }
                self.token
            }
            x if x >= 0 && (is_digit(x as u8) || x == b'-' as c_int) => {
                if unsafe { self.scan_number(c, error) } != 0 {
                    TOKEN_INVALID
                } else {
                    self.token
                }
            }
            x if x >= 0 && is_alpha(x as u8) => {
                loop {
                    c = unsafe { self.get_and_save(error) };
                    if !(c >= 0 && is_alpha(c as u8)) {
                        break;
                    }
                }
                unsafe {
                    self.unget_and_unsave(c);
                }

                match self.saved_text_display().as_str() {
                    "true" => TOKEN_TRUE,
                    "false" => TOKEN_FALSE,
                    "null" => TOKEN_NULL,
                    _ => TOKEN_INVALID,
                }
            }
            _ => {
                unsafe {
                    self.save_cached();
                }
                TOKEN_INVALID
            }
        };

        self.token
    }
}

fn decode_unicode_escape(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 5 || bytes[0] != b'u' {
        return None;
    }

    let mut value = 0u16;
    for &byte in &bytes[1..5] {
        value <<= 4;
        value |= match byte {
            b'0'..=b'9' => (byte - b'0') as u16,
            b'a'..=b'f' => (byte - b'a' + 10) as u16,
            b'A'..=b'F' => (byte - b'A' + 10) as u16,
            _ => return None,
        };
    }

    Some(value)
}

struct StringData {
    data: *const c_char,
    pos: usize,
}

unsafe fn string_get(data: *mut c_void) -> c_int {
    let state = unsafe { &mut *data.cast::<StringData>() };
    let c = unsafe { *state.data.add(state.pos) };
    if c == 0 {
        libc::EOF
    } else {
        state.pos += 1;
        c as u8 as c_int
    }
}

struct BufferData {
    data: *const c_char,
    len: usize,
    pos: usize,
}

unsafe fn buffer_get(data: *mut c_void) -> c_int {
    let state = unsafe { &mut *data.cast::<BufferData>() };
    if state.pos >= state.len {
        libc::EOF
    } else {
        let c = unsafe { *state.data.add(state.pos) };
        state.pos += 1;
        c as u8 as c_int
    }
}

unsafe fn file_get(data: *mut c_void) -> c_int {
    unsafe { libc::fgetc(data.cast::<FILE>()) }
}

unsafe fn fd_get(data: *mut c_void) -> c_int {
    let fd = unsafe { *data.cast::<c_int>() };
    let mut byte = 0u8;
    let count = unsafe { libc::read(fd, ptr::addr_of_mut!(byte).cast::<c_void>(), 1) };
    if count == 1 {
        byte as c_int
    } else {
        libc::EOF
    }
}

struct CallbackData {
    data: [u8; MAX_CALLBACK_BUF_LEN],
    len: usize,
    pos: usize,
    callback: json_load_callback_t,
    arg: *mut c_void,
}

unsafe fn callback_get(data: *mut c_void) -> c_int {
    let state = unsafe { &mut *data.cast::<CallbackData>() };
    if state.pos >= state.len {
        state.pos = 0;
        let Some(callback) = state.callback else {
            return libc::EOF;
        };

        state.len = unsafe {
            callback(
                state.data.as_mut_ptr().cast::<c_void>(),
                MAX_CALLBACK_BUF_LEN,
                state.arg,
            )
        };
        if state.len == 0 || state.len == usize::MAX {
            return libc::EOF;
        }
    }

    let c = state.data[state.pos];
    state.pos += 1;
    c as c_int
}

enum Frame {
    Array {
        value: *mut json_t,
        state: ArrayState,
    },
    Object {
        value: *mut json_t,
        state: ObjectState,
        key: *mut c_char,
        key_len: usize,
    },
}

#[derive(Clone, Copy)]
enum ArrayState {
    ExpectFirstValueOrEnd,
    ExpectValue,
    ExpectCommaOrEnd,
}

#[derive(Clone, Copy)]
enum ObjectState {
    ExpectFirstKeyOrEnd,
    ExpectKey,
    ExpectColon,
    ExpectValue,
    ExpectCommaOrEnd,
}

impl Frame {
    fn value(&self) -> *mut json_t {
        match *self {
            Frame::Array { value, .. } | Frame::Object { value, .. } => value,
        }
    }
}

unsafe fn cleanup_frames(frames: &mut Vec<Frame>, pending: *mut json_t) {
    if !pending.is_null() {
        unsafe {
            crate::abi::decref(pending);
        }
    }

    while let Some(frame) = frames.pop() {
        if let Frame::Object { key, .. } = frame {
            unsafe {
                jsonp_free(key.cast());
            }
        }
        unsafe {
            crate::abi::decref(frame.value());
        }
    }
}

unsafe fn push_value_frame(
    lex: &mut Lexer,
    frames: &mut Vec<Frame>,
    flags: usize,
    error: *mut json_error_t,
) -> Result<Option<*mut json_t>, ()> {
    if frames.len() + 1 > JSON_PARSER_MAX_DEPTH {
        unsafe {
            lex.error_set(
                error,
                json_error_code::json_error_stack_overflow,
                "maximum parsing depth reached".into(),
            );
        }
        return Err(());
    }

    match lex.token {
        TOKEN_STRING => {
            let Some((value, len)) = (unsafe { lex.steal_string() }) else {
                unsafe {
                    lex.oom(error);
                }
                return Err(());
            };

            let bytes = unsafe { slice::from_raw_parts(value.cast::<u8>(), len) };
            if flags & JSON_ALLOW_NUL == 0 && bytes.contains(&0) {
                unsafe {
                    jsonp_free(value.cast());
                    lex.error_set(
                        error,
                        json_error_code::json_error_null_character,
                        "\\u0000 is not allowed without JSON_ALLOW_NUL".into(),
                    );
                }
                return Err(());
            }

            let json = unsafe { crate::scalar::jsonp_stringn_nocheck_own(value, len) };
            if json.is_null() {
                unsafe {
                    lex.oom(error);
                }
                return Err(());
            }
            Ok(Some(json))
        }
        TOKEN_INTEGER => {
            let json = unsafe { crate::scalar::json_integer(lex.integer) };
            if json.is_null() {
                unsafe {
                    lex.oom(error);
                }
                return Err(());
            }
            Ok(Some(json))
        }
        TOKEN_REAL => {
            let json = unsafe { crate::scalar::json_real(lex.real) };
            if json.is_null() {
                unsafe {
                    lex.oom(error);
                }
                return Err(());
            }
            Ok(Some(json))
        }
        TOKEN_TRUE => Ok(Some(crate::scalar::json_true())),
        TOKEN_FALSE => Ok(Some(crate::scalar::json_false())),
        TOKEN_NULL => Ok(Some(crate::scalar::json_null())),
        x if x == b'{' as c_int => {
            let object = crate::object::json_object();
            if object.is_null() {
                unsafe {
                    lex.oom(error);
                }
                return Err(());
            }
            frames.push(Frame::Object {
                value: object,
                state: ObjectState::ExpectFirstKeyOrEnd,
                key: null_mut(),
                key_len: 0,
            });
            unsafe {
                lex.scan(error);
            }
            Ok(None)
        }
        x if x == b'[' as c_int => {
            let array = crate::array::json_array();
            if array.is_null() {
                unsafe {
                    lex.oom(error);
                }
                return Err(());
            }
            frames.push(Frame::Array {
                value: array,
                state: ArrayState::ExpectFirstValueOrEnd,
            });
            unsafe {
                lex.scan(error);
            }
            Ok(None)
        }
        TOKEN_INVALID => {
            unsafe {
                lex.error_set(
                    error,
                    json_error_code::json_error_invalid_syntax,
                    "invalid token".into(),
                );
            }
            Err(())
        }
        _ => {
            unsafe {
                lex.error_set(
                    error,
                    json_error_code::json_error_invalid_syntax,
                    "unexpected token".into(),
                );
            }
            Err(())
        }
    }
}

unsafe fn parse_json(lex: &mut Lexer, flags: usize, error: *mut json_error_t) -> *mut json_t {
    let mut frames = Vec::new();
    let mut pending: *mut json_t = null_mut();

    unsafe {
        lex.scan(error);
    }

    if flags & JSON_DECODE_ANY == 0 && lex.token != b'[' as c_int && lex.token != b'{' as c_int {
        unsafe {
            lex.error_set(
                error,
                json_error_code::json_error_invalid_syntax,
                "'[' or '{' expected".into(),
            );
        }
        return null_mut();
    }

    let root = loop {
        if !pending.is_null() {
            if let Some(frame) = frames.last_mut() {
                match frame {
                    Frame::Array { value, state } => {
                        let child = std::mem::replace(&mut pending, null_mut());
                        if crate::array::json_array_append_new(*value, child) != 0 {
                            unsafe {
                                lex.oom(error);
                                cleanup_frames(&mut frames, child);
                            }
                            return null_mut();
                        }
                        *state = ArrayState::ExpectCommaOrEnd;
                        unsafe {
                            lex.scan(error);
                        }
                        continue;
                    }
                    Frame::Object {
                        value,
                        state,
                        key,
                        key_len,
                    } => {
                        let child = std::mem::replace(&mut pending, null_mut());
                        if unsafe {
                            crate::object::json_object_setn_new_nocheck(
                                *value, *key, *key_len, child,
                            )
                        } != 0
                        {
                            unsafe {
                                lex.oom(error);
                                jsonp_free((*key).cast());
                                *key = null_mut();
                                *key_len = 0;
                                cleanup_frames(&mut frames, child);
                            }
                            return null_mut();
                        }
                        unsafe {
                            jsonp_free((*key).cast());
                        }
                        *key = null_mut();
                        *key_len = 0;
                        *state = ObjectState::ExpectCommaOrEnd;
                        unsafe {
                            lex.scan(error);
                        }
                        continue;
                    }
                }
            }

            break pending;
        }

        let next = if let Some(frame) = frames.last_mut() {
            match frame {
                Frame::Array { value, state } => match *state {
                    ArrayState::ExpectFirstValueOrEnd => {
                        if lex.token == b']' as c_int {
                            pending = *value;
                            frames.pop();
                            None
                        } else if lex.token == TOKEN_EOF {
                            unsafe {
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "']' expected".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        } else {
                            match unsafe { push_value_frame(lex, &mut frames, flags, error) } {
                                Ok(result) => result,
                                Err(()) => {
                                    unsafe {
                                        cleanup_frames(&mut frames, null_mut());
                                    }
                                    return null_mut();
                                }
                            }
                        }
                    }
                    ArrayState::ExpectValue => {
                        if lex.token == TOKEN_EOF {
                            unsafe {
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "']' expected".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }
                        match unsafe { push_value_frame(lex, &mut frames, flags, error) } {
                            Ok(result) => result,
                            Err(()) => {
                                unsafe {
                                    cleanup_frames(&mut frames, null_mut());
                                }
                                return null_mut();
                            }
                        }
                    }
                    ArrayState::ExpectCommaOrEnd => {
                        if lex.token == b',' as c_int {
                            *state = ArrayState::ExpectValue;
                            unsafe {
                                lex.scan(error);
                            }
                            None
                        } else if lex.token == b']' as c_int {
                            pending = *value;
                            frames.pop();
                            None
                        } else {
                            unsafe {
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "']' expected".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }
                    }
                },
                Frame::Object {
                    value,
                    state,
                    key,
                    key_len,
                } => match *state {
                    ObjectState::ExpectFirstKeyOrEnd => {
                        if lex.token == b'}' as c_int {
                            pending = *value;
                            frames.pop();
                            None
                        } else if lex.token != TOKEN_STRING {
                            unsafe {
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "string or '}' expected".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        } else {
                            let Some((name, len)) = (unsafe { lex.steal_string() }) else {
                                unsafe {
                                    lex.oom(error);
                                    cleanup_frames(&mut frames, null_mut());
                                }
                                return null_mut();
                            };

                            if unsafe { slice::from_raw_parts(name.cast::<u8>(), len) }.contains(&0) {
                                unsafe {
                                    jsonp_free(name.cast());
                                    lex.error_set(
                                        error,
                                        json_error_code::json_error_null_byte_in_key,
                                        "NUL byte in object key not supported".into(),
                                    );
                                    cleanup_frames(&mut frames, null_mut());
                                }
                                return null_mut();
                            }

                            if flags & JSON_REJECT_DUPLICATES != 0
                                && unsafe { crate::object::json_object_getn(*value, name, len) }.is_null() == false
                            {
                                unsafe {
                                    jsonp_free(name.cast());
                                    lex.error_set(
                                        error,
                                        json_error_code::json_error_duplicate_key,
                                        "duplicate object key".into(),
                                    );
                                    cleanup_frames(&mut frames, null_mut());
                                }
                                return null_mut();
                            }

                            *key = name;
                            *key_len = len;
                            *state = ObjectState::ExpectColon;
                            unsafe {
                                lex.scan(error);
                            }
                            None
                        }
                    }
                    ObjectState::ExpectKey => {
                        if lex.token != TOKEN_STRING {
                            unsafe {
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "string or '}' expected".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }

                        let Some((name, len)) = (unsafe { lex.steal_string() }) else {
                            unsafe {
                                lex.oom(error);
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        };

                        if unsafe { slice::from_raw_parts(name.cast::<u8>(), len) }.contains(&0) {
                            unsafe {
                                jsonp_free(name.cast());
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_null_byte_in_key,
                                    "NUL byte in object key not supported".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }

                        if flags & JSON_REJECT_DUPLICATES != 0
                            && unsafe { crate::object::json_object_getn(*value, name, len) }.is_null() == false
                        {
                            unsafe {
                                jsonp_free(name.cast());
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_duplicate_key,
                                    "duplicate object key".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }

                        *key = name;
                        *key_len = len;
                        *state = ObjectState::ExpectColon;
                        unsafe {
                            lex.scan(error);
                        }
                        None
                    }
                    ObjectState::ExpectColon => {
                        if lex.token != b':' as c_int {
                            unsafe {
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "':' expected".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }

                        *state = ObjectState::ExpectValue;
                        unsafe {
                            lex.scan(error);
                        }
                        None
                    }
                    ObjectState::ExpectValue => match unsafe { push_value_frame(lex, &mut frames, flags, error) } {
                        Ok(result) => result,
                        Err(()) => {
                            unsafe {
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }
                    },
                    ObjectState::ExpectCommaOrEnd => {
                        if lex.token == b',' as c_int {
                            *state = ObjectState::ExpectKey;
                            unsafe {
                                lex.scan(error);
                            }
                            None
                        } else if lex.token == b'}' as c_int {
                            pending = *value;
                            frames.pop();
                            None
                        } else {
                            unsafe {
                                lex.error_set(
                                    error,
                                    json_error_code::json_error_invalid_syntax,
                                    "'}' expected".into(),
                                );
                                cleanup_frames(&mut frames, null_mut());
                            }
                            return null_mut();
                        }
                    }
                },
            }
        } else {
            match unsafe { push_value_frame(lex, &mut frames, flags, error) } {
                Ok(result) => result,
                Err(()) => {
                    unsafe {
                        cleanup_frames(&mut frames, null_mut());
                    }
                    return null_mut();
                }
            }
        };

        if let Some(value) = next {
            pending = value;
        }
    };

    if flags & JSON_DISABLE_EOF_CHECK == 0 {
        unsafe {
            lex.scan(error);
        }
        if lex.token != TOKEN_EOF {
            unsafe {
                lex.error_set(
                    error,
                    json_error_code::json_error_end_of_input_expected,
                    "end of file expected".into(),
                );
                crate::abi::decref(root);
            }
            return null_mut();
        }
    }

    if !error.is_null() {
        unsafe {
            (*error).position = lex.stream.position as c_int;
        }
    }

    root
}

unsafe fn parse_with_source(
    get: unsafe fn(*mut c_void) -> c_int,
    data: *mut c_void,
    flags: usize,
    error: *mut json_error_t,
    source: *const c_char,
) -> *mut json_t {
    unsafe {
        jsonp_error_init(error, source);
    }

    let Some(mut lex) = (unsafe { Lexer::init(get, flags, data) }) else {
        return null_mut();
    };

    let result = unsafe { parse_json(&mut lex, flags, error) };
    unsafe {
        lex.close();
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn json_loads(
    input: *const c_char,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    unsafe {
        jsonp_error_init(error, SOURCE_STRING.as_ptr().cast::<c_char>());
    }

    if input.is_null() {
        unsafe {
            Lexer::plain_error_set(
                error,
                json_error_code::json_error_invalid_argument,
                "wrong arguments".into(),
            );
        }
        return null_mut();
    }

    let mut state = StringData { data: input, pos: 0 };
    unsafe {
        parse_with_source(
            string_get,
            ptr::addr_of_mut!(state).cast::<c_void>(),
            flags,
            error,
            SOURCE_STRING.as_ptr().cast::<c_char>(),
        )
    }
}

#[no_mangle]
pub unsafe extern "C" fn json_loadb(
    buffer: *const c_char,
    buflen: usize,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    unsafe {
        jsonp_error_init(error, SOURCE_BUFFER.as_ptr().cast::<c_char>());
    }

    if buffer.is_null() {
        unsafe {
            Lexer::plain_error_set(
                error,
                json_error_code::json_error_invalid_argument,
                "wrong arguments".into(),
            );
        }
        return null_mut();
    }

    let mut state = BufferData {
        data: buffer,
        len: buflen,
        pos: 0,
    };
    unsafe {
        parse_with_source(
            buffer_get,
            ptr::addr_of_mut!(state).cast::<c_void>(),
            flags,
            error,
            SOURCE_BUFFER.as_ptr().cast::<c_char>(),
        )
    }
}

#[no_mangle]
pub unsafe extern "C" fn json_loadf(
    input: *mut FILE,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    if input.is_null() {
        unsafe {
            jsonp_error_init(error, SOURCE_STREAM.as_ptr().cast::<c_char>());
            Lexer::plain_error_set(
                error,
                json_error_code::json_error_invalid_argument,
                "wrong arguments".into(),
            );
        }
        return null_mut();
    }

    let source = if unsafe { libc::fileno(input) } == libc::STDIN_FILENO {
        SOURCE_STDIN.as_ptr().cast::<c_char>()
    } else {
        SOURCE_STREAM.as_ptr().cast::<c_char>()
    };

    unsafe { parse_with_source(file_get, input.cast::<c_void>(), flags, error, source) }
}

#[no_mangle]
pub unsafe extern "C" fn json_loadfd(
    input: c_int,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    let source = if input == libc::STDIN_FILENO {
        SOURCE_STDIN.as_ptr().cast::<c_char>()
    } else {
        SOURCE_STREAM.as_ptr().cast::<c_char>()
    };
    unsafe {
        jsonp_error_init(error, source);
    }

    if input < 0 {
        unsafe {
            Lexer::plain_error_set(
                error,
                json_error_code::json_error_invalid_argument,
                "wrong arguments".into(),
            );
        }
        return null_mut();
    }

    let mut fd = input;
    unsafe {
        parse_with_source(
            fd_get,
            ptr::addr_of_mut!(fd).cast::<c_void>(),
            flags,
            error,
            source,
        )
    }
}

#[no_mangle]
pub unsafe extern "C" fn json_load_file(
    path: *const c_char,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    unsafe {
        jsonp_error_init(error, path);
    }

    if path.is_null() {
        unsafe {
            Lexer::plain_error_set(
                error,
                json_error_code::json_error_invalid_argument,
                "wrong arguments".into(),
            );
        }
        return null_mut();
    }

    let file = unsafe { libc::fopen(path, MODE_READ_BINARY.as_ptr().cast::<c_char>()) };
    if file.is_null() {
        let detail = unsafe { CStr::from_ptr(libc::strerror(get_errno())) }
            .to_string_lossy()
            .into_owned();
        let file_path = unsafe { CStr::from_ptr(path) }.to_string_lossy();
        unsafe {
            Lexer::plain_error_set(
                error,
                json_error_code::json_error_cannot_open_file,
                format!("unable to open {}: {}", file_path, detail),
            );
        }
        return null_mut();
    }

    let result = unsafe { json_loadf(file, flags, error) };
    unsafe {
        libc::fclose(file);
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn json_load_callback(
    callback: json_load_callback_t,
    data: *mut c_void,
    flags: usize,
    error: *mut json_error_t,
) -> *mut json_t {
    unsafe {
        jsonp_error_init(error, SOURCE_CALLBACK.as_ptr().cast::<c_char>());
    }

    if callback.is_none() {
        unsafe {
            Lexer::plain_error_set(
                error,
                json_error_code::json_error_invalid_argument,
                "wrong arguments".into(),
            );
        }
        return null_mut();
    }

    let mut state = CallbackData {
        data: [0; MAX_CALLBACK_BUF_LEN],
        len: 0,
        pos: 0,
        callback,
        arg: data,
    };

    unsafe {
        parse_with_source(
            callback_get,
            ptr::addr_of_mut!(state).cast::<c_void>(),
            flags,
            error,
            SOURCE_CALLBACK.as_ptr().cast::<c_char>(),
        )
    }
}
