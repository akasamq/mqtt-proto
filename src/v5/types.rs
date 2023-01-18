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
            Err(ErrorV5::InvalidPayloadFormat(value))
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
