use std::io;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt};

use crate::{
    read_string, read_u16, write_bytes, write_u16, Encodable, Error, Header, Pid, QoS, QosPid,
    TopicName,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publish {
    pub dup: bool,
    pub qos_pid: QosPid,
    pub retain: bool,
    pub topic_name: TopicName,
    pub payload: Bytes,
}

impl Publish {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, Error> {
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
        let payload = if remaining_len > 0 {
            let mut data = vec![0u8; remaining_len];
            reader.read_exact(&mut data).await?;
            data
        } else {
            Vec::new()
        };
        Ok(Publish {
            dup: header.dup,
            qos_pid,
            retain: header.retain,
            topic_name: TopicName::try_from(topic_name)?,
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
        write_bytes(writer, self.payload.as_ref())?;
        Ok(())
    }

    fn encode_len(&self) -> usize {
        let mut length = 2 + self.topic_name.len();
        match self.qos_pid {
            QosPid::Level0 => {}
            QosPid::Level1(_) | QosPid::Level2(_) => {
                length += 2;
            }
        }
        length += self.payload.len();
        length
    }
}
