use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use alloc::vec::Vec;

#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;

use super::{
    AsyncRead, Buffer, BufferHandle, BufferResult, Error, IoErrorKind, ReadStrategy, ToError,
};

impl<H: BufferHandle> BufferResult<H> {
    pub fn as_slice(&self) -> &[u8] {
        match self {
            BufferResult::Pooled(handle) => handle.as_slice(handle.len()),
            BufferResult::Owned(vec) => vec.as_slice(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            BufferResult::Pooled(handle) => handle.len(),
            BufferResult::Owned(vec) => vec.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone)]
pub enum GenericPollPacketState<H> {
    Header {
        control_byte: Option<u8>,
        var_idx: u8,
        var_int: u32,
    },
    Body {
        header: H,
        idx: usize,
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

pub struct GenericPollPacket<'a, T, H, B>
where
    B: Buffer,
{
    state: &'a mut GenericPollPacketState<H>,
    reader: &'a mut T,
    buffer: &'a mut B,
}

impl<'a, T, H, B> GenericPollPacket<'a, T, H, B>
where
    B: Buffer,
{
    pub fn new(
        state: &'a mut GenericPollPacketState<H>,
        reader: &'a mut T,
        buffer: &'a mut B,
    ) -> Self {
        GenericPollPacket {
            state,
            reader,
            buffer,
        }
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

async fn poll_packet_buffer_body<T, H, B>(
    reader: &mut T,
    header: H,
    idx: &mut usize,
    mut buf: B,
) -> Result<BufferResult<B>, H::Error>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error> + From<B::Error>,
    B: BufferHandle,
{
    let remaining_len = header.remaining_len();

    // Set the buffer length to the remaining length
    buf.set_len(remaining_len);

    let (buf_slice, _capacity) = buf.as_mut_slice();

    while *idx < remaining_len {
        let available_len = (buf_slice.len() - *idx).min(remaining_len - *idx);
        if available_len == 0 {
            return Err(Error::IoError(IoErrorKind::UnexpectedEof).into());
        }
        let slice = &mut buf_slice[*idx..*idx + available_len];
        let slice: &mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(slice.as_mut_ptr() as *mut u8, slice.len()) };
        match reader.read(slice).await {
            Ok(0) => return Err(Error::IoError(IoErrorKind::UnexpectedEof).into()),
            Ok(n) => *idx += n,
            Err(e) => return Err(Error::from(e).into()),
        }
    }

    // Return BufferHandle directly - zero copy!
    Ok(BufferResult::Pooled(buf))
}

async fn poll_packet_chunk_body<T, H, B>(
    reader: &mut T,
    header: H,
    chunk_size: usize,
    idx: &mut usize,
    acc: &mut Vec<u8>,
) -> Result<BufferResult<B::Handle>, H::Error>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error>,
    B: Buffer,
{
    let remaining_len = header.remaining_len();

    // Ensure accumulated_data has enough capacity
    if acc.capacity() < remaining_len {
        acc.reserve(remaining_len - acc.capacity());
    }

    // Read in chunks until we have all data
    while *idx < remaining_len {
        let bytes_to_read = chunk_size.min(remaining_len - *idx);

        // Resize accumulated_data to accommodate new chunk
        let old_len = acc.len();
        acc.resize(old_len + bytes_to_read, 0);

        // Get slice for this chunk
        let chunk_slice = &mut acc[old_len..old_len + bytes_to_read];

        // Read the chunk
        let mut chunk_bytes_read = 0;
        while chunk_bytes_read < bytes_to_read {
            match reader.read(&mut chunk_slice[chunk_bytes_read..]).await {
                Ok(0) => return Err(Error::IoError(IoErrorKind::UnexpectedEof).into()),
                Ok(n) => {
                    chunk_bytes_read += n;
                    *idx += n;
                }
                Err(e) => return Err(Error::from(e).into()),
            }
        }
    }

    // Return owned Vec directly - avoid copy by taking ownership
    let result = core::mem::take(acc); // Move out of acc, leaving empty vec
    Ok(BufferResult::Owned(result))
}

async fn poll_packet<T, H, B>(
    state: &mut GenericPollPacketState<H>,
    reader: &mut T,
    buffer: &mut B,
) -> Result<(usize, BufferResult<B::Handle>, H::Packet), H::Error>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error> + From<B::Error> + From<<B::Handle as BufferHandle>::Error>,
    B: Buffer,
{
    loop {
        match state {
            GenericPollPacketState::Header {
                control_byte,
                var_idx,
                var_int,
            } => {
                #[allow(clippy::useless_conversion)]
                let header: H = poll_packet_header(reader, control_byte, var_idx, var_int)
                    .await
                    .map_err(Into::<H::Error>::into)?;
                if let Some(empty_packet) = header.build_empty_packet() {
                    return Ok((2, BufferResult::Owned(Vec::new()), empty_packet));
                }
                if header.remaining_len() == 0 {
                    return Err(Error::InvalidRemainingLength.into());
                }
                *state = GenericPollPacketState::Body { header, idx: 0 };
            }
            GenericPollPacketState::Body { header, idx } => {
                let header_copy = *header;
                let total_len = header_copy.total_len();
                let strategy = buffer.read_strategy(total_len);

                let buffer_result = match strategy {
                    ReadStrategy::Buffer => {
                        // Acquire buffer and read with zero copy
                        let remaining_len = header_copy.remaining_len();
                        let buf = buffer.acquire(remaining_len).await?;
                        let result = poll_packet_buffer_body(reader, header_copy, idx, buf).await?;
                        result
                    }
                    ReadStrategy::Chunk(chunk_size) => {
                        // Use chunk reading with owned Vec
                        let mut acc = Vec::new();
                        poll_packet_chunk_body::<T, H, B>(
                            reader,
                            header_copy,
                            chunk_size,
                            idx,
                            &mut acc,
                        )
                        .await?
                    }
                };

                // Decode packet from buffer data
                let mut offset = 0;
                let packet = header_copy
                    .decode_buffer(buffer_result.as_slice(), &mut offset)
                    .map_err(|e| {
                        if H::is_eof_error(&e) {
                            Error::InvalidRemainingLength.into()
                        } else {
                            e
                        }
                    })?;

                *state = GenericPollPacketState::default(); // Reset
                return Ok((total_len, buffer_result, packet));
            }
        }
    }
}

#[cfg(feature = "tokio")]
impl<'a, T, H, B> Future for GenericPollPacket<'a, T, H, B>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error>
        + From<std::io::Error>
        + From<B::Error>
        + From<<B::Handle as BufferHandle>::Error>,
    B: Buffer,
{
    type Output = Result<(usize, BufferResult<B::Handle>, H::Packet), H::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let GenericPollPacket {
            ref mut state,
            ref mut reader,
            ref mut buffer,
        } = self.get_mut();

        let future = poll_packet(state, reader, buffer);
        futures_lite::pin!(future);
        future.as_mut().poll(cx)
    }
}

#[cfg(not(feature = "tokio"))]
impl<'a, T, H, B> Future for GenericPollPacket<'a, T, H, B>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error:
        From<Error> + From<T::Error> + From<B::Error> + From<<B::Handle as BufferHandle>::Error>,
    B: Buffer,
{
    type Output = Result<(usize, BufferResult<B::Handle>, H::Packet), H::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let GenericPollPacket {
            ref mut state,
            ref mut reader,
            ref mut buffer,
        } = self.get_mut();

        let future = poll_packet(state, reader, buffer);
        futures_lite::pin!(future);
        future.as_mut().poll(cx)
    }
}
