use core::mem;

use alloc::sync::Arc;

use bytes::Bytes;

use crate::v3::*;
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
fn test_encode_connect() {
    let packet = Connect {
        protocol: Protocol::V311,
        keep_alive: 120,
        client_id: Arc::new("sample".to_owned()),
        clean_session: true,
        last_will: None,
        username: None,
        password: None,
    };
    assert_encode(packet.into(), 20);

    let packet = Connect {
        protocol: Protocol::V311,
        keep_alive: 120,
        client_id: Arc::new("sample".to_owned()),
        clean_session: true,
        last_will: Some(LastWill {
            qos: QoS::Level1,
            retain: true,
            topic_name: TopicName::try_from("abc".to_owned()).unwrap(),
            message: Bytes::from("msg-content"),
        }),
        username: Some(Arc::new("username".to_owned())),
        password: Some(Bytes::from("password")),
    };
    assert_encode(packet.into(), 58);

    let packet = Connect {
        protocol: Protocol::V310,
        keep_alive: 120,
        client_id: Arc::new("sample".to_owned()),
        clean_session: true,
        last_will: None,
        username: None,
        password: None,
    };
    assert_encode(packet.into(), 22);
}

#[test]
fn test_encode_connack() {
    let packet = Connack {
        session_present: true,
        code: ConnectReturnCode::Accepted,
    };
    assert_encode(packet.into(), 4);
}

#[test]
fn test_encode_publish() {
    let packet = Publish {
        dup: false,
        qos_pid: QosPid::Level2(Pid::try_from(10).unwrap()),
        retain: true,
        topic_name: TopicName::try_from("asdf".to_owned()).unwrap(),
        payload: Bytes::from(b"hello".to_vec()),
    };
    assert_encode(packet.into(), 15);
}

#[test]
fn test_encode_puback() {
    let packet = Packet::Puback(Pid::try_from(19).unwrap());
    assert_encode(packet, 4);
}

#[test]
fn test_encode_pubrec() {
    let packet = Packet::Pubrec(Pid::try_from(19).unwrap());
    assert_encode(packet, 4);
}

#[test]
fn test_encode_pubrel() {
    let packet = Packet::Pubrel(Pid::try_from(19).unwrap());
    assert_encode(packet, 4);
}

#[test]
fn test_encode_pubcomp() {
    let packet = Packet::Pubcomp(Pid::try_from(19).unwrap());
    assert_encode(packet, 4);
}

#[test]
fn test_encode_subscribe() {
    let packet = Subscribe::new(
        Pid::try_from(345).unwrap(),
        vec![(
            TopicFilter::try_from("a/b".to_owned()).unwrap(),
            QoS::Level2,
        )],
    );
    assert_encode(packet.into(), 10);
}

#[test]
fn test_encode_suback() {
    let packet = Suback::new(
        Pid::try_from(12321).unwrap(),
        vec![SubscribeReturnCode::MaxLevel2],
    );
    assert_encode(packet.into(), 5);
}

#[test]
fn test_encode_unsubscribe() {
    let packet = Unsubscribe::new(
        Pid::try_from(12321).unwrap(),
        vec![(TopicFilter::try_from("a/b".to_owned()).unwrap())],
    );
    assert_encode(packet.into(), 9);
}

#[test]
fn test_encode_unsuback() {
    let packet = Packet::Unsuback(Pid::try_from(19).unwrap());
    assert_encode(packet, 4);
}

#[test]
fn test_encode_ping_req() {
    assert_encode(Packet::Pingreq, 2);
}

#[test]
fn test_encode_ping_resp() {
    assert_encode(Packet::Pingresp, 2);
}

#[test]
fn test_encode_disconnect() {
    assert_encode(Packet::Disconnect, 2);
}
