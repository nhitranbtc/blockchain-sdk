//! Phase 5 — `NetworkClient` PAL trait.
//!
//! Per plan §Phase 5 Task 4.1: `build_client / default_rpc_url`.
//! The core never reaches for `reqwest` directly — the platform impl
//! decides:
//!
//! - which TLS root store to load (webpki on mobile, OS roots on
//!   desktop, both behind `#[cfg]` flags per Round-1 grill Q6)
//! - whether to apply SPKI pinning (always optional; the impl just
//!   hands back a `Client` and the caller decides whether to use the
//!   SPKI-pinned path via `chain::SpkiPinnedVerifier`)
//! - what default TronGrid endpoint URL to suggest when the operator
//!   has not configured one
//!
//! **Why a trait, not a free `reqwest` import:** Phase 5 must compile
//! for `aarch64-apple-ios` + `aarch64-linux-android` per plan §Phase 5
//! Verification. FFI-bound mobile hosts don't ship the same
//! `rustls-native-certs` CAs as desktop — the platform impl chooses
//! `tls_built_in_root_certs(true)` on mobile and `tls_built_in_webpki_roots()`
//! on desktop, both via the same `reqwest::ClientBuilder` API surface.

use crate::error::Result;

/// HTTP client factory. Per plan §Phase 5 Task 4.1.
pub trait NetworkClient: Send + Sync {
    /// Build a `reqwest::Client` configured for the current platform.
    ///
    /// Implementations MUST:
    /// - configure TLS roots appropriate for the host (webpki on
    ///   mobile, OS roots on desktop per Round-1 grill Q6)
    /// - apply a 30-second default per-request timeout (test backends
    ///   override; production impls honor it)
    /// - NOT mutate per-call — the returned `Client` should be cheap
    ///   to clone and reusable across many requests
    fn build_client(&self) -> Result<reqwest::Client>;

    /// Operator hint when no `rpc_url` is configured. The PAL
    /// doesn't enforce one — the wallet binary lets the user pick
    /// `pinned://<pin>@<host>` or `https://<host>` per Phase 2 SPKI
    /// convention. This is just the conventional name to show in
    /// `tron config show` when empty.
    ///
    /// Per plan §Phase 5 Task 4.2 desktop impl: returns
    /// `"https://api.trongrid.io"`. Mobile / test impls may differ.
    fn default_rpc_url(&self) -> &'static str;
}
