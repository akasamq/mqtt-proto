use alloc::sync::Arc;

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
    InvalidProtocol(Arc<str>, u8),

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
    InvalidTopicName(Arc<str>),

    /// Invalid topic filter
    #[error("invalid topic filter: {0}")]
    InvalidTopicFilter(Arc<str>),

    /// Trying to decode a non-utf8 string.
    #[error("invalid string")]
    InvalidString,

    /// Catch-all error when converting from `io::Error`.
    #[error("io error: {0:?}")]
    IoError(IoErrorKind),
}

/// IoErrorKind for both std and no-std environments
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoErrorKind {
    UnexpectedEof,
    InvalidData,
    WriteZero,
    TimedOut,
    Other,
}

impl Error {
    pub fn is_eof(&self) -> bool {
        matches!(self, Error::IoError(IoErrorKind::UnexpectedEof))
    }
}

impl<E: embedded_io::Error> From<E> for Error {
    fn from(err: E) -> Error {
        match err.kind() {
            embedded_io::ErrorKind::InvalidData => Error::IoError(IoErrorKind::InvalidData),
            embedded_io::ErrorKind::WriteZero => Error::IoError(IoErrorKind::WriteZero),
            embedded_io::ErrorKind::TimedOut => Error::IoError(IoErrorKind::TimedOut),
            _ => Error::IoError(IoErrorKind::Other),
        }
    }
}

pub trait ToError {
    fn to_error(self) -> Error;
}

impl<E: Into<Error>> ToError for embedded_io::ReadExactError<E> {
    fn to_error(self) -> Error {
        match self {
            embedded_io::ReadExactError::UnexpectedEof => {
                Error::IoError(IoErrorKind::UnexpectedEof)
            }
            embedded_io::ReadExactError::Other(e) => e.into(),
        }
    }
}

#[cfg(feature = "std")]
impl ToError for std::io::Error {
    fn to_error(self) -> Error {
        match self.kind() {
            std::io::ErrorKind::UnexpectedEof => Error::IoError(IoErrorKind::UnexpectedEof),
            std::io::ErrorKind::InvalidData => Error::IoError(IoErrorKind::InvalidData),
            std::io::ErrorKind::WriteZero => Error::IoError(IoErrorKind::WriteZero),
            std::io::ErrorKind::TimedOut => Error::IoError(IoErrorKind::TimedOut),
            _ => Error::IoError(IoErrorKind::Other),
        }
    }
}

#[cfg(feature = "std")]
impl From<Error> for std::io::Error {
    fn from(err: Error) -> std::io::Error {
        match err {
            Error::IoError(IoErrorKind::UnexpectedEof) => {
                std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "unexpected eof")
            }
            Error::IoError(IoErrorKind::InvalidData) => {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid data")
            }
            Error::IoError(IoErrorKind::WriteZero) => {
                std::io::Error::new(std::io::ErrorKind::WriteZero, "write zero")
            }
            Error::IoError(IoErrorKind::TimedOut) => {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out")
            }
            Error::IoError(IoErrorKind::Other) => std::io::Error::other("other error"),
            _ => std::io::Error::new(std::io::ErrorKind::InvalidData, "mqtt protocol error"),
        }
    }
}
