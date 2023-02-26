use std::future::Future;
use std::io;
use std::mem::{self, MaybeUninit};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_lite::{future::block_on, io::AsyncRead};

use super::{
    Auth, Connack, Connect, Disconnect, ErrorV5, Header, Packet, PacketType, Puback, Pubcomp,
    Publish, Pubrec, Pubrel, Suback, Subscribe, Unsuback, Unsubscribe,
};
use crate::Error;

#[derive(Debug)]
pub enum PollPacketState {
    Header {
        packet_type: Option<u8>,
        var_idx: u8,
        var_int: u32,
    },
    Payload {
        header: Header,
        /// Packet total size (include header)
        total: usize,
        idx: usize,
        buf: Vec<MaybeUninit<u8>>,
    },
}

pub struct PollPacket<'a, T> {
    state: PollPacketState,
    reader: &'a mut T,
}

impl<'a, T> PollPacket<'a, T> {
    pub fn new(reader: &'a mut T) -> Self {
        let state = PollPacketState::Header {
            packet_type: None,
            var_idx: 0,
            var_int: 0,
        };
        PollPacket::from_state(state, reader)
    }

    pub fn from_state(state: PollPacketState, reader: &'a mut T) -> Self {
        PollPacket { state, reader }
    }

    pub fn into_state(self) -> PollPacketState {
        self.state
    }
}

impl<'a, T> Future for PollPacket<'a, T>
where
    T: AsyncRead + Unpin,
{
    type Output = Result<(usize, Packet), ErrorV5>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let PollPacket {
            ref mut state,
            ref mut reader,
        } = self.get_mut();
        loop {
            match state {
                PollPacketState::Header {
                    packet_type,
                    var_idx,
                    var_int,
                } => {
                    let mut buf = [0u8; 1];
                    loop {
                        match Pin::new(&mut *reader).poll_read(cx, &mut buf) {
                            Poll::Ready(Ok(0)) => {
                                return Poll::Ready(Err(Error::IoError(
                                    io::ErrorKind::UnexpectedEof,
                                    "eof".to_owned(),
                                )
                                .into()));
                            }
                            Poll::Ready(Ok(_)) => {
                                let byte = buf[0];
                                if packet_type.is_none() {
                                    *packet_type = Some(byte);
                                } else {
                                    *var_int |=
                                        (u32::from(byte) & 0x7F) << (7 * u32::from(*var_idx));
                                    if byte & 0x80 == 0 {
                                        break;
                                    } else if *var_idx < 3 {
                                        *var_idx += 1;
                                    } else {
                                        return Poll::Ready(Err(Error::InvalidVarByteInt.into()));
                                    }
                                }
                            }
                            Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                            Poll::Pending => return Poll::Pending,
                        }
                    }
                    let header = match Header::new_with(packet_type.unwrap(), *var_int) {
                        Ok(header) => header,
                        Err(err) => return Poll::Ready(Err(err)),
                    };
                    match header.typ {
                        PacketType::Pingreq => return Poll::Ready(Ok((2, Packet::Pingreq))),
                        PacketType::Pingresp => return Poll::Ready(Ok((2, Packet::Pingresp))),
                        PacketType::Auth if header.remaining_len == 0 => {
                            return Poll::Ready(Ok((2, Packet::Auth(Auth::new_success()))));
                        }
                        PacketType::Disconnect if header.remaining_len == 0 => {
                            return Poll::Ready(Ok((
                                2,
                                Packet::Disconnect(Disconnect::new_normal()),
                            )));
                        }
                        _ => {
                            let mut buf: Vec<MaybeUninit<u8>> =
                                Vec::with_capacity(header.remaining_len as usize);
                            unsafe {
                                buf.set_len(header.remaining_len as usize);
                            }
                            *state = PollPacketState::Payload {
                                header,
                                total: 1 + 1 + *var_idx as usize + header.remaining_len as usize,
                                idx: 0,
                                buf,
                            };
                        }
                    }
                }
                PollPacketState::Payload {
                    header,
                    idx,
                    buf,
                    total,
                } => loop {
                    let buf_refmut: &mut [u8] = unsafe { mem::transmute(&mut buf[*idx..]) };
                    match Pin::new(&mut *reader).poll_read(cx, buf_refmut) {
                        Poll::Ready(Ok(0)) => {
                            return Poll::Ready(Err(Error::IoError(
                                io::ErrorKind::UnexpectedEof,
                                "eof".to_owned(),
                            )
                            .into()));
                        }
                        Poll::Ready(Ok(size)) => {
                            *idx += size;
                            if *idx == buf.len() {
                                let mut buf_ref: &[u8] = unsafe { mem::transmute(&buf[..]) };
                                let result: Result<Packet, ErrorV5> = match header.typ {
                                    PacketType::Connect => {
                                        block_on(Connect::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Connack => {
                                        block_on(Connack::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Publish => {
                                        block_on(Publish::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Puback => {
                                        block_on(Puback::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Pubrec => {
                                        block_on(Pubrec::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Pubrel => {
                                        block_on(Pubrel::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Pubcomp => {
                                        block_on(Pubcomp::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Subscribe => {
                                        block_on(Subscribe::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Suback => {
                                        block_on(Suback::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Unsubscribe => {
                                        block_on(Unsubscribe::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Unsuback => {
                                        block_on(Unsuback::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Disconnect => {
                                        block_on(Disconnect::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    PacketType::Auth => {
                                        block_on(Auth::decode_async(&mut buf_ref, *header))
                                            .map(Into::into)
                                    }
                                    _ => unreachable!(),
                                };
                                if result.is_ok() && !buf_ref.is_empty() {
                                    return Poll::Ready(Err(ErrorV5::Common(
                                        Error::InvalidRemainingLength,
                                    )));
                                }
                                if let Err(err) = &result {
                                    if err.is_eof() {
                                        return Poll::Ready(Err(ErrorV5::Common(
                                            Error::InvalidRemainingLength,
                                        )));
                                    }
                                }
                                return Poll::Ready(result.map(|packet| (*total, packet)));
                            }
                        }
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                        Poll::Pending => return Poll::Pending,
                    }
                },
            }
        }
    }
}
