#[cfg(feature = "dhat-heap")]
use std::{
    future::{poll_fn, Future},
    pin::Pin,
    sync::Arc,
};

#[cfg(feature = "dhat-heap")]
use crate::*;

#[cfg(feature = "dhat-heap")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MockHeader {
    packet_type: u8,
    remaining_len: u32,
    total_len: u32,
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

    let payload: Vec<u8> = (0..payload_size as u32).map(|i| (i % 256) as u8).collect();

    let mut body = Vec::new();
    write_string(&mut body, topic_str).unwrap();
    write_u16(&mut body, mock_pid).unwrap();
    write_bytes(&mut body, &payload).unwrap();
    let body_len = body.len();

    let mut remaining_len_buf = Vec::new();
    write_var_int(&mut remaining_len_buf, body_len).unwrap();

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

    fn new_with(hd: u8, remaining_len: u32, total_len: u32) -> Result<Self, Self::Error> {
        Ok(MockHeader {
            packet_type: hd,
            remaining_len,
            total_len,
        })
    }

    fn build_empty_packet(&self) -> Option<Self::Packet> {
        if self.remaining_len == 0 {
            Some(MockPacket::Other)
        } else {
            None
        }
    }

    fn decode_buffer(self, buf: &[u8], offset: &mut usize) -> Result<Self::Packet, Self::Error> {
        let packet = match self.packet_type & 0xF0 {
            0x10 => {
                let protocol_name = read_string(buf, offset)?.to_string();
                let protocol_version = read_u8(buf, offset)?;
                MockPacket::Connect {
                    protocol_name,
                    protocol_version,
                }
            }
            0x30 => {
                let topic = read_string(buf, offset)?.to_string();
                let mock_pid = read_u16(buf, offset)?;
                let payload = read_bytes(buf, offset)?.to_vec();
                MockPacket::Publish {
                    topic,
                    mock_pid,
                    payload,
                }
            }
            _ => MockPacket::Other,
        };
        assert_eq!(*offset, buf.len(), "offset should move to end");
        Ok(packet)
    }

    async fn decode_stream<T: embedded_io_async::Read + Unpin>(
        self,
        reader: &mut T,
    ) -> Result<Self::Packet, Self::Error> {
        let packet = match self.packet_type & 0xF0 {
            0x10 => {
                let protocol_name = read_string_async(reader).await?.to_string();
                let protocol_version = read_u8_async(reader).await?;
                MockPacket::Connect {
                    protocol_name,
                    protocol_version,
                }
            }
            0x30 => {
                let topic = read_string_async(reader).await?.to_string();
                let mock_pid = read_u16_async(reader).await?;
                let payload = read_bytes_async(reader).await?;
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

    fn total_len(&self) -> usize {
        self.total_len as usize
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

    println!("\n--- `common::poll` Stream Simulation ({NUM_ROUNDS} rounds) ---");

    for i in 0..NUM_ROUNDS {
        println!("\n--- Round {} ---", i + 1);

        let mut reader_builder = tokio_test::io::Builder::new();
        reader_builder.read(&[mock_data.control_byte]);
        reader_builder.read(&mock_data.remaining_len_buf);
        for chunk in mock_data.body.chunks(256) {
            reader_builder.read(chunk);
        }
        #[cfg(feature = "tokio")]
        let mut reader = reader_builder.build();
        #[cfg(not(feature = "tokio"))]
        let mut reader = embedded_io_adapters::tokio_1::FromTokio::new(reader_builder.build());

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

    println!("\n--- `common::poll` Actor Model Simulation ({NUM_TASKS} jobs) ---");

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

            let mut reader_builder = tokio_test::io::Builder::new();
            reader_builder.read(&[mock_data.control_byte]);
            reader_builder.read(&mock_data.remaining_len_buf);
            reader_builder.read(&mock_data.body);
            #[cfg(feature = "tokio")]
            let mut reader = reader_builder.build();
            #[cfg(not(feature = "tokio"))]
            let mut reader = embedded_io_adapters::tokio_1::FromTokio::new(reader_builder.build());

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

    let elapsed = simulation_start.elapsed();
    let total_data_size = (PAYLOAD_SIZE + data.remaining_len_buf.len() + 1) * NUM_TASKS;
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

    let summary = super::MemorySummary::new(
        "common::poll",
        &stats_start,
        &stats_end,
        total_data_size,
        NUM_TASKS,
        elapsed,
    );
    println!("{}", serde_json::to_string(&summary).unwrap());

    println!("--- End Report ---");
}
