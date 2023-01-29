use std::convert::TryFrom;
use std::io;
use std::sync::Arc;

use futures_lite::io::AsyncRead;

use super::{
    decode_properties, encode_properties, encode_properties_len, ErrorV5, Header, PacketType,
    PropertyType, PropertyValue, UserProperty,
};
use crate::{
    decode_var_int, read_string, read_u16, read_u8, var_int_len, write_bytes, write_u16, write_u8,
    write_var_int, Encodable, Error, Pid, QoS, TopicFilter,
};

/// Payload type for SUBSCRIBE packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {
    pub pid: Pid,
    pub properties: SubscribeProperties,
    pub topics: Vec<(TopicFilter, SubscriptionOptions)>,
}

impl Subscribe {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len;
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let properties = SubscribeProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(2 + properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription.into());
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string(reader).await?)?;
            let options = {
                let opt_byte = read_u8(reader).await?;
                if opt_byte & 0b11000000 > 0 {
                    return Err(ErrorV5::InvalidSubscriptionOption);
                }
                let max_qos = QoS::from_u8(opt_byte & 0b11)
                    .map_err(|_| ErrorV5::InvalidSubscriptionOption)?;
                let no_local = opt_byte & 0b100 == 0b100;
                let retain_as_published = opt_byte & 0b1000 == 0b1000;
                let retain_handling = RetainHandling::from_u8((opt_byte & 0b110000) >> 4)?;
                SubscriptionOptions {
                    max_qos,
                    no_local,
                    retain_as_published,
                    retain_handling,
                }
            };
            remaining_len = remaining_len
                .checked_sub(3 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push((topic_filter, options));
        }
        Ok(Subscribe {
            pid,
            properties,
            topics,
        })
    }
}

impl Encodable for Subscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        self.properties.encode(writer)?;
        for (topic_filter, options) in &self.topics {
            write_bytes(writer, topic_filter.as_bytes())?;
            write_u8(writer, options.to_u8())?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self.properties.encode_len()
            + self
                .topics
                .iter()
                .map(|(filter, _)| 3 + filter.len())
                .sum::<usize>()
    }
}

/// Property list for SUBSCRIBE packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscribeProperties {
    pub subscription_id: Option<usize>,
    pub user_properties: Vec<UserProperty>,
}

impl SubscribeProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = SubscribeProperties::default();
        decode_properties!(packet_type, properties, reader, SubscriptionIdentifier,);
        Ok(properties)
    }
}

impl Encodable for SubscribeProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        encode_properties!(self, writer, SubscriptionIdentifier,);
        Ok(())
    }
    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(self, len, SubscriptionIdentifier,);
        len
    }
}

/// Subscription options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionOptions {
    pub max_qos: QoS,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: RetainHandling,
}

impl SubscriptionOptions {
    pub fn to_u8(&self) -> u8 {
        let mut byte = self.max_qos as u8;
        if self.no_local {
            byte |= 0b100;
        }
        if self.retain_as_published {
            byte |= 0b1000;
        }
        byte |= (self.retain_handling as u8) << 4;
        byte
    }
}

/// Retain handling type.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetainHandling {
    SendAtSubscribe = 0,
    SendAtSubscribeIfNotExist = 1,
    DoNotSend = 2,
}

impl RetainHandling {
    pub fn from_u8(value: u8) -> Result<Self, ErrorV5> {
        let opt = match value {
            0 => Self::SendAtSubscribe,
            1 => Self::SendAtSubscribeIfNotExist,
            2 => Self::DoNotSend,
            _ => return Err(ErrorV5::InvalidSubscriptionOption),
        };
        Ok(opt)
    }
}

/// Payload type for SUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suback {
    pub pid: Pid,
    pub properties: SubackProperties,
    pub topics: Vec<SubscribeReasonCode>,
}

impl Suback {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len;
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let properties = SubackProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(2 + properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let value = read_u8(reader).await?;
            let code = SubscribeReasonCode::from_u8(value)?;
            topics.push(code);
            remaining_len -= 1;
        }
        Ok(Suback {
            pid,
            properties,
            topics,
        })
    }
}

impl Encodable for Suback {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        self.properties.encode(writer)?;
        for reason_code in &self.topics {
            write_u8(writer, *reason_code as u8)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self.properties.encode_len() + self.topics.len()
    }
}

/// Property list for SUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubackProperties {
    pub reason_string: Option<Arc<String>>,
    pub user_properties: Vec<UserProperty>,
}

impl SubackProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = SubackProperties::default();
        decode_properties!(packet_type, properties, reader, ReasonString,);
        Ok(properties)
    }
}

impl Encodable for SubackProperties {
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

/// Reason code for SUBACK packet.
///
/// | Dec |  Hex | Reason Code name                       | Description                                                                                                        |
/// |-----|------|----------------------------------------|--------------------------------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Granted QoS 0                          | The subscription is accepted and the maximum QoS sent will be QoS 0. This might be a lower QoS than was requested. |
/// |   1 | 0x01 | Granted QoS 1                          | The subscription is accepted and the maximum QoS sent will be QoS 1. This might be a lower QoS than was requested. |
/// |   2 | 0x02 | Granted QoS 2                          | The subscription is accepted and any received QoS will be sent to this subscription.                               |
/// | 128 | 0x80 | Unspecified error                      | The subscription is not accepted and the Server either does not wish to reveal the reason                          |
/// |     |      |                                        | or none of the other Reason Codes apply.                                                                           |
/// | 131 | 0x83 | Implementation specific error          | The SUBSCRIBE is valid but the Server does not accept it.                                                          |
/// | 135 | 0x87 | Not authorized                         | The Client is not authorized to make this subscription.                                                            |
/// | 143 | 0x8F | Topic Filter invalid                   | The Topic Filter is correctly formed but is not allowed for this Client.                                           |
/// | 145 | 0x91 | Packet Identifier in use               | The specified Packet Identifier is already in use.                                                                 |
/// | 151 | 0x97 | Quota exceeded                         | An implementation or administrative imposed limit has been exceeded.                                               |
/// | 158 | 0x9E | Shared Subscriptions not supported     | The Server does not support Shared Subscriptions for this Client.                                                  |
/// | 161 | 0xA1 | Subscription Identifiers not supported | The Server does not support Subscription Identifiers; the subscription is not accepted.                            |
/// | 162 | 0xA2 | Wildcard Subscriptions not supported   | The Server does not support Wildcard Subscriptions; the subscription is not accepted.                              |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscribeReasonCode {
    GrantedQoS0 = 0x00,
    GrantedQoS1 = 0x01,
    GrantedQoS2 = 0x02,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicFilterInvalid = 0x8F,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    SharedSubscriptionNotSupported = 0x9E,
    SubscriptionIdentifiersNotSupported = 0xA1,
    WildcardSubscriptionsNotSupported = 0xA2,
}

impl SubscribeReasonCode {
    pub fn from_u8(value: u8) -> Result<Self, ErrorV5> {
        let code = match value {
            0x00 => Self::GrantedQoS0,
            0x01 => Self::GrantedQoS1,
            0x02 => Self::GrantedQoS2,
            0x80 => Self::UnspecifiedError,
            0x83 => Self::ImplementationSpecificError,
            0x87 => Self::NotAuthorized,
            0x8F => Self::TopicFilterInvalid,
            0x91 => Self::PacketIdentifierInUse,
            0x97 => Self::QuotaExceeded,
            0x9E => Self::SharedSubscriptionNotSupported,
            0xA1 => Self::SubscriptionIdentifiersNotSupported,
            0xA2 => Self::WildcardSubscriptionsNotSupported,
            _ => return Err(ErrorV5::InvalidReasonCode(value)),
        };
        Ok(code)
    }
}

/// Payload type for UNSUBSCRIBE packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsubscribe {
    pub pid: Pid,
    pub properties: Vec<UserProperty>,
    pub topics: Vec<TopicFilter>,
}

impl Unsubscribe {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len;
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let (mut property_len, mut properties_bytes) = decode_var_int(reader).await?;
        let mut properties = Vec::new();
        while property_len > 0 {
            let property_type = PropertyType::from_u8(read_u8(reader).await?)?;
            match property_type {
                PropertyType::UserProperty => {
                    let user_property = PropertyValue::decode_user_property(reader).await?;
                    properties_bytes += 4 + user_property.name.len() + user_property.value.len();
                    properties.push(user_property);
                }
                _ => return Err(ErrorV5::InvalidProperty(property_type, header.typ)),
            }
            property_len -= 1;
        }
        remaining_len = remaining_len
            .checked_sub(2 + properties_bytes)
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription.into());
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(3 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push(topic_filter);
        }
        Ok(Unsubscribe {
            pid,
            properties,
            topics,
        })
    }
}

impl Encodable for Unsubscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;

        write_var_int(writer, self.properties.len())?;
        for UserProperty { name, value } in &self.properties {
            crate::write_u8(writer, PropertyType::UserProperty as u8)?;
            crate::write_bytes(writer, name.as_bytes())?;
            crate::write_bytes(writer, value.as_bytes())?;
        }
        for topic_filter in &self.topics {
            write_bytes(writer, topic_filter.as_bytes())?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 2;
        len +=
            var_int_len(self.properties.len()).expect("user properties length exceed 268,435,455");
        len += self.properties.len();
        len += self
            .properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
        len += self
            .topics
            .iter()
            .map(|topic_filter| 2 + topic_filter.len())
            .sum::<usize>();
        len
    }
}

/// Payload type for UNSUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsuback {
    pub pid: Pid,
    pub properties: UnsubackProperties,
    pub topics: Vec<UnsubscribeReasonCode>,
}

impl Unsuback {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len;
        let pid = Pid::try_from(read_u16(reader).await?)?;
        let properties = UnsubackProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(2 + properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let value = read_u8(reader).await?;
            let code = UnsubscribeReasonCode::from_u8(value)?;
            topics.push(code);
            remaining_len -= 1;
        }
        Ok(Unsuback {
            pid,
            properties,
            topics,
        })
    }
}

impl Encodable for Unsuback {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        self.properties.encode(writer)?;
        for reason_code in &self.topics {
            write_u8(writer, *reason_code as u8)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self.properties.encode_len() + self.topics.len()
    }
}

/// Property list for UNSUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsubackProperties {
    pub reason_string: Option<Arc<String>>,
    pub user_properties: Vec<UserProperty>,
}

impl UnsubackProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = UnsubackProperties::default();
        decode_properties!(packet_type, properties, reader, ReasonString,);
        Ok(properties)
    }
}

impl Encodable for UnsubackProperties {
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

/// Reason code for UNSUBACK packet.
///
/// | Dec |  Hex | Reason Code name              | Description                                                                                   |
/// |-----|------|-------------------------------|-----------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Success                       | The subscription is deleted.                                                                  |
/// |  17 | 0x11 | No subscription existed       | No matching Topic Filter is being used by the Client.                                         |
/// | 128 | 0x80 | Unspecified error             | The unsubscribe could not be completed and                                                    |
/// |     |      |                               | the Server either does not wish to reveal the reason or none of the other Reason Codes apply. |
/// | 131 | 0x83 | Implementation specific error | The UNSUBSCRIBE is valid but the Server does not accept it.                                   |
/// | 135 | 0x87 | Not authorized                | The Client is not authorized to unsubscribe.                                                  |
/// | 143 | 0x8F | Topic Filter invalid          | The Topic Filter is correctly formed but is not allowed for this Client.                      |
/// | 145 | 0x91 | Packet Identifier in use      | The specified Packet Identifier is already in use.                                            |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsubscribeReasonCode {
    Success = 0x00,
    NoSubscriptionExisted = 0x11,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicFilterInvalid = 0x8F,
    PacketIdentifierInUse = 0x91,
}

impl UnsubscribeReasonCode {
    pub fn from_u8(value: u8) -> Result<Self, ErrorV5> {
        let code = match value {
            0x00 => Self::Success,
            0x11 => Self::NoSubscriptionExisted,
            0x80 => Self::UnspecifiedError,
            0x83 => Self::ImplementationSpecificError,
            0x87 => Self::NotAuthorized,
            0x8F => Self::TopicFilterInvalid,
            0x91 => Self::PacketIdentifierInUse,
            _ => return Err(ErrorV5::InvalidReasonCode(value)),
        };
        Ok(code)
    }
}
