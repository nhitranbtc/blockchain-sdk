//! Configuration root for `tron-wallet-core`.
//!
//! `TronConfig` is the single value the rest of the crate reads at every
//! request boundary. The defaults that ship with v0.1 — RPC URL per
//! network, SPKI pin per network — are recorded in
//! `tokens/network.json`, not in source. Editing that file changes
//! every embedder (CLI, FFI host, integration test) at once, with no
//! Rust recompile. The JSON is loaded once via [`std::sync::OnceLock`]
//! and the parsed strings are `Box::leak`-d into a `&'static` map so
//! the public API keeps its `&'static str` / `Option<SpkiPin>` shape
//! (the public types — `SpkiPin` — are unchanged).
//!
//! ## Why config in `tokens/`
//!
//! The plan's bundled token list (`tokens/{local,nile,mainnet}.json`)
//! already establishes `tokens/` as the operator-editable
//! configuration directory. Adding `tokens/network.json` (RPC URL +
//! per-network SPKI pin) + `tokens/test-vectors.json` (test-only
//! addresses that round-trip every test) keeps every operator-tunable
//! string in one tree, and keeps the Rust source free of hardcoded
//! endpoints / pins / addresses.
//!
//! ## Operator flow
//!
//! ```text
//! # Production: rotate a mainnet RPC endpoint
//! $EDITOR tokens/network.json          # update "mainnet.rpc_url"
//! cargo build
//!
//! # Production: rotate the mainnet SPKI pin
//! $EDITOR tokens/network.json          # update "mainnet.spki_pin_hex"
//! cargo build                         # constant-time pin compare
//!
//! # Production: enable Nile SPKI pinning
//! export TON_NILE_SPKI_PIN_HEX=$(openssl x509 -in nile.pem -pubkey -noout \
//!     | openssl pkey -pubin -outform DER | sha256sum | cut -d' ' -f1)
//! cargo build                         # env var overrides "nile.spki_pin_hex": null
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::chain::spki::SpkiPin;
use crate::error::{Error, Result};

/// One row of `tokens/network.json`. The `spki_pin_hex` is `null` when
/// no default pin ships with the crate (Nile / Shasta / Local — see
/// [`default_spki_pin`]).
#[derive(Debug, Clone, Deserialize)]
struct NetworkEntry {
    rpc_url: String,
    spki_pin_hex: Option<String>,
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
                 This file is the canonical source for RPC URLs and \
                 default SPKI pins — fix the JSON, not the loader."
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Default RPC URL for a given network. Read from
/// `tokens/network.json`; the value is `Box::leak`-d so the
/// `&'static str` return type survives.
pub fn default_rpc_url(network: Network) -> &'static str {
    let entry = network_table()
        .get(&network)
        .expect("tokens/network.json must contain a row for every Network variant");
    Box::leak(entry.rpc_url.clone().into_boxed_str())
}

/// SHA-256 SPKI pin for the network's leaf cert, read from
/// `tokens/network.json`. `None` when the JSON row has
/// `spki_pin_hex: null` (Shasta, Local) OR when the operator has
/// not yet set a Nile pin (Nile reads the env var as the override
/// path; see [`NILE_SPKI_PIN_ENV`]).
///
/// The mainnet pin lives in `tokens/network.json:mainnet.spki_pin_hex`
/// and was extracted per plan §Q5 from the live `api.trongrid.io`
/// leaf cert's `SubjectPublicKeyInfo`:
/// `openssl x509 -in leaf.pem -pubkey -noout | openssl pkey -pubin
/// -outform DER | sha256sum`. The verifier hashes only the SPKI
/// (not the whole cert) — see `src/chain/spki.rs`.
pub fn default_spki_pin(network: Network) -> Option<SpkiPin> {
    let entry = network_table()
        .get(&network)
        .expect("tokens/network.json must contain a row for every Network variant");
    let hex = match entry.spki_pin_hex.as_deref() {
        Some(h) => h,
        // Nile: the JSON marks the pin as null because the operator
        // extracts it at deploy time. The env-var override path is
        // the documented delivery mechanism (see
        // [`nile_default_spki_pin`]).
        None if network == Network::Nile => return nile_default_spki_pin(),
        None => return None,
    };
    parse_spki_pin_hex(network, hex)
}

fn parse_spki_pin_hex(network: Network, hex: &str) -> Option<SpkiPin> {
    let bytes = match hex::decode(hex) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "warning: tokens/network.json {network:?}.spki_pin_hex is not valid hex ({e}); \
                 falling back to no SPKI pin"
            );
            return None;
        }
    };
    // `bytes.len()` is consumed by `try_into` below, so capture the
    // length up front for the size-mismatch error message.
    let bytes_len = bytes.len();
    let arr: [u8; 32] = match bytes.try_into() {
        Ok(a) => a,
        Err(_) => {
            eprintln!(
                "warning: tokens/network.json {network:?}.spki_pin_hex must decode to \
                 exactly 32 bytes (got {bytes_len}); falling back to no SPKI pin"
            );
            return None;
        }
    };
    Some(SpkiPin::from_bytes(arr))
}

/// Decode the operator-supplied Nile SPKI pin from [`NILE_SPKI_PIN_ENV`].
///
/// The env var OVERRIDES `tokens/network.json:nile.spki_pin_hex` —
/// the env var is the documented delivery path for operator-extracted
/// pins because the JSON ships with the binary and rotation should
/// not require a recompile. Setting the env var to an empty value
/// disables the pin (the documented "default off" state for Nile).
pub fn nile_default_spki_pin() -> Option<SpkiPin> {
    let raw = std::env::var(NILE_SPKI_PIN_ENV).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bytes = match hex::decode(trimmed) {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "warning: {NILE_SPKI_PIN_ENV} is not valid hex ({e}); falling back to no SPKI pin"
            );
            return None;
        }
    };
    let arr: [u8; 32] = match bytes.try_into() {
        Ok(a) => a,
        Err(_) => {
            eprintln!(
                "warning: {NILE_SPKI_PIN_ENV} must decode to exactly 32 bytes (got {}); \
                 falling back to no SPKI pin",
                trimmed.len() / 2
            );
            return None;
        }
    };
    Some(SpkiPin::from_bytes(arr))
}

/// Environment variable an operator sets to enable SPKI pinning on the
/// Nile testnet endpoint (`nile.trongrid.io`). Value must be a
/// 32-byte hex string — the SHA-256 of the leaf cert's SubjectPublicKeyInfo.
///
/// Overrides `tokens/network.json:nile.spki_pin_hex` so a pin rotation
/// does not require a rebuild. Plan Phase 3 carry-over Task 2.7 —
/// closed only when the operator extracts the live pin from
/// `nile.trongrid.io`'s cert and exports this variable before
/// running the gated live tests (`RUN_TRON_NILE=1`). CI never sets
/// it; `cargo test` skips the gated tests when the env var is absent.
pub const NILE_SPKI_PIN_ENV: &str = "TON_NILE_SPKI_PIN_HEX";

/// Re-export of the mainnet pin, decoded from the JSON-loaded entry.
/// Kept as a function (not a constant) so callers that need
/// `SpkiPin` get the runtime type without an extra decode step.
/// The hex value itself comes from `tokens/network.json` — never
/// hardcoded in source.
pub fn mainnet_spki_pin() -> SpkiPin {
    default_spki_pin(Network::Mainnet)
        .expect("tokens/network.json:mainnet.spki_pin_hex is required")
}

/// Hex form of the mainnet SPKI pin, loaded from
/// `tokens/network.json:mainnet.spki_pin_hex`. Provided as a
/// `&'static str` for callers that compare against a hex string
/// (the regression test in `tests/v7_spki_pin.rs` is the only
/// current caller). The constant used to live as a `pub const
/// MAINNET_SPKI_PIN_HEX` in this file before the Phase 3 config
/// refactor; that const is gone — the JSON is the single source of
/// truth.
pub fn mainnet_spki_pin_hex() -> &'static str {
    let entry = network_table()
        .get(&Network::Mainnet)
        .expect("tokens/network.json must contain a row for every Network variant");
    Box::leak(
        entry
            .spki_pin_hex
            .as_deref()
            .unwrap_or("")
            .to_owned()
            .into_boxed_str(),
    )
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

    /// Mainnet AND Nile ship default SPKI pins in the JSON. Shasta
    /// and Local have no stable pin (operator-supplied or test-only).
    /// This test pins that contract as of 2026-09-06 when the Nile
    /// pin was extracted live and committed to the JSON (Phase 3
    /// carry-over Task 2.7 close).
    #[test]
    fn only_mainnet_and_nile_have_default_spki_pins_in_json() {
        let t = network_table();
        assert!(t[&Network::Mainnet].spki_pin_hex.is_some());
        assert!(
            t[&Network::Nile].spki_pin_hex.is_some(),
            "Nile ships with a SPKI pin now (live-extracted 2026-09-06, \
             Phase 3 carry-over Task 2.7); the JSON must hold the pin"
        );
        for n in [Network::Shasta, Network::Local] {
            assert!(
                t[&n].spki_pin_hex.is_none(),
                "{n:?} ships a default SPKI pin — the JSON should mark it null \
                 so operators supply it via TON_NILE_SPKI_PIN_HEX or a future config"
            );
        }
    }

    /// The mainnet pin, decoded through the public helper, must equal
    /// the canonical hex value the plan cited (Q5 live extraction).
    /// This is a build-time guard against the JSON being edited in a
    /// way that silently weakens pinning.
    #[test]
    fn mainnet_spki_pin_matches_plan_q5_live_extraction() {
        let pin = mainnet_spki_pin();
        let hex = format!("{}", pin);
        assert_eq!(
            hex, "0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d",
            "tokens/network.json:mainnet.spki_pin_hex drifted from the plan Q5 \
             canonical value. Re-extract from the live cert or document the \
             new value in the plan."
        );
    }
}
