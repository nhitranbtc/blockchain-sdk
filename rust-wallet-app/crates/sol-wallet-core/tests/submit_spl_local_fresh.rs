//! Phase 7.1c library e2e — `submit_spl_local_fresh` (deep-dive row 28).
//!
//! Transfers an SPL token to a fresh recipient with NO pre-existing ATA.
//! Verifies `prepend_create_ata` builder path + rent-delta accounting.

#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — Phase 7.1c e2e; surfpool not yet available in sandbox"]
fn submit_spl_local_fresh_creates_ata() {
    // Phase 7.1c scaffold:
    //   1. deploy mock USDC mint
    //   2. recipient has NO ATA pre-test
    //   3. transfer 100 USDC + prepend CreateATA instruction
    //   4. assert recipient ATA created + balance increased
    //   5. assert rent delta = 0.001428 SOL deducted from sender (ATA rent)
    //   see plan Phase 7.1c step 5 row 28.
    todo!("submit_spl_local_fresh_creates_ata — lands with surfpool CI integration");
}
