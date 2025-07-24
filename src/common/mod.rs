mod error;
mod poll;
mod types;
mod utils;

#[cfg(test)]
mod tests;

pub(crate) mod future {
    #[cfg(feature = "std")]
    pub use futures_lite::future::block_on;

    #[cfg(not(feature = "std"))]
    pub use embassy_futures::block_on;
}

pub(crate) mod io {
    #[cfg(feature = "std")]
    pub use std::io::{Read as SyncRead, Write as SyncWrite};

    #[cfg(not(feature = "std"))]
    pub use embedded_io::{Read as SyncRead, Write as SyncWrite};

    #[cfg(feature = "tokio")]
    pub use tokio::io::{AsyncRead, AsyncWrite};

    #[cfg(not(feature = "tokio"))]
    pub use embedded_io_async::{Read as AsyncRead, Write as AsyncWrite};
}

pub(crate) use future::block_on;
pub(crate) use io::{AsyncRead, AsyncWrite, SyncRead, SyncWrite};
pub(crate) use utils::{
    decode_var_int_async, encode_packet, packet_from, read_bytes_async, read_string_async,
    read_u16_async, read_u32_async, read_u8_async, write_bytes, write_string, write_u16, write_u32,
    write_u8, write_var_int, PacketBuf,
};

pub use error::{Error, IoErrorKind, ToError};
pub use poll::{GenericPollPacket, GenericPollPacketState, PollHeader};
pub use types::{
    ClientId, Encodable, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName, Username, VarBytes,
};
pub use utils::{decode_raw_header_async, header_len, remaining_len, total_len, var_int_len};

#[cfg(all(test, feature = "dhat-heap"))]
pub use tests::MemorySummary;

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
