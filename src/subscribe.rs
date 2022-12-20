use std::io;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Encodable, Error, QoS, TopicName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suback {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsubscribe {}

impl Subscribe {
    pub async fn decode<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, Error> {
        todo!()
    }
}
impl Encodable for Subscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        todo!()
    }
    fn encode_len(&self) -> usize {
        todo!()
    }
}

impl Suback {
    pub async fn decode<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, Error> {
        todo!()
    }
}
impl Encodable for Suback {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        todo!()
    }
    fn encode_len(&self) -> usize {
        todo!()
    }
}

impl Unsubscribe {
    pub async fn decode<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, Error> {
        todo!()
    }
}

impl Encodable for Unsubscribe {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        todo!()
    }
    fn encode_len(&self) -> usize {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeReturnCode {
    MaxLevel0,
    MaxLevel1,
    MaxLevel2,
    Failure,
}
