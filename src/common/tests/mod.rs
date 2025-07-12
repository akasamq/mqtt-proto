mod poll;

use serde::{Deserialize, Serialize};

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

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
    pub throughput_mbps: f64,
    /// Job processing rate
    pub jobs_per_sec: f64,
    /// Average time per job in microseconds
    pub avg_time_per_job_us: f64,
}
