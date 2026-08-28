//! ERC-20 calldata builders (Q3 — `transfer(address,uint256)` calldata shape).
//!
//! `transfer(beta, 100 USDC)` calldata = 4-byte selector (`0xa9059cbb`) +
//! 32-byte padded recipient address + 32-byte padded value. Same wire format
//! as EVM — what makes cross-chain replay protection possible (V10).

use alloy_primitives::{Address, U256};

/// Placeholder for the calldata builder — implemented in Phase 2.
pub fn _transfer_selector_placeholder() -> [u8; 4] {
    [0xa9, 0x05, 0x9c, 0xbb]
}

/// Placeholder for `transfer(to, value)` calldata layout sanity check.
pub fn _transfer_args_placeholder(_to: Address, _value: U256) {}
