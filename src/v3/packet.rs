use std::io;

use futures_lite::{
    future::block_on,
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
};

use super::{Connack, Connect, Publish, Suback, Subscribe, Unsubscribe};
use crate::{
    decode_raw_header, packet_from, read_u16, total_len, write_var_int, Encodable, Error, Pid, QoS,
    QosPid,
};

/// MQTT v3.x packet types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    /// [MQTT 3.1](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718028)
    Connect(Connect),
    /// [MQTT 3.2](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718033)
    Connack(Connack),
    /// [MQTT 3.3](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718037)
    Publish(Publish),
    /// [MQTT 3.4](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718043)
    Puback(Pid),
    /// [MQTT 3.5](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718048)
    Pubrec(Pid),
    /// [MQTT 3.6](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718053)
    Pubrel(Pid),
    /// [MQTT 3.7](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718058)
    Pubcomp(Pid),
    /// [MQTT 3.8](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718063)
    Subscribe(Subscribe),
    /// [MQTT 3.9](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718068)
    Suback(Suback),
    /// [MQTT 3.10](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718072)
    Unsubscribe(Unsubscribe),
    /// [MQTT 3.11](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718077)
    Unsuback(Pid),
    /// [MQTT 3.12](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718081)
    Pingreq,
    /// [MQTT 3.13](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718086)
    Pingresp,
    /// [MQTT 3.14](http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718090)
    Disconnect,
}

/// MQTT v3.x packet type variant, without the associated data.
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
}

impl Packet {
    /// Return the packet type variant.
    ///
    /// This can be used for matching, categorising, debuging, etc. Most users
    /// will match directly on `Packet` instead.
    pub fn get_type(&self) -> PacketType {
        match self {
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
            Packet::Pingreq => PacketType::Pingreq,
            Packet::Pingresp => PacketType::Pingresp,
            Packet::Disconnect => PacketType::Disconnect,
        }
    }

    /// Asynchronously decode a packet from an async reader.
    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let header = Header::decode_async(reader).await?;
        Ok(match header.typ {
            PacketType::Pingreq => Packet::Pingreq,
            PacketType::Pingresp => Packet::Pingresp,
            PacketType::Disconnect => Packet::Disconnect,

            PacketType::Connect => Connect::decode_async(reader).await?.into(),
            PacketType::Connack => Connack::decode_async(reader).await?.into(),
            PacketType::Publish => Publish::decode_async(reader, header).await?.into(),
            PacketType::Puback => Packet::Puback(Pid::new(read_u16(reader).await?)),
            PacketType::Pubrec => Packet::Pubrec(Pid::new(read_u16(reader).await?)),
            PacketType::Pubrel => Packet::Pubrel(Pid::new(read_u16(reader).await?)),
            PacketType::Pubcomp => Packet::Pubcomp(Pid::new(read_u16(reader).await?)),
            PacketType::Subscribe => Subscribe::decode_async(reader, header.remaining_len)
                .await?
                .into(),
            PacketType::Suback => Suback::decode_async(reader, header.remaining_len)
                .await?
                .into(),
            PacketType::Unsubscribe => Unsubscribe::decode_async(reader, header.remaining_len)
                .await?
                .into(),
            PacketType::Unsuback => Packet::Unsuback(Pid::new(read_u16(reader).await?)),
        })
    }

    /// Asynchronously encode the packet to an async writer.
    pub async fn encode_async<T: AsyncWrite + Unpin>(&self, writer: &mut T) -> Result<(), Error> {
        let data = self.encode()?;
        writer.write_all(data.as_slice()).await?;
        Ok(())
    }

    /// Decode a packet from some bytes. If not enough bytes to decode a packet,
    /// it will return `Ok(None)`.
    pub fn decode(mut bytes: &[u8]) -> Result<Option<Self>, Error> {
        match block_on(Self::decode_async(&mut bytes)) {
            Ok(pkt) => Ok(Some(pkt)),
            Err(Error::IoError(kind, info)) => {
                if kind == io::ErrorKind::UnexpectedEof {
                    Ok(None)
                } else {
                    Err(Error::IoError(kind, info))
                }
            }
            Err(err) => Err(err),
        }
    }

    /// Encode the packet to a dynamic vector or fixed array.
    pub fn encode(&self) -> Result<VarBytes, Error> {
        const VOID_PACKET_REMAINING_LEN: u8 = 0;
        let data = match self {
            Packet::Connect(connect) => {
                const CONTROL_BYTE: u8 = 0b00010000;
                VarBytes::Dynamic(encode_inner(connect, CONTROL_BYTE)?)
            }
            Packet::Connack(connack) => {
                const CONTROL_BYTE: u8 = 0b00100000;
                const REMAINING_LEN: u8 = 2;
                let flags: u8 = connack.session_present.into();
                let rc: u8 = connack.code as u8;
                VarBytes::Fixed4([CONTROL_BYTE, REMAINING_LEN, flags, rc])
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
                VarBytes::Dynamic(encode_inner(publish, control_byte)?)
            }
            Packet::Puback(pid) => {
                const CONTROL_BYTE: u8 = 0b01000000;
                VarBytes::Fixed4(encode_with_pid(CONTROL_BYTE, *pid))
            }
            Packet::Pubrec(pid) => {
                const CONTROL_BYTE: u8 = 0b01010000;
                VarBytes::Fixed4(encode_with_pid(CONTROL_BYTE, *pid))
            }
            Packet::Pubrel(pid) => {
                const CONTROL_BYTE: u8 = 0b01100010;
                VarBytes::Fixed4(encode_with_pid(CONTROL_BYTE, *pid))
            }
            Packet::Pubcomp(pid) => {
                const CONTROL_BYTE: u8 = 0b01110000;
                VarBytes::Fixed4(encode_with_pid(CONTROL_BYTE, *pid))
            }
            Packet::Subscribe(subscribe) => {
                const CONTROL_BYTE: u8 = 0b10000010;
                VarBytes::Dynamic(encode_inner(subscribe, CONTROL_BYTE)?)
            }
            Packet::Suback(suback) => {
                const CONTROL_BYTE: u8 = 0b10010000;
                VarBytes::Dynamic(encode_inner(suback, CONTROL_BYTE)?)
            }
            Packet::Unsubscribe(unsubscribe) => {
                const CONTROL_BYTE: u8 = 0b10100010;
                VarBytes::Dynamic(encode_inner(unsubscribe, CONTROL_BYTE)?)
            }
            Packet::Unsuback(pid) => {
                const CONTROL_BYTE: u8 = 0b10110000;
                VarBytes::Fixed4(encode_with_pid(CONTROL_BYTE, *pid))
            }
            Packet::Pingreq => {
                const CONTROL_BYTE: u8 = 0b11000000;
                VarBytes::Fixed2([CONTROL_BYTE, VOID_PACKET_REMAINING_LEN])
            }
            Packet::Pingresp => {
                const CONTROL_BYTE: u8 = 0b11010000;
                VarBytes::Fixed2([CONTROL_BYTE, VOID_PACKET_REMAINING_LEN])
            }
            Packet::Disconnect => {
                const CONTROL_BYTE: u8 = 0b11100000;
                VarBytes::Fixed2([CONTROL_BYTE, VOID_PACKET_REMAINING_LEN])
            }
        };
        Ok(data)
    }

    /// Return the total length of bytes the packet encoded into.
    pub fn encode_len(&self) -> Result<usize, Error> {
        let remaining_len = match self {
            Packet::Connect(inner) => inner.encode_len(),
            Packet::Connack(_) => 2,
            Packet::Publish(inner) => inner.encode_len(),
            Packet::Puback(_) => 2,
            Packet::Pubrec(_) => 2,
            Packet::Pubrel(_) => 2,
            Packet::Pubcomp(_) => 2,
            Packet::Subscribe(inner) => inner.encode_len(),
            Packet::Suback(inner) => inner.encode_len(),
            Packet::Unsubscribe(inner) => inner.encode_len(),
            Packet::Unsuback(_) => 2,
            Packet::Pingreq => 0,
            Packet::Pingresp => 0,
            Packet::Disconnect => 0,
        };
        total_len(remaining_len)
    }
}

/// A bytes data structure represent a dynamic vector or fixed array.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VarBytes {
    Dynamic(Vec<u8>),
    Fixed2([u8; 2]),
    Fixed4([u8; 4]),
}

impl VarBytes {
    /// Return the slice of the internal bytes.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            VarBytes::Dynamic(vec) => vec,
            VarBytes::Fixed2(arr) => &arr[..],
            VarBytes::Fixed4(arr) => &arr[..],
        }
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

    pub fn new_with(hd: u8, remaining_len: usize) -> Result<Header, Error> {
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
            _ => return Err(Error::InvalidHeader),
        };
        if !flags_ok {
            return Err(Error::InvalidHeader);
        }
        Ok(Header {
            typ,
            dup: hd & 0b1000 != 0,
            qos: QoS::from_u8((hd & 0b110) >> 1)?,
            retain: hd & 1 == 1,
            remaining_len,
        })
    }

    pub fn decode(mut reader: &[u8]) -> Result<Self, Error> {
        block_on(Self::decode_async(&mut reader))
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let (typ, remaining_len) = decode_raw_header(reader).await?;
        Header::new_with(typ, remaining_len)
    }
}

#[inline]
fn encode_with_pid(control_byte: u8, pid: Pid) -> [u8; 4] {
    const REMAINING_LEN: u8 = 2;
    let val = pid.value();
    [
        control_byte,
        REMAINING_LEN,
        (val >> 8) as u8,
        (val & 0xFF) as u8,
    ]
}

#[inline]
fn encode_header(buf: &mut Vec<u8>, control_byte: u8, mut remaining_len: usize) {
    buf.push(control_byte);
    write_var_int(buf, remaining_len).expect("encode header write var int");
}

#[inline]
fn encode_inner<E: Encodable>(inner: &E, control_byte: u8) -> Result<Vec<u8>, Error> {
    let remaining_len = inner.encode_len();
    let total = total_len(remaining_len)?;
    let mut buf = Vec::with_capacity(total);
    encode_header(&mut buf, control_byte, remaining_len);
    inner.encode(&mut buf)?;
    debug_assert_eq!(buf.len(), total);
    Ok(buf)
}

packet_from!(Connect, Publish, Suback, Connack, Subscribe, Unsubscribe);
