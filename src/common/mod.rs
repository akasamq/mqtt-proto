mod error;
mod poll;
mod types;
mod utils;

#[cfg(test)]
mod tests;

mod future {
    #[cfg(not(any(feature = "std", feature = "embassy")))]
    compile_error!("embassy feature is required when std is disabled");

    #[cfg(all(feature = "std", not(feature = "embassy")))]
    pub(crate) use futures_lite::future::block_on;

    #[cfg(feature = "embassy")]
    pub(crate) use embassy_futures::block_on;
}

mod io {
    #[cfg(not(any(feature = "std", feature = "embedded-io")))]
    compile_error!("embedded-io feature is required when std is disabled");

    #[cfg(not(any(feature = "embedded-io", all(feature = "tokio", feature = "std"))))]
    compile_error!("at least one async I/O implementation required: embedded-io or (tokio + std)");

    #[cfg(all(feature = "std", not(feature = "embedded-io")))]
    pub(crate) use std::io::{Read as SyncRead, Write as SyncWrite};

    #[cfg(feature = "embedded-io")]
    pub(crate) use embedded_io::{Read as SyncRead, Write as SyncWrite};

    #[cfg(all(feature = "embedded-io", not(all(feature = "tokio", feature = "std"))))]
    pub(crate) use embedded_io_async::{Read as AsyncRead, Write as AsyncWrite}; // Stateful style async trait

    #[cfg(all(feature = "tokio", feature = "std"))]
    pub(crate) use tokio::io::{AsyncRead, AsyncWrite}; // Futures style async trait
}

pub(crate) use future::block_on;
pub(crate) use io::{AsyncRead, AsyncWrite, SyncRead, SyncWrite};
pub(crate) use utils::{
    decode_var_int, encode_packet, packet_from, read_bytes, read_string, read_u16, read_u32,
    read_u8, write_bytes, write_string, write_u16, write_u32, write_u8, write_var_int,
};

pub use error::{Error, IoErrorKind, ToError};
pub use poll::{
    GenericPollBodyState, GenericPollPacket, GenericPollPacketState, PollHeader, PollHeaderState,
};
pub use types::{
    ClientId, Encodable, Pid, Protocol, QoS, QosPid, TopicFilter, TopicName, Username, VarBytes,
};
pub use utils::{decode_raw_header, header_len, remaining_len, total_len, var_int_len};

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
