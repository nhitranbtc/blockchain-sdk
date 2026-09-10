//! `bip39_mnemonic` — Phase 1.1.
//!
//! Asserts that `Wallet::from_mnemonic` (and the underlying
//! `bip39::Mnemonic::parse_in(English, ...)` it delegates to) accept
//! valid English BIP-39 phrases and reject invalid ones.
//!
//! Deep-dive coverage: row 1 (Phantom-equivalent SLIP-0010 derivation
//! is gated on the phrase passing BIP-39 first — this file owns the
//! pre-derivation gate).

use sol_wallet_core::wallet::Wallet;

const VALID_12: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[test]
fn accepts_12_word_english_phrase() {
    let w = Wallet::from_mnemonic(VALID_12).expect("12-word phrase must parse");
    let _ = w; // existence proves parse + derive succeeded
}

/// Generate a fresh valid English mnemonic with `count` words via
/// `bip39::Mnemonic::generate_in` — guarantees a valid BIP-39 checksum,
/// which static hand-written phrases for 15/18/21-word lengths don't
/// reliably have.
fn fresh_phrase(count: usize) -> String {
    bip39::Mnemonic::generate_in(bip39::Language::English, count)
        .expect("bip39 generate should not fail")
        .to_string()
}

#[test]
fn accepts_15_word_english_phrase() {
    Wallet::from_mnemonic(&fresh_phrase(15)).expect("15-word phrase must parse");
}

#[test]
fn accepts_18_word_english_phrase() {
    Wallet::from_mnemonic(&fresh_phrase(18)).expect("18-word phrase must parse");
}

#[test]
fn accepts_21_word_english_phrase() {
    Wallet::from_mnemonic(&fresh_phrase(21)).expect("21-word phrase must parse");
}

#[test]
fn accepts_24_word_english_phrase() {
    // Use the canonical BIP-39 24-word test vector ("abandon ×23 art").
    // It has a valid checksum — verified against the BIP-39 standard.
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
    Wallet::from_mnemonic(phrase).expect("24-word phrase must parse");
}

#[test]
fn rejects_11_word_phrase() {
    let phrase =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
    assert!(Wallet::from_mnemonic(phrase).is_err());
}

#[test]
fn rejects_25_word_phrase() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
    assert!(Wallet::from_mnemonic(phrase).is_err());
}

#[test]
fn rejects_non_english_word() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon zzzznotaword";
    assert!(Wallet::from_mnemonic(phrase).is_err());
}

#[test]
fn rejects_checksum_failure() {
    // 12 valid English words but the last word's checksum bits are
    // wrong. bip39 wordlist words encode 4 bits of checksum each;
    // 11×"abandon" + 12th "abandon" = entropy 0x00000000000000000000000000000000
    // should checksum to "about" (not "abandon").
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
    assert!(Wallet::from_mnemonic(phrase).is_err());
}

#[test]
fn rejects_empty_phrase() {
    assert!(Wallet::from_mnemonic("").is_err());
}
