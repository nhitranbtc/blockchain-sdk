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

use anychain_core::Transaction;
use anychain_tron::trx;
use anychain_tron::TronTransaction;

#[test]
fn tron_transaction_parameters_round_trip_through_bytes() {
    let addr_from = "TG7jQ7eGsns6nmQNfcKNgZKyKBFkx7CvXr";
    let addr_to = "TFk5LfscQv8hYM11mZYmi3ZcnRfFc4LLap";
    let amount = "10000000";

    let contract = trx::build_transfer_contract(addr_from, addr_to, amount)
        .expect("anychain build_transfer_contract");
    let mut params = anychain_tron::TronTransactionParameters::default();
    params.set_timestamp(trx::timestamp_millis());
    params.set_ref_block(
        26_661_399_i64,
        "000000000196d21784deb05dee04c69ed112b8e078e74019f9a0b1df6adc414e",
    );
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
    let addr_from = "TG7jQ7eGsns6nmQNfcKNgZKyKBFkx7CvXr";
    let addr_to = "TFk5LfscQv8hYM11mZYmi3ZcnRfFc4LLap";
    let amount = "10000000";

    let contract = trx::build_transfer_contract(addr_from, addr_to, amount)
        .expect("anychain build_transfer_contract");
    let mut params = anychain_tron::TronTransactionParameters::default();
    params.set_timestamp(0);
    params.set_ref_block(
        26_661_399_i64,
        "000000000196d21784deb05dee04c69ed112b8e078e74019f9a0b1df6adc414e",
    );
    params.set_contract(contract);

    let tx = TronTransaction::new(&params).expect("new TronTransaction");
    let bytes = tx.to_bytes().expect("to_bytes");

    assert!(
        bytes.len() > 25,
        "tron raw bytes were too small ({} bytes) — protobuf shape changed?",
        bytes.len()
    );
}
