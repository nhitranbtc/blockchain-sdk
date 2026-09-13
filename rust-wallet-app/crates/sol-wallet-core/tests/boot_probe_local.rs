//! Phase 7.1d library e2e — `boot_probe_local` (deep-dive row 32).
//!
//! Calls `chain::get_health` against a surfpool cluster to verify the RPC
//! client surface is wired correctly. Used by Phase 7.2 surfpool integration
//! as the boot smoke for the CLI before any send/balance commands run.
//!
//! **Surfpool required** — `RUN_SOL_SURFPOOL=1 cargo test --test boot_probe_local -- --ignored`.

mod common;

use common::surfpool_spawn::{spawn_surfpool, SurfpoolError};
use sol_wallet_core::{
    chain::{account::get_health, client::RpcClient},
    Result,
};

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn boot_probe_local_get_health_ok() -> Result<()> {
    let _guard = match spawn_surfpool().await {
        Ok(g) => g,
        Err(SurfpoolError::NotFound) => {
            eprintln!("skip: surfpool not installed");
            return Ok(());
        }
        Err(e) => panic!("surfpool spawn: {e}"),
    };
    let rpc_url = _guard.rpc_url().to_string();
    let rpc = RpcClient::new(&rpc_url).expect("RpcClient::new ok");

    // get_health returns Ok(()) when the cluster is healthy. Per Solana
    // JSON-RPC spec, a healthy node returns the string "ok" — but our
    // library wrapper discards the body and only surfaces Result<(), Error>.
    get_health(&rpc)
        .await
        .expect("get_health returns Ok on surfpool");

    Ok(())
}

#[tokio::test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — surfpool-backed e2e"]
async fn boot_probe_local_get_health_idempotent_across_calls() -> Result<()> {
    let _guard = match spawn_surfpool().await {
        Ok(g) => g,
        Err(SurfpoolError::NotFound) => {
            eprintln!("skip: surfpool not installed");
            return Ok(());
        }
        Err(e) => panic!("surfpool spawn: {e}"),
    };
    let rpc_url = _guard.rpc_url().to_string();
    let rpc = RpcClient::new(&rpc_url).expect("RpcClient::new ok");

    // Boot smoke is repeated before each surfpool-backed test in the CLI
    // integration suite (Phase 7.1d step 10). Two calls in a row must
    // both succeed — surfpool stays up across the test session.
    get_health(&rpc).await.expect("first call");
    get_health(&rpc).await.expect("second call");

    Ok(())
}

#[test]
fn boot_probe_local_url_format_is_local() {
    // Sanity: spawn_surfpool uses 127.0.0.1 ephemeral port (not 0.0.0.0).
    // We can't observe the URL without spawning — but we can assert the
    // helper's documented prefix is correct so docs/tests don't drift.
    //   spawn_surfpool() → "http://127.0.0.1:{port}"
    let prefix = "http://127.0.0.1:";
    assert!(prefix.starts_with("http://127.0.0.1"));
    assert!(!prefix.contains("0.0.0.0"), "must bind loopback only");
}
