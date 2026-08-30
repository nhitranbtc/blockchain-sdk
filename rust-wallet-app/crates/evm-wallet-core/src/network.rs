//! Network topology — chain-family + per-instance variants.
//!
//! Replaces the v0.2 ETH-only `Network` enum (which lived in `wallet.rs`).
//! Per Phase 0 of `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`,
//! Q1 Option A: the new top-level `Network` is two-level — a chain-family
//! (`Ethereum` | `Polygon`) wrapping per-chain-instance enums.
//! Future EVM L2s (Base, Arbitrum, Optimism) = add a new family variant +
//! instance enum. No core changes needed.

use serde::{Deserialize, Serialize};

/// Closed family enum backing `Network::parse_cli`'s two-layer dispatch
/// (Issue #472). Adding a new `Network` family variant (e.g.
/// `Network::Arbitrum(ArbitrumChain)` in v0.2) requires adding a `Family`
/// variant here AND an arm in the outer `match` in `parse_cli` — compile
/// error enforces exhaustiveness at the CLI boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Ethereum,
    Polygon,
}

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

    /// Every `Network` variant the wallet stack currently supports.
    /// Single source of truth — built by per-family iteration: each chain
    /// enum exposes its own `all()` and `Network::all()` concatenates.
    /// Adding a new `Network` variant (e.g. `Network::Arbitrum(ArbitrumChain)`
    /// in v0.2) requires extending the per-family loop below → compile error
    /// here AND at every callsite that iterates `all()`
    /// (`WalletManager::list_wallets`, `polygon` CLI sign dispatch).
    /// Sister pattern in `from_chain_id` + `parse_cli`: both dispatch
    /// per-family with no catch-all arm, so a new variant forces a compile
    /// error there too.
    pub fn all() -> Vec<Network> {
        let mut out = Vec::with_capacity(EthereumChain::all().len() + PolygonChain::all().len());
        for c in EthereumChain::all() {
            out.push(Network::Ethereum(c));
        }
        for c in PolygonChain::all() {
            out.push(Network::Polygon(c));
        }
        out
    }

    /// Parse a CLI `--network` flag at the family level. Accepts:
    ///   * "mainnet" / "sepolia" / "anvil" / "1" / "11155111" / "31337" / "dev" → Ethereum
    ///   * "polygon" / "polygon-mainnet" / "polygon-amoy" / "137" / "80002" → Polygon
    ///
    /// Two-layer per-family dispatch (Issue #472 / #461.1):
    ///   * Layer 1 — `classify_family` partitions input by target family.
    ///     Unknown inputs return `None` (catch-all lives at the boundary,
    ///     not in a `match` over variants).
    ///   * Layer 2 — outer `match` over the closed `Family` set is
    ///     exhaustive. New `Network::Arbitrum(ArbitrumChain)` requires
    ///     adding a `Family::Arbitrum` arm → compile error here + in
    ///     `classify_family`. Inner per-family parsers are exhaustive
    ///     over their own instance enums.
    pub fn parse_cli(s: &str) -> crate::Result<Self> {
        use crate::Error;
        // Two-layer dispatch: classify returns the family (None = unknown),
        // then the per-family parser handles instance matching (Err on
        // unknown instance, e.g. mumbai/80001). Outer match over the
        // closed `Family` set is exhaustive; new family → compile error.
        match Self::classify_family(s) {
            Some(Family::Ethereum) => EthereumChain::parse_cli(s).map(Network::Ethereum),
            Some(Family::Polygon) => PolygonChain::parse_cli(s).map(Network::Polygon),
            None => Err(Error::InvalidInput(format!(
                "unknown network '{s}' — expected mainnet|sepolia|anvil|polygon|polygon-amoy"
            ))),
        }
    }

    /// Inverse of `chain_id()` — resolve numeric EIP-155 chain id back to a
    /// `Network`. Built by iterating `all()` so a new variant added to
    /// `Network` (or to either chain enum) is resolved automatically with
    /// no `_ => None` catch-all to forget. The per-family `chain_id()`
    /// method remains the single source of truth for numeric IDs.
    pub fn from_chain_id(chain_id: u64) -> Option<Self> {
        Self::all().into_iter().find(|n| n.chain_id() == chain_id)
    }

    /// Layer-1 family classification. Closed family set: extending
    /// `Network` with a new family (e.g. `Arbitrum`) requires adding a
    /// `Family` variant + an arm in the outer `match` in `parse_cli` →
    /// compile error enforces exhaustiveness at the CLI boundary. The
    /// `_ => None` arm here is acceptable: this function answers
    /// "which family does this string look like", not "is this string
    /// a valid Network". Per-family parsers reject unknown instances
    /// (e.g. mumbai/80001) so the caller still surfaces a useful error.
    fn classify_family(s: &str) -> Option<Family> {
        match s.to_ascii_lowercase().as_str() {
            // ETH aliases
            "mainnet" | "1" | "sepolia" | "11155111" | "anvil" | "31337" | "dev" | "local" => {
                Some(Family::Ethereum)
            }
            // Polygon aliases (including deprecated Mumbai — per-family parser rejects)
            "polygon" | "polygon-mainnet" | "137" | "matic" | "polygon-amoy" | "amoy" | "80002"
            | "polygon-testnet" | "mumbai" | "80001" => Some(Family::Polygon),
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
            EthereumChain::Mainnet => "https://ethereum-rpc.publicnode.com",
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

    /// Every `EthereumChain` variant. Per-family single source of truth
    /// for `Network::all()`. The `match` inside `chain_id` / `rpc_url` /
    /// `gas_token_label` / `as_dir_name` already enforces exhaustiveness
    /// over this enum when used. Adding a 4th variant forces compile
    /// errors at every per-instance accessor AND at `Network::all()` (via
    /// the `Vec::with_capacity` length hint and the per-family loop).
    pub const fn all() -> [EthereumChain; 3] {
        [
            EthereumChain::Mainnet,
            EthereumChain::Sepolia,
            EthereumChain::Anvil,
        ]
    }
}

impl PolygonChain {
    /// Every `PolygonChain` variant. Per-family single source of truth.
    /// Sister to `EthereumChain::all()` — see that docstring for the
    /// exhaustiveness invariant.
    pub const fn all() -> [PolygonChain; 2] {
        [PolygonChain::Mainnet, PolygonChain::Amoy]
    }

    pub fn chain_id(&self) -> u64 {
        match self {
            PolygonChain::Mainnet => 137,
            PolygonChain::Amoy => 80002,
        }
    }

    pub fn rpc_url(&self) -> &'static str {
        match self {
            PolygonChain::Mainnet => "https://polygon-bor-rpc.publicnode.com",
            PolygonChain::Amoy => "https://polygon-amoy-bor-rpc.publicnode.com",
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
            "mainnet" | "polygon" | "polygon-mainnet" | "137" | "matic" => {
                Ok(PolygonChain::Mainnet)
            }
            "amoy" | "polygon-amoy" | "polygon-testnet" | "80002" => Ok(PolygonChain::Amoy),
            other => Err(Error::InvalidInput(format!(
                "unknown polygon network '{other}' — expected mainnet|amoy"
            ))),
        }
    }
}

// -- Result alias ------------------------------------------------------------

pub type Result<T> = std::result::Result<T, crate::error::Error>;
