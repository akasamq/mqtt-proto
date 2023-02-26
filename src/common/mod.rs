mod error;
mod poll;
mod types;
mod utils;

pub(crate) use utils::{
    decode_var_int, encode_packet, packet_from, read_bytes, read_string, read_u16, read_u32,
    read_u8, total_len, var_int_len, write_bytes, write_u16, write_u32, write_u8, write_var_int,
};

pub use error::Error;
pub use poll::{
    GenericPollPacket, GenericPollPacketState, GenericPollPayloadState, PollHeader, PollHeaderState,
};
pub use types::{Encodable, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName};
pub use utils::decode_raw_header;

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
