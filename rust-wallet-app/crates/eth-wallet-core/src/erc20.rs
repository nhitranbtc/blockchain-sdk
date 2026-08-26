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
//! `TransactionRequest.input` (or to feed to `provider.call(req)` for
//! the view methods).

use alloy_network::Ethereum;
use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::{sol, SolCall};

use crate::error::{Error, Result};
use crate::redact_rpc_url;

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

    /// ERC-20 `decimals() view returns (uint8)` — selector 0x313ce567.
    /// `wallet balance --token` calls this via `query_decimals` to
    /// auto-scale the raw `balanceOf` return value into human-readable units.
    function decimals() external view returns (uint8);

    /// ERC-20 `approve(address spender, uint256 value)` — selector 0x095ea7b3.
    function approve(address spender, uint256 value) external returns (bool);

    /// ERC-20 `symbol() view returns (string)` — selector 0x95d89b41. Used by
    /// Issue #360 to resolve the human-readable token symbol so `wallet balance
    /// --token <ADDR>` can print `USDC 15.000000` instead of `15.000000 0x1c7D…`.
    /// `string` (dynamic) — `abi_decode_returns` returns `String`.
    function symbol() external view returns (string);

    /// ERC-20 `name() view returns (string)` — selector 0x06fdde03. Reserved
    /// for Issue #360 acceptance bullet 2 (`query_name` async fn) — paired
    /// with `query_symbol` so callers can surface either identifier.
    function name() external view returns (string);
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

/// Build the calldata for `decimals()`. Returns the 4-byte selector
/// 0x313ce567 with no arguments. Used by Issue #356 to auto-detect
/// token decimal scale before formatting the raw `balanceOf` result.
pub fn decimals_calldata() -> Bytes {
    decimalsCall {}.abi_encode().into()
}

/// Build the calldata for `symbol()`. Returns the 4-byte selector
/// 0x95d89b41 with no arguments. Used by Issue #360 to resolve the
/// human-readable token symbol via `eth_call`. Pairs with
/// [`query_symbol`] for the decode side.
pub fn symbol_calldata() -> Bytes {
    symbolCall {}.abi_encode().into()
}

/// Build the calldata for `name()`. Returns the 4-byte selector
/// 0x06fdde03 with no arguments. Pairs with [`query_name`] for the
/// decode side. Reserved by Issue #360 (parallel to `query_symbol`).
pub fn name_calldata() -> Bytes {
    nameCall {}.abi_encode().into()
}

/// Read the raw ERC-20 `balanceOf(holder)` for `token` via `eth_call`.
/// Returns the raw U256 base-unit balance; caller scales by the token's
/// `decimals()` (Issue #356 auto-detects via [`query_decimals`] or
/// accepts a `--decimals <N>` override).
pub async fn token_balance(
    provider: &RootProvider<Ethereum>,
    token: Address,
    holder: Address,
) -> Result<U256> {
    let calldata = balance_of_calldata(holder);
    let req = TransactionRequest {
        to: Some(TxKind::Call(token)),
        input: calldata.into(),
        ..Default::default()
    };
    let raw = provider
        .call(req)
        .await
        .map_err(|e| Error::Rpc(format!("eth_call balanceOf: {}", redact_rpc_url(&e))))?;
    let balance: U256 =
        balanceOfCall::abi_decode_returns(&raw).map_err(|e| Error::AbiDecodeFailed {
            context: "balanceOf".into(),
            reason: e.to_string(),
        })?;
    Ok(balance)
}

/// Auto-detect an ERC-20 token's `decimals()` via `eth_call`. Returns
/// `Err(Error::Rpc(...))` if the call reverts or the response can't be
/// decoded. Pass `--decimals <N>` to the CLI to skip auto-detect.
pub async fn query_decimals(provider: &RootProvider<Ethereum>, token: Address) -> Result<u8> {
    let calldata = decimals_calldata();
    let req = TransactionRequest {
        to: Some(TxKind::Call(token)),
        input: calldata.into(),
        ..Default::default()
    };
    let raw = provider
        .call(req)
        .await
        .map_err(|e| Error::Rpc(format!("eth_call decimals: {}", redact_rpc_url(&e))))?;
    let decimals: u8 =
        decimalsCall::abi_decode_returns(&raw).map_err(|e| Error::AbiDecodeFailed {
            context: "decimals".into(),
            reason: e.to_string(),
        })?;
    Ok(decimals)
}

/// Resolve an ERC-20 token's `symbol()` via `eth_call`. Returns the
/// human-readable symbol (e.g., `"USDC"`). Issues #360:
///
/// - Selector `0x95d89b41`, no arguments, returns a dynamic `string`.
/// - RPC errors are redacted via [`crate::redact_rpc_url`] (H-6 finding
///   from the L12 security review — see `token_balance` for the same
///   pattern).
/// - Decode failures (contract doesn't actually expose `symbol()` or
///   returns non-UTF-8 bytes) bubble as `Error::AbiDecodeFailed { context:
///   "symbol", reason }` — the caller can fall back to printing the
///   contract address instead.
///
/// Callers SHOULD short-circuit via the bundled token registry first
/// (see [`crate::tokens::lookup_by_address`]) to skip the RPC roundtrip
/// for known tokens.
pub async fn query_symbol(provider: &RootProvider<Ethereum>, token: Address) -> Result<String> {
    let calldata = symbol_calldata();
    let req = TransactionRequest {
        to: Some(TxKind::Call(token)),
        input: calldata.into(),
        ..Default::default()
    };
    let raw = provider
        .call(req)
        .await
        .map_err(|e| Error::Rpc(format!("eth_call symbol: {}", redact_rpc_url(&e))))?;
    symbolCall::abi_decode_returns(&raw).map_err(|e| Error::AbiDecodeFailed {
        context: "symbol".into(),
        reason: e.to_string(),
    })
}

/// Resolve an ERC-20 token's `name()` via `eth_call`. Returns the
/// human-readable name (e.g., `"USD Coin"`). Parallel to [`query_symbol`]
/// per Issue #360 acceptance bullet 2. Same error model: `Error::Rpc`
/// (transport, redacted) or `Error::AbiDecodeFailed { context: "name" }`
/// (decode).
pub async fn query_name(provider: &RootProvider<Ethereum>, token: Address) -> Result<String> {
    let calldata = name_calldata();
    let req = TransactionRequest {
        to: Some(TxKind::Call(token)),
        input: calldata.into(),
        ..Default::default()
    };
    let raw = provider
        .call(req)
        .await
        .map_err(|e| Error::Rpc(format!("eth_call name: {}", redact_rpc_url(&e))))?;
    nameCall::abi_decode_returns(&raw).map_err(|e| Error::AbiDecodeFailed {
        context: "name".into(),
        reason: e.to_string(),
    })
}

/// ERC-20 function selector constants — exposed as `[u8; 4]` for callers
/// that want to verify selector identity without recomputing keccak256.
pub mod selectors {
    /// `keccak256("transfer(address,uint256)")[0..4]` — 0xa9059cbb.
    pub const TRANSFER: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
    /// `keccak256("balanceOf(address)")[0..4]` — 0x70a08231.
    pub const BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];
    /// `keccak256("decimals()")[0..4]` — 0x313ce567.
    pub const DECIMALS: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67];
    /// `keccak256("approve(address,uint256)")[0..4]` — 0x095ea7b3.
    pub const APPROVE: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];
    /// `keccak256("symbol()")[0..4]` — 0x95d89b41. Issue #360.
    pub const SYMBOL: [u8; 4] = [0x95, 0xd8, 0x9b, 0x41];
    /// `keccak256("name()")[0..4]` — 0x06fdde03. Issue #360.
    pub const NAME: [u8; 4] = [0x06, 0xfd, 0xde, 0x03];
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

    // ---- Issue #360 — symbol() / name() bindings ---------------------

    #[test]
    fn symbol_selector_is_0x95d89b41() {
        // Issue #360 AC #1: `sol! { function symbol() external view returns (string); }`
        // produces a selector 0x95d89b41 per keccak256("symbol()")[0..4]. No
        // arguments → calldata is exactly the 4-byte selector.
        let calldata = symbol_calldata();
        assert_eq!(
            calldata.len(),
            4,
            "symbol() takes no args → 4-byte calldata"
        );
        assert_eq!(&calldata[..4], &selectors::SYMBOL);
        assert_eq!(&calldata[..4], &[0x95, 0xd8, 0x9b, 0x41]);
    }

    #[test]
    fn name_selector_is_0x06fdde03() {
        // Parallel test for `name()`. No arguments → 4-byte calldata.
        let calldata = name_calldata();
        assert_eq!(calldata.len(), 4, "name() takes no args → 4-byte calldata");
        assert_eq!(&calldata[..4], &selectors::NAME);
        assert_eq!(&calldata[..4], &[0x06, 0xfd, 0xde, 0x03]);
    }
}
