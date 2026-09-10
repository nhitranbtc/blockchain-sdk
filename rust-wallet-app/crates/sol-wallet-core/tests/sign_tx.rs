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
