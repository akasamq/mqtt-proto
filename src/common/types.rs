use core::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use core::convert::TryFrom;
use core::hash::{Hash, Hasher};
use core::ops::Deref;

use alloc::sync::Arc;
use alloc::vec::Vec;

use simdutf8::basic::from_utf8;

use crate::{
    write_bytes, write_u8, AsyncRead, Error, PacketBuf, SyncWrite, LEVEL_SEP, MATCH_ALL_CHAR,
    MATCH_ONE_CHAR, SHARED_PREFIX, SYS_PREFIX,
};

use super::{read_bytes_async, read_u8_async};

pub const MQISDP: &[u8] = b"MQIsdp";
pub const MQTT: &[u8] = b"MQTT";

/// The ability of encoding type into write trait, and calculating encoded size.
pub trait Encodable {
    /// Encode type into writer
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error>;
    /// Calculate the encoded size.
    fn encode_len(&self) -> usize;
}

/// Protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum Protocol {
    /// [MQTT 3.1]
    ///
    /// [MQTT 3.1]: https://public.dhe.ibm.com/software/dw/webservices/ws-mqtt/mqtt-v3r1.html
    V310 = 3,

    /// [MQTT 3.1.1] is the most commonly implemented version.
    ///
    /// [MQTT 3.1.1]: https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html
    V311 = 4,

    /// [MQTT 5.0] is the latest version
    ///
    /// [MQTT 5.0]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html
    V500 = 5,
}

impl Protocol {
    pub fn new(name: &[u8], level: u8) -> Result<Protocol, Error> {
        match (name, level) {
            (MQISDP, 3) => Ok(Protocol::V310),
            (MQTT, 4) => Ok(Protocol::V311),
            (MQTT, 5) => Ok(Protocol::V500),
            _ => {
                let name = from_utf8(name).map_err(|_| Error::InvalidString)?;
                Err(Error::InvalidProtocol(name.into(), level))
            }
        }
    }

    pub fn to_pair(self) -> (&'static [u8], u8) {
        match self {
            Self::V310 => (MQISDP, 3),
            Self::V311 => (MQTT, 4),
            Self::V500 => (MQTT, 5),
        }
    }

    pub fn decode(buf: &mut PacketBuf) -> Result<Self, Error> {
        let name_buf = buf.read_bytes()?;
        let level = buf.read_u8()?;
        Protocol::new(&name_buf, level)
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let name_buf = read_bytes_async(reader).await?;
        let level = read_u8_async(reader).await?;
        Protocol::new(&name_buf, level)
    }
}

impl core::fmt::Display for Protocol {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let output = match self {
            Self::V310 => "v3.1",
            Self::V311 => "v3.1.1",
            Self::V500 => "v5.0",
        };
        write!(f, "{output}")
    }
}

impl Encodable for Protocol {
    fn encode<W: SyncWrite>(&self, writer: &mut W) -> Result<(), Error> {
        let (name, level) = self.to_pair();
        write_bytes(writer, name)?;
        write_u8(writer, level)?;
        Ok(())
    }

    fn encode_len(&self) -> usize {
        match self {
            Self::V310 => 2 + 6 + 1,
            Self::V311 => 2 + 4 + 1,
            Self::V500 => 2 + 4 + 1,
        }
    }
}

/// Packet identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Pid(u16);

impl Pid {
    /// Get the `Pid` as a raw `u16`.
    pub fn value(self) -> u16 {
        self.0
    }
}

impl Default for Pid {
    fn default() -> Pid {
        Pid(1)
    }
}

impl TryFrom<u16> for Pid {
    type Error = Error;
    fn try_from(value: u16) -> Result<Self, Error> {
        if value == 0 {
            Err(Error::ZeroPid)
        } else {
            Ok(Pid(value))
        }
    }
}

impl core::ops::Add<u16> for Pid {
    type Output = Pid;

    /// Adding a `u16` to a `Pid` will wrap around and avoid 0.
    fn add(self, u: u16) -> Pid {
        let n = match self.0.overflowing_add(u) {
            (n, false) => n,
            (n, true) => n + 1,
        };
        Pid(n)
    }
}

impl core::ops::AddAssign<u16> for Pid {
    fn add_assign(&mut self, other: u16) {
        *self = *self + other;
    }
}

impl core::ops::Sub<u16> for Pid {
    type Output = Pid;

    /// Subing a `u16` to a `Pid` will wrap around and avoid 0.
    fn sub(self, u: u16) -> Pid {
        let n = match self.0.overflowing_sub(u) {
            (0, _) => u16::MAX,
            (n, false) => n,
            (n, true) => n - 1,
        };
        Pid(n)
    }
}

impl core::ops::SubAssign<u16> for Pid {
    fn sub_assign(&mut self, other: u16) {
        *self = *self - other;
    }
}

/// Packet delivery [Quality of Service] level.
///
/// [Quality of Service]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718099
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum QoS {
    /// `QoS 0`. At most once. No ack needed.
    Level0 = 0,
    /// `QoS 1`. At least once. One ack needed.
    Level1 = 1,
    /// `QoS 2`. Exactly once. Two acks needed.
    Level2 = 2,
}

impl QoS {
    pub fn from_u8(byte: u8) -> Result<QoS, Error> {
        match byte {
            0 => Ok(QoS::Level0),
            1 => Ok(QoS::Level1),
            2 => Ok(QoS::Level2),
            n => Err(Error::InvalidQos(n)),
        }
    }
}

/// Combined [`QoS`] and [`Pid`].
///
/// Used only in [`Publish`] packets.
///
/// [`Publish`]: struct.Publish.html
/// [`QoS`]: enum.QoS.html
/// [`Pid`]: struct.Pid.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum QosPid {
    Level0,
    Level1(Pid),
    Level2(Pid),
}

impl QosPid {
    /// Extract the [`Pid`] from a `QosPid`, if any.
    ///
    /// [`Pid`]: struct.Pid.html
    pub fn pid(self) -> Option<Pid> {
        match self {
            QosPid::Level0 => None,
            QosPid::Level1(p) => Some(p),
            QosPid::Level2(p) => Some(p),
        }
    }

    /// Extract the [`QoS`] from a `QosPid`.
    ///
    /// [`QoS`]: enum.QoS.html
    pub fn qos(self) -> QoS {
        match self {
            QosPid::Level0 => QoS::Level0,
            QosPid::Level1(_) => QoS::Level1,
            QosPid::Level2(_) => QoS::Level2,
        }
    }
}

/// Topic name.
///
/// See [MQTT 4.7]. The internal value is `Arc<str>`.
///
/// [MQTT 4.7]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718106
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct TopicName(Arc<str>);

impl TopicName {
    /// Check if the topic name is invalid.
    pub fn is_invalid(value: &str) -> bool {
        if value.len() > u16::MAX as usize {
            return true;
        }
        value.contains([MATCH_ONE_CHAR, MATCH_ALL_CHAR, '\0'])
    }

    pub fn is_shared(&self) -> bool {
        self.0.starts_with(SHARED_PREFIX)
    }
    pub fn is_sys(&self) -> bool {
        self.0.starts_with(SYS_PREFIX)
    }
}

impl core::fmt::Display for TopicName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for TopicName {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Error> {
        if TopicName::is_invalid(value) {
            Err(Error::InvalidTopicName(value.into()))
        } else {
            Ok(TopicName(value.into()))
        }
    }
}

impl TryFrom<Arc<str>> for TopicName {
    type Error = Error;
    fn try_from(value: Arc<str>) -> Result<Self, Error> {
        if TopicName::is_invalid(&value) {
            Err(Error::InvalidTopicName(value))
        } else {
            Ok(TopicName(value))
        }
    }
}

impl Deref for TopicName {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// Topic filter.
///
/// See [MQTT 4.7]. The internal value is `Arc<str>` and a cache value for
/// where shared filter byte index started. The traits:
/// `Hash`/`Ord`/`PartialOrd`/`Eq`/`PartialEq` are all manually implemented for
/// only contains the string value.
///
/// [MQTT 4.7]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718106
#[derive(Debug, Clone)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct TopicFilter {
    inner: Arc<str>,
    shared_filter_sep: u16,
}

impl TopicFilter {
    /// Check if the topic filter is invalid.
    ///
    ///   * The u16 returned is where the bytes index of '/' char before shared topic filter
    pub fn is_invalid(value: &str) -> (bool, u16) {
        if value.len() > u16::MAX as usize {
            return (true, 0);
        }

        const SHARED_PREFIX_CHARS: [char; 7] = ['$', 's', 'h', 'a', 'r', 'e', '/'];

        // v5.0 [MQTT-4.7.3-1]
        if value.is_empty() {
            return (true, 0);
        }

        let mut last_sep: Option<usize> = None;
        let mut has_all = false;
        let mut has_one = false;
        let mut byte_idx = 0;
        let mut is_shared = true;
        let mut shared_group_sep = 0;
        let mut shared_filter_sep = 0;
        for (char_idx, c) in value.chars().enumerate() {
            if c == '\0' {
                return (true, 0);
            }
            // "#" must be last char
            if has_all {
                return (true, 0);
            }

            if is_shared && char_idx < 7 && c != SHARED_PREFIX_CHARS[char_idx] {
                is_shared = false;
            }

            if c == LEVEL_SEP {
                if is_shared {
                    if shared_group_sep == 0 {
                        shared_group_sep = byte_idx as u16;
                    } else if shared_filter_sep == 0 {
                        shared_filter_sep = byte_idx as u16;
                    }
                }
                // "+" must occupy an entire level of the filter
                if has_one && Some(char_idx) != last_sep.map(|v| v + 2) && char_idx != 1 {
                    return (true, 0);
                }
                last_sep = Some(char_idx);
                has_one = false;
            } else if c == MATCH_ALL_CHAR {
                // v5.0 [MQTT-4.8.2-2]
                if shared_group_sep > 0 && shared_filter_sep == 0 {
                    return (true, 0);
                }
                if has_one {
                    // invalid topic filter: "/+#"
                    return (true, 0);
                } else if Some(char_idx) == last_sep.map(|v| v + 1) || char_idx == 0 {
                    has_all = true;
                } else {
                    // invalid topic filter: "/ab#"
                    return (true, 0);
                }
            } else if c == MATCH_ONE_CHAR {
                // v5.0 [MQTT-4.8.2-2]
                if shared_group_sep > 0 && shared_filter_sep == 0 {
                    return (true, 0);
                }
                if has_one {
                    // invalid topic filter: "/++"
                    return (true, 0);
                } else if Some(char_idx) == last_sep.map(|v| v + 1) || char_idx == 0 {
                    has_one = true;
                } else {
                    return (true, 0);
                }
            }

            byte_idx += c.len_utf8();
        }

        // v5.0 [MQTT-4.7.3-1]
        if shared_filter_sep > 0 && shared_filter_sep as usize == value.len() - 1 {
            return (true, 0);
        }
        // v5.0 [MQTT-4.8.2-2]
        if shared_group_sep > 0 && shared_filter_sep == 0 {
            return (true, 0);
        }
        // v5.0 [MQTT-4.8.2-1]
        if shared_group_sep + 1 == shared_filter_sep {
            return (true, 0);
        }

        debug_assert!(shared_group_sep == 0 || shared_group_sep == 6);

        (false, shared_filter_sep)
    }

    pub fn is_shared(&self) -> bool {
        self.shared_filter_sep > 0
    }
    pub fn is_sys(&self) -> bool {
        self.inner.starts_with(SYS_PREFIX)
    }

    pub fn shared_group_name(&self) -> Option<&str> {
        if self.is_shared() {
            let group_end = self.shared_filter_sep as usize;
            Some(&self.inner[7..group_end])
        } else {
            None
        }
    }

    pub fn shared_filter(&self) -> Option<&str> {
        if self.is_shared() {
            let filter_begin = self.shared_filter_sep as usize + 1;
            Some(&self.inner[filter_begin..])
        } else {
            None
        }
    }

    /// return (shared group name, shared filter)
    pub fn shared_info(&self) -> Option<(&str, &str)> {
        if self.is_shared() {
            let group_end = self.shared_filter_sep as usize;
            let filter_begin = self.shared_filter_sep as usize + 1;
            Some((&self.inner[7..group_end], &self.inner[filter_begin..]))
        } else {
            None
        }
    }
}

impl Hash for TopicFilter {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl Ord for TopicFilter {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl PartialOrd for TopicFilter {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TopicFilter {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl Eq for TopicFilter {}

impl core::fmt::Display for TopicFilter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl TryFrom<&str> for TopicFilter {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (is_invalid, shared_filter_sep) = TopicFilter::is_invalid(value);
        if is_invalid {
            Err(Error::InvalidTopicFilter(value.into()))
        } else {
            Ok(TopicFilter {
                inner: value.into(),
                shared_filter_sep,
            })
        }
    }
}

impl TryFrom<Arc<str>> for TopicFilter {
    type Error = Error;
    fn try_from(value: Arc<str>) -> Result<Self, Self::Error> {
        let (is_invalid, shared_filter_sep) = TopicFilter::is_invalid(&value);
        if is_invalid {
            Err(Error::InvalidTopicFilter(value))
        } else {
            Ok(TopicFilter {
                inner: value,
                shared_filter_sep,
            })
        }
    }
}

impl Deref for TopicFilter {
    type Target = str;
    fn deref(&self) -> &str {
        &self.inner
    }
}

/// A bytes data structure represent a dynamic vector or fixed array.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VarBytes {
    Dynamic(Vec<u8>),
    Fixed2([u8; 2]),
    Fixed4([u8; 4]),
}

impl AsRef<[u8]> for VarBytes {
    /// Return the slice of the internal bytes.
    fn as_ref(&self) -> &[u8] {
        match self {
            VarBytes::Dynamic(vec) => vec,
            VarBytes::Fixed2(arr) => &arr[..],
            VarBytes::Fixed4(arr) => &arr[..],
        }
    }
}

/// The [client identifier](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901059).
pub type ClientId = Arc<str>;

/// The [user name](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901071).
pub type Username = Arc<str>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_add_sub() {
        let t: Vec<(u16, u16, u16, u16)> = alloc::vec![
            (2, 1, 1, 3),
            (100, 1, 99, 101),
            (1, 1, u16::MAX, 2),
            (1, 2, u16::MAX - 1, 3),
            (1, 3, u16::MAX - 2, 4),
            (u16::MAX, 1, u16::MAX - 1, 1),
            (u16::MAX, 2, u16::MAX - 2, 2),
            (10, u16::MAX, 10, 10),
            (10, 0, 10, 10),
            (1, 0, 1, 1),
            (u16::MAX, 0, u16::MAX, u16::MAX),
        ];
        for (cur, d, prev, next) in t {
            let cur = Pid::try_from(cur).unwrap();
            let sub = cur - d;
            let add = cur + d;
            assert_eq!(prev, sub.value(), "{cur:?} - {d} should be {prev}");
            assert_eq!(next, add.value(), "{cur:?} + {d} should be {next}");
        }
    }

    #[test]
    fn test_valid_topic_name() {
        // valid topic name
        assert!(!TopicName::is_invalid("/abc/def"));
        assert!(!TopicName::is_invalid("abc/def"));
        assert!(!TopicName::is_invalid("abc"));
        assert!(!TopicName::is_invalid("/"));
        assert!(!TopicName::is_invalid("//"));
        // NOTE: Because v5.0 topic alias, we let up level to check empty topic name
        assert!(!TopicName::is_invalid(""));
        assert!(!TopicName::is_invalid(
            "a".repeat(u16::MAX as usize).as_str()
        ));

        // invalid topic name
        assert!(TopicName::is_invalid("#"));
        assert!(TopicName::is_invalid("+"));
        assert!(TopicName::is_invalid("/+"));
        assert!(TopicName::is_invalid("/#"));
        assert!(TopicName::is_invalid("abc/\0"));
        assert!(TopicName::is_invalid("abc\0def"));
        assert!(TopicName::is_invalid("abc#def"));
        assert!(TopicName::is_invalid("abc+def"));
        assert!(TopicName::is_invalid(
            "a".repeat(u16::MAX as usize + 1).as_str()
        ));
    }

    #[test]
    fn test_valid_topic_filter() {
        let string_65535 = "a".repeat(u16::MAX as usize);
        let string_65536 = "a".repeat(u16::MAX as usize + 1);
        for (is_invalid, topic) in [
            // valid topic filter
            (false, "abc/def"),
            (false, "abc/+"),
            (false, "abc/#"),
            (false, "#"),
            (false, "+"),
            (false, "+/"),
            (false, "+/+"),
            (false, "///"),
            (false, "//+/"),
            (false, "//abc/"),
            (false, "//+//#"),
            (false, "/abc/+//#"),
            (false, "+/abc/+"),
            (false, string_65535.as_str()),
            // invalid topic filter
            (true, ""),
            (true, "abc\0def"),
            (true, "abc/\0def"),
            (true, "++"),
            (true, "++/"),
            (true, "/++"),
            (true, "abc/++"),
            (true, "abc/++/"),
            (true, "#/abc"),
            (true, "/ab#"),
            (true, "##"),
            (true, "/abc/ab#"),
            (true, "/+#"),
            (true, "//+#"),
            (true, "/abc/+#"),
            (true, "xxx/abc/+#"),
            (true, "xxx/a+bc/"),
            (true, "x+x/abc/"),
            (true, "x+/abc/"),
            (true, "+x/abc/"),
            (true, "+/abc/++"),
            (true, "+/a+c/+"),
            (true, string_65536.as_str()),
        ] {
            assert_eq!((is_invalid, 0), TopicFilter::is_invalid(topic));
        }
    }

    #[test]
    fn test_valid_shared_topic_filter() {
        for (is_invalid, topic) in [
            // valid topic filter
            (false, "abc/def"),
            (false, "abc/+"),
            (false, "abc/#"),
            (false, "#"),
            (false, "+"),
            (false, "+/"),
            (false, "+/+"),
            (false, "///"),
            (false, "//+/"),
            (false, "//abc/"),
            (false, "//+//#"),
            (false, "/abc/+//#"),
            (false, "+/abc/+"),
            // invalid topic filter
            (true, "abc\0def"),
            (true, "abc/\0def"),
            (true, "++"),
            (true, "++/"),
            (true, "/++"),
            (true, "abc/++"),
            (true, "abc/++/"),
            (true, "#/abc"),
            (true, "/ab#"),
            (true, "##"),
            (true, "/abc/ab#"),
            (true, "/+#"),
            (true, "//+#"),
            (true, "/abc/+#"),
            (true, "xxx/abc/+#"),
            (true, "xxx/a+bc/"),
            (true, "x+x/abc/"),
            (true, "x+/abc/"),
            (true, "+x/abc/"),
            (true, "+/abc/++"),
            (true, "+/a+c/+"),
        ] {
            let result = if is_invalid { (true, 0) } else { (false, 10) };
            assert_eq!(
                result,
                TopicFilter::is_invalid(alloc::format!("$share/xyz/{topic}").as_str()),
            );
        }

        for (result, raw_filter) in [
            (Some((None, None)), "$abc/a/b"),
            (Some((None, None)), "$abc/a/b/xyz/def"),
            (Some((None, None)), "$sys/abc"),
            (Some((Some("abc"), Some("xyz"))), "$share/abc/xyz"),
            (Some((Some("abc"), Some("xyz/ijk"))), "$share/abc/xyz/ijk"),
            (Some((Some("abc"), Some("/xyz"))), "$share/abc//xyz"),
            (Some((Some("abc"), Some("/#"))), "$share/abc//#"),
            (Some((Some("abc"), Some("/a/x/+"))), "$share/abc//a/x/+"),
            (Some((Some("abc"), Some("+"))), "$share/abc/+"),
            (Some((Some("你好"), Some("+"))), "$share/你好/+"),
            (Some((Some("你好"), Some("你好"))), "$share/你好/你好"),
            (Some((Some("abc"), Some("#"))), "$share/abc/#"),
            (Some((Some("abc"), Some("#"))), "$share/abc/#"),
            (None, "$share/abc/"),
            (None, "$share/abc"),
            (None, "$share/+/y"),
            (None, "$share/+/+"),
            (None, "$share//y"),
            (None, "$share//+"),
        ] {
            if let Some((shared_group, shared_filter)) = result {
                let filter = TopicFilter::try_from(raw_filter).unwrap();
                assert_eq!(filter.shared_group_name(), shared_group);
                assert_eq!(filter.shared_filter(), shared_filter);
                if let Some(group_name) = shared_group {
                    assert_eq!(
                        filter.shared_info(),
                        Some((group_name, shared_filter.unwrap()))
                    );
                }
            } else {
                assert_eq!((true, 0), TopicFilter::is_invalid(raw_filter));
            }
        }
    }
}
