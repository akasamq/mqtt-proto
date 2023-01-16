use std::sync::Arc;

use bytes::Bytes;
use futures_lite::future::block_on;

use crate::*;

fn assert_decode(pkt: Packet, len: usize) {
    let mut data_async = Vec::new();
    block_on(pkt.encode_async(&mut data_async)).unwrap();
    let var_bytes = pkt.encode().unwrap();
    assert_eq!(var_bytes.as_slice(), &data_async);
    assert_eq!(pkt.encode_len().unwrap(), len);
    assert_eq!(data_async.len(), len);

    let decoded_pkt = Packet::decode(&data_async).unwrap().unwrap();
    assert_eq!(pkt, decoded_pkt);
}

#[test]
fn test_encode_connect() {
    let packet = Connect {
        protocol: Protocol::MqttV311,
        keep_alive: 120,
        client_id: Arc::new("imvj".to_owned()),
        clean_session: true,
        last_will: None,
        username: None,
        password: None,
    };
    assert_decode(packet.into(), 18);

    let packet = Connect {
        protocol: Protocol::MqttV31,
        keep_alive: 120,
        client_id: Arc::new("imvj".to_owned()),
        clean_session: true,
        last_will: None,
        username: None,
        password: None,
    };
    assert_decode(packet.into(), 20);
}

#[test]
fn test_encode_connack() {
    let packet = Connack {
        session_present: true,
        code: ConnectReturnCode::Accepted,
    };
    assert_decode(packet.into(), 4);
}

#[test]
fn test_encode_publish() {
    let packet = Publish {
        dup: false,
        qos_pid: QosPid::Level2(Pid::new(10)),
        retain: true,
        topic_name: TopicName::try_from("asdf".to_owned()).unwrap(),
        payload: Bytes::from(b"hello".to_vec()),
    };
    assert_decode(packet.into(), 15);
}

#[test]
fn test_encode_puback() {
    let packet = Packet::Puback(Pid::new(19));
    assert_decode(packet, 4);
}

#[test]
fn test_encode_pubrec() {
    let packet = Packet::Pubrec(Pid::new(19));
    assert_decode(packet, 4);
}

#[test]
fn test_encode_pubrel() {
    let packet = Packet::Pubrel(Pid::new(19));
    assert_decode(packet, 4);
}

#[test]
fn test_encode_pubcomp() {
    let packet = Packet::Pubcomp(Pid::new(19));
    assert_decode(packet, 4);
}

#[test]
fn test_encode_subscribe() {
    let packet = Subscribe::new(
        Pid::new(345),
        vec![(
            TopicFilter::try_from("a/b".to_owned()).unwrap(),
            QoS::Level2,
        )],
    );
    assert_decode(packet.into(), 10);
}

#[test]
fn test_encode_suback() {
    let packet = Suback::new(Pid::new(12321), vec![SubscribeReturnCode::MaxLevel2]);
    assert_decode(packet.into(), 5);
}

#[test]
fn test_encode_unsubscribe() {
    let packet = Unsubscribe::new(
        Pid::new(12321),
        vec![(TopicFilter::try_from("a/b".to_owned()).unwrap())],
    );
    assert_decode(packet.into(), 9);
}

#[test]
fn test_encode_unsuback() {
    let packet = Packet::Unsuback(Pid::new(19));
    assert_decode(packet, 4);
}

#[test]
fn test_encode_ping_req() {
    assert_decode(Packet::Pingreq, 2);
}

#[test]
fn test_encode_ping_resp() {
    assert_decode(Packet::Pingresp, 2);
}

#[test]
fn test_encode_disconnect() {
    assert_decode(Packet::Disconnect, 2);
}
