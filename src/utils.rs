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
pub(crate) fn write_u8<W: io::Write>(writer: &mut W, value: u8) -> io::Result<()> {
    writer.write_all(slice::from_ref(&value))
}

#[inline]
pub(crate) fn write_u16<W: io::Write>(writer: &mut W, value: u16) -> io::Result<()> {
    writer.write_all(&value.to_be_bytes())
}

#[inline]
pub(crate) fn write_bytes<W: io::Write>(writer: &mut W, data: &[u8]) -> io::Result<()> {
    write_u16(writer, data.len() as u16)?;
    writer.write_all(data)
}
