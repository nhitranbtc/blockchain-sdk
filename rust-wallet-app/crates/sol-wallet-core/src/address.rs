//! Solana address surface (base58, `is_on_curve`, PDA).
//!
//! Phase 0 ships an empty module — the module declaration itself is the
//! Phase 0 surface, so the `pub use solana_sdk::*` facade in `lib.rs`
//! resolves. Behaviour lands in Phase 2 per plan §Phase 2 Task 2.1:
//! `from_base58`, `to_base58`, `is_on_curve`, `find_program_address`.
