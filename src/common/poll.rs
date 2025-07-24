use core::future::Future;
use core::mem::{self, MaybeUninit};
use core::pin::Pin;
use core::task::{Context, Poll};

use alloc::vec::Vec;

use embedded_io_async::Read;

use crate::{AsyncRead, Error, IoErrorKind, PacketBuf, ToError};

#[derive(Debug, Clone)]
pub enum GenericPollPacketState<H> {
    Header {
        control_byte: Option<u8>,
        var_idx: u8,
        var_int: u32,
    },
    Body {
        header: H,
        /// Packet total size (include header)
        total: usize,
        idx: usize,
        buf: Vec<MaybeUninit<u8>>,
    },
}

#[allow(async_fn_in_trait)]
pub trait PollHeader {
    type Error;
    type Packet;

    fn new_with(hd: u8, remaining_len: u32) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Packet without body is empty packet
    fn build_empty_packet(&self) -> Option<Self::Packet>;

    /// Synchronous decode method for direct buffer access
    fn decode_buffer(self, buf: &mut PacketBuf) -> Result<Self::Packet, Self::Error>;

    /// Async decode method for stream-based processing
    async fn decode_stream<T: Read + Unpin>(
        self,
        reader: &mut T,
    ) -> Result<Self::Packet, Self::Error>;

    fn remaining_len(&self) -> usize;

    fn is_eof_error(err: &Self::Error) -> bool;

    fn prefer_cache_decode(&self) -> bool {
        self.remaining_len() < 4096
    }
}

impl<H> Default for GenericPollPacketState<H> {
    fn default() -> Self {
        GenericPollPacketState::Header {
            control_byte: None,
            var_idx: 0,
            var_int: 0,
        }
    }
}

pub struct GenericPollPacket<'a, T, H> {
    state: &'a mut GenericPollPacketState<H>,
    reader: &'a mut T,
}

impl<'a, T, H> GenericPollPacket<'a, T, H> {
    pub fn new(state: &'a mut GenericPollPacketState<H>, reader: &'a mut T) -> Self {
        GenericPollPacket { state, reader }
    }
}

async fn poll_packet<T, H>(
    state: &mut GenericPollPacketState<H>,
    reader: &mut T,
) -> Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>
where
    T: Read + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error> + From<T::Error>,
{
    loop {
        match state {
            GenericPollPacketState::Header {
                control_byte,
                var_idx,
                var_int,
            } => {
                if control_byte.is_none() {
                    let mut buf = [0u8; 1];
                    reader
                        .read_exact(&mut buf)
                        .await
                        .map_err(ToError::to_error)?;
                    *control_byte = Some(buf[0]);
                }

                loop {
                    let mut buf = [0u8; 1];
                    reader
                        .read_exact(&mut buf)
                        .await
                        .map_err(ToError::to_error)?;

                    let byte = buf[0];
                    *var_int |= (u32::from(byte) & 0x7F) << (7 * u32::from(*var_idx));
                    if byte & 0x80 == 0 {
                        break;
                    } else if *var_idx < 3 {
                        *var_idx += 1;
                    } else {
                        return Err(Error::InvalidVarByteInt.into());
                    }
                }
                let header = match H::new_with(control_byte.unwrap(), *var_int) {
                    Ok(header) => header,
                    Err(err) => return Err(err),
                };
                if let Some(empty_packet) = header.build_empty_packet() {
                    return Ok((2, Vec::new(), empty_packet));
                }
                if header.remaining_len() == 0 {
                    return Err(Error::InvalidRemainingLength.into());
                }
                let mut buf: Vec<MaybeUninit<u8>> = Vec::with_capacity(header.remaining_len());
                unsafe {
                    buf.set_len(header.remaining_len());
                }
                *state = GenericPollPacketState::Body {
                    header,
                    total: 1 + 1 + *var_idx as usize + header.remaining_len(),
                    idx: 0,
                    buf,
                };
            }
            GenericPollPacketState::Body {
                header,
                idx,
                buf,
                total,
            } => {
                while *idx < buf.len() {
                    let remaining = buf.len() - *idx;
                    let buf_slice: &mut [u8] = unsafe {
                        core::slice::from_raw_parts_mut(
                            buf[*idx..].as_mut_ptr() as *mut u8,
                            remaining,
                        )
                    };

                    match reader.read(buf_slice).await {
                        Ok(0) => return Err(Error::IoError(IoErrorKind::UnexpectedEof).into()),
                        Ok(n) => *idx += n,
                        Err(e) => return Err(e.into()),
                    }
                }

                let buf_slice: &[u8] = unsafe { mem::transmute(&buf[..]) };

                let result = if header.prefer_cache_decode() {
                    let mut packet_buf = PacketBuf::new(buf_slice);
                    let packet = header.decode_buffer(&mut packet_buf);
                    if packet.is_ok() && !packet_buf.is_fully_consumed() {
                        return Err(Error::InvalidRemainingLength.into());
                    }
                    packet
                } else {
                    let mut buf_ref: &[u8] = buf_slice;
                    let stream_result = header.decode_stream(&mut buf_ref).await;
                    if stream_result.is_ok() && !buf_ref.is_empty() {
                        return Err(Error::InvalidRemainingLength.into());
                    }
                    stream_result
                };

                if let Err(e) = &result {
                    if H::is_eof_error(&e) {
                        return Err(Error::InvalidRemainingLength.into());
                    }
                }
                return result.map(|packet| (*total, mem::take(buf), packet));
            }
        }
    }
}

#[cfg(feature = "tokio")]
impl<'a, T, H> Future for GenericPollPacket<'a, T, H>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error> + From<std::io::Error>,
{
    type Output = Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        use embedded_io_adapters::tokio_1::FromTokio;

        let GenericPollPacket {
            ref mut state,
            ref mut reader,
        } = self.get_mut();

        let mut adapter = FromTokio::new(reader);
        let future = poll_packet(state, &mut adapter);
        futures_lite::pin!(future);
        future.as_mut().poll(cx)
    }
}

#[cfg(not(feature = "tokio"))]
impl<'a, T, H> Future for GenericPollPacket<'a, T, H>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error> + From<T::Error>,
{
    type Output = Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let GenericPollPacket {
            ref mut state,
            ref mut reader,
        } = self.get_mut();

        let future = poll_packet(state, reader);
        futures_lite::pin!(future);
        future.as_mut().poll(cx)
    }
}
