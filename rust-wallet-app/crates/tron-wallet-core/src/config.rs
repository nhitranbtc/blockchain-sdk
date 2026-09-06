//! Configuration root for `tron-wallet-core`.
//!
//! `TronConfig` is the single value the rest of the crate reads at every
//! request boundary. The defaults that ship with v0.1 — RPC URL per network,
//! SPKI pin per network — are recorded here, not in the CLI, so any
//! embedder (CLI, FFI host, integration test) gets the same defaults.

use crate::chain::spki::SpkiPin;
use crate::error::{Error, Result};

/// The canonical mainnet SPKI pin (per the plan's Q5 live extraction).
pub const MAINNET_SPKI_PIN_HEX: &str =
    "0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d";

/// Decode [`MAINNET_SPKI_PIN_HEX`] to the runtime type on demand. Lives
/// in `crate::config` so embedders that import the whole `config` surface
/// only need one path. `disambig` re-exports the same function for callers
/// that anchored there.
pub fn mainnet_spki_pin() -> SpkiPin {
    let bytes = hex::decode(MAINNET_SPKI_PIN_HEX)
        .expect("MAINNET_SPKI_PIN_HEX must be a valid 32-byte hex string");
    SpkiPin::from_bytes(
        bytes
            .try_into()
            .expect("MAINNET_SPKI_PIN_HEX must decode to exactly 32 bytes"),
    )
}

/// The TRON network this crate talks to. The numeric chain-id is what
/// `POST /jsonrpc {"method":"eth_chainId"}` returns (the plan's discovery of
/// the JSON-RPC path; TronGrid's `/wallet/getchainid` returns HTTP 405).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// TRON mainnet. Chain-id 0x2c mainnet (decimal 728126428) on TronGrid's
    /// Ethereum-shaped JSON-RPC endpoint.
    Mainnet,
    /// Shasta public testnet (legacy; not in v0.1 release-train smoke).
    Shasta,
    /// Nile community testnet (the v0.1 test target).
    Nile,
    /// A user-supplied local node, e.g. a TronBox Docker container.
    /// Wraps a custom RPC URL; chain-id is what the endpoint reports.
    Local,
}

impl Network {
    /// Lower-case network tag for SPKI pin lookup keys.
    pub fn tag(&self) -> &'static str {
        match self {
            Network::Mainnet => "mainnet",
            Network::Shasta => "shasta",
            Network::Nile => "nile",
            Network::Local => "local",
        }
    }
}

/// Default RPC URL for a given network.
pub fn default_rpc_url(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "https://api.trongrid.io",
        Network::Shasta => "https://api.shasta.trongrid.io",
        Network::Nile => "https://nile.trongrid.io",
        // Local TronBox default; overridable via `--rpc <url>` (Phase 6 CLI).
        Network::Local => "http://127.0.0.1:8090",
    }
}

/// SHA-256 SPKI pin for `api.trongrid.io`'s leaf cert, verified against
/// the live cert on 2026-09-05 (plan Q5).
///
/// Recorded here so any embedder that enables SPKI pinning gets the same
/// constant. `None` is a deliberate choice — operators running against a
/// non-TronGrid endpoint (TronBox, a private cluster) should not have a
/// mainnet pin accidentally pinned to it.
pub fn default_spki_pin(network: Network) -> Option<SpkiPin> {
    match network {
        Network::Mainnet => Some(SpkiPin::from_bytes([
            0x0e, 0x43, 0xf6, 0x11, 0x0b, 0xbe, 0xe5, 0xe1, 0x99, 0xc6, 0x77, 0x5c, 0xf8, 0x8a,
            0x30, 0x50, 0xa9, 0xbd, 0x51, 0xf3, 0xbb, 0x4a, 0x31, 0xae, 0xef, 0xb7, 0x12, 0x2f,
            0x79, 0x11, 0x9f, 0x0d,
        ])),
        Network::Nile | Network::Shasta | Network::Local => None,
    }
}

/// Crate-wide configuration. One value, one build.
#[derive(Debug, Clone)]
pub struct TronConfig {
    /// Which network the [`crate::chain::TronGridClient`] talks to.
    pub network: Network,
    /// RPC base URL (no trailing slash; pass to `TronGridClient::new`).
    pub rpc_url: String,
    /// SPKI pin override. `None` ⇒ use the per-network default (or no pin
    /// in the case where the network has no default).
    pub spki_pin: Option<SpkiPin>,
    /// Default smart-contract energy ceiling in SUN. Sized for USDT-TRC20
    /// at 100 TRX (`100_000_000`). Callers may override per-transaction.
    pub fee_limit_sun: i64,
    /// Optional filesystem data directory (Phase 5 PAL picks this up;
    /// here only as a typed holder so configs can serialise later).
    pub data_dir: Option<std::path::PathBuf>,
}

impl TronConfig {
    /// `TronConfig` for a network, with defaults filled in.
    pub fn for_network(network: Network) -> Self {
        Self {
            network,
            rpc_url: default_rpc_url(network).to_owned(),
            spki_pin: default_spki_pin(network),
            fee_limit_sun: 100_000_000,
            data_dir: None,
        }
    }

    /// Override the RPC URL.
    pub fn with_rpc_url(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = url.into();
        self
    }

    /// Set the SPKI pin explicitly. Builders take `Option`-friendly
    /// semantics: pass `Some` to pin, `None` to clear.
    pub fn with_spki_pin(mut self, pin: Option<SpkiPin>) -> Self {
        self.spki_pin = pin;
        self
    }

    /// Validate that required fields are present (RPC URL non-empty, etc.).
    /// Cheap; called at startup and before network calls.
    pub fn validate(&self) -> Result<()> {
        if self.rpc_url.trim().is_empty() {
            return Err(Error::Config("rpc_url is empty".into()));
        }
        Ok(())
    }
}
