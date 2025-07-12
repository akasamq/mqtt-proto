mod poll;

use serde::{Deserialize, Serialize, Serializer};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn serialize_f64_2<S>(x: &f64, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let rounded = (x * 100.0).round() / 100.0;
    s.serialize_f64(rounded)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemorySummary {
    /// Test function name
    pub test: &'static str,
    /// Start and end memory usage in bytes
    pub bytes: (u64, u64),
    /// Start and end memory blocks
    pub blocks: (u64, u64),
    /// Peak memory usage in bytes
    pub peak_bytes: u64,
    /// Peak memory blocks
    pub peak_blocks: u64,
    /// Throughput in Mbps
    #[serde(serialize_with = "serialize_f64_2")]
    pub throughput_mbps: f64,
    /// Job processing rate
    #[serde(serialize_with = "serialize_f64_2")]
    pub jobs_per_sec: f64,
    /// Average time per job in microseconds
    #[serde(serialize_with = "serialize_f64_2")]
    pub avg_time_per_job_us: f64,
}
