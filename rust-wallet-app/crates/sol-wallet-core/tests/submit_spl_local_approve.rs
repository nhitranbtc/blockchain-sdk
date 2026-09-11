//! Phase 7.1c library e2e — `submit_spl_local_approve` (deep-dive row 29).
//!
//! Delegates spending authority on an SPL token account via `build_spl_approve`.

#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — Phase 7.1c e2e; surfpool not yet available in sandbox"]
fn submit_spl_local_approve_delegate() {
    // Phase 7.1c scaffold:
    //   1. deploy mock USDC mint + fund holder
    //   2. build_spl_approve(holder, mint, delegate, 500_000_000)
    //   3. sign + send_and_confirm
    //   4. assert holder.token_account.delegate == delegate + delegated amount
    //   see plan Phase 7.1c step 5 row 29.
    todo!("submit_spl_local_approve_delegate — lands with surfpool CI integration");
}
