use std::fmt;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use super::ErrorV5;
use crate::{read_bytes, read_string, read_u16, read_u32, read_u8, TopicName};

/// [Property type](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901027)
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
pub enum PropertyType {
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

impl PropertyType {
    pub(crate) fn from_u8(value: u8) -> Result<PropertyType, ErrorV5> {
        let typ = match value {
            0x01 => PropertyType::PayloadFormatIndicator,
            0x02 => PropertyType::MessageExpiryInterval,
            0x03 => PropertyType::ContentType,
            0x08 => PropertyType::ResponseTopic,
            0x09 => PropertyType::CorrelationData,
            0x0B => PropertyType::SubscriptionIdentifier,
            0x11 => PropertyType::SessionExpiryInterval,
            0x12 => PropertyType::AssignedClientIdentifier,
            0x13 => PropertyType::ServerKeepAlive,
            0x15 => PropertyType::AuthenticationMethod,
            0x16 => PropertyType::AuthenticationData,
            0x17 => PropertyType::RequestProblemInformation,
            0x18 => PropertyType::WillDelayInterval,
            0x19 => PropertyType::RequestResponseInformation,
            0x1A => PropertyType::ResponseInformation,
            0x1C => PropertyType::ServerReference,
            0x1F => PropertyType::ReasonString,
            0x21 => PropertyType::ReceiveMaximum,
            0x22 => PropertyType::TopicAliasMaximum,
            0x23 => PropertyType::TopicAlias,
            0x24 => PropertyType::MaximumQoS,
            0x25 => PropertyType::RetainAvailable,
            0x26 => PropertyType::UserProperty,
            0x27 => PropertyType::MaximumPacketSize,
            0x28 => PropertyType::WildcardSubscriptionAvailable,
            0x29 => PropertyType::SubscriptionIdentifierAvailable,
            0x2A => PropertyType::SharedSubscriptionAvailable,
            _ => return Err(ErrorV5::InvalidPropertyType(value)),
        };
        Ok(typ)
    }
}

impl fmt::Display for PropertyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// A helper type to decode/encode property value
pub(crate) struct PropertyValue;

impl PropertyValue {
    #[inline]
    pub(crate) async fn decode_bool<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_type: PropertyType,
        target: &mut Option<bool>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_type));
        }
        let value = read_u8(reader).await?;
        if value > 1 {
            Err(ErrorV5::InvalidBytePropertyValue(property_type, value))
        } else {
            *target = Some(value == 1);
            Ok(())
        }
    }

    #[inline]
    pub(crate) async fn decode_u16<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_type: PropertyType,
        target: &mut Option<u16>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_type));
        }
        *target = Some(read_u16(reader).await?);
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_u32<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_type: PropertyType,
        target: &mut Option<u32>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_type));
        }
        *target = Some(read_u32(reader).await?);
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_string<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_type: PropertyType,
        target: &mut Option<Arc<String>>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_type));
        }
        *target = Some(Arc::new(read_string(reader).await?));
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_topic_name<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_type: PropertyType,
        target: &mut Option<TopicName>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_type));
        }
        let content = read_string(reader).await?;
        *target = Some(TopicName::try_from(content)?);
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_bytes<T: AsyncRead + Unpin>(
        reader: &mut T,
        property_type: PropertyType,
        target: &mut Option<Bytes>,
    ) -> Result<(), ErrorV5> {
        if target.is_some() {
            return Err(ErrorV5::DuplicatedProperty(property_type));
        }
        *target = Some(Bytes::from(read_bytes(reader).await?));
        Ok(())
    }

    #[inline]
    pub(crate) async fn decode_user_property<T: AsyncRead + Unpin>(
        reader: &mut T,
    ) -> Result<UserProperty, ErrorV5> {
        let name = read_string(reader).await?;
        let value = read_string(reader).await?;
        Ok(UserProperty {
            name: Arc::new(name),
            value: Arc::new(value),
        })
    }
}

/// User Property is a UTF-8 String Pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserProperty {
    /// The name of the user property.
    pub name: Arc<String>,
    /// The value of the user property.
    pub value: Arc<String>,
}

macro_rules! decode_property {
    (PayloadFormatIndicator, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_type,
            &mut $properties.payload_is_utf8,
        )
        .await?;
    };
    (MessageExpiryInterval, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_type,
            &mut $properties.message_expiry_interval,
        )
        .await?;
    };
    (ContentType, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_type,
            &mut $properties.content_type,
        )
        .await?;
    };
    (ResponseTopic, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_topic_name(
            $reader,
            $property_type,
            &mut $properties.response_topic,
        )
        .await?;
    };
    (CorrelationData, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bytes(
            $reader,
            $property_type,
            &mut $properties.correlation_data,
        )
        .await?;
    };
    (SubscriptionIdentifier, $properties:expr, $reader:expr, $property_type:expr) => {
        if $properties.subscription_id.is_some() {
            return Err(crate::v5::ErrorV5::DuplicatedProperty($property_type));
        }
        let (value, _bytes) = crate::decode_var_int($reader).await?;
        $properties.subscription_id = Some(value);
    };
    (SessionExpiryInterval, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_type,
            &mut $properties.session_expiry_interval,
        )
        .await?;
    };
    (AssignedClientIdentifier, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_type,
            &mut $properties.assigned_client_id,
        )
        .await?;
    };
    (ServerKeepAlive, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u16(
            $reader,
            $property_type,
            &mut $properties.server_keep_alive,
        )
        .await?;
    };
    (AuthenticationMethod, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_type,
            &mut $properties.auth_method,
        )
        .await?;
    };
    (AuthenticationData, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bytes($reader, $property_type, &mut $properties.auth_data)
            .await?;
    };
    (RequestProblemInformation, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_type,
            &mut $properties.request_problem_info,
        )
        .await?;
    };
    (WillDelayInterval, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_type,
            &mut $properties.delay_interval,
        )
        .await?;
    };
    (RequestResponseInformation, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_type,
            &mut $properties.request_response_info,
        )
        .await?;
    };
    (ResponseInformation, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_type,
            &mut $properties.response_info,
        )
        .await?;
    };
    (ServerReference, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_type,
            &mut $properties.server_reference,
        )
        .await?;
    };
    (ReasonString, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_string(
            $reader,
            $property_type,
            &mut $properties.reason_string,
        )
        .await?;
    };
    (ReceiveMaximum, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u16($reader, $property_type, &mut $properties.receive_max)
            .await?;
    };
    (TopicAliasMaximum, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u16(
            $reader,
            $property_type,
            &mut $properties.topic_alias_max,
        )
        .await?;
    };
    (TopicAlias, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u16($reader, $property_type, &mut $properties.topic_alias)
            .await?;
    };
    (MaximumQoS, $properties:expr, $reader:expr, $property_type:expr) => {
        if $properties.max_qos.is_some() {
            return Err(crate::v5::ErrorV5::DuplicatedProperty($property_type));
        }
        let value = crate::read_u8($reader).await?;
        if value > 1 {
            return Err(crate::v5::ErrorV5::InvalidBytePropertyValue(
                $property_type,
                value,
            ));
        } else {
            $properties.max_qos = Some(crate::QoS::from_u8(value)?);
        }
    };
    (RetainAvailable, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_type,
            &mut $properties.retain_available,
        )
        .await?;
    };
    (UserProperty, $properties:expr, $reader:expr, $property_type:expr) => {
        let user_property = crate::v5::PropertyValue::decode_user_property($reader).await?;
        $properties.user_properties.push(user_property);
    };
    (MaximumPacketSize, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_u32(
            $reader,
            $property_type,
            &mut $properties.max_packet_size,
        )
        .await?;
    };
    (SubscriptionIdentifierAvailable, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_type,
            &mut $properties.subscription_id_available,
        )
        .await?;
    };
    (SharedSubscriptionAvailable, $properties:expr, $reader:expr, $property_type:expr) => {
        crate::v5::PropertyValue::decode_bool(
            $reader,
            $property_type,
            &mut $properties.shared_subscription_available,
        )
        .await?;
    };
}

macro_rules! decode_properties {
    (LastWill, $properties:expr, $reader:expr, $($t:ident,)+) => {
        let (mut property_len, _bytes) = crate::decode_var_int($reader).await?;
        while property_len > 0 {
            let property_type = crate::v5::PropertyType::from_u8(crate::read_u8($reader).await?)?;
            match property_type {
                $(
                    crate::v5::PropertyType::$t => {
                        crate::v5::decode_property!($t, $properties, $reader, property_type);
                    }
                )+
                    _ => return Err(crate::v5::ErrorV5::InvalidWillProperty(property_type)),
            }
            property_len -= 1;
        }
    };
    ($packet_type:expr, $properties:expr, $reader:expr, $($t:ident,)+) => {
        let (mut property_len, _bytes) = crate::decode_var_int($reader).await?;
        while property_len > 0 {
            let property_type = crate::v5::PropertyType::from_u8(crate::read_u8($reader).await?)?;
            match property_type {
                $(
                    crate::v5::PropertyType::$t => {
                        crate::v5::decode_property!($t, $properties, $reader, property_type);
                    }
                )+
                    _ => return Err(crate::v5::ErrorV5::InvalidProperty(property_type, $packet_type)),
            }
            property_len -= 1;
        }
    };
}

pub(crate) use decode_properties;
pub(crate) use decode_property;

macro_rules! property_field {
    (PayloadFormatIndicator, $properties:expr) => {
        $properties.payload_is_utf8
    };
    (MessageExpiryInterval, $properties:expr) => {
        $properties.message_expiry_interval
    };
    (ContentType, $properties:expr) => {
        $properties.content_type
    };
    (ResponseTopic, $properties:expr) => {
        $properties.response_topic
    };
    (CorrelationData, $properties:expr) => {
        $properties.correlation_data
    };
    (SubscriptionIdentifier, $properties:expr) => {
        $properties.subscription_id
    };
    (SessionExpiryInterval, $properties:expr) => {
        $properties.session_expiry_interval
    };
    (AssignedClientIdentifier, $properties:expr) => {
        $properties.assigned_client_id
    };
    (ServerKeepAlive, $properties:expr) => {
        $properties.server_keep_alive
    };
    (AuthenticationMethod, $properties:expr) => {
        $properties.auth_method
    };
    (AuthenticationData, $properties:expr) => {
        $properties.auth_data
    };
    (RequestProblemInformation, $properties:expr) => {
        $properties.request_problem_info
    };
    (WillDelayInterval, $properties:expr) => {
        $properties.delay_interval
    };
    (RequestResponseInformation, $properties:expr) => {
        $properties.request_response_info
    };
    (ResponseInformation, $properties:expr) => {
        $properties.response_info
    };
    (ServerReference, $properties:expr) => {
        $properties.server_reference
    };
    (ReasonString, $properties:expr) => {
        $properties.reason_string
    };
    (ReceiveMaximum, $properties:expr) => {
        $properties.receive_max
    };
    (TopicAliasMaximum, $properties:expr) => {
        $properties.topic_alias_max
    };
    (TopicAlias, $properties:expr) => {
        $properties.topic_alias
    };
    (MaximumQoS, $properties:expr) => {
        $properties.max_qos
    };
    (RetainAvailable, $properties:expr) => {
        $properties.retain_available
    };
    (MaximumPacketSize, $properties:expr) => {
        $properties.max_packet_size
    };
    (SubscriptionIdentifierAvailable, $properties:expr) => {
        $properties.subscription_id_available
    };
    (SharedSubscriptionAvailable, $properties:expr) => {
        $properties.shared_subscription_available
    };
}

macro_rules! encode_property {
    (PayloadFormatIndicator, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.payload_is_utf8 {
            crate::write_u8(
                $writer,
                crate::v5::PropertyType::PayloadFormatIndicator as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (MessageExpiryInterval, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.message_expiry_interval {
            crate::write_u8(
                $writer,
                crate::v5::PropertyType::MessageExpiryInterval as u8,
            )?;
            crate::write_u32($writer, value)?;
        }
    };
    (ContentType, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.content_type.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyType::ContentType as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (ResponseTopic, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.response_topic.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyType::ResponseTopic as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (CorrelationData, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.correlation_data.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyType::CorrelationData as u8)?;
            crate::write_bytes($writer, value.as_ref())?;
        }
    };
    (SubscriptionIdentifier, $properties:expr, $writer: expr) => {
        $properties.subscription_id
    };
    (SessionExpiryInterval, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.session_expiry_interval {
            crate::write_u8(
                $writer,
                crate::v5::PropertyType::SessionExpiryInterval as u8,
            )?;
            crate::write_u32($writer, value)?;
        }
    };
    (AssignedClientIdentifier, $properties:expr, $writer: expr) => {
        $properties.assigned_client_id
    };
    (ServerKeepAlive, $properties:expr, $writer: expr) => {
        $properties.server_keep_alive
    };
    (AuthenticationMethod, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.auth_method.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyType::AuthenticationMethod as u8)?;
            crate::write_bytes($writer, value.as_bytes())?;
        }
    };
    (AuthenticationData, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.auth_data.as_ref() {
            crate::write_u8($writer, crate::v5::PropertyType::AuthenticationData as u8)?;
            crate::write_bytes($writer, value.as_ref())?;
        }
    };
    (RequestProblemInformation, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.request_problem_info {
            crate::write_u8(
                $writer,
                crate::v5::PropertyType::RequestProblemInformation as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (WillDelayInterval, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.delay_interval {
            crate::write_u8($writer, crate::v5::PropertyType::WillDelayInterval as u8)?;
            crate::write_u32($writer, value)?;
        }
    };
    (RequestResponseInformation, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.request_response_info {
            crate::write_u8(
                $writer,
                crate::v5::PropertyType::RequestResponseInformation as u8,
            )?;
            crate::write_u8($writer, u8::from(value))?;
        }
    };
    (ResponseInformation, $properties:expr, $writer: expr) => {
        $properties.response_info
    };
    (ServerReference, $properties:expr, $writer: expr) => {
        $properties.server_reference
    };
    (ReasonString, $properties:expr, $writer: expr) => {
        $properties.reason_string
    };
    (ReceiveMaximum, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.receive_max {
            crate::write_u8($writer, crate::v5::PropertyType::ReceiveMaximum as u8)?;
            crate::write_u16($writer, value)?;
        }
    };
    (TopicAliasMaximum, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.topic_alias_max {
            crate::write_u8($writer, crate::v5::PropertyType::TopicAliasMaximum as u8)?;
            crate::write_u16($writer, value)?;
        }
    };
    (TopicAlias, $properties:expr, $writer: expr) => {
        $properties.topic_alias
    };
    (MaximumQoS, $properties:expr, $writer: expr) => {
        $properties.max_qos
    };
    (RetainAvailable, $properties:expr, $writer: expr) => {
        $properties.retain_available
    };
    (MaximumPacketSize, $properties:expr, $writer: expr) => {
        if let Some(value) = $properties.max_packet_size {
            crate::write_u8($writer, crate::v5::PropertyType::MaximumPacketSize as u8)?;
            crate::write_u32($writer, value)?;
        }
    };
    (SubscriptionIdentifierAvailable, $properties:expr, $writer: expr) => {
        $properties.subscription_id_available
    };
    (SharedSubscriptionAvailable, $properties:expr, $writer: expr) => {
        $properties.shared_subscription_available
    };
}

macro_rules! encode_properties {
    ($properties:expr, $writer:expr, $($t:ident,)+) => {
        let mut property_len = $properties.user_properties.len();
        $(
            if crate::v5::property_field!($t, $properties).is_some() {
                property_len += 1;
            }
        )+

            crate::write_var_int($writer, property_len)?;

        $(
            crate::v5::encode_property!($t, $properties, $writer);
        ) +

            for UserProperty { name, value } in $properties.user_properties.iter() {
                crate::write_u8($writer, crate::v5::PropertyType::UserProperty as u8)?;
                crate::write_bytes($writer, name.as_bytes())?;
                crate::write_bytes($writer, value.as_bytes())?;
            }
    };
}

pub(crate) use encode_properties;
pub(crate) use encode_property;
pub(crate) use property_field;

macro_rules! encode_property_len {
    (PayloadFormatIndicator, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.payload_is_utf8.is_some() {
            $property_len += 1;
            $len += 1;
        }
    };
    (MessageExpiryInterval, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.message_expiry_interval.is_some() {
            $property_len += 1;
            $len += 4;
        }
    };
    (ContentType, $properties:expr, $len:expr, $property_len:expr) => {
        if let Some(value) = $properties.content_type.as_ref() {
            $property_len += 1;
            $len += 2 + value.len();
        }
    };
    (ResponseTopic, $properties:expr, $len:expr, $property_len:expr) => {
        if let Some(value) = $properties.response_topic.as_ref() {
            $property_len += 1;
            $len += 2 + value.len();
        }
    };
    (CorrelationData, $properties:expr, $len:expr, $property_len:expr) => {
        if let Some(value) = $properties.correlation_data.as_ref() {
            $property_len += 1;
            $len += 2 + value.len();
        }
    };
    (SubscriptionIdentifier, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.subscription_id
    };
    (SessionExpiryInterval, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.session_expiry_interval.is_some() {
            $property_len += 1;
            $len += 4;
        }
    };
    (AssignedClientIdentifier, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.assigned_client_id
    };
    (ServerKeepAlive, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.server_keep_alive
    };
    (AuthenticationMethod, $properties:expr, $len:expr, $property_len:expr) => {
        if let Some(value) = $properties.auth_method.as_ref() {
            $property_len += 1;
            $len += 2 + value.len();
        }
    };
    (AuthenticationData, $properties:expr, $len:expr, $property_len:expr) => {
        if let Some(value) = $properties.auth_data.as_ref() {
            $property_len += 1;
            $len += 2 + value.len();
        }
    };
    (RequestProblemInformation, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.request_problem_info.is_some() {
            $property_len += 1;
            $len += 1;
        }
    };
    (WillDelayInterval, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.delay_interval.is_some() {
            $property_len += 1;
            $len += 4;
        }
    };
    (RequestResponseInformation, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.request_response_info.is_some() {
            $property_len += 1;
            $len += 1;
        }
    };
    (ResponseInformation, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.response_info
    };
    (ServerReference, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.server_reference
    };
    (ReasonString, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.reason_string
    };
    (ReceiveMaximum, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.receive_max.is_some() {
            $property_len += 1;
            $len += 2;
        }
    };
    (TopicAliasMaximum, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.topic_alias_max.is_some() {
            $property_len += 1;
            $len += 2;
        }
    };
    (TopicAlias, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.topic_alias
    };
    (MaximumQoS, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.max_qos
    };
    (RetainAvailable, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.retain_available
    };
    (MaximumPacketSize, $properties:expr, $len:expr, $property_len:expr) => {
        if $properties.max_packet_size.is_some() {
            $property_len += 1;
            $len += 4;
        }
    };
    (SubscriptionIdentifierAvailable, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.subscription_id_available
    };
    (SharedSubscriptionAvailable, $properties:expr, $len:expr, $property_len:expr) => {
        $properties.shared_subscription_available
    };
}

macro_rules! encode_properties_len {
    ($properties:expr, $len:expr, $($t:ident,)+) => {
        let mut property_len = $properties.user_properties.len();
        $(
            crate::v5::encode_property_len!($t, $properties, $len, property_len);
        )+

            $len += var_int_len(property_len).expect("huge user properties");
        $len += property_len;
        $len += $properties
            .user_properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
    };
}

pub(crate) use encode_properties_len;
pub(crate) use encode_property_len;
