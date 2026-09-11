//! Phase 6.1 Task 6.1 Step 12 — `throwaway_keypair() -> Zeroizing<Keypair>`.
//!
//! Per audit P6-13: helper wraps in `Zeroizing<Keypair>` so the
//! bytes zero on panic unwind.

use solana_sdk::signature::Keypair;
use zeroize::Zeroizing;

/// Throwaway keypair — never logged, dropped after test.
pub fn throwaway_keypair() -> Zeroizing<Keypair> {
    Zeroizing::new(Keypair::new())
}
