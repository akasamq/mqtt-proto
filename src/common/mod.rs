mod error;
mod types;
mod utils;

pub(crate) use utils::{
    decode_raw_header, decode_var_int, encode_packet, packet_from, read_bytes, read_string,
    read_u16, read_u32, read_u8, total_len, var_int_len, write_bytes, write_u16, write_u32,
    write_u8, write_var_int,
};

pub use error::Error;
pub use types::{Encodable, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName};

pub const MATCH_ONE_STR: &str = "+";
pub const MATCH_ALL_STR: &str = "#";
pub const MATCH_ONE_CHAR: char = '+';
pub const MATCH_ALL_CHAR: char = '#';
pub const LEVEL_SEP: char = '/';
