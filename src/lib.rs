#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

mod common;
pub mod v3;
pub mod v5;

#[allow(unused_imports)]
pub(crate) use common::{
    block_on, decode_var_int, encode_packet, from_read_exact_error, packet_from, read_bytes,
    read_string, read_u16, read_u32, read_u8, write_bytes, write_string, write_u16, write_u32,
    write_u8, write_var_int, AsyncRead, AsyncWrite, SyncRead, SyncWrite,
};

pub use common::{
    decode_raw_header, header_len, remaining_len, total_len, var_int_len, Encodable, Error,
    GenericPollBodyState, GenericPollPacket, GenericPollPacketState, IoErrorKind, Pid, PollHeader,
    PollHeaderState, Protocol, QoS, QosPid, TopicFilter, TopicName, VarBytes, LEVEL_SEP,
    MATCH_ALL_CHAR, MATCH_ALL_STR, MATCH_ONE_CHAR, MATCH_ONE_STR, SHARED_PREFIX, SYS_PREFIX,
};

// Re-export std & tokio adapter
#[cfg(feature = "std")]
pub use embedded_io_adapters::{
    std::{FromStd, ToStd},
    tokio_1::FromTokio,
};
