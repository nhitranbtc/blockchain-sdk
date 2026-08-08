//! Chain backends: SPKI pin primitives, Esplora client, (future) Electrum client.
//!
//! **Task 7 surface**:
//! - [`spki`] — `SpkiPin`, `SpkiPinSet` (F20 typed primitives)
//! - [`esplora`] — `EsploraClient`, `TlsPolicy` (F20 SPKI-pinned TLS)
//!
//! `chain::network` (per `coin_type_for(Network)`) lives in Task 8
//! (see [`super::chain::network`] when it lands).
//!
//! **Threat-model coverage:**
//!
//! - F20 (SPKI pubkey pinning per U2) — defended by every certificate
//!   chain validated through `EsploraVerifier` against the configured
//!   `SpkiPinSet`.
//! - A3 (network MITM) — defeated by F20 + CA chain validation.

pub mod esplora;
pub mod spki;
