#![no_main]

use futures_lite::future::block_on;
use libfuzzer_sys::fuzz_target;

use mqtt_proto::v3::Packet;

fuzz_target!(|pkt: Packet| {
    let mut data_async = Vec::new();
    block_on(pkt.encode_async(&mut data_async)).unwrap();
    let var_bytes = pkt.encode().unwrap();
    assert_eq!(var_bytes.as_slice(), &data_async);

    match Packet::decode(&data_async) {
        Ok(decoded_pkt) => assert_eq!(pkt, decoded_pkt.unwrap()),
        Err(_err) => {}
    }
});
