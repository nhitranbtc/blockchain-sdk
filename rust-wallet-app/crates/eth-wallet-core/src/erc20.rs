//! ERC-20 calldata builders — Issue #306 (Task 7).
//!
//! V5 spike evidence: `rust-wallet-app/spikes/alloy-v1/tests/v5_erc20_calldata.rs`.
//! Selector reference per EIP-20 + keccak256("transfer(address,uint256)")[0..4] etc.
//!
//! Provides three calldata builders for the most common ERC-20 ops:
//!   - `transfer(address,uint256)`        — selector 0xa9059cbb
//!   - `balanceOf(address) (view returns uint256)` — selector 0x70a08231
//!   - `approve(address,uint256)`         — selector 0x095ea7b3
//!
//! All three return `alloy_primitives::Bytes` ready to attach to a
//! `TransactionRequest.input` (or to feed to `provider.call(&req)` for
//! the view methods).

use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{sol, SolCall};

// Sol! macro for typed ERC-20 surface. Auto-generates `*Call` structs
// with `abi_encode()` + `decode()` methods under the `SolCall` trait.
//
// **Important:** Task 7 uses 3 individual `sol!` blocks rather than one
// nested contract — v0.2 RPC ergonomic preference (per Issue #306 plan
// step 2: each call type as a separate inline function).
sol! {
    /// ERC-20 `transfer(address,uint256)` — selector 0xa9059cbb.
    function transfer(address to, uint256 value) external returns (bool);

    /// ERC-20 `balanceOf(address) view returns (uint256)` — selector
    /// 0x70a08231. Note `view` keyword in the type signature.
    function balanceOf(address account) external view returns (uint256);

    /// ERC-20 `approve(address spender, uint256 value)` — selector 0x095ea7b3.
    function approve(address spender, uint256 value) external returns (bool);
}

/// Build the calldata for `transfer(to, value)`. Returns the 4-byte
/// selector 0xa9059cbb followed by the 32-byte-padded `to` address +
/// 32-byte-padded `value` (BE).
pub fn transfer_calldata(to: Address, value: U256) -> Bytes {
    transferCall { to, value }.abi_encode().into()
}

/// Build the calldata for `balanceOf(owner)`. Returns the 4-byte
/// selector 0x70a08231 followed by the 32-byte-padded `owner` address.
pub fn balance_of_calldata(owner: Address) -> Bytes {
    balanceOfCall { account: owner }.abi_encode().into()
}

/// Build the calldata for `approve(spender, value)`. Returns the
/// 4-byte selector 0x095ea7b3 followed by the 32-byte-padded
/// `spender` + 32-byte-padded `value` (BE).
pub fn approve_calldata(spender: Address, value: U256) -> Bytes {
    approveCall { spender, value }.abi_encode().into()
}

/// ERC-20 function selector constants — exposed as `u32` for callers
/// that want to verify selector identity without recomputing keccak256.
pub mod selectors {
    /// `keccak256("transfer(address,uint256)")[0..4]` — 0xa9059cbb.
    pub const TRANSFER: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
    /// `keccak256("balanceOf(address)")[0..4]` — 0x70a08231.
    pub const BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];
    /// `keccak256("approve(address,uint256)")[0..4]` — 0x095ea7b3.
    pub const APPROVE: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];
}

#[cfg(test)]
mod tests {
    use super::*;

    // `Address::from([u8; 20])` is const-fn in 1.8 but `Address::from_str` is the
    // only const-evaluable path in this crate's toolchain — use hex-literal
    // strings. Each string is 40 hex chars after the 0x prefix (20 bytes).
    fn rec() -> Address {
        "0xabababababababababababababababababababab"
            .parse()
            .expect("a")
    }
    // Simpler: lift the addresses into `lazy_static`-style `OnceLock`s, or
    // just use literal `"0x..".parse().unwrap()` at the top of each test.
    // For now, parse once and `unwrap` lazily via a one-shot closure in each
    // test (avoids pulling in lazy_static + keeps tests concise).
    fn owner_addr() -> Address {
        "0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
            .parse()
            .expect("o")
    }
    fn spender_addr() -> Address {
        "0xffffffffffffffffffffffffffffffffffffffff"
            .parse()
            .expect("s")
    }

    #[test]
    fn transfer_selector_is_0xa9059cbb() {
        let calldata = transfer_calldata(rec(), U256::from(1_500_000u64));
        assert_eq!(&calldata[..4], &selectors::TRANSFER);
        assert_eq!(&calldata[..4], &[0xa9, 0x05, 0x9c, 0xbb]);
    }

    #[test]
    fn balance_of_selector_is_0x70a08231() {
        let calldata = balance_of_calldata(owner_addr());
        assert_eq!(&calldata[..4], &selectors::BALANCE_OF);
        assert_eq!(&calldata[..4], &[0x70, 0xa0, 0x82, 0x31]);
    }

    #[test]
    fn approve_selector_is_0x095ea7b3() {
        let calldata = approve_calldata(spender_addr(), U256::from(u128::MAX));
        assert_eq!(&calldata[..4], &selectors::APPROVE);
        assert_eq!(&calldata[..4], &[0x09, 0x5e, 0xa7, 0xb3]);
    }

    #[test]
    fn transfer_calldata_has_correct_length() {
        let calldata = transfer_calldata(rec(), U256::from(42u64));
        assert_eq!(calldata.len(), 68, "selector(4) + address(32) + value(32)");
    }

    #[test]
    fn round_trip_decode_transfer() {
        let value = U256::from(1_500_000u128);
        let calldata = transfer_calldata(rec(), value);
        let decoded = transferCall::abi_decode(&calldata).expect("decode");
        assert_eq!(decoded.to, rec());
        assert_eq!(decoded.value, U256::from(1_500_000u128));
    }

    #[test]
    fn round_trip_decode_balance_of() {
        let calldata = balance_of_calldata(owner_addr());
        let decoded = balanceOfCall::abi_decode(&calldata).expect("decode");
        assert_eq!(decoded.account, owner_addr());
    }

    #[test]
    fn round_trip_decode_approve() {
        let value = U256::from(123_456_789u64);
        let calldata = approve_calldata(spender_addr(), value);
        let decoded = approveCall::abi_decode(&calldata).expect("decode");
        assert_eq!(decoded.spender, spender_addr());
        assert_eq!(decoded.value, value);
    }
}
