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
    read_bytes, read_string, read_u16, read_u8, write_bytes, write_u16, write_u8, Encodable, Error,
    Pid, Protocol, QoS, QosPid, TopicName,
};

/// Payload type of PUBLISH packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publish {
    dup: bool,
    qos_pid: QosPid,
    retain: bool,
    topic_name: TopicName,
    properties: PublishProperties,
    payload: Bytes,
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
                QosPid::Level1(Pid::new(read_u16(reader).await?))
            }
            QoS::Level2 => {
                remaining_len = remaining_len
                    .checked_sub(2)
                    .ok_or(Error::InvalidRemainingLength)?;
                QosPid::Level2(Pid::new(read_u16(reader).await?))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puback;

impl Puback {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pubrec;

impl Pubrec {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pubrel;

impl Pubrel {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pubcomp;

impl Pubcomp {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}
