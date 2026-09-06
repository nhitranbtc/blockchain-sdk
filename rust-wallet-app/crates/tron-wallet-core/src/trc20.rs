//! TRC-20 (EIP-20-shaped) token view calls + ABI encode/decode helpers.
//!
//! Plan Task 3.4 (`balanceOf` + `decimals` + `symbol`) and the encoding half
//! of Task 3.3 (`triggerconstantcontract` calldata). The mutation halves —
//! `transfer` / `approve` contract builders — live in
//! [`crate::tx::builder`], where the rest of the transaction-parameter
//! helpers are concentrated.
//!
//! ## Wire-format contract (plan Task 3.3, corrected via #410 + live-revert fix)
//!
//! TronGrid's `wallet/triggerconstantcontract` endpoint takes:
//!
//! - `function_selector`: the **human-readable Solidity signature** of the
//!   function being called (e.g. `"decimals()"`, `"balanceOf(address)"`).
//!   The server resolves the signature against the contract's published
//!   ABI to derive the 4-byte selector at call time.
//! - `parameter`: hex-encoded ABI argument block (no selector prefix).
//! - `owner_address`: required even for view calls (server uses it as the
//!   simulated caller). Pass any valid T-address for read-only paths.
//!
//! Earlier we sent a 4-byte hex selector in `function_selector` — the
//! server then derived a different 4-byte prefix from those hex bytes,
//! the simulated call reverted with `REVERT opcode executed`, and
//! `constant_result` came back empty. Sending the signature string fixes
//! the call (verified live against mainnet USDT).
//!
//! ## ABI encoding rules we rely on
//!
//! - Static types (`uint256`, `address`) are encoded as 32-byte slots, big
//!   endian, left-padded with zeros.
//! - Dynamic types (`string`, `bytes`) are encoded as `(offset, length, data)`
//!   where `offset` and `length` are themselves 32-byte slots.
//!
//! The decoder in this module only handles the subset TRC-20 tokens emit:
//! single `uint256` for `balanceOf` / `decimals`, and a single `string` for
//! `symbol` / `name`. A general ABI decoder is out of scope for v0.1 — we
//! only read what we also encode, and the four signatures below are the
//! ones the bundled registry exposes.

use anychain_tron::abi;
use ethereum_types::U256;

use crate::chain::TronGridClient;
use crate::error::{Error, Result};

/// `transfer(address,uint256)` — used to build TRC-20 transfer calldata.
pub const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
/// Solidity signature for `transfer(address,uint256)`. Pass this to
/// [`crate::chain::TronGridClient::trigger_constant_contract`] /
/// `estimate_energy` as the `function` argument; the server resolves it
/// against the contract's ABI to derive the actual selector.
pub const TRANSFER_SIGNATURE: &str = "transfer(address,uint256)";

/// `approve(address,uint256)` — used to build TRC-20 approval calldata.
pub const APPROVE_SELECTOR: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];
/// Solidity signature for `approve(address,uint256)`.
pub const APPROVE_SIGNATURE: &str = "approve(address,uint256)";

/// `balanceOf(address)` — view call that returns the holder's token balance.
pub const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];
/// Solidity signature for `balanceOf(address)`.
pub const BALANCE_OF_SIGNATURE: &str = "balanceOf(address)";

/// `decimals()` — view call that returns the token's decimal precision.
pub const DECIMALS_SELECTOR: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67];
/// Solidity signature for `decimals()`.
pub const DECIMALS_SIGNATURE: &str = "decimals()";

/// `symbol()` — view call that returns the token's ticker symbol.
pub const SYMBOL_SELECTOR: [u8; 4] = [0x95, 0xd8, 0x9b, 0x41];
/// Solidity signature for `symbol()`.
pub const SYMBOL_SIGNATURE: &str = "symbol()";

/// `name()` — view call that returns the token's human-readable name.
pub const NAME_SELECTOR: [u8; 4] = [0x06, 0xfd, 0xde, 0x03];
/// Solidity signature for `name()`.
pub const NAME_SIGNATURE: &str = "name()";

/// 32-byte slot size used by the ABI for both static and dynamic encodings.
const ABI_WORD: usize = 32;

/// Encode a TRON address into a 32-byte ABI slot, left-padded with zeros.
///
/// TRON T-base58check addresses are 21 bytes (the `0x41` prefix plus 20
/// account bytes). TRC-20 contracts on Tron interpret the address slot
/// as the *full* 21-byte T-address — the `0x41` prefix stays, with 11
/// zero bytes padding on the left to fill the 32-byte slot. anychain's
/// `abi::trc20_transfer` confirms this in its test fixture:
///
/// ```text
/// a9059cbb 0000000000000000000000 41 436d74fc1577266b7290b85801145d9c5287e194
/// ```
///
/// Stripping the prefix would break a Tron VM contract that compares
/// the slot against an on-chain T-address — the contract sees the full
/// 21 bytes, not the 20-byte Ethereum account slice.
fn encode_address_arg(addr_bytes: &[u8]) -> [u8; ABI_WORD] {
    debug_assert_eq!(addr_bytes.len(), crate::address::ADDRESS_LEN);
    let mut slot = [0u8; ABI_WORD];
    slot[11..].copy_from_slice(addr_bytes);
    slot
}

/// Encode a `uint256` value as a 32-byte big-endian slot.
///
/// Exposed publicly so callers that hand-roll a TRC-20 call (e.g. a custom
/// selector outside the bundled ones) get the same ABI encoding rules
/// the crate uses internally.
pub fn encode_uint256_arg(value: U256) -> [u8; ABI_WORD] {
    let mut slot = [0u8; ABI_WORD];
    value.to_big_endian(&mut slot);
    slot
}

/// `balanceOf(address)` calldata body — the argument block only (no
/// selector). The 4-byte selector is passed separately to TronGrid; the
/// server prepends it.
pub fn balance_of_args(owner_bytes: &[u8]) -> [u8; ABI_WORD] {
    encode_address_arg(owner_bytes)
}

/// Calldata body for selectors that take no arguments (`decimals`, `symbol`,
/// `name`). Returned as a `Vec` so callers can borrow it without a slice
/// conversion.
pub fn no_args() -> Vec<u8> {
    Vec::new()
}

/// Read the balance of `owner` against `contract`.
///
/// Decodes the 32-byte response as a `uint256`. The value is in the
/// token's smallest unit (e.g. USDT has 6 decimals — divide by 10^6 for
/// the human-readable amount).
pub async fn balance_of(rpc: &TronGridClient, contract: &str, owner: &str) -> Result<U256> {
    let owner_addr: crate::address::Address = owner.parse()?;
    let arg = balance_of_args(owner_addr.as_bytes());

    let response = rpc
        .trigger_constant_contract(contract, owner, "balanceOf(address)", &arg)
        .await?;

    let raw = response.constant_result_bytes()?;
    decode_uint256(&raw)
        .map_err(|e| Error::NodeResponse(format!("balanceOf({contract}, {owner}) decode: {e}")))
}

/// Read `decimals()` from `contract`. Returns 6 for USDT/USDC, 18 for
/// TUSD/USDD. The bundled registry already records this value; the live
/// call exists so a token can be sanity-checked before the registry is
/// updated (and to catch a misconfigured registry entry).
///
/// `owner` is the simulated caller TronGrid requires on every
/// `triggerconstantcontract` request — for a view call like `decimals`
/// the value does not affect the result, but the server rejects with
/// `owner_address isn't set` if the field is empty.
pub async fn decimals(rpc: &TronGridClient, contract: &str, owner: &str) -> Result<u8> {
    let response = rpc
        .trigger_constant_contract(contract, owner, "decimals()", &no_args())
        .await?;

    let raw = response.constant_result_bytes()?;
    let value = decode_uint256(&raw)
        .map_err(|e| Error::NodeResponse(format!("decimals({contract}) decode: {e}")))?;
    // The contract returns the value as a uint256; the low byte is the
    // decimal precision. 0..=255 fits in a u8 by ABI definition — anything
    // else means the contract is non-conformant and the caller should fail
    // loudly rather than truncate.
    let lo: u8 = value.try_into().map_err(|_| {
        Error::NodeResponse(format!("decimals({contract}) out of u8 range: {value}"))
    })?;
    Ok(lo)
}

/// Read `symbol()` from `contract`. Returns the ABI-decoded string (e.g.
/// `"USDT"`).
///
/// See [`decimals`] for why `owner` is required — TronGrid rejects the
/// request without it.
pub async fn symbol(rpc: &TronGridClient, contract: &str, owner: &str) -> Result<String> {
    let response = rpc
        .trigger_constant_contract(contract, owner, "symbol()", &no_args())
        .await?;

    let raw = response.constant_result_bytes()?;
    decode_abi_string(&raw, "symbol")
        .map_err(|e| Error::NodeResponse(format!("symbol({contract}) decode: {e}")))
}

/// Read `name()` from `contract`. Mirrors [`symbol`] for the full display
/// name (e.g. `"Tether USD"`).
///
/// See [`decimals`] for why `owner` is required — TronGrid rejects the
/// request without it.
pub async fn name(rpc: &TronGridClient, contract: &str, owner: &str) -> Result<String> {
    let response = rpc
        .trigger_constant_contract(contract, owner, "name()", &no_args())
        .await?;

    let raw = response.constant_result_bytes()?;
    decode_abi_string(&raw, "name")
        .map_err(|e| Error::NodeResponse(format!("name({contract}) decode: {e}")))
}

/// Re-export the mutation-half calldata builders from `anychain_tron::abi`
/// so call sites for the encode side (`transfer`, `approve`) and the view
/// side (`balanceOf`, `decimals`, `symbol`) live behind the same path.
pub use abi::trc20_approve as encode_approve_calldata;
pub use abi::trc20_transfer as encode_transfer_calldata;

/// Decode a 32-byte ABI slot as a `uint256`. Returns an error if the input
/// is short — the value is always encoded left-padded, so a short input is
/// always malformed.
fn decode_uint256(bytes: &[u8]) -> Result<U256> {
    if bytes.len() < ABI_WORD {
        return Err(Error::NodeResponse(format!(
            "expected {ABI_WORD}-byte uint256, got {}",
            bytes.len()
        )));
    }
    Ok(U256::from_big_endian(&bytes[..ABI_WORD]))
}

/// Decode an ABI-encoded string. Layout for a single-string return:
///
/// ```text
/// [0..32]    offset to the string data (typically 0x20)
/// [32..64]   string length in bytes
/// [64..64+N] UTF-8 bytes, right-padded with zeros to a 32-byte boundary
/// ```
///
/// The `selector_label` is included in error messages so a mis-shapen
/// response can be traced back to the call that produced it.
fn decode_abi_string(bytes: &[u8], selector_label: &str) -> Result<String> {
    if bytes.len() < 2 * ABI_WORD {
        return Err(Error::NodeResponse(format!(
            "{selector_label}() ABI string missing offset+length header ({} bytes)",
            bytes.len()
        )));
    }
    let offset = U256::from_big_endian(&bytes[..ABI_WORD]).as_usize();
    let length = U256::from_big_endian(&bytes[ABI_WORD..2 * ABI_WORD]).as_usize();

    if offset != ABI_WORD {
        // The standard layout always uses offset 0x20 (32). Anything else
        // would mean the contract returned a tuple, not a bare string —
        // not a TRC-20 shape we recognise.
        return Err(Error::NodeResponse(format!(
            "{selector_label}() non-standard ABI string offset {offset} (expected 32)"
        )));
    }

    let data_start = 2 * ABI_WORD;
    let data_end = data_start
        .checked_add(length)
        .ok_or_else(|| Error::NodeResponse(format!("{selector_label}() length overflow")))?;
    if bytes.len() < data_end {
        return Err(Error::NodeResponse(format!(
            "{selector_label}() body short by {} bytes",
            data_end - bytes.len()
        )));
    }

    core::str::from_utf8(&bytes[data_start..data_end])
        .map(|s| s.to_owned())
        .map_err(|e| Error::NodeResponse(format!("{selector_label}() not UTF-8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_of_selector_is_correct() {
        // 0x70a08231 = keccak256("balanceOf(address)")[..4]
        assert_eq!(hex::encode(BALANCE_OF_SELECTOR), "70a08231");
    }

    #[test]
    fn decimals_selector_is_correct() {
        assert_eq!(hex::encode(DECIMALS_SELECTOR), "313ce567");
    }

    #[test]
    fn symbol_selector_is_correct() {
        assert_eq!(hex::encode(SYMBOL_SELECTOR), "95d89b41");
    }

    #[test]
    fn name_selector_is_correct() {
        assert_eq!(hex::encode(NAME_SELECTOR), "06fdde03");
    }

    #[test]
    fn transfer_selector_is_correct() {
        assert_eq!(hex::encode(TRANSFER_SELECTOR), "a9059cbb");
    }

    #[test]
    fn approve_selector_is_correct() {
        assert_eq!(hex::encode(APPROVE_SELECTOR), "095ea7b3");
    }

    /// 32-byte slot for `0x41436d74fc1577266b7290b85801145d9c5287e19`:
    /// the full 21-byte T-address (including `0x41` prefix) lives in the
    /// low 21 bytes of the slot, with 11 zero bytes padding on the left.
    /// TRC-20 contracts on Tron interpret the slot as the full T-address.
    #[test]
    fn address_arg_slot_left_pads_with_eleven_zeros() {
        let bytes = [
            0x41, 0x43, 0x6d, 0x74, 0xfc, 0x15, 0x77, 0x26, 0xb7, 0x29, 0x0b, 0x85, 0x80, 0x11,
            0x45, 0xd9, 0xc5, 0x28, 0x7e, 0x19, 0x40,
        ];
        let slot = encode_address_arg(&bytes);
        assert_eq!(&slot[..11], &[0u8; 11]);
        assert_eq!(&slot[11..], &bytes);
    }

    #[test]
    fn uint256_slot_is_big_endian() {
        let value = U256::from(0x1234u64);
        let slot = encode_uint256_arg(value);
        assert_eq!(slot[30], 0x12);
        assert_eq!(slot[31], 0x34);
        assert!(slot[..30].iter().all(|&b| b == 0));
    }

    #[test]
    fn decode_uint256_handles_short_input() {
        assert!(decode_uint256(&[0u8; 31]).is_err());
        assert!(decode_uint256(&[]).is_err());
    }

    #[test]
    fn decode_uint256_round_trips_a_large_value() {
        let value = U256::from_dec_str("1000000000000000000").unwrap(); // 1e18
        let mut bytes = [0u8; 32];
        value.to_big_endian(&mut bytes);
        assert_eq!(decode_uint256(&bytes).unwrap(), value);
    }

    #[test]
    fn decode_abi_string_round_trips_short_string() {
        // ABI encoding of "USDT" (4 bytes) — 32-byte offset, 32-byte length, 32-byte data padded.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0u8; 28]);
        bytes.extend_from_slice(&[0, 0, 0, 0x20]); // offset = 32
        bytes.extend_from_slice(&[0u8; 28]);
        bytes.extend_from_slice(&[0, 0, 0, 4]); // length = 4
        bytes.extend_from_slice(b"USDT");
        bytes.extend_from_slice(&[0u8; 28]); // padding to 32-byte boundary

        assert_eq!(decode_abi_string(&bytes, "symbol").unwrap(), "USDT");
    }

    #[test]
    fn decode_abi_string_rejects_non_standard_offset() {
        let bytes = vec![0u8; 64];
        // offset = 0 (instead of 0x20)
        assert!(decode_abi_string(&bytes, "symbol").is_err());
    }

    #[test]
    fn decode_abi_string_rejects_truncated_body() {
        // offset + length declared but body shorter than length.
        let mut bytes = vec![0u8; 64];
        bytes[31] = 0x20; // offset 32
        bytes[63] = 100; // length 100
        assert!(decode_abi_string(&bytes, "symbol").is_err());
    }
}
