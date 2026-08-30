//! RPC provider surface for `eth-wallet-core`.
//!
//! ## What's here (v0.2)
//!
//! * `new_http(rpc_url)` — system-TLS-backed `RootProvider<Ethereum>` for
//!   localhost Anvil regtest + dev. Per Q4 (no auto-fillers; explicit nonce +
//!   gas). Issue #305 (Task 6).
//! * `new_http_polygon_mainnet()` — convenience constructor against
//!   `https://polygon-bor-rpc.publicnode.com` (Q4). Returns a `RootProvider<Ethereum>`
//!   ready for `eth_chainId` / `estimate_eip1559_fees` calls. Issue
//!   #424 (Phase 2 / Task 3 of #416).
//! * `new_http_polygon_amoy()` — convenience constructor against
//!   `https://polygon-amoy-bor-rpc.publicnode.com` (Q4 testnet). Issue #424.
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

/// Convenience constructor: open a `RootProvider<Ethereum>` against the
/// public Polygon mainnet RPC (`https://polygon-bor-rpc.publicnode.com`, EIP-155
/// chain_id 137). Issue #424 / Task 3 of #416. Drift from original
/// `polygon-rpc.com` default per Issue #474 (2025-Q3 keyless-tier tightening).
///
/// Thin wrapper over `new_http` — no SPKI pin is applied (see module
/// docs for why the ETH-side verifier was removed in F20 M-2). Relies
/// on rustls default system CAs.
pub fn new_http_polygon_mainnet() -> Result<RootProvider<Ethereum>> {
    let url: Url = "https://polygon-bor-rpc.publicnode.com"
        .parse()
        .expect("polygon mainnet RPC URL is a known-valid literal");
    new_http(url)
}

/// Convenience constructor: open a `RootProvider<Ethereum>` against the
/// public Polygon Amoy testnet RPC (`https://polygon-amoy-bor-rpc.publicnode.com`,
/// EIP-155 chain_id 80_002). Issue #424 / Task 3 of #416. Drift from
/// original `polygon-amoy.drpc.org` default per Issue #474.
///
/// Thin wrapper over `new_http` — no SPKI pin is applied (see module
/// docs for why the ETH-side verifier was removed in F20 M-2). Relies
/// on rustls default system CAs.
pub fn new_http_polygon_amoy() -> Result<RootProvider<Ethereum>> {
    let url: Url = "https://polygon-amoy-bor-rpc.publicnode.com"
        .parse()
        .expect("polygon amoy RPC URL is a known-valid literal");
    new_http(url)
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
