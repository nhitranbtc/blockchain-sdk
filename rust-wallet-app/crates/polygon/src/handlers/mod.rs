//! `polygon` CLI handlers — Issue #426 / Phase 4 of #416.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.3 + §6.2. Batch B (TDD): `parse_network` (this file); remaining
//! handlers land in subsequent batches.

use polygon_wallet_core::{Error, Network};

// Re-export submodules so callers (main.rs) can write `use handlers::*`
// instead of reaching into individual files. Mirrors design doc §5.3.
pub mod config;
pub mod erc20;
pub mod fee;
pub mod sign;
pub mod tx;
pub mod wallet;

/// Guard: only allow `https` RPC URLs and `http` to loopback hosts.
///
/// Closes the transport-security finding from the automated push
/// sweep on commit `8f34994`: the prior `wallet_balance` /
/// `wallet_sync` match arms accepted any URL scheme, including
/// `file://` and `ftp://`. `http` to a non-loopback host is also
/// rejected — cleartext RPC credentials + signed payloads must not
/// cross the wire. Returns `Error::InvalidInput` naming the rejected
/// scheme so operators see exactly what to fix.
///
/// **T6d-1 (Issue #426 / PR in flight):** moved here from
/// `handlers/wallet.rs` so future handlers (fee, tx, erc20, ...)
/// share the same scheme policy. Centralizing prevents
/// per-handler drift — each new RPC call site gets the guard by
/// default rather than remembering to inline a copy.
pub(super) fn validate_rpc_scheme(url: &url::Url) -> polygon_wallet_core::Result<()> {
    match url.scheme() {
        "https" => Ok(()),
        "http"
            if matches!(
                url.host_str(),
                Some("localhost") | Some("127.0.0.1") | Some("::1")
            ) =>
        {
            Ok(())
        }
        other => Err(Error::InvalidInput(format!(
            "rpc url scheme not allowed: {other}; use https (or http for localhost)"
        ))),
    }
}

/// Map `WalletError` (lib-level) onto `polygon_wallet_core::Error`
/// (CLI-canonical). Called by every handler that touches `WalletManager`
/// so the CLI's `Error::exit_code()` table applies (design §3.5 cross-
/// cutting line 293). Localities:
///
///   `InvalidInput` (exit 2) for caller-side errors (password, name,
///       mnemonic, duplicate name, missing wallet).
///   `Rpc` for filesystem + serialization + corruption — distinct
///       exit code per the canonical `evm-wallet-core::Error::exit_code()`
///       table.
///
/// T6c4: required because `WalletError` is the lib-canonical error
/// type (per Drift §2.5 + re-export added in
/// `polygon-wallet-core/src/lib.rs`). Centralizing the translation
/// here keeps per-handler code small + the exit-code table
/// authoritative.
pub(crate) fn map_wallet_err(e: polygon_wallet_core::WalletError) -> polygon_wallet_core::Error {
    use polygon_wallet_core::WalletError;
    match e {
        WalletError::Crypto(c) => {
            polygon_wallet_core::Error::InvalidInput(format!("wallet crypto error: {c}"))
        }
        WalletError::AlreadyExists { name, network } => polygon_wallet_core::Error::InvalidInput(
            format!("wallet '{name}' already exists on {network:?}"),
        ),
        WalletError::Mnemonic(s) => {
            polygon_wallet_core::Error::InvalidInput(format!("invalid mnemonic: {s}"))
        }
        WalletError::PrivateKey(s) => {
            polygon_wallet_core::Error::InvalidInput(format!("invalid private key: {s}"))
        }
        WalletError::NotFound { wallet_id } => {
            polygon_wallet_core::Error::InvalidInput(format!("wallet not found: {wallet_id}"))
        }
        WalletError::NotFoundByName { name, network } => polygon_wallet_core::Error::InvalidInput(
            format!("wallet '{name}' not found on {network:?}"),
        ),
        WalletError::Corrupt { reason } => {
            polygon_wallet_core::Error::Rpc(format!("wallet file corrupt: {reason}"))
        }
        WalletError::Io(io) => polygon_wallet_core::Error::Rpc(format!("io: {io}")),
        WalletError::Json(j) => polygon_wallet_core::Error::Rpc(format!("json: {j}")),
        WalletError::Path(s) => polygon_wallet_core::Error::Rpc(format!("path: {s}")),
    }
}

/// Validate a wallet name against the CLI-side charset. Mirrors
/// `eth/src/handlers.rs:95-113` with a tightening per
/// `code-review L2` + `type-design F5` + `security L-6` (L12 cluster
/// for T6c4): charset `[A-Za-z0-9_-]` (no space) — drops the space
/// to close the all-whitespace UX footgun where a wallet named `"   "`
/// would be invisible in `wallet list`. Length 1..=32 chars
/// (byte length, ASCII-only by construction). Separate error messages
/// per violation: empty / too-long / bad-charset / all-whitespace.
/// Returns `Error::InvalidInput` (exit 2) on any violation.
pub(crate) fn validate_wallet_name(name: &str) -> polygon_wallet_core::Result<()> {
    if name.is_empty() {
        return Err(polygon_wallet_core::Error::InvalidInput(
            "wallet name must not be empty".into(),
        ));
    }
    if name.len() > 32 {
        return Err(polygon_wallet_core::Error::InvalidInput(format!(
            "wallet name must be 1..=32 chars; got {}",
            name.len()
        )));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(polygon_wallet_core::Error::InvalidInput(
            "wallet name charset: [A-Za-z0-9_-] only (no whitespace)".into(),
        ));
    }
    if name.chars().all(|c| c.is_whitespace()) {
        return Err(polygon_wallet_core::Error::InvalidInput(
            "wallet name must contain at least one non-whitespace character".into(),
        ));
    }
    Ok(())
}

/// Parse `--network` at the polygon CLI boundary, narrowing the
/// vocabulary to `Network::Polygon(...)` only. Delegates to
/// `PolygonChain::parse_cli`; anything outside the polygon-flavored
/// vocabulary falls into the catch-all `Err(Error::InvalidInput(...))`
/// arm (per `evm-wallet-core/src/network.rs:228-237`).
///
/// L13 Round 1 fix #5 (drift #2 — design doc §2.2): the wrapper
/// surfaces a friendlier error message for `anvil` (the Ethereum
/// fork's name) and `31337` (the chain_id) — directs the operator to
/// the `eth` CLI for Anvil regtest. The base lib's generic
/// "unknown polygon network" message stays for all other unknowns.
#[allow(dead_code)] // wired into cli.rs --network flag in T6 follow-up
pub fn parse_network(s: &str) -> polygon_wallet_core::Result<Network> {
    polygon_wallet_core::PolygonChain::parse_cli(s)
        .map_err(|e| match s.to_ascii_lowercase().as_str() {
            "anvil" | "31337" => Error::InvalidInput(
                "polygon-cli targets Polygon PoS; for Anvil regtest (chain_id 31337), \
                 run the `eth` CLI with --network anvil (drift #2)"
                    .into(),
            ),
            _ => e,
        })
        .map(Network::Polygon)
}

#[cfg(test)]
mod tests {
    //! Batch B tests (per design doc §6.2): parse_network rejects anvil +
    //! mumbai + unknown; accepts amoy + mainnet.
    use super::parse_network;
    use polygon_wallet_core::{Error, Network, PolygonChain};

    /// Batch B test #1 (failing seed per design doc §6.2): amoy → Ok(Polygon(Amoy)).
    #[test]
    fn parse_network_amoy_returns_polygon_amoy() {
        let net = parse_network("amoy").expect("amoy parses");
        assert!(matches!(net, Network::Polygon(PolygonChain::Amoy)));
    }

    /// Batch B test #2: mainnet → Ok(Polygon(Mainnet)).
    #[test]
    fn parse_network_mainnet_returns_polygon_mainnet() {
        let net = parse_network("mainnet").expect("mainnet parses");
        assert!(matches!(net, Network::Polygon(PolygonChain::Mainnet)));
    }

    /// Batch B test #3 (Drift #2 explicit test): "anvil" rejected.
    #[test]
    fn parse_network_anvil_returns_invalid_input() {
        let r = parse_network("anvil");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "anvil must be rejected (cross-chain identity footgun); got {r:?}"
        );
    }

    /// Batch B test #4: "mumbai" rejected (deprecation 2024-Q2).
    #[test]
    fn parse_network_mumbai_returns_invalid_input() {
        let r = parse_network("mumbai");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "mumbai must be rejected (deprecation 2024-Q2); got {r:?}"
        );
    }

    /// Batch B test #5: unknown network rejected.
    #[test]
    fn parse_network_unknown_returns_invalid_input() {
        let r = parse_network("fakenet");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "unknown network must be rejected; got {r:?}"
        );
    }
}
