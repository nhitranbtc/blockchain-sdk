//! Chain backends: SPKI pin primitives, Esplora client, network selector.
//!
//! **Task 7 surface**:
//! - [`spki`] — `SpkiPin`, `SpkiPinSet` (F20 typed primitives)
//! - [`esplora`] — `EsploraClient`, `TlsPolicy` (F20 SPKI-pinned TLS)
//!
//! **Task 8 surface**:
//! - [`network`] — `coin_type_for(Network) -> u32` (BIP-44 derivation path per F37; never returns 0 for non-mainnet)
//!
//! **Threat-model coverage:**
//!
//! - F20 (SPKI pubkey pinning per U2) — defended by every certificate
//!   chain validated through `EsploraVerifier` against the configured
//!   `SpkiPinSet`.
//! - A3 (network MITM) — defeated by F20 + CA chain validation.
//! - F37 (BIP-44 coin-type derivation path per network) — `coin_type_for` defeats caller-supplied-wrong-coin-type footgun.

pub mod esplora;
pub mod network;
pub mod spki;
