//! V1 — cargo build + workspace co-build (Q1 EVM-reuse path).
//!
//! If this test compiles and runs, the crate's surface resolves. The
//! acceptance check in #417 / V1 is the cargo build invocation; this
//! test is the integration-test counterpart.

#[test]
fn v1_crate_compiles_and_modules_resolve() {
    // Touch each pub mod to confirm symbol availability.
    use polygon_v1_spike::*;
    let _ = std::mem::size_of::<config::ChainConfig>();
    let _ = std::mem::size_of::<address::DeriveError>();
    let _ = std::mem::size_of::<erc20::MockUSDC::balanceOfCall>();
    let _ = eip712::chain_id(polygon_v1_spike::config::Network::Polygon);
}
