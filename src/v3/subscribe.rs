use std::io;

use futures_lite::io::AsyncRead;

use crate::{
    read_string, read_u16, read_u8, write_bytes, write_u16, write_u8, Encodable, Error, Pid, QoS,
    TopicFilter,
};

/// Subscribe packet payload type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Subscribe {
    pub pid: Pid,
    pub topics: Vec<(TopicFilter, QoS)>,
}

/// Suback packet payload type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Suback {
    pub pid: Pid,
    pub topics: Vec<SubscribeReturnCode>,
}

/// Unsubscribe packet payload type.
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

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        mut remaining_len: usize,
    ) -> Result<Self, Error> {
        let pid = Pid::try_from(read_u16(reader).await?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription);
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string(reader).await?)?;
            let max_qos = QoS::from_u8(read_u8(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(3 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push((topic_filter, max_qos));
        }
        Ok(Subscribe { pid, topics })
    }
}

impl Encodable for Subscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        for (topic_filter, max_qos) in &self.topics {
            write_bytes(writer, topic_filter.as_bytes())?;
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

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        mut remaining_len: usize,
    ) -> Result<Self, Error> {
        let pid = Pid::try_from(read_u16(reader).await?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let value = read_u8(reader).await?;
            let code = SubscribeReturnCode::from_u8(value)?;
            topics.push(code);
            remaining_len -= 1;
        }
        Ok(Suback { pid, topics })
    }
}

impl Encodable for Suback {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
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

    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        mut remaining_len: usize,
    ) -> Result<Self, Error> {
        let pid = Pid::try_from(read_u16(reader).await?)?;
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        if remaining_len == 0 {
            return Err(Error::EmptySubscription);
        }
        let mut topics = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(2 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            topics.push(topic_filter);
        }
        Ok(Unsubscribe { pid, topics })
    }
}

impl Encodable for Unsubscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        for topic_filter in &self.topics {
            write_bytes(writer, topic_filter.as_bytes())?;
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
