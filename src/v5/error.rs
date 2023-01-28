use thiserror::Error;

use super::{PacketType, PropertyType};
use crate::Protocol;

/// MQTT v5.0 errors returned by encoding and decoding process.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ErrorV5 {
    #[error("common error of v3/v5: {0}")]
    Common(#[from] crate::Error),

    #[error("unexpected protocol version: `{0}`, expected `v5.0`")]
    UnexpectedProtocol(Protocol),

    #[error("invalid CONNECT reason code: `{0}`")]
    InvalidConnectReasonCode(u8),

    #[error("invalid DISCONNECT reason code: `0`")]
    InvalidDisconnectReasonCode(u8),

    #[error("invalid AUTH reason code: `0`")]
    InvalidAuthReasonCode(u8),

    #[error("invalid PUBACK reason code: `0`")]
    InvalidPubackReasonCode(u8),

    #[error("invalid PUBREC reason code: `0`")]
    InvalidPubrecReasonCode(u8),

    #[error("invalid PUBREL reason code: `0`")]
    InvalidPubrelReasonCode(u8),

    #[error("invalid PUBCOMP reason code: `0`")]
    InvalidPubcompReasonCode(u8),

    #[error("invalid property type: `{0}`")]
    InvalidPropertyType(u8),

    #[error("invalid byte property value: type=`{0}`, value=`{1}`")]
    InvalidBytePropertyValue(PropertyType, u8),

    #[error("duplicated property: `{0}`")]
    DuplicatedProperty(PropertyType),

    #[error("invlaid property `{0}` for packet `{0}`")]
    InvalidProperty(PropertyType, PacketType),

    #[error("invalid will property: `{0}`")]
    InvalidWillProperty(PropertyType),

    #[error("Authentication Data exists but Authentication Method is mssing")]
    AuthMethodMissing,
}
