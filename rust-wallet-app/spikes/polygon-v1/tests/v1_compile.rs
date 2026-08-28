//! V1 — cargo build + workspace co-build (Q1 EVM-reuse path).
//!
//! If this test compiles and runs, the crate's surface resolves. The
//! acceptance check in #417 / V1 is the cargo build invocation; this
//! test is the integration-test counterpart.

#[test]
fn v1_crate_compiles_and_modules_resolve() {
    // L12 finding: prefer type-asserted bindings over `size_of::<T>()`
    // because size_of doesn't fail if a public type loses its methods.
    // Each binding names a specific surface the rest of the spike depends
    // on; signature drift fails the assertion.
    use polygon_v1_spike::config::Network;
    use polygon_v1_spike::*;
    let _: fn(&str, Network) -> Result<alloy_primitives::Address, _> = address::derive_evm_address;
    let _ = std::mem::size_of::<erc20::MockUSDC::balanceOfCall>();
    let _: [u8; 4] = eip712::CHAIN_ID_POLYGON.to_be_bytes()[..4]
        .try_into()
        .unwrap();
}
