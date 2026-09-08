//! Phase 0 canonical test: proves the crate compiles and links against the
//! pinned anychain stack. Replaced by real address/key/signing tests in Phase 1.
//!
//! The plan (Task 0.3) specifies `assert!(true)` here, but that trips
//! `clippy::assertions_on_constants` under the repo's `-D warnings` gate:
//!   error: `assert!(true)` will be optimized out by the compiler
//! The assertion below carries the same (small) signal without the lint: this
//! test binary only exists if `tron-wallet-core` compiled and linked.

#[test]
fn it_compiles() {
    assert_eq!(env!("CARGO_PKG_NAME"), "tron-wallet-core");
}
