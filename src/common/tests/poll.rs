#[cfg(feature = "dhat-heap")]
use std::{pin::Pin, sync::Arc};

#[cfg(feature = "dhat-heap")]
use futures_lite::{
    future::{block_on, poll_fn},
    Future,
};

#[cfg(feature = "dhat-heap")]
use crate::*;

#[cfg(feature = "dhat-heap")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MockHeader {
    remaining_len: u32,
    packet_type: u8,
}

#[cfg(feature = "dhat-heap")]
#[derive(Debug, PartialEq, Eq, Clone)]
enum MockPacket {
    Connect {
        protocol_name: String,
        protocol_version: u8,
    },
    Publish {
        topic: String,
        mock_pid: u16,
        payload: Vec<u8>,
    },
    Other,
}

#[cfg(feature = "dhat-heap")]
fn read_string(reader: &mut &[u8]) -> String {
    if reader.len() < 2 {
        return String::new();
    }
    let len_bytes: [u8; 2] = reader[0..2].try_into().unwrap();
    let len = u16::from_be_bytes(len_bytes);
    *reader = &reader[2..];
    if reader.len() < len as usize {
        return String::new();
    }
    let s = String::from_utf8_lossy(&reader[..len as usize]).to_string();
    *reader = &reader[len as usize..];
    s
}

#[cfg(feature = "dhat-heap")]
struct MockPublishData {
    control_byte: u8,
    remaining_len_buf: Vec<u8>,
    body: Vec<u8>,
    expected_packet: MockPacket,
}

#[cfg(feature = "dhat-heap")]
fn prepare_mock_publish_data(
    topic_str: &str,
    payload_size: usize,
    mock_pid: u16,
) -> MockPublishData {
    let control_byte = 0x30; // Publish packet type

    let mut topic_buf = Vec::new();
    topic_buf.extend_from_slice(&(topic_str.len() as u16).to_be_bytes());
    topic_buf.extend_from_slice(topic_str.as_bytes());

    let payload: Vec<u8> = (0..payload_size as u32).map(|i| (i % 256) as u8).collect();

    let mut body = topic_buf;
    body.extend_from_slice(&mock_pid.to_be_bytes());
    body.extend_from_slice(&payload);
    let body_len = body.len();

    let mut remaining_len_buf = Vec::new();
    crate::common::write_var_int(&mut remaining_len_buf, body_len).unwrap();

    let expected_packet = MockPacket::Publish {
        topic: topic_str.to_string(),
        mock_pid,
        payload,
    };

    MockPublishData {
        control_byte,
        remaining_len_buf,
        body,
        expected_packet,
    }
}

#[cfg(feature = "dhat-heap")]
impl PollHeader for MockHeader {
    type Error = Error;
    type Packet = MockPacket;

    fn new_with(hd: u8, remaining_len: u32) -> Result<Self, Self::Error> {
        Ok(MockHeader {
            remaining_len,
            packet_type: hd,
        })
    }

    fn build_empty_packet(&self) -> Option<Self::Packet> {
        if self.remaining_len == 0 {
            Some(MockPacket::Other)
        } else {
            None
        }
    }

    fn block_decode(self, reader: &mut &[u8]) -> Result<Self::Packet, Self::Error> {
        let packet = match self.packet_type & 0xF0 {
            0x10 => {
                let protocol_name = read_string(reader);
                if reader.is_empty() {
                    return Err(Error::InvalidVarByteInt);
                }
                let protocol_version = reader[0];
                *reader = &reader[1..];
                MockPacket::Connect {
                    protocol_name,
                    protocol_version,
                }
            }
            0x30 => {
                let topic = read_string(reader);
                let mock_pid = block_on(read_u16(reader))?;
                let payload = reader.to_vec();
                *reader = &[];
                MockPacket::Publish {
                    topic,
                    mock_pid,
                    payload,
                }
            }
            _ => MockPacket::Other,
        };
        Ok(packet)
    }

    fn remaining_len(&self) -> usize {
        self.remaining_len as usize
    }

    fn is_eof_error(err: &Self::Error) -> bool {
        err.is_eof()
    }
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "dhat-heap")]
async fn poll_stream_simulation() {
    let _profiler = dhat::Profiler::builder().testing().build();

    const PAYLOAD_SIZE: usize = 1024;
    const MOCK_PID: u16 = 42;
    const NUM_ROUNDS: usize = 5;
    const TOPIC: &str = "a/b/c";

    let mock_data = prepare_mock_publish_data(TOPIC, PAYLOAD_SIZE, MOCK_PID);

    println!(
        "\n--- `common::poll` Stream Simulation ({} rounds) ---",
        NUM_ROUNDS
    );

    for i in 0..NUM_ROUNDS {
        println!("\n--- Round {} ---", i + 1);

        let mut reader_builder = tokio_test::io::Builder::new();
        reader_builder.read(&[mock_data.control_byte]);
        reader_builder.read(&mock_data.remaining_len_buf);
        for chunk in mock_data.body.chunks(256) {
            reader_builder.read(chunk);
        }
        let mut reader = reader_builder.build();

        let mut state = GenericPollPacketState::<MockHeader>::default();
        let mut poll_packet = GenericPollPacket::new(&mut state, &mut reader);

        let stats_start = dhat::HeapStats::get();
        println!(
            "Start:                  {:>5} bytes in {:>2} blocks",
            stats_start.curr_bytes, stats_start.curr_blocks
        );

        let result = poll_fn(|cx| Pin::new(&mut poll_packet).poll(cx)).await;
        assert!(result.is_ok());

        let stats_decoded = dhat::HeapStats::get();
        println!(
            "Poll & Decode (net):    {:>5} bytes in {:>2} blocks. Change: {:>+5} bytes, {:>+3} blocks",
            stats_decoded.curr_bytes,
            stats_decoded.curr_blocks,
            stats_decoded.curr_bytes as i64 - stats_start.curr_bytes as i64,
            stats_decoded.curr_blocks as i64 - stats_start.curr_blocks as i64
        );

        let (_total_len, buf, packet) = result.unwrap();
        assert_eq!(packet, mock_data.expected_packet);

        drop(buf);
        let stats_dropped_buf = dhat::HeapStats::get();
        println!(
            "Drop buffer:            {:>5} bytes in {:>2} blocks. Change: {:>+5} bytes, {:>+3} blocks",
            stats_dropped_buf.curr_bytes,
            stats_dropped_buf.curr_blocks,
            stats_dropped_buf.curr_bytes as i64 - stats_decoded.curr_bytes as i64,
            stats_dropped_buf.curr_blocks as i64 - stats_decoded.curr_blocks as i64
        );

        drop(packet);
        let stats_end_of_round = dhat::HeapStats::get();
        println!(
            "Drop packet:            {:>5} bytes in {:>2} blocks. Change: {:>+5} bytes, {:>+3} blocks",
            stats_end_of_round.curr_bytes,
            stats_end_of_round.curr_blocks,
            stats_end_of_round.curr_bytes as i64 - stats_dropped_buf.curr_bytes as i64,
            stats_end_of_round.curr_blocks as i64 - stats_dropped_buf.curr_blocks as i64
        );
    }
    println!("\n--- End Report ---");
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "dhat-heap")]
async fn poll_actor_model_simulation() {
    let _profiler = dhat::Profiler::builder().testing().build();

    const PAYLOAD_SIZE: usize = 1024;
    const MOCK_PID: u16 = 42;
    const NUM_TASKS: usize = 100_000;
    const TOPIC: &str = "a/b/c";

    let data = Arc::new(prepare_mock_publish_data(TOPIC, PAYLOAD_SIZE, MOCK_PID));

    println!(
        "\n--- `common::poll` Actor Model Simulation ({} jobs) ---",
        NUM_TASKS
    );

    let stats_start = dhat::HeapStats::get();
    println!(
        "Start:               {:>5} bytes in {:>2} blocks",
        stats_start.curr_bytes, stats_start.curr_blocks
    );

    let simulation_start = std::time::Instant::now();
    let mut handles = Vec::with_capacity(NUM_TASKS);

    for _ in 0..NUM_TASKS {
        let data = data.clone();

        handles.push(tokio::spawn(async move {
            let mock_data = &*data;

            let mut reader = tokio_test::io::Builder::new()
                .read(&[mock_data.control_byte])
                .read(&mock_data.remaining_len_buf)
                .read(&mock_data.body)
                .build();

            let mut state = GenericPollPacketState::<MockHeader>::default();
            let mut poll_packet = GenericPollPacket::new(&mut state, &mut reader);

            let result = poll_fn(|cx| Pin::new(&mut poll_packet).poll(cx)).await;
            assert!(result.is_ok());

            let (_total_len, buf, packet) = result.unwrap();
            assert_eq!(packet, mock_data.expected_packet);

            drop(buf);
            drop(packet);
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total_simulation_time = simulation_start.elapsed();
    let total_data_size = (PAYLOAD_SIZE + data.remaining_len_buf.len() + 1) * NUM_TASKS;
    let throughput_mbps =
        (total_data_size as f64 * 8.0) / (total_simulation_time.as_secs_f64() * 1_000_000.0);
    let actors_per_second = NUM_TASKS as f64 / total_simulation_time.as_secs_f64();

    drop(data);

    let stats_end = dhat::HeapStats::get();
    println!(
        "End:                 {:>5} bytes in {:>2} blocks. Change: {:>+5} bytes, {:>+3} blocks",
        stats_end.curr_bytes,
        stats_end.curr_blocks,
        stats_end.curr_bytes as i64 - stats_start.curr_bytes as i64,
        stats_end.curr_blocks as i64 - stats_start.curr_blocks as i64
    );
    println!(
        "Peak memory usage:   {:>5} bytes in {:>2} blocks",
        stats_end.max_bytes, stats_end.max_blocks
    );

    let summary = super::MemorySummary {
        test: "common::poll",
        bytes: (stats_start.curr_bytes as u64, stats_end.curr_bytes as u64),
        blocks: (stats_start.curr_blocks as u64, stats_end.curr_blocks as u64),
        peak_bytes: stats_end.max_bytes as u64,
        peak_blocks: stats_end.max_blocks as u64,
        throughput_mbps,
        jobs_per_sec: actors_per_second,
        avg_time_per_job_us: total_simulation_time.as_micros() as f64 / NUM_TASKS as f64,
    };
    println!("{}", serde_json::to_string(&summary).unwrap());

    println!("--- End Report ---");
}
