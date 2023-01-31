use std::convert::TryFrom;
use std::io;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt};

use super::{
    decode_properties, encode_properties, encode_properties_len, ErrorV5, Header, PacketType,
    UserProperty,
};
use crate::{
    read_string, read_u16, read_u8, write_bytes, write_u16, write_u8, Encodable, Error, Pid, QoS,
    QosPid, TopicName,
};

/// Payload type of PUBLISH packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publish {
    pub dup: bool,
    pub qos_pid: QosPid,
    pub retain: bool,
    pub topic_name: TopicName,
    pub properties: PublishProperties,
    pub payload: Bytes,
}

impl Publish {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len;
        let topic_name = read_string(reader).await?;
        remaining_len = remaining_len
            .checked_sub(2 + topic_name.len())
            .ok_or(Error::InvalidRemainingLength)?;
        let qos_pid = match header.qos {
            QoS::Level0 => QosPid::Level0,
            QoS::Level1 => {
                remaining_len = remaining_len
                    .checked_sub(2)
                    .ok_or(Error::InvalidRemainingLength)?;
                QosPid::Level1(Pid::try_from(read_u16(reader).await?)?)
            }
            QoS::Level2 => {
                remaining_len = remaining_len
                    .checked_sub(2)
                    .ok_or(Error::InvalidRemainingLength)?;
                QosPid::Level2(Pid::try_from(read_u16(reader).await?)?)
            }
        };
        let properties = PublishProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        let payload = if remaining_len > 0 {
            let mut data = vec![0u8; remaining_len];
            reader
                .read_exact(&mut data)
                .await
                .map_err(|err| Error::IoError(err.kind(), err.to_string()))?;
            data
        } else {
            Vec::new()
        };
        Ok(Publish {
            dup: header.dup,
            qos_pid,
            retain: header.retain,
            topic_name: TopicName::try_from(topic_name)?,
            properties,
            payload: Bytes::from(payload),
        })
    }
}

impl Encodable for Publish {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_bytes(writer, self.topic_name.as_bytes())?;
        match self.qos_pid {
            QosPid::Level0 => {}
            QosPid::Level1(pid) | QosPid::Level2(pid) => {
                write_u16(writer, pid.value())?;
            }
        }
        self.properties.encode(writer)?;
        writer.write_all(self.payload.as_ref())?;
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 2 + self.topic_name.len();
        match self.qos_pid {
            QosPid::Level0 => {}
            QosPid::Level1(_) | QosPid::Level2(_) => {
                len += 2;
            }
        }
        len += self.properties.encode_len();
        len += self.payload.len();
        len
    }
}

/// Property list for PUBLISH packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishProperties {
    pub payload_is_utf8: Option<bool>,
    pub message_expiry_interval: Option<u32>,
    pub topic_alias: Option<u16>,
    pub response_topic: Option<TopicName>,
    pub correlation_data: Option<Bytes>,
    pub user_properties: Vec<UserProperty>,
    pub subscription_id: Option<usize>,
    pub content_type: Option<Arc<String>>,
}

impl PublishProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = PublishProperties::default();
        decode_properties!(
            packet_type,
            properties,
            reader,
            PayloadFormatIndicator,
            MessageExpiryInterval,
            TopicAlias,
            ResponseTopic,
            CorrelationData,
            SubscriptionIdentifier,
            ContentType,
        );
        Ok(properties)
    }
}

impl Encodable for PublishProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        encode_properties!(
            self,
            writer,
            PayloadFormatIndicator,
            MessageExpiryInterval,
            TopicAlias,
            ResponseTopic,
            CorrelationData,
            SubscriptionIdentifier,
            ContentType,
        );
        Ok(())
    }
    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(
            self,
            len,
            PayloadFormatIndicator,
            MessageExpiryInterval,
            TopicAlias,
            ResponseTopic,
            CorrelationData,
            SubscriptionIdentifier,
            ContentType,
        );
        len
    }
}

/// Payload type for PUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puback {
    pub pid: Pid,
    pub reason_code: PubackReasonCode,
    pub properties: PubackProperties,
}

impl Puback {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            let reason_code = PubackReasonCode::Success;
            let properties = PubackProperties::default();
            (reason_code, properties)
        } else {
            let reason_byte = read_u8(reader).await?;
            let reason_code = PubackReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            let properties = PubackProperties::decode_async(reader, header.typ).await?;
            (reason_code, properties)
        };
        Ok(Puback {
            pid,
            reason_code,
            properties,
        })
    }
}

impl Encodable for Puback {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        if self.reason_code != PubackReasonCode::Success
            || self.properties != PubackProperties::default()
        {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.reason_code == PubackReasonCode::Success
            && self.properties == PubackProperties::default()
        {
            2
        } else {
            2 + 1 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PubackProperties {
    pub reason_string: Option<Arc<String>>,
    pub user_properties: Vec<UserProperty>,
}

impl PubackProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = PubackProperties::default();
        decode_properties!(packet_type, properties, reader, ReasonString,);
        Ok(properties)
    }
}

impl Encodable for PubackProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        encode_properties!(self, writer, ReasonString,);
        Ok(())
    }
    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(self, len, ReasonString,);
        len
    }
}

/// Reason code for PUBACK packet.
///
/// | Dec |  Hex | Reason Code name              | Description                                                                                                        |
/// |-----|------|-------------------------------|--------------------------------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Success                       | The message is accepted. Publication of the QoS 1 message proceeds.                                                |
/// |  16 | 0x10 | No matching subscribers       | The message is accepted but there are no subscribers. This is sent only by the Server.                             |
/// |     |      |                               | If the Server knows that there are no matching subscribers, it MAY use this Reason Code instead of 0x00 (Success). |
/// | 128 | 0x80 | Unspecified error             | The receiver does not accept the publish but either does not want to reveal the reason,                            |
/// |     |      |                               | or it does not match one of the other values.                                                                      |
/// | 131 | 0x83 | Implementation specific error | The PUBLISH is valid but the receiver is not willing to accept it.                                                 |
/// | 135 | 0x87 | Not authorized                | The PUBLISH is not authorized.                                                                                     |
/// | 144 | 0x90 | Topic Name invalid            | The Topic Name is not malformed, but is not accepted by this Client or Server.                                     |
/// | 145 | 0x91 | Packet identifier in use      | The Packet Identifier is already in use.                                                                           |
/// |     |      |                               | This might indicate a mismatch in the Session State between the Client and Server.                                 |
/// | 151 | 0x97 | Quota exceeded                | An implementation or administrative imposed limit has been exceeded.                                               |
/// | 153 | 0x99 | Payload format invalid        | The payload format does not match the specified Payload Format Indicator.                                          |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PubackReasonCode {
    Success = 0x00,
    NoMatchingSubscribers = 0x10,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicNameInvalid = 0x90,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
}

impl PubackReasonCode {
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => Self::Success,
            0x10 => Self::NoMatchingSubscribers,
            0x80 => Self::UnspecifiedError,
            0x83 => Self::ImplementationSpecificError,
            0x87 => Self::NotAuthorized,
            0x90 => Self::TopicNameInvalid,
            0x91 => Self::PacketIdentifierInUse,
            0x97 => Self::QuotaExceeded,
            0x99 => Self::PayloadFormatInvalid,
            _ => return None,
        };
        Some(code)
    }
}

/// Payload type for PUBREC packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pubrec {
    pub pid: Pid,
    pub reason_code: PubrecReasonCode,
    pub properties: PubrecProperties,
}

impl Pubrec {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            let reason_code = PubrecReasonCode::Success;
            let properties = PubrecProperties::default();
            (reason_code, properties)
        } else {
            let reason_byte = read_u8(reader).await?;
            let reason_code = PubrecReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            let properties = PubrecProperties::decode_async(reader, header.typ).await?;
            (reason_code, properties)
        };
        Ok(Pubrec {
            pid,
            reason_code,
            properties,
        })
    }
}

impl Encodable for Pubrec {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        if self.reason_code != PubrecReasonCode::Success
            || self.properties != PubrecProperties::default()
        {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.reason_code == PubrecReasonCode::Success
            && self.properties == PubrecProperties::default()
        {
            2
        } else {
            2 + 1 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBREC packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PubrecProperties {
    pub reason_string: Option<Arc<String>>,
    pub user_properties: Vec<UserProperty>,
}

impl PubrecProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = PubrecProperties::default();
        decode_properties!(packet_type, properties, reader, ReasonString,);
        Ok(properties)
    }
}

impl Encodable for PubrecProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        encode_properties!(self, writer, ReasonString,);
        Ok(())
    }
    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(self, len, ReasonString,);
        len
    }
}

/// Reason code for PUBREC packet.
///
/// | Dec |  Hex | Reason Code name              | Description                                                                                                        |
/// |-----|------|-------------------------------|--------------------------------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Success                       | The message is accepted. Publication of the QoS 2 message proceeds.                                                |
/// |  16 | 0x10 | No matching subscribers       | The message is accepted but there are no subscribers. This is sent only by the Server.                             |
/// |     |      |                               | If the Server knows that there are no matching subscribers, it MAY use this Reason Code instead of 0x00 (Success). |
/// | 128 | 0x80 | Unspecified error             | The receiver does not accept the publish but either does not want to reveal the reason,                            |
/// |     |      |                               | or it does not match one of the other values.                                                                      |
/// | 131 | 0x83 | Implementation specific error | The PUBLISH is valid but the receiver is not willing to accept it.                                                 |
/// | 135 | 0x87 | Not authorized                | The PUBLISH is not authorized.                                                                                     |
/// | 144 | 0x90 | Topic Name invalid            | The Topic Name is not malformed, but is not accepted by this Client or Server.                                     |
/// | 145 | 0x91 | Packet identifier in use      | The Packet Identifier is already in use.                                                                           |
/// |     |      |                               | This might indicate a mismatch in the Session State between the Client and Server.                                 |
/// | 151 | 0x97 | Quota exceeded                | An implementation or administrative imposed limit has been exceeded.                                               |
/// | 153 | 0x99 | Payload format invalid        | The payload format does not match the specified Payload Format Indicator.                                          |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PubrecReasonCode {
    Success = 0x00,
    NoMatchingSubscribers = 0x10,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicNameInvalid = 0x90,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
}

impl PubrecReasonCode {
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => Self::Success,
            0x10 => Self::NoMatchingSubscribers,
            0x80 => Self::UnspecifiedError,
            0x83 => Self::ImplementationSpecificError,
            0x87 => Self::NotAuthorized,
            0x90 => Self::TopicNameInvalid,
            0x91 => Self::PacketIdentifierInUse,
            0x97 => Self::QuotaExceeded,
            0x99 => Self::PayloadFormatInvalid,
            _ => return None,
        };
        Some(code)
    }
}

/// Payload type for PUBREL packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pubrel {
    pub pid: Pid,
    pub reason_code: PubrelReasonCode,
    pub properties: PubrelProperties,
}

impl Pubrel {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            let reason_code = PubrelReasonCode::Success;
            let properties = PubrelProperties::default();
            (reason_code, properties)
        } else {
            let reason_byte = read_u8(reader).await?;
            let reason_code = PubrelReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            let properties = PubrelProperties::decode_async(reader, header.typ).await?;
            (reason_code, properties)
        };
        Ok(Pubrel {
            pid,
            reason_code,
            properties,
        })
    }
}

impl Encodable for Pubrel {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        if self.reason_code != PubrelReasonCode::Success
            || self.properties != PubrelProperties::default()
        {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.reason_code == PubrelReasonCode::Success
            && self.properties == PubrelProperties::default()
        {
            2
        } else {
            2 + 1 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBREL packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PubrelProperties {
    pub reason_string: Option<Arc<String>>,
    pub user_properties: Vec<UserProperty>,
}

impl PubrelProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = PubrelProperties::default();
        decode_properties!(packet_type, properties, reader, ReasonString,);
        Ok(properties)
    }
}

impl Encodable for PubrelProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        encode_properties!(self, writer, ReasonString,);
        Ok(())
    }
    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(self, len, ReasonString,);
        len
    }
}

/// Reason code for PUBREL packet.
///
/// | Dec |  Hex | Reason Code name            | Description                                                                                 |
/// |-----|------|-----------------------------|---------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Success                     | Message released.                                                                           |
/// | 146 | 0x92 | Packet Identifier not found | The Packet Identifier is not known. This is not an error during recovery,                   |
/// |     |      |                             | but at other times indicates a mismatch between the Session State on the Client and Server. |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PubrelReasonCode {
    Success = 0x00,
    PacketIdentifierNotFound = 0x92,
}

impl PubrelReasonCode {
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => Self::Success,
            0x92 => Self::PacketIdentifierNotFound,
            _ => return None,
        };
        Some(code)
    }
}

/// Payload type for PUBCOMP packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pubcomp {
    pub pid: Pid,
    pub reason_code: PubcompReasonCode,
    pub properties: PubcompProperties,
}

impl Pubcomp {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            let reason_code = PubcompReasonCode::Success;
            let properties = PubcompProperties::default();
            (reason_code, properties)
        } else {
            let reason_byte = read_u8(reader).await?;
            let reason_code = PubcompReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            let properties = PubcompProperties::decode_async(reader, header.typ).await?;
            (reason_code, properties)
        };
        Ok(Pubcomp {
            pid,
            reason_code,
            properties,
        })
    }
}

impl Encodable for Pubcomp {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        if self.reason_code != PubcompReasonCode::Success
            || self.properties != PubcompProperties::default()
        {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.reason_code == PubcompReasonCode::Success
            && self.properties == PubcompProperties::default()
        {
            2
        } else {
            2 + 1 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBCOMP packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PubcompProperties {
    pub reason_string: Option<Arc<String>>,
    pub user_properties: Vec<UserProperty>,
}

impl PubcompProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = PubcompProperties::default();
        decode_properties!(packet_type, properties, reader, ReasonString,);
        Ok(properties)
    }
}

impl Encodable for PubcompProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        encode_properties!(self, writer, ReasonString,);
        Ok(())
    }
    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(self, len, ReasonString,);
        len
    }
}

/// Reason code for PUBCOMP packet.
///
/// | Dec |  Hex | Reason Code name            | Description                                                                                 |
/// |-----|------|-----------------------------|---------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Success                     | Packet Identifier released. Publication of QoS 2 message is complete.                       |
/// | 146 | 0x92 | Packet Identifier not found | The Packet Identifier is not known. This is not an error during recovery,                   |
/// |     |      |                             | but at other times indicates a mismatch between the Session State on the Client and Server. |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PubcompReasonCode {
    Success = 0x00,
    PacketIdentifierNotFound = 0x92,
}

impl PubcompReasonCode {
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => Self::Success,
            0x92 => Self::PacketIdentifierNotFound,
            _ => return None,
        };
        Some(code)
    }
}
