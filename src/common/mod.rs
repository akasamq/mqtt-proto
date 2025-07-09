mod error;
mod poll;
mod types;
mod utils;

pub(crate) use embedded_io::{Read as SyncRead, Write as SyncWrite};
pub(crate) use embedded_io_async::{Read as AsyncRead, Write as AsyncWrite};

#[cfg(feature = "std")]
pub(crate) use futures_lite::future::block_on;

#[cfg(not(feature = "std"))]
pub(crate) use embassy_futures::block_on;

pub(crate) use utils::{
    decode_var_int, encode_packet, packet_from, read_bytes, read_string, read_u16, read_u32,
    read_u8, write_bytes, write_string, write_u16, write_u32, write_u8, write_var_int,
};

pub use error::{Error, IoErrorKind};
pub use poll::{
    GenericPollBodyState, GenericPollPacket, GenericPollPacketState, PollHeader, PollHeaderState,
};
pub use types::{Encodable, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName, VarBytes};
pub use utils::{decode_raw_header, header_len, remaining_len, total_len, var_int_len};

/// Character used to separate each level within a topic tree and provide a hierarchical structure.
pub const LEVEL_SEP: char = '/';
/// Wildcard character that matches only one topic level.
pub const MATCH_ONE_CHAR: char = '+';
/// Wildcard character that matches any number of levels within a topic.
pub const MATCH_ALL_CHAR: char = '#';
/// The &str version of `MATCH_ONE_CHAR`
pub const MATCH_ONE_STR: &str = "+";
/// The &str version of `MATCH_ALL_CHAR`
pub const MATCH_ALL_STR: &str = "#";

/// System topic prefix
pub const SYS_PREFIX: &str = "$SYS/";
/// Shared topic prefix
pub const SHARED_PREFIX: &str = "$share/";
