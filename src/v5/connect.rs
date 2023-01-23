use std::convert::TryFrom;
use std::io;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt};

use super::{ErrorV5, Header, PacketType, PropertyType, PropertyValue, UserProperty};
use crate::{
    decode_var_int, read_bytes, read_string, read_u16, read_u32, read_u8, var_int_len, write_bytes,
    write_u16, write_u32, write_u8, write_var_int, Encodable, Error, Protocol, QoS, TopicName,
};

/// [Connect] packet payload type.
///
/// [Connect]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901033
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
    pub client_id: Arc<String>,

    /// The [will] message.
    ///
    /// [will]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901060
    pub last_will: Option<LastWill>,

    /// The [user name](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901071).
    pub username: Option<Arc<String>>,

    /// The [password](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901072).
    pub password: Option<Bytes>,
}

impl Connect {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        let protocol = Protocol::decode_async(reader).await?;
        if protocol != Protocol::MqttV50 {
            return Err(ErrorV5::UnexpectedProtocol(protocol));
        }
        let connect_flags: u8 = read_u8(reader).await?;
        let keep_alive = read_u16(reader).await?;

        // FIXME: check remaining length
        // const PROTOCOL_NAME_LEN: usize = 2 + 4;
        // const PROTOCOL_VER_LEN: usize = 1;
        // const CONNECT_FLAGS_LEN: usize = 1;
        // const KEEP_ALIVE_LEN: usize = 2;
        // header.remaining_len = header
        //     .remaining_len
        //     .checked_sub(
        //         PROTOCOL_NAME_LEN
        //             + PROTOCOL_VER_LEN
        //             + CONNECT_FLAGS_LEN
        //             + KEEP_ALIVE_LEN
        //             + property_len_bytes,
        //     )
        //     .ok_or(Error::InvalidRemainingLength)?;

        let properties = ConnectProperties::decode_async(reader, header.typ).await?;
        let client_id = Arc::new(read_string(reader).await?);
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
            Some(Arc::new(read_string(reader).await?))
        } else {
            None
        };
        let password = if connect_flags & 0b01000000 != 0 {
            Some(Bytes::from(read_bytes(reader).await?))
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
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
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

/// Property list for connect packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectProperties {
    /// [Session Expiry Interval](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901048)
    pub session_expiry_interval: Option<u32>,
    /// [Receive Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901049)
    pub receive_max: Option<u16>,
    /// [Maximum Packet Size](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901050)
    pub max_packet_size: Option<u32>,
    /// [Topic Alias Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901051)
    pub topic_alias_max: Option<u16>,
    /// [Request Response Information](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901052). If absent the default value should be false.
    pub request_response_info: Option<bool>,
    /// [Request Problem Information](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901053). If absent the default value should be true.
    pub request_problem_info: Option<bool>,
    /// [User Property](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901054)
    pub user_properties: Vec<UserProperty>,
    /// [Authentication Method](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901055)
    pub auth_method: Option<Arc<String>>,
    /// [Authentication Data](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901056)
    pub auth_data: Option<Bytes>,
}

impl ConnectProperties {
    pub(crate) async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        packet_type: PacketType,
    ) -> Result<Self, ErrorV5> {
        let (mut property_len, _bytes) = decode_var_int(reader).await?;
        let mut properties = ConnectProperties::default();
        while property_len > 0 {
            let property_type = PropertyType::from_u8(read_u8(reader).await?)?;
            match property_type {
                PropertyType::SessionExpiryInterval => {
                    PropertyValue::decode_u32(
                        reader,
                        property_type,
                        &mut properties.session_expiry_interval,
                    )
                    .await?;
                }
                PropertyType::ReceiveMaximum => {
                    PropertyValue::decode_u16(reader, property_type, &mut properties.receive_max)
                        .await?;
                }
                PropertyType::MaximumPacketSize => {
                    PropertyValue::decode_u32(
                        reader,
                        property_type,
                        &mut properties.max_packet_size,
                    )
                    .await?;
                }
                PropertyType::TopicAliasMaximum => {
                    PropertyValue::decode_u16(
                        reader,
                        property_type,
                        &mut properties.topic_alias_max,
                    )
                    .await?;
                }
                PropertyType::RequestResponseInformation => {
                    PropertyValue::decode_bool(
                        reader,
                        property_type,
                        &mut properties.request_response_info,
                    )
                    .await?;
                }
                PropertyType::RequestProblemInformation => {
                    PropertyValue::decode_bool(
                        reader,
                        property_type,
                        &mut properties.request_problem_info,
                    )
                    .await?;
                }
                PropertyType::UserProperty => {
                    let user_property = PropertyValue::decode_user_property(reader).await?;
                    properties.user_properties.push(user_property);
                }
                PropertyType::AuthenticationMethod => {
                    PropertyValue::decode_string(
                        reader,
                        property_type,
                        &mut properties.auth_method,
                    )
                    .await?;
                }
                PropertyType::AuthenticationData => {
                    PropertyValue::decode_bytes(reader, property_type, &mut properties.auth_data)
                        .await?;
                }
                _ => return Err(ErrorV5::InvalidProperty(property_type, packet_type)),
            }
            property_len -= 1;
        }
        if properties.auth_data.is_some() && properties.auth_method.is_none() {
            return Err(ErrorV5::AuthMethodMissing);
        }
        Ok(properties)
    }
}

impl Encodable for ConnectProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let mut property_len = self.user_properties.len();
        if self.session_expiry_interval.is_some() {
            property_len += 1;
        }
        if self.receive_max.is_some() {
            property_len += 1;
        }
        if self.max_packet_size.is_some() {
            property_len += 1;
        }
        if self.topic_alias_max.is_some() {
            property_len += 1;
        }
        if self.request_response_info.is_some() {
            property_len += 1;
        }
        if self.request_problem_info.is_some() {
            property_len += 1;
        }
        if self.auth_method.is_some() {
            property_len += 1;
        }
        if self.auth_data.is_some() {
            property_len += 1;
        }

        write_var_int(writer, property_len)?;
        if let Some(value) = self.session_expiry_interval {
            write_u8(writer, PropertyType::SessionExpiryInterval as u8)?;
            write_u32(writer, value)?;
        }
        if let Some(value) = self.receive_max {
            write_u8(writer, PropertyType::ReceiveMaximum as u8)?;
            write_u16(writer, value)?;
        }
        if let Some(value) = self.max_packet_size {
            write_u8(writer, PropertyType::MaximumPacketSize as u8)?;
            write_u32(writer, value)?;
        }
        if let Some(value) = self.topic_alias_max {
            write_u8(writer, PropertyType::TopicAliasMaximum as u8)?;
            write_u16(writer, value)?;
        }
        if let Some(value) = self.request_response_info {
            write_u8(writer, PropertyType::RequestResponseInformation as u8)?;
            write_u8(writer, if value { 1u8 } else { 0u8 })?;
        }
        if let Some(value) = self.request_problem_info {
            write_u8(writer, PropertyType::RequestProblemInformation as u8)?;
            write_u8(writer, if value { 1u8 } else { 0u8 })?;
        }
        if let Some(value) = self.auth_method.as_ref() {
            write_u8(writer, PropertyType::AuthenticationMethod as u8)?;
            write_bytes(writer, value.as_bytes())?;
        }
        if let Some(value) = self.auth_data.as_ref() {
            write_u8(writer, PropertyType::AuthenticationData as u8)?;
            write_bytes(writer, value.as_ref())?;
        }
        for UserProperty { name, value } in &self.user_properties {
            write_u8(writer, PropertyType::UserProperty as u8)?;
            write_bytes(writer, name.as_bytes())?;
            write_bytes(writer, value.as_bytes())?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 0;
        let mut property_len = self.user_properties.len();
        if self.session_expiry_interval.is_some() {
            property_len += 1;
            len += 4;
        }
        if self.receive_max.is_some() {
            property_len += 1;
            len += 2;
        }
        if self.max_packet_size.is_some() {
            property_len += 1;
            len += 4;
        }
        if self.topic_alias_max.is_some() {
            property_len += 1;
            len += 2;
        }
        if self.request_response_info.is_some() {
            property_len += 1;
            len += 1;
        }
        if self.request_problem_info.is_some() {
            property_len += 1;
            len += 1;
        }
        if let Some(value) = self.auth_method.as_ref() {
            property_len += 1;
            len += 2 + value.len();
        }
        if let Some(value) = self.auth_data.as_ref() {
            property_len += 1;
            len += 2 + value.len();
        }
        len += var_int_len(property_len).expect("huge user properties");
        len += property_len;
        len += self
            .user_properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
        len
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWill {
    pub qos: QoS,
    pub retain: bool,
    pub properties: WillProperties,
    pub topic_name: TopicName,
    pub payload: Bytes,
}

impl LastWill {
    pub(crate) async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        qos: QoS,
        retain: bool,
    ) -> Result<Self, ErrorV5> {
        let properties = WillProperties::decode_async(reader).await?;
        let topic_name = TopicName::try_from(read_string(reader).await?)?;
        let payload = Bytes::from(read_bytes(reader).await?);
        Ok(LastWill {
            qos,
            retain,
            properties,
            topic_name,
            payload,
        })
    }
}

impl Encodable for LastWill {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
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

/// Property list for will message in connect packet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WillProperties {
    ///
    pub delay_interval: Option<u32>,
    /// Payload Format Indicator. This value specifies whether the payload format is UTF-8.
    pub payload_is_utf8: Option<bool>,
    ///
    pub message_expiry_interval: Option<u32>,
    ///
    pub content_type: Option<Arc<String>>,
    ///
    pub response_topic: Option<TopicName>,
    ///
    pub correlation_data: Option<Bytes>,
    /// User Property
    pub user_properties: Vec<UserProperty>,
}

impl WillProperties {
    pub(crate) async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
    ) -> Result<Self, ErrorV5> {
        let (mut property_len, _bytes) = decode_var_int(reader).await?;
        let mut properties = WillProperties::default();
        while property_len > 0 {
            let property_type = PropertyType::from_u8(read_u8(reader).await?)?;
            match property_type {
                PropertyType::WillDelayInterval => {
                    PropertyValue::decode_u32(
                        reader,
                        property_type,
                        &mut properties.delay_interval,
                    )
                    .await?;
                }
                PropertyType::PayloadFormatIndicator => {
                    PropertyValue::decode_bool(
                        reader,
                        property_type,
                        &mut properties.payload_is_utf8,
                    )
                    .await?;
                }
                PropertyType::MessageExpiryInterval => {
                    PropertyValue::decode_u32(
                        reader,
                        property_type,
                        &mut properties.message_expiry_interval,
                    )
                    .await?;
                }
                PropertyType::ContentType => {
                    PropertyValue::decode_string(
                        reader,
                        property_type,
                        &mut properties.content_type,
                    )
                    .await?;
                }
                PropertyType::ResponseTopic => {
                    PropertyValue::decode_topic_name(
                        reader,
                        property_type,
                        &mut properties.response_topic,
                    )
                    .await?;
                }
                PropertyType::CorrelationData => {
                    PropertyValue::decode_bytes(
                        reader,
                        property_type,
                        &mut properties.correlation_data,
                    )
                    .await?;
                }
                PropertyType::UserProperty => {
                    let user_property = PropertyValue::decode_user_property(reader).await?;
                    properties.user_properties.push(user_property);
                }
                _ => return Err(ErrorV5::InvalidWillProperty(property_type)),
            }
            property_len -= 1;
        }
        Ok(properties)
    }
}

impl Encodable for WillProperties {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let mut property_len = self.user_properties.len();
        if self.delay_interval.is_some() {
            property_len += 1;
        }
        if self.payload_is_utf8.is_some() {
            property_len += 1;
        }
        if self.message_expiry_interval.is_some() {
            property_len += 1;
        }
        if self.content_type.is_some() {
            property_len += 1;
        }
        if self.response_topic.is_some() {
            property_len += 1;
        }
        if self.correlation_data.is_some() {
            property_len += 1;
        }

        write_var_int(writer, property_len)?;
        if let Some(value) = self.delay_interval {
            write_u8(writer, PropertyType::WillDelayInterval as u8)?;
            write_u32(writer, value)?;
        }
        if let Some(value) = self.payload_is_utf8 {
            write_u8(writer, PropertyType::PayloadFormatIndicator as u8)?;
            write_u8(writer, if value { 1u8 } else { 0u8 })?;
        }
        if let Some(value) = self.message_expiry_interval {
            write_u8(writer, PropertyType::MessageExpiryInterval as u8)?;
            write_u32(writer, value)?;
        }
        if let Some(value) = self.content_type.as_ref() {
            write_u8(writer, PropertyType::ContentType as u8)?;
            write_bytes(writer, value.as_bytes())?;
        }
        if let Some(value) = self.response_topic.as_ref() {
            write_u8(writer, PropertyType::ResponseTopic as u8)?;
            write_bytes(writer, value.as_bytes())?;
        }
        if let Some(value) = self.correlation_data.as_ref() {
            write_u8(writer, PropertyType::CorrelationData as u8)?;
            write_bytes(writer, value.as_ref())?;
        }
        for UserProperty { name, value } in &self.user_properties {
            write_u8(writer, PropertyType::UserProperty as u8)?;
            write_bytes(writer, name.as_bytes())?;
            write_bytes(writer, value.as_bytes())?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut len = 0;
        let mut property_len = self.user_properties.len();
        if self.delay_interval.is_some() {
            property_len += 1;
            len += 4;
        }
        if self.payload_is_utf8.is_some() {
            property_len += 1;
            len += 1;
        }
        if self.message_expiry_interval.is_some() {
            property_len += 1;
            len += 4;
        }
        if let Some(value) = self.content_type.as_ref() {
            property_len += 1;
            len += 2 + value.len();
        }
        if let Some(value) = self.response_topic.as_ref() {
            property_len += 1;
            len += 2 + value.len();
        }
        if let Some(value) = self.correlation_data.as_ref() {
            property_len += 1;
            len += 2 + value.len();
        }
        len += var_int_len(property_len).expect("huge user properties");
        len += property_len;
        len += self
            .user_properties
            .iter()
            .map(|property| 4 + property.name.len() + property.value.len())
            .sum::<usize>();
        len
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connack {
    pub session_present: bool,
    pub code: ConnectReturnCode,
}

impl Connack {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectReturnCode {
    Accepted = 0,
    UnacceptableProtocolVersion = 1,
    IdentifierRejected = 2,
    ServerUnavailable = 3,
    BadUsernamePassword = 4,
    NotAuthorized = 5,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disconnect;

impl Disconnect {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auth;

impl Auth {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}
