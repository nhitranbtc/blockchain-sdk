//! ERC-20 surface (Q3 — transfer calldata + Anvil-fork deploy/transfer/balanceOf).

use alloy_primitives::U256;
use alloy_sol_types::sol;

/// Canonical selector for `transfer(address,uint256)`.
/// keccak256("transfer(address,uint256)")[..4].
pub fn transfer_selector() -> [u8; 4] {
    [0xa9, 0x05, 0x9c, 0xbb]
}

/// Canonical selector for `balanceOf(address)`.
/// keccak256("balanceOf(address)")[..4].
pub fn balance_of_selector() -> [u8; 4] {
    [0x70, 0xa0, 0x82, 0x31]
}

sol! {
    contract MockUSDC {
        constructor(uint256 initialSupply) {
            _balances[msg.sender] = initialSupply;
        }

        mapping(address => uint256) public _balances;

        function balanceOf(address account) external view returns (uint256) {
            return _balances[account];
        }

        function transfer(address to, uint256 value) external returns (bool) {
            require(_balances[msg.sender] >= value, "insufficient");
            _balances[msg.sender] -= value;
            _balances[to] += value;
            return true;
        }
    }
}

/// USDC decimal precision (6). Pinned for V9 + use_case.
pub const USDC_DECIMALS: u8 = 6;

/// 100 USDC in raw units (6-decimal). Pinned for use_case.
pub const ONE_HUNDRED_USDC_RAW: u64 = 100_000_000; // 100 * 10^6

/// Build the U256 raw value for `n` USDC at 6 decimals.
pub fn usdc_to_raw(units: u64) -> U256 {
    U256::from(units) * U256::from(10_u64).pow(U256::from(USDC_DECIMALS as u64))
}
