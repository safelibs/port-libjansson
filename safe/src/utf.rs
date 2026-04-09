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
fn is_continuation(byte: u8) -> bool {
    matches!(byte, 0x80..=0xBF)
}

fn decode_one(bytes: &[u8], offset: usize) -> Result<(u32, usize), Utf8Error> {
    let first = match bytes.get(offset).copied() {
        Some(first) => first,
        None => return Err(Utf8Error { offset }),
    };

    if first < 0x80 {
        return Ok((u32::from(first), 1));
    }

    let invalid = || Err(Utf8Error { offset });
    let b1 = |index| bytes.get(index).copied().ok_or(Utf8Error { offset });

    match first {
        0xC2..=0xDF => {
            let second = b1(offset + 1)?;
            if !is_continuation(second) {
                return invalid();
            }

            let code_point = (u32::from(first & 0x1F) << 6) | u32::from(second & 0x3F);
            Ok((code_point, 2))
        }
        0xE0 => {
            let second = b1(offset + 1)?;
            let third = b1(offset + 2)?;
            if !matches!(second, 0xA0..=0xBF) || !is_continuation(third) {
                return invalid();
            }

            let code_point = (u32::from(first & 0x0F) << 12)
                | (u32::from(second & 0x3F) << 6)
                | u32::from(third & 0x3F);
            Ok((code_point, 3))
        }
        0xE1..=0xEC | 0xEE..=0xEF => {
            let second = b1(offset + 1)?;
            let third = b1(offset + 2)?;
            if !is_continuation(second) || !is_continuation(third) {
                return invalid();
            }

            let code_point = (u32::from(first & 0x0F) << 12)
                | (u32::from(second & 0x3F) << 6)
                | u32::from(third & 0x3F);
            Ok((code_point, 3))
        }
        0xED => {
            let second = b1(offset + 1)?;
            let third = b1(offset + 2)?;
            if !matches!(second, 0x80..=0x9F) || !is_continuation(third) {
                return invalid();
            }

            let code_point = (u32::from(first & 0x0F) << 12)
                | (u32::from(second & 0x3F) << 6)
                | u32::from(third & 0x3F);
            Ok((code_point, 3))
        }
        0xF0 => {
            let second = b1(offset + 1)?;
            let third = b1(offset + 2)?;
            let fourth = b1(offset + 3)?;
            if !matches!(second, 0x90..=0xBF) || !is_continuation(third) || !is_continuation(fourth)
            {
                return invalid();
            }

            let code_point = (u32::from(first & 0x07) << 18)
                | (u32::from(second & 0x3F) << 12)
                | (u32::from(third & 0x3F) << 6)
                | u32::from(fourth & 0x3F);
            Ok((code_point, 4))
        }
        0xF1..=0xF3 => {
            let second = b1(offset + 1)?;
            let third = b1(offset + 2)?;
            let fourth = b1(offset + 3)?;
            if !is_continuation(second) || !is_continuation(third) || !is_continuation(fourth) {
                return invalid();
            }

            let code_point = (u32::from(first & 0x07) << 18)
                | (u32::from(second & 0x3F) << 12)
                | (u32::from(third & 0x3F) << 6)
                | u32::from(fourth & 0x3F);
            Ok((code_point, 4))
        }
        0xF4 => {
            let second = b1(offset + 1)?;
            let third = b1(offset + 2)?;
            let fourth = b1(offset + 3)?;
            if !matches!(second, 0x80..=0x8F) || !is_continuation(third) || !is_continuation(fourth)
            {
                return invalid();
            }

            let code_point = (u32::from(first & 0x07) << 18)
                | (u32::from(second & 0x3F) << 12)
                | (u32::from(third & 0x3F) << 6)
                | u32::from(fourth & 0x3F);
            Ok((code_point, 4))
        }
        _ => invalid(),
    }
}

impl<'a> Iterator for Utf8Iter<'a> {
    type Item = Result<u32, Utf8Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.bytes.len() {
            return None;
        }

        match decode_one(self.bytes, self.offset) {
            Ok((code_point, width)) => {
                self.offset += width;
                Some(Ok(code_point))
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
            buffer[0] = 0xC0 | ((code_point >> 6) as u8);
            buffer[1] = 0x80 | ((code_point & 0x3F) as u8);
            Some(2)
        }
        0x0800..=0xD7FF | 0xE000..=0xFFFF => {
            buffer[0] = 0xE0 | ((code_point >> 12) as u8);
            buffer[1] = 0x80 | (((code_point >> 6) & 0x3F) as u8);
            buffer[2] = 0x80 | ((code_point & 0x3F) as u8);
            Some(3)
        }
        0x10000..=0x10FFFF => {
            buffer[0] = 0xF0 | ((code_point >> 18) as u8);
            buffer[1] = 0x80 | (((code_point >> 12) & 0x3F) as u8);
            buffer[2] = 0x80 | (((code_point >> 6) & 0x3F) as u8);
            buffer[3] = 0x80 | ((code_point & 0x3F) as u8);
            Some(4)
        }
        _ => None,
    }
}
