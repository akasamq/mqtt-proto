use core::convert::TryFrom;

use alloc::sync::Arc;

use bytes::Bytes;

use crate::{
    read_bytes_async, read_string_async, read_u16_async, read_u32_async, read_u8_async, AsyncRead,
    Error, TopicName,
};

use super::ErrorV5;

/// [Property identifier](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901027)
///
/// | Dec |  Hex | Name (usage)                      | Type                  | Packet / Will Properties                        |
/// |-----|------|-----------------------------------|-----------------------|-------------------------------------------------|
/// |   1 | 0x01 | Payload Format Indicator          | Byte                  | PUBLISH, Will Properties                        |
/// |   2 | 0x02 | Message Expiry Interval           | Four Byte Integer     | PUBLISH, Will Properties                        |
/// |   3 | 0x03 | Content Type                      | UTF-8 Encoded String  | PUBLISH, Will Properties                        |
/// |   8 | 0x08 | Response Topic                    | UTF-8 Encoded String  | PUBLISH, Will Properties                        |
/// |   9 | 0x09 | Correlation Data                  | Binary Data           | PUBLISH, Will Properties                        |
/// |  11 | 0x0B | Subscription Identifier           | Variable Byte Integer | PUBLISH, SUBSCRIBE                              |
/// |  17 | 0x11 | Session Expiry Interval           | Four Byte Integer     | CONNECT, CONNACK, DISCONNECT                    |
/// |  18 | 0x12 | Assigned Client Identifier        | UTF-8 Encoded String  | CONNACK                                         |
/// |  19 | 0x13 | Server Keep Alive                 | Two Byte Integer      | CONNACK                                         |
/// |  21 | 0x15 | Authentication Method             | UTF-8 Encoded String  | CONNECT, CONNACK, AUTH                          |
/// |  22 | 0x16 | Authentication Data               | Binary Data           | CONNECT, CONNACK, AUTH                          |
/// |  23 | 0x17 | Request Problem Information       | Byte                  | CONNECT                                         |
/// |  24 | 0x18 | Will Delay Interval               | Four Byte Integer     | Will Properties                                 |
/// |  25 | 0x19 | Request Response Information      | Byte                  | CONNECT                                         |
/// |  26 | 0x1A | Response Information              | UTF-8 Encoded String  | CONNACK                                         |
/// |  28 | 0x1C | Server Reference                  | UTF-8 Encoded String  | CONNACK, DISCONNECT                             |
/// |  31 | 0x1F | Reason String                     | UTF-8 Encoded String  | CONNACK, PUBACK, PUBREC, PUBREL, PUBCOMP,       |
/// |     |      |                                   |                       | SUBACK, UNSUBACK, DISCONNECT, AUTH              |
/// |  33 | 0x21 | Receive Maximum                   | Two Byte Integer      | CONNECT, CONNACK                                |
/// |  34 | 0x22 | Topic Alias Maximum               | Two Byte Integer      | CONNECT, CONNACK                                |
/// |  35 | 0x23 | Topic Alias                       | Two Byte Integer      | PUBLISH                                         |
/// |  36 | 0x24 | Maximum QoS                       | Byte                  | CONNACK                                         |
/// |  37 | 0x25 | Retain Available                  | Byte                  | CONNACK                                         |
/// |  38 | 0x26 | User Property                     | UTF-8 String Pair     | CONNECT, CONNACK, PUBLISH, Will Properties,     |
/// |     |      |                                   |                       | PUBACK, PUBREC, PUBREL, PUBCOMP, SUBSCRIBE,     |
/// |     |      |                                   |                       | SUBACK, UNSUBSCRIBE, UNSUBACK, DISCONNECT, AUTH |
/// |  39 | 0x27 | Maximum Packet Size               | Four Byte Integer     | CONNECT, CONNACK                                |
/// |  40 | 0x28 | Wildcard Subscription Available   | Byte                  | CONNACK                                         |
/// |  41 | 0x29 | Subscription Identifier Available | Byte                  | CONNACK                                         |
/// |  42 | 0x2A | Shared Subscription Available     | Byte                  | CONNACK                                         |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyId {
    PayloadFormatIndicator = 0x01,
    MessageExpiryInterval = 0x02,
    ContentType = 0x03,
    ResponseTopic = 0x08,
    CorrelationData = 0x09,
    SubscriptionIdentifier = 0x0B,
    SessionExpiryInterval = 0x11,
    AssignedClientIdentifier = 0x12,
    ServerKeepAlive = 0x13,
    AuthenticationMethod = 0x15,
    AuthenticationData = 0x16,
    RequestProblemInformation = 0x17,
    WillDelayInterval = 0x18,
    RequestResponseInformation = 0x19,
    ResponseInformation = 0x1A,
    ServerReference = 0x1C,
    ReasonString = 0x1F,
    ReceiveMaximum = 0x21,
    TopicAliasMaximum = 0x22,
    TopicAlias = 0x23,
    MaximumQoS = 0x24,
    RetainAvailable = 0x25,
    UserProperty = 0x26,
    MaximumPacketSize = 0x27,
    WildcardSubscriptionAvailable = 0x28,
    SubscriptionIdentifierAvailable = 0x29,
    SharedSubscriptionAvailable = 0x2A,
}

impl PropertyId {
    pub fn from_u8(value: u8) -> Result<Self, ErrorV5> {
        let typ = match value {
            0x01 => Self::PayloadFormatIndicator,
            0x02 => Self::MessageExpiryInterval,
            0x03 => Self::ContentType,
            0x08 => Self::ResponseTopic,
            0x09 => Self::CorrelationData,
            0x0B => Self::SubscriptionIdentifier,
            0x11 => Self::SessionExpiryInterval,
            0x12 => Self::AssignedClientIdentifier,
            0x13 => Self::ServerKeepAlive,
            0x15 => Self::AuthenticationMethod,
            0x16 => Self::AuthenticationData,
            0x17 => Self::RequestProblemInformation,
            0x18 => Self::WillDelayInterval,
            0x19 => Self::RequestResponseInformation,
            0x1A => Self::ResponseInformation,
            0x1C => Self::ServerReference,
            0x1F => Self::ReasonString,
            0x21 => Self::ReceiveMaximum,
            0x22 => Self::TopicAliasMaximum,
            0x23 => Self::TopicAlias,
            0x24 => Self::MaximumQoS,
            0x25 => Self::RetainAvailable,
            0x26 => Self::UserProperty,
            0x27 => Self::MaximumPacketSize,
            0x28 => Self::WildcardSubscriptionAvailable,
            0x29 => Self::SubscriptionIdentifierAvailable,
            0x2A => Self::SharedSubscriptionAvailable,
            _ => return Err(ErrorV5::InvalidPropertyId(value)),
        };
        Ok(typ)
    }
}

impl core::fmt::Display for PropertyId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

// A helper type to decode/encode property value
pub(crate) struct PropertyValue;

impl PropertyValue {
    #[inline]
    pub(crate) async fn decode_bool<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_id: PropertyId,
        target: &mut Option<bool>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_id));
        }
        let value = read_u8_async(reader).await?;
        if value > 1 {
            Err(ErrorV5::InvalidByteProperty(property_id, value))
        } else {
            *target = Some(value == 1);
            Ok(())
        }
    }

    #[inline]
    pub(crate) async fn decode_u16<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_id: PropertyId,
        target: &mut Option<u16>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_id));
        }
        *target = Some(read_u16_async(reader).await?);
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_u32<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_id: PropertyId,
        target: &mut Option<u32>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_id));
        }
        *target = Some(read_u32_async(reader).await?);
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_string<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_id: PropertyId,
        target: &mut Option<Arc<str>>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_id));
        }
        *target = Some(read_string_async(reader).await?);
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_topic_name<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_id: PropertyId,
        target: &mut Option<TopicName>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_id));
        }
        let content = read_string_async(reader).await?;
        *target = Some(TopicName::try_from(content)?);
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_bytes<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_id: PropertyId,
        target: &mut Option<Bytes>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_id));
        }
        *target = Some(Bytes::from(read_bytes_async(reader).await?));
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_user_property<T: AsyncRead + Unpin>(
        reader: &mut T,
    ) -> Result<UserProperty, ErrorV5> {
        let name = read_string_async(reader).await?;
        let value = read_string_async(reader).await?;
        Ok(UserProperty { name, value })
    }
}

/// User Property is a UTF-8 String Pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct UserProperty {
    /// The name of the user property.
    pub name: Arc<str>,
    /// The value of the user property.
    pub value: Arc<str>,
}

/// Variable Byte Integer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct VarByteInt(u32);

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for VarByteInt {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let value: u32 = u.arbitrary()?;
        Ok(VarByteInt(value % 268435456))
    }
}

impl VarByteInt {
    pub fn value(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for VarByteInt {
    type Error = ErrorV5;
    fn try_from(value: u32) -> Result<Self, ErrorV5> {
        if value < 268435456 {
            Ok(VarByteInt(value))
        } else {
            Err(Error::InvalidVarByteInt.into())
        }
    }
}

macro_rules! decode_property {
    (PayloadFormatIndicator, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_id,
            &mut $properties.payload_is_utf8,
        )
        .await?;
    };
    (MessageExpiryInterval, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_id,
            &mut $properties.message_expiry_interval,
        )
        .await?;
    };
    (ContentType, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_id,
            &mut $properties.content_type,
        )
        .await?;
    };
    (ResponseTopic, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_topic_name(
            $reader,
            $property_id,
            &mut $properties.response_topic,
        )
        .await
        .map_err(|err| match err {
            crate::v5::ErrorV5::Common(crate::Error::InvalidTopicName(_)) => {
                crate::v5::ErrorV5::InvalidResponseTopic
            }
            err => err,
        })?;
    };
    (CorrelationData, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bytes(
            $reader,
            $property_id,
            &mut $properties.correlation_data,
        )
        .await?;
    };
    (SubscriptionIdentifier, $properties:expr, $reader:expr, $property_id:expr) => {
        if $properties.subscription_id.is_some() {
            return Err(crate::v5::ErrorV5::DuplicatedProperty($property_id));
        }
        let (value, _bytes) = crate::decode_var_int_async($reader).await?;
        $properties.subscription_id = Some(crate::v5::VarByteInt::try_from(value)?);
    };
    (SessionExpiryInterval, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_id,
            &mut $properties.session_expiry_interval,
        )
        .await?;
    };
    (AssignedClientIdentifier, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_id,
            &mut $properties.assigned_client_id,
        )
        .await?;
    };
    (ServerKeepAlive, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u16(
            $reader,
            $property_id,
            &mut $properties.server_keep_alive,
        )
        .await?;
    };
    (AuthenticationMethod, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_id,
            &mut $properties.auth_method,
        )
        .await?;
    };
    (AuthenticationData, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bytes($reader, $property_id, &mut $properties.auth_data)
            .await?;
    };
    (RequestProblemInformation, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_id,
            &mut $properties.request_problem_info,
        )
        .await?;
    };
    (WillDelayInterval, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_id,
            &mut $properties.delay_interval,
        )
        .await?;
    };
    (RequestResponseInformation, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_id,
            &mut $properties.request_response_info,
        )
        .await?;
    };
    (ResponseInformation, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_id,
            &mut $properties.response_info,
        )
        .await?;
    };
    (ServerReference, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_id,
            &mut $properties.server_reference,
        )
        .await?;
    };
    (ReasonString, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_id,
            &mut $properties.reason_string,
        )
        .await?;
    };
    (ReceiveMaximum, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u16($reader, $property_id, &mut $properties.receive_max)
            .await?;
    };
    (TopicAliasMaximum, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u16(
            $reader,
            $property_id,
            &mut $properties.topic_alias_max,
        )
        .await?;
    };
    (TopicAlias, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u16($reader, $property_id, &mut $properties.topic_alias)
            .await?;
    };
    (MaximumQoS, $properties:expr, $reader:expr, $property_id:expr) => {
        if $properties.max_qos.is_some() {
            return Err(crate::v5::ErrorV5::DuplicatedProperty($property_id));
        }
        let value = crate::read_u8_async($reader).await?;
        if value > 1 {
            return Err(crate::v5::ErrorV5::InvalidByteProperty($property_id, value));
        } else {
            $properties.max_qos = Some(crate::QoS::from_u8(value).expect("0/1 qos"));
        }
    };
    (RetainAvailable, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_id,
            &mut $properties.retain_available,
        )
        .await?;
    };
    (UserProperty, $properties:expr, $reader:expr, $property_id:expr) => {
        let user_property = crate::v5::PropertyValue::decode_user_property($reader).await?;
        $properties.user_properties.push(user_property);
    };
    (MaximumPacketSize, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_id,
            &mut $properties.max_packet_size,
        )
        .await?;
    };
    (WildcardSubscriptionAvailable, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_id,
            &mut $properties.wildcard_subscription_available,
        )
        .await?;
    };
    (SubscriptionIdentifierAvailable, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_id,
            &mut $properties.subscription_id_available,
        )
        .await?;
    };
    (SharedSubscriptionAvailable, $properties:expr, $reader:expr, $property_id:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_id,
            &mut $properties.shared_subscription_available,
        )
        .await?;
    };
}

macro_rules! decode_properties {
    (LastWill, $properties:expr, $reader:expr, $($t:ident,)*) => {
        let (property_len, _bytes) = crate::decode_var_int_async($reader).await?;
        let mut len = 0;
        while property_len as usize > len {
            let property_id = crate::v5::PropertyId::from_u8(crate::read_u8_async($reader).await?)?;
            match property_id {
                $(
                    crate::v5::PropertyId::$t => {
                        crate::v5::decode_property!($t, $properties, $reader, property_id);
                        crate::v5::encode_property_len!($t, $properties, len);
                    }
                )*
                    crate::v5::PropertyId::UserProperty => {
                        crate::v5::decode_property!(UserProperty, $properties, $reader, property_id);
                        let last = $properties.user_properties.last().expect("user property exists");
                        len += 1 + 4 + last.name.len() + last.value.len();
                    }
                    _ => return Err(crate::v5::ErrorV5::InvalidWillProperty(property_id)),
            }
        }
        if property_len as usize != len {
            return Err(crate::v5::ErrorV5::InvalidPropertyLength(property_len));
        }
    };
    ($packet_type:expr, $properties:expr, $reader:expr, $($t:ident,)*) => {
        let (property_len, _bytes) = crate::decode_var_int_async($reader).await?;
        let mut len = 0;
        while property_len as usize > len {
            let property_id = crate::v5::PropertyId::from_u8(crate::read_u8_async($reader).await?)?;
            match property_id {
                $(
                    crate::v5::PropertyId::$t => {
                        crate::v5::decode_property!($t, $properties, $reader, property_id);
                        crate::v5::encode_property_len!($t, $properties, len);
                    }
                )*
                    crate::v5::PropertyId::UserProperty => {
                        crate::v5::decode_property!(UserProperty, $properties, $reader, property_id);
                        let last = $properties.user_properties.last().expect("user property exists");
                        len += 1 + 4 + last.name.len() + last.value.len();
                    }
                _ => return Err(crate::v5::ErrorV5::InvalidProperty($packet_type, property_id)),
            }
        }
        if property_len as usize != len {
            return Err(crate::v5::ErrorV5::InvalidPropertyLength(property_len));
        }
    };
}

pub(crate) use decode_properties;
pub(crate) use decode_property;

macro_rules! encode_property {
    (PayloadFormatIndicator, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.payload_is_utf8 {
            crate::write_u8($writer, crate::v5::PropertyId::PayloadFormatIndicator as u8)?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (MessageExpiryInterval, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.message_expiry_interval {
            crate::write_u8($writer, crate::v5::PropertyId::MessageExpiryInterval as u8)?;
            crate::write_u32($writer, value)?;
        }
    };
    (ContentType, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.content_type.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::ContentType as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (ResponseTopic, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.response_topic.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::ResponseTopic as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (CorrelationData, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.correlation_data.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::CorrelationData as u8)?;
            crate::write_bytes($writer, value.as_ref())?;
        }
    };
    (SubscriptionIdentifier, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.subscription_id {
            crate::write_u8($writer, crate::v5::PropertyId::SubscriptionIdentifier as u8)?;
            crate::write_var_int($writer, value.value() as usize)?;
        }
    };
    (SessionExpiryInterval, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.session_expiry_interval {
            crate::write_u8($writer, crate::v5::PropertyId::SessionExpiryInterval as u8)?;
            crate::write_u32($writer, value)?;
        }
    };
    (AssignedClientIdentifier, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.assigned_client_id.as_ref() {
            crate::write_u8(
                $writer,
                crate::v5::PropertyId::AssignedClientIdentifier as u8,
            )?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (ServerKeepAlive, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.server_keep_alive {
            crate::write_u8($writer, crate::v5::PropertyId::ServerKeepAlive as u8)?;
            crate::write_u16($writer, value)?;
        }
    };
    (AuthenticationMethod, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.auth_method.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::AuthenticationMethod as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (AuthenticationData, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.auth_data.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::AuthenticationData as u8)?;
            crate::write_bytes($writer, value.as_ref())?;
        }
    };
    (RequestProblemInformation, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.request_problem_info {
            crate::write_u8(
                $writer,
                crate::v5::PropertyId::RequestProblemInformation as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (WillDelayInterval, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.delay_interval {
            crate::write_u8($writer, crate::v5::PropertyId::WillDelayInterval as u8)?;
            crate::write_u32($writer, value)?;
        }
    };
    (RequestResponseInformation, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.request_response_info {
            crate::write_u8(
                $writer,
                crate::v5::PropertyId::RequestResponseInformation as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (ResponseInformation, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.response_info.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::ResponseInformation as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (ServerReference, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.server_reference.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::ServerReference as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (ReasonString, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.reason_string.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyId::ReasonString as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (ReceiveMaximum, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.receive_max {
            crate::write_u8($writer, crate::v5::PropertyId::ReceiveMaximum as u8)?;
            crate::write_u16($writer, value)?;
        }
    };
    (TopicAliasMaximum, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.topic_alias_max {
            crate::write_u8($writer, crate::v5::PropertyId::TopicAliasMaximum as u8)?;
            crate::write_u16($writer, value)?;
        }
    };
    (TopicAlias, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.topic_alias {
            crate::write_u8($writer, crate::v5::PropertyId::TopicAlias as u8)?;
            crate::write_u16($writer, value)?;
        }
    };
    (MaximumQoS, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.max_qos {
            crate::write_u8($writer, crate::v5::PropertyId::MaximumQoS as u8)?;
            crate::write_u8($writer, value as u8)?;
        }
    };
    (RetainAvailable, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.retain_available {
            crate::write_u8($writer, crate::v5::PropertyId::RetainAvailable as u8)?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (MaximumPacketSize, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.max_packet_size {
            crate::write_u8($writer, crate::v5::PropertyId::MaximumPacketSize as u8)?;
            crate::write_u32($writer, value)?;
        }
    };
    (WildcardSubscriptionAvailable, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.wildcard_subscription_available {
            crate::write_u8(
                $writer,
                crate::v5::PropertyId::WildcardSubscriptionAvailable as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (SubscriptionIdentifierAvailable, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.subscription_id_available {
            crate::write_u8(
                $writer,
                crate::v5::PropertyId::SubscriptionIdentifierAvailable as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (SharedSubscriptionAvailable, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.shared_subscription_available {
            crate::write_u8(
                $writer,
                crate::v5::PropertyId::SharedSubscriptionAvailable as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
}

macro_rules! encode_properties {
    ($properties:expr, $writer:expr) => {
        let property_len = $properties.user_properties.len() + $properties
            .user_properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
        crate::write_var_int($writer, property_len)?;
        for UserProperty { name, value } in $properties.user_properties.iter() {
            crate::write_u8($writer, crate::v5::PropertyId::UserProperty as u8)?;
            crate::write_bytes($writer, name.as_bytes())?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    ($properties:expr, $writer:expr, $($t:ident,)+) => {
        let mut property_len = $properties.user_properties.len() + $properties
            .user_properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
        $(
            crate::v5::encode_property_len!($t, $properties, property_len);
        )+

            crate::write_var_int($writer, property_len)?;
        $(
            crate::v5::encode_property!($t, $properties, $writer);
        )*

            for UserProperty { name, value } in $properties.user_properties.iter() {
                crate::write_u8($writer, crate::v5::PropertyId::UserProperty as u8)?;
                crate::write_bytes($writer, name.as_bytes())?;
                crate::write_bytes($writer, value.as_bytes())?;
            }
    };
}

pub(crate) use encode_properties;
pub(crate) use encode_property;

macro_rules! encode_property_len {
    (PayloadFormatIndicator, $properties:expr, $property_len:expr) => {
        if $properties.payload_is_utf8.is_some() {
            $property_len += 1 + 1;
        }
    };
    (MessageExpiryInterval, $properties:expr, $property_len:expr) => {
        if $properties.message_expiry_interval.is_some() {
            $property_len += 1 + 4;
        }
    };
    (ContentType, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.content_type.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (ResponseTopic, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.response_topic.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (CorrelationData, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.correlation_data.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (SubscriptionIdentifier, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.subscription_id {
            $property_len += 1 + crate::var_int_len(value.value() as usize)
                .expect("subscription id exceed 268,435,455");
        }
    };
    (SessionExpiryInterval, $properties:expr, $property_len:expr) => {
        if $properties.session_expiry_interval.is_some() {
            $property_len += 1 + 4;
        }
    };
    (AssignedClientIdentifier, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.assigned_client_id.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (ServerKeepAlive, $properties:expr, $property_len:expr) => {
        if $properties.server_keep_alive.is_some() {
            $property_len += 1 + 2;
        }
    };
    (AuthenticationMethod, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.auth_method.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (AuthenticationData, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.auth_data.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (RequestProblemInformation, $properties:expr, $property_len:expr) => {
        if $properties.request_problem_info.is_some() {
            $property_len += 1 + 1;
        }
    };
    (WillDelayInterval, $properties:expr, $property_len:expr) => {
        if $properties.delay_interval.is_some() {
            $property_len += 1 + 4;
        }
    };
    (RequestResponseInformation, $properties:expr, $property_len:expr) => {
        if $properties.request_response_info.is_some() {
            $property_len += 1 + 1;
        }
    };
    (ResponseInformation, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.response_info.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (ServerReference, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.server_reference.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (ReasonString, $properties:expr, $property_len:expr) => {
        if let Some(value) = $properties.reason_string.as_ref() {
            $property_len += 1 + 2 + value.len();
        }
    };
    (ReceiveMaximum, $properties:expr, $property_len:expr) => {
        if $properties.receive_max.is_some() {
            $property_len += 1 + 2;
        }
    };
    (TopicAliasMaximum, $properties:expr, $property_len:expr) => {
        if $properties.topic_alias_max.is_some() {
            $property_len += 1 + 2;
        }
    };
    (TopicAlias, $properties:expr, $property_len:expr) => {
        if $properties.topic_alias.is_some() {
            $property_len += 1 + 2;
        }
    };
    (MaximumQoS, $properties:expr, $property_len:expr) => {
        if $properties.max_qos.is_some() {
            $property_len += 1 + 1;
        }
    };
    (RetainAvailable, $properties:expr, $property_len:expr) => {
        if $properties.retain_available.is_some() {
            $property_len += 1 + 1;
        }
    };
    (MaximumPacketSize, $properties:expr, $property_len:expr) => {
        if $properties.max_packet_size.is_some() {
            $property_len += 1 + 4;
        }
    };
    (WildcardSubscriptionAvailable, $properties:expr, $property_len:expr) => {
        if $properties.wildcard_subscription_available.is_some() {
            $property_len += 1 + 1;
        }
    };
    (SubscriptionIdentifierAvailable, $properties:expr, $property_len:expr) => {
        if $properties.subscription_id_available.is_some() {
            $property_len += 1 + 1;
        }
    };
    (SharedSubscriptionAvailable, $properties:expr, $property_len:expr) => {
        if $properties.shared_subscription_available.is_some() {
            $property_len += 1 + 1;
        }
    };
}

macro_rules! encode_properties_len {
    ($properties:expr, $len:expr) => {
        // Every properties have user property
        let property_len: usize = $properties.user_properties.len() + $properties
            .user_properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
        $len += property_len + crate::var_int_len(property_len).expect("total properties length exceed 268,435,455");
    };
    ($properties:expr, $len:expr, $($t:ident,)+) => {
        // Every properties have user property
        let mut property_len: usize = $properties.user_properties.len() + $properties
            .user_properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
        $(
            crate::v5::encode_property_len!($t, $properties, property_len);
        )+

            $len += property_len + crate::var_int_len(property_len).expect("total properties length exceed 268,435,455");
    };
}

pub(crate) use encode_properties_len;
pub(crate) use encode_property_len;
