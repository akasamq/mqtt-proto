use std::io;

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Trying to encode/decode an invalid length.
    #[error("invalid length")]
    InvalidLength,

    /// Tried to encode ProcessIdentifier==0.
    #[error("invalid pid to encode")]
    InvalidPid,

    /// Tried to decode a QoS > 2.
    #[error("invalid qos: `{0}`")]
    InvalidQos(u8),

    /// Tried to decode a ConnectReturnCode > 5.
    #[error("invalid connect return code: `{0}`")]
    InvalidConnectReturnCode(u8),

    /// Tried to decode an unknown protocol.
    #[cfg(feature = "std")]
    #[error("invalid protocol: {0}, {1}")]
    InvalidProtocol(String, u8),

    #[cfg(not(feature = "std"))]
    #[error("invalid protocol: {0}, {1}")]
    InvalidProtocol(heapless::String<10>, u8),

    /// Tried to decode an invalid fixed header (packet type, flags, or remaining_length).
    #[error("invalid header")]
    InvalidHeader,

    /// Trying to decode a non-utf8 string.
    #[error("invalid string")]
    InvalidString(#[from] core::str::Utf8Error),

    /// Catch-all error when converting from `std::io::Error`.
    #[error("io error: {0}, {1}")]
    IoError(io::ErrorKind, String),
}

#[cfg(feature = "std")]
impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IoError(err.kind(), err.to_string())
    }
}
