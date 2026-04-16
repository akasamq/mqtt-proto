# mqtt-proto patch

> [!CAUTION]
> **DO NOT DELETE**: This branch is pinned in `Cargo.toml` via `[patch]` / git dependency to lock `mqtt-proto` features and prevent feature unification pollution with `akasa-core`. Removing this will break the build.

MQTT encoding/decoding library (support no-std)

## Simple usage example
Run:

```bash
cargo run --example basic_usage --features tokio
```

This example connects to a real broker, then sends:
1. `CONNECT` and waits for `CONNACK`
2. one QoS0 `PUBLISH`
3. `DISCONNECT`

Optional env vars:

```bash
BROKER_ADDR=broker.emqx.io:1883
MQTT_TOPIC=mqtt-proto/demo/basic
```

Warning: this is a public broker. Do not send sensitive data.
