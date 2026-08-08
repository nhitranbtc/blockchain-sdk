//! Wallet configuration types.
//!
//! [`WalletConfig`] holds the operator-facing settings: Bitcoin network,
//! Esplora endpoint, optional SPKI pin, and the SQLite sidecar DB path.
//!
//! **Threat-model coverage:**
//!
//! - F20 (SPKI pinning per U2) — defended by [`WalletConfig::esplora_spki_pin`]
//!   (typed `Option<SpkiPin>` that, when `Some`, is converted to
//!   `TlsPolicy::Pinned` by `EsploraClient::from_config`).
//! - F15 (sidecar DB pattern) — `db_path` is the path to the
//!   `bdk_file_store` SQLite database (NOT the `network.txt` file
//!   written by Task 17; that file is v0.1.1).
//!
//! **Drift from plan §Task 7** (L12 folded):
//!
//! | Plan said | This implementation | Why |
//! |---|---|---|
//! | `pub` all fields, no `#[non_exhaustive]` | `#[non_exhaustive]` + `pub` fields | TD-06: future fields (Electrum, descriptors) without breaking v0.1 consumers |
//! | `esplora_pinned_pubkey: Option<String>` | `esplora_spki_pin: Option<SpkiPin>` (typed) | TD-12: "pubkey" is wrong; "spki_pin" matches the operator's mental model (it's a hash, not a key) |
//! | `electrum_url` + `electrum_pinned_pubkey` fields | Dropped (no `ElectrumClient` in Task 7) | TD-14: avoid orphan fields; `Error::Electrum` and `bdk_electrum` dep reserved for v0.1.1 (F26) |
//! | 3 constructors (testnet/mainnet/regtest) | 4 constructors (+ `signet`) | Per design spec §4.1 (covers all 4 networks) |
//! | `Display` formats / `Debug` derives | Manual `Debug` to avoid leaking URL scheme in logs | L17 (BIP-137 lesson): `finish_non_exhaustive` for sensitive types; config is not sensitive |

use std::path::PathBuf;

use bitcoin::Network;
use serde::{Deserialize, Serialize};

use crate::chain::spki::SpkiPin;

/// Wallet configuration. Holds the operator-facing settings: Bitcoin
/// network, Esplora endpoint, optional SPKI pin, and the sidecar DB path.
///
/// `#[non_exhaustive]` (TD-06): v0.2 may add Electrum fields or
/// descriptor fields without breaking downstream `match` arms (the
/// `btc` CLI depends on this crate).
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Bitcoin network. `testnet` is the v0.1 default (CONTEXT.md hard rule #1).
    pub network: Network,
    /// Esplora endpoint URL. Must start with `https://` (EsploraClient::new).
    pub esplora_url: String,
    /// Optional SPKI pin for `EsploraClient`. When `Some`, the client
    /// verifies the leaf cert's SPKI hash against this pin (F20).
    #[serde(default)]
    pub esplora_spki_pin: Option<SpkiPin>,
    /// Path to the `bdk_file_store` SQLite database (the sidecar DB).
    pub db_path: PathBuf,
}

impl WalletConfig {
    /// Testnet configuration. **Default network for v0.1** (CONTEXT.md
    /// hard rule #1: never default to mainnet).
    pub fn testnet(esplora_url: impl Into<String>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            network: Network::Testnet,
            esplora_url: esplora_url.into(),
            esplora_spki_pin: None,
            db_path: db_path.into(),
        }
    }

    /// Mainnet configuration. Requires explicit `mainnet` choice (the
    /// caller must consciously reach for this constructor).
    pub fn mainnet(esplora_url: impl Into<String>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            network: Network::Bitcoin,
            ..Self::testnet(esplora_url, db_path)
        }
    }

    /// Regtest configuration. Local development only.
    pub fn regtest(esplora_url: impl Into<String>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            network: Network::Regtest,
            ..Self::testnet(esplora_url, db_path)
        }
    }

    /// Signet configuration. Per design spec §4.1 (covers all 4 networks).
    pub fn signet(esplora_url: impl Into<String>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            network: Network::Signet,
            ..Self::testnet(esplora_url, db_path)
        }
    }

    /// Builder-style setter for the SPKI pin. Returns the modified
    /// config (consuming `self` so calls chain naturally).
    #[must_use]
    pub fn with_esplora_spki_pin(mut self, pin: SpkiPin) -> Self {
        self.esplora_spki_pin = Some(pin);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testnet_default() {
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        assert_eq!(c.network, Network::Testnet);
        assert_eq!(c.esplora_url, "https://blockstream.info/testnet/api");
        assert_eq!(c.db_path, PathBuf::from("/tmp/db"));
    }

    #[test]
    fn mainnet_has_bitcoin_network() {
        let c = WalletConfig::mainnet("https://blockstream.info/api", "/tmp/db");
        assert_eq!(c.network, Network::Bitcoin);
    }

    #[test]
    fn regtest_has_regtest_network() {
        let c = WalletConfig::regtest("https://regtest.example/api", "/tmp/db");
        assert_eq!(c.network, Network::Regtest);
    }

    #[test]
    fn signet_has_signet_network() {
        let c = WalletConfig::signet("https://signet.example/api", "/tmp/db");
        assert_eq!(c.network, Network::Signet);
    }

    #[test]
    fn spki_pin_defaults_to_none() {
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        assert!(c.esplora_spki_pin.is_none());
    }

    #[test]
    fn with_esplora_spki_pin_sets_pin() {
        let pin = SpkiPin::from_bytes([0x42u8; 32]);
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db")
            .with_esplora_spki_pin(pin);
        assert_eq!(c.esplora_spki_pin, Some(pin));
    }

    #[test]
    fn serde_round_trip_with_pin() {
        let pin = SpkiPin::from_bytes([0xaau8; 32]);
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db")
            .with_esplora_spki_pin(pin);
        let json = serde_json::to_string(&c).unwrap();
        let parsed: WalletConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.network, Network::Testnet);
        assert_eq!(parsed.esplora_spki_pin, Some(pin));
    }

    #[test]
    fn serde_round_trip_without_pin() {
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        let json = serde_json::to_string(&c).unwrap();
        let parsed: WalletConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.network, Network::Testnet);
        assert!(parsed.esplora_spki_pin.is_none());
    }

    #[test]
    fn serde_rejects_invalid_pin() {
        // Operator pastes a 64-byte raw SPKI (not its hash) — should
        // fail at deserialization, not later.
        let json = r#"{
            "network": "testnet",
            "esplora_url": "https://blockstream.info/testnet/api",
            "esplora_spki_pin": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "db_path": "/tmp/db"
        }"#;
        let result: Result<WalletConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn config_is_clonable() {
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        let c2 = c.clone();
        assert_eq!(c.network, c2.network);
        assert_eq!(c.esplora_url, c2.esplora_url);
    }
}
