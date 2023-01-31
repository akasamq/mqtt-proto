use std::convert::TryFrom;
use std::io;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt};

use crate::{
    read_bytes, read_string, read_u16, read_u8, write_bytes, write_u16, write_u8, Encodable, Error,
    Protocol, QoS, TopicName,
};

/// Connect packet payload type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    pub protocol: Protocol,
    pub clean_session: bool,
    pub keep_alive: u16,
    pub client_id: Arc<String>,
    pub last_will: Option<LastWill>,
    pub username: Option<Arc<String>>,
    pub password: Option<Bytes>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Connect {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Connect {
            protocol: u.arbitrary()?,
            clean_session: u.arbitrary()?,
            keep_alive: u.arbitrary()?,
            client_id: u.arbitrary()?,
            last_will: u.arbitrary()?,
            username: u.arbitrary()?,
            password: Option::<Vec<u8>>::arbitrary(u)?.map(Bytes::from),
        })
    }
}

impl Connect {
    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let protocol = Protocol::decode_async(reader).await?;
        Self::decode_with_protocol(reader, protocol).await
    }

    #[inline]
    pub async fn decode_with_protocol<T: AsyncRead + Unpin>(
        reader: &mut T,
        protocol: Protocol,
    ) -> Result<Self, Error> {
        if protocol as u8 > 4 {
            return Err(Error::UnexpectedProtocol(protocol));
        }
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
        } else if connect_flags & 0b11000 != 0 {
            return Err(Error::InvalidConnectFlags(connect_flags));
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

        self.protocol.encode(writer)?;
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

/// Connack packet payload type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Connack {
    pub session_present: bool,
    pub code: ConnectReturnCode,
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

/// Message that the server should publish when the client disconnects.
///
/// Sent by the client in the [Connect] packet. [MQTT 3.1.3.3].
///
/// [Connect]: struct.Connect.html
/// [MQTT 3.1.3.3]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718031
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWill {
    pub qos: QoS,
    pub retain: bool,
    pub topic_name: TopicName,
    pub message: Bytes,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for LastWill {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(LastWill {
            qos: u.arbitrary()?,
            retain: u.arbitrary()?,
            topic_name: u.arbitrary()?,
            message: Bytes::from(Vec::<u8>::arbitrary(u)?),
        })
    }
}

/// Return code of a [Connack] packet.
///
/// See [MQTT 3.2.2.3] for interpretations.
///
/// [Connack]: struct.Connack.html
/// [MQTT 3.2.2.3]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718035
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
