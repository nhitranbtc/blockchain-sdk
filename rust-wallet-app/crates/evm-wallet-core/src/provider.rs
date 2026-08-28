//! RPC provider surface for `eth-wallet-core`.
//!
//! ## What's here (v0.2)
//!
//! * `new_http(rpc_url)` — system-TLS-backed `RootProvider<Ethereum>` for
//!   localhost Anvil regtest + dev. Per Q4 (no auto-fillers; explicit nonce +
//!   gas). Issue #305 (Task 6).
//!
//! ## Task 5 (Issue #304) status: REMOVED
//!
//! The SPKI pin verifier + `new_http_pinned` API were removed entirely.
//! The verifier (commit `36ff115`) was unsafe-by-design — its TLS
//! signature verify overrides returned `Ok` unconditionally, pending
//! composition with a `webpki` chain validator. Without that composition,
//! a MITM presenting a self-signed cert whose SPKI happened to match a
//! pinned value would pass signature verification (M-2 in the F20 review).
//!
//! Production RPC traffic uses `new_http` (default rustls TLS + system
//! CAs) for all endpoints. The BTC side (`bitcoin-wallet-core`) retains
//! its `SpkiPinSet` + `EsploraClient::from_config` chain — those wire
//! through `webpki`-backed composition and are not affected.
//!
//! To reintroduce ETH-side SPKI pinning, the verifier must:
//! 1. Compose with `rustls::client::WebPkiServerVerifier` for chain +
//!    hostname + expiry validation (delegate from `verify_server_cert`).
//! 2. Compose with `webpki` verifier for signature verification
//!    (delegate from `verify_tls12_signature` / `verify_tls13_signature`).
//! 3. Use `x509-parser` (not the length-window heuristic) for RSA-2048
//!    SPKI extraction.
//!
//! Until all three are in place, shipping the verifier is a security
//! regression.

use alloy_network::Ethereum;
use alloy_provider::RootProvider;
use alloy_transport_http::reqwest::Url;

use crate::error::Result;

/// Open the default-TLS-backed `RootProvider<Ethereum>` for Anvil regtest,
/// private chains, or non-pinned environments. Per Q4: no auto-fillers —
/// callers pass explicit `chain_id` + nonce + gas in
/// `TransactionRequest`-shaped arguments.
pub fn new_http(rpc_url: Url) -> Result<RootProvider<Ethereum>> {
    Ok(RootProvider::new_http(rpc_url))
}

/// Insecure variant of `new_http` that BYPASSES TLS verification.
/// **Debug-only.** Production code MUST NOT be able to reference this —
/// gated behind `debug_assertions` so release builds can't link against
/// it. CI + dev use this for fast iteration against local Anvil.
#[cfg(any(debug_assertions, feature = "insecure_tls"))]
pub fn new_http_insecure(rpc_url: Url) -> Result<RootProvider<Ethereum>> {
    Ok(RootProvider::new_http(rpc_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_http_compiles_for_localhost() {
        // Smoke test that the function constructs without I/O. Network
        // roundtrip integration is L29 / Sepolia smoke (Task 11).
        let url: Url = "http://127.0.0.1:8545".parse().expect("parse");
        let _ = new_http(url);
    }
}
