// SPDX-License-Identifier: MIT
//
// Minimal fixed-supply TRC-20 token for Phase 4 §4.2 integration tests.
// Compiled inside the spawned tronbox/tre container via `npx solc@0.8.20`.
// Mirrors the ERC-20 surface; `decimals=6` to match USDT-TRC20 semantics.
//
// Phase 4 §4.2 row matrix depends on:
//   - transfer(address,uint256)         — rows 2, 3, 6, 7, 8
//   - balanceOf(address)               — rows 2, 3, 8 verification
//   - approve(address,uint256)         — row 4
//   - allowance(address,address)       — row 4 verification
//   - decimals()                       — symbol/decimals sanity
//
// Total supply = 1,000,000 * 10^6 raw (1M tokens, 6-dec). Minted to deployer
// in the constructor; deployer then funds test senders via transfer().

pragma solidity ^0.8.0;

contract MockTRC20 {
    string public name = "Mock USDT";
    string public symbol = "mUSDT";
    uint8 public decimals = 6;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);

    constructor() {
        totalSupply = 1_000_000 * 10 ** uint256(decimals);
        balanceOf[msg.sender] = totalSupply;
        emit Transfer(address(0), msg.sender, totalSupply);
    }

    function transfer(address to, uint256 value) external returns (bool) {
        require(balanceOf[msg.sender] >= value, "insufficient");
        balanceOf[msg.sender] -= value;
        balanceOf[to] += value;
        emit Transfer(msg.sender, to, value);
        return true;
    }

    function approve(address spender, uint256 value) external returns (bool) {
        allowance[msg.sender][spender] = value;
        emit Approval(msg.sender, spender, value);
        return true;
    }

    function transferFrom(
        address from,
        address to,
        uint256 value
    ) external returns (bool) {
        require(balanceOf[from] >= value, "insufficient");
        require(allowance[from][msg.sender] >= value, "allowance");
        balanceOf[from] -= value;
        balanceOf[to] += value;
        allowance[from][msg.sender] -= value;
        emit Transfer(from, to, value);
        return true;
    }
}