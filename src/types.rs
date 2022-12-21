use std::convert::TryFrom;
use std::io;
use std::ops::Deref;
use std::sync::Arc;

use crate::Error;

pub trait Encodable {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;
    fn encode_len(&self) -> usize;
}

/// Packet identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Pid(u16);

impl Pid {
    pub fn new(value: u16) -> Self {
        Pid(value)
    }

    /// Get the `Pid` as a raw `u16`.
    pub fn value(self) -> u16 {
        self.0
    }

    /// Pid should be non-zero value
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl Default for Pid {
    fn default() -> Pid {
        Pid(1)
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
            let cur = Pid::new(cur);
            assert!(cur.is_valid());
            let sub = cur - d;
            let add = cur + d;
            assert_eq!(prev, sub.value(), "{:?} - {} should be {}", cur, d, prev);
            assert_eq!(next, add.value(), "{:?} + {} should be {}", cur, d, next);
        }
    }
}
