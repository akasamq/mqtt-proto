use std::convert::TryFrom;
use std::io;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::io::{AsyncRead, AsyncReadExt};

use super::{ErrorV5, UserProperty};
use crate::{
    read_bytes, read_string, read_u16, read_u8, write_bytes, write_u16, write_u8, Encodable, Error,
    Protocol, QoS, TopicName,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe;

impl Subscribe {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suback;

impl Suback {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsubscribe;

impl Unsubscribe {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsuback;

impl Unsuback {
    pub async fn decode_async<T: AsyncRead + Unpin>(
        reader: &mut T,
        remaining_len: usize,
    ) -> Result<Self, ErrorV5> {
        todo!()
    }
}
