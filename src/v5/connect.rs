use core::convert::TryFrom;

use alloc::sync::Arc;
use alloc::vec::Vec;

use bytes::Bytes;
use simdutf8::basic::from_utf8;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;

use crate::{
    read_bytes_async, read_string_async, read_u16_async, read_u8_async, write_bytes, write_u16,
    write_u8, AsyncRead, ClientId, Encodable, Error, Protocol, QoS, SyncWrite, ToError, TopicName,
    Username,
};

use super::{
    decode_properties, encode_properties, encode_properties_len, ErrorV5, Header, PacketType,
    UserProperty,
};

/// Body type of CONNECT packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    /// The [protocol version](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901036).
    pub protocol: Protocol,

    /// [Clean start] flag. This value specifies whether the Connection starts a new Session or is a continuation of an existing Session.
    ///
    /// [Clean start]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901039
    pub clean_start: bool,

    /// The [keep alive]. A time interval measured in seconds. It is the
    /// maximum time interval that is permitted to elapse between the point at
    /// which the Client finishes transmitting one MQTT Control Packet and the
    /// point it starts sending the next.
    ///
    /// [keep alive]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901045
    pub keep_alive: u16,

    /// Properties
    pub properties: ConnectProperties,

    /// The [client identifier] (ClientID).
    ///
    /// [client identifier]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901059
    pub client_id: ClientId,

    /// The [will] message.
    ///
    /// [will]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901060
    pub last_will: Option<LastWill>,

    /// The [user name](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901071).
    pub username: Option<Username>,

    /// The [password](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901072).
    pub password: Option<Bytes>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Connect {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Connect {
            protocol: u.arbitrary()?,
            clean_start: u.arbitrary()?,
            keep_alive: u.arbitrary()?,
            properties: u.arbitrary()?,
            client_id: u.arbitrary()?,
            last_will: u.arbitrary()?,
            username: u.arbitrary()?,
            password: Option::<Vec<u8>>::arbitrary(u)?.map(Bytes::from),
        })
    }
}

impl Connect {
    pub fn new(client_id: ClientId, keep_alive: u16) -> Self {
        Connect {
            protocol: Protocol::V500,
            clean_start: true,
            keep_alive,
            properties: ConnectProperties::default(),
            client_id,
            last_will: None,
            username: None,
            password: None,
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let protocol = Protocol::decode_async(reader).await?;
        Self::decode_with_protocol(reader, header, protocol).await
    }

    #[inline]
    pub async fn decode_with_protocol<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
        protocol: Protocol,
    ) -> Result<Self, ErrorV5> {
        if protocol != Protocol::V500 {
            return Err(Error::UnexpectedProtocol(protocol).into());
        }
        let connect_flags: u8 = read_u8_async(reader).await?;
        if connect_flags & 1 != 0 {
            return Err(Error::InvalidConnectFlags(connect_flags).into());
        }
        let keep_alive = read_u16_async(reader).await?;

        // FIXME: check remaining length

        let properties = ConnectProperties::decode_async(reader, header.typ).await?;
        let client_id = read_string_async(reader).await?;
        let last_will = if connect_flags & 0b100 != 0 {
            let qos = QoS::from_u8((connect_flags & 0b11000) >> 3)?;
            let retain = (connect_flags & 0b00100000) != 0;
            Some(LastWill::decode_async(reader, qos, retain).await?)
        } else if connect_flags & 0b11000 != 0 {
            return Err(Error::InvalidConnectFlags(connect_flags).into());
        } else {
            None
        };
        let username = if connect_flags & 0b10000000 != 0 {
            Some(read_string_async(reader).await?)
        } else {
            None
        };
        let password = if connect_flags & 0b01000000 != 0 {
            Some(Bytes::from(read_bytes_async(reader).await?))
        } else {
            None
        };
        let clean_start = (connect_flags & 0b10) != 0;

        Ok(Connect {
            protocol,
            clean_start,
            properties,
            keep_alive,
            client_id,
            last_will,
            username,
            password,
        })
    }
}

impl Encodable for Connect {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        let mut connect_flags: u8 = 0b00000000;
        if self.clean_start {
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
        self.properties.encode(writer)?;
        write_bytes(writer, self.client_id.as_bytes())?;
        if let Some(last_will) = self.last_will.as_ref() {
            last_will.encode(writer)?;
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
        let mut len = self.protocol.encode_len();
        // flags + keep-alive
        len += 1 + 2;
        // properties
        len += self.properties.encode_len();
        // client identifier
        len += 2 + self.client_id.len();
        if let Some(last_will) = self.last_will.as_ref() {
            len += last_will.encode_len();
        }
        if let Some(username) = self.username.as_ref() {
            len += 2 + username.len();
        }
        if let Some(password) = self.password.as_ref() {
            len += 2 + password.len();
        }
        len
    }
}

/// Property list for CONNECT packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectProperties {
    /// Session Expiry Interval
    pub session_expiry_interval: Option<u32>,
    /// Receive Maximum
    pub receive_max: Option<u16>,
    /// Maximum Packet Size
    pub max_packet_size: Option<u32>,
    /// Topic Alias Maximum
    pub topic_alias_max: Option<u16>,
    /// Request Response Information. If absent the default value should be false.
    pub request_response_info: Option<bool>,
    /// Request Problem Information. If absent the default value should be true.
    pub request_problem_info: Option<bool>,
    /// User Property
    pub user_properties: Vec<UserProperty>,
    /// Authentication Method
    pub auth_method: Option<Arc<str>>,
    /// Authentication Data
    pub auth_data: Option<Bytes>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ConnectProperties {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(ConnectProperties {
            session_expiry_interval: u.arbitrary()?,
            receive_max: u.arbitrary()?,
            max_packet_size: u.arbitrary()?,
            topic_alias_max: u.arbitrary()?,
            request_response_info: u.arbitrary()?,
            request_problem_info: u.arbitrary()?,
            user_properties: u.arbitrary()?,
            auth_method: u.arbitrary()?,
            auth_data: Option::<Vec<u8>>::arbitrary(u)?.map(Bytes::from),
        })
    }
}

impl ConnectProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = ConnectProperties::default();
        decode_properties!(
            packet_type,
            properties,
            reader,
            SessionExpiryInterval,
            ReceiveMaximum,
            MaximumPacketSize,
            TopicAliasMaximum,
            RequestResponseInformation,
            RequestProblemInformation,
            AuthenticationMethod,
            AuthenticationData,
        );
        Ok(properties)
    }
}

impl Encodable for ConnectProperties {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        encode_properties!(
            self,
            writer,
            SessionExpiryInterval,
            ReceiveMaximum,
            MaximumPacketSize,
            TopicAliasMaximum,
            RequestResponseInformation,
            RequestProblemInformation,
            AuthenticationMethod,
            AuthenticationData,
        );
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(
            self,
            len,
            SessionExpiryInterval,
            ReceiveMaximum,
            MaximumPacketSize,
            TopicAliasMaximum,
            RequestResponseInformation,
            RequestProblemInformation,
            AuthenticationMethod,
            AuthenticationData,
        );
        len
    }
}

/// The will message for CONNECT packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWill {
    pub qos: QoS,
    pub retain: bool,
    pub topic_name: TopicName,
    pub payload: Bytes,
    pub properties: WillProperties,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for LastWill {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(LastWill {
            qos: u.arbitrary()?,
            retain: u.arbitrary()?,
            properties: u.arbitrary()?,
            topic_name: u.arbitrary()?,
            payload: Bytes::from(Vec::<u8>::arbitrary(u)?),
        })
    }
}

impl LastWill {
    pub fn new(qos: QoS, topic_name: TopicName, payload: Bytes) -> Self {
        LastWill {
            qos,
            retain: false,
            topic_name,
            payload,
            properties: WillProperties::default(),
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        qos: QoS,
        retain: bool,
    ) -> Result<Self, ErrorV5> {
        let properties = WillProperties::decode_async(reader).await?;
        let topic_name = TopicName::try_from(read_string_async(reader).await?)?;
        let payload = read_bytes_async(reader).await?;
        if properties.payload_is_utf8 == Some(true) && from_utf8(&payload).is_err() {
            return Err(ErrorV5::InvalidPayloadFormat);
        }
        Ok(LastWill {
            qos,
            retain,
            properties,
            topic_name,
            payload: Bytes::from(payload),
        })
    }
}

impl Encodable for LastWill {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        self.properties.encode(writer)?;
        write_bytes(writer, self.topic_name.as_bytes())?;
        write_bytes(writer, self.payload.as_ref())?;
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = self.properties.encode_len();
        len += 4;
        len += self.topic_name.len();
        len += self.payload.len();
        len
    }
}

/// Property list for will message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WillProperties {
    pub delay_interval: Option<u32>,
    pub payload_is_utf8: Option<bool>,
    pub message_expiry_interval: Option<u32>,
    pub content_type: Option<Arc<str>>,
    pub response_topic: Option<TopicName>,
    pub correlation_data: Option<Bytes>,
    pub user_properties: Vec<UserProperty>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for WillProperties {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(WillProperties {
            delay_interval: u.arbitrary()?,
            payload_is_utf8: u.arbitrary()?,
            message_expiry_interval: u.arbitrary()?,
            content_type: u.arbitrary()?,
            response_topic: u.arbitrary()?,
            correlation_data: Option::<Vec<u8>>::arbitrary(u)?.map(Bytes::from),
            user_properties: u.arbitrary()?,
        })
    }
}

impl WillProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, ErrorV5> {
        let mut properties = WillProperties::default();
        decode_properties!(
            LastWill,
            properties,
            reader,
            WillDelayInterval,
            PayloadFormatIndicator,
            MessageExpiryInterval,
            ContentType,
            ResponseTopic,
            CorrelationData,
        );
        Ok(properties)
    }
}

impl Encodable for WillProperties {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        encode_properties!(
            self,
            writer,
            WillDelayInterval,
            PayloadFormatIndicator,
            MessageExpiryInterval,
            ContentType,
            ResponseTopic,
            CorrelationData,
        );
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(
            self,
            len,
            WillDelayInterval,
            PayloadFormatIndicator,
            MessageExpiryInterval,
            ContentType,
            ResponseTopic,
            CorrelationData,
        );
        len
    }
}

/// Body type of CONNACK packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Connack {
    pub session_present: bool,
    pub reason_code: ConnectReasonCode,
    pub properties: ConnackProperties,
}

impl Connack {
    pub fn new(session_present: bool, reason_code: ConnectReasonCode) -> Self {
        Connack {
            session_present,
            reason_code,
            properties: ConnackProperties::default(),
        }
    }
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let mut payload = [0u8; 2];
        reader
            .read_exact(&mut payload)
            .await
            .map_err(ToError::to_error)?;
        let session_present = match payload[0] {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidConnackFlags(payload[0]).into()),
        };
        let reason_code = ConnectReasonCode::from_u8(payload[1])
            .ok_or(ErrorV5::InvalidReasonCode(header.typ, payload[1]))?;
        let properties = ConnackProperties::decode_async(reader, header.typ).await?;
        Ok(Connack {
            session_present,
            reason_code,
            properties,
        })
    }
}

impl Encodable for Connack {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u8(writer, u8::from(self.session_present))?;
        write_u8(writer, self.reason_code as u8)?;
        self.properties.encode(writer)?;
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self.properties.encode_len()
    }
}

/// Reason code for CONNECT packet.
///
/// | Dec |  Hex | Reason Code name              | Description                                                                                              |
/// |-----|------|-------------------------------|----------------------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Success                       | The Connection is accepted.                                                                              |
/// | 128 | 0x80 | Unspecified error             | The Server does not wish to reveal the reason for the failure, or none of the other Reason Codes apply.  |
/// | 129 | 0x81 | Malformed Packet              | Data within the CONNECT packet could not be correctly parsed.                                            |
/// | 130 | 0x82 | Protocol Error                | Data in the CONNECT packet does not conform to this specification.                                       |
/// | 131 | 0x83 | Implementation specific error | The CONNECT is valid but is not accepted by this Server.                                                 |
/// | 132 | 0x84 | Unsupported Protocol Version  | The Server does not support the version of the MQTT protocol requested by the Client.                    |
/// | 133 | 0x85 | Client Identifier not valid   | The Client Identifier is a valid string but is not allowed by the Server.                                |
/// | 134 | 0x86 | Bad User Name or Password     | The Server does not accept the User Name or Password specified by the Client                             |
/// | 135 | 0x87 | Not authorized                | The Client is not authorized to connect.                                                                 |
/// | 136 | 0x88 | Server unavailable            | The MQTT Server is not available.                                                                        |
/// | 137 | 0x89 | Server busy                   | The Server is busy. Try again later.                                                                     |
/// | 138 | 0x8A | Banned                        | This Client has been banned by administrative action. Contact the server administrator.                  |
/// | 140 | 0x8C | Bad authentication method     | The authentication method is not supported or does not match the authentication method currently in use. |
/// | 144 | 0x90 | Topic Name invalid            | The Will Topic Name is not malformed, but is not accepted by this Server.                                |
/// | 149 | 0x95 | Packet too large              | The CONNECT packet exceeded the maximum permissible size.                                                |
/// | 151 | 0x97 | Quota exceeded                | An implementation or administrative imposed limit has been exceeded.                                     |
/// | 153 | 0x99 | Payload format invalid        | The Will Payload does not match the specified Payload Format Indicator.                                  |
/// | 154 | 0x9A | Retain not supported          | The Server does not support retained messages, and Will Retain was set to 1.                             |
/// | 155 | 0x9B | QoS not supported             | The Server does not support the QoS set in Will QoS.                                                     |
/// | 156 | 0x9C | Use another server            | The Client should temporarily use another server.                                                        |
/// | 157 | 0x9D | Server moved                  | The Client should permanently use another server.                                                        |
/// | 159 | 0x9F | Connection rate exceeded      | The connection rate limit has been exceeded.                                                             |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum ConnectReasonCode {
    Success = 0x00,
    UnspecifiedError = 0x80,
    MalformedPacket = 0x81,
    ProtocolError = 0x82,
    ImplementationSpecificError = 0x83,
    UnsupportedProtocolVersion = 0x84,
    ClientIdentifierNotValid = 0x85,
    BadUserNameOrPassword = 0x86,
    NotAuthorized = 0x87,
    ServerUnavailable = 0x88,
    ServerBusy = 0x89,
    Banned = 0x8A,
    BadAuthMethod = 0x8C,
    TopicNameInvalid = 0x90,
    PacketTooLarge = 0x95,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
    RetainNotSupported = 0x9A,
    QoSNotSupported = 0x9B,
    UseAnotherServer = 0x9C,
    ServerMoved = 0x9D,
    ConnectionRateExceeded = 0x9F,
}

impl ConnectReasonCode {
    pub fn from_u8(value: u8) -> Option<ConnectReasonCode> {
        let code = match value {
            0x00 => ConnectReasonCode::Success,
            0x80 => ConnectReasonCode::UnspecifiedError,
            0x81 => ConnectReasonCode::MalformedPacket,
            0x82 => ConnectReasonCode::ProtocolError,
            0x83 => ConnectReasonCode::ImplementationSpecificError,
            0x84 => ConnectReasonCode::UnsupportedProtocolVersion,
            0x85 => ConnectReasonCode::ClientIdentifierNotValid,
            0x86 => ConnectReasonCode::BadUserNameOrPassword,
            0x87 => ConnectReasonCode::NotAuthorized,
            0x88 => ConnectReasonCode::ServerUnavailable,
            0x89 => ConnectReasonCode::ServerBusy,
            0x8A => ConnectReasonCode::Banned,
            0x8C => ConnectReasonCode::BadAuthMethod,
            0x90 => ConnectReasonCode::TopicNameInvalid,
            0x95 => ConnectReasonCode::PacketTooLarge,
            0x97 => ConnectReasonCode::QuotaExceeded,
            0x99 => ConnectReasonCode::PayloadFormatInvalid,
            0x9A => ConnectReasonCode::RetainNotSupported,
            0x9B => ConnectReasonCode::QoSNotSupported,
            0x9C => ConnectReasonCode::UseAnotherServer,
            0x9D => ConnectReasonCode::ServerMoved,
            0x9F => ConnectReasonCode::ConnectionRateExceeded,
            _ => return None,
        };
        Some(code)
    }
}

/// Property list for CONNACK packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnackProperties {
    pub session_expiry_interval: Option<u32>,
    pub receive_max: Option<u16>,
    pub max_qos: Option<QoS>,
    pub retain_available: Option<bool>,
    pub max_packet_size: Option<u32>,
    pub assigned_client_id: Option<Arc<str>>,
    pub topic_alias_max: Option<u16>,
    pub reason_string: Option<Arc<str>>,
    pub user_properties: Vec<UserProperty>,
    pub wildcard_subscription_available: Option<bool>,
    pub subscription_id_available: Option<bool>,
    pub shared_subscription_available: Option<bool>,
    pub server_keep_alive: Option<u16>,
    pub response_info: Option<Arc<str>>,
    pub server_reference: Option<Arc<str>>,
    pub auth_method: Option<Arc<str>>,
    pub auth_data: Option<Bytes>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ConnackProperties {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(ConnackProperties {
            session_expiry_interval: u.arbitrary()?,
            receive_max: u.arbitrary()?,
            max_qos: u.arbitrary()?,
            retain_available: u.arbitrary()?,
            max_packet_size: u.arbitrary()?,
            assigned_client_id: u.arbitrary()?,
            topic_alias_max: u.arbitrary()?,
            reason_string: u.arbitrary()?,
            user_properties: u.arbitrary()?,
            wildcard_subscription_available: u.arbitrary()?,
            subscription_id_available: u.arbitrary()?,
            shared_subscription_available: u.arbitrary()?,
            server_keep_alive: u.arbitrary()?,
            response_info: u.arbitrary()?,
            server_reference: u.arbitrary()?,
            auth_method: u.arbitrary()?,
            auth_data: Option::<Vec<u8>>::arbitrary(u)?.map(Bytes::from),
        })
    }
}

impl ConnackProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = ConnackProperties::default();
        decode_properties!(
            packet_type,
            properties,
            reader,
            SessionExpiryInterval,
            ReceiveMaximum,
            MaximumQoS,
            RetainAvailable,
            MaximumPacketSize,
            AssignedClientIdentifier,
            TopicAliasMaximum,
            ReasonString,
            WildcardSubscriptionAvailable,
            SubscriptionIdentifierAvailable,
            SharedSubscriptionAvailable,
            ServerKeepAlive,
            ResponseInformation,
            ServerReference,
            AuthenticationMethod,
            AuthenticationData,
        );
        Ok(properties)
    }
}

impl Encodable for ConnackProperties {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        encode_properties!(
            self,
            writer,
            SessionExpiryInterval,
            ReceiveMaximum,
            MaximumQoS,
            RetainAvailable,
            MaximumPacketSize,
            AssignedClientIdentifier,
            TopicAliasMaximum,
            ReasonString,
            WildcardSubscriptionAvailable,
            SubscriptionIdentifierAvailable,
            SharedSubscriptionAvailable,
            ServerKeepAlive,
            ResponseInformation,
            ServerReference,
            AuthenticationMethod,
            AuthenticationData,
        );
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(
            self,
            len,
            SessionExpiryInterval,
            ReceiveMaximum,
            MaximumQoS,
            RetainAvailable,
            MaximumPacketSize,
            AssignedClientIdentifier,
            TopicAliasMaximum,
            ReasonString,
            WildcardSubscriptionAvailable,
            SubscriptionIdentifierAvailable,
            SharedSubscriptionAvailable,
            ServerKeepAlive,
            ResponseInformation,
            ServerReference,
            AuthenticationMethod,
            AuthenticationData,
        );
        len
    }
}

/// Body type for DISCONNECT packet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Disconnect {
    pub reason_code: DisconnectReasonCode,
    pub properties: DisconnectProperties,
}

impl Disconnect {
    pub fn new(reason_code: DisconnectReasonCode) -> Self {
        Disconnect {
            reason_code,
            properties: DisconnectProperties::default(),
        }
    }

    pub fn new_normal() -> Self {
        Self::new(DisconnectReasonCode::NormalDisconnect)
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let (reason_code, properties) = if header.remaining_len == 0 {
            (DisconnectReasonCode::NormalDisconnect, Default::default())
        } else if header.remaining_len == 1 {
            let reason_byte = read_u8_async(reader).await?;
            let reason_code = DisconnectReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            (reason_code, Default::default())
        } else {
            let reason_byte = read_u8_async(reader).await?;
            let reason_code = DisconnectReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            let properties = DisconnectProperties::decode_async(reader, header.typ).await?;
            (reason_code, properties)
        };
        Ok(Disconnect {
            reason_code,
            properties,
        })
    }
}

impl Encodable for Disconnect {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        if self.properties == DisconnectProperties::default() {
            if self.reason_code != DisconnectReasonCode::NormalDisconnect {
                write_u8(writer, self.reason_code as u8)?;
            }
        } else {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.properties == DisconnectProperties::default() {
            if self.reason_code == DisconnectReasonCode::NormalDisconnect {
                0
            } else {
                1
            }
        } else {
            1 + self.properties.encode_len()
        }
    }
}

/// Reason code for DISCONNECT packet.
///
/// | Dec |  Hex | Reason Code name                       | Sent by       | Description                                                                                    |
/// |-----|------|----------------------------------------|---------------|------------------------------------------------------------------------------------------------|
/// |   0 | 0x00 | Normal disconnection                   | Client/Server | Close the connection normally. Do not send the Will Message.                                   |
/// |   4 | 0x04 | Disconnect with Will Message           | Client        | The Client wishes to disconnect but requires that the Server also publishes its Will Message.  |
/// | 128 | 0x80 | Unspecified error                      | Client/Server | The Connection is closed but the sender either does not wish to reveal the reason,             |
/// |     |      |                                        |               | or none of the other Reason Codes apply.                                                       |
/// | 129 | 0x81 | Malformed Packet                       | Client/Server | The received packet does not conform to this specification.                                    |
/// | 130 | 0x82 | Protocol Error                         | Client/Server | An unexpected or out of order packet was received.                                             |
/// | 131 | 0x83 | Implementation specific error          | Client/Server | The packet received is valid but cannot be processed by this implementation.                   |
/// | 135 | 0x87 | Not authorized                         | Server        | The request is not authorized.                                                                 |
/// | 137 | 0x89 | Server busy                            | Server        | The Server is busy and cannot continue processing requests from this Client.                   |
/// | 139 | 0x8B | Server shutting down                   | Server        | The Server is shutting down.                                                                   |
/// | 141 | 0x8D | Keep Alive timeout                     | Server        | The Connection is closed because no packet has been received for 1.5 times the Keepalive time. |
/// | 142 | 0x8E | Session taken over                     | Server        | Another Connection using the same ClientID has connected causing this Connection to be closed. |
/// | 143 | 0x8F | Topic Filter invalid                   | Server        | The Topic Filter is correctly formed, but is not accepted by this Sever.                       |
/// | 144 | 0x90 | Topic Name invalid                     | Client/Server | The Topic Name is correctly formed, but is not accepted by this Client/Server.                 |
/// | 147 | 0x93 | Receive Maximum exceeded               | Client/Server | The Client/Server has received more than Receive Maximum publication for                       |
/// |     |      |                                        |               | which it has not sent PUBACK or PUBCOMP.                                                       |
/// | 148 | 0x94 | Topic Alias invalid                    | Client/Server | The Client/Server has received a PUBLISH packet containing a Topic Alias                       |
/// |     |      |                                        |               | which is greater than the Maximum Topic Alias it sent in the CONNECT or CONNACK packet.        |
/// | 149 | 0x95 | Packet too large                       | Client/Server | The packet size is greater than Maximum Packet Size for this Client/Server.                    |
/// | 150 | 0x96 | Message rate too high                  | Client/Server | The received data rate is too high.                                                            |
/// | 151 | 0x97 | Quota exceeded                         | Client/Server | An implementation or administrative imposed limit has been exceeded.                           |
/// | 152 | 0x98 | Administrative action                  | Client/Server | The Connection is closed due to an administrative action.                                      |
/// | 153 | 0x99 | Payload format invalid                 | Client/Server | The payload format does not match the one specified by the Payload Format Indicator.           |
/// | 154 | 0x9A | Retain not supported                   | Server        | The Server has does not support retained messages.                                             |
/// | 155 | 0x9B | QoS not supported                      | Server        | The Client specified a QoS greater than the QoS specified in a Maximum QoS in the CONNACK.     |
/// | 156 | 0x9C | Use another server                     | Server        | The Client should temporarily change its Server.                                               |
/// | 157 | 0x9D | Server moved                           | Server        | The Server is moved and the Client should permanently change its server location.              |
/// | 158 | 0x9E | Shared Subscriptions not supported     | Server        | The Server does not support Shared Subscriptions.                                              |
/// | 159 | 0x9F | Connection rate exceeded               | Server        | This connection is closed because the connection rate is too high.                             |
/// | 160 | 0xA0 | Maximum connect time                   | Server        | The maximum connection time authorized for this connection has been exceeded.                  |
/// | 161 | 0xA1 | Subscription Identifiers not supported | Server        | The Server does not support Subscription Identifiers; the subscription is not accepted.        |
/// | 162 | 0xA2 | Wildcard Subscriptions not supported   | Server        | The Server does not support Wildcard Subscriptions; the subscription is not accepted.          |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum DisconnectReasonCode {
    NormalDisconnect = 0x00,
    DisconnectWithWillMessage = 0x04,
    UnspecifiedError = 0x80,
    MalformedPacket = 0x81,
    ProtocolError = 0x82,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    ServerBusy = 0x89,
    ServerShuttingDown = 0x8B,
    KeepAliveTimeout = 0x8D,
    SessionTakenOver = 0x8E,
    TopicFilterInvalid = 0x8F,
    TopicNameInvalid = 0x90,
    ReceiveMaximumExceeded = 0x93,
    TopicAliasInvalid = 0x94,
    PacketTooLarge = 0x95,
    MessageRateTooHigh = 0x96,
    QuotaExceeded = 0x97,
    AdministrativeAction = 0x98,
    PayloadFormatInvalid = 0x99,
    RetainNotSupported = 0x9A,
    QoSNotSupported = 0x9B,
    UserAnotherServer = 0x9C,
    ServerMoved = 0x9D,
    SharedSubscriptionNotSupported = 0x9E,
    ConnectionRateExceeded = 0x9F,
    MaximumConnectTime = 0xA0,
    SubscriptionIdentifiersNotSupported = 0xA1,
    WildcardSubscriptionsNotSupported = 0xA2,
}

impl DisconnectReasonCode {
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => Self::NormalDisconnect,
            0x04 => Self::DisconnectWithWillMessage,
            0x80 => Self::UnspecifiedError,
            0x81 => Self::MalformedPacket,
            0x82 => Self::ProtocolError,
            0x83 => Self::ImplementationSpecificError,
            0x87 => Self::NotAuthorized,
            0x89 => Self::ServerBusy,
            0x8B => Self::ServerShuttingDown,
            0x8D => Self::KeepAliveTimeout,
            0x8E => Self::SessionTakenOver,
            0x8F => Self::TopicFilterInvalid,
            0x90 => Self::TopicNameInvalid,
            0x93 => Self::ReceiveMaximumExceeded,
            0x94 => Self::TopicAliasInvalid,
            0x95 => Self::PacketTooLarge,
            0x96 => Self::MessageRateTooHigh,
            0x97 => Self::QuotaExceeded,
            0x98 => Self::AdministrativeAction,
            0x99 => Self::PayloadFormatInvalid,
            0x9A => Self::RetainNotSupported,
            0x9B => Self::QoSNotSupported,
            0x9C => Self::UserAnotherServer,
            0x9D => Self::ServerMoved,
            0x9E => Self::SharedSubscriptionNotSupported,
            0x9F => Self::ConnectionRateExceeded,
            0xA0 => Self::MaximumConnectTime,
            0xA1 => Self::SubscriptionIdentifiersNotSupported,
            0xA2 => Self::WildcardSubscriptionsNotSupported,
            _ => return None,
        };
        Some(code)
    }
}

/// Property list for DISCONNECT packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct DisconnectProperties {
    pub session_expiry_interval: Option<u32>,
    pub reason_string: Option<Arc<str>>,
    pub user_properties: Vec<UserProperty>,
    pub server_reference: Option<Arc<str>>,
}

impl DisconnectProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = DisconnectProperties::default();
        decode_properties!(
            packet_type,
            properties,
            reader,
            SessionExpiryInterval,
            ReasonString,
            ServerReference,
        );
        Ok(properties)
    }
}

impl Encodable for DisconnectProperties {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        encode_properties!(
            self,
            writer,
            SessionExpiryInterval,
            ReasonString,
            ServerReference,
        );
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(
            self,
            len,
            SessionExpiryInterval,
            ReasonString,
            ServerReference,
        );
        len
    }
}

/// Body type of AUTH packet .
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Auth {
    pub reason_code: AuthReasonCode,
    pub properties: AuthProperties,
}

impl Auth {
    pub fn new(reason_code: AuthReasonCode) -> Self {
        Auth {
            reason_code,
            properties: AuthProperties::default(),
        }
    }

    pub fn new_success() -> Self {
        Self::new(AuthReasonCode::Success)
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let auth = if header.remaining_len == 0 {
            Auth {
                reason_code: AuthReasonCode::Success,
                properties: AuthProperties::default(),
            }
        } else {
            let reason_byte = read_u8_async(reader).await?;
            let reason_code = AuthReasonCode::from_u8(reason_byte)
                .ok_or(ErrorV5::InvalidReasonCode(header.typ, reason_byte))?;
            let properties = AuthProperties::decode_async(reader, header.typ).await?;
            Auth {
                reason_code,
                properties,
            }
        };
        Ok(auth)
    }
}

impl Encodable for Auth {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        if self.reason_code != AuthReasonCode::Success
            || self.properties != AuthProperties::default()
        {
            write_u8(writer, self.reason_code as u8)?;
            self.properties.encode(writer)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        if self.reason_code == AuthReasonCode::Success
            && self.properties == AuthProperties::default()
        {
            0
        } else {
            1 + self.properties.encode_len()
        }
    }
}

/// Reason code for AUTH packet.
///
/// | Dec |  Hex | Reason Code name        | Sent by       | Description                                   |
/// |-----|------|-------------------------|---------------|-----------------------------------------------|
/// |   0 | 0x00 | Success                 | Server        | Authentication is successful                  |
/// |  24 | 0x18 | Continue authentication | Client/Server | Continue the authentication with another step |
/// |  25 | 0x19 | Re-authenticate         | Client        | Initiate a re-authentication                  |
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum AuthReasonCode {
    Success = 0x00,
    ContinueAuthentication = 0x18,
    ReAuthentication = 0x19,
}

impl AuthReasonCode {
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => Self::Success,
            0x18 => Self::ContinueAuthentication,
            0x19 => Self::ReAuthentication,
            _ => return None,
        };
        Some(code)
    }
}

/// Property list for AUTH packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthProperties {
    pub auth_method: Option<Arc<str>>,
    pub auth_data: Option<Bytes>,
    pub reason_string: Option<Arc<str>>,
    pub user_properties: Vec<UserProperty>,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for AuthProperties {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(AuthProperties {
            auth_method: u.arbitrary()?,
            auth_data: Option::<Vec<u8>>::arbitrary(u)?.map(Bytes::from),
            reason_string: u.arbitrary()?,
            user_properties: u.arbitrary()?,
        })
    }
}

impl AuthProperties {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let mut properties = AuthProperties::default();
        decode_properties!(
            packet_type,
            properties,
            reader,
            AuthenticationMethod,
            AuthenticationData,
            ReasonString,
        );
        Ok(properties)
    }
}

impl Encodable for AuthProperties {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        encode_properties!(
            self,
            writer,
            AuthenticationMethod,
            AuthenticationData,
            ReasonString,
        );
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 0;
        encode_properties_len!(
            self,
            len,
            AuthenticationMethod,
            AuthenticationData,
            ReasonString,
        );
        len
    }
}
