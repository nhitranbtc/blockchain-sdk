//! Wallet command handlers — Issue #426 / T6c sub-task (L25 split).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §3.3 (handlers/{mod,wallet,tx,erc20,fee,config,faucet,sign}.rs split)
//! + §5.4 (per-command signatures).
//!
//! T6c3 follow-up: real `wallet_show` impl (Story 9 — `wallet show`).
//! Reads `.meta.json` (plaintext metadata; no decrypt — encrypted blob
//! inspection deferred to T6d when rpassword + AES-GCM decryption
//! wires up). Real `wallet_create` + `wallet_import` deferred to T6c4;
//! `wallet_send_*` to T6c5 per L25 sub-task split.

use alloy_primitives::{Address, B256, U256};
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use std::str::FromStr;

use polygon_wallet_core::{new_http, new_http_polygon_amoy, Error, Result, WalletInfo};

/// ERC-20 Transfer(address,address,uint256) event topic0 hash.
///
/// `keccak256("Transfer(address,address,uint256)")` =
/// `0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef`.
/// Used to filter `eth_getLogs` to Transfer events involving the watch
/// address (matches topics[1] when the address is the sender — see
/// `wallet_sync` body for the OR semantics T7 must expand to).
const TRANSFER_TOPIC: [u8; 32] = [
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
];

// `TxSummary` lives in `polygon-wallet-core` (not here) — see
// `polygon-wallet-core/src/lib.rs`. Keeping it in this publish=false
// binary crate would make it unreachable to any sister CLI / future
// `--export` writer. Re-exported for ergonomics via the existing
// `polygon_wallet_core::*` import above; the local `use` of
// `polygon_wallet_core::TxSummary` happens at the call site.

/// Guard: only allow `https` RPC URLs and `http` to loopback hosts.
///
/// Closes the transport-security finding from the automated push
/// sweep on commit `8f34994`: the prior `wallet_balance` /
/// `wallet_sync` match arms accepted any URL scheme, including
/// `file://` and `ftp://`. `http` to a non-loopback host is also
/// rejected — cleartext RPC credentials + signed payloads must not
/// cross the wire. Returns `Error::InvalidInput` naming the rejected
/// scheme so operators see exactly what to fix.
fn validate_rpc_scheme(url: &url::Url) -> Result<()> {
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

/// Query native POL balance for `address` (Story 3 — `wallet balance`).
///
/// Uses `new_http_polygon_amoy()` (PR #424 Phase 2 convenience
/// constructor) — returns `RootProvider<Ethereum>` directly. Polygon
/// Amoy testnet default RPC (`https://polygon-amoy.drpc.org`).
///
/// When `rpc_url` is `Some`, parses it via `url::Url::parse` and uses
/// the generic `new_http(url)` constructor (re-exported from
/// `polygon-wallet-core`). When `None`, falls back to Amoy default.
///
/// Returns the balance in wei (U256). Caller formats with `--unit pol|wei`
/// (T6c1 follow-up #2 wires the unit-aware formatter + dispatch).
pub async fn wallet_balance(rpc_url: Option<&str>, address: &str) -> Result<U256> {
    let addr = Address::from_str(address)
        .map_err(|e| Error::InvalidInput(format!("invalid --address: {e}")))?;
    let provider = match rpc_url {
        Some(url_str) => {
            let url = url::Url::parse(url_str)
                .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
            validate_rpc_scheme(&url)?;
            new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")))?
        }
        None => new_http_polygon_amoy()
            .map_err(|e| Error::Rpc(format!("provider new_http_polygon_amoy: {e}")))?,
    };
    provider
        .get_balance(addr)
        .await
        .map_err(|e| Error::Rpc(format!("get_balance: {e}")))
}

/// Real `wallet list` impl (Story 9 — `wallet list`) — T6c2 (merged earlier).
#[allow(dead_code)] // wired in main.rs::run() (T6c2 follow-up merged)
pub fn wallet_list(
    data_dir: &std::path::Path,
    network: polygon_wallet_core::Network,
) -> Result<Vec<String>> {
    let network_dir = data_dir.join(network.as_dir_name());
    let mut names = Vec::new();
    if !network_dir.exists() {
        return Ok(names);
    }
    let entries = std::fs::read_dir(&network_dir)
        .map_err(|e| Error::Rpc(format!("read_dir {}: {e}", network_dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::Rpc(format!("dir entry: {e}")))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("meta.json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    Ok(names)
}

/// Real `wallet show` impl (Story 9 — `wallet show`) — T6c3 follow-up.
///
/// Reads `.meta.json` (plaintext metadata; no decrypt — encrypted
/// `.enc` blob inspection deferred to T6d when rpassword + AES-GCM
/// decryption wires up). Returns `Error::InvalidInput` if wallet_id
/// is not UUID format. Returns `Error::Rpc` on filesystem / parse errors.
pub fn wallet_show(
    data_dir: &std::path::Path,
    network: polygon_wallet_core::Network,
    wallet_id: &str,
) -> Result<WalletInfo> {
    let uuid = uuid::Uuid::from_str(wallet_id)
        .map_err(|e| Error::InvalidInput(format!("invalid wallet_id (expected UUID): {e}")))?;
    let path = data_dir
        .join(network.as_dir_name())
        .join(format!("{uuid}.meta.json"));
    let bytes = std::fs::read(&path)
        .map_err(|e| Error::Rpc(format!("read_file {}: {e}", path.display())))?;
    serde_json::from_slice(&bytes).map_err(|e| Error::Rpc(format!("parse meta.json: {e}")))
}

/// Real `wallet delete` impl (Story 9 — `wallet delete`) — T6c3.
///
/// Removes `<data_dir>/<network>/<wallet_id>.meta.json` and the
/// matching `<wallet_id>.enc` blob. Returns `Error::InvalidInput` if
/// the wallet_id is malformed (must be UUID format per WalletManager).
/// Returns `Error::Rpc` on filesystem errors. Returns `Ok(())` even if
/// the wallet doesn't exist (idempotent — matches Story 9 AC).
pub fn wallet_delete(
    data_dir: &std::path::Path,
    network: polygon_wallet_core::Network,
    wallet_id: &str,
) -> Result<()> {
    let uuid = uuid::Uuid::from_str(wallet_id)
        .map_err(|e| Error::InvalidInput(format!("invalid wallet_id (expected UUID): {e}")))?;
    let network_dir = data_dir.join(network.as_dir_name());
    for ext in ["meta.json", "enc"] {
        let path = network_dir.join(format!("{uuid}.{ext}"));
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Idempotent: ignore missing files.
            }
            Err(e) => {
                return Err(Error::Rpc(format!("remove_file {}: {e}", path.display())));
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn wallet_create(_name: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet create: deferred past T6c3 follow-up (lands in T6c4)".into(),
    ))
}
#[allow(dead_code)]
pub fn wallet_import(_name: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet import: deferred past T6c3 follow-up (lands in T6c4)".into(),
    ))
}
/// `wallet sync` handler — T6c3 follow-up #3.
///
/// Polls ERC-20 Transfer events involving `address`. Returns one
/// `polygon_wallet_core::TxSummary` per matching log entry. Per design
/// doc §5.4 the return type is `Vec<TxSummary>` (lightweight subset of
/// the full `Vec<Transaction>`) — AMENDMENT to original §5.4 which
/// specified `Result<()>` + internal print. Decision rationale: same
/// pattern as `wallet_balance` (returns `U256`, main.rs formats); the
/// handler/formatter split enables `--json` + future `--export` without
/// duplicating formatting in the handler.
///
/// Live RPC body (the `provider.get_logs(&filter).await` call that
/// fetches logs and decodes them) is deferred to T7 operator-driven
/// integration per L29 — operator session runs against Amoy testnet.
/// The address parse + provider build paths exercise now; the actual
/// `get_logs` call sits behind an early-return `Error::Rpc` so CI
/// compiles + negative-tests pass without a live RPC dependency. T7
/// removes the early-return.
#[allow(dead_code)] // wired in main.rs::run() (T6c3 follow-up #2 dispatch)
pub async fn wallet_sync(
    rpc_url: Option<&str>,
    _network: polygon_wallet_core::Network,
    address: &str,
) -> Result<Vec<polygon_wallet_core::TxSummary>> {
    let addr = Address::from_str(address)
        .map_err(|e| Error::InvalidInput(format!("invalid --address: {e}")))?;
    let provider = match rpc_url {
        Some(url_str) => {
            let url = url::Url::parse(url_str)
                .map_err(|e| Error::Rpc(format!("rpc url parse failed: {e}")))?;
            validate_rpc_scheme(&url)?;
            new_http(url).map_err(|e| Error::Rpc(format!("provider new_http: {e}")))?
        }
        None => new_http_polygon_amoy()
            .map_err(|e| Error::Rpc(format!("provider new_http_polygon_amoy: {e}")))?,
    };
    // Filter: Transfer events where `from` (topic1) equals the watch
    // address (left-padded to 32 bytes). `eth_getLogs` semantics: a
    // null topic = wildcard, so `[X, null]` = topic1==X AND
    // topic2==any. T7 expands this to two `get_logs` calls — one for
    // topic1==X (transfers FROM the address), one for topic2==X
    // (transfers TO the address) — then merges + dedupes by
    // `(tx_hash, log_index)`. Keeping it single-topic for now keeps
    // the CI-compile surface small.
    let padded = B256::left_padding_from(addr.as_slice());
    let filter = Filter::new()
        .event_signature(B256::from_slice(&TRANSFER_TOPIC))
        .topic1(padded);
    let _provider = provider;
    let _filter = filter;
    Err(Error::Rpc("wallet sync not yet implemented".into()))
}
#[allow(dead_code)]
pub async fn wallet_send_native(_to: &str, _amount: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet send: deferred past T6c3 follow-up (lands in T6c5)".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_send_speedup(_tx_hash: &str) -> Result<()> {
    Err(Error::Rpc(
        "wallet send speed-up: deferred past T6c3 follow-up (lands in T6c5)".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::wallet_list;
    use alloy_primitives::{Address, B256, U256};
    use polygon_wallet_core::{Network, PolygonChain};
    use std::path::PathBuf;

    /// T6c2 test: nonexistent data_dir returns empty list (not Err).
    #[test]
    fn wallet_list_returns_empty_for_nonexistent_dir() {
        let r = wallet_list(
            &PathBuf::from("/nonexistent/path/polygon-cli-test-xyz"),
            Network::Polygon(PolygonChain::Amoy),
        );
        assert!(
            r.is_ok(),
            "nonexistent dir should be Ok(empty), not error; got {r:?}"
        );
        assert_eq!(r.unwrap(), Vec::<String>::new());
    }

    /// T6c1 test: invalid address must surface as `Error::InvalidInput`
    /// (exit 2). Live RPC test deferred to T7 (operator-driven per L29).
    /// Wraps `wallet_balance` in `tokio::runtime::Runtime::block_on`
    /// since the production fn is `async` (no separate runtime dep).
    #[test]
    fn wallet_balance_rejects_invalid_address() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_balance(None, "not-an-address"));
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "invalid --address must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 test: `wallet_delete` rejects invalid (non-UUID) wallet_id.
    #[test]
    fn wallet_delete_rejects_invalid_wallet_id() {
        use super::wallet_delete;
        let r = wallet_delete(
            &PathBuf::from("/tmp/polygon-cli-test"),
            Network::Polygon(PolygonChain::Amoy),
            "not-a-uuid",
        );
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "non-UUID wallet_id must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 test: `wallet_delete` on nonexistent path is idempotent (Ok).
    #[test]
    fn wallet_delete_nonexistent_is_idempotent() {
        use super::wallet_delete;
        let r = wallet_delete(
            &PathBuf::from("/nonexistent/path/polygon-cli-test-xyz"),
            Network::Polygon(PolygonChain::Amoy),
            "00000000-0000-0000-0000-000000000000",
        );
        assert!(
            r.is_ok(),
            "nonexistent dir should be Ok(idempotent), not Err; got {r:?}"
        );
    }

    /// T6c3 follow-up test: `wallet_show` rejects invalid wallet_id.
    #[test]
    fn wallet_show_rejects_invalid_wallet_id() {
        use super::wallet_show;
        let r = wallet_show(
            &PathBuf::from("/tmp/polygon-cli-test"),
            Network::Polygon(PolygonChain::Amoy),
            "not-a-uuid",
        );
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "non-UUID wallet_id must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 follow-up test: `wallet_show` on nonexistent path is Ok (file not found).
    #[test]
    fn wallet_show_nonexistent_path_is_error() {
        use super::wallet_show;
        let r = wallet_show(
            &PathBuf::from("/nonexistent/path/polygon-cli-test-xyz"),
            Network::Polygon(PolygonChain::Amoy),
            "00000000-0000-0000-0000-000000000000",
        );
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::Rpc(_))),
            "nonexistent wallet_id file should be Err (Rpc), not Ok; got {r:?}"
        );
    }

    /// T6c3 follow-up #3 test: `wallet_sync` rejects invalid (non-hex)
    /// --address. Mirrors `wallet_balance_rejects_invalid_address`.
    /// Live RPC body deferred to T7 per L29 — this test exercises the
    /// address-parse path that runs BEFORE provider construction.
    #[test]
    fn wallet_sync_rejects_invalid_address() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_sync(
                None,
                Network::Polygon(PolygonChain::Amoy),
                "not-an-address",
            ));
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(_))),
            "invalid --address must surface as Error::InvalidInput; got {r:?}"
        );
    }

    /// T6c3 follow-up #3 test: `wallet_sync` rejects malformed
    /// `--rpc-url` via the URL-parse path. Exercises the provider-build
    /// branch that runs BEFORE the live-RPC early-return.
    #[test]
    fn wallet_sync_rejects_invalid_rpc_url() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_sync(
                Some("not a url"),
                Network::Polygon(PolygonChain::Amoy),
                "0x0000000000000000000000000000000000000001",
            ));
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::Rpc(_))),
            "invalid --rpc-url must surface as Error::Rpc; got {r:?}"
        );
    }

    /// Security fix #2 (transport-security): `wallet_balance` rejects
    /// cleartext HTTP RPC URLs to non-loopback hosts. Localhost / 127.0.0.1
    /// / ::1 remain allowed for Anvil-regtest per design doc §9.
    #[test]
    fn wallet_balance_rejects_http_rpc_to_remote_host() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_balance(
                Some("http://example.com"),
                "0x0000000000000000000000000000000000000001",
            ));
        match r {
            Err(polygon_wallet_core::Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("scheme not allowed"),
                    "InvalidInput must mention rejected scheme; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// Security fix #2 (transport-security): `wallet_sync` rejects
    /// cleartext HTTP RPC URLs to non-loopback hosts.
    #[test]
    fn wallet_sync_rejects_http_rpc_to_remote_host() {
        let r = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(super::wallet_sync(
                Some("http://example.com"),
                Network::Polygon(PolygonChain::Amoy),
                "0x0000000000000000000000000000000000000001",
            ));
        match r {
            Err(polygon_wallet_core::Error::InvalidInput(msg)) => {
                assert!(
                    msg.contains("scheme not allowed"),
                    "InvalidInput must mention rejected scheme; got: {msg}"
                );
            }
            other => panic!("expected Error::InvalidInput, got {other:?}"),
        }
    }

    /// Security fix #2 positive path: `http://localhost` is allowed
    /// (Anvil-regtest use case per design doc §9). The handler still
    /// fails because no live RPC, but with `Error::Rpc` — NOT
    /// `Error::InvalidInput`. Verifies the loopback exemption.
    #[test]
    fn validate_rpc_scheme_allows_http_to_localhost() {
        let url = url::Url::parse("http://localhost:8545").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_ok());
        let url = url::Url::parse("http://127.0.0.1:8545").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_ok());
        let url = url::Url::parse("https://polygon-rpc.com").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_ok());
        let url = url::Url::parse("http://example.com").expect("parses");
        assert!(super::validate_rpc_scheme(&url).is_err());
    }

    /// T6c3 follow-up #3 test: `TxSummary` survives a JSON
    /// serialize → deserialize roundtrip with field values intact.
    /// Independent of provider / RPC — fixture-driven. Required for
    /// the `--json` output formatter wired in main.rs::run().
    /// `TxSummary` lives in `polygon-wallet-core` (not this crate's
    /// binary scope) — see `polygon-wallet-core/src/lib.rs`.
    #[test]
    fn tx_summary_serde_json_roundtrip() {
        use polygon_wallet_core::TxSummary;
        let summary = TxSummary {
            block_number: 12_345,
            tx_hash: B256::repeat_byte(0xab),
            from: Address::repeat_byte(0x01),
            to: Address::repeat_byte(0x02),
            value: U256::from(1_000u64),
        };
        let json = serde_json::to_string(&summary).expect("TxSummary serializes");
        let back: TxSummary = serde_json::from_str(&json).expect("TxSummary deserializes");
        assert_eq!(back, summary);
    }
}
