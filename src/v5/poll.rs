use embedded_io_async::Read;

use crate::{GenericPollPacket, GenericPollPacketState, PollHeader};

use super::{
    Auth, Connack, Connect, Disconnect, ErrorV5, Header, Packet, PacketType, Puback, Pubcomp,
    Publish, Pubrec, Pubrel, Suback, Subscribe, Unsuback, Unsubscribe,
};

impl PollHeader for Header {
    type Error = ErrorV5;
    type Packet = Packet;

    fn new_with(hd: u8, remaining_len: u32) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Header::new_with(hd, remaining_len)
    }

    fn build_empty_packet(&self) -> Option<Self::Packet> {
        let packet = match self.typ {
            PacketType::Pingreq => Packet::Pingreq,
            PacketType::Pingresp => Packet::Pingresp,
            PacketType::Auth if self.remaining_len == 0 => Auth::new_success().into(),
            PacketType::Disconnect if self.remaining_len == 0 => Disconnect::new_normal().into(),
            _ => return None,
        };
        Some(packet)
    }

    #[rustfmt::skip]
    async fn stream_decode<T: Read + Unpin>(
        self,
        reader: &mut T,
    ) -> Result<Self::Packet, Self::Error> {
        match self.typ {
            PacketType::Connect => Connect::decode_async(reader, self).await.map(Into::into),
            PacketType::Connack => Connack::decode_async(reader, self).await.map(Into::into),
            PacketType::Publish => Publish::decode_async(reader, self).await.map(Into::into),
            PacketType::Puback => Puback::decode_async(reader, self).await.map(Into::into),
            PacketType::Pubrec => Pubrec::decode_async(reader, self).await.map(Into::into),
            PacketType::Pubrel => Pubrel::decode_async(reader, self).await.map(Into::into),
            PacketType::Pubcomp => Pubcomp::decode_async(reader, self).await.map(Into::into),
            PacketType::Subscribe => Subscribe::decode_async(reader, self).await.map(Into::into),
            PacketType::Suback => Suback::decode_async(reader, self).await.map(Into::into),
            PacketType::Unsubscribe => Unsubscribe::decode_async(reader, self).await.map(Into::into),
            PacketType::Unsuback => Unsuback::decode_async(reader, self).await.map(Into::into),
            PacketType::Disconnect => Disconnect::decode_async(reader, self).await.map(Into::into),
            PacketType::Auth => Auth::decode_async(reader, self).await.map(Into::into),
            PacketType::Pingreq | PacketType::Pingresp => unreachable!(),
        }
    }

    fn remaining_len(&self) -> usize {
        self.remaining_len as usize
    }

    fn is_eof_error(err: &Self::Error) -> bool {
        err.is_eof()
    }
}

pub type PollPacket<'a, T> = GenericPollPacket<'a, T, Header>;
pub type PollPacketState = GenericPollPacketState<Header>;
