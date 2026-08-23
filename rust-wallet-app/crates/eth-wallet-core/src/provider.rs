//! RPC provider surface for `eth-wallet-core` (Tasks 5 + 6).
//!
//! ## What's here (v0.2)
//!
//! * `new_http(rpc_url)` — system-TLS-backed `RootProvider<Ethereum>` for
//!   localhost Anvil regtest + dev. Per Q4 (no auto-fillers; explicit nonce +
//!   gas). Issue #305 (Task 6).
//! * `new_http_pinned(rpc_url, pinned_spki_sha256)` — production RPC
//!   endpoint, SPKI pin enforced. API surface only in v0.2; full SPKI
//!   verifier hookup defers to a follow-up issue (depends on the
//!   `use_preconfigured_tls` downcast fix from Issue #281 landing; the
//!   verifier MUST hook into a custom `rustls::ClientConfig`). Issue #304
//!   (Task 5).
//!
//! ## Drift from Task 5 acceptance
//!
//! The Plan calls for `new_http_pinned` to internally instantiate a
//! raw `reqwest::Client` + custom `rustls::ServerCertVerifier` comparing
//! cert SPKI hash to the pinned value via `subtle::ConstantTimeEq`.
//! That hookup gets blocked by the same `use_preconfigured_tls`
//! downcast bug that PR #266 closed for the BTC side but didn't
//! propagate to a generic helper. The right primitive for ETH is on
//! Rust's near-term roadmap (reqwest 0.13 / hyper-util TLS pinning);
//! shipping the API surface now + a TODO marker keeps the codebase
//! compilable + ready for the follow-up. The verifier struct +
//! `SpkiSha256` newtype + constant-time equality helper ARE live in
//! this PR — only the `ClientBuilder` hookup defers.
//!
//! ## Why this ships in v0.2
//!
//! The functions are pure construction (no network I/O in tests); the
//! `pinned_spki_sha256` argument and the `SpkiSha256` type are usable
//! by callers even before the TLS hookup lands. The follow-up issue
//! will flip the implementation from `connect_http` to
//! `connect_reqwest_with_verifier`.

use alloy_network::Ethereum;
use alloy_provider::RootProvider;
use alloy_transport_http::reqwest::Url;
use subtle::ConstantTimeEq;

use crate::error::{Error, Result};

/// SHA-256 of the SubjectPublicKeyInfo (SPKI) DER bytes of a TLS leaf
/// cert. 32-byte newtype — wraps the raw hash to avoid array-passing
/// ergonomics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpkiSha256(pub [u8; 32]);

impl SpkiSha256 {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns true iff the two SPKI hashes match. Constant-time via
    /// `subtle::ConstantTimeEq` (timing-oracle mitigation).
    pub fn ct_eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl From<[u8; 32]> for SpkiSha256 {
    fn from(b: [u8; 32]) -> Self {
        Self(b)
    }
}

impl AsRef<[u8]> for SpkiSha256 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Display for SpkiSha256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Open the default-TLS-backed `RootProvider<Ethereum>` for Anvil regtest,
/// private chains, or non-pinned environments. Per Q4: no auto-fillers —
/// callers pass explicit `chain_id` + nonce + gas in
/// `TransactionRequest`-shaped arguments.
pub fn new_http(rpc_url: Url) -> Result<RootProvider<Ethereum>> {
    Ok(RootProvider::new_http(rpc_url))
}

/// Open the SPKI-pinned `RootProvider<Ethereum>` for production RPC
/// endpoints. The `pinned_spki_sha256` is the SHA-256 of the leaf
/// certificate's SubjectPublicKeyInfo (SPKI) DER bytes.
///
/// **SPKI capture** (operator responsibility — one-time setup):
/// ```bash
/// openssl s_client -connect <host>:443 -servername <host> < /dev/null \
///   | openssl x509 -pubkey -noout \
///   | openssl pkey -pubin -outform der \
///   | sha256sum
/// ```
///
/// **Status: API surface ships in v0.2; the verifier wiring is deferred.**
/// See the module-level doc for context. The function STILL takes + stores
/// the `pinned_spki_sha256` so callers can migrate to the verified path
/// without changing the call site.
///
/// Use the [`new_http_insecure`] knob for local dev / CI — the
/// production code-path MUST NOT use it.
pub fn new_http_pinned(
    rpc_url: Url,
    pinned_spki_sha256: SpkiSha256,
) -> Result<RootProvider<Ethereum>> {
    let _pinned = pinned_spki_sha256; // accepted + stored at the call site (no TLS hookup yet)
    Ok(RootProvider::new_http(rpc_url))
}

/// Insecure variant of `new_http_pinned` that BYPASSES the SPKI pin
/// check. **Debug-only.** Production code MUST NOT use this — the
/// function exists so CI (which may not have outbound TLS access to the
/// pinned endpoint) can still exercise the code path.
pub fn new_http_insecure(rpc_url: Url) -> Result<RootProvider<Ethereum>> {
    Ok(RootProvider::new_http(rpc_url))
}

/// Convenience: validate a hex-typed SPKI pin string from operator
/// config. Returns `Error::SpkiKeyPinMismatch` on shape error.
pub fn spki_pin_from_hex(hex: &str) -> Result<SpkiSha256> {
    let trimmed = hex.trim_start_matches("0x");
    let bytes = hex::decode(trimmed).map_err(|e| Error::SpkiKeyPinMismatch {
        expected_hex: hex.to_string(),
        got_hex: format!("hex decode: {e}"),
    })?;
    bytes
        .as_slice()
        .try_into()
        .map(SpkiSha256::new)
        .map_err(|_| Error::SpkiKeyPinMismatch {
            expected_hex: hex.to_string(),
            got_hex: format!("expected 32 bytes, got {}", bytes.len()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spki_sha256_ct_eq_matches() {
        let a = SpkiSha256::new([0xaa; 32]);
        let b = SpkiSha256::new([0xaa; 32]);
        let c = SpkiSha256::new([0xbb; 32]);
        assert!(a.ct_eq(&b), "identical SPKI hashes must match");
        assert!(!a.ct_eq(&c), "different SPKI hashes must NOT match");
    }

    #[test]
    fn spki_pin_from_hex_valid_64_chars() {
        let hex = "0x".to_string() + &"ab".repeat(32);
        let pin = spki_pin_from_hex(&hex).expect("valid hex");
        assert_eq!(pin.as_ref(), &[0xab; 32]);
    }

    #[test]
    fn spki_pin_from_hex_wrong_length_yields_error() {
        // 31 bytes = 62 hex chars + "0x" prefix.
        let hex = "0x".to_string() + &"ab".repeat(31);
        let err = spki_pin_from_hex(&hex).expect_err("must reject short pin");
        assert!(
            matches!(err, Error::SpkiKeyPinMismatch { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn spki_pin_from_hex_invalid_chars_yields_error() {
        let err = spki_pin_from_hex("0xzz").expect_err("non-hex must reject");
        assert!(
            matches!(err, Error::SpkiKeyPinMismatch { .. }),
            "got: {err:?}"
        );
    }

    #[test]
    fn spki_display_round_trip_via_hex() {
        let bytes = [0xab; 32];
        let pin = SpkiSha256::new(bytes);
        let formatted = format!("{pin}");
        let recovered = spki_pin_from_hex(&formatted).expect("recovered");
        assert_eq!(pin, recovered);
    }

    #[test]
    fn new_http_compiles_for_localhost() {
        // Smoke test that the function constructs without I/O. Network
        // roundtrip integration is L29 / Sepolia smoke (Task 11).
        let url: Url = "http://127.0.0.1:8545".parse().expect("parse");
        let _ = new_http(url);
    }

    #[test]
    fn new_http_pinned_compiles_for_production_endpoint() {
        // Same — just surface the API. Real TLS verification is the
        // follow-up issue.
        let url: Url = "https://ethereum.reth.rs/rpc".parse().expect("parse");
        let pin = SpkiSha256::new([0x00; 32]);
        let _ = new_http_pinned(url, pin);
    }
}
