use futures_lite::future::block_on;

use super::{
    Auth, Connack, Connect, Disconnect, ErrorV5, Header, Packet, PacketType, Puback, Pubcomp,
    Publish, Pubrec, Pubrel, Suback, Subscribe, Unsuback, Unsubscribe,
};
use crate::{GenericPollPacket, GenericPollPacketState, GenericPollPayloadState, PollHeader};

impl PollHeader for Header {
    type TheError = ErrorV5;
    type ThePacket = Packet;

    fn new_with(hd: u8, remaining_len: u32) -> Result<Self, Self::TheError>
    where
        Self: Sized,
    {
        Header::new_with(hd, remaining_len)
    }

    fn build_empty_packet(&self) -> Option<Self::ThePacket> {
        let packet = match self.typ {
            PacketType::Pingreq => Packet::Pingreq,
            PacketType::Pingresp => Packet::Pingresp,
            PacketType::Auth if self.remaining_len == 0 => Auth::new_success().into(),
            PacketType::Disconnect if self.remaining_len == 0 => Disconnect::new_normal().into(),
            _ => return None,
        };
        Some(packet)
    }

    fn block_decode(self, reader: &mut &[u8]) -> Result<Self::ThePacket, Self::TheError> {
        match self.typ {
            PacketType::Connect => block_on(Connect::decode_async(reader, self)).map(Into::into),
            PacketType::Connack => block_on(Connack::decode_async(reader, self)).map(Into::into),
            PacketType::Publish => block_on(Publish::decode_async(reader, self)).map(Into::into),
            PacketType::Puback => block_on(Puback::decode_async(reader, self)).map(Into::into),
            PacketType::Pubrec => block_on(Pubrec::decode_async(reader, self)).map(Into::into),
            PacketType::Pubrel => block_on(Pubrel::decode_async(reader, self)).map(Into::into),
            PacketType::Pubcomp => block_on(Pubcomp::decode_async(reader, self)).map(Into::into),
            PacketType::Subscribe => {
                block_on(Subscribe::decode_async(reader, self)).map(Into::into)
            }
            PacketType::Suback => block_on(Suback::decode_async(reader, self)).map(Into::into),
            PacketType::Unsubscribe => {
                block_on(Unsubscribe::decode_async(reader, self)).map(Into::into)
            }
            PacketType::Unsuback => block_on(Unsuback::decode_async(reader, self)).map(Into::into),
            PacketType::Disconnect => {
                block_on(Disconnect::decode_async(reader, self)).map(Into::into)
            }
            PacketType::Auth => block_on(Auth::decode_async(reader, self)).map(Into::into),
            PacketType::Pingreq | PacketType::Pingresp => unreachable!(),
        }
    }

    fn remaining_len(&self) -> usize {
        self.remaining_len as usize
    }

    fn is_eof_error(err: &Self::TheError) -> bool {
        err.is_eof()
    }
}

pub type PollPacket<'a, T> = GenericPollPacket<'a, T, Header>;
pub type PollPacketState = GenericPollPacketState<Header>;
pub type PollPayloadState = GenericPollPayloadState<Header>;
