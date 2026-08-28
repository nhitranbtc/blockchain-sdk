//! Wallet command handlers — Issue #426 / T6c sub-task (L25 split).
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §3.3 (handlers/{mod,wallet,tx,erc20,fee,config,faucet,sign}.rs split)
//! + §5.4 (per-command signatures).
//!
//! T6c scaffold-only — handler BODIES (wallet_create, wallet_import,
//! wallet_list, wallet_show, wallet_delete, wallet_balance, wallet_sync,
//! wallet_send, wallet_send_speedup) deferred to subsequent T6c commits.
//! This commit lands the file with `pub fn` stubs returning
//! `Error::Rpc("deferred past T6c — landing in subsequent commit")` so the
//! binary builds + dispatch wiring (added in T6b) compiles against the
//! full handler surface. Real alloy-* impls land when the alloy transport
//! type-system surface (Http<Network>, RootProvider, etc.) is pinned down
//! for the polygon CLI specifically — the 1.8.x release has known
//! type-inference rough edges that benefit from a dedicated impl pass.
use polygon_wallet_core::Error;
/// T6c scaffold — wallet command stubs. Real impls deferred to subsequent
/// T6c commits per L25 sub-task split. Each stub exits with a clear
/// operator-facing message identifying which sub-task is needed.
#[allow(dead_code)]
pub fn wallet_create(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet create: deferred past T6c scaffold".into(),
    ))
}
#[allow(dead_code)]
pub fn wallet_import(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet import: deferred past T6c scaffold".into(),
    ))
}
#[allow(dead_code)]
pub fn wallet_list() -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc("wallet list: deferred past T6c scaffold".into()))
}
#[allow(dead_code)]
pub fn wallet_show(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc("wallet show: deferred past T6c scaffold".into()))
}
#[allow(dead_code)]
pub fn wallet_delete(_name: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet delete: deferred past T6c scaffold".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_balance(
    _rpc_url: Option<&str>,
    _address: &str,
) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet balance: deferred past T6c scaffold".into(),
    ))
}
#[allow(dead_code)]
pub async fn wallet_sync(_address: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc("wallet sync: deferred past T6c scaffold".into()))
}
#[allow(dead_code)]
pub async fn wallet_send_native(_to: &str, _amount: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc("wallet send: deferred past T6c scaffold".into()))
}
#[allow(dead_code)]
pub async fn wallet_send_speedup(_tx_hash: &str) -> polygon_wallet_core::Result<()> {
    Err(Error::Rpc(
        "wallet send speed-up: deferred past T6c scaffold".into(),
    ))
}
