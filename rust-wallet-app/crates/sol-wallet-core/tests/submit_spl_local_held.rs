//! Phase 7.1c library e2e — `submit_spl_local_held` (deep-dive row 27).
//!
//! Transfers an SPL token from a holder who already has an ATA for the mint.
//! Used to validate `build_spl_transfer_checked` + existing-ATA path.

#[test]
#[ignore = "RUN_SOL_SURFPOOL=1 required — Phase 7.1c e2e; surfpool not yet available in sandbox"]
fn submit_spl_local_held_ata_path() {
    // Phase 7.1c scaffold:
    //   1. deploy mock USDC mint via mock_spl_usdc::deploy_usdc_mint (1B to holder)
    //   2. holder's ATA exists pre-test (deposit path)
    //   3. transfer 100 USDC (100_000_000 raw = 6 decimals) to dest
    //   4. assert holder ATA balance decreased, dest ATA increased
    //   see plan Phase 7.1c step 5 row 27.
    todo!("submit_spl_local_held_ata_path — lands with surfpool CI integration");
}
