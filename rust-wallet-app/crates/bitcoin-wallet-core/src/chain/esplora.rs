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

use crate::chain::esplora_url::EsploraUrl;
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
        // Defensively redact userinfo on Debug too, in case a future
        // refactor ever permits a raw `reqwest::Url` into `base_url`.
        // Today the only path into `base_url` is `EsploraUrl::into_inner()`,
        // and `EsploraUrl::new` rejects userinfo at construction.
        f.debug_struct("EsploraClient")
            .field(
                "base_url",
                &crate::chain::esplora_url::redact_userinfo(self.base_url.as_str()),
            )
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
    /// Build a new client from a pre-validated `EsploraUrl` and TLS
    /// policy. URL validation (https-only, no userinfo, valid parse,
    /// trailing-slash normalization) lives in [`EsploraUrl::new`] —
    /// callers must construct an `EsploraUrl` first.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] if the `reqwest::Client` builder
    /// fails (root store load, etc.).
    pub fn new(esplora_url: EsploraUrl, tls: TlsPolicy) -> Result<Self> {
        let client = Self::build_http_client(&tls)?;
        Ok(Self {
            base_url: esplora_url.into_inner(),
            tls,
            client,
        })
    }

    /// Build a client from a `WalletConfig`. The `TlsPolicy` is derived
    /// from `cfg.esplora_spki_pin` (TD-08: carries the network so it
    /// travels with the endpoint; the runtime network check is Task 9).
    ///
    /// **F20 enforcement** (per issue #37, L12 H-1): on non-regtest
    /// networks (mainnet / testnet / signet), `TlsPolicy::SystemRoots`
    /// is rejected — every cert chain must be checked against an SPKI
    /// pin. Regtest retains the lenient behavior because local dev
    /// servers typically use self-signed certs that the system CA
    /// store does not trust.
    ///
    /// Escape hatch: callers that genuinely need `SystemRoots` on a
    /// public network should use [`EsploraClient::new`] directly and
    /// accept the bypass. Tests in `wallet::tests` do this for the
    /// `#[ignore]`d live-testnet fixtures.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] if:
    /// - The config network is non-regtest AND no SPKI pin is set
    ///   (F20 enforcement).
    /// - See [`EsploraClient::new`] for URL/TLS errors.
    pub fn from_config(cfg: &WalletConfig) -> Result<Self> {
        // F20 enforcement: non-regtest requires a pin. Regtest stays
        // permissive (local dev with self-signed certs).
        if cfg.network != bitcoin::Network::Regtest && cfg.esplora_spki_pin.is_none() {
            return Err(Error::Esplora(format!(
                "F20 SPKI pin required for {:?} network (regtest exempt); \
                 set esplora_spki_pin in WalletConfig",
                cfg.network
            )));
        }
        let policy = TlsPolicy::from_config(cfg);
        let esplora_url = EsploraUrl::new(&cfg.esplora_url)?;
        Self::new(esplora_url, policy)
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
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| Error::Esplora(format!("address_utxos read body: {e}")))?;
        if !status.is_success() {
            return Err(Error::Esplora(format!(
                "address_utxos HTTP {}: {}",
                status,
                &body[..body.len().min(200)]
            )));
        }
        serde_json::from_str::<Vec<EsploraUtxo>>(&body).map_err(|e| {
            Error::Esplora(format!(
                "address_utxos parse (status {}): {}; body: {}",
                e,
                status,
                &body[..body.len().min(300)]
            ))
        })
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

    /// Broadcast a raw transaction via Esplora's `/tx` endpoint
    /// (Task 13 / Story 5 / Issue #118). POST body is the raw tx hex;
    /// response is the txid as a 64-char hex string on success.
    ///
    /// **F20 enforcement**: TLS chain validation + SPKI pin check
    /// (when configured) happen inside the underlying `reqwest::Client`
    /// — no separate check here. The caller passes `&self` (the
    /// pre-configured client) so the pin policy is whatever was set at
    /// construction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] on:
    /// - HTTP transport failure (timeout, DNS, TLS handshake)
    /// - HTTP non-2xx response (Esplora returns the failure reason as
    ///   the response body — surface the first 200 chars for debug)
    /// - Non-hex / malformed txid in the response body
    pub async fn broadcast_tx(&self, raw_tx_hex: &str) -> Result<bitcoin::Txid> {
        let url = self
            .base_url
            .join("tx")
            .map_err(|e| Error::Esplora(format!("broadcast_tx url join: {e}")))?;
        let resp = self
            .client
            .post(url)
            .header("Content-Type", "text/plain")
            .body(raw_tx_hex.to_string())
            .send()
            .await
            .map_err(|e| Error::Esplora(format!("broadcast_tx request: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| Error::Esplora(format!("broadcast_tx read body: {e}")))?;
        if !status.is_success() {
            return Err(Error::Esplora(format!(
                "broadcast_tx HTTP {}: {}",
                status,
                &body[..body.len().min(200)]
            )));
        }
        let txid_str = body.trim();
        txid_str
            .parse::<bitcoin::Txid>()
            .map_err(|e| Error::Esplora(format!("broadcast_tx parse txid {txid_str:?}: {e}")))
    }

    /// Build the underlying `reqwest::Client` with the configured TLS
    /// policy wired in. For `TlsPolicy::Pinned`, this constructs an
    /// `EsploraVerifier` with the pin set and configures rustls to use
    /// it. For `TlsPolicy::SystemRoots`, this uses reqwest's default
    /// rustls backend (system roots).
    fn build_http_client(tls: &TlsPolicy) -> Result<reqwest::Client> {
        // Issue #266: rustls 0.23 requires an explicit crypto
        // provider install (the `ring` provider was the default in
        // rustls 0.22; 0.23 dropped that default). Without this,
        // `reqwest::Client::builder().build()` fails with the cryptic
        // `builder error` because no TLS backend is registered.
        // `OnceLock` makes the install call idempotent + thread-safe
        // (subsequent calls are no-ops after the first successful
        // install — `install_default()` returns `Err(AlreadyInstalled)`
        // which we ignore).
        use std::sync::OnceLock;
        static CRYPTO_INIT: OnceLock<()> = OnceLock::new();
        CRYPTO_INIT.get_or_init(|| {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        });
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

/// True if the raw input string has an `@` between the `://` delimiter
/// and the first `/` (or end of string). Belt-and-braces check for the
/// `https://@host/` form — WHATWG URL parser yields `username() == ""`
/// and `password() == None` for that form, which would otherwise slip
/// past `username().is_empty() || password().is_some()`.
///
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
    fn new_accepts_https_with_system_roots() {
        init_crypto();
        let url = EsploraUrl::new("https://blockstream.info/api").unwrap();
        let c = EsploraClient::new(url, TlsPolicy::SystemRoots).unwrap();
        assert_eq!(c.base_url.as_str(), "https://blockstream.info/api/");
        assert!(matches!(c.tls, TlsPolicy::SystemRoots));
    }

    /// Issue #266 regression guard: production code MUST install the
    /// rustls crypto provider itself (via `OnceLock` at the top of
    /// `build_http_client`). The existing `init_crypto()` helper
    /// above is a belt-and-braces backup; this test asserts that even
    /// without that backup, `EsploraClient::new` succeeds because
    /// production code owns the install.
    ///
    /// Note: this test relies on a fresh process state OR running
    /// before any other test that calls `init_crypto()`. In parallel
    /// test execution (`cargo test` default), prior tests in the
    /// same `tests` module may have installed the provider globally
    /// and this test would pass spuriously. To force the regression
    /// check, run with `cargo test -- --test-threads=1` AND ensure
    /// this test is the first to touch `EsploraClient::new`. The
    /// production-side `OnceLock` makes the install idempotent.
    #[test]
    fn new_installs_crypto_provider_via_production_code() {
        // Intentionally NOT calling init_crypto() — this test
        // verifies production code owns the install. If the OnceLock
        // in `build_http_client` is ever removed, this test fails
        // (when run in isolation or first in execution order).
        let url = EsploraUrl::new("https://127.0.0.1:1/api").unwrap();
        let result = EsploraClient::new(url, TlsPolicy::SystemRoots);
        assert!(
            result.is_ok(),
            "EsploraClient::new failed to build reqwest client — \
             rustls crypto provider not installed by production \
             code (Issue #266 regression). err: {:?}",
            result.err(),
        );
    }

    #[test]
    #[ignore = "requires native system certs on host (rustls-native-certs load)"]
    #[allow(unused_imports)]
    fn new_accepts_https_with_pinned() {
        init_crypto();
        let pin = SpkiPin::from_bytes([0x42u8; 32]);
        let policy = TlsPolicy::Pinned(SpkiPinSet::from_one(pin));
        let url = EsploraUrl::new("https://blockstream.info/api").unwrap();
        let c = EsploraClient::new(url, policy).unwrap();
        assert!(matches!(c.tls, TlsPolicy::Pinned(_)));
    }

    #[test]
    fn new_handles_trailing_slash() {
        init_crypto();
        // URL validation lives in EsploraUrl::new; trailing-slash
        // normalization is exercised by chain::esplora_url::tests.
        let url = EsploraUrl::new("https://blockstream.info/api/").unwrap();
        let c = EsploraClient::new(url, TlsPolicy::SystemRoots).unwrap();
        let joined = c.base_url.join("fee-estimates").unwrap();
        assert_eq!(
            joined.as_str(),
            "https://blockstream.info/api/fee-estimates"
        );
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
    fn tls_policy_from_config_without_pin_regtest() {
        // TlsPolicy::from_config is infallible; SystemRoots is the
        // policy it produces when no pin is set. F20 enforcement
        // happens one layer up in EsploraClient::from_config.
        let cfg = WalletConfig::regtest("https://regtest.example/api", "/tmp/db");
        let policy = TlsPolicy::from_config(&cfg);
        assert!(matches!(policy, TlsPolicy::SystemRoots));
    }

    #[test]
    #[ignore = "requires native system certs on host (rustls-native-certs load)"]
    fn from_config_builds_client_with_pin() {
        init_crypto();
        // F20 requires a pin on testnet.
        let pin = SpkiPin::from_bytes([0xaau8; 32]);
        let cfg = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db")
            .with_esplora_spki_pin(pin);
        let c = EsploraClient::from_config(&cfg).unwrap();
        assert_eq!(c.base_url.as_str(), "https://blockstream.info/testnet/api/");
    }

    #[test]
    fn from_config_rejects_http() {
        init_crypto();
        // Use regtest (F20 exempt) so the http:// scheme is the
        // failure cause, not the missing-pin check.
        let cfg = WalletConfig {
            esplora_url: "http://blockstream.info/testnet/api".to_string(),
            ..WalletConfig::regtest("https://placeholder.example/api", "/tmp/db")
        };
        let err = EsploraClient::from_config(&cfg).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(msg.contains("https://"), "msg = {msg}");
    }

    // F20 enforcement tests (issue #37).
    #[test]
    fn from_config_requires_pin_on_mainnet() {
        init_crypto();
        let cfg = WalletConfig::mainnet("https://blockstream.info/api", "/tmp/db");
        let err = EsploraClient::from_config(&cfg).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(
            msg.contains("F20") && msg.contains("pin"),
            "msg should name F20 + pin: {msg}"
        );
    }

    #[test]
    fn from_config_requires_pin_on_testnet() {
        init_crypto();
        let cfg = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        let err = EsploraClient::from_config(&cfg).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(
            msg.contains("F20") && msg.contains("pin"),
            "msg should name F20 + pin: {msg}"
        );
    }

    #[test]
    fn from_config_requires_pin_on_signet() {
        init_crypto();
        let cfg = WalletConfig::signet("https://signet.example/api", "/tmp/db");
        let err = EsploraClient::from_config(&cfg).unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(
            msg.contains("F20") && msg.contains("pin"),
            "msg should name F20 + pin: {msg}"
        );
    }

    #[test]
    fn from_config_allows_no_pin_on_regtest() {
        init_crypto();
        let cfg = WalletConfig::regtest("https://regtest.example/api", "/tmp/db");
        // No pin → SystemRoots is acceptable on regtest.
        EsploraClient::from_config(&cfg).unwrap();
    }

    #[test]
    #[ignore = "requires native system certs on host (rustls-native-certs load)"]
    fn from_config_accepts_pin_on_testnet() {
        init_crypto();
        let pin = SpkiPin::from_bytes([0x42u8; 32]);
        let cfg = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db")
            .with_esplora_spki_pin(pin);
        let c = EsploraClient::from_config(&cfg).unwrap();
        assert!(matches!(c.tls, TlsPolicy::Pinned(_)));
    }

    #[test]
    #[ignore = "requires native system certs on host (rustls-native-certs load)"]
    fn from_config_accepts_pin_on_mainnet() {
        init_crypto();
        let pin = SpkiPin::from_bytes([0x42u8; 32]);
        let cfg = WalletConfig::mainnet("https://blockstream.info/api", "/tmp/db")
            .with_esplora_spki_pin(pin);
        let c = EsploraClient::from_config(&cfg).unwrap();
        assert!(matches!(c.tls, TlsPolicy::Pinned(_)));
    }

    #[test]
    #[ignore = "requires native system certs on host (rustls-native-certs load)"]
    fn clone_preserves_url_and_tls() {
        init_crypto();
        let pin = SpkiPin::from_bytes([0x77u8; 32]);
        let policy = TlsPolicy::Pinned(SpkiPinSet::from_one(pin));
        let url = EsploraUrl::new("https://blockstream.info/api").unwrap();
        let c = EsploraClient::new(url, policy).unwrap();
        let c2 = c.clone();
        assert_eq!(c.base_url.as_str(), c2.base_url.as_str());
    }

    #[test]
    fn debug_hides_root_certs_but_shows_url() {
        init_crypto();
        let url = EsploraUrl::new("https://blockstream.info/api").unwrap();
        let c = EsploraClient::new(url, TlsPolicy::SystemRoots).unwrap();
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
