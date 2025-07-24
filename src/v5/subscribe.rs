use core::convert::TryFrom;

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::{
    decode_var_int_async, read_string_async, read_u16_async, read_u8_async, write_bytes, write_u16,
    write_u8, AsyncRead, Encodable, Error, Pid, QoS, SyncWrite, TopicFilter,
};

use super::{
    decode_properties, encode_properties, encode_properties_len, ErrorV5, Header, PacketType,
    PropertyId, PropertyValue, UserProperty, VarByteInt,
};

/// Body type for SUBSCRIBE packet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Subscribe {
    pub pid: Pid,
    pub properties: SubscribeProperties,
    pub topics: Vec<(TopicFilter, SubscriptionOptions)>,
}

impl Subscribe {
    pub fn new(pid: Pid, topics: Vec<(TopicFilter, SubscriptionOptions)>) -> Self {
        Subscribe {
            pid,
            properties: SubscribeProperties::default(),
            topics,
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let properties = SubscribeProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(2 + properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription.into());
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string_async(reader).await?)?;
            let options = {
                let opt_byte = read_u8_async(reader).await?;
                if opt_byte & 0b11000000 > 0 {
                    return Err(ErrorV5::InvalidSubscriptionOption(opt_byte));
                }
                let max_qos = QoS::from_u8(opt_byte & 0b11)
                    .map_err(|_| ErrorV5::InvalidSubscriptionOption(opt_byte))?;
                let no_local = opt_byte & 0b100 == 0b100;
                let retain_as_published = opt_byte & 0b1000 == 0b1000;
                let retain_handling = RetainHandling::from_u8((opt_byte & 0b110000) >> 4)
                    .ok_or(ErrorV5::InvalidSubscriptionOption(opt_byte))?;
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SubscribeProperties {
    pub subscription_id: Option<VarByteInt>,
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SubscriptionOptions {
    pub max_qos: QoS,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: RetainHandling,
}

impl SubscriptionOptions {
    pub fn new(max_qos: QoS) -> Self {
        SubscriptionOptions {
            max_qos,
            no_local: false,
            retain_as_published: true,
            retain_handling: RetainHandling::SendAtSubscribe,
        }
    }

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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum RetainHandling {
    SendAtSubscribe = 0,
    SendAtSubscribeIfNotExist = 1,
    DoNotSend = 2,
}

impl RetainHandling {
    pub fn from_u8(value: u8) -> Option<Self> {
        let opt = match value {
            0 => Self::SendAtSubscribe,
            1 => Self::SendAtSubscribeIfNotExist,
            2 => Self::DoNotSend,
            _ => return None,
        };
        Some(opt)
    }
}

/// Body type for SUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Suback {
    pub pid: Pid,
    pub properties: SubackProperties,
    pub topics: Vec<SubscribeReasonCode>,
}

impl Suback {
    pub fn new(pid: Pid, topics: Vec<SubscribeReasonCode>) -> Self {
        Suback {
            pid,
            properties: SubackProperties::default(),
            topics,
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let properties = SubackProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(2 + properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let value = read_u8_async(reader).await?;
            let code = SubscribeReasonCode::from_u8(value)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, value))?;
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SubackProperties {
    pub reason_string: Option<Arc<str>>,
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
    pub fn from_u8(value: u8) -> Option<Self> {
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
            _ => return None,
        };
        Some(code)
    }
}

/// Body type for UNSUBSCRIBE packet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Unsubscribe {
    pub pid: Pid,
    pub properties: UnsubscribeProperties,
    pub topics: Vec<TopicFilter>,
}

impl Unsubscribe {
    pub fn new(pid: Pid, topics: Vec<TopicFilter>) -> Self {
        Unsubscribe {
            pid,
            properties: Default::default(),
            topics,
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let (property_len, property_len_bytes) = decode_var_int_async(reader).await?;
        let mut properties = UnsubscribeProperties::default();
        let mut len = 0;
        while property_len as usize > len {
            let property_id = PropertyId::from_u8(read_u8_async(reader).await?)?;
            match property_id {
                PropertyId::UserProperty => {
                    let property = PropertyValue::decode_user_property(reader).await?;
                    len += 1 + 4 + property.name.len() + property.value.len();
                    properties.user_properties.push(property);
                }
                _ => return Err(ErrorV5::InvalidProperty(header.typ, property_id)),
            }
        }
        if property_len as usize != len {
            return Err(ErrorV5::InvalidPropertyLength(property_len));
        }
        remaining_len = remaining_len
            .checked_sub(2 + property_len_bytes + len)
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription.into());
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string_async(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(2 + topic_filter.len())
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        self.properties.encode(writer)?;
        for topic_filter in &self.topics {
            write_bytes(writer, topic_filter.as_bytes())?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 2;
        len += self.properties.encode_len();
        len += self
            .topics
            .iter()
            .map(|topic_filter| 2 + topic_filter.len())
            .sum::<usize>();
        len
    }
}

/// Property list for UNSUBSCRIBE packet.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct UnsubscribeProperties {
    pub user_properties: Vec<UserProperty>,
}

impl UnsubscribeProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = UnsubscribeProperties::default();
        decode_properties!(packet_type, properties, reader,);
        Ok(properties)
    }
}

impl Encodable for UnsubscribeProperties {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        encode_properties!(self, writer);
        Ok(())
    }
    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(self, len);
        len
    }
}

impl From<Vec<UserProperty>> for UnsubscribeProperties {
    fn from(user_properties: Vec<UserProperty>) -> UnsubscribeProperties {
        UnsubscribeProperties { user_properties }
    }
}

/// Body type for UNSUBACK packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Unsuback {
    pub pid: Pid,
    pub properties: UnsubackProperties,
    pub topics: Vec<UnsubscribeReasonCode>,
}

impl Unsuback {
    pub fn new(pid: Pid, topics: Vec<UnsubscribeReasonCode>) -> Self {
        Unsuback {
            pid,
            properties: UnsubackProperties::default(),
            topics,
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        let properties = UnsubackProperties::decode_async(reader, header.typ).await?;
        remaining_len = remaining_len
            .checked_sub(2 + properties.encode_len())
            .ok_or(Error::InvalidRemainingLength)?;
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let value = read_u8_async(reader).await?;
            let code = UnsubscribeReasonCode::from_u8(value)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, value))?;
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
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct UnsubackProperties {
    pub reason_string: Option<Arc<str>>,
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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => Self::Success,
            0x11 => Self::NoSubscriptionExisted,
            0x80 => Self::UnspecifiedError,
            0x83 => Self::ImplementationSpecificError,
            0x87 => Self::NotAuthorized,
            0x8F => Self::TopicFilterInvalid,
            0x91 => Self::PacketIdentifierInUse,
            _ => return None,
        };
        Some(code)
    }
}
