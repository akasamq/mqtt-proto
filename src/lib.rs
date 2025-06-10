#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

mod common;
pub mod v3;
//pub mod v5;

pub(crate) use embedded_io_async::{Read as AsyncRead, Write as AsyncWrite};

#[cfg(feature = "std")]
pub(crate) use futures_lite::future::block_on;

#[cfg(not(feature = "std"))]
pub(crate) use embassy_futures::block_on;

pub(crate) use common::{
    decode_var_int, encode_packet, packet_from, read_bytes, read_string, read_u16, read_u32,
    read_u8, write_bytes, write_u16, write_u32, write_u8, write_var_int, WriteAll,
};

pub use common::{
    decode_raw_header, header_len, remaining_len, total_len, var_int_len, Encodable, Error,
    GenericPollBodyState, GenericPollPacket, GenericPollPacketState, IoErrorKind, Pid, PollHeader,
    PollHeaderState, Protocol, QoS, QosPid, TopicFilter, TopicName, VarBytes, LEVEL_SEP,
    MATCH_ALL_CHAR, MATCH_ALL_STR, MATCH_ONE_CHAR, MATCH_ONE_STR, SHARED_PREFIX, SYS_PREFIX,
};
