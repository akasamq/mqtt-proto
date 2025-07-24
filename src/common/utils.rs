use core::slice;

use alloc::sync::Arc;
use alloc::vec::Vec;

use simdutf8::basic::from_utf8;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;

use crate::{AsyncRead, Encodable, Error, SyncWrite, ToError};

/// Read first byte(packet type and flags) and decode remaining length
#[inline]
pub async fn decode_raw_header_async<T: AsyncRead + Unpin>(
    reader: &mut T,
) -> Result<(u8, u32), Error> {
    let typ = read_u8_async(reader).await?;
    let (remaining_len, _bytes) = decode_var_int_async(reader).await?;
    Ok((typ, remaining_len))
}

#[inline]
pub(crate) async fn read_string_async<T: AsyncRead + Unpin>(
    reader: &mut T,
) -> Result<Arc<str>, Error> {
    let data_buf = read_bytes_async(reader).await?;
    let s = from_utf8(&data_buf).map_err(|_| Error::InvalidString)?;
    Ok(s.into())
}

#[inline]
pub(crate) async fn read_bytes_async<T: AsyncRead + Unpin>(
    reader: &mut T,
) -> Result<Vec<u8>, Error> {
    let data_len = read_u16_async(reader).await?;
    let mut data_buf = alloc::vec![0u8; data_len as usize];
    reader
        .read_exact(&mut data_buf)
        .await
        .map_err(ToError::to_error)?;
    Ok(data_buf)
}

// Only for v5.0
#[inline]
pub(crate) async fn read_u32_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<u32, Error> {
    let mut len4_bytes = [0u8; 4];
    reader
        .read_exact(&mut len4_bytes)
        .await
        .map_err(ToError::to_error)?;
    Ok(u32::from_be_bytes(len4_bytes))
}

#[inline]
pub(crate) async fn read_u16_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<u16, Error> {
    let mut len2_bytes = [0u8; 2];
    reader
        .read_exact(&mut len2_bytes)
        .await
        .map_err(ToError::to_error)?;
    Ok(u16::from_be_bytes(len2_bytes))
}

#[inline]
pub(crate) async fn read_u8_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<u8, Error> {
    let mut byte = [0u8; 1];
    reader
        .read_exact(&mut byte)
        .await
        .map_err(ToError::to_error)?;
    Ok(byte[0])
}

#[derive(Debug)]
pub struct PacketBuf<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> PacketBuf<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    #[inline]
    pub fn position(&self) -> usize {
        self.offset
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }

    #[inline]
    pub fn is_consumed(&self) -> bool {
        self.offset >= self.data.len()
    }

    #[inline]
    pub fn is_fully_consumed(&self) -> bool {
        self.offset == self.data.len()
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        self.data
    }

    #[inline]
    pub fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    #[inline]
    pub fn read_u8(&mut self) -> Result<u8, Error> {
        if self.offset >= self.data.len() {
            return Err(Error::IoError(crate::IoErrorKind::UnexpectedEof));
        }
        let value = self.data[self.offset];
        self.offset += 1;
        Ok(value)
    }

    #[inline]
    pub fn read_u16(&mut self) -> Result<u16, Error> {
        if self.offset + 2 > self.data.len() {
            return Err(Error::IoError(crate::IoErrorKind::UnexpectedEof));
        }
        let value = u16::from_be_bytes([self.data[self.offset], self.data[self.offset + 1]]);
        self.offset += 2;
        Ok(value)
    }

    #[inline]
    pub fn read_u32(&mut self) -> Result<u32, Error> {
        if self.offset + 4 > self.data.len() {
            return Err(Error::IoError(crate::IoErrorKind::UnexpectedEof));
        }
        let value = u32::from_be_bytes([
            self.data[self.offset],
            self.data[self.offset + 1],
            self.data[self.offset + 2],
            self.data[self.offset + 3],
        ]);
        self.offset += 4;
        Ok(value)
    }

    #[inline]
    pub fn read_string(&mut self) -> Result<&'a str, Error> {
        let data_slice = self.read_bytes()?;
        from_utf8(data_slice).map_err(|_| Error::InvalidString)
    }

    #[inline]
    pub fn read_bytes(&mut self) -> Result<&'a [u8], Error> {
        let data_len = self.read_u16()? as usize;
        if self.offset + data_len > self.data.len() {
            return Err(Error::IoError(crate::IoErrorKind::UnexpectedEof));
        }
        let result = &self.data[self.offset..self.offset + data_len];
        self.offset += data_len;
        Ok(result)
    }

    #[inline]
    pub fn read_raw_bytes(&mut self, len: usize) -> Result<&'a [u8], Error> {
        if self.offset + len > self.data.len() {
            return Err(Error::IoError(crate::IoErrorKind::UnexpectedEof));
        }
        let result = &self.data[self.offset..self.offset + len];
        self.offset += len;
        Ok(result)
    }

    #[inline]
    pub fn decode_var_int(&mut self) -> Result<(u32, usize), Error> {
        let start_offset = self.offset;
        let mut var_int: u32 = 0;
        let mut i = 0;
        loop {
            if self.offset >= self.data.len() {
                return Err(Error::IoError(crate::IoErrorKind::UnexpectedEof));
            }
            let byte = self.data[self.offset];
            self.offset += 1;
            var_int |= (u32::from(byte) & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                break;
            } else if i < 3 {
                i += 1;
            } else {
                return Err(Error::InvalidVarByteInt);
            }
        }
        Ok((var_int, self.offset - start_offset))
    }
}

#[inline]
pub(crate) fn write_string<W: SyncWrite>(writer: &mut W, value: &str) -> Result<(), Error> {
    write_bytes(writer, value.as_bytes())?;
    Ok(())
}

#[inline]
pub(crate) fn write_bytes<W: SyncWrite>(writer: &mut W, data: &[u8]) -> Result<(), Error> {
    write_u16(writer, data.len() as u16)?;
    writer.write_all(data)?;
    Ok(())
}

#[inline]
pub(crate) fn write_u32<W: SyncWrite>(writer: &mut W, value: u32) -> Result<(), Error> {
    writer.write_all(&value.to_be_bytes())?;
    Ok(())
}

#[inline]
pub(crate) fn write_u16<W: SyncWrite>(writer: &mut W, value: u16) -> Result<(), Error> {
    writer.write_all(&value.to_be_bytes())?;
    Ok(())
}

#[inline]
pub(crate) fn write_u8<W: SyncWrite>(writer: &mut W, value: u8) -> Result<(), Error> {
    writer.write_all(slice::from_ref(&value))?;
    Ok(())
}

#[inline]
pub(crate) fn write_var_int<W: SyncWrite>(writer: &mut W, mut len: usize) -> Result<(), Error> {
    loop {
        let mut byte = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            byte |= 128;
        }
        write_u8(writer, byte)?;
        if len == 0 {
            break;
        }
    }
    Ok(())
}

/// Decode a variable byte integer (4 bytes max)
#[inline]
pub(crate) async fn decode_var_int_async<T: AsyncRead + Unpin>(
    reader: &mut T,
) -> Result<(u32, usize), Error> {
    let mut var_int: u32 = 0;
    let mut i = 0;
    loop {
        let mut buf = [0u8; 1];
        reader
            .read_exact(&mut buf)
            .await
            .map_err(ToError::to_error)?;
        let byte = buf[0];
        var_int |= (u32::from(byte) & 0x7F) << (7 * i);
        if byte & 0x80 == 0 {
            break;
        } else if i < 3 {
            i += 1;
        } else {
            return Err(Error::InvalidVarByteInt);
        }
    }
    Ok((var_int, i + 1))
}

/// Return the encoded size of the variable byte integer.
#[inline]
pub fn var_int_len(value: usize) -> Result<usize, Error> {
    let len = if value < 128 {
        1
    } else if value < 16384 {
        2
    } else if value < 2097152 {
        3
    } else if value < 268435456 {
        4
    } else {
        return Err(Error::InvalidVarByteInt);
    };
    Ok(len)
}

/// Return the packet total encoded length by a given remaining length.
#[inline]
pub fn total_len(remaining_len: usize) -> Result<usize, Error> {
    let header_len = if remaining_len < 128 {
        2
    } else if remaining_len < 16384 {
        3
    } else if remaining_len < 2097152 {
        4
    } else if remaining_len < 268435456 {
        5
    } else {
        return Err(Error::InvalidVarByteInt);
    };
    Ok(header_len + remaining_len)
}

/// Calculate remaining length by given total length (the total length MUST be
/// valid value).
#[inline]
pub fn remaining_len(total_len: usize) -> usize {
    total_len - header_len(total_len)
}

/// Calculate header length by given total length (the total length MUST be
/// valid value).
#[inline]
pub fn header_len(total_len: usize) -> usize {
    if total_len < 128 + 2 {
        2
    } else if total_len < 16384 + 3 {
        3
    } else if total_len < 2097152 + 4 {
        4
    } else {
        5
    }
}

/// Encode packet use control byte and body type
#[inline]
pub(crate) fn encode_packet<E: Encodable>(control_byte: u8, body: &E) -> Result<Vec<u8>, Error> {
    let remaining_len = body.encode_len();
    let total = total_len(remaining_len)?;
    let mut buf = Vec::with_capacity(total);

    // encode header
    buf.push(control_byte);
    write_var_int(&mut buf, remaining_len)?;

    body.encode(&mut buf)?;
    debug_assert_eq!(buf.len(), total);
    Ok(buf)
}

macro_rules! packet_from {
    ($($t:ident),+) => {
        $(
            impl From<$t> for Packet {
                fn from(p: $t) -> Self {
                    Packet::$t(p)
                }
            }
        )+
    }
}

pub(crate) use packet_from;

#[cfg(test)]
mod tests {
    use crate::block_on;

    use super::*;

    #[test]
    fn test_decode_var_int() {
        for (mut data, value, size) in [
            (&[0xff, 0xff, 0xff, 0x7f][..], 268435455, 4),
            (&[0x80, 0x80, 0x80, 0x01][..], 2097152, 4),
            (&[0xff, 0xff, 0x7f][..], 2097151, 3),
            (&[0x80, 0x80, 0x01][..], 16384, 3),
            (&[0xff, 0x7f][..], 16383, 2),
            (&[0x80, 0x01][..], 128, 2),
            (&[0x7f][..], 127, 1),
            (&[0x00][..], 0, 1),
        ] {
            assert_eq!(
                block_on(decode_var_int_async(&mut data)).unwrap(),
                (value, size)
            );
        }

        let mut err_data = &[0xff, 0xff, 0xff][..];
        assert!(block_on(decode_var_int_async(&mut err_data))
            .unwrap_err()
            .is_eof());
    }
}
