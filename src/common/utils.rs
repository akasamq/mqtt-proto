use std::io;
use std::slice;

use futures_lite::io::{AsyncRead, AsyncReadExt};

use crate::Error;

#[inline]
pub(crate) async fn read_string<T: AsyncRead + Unpin>(reader: &mut T) -> Result<String, Error> {
    let data_buf = read_bytes(reader).await?;
    Ok(String::from_utf8(data_buf).map_err(|err| err.utf8_error())?)
}

#[inline]
pub(crate) async fn read_bytes<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Vec<u8>, Error> {
    let data_len = read_u16(reader).await?;
    let mut data_buf = vec![0u8; data_len as usize];
    reader.read_exact(&mut data_buf).await?;
    Ok(data_buf)
}

// Only for v5.0
#[inline]
pub(crate) async fn read_u32<T: AsyncRead + Unpin>(reader: &mut T) -> Result<u32, Error> {
    let mut len4_bytes = [0u8; 4];
    reader.read_exact(&mut len4_bytes).await?;
    Ok(u32::from_be_bytes(len4_bytes))
}

#[inline]
pub(crate) async fn read_u16<T: AsyncRead + Unpin>(reader: &mut T) -> Result<u16, Error> {
    let mut len2_bytes = [0u8; 2];
    reader.read_exact(&mut len2_bytes).await?;
    Ok(u16::from_be_bytes(len2_bytes))
}

#[inline]
pub(crate) async fn read_u8<T: AsyncRead + Unpin>(reader: &mut T) -> Result<u8, Error> {
    let mut byte = 0u8;
    reader.read_exact(slice::from_mut(&mut byte)).await?;
    Ok(byte)
}

#[inline]
pub(crate) fn write_bytes<W: io::Write>(writer: &mut W, data: &[u8]) -> io::Result<()> {
    write_u16(writer, data.len() as u16)?;
    writer.write_all(data)
}

#[inline]
pub(crate) fn write_u32<W: io::Write>(writer: &mut W, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_be_bytes())
}

#[inline]
pub(crate) fn write_u16<W: io::Write>(writer: &mut W, value: u16) -> io::Result<()> {
    writer.write_all(&value.to_be_bytes())
}

#[inline]
pub(crate) fn write_u8<W: io::Write>(writer: &mut W, value: u8) -> io::Result<()> {
    writer.write_all(slice::from_ref(&value))
}

#[inline]
pub(crate) fn write_var_int<W: io::Write>(writer: &mut W, mut len: usize) -> io::Result<()> {
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

/// Read first byte(packet type and flags) and decode remaining length
#[inline]
pub async fn decode_raw_header<T: AsyncRead + Unpin>(reader: &mut T) -> Result<(u8, usize), Error> {
    let typ = read_u8(reader).await?;
    let (remaining_len, _bytes) = decode_var_int(reader).await?;
    Ok((typ, remaining_len))
}

/// Decode a variable byte integer (4 bytes max)
#[inline]
pub async fn decode_var_int<T: AsyncRead + Unpin>(reader: &mut T) -> Result<(usize, usize), Error> {
    let mut byte = 0u8;
    let mut var_int: usize = 0;
    let mut i = 0;
    loop {
        reader.read_exact(slice::from_mut(&mut byte)).await?;
        var_int |= (usize::from(byte) & 0x7F) << (7 * i);
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