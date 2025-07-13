#![feature(test)]

extern crate test;

use std::convert::TryFrom;
use std::sync::Arc;

use test::{black_box, Bencher};

use mqtt_proto::{
    v3::{Connect as ConnectV3, LastWill as LastWillV3, Packet as PacketV3},
    v5::{Connect as ConnectV5, LastWill as LastWillV5, Packet as PacketV5},
    Pid, QoS, QosPid, TopicName,
};

fn create_v3_connect_packet() -> PacketV3 {
    let client_id = Arc::new("test-client".to_string());
    let mut connect = ConnectV3::new(client_id, 30);
    connect.last_will = Some(LastWillV3 {
        qos: QoS::Level1,
        retain: false,
        topic_name: TopicName::try_from("will/topic".to_string()).unwrap(),
        message: "will message".into(),
    });
    connect.username = Some(Arc::new("user".to_string()));
    connect.password = Some("password".into());
    PacketV3::Connect(connect)
}

fn create_v5_connect_packet() -> PacketV5 {
    let client_id = Arc::new("test-client-v5".to_string());
    let mut connect = ConnectV5::new(client_id, 60);
    connect.last_will = Some(LastWillV5 {
        qos: QoS::Level2,
        retain: true,
        topic_name: TopicName::try_from("will/topic/v5".to_string()).unwrap(),
        payload: "will message v5".into(),
        properties: Default::default(),
    });
    connect.username = Some(Arc::new("user-v5".to_string()));
    connect.password = Some("password-v5".into());
    PacketV5::Connect(connect)
}

fn create_v3_publish_packet() -> PacketV3 {
    PacketV3::Publish(mqtt_proto::v3::Publish {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level1(Pid::try_from(10).unwrap()),
        topic_name: TopicName::try_from("test/topic".to_string()).unwrap(),
        payload: "hello world".into(),
    })
}

fn create_v5_publish_packet() -> PacketV5 {
    PacketV5::Publish(mqtt_proto::v5::Publish {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level2(Pid::try_from(20).unwrap()),
        topic_name: TopicName::try_from("test/topic/v5".to_string()).unwrap(),
        payload: "hello world v5".into(),
        properties: Default::default(),
    })
}

#[bench]
fn v3_connect_encode(b: &mut Bencher) {
    let packet = create_v3_connect_packet();
    b.iter(|| black_box(packet.encode()));
}

#[bench]
fn v5_connect_encode(b: &mut Bencher) {
    let packet = create_v5_connect_packet();
    b.iter(|| black_box(packet.encode()));
}

#[bench]
fn v3_publish_encode(b: &mut Bencher) {
    let packet = create_v3_publish_packet();
    b.iter(|| black_box(packet.encode()));
}

#[bench]
fn v5_publish_encode(b: &mut Bencher) {
    let packet = create_v5_publish_packet();
    b.iter(|| black_box(packet.encode()));
}

#[bench]
fn v3_connect_decode(b: &mut Bencher) {
    let packet = create_v3_connect_packet();
    let bytes = packet.encode().unwrap();
    b.iter(|| black_box(PacketV3::decode(bytes.as_ref())));
}

#[bench]
fn v5_connect_decode(b: &mut Bencher) {
    let packet = create_v5_connect_packet();
    let bytes = packet.encode().unwrap();
    b.iter(|| black_box(PacketV5::decode(bytes.as_ref())));
}

#[bench]
fn v3_publish_decode(b: &mut Bencher) {
    let packet = create_v3_publish_packet();
    let bytes = packet.encode().unwrap();
    b.iter(|| black_box(PacketV3::decode(bytes.as_ref())));
}

#[bench]
fn v5_publish_decode(b: &mut Bencher) {
    let packet = create_v5_publish_packet();
    let bytes = packet.encode().unwrap();
    b.iter(|| black_box(PacketV5::decode(bytes.as_ref())));
}
