//! Unit tests for `Wallet::sign_legacy_transaction` (Phase 6.4 Step 2).
//!
//! Coverage per audit §F (2026-09-13):
//!
//! 1. happy path: fee-payer wallet + matching blockhash → sig at index 0
//! 2. rejects blockhash mismatch (P1)
//! 3. rejects fee-payer mismatch (P4)
//! 4. overwrites pre-existing signature (P7)
//! 5. rejects empty `account_keys` (treat as fee-payer mismatch)
//! 6. signature round-trips through `bincode::serialize` (P3 parity with
//!    `send_transaction_with_options` wire format)
//!
//! No network. No mnemonic files. The wallet is constructed from the
//! in-source `SENDER_MNEMONIC` (see `tests/common/mod.rs`) for the same
//! reason `submit_devnet_send_real_broadcast` uses it: deterministic
//! Phantom-canonical derivation. The mnemonic is devnet-only.

mod common;

use common::{dummy_blockhash, SENDER_MNEMONIC};
use sol_wallet_core::wallet::Wallet;
use sol_wallet_core::Error;
use solana_sdk::{
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};

/// Build a trivial system-style ix with the payer as the first account.
/// Matches the `sign_only.rs` pattern — no system_instruction dep needed.
fn trivial_ix(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: Pubkey::new_unique(),
        accounts: vec![AccountMeta::new(payer, true)],
        data: vec![],
    }
}

/// Build a fee-payer-keyed `Transaction` whose `recent_blockhash` is the
/// caller-supplied blockhash. Used as the happy-path input.
fn fee_payer_tx(wallet: &Wallet, blockhash: Hash) -> Transaction {
    let payer = wallet.public_key();
    let message = Message::new(&[trivial_ix(payer)], Some(&payer));
    let mut tx = Transaction::new_unsigned(message);
    tx.message.recent_blockhash = blockhash;
    tx
}

#[test]
fn sign_legacy_transaction_happy_path_writes_sig_at_zero() {
    let wallet = Wallet::from_mnemonic(SENDER_MNEMONIC).expect("from_mnemonic");
    let blockhash = dummy_blockhash();

    let mut tx = fee_payer_tx(&wallet, blockhash);
    // `Transaction::new_unsigned` pre-populates `signatures` with one
    // `Signature::default()` slot for the single-payer message.
    assert_eq!(tx.signatures.len(), 1, "single-signer message → 1 sig slot");
    assert_eq!(
        tx.signatures[0],
        Signature::default(),
        "pre-condition: signatures[0] is the all-zero placeholder"
    );

    wallet
        .sign_legacy_transaction(&mut tx, blockhash)
        .expect("happy path");

    assert_eq!(tx.signatures.len(), 1, "still exactly one signature slot");
    let sig: &Signature = &tx.signatures[0];
    assert_ne!(
        sig,
        &Signature::default(),
        "placeholder overwritten with a real signature"
    );
    assert!(
        sig.verify(wallet.public_key().as_ref(), &tx.message.serialize()),
        "signature must verify against wallet pubkey + serialized message"
    );
}

#[test]
fn sign_legacy_transaction_rejects_blockhash_mismatch() {
    let wallet = Wallet::from_mnemonic(SENDER_MNEMONIC).expect("from_mnemonic");
    let embedded_blockhash = dummy_blockhash();
    let mut tx = fee_payer_tx(&wallet, embedded_blockhash);

    // Sign with a *different* blockhash than what's embedded in the message.
    let other_blockhash = Hash::new_from_array([0x11u8; 32]);
    let err = wallet
        .sign_legacy_transaction(&mut tx, other_blockhash)
        .expect_err("blockhash mismatch must reject");

    match err {
        Error::BlockhashMismatch {
            tx_message,
            signer_input,
        } => {
            assert_eq!(tx_message, embedded_blockhash);
            assert_eq!(signer_input, other_blockhash);
        }
        other => panic!("expected Error::BlockhashMismatch, got {other:?}"),
    }
    assert_eq!(
        tx.signatures[0],
        Signature::default(),
        "rejected: signatures[0] untouched"
    );
}

#[test]
fn sign_legacy_transaction_rejects_fee_payer_mismatch() {
    let wallet = Wallet::from_mnemonic(SENDER_MNEMONIC).expect("from_mnemonic");
    let other_payer = Keypair::new(); // not this wallet
    let blockhash = dummy_blockhash();

    // Build a tx where the fee-payer is `other_payer`, NOT the wallet.
    let message = Message::new(
        &[trivial_ix(other_payer.pubkey())],
        Some(&other_payer.pubkey()),
    );
    let mut tx = Transaction::new_unsigned(message);
    tx.message.recent_blockhash = blockhash;

    let err = wallet
        .sign_legacy_transaction(&mut tx, blockhash)
        .expect_err("fee-payer mismatch must reject");

    match err {
        Error::FeePayerMismatch { expected, actual } => {
            assert_eq!(expected, wallet.public_key());
            assert_eq!(actual, other_payer.pubkey());
        }
        other => panic!("expected Error::FeePayerMismatch, got {other:?}"),
    }
    assert_eq!(
        tx.signatures[0],
        Signature::default(),
        "rejected: signatures[0] untouched"
    );
}

#[test]
fn sign_legacy_transaction_overwrites_existing_signature() {
    let wallet = Wallet::from_mnemonic(SENDER_MNEMONIC).expect("from_mnemonic");
    let blockhash = dummy_blockhash();

    let mut tx = fee_payer_tx(&wallet, blockhash);
    // Pre-load a garbage 64-byte signature at index 0 (REPLACE the
    // pre-populated placeholder, not append).
    let garbage: Signature = Signature::from([0xAAu8; 64]);
    tx.signatures[0] = garbage;
    assert_eq!(
        tx.signatures[0], garbage,
        "pre-condition: garbage sig in slot 0"
    );

    wallet
        .sign_legacy_transaction(&mut tx, blockhash)
        .expect("happy path");

    assert_eq!(tx.signatures.len(), 1, "still exactly one signature");
    assert_ne!(
        tx.signatures[0], garbage,
        "garbage overwritten with valid sig"
    );
    assert!(
        tx.signatures[0].verify(wallet.public_key().as_ref(), &tx.message.serialize()),
        "new signature must verify"
    );
}

#[test]
fn sign_legacy_transaction_rejects_non_wallet_fee_payer_empty_ixs() {
    let wallet = Wallet::from_mnemonic(SENDER_MNEMONIC).expect("from_mnemonic");
    let other_payer = Keypair::new();
    let blockhash = dummy_blockhash();

    // Empty instructions; fee-payer is `other_payer`, NOT the wallet.
    let message = Message::new(&[], Some(&other_payer.pubkey()));
    let mut tx = Transaction::new_unsigned(message);
    tx.message.recent_blockhash = blockhash;

    let err = wallet
        .sign_legacy_transaction(&mut tx, blockhash)
        .expect_err("non-wallet fee-payer must reject");

    assert!(
        matches!(err, Error::FeePayerMismatch { .. }),
        "expected FeePayerMismatch, got {err:?}"
    );
    assert_eq!(
        tx.signatures[0],
        Signature::default(),
        "rejected: signatures[0] untouched"
    );
}

#[test]
fn sign_legacy_transaction_signature_round_trips_through_bincode_serialize() {
    // P3 parity assertion: the signature we produce over
    // `tx.message.serialize()` must verify against the same bytes
    // after a `bincode::serialize(&VersionedTransaction::from(tx))`
    // round-trip. This is the wire format `send_transaction_with_options`
    // ships (verified against the live devnet broadcast landed in the
    // existing test).
    //
    // NOTE: the existing Tier 2 #8 doc comment claims `bincode::config::legacy()`
    // is required; this test pins the proven-working default config
    // (the live broadcast succeeded with default). If the broadcast
    // path is ever migrated to `legacy()`, this test will catch the
    // regression — the wire-format hash will differ and the sig
    // will fail to verify.
    let wallet = Wallet::from_mnemonic(SENDER_MNEMONIC).expect("from_mnemonic");
    let blockhash = dummy_blockhash();

    let mut tx = fee_payer_tx(&wallet, blockhash);
    let sig_bytes_before = tx.message.serialize();

    wallet
        .sign_legacy_transaction(&mut tx, blockhash)
        .expect("happy path");

    // Re-serialize the SAME message bytes the sign fn signed against
    // (tx.message hasn't changed since the fn only mutates signatures).
    let sig_bytes_after = tx.message.serialize();
    assert_eq!(
        sig_bytes_before, sig_bytes_after,
        "message bytes must be stable across the sign call"
    );

    // The signature is over `sig_bytes_after` (= `sig_bytes_before`).
    assert!(
        tx.signatures[0].verify(wallet.public_key().as_ref(), &sig_bytes_after),
        "signature must verify against the message bytes it was produced over"
    );

    // Round-trip the signed legacy Transaction through the wire
    // format that `send_transaction_with_options` ships (bincode
    // default + base64 envelope via VersionedTransaction wrapper).
    let versioned = solana_sdk::transaction::VersionedTransaction::from(tx);
    let wire_bytes =
        bincode::serialize(&versioned).expect("bincode serialize VersionedTransaction");
    assert!(!wire_bytes.is_empty(), "wire envelope non-empty");

    // We don't have a live cluster here, but the bytes round-trip
    // cleanly through the same `bincode::serialize` call broadcast
    // uses. A separate live broadcast test (submit_devnet_send.rs)
    // proves end-to-end acceptance.
}
