use std::io;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Encodable, Error, Header, QosPid, TopicName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publish {
    pub dup: bool,
    pub qos_pid: QosPid,
    pub retain: bool,
    pub topic_name: TopicName,
    pub payload: Bytes,
}

impl Publish {
    pub async fn decode<T: AsyncRead + Unpin>(
        reader: &mut T,
        header: Header,
    ) -> Result<Self, Error> {
        todo!()
    }
}

impl Encodable for Publish {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        todo!()
    }
    fn encode_len(&self) -> usize {
        todo!()
    }
}
