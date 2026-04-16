use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use mqtt_proto::v5::{
    Connect, ConnectReasonCode, Disconnect, DisconnectReasonCode, Packet, Publish,
};
use mqtt_proto::{QosPid, TopicName};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

async fn write_packet(
    stream: &mut TcpStream,
    packet: &Packet,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = packet.encode()?;
    stream.write_all(bytes.as_ref()).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("Run with: cargo run --example basic_usage --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let broker = std::env::var("BROKER_ADDR").unwrap_or_else(|_| "broker.emqx.io:1883".into());
    let topic = std::env::var("MQTT_TOPIC").unwrap_or_else(|_| "mqtt-proto/demo/basic".into());

    let mut stream =
        tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(&broker)).await??;

    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let client_id = format!("mqtt-proto-basic-{unique}");
    let connect = Connect::new(client_id.into(), 30);
    write_packet(&mut stream, &Packet::Connect(connect)).await?;

    let connack =
        tokio::time::timeout(Duration::from_secs(10), Packet::decode_async(&mut stream)).await??;

    match connack {
        Packet::Connack(resp) if resp.reason_code == ConnectReasonCode::Success => {
            println!("connected to broker: {broker}");
        }
        Packet::Connack(resp) => {
            return Err(format!("connect rejected: {:?}", resp.reason_code).into());
        }
        other => {
            return Err(format!("expected CONNACK, got {:?}", other.get_type()).into());
        }
    }

    let publish = Publish::new(
        QosPid::Level0,
        TopicName::try_from(topic.as_str())?,
        Bytes::from("hello from mqtt-proto basic usage"),
    );
    write_packet(&mut stream, &Packet::Publish(publish)).await?;
    println!("published one message to topic: {topic}");

    let disconnect = Disconnect::new(DisconnectReasonCode::NormalDisconnect);
    write_packet(&mut stream, &Packet::Disconnect(disconnect)).await?;
    println!("disconnected.");
    Ok(())
}
