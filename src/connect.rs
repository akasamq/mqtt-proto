use std::convert::TryFrom;
use std::io;
use std::slice;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt};

use crate::{
    read_bytes, read_string, read_u16, read_u8, write_bytes, write_u16, write_u8, Encodable, Error,
    QoS, TopicName,
};

pub const MQISDP: &[u8] = b"MQIsdp";
pub const MQTT: &[u8] = b"MQTT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    pub protocol: Protocol,
    pub keep_alive: u16,
    pub client_id: Arc<String>,
    pub clean_session: bool,
    pub last_will: Option<LastWill>,
    pub username: Option<Arc<String>>,
    pub password: Option<Bytes>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connack {
    pub session_present: bool,
    pub code: ConnectReturnCode,
}

impl Connect {
    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let protocol = Protocol::decode_async(reader).await?;
        let connect_flags: u8 = read_u8(reader).await?;
        let keep_alive = read_u16(reader).await?;
        let client_id = Arc::new(read_string(reader).await?);
        let last_will = if connect_flags & 0b100 != 0 {
            let topic_name = read_string(reader).await?;
            let message = read_bytes(reader).await?;
            let qos = QoS::from_u8((connect_flags & 0b11000) >> 3)?;
            let retain = (connect_flags & 0b00100000) != 0;
            Some(LastWill {
                topic_name: TopicName::try_from(topic_name)?,
                message: Bytes::from(message),
                qos,
                retain,
            })
        } else {
            None
        };
        let username = if connect_flags & 0b10000000 != 0 {
            Some(Arc::new(read_string(reader).await?))
        } else {
            None
        };
        let password = if connect_flags & 0b01000000 != 0 {
            Some(Bytes::from(read_bytes(reader).await?))
        } else {
            None
        };
        let clean_session = (connect_flags & 0b10) != 0;
        Ok(Connect {
            protocol,
            keep_alive,
            client_id,
            username,
            password,
            last_will,
            clean_session,
        })
    }
}

impl Encodable for Connect {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        self.protocol.encode(writer)?;
        let mut connect_flags: u8 = 0b00000000;
        if self.clean_session {
            connect_flags |= 0b10;
        }
        if self.username.is_some() {
            connect_flags |= 0b10000000;
        }
        if self.password.is_some() {
            connect_flags |= 0b01000000;
        }
        if let Some(last_will) = self.last_will.as_ref() {
            connect_flags |= 0b00000100;
            connect_flags |= (last_will.qos as u8) << 3;
            if last_will.retain {
                connect_flags |= 0b00100000;
            }
        }

        write_u8(writer, connect_flags)?;
        write_u16(writer, self.keep_alive)?;
        write_bytes(writer, self.client_id.as_bytes())?;
        if let Some(last_will) = self.last_will.as_ref() {
            write_bytes(writer, last_will.topic_name.as_bytes())?;
            write_bytes(writer, last_will.message.as_ref())?;
        }
        if let Some(username) = self.username.as_ref() {
            write_bytes(writer, username.as_bytes())?;
        }
        if let Some(password) = self.password.as_ref() {
            write_bytes(writer, password.as_ref())?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut length = self.protocol.encode_len();
        // flags + keep-alive
        length += 1 + 2;
        // client identifier
        length += 2 + self.client_id.len();
        if let Some(last_will) = self.last_will.as_ref() {
            length += 4;
            length += last_will.topic_name.len();
            length += last_will.message.len();
        }
        if let Some(username) = self.username.as_ref() {
            length += 2 + username.len();
        }
        if let Some(password) = self.password.as_ref() {
            length += 2 + password.len();
        }
        length
    }
}

impl Connack {
    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let mut payload = [0u8; 2];
        reader.read_exact(&mut payload).await?;
        let session_present = match payload[0] {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidConnackFlags(payload[0])),
        };
        let code = ConnectReturnCode::from_u8(payload[1])?;
        Ok(Connack {
            session_present,
            code,
        })
    }
}

/// Protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// [MQTT 3.1]
    /// https://public.dhe.ibm.com/software/dw/webservices/ws-mqtt/mqtt-v3r1.html
    MQTT310 = 3,

    /// [MQTT 3.1.1] is the most commonly implemented version.
    /// [MQTT 3.1.1]: https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html
    MQTT311 = 4,

    /// [MQTT 5.0]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html
    MQTT500 = 5,
}

impl Protocol {
    pub fn new(name: &[u8], level: u8) -> Result<Protocol, Error> {
        match (name, level) {
            (MQISDP, 3) => Ok(Protocol::MQTT310),
            (MQTT, 4) => Ok(Protocol::MQTT311),
            (MQTT, 5) => Ok(Protocol::MQTT500),
            _ => {
                let name = core::str::from_utf8(name)?;
                Err(Error::InvalidProtocol(name.into(), level))
            }
        }
    }

    pub fn to_pair(self) -> (&'static [u8], u8) {
        match self {
            Self::MQTT310 => (MQISDP, 3),
            Self::MQTT311 => (MQTT, 4),
            Self::MQTT500 => (MQTT, 5),
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let name_buf = read_bytes(reader).await?;
        let level = read_u8(reader).await?;
        Protocol::new(&name_buf, level)
    }
}

impl Encodable for Protocol {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let (name, level) = self.to_pair();
        writer.write_all(&(name.len() as u16).to_be_bytes())?;
        writer.write_all(name)?;
        writer.write_all(slice::from_ref(&level))?;
        Ok(())
    }

    fn encode_len(&self) -> usize {
        match self {
            Self::MQTT310 => 2 + 6 + 1,
            Self::MQTT311 => 2 + 4 + 1,
            Self::MQTT500 => 2 + 4 + 1,
        }
    }
}

/// Message that the server should publish when the client disconnects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWill {
    pub topic_name: TopicName,
    pub message: Bytes,
    pub qos: QoS,
    pub retain: bool,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectReturnCode {
    Accepted = 0,
    UnacceptableProtocolVersion = 1,
    IdentifierRejected = 2,
    ServerUnavailable = 3,
    BadUsernamePassword = 4,
    NotAuthorized = 5,
}

impl ConnectReturnCode {
    pub(crate) fn from_u8(byte: u8) -> Result<ConnectReturnCode, Error> {
        match byte {
            0 => Ok(ConnectReturnCode::Accepted),
            1 => Ok(ConnectReturnCode::UnacceptableProtocolVersion),
            2 => Ok(ConnectReturnCode::IdentifierRejected),
            3 => Ok(ConnectReturnCode::ServerUnavailable),
            4 => Ok(ConnectReturnCode::BadUsernamePassword),
            5 => Ok(ConnectReturnCode::NotAuthorized),
            n => Err(Error::InvalidConnectReturnCode(n)),
        }
    }
}
