//! TronGrid HTTP client + SPKI pin verifier.
//!
//! The split between this module and `crate::tx`:
//!
//! - `crate::tx` builds, signs, and serialises. It never opens a socket.
//! - `crate::chain` (this module) owns the HTTP client, the wire envelope it
//!   sends, and the TLS pinning policy under which it operates.
//!
//! Pure Rust core does not depend on anything here; this is the only place
//! `reqwest`/`rustls` show up in the crate.

pub mod client;
pub mod spki;

pub use client::TronGridClient;
pub use spki::{SpkiPin, SpkiPinnedVerifier};
