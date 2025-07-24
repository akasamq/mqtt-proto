use alloc::vec::Vec;

use crate::{
    read_string_async, read_u16_async, read_u8_async, write_string, write_u16, write_u8, AsyncRead,
    Encodable, Error, PacketBuf, Pid, QoS, SyncWrite, TopicFilter,
};

use super::Header;

/// Subscribe packet body type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Subscribe {
    pub pid: Pid,
    pub topics: Vec<(TopicFilter, QoS)>,
}

/// Suback packet body type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Suback {
    pub pid: Pid,
    pub topics: Vec<SubscribeReturnCode>,
}

/// Unsubscribe packet body type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Unsubscribe {
    pub pid: Pid,
    pub topics: Vec<TopicFilter>,
}

impl Subscribe {
    pub fn new(pid: Pid, topics: Vec<(TopicFilter, QoS)>) -> Self {
        Self { pid, topics }
    }

    pub fn decode(buf: &mut PacketBuf, header: Header) -> Result<Self, Error> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(buf.read_u16()?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        if buf.remaining() == 0 {
            return Err(Error::EmptySubscription);
        }
        let mut topics = Vec::new();
        while buf.remaining() > 0 {
            let topic_filter = TopicFilter::try_from(buf.read_string()?)?;
            let max_qos = QoS::from_u8(buf.read_u8()?)?;
            remaining_len = remaining_len
                .checked_sub(3 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push((topic_filter, max_qos));
        }
        Ok(Subscribe { pid, topics })
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, Error> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription);
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string_async(reader).await?)?;
            let max_qos = QoS::from_u8(read_u8_async(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(3 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push((topic_filter, max_qos));
        }
        Ok(Subscribe { pid, topics })
    }
}

impl Encodable for Subscribe {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        for (topic_filter, max_qos) in &self.topics {
            write_string(writer, topic_filter)?;
            write_u8(writer, *max_qos as u8)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self
            .topics
            .iter()
            .map(|(filter, _)| 3 + filter.len())
            .sum::<usize>()
    }
}

impl Suback {
    pub fn new(pid: Pid, topics: Vec<SubscribeReturnCode>) -> Self {
        Self { pid, topics }
    }

    pub fn decode(buf: &mut PacketBuf, header: Header) -> Result<Self, Error> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(buf.read_u16()?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let value = buf.read_u8()?;
            let code = SubscribeReturnCode::from_u8(value)?;
            topics.push(code);
            remaining_len -= 1;
        }
        Ok(Suback { pid, topics })
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, Error> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let value = read_u8_async(reader).await?;
            let code = SubscribeReturnCode::from_u8(value)?;
            topics.push(code);
            remaining_len -= 1;
        }
        Ok(Suback { pid, topics })
    }
}

impl Encodable for Suback {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        for code in &self.topics {
            write_u8(writer, *code as u8)?;
        }
        Ok(())
    }
    fn encode_len(&self) -> usize {
        2 + self.topics.len()
    }
}

impl Unsubscribe {
    pub fn new(pid: Pid, topics: Vec<TopicFilter>) -> Self {
        Self { pid, topics }
    }

    pub fn decode(buf: &mut PacketBuf, header: Header) -> Result<Self, Error> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(buf.read_u16()?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription);
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(buf.read_string()?)?;
            remaining_len = remaining_len
                .checked_sub(2 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push(topic_filter);
        }
        Ok(Unsubscribe { pid, topics })
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, Error> {
        let mut remaining_len = header.remaining_len as usize;
        let pid = Pid::try_from(read_u16_async(reader).await?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription);
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string_async(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(2 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push(topic_filter);
        }
        Ok(Unsubscribe { pid, topics })
    }
}

impl Encodable for Unsubscribe {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        write_u16(writer, self.pid.value())?;
        for topic_filter in &self.topics {
            write_string(writer, topic_filter)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self
            .topics
            .iter()
            .map(|filter| 2 + filter.len())
            .sum::<usize>()
    }
}

/// Subscribe return code type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum SubscribeReturnCode {
    MaxLevel0,
    MaxLevel1,
    MaxLevel2,
    Failure,
}

impl SubscribeReturnCode {
    pub fn from_u8(value: u8) -> Result<SubscribeReturnCode, Error> {
        match value {
            0x80 => Ok(SubscribeReturnCode::Failure),
            0 => Ok(SubscribeReturnCode::MaxLevel0),
            1 => Ok(SubscribeReturnCode::MaxLevel1),
            2 => Ok(SubscribeReturnCode::MaxLevel2),
            _ => Err(Error::InvalidQos(value)),
        }
    }
}

impl From<QoS> for SubscribeReturnCode {
    fn from(qos: QoS) -> SubscribeReturnCode {
        match qos {
            QoS::Level0 => SubscribeReturnCode::MaxLevel0,
            QoS::Level1 => SubscribeReturnCode::MaxLevel1,
            QoS::Level2 => SubscribeReturnCode::MaxLevel2,
        }
    }
}
