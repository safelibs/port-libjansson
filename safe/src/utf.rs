use std::ffi::c_char;
use std::slice;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Utf8Error {
    pub offset: usize,
}

pub struct Utf8Iter<'a> {
    bytes: &'a [u8],
    offset: usize,
}

#[inline]
pub fn check_first(byte: u8) -> usize {
    if byte < 0x80 {
        1
    } else if matches!(byte, 0x80..=0xBF | 0xC0 | 0xC1) {
        0
    } else if matches!(byte, 0xC2..=0xDF) {
        2
    } else if matches!(byte, 0xE0..=0xEF) {
        3
    } else if matches!(byte, 0xF0..=0xF4) {
        4
    } else {
        0
    }
}

pub fn check_full(buffer: &[u8], size: usize) -> Option<u32> {
    if buffer.len() < size {
        return None;
    }

    let first = buffer[0];
    let mut value = match size {
        2 => u32::from(first & 0x1F),
        3 => u32::from(first & 0x0F),
        4 => u32::from(first & 0x07),
        _ => return None,
    };

    for &byte in &buffer[1..size] {
        if !(0x80..=0xBF).contains(&byte) {
            return None;
        }
        value = (value << 6) | u32::from(byte & 0x3F);
    }

    if value > 0x10FFFF
        || matches!(value, 0xD800..=0xDFFF)
        || (size == 2 && value < 0x80)
        || (size == 3 && value < 0x800)
        || (size == 4 && value < 0x10000)
    {
        return None;
    }

    Some(value)
}

pub fn iterate(bytes: &[u8], offset: usize) -> Result<(u32, usize), Utf8Error> {
    let Some(&first) = bytes.get(offset) else {
        return Err(Utf8Error { offset });
    };

    let count = check_first(first);
    if count == 0 {
        return Err(Utf8Error { offset });
    }
    if count == 1 {
        return Ok((u32::from(first), 1));
    }

    let end = match offset.checked_add(count) {
        Some(end) => end,
        None => return Err(Utf8Error { offset }),
    };
    let Some(slice) = bytes.get(offset..end) else {
        return Err(Utf8Error { offset });
    };

    match check_full(slice, count) {
        Some(codepoint) => Ok((codepoint, count)),
        None => Err(Utf8Error { offset }),
    }
}

impl<'a> Iterator for Utf8Iter<'a> {
    type Item = Result<u32, Utf8Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.bytes.len() {
            return None;
        }

        match iterate(self.bytes, self.offset) {
            Ok((codepoint, width)) => {
                self.offset += width;
                Some(Ok(codepoint))
            }
            Err(err) => {
                self.offset = self.bytes.len();
                Some(Err(err))
            }
        }
    }
}

pub fn iter(bytes: &[u8]) -> Utf8Iter<'_> {
    Utf8Iter { bytes, offset: 0 }
}

pub fn validate(bytes: &[u8]) -> bool {
    iter(bytes).all(|item| item.is_ok())
}

pub unsafe fn validate_ptr(value: *const c_char, len: usize) -> bool {
    if value.is_null() {
        return false;
    }

    let bytes = unsafe { slice::from_raw_parts(value.cast::<u8>(), len) };
    validate(bytes)
}

pub fn encode(code_point: u32, buffer: &mut [u8; 4]) -> Option<usize> {
    match code_point {
        0x0000..=0x007F => {
            buffer[0] = code_point as u8;
            Some(1)
        }
        0x0080..=0x07FF => {
            buffer[0] = 0xC0 | ((code_point & 0x07C0) >> 6) as u8;
            buffer[1] = 0x80 | (code_point & 0x003F) as u8;
            Some(2)
        }
        0x0800..=0xFFFF => {
            buffer[0] = 0xE0 | ((code_point & 0xF000) >> 12) as u8;
            buffer[1] = 0x80 | ((code_point & 0x0FC0) >> 6) as u8;
            buffer[2] = 0x80 | (code_point & 0x003F) as u8;
            Some(3)
        }
        0x10000..=0x10FFFF => {
            buffer[0] = 0xF0 | ((code_point & 0x1C0000) >> 18) as u8;
            buffer[1] = 0x80 | ((code_point & 0x03F000) >> 12) as u8;
            buffer[2] = 0x80 | ((code_point & 0x000FC0) >> 6) as u8;
            buffer[3] = 0x80 | (code_point & 0x00003F) as u8;
            Some(4)
        }
        _ => None,
    }
}
