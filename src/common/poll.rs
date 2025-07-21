use core::future::Future;
use core::mem::{self, MaybeUninit};
use core::pin::Pin;
use core::task::{Context, Poll};

use alloc::vec::Vec;

#[cfg(all(feature = "tokio", feature = "std"))]
use tokio::io::AsyncReadExt;

use crate::{AsyncRead, Error, IoErrorKind, ToError};

#[derive(Debug, Clone)]
pub enum GenericPollPacketState<H> {
    Header(PollHeaderState),
    Body(GenericPollBodyState<H>),
}

#[derive(Debug, Clone, Default)]
pub struct PollHeaderState {
    pub control_byte: Option<u8>,
    pub var_idx: u8,
    pub var_int: u32,
}

#[derive(Debug, Clone)]
pub struct GenericPollBodyState<H> {
    pub header: H,
    /// Packet total size (include header)
    pub total: usize,
    pub idx: usize,
    pub buf: Vec<MaybeUninit<u8>>,
}

pub trait PollHeader {
    type Error;
    type Packet;

    fn new_with(hd: u8, remaining_len: u32) -> Result<Self, Self::Error>
    where
        Self: Sized;
    /// Packet without body is empty packet
    fn build_empty_packet(&self) -> Option<Self::Packet>;
    fn block_decode(self, reader: &mut &[u8]) -> Result<Self::Packet, Self::Error>;
    fn remaining_len(&self) -> usize;
    fn is_eof_error(err: &Self::Error) -> bool;
}

impl<H> Default for GenericPollPacketState<H> {
    fn default() -> Self {
        GenericPollPacketState::Header(PollHeaderState::default())
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

macro_rules! impl_generic_poll_packet_future {
    () => {
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let GenericPollPacket {
                ref mut state,
                ref mut reader,
            } = self.get_mut();

            let future = async move {
                loop {
                    match state {
                        GenericPollPacketState::Header(PollHeaderState {
                            control_byte,
                            var_idx,
                            var_int,
                        }) => {
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

                            let mut buf: Vec<MaybeUninit<u8>> =
                                Vec::with_capacity(header.remaining_len());
                            unsafe {
                                buf.set_len(header.remaining_len());
                            }

                            **state = GenericPollPacketState::Body(GenericPollBodyState {
                                header,
                                total: 1 + 1 + *var_idx as usize + header.remaining_len(),
                                idx: 0,
                                buf,
                            });
                        }
                        GenericPollPacketState::Body(GenericPollBodyState {
                            header,
                            idx,
                            buf,
                            total,
                        }) => {
                            while *idx < buf.len() {
                                let remaining = buf.len() - *idx;
                                let buf_slice: &mut [u8] = unsafe {
                                    core::slice::from_raw_parts_mut(
                                        buf[*idx..].as_mut_ptr() as *mut u8,
                                        remaining,
                                    )
                                };

                                match reader.read(buf_slice).await {
                                    Ok(0) => {
                                        return Err(
                                            Error::IoError(IoErrorKind::UnexpectedEof).into()
                                        )
                                    }
                                    Ok(n) => *idx += n,
                                    Err(e) => return Err(e.into()),
                                }
                            }

                            let mut buf_ref: &[u8] = unsafe { mem::transmute(&buf[..]) };
                            let result = header.block_decode(&mut buf_ref);
                            if result.is_ok() && !buf_ref.is_empty() {
                                return Err(Error::InvalidRemainingLength.into());
                            }
                            if let Err(err) = &result {
                                if H::is_eof_error(err) {
                                    return Err(Error::InvalidRemainingLength.into());
                                }
                            }
                            return result.map(|packet| (*total, mem::take(buf), packet));
                        }
                    }
                }
            };

            futures_lite::pin!(future);
            future.as_mut().poll(cx)
        }
    };
}

#[cfg(all(feature = "embedded-io", not(all(feature = "tokio", feature = "std"))))]
impl<'a, T, H> Future for GenericPollPacket<'a, T, H>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error> + From<T::Error>,
{
    type Output = Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>;

    impl_generic_poll_packet_future!();
}

#[cfg(all(feature = "tokio", feature = "std"))]
impl<'a, T, H> Future for GenericPollPacket<'a, T, H>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::Error: From<Error> + From<std::io::Error>,
{
    type Output = Result<(usize, Vec<MaybeUninit<u8>>, H::Packet), H::Error>;

    impl_generic_poll_packet_future!();
}
