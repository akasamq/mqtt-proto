use std::convert::TryFrom;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use mqtt_proto::{
    v3::{Connect as ConnectV3, LastWill as LastWillV3, Packet as PacketV3, Publish as PublishV3},
    v5::{Connect as ConnectV5, LastWill as LastWillV5, Packet as PacketV5, Publish as PublishV5},
    Pid, QoS, QosPid, TopicName,
};

fn payload(len: usize) -> Vec<u8> {
    // Use 64-bit XorShift* as PRNG
    let mut s: usize = len.wrapping_add(0x9e37_79b9_7f4a_7c15); // MAGIC
    let mut out = Vec::with_capacity(len);

    for _ in 0..len {
        // 32-bit -> 64-bit XorShift*
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        let val = s.wrapping_mul(0x2545_f491_4f6c_dd1d) as u32;
        out.push((val >> 24) as u8);
    }
    out
}

fn create_v3_connect(payload_size: usize) -> PacketV3 {
    let mut conn = ConnectV3::new("bench-client".into(), 60);
    conn.username = Some("user".into());
    conn.password = Some("pass".into());
    conn.last_will = Some(LastWillV3 {
        qos: QoS::Level1,
        retain: false,
        topic_name: TopicName::try_from("will/topic").unwrap(),
        message: payload(payload_size).into(),
    });
    PacketV3::Connect(conn)
}

fn create_v5_connect(payload_size: usize) -> PacketV5 {
    let mut conn = ConnectV5::new("bench-client-v5".into(), 60);
    conn.username = Some("user-v5".into());
    conn.password = Some("pass-v5".into());
    conn.last_will = Some(LastWillV5 {
        qos: QoS::Level2,
        retain: true,
        topic_name: TopicName::try_from("will/topic/v5").unwrap(),
        payload: payload(payload_size).into(),
        properties: Default::default(),
    });
    PacketV5::Connect(conn)
}

fn create_v3_publish(payload_size: usize) -> PacketV3 {
    PacketV3::Publish(PublishV3 {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level1(Pid::try_from(1).unwrap()),
        topic_name: TopicName::try_from("benchmark/topic").unwrap(),
        payload: payload(payload_size).into(),
    })
}

fn create_v5_publish(payload_size: usize) -> PacketV5 {
    PacketV5::Publish(PublishV5 {
        dup: false,
        retain: false,
        qos_pid: QosPid::Level2(Pid::try_from(2).unwrap()),
        topic_name: TopicName::try_from("benchmark/topic/v5").unwrap(),
        payload: payload(payload_size).into(),
        properties: Default::default(),
    })
}

fn bench_encode(c: &mut Criterion) {
    let sizes = [64, 4096, 16 * 1024];
    let mut group = c.benchmark_group("encode");

    for &size in &sizes {
        macro_rules! bench {
            ($name:expr, $pkt:expr) => {
                group.bench_with_input(BenchmarkId::new($name, size), &$pkt, |b, p| {
                    b.iter(|| black_box(p.encode().unwrap()))
                });
            };
        }

        bench!("v3_connect", create_v3_connect(size));
        bench!("v5_connect", create_v5_connect(size));
        bench!("v3_publish", create_v3_publish(size));
        bench!("v5_publish", create_v5_publish(size));
    }
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let sizes = [64, 4096, 16 * 1024];
    let mut group = c.benchmark_group("decode");

    for &size in &sizes {
        let v3_conn_bytes = create_v3_connect(size).encode().unwrap();
        let v5_conn_bytes = create_v5_connect(size).encode().unwrap();
        let v3_pub_bytes = create_v3_publish(size).encode().unwrap();
        let v5_pub_bytes = create_v5_publish(size).encode().unwrap();

        macro_rules! bench {
            ($name:expr, $bytes:expr, $decode:path) => {
                group.bench_with_input(BenchmarkId::new($name, size), $bytes, |b, bytes| {
                    b.iter(|| black_box($decode(bytes.as_ref()).unwrap()))
                });
            };
        }

        bench!("v3_connect", &v3_conn_bytes, PacketV3::decode);
        bench!("v5_connect", &v5_conn_bytes, PacketV5::decode);
        bench!("v3_publish", &v3_pub_bytes, PacketV3::decode);
        bench!("v5_publish", &v5_pub_bytes, PacketV5::decode);
    }
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
