use core::convert::TryFrom;

use alloc::sync::Arc;
use alloc::vec::Vec;

use bytes::Bytes;
use simdutf8::basic::from_utf8;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;

use crate::{
    read_string_async, read_u16_async, read_u8_async, write_bytes, write_u16, write_u8, AsyncRead,
    Encodable, Error, Pid, QoS, QosPid, SyncWrite, ToError, TopicName,
};

use super::{
    decode_properties, encode_properties, encode_properties_len, ErrorV5, Header, PacketType,
    UserProperty, VarByteInt,
};

/// Body type of PUBLISH packet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Publish {
    pub dup: bool,
    pub retain: bool,
    pub qos_pid: QosPid,
    pub topic_name: TopicName,
    pub payload: Bytes,
    pub properties: PublishProperties,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Publish {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Publish {
            dup: u.arbitrary()?,
            retain: u.arbitrary()?,
            qos_pid: u.arbitrary()?,
            topic_name: u.arbitrary()?,
            properties: u.arbitrary()?,
            payload: Bytes::from(Vec::<u8>::arbitrary(u)?),
        })
    }
}

impl Publish {
    pub fn new(qos_pid: QosPid, topic_name: TopicName, payload: Bytes) -> Self {
        Publish {
            dup: false,
            retain: false,
            qos_pid,
            topic_name,
            payload,
            properties: PublishProperties::default(),
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len as usize;
        let topic_name = read_string_async(reader).await?;
        remaining_len = remaining_len
            .checked_sub(2 + topic_name.len())
            .ok_or(Error::InvalidRemainingLength)?;
        let qos_pid = match header.qos {
            QoS::Level0 => QosPid::Level0,
            QoS::Level1 => {
                remaining_len = remaining_len
                    .checked_sub(2)
                    .ok_or(Error::InvalidRemainingLength)?;
                QosPid::Level1(Pid::try_from(read_u16_async(reader).await?)?)
            }
            QoS::Level2 => {
                remaining_len = remaining_len
                    .checked_sub(2)
                    .ok_or(Error::InvalidRemainingLength)?;
                QosPid::Level2(Pid::try_from(read_u16_async(reader).await?)?)
            }
        };
        let properties = PublishProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        let payload = if remaining_len > 0 {
            let mut data = alloc::vec![0u8; remaining_len];
            reader
                .read_exact(&mut data)
                .await
                .map_err(ToError::to_error)?;
            if properties.payload_is_utf8 == Some(true) && from_utf8(&data).is_err() {
                return Err(ErrorV5::InvalidPayloadFormat);
            }
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct PublishProperties {
    pub payload_is_utf8: Option<bool>,
    pub message_expiry_interval: Option<u32>,
    pub topic_alias: Option<u16>,
    pub response_topic: Option<TopicName>,
    pub correlation_data: Option<Bytes>,
    pub user_properties: Vec<UserProperty>,
    // FIXME: this is a list of identifiers
    pub subscription_id: Option<VarByteInt>,
    pub content_type: Option<Arc<str>>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for PublishProperties {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(PublishProperties {
            payload_is_utf8: u.arbitrary()?,
            message_expiry_interval: u.arbitrary()?,
            topic_alias: u.arbitrary()?,
            response_topic: u.arbitrary()?,
            correlation_data: Option::<Vec<u8>>::arbitrary(u)?.map(Bytes::from),
            user_properties: u.arbitrary()?,
            subscription_id: u.arbitrary()?,
            content_type: u.arbitrary()?,
        })
    }
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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

/// Body type for PUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Puback {
    pub pid: Pid,
    pub reason_code: PubackReasonCode,
    pub properties: PubackProperties,
}

impl Puback {
    pub fn new(pid: Pid, reason_code: PubackReasonCode) -> Self {
        Puback {
            pid,
            reason_code,
            properties: PubackProperties::default(),
        }
    }

    pub fn new_success(pid: Pid) -> Self {
        Self::new(pid, PubackReasonCode::Success)
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            (PubackReasonCode::Success, PubackProperties::default())
        } else if header.remaining_len == 3 {
            let reason_byte = read_u8_async(reader).await?;
            let reason_code = PubackReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            (reason_code, PubackProperties::default())
        } else {
            let reason_byte = read_u8_async(reader).await?;
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        if self.properties == PubackProperties::default() {
            if self.reason_code != PubackReasonCode::Success {
                write_u8(writer, self.reason_code as u8)?;
            }
        } else {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.properties == PubackProperties::default() {
            if self.reason_code == PubackReasonCode::Success {
                2
            } else {
                3
            }
        } else {
            3 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PubackProperties {
    pub reason_string: Option<Arc<str>>,
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// Body type for PUBREC packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Pubrec {
    pub pid: Pid,
    pub reason_code: PubrecReasonCode,
    pub properties: PubrecProperties,
}

impl Pubrec {
    pub fn new(pid: Pid, reason_code: PubrecReasonCode) -> Self {
        Pubrec {
            pid,
            reason_code,
            properties: PubrecProperties::default(),
        }
    }

    pub fn new_success(pid: Pid) -> Self {
        Self::new(pid, PubrecReasonCode::Success)
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            (PubrecReasonCode::Success, PubrecProperties::default())
        } else if header.remaining_len == 3 {
            let reason_byte = read_u8_async(reader).await?;
            let reason_code = PubrecReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            (reason_code, PubrecProperties::default())
        } else {
            let reason_byte = read_u8_async(reader).await?;
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        if self.properties == PubrecProperties::default() {
            if self.reason_code != PubrecReasonCode::Success {
                write_u8(writer, self.reason_code as u8)?;
            }
        } else {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.properties == PubrecProperties::default() {
            if self.reason_code == PubrecReasonCode::Success {
                2
            } else {
                3
            }
        } else {
            3 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBREC packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PubrecProperties {
    pub reason_string: Option<Arc<str>>,
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// Body type for PUBREL packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Pubrel {
    pub pid: Pid,
    pub reason_code: PubrelReasonCode,
    pub properties: PubrelProperties,
}

impl Pubrel {
    pub fn new(pid: Pid, reason_code: PubrelReasonCode) -> Self {
        Pubrel {
            pid,
            reason_code,
            properties: PubrelProperties::default(),
        }
    }

    pub fn new_success(pid: Pid) -> Self {
        Self::new(pid, PubrelReasonCode::Success)
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            (PubrelReasonCode::Success, PubrelProperties::default())
        } else if header.remaining_len == 3 {
            let reason_byte = read_u8_async(reader).await?;
            let reason_code = PubrelReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            (reason_code, PubrelProperties::default())
        } else {
            let reason_byte = read_u8_async(reader).await?;
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        if self.properties == PubrelProperties::default() {
            if self.reason_code != PubrelReasonCode::Success {
                write_u8(writer, self.reason_code as u8)?;
            }
        } else {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.properties == PubrelProperties::default() {
            if self.reason_code == PubrelReasonCode::Success {
                2
            } else {
                3
            }
        } else {
            3 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBREL packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PubrelProperties {
    pub reason_string: Option<Arc<str>>,
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// Body type for PUBCOMP packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Pubcomp {
    pub pid: Pid,
    pub reason_code: PubcompReasonCode,
    pub properties: PubcompProperties,
}

impl Pubcomp {
    pub fn new(pid: Pid, reason_code: PubcompReasonCode) -> Self {
        Pubcomp {
            pid,
            reason_code,
            properties: PubcompProperties::default(),
        }
    }

    pub fn new_success(pid: Pid) -> Self {
        Self::new(pid, PubcompReasonCode::Success)
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let (reason_code, properties) = if header.remaining_len == 2 {
            (PubcompReasonCode::Success, PubcompProperties::default())
        } else if header.remaining_len == 3 {
            let reason_byte = read_u8_async(reader).await?;
            let reason_code = PubcompReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            (reason_code, PubcompProperties::default())
        } else {
            let reason_byte = read_u8_async(reader).await?;
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        if self.properties == PubcompProperties::default() {
            if self.reason_code != PubcompReasonCode::Success {
                write_u8(writer, self.reason_code as u8)?;
            }
        } else {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.properties == PubcompProperties::default() {
            if self.reason_code == PubcompReasonCode::Success {
                2
            } else {
                3
            }
        } else {
            3 + self.properties.encode_len()
        }
    }
}

/// Property list for PUBCOMP packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct PubcompProperties {
    pub reason_string: Option<Arc<str>>,
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
