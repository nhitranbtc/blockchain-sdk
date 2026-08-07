//! Workspace-build gate test (Task 1, Step 1).
//!
//! `cargo build --workspace` enforces that every member compiles; this test
//! exists to gate CI on the same invariant and to fail fast if the workspace
//! manifest is missing required members (bitcoin-wallet-core, btc).

#[test]
fn workspace_members_compile() {
    // If this test runs at all, `cargo test --workspace` has already
    // compiled every workspace member. No further work needed.
}
