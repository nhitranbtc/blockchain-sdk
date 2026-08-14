//! Chain backends: SPKI pin primitives, Esplora client, network selector.
//!
//! **Task 7 surface**:
//! - [`spki`] — `SpkiPin`, `SpkiPinSet` (F20 typed primitives)
//! - [`esplora`] — `EsploraClient`, `TlsPolicy` (F20 SPKI-pinned TLS)
//! - [`esplora_url`] — `EsploraUrl` (type-safe validated base URL; rejects
//!   http, userinfo, etc. at construction)
//!
//! **Task 8 surface**:
//! - [`network`] — `coin_type_for(Network) -> u32` (BIP-44 derivation path per F37; never returns 0 for non-mainnet)
//!
//! **Threat-model coverage:**
//!
//! - F20 (SPKI pubkey pinning per U2) — defended by every certificate
//!   chain validated through `EsploraVerifier` against the configured
//!   `SpkiPinSet`. URL-level defenses (https-only, no userinfo) live in
//!   `EsploraUrl::new` so the client constructor can't bypass them.
//! - A3 (network MITM) — defeated by F20 + CA chain validation.
//! - F37 (BIP-44 coin-type derivation path per network) — `coin_type_for` defeats caller-supplied-wrong-coin-type footgun.

pub mod esplora;
pub mod esplora_url;
pub mod explorer;
pub mod network;
pub mod spki;
