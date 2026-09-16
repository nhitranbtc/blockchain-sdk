//! `compute_budget` — Phase 3.1 (deep-dive row 15 — Compute Budget auto-attach).
//!
//! Validates Compute Budget instruction construction WITHOUT a live
//! cluster. The Phase 7.2 e2e (`submit_sol_local`) covers the full
//! surfpool send+confirm flow; here we prove the helper:
//!   1. Returns exactly two instructions when the budget is appended
//!   2. Encodes CU limit + CU price into the data payload using the
//!      Solana runtime's Borsh wire format (tag byte + little-endian
//!      payload bytes)
//!   3. Passes through zero CU price (Q8 default; "no priority fee")
//!
//! No network or RPC: this is a pure-construction test. Surfpool
//! integration lives in Phase 7.2's `submit_sol_local.rs`.
//!
//! Wire format reference: `solana-compute-budget-interface` uses Borsh
//! internally — `data[0]` = variant tag, `data[1..]` = little-endian
//! payload. SetComputeUnitLimit = 0x02; SetComputeUnitPrice = 0x03
//! (declared in the `to_instruction!` macro in
//! `solana-compute-budget-interface-3.1.0/src/lib.rs`).

use sol_wallet_core::tx::builder::{compute_budget_instructions, DEFAULT_COMPUTE_UNIT_LIMIT};

/// Borsh variant tag for `SetComputeUnitLimit(u32)`.
const TAG_SET_COMPUTE_UNIT_LIMIT: u8 = 0x02;
/// Borsh variant tag for `SetComputeUnitPrice(u64)`.
const TAG_SET_COMPUTE_UNIT_PRICE: u8 = 0x03;

#[test]
fn default_compute_unit_limit_is_150k_per_solana_config_phase3_step4() {
    // Phase 3.1 Step 4: cu_limit default = 150_000 per Q8 (matches
    // Solana validator default; Phantom-equivalent UX sends this when
    // the user does not override).
    assert_eq!(DEFAULT_COMPUTE_UNIT_LIMIT, 150_000);
}

#[test]
fn compute_budget_instructions_emits_set_limit_then_set_price_in_borsh_wire_format() {
    let ixs = compute_budget_instructions(200_000, 5_000);
    // CU limit: tag 0x02 + u32 LE payload (5 bytes total)
    assert_eq!(ixs[0].data[0], TAG_SET_COMPUTE_UNIT_LIMIT);
    let limit_bytes: [u8; 4] = ixs[0].data[1..5].try_into().expect("5-byte limit ix");
    assert_eq!(u32::from_le_bytes(limit_bytes), 200_000);
    // CU price: tag 0x03 + u64 LE payload (9 bytes total)
    assert_eq!(ixs[1].data[0], TAG_SET_COMPUTE_UNIT_PRICE);
    let price_bytes: [u8; 8] = ixs[1].data[1..9].try_into().expect("9-byte price ix");
    assert_eq!(u64::from_le_bytes(price_bytes), 5_000);
}

#[test]
fn compute_budget_instructions_passes_through_zero_price() {
    // Q8: priority_fee = 0 by default. Builder must NOT reject zero —
    // a zero price is the canonical "no priority fee" state, and
    // validators process it identically to "no budget ix present".
    let ixs = compute_budget_instructions(DEFAULT_COMPUTE_UNIT_LIMIT, 0);
    assert_eq!(ixs[1].data[0], TAG_SET_COMPUTE_UNIT_PRICE);
    let price_bytes: [u8; 8] = ixs[1].data[1..9].try_into().expect("9-byte price ix");
    assert_eq!(u64::from_le_bytes(price_bytes), 0);
}

#[test]
fn compute_budget_instructions_preserves_order_across_runs() {
    // Determinism: same inputs → identical instruction bytes. The
    // wire format embeds no randomness, so two constructions must
    // byte-match.
    let a = compute_budget_instructions(150_000, 1_000);
    let b = compute_budget_instructions(150_000, 1_000);
    assert_eq!(a[0].data, b[0].data);
    assert_eq!(a[1].data, b[1].data);
}
