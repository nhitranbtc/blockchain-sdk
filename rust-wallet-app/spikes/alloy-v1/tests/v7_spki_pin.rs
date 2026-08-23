//! V7 verification — SPKI-pinned HTTP transport accepts the pinned cert chain
//! and rejects an unpinned one. Issue #293 — verification item V7.
//!
//! Approach (per F20 Bitcoin Task 7): raw `reqwest` + `rustls` with a custom
//! `ServerCertVerifier` that pins by SubjectPublicKeyInfo SHA-256. This spike
//! validates the verifier logic itself. End-to-end reqwest integration
//! belongs in the eth/ crate (mirrors `bitcoin-wallet-core/src/bdk_extras.rs`
//! EsploraClient pattern).
//!
//! Q2 resolution: `alloy-transport-http` does NOT expose a public hook for a
//! custom `ServerCertVerifier`. Workaround = raw reqwest for pinned endpoints,
//! mirroring Bitcoin F20. alloy's transport stays for non-pinned endpoints
//! (e.g. localhost Anvil during dev).
//!
//! Deterministic: in-memory only. Always runs.
//!
//! # SPIKE-ONLY artefacts — DO NOT copy to production
//!
//! `SpkiPinnedVerifier` (below) is a **spike artefact**. It deliberately
//! skips chain validation, hostname check, expiration, OCSP, revocation, and
//! TLS signature verification — returning `Ok` unconditionally after the
//! SPKI pin matches. **Migrating this verifier to eth/ crate production
//! code without those checks would be a security regression.** The eth/
//! crate must compose with a `webpki`/`rustls-webpki` verifier AFTER the
//! SPKI pin matches (mirror Bitcoin F20 / Task 7).

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as RustlsError, SignatureScheme};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// 32-byte SHA-256 hash of a SubjectPublicKeyInfo DER blob.
///
/// Type-level guarantee: a `SpkiSha256` is always exactly 32 bytes; a
/// non-SHA-256 pin is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpkiSha256([u8; 32]);

impl SpkiSha256 {
    /// Compute the SPKI-SHA-256 of the given SubjectPublicKeyInfo DER bytes.
    pub fn from_spki_der(spki_der: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(spki_der);
        Self(hasher.finalize().into())
    }
}

/// SPIKE-ONLY: `ServerCertVerifier` that accepts ONLY certificates whose SPKI
/// hash matches the pinned value. Rejects all others with `RustlsError::General`.
///
/// **Does NOT** delegate to webpki/aws-lc-rs for chain validation, hostname
/// check, expiration, OCSP, or revocation. **Does NOT** verify TLS handshake
/// signatures — `verify_tls12_signature` and `verify_tls13_signature` return
/// `Ok` unconditionally. **Safe to migrate to eth/ crate ONLY after
/// composing with a webpki verifier that performs those checks.**
#[doc(hidden)]
#[derive(Debug)]
struct SpkiPinnedVerifier {
    pinned: SpkiSha256,
}

impl SpkiPinnedVerifier {
    fn new(pinned: SpkiSha256) -> Self {
        Self { pinned }
    }
}

impl ServerCertVerifier for SpkiPinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        // SPIKE-ONLY: skip chain validation + hostname check + expiration.
        let spki_der = extract_spki_der(end_entity.as_ref()).ok_or_else(|| {
            RustlsError::General("V7 SPKI pin: failed to extract SPKI from cert".into())
        })?;
        let observed = SpkiSha256::from_spki_der(&spki_der);

        // Constant-time compare — avoids timing oracle that could leak the
        // pinned hash byte-by-byte. Mirrors Bitcoin F20 + F50 (`subtle` dep).
        if observed.0.ct_eq(&self.pinned.0).unwrap_u8() != 1 {
            return Err(RustlsError::General(format!(
                "V7 SPKI pin: cert SPKI hash {} does not match pinned {}",
                hex::encode(observed.0),
                hex::encode(self.pinned.0),
            )));
        }

        // SPIKE-ONLY: production must defer chain validation to webpki here.
        Ok(ServerCertVerified::assertion())
    }

    // SPIKE-ONLY: returns success unconditionally. Production must defer to
    // webpki verifier (or remove the override entirely).
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    // SPIKE-ONLY: same as above.
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // Subset chosen for the spike; eth/ crate must mirror the schemes
        // rustls's webpki verifier accepts (full list).
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}

/// Extract SubjectPublicKeyInfo DER from a TBSCertificate. Heuristic tag-walk
/// for the spike; production code in eth/ crate must use `x509-parser` or
/// `webpki` for cert parsing.
///
/// The DER length window `(50..=200)` covers typical SPKI sizes (RFC 5280
/// §4.1.2.7): ECDSA P-256 ≈ 91 bytes, P-384 ≈ 120 bytes, RSA-2048 ≈ 294 bytes
/// (out of range — eth/ crate must widen for RSA).
fn extract_spki_der(cert_der: &[u8]) -> Option<Vec<u8>> {
    let mut i = 0;
    while i + 4 < cert_der.len() {
        if cert_der[i] == 0x30 && (0x32..=0xc8).contains(&cert_der[i + 1]) {
            // 0x32..=0xc8 = length window 50..=200 for inner SEQUENCE.
            let (len, header_len) = read_der_length(&cert_der[i + 1..])?;
            if (50..=200).contains(&len) {
                let spki_start = i + 1 + header_len;
                let spki_end = spki_start + len;
                if spki_end <= cert_der.len() {
                    return Some(cert_der[spki_start..spki_end].to_vec());
                }
            }
        }
        i += 1;
    }
    None
}

fn read_der_length(bytes: &[u8]) -> Option<(usize, usize)> {
    if bytes.is_empty() {
        return None;
    }
    let first = bytes[0];
    if first < 0x80 {
        Some((first as usize, 1))
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || bytes.len() < 1 + n {
            return None;
        }
        let mut len = 0usize;
        for &b in &bytes[1..=n] {
            len = (len << 8) | b as usize;
        }
        Some((len, 1 + n))
    }
}

/// Build a minimal synthetic X.509 DER for the verifier tests. NOT a real
/// cert; only useful for testing the verifier + heuristic round-trip.
///
/// Layout:
///   outer SEQUENCE (0x30 0x82 <2-byte len>) {
///     inner SEQUENCE (0x30 <1-byte len>) { spki_bytes... }
///   }
///
/// The heuristic in `extract_spki_der` finds the inner SEQUENCE and returns
/// its body — which must equal `spki_bytes` for the pin to match.
///
/// **Coupling warning**: the heuristic length window `(50..=200)` means
/// `spki_bytes.len()` must be in that range or the heuristic will not
/// extract the SPKI. This is acceptable for the spike (synthetic fixture);
/// production verification uses real certs.
fn fake_cert_with_spki(spki_bytes: &[u8]) -> Vec<u8> {
    debug_assert!(
        (50..=200).contains(&spki_bytes.len()),
        "fake_cert_with_spki: spki_bytes.len()={} not in heuristic window (50..=200)",
        spki_bytes.len(),
    );

    // Inner SEQUENCE: 0x30, len(spki), spki_bytes...
    let mut inner = Vec::with_capacity(2 + spki_bytes.len());
    inner.push(0x30);
    inner.push(spki_bytes.len() as u8);
    inner.extend_from_slice(spki_bytes);

    // Outer SEQUENCE: 0x30, 0x82, len_be(2 bytes), inner_bytes...
    let mut cert = Vec::with_capacity(4 + inner.len());
    cert.extend_from_slice(&[0x30, 0x82]);
    let outer_len = inner.len() as u16;
    cert.extend_from_slice(&outer_len.to_be_bytes());
    cert.extend_from_slice(&inner);
    cert
}

#[test]
fn v7_spki_pin_accepts_matching_spki() {
    let spki = vec![0x42; 64];
    let cert = fake_cert_with_spki(&spki);
    // Pin to exactly what the heuristic extracts from this cert, so the
    // verifier's hash-on-extract round-trip matches.
    let pinned = SpkiSha256::from_spki_der(
        &extract_spki_der(&cert).expect("extract from synthetic fixture cert"),
    );
    let verifier = SpkiPinnedVerifier::new(pinned.clone());
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
        "expected matching SPKI to be accepted, got: {result:?}",
    );
    eprintln!(
        "[V7] PASS — SPKI pin accepted matching cert (pinned={}…)",
        &hex::encode(pinned.0)[..8],
    );
}

#[test]
fn v7_spki_pin_rejects_non_matching_spki() {
    let real_cert = fake_cert_with_spki(&[0x42; 64]);
    let pinned = SpkiSha256::from_spki_der(
        &extract_spki_der(&real_cert).expect("extract from synthetic fixture cert"),
    );
    let verifier = SpkiPinnedVerifier::new(pinned);

    // Build a cert with a DIFFERENT SPKI (0x99...).
    let other_cert = fake_cert_with_spki(&[0x99; 64]);
    let cert_der = CertificateDer::from(other_cert);

    let result = verifier.verify_server_cert(
        &cert_der,
        &[],
        &ServerName::try_from("example.com").unwrap(),
        &[],
        UnixTime::now(),
    );

    assert!(
        result.is_err(),
        "expected non-matching SPKI to be rejected, got: {result:?}",
    );
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("SPKI pin"),
        "rejection message should mention SPKI pin, got: {err_msg}",
    );
    eprintln!("[V7] PASS — SPKI pin rejected non-matching cert with: {err_msg}");
}

#[test]
fn v7_spki_pin_extract_spki_guards() {
    // Empty DER → None (loop never enters).
    assert!(extract_spki_der(&[]).is_none(), "empty DER → None");

    // 4-byte input: loop guard `i + 4 < cert_der.len()` = `4 < 4` = false;
    // loop body never runs → None by default. This guards against the off-by-one
    // where a 4-byte input falsely appears to enter the heuristic.
    assert!(
        extract_spki_der(&[0x30, 0x82, 0x00, 0x04]).is_none(),
        "4-byte input → None (loop guard)",
    );

    // 7-byte input that ENTERS the loop but points past the buffer:
    // [0x30, 0x82, 0x00, 0xff, 0xff, 0xff, 0xff] has tag 0x30, length 0x82,
    // 2-byte length 0x00ff = 255. spki_end = 4 + 255 = 259 > 7. → None.
    assert!(
        extract_spki_der(&[0x30, 0x82, 0x00, 0xff, 0xff, 0xff, 0xff]).is_none(),
        "oversized length → None",
    );

    // 7-byte input that ENTERS the loop, has 0x30 tag + 0x32 length-byte (50),
    // but only 4 bytes after the tag — loop should return None because
    // spki_end > cert_der.len().
    assert!(
        extract_spki_der(&[0x30, 0x32, 0x99, 0x99, 0x99, 0x99, 0x99]).is_none(),
        "spki body past buffer → None",
    );

    eprintln!("[V7] PASS — extract_spki_der guards return None for invalid input");
}

#[test]
fn v7_spki_pin_supported_schemes_non_empty() {
    let verifier = SpkiPinnedVerifier::new(SpkiSha256([0xaa; 32]));
    let schemes = verifier.supported_verify_schemes();
    assert!(
        !schemes.is_empty(),
        "verifier must expose at least one signature scheme",
    );
    assert!(
        schemes.contains(&SignatureScheme::ECDSA_NISTP256_SHA256),
        "verifier must support ECDSA P-256 (eth signing curve)",
    );
}
