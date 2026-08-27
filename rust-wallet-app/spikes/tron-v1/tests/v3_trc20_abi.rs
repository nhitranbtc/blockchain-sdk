//! V3 — TRC-20 ABI (Q3).
//!
//! Plan §Q3: hand-rolled `transfer(address,uint256)` → `0xa9059cbb` selector + 32-byte
//! `to` + 32-byte `value`. Total 68 bytes. Round-trips against canonical ERC-20 vector:
//! keccak256("transfer(address,uint256)") = 0xa9059cbb...d0e5...

use tron_v1_spike::abi::encode_transfer;
use tron_v1_spike::abi::{selector, BALANCE_OF_SELECTOR, DECIMALS_SELECTOR, TRANSFER_SELECTOR};

#[test]
fn v3_canonical_erc20_selectors() {
    // keccak256("transfer(address,uint256)") first 4 bytes = 0xa9059cbb
    assert_eq!(selector("transfer(address,uint256)"), TRANSFER_SELECTOR);
    assert_eq!(TRANSFER_SELECTOR, [0xa9, 0x05, 0x9c, 0xbb]);

    // keccak256("balanceOf(address)") first 4 bytes = 0x70a08231
    assert_eq!(selector("balanceOf(address)"), BALANCE_OF_SELECTOR);
    assert_eq!(BALANCE_OF_SELECTOR, [0x70, 0xa0, 0x82, 0x31]);

    // keccak256("decimals()") first 4 bytes = 0x313ce567
    assert_eq!(selector("decimals()"), DECIMALS_SELECTOR);
    assert_eq!(DECIMALS_SELECTOR, [0x31, 0x3c, 0xe5, 0x67]);
}

#[test]
fn v3_transfer_calldata_68_bytes() {
    let to = [0xab; 20];
    let value = [0u8; 32];
    let calldata = encode_transfer(&to, &value);

    assert_eq!(calldata.len(), 68);
    assert_eq!(&calldata[0..4], &TRANSFER_SELECTOR);

    // `to` is left-padded to 32 bytes (12 zero bytes prefix).
    assert_eq!(&calldata[4..16], &[0u8; 12]);
    assert_eq!(&calldata[16..36], &to);

    // `value` is the last 32 bytes.
    assert_eq!(&calldata[36..68], &value);
}

#[test]
fn v3_transfer_calldata_with_value() {
    let to = [0x12; 20];
    // arbitrary test value (1 USDT = 1_000_000 with 6 decimals)
    let mut value = [0u8; 32];
    value[31] = 0x42;
    let value_u32 = value[31];

    let calldata = encode_transfer(&to, &value);
    assert_eq!(calldata[36..68], value);
    assert_eq!(calldata[36 + 31], value_u32);
}
