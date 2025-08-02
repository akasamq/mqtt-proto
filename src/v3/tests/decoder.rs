use core::ops::Deref;

use bytes::Bytes;

use crate::v3::*;
use crate::*;

#[test]
fn test_header_firstbyte() {
    use PacketType::*;
    use QoS::*;

    #[rustfmt::skip]
    let valid = alloc::vec![
        (0b0001_0000, Header::new(Connect, false, Level0, false, 0, 1)),
        (0b0010_0000, Header::new(Connack, false, Level0, false, 0, 1)),
        (0b0011_0000, Header::new(Publish, false, Level0, false, 0, 1)),
        (0b0011_0001, Header::new(Publish, false, Level0, true, 0, 1)),
        (0b0011_0010, Header::new(Publish, false, Level1, false, 0, 1)),
        (0b0011_0011, Header::new(Publish, false, Level1, true, 0, 1)),
        (0b0011_0100, Header::new(Publish, false, Level2, false, 0, 1)),
        (0b0011_0101, Header::new(Publish, false, Level2, true, 0, 1)),
        (0b0011_1000, Header::new(Publish, true, Level0, false, 0, 1)),
        (0b0011_1001, Header::new(Publish, true, Level0, true, 0, 1)),
        (0b0011_1010, Header::new(Publish, true, Level1, false, 0, 1)),
        (0b0011_1011, Header::new(Publish, true, Level1, true, 0, 1)),
        (0b0011_1100, Header::new(Publish, true, Level2, false, 0, 1)),
        (0b0011_1101, Header::new(Publish, true, Level2, true, 0, 1)),
        (0b0100_0000, Header::new(Puback, false, Level0, false, 0, 1)),
        (0b0101_0000, Header::new(Pubrec, false, Level0, false, 0, 1)),
        (0b0110_0010, Header::new(Pubrel, false, Level0, false, 0, 1)),
        (0b0111_0000, Header::new(Pubcomp, false, Level0, false, 0, 1)),
        (0b1000_0010, Header::new(Subscribe, false, Level0, false, 0, 1)),
        (0b1001_0000, Header::new(Suback, false, Level0, false, 0, 1)),
        (0b1010_0010, Header::new(Unsubscribe, false, Level0, false, 0, 1)),
        (0b1011_0000, Header::new(Unsuback, false, Level0, false, 0, 1)),
        (0b1100_0000, Header::new(Pingreq, false, Level0, false, 0, 1)),
        (0b1101_0000, Header::new(Pingresp, false, Level0, false, 0, 1)),
        (0b1110_0000, Header::new(Disconnect, false, Level0, false, 0, 1)),
    ];
    for n in 0..=255 {
        let res = match valid.iter().find(|(byte, _)| *byte == n) {
            Some((_, header)) => Ok(*header),
            None if ((n & 0b110) == 0b110) && (n >> 4 == 3) => Err(Error::InvalidQos(3)),
            None => Err(Error::InvalidHeader),
        };
        let buf: &[u8] = &[n, 0];
        assert_eq!(res, Header::decode(buf), "{n:08b}");
    }
}

#[test]
fn test_header_len() {
    use PacketType::*;
    use QoS::*;

    for (bytes, res) in alloc::vec![
        (
            alloc::vec![1 << 4, 0],
            Ok(Header::new(Connect, false, Level0, false, 0, 1)),
        ),
        (
            alloc::vec![1 << 4, 127],
            Ok(Header::new(Connect, false, Level0, false, 127, 1)),
        ),
        (
            alloc::vec![1 << 4, 0x80, 0],
            Ok(Header::new(Connect, false, Level0, false, 0, 2)),
        ), //Weird encoding for "0" buf matches spec
        (
            alloc::vec![1 << 4, 0x80, 1],
            Ok(Header::new(Connect, false, Level0, false, 128, 2)),
        ),
        (
            alloc::vec![1 << 4, 0x80 + 16, 78],
            Ok(Header::new(Connect, false, Level0, false, 10000, 2)),
        ),
        (
            alloc::vec![1 << 4, 0x80, 0x80, 0x80, 0x80],
            Err(Error::InvalidVarByteInt),
        ),
    ] {
        let slice_buf = bytes.as_slice();
        assert_eq!(res, Header::decode(slice_buf));
    }
}

#[test]
fn test_non_utf8_string() {
    let mut data: &[u8] = &[
        0b00110000, 10, // type=Publish, remaining_len=10
        0x00, 0x03, b'a', b'/', 0xc0_u8, // Topic with Invalid utf8
        b'h', b'e', b'l', b'l', b'o', // payload
    ];
    assert!(matches!(
        Packet::decode(data).unwrap_err(),
        Error::InvalidString
    ));
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap_err()
    );
}

#[test]
fn test_inner_length_too_long() {
    let mut data: &[u8] = &[
        0b00010000, 20, // Connect packet, remaining_len=20
        0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, 0b01000000, // +password
        0x00, 0x0a, // keepalive 10 sec
        0x00, 0x04, b't', b'e', b's', b't', // client_id
        0x00, 0x03, b'm', b'q', // password with invalid length
    ];
    assert_eq!(Ok(None), Packet::decode(data));
    assert_eq!(
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap_err(),
        Error::InvalidRemainingLength
    );
}

#[test]
fn test_decode_half_connect() {
    let mut data: &[u8] = &[
        0b00010000, 39, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00,
        0x0a, // 10 sec
              // 0x00, 0x04, 't' as u8, 'e' as u8, 's' as u8, 't' as u8, // client_id
              // 0x00, 0x02, '/' as u8, 'a' as u8, // will topic = '/a'
              // 0x00, 0x07, 'o' as u8, 'f' as u8, 'f' as u8, 'l' as u8, 'i' as u8, 'n' as u8,
              // 'e' as u8, // will msg = 'offline'
              // 0x00, 0x04, 'r' as u8, 'u' as u8, 's' as u8, 't' as u8, // username = 'rust'
              // 0x00, 0x02, 'm' as u8, 'q' as u8, // password = 'mq'
    ];
    assert_eq!(Ok(None), Packet::decode(data));
    assert_eq!(12, data.len());
    assert!(block_on(PollPacket::new(
        &mut Default::default(),
        &mut data,
        &mut MockBuffer::default()
    ))
    .unwrap_err()
    .is_eof());
}

#[test]
fn test_decode_connect_wrong_version() {
    let mut data: &[u8] = &[
        0b00010000, 39, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x01,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
        0x00, 0x04, b't', b'e', b's', b't', // client_id
        0x00, 0x02, b'/', b'a', // will topic = '/a'
        0x00, 0x07, b'o', b'f', b'f', b'l', b'i', b'n', b'e', // will msg = 'offline'
        0x00, 0x04, b'r', b'u', b's', b't', // username = 'rust'
        0x00, 0x02, b'm', b'q', // password = 'mq'
    ];
    assert_eq!(
        Packet::decode(data),
        Err(Error::InvalidProtocol("MQTT".into(), 1)),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap_err()
    );
}

#[test]
fn test_decode_reserved_connect_flags() {
    let mut data: &[u8] = &[
        0b00010000, 16, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04,
        0b11001111, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
        0x00, 0x04, b't', b'e', b's', b't', // client_id
    ];
    assert_eq!(
        Packet::decode(data),
        Err(Error::InvalidConnectFlags(0b11001111)),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap_err()
    );
}

#[test]
fn test_decode_packet_n() {
    let data: &[u8] = &[
        // connect packet
        0b00010000, 39, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
        0x00, 0x04, b't', b'e', b's', b't', // client_id
        0x00, 0x02, b'/', b'a', // will topic = '/a'
        0x00, 0x07, b'o', b'f', b'f', b'l', b'i', b'n', b'e', // will msg = 'offline'
        0x00, 0x04, b'r', b'u', b's', b't', // username = 'rust'
        0x00, 0x02, b'm', b'q', // password = 'mq'
        // pingreq packet
        0b11000000, 0b00000000, // pingresp packet
        0b11010000, 0b00000000,
    ];

    let pkt1 = v3::Connect {
        protocol: Protocol::V311,
        keep_alive: 10,
        client_id: "test".into(),
        clean_session: true,
        last_will: Some(LastWill {
            topic_name: TopicName::try_from("/a").unwrap(),
            message: Bytes::from(b"offline".to_vec()),
            qos: QoS::Level1,
            retain: false,
        }),
        username: Some("rust".into()),
        password: Some(Bytes::from(b"mq".to_vec())),
    };

    let pkt2 = Packet::Pingreq;
    let pkt3 = Packet::Pingresp;

    // decode 3 packets in a sequence stored in the same buffer
    let mut offset = 0;
    let mut data1 = &data[offset..];
    let decode_pkt1 = Packet::decode(data1).unwrap().unwrap();
    assert_eq!(
        Packet::decode(data1).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data1,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );

    offset += total_len(pkt1.encode_len()).unwrap();
    let mut data2 = &data[offset..];
    let decode_pkt2 = Packet::decode(data2).unwrap().unwrap();
    assert_eq!(
        Packet::decode(data2).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data2,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );

    offset += total_len(0).unwrap();
    let mut data3 = &data[offset..];
    let decode_pkt3 = Packet::decode(data3).unwrap().unwrap();
    assert_eq!(
        Packet::decode(data3).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data3,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );

    assert_eq!(Packet::Connect(pkt1), decode_pkt1);
    assert_eq!(pkt2, decode_pkt2);
    assert_eq!(pkt3, decode_pkt3);
}

#[test]
fn test_decode_connack() {
    let mut data: &[u8] = &[0b00100000, 2, 0b00000000, 0b00000001];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Connack(v3::Connack {
            session_present: false,
            code: ConnectReturnCode::UnacceptableProtocolVersion,
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_ping_req() {
    let mut data: &[u8] = &[0b11000000, 0b00000000];
    assert_eq!(Ok(Some(Packet::Pingreq)), Packet::decode(data));
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_ping_resp() {
    let mut data: &[u8] = &[0b11010000, 0b00000000];
    assert_eq!(Ok(Some(Packet::Pingresp)), Packet::decode(data));
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_disconnect() {
    let mut data: &[u8] = &[0b11100000, 0b00000000];
    assert_eq!(Ok(Some(Packet::Disconnect)), Packet::decode(data));
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_publish() {
    let data: &[u8] = &[
        0b00110000, 10, 0x00, 0x03, b'a', b'/', b'b', b'h', b'e', b'l', b'l', b'o', //
        0b00111000, 10, 0x00, 0x03, b'a', b'/', b'b', b'h', b'e', b'l', b'l', b'o', //
        0b00111101, 12, 0x00, 0x03, b'a', b'/', b'b', 0, 10, b'h', b'e', b'l', b'l', b'o',
    ];

    let mut data1 = data;
    assert_eq!(
        Header::decode(data1).unwrap(),
        Header::new_with(0b00110000, 10, 1).unwrap(),
    );
    assert_eq!(data.len(), 38);

    match Packet::decode(data1).unwrap().unwrap() {
        Packet::Publish(p) => {
            assert!(!p.dup);
            assert!(!p.retain);
            assert_eq!(p.qos_pid, QosPid::Level0);
            assert_eq!(p.topic_name.deref(), "a/b");
            assert_eq!(core::str::from_utf8(p.payload.as_ref()).unwrap(), "hello");
        }
        other => panic!("Failed decode: {other:?}"),
    }
    assert_eq!(
        Packet::decode(data1).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data1,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );

    let mut data2 = &data[12..];
    match Packet::decode(data2).unwrap().unwrap() {
        Packet::Publish(p) => {
            assert!(p.dup);
            assert!(!p.retain);
            assert_eq!(p.qos_pid, QosPid::Level0);
            assert_eq!(p.topic_name.deref(), "a/b");
            assert_eq!(core::str::from_utf8(p.payload.as_ref()).unwrap(), "hello");
        }
        other => panic!("Failed decode: {other:?}"),
    }
    assert_eq!(
        Packet::decode(data2).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data2,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );

    let mut data3 = &data[24..];
    match Packet::decode(data3).unwrap().unwrap() {
        Packet::Publish(p) => {
            assert!(p.dup);
            assert!(p.retain);
            assert_eq!(p.qos_pid, QosPid::Level2(Pid::try_from(10).unwrap()));
            assert_eq!(p.topic_name.deref(), "a/b");
            assert_eq!(core::str::from_utf8(p.payload.as_ref()).unwrap(), "hello");
        }
        other => panic!("Failed decode: {other:?}"),
    }
    assert_eq!(
        Packet::decode(data3).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data3,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_pub_ack() {
    let mut data: &[u8] = &[0b01000000, 0b00000010, 0, 10];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Puback(Pid::try_from(10).unwrap())
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_pub_rec() {
    let mut data: &[u8] = &[0b01010000, 0b00000010, 0, 10];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrec(Pid::try_from(10).unwrap())
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_pub_rel() {
    let mut data: &[u8] = &[0b01100010, 0b00000010, 0, 10];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrel(Pid::try_from(10).unwrap())
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_pub_comp() {
    let mut data: &[u8] = &[0b01110000, 0b00000010, 0, 10];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubcomp(Pid::try_from(10).unwrap())
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_subscribe() {
    let mut data: &[u8] = &[0b10000010, 8, 0, 10, 0, 3, b'a', b'/', b'b', 0];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Subscribe(v3::Subscribe {
            pid: Pid::try_from(10).unwrap(),
            topics: alloc::vec![(TopicFilter::try_from("a/b").unwrap(), QoS::Level0)],
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_suback() {
    let mut data: &[u8] = &[0b10010000, 3, 0, 10, 0b00000010];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Suback(v3::Suback {
            pid: Pid::try_from(10).unwrap(),
            topics: alloc::vec![SubscribeReturnCode::MaxLevel2],
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_unsubscribe() {
    let mut data: &[u8] = &[0b10100010, 5, 0, 10, 0, 1, b'a'];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Unsubscribe(v3::Unsubscribe {
            pid: Pid::try_from(10).unwrap(),
            topics: alloc::vec![TopicFilter::try_from("a").unwrap(),],
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[test]
fn test_decode_unsub_ack() {
    let mut data: &[u8] = &[0b10110000, 2, 0, 10];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Unsuback(Pid::try_from(10).unwrap())
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(
            &mut Default::default(),
            &mut data,
            &mut MockBuffer::default()
        ))
        .unwrap()
        .2
    );
}

#[tokio::test(flavor = "current_thread")]
#[cfg(feature = "dhat-heap")]
async fn poll_actor_model_simulation_v3() {
    let _profiler = dhat::Profiler::builder().testing().build();

    const NUM_TASKS: usize = 100_000;

    let mut packets = Vec::new();

    for len in (0..10).map(|i| 1 << i) {
        let client_id = "a".repeat(len);
        let pkt = Packet::Connect(Connect {
            protocol: Protocol::V311,
            keep_alive: 60,
            client_id: client_id.into(),
            clean_session: true,
            last_will: None,
            username: None,
            password: None,
        });
        packets.push(pkt.encode().unwrap());
    }

    for size in (0..15).map(|i| 1 << i) {
        let payload = vec![b'x'; size];
        let pkt = Packet::Publish(Publish {
            dup: false,
            qos_pid: QosPid::Level1(Pid::try_from(1).unwrap()),
            retain: false,
            topic_name: TopicName::try_from("topic/test").unwrap(),
            payload: Bytes::from(payload),
        });
        packets.push(pkt.encode().unwrap());
    }

    for qos in [QoS::Level0, QoS::Level1, QoS::Level2] {
        let pkt = Packet::Subscribe(Subscribe::new(
            Pid::try_from(10).unwrap(),
            vec![(TopicFilter::try_from("a/+").unwrap(), qos)],
        ));
        packets.push(pkt.encode().unwrap());
    }
    for _ in 0..3 {
        let pkt = Packet::Unsubscribe(Unsubscribe::new(
            Pid::try_from(20).unwrap(),
            vec![TopicFilter::try_from("b/#").unwrap()],
        ));
        packets.push(pkt.encode().unwrap());
    }
    packets.push(Packet::Pingreq.encode().unwrap());
    packets.push(Packet::Pingresp.encode().unwrap());
    packets.push(Packet::Disconnect.encode().unwrap());

    let mut rng_seed = 42u64;
    for i in (1..packets.len()).rev() {
        rng_seed = rng_seed.wrapping_mul(1103515245).wrapping_add(12345);
        let j = (rng_seed % (i + 1) as u64) as usize;
        packets.swap(i, j);
    }

    let data: std::sync::Arc<Vec<VarBytes>> = std::sync::Arc::new(packets);

    println!("\n--- `v3::decoder` Actor Model Simulation ({NUM_TASKS} jobs) ---");

    let stats_start = dhat::HeapStats::get();
    println!(
        "Start:               {:>5} bytes in {:>2} blocks",
        stats_start.curr_bytes, stats_start.curr_blocks
    );

    let simulation_start = std::time::Instant::now();
    let mut handles = Vec::with_capacity(NUM_TASKS);

    for i in 0..NUM_TASKS {
        let packets = data.clone();
        let idx = i % packets.len();
        let data = packets[idx].clone();

        handles.push(tokio::spawn(async move {
            let mut buf: &[u8] = data.as_ref();
            let _ = Packet::decode_async(&mut buf).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = simulation_start.elapsed();
    let total_data_size = data.len() * NUM_TASKS;
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

    let summary = common::MemorySummary::new(
        "v3::decoder",
        &stats_start,
        &stats_end,
        total_data_size,
        NUM_TASKS,
        elapsed,
    );
    println!("{}", serde_json::to_string(&summary).unwrap());

    println!("--- End Report ---");
}
