use core::convert::AsRef;

#[cfg(feature = "tokio")]
use tokio::io::AsyncWriteExt;

use crate::{
    block_on, decode_raw_header_async, encode_packet, packet_from, total_len, AsyncRead,
    AsyncWrite, Encodable, Error, QoS, QosPid, VarBytes,
};

use super::{
    Auth, Connack, Connect, Disconnect, ErrorV5, Puback, Pubcomp, Publish, Pubrec, Pubrel, Suback,
    Subscribe, Unsuback, Unsubscribe,
};

/// MQTT v5.0 packet types.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
        Ok(match header.typ {
            PacketType::Pingreq => Packet::Pingreq,
            PacketType::Pingresp => Packet::Pingresp,
            PacketType::Connect => Connect::decode_async(reader, header).await?.into(),
            PacketType::Connack => Connack::decode_async(reader, header).await?.into(),
            PacketType::Publish => Publish::decode_async(reader, header).await?.into(),
            PacketType::Puback => Puback::decode_async(reader, header).await?.into(),
            PacketType::Pubrec => Pubrec::decode_async(reader, header).await?.into(),
            PacketType::Pubrel => Pubrel::decode_async(reader, header).await?.into(),
            PacketType::Pubcomp => Pubcomp::decode_async(reader, header).await?.into(),
            PacketType::Subscribe => Subscribe::decode_async(reader, header).await?.into(),
            PacketType::Suback => Suback::decode_async(reader, header).await?.into(),
            PacketType::Unsubscribe => Unsubscribe::decode_async(reader, header).await?.into(),
            PacketType::Unsuback => Unsuback::decode_async(reader, header).await?.into(),
            PacketType::Disconnect => Disconnect::decode_async(reader, header).await?.into(),
            PacketType::Auth => Auth::decode_async(reader, header).await?.into(),
        })
    }

    /// Asynchronously encode the packet to an async writer.
    pub async fn encode_async<T: AsyncWrite + Unpin>(&self, writer: &mut T) -> Result<(), ErrorV5> {
        let data = self.encode().map_err(ErrorV5::Common)?;
        writer.write_all(data.as_ref()).await?;
        Ok(())
    }

    /// Decode a packet from some bytes. If not enough bytes to decode a packet,
    /// it will return `Ok(None)`.
    pub fn decode(mut bytes: &[u8]) -> Result<Option<Self>, ErrorV5> {
        match block_on(Self::decode_async(&mut bytes)) {
            Ok(pkt) => Ok(Some(pkt)),
            Err(err) => {
                if let ErrorV5::Common(e) = &err {
                    if e.is_eof() {
                        return Ok(None);
                    }
                }
                Err(err)
            }
        }
    }

    /// Encode the packet to a dynamic vector or fixed array.
    pub fn encode(&self) -> Result<VarBytes, Error> {
        const VOID_PACKET_REMAINING_LEN: u8 = 0;
        let data = match self {
            Packet::Pingreq => {
                const CONTROL_BYTE: u8 = 0b11000000;
                return Ok(VarBytes::Fixed2([CONTROL_BYTE, VOID_PACKET_REMAINING_LEN]));
            }
            Packet::Pingresp => {
                const CONTROL_BYTE: u8 = 0b11010000;
                return Ok(VarBytes::Fixed2([CONTROL_BYTE, VOID_PACKET_REMAINING_LEN]));
            }
            Packet::Publish(publish) => {
                let mut control_byte: u8 = match publish.qos_pid {
                    QosPid::Level0 => 0b00110000,
                    QosPid::Level1(_) => 0b00110010,
                    QosPid::Level2(_) => 0b00110100,
                };
                if publish.dup {
                    control_byte |= 0b00001000;
                }
                if publish.retain {
                    control_byte |= 0b00000001;
                }
                encode_packet(control_byte, publish)?
            }
            Packet::Connect(inner) => {
                const CONTROL_BYTE: u8 = 0b00010000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Connack(inner) => {
                const CONTROL_BYTE: u8 = 0b00100000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Puback(inner) => {
                const CONTROL_BYTE: u8 = 0b01000000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Pubrec(inner) => {
                const CONTROL_BYTE: u8 = 0b01010000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Pubrel(inner) => {
                const CONTROL_BYTE: u8 = 0b01100010;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Pubcomp(inner) => {
                const CONTROL_BYTE: u8 = 0b01110000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Subscribe(inner) => {
                const CONTROL_BYTE: u8 = 0b10000010;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Suback(inner) => {
                const CONTROL_BYTE: u8 = 0b10010000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Unsubscribe(inner) => {
                const CONTROL_BYTE: u8 = 0b10100010;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Unsuback(inner) => {
                const CONTROL_BYTE: u8 = 0b10110000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Disconnect(inner) => {
                const CONTROL_BYTE: u8 = 0b11100000;
                encode_packet(CONTROL_BYTE, inner)?
            }
            Packet::Auth(inner) => {
                const CONTROL_BYTE: u8 = 0b11110000;
                encode_packet(CONTROL_BYTE, inner)?
            }
        };
        Ok(VarBytes::Dynamic(data))
    }

    /// Return the total length of bytes the packet encoded into.
    pub fn encode_len(&self) -> Result<usize, ErrorV5> {
        let remaining_len = match self {
            Packet::Pingreq => return Ok(2),
            Packet::Pingresp => return Ok(2),
            Packet::Connect(inner) => inner.encode_len(),
            Packet::Connack(inner) => inner.encode_len(),
            Packet::Publish(inner) => inner.encode_len(),
            Packet::Puback(inner) => inner.encode_len(),
            Packet::Pubrec(inner) => inner.encode_len(),
            Packet::Pubrel(inner) => inner.encode_len(),
            Packet::Pubcomp(inner) => inner.encode_len(),
            Packet::Subscribe(inner) => inner.encode_len(),
            Packet::Suback(inner) => inner.encode_len(),
            Packet::Unsubscribe(inner) => inner.encode_len(),
            Packet::Unsuback(inner) => inner.encode_len(),
            Packet::Disconnect(inner) => inner.encode_len(),
            Packet::Auth(inner) => inner.encode_len(),
        };
        Ok(total_len(remaining_len)?)
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

impl core::fmt::Display for PacketType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Fixed header type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Header {
    pub typ: PacketType,
    pub dup: bool,
    pub qos: QoS,
    pub retain: bool,
    pub remaining_len: u32,
}

impl Header {
    pub fn new(typ: PacketType, dup: bool, qos: QoS, retain: bool, remaining_len: u32) -> Self {
        Self {
            typ,
            dup,
            qos,
            retain,
            remaining_len,
        }
    }

    pub fn new_with(hd: u8, remaining_len: u32) -> Result<Header, ErrorV5> {
        const FLAGS_MASK: u8 = 0b1111;
        let (typ, flags_ok) = match hd >> 4 {
            1 => (PacketType::Connect, hd & FLAGS_MASK == 0),
            2 => (PacketType::Connack, hd & FLAGS_MASK == 0),
            3 => {
                return Ok(Header {
                    typ: PacketType::Publish,
                    dup: hd & 0b1000 != 0,
                    qos: QoS::from_u8((hd & 0b110) >> 1)?,
                    retain: hd & 1 == 1,
                    remaining_len,
                });
            }
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
            dup: false,
            qos: QoS::Level0,
            retain: false,
            remaining_len,
        })
    }

    pub fn decode(mut reader: &[u8]) -> Result<Self, ErrorV5> {
        block_on(Self::decode_async(&mut reader))
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, ErrorV5> {
        let (typ, remaining_len) = decode_raw_header_async(reader).await?;
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
