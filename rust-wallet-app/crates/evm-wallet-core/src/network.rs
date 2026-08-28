//! Network topology — chain-family + per-instance variants.
//!
//! Replaces the v0.2 ETH-only `Network` enum (which lived in `wallet.rs`).
//! Per Phase 0 of `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`,
//! Q1 Option A: the new top-level `Network` is two-level — a chain-family
//! (`Ethereum` | `Polygon`) wrapping per-chain-instance enums.
//! Future EVM L2s (Base, Arbitrum, Optimism) = add a new family variant +
//! instance enum. No core changes needed.

use serde::{Deserialize, Serialize};

/// Top-level chain family. Each variant wraps an instance-level enum so
/// per-chain methods (chain id, RPC URL, gas token) stay instance-specific.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "family", content = "instance", rename_all = "lowercase")]
pub enum Network {
    Ethereum(EthereumChain),
    Polygon(PolygonChain),
}

/// Ethereum chain instances (1, 11155111, 31337).
/// Replaces the v0.2 flat `Network` enum that lived in `wallet.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EthereumChain {
    Mainnet,
    Sepolia,
    Anvil,
}

/// Polygon PoS chain instances (137, 80002). Mumbai (80001) was deprecated
/// 2024-Q2; Phase 0.0 records the rejection per plan §0.0.a.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolygonChain {
    Mainnet,
    Amoy,
}

// -- Per-family accessors ---------------------------------------------------

impl Network {
    /// EIP-155 chain id for this chain. Used by RPC + replay-protection
    /// guards per Q7 of the polygon plan.
    pub fn chain_id(&self) -> u64 {
        match self {
            Network::Ethereum(c) => c.chain_id(),
            Network::Polygon(c) => c.chain_id(),
        }
    }

    /// Default RPC endpoint for this chain. Overridable via `--rpc-url` at
    /// the CLI layer (Phases 2 / 4).
    pub fn rpc_url(&self) -> &'static str {
        match self {
            Network::Ethereum(c) => c.rpc_url(),
            Network::Polygon(c) => c.rpc_url(),
        }
    }

    /// Display label for the native gas token. CLI surfaces `--unit pol`
    /// vs `--unit eth` based on this.
    pub fn gas_token_label(&self) -> &'static str {
        match self {
            Network::Ethereum(c) => c.gas_token_label(),
            Network::Polygon(c) => c.gas_token_label(),
        }
    }

    /// Legacy gas-token alias for wallets that pre-date a rebrand.
    /// Polygon MATIC → POL rebrand was 2024-09-04; ETH has no legacy alias.
    pub fn legacy_gas_token_label(&self) -> Option<&'static str> {
        match self {
            Network::Ethereum(_) => None,
            Network::Polygon(_) => Some("MATIC"),
        }
    }

    /// Filesystem directory name (matches existing `wallets/<dir>/` layout).
    pub fn as_dir_name(&self) -> &'static str {
        match self {
            Network::Ethereum(c) => c.as_dir_name(),
            Network::Polygon(c) => c.as_dir_name(),
        }
    }

    /// Parse a CLI `--network` flag at the family level. Accepts:
    ///   * "mainnet" / "sepolia" / "anvil" / "1" / "11155111" / "31337" / "dev" → Ethereum
    ///   * "polygon" / "polygon-mainnet" / "polygon-amoy" / "137" / "80002" → Polygon
    ///
    /// The v0.2 `Network::parse_cli("polygon")` returned `Err`. After Phase 0
    /// family-level parsing accepts the family. Per-family narrowing for the
    /// ETH-only CLI lives on `EthereumChain::parse_cli`.
    pub fn parse_cli(s: &str) -> crate::Result<Self> {
        use crate::Error;
        match s.to_ascii_lowercase().as_str() {
            // ETH (family-level aliases)
            "mainnet" | "1" => Ok(Network::Ethereum(EthereumChain::Mainnet)),
            "sepolia" | "11155111" => Ok(Network::Ethereum(EthereumChain::Sepolia)),
            "anvil" | "31337" | "dev" | "local" => Ok(Network::Ethereum(EthereumChain::Anvil)),

            // Polygon (Phase 4 polygon CLI territory)
            "polygon" | "polygon-mainnet" | "137" | "matic" => {
                Ok(Network::Polygon(PolygonChain::Mainnet))
            }
            "polygon-amoy" | "amoy" | "80002" | "polygon-testnet" => {
                Ok(Network::Polygon(PolygonChain::Amoy))
            }
            // Explicit rejection of deprecated Mumbai per Phase 0.0.a.
            "mumbai" | "80001" => Err(Error::InvalidInput(format!(
                "unknown network '{s}' — Mumbai (80001) was deprecated 2024-Q2; \
                 use polygon-amoy (80002) for testnet"
            ))),

            other => Err(Error::InvalidInput(format!(
                "unknown network '{other}' — expected mainnet|sepolia|anvil|polygon|polygon-amoy"
            ))),
        }
    }

    /// Inverse of `chain_id()` — resolve numeric EIP-155 chain id back to a
    /// `Network`. Single source of truth for the network table; replaces
    /// the v0.2 `Network::from_chain_id`.
    pub fn from_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            1 => Some(Network::Ethereum(EthereumChain::Mainnet)),
            11_155_111 => Some(Network::Ethereum(EthereumChain::Sepolia)),
            31_337 => Some(Network::Ethereum(EthereumChain::Anvil)),
            137 => Some(Network::Polygon(PolygonChain::Mainnet)),
            80002 => Some(Network::Polygon(PolygonChain::Amoy)),
            _ => None,
        }
    }

    /// Default for v0.2 backward-compat callers (Sepolia testnet).
    /// The v0.2 `Network::default_v0_2()` was a placeholder Phase 0 commits
    /// to land; equivalent here + at every CLI entry point.
    pub fn default_v0_2() -> Self {
        Network::Ethereum(EthereumChain::Sepolia)
    }

    /// ETH-only CLI parser. Used by `eth` CLI which must reject Polygon
    /// inputs (`polygon`, `137`, `80002`, etc) without a CLI flag flip.
    /// Equivalent to `EthereumChain::parse_cli(s).map(Network::Ethereum)`.
    pub fn parse_cli_eth(s: &str) -> crate::Result<Self> {
        Ok(Network::Ethereum(EthereumChain::parse_cli(s)?))
    }
}

// -- Per-instance accessors --------------------------------------------------

impl EthereumChain {
    pub fn chain_id(&self) -> u64 {
        match self {
            EthereumChain::Mainnet => 1,
            EthereumChain::Sepolia => 11_155_111,
            EthereumChain::Anvil => 31_337,
        }
    }

    pub fn rpc_url(&self) -> &'static str {
        match self {
            EthereumChain::Mainnet => "https://cloudflare-eth.com",
            EthereumChain::Sepolia => "https://ethereum-sepolia-rpc.publicnode.com",
            EthereumChain::Anvil => "http://127.0.0.1:8545",
        }
    }

    pub fn gas_token_label(&self) -> &'static str {
        match self {
            EthereumChain::Mainnet => "ETH",
            EthereumChain::Sepolia => "ETH",
            EthereumChain::Anvil => "ETH",
        }
    }

    pub fn as_dir_name(&self) -> &'static str {
        match self {
            EthereumChain::Mainnet => "mainnet",
            EthereumChain::Sepolia => "sepolia",
            EthereumChain::Anvil => "anvil",
        }
    }

    /// ETH-only parser. Used by `eth` CLI which rejects "polygon".
    /// The Phase 0 unit test `assert!(Network::parse_cli("polygon").is_err())`
    /// migrates here.
    pub fn parse_cli(s: &str) -> crate::Result<Self> {
        use crate::Error;
        match s.to_ascii_lowercase().as_str() {
            "mainnet" | "1" => Ok(EthereumChain::Mainnet),
            "sepolia" | "11155111" => Ok(EthereumChain::Sepolia),
            "anvil" | "31337" | "dev" | "local" => Ok(EthereumChain::Anvil),
            other => Err(Error::InvalidInput(format!(
                "unknown ethereum network '{other}' — expected mainnet|sepolia|anvil"
            ))),
        }
    }
}

impl PolygonChain {
    pub fn chain_id(&self) -> u64 {
        match self {
            PolygonChain::Mainnet => 137,
            PolygonChain::Amoy => 80002,
        }
    }

    pub fn rpc_url(&self) -> &'static str {
        match self {
            PolygonChain::Mainnet => "https://polygon-rpc.com",
            PolygonChain::Amoy => "https://polygon-amoy.drpc.org",
        }
    }

    pub fn gas_token_label(&self) -> &'static str {
        "POL"
    }

    pub fn as_dir_name(&self) -> &'static str {
        match self {
            PolygonChain::Mainnet => "polygon_mainnet",
            PolygonChain::Amoy => "polygon_amoy",
        }
    }

    /// Inverse of `chain_id()`: returns `Some(Self)` iff `chain_id`
    /// corresponds to a `PolygonChain` variant. Single source of truth —
    /// adding a new variant (e.g. `PolygonChain::ZkEvm` for v0.2 per
    /// design doc §9 backlog) extends this match arm and the new
    /// chain becomes accepted automatically. The compiler enforces
    /// exhaustiveness when the enum grows.
    pub fn from_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            137 => Some(PolygonChain::Mainnet),
            80002 => Some(PolygonChain::Amoy),
            _ => None,
        }
    }

    /// Q7 + C1 enforcement: `true` iff `chain_id` is a `PolygonChain`
    /// variant. Single chokepoint — `sign_typed_data` + future EIP-712
    /// paths (Permit2, route handlers) call this before signing.
    pub fn is_polygon_chain_id(chain_id: u64) -> bool {
        Self::from_chain_id(chain_id).is_some()
    }

    /// Polygon-only parser. Used by Phase 4 `polygon` CLI.
    pub fn parse_cli(s: &str) -> crate::Result<Self> {
        use crate::Error;
        match s.to_ascii_lowercase().as_str() {
            "mainnet" | "polygon-mainnet" | "137" | "matic" => Ok(PolygonChain::Mainnet),
            "amoy" | "polygon-amoy" | "polygon-testnet" | "80002" => Ok(PolygonChain::Amoy),
            other => Err(Error::InvalidInput(format!(
                "unknown polygon network '{other}' — expected mainnet|amoy"
            ))),
        }
    }
}

// -- Result alias ------------------------------------------------------------

pub type Result<T> = std::result::Result<T, crate::error::Error>;
