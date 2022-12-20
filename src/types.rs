use core::{convert::TryFrom, fmt, num::NonZeroU16};
use std::io;
use std::sync::Arc;

use futures_lite::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::Error;

pub trait Encodable {
    fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()>;
    fn encode_len(&self) -> usize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Pid(NonZeroU16);
impl Pid {
    /// Returns a new `Pid` with value `1`.
    pub fn new() -> Self {
        Pid(NonZeroU16::new(1).unwrap())
    }

    /// Get the `Pid` as a raw `u16`.
    pub fn get(self) -> u16 {
        self.0.get()
    }

    pub async fn decode<T: AsyncRead + Unpin>(reader: &mut T) -> Result<Self, Error> {
        let mut pid_buf = [0u8; 2];
        reader.read_exact(&mut pid_buf).await?;
        Pid::try_from(u16::from_be_bytes(pid_buf))
    }

    pub fn encode<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.get().to_be_bytes())
    }
    pub const fn encode_len(self) -> usize {
        2
    }
}
impl Default for Pid {
    fn default() -> Pid {
        Pid::new()
    }
}

impl core::ops::Add<u16> for Pid {
    type Output = Pid;

    /// Adding a `u16` to a `Pid` will wrap around and avoid 0.
    fn add(self, u: u16) -> Pid {
        let n = match self.get().overflowing_add(u) {
            (n, false) => n,
            (n, true) => n + 1,
        };
        Pid(NonZeroU16::new(n).unwrap())
    }
}

impl core::ops::Sub<u16> for Pid {
    type Output = Pid;

    /// Subing a `u16` to a `Pid` will wrap around and avoid 0.
    fn sub(self, u: u16) -> Pid {
        let n = match self.get().overflowing_sub(u) {
            (0, _) => core::u16::MAX,
            (n, false) => n,
            (n, true) => n - 1,
        };
        Pid(NonZeroU16::new(n).unwrap())
    }
}

impl From<Pid> for u16 {
    /// Convert `Pid` to `u16`.
    fn from(p: Pid) -> Self {
        p.0.get()
    }
}

impl TryFrom<u16> for Pid {
    type Error = Error;

    /// Convert `u16` to `Pid`. Will fail for value 0.
    fn try_from(u: u16) -> Result<Self, Error> {
        match NonZeroU16::new(u) {
            Some(nz) => Ok(Pid(nz)),
            None => Err(Error::InvalidPid),
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicName(Arc<String>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicFilter(Arc<String>);
