#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

mod common;
pub mod v3;
pub mod v5;

#[allow(unused_imports)]
pub(crate) use common::{
    block_on, decode_var_int_async, encode_packet, packet_from, read_bytes_async,
    read_string_async, read_u16_async, read_u32_async, read_u8_async, write_bytes, write_string,
    write_u16, write_u32, write_u8, write_var_int, AsyncRead, AsyncWrite, PacketBuf, SyncRead,
    SyncWrite, ToError,
};

pub use common::{
    decode_raw_header_async, header_len, remaining_len, total_len, var_int_len, ClientId,
    Encodable, Error, GenericPollPacket, GenericPollPacketState, IoErrorKind, Pid, PollHeader,
    Protocol, QoS, QosPid, TopicFilter, TopicName, Username, VarBytes, LEVEL_SEP, MATCH_ALL_CHAR,
    MATCH_ALL_STR, MATCH_ONE_CHAR, MATCH_ONE_STR, SHARED_PREFIX, SYS_PREFIX,
};
