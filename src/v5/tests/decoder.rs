use bytes::Bytes;
use std::sync::Arc;

use futures_lite::future::block_on;

use crate::v5::*;
use crate::*;
use QoS::*;

#[test]
fn test_v5_header_firstbyte() {
    use PacketType::*;
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
        (0b0110_0010, Header::new(Pubrel, false, Level0, false, 0)),
        (0b0111_0000, Header::new(Pubcomp, false, Level0, false, 0)),
        (0b1000_0010, Header::new(Subscribe, false, Level0, false, 0)),
        (0b1001_0000, Header::new(Suback, false, Level0, false, 0)),
        (
            0b1010_0010,
            Header::new(Unsubscribe, false, Level0, false, 0),
        ),
        (0b1011_0000, Header::new(Unsuback, false, Level0, false, 0)),
        (0b1100_0000, Header::new(Pingreq, false, Level0, false, 0)),
        (0b1101_0000, Header::new(Pingresp, false, Level0, false, 0)),
        (
            0b1110_0000,
            Header::new(Disconnect, false, Level0, false, 0),
        ),
        (0b1111_0000, Header::new(Auth, false, Level0, false, 0)),
    ];
    for n in 0..=255 {
        let res = match valid.iter().find(|(byte, _)| *byte == n) {
            Some((_, header)) => Ok(*header),
            None if ((n & 0b110) == 0b110) && (n >> 4 == 3) => Err(Error::InvalidQos(3).into()),
            None => Err(Error::InvalidHeader.into()),
        };
        let buf: &[u8] = &[n, 0];
        assert_eq!(res, Header::decode(buf), "{:08b}", n);
    }
}

#[test]
fn test_v5_header_len() {
    use PacketType::*;
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
            Err(Error::InvalidVarByteInt.into()),
        ),
    ] {
        let slice_buf = bytes.as_slice();
        assert_eq!(res, Header::decode(slice_buf));
    }
}

#[test]
fn test_v5_non_utf8_string() {
    let mut data: &[u8] = &[
        0b00110000, // type=Publish
        11,         // remaining length
        0x00, 0x03, 'a' as u8, '/' as u8, 0xc0 as u8, // Topic with Invalid utf8
        0x00,       // properties
        'h' as u8, 'e' as u8, 'l' as u8, 'l' as u8, 'o' as u8, // payload
    ];
    assert!(matches!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::Common(Error::InvalidString)
    ));
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_connect() {
    let mut data: &[u8] = &[
        0b00010000, 22, // Connect packet, remaining length
        0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x05, 0b01000000, // +password
        0x00, 0x0a, // keepalive 10 sec
        0x00, // properties
        0x00, 0x04, 't' as u8, 'e' as u8, 's' as u8, 't' as u8, // client_id
        0x00, 0x03, 'm' as u8, 'q' as u8, 't' as u8, // password
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Connect(Connect {
            protocol: Protocol::V500,
            clean_start: false,
            keep_alive: 10,
            properties: Default::default(),
            client_id: Arc::new("test".to_string()),
            last_will: None,
            username: None,
            password: Some(Bytes::from(vec!['m' as u8, 'q' as u8, 't' as u8])),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        0b00010000, 21, // Connect packet, remaining length
        0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x05, 0b01000000, // +password
        0x00, 0x0a, // keepalive 10 sec
        0x00, // properties
        0x00, 0x04, 't' as u8, 'e' as u8, 's' as u8, 't' as u8, // client_id
        0x00, 0x03, 'm' as u8, 'q' as u8, // password with invalid length
    ];
    assert_eq!(Ok(None), Packet::decode(data));
    assert_eq!(
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err(),
        Error::InvalidRemainingLength.into()
    );

    let mut data: &[u8] = &[
        0b00010000, 11, 0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x05,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
    ];
    assert_eq!(Ok(None), Packet::decode(data));
    assert!(
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap_err()
            .is_eof()
    );

    let mut data: &[u8] = &[
        0b00010000, 11, 0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x01,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
        0x00, // properties
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::Common(Error::InvalidProtocol("MQTT".to_owned(), 1)),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        0b00010000, 11, 0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x04,
        0b11001110, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
        0x00, // properties
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::Common(Error::UnexpectedProtocol(Protocol::V311)),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        0b00010000, 10, 0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x05,
        0b11001111, // +username, +password, -will retain, will qos=1, +last_will, +clean_session
        0x00, 0x0a, // 10 sec
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::Common(Error::InvalidConnectFlags(0b11001111)),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        0b00010000, // packet type
        24,         // remaining length
        0x00, 0x04, 'M' as u8, 'Q' as u8, 'T' as u8, 'T' as u8, 0x05,       // protocol (size=7)
        0b00000100, // connect flags +will
        0x00, 0x0a, // keepalive 10 sec
        0x00, // properties.len = 0
        0x00, 0x01, 't' as u8, // client_id = "t"
        0x02,      // WillProperties.len = 1
        0x01,      // PayloadFormatIndicator = true
        0x01, 0x00, // topic name = "t"
        0x01, b't', 0x00, // payload = "0xff,0xfc"
        0x02, 0xff, 0xfc,
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidPayloadFormat,
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_connack() {
    // FIXME: check remaining length in Packet::decode_async()
    let mut data: &[u8] = &[0b00100000, 3, 0x00, 0x84, 0x00];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Connack(Connack {
            session_present: false,
            reason_code: ConnectReasonCode::UnsupportedProtocolVersion,
            properties: ConnackProperties::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        0b00100000, // packet type
        11,         // remaining length
        0x00,       // session_present
        0x84,       // reason code
        0x08,       // property length
        0x24, 0x01, // maximum qos
        0x1F, 0x00, 0x03, b'a', b'b', b'c', // reason string
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Connack(Connack {
            session_present: false,
            reason_code: ConnectReasonCode::UnsupportedProtocolVersion,
            properties: ConnackProperties {
                max_qos: Some(QoS::Level1),
                reason_string: Some(Arc::new("abc".to_string())),
                ..Default::default()
            },
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    for byte0 in [2, 3, 4, 128u8] {
        let mut data: &[u8] = &[0b00100000, 2, byte0, 0x84, 0x00];
        assert_eq!(
            Packet::decode(data).unwrap_err(),
            ErrorV5::Common(Error::InvalidConnackFlags(byte0)),
        );
        assert_eq!(
            Packet::decode(data).unwrap_err(),
            block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
        );
    }

    let mut data: &[u8] = &[
        0b00100000, // packet type
        7,          // remaining length
        0x00,       // session_present
        0x84,       // reason code
        0x04,       // property length
        0x24, 0x01, // maximum qos
        0x24, 0x01, // maximum qos
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::DuplicatedProperty(PropertyId::MaximumQoS),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        0b00100000, // packet type
        5,          // remaining length
        0x00,       // session_present
        0x84,       // reason code
        0x01,       // property length
        0x25, 0x02, // retain available
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidByteProperty(PropertyId::RetainAvailable, 0x02),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_disconnect() {
    let mut data: &[u8] = &[
        14 << 4, // packet type
        7,       // remaining length
        0x89,    // reason: server busy
        0x05,    // properties.len = 5
        0x11,    // SessionExpiryInterval=0x33
        0x00,
        0x00,
        0x00,
        0x33,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Disconnect(Disconnect {
            reason_code: DisconnectReasonCode::ServerBusy,
            properties: DisconnectProperties {
                session_expiry_interval: Some(0x33),
                ..Default::default()
            }
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        14 << 4, // packet type
        2,       // remaining length
        0x89,    // reason: server busy
        0x00,    // properties.len() = 0
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Disconnect(Disconnect {
            reason_code: DisconnectReasonCode::ServerBusy,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        14 << 4, // packet type
        1,       // remaining length
        0x89,    // reason: server busy
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Disconnect(Disconnect {
            reason_code: DisconnectReasonCode::ServerBusy,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        14 << 4, // packet type
        0,       // remaining length
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Disconnect(Disconnect {
            reason_code: DisconnectReasonCode::NormalDisconnect,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );
}
#[test]
fn test_v5_decode_auth() {
    let mut data: &[u8] = &[
        15 << 4, // packet type
        7,       // remaining length
        0x18,    // reason: Continuation Authentication
        0x05,    // properties.len = 5
        0x1F,    // reason string = "xy"
        0x00,
        0x02,
        'x' as u8,
        'y' as u8,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Auth(Auth {
            reason_code: AuthReasonCode::ContinueAuthentication,
            properties: AuthProperties {
                reason_string: Some(Arc::new("xy".to_string())),
                ..Default::default()
            },
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        15 << 4, // packet type
        2,       // remaining length
        0x00,    // reason code
        0x00,    // properties.len = 0
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Auth(Auth {
            reason_code: AuthReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        15 << 4, // packet type
        0,       // remaining length
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Auth(Auth {
            reason_code: AuthReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        15 << 4, // packet type
        2,       // remaining length
        0x59,    // reason code
        0x00,    // properties.len = 0
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidReasonCode(PacketType::Auth, 0x59),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_publish() {
    let mut data: &[u8] = &[
        3 << 4, // packet type
        7,      // remaining length
        0x00,   // topic name = "xy"
        0x02,
        'x' as u8,
        'y' as u8,
        0x00, // properties.len = 0
        0xaa, // payload = "0xaa,0xbb"
        0xbb,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Publish(Publish {
            dup: false,
            qos_pid: QosPid::Level0,
            retain: false,
            topic_name: TopicName::try_from("xy".to_string()).unwrap(),
            properties: Default::default(),
            payload: Bytes::from(vec![0xaa, 0xbb]),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        3 << 4 | 0b1000, // packet type, dup = true
        10,              // remaining length
        0x00,            // topic name = "xy"
        0x02,
        'x' as u8,
        'y' as u8,
        0x03, // properties.len = 3
        0x23, // topic alias = 0x33
        0x11,
        0x33,
        0xaa, // payload = "0xaa,0xbb"
        0xbb,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Publish(Publish {
            dup: true,
            qos_pid: QosPid::Level0,
            retain: false,
            topic_name: TopicName::try_from("xy".to_string()).unwrap(),
            properties: PublishProperties {
                topic_alias: Some(0x1133),
                ..Default::default()
            },
            payload: Bytes::from(vec![0xaa, 0xbb]),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        3 << 4 | 0b0010, // packet type, qos = 1
        11,              // remaining length
        0x00,            // topic name = "xy"
        0x02,
        'x' as u8,
        'y' as u8,
        0x22, // pid = 0x2244
        0x44,
        0x02, // properties.len = 2
        0x01, // payload is utf8 = true
        0x01,
        0x61, // payload = "ab"
        0x62,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Publish(Publish {
            dup: false,
            qos_pid: QosPid::Level1(Pid::try_from(0x2244).unwrap()),
            retain: false,
            topic_name: TopicName::try_from("xy".to_string()).unwrap(),
            properties: PublishProperties {
                payload_is_utf8: Some(true),
                ..Default::default()
            },
            payload: Bytes::from("ab".as_bytes().to_vec()),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        3 << 4 | 0b0001, // packet type, retain = true
        5,               // remaining length
        0x00,            // topic name = "xy"
        0x02,
        'x' as u8,
        'y' as u8,
        0x00, // properties.len = 0
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Publish(Publish {
            dup: false,
            qos_pid: QosPid::Level0,
            retain: true,
            topic_name: TopicName::try_from("xy".to_string()).unwrap(),
            properties: Default::default(),
            payload: Bytes::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        3 << 4 | 0b1101, // packet type, dup=true, qos=2 retain=true
        7,               // remaining length
        0x00,            // topic name = "xy"
        0x02,
        'x' as u8,
        'y' as u8,
        0x11, // pid = 0x1122
        0x22,
        0x00, // properties.len = 0
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Publish(Publish {
            dup: true,
            qos_pid: QosPid::Level2(Pid::try_from(0x1122).unwrap()),
            retain: true,
            topic_name: TopicName::try_from("xy".to_string()).unwrap(),
            properties: Default::default(),
            payload: Bytes::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        3 << 4, // packet type
        6,      // remaining length
        0x00,   // topic name = "t"
        0x01,
        't' as u8,
        0x01, // properties.len = 1
        0x24, // maximum qos = 1
        0x01,
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidProperty(PacketType::Publish, PropertyId::MaximumQoS),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        3 << 4,
        2,
        0x00, // topic name = "t"
        0x01,
        't' as u8,
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::Common(Error::InvalidRemainingLength),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        3 << 4,
        8,
        0x00, // topic name = "t"
        0x01,
        't' as u8,
        0x02, // properties.len = 2
        0x01, // PayloadFormatIndicator = true
        0x01,
        0xff, // payload = "0xff,0xfc"
        0xfc,
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidPayloadFormat,
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        3 << 4,
        10,
        0x00, // topic name = "t"
        0x01,
        't' as u8,
        0x04, // properties.len = 4
        0x08, // ResponseTopic = "+"
        0x00,
        0x01,
        '+' as u8,
        0xff, // payload = "0xff,0xfc"
        0xfc,
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidResponseTopic,
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_puback() {
    let mut data: &[u8] = &[
        4 << 4, // packet type
        8,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x87, // reason code = NotAuthorized
        0x04, // properties.len = 4
        0x1F, // reason string = "e"
        0x00,
        0x01,
        'e' as u8,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Puback(Puback {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubackReasonCode::NotAuthorized,
            properties: PubackProperties {
                reason_string: Some(Arc::new("e".to_string())),
                user_properties: Vec::new(),
            },
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        4 << 4, // packet type
        2,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Puback(Puback {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubackReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        4 << 4, // packet type
        3,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x00, // reason code = success
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Puback(Puback {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubackReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );
}

#[test]
fn test_v5_decode_pubrec() {
    let mut data: &[u8] = &[
        5 << 4, // packet type
        8,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x87, // reason code = NotAuthorized
        0x04, // properties.len = 4
        0x1F, // reason string = "e"
        0x00,
        0x01,
        'e' as u8,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrec(Pubrec {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubrecReasonCode::NotAuthorized,
            properties: PubrecProperties {
                reason_string: Some(Arc::new("e".to_string())),
                user_properties: Vec::new(),
            },
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        5 << 4, // packet type
        2,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrec(Pubrec {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubrecReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        5 << 4, // packet type
        3,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x00, // reason code = success
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrec(Pubrec {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubrecReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );
}
#[test]
fn test_v5_decode_pubrel() {
    let mut data: &[u8] = &[
        6 << 4 | 2, // packet type
        8,          // remaining length
        0x11,       // packet identifier = 0x1122
        0x22,
        0x92, // reason code = PacketIdentifierNotFound
        0x04, // properties.len = 4
        0x1F, // reason string = "e"
        0x00,
        0x01,
        'e' as u8,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrel(Pubrel {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubrelReasonCode::PacketIdentifierNotFound,
            properties: PubrelProperties {
                reason_string: Some(Arc::new("e".to_string())),
                user_properties: Vec::new(),
            },
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        6 << 4 | 2, // packet type
        2,          // remaining length
        0x11,       // packet identifier = 0x1122
        0x22,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrel(Pubrel {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubrelReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        6 << 4 | 2, // packet type
        3,          // remaining length
        0x11,       // packet identifier = 0x1122
        0x22,
        0x00, // reason code = success
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubrel(Pubrel {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubrelReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );
}
#[test]
fn test_v5_decode_pubcomp() {
    let mut data: &[u8] = &[
        7 << 4, // packet type
        8,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x92, // reason code = PacketIdentifierNotFound
        0x04, // properties.len = 4
        0x1F, // reason string = "e"
        0x00,
        0x01,
        'e' as u8,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubcomp(Pubcomp {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubcompReasonCode::PacketIdentifierNotFound,
            properties: PubcompProperties {
                reason_string: Some(Arc::new("e".to_string())),
                user_properties: Vec::new(),
            },
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        7 << 4, // packet type
        2,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubcomp(Pubcomp {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubcompReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        7 << 4, // packet type
        3,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x00, // reason code = success
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Pubcomp(Pubcomp {
            pid: Pid::try_from(0x1122).unwrap(),
            reason_code: PubcompReasonCode::Success,
            properties: Default::default(),
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );
}

#[test]
fn test_v5_decode_subscribe() {
    let mut data: &[u8] = &[
        8 << 4 | 2, // packet type
        11,         // remaining length
        0x11,       // packet identifier = 0x1122
        0x22,
        0x03, // properties.len = 3
        0x0B, // subscription identifier = 16,383
        0xFF,
        0x7F,
        0x00, // topic filter = "/+"
        0x02,
        '/' as u8,
        '+' as u8,
        0x00, // options = max_qos=0, no_local=false, retain_as_published=false, retain_handling=SendAtSubscribe
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Subscribe(Subscribe {
            pid: Pid::try_from(0x1122).unwrap(),
            properties: SubscribeProperties {
                subscription_id: Some(VarByteInt::try_from(16383).unwrap()),
                user_properties: Vec::new(),
            },
            topics: vec![(
                TopicFilter::try_from("/+".to_string()).unwrap(),
                SubscriptionOptions {
                    max_qos: QoS::Level0,
                    no_local: false,
                    retain_as_published: false,
                    retain_handling: RetainHandling::SendAtSubscribe,
                }
            )],
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        8 << 4 | 2, // packet type
        8,          // remaining length
        0x11,       // packet identifier = 0x1122
        0x22,
        0x00, // properties.len = 0
        0x00, // topic filter = "/+"
        0x02,
        '/' as u8,
        '+' as u8,
        0b00101110, // options = max_qos=2, no_local=true, retain_as_published=true, retain_handling=DoNotSend
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Subscribe(Subscribe {
            pid: Pid::try_from(0x1122).unwrap(),
            properties: Default::default(),
            topics: vec![(
                TopicFilter::try_from("/+".to_string()).unwrap(),
                SubscriptionOptions {
                    max_qos: QoS::Level2,
                    no_local: true,
                    retain_as_published: true,
                    retain_handling: RetainHandling::DoNotSend,
                }
            )],
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    for opt_byte in [
        0b01000000, // reserved bits
        0b00000011, // max_qos=3
        0b00110000, // RetainHandling=3
    ] {
        let mut data: &[u8] = &[
            8 << 4 | 2, // packet type
            8,          // remaining length
            0x11,       // packet identifier = 0x1122
            0x22,
            0x00, // properties.len = 0
            0x00, // topic filter = "/+"
            0x02,
            '/' as u8,
            '+' as u8,
            opt_byte,
        ];
        assert_eq!(
            Packet::decode(data).unwrap_err(),
            ErrorV5::InvalidSubscriptionOption(opt_byte),
        );
        assert_eq!(
            Packet::decode(data).unwrap_err(),
            block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
        );
    }

    let mut data: &[u8] = &[
        8 << 4 | 2, // packet type
        3,          // remaining length
        0x11,       // packet identifier = 0x1122
        0x22,
        0x00, // properties.len = 0
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::Common(Error::EmptySubscription),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_suback() {
    let mut data: &[u8] = &[
        9 << 4, // packet type
        9,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x04, // properties.len = 4
        0x1F, // reason string = "e"
        0x00,
        0x01,
        'e' as u8,
        0x83, // SubscribeReasonCode = ImplementationSpecificError
        0x97, // SubscribeReasonCode = QuotaExceeded
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Suback(Suback {
            pid: Pid::try_from(0x1122).unwrap(),
            properties: SubackProperties {
                reason_string: Some(Arc::new("e".to_string())),
                user_properties: Vec::new(),
            },
            topics: vec![
                SubscribeReasonCode::ImplementationSpecificError,
                SubscribeReasonCode::QuotaExceeded
            ]
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        9 << 4, // packet type
        4,      // remaining length
        0x11,   // packet identifier = 0x1122
        0x22,
        0x00, // properties.len = 0
        0x43, // InvalidReasonCode(0x43)
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidReasonCode(PacketType::Suback, 0x43),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}
#[test]
fn test_v5_decode_unsubscribe() {
    let mut data: &[u8] = &[
        10 << 4 | 2, // packet type
        28,          // remaining length
        0x11,        // packet identifier = 0x1122
        0x22,
        0x12, // properties.len = 18
        0x26, // UserProperty { name: "k1", value: "v1" }
        0x00,
        0x02,
        'k' as u8,
        '1' as u8,
        0x00,
        0x02,
        'v' as u8,
        '1' as u8,
        0x26, // UserProperty { name: "k2", value: "v2" }
        0x00,
        0x02,
        'k' as u8,
        '2' as u8,
        0x00,
        0x02,
        'v' as u8,
        '2' as u8,
        0x00, // topic filter = "/+"
        0x02,
        '/' as u8,
        '+' as u8,
        0x00, // topic filter = "/"
        0x01,
        '/' as u8,
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Unsubscribe(Unsubscribe {
            pid: Pid::try_from(0x1122).unwrap(),
            properties: vec![
                UserProperty {
                    name: Arc::new("k1".to_string()),
                    value: Arc::new("v1".to_string()),
                },
                UserProperty {
                    name: Arc::new("k2".to_string()),
                    value: Arc::new("v2".to_string()),
                },
            ],
            topics: vec![
                TopicFilter::try_from("/+".to_string()).unwrap(),
                TopicFilter::try_from("/".to_string()).unwrap(),
            ],
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        10 << 4 | 2, // packet type
        4,           // remaining length
        0x11,        // packet identifier = 0x1122
        0x22,
        0x02, // properties.len = 2
        0x27, // InvalidProperty = MaximumPacketSize
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidProperty(PacketType::Unsubscribe, PropertyId::MaximumPacketSize),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );

    let mut data: &[u8] = &[
        10 << 4 | 2, // packet type
        4,           // remaining length
        0x11,        // packet identifier = 0x1122
        0x22,
        0x02, // properties.len = 2
        0xAA, // InvalidPropertyId(0xAA)
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidPropertyId(0xAA),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_unsuback() {
    let mut data: &[u8] = &[
        11 << 4, // packet type
        9,       // remaining length
        0x11,    // packet identifier = 0x1122
        0x22,
        0x04, // properties.len = 4
        0x1F, // reason string = "e"
        0x00,
        0x01,
        'e' as u8,
        0x00, // SubscribeReasonCode = Success
        0x8F, // SubscribeReasonCode = TopicFilterInvalid
    ];
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        Packet::Unsuback(Unsuback {
            pid: Pid::try_from(0x1122).unwrap(),
            properties: UnsubackProperties {
                reason_string: Some(Arc::new("e".to_string())),
                user_properties: Vec::new(),
            },
            topics: vec![
                UnsubscribeReasonCode::Success,
                UnsubscribeReasonCode::TopicFilterInvalid,
            ]
        })
    );
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[
        11 << 4, // packet type
        4,       // remaining length
        0x11,    // packet identifier = 0x1122
        0x22,
        0x00, // properties.len = 0
        0x43, // InvalidReasonCode(0x43)
    ];
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        ErrorV5::InvalidReasonCode(PacketType::Unsuback, 0x43),
    );
    assert_eq!(
        Packet::decode(data).unwrap_err(),
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap_err()
    );
}

#[test]
fn test_v5_decode_pingreq() {
    let mut data: &[u8] = &[12 << 4, 0];
    assert_eq!(Packet::decode(data).unwrap().unwrap(), Packet::Pingreq);
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[12 << 4, 0, 0x11, 0x22];
    assert_eq!(Packet::decode(data).unwrap().unwrap(), Packet::Pingreq);
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );
}

#[test]
fn test_v5_decode_pingresp() {
    let mut data: &[u8] = &[13 << 4, 0];
    assert_eq!(Packet::decode(data).unwrap().unwrap(), Packet::Pingresp);
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );

    let mut data: &[u8] = &[13 << 4, 0, 0x11, 0x22];
    assert_eq!(Packet::decode(data).unwrap().unwrap(), Packet::Pingresp);
    assert_eq!(
        Packet::decode(data).unwrap().unwrap(),
        block_on(PollPacket::new(&mut Default::default(), &mut data))
            .unwrap()
            .1,
    );
}
