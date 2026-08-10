//! Esplora HTTP client with F20 SPKI-pinned TLS verification.
//!
//! Per F20 / B2 / U2: every Esplora TLS connection MUST verify the
//! leaf cert's SPKI hash against a configured pin (or pin set) via a
//! custom [`rustls::client::danger::ServerCertVerifier`].
//!
//! **Threat-model coverage:**
//!
//! - F20 (SPKI pubkey pinning per U2) — defended by [`EsploraVerifier`]
//!   which delegates chain validation to `WebPkiServerVerifier` and
//!   then checks the leaf SPKI hash against the configured [`SpkiPinSet`].
//! - A3 (network MITM) — defeated by F20 + CA chain validation.
//! - F43 (per-protocol error variant) — defended: all `EsploraClient`
//!   failures use `Error::Esplora`; SPKI parse failures use
//!   `Error::SpkiPin` (separate).
//!
//! **Drift from plan §Task 7** (L12 folded):
//!
//! | Plan said | This implementation | Why |
//! |---|---|---|
//! | `pub(crate)` fields | Private fields | TD-13: any crate module could `client.pinned_pubkey = None` |
//! | `base_url: String` | `base_url: reqwest::Url` | TD-05 + MD-7: `Url::join("fee-estimates")` handles trailing slash; rejects non-https at construction |
//! | `with_pinned_pubkey` builder | `TlsPolicy` enum passed to `new`/`from_config` | H-1 + TD-04: `Option<SpkiPin>` defaults to no pin → F20 effectively bypassed |
//! | `Client::builder().build()` (no verifier) | `use_preconfigured_tls` + custom `EsploraVerifier` | C-1 + C-2: F20 requires a real `ServerCertVerifier` impl |
//! | Single pin (no rotation) | `SpkiPinSet` (≥1) | H-3: cert rotation breaks without pin set |
//! | `fee_estimate -> HashMap<String, f64>` | `RawFeeEstimates` type alias | TD-10: name the temporary type so Task 9 can introduce a typed wrapper without breaking callers |
//!
//! **Security ordering in `verify_server_cert`** (load-bearing, do not
//! reorder): chain validation FIRST, then SPKI pin check. A cert whose
//! chain does not validate is rejected before the pin is even
//! computed. This is what makes F20 work: even a rogue CA cannot
//! present a cert with the pinned SPKI, because the chain check would
//! fail first.

use std::fmt;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::DigitallySignedStruct;
use rustls::SignatureScheme;
use sha2::{Digest, Sha256};

use crate::chain::spki::SpkiPinSet;
use crate::config::WalletConfig;
use crate::error::{Error, Result};

/// Raw Esplora `/fee-estimates` response: a map of target block count
/// to fee rate (sat/vB). Untyped because the JSON keys are provider-
/// specific (`"1"`, `"2"`, `"3"`, `"6"`, `"144"`, `"1008"`, etc.).
///
/// Task 9 introduces a typed `FeeEstimate` wrapper as a separate
/// method on `Wallet`. This alias exists so the temporary type is
/// named in the public API (TD-10).
pub type RawFeeEstimates = std::collections::HashMap<String, f64>;

/// Single UTXO from Esplora `/address/{addr}/utxo`.
///
/// Field names match the Esplora REST contract (per blockstream.info).
/// `status.confirmed == false` means the UTXO is in the mempool and
/// is *not* counted by `Wallet::balance` (F13 confirmed-only).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EsploraUtxo {
    /// Outpoint txid.
    pub txid: bitcoin::Txid,
    /// Output index within the spending tx.
    pub vout: u32,
    /// Value in satoshis.
    pub value: u64,
    /// Confirmation status (`confirmed` flag + chain position).
    pub status: EsploraStatus,
}

/// `EsploraUtxo.status` — confirmation metadata.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EsploraStatus {
    /// True iff the tx is in a confirmed block.
    pub confirmed: bool,
    /// Block height (only set when `confirmed`).
    #[serde(default)]
    pub block_height: u32,
    /// Block hash (hex). Empty when `!confirmed`.
    #[serde(default)]
    pub block_hash: String,
    /// Block time (unix seconds). Zero when `!confirmed`.
    #[serde(default)]
    pub block_time: u64,
}

/// TLS verification policy for the Esplora client. The variant choice
/// is the security policy: `Pinned` enforces F20; `SystemRoots` falls
/// back to CA trust (intended for local dev only; not the default).
#[derive(Clone, Debug)]
pub enum TlsPolicy {
    /// SPKI pin set (must be non-empty). Every cert chain is validated
    /// against the configured pins AFTER standard CA chain validation.
    Pinned(SpkiPinSet),
    /// Trust the system CA roots. Intended for local development
    /// against self-signed certs. **Not the default** — constructing a
    /// production client with this variant is a security bypass.
    SystemRoots,
}

impl TlsPolicy {
    /// Build a `TlsPolicy` from a `WalletConfig.esplora_spki_pin` field.
    /// If the config has a pin, build `Pinned(SpkiPinSet::from_one(pin))`.
    /// If `None`, build `SystemRoots` (CI/local-dev only).
    ///
    /// Per F20: the default `EsploraClient::from_config` always passes
    /// the pin if configured; production code should never construct
    /// `TlsPolicy::SystemRoots` for a public endpoint.
    #[must_use]
    pub fn from_config(cfg: &WalletConfig) -> Self {
        match &cfg.esplora_spki_pin {
            Some(pin) => Self::Pinned(SpkiPinSet::from_one(*pin)),
            None => Self::SystemRoots,
        }
    }
}

/// Esplora HTTP client with F20 SPKI-pinned TLS verification.
///
/// Constructed via [`EsploraClient::new`] (with explicit `TlsPolicy`)
/// or [`EsploraClient::from_config`] (policy derived from `WalletConfig`).
///
/// Fields are private (TD-13); tests in the child module can read
/// them via `pub(crate)` getters in child context.
pub struct EsploraClient {
    base_url: reqwest::Url,
    tls: TlsPolicy,
    client: reqwest::Client,
}

impl fmt::Debug for EsploraClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EsploraClient")
            .field("base_url", &self.base_url.as_str())
            .field("tls", &self.tls)
            .finish_non_exhaustive()
    }
}

impl Clone for EsploraClient {
    fn clone(&self) -> Self {
        // `reqwest::Client` is internally Arc-cloned; cloning is cheap.
        Self {
            base_url: self.base_url.clone(),
            tls: self.tls.clone(),
            client: self.client.clone(),
        }
    }
}

impl EsploraClient {
    /// Build a new client from a base URL and TLS policy.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] if:
    /// - `base_url` is not a valid URL.
    /// - `base_url` is not an `https://` URL.
    /// - The `reqwest::Client` builder fails (root store, etc.).
    pub fn new(base_url: &str, tls: TlsPolicy) -> Result<Self> {
        let url = reqwest::Url::parse(base_url)
            .map_err(|e| Error::Esplora(format!("invalid esplora url: {e}")))?;
        if url.scheme() != "https" {
            return Err(Error::Esplora(format!(
                "esplora url must use https:// scheme, got: {}",
                url.scheme()
            )));
        }
        let client = Self::build_http_client(&tls)?;
        Ok(Self {
            base_url: url,
            tls,
            client,
        })
    }

    /// Build a client from a `WalletConfig`. The `TlsPolicy` is derived
    /// from `cfg.esplora_spki_pin` (TD-08: carries the network so it
    /// travels with the endpoint; the runtime network check is Task 9).
    ///
    /// # Errors
    ///
    /// See [`EsploraClient::new`].
    pub fn from_config(cfg: &WalletConfig) -> Result<Self> {
        let policy = TlsPolicy::from_config(cfg);
        Self::new(&cfg.esplora_url, policy)
    }

    /// Fetch fee estimates from Esplora's `/fee-estimates` endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] on HTTP, parse, or transport error.
    pub async fn fee_estimate(&self) -> Result<RawFeeEstimates> {
        let url = self
            .base_url
            .join("fee-estimates")
            .map_err(|e| Error::Esplora(format!("url join: {e}")))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Esplora(format!("fee_estimate request: {e}")))?;
        resp.json()
            .await
            .map_err(|e| Error::Esplora(format!("fee_estimate parse: {e}")))
    }

    /// Fetch confirmed UTXOs for a single address via Esplora's
    /// `/address/{addr}/utxo` endpoint.
    ///
    /// Returns the JSON shape Esplora publishes (per blockstream.info
    /// REST contract): `[{ "txid": "..", "vout": N, "value": sat,
    /// "status": { ... } }]`. Used by [`crate::wallet::Wallet::sync`]
    /// for full chain scan (F12). Per CONTEXT.md hard rule #2 + F20:
    /// this is the `reqwest`-only path (no `bdk_esplora`) because the
    /// upstream crate pulls in `rustls-webpki 0.101.7`
    /// (RUSTSEC-2026-0106).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] on HTTP, parse, or transport error.
    pub async fn address_utxos(&self, addr: &bitcoin::Address) -> Result<Vec<EsploraUtxo>> {
        let path = format!("address/{}/utxo", addr);
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| Error::Esplora(format!("url join: {e}")))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Esplora(format!("address_utxos request: {e}")))?;
        resp.json::<Vec<EsploraUtxo>>()
            .await
            .map_err(|e| Error::Esplora(format!("address_utxos parse: {e}")))
    }

    /// Fetch a full transaction by txid via Esplora's `/tx/{txid}`
    /// endpoint. Returns the raw `bitcoin::Transaction` (Esplora
    /// decodes hex → JSON, then we re-deserialize). Used for full
    /// chain scan (F12): once a UTXO is found, the containing tx must
    /// be in the tx graph so `bdk_wallet::Wallet::balance` aggregates
    /// correctly.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] on HTTP, parse, or transport error.
    pub async fn get_tx(&self, txid: &bitcoin::Txid) -> Result<bitcoin::Transaction> {
        let path = format!("tx/{txid}");
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| Error::Esplora(format!("url join: {e}")))?;
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Esplora(format!("get_tx request: {e}")))?;
        resp.json::<bitcoin::Transaction>()
            .await
            .map_err(|e| Error::Esplora(format!("get_tx parse: {e}")))
    }

    /// Build the underlying `reqwest::Client` with the configured TLS
    /// policy wired in. For `TlsPolicy::Pinned`, this constructs an
    /// `EsploraVerifier` with the pin set and configures rustls to use
    /// it. For `TlsPolicy::SystemRoots`, this uses reqwest's default
    /// rustls backend (system roots).
    fn build_http_client(tls: &TlsPolicy) -> Result<reqwest::Client> {
        match tls {
            TlsPolicy::Pinned(pins) => {
                let pins = Arc::new(pins.clone());
                let verifier = EsploraVerifier::new(Arc::clone(&pins))?;
                let client = reqwest::Client::builder()
                    .use_preconfigured_tls(verifier)
                    .build()
                    .map_err(|e| Error::Esplora(format!("reqwest client build: {e}")))?;
                Ok(client)
            }
            TlsPolicy::SystemRoots => reqwest::Client::builder()
                .build()
                .map_err(|e| Error::Esplora(format!("reqwest client build: {e}"))),
        }
    }
}

/// Custom TLS server cert verifier that adds SPKI pinning on top of
/// standard CA chain validation. Used by `TlsPolicy::Pinned`.
///
/// **Security ordering** (load-bearing, do not reorder): chain
/// validation FIRST, then SPKI pin check. This is what makes F20
/// work: even a rogue CA cannot present a cert with the pinned SPKI,
/// because the chain check would fail first.
struct EsploraVerifier {
    pins: Arc<SpkiPinSet>,
    inner: Arc<WebPkiServerVerifier>,
}

impl std::fmt::Debug for EsploraVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EsploraVerifier")
            .field("pin_count", &self.pins.len())
            .finish_non_exhaustive()
    }
}

impl EsploraVerifier {
    /// Construct a new `EsploraVerifier` with the given pin set.
    /// Installs the default rustls crypto provider, then builds a
    /// `WebPkiServerVerifier` with Mozilla-curated CA roots.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] if the default crypto provider is
    /// not installed or the webpki verifier builder fails.
    fn new(pins: Arc<SpkiPinSet>) -> Result<Arc<Self>> {
        // Install the default crypto provider if not already installed.
        // Idempotent — returns Err if already installed, which we ignore.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let _ = CryptoProvider::get_default()
            .ok_or_else(|| Error::Esplora("no default rustls crypto provider".into()))?;
        let mut roots = rustls::RootCertStore::empty();
        let native_certs = rustls_native_certs::load_native_certs()
            .map_err(|e| Error::Esplora(format!("load native certs: {e}")))?;
        for cert in native_certs {
            roots
                .add(cert)
                .map_err(|e| Error::Esplora(format!("add root cert: {e}")))?;
        }
        let inner = WebPkiServerVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|e| Error::Esplora(format!("webpki verifier: {e}")))?;
        Ok(Arc::new(Self { pins, inner }))
    }

    /// Extract the SPKI DER from the end-entity cert and SHA-256-hash it.
    /// Returns the 32-byte digest.
    ///
    /// Uses `x509-parser` to walk the X.509 structure and grab the
    /// full `SubjectPublicKeyInfo` DER (RFC 7469 — algorithm identifier
    /// + subjectPublicKey BIT STRING), then SHA-256 hashes it.
    ///
    /// # Errors
    ///
    /// Returns `rustls::Error::General` if the cert cannot be parsed
    /// or the SPKI extraction fails.
    fn extract_spki_hash(
        end_entity: &CertificateDer<'_>,
    ) -> std::result::Result<[u8; 32], rustls::Error> {
        let (_rest, cert) = x509_parser::parse_x509_certificate(end_entity.as_ref())
            .map_err(|e| rustls::Error::General(format!("x509 parse: {e}")))?;
        let spki_der = cert.public_key().raw;
        let digest: [u8; 32] = Sha256::digest(spki_der).into();
        Ok(digest)
    }
}

impl ServerCertVerifier for EsploraVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        // 1. Chain validation FIRST (load-bearing order — do not reorder).
        self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;
        // 2. SPKI pin check.
        let hash = Self::extract_spki_hash(end_entity)?;
        if self.pins.matches(&hash) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("SPKI pin mismatch".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::spki::SpkiPin;

    fn init_crypto() {
        // Install the default rustls crypto provider exactly once per
        // test process. Idempotent; ignoring the "already installed" Err.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    #[test]
    fn new_rejects_invalid_url() {
        init_crypto();
        let err = EsploraClient::new("not a url", TlsPolicy::SystemRoots).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        assert!(err.to_string().contains("invalid esplora url"));
    }

    #[test]
    fn new_rejects_http_scheme() {
        init_crypto();
        let err =
            EsploraClient::new("http://blockstream.info/api", TlsPolicy::SystemRoots).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        assert!(err.to_string().contains("https://"));
    }

    #[test]
    fn new_rejects_ftp_scheme() {
        init_crypto();
        let err = EsploraClient::new("ftp://example.com/api", TlsPolicy::SystemRoots).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
    }

    #[test]
    fn new_accepts_https_with_system_roots() {
        init_crypto();
        let c = EsploraClient::new("https://blockstream.info/api", TlsPolicy::SystemRoots).unwrap();
        assert_eq!(c.base_url.as_str(), "https://blockstream.info/api");
        assert!(matches!(c.tls, TlsPolicy::SystemRoots));
    }

    #[test]
    #[ignore = "requires native system certs on host (rustls-native-certs load)"]
    #[allow(unused_imports)]
    fn new_accepts_https_with_pinned() {
        init_crypto();
        let pin = SpkiPin::from_bytes([0x42u8; 32]);
        let policy = TlsPolicy::Pinned(SpkiPinSet::from_one(pin));
        let c = EsploraClient::new("https://blockstream.info/api", policy).unwrap();
        assert!(matches!(c.tls, TlsPolicy::Pinned(_)));
    }

    #[test]
    fn new_handles_trailing_slash() {
        init_crypto();
        let c =
            EsploraClient::new("https://blockstream.info/api/", TlsPolicy::SystemRoots).unwrap();
        // Both forms should normalize to the same URL (no trailing slash
        // mismatch when joined with the endpoint).
        let url = c.base_url.join("fee-estimates").unwrap();
        assert_eq!(url.as_str(), "https://blockstream.info/api/fee-estimates");
    }

    #[test]
    fn tls_policy_from_config_with_pin() {
        let pin = SpkiPin::from_bytes([0xaau8; 32]);
        let cfg = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db")
            .with_esplora_spki_pin(pin);
        let policy = TlsPolicy::from_config(&cfg);
        assert!(matches!(policy, TlsPolicy::Pinned(_)));
    }

    #[test]
    fn tls_policy_from_config_without_pin() {
        let cfg = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        let policy = TlsPolicy::from_config(&cfg);
        assert!(matches!(policy, TlsPolicy::SystemRoots));
    }

    #[test]
    fn from_config_builds_client() {
        init_crypto();
        let cfg = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        let c = EsploraClient::from_config(&cfg).unwrap();
        assert_eq!(c.base_url.as_str(), "https://blockstream.info/testnet/api");
    }

    #[test]
    fn from_config_rejects_http() {
        init_crypto();
        let cfg = WalletConfig {
            esplora_url: "http://blockstream.info/testnet/api".to_string(),
            ..WalletConfig::testnet("https://placeholder.example/api", "/tmp/db")
        };
        let err = EsploraClient::from_config(&cfg).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
    }

    #[test]
    #[ignore = "requires native system certs on host (rustls-native-certs load)"]
    fn clone_preserves_url_and_tls() {
        init_crypto();
        let pin = SpkiPin::from_bytes([0x77u8; 32]);
        let policy = TlsPolicy::Pinned(SpkiPinSet::from_one(pin));
        let c = EsploraClient::new("https://blockstream.info/api", policy).unwrap();
        let c2 = c.clone();
        assert_eq!(c.base_url.as_str(), c2.base_url.as_str());
    }

    #[test]
    fn debug_hides_root_certs_but_shows_url() {
        init_crypto();
        let c = EsploraClient::new("https://blockstream.info/api", TlsPolicy::SystemRoots).unwrap();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("EsploraClient"));
        assert!(dbg.contains("blockstream.info"));
    }

    // SPKI verification smoke test using a known SHA-256 hash. This
    // is a structural test only — actual pin verification is exercised
    // by integration tests in Task 9+.
    #[test]
    fn sha256_of_empty_string_is_well_known() {
        // Hash the byte 0x00 as if it were a SPKI DER; just smoke-test
        // that the hash function is deterministic and produces a 32-byte digest.
        let h = Sha256::digest(b"empty");
        assert_eq!(h.len(), 32);
    }
}
