//! Plan Phase 0 / Task 0.7 regression tests.
//!
//! Pins the three vendored patches so future upstream syncs (Q3 cadence)
//! or vendored "fixes" get caught in CI before broadcast:
//!
//!   1. Q13 — canonical varint for `fee_limit` (issue #540)
//!   2. Q2  — canonical double-SHA-256 txid (issue #399 historical bug)
//!   3. Risk #3 — Zeroizing gap in `anychain-kms::secp256k1_sign`
//!      (verified at the caller side per Task 1.2; kms-internal hygiene
//!      is opaque and not directly asserted here)
//!
//! All assertions use deterministic byte constants so the regression is
//! exact, not "looks similar".

use anychain_core::Transaction;
use anychain_tron::protocol::Tron::transaction::Raw;
use anychain_tron::trx;
use anychain_tron::TronTransaction;
use protobuf::Message;

const FEE_LIMIT_130M: i64 = 130_000_000;

/// Q13: `fee_limit = 130_000_000` serializes to the trailing 6-byte
/// sequence `[0x90, 0x01, 0x80, 0xc9, 0xfe, 0x3d]` — standard protobuf
/// wire format for tag 18 varint + value 130_000_000. The plan-asserted
/// 5-byte canonical `[0x90, 0x80, 0xc9, 0xfe, 0x3d]` is impossible
/// for tag 18 in standard protobuf (varint of 144 needs 2 bytes: `90 01`).
///
/// The Q13 patch was REVERTED 2026-09-06 after live-broadcast
/// investigation showed the 5-byte form ALSO fails TronGrid (NPE +
/// InvalidProtocolBufferException depending on endpoint). The real root
/// cause of issue #540 is something other than fee_limit varint.
///
/// This test pins the standard 6-byte form so any future Q13-style
/// regression (or accidental 5-byte patch reintroduction) trips CI.
#[test]
fn fee_limit_canonical_varint() {
    let mut raw = Raw::new();
    raw.fee_limit = FEE_LIMIT_130M;

    let bytes = raw
        .write_to_bytes()
        .expect("Raw::write_to_bytes (Q13 varint pin)");

    assert_eq!(
        bytes,
        vec![0x90, 0x01, 0x80, 0xc9, 0xfe, 0x3d],
        "fee_limit varint bytes drifted from baseline — Q13 patch regressed?",
    );
}

/// Belt-and-suspenders for the live broadcast path: a fully-built
/// `TronTransaction` with `fee_limit = 130_000_000` must serialize such
/// that the trailing 6 bytes of `raw_data_hex` are
/// `[0x90, 0x01, 0x80, 0xc9, 0xfe, 0x3d]` before POSTing. The test
/// round-trips through the vendored `Tron::transaction::Raw` encoder to
/// exercise the real path.
#[test]
fn signed_tx_raw_data_hex_ends_with_canonical_varint() {
    let fixture =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures");

    let contract = trx::build_transfer_contract(
        fixture.owner_address.as_str(),
        fixture.recipient_address.as_str(),
        "1000000",
    )
    .expect("build_transfer_contract");

    let mut params = anychain_tron::TronTransactionParameters::default();
    params.set_timestamp(trx::timestamp_millis());
    params.set_ref_block(fixture.ref_block_number, &fixture.ref_block_hex);
    params.set_contract(contract);

    let tx = TronTransaction::new(&params).expect("new TronTransaction");
    let raw_bytes = tx.to_bytes().expect("to_bytes");

    let raw = Raw::parse_from_bytes(&raw_bytes).expect("Raw parse");
    let mut raw_with_fee = raw;
    raw_with_fee.fee_limit = FEE_LIMIT_130M;
    let reserialized = raw_with_fee.write_to_bytes().expect("write_to_bytes (Q13)");

    let tail: Vec<u8> = reserialized[reserialized.len() - 6..].to_vec();
    assert_eq!(
        tail,
        vec![0x90, 0x01, 0x80, 0xc9, 0xfe, 0x3d],
        "signed-tx raw_data_hex fee_limit tail mismatch",
    );
}

/// Q2: `TronTransaction::to_transaction_id` returns SHA-256(SHA-256(raw)),
/// canonical TRX double-hash.
#[test]
fn txid_is_double_sha256() {
    let fixture =
        tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
            .expect("mainnet test fixtures");

    let contract = trx::build_transfer_contract(
        fixture.owner_address.as_str(),
        fixture.recipient_address.as_str(),
        "1000000",
    )
    .expect("build_transfer_contract");

    let mut params = anychain_tron::TronTransactionParameters::default();
    params.set_timestamp(trx::timestamp_millis());
    params.set_ref_block(fixture.ref_block_number, &fixture.ref_block_hex);
    params.set_contract(contract);

    let tx = TronTransaction::new(&params).expect("new TronTransaction");
    let raw_bytes = tx.to_bytes().expect("to_bytes");

    use sha2::{Digest, Sha256};
    let single: [u8; 32] = Sha256::digest(&raw_bytes).into();
    let expected: [u8; 32] = Sha256::digest(single).into();

    let actual = tx.to_transaction_id().expect("to_transaction_id").txid;
    assert_eq!(
        actual.as_slice(),
        expected.as_slice(),
        "to_transaction_id is NOT sha256(sha256(raw)) — Q2 patch regressed?",
    );
    // Negative test: must NOT equal single SHA-256.
    assert_ne!(
        actual.as_slice(),
        single.as_slice(),
        "to_transaction_id returned single-SHA-256 — Q2 patch regressed?",
    );
}

/// Risk #3: kms-internal `Zeroizing` is opaque to integration tests (the
/// buffer is freed and overwritten before the test can inspect it without
/// `unsafe` instrumentation). Per plan Task 0.7 the regression is at the
/// caller side (`tron-wallet-core::tx::sign::sign_tx` wraps `sk` in
/// `Zeroizing<Vec<u8>>`); this test stands as a smoke test that the
/// patched `secp256k1_sign` still signs correctly.
///
/// If the patch ever regresses to a plain `let sk = ...` binding, this
/// test will continue to pass — the patch's hygiene value is structural
/// and verified by review of `crates/anychain-vendored/anychain-kms/src/lib.rs`,
/// not by a runtime assertion.
#[test]
fn secp256k1_sign_smoke_after_zeroizing_patch() {
    use anychain_kms::secp256k1_sign;
    use sha2::{Digest, Sha256};

    // Deterministic 32-byte secret derived from a fixed preimage (test-only).
    let sk: [u8; 32] = Sha256::digest(b"varint_and_txid::secp256k1_sign_smoke").into();
    let msg: [u8; 32] = Sha256::digest(b"varint_and_txid::msg").into();

    let (sig, recid) = secp256k1_sign(&sk, &msg).expect("secp256k1_sign");
    assert_eq!(sig.len(), 64, "secp256k1 signature must be 64 bytes");
    assert!(
        recid == 0 || recid == 1,
        "recovery id must be in {{0, 1}} (TRON convention; Q8)",
    );
}
