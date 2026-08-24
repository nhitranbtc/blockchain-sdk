//! Transport-layer primitives for Ethereum RPC endpoints.
//!
//! Currently houses the SPKI pin verifier used by
//! [`crate::provider::new_http_pinned`]. Future additions (e.g.
//! per-host policy, connection-pool tuning) live alongside.

pub mod verifier;
