//! Plan Task 2.6 / Phase-7 spike V2: protobuf wire format round-trip.
//!
//! Building a TRX transfer, serialising it, then re-parsing the bytes must
//! yield an equivalent `TronTransaction`. If this test ever stops being a
//! tautology, an anychain-bump has drifted away from what the network
//! expects.
//!
//! The contract addresses and amount come from `anychain_tron::transaction::tests`
//! (the upstream test fixture), kept verbatim so a regression that moves
//! the wire form is visible against the same input.
//!
//! Also covers carry-over Task 2.6 (Phase 3): `TriggerSmartContract.data`
//! must be encoded at proto field 4, not field 3. The wire-tag for
//! field 4 wire-type 2 (length-delimited) is `0x22`; field 3 wire-type 2
//! would be `0x1a`. The test asserts the encoded `TronTransaction` for a
//! TRC-20 transfer contains `0x22` immediately followed by the
//! `0xa9059cbb` selector of `transfer(address,uint256)`.

use anychain_core::Transaction;
use anychain_tron::trx;
use anychain_tron::TronTransaction;

#[test]
fn tron_transaction_parameters_round_trip_through_bytes() {
    let addr_from =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures must be present")
            .owner_address
            .as_str();
    let addr_to =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures must be present")
            .recipient_address
            .as_str();
    let amount = "10000000";

    let contract = trx::build_transfer_contract(addr_from, addr_to, amount)
        .expect("anychain build_transfer_contract");
    let mut params = anychain_tron::TronTransactionParameters::default();
    params.set_timestamp(trx::timestamp_millis());
    let test = tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
        .expect("mainnet test fixtures must be present");
    params.set_ref_block(test.ref_block_number, &test.ref_block_hex);
    params.set_contract(contract);

    let tx = TronTransaction::new(&params).expect("new TronTransaction");
    let bytes = tx.to_bytes().expect("to_bytes");

    let parsed = TronTransaction::from_bytes(&bytes).expect("from_bytes");

    assert_eq!(parsed.data.contract, tx.data.contract);
    assert_eq!(parsed.data.timestamp, tx.data.timestamp);
    assert_eq!(parsed.data.ref_block_bytes, tx.data.ref_block_bytes);
    assert_eq!(parsed.data.ref_block_hash, tx.data.ref_block_hash);
}

#[test]
fn raw_data_bytes_are_stable_against_upstream_test_vector() {
    let addr_from =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures must be present")
            .owner_address
            .as_str();
    let addr_to =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures must be present")
            .recipient_address
            .as_str();
    let amount = "10000000";

    let contract = trx::build_transfer_contract(addr_from, addr_to, amount)
        .expect("anychain build_transfer_contract");
    let mut params = anychain_tron::TronTransactionParameters::default();
    params.set_timestamp(0);
    let test = tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
        .expect("mainnet test fixtures must be present");
    params.set_ref_block(test.ref_block_number, &test.ref_block_hex);
    params.set_contract(contract);

    let tx = TronTransaction::new(&params).expect("new TronTransaction");
    let bytes = tx.to_bytes().expect("to_bytes");

    assert!(
        bytes.len() > 25,
        "tron raw bytes were too small ({} bytes) — protobuf shape changed?",
        bytes.len()
    );
}

/// Carry-over from Phase 2 Task 2.6: `TriggerSmartContract.data` MUST be
/// encoded at proto field 4, not field 3. The plan calls this out
/// explicitly, and the network rejects a transaction whose `data` lands
/// in the wrong field — the TronGrid node fails the build with
/// `TriggerSmartContract.data` empty.
///
/// How the assertion is grounded: build a `TriggerSmartContract` via
/// anychain's public builder, parse the wrapped Any envelope back to
/// the inner message, then walk through it two ways:
///
/// 1. **Round-trip correctness**: the parsed struct's `data` field must
///    equal the calldata the builder produced. A drop, rewrite, or
///    field-number swap by anychain would surface here.
/// 2. **Wire-format tag byte**: serialise the parsed struct back to
///    bytes and inspect the protobuf envelope directly. We walk every
///    tag byte and assert `data` lives at proto field 4 — i.e. tag
///    `0x22` = `(4 << 3) | 2` (length-delimited). A drift to field 3
///    would emit tag `0x1a`; a drift to any other length-delimited
///    field would emit a different tag entirely. The previous version
///    of this test scanned one byte back from the selector and tripped
///    on the length-varint byte; the safer approach is to walk the
///    envelope top-down and check field numbers.
#[test]
fn trigger_smart_contract_data_lives_at_proto_field_4() {
    use anychain_tron::protocol::smart_contract::TriggerSmartContract;
    use protobuf::Message;

    let owner =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures must be present")
            .owner_address
            .as_str();
    let contract =
        tron_wallet_core::tokens::by_symbol(tron_wallet_core::config::Network::Mainnet, "USDT")
            .expect("mainnet USDT must be in the bundle")
            .address
            .as_str();
    let recipient =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures must be present")
            .recipient_address
            .as_str();
    let amount = "1000000"; // 1 USDT (6 decimals)

    let contract_pb = trx::build_trc20_transfer_contract(owner, contract, recipient, amount)
        .expect("anychain build_trc20_transfer_contract");

    let any_pb = contract_pb
        .parameter
        .as_ref()
        .expect("Contract.parameter (the Any) is present")
        .value
        .clone();

    let inner = TriggerSmartContract::parse_from_bytes(&any_pb)
        .expect("inner TriggerSmartContract parses from Any.value");

    // Round-trip correctness: the parsed struct must carry the same
    // calldata the builder produced. A drop, rewrite, or field-number
    // swap by anychain would surface here.
    let expected_calldata = anychain_tron::abi::trc20_transfer(recipient, amount);
    assert_eq!(
        inner.data, expected_calldata,
        "inner TriggerSmartContract.data must equal the calldata anychain built; \
         a mismatch means anychain dropped or rewrote the data bytes"
    );
    // First 4 bytes of data = `transfer(address,uint256)` selector.
    assert_eq!(
        inner.data[..4],
        [0xa9, 0x05, 0x9c, 0xbb],
        "data starts with TRC-20 transfer selector 0xa9059cbb"
    );

    // Walk the serialised envelope field-by-field and assert the
    // `data` field is at proto field 4 (tag byte 0x22).
    let bytes = inner
        .write_to_bytes()
        .expect("inner TriggerSmartContract serialises");

    let mut i = 0;
    let mut data_field: Option<u32> = None;
    while i < bytes.len() {
        let tag = bytes[i];
        let field_num = (tag >> 3) as u32;
        let wire_type = tag & 0x7;
        i += 1;
        match wire_type {
            0 => {
                // VARINT — skip past.
                while i < bytes.len() && bytes[i] >= 0x80 {
                    i += 1;
                }
                i += 1;
            }
            1 => i += 8,  // FIXED64
            2 => {
                // LEN — read varint length, then skip the body. The
                // body starts at the selector for our `data` field.
                let mut len: usize = 0;
                let mut shift = 0;
                loop {
                    let b = bytes[i];
                    i += 1;
                    len |= ((b & 0x7f) as usize) << shift;
                    if b < 0x80 {
                        break;
                    }
                    shift += 7;
                }
                if field_num == 4 && bytes[i..].starts_with(&[0xa9, 0x05, 0x9c, 0xbb]) {
                    data_field = Some(field_num);
                }
                i += len;
            }
            5 => i += 4,  // FIXED32
            _ => panic!(
                "unexpected wire type {wire_type} at byte {tag_byte} in encoded TriggerSmartContract",
                tag_byte = i - 1
            ),
        }
    }
    assert_eq!(
        data_field,
        Some(4),
        "TriggerSmartContract.data must be encoded at proto field 4 (tag 0x22). \
         Found at field {data_field:?}. Encoded bytes: {}",
        hex::encode(&bytes)
    );
}
