use thiserror::Error;

use super::{PacketType, PropertyId};

/// MQTT v5.0 errors returned by encoding and decoding process.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ErrorV5 {
    /// Common error of MQTT v3 and v5.
    #[error("common error of v3/v5: {0}")]
    Common(#[from] crate::Error),

    /// Invalid reason code.
    #[error("invalid reason code `{1}` for packet `{0}`")]
    InvalidReasonCode(PacketType, u8),

    /// Invalid subscription option.
    #[error("invalid subscription option: `{0}`")]
    InvalidSubscriptionOption(u8),

    /// Invalid payload format, utf8 expected
    #[error("invalid payload format, utf8 expected")]
    InvalidPayloadFormat,

    /// Invalid response topic name
    #[error("invalid response topic")]
    InvalidResponseTopic,

    /// Invalid property identifier.
    #[error("invalid property identifier: `{0}`")]
    InvalidPropertyId(u8),

    /// Invalid property length.
    #[error("invalid property length: `{0}`")]
    InvalidPropertyLength(u32),

    /// Invalid byte property value.
    #[error("invalid byte value `{1}` for property `{0}`")]
    InvalidByteProperty(PropertyId, u8),

    /// Duplicated property.
    #[error("duplicated property: `{0}`")]
    DuplicatedProperty(PropertyId),

    /// Invalid property.
    #[error("invalid property `{1}` for packet `{0}`")]
    InvalidProperty(PacketType, PropertyId),

    /// Invalid will property (connect packet).
    #[error("invalid will property: `{0}`")]
    InvalidWillProperty(PropertyId),
}

impl ErrorV5 {
    pub fn is_eof(&self) -> bool {
        match self {
            ErrorV5::Common(err) => err.is_eof(),
            _ => false,
        }
    }
}
