//! V2 — protobuf roundtrip (Q2).
//!
//! Plan §Q2: `prost-build` compiles pinned `core/Tron.proto` (SHA `851575d`); a
//! representative top-level message (e.g. `AccountId`, `Transaction::Raw`) round-trips
//! through `encode → decode` byte-equal; `txID = SHA-256(protobuf-serialize(raw_data))`.
//!
//! **Drift note (2026-08-27, Issue #403):** the plan referenced `TransferContract` and
//! `TriggerSmartContract`, but those message types live in separate `core/contract/*.proto`
//! files outside the vendored Tron.proto. The spike only vendors `Tron.proto` + its
//! `core/Discover.proto` and `core/contract/common.proto` imports — the full TRON
//! contract proto tree is out of scope for the spike (production code lives in
//! `crates/tron-wallet-core/` per plan §File Structure). V2 exercises the roundtrip +
//! txID pipeline on the messages that ARE in the vendored proto.

use prost::Message;
use tron_v1_spike::proto;
use tron_v1_spike::protobuf::tx_id;

#[test]
fn v2_account_id_roundtrip() {
    // AccountId: { name: bytes, address: bytes } — top-level message in core/Tron.proto.
    let original = proto::AccountId {
        name: b"test-account".to_vec(),
        address: vec![0x41u8; 21], // 21-byte raw T-address (0x41 prefix + 20 zero bytes)
    };

    let bytes = original.encode_to_vec();
    let decoded = proto::AccountId::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.name, b"test-account");
    assert_eq!(decoded.address, vec![0x41u8; 21]);
    assert_eq!(original.encode_to_vec(), bytes); // re-encode stable (deterministic)
}

#[test]
fn v2_transaction_raw_roundtrip() {
    // Transaction::Raw (line 1045 in generated protocol.rs) — uses Contract + ContractType.
    let raw_data = proto::transaction::Raw {
        contract: vec![proto::transaction::Contract {
            r#type: proto::transaction::contract::ContractType::TransferContract as i32,
            parameter: Some(prost_types::Any {
                type_url: "type.googleapis.com/protocol.TransferContract".to_string(),
                value: vec![0xde, 0xad, 0xbe, 0xef],
            }),
            ..Default::default()
        }],
        ref_block_bytes: vec![0xab],
        ref_block_hash: vec![0xcd],
        expiration: 1_700_000_000_000,
        timestamp: 1_700_000_000_000,
        ..Default::default()
    };

    let bytes = raw_data.encode_to_vec();
    let decoded = proto::transaction::Raw::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.ref_block_bytes, vec![0xab]);
    assert_eq!(decoded.ref_block_hash, vec![0xcd]);
    assert_eq!(decoded.expiration, 1_700_000_000_000);
    assert_eq!(decoded.timestamp, 1_700_000_000_000);
    assert_eq!(decoded.contract.len(), 1);
    assert_eq!(
        decoded.contract[0].r#type,
        proto::transaction::contract::ContractType::TransferContract as i32
    );
}

#[test]
fn v2_tx_id_is_sha256_of_encoded_raw_data() {
    // txID = SHA-256(protobuf-serialize(raw_data)) — per plan §Q2.
    let raw_data = proto::transaction::Raw {
        ref_block_bytes: vec![0x12, 0x34],
        ref_block_hash: vec![0xab, 0xcd],
        expiration: 1_700_000_000_000,
        timestamp: 1_700_000_000_000,
        ..Default::default()
    };

    let id1 = tx_id(&raw_data);
    let id2 = tx_id(&raw_data);
    assert_eq!(id1, id2); // deterministic

    // Cross-check against manual SHA-256.
    use sha2::{Digest, Sha256};
    let encoded = raw_data.encode_to_vec();
    let expected: [u8; 32] = Sha256::digest(&encoded).into();
    assert_eq!(id1, expected);
}
