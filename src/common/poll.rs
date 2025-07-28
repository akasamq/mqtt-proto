use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::{Context, Poll};

use alloc::vec::Vec;

#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;

use crate::{AsyncRead, Error, IoErrorKind, ToError};

const PACKET_BUFFER_THRESHOLD: usize = 8192;

#[derive(Debug, Clone)]
pub enum GenericPollPacketState<H> {
    Header {
        control_byte: Option<u8>,
        var_idx: u8,
        var_int: u32,
    },
    BufferBody {
        header: H,
        /// Packet total size (include header)
        total: usize,
        idx: usize,
        buf: Vec<MaybeUninit<u8>>,
    },
    StreamBody {
        header: H,
    },
}

#[allow(async_fn_in_trait)]
pub trait PollHeader {
    type Error: From<Error>;
    type Packet;

    fn new_with(hd: u8, remaining_len: u32, total_len: u32) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Packet without body is empty packet
    fn build_empty_packet(&self) -> Option<Self::Packet>;

    /// Synchronous decode method for direct buffer access
    fn decode_buffer(self, buf: &[u8], offset: &mut usize) -> Result<Self::Packet, Self::Error>;

    /// Async decode method for stream-based processing
    async fn decode_stream<T: AsyncRead + Unpin>(
        self,
        reader: &mut T,
    ) -> Result<Self::Packet, Self::Error>;

    /// The remaining length of the packet to decode
    fn remaining_len(&self) -> usize;

    /// The total length of the packet, including the header and the body
    fn total_len(&self) -> usize;

    fn is_eof_error(err: &Self::Error) -> bool;

    fn prefer_cache_decode(&self) -> bool {
        self.total_len() < PACKET_BUFFER_THRESHOLD
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

async fn poll_packet_header<T, H>(
    reader: &mut T,
    control_byte: &mut Option<u8>,
    var_idx: &mut u8,
    var_int: &mut u32,
) -> Result<H, H::Error>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Unpin,
    H::Error: From<Error>,
{
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
    let header = H::new_with(
        control_byte.unwrap(),
        *var_int,
        1 + 1 + (*var_idx as u32) + *var_int,
    )?;
    Ok(header)
}

async fn poll_packet_buffer_body<T, H>(
    reader: &mut T,
    header: H,
) -> Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error>,
{
    let remaining_len = header.remaining_len();
    let mut buf: Vec<MaybeUninit<u8>> = Vec::with_capacity(remaining_len);
    unsafe { buf.set_len(remaining_len) };

    let mut idx = 0;
    while idx < remaining_len {
        let slice: &mut [u8] = unsafe {
            core::slice::from_raw_parts_mut(buf[idx..].as_mut_ptr() as *mut u8, remaining_len - idx)
        };
        match reader.read(slice).await {
            Ok(0) => return Err(Error::IoError(IoErrorKind::UnexpectedEof).into()),
            Ok(n) => idx += n,
            Err(e) => return Err(Error::from(e).into()),
        }
    }

    let mut offset = 0;
    let buf_slice: &[u8] = unsafe { core::mem::transmute(&buf[..]) };
    let packet = header.decode_buffer(buf_slice, &mut offset).map_err(|e| {
        if H::is_eof_error(&e) {
            Error::InvalidRemainingLength.into()
        } else {
            e
        }
    })?;
    Ok((header.total_len(), buf, packet))
}

async fn poll_packet_stream_body<T, H>(
    reader: &mut T,
    header: H,
) -> Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error>,
{
    let packet = header.decode_stream(reader).await.map_err(|e| {
        if H::is_eof_error(&e) {
            Error::InvalidRemainingLength.into()
        } else {
            e
        }
    })?;
    Ok((header.total_len(), Vec::new(), packet))
}

async fn poll_packet<T, H>(
    state: &mut GenericPollPacketState<H>,
    reader: &mut T,
) -> Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error>,
{
    loop {
        match state {
            GenericPollPacketState::Header {
                ref mut control_byte,
                ref mut var_idx,
                ref mut var_int,
            } => {
                let header: H = poll_packet_header(reader, control_byte, var_idx, var_int)
                    .await
                    .map_err(Into::<H::Error>::into)?;
                if let Some(empty_packet) = header.build_empty_packet() {
                    return Ok((2, Vec::new(), empty_packet));
                }
                if header.remaining_len() == 0 {
                    return Err(Error::InvalidRemainingLength.into());
                }
                if header.prefer_cache_decode() {
                    let remaining_len = header.remaining_len();
                    let mut buf: Vec<MaybeUninit<u8>> = Vec::with_capacity(remaining_len);
                    unsafe { buf.set_len(remaining_len) };
                    *state = GenericPollPacketState::BufferBody {
                        header,
                        total: header.total_len(),
                        idx: 0,
                        buf,
                    };
                } else {
                    *state = GenericPollPacketState::StreamBody { header };
                }
            }
            GenericPollPacketState::BufferBody { header, .. } => {
                let header_copy = *header;
                let (total, buf, packet) = poll_packet_buffer_body(reader, header_copy).await?;
                *state = GenericPollPacketState::default(); // Reset
                return Ok((total, buf, packet));
            }
            GenericPollPacketState::StreamBody { header } => {
                let header_copy = *header;
                let (total, buf, packet) = poll_packet_stream_body(reader, header_copy).await?;
                *state = GenericPollPacketState::default(); // Reset
                return Ok((total, buf, packet));
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
        let GenericPollPacket {
            ref mut state,
            ref mut reader,
        } = self.get_mut();

        let future = poll_packet(state, reader);
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
