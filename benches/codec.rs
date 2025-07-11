use std::convert::TryFrom;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
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

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("Encode");

    let v3_connect = create_v3_connect_packet();
    group.bench_function("v3_connect_encode", |b| {
        b.iter(|| v3_connect.encode())
    });

    let v5_connect = create_v5_connect_packet();
    group.bench_function("v5_connect_encode", |b| {
        b.iter(|| v5_connect.encode())
    });

    let v3_publish = PacketV3::Publish(mqtt_proto::v3::Publish {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level1(Pid::try_from(10).unwrap()),
        topic_name: TopicName::try_from("test/topic".to_string()).unwrap(),
        payload: "hello world".into(),
    });
    group.bench_function("v3_publish_encode", |b| {
        b.iter(|| v3_publish.encode())
    });

    let v5_publish = PacketV5::Publish(mqtt_proto::v5::Publish {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level2(Pid::try_from(20).unwrap()),
        topic_name: TopicName::try_from("test/topic/v5".to_string()).unwrap(),
        payload: "hello world v5".into(),
        properties: Default::default(),
    });
    group.bench_function("v5_publish_encode", |b| {
        b.iter(|| v5_publish.encode())
    });

    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("Decode");

    let v3_connect_packet = create_v3_connect_packet();
    let v3_connect_bytes = v3_connect_packet.encode().unwrap();
    group.bench_function("v3_connect_decode", |b| {
        b.iter(|| PacketV3::decode(v3_connect_bytes.as_ref()))
    });

    let v5_connect_packet = create_v5_connect_packet();
    let v5_connect_bytes = v5_connect_packet.encode().unwrap();
    group.bench_function("v5_connect_decode", |b| {
        b.iter(|| PacketV5::decode(v5_connect_bytes.as_ref()))
    });

    let v3_publish = PacketV3::Publish(mqtt_proto::v3::Publish {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level1(Pid::try_from(10).unwrap()),
        topic_name: TopicName::try_from("test/topic".to_string()).unwrap(),
        payload: "hello world".into(),
    });
    let v3_publish_bytes = v3_publish.encode().unwrap();
    group.bench_function("v3_publish_decode", |b| {
        b.iter(|| PacketV3::decode(v3_publish_bytes.as_ref()))
    });

    let v5_publish = PacketV5::Publish(mqtt_proto::v5::Publish {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level2(Pid::try_from(20).unwrap()),
        topic_name: TopicName::try_from("test/topic/v5".to_string()).unwrap(),
        payload: "hello world v5".into(),
        properties: Default::default(),
    });
    let v5_publish_bytes = v5_publish.encode().unwrap();
    group.bench_function("v5_publish_decode", |b| {
        b.iter(|| PacketV5::decode(v5_publish_bytes.as_ref()))
    });

    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
