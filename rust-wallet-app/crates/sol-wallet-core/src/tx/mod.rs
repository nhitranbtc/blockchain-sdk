//! `tx` — transaction construction surface.
//!
//! Phase 3 owns the SOL transfer builder + Compute Budget auto-attach
//! (`builder.rs`). Phase 4 extends `builder.rs` with the SPL builders
//! (`build_spl_transfer_checked`, `build_spl_approve`,
//! `build_spl_close_account`, `prepend_create_ata`); Phase 5 owns the
//! broadcast + retry surface (`broadcast.rs`) — those modules are
//! created in their own phases per the plan.
//!
//! Public surface today (Phase 3):
//!
//! | Function                       | Lands in |
//! |--------------------------------|----------|
//! | `build_sol_transfer`           | Phase 3  |
//! | `build_sol_transfer_with_budget` | Phase 3 |
//! | `compute_budget_instructions`  | Phase 3  |
//!
//! V0.1 ships only the builder. The signer / broadcaster live in
//! Phase 5 (`tx::broadcast`); this module re-exports nothing from
//! there on purpose — the Phase 7 CLI pulls `tx::builder` and
//! `tx::broadcast` separately to keep the dependency graph shallow.

pub mod builder;
