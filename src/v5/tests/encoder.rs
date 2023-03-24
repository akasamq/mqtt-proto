use std::mem;
use std::sync::Arc;

use bytes::Bytes;
use futures_lite::future::block_on;

use crate::v5::*;
use crate::*;

fn assert_encode(pkt: Packet, len: usize) {
    let mut data_async = Vec::new();
    block_on(pkt.encode_async(&mut data_async)).unwrap();
    let var_bytes = pkt.encode().unwrap();
    assert_eq!(var_bytes.as_ref(), &data_async);
    assert_eq!(pkt.encode_len().unwrap(), len);
    assert_eq!(data_async.len(), len);

    let decoded_pkt = Packet::decode(&data_async).unwrap().unwrap();
    assert_eq!(pkt, decoded_pkt);

    let mut data = &data_async[..];
    let (total, buf, polled_pkt) =
        block_on(PollPacket::new(&mut Default::default(), &mut data)).unwrap();
    let buf_ref: &[u8] = unsafe { mem::transmute(&buf[..]) };
    assert_eq!(total, len);
    assert_eq!(buf_ref, &data_async[header_len(total)..]);
    assert_eq!(pkt, polled_pkt);
}

#[test]
fn test_v5_encode_pingreq() {
    assert_encode(Packet::Pingreq, 2);
}
#[test]
fn test_v5_encode_pingresp() {
    assert_encode(Packet::Pingresp, 2);
}

#[test]
fn test_v5_encode_connect() {
    let packet = Connect {
        // 2 + 4 + 1 = 7
        protocol: Protocol::V500,
        // 1
        clean_start: true,
        // 2
        keep_alive: 33,
        // 1 + 1 + 2 = 4
        properties: ConnectProperties {
            receive_max: Some(22),
            ..Default::default()
        },
        // 2 + 11 = 13
        client_id: Arc::new("client no.1".to_string()),
        // 8 + 5 + 4 = 17
        last_will: Some(LastWill {
            qos: QoS::Level1,
            retain: false,
            // 1 + 1 + 2 + 4 = 8
            properties: WillProperties {
                content_type: Some(Arc::new("text".to_string())),
                ..Default::default()
            },
            // 2 + 3 = 5
            topic_name: TopicName::try_from("a/b".to_string()).unwrap(),
            // 2 + 2 = 4
            payload: Bytes::from(vec![0u8, 1u8]),
        }),
        // 2 + 6 = 8
        username: Some(Arc::new("nahida".to_string())),
        // 2 + 2 = 4
        password: Some(Bytes::from(vec![3u8, 4u8])),
    };

    let len = [
        2,  // control byte + remaining length
        7,  // protocol
        1,  // connect flags
        2,  // keep alive
        4,  // properties
        13, // client identifier
        17, // last will
        8,  // username
        4,  // password
    ]
    .into_iter()
    .sum();
    assert_eq!(len, 58);
    assert_encode(packet.clone().into(), len);

    let packet_large = Connect {
        client_id: Arc::new("a".repeat(128).to_string()),
        ..packet
    };
    let len = [
        3,   // control byte + remaining length
        7,   // protocol
        1,   // connect flags
        2,   // keep alive
        4,   // properties
        130, // client identifier
        17,  // last will
        8,   // username
        4,   // password
    ]
    .into_iter()
    .sum();
    assert_eq!(len, 176);
    assert_encode(packet_large.into(), len);
}

#[test]
fn test_v5_encode_connack() {
    let packet = Connack {
        session_present: true,
        reason_code: ConnectReasonCode::MalformedPacket,
        // 1 + 6 + 14 = 21
        properties: ConnackProperties {
            // 1 + 2 + 3 = 6
            auth_method: Some(Arc::new("tls".to_string())),
            // 1 + 4 + 9 = 14
            user_properties: vec![UserProperty {
                name: Arc::new("name".to_string()),
                value: Arc::new("value".to_string()),
            }],
            ..Default::default()
        },
    };

    let len = [
        2,  // header
        1,  // connack flags
        1,  // reason code
        21, // properties
    ]
    .into_iter()
    .sum();
    assert_eq!(len, 25);
    assert_encode(packet.into(), len);
}

#[test]
fn test_v5_encode_disconnect() {
    let packet = Disconnect {
        // 1
        reason_code: DisconnectReasonCode::NotAuthorized,
        // 1 + 5 + 7 = 13
        properties: DisconnectProperties {
            // 1 + 4 = 5
            session_expiry_interval: Some(456),
            // 1 + 2 + 4 = 7
            server_reference: Some(Arc::new("http".to_string())),
            ..Default::default()
        },
    };
    let len = [
        2,  // header
        1,  // reason code
        13, // properties
    ]
    .into_iter()
    .sum();
    assert_eq!(len, 16);
    assert_encode(packet.into(), len);

    let packet1 = Disconnect {
        reason_code: DisconnectReasonCode::NotAuthorized,
        properties: Default::default(),
    };
    assert_encode(packet1.into(), 3);

    let packet2 = Disconnect {
        reason_code: DisconnectReasonCode::NormalDisconnect,
        properties: Default::default(),
    };
    assert_encode(packet2.into(), 2);

    // SUMMARY: libFuzzer: deadly signal
    //     MS: 1 CrossOver-; base unit: 19e986892fb058b7e2fdf069390b2ef591738747
    //     0xc0,0xdc,0xa4,0xe2,0xd5,0xfc,0xff,0x26,
    // \300\334\244\342\325\374\377&
    //     artifact_prefix='../fuzz/artifacts/mqtt_v5_arbitrary/'; Test unit written to ../fuzz/artifacts/mqtt_v5_arbitrary/crash-4f2714598b45a6081720cbe109372e984f1c28ff
    //     Base64: wNyk4tX8/yY=
    let packet3 = Disconnect {
        reason_code: DisconnectReasonCode::ProtocolError,
        properties: DisconnectProperties {
            session_expiry_interval: None,
            reason_string: None,
            user_properties: Vec::new(),
            server_reference: None,
        },
    };
    assert_encode(packet3.into(), 3);
}

#[test]
fn test_v5_encode_auth() {
    let packet = Auth {
        // 1
        reason_code: AuthReasonCode::Success,
        // 1 + 8 = 9
        properties: AuthProperties {
            // 1 + 2 + 5 = 8
            reason_string: Some(Arc::new("error".to_string())),
            ..Default::default()
        },
    };
    let len = [
        2, // header
        1, // reason code
        9, // properties
    ]
    .into_iter()
    .sum();
    assert_eq!(len, 12);
    assert_encode(packet.into(), len);

    let packet1 = Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: Default::default(),
    };
    assert_encode(packet1.into(), 4);

    let packet2 = Auth {
        reason_code: AuthReasonCode::Success,
        properties: Default::default(),
    };
    assert_encode(packet2.into(), 2);
}

#[test]
fn test_v5_encode_publish() {
    let packet = Publish {
        dup: false,
        // 2
        qos_pid: QosPid::Level1(Pid::try_from(10).unwrap()),
        retain: false,
        // 2 + 3 = 5
        topic_name: TopicName::try_from("a/b".to_string()).unwrap(),
        // 1 + 3 + 5 = 9
        properties: PublishProperties {
            // 1 + 2 = 3
            topic_alias: Some(23),
            // 1 + 2 + 2 = 5
            correlation_data: Some(Bytes::from(vec![0u8, 1])),
            ..Default::default()
        },
        // 3
        payload: Bytes::from(vec![1u8, 2u8, 3u8]),
    };
    let len = [
        2, // header
        2, // packet identifier
        5, // topic name
        9, // properties
        3, // payload
    ]
    .into_iter()
    .sum();
    assert_encode(packet.clone().into(), len);

    let packet1 = Publish {
        properties: Default::default(),
        payload: Bytes::default(),
        ..packet.clone()
    };
    let len = [2, 2, 5, 1, 0].into_iter().sum();
    assert_encode(packet1.clone().into(), len);

    let packet2 = Publish {
        qos_pid: QosPid::Level0,
        ..packet1
    };
    let len = [2, 0, 5, 1, 0].into_iter().sum();
    assert_encode(packet2.clone().into(), len);

    let packet3 = Publish {
        properties: PublishProperties {
            payload_is_utf8: Some(true),
            ..Default::default()
        },
        payload: Bytes::from("abc".as_bytes().to_vec()),
        ..packet
    };
    let len = [2, 2, 5, 3, 3].into_iter().sum();
    assert_encode(packet3.clone().into(), len);
}

#[test]
fn test_v5_encode_puback() {
    let packet = Puback {
        // 2
        pid: Pid::try_from(10).unwrap(),
        // 1
        reason_code: PubackReasonCode::ImplementationSpecificError,
        // 1 + 7 + 27 = 35
        properties: PubackProperties {
            // 1 + 2 + 4 = 7
            reason_string: Some(Arc::new("auth".to_string())),
            // 13 + 14 = 27
            user_properties: vec![
                // 1 + 4 + 9 = 14
                UserProperty {
                    name: Arc::new("name".to_string()),
                    value: Arc::new("value".to_string()),
                },
                // 1 + 4 + 8 = 13
                UserProperty {
                    name: Arc::new("key".to_string()),
                    value: Arc::new("value".to_string()),
                },
            ],
        },
    };
    let len = [
        2,  // header
        2,  // packet identifier
        1,  // reason code
        35, // properties
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);

    let packet1 = Puback {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubackReasonCode::ImplementationSpecificError,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
        1, // reason code
    ]
    .into_iter()
    .sum();
    assert_encode(packet1.into(), len);

    let packet2 = Puback {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubackReasonCode::Success,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
    ]
    .into_iter()
    .sum();
    assert_encode(packet2.into(), len);
}

#[test]
fn test_v5_encode_pubrec() {
    let packet = Pubrec {
        // 2
        pid: Pid::try_from(10).unwrap(),
        // 1
        reason_code: PubrecReasonCode::ImplementationSpecificError,
        // 1 + 7 + 27 = 35
        properties: PubrecProperties {
            // 1 + 2 + 4 = 7
            reason_string: Some(Arc::new("auth".to_string())),
            // 13 + 14 = 27
            user_properties: vec![
                // 1 + 4 + 9 = 14
                UserProperty {
                    name: Arc::new("name".to_string()),
                    value: Arc::new("value".to_string()),
                },
                // 1 + 4 + 8 = 13
                UserProperty {
                    name: Arc::new("key".to_string()),
                    value: Arc::new("value".to_string()),
                },
            ],
        },
    };
    let len = [
        2,  // header
        2,  // packet identifier
        1,  // reason code
        35, // properties
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);

    let packet1 = Pubrec {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubrecReasonCode::ImplementationSpecificError,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
        1, // reason code
    ]
    .into_iter()
    .sum();
    assert_encode(packet1.into(), len);

    let packet2 = Pubrec {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubrecReasonCode::Success,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
    ]
    .into_iter()
    .sum();
    assert_encode(packet2.into(), len);
}

#[test]
fn test_v5_encode_pubrel() {
    let packet = Pubrel {
        // 2
        pid: Pid::try_from(10).unwrap(),
        // 1
        reason_code: PubrelReasonCode::PacketIdentifierNotFound,
        // 1 + 7 + 27 = 35
        properties: PubrelProperties {
            // 1 + 2 + 4 = 7
            reason_string: Some(Arc::new("auth".to_string())),
            // 13 + 14 = 27
            user_properties: vec![
                // 1 + 4 + 9 = 14
                UserProperty {
                    name: Arc::new("name".to_string()),
                    value: Arc::new("value".to_string()),
                },
                // 1 + 4 + 8 = 13
                UserProperty {
                    name: Arc::new("key".to_string()),
                    value: Arc::new("value".to_string()),
                },
            ],
        },
    };
    let len = [
        2,  // header
        2,  // packet identifier
        1,  // reason code
        35, // properties
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);

    let packet1 = Pubrel {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubrelReasonCode::PacketIdentifierNotFound,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
        1, // reason code
    ]
    .into_iter()
    .sum();
    assert_encode(packet1.into(), len);

    let packet2 = Pubrel {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubrelReasonCode::Success,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
    ]
    .into_iter()
    .sum();
    assert_encode(packet2.into(), len);
}

#[test]
fn test_v5_encode_pubcomp() {
    let packet = Pubcomp {
        // 2
        pid: Pid::try_from(10).unwrap(),
        // 1
        reason_code: PubcompReasonCode::PacketIdentifierNotFound,
        // 1 + 7 + 27 = 35
        properties: PubcompProperties {
            // 1 + 2 + 4 = 7
            reason_string: Some(Arc::new("auth".to_string())),
            // 13 + 14 = 27
            user_properties: vec![
                // 1 + 4 + 9 = 14
                UserProperty {
                    name: Arc::new("name".to_string()),
                    value: Arc::new("value".to_string()),
                },
                // 1 + 4 + 8 = 13
                UserProperty {
                    name: Arc::new("key".to_string()),
                    value: Arc::new("value".to_string()),
                },
            ],
        },
    };
    let len = [
        2,  // header
        2,  // packet identifier
        1,  // reason code
        35, // properties
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);

    let packet1 = Pubcomp {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubcompReasonCode::PacketIdentifierNotFound,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
        1, // reason code
    ]
    .into_iter()
    .sum();
    assert_encode(packet1.into(), len);

    let packet2 = Pubcomp {
        pid: Pid::try_from(10).unwrap(),
        reason_code: PubcompReasonCode::Success,
        properties: Default::default(),
    };
    let len = [
        2, // header
        2, // packet identifier
    ]
    .into_iter()
    .sum();
    assert_encode(packet2.into(), len);
}

#[test]
fn test_v5_encode_subscribe() {
    let packet = Subscribe {
        // 2
        pid: Pid::try_from(10).unwrap(),
        // 1 + 3 = 4
        properties: SubscribeProperties {
            // 1 + 2 = 3
            subscription_id: Some(VarByteInt::try_from(3344).unwrap()),
            user_properties: Vec::new(),
        },
        // 5 + 1 = 6
        topics: vec![(
            // 2 + 3 = 5
            TopicFilter::try_from("a/+".to_string()).unwrap(),
            // 1
            SubscriptionOptions {
                max_qos: QoS::Level1,
                no_local: true,
                retain_as_published: false,
                retain_handling: RetainHandling::SendAtSubscribe,
            },
        )],
    };
    let len = [
        2, // header
        2, // packet identifier
        4, // properties
        6, // topics
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);
}

#[test]
fn test_v5_encode_suback() {
    let packet = Suback {
        // 2
        pid: Pid::try_from(10).unwrap(),
        // 1 + 7 = 8
        properties: SubackProperties {
            // 1 + 2 + 4 = 7
            reason_string: Some(Arc::new("warn".to_string())),
            user_properties: Vec::new(),
        },
        // 1
        topics: vec![SubscribeReasonCode::GrantedQoS2],
    };
    let len = [
        2, // header
        2, // packet identifier
        8, // properties
        1, // topics
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);
}

#[test]
fn test_v5_encode_unsubscribe() {
    let packet = Unsubscribe {
        pid: Pid::try_from(10).unwrap(),
        // 1 + 14 = 15
        properties: vec![
            // 1 + 4 + 9 = 14
            UserProperty {
                name: Arc::new("name".to_string()),
                value: Arc::new("value".to_string()),
            },
        ]
        .into(),
        // 5 + 3 = 8
        topics: vec![
            // 2 + 3 = 5
            TopicFilter::try_from("a/+".to_string()).unwrap(),
            // 2 + 1 = 3
            TopicFilter::try_from("b".to_string()).unwrap(),
        ],
    };
    let len = [
        2,  // header
        2,  // packet identifier
        15, // properties
        8,  // topics
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);

    let packet1 = Unsubscribe {
        pid: Pid::try_from(10).unwrap(),
        // 1 + 14 + 13 = 28
        properties: vec![
            // 1 + 4 + 9 = 14
            UserProperty {
                name: Arc::new("name".to_string()),
                value: Arc::new("value".to_string()),
            },
            // 1 + 4 + 8 = 13
            UserProperty {
                name: Arc::new("key".to_string()),
                value: Arc::new("value".to_string()),
            },
        ]
        .into(),
        // 5 + 3 = 8
        topics: vec![
            // 2 + 3 = 5
            TopicFilter::try_from("a/+".to_string()).unwrap(),
            // 2 + 1 = 3
            TopicFilter::try_from("b".to_string()).unwrap(),
        ],
    };
    let len = [
        2,  // header
        2,  // packet identifier
        28, // properties
        8,  // topics
    ]
    .into_iter()
    .sum();
    assert_encode(packet1.into(), len);
}

#[test]
fn test_v5_encode_unsuback() {
    let packet = Unsuback {
        // 2
        pid: Pid::try_from(10).unwrap(),
        // 1 + 7 = 8
        properties: UnsubackProperties {
            // 1 + 2 + 4 = 7
            reason_string: Some(Arc::new("warn".to_string())),
            user_properties: Vec::new(),
        },
        // 1
        topics: vec![UnsubscribeReasonCode::UnspecifiedError],
    };
    let len = [
        2, // header
        2, // packet identifier
        8, // properties
        1, // topics
    ]
    .into_iter()
    .sum();
    assert_encode(packet.into(), len);
}
