use std::io;

use futures_lite::io::AsyncRead;

use crate::{
    read_string, read_u16, read_u8, write_bytes, write_u16, write_u8, Encodable, Error, Pid, QoS,
    TopicFilter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {
    pub pid: Pid,
    pub subscribes: Vec<(TopicFilter, QoS)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suback {
    pub pid: Pid,
    pub subscribes: Vec<SubscribeReturnCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsubscribe {
    pub pid: Pid,
    pub subscribes: Vec<TopicFilter>,
}

impl Subscribe {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        mut remaining_len: usize,
    ) -> Result<Self, Error> {
        let pid = Pid::new(read_u16(reader).await?);
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        let mut subscribes = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string(reader).await?)?;
            let qos = QoS::from_u8(read_u8(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(3 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            subscribes.push((topic_filter, qos));
        }
        if remaining_len != 0 {
            return Err(Error::InvalidRemainingLength);
        }
        Ok(Subscribe { pid, subscribes })
    }
}

impl Encodable for Subscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        for (topic_filter, qos) in &self.subscribes {
            write_bytes(writer, topic_filter.as_bytes())?;
            write_u8(writer, *qos as u8)?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self
            .subscribes
            .iter()
            .map(|(filter, _)| 3 + filter.len())
            .sum::<usize>()
    }
}

impl Suback {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        mut remaining_len: usize,
    ) -> Result<Self, Error> {
        let pid = Pid::new(read_u16(reader).await?);
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        let mut subscribes = Vec::new();
        while remaining_len > 0 {
            let value = read_u8(reader).await?;
            let code = SubscribeReturnCode::from_u8(value)?;
            subscribes.push(code);
            remaining_len -= 1;
        }
        Ok(Suback { pid, subscribes })
    }
}

impl Encodable for Suback {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        for code in &self.subscribes {
            write_u8(writer, *code as u8)?;
        }
        Ok(())
    }
    fn encode_len(&self) -> usize {
        2 + self.subscribes.len()
    }
}

impl Unsubscribe {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        mut remaining_len: usize,
    ) -> Result<Self, Error> {
        let pid = Pid::new(read_u16(reader).await?);
        remaining_len = remaining_len
            .checked_sub(2)
            .ok_or(Error::InvalidRemainingLength)?;
        let mut subscribes = Vec::new();
        while remaining_len > 0 {
            let topic_filter = TopicFilter::try_from(read_string(reader).await?)?;
            remaining_len = remaining_len
                .checked_sub(2 + topic_filter.len())
                .ok_or(Error::InvalidRemainingLength)?;
            subscribes.push(topic_filter);
        }
        if remaining_len != 0 {
            return Err(Error::InvalidRemainingLength);
        }
        Ok(Unsubscribe { pid, subscribes })
    }
}

impl Encodable for Unsubscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        write_u16(writer, self.pid.value())?;
        for topic_filter in &self.subscribes {
            write_bytes(writer, topic_filter.as_bytes())?;
        }
        Ok(())
    }

    fn encode_len(&self) -> usize {
        2 + self
            .subscribes
            .iter()
            .map(|filter| 2 + filter.len())
            .sum::<usize>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
