use futures_lite::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    Connack, Connect, Encodable, Error, Pid, Publish, QoS, QosPid, Suback, Subscribe, Unsubscribe,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    Connect(Connect),
    Connack(Connack),
    Publish(Publish),
    Puback(Pid),
    Pubrec(Pid),
    Pubrel(Pid),
    Pubcomp(Pid),
    Subscribe(Subscribe),
    Suback(Suback),
    Unsubscribe(Unsubscribe),
    Unsuback(Pid),
    Pingreq,
    Pingresp,
    Disconnect,
}

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

    pub async fn decode<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let header = Header::decode(reader).await?;
        Ok(match header.typ {
            PacketType::Pingreq => Packet::Pingreq,
            PacketType::Pingresp => Packet::Pingresp,
            PacketType::Disconnect => Packet::Disconnect,

            PacketType::Connect => Connect::decode(reader).await?.into(),
            PacketType::Connack => Connack::decode(reader).await?.into(),
            PacketType::Publish => Publish::decode(reader, header).await?.into(),
            PacketType::Puback => Packet::Puback(Pid::decode(reader).await?),
            PacketType::Pubrec => Packet::Pubrec(Pid::decode(reader).await?),
            PacketType::Pubrel => Packet::Pubrel(Pid::decode(reader).await?),
            PacketType::Pubcomp => Packet::Pubcomp(Pid::decode(reader).await?),
            PacketType::Subscribe => Subscribe::decode(reader, header.remaining_len)
                .await?
                .into(),
            PacketType::Suback => Suback::decode(reader, header.remaining_len).await?.into(),
            PacketType::Unsubscribe => Unsubscribe::decode(reader, header.remaining_len)
                .await?
                .into(),
            PacketType::Unsuback => Packet::Unsuback(Pid::decode(reader).await?),
        })
    }

    pub async fn encode<T: AsyncWrite + Unpin>(&self, writer: &mut T) -> Result<(), Error> {
        match self {
            Packet::Connect(connect) => {
                let header: u8 = 0b00010000;
                writer.write_all(&encode_inner(connect, header)?).await?;
            }
            Packet::Connack(connack) => {
                let header: u8 = 0b00100000;
                writer.write_all(&encode_inner(connack, header)?).await?;
            }
            Packet::Publish(publish) => {
                let mut header: u8 = match publish.qos_pid {
                    QosPid::Level0 => 0b00110000,
                    QosPid::Level1(_) => 0b00110010,
                    QosPid::Level2(_) => 0b00110100,
                };
                if publish.dup {
                    header |= 0b00001000;
                }
                if publish.retain {
                    header |= 0b00000001;
                }
                writer.write_all(&encode_inner(publish, header)?).await?;
            }
            Packet::Puback(pid) => {
                let header: u8 = 0b01000000;
                let length: u8 = 2;
                writer.write_all(&with_pid(header, length, *pid)).await?;
            }
            Packet::Pubrec(pid) => {
                let header: u8 = 0b01010000;
                let length: u8 = 2;
                writer.write_all(&with_pid(header, length, *pid)).await?;
            }
            Packet::Pubrel(pid) => {
                let header: u8 = 0b01100010;
                let length: u8 = 2;
                writer.write_all(&with_pid(header, length, *pid)).await?;
            }
            Packet::Pubcomp(pid) => {
                let header: u8 = 0b01110000;
                let length: u8 = 2;
                writer.write_all(&with_pid(header, length, *pid)).await?;
            }
            Packet::Subscribe(subscribe) => {
                let header: u8 = 0b10000010;
                writer.write_all(&encode_inner(subscribe, header)?).await?;
            }
            Packet::Suback(suback) => {
                let header: u8 = 0b10010000;
                writer.write_all(&encode_inner(suback, header)?).await?;
            }
            Packet::Unsubscribe(unsubscribe) => {
                let header = 0b10100010;
                writer
                    .write_all(&encode_inner(unsubscribe, header)?)
                    .await?;
            }
            Packet::Unsuback(pid) => {
                let header: u8 = 0b10110000;
                let length: u8 = 2;
                writer.write_all(&with_pid(header, length, *pid)).await?;
            }
            Packet::Pingreq => {
                let header: u8 = 0b11000000;
                let length: u8 = 0;
                writer.write_all(&[header, length]).await?;
            }
            Packet::Pingresp => {
                let header: u8 = 0b11010000;
                let length: u8 = 0;
                writer.write_all(&[header, length]).await?;
            }
            Packet::Disconnect => {
                let header: u8 = 0b11100000;
                let length: u8 = 0;
                writer.write_all(&[header, length]).await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Header {
    pub typ: PacketType,
    pub dup: bool,
    pub qos: QoS,
    pub retain: bool,
    pub remaining_len: usize,
}

impl Header {
    pub fn new(hd: u8, remaining_len: usize) -> Result<Header, Error> {
        let (typ, flags_ok) = match hd >> 4 {
            1 => (PacketType::Connect, hd & 0b1111 == 0),
            2 => (PacketType::Connack, hd & 0b1111 == 0),
            3 => (PacketType::Publish, true),
            4 => (PacketType::Puback, hd & 0b1111 == 0),
            5 => (PacketType::Pubrec, hd & 0b1111 == 0),
            6 => (PacketType::Pubrel, hd & 0b1111 == 0b0010),
            7 => (PacketType::Pubcomp, hd & 0b1111 == 0),
            8 => (PacketType::Subscribe, hd & 0b1111 == 0b0010),
            9 => (PacketType::Suback, hd & 0b1111 == 0),
            10 => (PacketType::Unsubscribe, hd & 0b1111 == 0b0010),
            11 => (PacketType::Unsuback, hd & 0b1111 == 0),
            12 => (PacketType::Pingreq, hd & 0b1111 == 0),
            13 => (PacketType::Pingresp, hd & 0b1111 == 0),
            14 => (PacketType::Disconnect, hd & 0b1111 == 0),
            _ => (PacketType::Connect, false),
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

    pub async fn decode<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let mut typ = 0u8;
        reader.read_exact(std::slice::from_mut(&mut typ)).await?;

        let mut byte = 0u8;
        let mut remaining_len: usize = 0;
        let mut i = 0;
        loop {
            reader.read_exact(std::slice::from_mut(&mut byte)).await?;
            remaining_len |= (usize::from(byte) & 0x7F) << (7 * i);
            if byte & 0x80 == 0 {
                break;
            } else if i < 3 {
                i += 1;
            } else {
                return Err(Error::InvalidHeader);
            }
        }
        Header::new(typ, remaining_len)
    }
}

#[inline]
pub fn total_len(remaining_len: usize) -> Result<usize, Error> {
    let header_len = if remaining_len < 128 {
        2
    } else if remaining_len < 16384 {
        3
    } else if remaining_len < 2097152 {
        4
    } else if remaining_len < 268435456 {
        5
    } else {
        return Err(Error::InvalidLength);
    };
    Ok(header_len + remaining_len)
}

#[inline]
fn with_pid(header: u8, length: u8, pid: Pid) -> [u8; 4] {
    let val = pid.get();
    [header, length, (val >> 8) as u8, (val & 0xFF) as u8]
}

#[inline]
fn encode_header(buf: &mut Vec<u8>, header: u8, mut remaining_len: usize) {
    buf.push(header);
    loop {
        let mut byte = (remaining_len % 128) as u8;
        remaining_len /= 128;
        if remaining_len > 0 {
            byte |= 128;
        }
        buf.push(byte);
        if remaining_len == 0 {
            break;
        }
    }
}

#[inline]
fn encode_inner<E: Encodable>(inner: &E, header: u8) -> Result<Vec<u8>, Error> {
    let remaining_len = inner.encode_len();
    let total = total_len(remaining_len)?;
    let mut buf = Vec::with_capacity(total);
    encode_header(&mut buf, header, remaining_len);
    inner.encode(&mut buf)?;
    debug_assert_eq!(buf.len(), total);
    Ok(buf)
}

macro_rules! packet_from {
    ($($t:ident),+) => {
        $(
            impl<'a> From<$t> for Packet {
                fn from(p: $t) -> Self {
                    Packet::$t(p)
                }
            }
        )+
    }
}

packet_from!(Connect, Publish, Suback, Connack, Subscribe, Unsubscribe);
