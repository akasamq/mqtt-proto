mod buffer;
mod poll;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(feature = "dhat-heap")]
fn serialize_f64_2<S>(x: &f64, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let rounded = (x * 100.0).round() / 100.0;
    s.serialize_f64(rounded)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg(feature = "dhat-heap")]
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

#[cfg(feature = "dhat-heap")]
impl MemorySummary {
    pub fn new(
        test: &'static str,
        stats_start: &dhat::HeapStats,
        stats_end: &dhat::HeapStats,
        total_data_size: usize,
        num_jobs: usize,
        elapsed: core::time::Duration,
    ) -> Self {
        let throughput_mbps =
            (total_data_size as f64 * 8.0) / (elapsed.as_secs_f64() * 1_000_000.0);
        let jobs_per_sec = num_jobs as f64 / elapsed.as_secs_f64();
        let avg_time_per_job_us = elapsed.as_micros() as f64 / num_jobs as f64;
        Self {
            test,
            bytes: (stats_start.curr_bytes as u64, stats_end.curr_bytes as u64),
            blocks: (stats_start.curr_blocks as u64, stats_end.curr_blocks as u64),
            peak_bytes: stats_end.max_bytes as u64,
            peak_blocks: stats_end.max_blocks as u64,
            throughput_mbps,
            jobs_per_sec,
            avg_time_per_job_us,
        }
    }
}
