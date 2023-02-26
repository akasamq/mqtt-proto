use futures_lite::future::block_on;

use super::{
    Connack, Connect, Header, Packet, PacketType, Publish, Suback, Subscribe, Unsubscribe,
};
use crate::{
    read_u16, Error, GenericPollPacket, GenericPollPacketState, GenericPollPayloadState, Pid,
    PollHeader,
};

impl PollHeader for Header {
    type TheError = Error;
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
            PacketType::Disconnect => Packet::Disconnect,
            _ => return None,
        };
        Some(packet)
    }

    fn block_decode(self, reader: &mut &[u8]) -> Result<Self::ThePacket, Self::TheError> {
        match self.typ {
            PacketType::Connect => block_on(Connect::decode_async(reader)).map(Into::into),
            PacketType::Connack => block_on(Connack::decode_async(reader)).map(Into::into),
            PacketType::Publish => block_on(Publish::decode_async(reader, self)).map(Into::into),
            PacketType::Puback => Ok(Packet::Puback(Pid::try_from(block_on(read_u16(reader))?)?)),
            PacketType::Pubrec => Ok(Packet::Pubrec(Pid::try_from(block_on(read_u16(reader))?)?)),
            PacketType::Pubrel => Ok(Packet::Pubrel(Pid::try_from(block_on(read_u16(reader))?)?)),
            PacketType::Pubcomp => Ok(Packet::Pubcomp(Pid::try_from(block_on(read_u16(reader))?)?)),
            PacketType::Subscribe => {
                block_on(Subscribe::decode_async(reader, self.remaining_len())).map(Into::into)
            }
            PacketType::Suback => {
                block_on(Suback::decode_async(reader, self.remaining_len())).map(Into::into)
            }
            PacketType::Unsubscribe => {
                block_on(Unsubscribe::decode_async(reader, self.remaining_len())).map(Into::into)
            }
            PacketType::Unsuback => Ok(Packet::Unsuback(Pid::try_from(block_on(read_u16(
                reader,
            ))?)?)),
            PacketType::Pingreq | PacketType::Pingresp | PacketType::Disconnect => unreachable!(),
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
