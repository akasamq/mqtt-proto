use std::io;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Encodable, Error, QoS, TopicName};

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
    pub async fn decode<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let protocol = Protocol::decode(reader).await?;
        todo!()
    }
}

impl Encodable for Connect {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        todo!()
    }
    fn encode_len(&self) -> usize {
        todo!()
    }
}

impl Connack {
    pub async fn decode<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        todo!()
    }
}

impl Encodable for Connack {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        todo!()
    }
    fn encode_len(&self) -> usize {
        todo!()
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

    pub async fn decode<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let mut strlen_buf = [0u8; 2];
        reader.read_exact(&mut strlen_buf).await?;
        let len = u16::from_be_bytes(strlen_buf) as usize;
        let mut name_buf = vec![0; len];
        reader.read_exact(&mut name_buf).await?;
        let mut level = 0u8;
        reader.read_exact(std::slice::from_mut(&mut level)).await?;
        Protocol::new(&name_buf, level)
    }

    pub fn encode<W: io::Write>(self, writer: &mut W) -> io::Result<()> {
        let (name, level) = self.to_pair();
        writer.write_all(&(name.len() as u16).to_be_bytes())?;
        writer.write_all(name)?;
        writer.write_all(std::slice::from_ref(&level))?;
        Ok(())
    }

    pub fn encode_len(self) -> usize {
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
    pub topic: TopicName,
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
