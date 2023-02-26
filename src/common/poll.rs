use std::future::Future;
use std::io;
use std::mem::{self, MaybeUninit};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_lite::io::AsyncRead;

use crate::Error;

#[derive(Debug, Clone)]
pub enum GenericPollPacketState<H> {
    Header(PollHeaderState),
    Payload(GenericPollPayloadState<H>),
}

#[derive(Debug, Clone, Default)]
pub struct PollHeaderState {
    pub packet_type: Option<u8>,
    pub var_idx: u8,
    pub var_int: u32,
}

#[derive(Debug, Clone)]
pub struct GenericPollPayloadState<H> {
    pub header: H,
    /// Packet total size (include header)
    pub total: usize,
    pub idx: usize,
    pub buf: Vec<MaybeUninit<u8>>,
}

pub trait PollHeader {
    type TheError;
    type ThePacket;

    fn new_with(hd: u8, remaining_len: u32) -> Result<Self, Self::TheError>
    where
        Self: Sized;
    /// Packet without payload is empty packet
    fn build_empty_packet(&self) -> Option<Self::ThePacket>;
    fn block_decode(self, reader: &mut &[u8]) -> Result<Self::ThePacket, Self::TheError>;
    fn remaining_len(&self) -> usize;
    fn is_eof_error(err: &Self::TheError) -> bool;
}

impl<H> Default for GenericPollPacketState<H> {
    fn default() -> Self {
        GenericPollPacketState::Header(PollHeaderState::default())
    }
}

pub struct GenericPollPacket<'a, T, H> {
    state: GenericPollPacketState<H>,
    reader: &'a mut T,
}

impl<'a, T, H> GenericPollPacket<'a, T, H> {
    pub fn new(reader: &'a mut T) -> Self {
        GenericPollPacket::from_state(GenericPollPacketState::default(), reader)
    }

    pub fn from_state(state: GenericPollPacketState<H>, reader: &'a mut T) -> Self {
        GenericPollPacket { state, reader }
    }

    pub fn into_state(self) -> GenericPollPacketState<H> {
        self.state
    }
}

impl<'a, T, H> Future for GenericPollPacket<'a, T, H>
where
    T: AsyncRead + Unpin,
    H: PollHeader + Copy + Unpin,
    H::TheError: From<io::Error> + From<Error>,
{
    type Output = Result<(usize, H::ThePacket), H::TheError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let GenericPollPacket {
            ref mut state,
            ref mut reader,
        } = self.get_mut();
        loop {
            match state {
                GenericPollPacketState::Header(PollHeaderState {
                    packet_type,
                    var_idx,
                    var_int,
                }) => {
                    let mut buf = [0u8; 1];
                    loop {
                        let _size = match Pin::new(&mut *reader).poll_read(cx, &mut buf) {
                            Poll::Ready(Ok(0)) => {
                                return Poll::Ready(Err(Error::IoError(
                                    io::ErrorKind::UnexpectedEof,
                                    "eof".to_owned(),
                                )
                                .into()));
                            }
                            Poll::Ready(Ok(size)) => size,
                            Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                            Poll::Pending => return Poll::Pending,
                        };

                        let byte = buf[0];
                        if packet_type.is_none() {
                            *packet_type = Some(byte);
                        } else {
                            *var_int |= (u32::from(byte) & 0x7F) << (7 * u32::from(*var_idx));
                            if byte & 0x80 == 0 {
                                break;
                            } else if *var_idx < 3 {
                                *var_idx += 1;
                            } else {
                                return Poll::Ready(Err(Error::InvalidVarByteInt.into()));
                            }
                        }
                    }

                    let header = match H::new_with(packet_type.unwrap(), *var_int) {
                        Ok(header) => header,
                        Err(err) => return Poll::Ready(Err(err)),
                    };
                    if let Some(empty_packet) = header.build_empty_packet() {
                        return Poll::Ready(Ok((2, empty_packet)));
                    }
                    if header.remaining_len() == 0 {
                        return Poll::Ready(Err(Error::InvalidRemainingLength.into()));
                    }
                    let mut buf: Vec<MaybeUninit<u8>> = Vec::with_capacity(header.remaining_len());
                    unsafe {
                        buf.set_len(header.remaining_len());
                    }
                    *state = GenericPollPacketState::Payload(GenericPollPayloadState {
                        header,
                        total: 1 + 1 + *var_idx as usize + header.remaining_len(),
                        idx: 0,
                        buf,
                    });
                }
                GenericPollPacketState::Payload(GenericPollPayloadState {
                    header,
                    idx,
                    buf,
                    total,
                }) => loop {
                    let buf_refmut: &mut [u8] = unsafe { mem::transmute(&mut buf[*idx..]) };
                    let size = match Pin::new(&mut *reader).poll_read(cx, buf_refmut) {
                        Poll::Ready(Ok(0)) => {
                            return Poll::Ready(Err(Error::IoError(
                                io::ErrorKind::UnexpectedEof,
                                "eof".to_owned(),
                            )
                            .into()));
                        }
                        Poll::Ready(Ok(size)) => size,
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                        Poll::Pending => return Poll::Pending,
                    };

                    *idx += size;
                    if *idx == buf.len() {
                        let mut buf_ref: &[u8] = unsafe { mem::transmute(&buf[..]) };
                        let result = header.block_decode(&mut buf_ref);
                        if result.is_ok() && !buf_ref.is_empty() {
                            return Poll::Ready(Err(Error::InvalidRemainingLength.into()));
                        }
                        if let Err(err) = &result {
                            if H::is_eof_error(err) {
                                return Poll::Ready(Err(Error::InvalidRemainingLength.into()));
                            }
                        }
                        return Poll::Ready(result.map(|packet| (*total, packet)));
                    }
                },
            }
        }
    }
}
