use std::io;
use std::ops::Deref;
use std::sync::Arc;

use bytes::Bytes;

use crate::*;
use PacketType::*;
use QoS::*;

pub(crate) fn unexpected_eof() -> Error {
    Error::IoError(
        io::ErrorKind::UnexpectedEof,
        "unexpected end of file".to_string(),
    )
}

#[test]
fn header_firstbyte() {
    let valid = vec![
        (0b0001_0000, Header::new(Connect, false, Level0, false, 0)),
        (0b0010_0000, Header::new(Connack, false, Level0, false, 0)),
        (0b0011_0000, Header::new(Publish, false, Level0, false, 0)),
        (0b0011_0001, Header::new(Publish, false, Level0, true, 0)),
        (0b0011_0010, Header::new(Publish, false, Level1, false, 0)),
        (0b0011_0011, Header::new(Publish, false, Level1, true, 0)),
        (0b0011_0100, Header::new(Publish, false, Level2, false, 0)),
        (0b0011_0101, Header::new(Publish, false, Level2, true, 0)),
        (0b0011_1000, Header::new(Publish, true, Level0, false, 0)),
        (0b0011_1001, Header::new(Publish, true, Level0, true, 0)),
        (0b0011_1010, Header::new(Publish, true, Level1, false, 0)),
        (0b0011_1011, Header::new(Publish, true, Level1, true, 0)),
        (0b0011_1100, Header::new(Publish, true, Level2, false, 0)),
        (0b0011_1101, Header::new(Publish, true, Level2, true, 0)),
        (0b0100_0000, Header::new(Puback, false, Level0, false, 0)),
        (0b0101_0000, Header::new(Pubrec, false, Level0, false, 0)),
        (0b0110_0010, Header::new(Pubrel, false, Level1, false, 0)),
        (0b0111_0000, Header::new(Pubcomp, false, Level0, false, 0)),
        (0b1000_0010, Header::new(Subscribe, false, Level1, false, 0)),
        (0b1001_0000, Header::new(Suback, false, Level0, false, 0)),
        (
            0b1010_0010,
            Header::new(Unsubscribe, false, Level1, false, 0),
        ),
        (0b1011_0000, Header::new(Unsuback, false, Level0, false, 0)),
        (0b1100_0000, Header::new(Pingreq, false, Level0, false, 0)),
        (0b1101_0000, Header::new(Pingresp, false, Level0, false, 0)),
        (
            0b1110_0000,
            Header::new(Disconnect, false, Level0, false, 0),
        ),
    ];
    for n in 0..=255 {
        let res = match valid.iter().find(|(byte, _)| *byte == n) {
            Some((_, header)) => Ok(*header),
            None if ((n & 0b110) == 0b110) && (n >> 4 == 3) => Err(Error::InvalidQos(3)),
            None => Err(Error::InvalidHeader),
        };
        let buf: &[u8] = &[n, 0];
        assert_eq!(res, Header::decode(buf), "{:08b}", n);
    }
}

#[test]
fn header_len() {
    for (bytes, res) in vec![
        (
            vec![1 << 4, 0],
            Ok(Header::new(Connect, false, Level0, false, 0)),
        ),
        (
            vec![1 << 4, 127],
            Ok(Header::new(Connect, false, Level0, false, 127)),
        ),
        (
            vec![1 << 4, 0x80, 0],
            Ok(Header::new(Connect, false, Level0, false, 0)),
        ), //Weird encoding for "0" buf matches spec
        (
            vec![1 << 4, 0x80, 1],
            Ok(Header::new(Connect, false, Level0, false, 128)),
        ),
        (
            vec![1 << 4, 0x80 + 16, 78],
            Ok(Header::new(Connect, false, Level0, false, 10000)),
        ),
        (
            vec![1 << 4, 0x80, 0x80, 0x80, 0x80],
            Err(Error::InvalidHeader),
        ),
    ] {
        let slice_buf = bytes.as_slice();
        assert_eq!(res, Header::decode(slice_buf));
    }
}

#[test]
fn non_utf8_string() {
    let data: &[u8] = &[
        0b00110000, 10, // type=Publish, remaining_len=10
        0x00, 0x03, 'a' as u8, '/' as u8, 0xc0 as u8, // Topic with Invalid utf8
        'h' as u8, 'e' as u8, 'l' as u8, 'l' as u8, 'o' as u8, // payload
    ];
    assert!(matches!(
        Packet::decode(data).unwrap_err(),
        Error::InvalidString(_)
    ));
}

#[test]
fn inner_length_too_long() {
    let data: &[u8] = &[
        0b00010000, 20, // Connect packet, remaining_len=20
        0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x04, 0b01000000, // +password
        0x00, 0x0a, // keepalive 10 sec
        0x00, 0x04, 't' as u8, 'e' as u8, 's' as u8, 't' as u8, // client_id
        0x00, 0x03, 'm' as u8, 'q' as u8, // password with invalid length
    ];
    assert_eq!(Err(unexpected_eof()), Packet::decode(data));

    let slice: &[u8] = &[
        0b00010000, 20, // Connect packet, remaining_len=20
        0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x04, 0b01000000, // +password
        0x00, 0x0a, // keepalive 10 sec
        0x00, 0x04, 't' as u8, 'e' as u8, 's' as u8, 't' as u8, // client_id
        0x00, 0x03, 'm' as u8, 'q' as u8, // password with invalid length
    ];
    assert_eq!(Err(unexpected_eof()), Packet::decode(slice));
}

#[test]
fn test_decode_half_connect() {
    let data: &[u8] = &[
        0b00010000, 39, 0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x04,
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
    assert_eq!(Err(unexpected_eof()), Packet::decode(data));
    assert_eq!(12, data.len());
}

#[test]
fn test_decode_connect_wrong_version() {
    let data: &[u8] = &[
        0b00010000, 39, 0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x01,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
        0x00, 0x04, 't' as u8, 'e' as u8, 's' as u8, 't' as u8, // client_id
        0x00, 0x02, '/' as u8, 'a' as u8, // will topic = '/a'
        0x00, 0x07, 'o' as u8, 'f' as u8, 'f' as u8, 'l' as u8, 'i' as u8, 'n' as u8,
        'e' as u8, // will msg = 'offline'
        0x00, 0x04, 'r' as u8, 'u' as u8, 's' as u8, 't' as u8, // username = 'rust'
        0x00, 0x02, 'm' as u8, 'q' as u8, // password = 'mq'
    ];
    assert_eq!(
        Packet::decode(data),
        Err(Error::InvalidProtocol("MQTT".to_owned(), 1)),
    );
}

#[test]
fn test_decode_packet_n() {
    let data: &[u8] = &[
        // connect packet
        0b00010000, 39, 0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x04,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
        0x00, 0x04, 't' as u8, 'e' as u8, 's' as u8, 't' as u8, // client_id
        0x00, 0x02, '/' as u8, 'a' as u8, // will topic = '/a'
        0x00, 0x07, 'o' as u8, 'f' as u8, 'f' as u8, 'l' as u8, 'i' as u8, 'n' as u8,
        'e' as u8, // will msg = 'offline'
        0x00, 0x04, 'r' as u8, 'u' as u8, 's' as u8, 't' as u8, // username = 'rust'
        0x00, 0x02, 'm' as u8, 'q' as u8, // password = 'mq'
        // pingreq packet
        0b11000000, 0b00000000, // pingresp packet
        0b11010000, 0b00000000,
    ];

    let pkt1 = crate::Connect {
        protocol: Protocol::MQTT311,
        keep_alive: 10,
        client_id: Arc::new("test".to_owned()),
        clean_session: true,
        last_will: Some(LastWill {
            topic_name: TopicName::try_from("/a".to_owned()).unwrap(),
            message: Bytes::from(b"offline".to_vec()),
            qos: QoS::Level1,
            retain: false,
        }),
        username: Some(Arc::new("rust".to_owned())),
        password: Some(Bytes::from(b"mq".to_vec())),
    };

    let pkt2 = Packet::Pingreq;
    let pkt3 = Packet::Pingresp;

    // decode 3 packets in a sequence stored in the same buffer
    let mut offset = 0;
    let decode_pkt1 = Packet::decode(&data[offset..]).unwrap();
    offset += total_len(pkt1.encode_len()).unwrap();
    let decode_pkt2 = Packet::decode(&data[offset..]).unwrap();
    offset += total_len(0).unwrap();
    let decode_pkt3 = Packet::decode(&data[offset..]).unwrap();

    assert_eq!(Packet::Connect(pkt1), decode_pkt1);
    assert_eq!(pkt2, decode_pkt2);
    assert_eq!(pkt3, decode_pkt3);
}

#[test]
fn test_decode_connack() {
    let data: &[u8] = &[0b00100000, 2, 0b00000000, 0b00000001];
    assert_eq!(
        Packet::decode(data).unwrap(),
        Packet::Connack(crate::Connack {
            session_present: false,
            code: ConnectReturnCode::UnacceptableProtocolVersion,
        })
    );
}

#[test]
fn test_decode_ping_req() {
    let data: &[u8] = &[0b11000000, 0b00000000];
    assert_eq!(Ok(Packet::Pingreq), Packet::decode(data));
}

#[test]
fn test_decode_ping_resp() {
    let data: &[u8] = &[0b11010000, 0b00000000];
    assert_eq!(Ok(Packet::Pingresp), Packet::decode(data));
}

#[test]
fn test_decode_disconnect() {
    let data: &[u8] = &[0b11100000, 0b00000000];
    assert_eq!(Ok(Packet::Disconnect), Packet::decode(data));
}

#[test]
fn test_decode_publish() {
    let data: &[u8] = &[
        0b00110000, 10, 0x00, 0x03, 'a' as u8, '/' as u8, 'b' as u8, 'h' as u8, 'e' as u8,
        'l' as u8, 'l' as u8, 'o' as u8, //
        0b00111000, 10, 0x00, 0x03, 'a' as u8, '/' as u8, 'b' as u8, 'h' as u8, 'e' as u8,
        'l' as u8, 'l' as u8, 'o' as u8, //
        0b00111101, 12, 0x00, 0x03, 'a' as u8, '/' as u8, 'b' as u8, 0, 10, 'h' as u8, 'e' as u8,
        'l' as u8, 'l' as u8, 'o' as u8,
    ];

    assert_eq!(
        Header::decode(data).unwrap(),
        Header::new_with(0b00110000, 10).unwrap(),
    );
    assert_eq!(data.len(), 38);

    match Packet::decode(data).unwrap() {
        Packet::Publish(p) => {
            assert_eq!(p.dup, false);
            assert_eq!(p.retain, false);
            assert_eq!(p.qos_pid, QosPid::Level0);
            assert_eq!(p.topic_name.deref(), "a/b");
            assert_eq!(core::str::from_utf8(p.payload.as_ref()).unwrap(), "hello");
        }
        other => panic!("Failed decode: {:?}", other),
    }

    let data2 = &data[12..];
    match Packet::decode(data2).unwrap() {
        Packet::Publish(p) => {
            assert_eq!(p.dup, true);
            assert_eq!(p.retain, false);
            assert_eq!(p.qos_pid, QosPid::Level0);
            assert_eq!(p.topic_name.deref(), "a/b");
            assert_eq!(core::str::from_utf8(p.payload.as_ref()).unwrap(), "hello");
        }
        other => panic!("Failed decode: {:?}", other),
    }

    let data3 = &data[24..];
    match Packet::decode(data3).unwrap() {
        Packet::Publish(p) => {
            assert_eq!(p.dup, true);
            assert_eq!(p.retain, true);
            assert_eq!(p.qos_pid, QosPid::Level2(Pid::new(10)));
            assert_eq!(p.topic_name.deref(), "a/b");
            assert_eq!(core::str::from_utf8(p.payload.as_ref()).unwrap(), "hello");
        }
        other => panic!("Failed decode: {:?}", other),
    }
}

#[test]
fn test_decode_pub_ack() {
    let data: &[u8] = &[0b01000000, 0b00000010, 0, 10];
    assert_eq!(Packet::decode(data).unwrap(), Packet::Puback(Pid::new(10)));
}

#[test]
fn test_decode_pub_rec() {
    let data: &[u8] = &[0b01010000, 0b00000010, 0, 10];
    assert_eq!(Packet::decode(data).unwrap(), Packet::Pubrec(Pid::new(10)));
}

#[test]
fn test_decode_pub_rel() {
    let data: &[u8] = &[0b01100010, 0b00000010, 0, 10];
    assert_eq!(Packet::decode(data).unwrap(), Packet::Pubrel(Pid::new(10)));
}

#[test]
fn test_decode_pub_comp() {
    let data: &[u8] = &[0b01110000, 0b00000010, 0, 10];
    assert_eq!(Packet::decode(data).unwrap(), Packet::Pubcomp(Pid::new(10)));
}

#[test]
fn test_decode_subscribe() {
    let data: &[u8] = &[
        0b10000010, 8, 0, 10, 0, 3, 'a' as u8, '/' as u8, 'b' as u8, 0,
    ];
    assert_eq!(
        Packet::decode(data).unwrap(),
        Packet::Subscribe(crate::Subscribe {
            pid: Pid::new(10),
            subscribes: vec![(
                TopicFilter::try_from("a/b".to_owned()).unwrap(),
                QoS::Level0
            )],
        })
    );
}

#[test]
fn test_decode_suback() {
    let data: &[u8] = &[0b10010000, 3, 0, 10, 0b00000010];
    assert_eq!(
        Packet::decode(data).unwrap(),
        Packet::Suback(crate::Suback {
            pid: Pid::new(10),
            subscribes: vec![SubscribeReturnCode::MaxLevel2],
        })
    );
}

#[test]
fn test_decode_unsubscribe() {
    let data: &[u8] = &[0b10100010, 5, 0, 10, 0, 1, 'a' as u8];
    assert_eq!(
        Packet::decode(data).unwrap(),
        Packet::Unsubscribe(crate::Unsubscribe {
            pid: Pid::new(10),
            subscribes: vec![TopicFilter::try_from("a".to_owned()).unwrap(),],
        })
    );
}

#[test]
fn test_decode_unsub_ack() {
    let data: &[u8] = &[0b10110000, 2, 0, 10];
    assert_eq!(
        Packet::decode(data).unwrap(),
        Packet::Unsuback(Pid::new(10))
    );
}
