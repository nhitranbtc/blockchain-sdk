//! Configuration root for `tron-wallet-core`.
//!
//! `TronConfig` is the single value the rest of the crate reads at every
//! request boundary. The defaults that ship with v0.1 — RPC URL per
//! network — are recorded in `tokens/network.json`, not in source.
//! Editing that file changes every embedder (CLI, FFI host,
//! integration test) at once, with no Rust recompile. The JSON is
//! loaded once via [`std::sync::OnceLock`] and the parsed strings are
//! `Box::leak`-d into a `&'static` map so the public API keeps its
//! `&'static str` shape.
//!
//! ## Why config in `tokens/`
//!
//! The plan's bundled token list (`tokens/{local,nile,mainnet}.json`)
//! already establishes `tokens/` as the operator-editable
//! configuration directory. Adding `tokens/network.json` (RPC URL
//! per network) + `tokens/test-vectors.json` (test-only addresses
//! that round-trip every test) keeps every operator-tunable string
//! in one tree, and keeps the Rust source free of hardcoded
//! endpoints or addresses.
//!
//! ## Operator flow
//!
//! ```text
//! # Production: rotate a mainnet RPC endpoint
//! $EDITOR tokens/network.json          # update "mainnet.rpc_url"
//! cargo build
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// One row of `tokens/network.json`.
#[derive(Debug, Clone, Deserialize)]
struct NetworkEntry {
    rpc_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct NetworkTable {
    mainnet: NetworkEntry,
    shasta: NetworkEntry,
    nile: NetworkEntry,
    local: NetworkEntry,
}

/// Decoded `tokens/network.json`, lazily loaded on first call and
/// `Box::leak`-d into a `&'static` so the public `&'static str`
/// return types keep working without an owned-string refactor.
static NETWORK_TABLE: OnceLock<HashMap<Network, NetworkEntry>> = OnceLock::new();

fn network_table() -> &'static HashMap<Network, NetworkEntry> {
    NETWORK_TABLE.get_or_init(|| {
        let raw = include_str!("../tokens/network.json");
        let parsed: NetworkTable = match serde_json::from_str(raw) {
            Ok(t) => t,
            Err(e) => panic!(
                "bundled tokens/network.json failed to parse: {e}. \
                 This file is the canonical source for RPC URLs — \
                 fix the JSON, not the loader."
            ),
        };
        let mut map = HashMap::with_capacity(4);
        map.insert(Network::Mainnet, parsed.mainnet);
        map.insert(Network::Shasta, parsed.shasta);
        map.insert(Network::Nile, parsed.nile);
        map.insert(Network::Local, parsed.local);
        map
    })
}

/// The TRON network this crate talks to. The numeric chain-id is what
/// `POST /jsonrpc {"method":"eth_chainId"}` returns (the plan's discovery of
/// the JSON-RPC path; TronGrid's `/wallet/getchainid` returns HTTP 405).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum Network {
    /// TRON mainnet. Chain-id 0x2c mainnet (decimal 728126428) on TronGrid's
    /// Ethereum-shaped JSON-RPC endpoint.
    Mainnet,
    /// Shasta public testnet (legacy; not in v0.1 release-train smoke).
    Shasta,
    /// Nile community testnet (the v0.1 test target). The `Default` — a fresh
    /// `tron` invocation without an explicit `--network` should never
    /// accidentally land on mainnet.
    #[default]
    Nile,
    /// A user-supplied local node, e.g. a TronBox Docker container.
    /// Wraps a custom RPC URL; chain-id is what the endpoint reports.
    Local,
}

impl Network {
    /// Lower-case network tag.
    pub fn tag(&self) -> &'static str {
        match self {
            Network::Mainnet => "mainnet",
            Network::Shasta => "shasta",
            Network::Nile => "nile",
            Network::Local => "local",
        }
    }
}

/// Default RPC URL for a given network. Read from
/// `tokens/network.json`; the value is `Box::leak`-d so the
/// `&'static str` return type survives.
pub fn default_rpc_url(network: Network) -> &'static str {
    let entry = network_table()
        .get(&network)
        .expect("tokens/network.json must contain a row for every Network variant");
    Box::leak(entry.rpc_url.clone().into_boxed_str())
}

/// Crate-wide configuration. One value, one build.
#[derive(Debug, Clone)]
pub struct TronConfig {
    /// Which network the [`crate::chain::TronGridClient`] talks to.
    pub network: Network,
    /// RPC base URL (no trailing slash; pass to `TronGridClient::new`).
    pub rpc_url: String,
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
            fee_limit_sun: 100_000_000,
            data_dir: None,
        }
    }

    /// Override the RPC URL.
    pub fn with_rpc_url(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = url.into();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The JSON must contain every Network variant. A typo or a stale
    /// file (one row deleted) is a silent default change — fail loudly
    /// at first call rather than at the first network call.
    #[test]
    fn network_table_contains_every_variant() {
        let t = network_table();
        for n in [
            Network::Mainnet,
            Network::Shasta,
            Network::Nile,
            Network::Local,
        ] {
            assert!(
                t.contains_key(&n),
                "tokens/network.json missing row for {n:?}"
            );
        }
    }

    /// `clap::ValueEnum::value_variants()` is the canonical list clap hands
    /// back to `--help` and to `--possible_value`. Drift between the enum
    /// and the registered values turns into silent operator surprises
    /// (e.g. `--network foo` accepted but never wired).
    #[test]
    fn network_value_enum_lists_all_variants() {
        let listed: Vec<Network> = Network::value_variants().to_vec();
        assert_eq!(listed.len(), 4, "value_variants() must list every variant");
        for n in [
            Network::Mainnet,
            Network::Shasta,
            Network::Nile,
            Network::Local,
        ] {
            assert!(listed.contains(&n), "value_variants() missing {n:?}");
        }
    }

    /// The serde lowercase tags must match what `tag()` returns, so a JSON
    /// serialised with `tag()` round-trips through `serde_json::from_str`.
    #[test]
    fn network_serde_tags_match_tag_method() {
        for n in [
            Network::Mainnet,
            Network::Shasta,
            Network::Nile,
            Network::Local,
        ] {
            let body = serde_json::to_string(&n).expect("serialise");
            assert_eq!(body, format!("\"{}\"", n.tag()));
            let back: Network = serde_json::from_str(&body).expect("deserialise");
            assert_eq!(back, n);
        }
    }
}
