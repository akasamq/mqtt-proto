use std::io;

use thiserror::Error;

/// Errors returned by encoding and decoding process.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Trying to decode with an invalid remaining length.
    #[error("invalid remaining length")]
    InvalidRemainingLength,

    /// No subscription in subscribe packet
    #[error("empty subscription")]
    EmptySubscription,

    // /// Tried to encode ProcessIdentifier==0.
    // #[error("invalid pid to encode")]
    // InvalidPid,
    /// Tried to decode a QoS > 2.
    #[error("invalid qos: `{0}`")]
    InvalidQos(u8),

    /// Tired to decode connect flags, that will flag is 0 but will qos is not 0.
    #[error("invalid connect flags: `{0}`")]
    InvalidConnectFlags(u8),

    /// Tired to decode connack flags not 0 or 1.
    #[error("invalid connack flags: `{0}`")]
    InvalidConnackFlags(u8),

    /// Tried to decode a ConnectReturnCode > 5.
    #[error("invalid connect return code: `{0}`")]
    InvalidConnectReturnCode(u8),

    /// Tried to decode an unknown protocol.
    #[error("invalid protocol: {0}, {1}")]
    InvalidProtocol(String, u8),

    /// Tried to decode an invalid fixed header (packet type, flags, or remaining_length).
    #[error("invalid header")]
    InvalidHeader,

    #[error("invalid variable byte integer")]
    InvalidVarByteInt,

    /// Trying to decode a non-utf8 string.
    #[error("invalid string")]
    InvalidString(#[from] core::str::Utf8Error),

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
