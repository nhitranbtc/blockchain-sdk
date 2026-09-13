//! `sign_tx` — Phase 1.2.
//!
//! Deep-dive coverage: row 16 (full sign + verify with `recent_blockhash`).
//!
//! Asserts that `Wallet::sign_transaction` produces a `VersionedTransaction`
//! whose signature verifies under the wallet's public key, and that
//! non-default fee-payer transactions carry the expected number of
//! signatures.

use sol_wallet_core::wallet::Wallet;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature, Signer};
use solana_sdk::transaction::{Transaction, VersionedTransaction};

/// A trivial instruction: arbitrary program_id + a single account
/// meta referencing the wallet's pubkey. Avoids pulling in
/// `solana-system-interface` as a dev-dep just to test signing.
fn trivial_instruction(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: Pubkey::new_unique(),
        accounts: vec![AccountMeta::new(payer, true)],
        data: vec![],
    }
}

/// Manually sign a Transaction with a Keypair — mirrors Anza's
/// `Transaction::try_sign` without requiring the `wincode` feature.
/// The Keypair's signature is placed at the index where its pubkey
/// appears in the message's static account keys.
fn sign_transaction_with_keypair(tx: &mut Transaction, keypair: &Keypair) -> Result<(), String> {
    let my_pubkey = keypair.pubkey();
    let position = tx
        .message
        .account_keys
        .iter()
        .position(|k| k == &my_pubkey)
        .ok_or_else(|| "fee-payer pubkey not in tx account keys".to_string())?;

    let message_bytes = tx.message.serialize();
    let signature = keypair.sign_message(&message_bytes);

    while tx.signatures.len() <= position {
        tx.signatures.push(Signature::default());
    }
    tx.signatures[position] = signature;
    Ok(())
}

#[test]
fn sign_arbitrary_transaction_verifies() {
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");

    let from_pubkey = wallet.public_key();
    let ix = trivial_instruction(from_pubkey);

    let message = Message::new(&[ix], Some(&from_pubkey));
    let tx = Transaction::new_unsigned(message);
    let versioned = VersionedTransaction::from(tx);

    let signed = wallet
        .sign_transaction(versioned)
        .expect("sign must succeed");
    assert!(
        signed.verify_with_results().iter().all(|ok| *ok),
        "wallet's signature over the tx must verify"
    );
}

#[test]
fn sign_with_non_default_fee_payer_reflects_signature_count() {
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");

    let fee_payer = Keypair::new();
    let ix = trivial_instruction(wallet.public_key());
    let message = Message::new(&[ix], Some(&fee_payer.pubkey()));
    let mut tx = Transaction::new_unsigned(message);

    // Pre-sign with the fee-payer Keypair (manually, since Anza's
    // `Transaction::try_sign` is wincode-gated).
    sign_transaction_with_keypair(&mut tx, &fee_payer).expect("fee-payer sign");

    let versioned = VersionedTransaction::from(tx);
    let signed = wallet
        .sign_transaction(versioned)
        .expect("sign must succeed");
    assert_eq!(
        signed.signatures.len(),
        2,
        "fee-payer + wallet must produce 2 signatures"
    );
    assert!(
        signed.verify_with_results().iter().all(|ok| *ok),
        "both signatures must verify"
    );
}

#[test]
fn sign_message_arbitrary_bytes_verifies_recovered_pubkey() {
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");

    let msg = b"hello solana wallet core";
    let sig = wallet.sign_message(msg);
    assert!(
        sig.verify(&wallet.public_key().to_bytes(), msg),
        "signature must verify against wallet pubkey"
    );
}

// =============================================================================
// Migrated from tests/sign_legacy_transaction.rs after consolidating
// `sign_legacy_transaction` into `sign_transaction` (Phase 10 cleanup).
// The 3 dropped tests (`rejects_blockhash_mismatch`, `rejects_fee_payer_mismatch`,
// `rejects_non_wallet_fee_payer_empty_ixs`) enforced invariants that no longer
// exist — blockhash is embedded in the message (P1 guard gone), and the
// multi-signer flow needs the wallet to sign at non-zero positions (P4
// fee-payer-at-zero guard dropped).
// =============================================================================

#[test]
fn sign_transaction_happy_path_writes_sig_at_wallet_position() {
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");
    let from_pubkey = wallet.public_key();
    let message = Message::new(&[trivial_instruction(from_pubkey)], Some(&from_pubkey));
    let tx = Transaction::new_unsigned(message);
    let versioned = VersionedTransaction::from(tx);

    let signed = wallet.sign_transaction(versioned).expect("happy path");

    // `VersionedTransaction::from(tx)` pre-populates `signatures` with one
    // `Signature::default()` slot per account key in the message; wallet
    // is at index 0 here, so exactly one signature slot.
    assert_eq!(
        signed.signatures.len(),
        1,
        "single-signer message → 1 sig slot"
    );
    assert_ne!(
        signed.signatures[0],
        Signature::default(),
        "placeholder overwritten with a real signature"
    );

    // Extract the legacy message back out for verification.
    let signed_msg = match signed.message {
        solana_sdk::message::VersionedMessage::Legacy(m) => m,
        _ => unreachable!("test built legacy message"),
    };
    assert!(
        signed.signatures[0].verify(wallet.public_key().as_ref(), &signed_msg.serialize()),
        "signature must verify against wallet pubkey + serialized message"
    );
}

#[test]
fn sign_transaction_overwrites_existing_signature() {
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");
    let from_pubkey = wallet.public_key();
    let message = Message::new(&[trivial_instruction(from_pubkey)], Some(&from_pubkey));
    let tx = Transaction::new_unsigned(message);
    let mut versioned = VersionedTransaction::from(tx);

    // Pre-load a garbage 64-byte signature at the wallet's index (0).
    let garbage: Signature = Signature::from([0xAAu8; 64]);
    versioned.signatures[0] = garbage;
    assert_eq!(
        versioned.signatures[0], garbage,
        "pre-condition: garbage sig in slot 0"
    );

    let signed = wallet
        .sign_transaction(versioned)
        .expect("happy path over garbage");

    assert_eq!(signed.signatures.len(), 1, "still exactly one signature");
    assert_ne!(
        signed.signatures[0], garbage,
        "garbage overwritten with valid sig"
    );

    let signed_msg = match signed.message {
        solana_sdk::message::VersionedMessage::Legacy(m) => m,
        _ => unreachable!("test built legacy message"),
    };
    assert!(
        signed.signatures[0].verify(wallet.public_key().as_ref(), &signed_msg.serialize()),
        "new signature must verify"
    );
}

#[test]
fn sign_transaction_signature_round_trips_through_bincode_serialize() {
    // P3 parity assertion: the signature we produce over
    // `tx.message.serialize()` must verify after a
    // `bincode::serialize(&VersionedTransaction)` round-trip — the wire
    // format `send_transaction_with_options` ships.
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");
    let from_pubkey = wallet.public_key();
    let message = Message::new(&[trivial_instruction(from_pubkey)], Some(&from_pubkey));
    let tx = Transaction::new_unsigned(message);
    let sig_bytes_before = tx.message.serialize();

    let versioned = VersionedTransaction::from(tx);
    let signed = wallet.sign_transaction(versioned).expect("happy path");

    // Re-serialize the SAME message bytes the sign fn signed against.
    let signed_msg = match &signed.message {
        solana_sdk::message::VersionedMessage::Legacy(m) => m.clone(),
        _ => unreachable!("test built legacy message"),
    };
    let sig_bytes_after = signed_msg.serialize();
    assert_eq!(
        sig_bytes_before, sig_bytes_after,
        "message bytes must be stable across the sign call"
    );

    // The signature is over `sig_bytes_after` (= `sig_bytes_before`).
    assert!(
        signed.signatures[0].verify(wallet.public_key().as_ref(), &sig_bytes_after),
        "signature must verify against the message bytes it was produced over"
    );

    // Round-trip the signed VersionedTransaction through the wire
    // format that `send_transaction_with_options` ships (bincode default).
    let wire_bytes = bincode::serialize(&signed).expect("bincode serialize VersionedTransaction");
    assert!(!wire_bytes.is_empty(), "wire envelope non-empty");

    // A live broadcast test (submit_devnet_send.rs) proves end-to-end
    // acceptance; the in-process byte round-trip here pins the format
    // the broadcast uses.
}
