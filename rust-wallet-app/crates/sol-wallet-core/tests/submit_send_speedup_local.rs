//! Phase 7.1c library e2e — `submit_send_speedup_local` (deep-dive row 35).
//!
//! Re-broadcasts the same SOL transfer with a higher priority fee. Solana
//! has no RBF; this exercises fee-bumping via a fresh signature + same
//! recent blockhash.

#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — Phase 7.1c e2e; surfpool not yet available in sandbox"]
fn submit_send_speedup_local_bumps_fee() {
    // Phase 7.1c scaffold:
    //   1. submit_sol_local_round_trip-style setup; capture first signature
    //   2. build_sol_transfer_with_budget with priority_fee 10x larger
    //   3. sign same keypair + same blockhash + send_and_confirm
    //   4. assert new signature distinct from first; blockhash reused
    //   5. assert OwnedLock held across 2 RPCs (P7-2 longer-lifetime case);
    //      verify zeroize on Drop via heap probe
    //   see plan Phase 7.1c step 5 row 35.
    todo!("submit_send_speedup_local_bumps_fee — lands with surfpool CI integration");
}
