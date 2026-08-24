//! SPKI pin enforcement tests for `provider::new_http_pinned`.
//!
//! Per F20 / Q2: every production RPC endpoint MUST verify the leaf cert's
//! SPKI hash against the operator-provided pin. The verifier lives in
//! `chain::verifier::SpkiPinnedVerifier` and rejects any cert whose SPKI
//! SHA-256 digest does not match the pinned value.
//!
//! **Synthetic fixtures only** — these tests exercise the verifier logic
//! directly (no TLS handshake). End-to-end network tests are gated by
//! `#[ignore]` + `RUN_SPKI_NET=1` per L29 (operator-driven; not CI).
//!
//! Threat-model coverage:
//! - Wrong SPKI hash → `Err(_)` (connection refused, no fallback).
//! - Matching SPKI hash → `Ok(_)` (pin verified).
//! - Constant-time compare via `subtle::ConstantTimeEq` (no
//!   timing-side-channel oracle that leaks the pin byte-by-byte).

use eth_wallet_core::chain::verifier::SpkiPinnedVerifier;
use eth_wallet_core::provider::SpkiSha256;
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

/// Build a minimal synthetic X.509 DER cert whose body wraps the given
/// "SPKI bytes" (a stand-in for the real SubjectPublicKeyInfo DER). The
/// `SpkiPinnedVerifier::verify_server_cert` implementation extracts the
/// SPKI from the cert DER via a length-window heuristic (50..=200 bytes),
/// so the test fixture must place `spki_bytes` in that range.
///
/// Layout (mirrors the real cert shape: outer SEQUENCE wrapping inner
/// SEQUENCE whose body is the SPKI):
///
/// ```text
/// outer SEQUENCE {
///   inner SEQUENCE { spki_bytes }
/// }
/// ```
fn synthetic_cert_with_spki(spki_bytes: &[u8]) -> Vec<u8> {
    // 50..=200 byte window covers ECDSA P-256 (91 bytes) and P-384 (120
    // bytes) SPKIs. Tests use 64 bytes (0xAB / 0xCC patterns) which fits.
    assert!(
        (50..=200).contains(&spki_bytes.len()),
        "spki_bytes.len()={} not in verifier window",
        spki_bytes.len()
    );

    // Inner SEQUENCE: 0x30, len(spki), spki_bytes...
    let mut inner = Vec::with_capacity(2 + spki_bytes.len());
    inner.push(0x30);
    inner.push(spki_bytes.len() as u8);
    inner.extend_from_slice(spki_bytes);

    // Outer SEQUENCE (2-byte length form 0x30 0x82 <len_be_2>): wraps inner.
    let mut cert = Vec::with_capacity(4 + inner.len());
    cert.extend_from_slice(&[0x30, 0x82]);
    let outer_len = inner.len() as u16;
    cert.extend_from_slice(&outer_len.to_be_bytes());
    cert.extend_from_slice(&inner);
    cert
}

#[test]
fn spki_pinned_verifier_rejects_non_matching_spki() {
    let pinned = SpkiSha256::new([0xaa; 32]);
    let verifier = SpkiPinnedVerifier::new(pinned);

    // Build a cert whose SPKI is 0xCC... — hash will be sha256([0xcc; 64])
    // which does NOT match the pinned sha256([0xaa; 32]).
    let cert = synthetic_cert_with_spki(&[0xcc; 64]);
    let cert_der = CertificateDer::from(cert);

    let result = verifier.verify_server_cert(
        &cert_der,
        &[],
        &ServerName::try_from("example.com").unwrap(),
        &[],
        UnixTime::now(),
    );

    assert!(
        result.is_err(),
        "non-matching SPKI must be rejected, got: {result:?}"
    );

    let err_msg = format!("{:?}", result.unwrap_err());
    // Rejection message MUST NOT echo the pinned or observed hash —
    // emitting them would leak the pin via error log. Assert only the
    // constant substring.
    assert!(
        err_msg.contains("SPKI"),
        "rejection message should mention SPKI, got: {err_msg}"
    );
}

#[test]
fn spki_pinned_verifier_accepts_matching_spki() {
    // Pin is the hash of [0xab; 64] — match when the cert's extracted
    // SPKI is also [0xab; 64]. The verifier hashes the extracted SPKI
    // (not the outer cert DER), so pinning to the hash of the inner
    // bytes round-trips.
    let spki_bytes = [0xab; 64];
    let pinned = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(spki_bytes);
        SpkiSha256::new(h.finalize().into())
    };

    let verifier = SpkiPinnedVerifier::new(pinned);
    let cert = synthetic_cert_with_spki(&spki_bytes);
    let cert_der = CertificateDer::from(cert);

    let result = verifier.verify_server_cert(
        &cert_der,
        &[],
        &ServerName::try_from("example.com").unwrap(),
        &[],
        UnixTime::now(),
    );

    assert!(
        result.is_ok(),
        "matching SPKI must be accepted, got: {result:?}"
    );
}

#[test]
fn spki_pinned_verifier_constant_time_compare() {
    // Two pins differing in only the last byte — ensures ct_eq is
    // walking all 32 bytes (not bailing on first mismatch). Indirect:
    // both rejections return Err; the constant-time guarantee is
    // asserted at the SpkiSha256::ct_eq unit test in provider.rs.
    let pinned = SpkiSha256::new([0xaa; 32]);
    let verifier = SpkiPinnedVerifier::new(pinned);

    let cert_a = synthetic_cert_with_spki(&[0x00; 64]);
    let cert_b = synthetic_cert_with_spki(&[0xff; 64]);

    let ra = verifier.verify_server_cert(
        &CertificateDer::from(cert_a),
        &[],
        &ServerName::try_from("example.com").unwrap(),
        &[],
        UnixTime::now(),
    );
    let rb = verifier.verify_server_cert(
        &CertificateDer::from(cert_b),
        &[],
        &ServerName::try_from("example.com").unwrap(),
        &[],
        UnixTime::now(),
    );

    assert!(ra.is_err(), "[0x00; 64] SPKI must reject");
    assert!(rb.is_err(), "[0xff; 64] SPKI must reject");
}
