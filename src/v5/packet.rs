use std::fmt;
use std::io;

use futures_lite::{
    future::block_on,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
};

use super::{
    Auth, Connack, Connect, Disconnect, ErrorV5, Puback, Pubcomp, Publish, Pubrec, Pubrel, Suback,
    Subscribe, Unsuback, Unsubscribe,
};
use crate::{decode_raw_header, packet_from, read_u16, Encodable, Error, Pid, QoS, QosPid};

/// MQTT v5.0 packet types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    /// [MQTT 3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901033)
    Connect(Connect),
    /// [MQTT 3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901074)
    Connack(Connack),
    /// [MQTT 3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901100)
    Publish(Publish),
    /// [MQTT 3.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901121)
    Puback(Puback),
    /// [MQTT 3.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901131)
    Pubrec(Pubrec),
    /// [MQTT 3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901141)
    Pubrel(Pubrel),
    /// [MQTT 3.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901151)
    Pubcomp(Pubcomp),
    /// [MQTT 3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901161)
    Subscribe(Subscribe),
    /// [MQTT 3.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901171)
    Suback(Suback),
    /// [MQTT 3.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901179)
    Unsubscribe(Unsubscribe),
    /// [MQTT 3.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901187)
    Unsuback(Unsuback),
    /// [MQTT 3.12](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901195)
    Pingreq,
    /// [MQTT 3.13](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901200)
    Pingresp,
    /// [MQTT 3.14](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901205)
    Disconnect(Disconnect),
    /// [MQTT 3.15](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901217)
    Auth(Auth),
}

impl Packet {
    /// Return the packet type variant.
    ///
    /// This can be used for matching, categorising, debuging, etc. Most users
    /// will match directly on `Packet` instead.
    pub fn get_type(&self) -> PacketType {
        match self {
            Packet::Pingreq => PacketType::Pingreq,
            Packet::Pingresp => PacketType::Pingresp,
            Packet::Connect(_) => PacketType::Connect,
            Packet::Connack(_) => PacketType::Connack,
            Packet::Publish(_) => PacketType::Publish,
            Packet::Puback(_) => PacketType::Puback,
            Packet::Pubrec(_) => PacketType::Pubrec,
            Packet::Pubrel(_) => PacketType::Pubrel,
            Packet::Pubcomp(_) => PacketType::Pubcomp,
            Packet::Subscribe(_) => PacketType::Subscribe,
            Packet::Suback(_) => PacketType::Suback,
            Packet::Unsubscribe(_) => PacketType::Unsubscribe,
            Packet::Unsuback(_) => PacketType::Unsuback,
            Packet::Disconnect(_) => PacketType::Disconnect,
            Packet::Auth(_) => PacketType::Auth,
        }
    }

    /// Asynchronously decode a packet from an async reader.
    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, ErrorV5> {
        let header = Header::decode_async(reader).await?;
        let remaining_len = header.remaining_len;
        Ok(match header.typ {
            PacketType::Pingreq => Packet::Pingreq,
            PacketType::Pingresp => Packet::Pingresp,
            PacketType::Connect => Connect::decode_async(reader, header).await?.into(),
            PacketType::Connack => Connack::decode_async(reader, remaining_len).await?.into(),
            PacketType::Publish => Publish::decode_async(reader, header).await?.into(),
            PacketType::Puback => Puback::decode_async(reader, remaining_len).await?.into(),
            PacketType::Pubrec => Pubrec::decode_async(reader, remaining_len).await?.into(),
            PacketType::Pubrel => Pubrel::decode_async(reader, remaining_len).await?.into(),
            PacketType::Pubcomp => Pubcomp::decode_async(reader, remaining_len).await?.into(),
            PacketType::Subscribe => Subscribe::decode_async(reader, remaining_len).await?.into(),
            PacketType::Suback => Suback::decode_async(reader, remaining_len).await?.into(),
            PacketType::Unsubscribe => Unsubscribe::decode_async(reader, remaining_len)
                .await?
                .into(),
            PacketType::Unsuback => Unsuback::decode_async(reader, remaining_len).await?.into(),
            PacketType::Disconnect => Disconnect::decode_async(reader, remaining_len)
                .await?
                .into(),
            PacketType::Auth => Auth::decode_async(reader, remaining_len).await?.into(),
        })
    }
}

/// MQTT v5.0 packet type variant, without the associated data.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PacketType {
    Connect,
    Connack,
    Publish,
    Puback,
    Pubrec,
    Pubrel,
    Pubcomp,
    Subscribe,
    Suback,
    Unsubscribe,
    Unsuback,
    Pingreq,
    Pingresp,
    Disconnect,
    Auth,
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Fixed header type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Header {
    pub typ: PacketType,
    pub dup: bool,
    pub qos: QoS,
    pub retain: bool,
    pub remaining_len: usize,
}

impl Header {
    pub fn new(typ: PacketType, dup: bool, qos: QoS, retain: bool, remaining_len: usize) -> Self {
        Self {
            typ,
            dup,
            qos,
            retain,
            remaining_len,
        }
    }

    pub fn new_with(hd: u8, remaining_len: usize) -> Result<Header, ErrorV5> {
        const FLAGS_MASK: u8 = 0b1111;
        let (typ, flags_ok) = match hd >> 4 {
            1 => (PacketType::Connect, hd & FLAGS_MASK == 0),
            2 => (PacketType::Connack, hd & FLAGS_MASK == 0),
            3 => (PacketType::Publish, true),
            4 => (PacketType::Puback, hd & FLAGS_MASK == 0),
            5 => (PacketType::Pubrec, hd & FLAGS_MASK == 0),
            6 => (PacketType::Pubrel, hd & FLAGS_MASK == 0b0010),
            7 => (PacketType::Pubcomp, hd & FLAGS_MASK == 0),
            8 => (PacketType::Subscribe, hd & FLAGS_MASK == 0b0010),
            9 => (PacketType::Suback, hd & FLAGS_MASK == 0),
            10 => (PacketType::Unsubscribe, hd & FLAGS_MASK == 0b0010),
            11 => (PacketType::Unsuback, hd & FLAGS_MASK == 0),
            12 => (PacketType::Pingreq, hd & FLAGS_MASK == 0),
            13 => (PacketType::Pingresp, hd & FLAGS_MASK == 0),
            14 => (PacketType::Disconnect, hd & FLAGS_MASK == 0),
            15 => (PacketType::Auth, hd & FLAGS_MASK == 0),
            _ => return Err(Error::InvalidHeader.into()),
        };
        if !flags_ok {
            return Err(Error::InvalidHeader.into());
        }
        Ok(Header {
            typ,
            dup: hd & 0b1000 != 0,
            qos: QoS::from_u8((hd & 0b110) >> 1)?,
            retain: hd & 1 == 1,
            remaining_len,
        })
    }

    pub fn decode(mut reader: &[u8]) -> Result<Self, ErrorV5> {
        block_on(Self::decode_async(&mut reader))
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, ErrorV5> {
        let (typ, remaining_len) = decode_raw_header(reader).await?;
        Header::new_with(typ, remaining_len)
    }
}

packet_from!(
    Connect,
    Connack,
    Publish,
    Puback,
    Pubrec,
    Pubrel,
    Pubcomp,
    Subscribe,
    Suback,
    Unsubscribe,
    Unsuback,
    Disconnect,
    Auth
);
