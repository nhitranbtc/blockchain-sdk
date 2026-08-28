//! SPKI pin parser (Q4 RPC TLS pinning — reuses bitcoin-wallet-core primitives).
//!
//! Per F20 + the TRON spike's #408 ship-gate precedent, the pin is parsed and
//! recorded on the JsonRpcClient but the `post_*` helpers currently use Rustls
//! default verification. Wiring the SPKI verifier into a custom reqwest
//! `ClientBuilder` is ship-gate follow-up work — the spike's live path
//! currently trusts the system trust store plus the URL-pinned endpoint identity
//! (Polygon mainnet + Amoy are well-known hosts).

use bitcoin_wallet_core::chain::spki::{SpkiPin, SpkiPinSet};

/// Placeholder for the SPKI pin-set loader — implemented in Phase 2.
pub fn _spki_set_placeholder() -> SpkiPinSet {
    // SpkiPinSet::new expects pins; empty list is a valid construction (no pins
    // means "trust system store"). Phase 2 wires the real Amoy + Polygon mainnet
    // SPKI pins via `from_one`/`new`.
    SpkiPinSet::new(Vec::<SpkiPin>::new()).expect("empty SpkiPinSet construction is infallible")
}
