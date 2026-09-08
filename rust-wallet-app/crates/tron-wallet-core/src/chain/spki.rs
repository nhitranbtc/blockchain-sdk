//! SPKI pin: SHA-256 digest of a TLS server's `SubjectPublicKeyInfo` DER.
//!
//! The trait that actually connects a pin to a `reqwest`/`rustls` handshake is
//! [`SpkiPinnedVerifier`], which implements
//! [`rustls::client::danger::ServerCertVerifier`]. Construction lives in
//! [`crate::config::TronConfig::for_network`], which passes a verified pin in.
//! A live-network integration test (V7) is gating on `RUN_TRON_NILE=1` and
//! lives in `tests/v7_spki_pin.rs`.
//!
//! # Drift from the plan
//!
//! Plan §"Cross-crate reuse" says "reuse `bitcoin_wallet_core::chain::spki::SpkiPinnedVerifier`".
//! That type does not exist there today — `bitcoin-wallet-core` exposes a
//! [`SpkiPin`] but no verifier. The drift is intentional for v0.1 scope:
//! adding a cross-crate API to `bitcoin-wallet-core` from a `tron-wallet-core`
//! task would pull an unrelated crate's CHANGELOG. When
//! `bitcoin-wallet-core` gains a `SpkiPinnedVerifier`, this file should
//! switch to the upstream type.

use std::fmt;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as TlsError, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use x509_parser::prelude::FromDer;

use crate::error::{Error, Result};

/// SHA-256 SPKI pin. The inner field is the digest of the leaf cert's
/// `SubjectPublicKeyInfo` DER (algorithm identifier + subjectPublicKey
/// BIT STRING per RFC 7469), not the raw key bytes and not the
/// whole certificate. The verifier hashes only the SPKI, so the
/// operator can rotate the cert without losing the pin as long as
/// the new leaf carries the same key.
///
/// `Copy` is safe — the inner `[u8; 32]` is small and the pin is
/// operator-public (its whole purpose is to be published in config).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpkiPin {
    digest: [u8; 32],
}

impl SpkiPin {
    /// Wrap a 32-byte digest. The caller asserts SHA-256 origin; this layer
    /// cannot verify it.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { digest: bytes }
    }

    /// The 32-byte digest, read-only.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.digest
    }
}

impl fmt::Display for SpkiPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.digest))
    }
}

impl fmt::Debug for SpkiPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SpkiPin({})", hex::encode(self.digest))
    }
}

/// `ServerCertVerifier` that runs the full webpki chain/hostname/signature
/// checks first, and then adds an SPKI pin check on top.
#[derive(Clone, Debug)]
pub struct SpkiPinnedVerifier {
    pin: SpkiPin,
    inner: Arc<dyn ServerCertVerifier>,
}

impl SpkiPinnedVerifier {
    /// Build a verifier pinned to a single SPKI digest.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the platform's default crypto provider is not
    /// available (no `ring`/`aws-lc-rs` features turned on in `rustls`).
    pub fn new(pin: SpkiPin) -> Result<Self> {
        let mut roots = RootCertStore::empty();
        if let Ok(certs) = rustls_native_certs::load_native_certs() {
            for der in certs {
                // Best-effort: a malformed system cert does not abort.
                let _ = roots.add(der);
            }
        }

        // Acquire a `CryptoProvider`. If no process-default exists, install
        // ring's. `install_default` returns `Result<(), Arc<Self>>` —
        // success leaves no value, so we re-read the default after.
        let provider: Arc<CryptoProvider> = CryptoProvider::get_default()
            .map_or_else(
                || {
                    let _ = rustls::crypto::ring::default_provider().install_default();
                    CryptoProvider::get_default()
                },
                Some,
            )
            .ok_or_else(|| Error::SpkiPin("no default rustls crypto provider".into()))?
            .clone();
        let inner = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
            .build()
            .map_err(|e| Error::SpkiPin(format!("build webpki verifier: {e}")))?;

        Ok(Self { pin, inner })
    }

    /// Build a [`rustls::ClientConfig`] that pins via this verifier.
    pub fn into_client_config(self) -> rustls::ClientConfig {
        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(self))
            .with_no_client_auth()
    }

    /// Compute the SHA-256 digest of the leaf cert's `SubjectPublicKeyInfo`
    /// per RFC 7469 — what the operator-supplied pin records.
    ///
    /// Exposed for the [`tests::v7_spki_pin`] integration test, which
    /// constructs a pin from a known DER blob and asserts the verifier
    /// accepts the chain.
    pub fn leaf_spki_digest(leaf_der: &[u8]) -> [u8; 32] {
        // RFC 7469: pin is the SHA-256 of the full SPKI DER (algorithm
        // identifier + the public-key BIT STRING). `tbs_certificate.public_key().raw`
        // exposes that exact DER blob.
        match x509_parser::certificate::X509Certificate::from_der(leaf_der) {
            Ok((_, cert)) => Sha256::digest(cert.tbs_certificate.public_key().raw).into(),
            // Parse failure here is a soft-fail: the upstream webpki
            // chain check will reject the certificate with a more useful
            // error. We return a digest-of-zero so the verifier never
            // accidentally matches.
            Err(_) => [0u8; 32],
        }
    }
}

impl ServerCertVerifier for SpkiPinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, TlsError> {
        // Step 1: webpki chain + hostname + signature checks.
        self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        // Step 2: SPKI pin match. The leaf cert's `SubjectPublicKeyInfo`
        // SHA-256 must equal the operator-published pin. Hashing only the
        // SPKI (per RFC 7469) — not the whole cert — means the operator
        // can rotate the cert while keeping the same key without
        // invalidating the pin. `leaf_spki_digest` soft-fails to
        // `[0; 32]` on a parse error, so a malformed leaf cannot
        // accidentally match a zero-byte pin (the upstream webpki
        // chain check rejects malformed certs with a more useful
        // message before this code runs).
        let leaf_digest: [u8; 32] = Self::leaf_spki_digest(end_entity.as_ref());
        if leaf_digest.ct_eq(self.pin.as_bytes()).unwrap_u8() == 0 {
            return Err(TlsError::General(
                "spki pin mismatch (leaf spki did not equal configured pin)".into(),
            ));
        }

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}
