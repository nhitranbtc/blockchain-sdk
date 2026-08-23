//! V5 verification — `transferCall { to, value }.abi_encode()` produces
//! calldata whose first 4 bytes are `0xa9059cbb` (ERC-20 `transfer(address,uint256)` selector).
//!
//! Issue #293 — verification item V5.
//!
//! Deterministic: in-memory only. Always runs.

use alloy_primitives::{address, U256};
use alloy_sol_types::{sol, SolCall};

sol! {
    function transfer(address to, uint256 value) external returns (bool);
}

/// Expected 4-byte function selector for ERC-20 `transfer(address,uint256)`.
/// Computed as `keccak256("transfer(address,uint256)")[0..4]` per Solidity ABI spec.
const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

#[test]
fn v5_transfer_call_selector_matches_0xa9059cbb() {
    let call = transferCall {
        to: address!("0x1111111111111111111111111111111111111111"),
        value: U256::from(1_500_000_u64),
    };

    // The selector is the first 4 bytes of the ABI-encoded calldata (SolCall
    // ABI spec). Verify directly from the encoded bytes.
    let calldata = call.abi_encode();
    assert_eq!(
        &calldata[0..4],
        TRANSFER_SELECTOR,
        "ERC-20 transfer selector mismatch: expected 0xa9059cbb",
    );
    eprintln!(
        "[V5] PASS — transferCall selector = 0xa9059cbb (matches EIP-20 transfer(address,uint256))",
    );
}

#[test]
fn v5_transfer_calldata_starts_with_selector() {
    let call = transferCall {
        to: address!("0x2222222222222222222222222222222222222222"),
        value: U256::from(42_u64),
    };

    let calldata = call.abi_encode();
    assert!(
        calldata.len() >= 4,
        "calldata must be at least 4 bytes (selector), got {}",
        calldata.len(),
    );
    assert_eq!(
        &calldata[0..4],
        &TRANSFER_SELECTOR,
        "calldata must start with 0xa9059cbb selector",
    );

    // Sanity: ABI encoding of (address, uint256) = 4 selector + 32 address + 32 value = 68 bytes
    assert_eq!(
        calldata.len(),
        68,
        "calldata should be 68 bytes (4 selector + 32 padded address + 32 padded value)",
    );

    eprintln!(
        "[V5] PASS — transferCall calldata prefix = 0xa9059cbb, length = {} bytes",
        calldata.len(),
    );
}

#[test]
fn v5_transfer_calldata_roundtrip() {
    // Encode → decode → assert round-trip preserves to + value.
    let original_to = address!("0x3333333333333333333333333333333333333333");
    let original_value = U256::from(u64::MAX);

    let call = transferCall {
        to: original_to,
        value: original_value,
    };
    let calldata = call.abi_encode();

    let decoded = transferCall::abi_decode(&calldata).expect("decode round-trip");
    assert_eq!(decoded.to, original_to, "address round-trip");
    assert_eq!(decoded.value, original_value, "value round-trip");

    eprintln!("[V5] PASS — transferCall abi_encode → abi_decode round-trip preserves to + value");
}
