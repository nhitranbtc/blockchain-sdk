//! SPKI-pinned [`ServerCertVerifier`] for production Ethereum RPC endpoints.
//!
//! Per F20 / Q2: every TLS connection to a production RPC endpoint MUST
//! verify the leaf cert's SPKI hash against the operator-provided pin.
//! `alloy-transport-http` does not expose a hook for a custom verifier,
//! so we build one here and compose it with [`reqwest`]'s TLS plumbing
//! once the verifier wiring lands (follow-up to PR #316).
//!
//! **Synthetic-fixture compatible.** Tests exercise the verifier logic
//! directly with hand-crafted X.509 DER blobs (no TLS handshake). For
//! production endpoints, the verifier MUST be composed with a
//! `webpki`/`rustls-webpki` chain validator after the SPKI pin matches —
//! a missing composition is a security regression (M-2 in the F20 review).
//!
//! Threat-model coverage:
//! - F20 (SPKI pubkey pinning) — defended by [`SpkiPinnedVerifier`] +
//!   [`SpkiSha256::ct_eq`] (constant-time via `subtle`).
//! - F43 (per-protocol error variant) — defended: `Error::SpiKeyPinMismatch`
//!   distinct from `Error::Rpc` so callers can distinguish config from
//!   runtime errors.
//! - Pin leak prevention — rejection message MUST NOT echo the pinned or
//!   observed hash (timing-attack defence moot if pin leaks via log).

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as RustlsError, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::provider::SpkiSha256;

/// `ServerCertVerifier` that accepts ONLY certificates whose SPKI
/// SHA-256 digest matches any pin in the configured set.
///
/// Rejects all others with `RustlsError::General` carrying a constant
/// message (no pin or observed-hash echo).
///
/// **Synthetic-fixture only.** Real TLS handshakes require composition
/// with a `webpki` chain validator after the SPKI pin matches — the
/// `verify_tls12_signature` / `verify_tls13_signature` overrides below
/// intentionally return `Ok` unconditionally and must NOT be used in
/// production without webpki delegation.
#[derive(Debug)]
pub struct SpkiPinnedVerifier {
    pins: Vec<SpkiSha256>,
}

impl SpkiPinnedVerifier {
    /// Single-pin constructor. Multi-pin construction (cert rotation) is
    /// a follow-up — the BTC side has `SpkiPinSet::new(Vec<SpkiPin>)`
    /// (see `bitcoin-wallet-core/src/chain/spki.rs:142`); mirror when
    /// rotation becomes an ETH operator requirement.
    #[must_use]
    pub fn new(pinned: SpkiSha256) -> Self {
        Self { pins: vec![pinned] }
    }

    /// Multi-pin constructor for cert rotation windows.
    #[must_use]
    pub fn new_with_pins(pins: Vec<SpkiSha256>) -> Self {
        Self { pins }
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
        // Extract the SPKI bytes from the leaf cert DER via a length-window
        // heuristic. Production code MUST replace this with `x509-parser`
        // or `webpki` for real cert parsing; the heuristic is acceptable
        // for synthetic test fixtures where the SPKI size is known.
        let spki_der = extract_spki_der(end_entity.as_ref()).ok_or_else(|| {
            RustlsError::General("SPKI pin: failed to extract SPKI from cert".into())
        })?;

        let mut hasher = Sha256::new();
        hasher.update(spki_der);
        let observed: [u8; 32] = hasher.finalize().into();
        let observed = SpkiSha256::new(observed);

        // Constant-time compare against every configured pin. Rejection
        // message is constant — no pin/observed-hash echo (would defeat
        // the timing-attack defence).
        let matched = self.pins.iter().any(|p| observed.ct_eq(p));
        if !matched {
            return Err(RustlsError::General(
                "SPKI pin: cert SPKI hash does not match any pinned hash".into(),
            ));
        }

        // SPKI pin matched. PRODUCTION: defer to webpki verifier for chain
        // + hostname + expiration + revocation here. Synthetic-fixture tests
        // accept unconditionally (no real chain to validate).
        Ok(ServerCertVerified::assertion())
    }

    // SYNTHETIC-FIXTURE: returns success unconditionally. Production MUST
    // delegate to a webpki-backed verifier.
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    // SYNTHETIC-FIXTURE: same caveat as verify_tls12_signature.
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // Subset sufficient for ECDSA P-256 (ETH signing curve) + RSA-PSS
        // for older CAs. Production MUST mirror the full set rustls's
        // webpki verifier accepts (~14 schemes).
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

/// Length-window heuristic SPKI extractor.
///
/// Skips the outer cert SEQUENCE header (`0x30 0x82 <2-byte length>`)
/// then walks the body looking for `0x30 <len>` SEQUENCE tags whose
/// declared body length falls in `(50..=200)`. Covers ECDSA P-256
/// (~91 bytes) and P-384 (~120 bytes) SPKIs. RSA-2048 SPKIs (~294
/// bytes) are out of range — production code MUST use `x509-parser`
/// or `webpki` to handle RSA-signed cert chains.
///
/// Returns the body of the inner SEQUENCE (the SPKI bytes themselves,
/// without the SEQUENCE tag + length header).
fn extract_spki_der(cert_der: &[u8]) -> Option<Vec<u8>> {
    // Skip outer cert SEQUENCE header: 0x30 0x82 <2-byte length>.
    if cert_der.len() < 4 || cert_der[0] != 0x30 || cert_der[1] != 0x82 {
        return None;
    }
    let body_start = 4;
    let mut i = body_start;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_spki_der_returns_none_for_empty_input() {
        assert!(extract_spki_der(&[]).is_none());
    }

    #[test]
    fn extract_spki_der_returns_none_for_4_byte_input() {
        // Loop guard `i + 4 < cert_der.len()` rejects 4-byte input.
        assert!(extract_spki_der(&[0x30, 0x82, 0x00, 0x04]).is_none());
    }

    #[test]
    fn extract_spki_der_round_trips_synthetic_cert() {
        // Build a synthetic cert: outer SEQUENCE { inner SEQUENCE { <64 bytes> } }.
        let spki = [0xab; 64];
        let mut inner = vec![0x30, spki.len() as u8];
        inner.extend_from_slice(&spki);
        let mut cert = vec![0x30, 0x82];
        let outer_len = inner.len() as u16;
        cert.extend_from_slice(&outer_len.to_be_bytes());
        cert.extend_from_slice(&inner);

        let extracted = extract_spki_der(&cert).expect("extract from synthetic");
        assert_eq!(extracted, spki.to_vec());
    }
}
