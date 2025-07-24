use embedded_io_async::Read;

use crate::{read_u16_async, Error, GenericPollPacket, GenericPollPacketState, Pid, PollHeader};

use super::{
    Connack, Connect, Header, Packet, PacketType, Publish, Suback, Subscribe, Unsubscribe,
};

impl PollHeader for Header {
    type Error = Error;
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
            PacketType::Disconnect => Packet::Disconnect,
            _ => return None,
        };
        Some(packet)
    }

    fn decode_buffer(self, buf: &mut crate::PacketBuf) -> Result<Self::Packet, Self::Error> {
        match self.typ {
            PacketType::Connect => Connect::decode(buf).map(Into::into),
            PacketType::Connack => Connack::decode(buf).map(Into::into),
            PacketType::Publish => Publish::decode(buf, self).map(Into::into),
            PacketType::Puback => Ok(Packet::Puback(Pid::try_from(buf.read_u16()?)?)),
            PacketType::Pubrec => Ok(Packet::Pubrec(Pid::try_from(buf.read_u16()?)?)),
            PacketType::Pubrel => Ok(Packet::Pubrel(Pid::try_from(buf.read_u16()?)?)),
            PacketType::Pubcomp => Ok(Packet::Pubcomp(Pid::try_from(buf.read_u16()?)?)),
            PacketType::Subscribe => Subscribe::decode(buf, self).map(Into::into),
            PacketType::Suback => Suback::decode(buf, self).map(Into::into),
            PacketType::Unsubscribe => Unsubscribe::decode(buf, self).map(Into::into),
            PacketType::Unsuback => Ok(Packet::Unsuback(Pid::try_from(buf.read_u16()?)?)),
            PacketType::Pingreq | PacketType::Pingresp | PacketType::Disconnect => unreachable!(),
        }
    }

    #[rustfmt::skip]
    async fn decode_stream<T: Read + Unpin>(
        self,
        reader: &mut T,
    ) -> Result<Self::Packet, Self::Error> {
        match self.typ {
            PacketType::Connect => Connect::decode_async(reader).await.map(Into::into),
            PacketType::Connack => Connack::decode_async(reader).await.map(Into::into),
            PacketType::Publish => Publish::decode_async(reader, self).await.map(Into::into),
            PacketType::Puback => Ok(Packet::Puback(Pid::try_from(read_u16_async(reader).await?)?)),
            PacketType::Pubrec => Ok(Packet::Pubrec(Pid::try_from(read_u16_async(reader).await?)?)),
            PacketType::Pubrel => Ok(Packet::Pubrel(Pid::try_from(read_u16_async(reader).await?)?)),
            PacketType::Pubcomp => Ok(Packet::Pubcomp(Pid::try_from(read_u16_async(reader).await?)?)),
            PacketType::Subscribe => Subscribe::decode_async(reader, self).await.map(Into::into),
            PacketType::Suback => Suback::decode_async(reader, self).await.map(Into::into),
            PacketType::Unsubscribe => Unsubscribe::decode_async(reader, self).await.map(Into::into),
            PacketType::Unsuback => Ok(Packet::Unsuback(Pid::try_from(read_u16_async(reader).await?)?)),
            PacketType::Pingreq | PacketType::Pingresp | PacketType::Disconnect => unreachable!(),
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
