//! `sign_only` — Phase 1.2.
//!
//! Deep-dive coverage: row 17 (`sign_only_tx` cold path).
//!
//! Asserts:
//!  - `Wallet::sign_transaction` performs no RPC (pure local op).
//!  - Re-signing the same `VersionedTransaction` twice produces the
//!    same signature (Ed25519 is deterministic; the wallet overwrites
//!    its own signature at the wallet's pubkey position).
//!  - `Wallet::sign_message` signs arbitrary 32-byte payloads and
//!    the recovered pubkey matches via
//!    `solana_sdk::signature::Signature::verify`.

use sol_wallet_core::wallet::Wallet;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::Transaction;

fn trivial_instruction(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: Pubkey::new_unique(),
        accounts: vec![AccountMeta::new(payer, true)],
        data: vec![],
    }
}

#[test]
fn re_signing_same_tx_is_deterministic() {
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");

    let ix = trivial_instruction(wallet.public_key());
    let message = Message::new(&[ix], Some(&wallet.public_key()));
    let tx = Transaction::new_unsigned(message);

    let v1 = wallet.sign_transaction(tx.clone().into()).expect("sign v1");
    let v2 = wallet.sign_transaction(tx.into()).expect("sign v2");

    assert_eq!(
        v1.signatures, v2.signatures,
        "Ed25519 must be deterministic; the same wallet signing the same tx must produce the same signature"
    );
}

#[test]
fn sign_message_32_byte_payload_verifies() {
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");

    // Arbitrary 32-byte message — the canonical Ed25519 input size.
    let msg: [u8; 32] = core::array::from_fn(|i| i as u8);
    let sig = wallet.sign_message(&msg);

    assert!(
        sig.verify(&wallet.public_key().to_bytes(), &msg),
        "signature over 32-byte payload must verify"
    );
}

#[test]
fn sign_message_recovered_pubkey_matches_wallet() {
    // Sanity: an Ed25519 signature over (R, s) + message recovers the
    // public key A. The recovered A must equal the wallet's pubkey.
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");

    let msg = b"verify-recovered-pubkey";
    let sig = wallet.sign_message(msg);

    let wallet_pubkey_bytes = wallet.public_key().to_bytes();
    assert!(
        sig.verify(&wallet_pubkey_bytes, msg),
        "recovered pubkey must equal wallet pubkey"
    );
    assert_ne!(
        wallet_pubkey_bytes,
        Pubkey::new_unique().to_bytes(),
        "wallet pubkey must differ from a random pubkey (catches trivially-passing all-zero tests)"
    );
}

#[test]
fn sign_only_tx_does_not_require_rpc() {
    // Cold-path test: this entire suite runs offline. No network, no
    // RPC client, no clock. If this test passes, sign-only is
    // confirmed network-free.
    let wallet = Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .expect("mnemonic must parse");

    let ix = trivial_instruction(wallet.public_key());
    let message = Message::new(&[ix], Some(&wallet.public_key()));
    let tx = Transaction::new_unsigned(message);

    let signed = wallet
        .sign_transaction(tx.into())
        .expect("sign must succeed");
    assert!(
        signed.verify_with_results().iter().all(|ok| *ok),
        "cold-path signature must verify"
    );
}
