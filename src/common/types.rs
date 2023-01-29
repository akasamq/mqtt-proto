use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::ops::Deref;
use std::slice;
use std::sync::Arc;

use futures_lite::io::AsyncRead;

use super::{read_bytes, read_u8};
use crate::Error;

pub const MQISDP: &[u8] = b"MQIsdp";
pub const MQTT: &[u8] = b"MQTT";

/// The ability of encoding type into `io::Write`, and calculating encoded size.
pub trait Encodable {
    /// Encode type into `io::Write`
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;
    /// Calculate the encoded size.
    fn encode_len(&self) -> usize;
}

/// Protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// [MQTT 3.1]
    ///
    /// [MQTT 3.1]: https://public.dhe.ibm.com/software/dw/webservices/ws-mqtt/mqtt-v3r1.html
    MqttV31 = 3,

    /// [MQTT 3.1.1] is the most commonly implemented version.
    ///
    /// [MQTT 3.1.1]: https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html
    MqttV311 = 4,

    /// [MQTT 5.0] is the latest version
    ///
    /// [MQTT 5.0]: https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html
    MqttV50 = 5,
}

impl Protocol {
    pub fn new(name: &[u8], level: u8) -> Result<Protocol, Error> {
        match (name, level) {
            (MQISDP, 3) => Ok(Protocol::MqttV31),
            (MQTT, 4) => Ok(Protocol::MqttV311),
            (MQTT, 5) => Ok(Protocol::MqttV50),
            _ => {
                let name = core::str::from_utf8(name)?;
                Err(Error::InvalidProtocol(name.into(), level))
            }
        }
    }

    pub fn to_pair(self) -> (&'static [u8], u8) {
        match self {
            Self::MqttV31 => (MQISDP, 3),
            Self::MqttV311 => (MQTT, 4),
            Self::MqttV50 => (MQTT, 5),
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let name_buf = read_bytes(reader).await?;
        let level = read_u8(reader).await?;
        Protocol::new(&name_buf, level)
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self {
            Self::MqttV31 => "v3.1",
            Self::MqttV311 => "v3.1.1",
            Self::MqttV50 => "v5.0",
        };
        write!(f, "{}", output)
    }
}

impl Encodable for Protocol {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        let (name, level) = self.to_pair();
        writer.write_all(&(name.len() as u16).to_be_bytes())?;
        writer.write_all(name)?;
        writer.write_all(slice::from_ref(&level))?;
        Ok(())
    }

    fn encode_len(&self) -> usize {
        match self {
            Self::MqttV31 => 2 + 6 + 1,
            Self::MqttV311 => 2 + 4 + 1,
            Self::MqttV50 => 2 + 4 + 1,
        }
    }
}

/// Packet identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
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
            Err(Error::InvalidPid)
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

impl core::ops::Sub<u16> for Pid {
    type Output = Pid;

    /// Subing a `u16` to a `Pid` will wrap around and avoid 0.
    fn sub(self, u: u16) -> Pid {
        let n = match self.0.overflowing_sub(u) {
            (0, _) => core::u16::MAX,
            (n, false) => n,
            (n, true) => n - 1,
        };
        Pid(n)
    }
}

/// Packet delivery [Quality of Service] level.
///
/// [Quality of Service]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718099
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QoS {
    /// `QoS 0`. At most once. No ack needed.
    Level0 = 0,
    /// `QoS 1`. At least once. One ack needed.
    Level1 = 1,
    /// `QoS 2`. Exactly once. Two acks needed.
    Level2 = 2,
}

impl QoS {
    pub(crate) fn from_u8(byte: u8) -> Result<QoS, Error> {
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
/// See [MQTT 4.7]. The internal value is `Arc<String>`.
///
/// [MQTT 4.7]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718106
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicName(Arc<String>);

impl TryFrom<String> for TopicName {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        // FIXME: check topic name
        Ok(TopicName(Arc::new(value)))
    }
}
impl Deref for TopicName {
    type Target = str;
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

/// Topic filter.
///
/// See [MQTT 4.7]. The internal value is `Arc<String>`.
///
/// [MQTT 4.7]: http://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc398718106
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicFilter(Arc<String>);

impl TryFrom<String> for TopicFilter {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        // FIXME: check topic filter
        Ok(TopicFilter(Arc::new(value)))
    }
}
impl Deref for TopicFilter {
    type Target = str;
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_add_sub() {
        let t: Vec<(u16, u16, u16, u16)> = vec![
            (2, 1, 1, 3),
            (100, 1, 99, 101),
            (1, 1, core::u16::MAX, 2),
            (1, 2, core::u16::MAX - 1, 3),
            (1, 3, core::u16::MAX - 2, 4),
            (core::u16::MAX, 1, core::u16::MAX - 1, 1),
            (core::u16::MAX, 2, core::u16::MAX - 2, 2),
            (10, core::u16::MAX, 10, 10),
            (10, 0, 10, 10),
            (1, 0, 1, 1),
            (core::u16::MAX, 0, core::u16::MAX, core::u16::MAX),
        ];
        for (cur, d, prev, next) in t {
            let cur = Pid::try_from(cur).unwrap();
            let sub = cur - d;
            let add = cur + d;
            assert_eq!(prev, sub.value(), "{:?} - {} should be {}", cur, d, prev);
            assert_eq!(next, add.value(), "{:?} + {} should be {}", cur, d, next);
        }
    }
}
