//! Hand-rolled TRC-20 ABI encoder (Q3).
//!
//! Functions: `transfer(address,uint256)`, `balanceOf(address)`, `decimals()`.
//! ~30 lines; re-evaluate `alloy-sol-types` reuse at v0.3 per plan §Q3.

/// 4-byte function selector = first 4 bytes of keccak256(signature).
pub fn selector(signature: &str) -> [u8; 4] {
    let h = crate::keccak::keccak256(signature.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

/// `transfer(address to, uint256 value)` selector = 0xa9059cbb.
pub const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

/// `balanceOf(address owner)` selector = 0x70a08231.
pub const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

/// `decimals()` selector = 0x313ce567.
pub const DECIMALS_SELECTOR: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67];

/// Encode `transfer(address,uint256)` calldata. 68 bytes total:
/// `selector(4) � to_32_be(32) ‖ value_32_be(32)`.
///
/// `to` = 20-byte address (left-padded to 32 bytes with zeros).
/// `value` = uint256 (left-padded to 32 bytes with zeros).
pub fn encode_transfer(to: &[u8; 20], value: &[u8; 32]) -> [u8; 68] {
    let mut out = [0u8; 68];
    out[0..4].copy_from_slice(&TRANSFER_SELECTOR);
    out[4..36].copy_from_slice(&left_pad_32(to));
    out[36..68].copy_from_slice(value);
    out
}

/// Left-pad a 20-byte address to 32 bytes (high 12 bytes = 0).
fn left_pad_32(addr20: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(addr20);
    out
}

/// Encode `balanceOf(address owner)` calldata. 36 bytes total:
/// `selector(4) ‖ owner_32_be(32)`.
///
/// `owner` = 20-byte address (left-padded to 32 bytes with zeros).
pub fn encode_balance_of(owner: &[u8; 20]) -> [u8; 36] {
    let mut out = [0u8; 36];
    out[0..4].copy_from_slice(&BALANCE_OF_SELECTOR);
    out[4..36].copy_from_slice(&left_pad_32(owner));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_transfer_is_0xa9059cbb() {
        assert_eq!(selector("transfer(address,uint256)"), TRANSFER_SELECTOR);
    }

    #[test]
    fn encode_transfer_layout() {
        let to = [0xab; 20];
        let value = [0u8; 32];
        let calldata = encode_transfer(&to, &value);
        assert_eq!(calldata.len(), 68);
        assert_eq!(&calldata[0..4], &TRANSFER_SELECTOR);
        assert_eq!(&calldata[4..16], &[0u8; 12]); // zero-pad
        assert_eq!(&calldata[16..36], &to);
        assert_eq!(&calldata[36..68], &value);
    }

    #[test]
    fn encode_balance_of_layout() {
        let owner = [0xcd; 20];
        let calldata = encode_balance_of(&owner);
        assert_eq!(calldata.len(), 36);
        assert_eq!(&calldata[0..4], &BALANCE_OF_SELECTOR);
        assert_eq!(&calldata[4..16], &[0u8; 12]); // zero-pad prefix
        assert_eq!(&calldata[16..36], &owner);
    }
}
