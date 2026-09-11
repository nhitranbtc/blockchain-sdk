//! Phase 7.1d library e2e — `boot_probe_local` (deep-dive row 32).
//!
//! Calls `chain::get_health` against a surfpool cluster to verify the RPC
//! client surface is wired correctly. Used by Phase 7.2 surfpool integration
//! as the boot smoke for the CLI before any send/balance commands run.
//!
//! **Surfpool required** — `RUN_SOL_SURFPOOL=1 cargo test --test boot_probe_local -- --ignored`.

#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — Phase 7.1d e2e; surfpool not yet available in sandbox"]
fn boot_probe_local_get_health_ok() {
    // Phase 7.1d scaffold:
    //   1. spin up RpcClient(http://127.0.0.1:8899)
    //   2. chain::get_health(&rpc) — assert Ok with status "ok"
    //   3. assert response shape matches JSON-RPC envelope
    //   see plan 2026-09-09-sol-wallet-core-v0.1.md Phase 7.1d step 10.
    todo!("boot_probe_local_get_health_ok — lands with surfpool CI integration");
}
