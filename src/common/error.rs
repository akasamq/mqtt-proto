use std::io;

use thiserror::Error;

use crate::Protocol;

/// Errors returned by encoding and decoding process.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Invalid remaining length.
    #[error("invalid remaining length")]
    InvalidRemainingLength,

    /// No subscription in subscribe packet.
    #[error("empty subscription")]
    EmptySubscription,

    /// Packet identifier is 0.
    #[error("packet identifier is 0")]
    ZeroPid,

    /// Invalid QoS value.
    #[error("invalid qos: `{0}`")]
    InvalidQos(u8),

    /// Invalid connect flags.
    #[error("invalid connect flags: `{0}`")]
    InvalidConnectFlags(u8),

    /// Invalid connack flags (not 0 or 1).
    #[error("invalid connack flags: `{0}`")]
    InvalidConnackFlags(u8),

    /// Invalid connect return code (value > 5).
    #[error("invalid connect return code: `{0}`")]
    InvalidConnectReturnCode(u8),

    /// Invalid protocol.
    #[error("invalid protocol: {0}, {1}")]
    InvalidProtocol(String, u8),

    /// Unexpected protocol
    #[error("unexpected protocol version: `{0}`")]
    UnexpectedProtocol(Protocol),

    /// Invalid fixed header (packet type, flags, or remaining_length).
    #[error("invalid header")]
    InvalidHeader,

    /// Invalid variable byte integer, the value MUST smaller than `268,435,456`.
    #[error("invalid variable byte integer")]
    InvalidVarByteInt,

    /// Invalid Topic Name
    #[error("invalid topic name: {0}")]
    InvalidTopicName(String),

    /// Invalid topic filter
    #[error("invalid topic filter: {0}")]
    InvalidTopicFilter(String),

    /// Trying to decode a non-utf8 string.
    #[error("invalid string")]
    InvalidString,

    /// Catch-all error when converting from `std::io::Error`.
    #[error("io error: {0}, {1}")]
    IoError(io::ErrorKind, String),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IoError(err.kind(), err.to_string())
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        match err {
            Error::IoError(kind, _info) => kind.into(),
            _ => io::ErrorKind::InvalidData.into(),
        }
    }
}
