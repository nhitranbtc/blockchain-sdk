//! Phase 7.1c library e2e — `submit_sol_local` (deep-dive row 26).
//!
//! Signs a native SOL transfer with `OwnedLock<Zeroizing<Keypair>>`, broadcasts
//! via `send_and_confirm`, asserts on-chain balance delta on surfpool.
//!
//! **Surfpool required** — `RUN_SOL_SURFPOOL=1 cargo test --test submit_sol_local -- --ignored`.
//! Scaffolded in 7.1c; body lands when surfpool CI integration is available.

#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — Phase 7.1c e2e; surfpool not yet available in sandbox"]
fn submit_sol_local_round_trip() {
    // Phase 7.1c scaffold:
    //   1. spin up RpcClient(http://127.0.0.1:8899)
    //   2. throwaway_keypair() → airdrop 2 SOL via faucet::airdrop_to_keypair
    //   3. WalletManager::unlock(id, password) → OwnedLock<Zeroizing<Keypair>>
    //   4. build_sol_transfer(&src, &dest, 1_000_000_000) + send_and_confirm
    //   5. assert dest balance increased by 1 SOL
    //   6. assert OwnedLock::Drop zeroizes keypair (heap probe via std::ptr)
    //   see plan 2026-09-09-sol-wallet-core-v0.1.md Phase 7.1c step 5 row 26.
    todo!("submit_sol_local_round_trip — lands with surfpool CI integration");
}
