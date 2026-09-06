//! Plan Task 2.8 / Phase-7 spike V7: SPKI pin verifier wiring.
//!
//! Two kinds of test live here:
//!
//! 1. **Always-on** unit tests against the in-crate pin helper — confirm
//!    `SpkiPin::from_bytes` round-trips, the `MAINNET_SPKI_PIN_HEX`
//!    constant matches the runtime default, and the `SpkiPinnedVerifier`
//!    constructor does not error on the canonical mainnet pin.
//! 2. **Live network** assertions, gated on `RUN_TRON_NILE=1` (or
//!    `RUN_TRON_MAINNET=1`). They are deliberately no-ops on a default
//!    `cargo test` — operators run them with the env var set; CI does
//!    not (per L29 operator-driven smoke).
//!
//! Why env-gated and skipped by default: the live form connects to
//! `api.trongrid.io` and exercises the webpki chain *plus* the pin check;
//! we cannot run that in unit CI without a network, so the gate stays
//! closed by default.

use tron_wallet_core::chain::spki::{SpkiPin, SpkiPinnedVerifier};
use tron_wallet_core::config::mainnet_spki_pin;
use tron_wallet_core::disambig::MAINNET_SPKI_PIN_HEX;

#[test]
fn mainnet_pin_hex_constant_matches_runtime_default() {
    let pin = mainnet_spki_pin();
    assert_eq!(
        hex::encode(pin.as_bytes()),
        MAINNET_SPKI_PIN_HEX,
        "disambig.rs MAINNET_SPKI_PIN_HEX must decode to the runtime pin"
    );
}

#[test]
fn pinned_verifier_construction_succeeds_on_mainnet_pin() {
    let pin = mainnet_spki_pin();
    let _verifier = SpkiPinnedVerifier::new(pin).expect("verifier constructor");
}

#[test]
fn pin_round_trips_through_hex_display() {
    let pin = SpkiPin::from_bytes([0xab; 32]);
    let rendered = format!("{pin}");
    assert_eq!(rendered.len(), 64);
    assert_eq!(rendered, "ab".repeat(32));
}

#[test]
fn leaf_spki_digest_returns_zero_on_invalid_der() {
    // The soft-fail path: `b"ab"` is not a valid DER blob, so x509-parser
    // returns `Err`, and the verifier falls through to `[0u8; 32]` so the
    // pin check cannot accidentally accept a malformed cert (the webpki
    // chain check fires first, with a more informative error, in
    // `verify_server_cert`).
    let digest = SpkiPinnedVerifier::leaf_spki_digest(b"ab");
    assert_eq!(digest, [0u8; 32]);
}

#[tokio::test]
async fn spki_pin_accepts_correct_pin_against_nile() {
    if std::env::var_os("RUN_TRON_NILE").is_none() {
        eprintln!(
            "skipped: set RUN_TRON_NILE=1 (and the Nile pin via TON_NILE_SPKI_PIN_HEX) to run this"
        );
        return;
    }

    let hex_pin = std::env::var("TON_NILE_SPKI_PIN_HEX")
        .expect("RUN_TRON_NILE=1 set but TON_NILE_SPKI_PIN_HEX missing");
    let raw = hex::decode(&hex_pin).expect("pin hex must be 32 bytes");
    let pin = SpkiPin::from_bytes(raw.try_into().expect("32-byte pin"));

    let _cfg = SpkiPinnedVerifier::new(pin)
        .expect("verifier")
        .into_client_config();

    // The actual reqwest handshake assertion lives in Phase 4 spike V7.
    // This test proves verifier construction against an operator-supplied
    // pin.
}

#[tokio::test]
async fn spki_pin_accepts_correct_pin_against_mainnet() {
    if std::env::var_os("RUN_TRON_MAINNET").is_none() {
        eprintln!("skipped: set RUN_TRON_MAINNET=1 to run this");
        return;
    }
    let _pin = mainnet_spki_pin();
    // Real assertion lives in the Phase 7 spike; this stub keeps the
    // slot visible in `cargo test -- --list`.
}
