//! V7 — SPKI pin (Q7) — GATED for live cert verification (L29).
//!
//! Plan §Q7: `bitcoin_wallet_core::chain::spki` SPKI pin primitives reused
//! (`SpkiPin` + `SpkiPinSet`; drift note in `src/spki.rs`).
//! `pinned://<spki-base64>@host[:port]` URL scheme. Pin against
//! `api.trongrid.io` requires Cloudflare rotation handling (~30 day cadence).
//! For test, we use a placeholder pin to confirm the URL parser + pin-set
//! wiring. The live cert check is gated behind RUN_TRON_NILE=1.

use bitcoin_wallet_core::chain::spki::{SpkiPin, SpkiPinSet};
use tron_v1_spike::spki::{parse_pinned_url, pin_set_from_bytes, ParseError};

#[test]
fn v7_pinned_url_parse_basic() {
    let url = "pinned://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef@api.trongrid.io:443";
    let (pin, host, port) = parse_pinned_url(url).unwrap();
    assert_eq!(pin.len(), 64); // 32-byte SHA-256 hex = 64 chars
    assert_eq!(host, "api.trongrid.io");
    assert_eq!(port, 443);
}

#[test]
fn v7_pinned_url_default_port_443() {
    let url = "pinned://abcdef@api.trongrid.io";
    let (_, _, port) = parse_pinned_url(url).unwrap();
    assert_eq!(port, 443);
}

#[test]
fn v7_pinned_url_rejects_wrong_scheme() {
    let url = "https://abcdef@api.trongrid.io";
    assert_eq!(parse_pinned_url(url), Err(ParseError::BadScheme));
}

#[test]
fn v7_pinned_url_rejects_missing_at() {
    let url = "pinned://api.trongrid.io";
    assert_eq!(parse_pinned_url(url), Err(ParseError::NoAt));
}

#[test]
fn v7_pinned_url_rejects_bad_port() {
    let url = "pinned://abcdef@api.trongrid.io:notaport";
    assert_eq!(parse_pinned_url(url), Err(ParseError::BadPort));
}

#[test]
fn v7_pinset_constructable_from_bytes() {
    // Type-check only: confirms `SpkiPinSet` is reachable via the spike crate.
    // Live TLS handshake + cert rejection is gated on RUN_TRON_NILE=1 + a real Cloudflare
    // SPKI pin (rotates ~every 30 days; placeholder pin would always reject).
    let pin = SpkiPin::from_bytes([0x42u8; 32]);
    let pin_set = SpkiPinSet::new(vec![pin]).expect("pin set accepts single pin");
    assert_eq!(pin_set.len(), 1);
}

#[test]
fn v7_pinned_url_to_pinset_roundtrip() {
    // End-to-end URL-parse → SpkiPinSet pipeline used by the V7 live test.
    // `pinned://` URL format per plan §Q7 uses 64-char hex SPKI SHA-256.
    let hex_pin = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let url = format!("pinned://{hex_pin}@api.trongrid.io:443");
    let (pin_hex, host, port) = parse_pinned_url(&url).unwrap();
    assert_eq!(host, "api.trongrid.io");
    assert_eq!(port, 443);
    let _pin_set = tron_v1_spike::spki::pin_set_from_hex(&pin_hex)
        .expect("valid 64-char hex should construct SpkiPinSet");
}

#[test]
fn v7_pinset_from_raw_bytes_via_helper() {
    // Exercises the `pin_set_from_bytes` helper exposed by the spike crate.
    let _pin_set = pin_set_from_bytes([0xab; 32]).expect("valid 32-byte pin should construct");
}
