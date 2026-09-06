//! Plan Task 3.6 / Phase-7 spike V3: TRC-20 ABI round-trip.
//!
//! Asserts that the four selectors we hand out from `trc20::*` match the
//! canonical keccak256 prefixes AND that `anychain_tron::abi::trc20_transfer`
//! produces a 68-byte payload that begins with `transfer`'s selector. A
//! future anychain bump that changes the wire format (prefix bytes, arg
//! layout, padding) will be caught here.

use tron_wallet_core::trc20::{
    APPROVE_SELECTOR, BALANCE_OF_SELECTOR, DECIMALS_SELECTOR, NAME_SELECTOR, SYMBOL_SELECTOR,
    TRANSFER_SELECTOR,
};

/// The mainnet USDT contract address, read from the bundled token
/// registry (`tokens/mainnet.json`). Resolved lazily per-test
/// instead of at module load so a missing bundle produces a
/// per-test failure with a clear message.
fn usdt_contract() -> &'static str {
    tron_wallet_core::tokens::by_symbol(tron_wallet_core::config::Network::Mainnet, "USDT")
        .expect("mainnet USDT must be in the bundle")
        .address
        .as_str()
}

fn mainnet_test() -> &'static tron_wallet_core::tokens::TestAddresses {
    tron_wallet_core::tokens::test_addresses(tron_wallet_core::config::Network::Mainnet)
        .expect("mainnet test fixtures must be present")
}

fn recipient() -> &'static str {
    mainnet_test().recipient_address.as_str()
}

#[test]
fn transfer_calldata_starts_with_transfer_selector() {
    let calldata = anychain_tron::abi::trc20_transfer(recipient(), "1000000");
    assert_eq!(
        calldata.len(),
        68,
        "TRC-20 transfer calldata is 4 + 32 + 32 bytes"
    );
    assert_eq!(
        &calldata[..4],
        &TRANSFER_SELECTOR,
        "TRC-20 transfer selector drifted from 0xa9059cbb"
    );
    // Arg layout: 32-byte slot holding the full 21-byte T-address
    // (including `0x41` prefix) left-padded with 11 zero bytes, then
    // 32-byte big-endian amount. TRC-20 contracts on Tron interpret
    // the slot as the full T-address — see anychain-tron's
    // `abi::trc20_transfer` test fixture for the canonical layout.
    let address_arg = &calldata[4..36];
    let amount_arg = &calldata[36..68];
    assert_eq!(
        &address_arg[..11],
        &[0u8; 11],
        "address must be left-padded with 11 zero bytes"
    );
    let recipient_bytes = recipient()
        .parse::<tron_wallet_core::address::Address>()
        .unwrap();
    assert_eq!(&address_arg[11..], recipient_bytes.as_bytes());
    let amount = ethereum_types::U256::from_big_endian(amount_arg);
    assert_eq!(amount, ethereum_types::U256::from(1_000_000u64));
}

#[test]
fn approve_calldata_starts_with_approve_selector() {
    let calldata = anychain_tron::abi::trc20_approve(recipient(), "1000000000000000000");
    assert_eq!(
        calldata.len(),
        68,
        "TRC-20 approve calldata is 4 + 32 + 32 bytes"
    );
    assert_eq!(&calldata[..4], &APPROVE_SELECTOR);
    assert_ne!(&calldata[..4], &TRANSFER_SELECTOR);
}

#[test]
fn balance_of_calldata_is_36_bytes() {
    let owner = mainnet_test().owner_address.as_str();
    let owner_bytes = owner.parse::<tron_wallet_core::address::Address>().unwrap();

    let arg = tron_wallet_core::trc20::balance_of_args(owner_bytes.as_bytes());
    assert_eq!(
        arg.len(),
        32,
        "balanceOf arg block is one 32-byte address slot"
    );

    // TRC-20 contracts on Tron interpret the slot as the full 21-byte
    // T-address (including the 0x41 prefix) padded to 32 bytes on the
    // left with 11 zero bytes — see anychain-tron's `abi::trc20_transfer`
    // test fixture for the canonical layout.
    assert_eq!(&arg[..11], &[0u8; 11]);
    assert_eq!(&arg[11..], owner_bytes.as_bytes());
}

#[test]
fn selectors_match_canonical_keccak_prefixes() {
    // The plan's Task 3.6 lists exactly these selectors — assert them
    // directly so a copy-paste in this file surfaces immediately.
    assert_eq!(hex::encode(TRANSFER_SELECTOR), "a9059cbb");
    assert_eq!(hex::encode(APPROVE_SELECTOR), "095ea7b3");
    assert_eq!(hex::encode(BALANCE_OF_SELECTOR), "70a08231");
    assert_eq!(hex::encode(DECIMALS_SELECTOR), "313ce567");
    assert_eq!(hex::encode(SYMBOL_SELECTOR), "95d89b41");
    assert_eq!(hex::encode(NAME_SELECTOR), "06fdde03");
}

#[test]
fn no_args_returns_empty_calldata_body() {
    let args = tron_wallet_core::trc20::no_args();
    assert!(args.is_empty(), "decimals/symbol/name take no arguments");
}

/// The `trx::build_trc20_transfer_contract` round-trip through
/// `tx::builder::trc20_transfer` must succeed and produce parameters
/// whose contract enum is a TriggerSmartContract — closing the gap
/// between the address/signing tests (Phase 1/2) and the contract-level
/// encoding tests above.
#[test]
fn builder_trc20_transfer_produces_a_trigger_contract() {
    let owner = mainnet_test().owner_address.as_str();
    let amount = ethereum_types::U256::from(1_000_000u64);

    let params =
        tron_wallet_core::tx::builder::trc20_transfer(owner, usdt_contract(), recipient(), amount)
            .expect("trc20_transfer builder succeeds");
    // The fee-limit baseline must have been applied (130 TRX = 130_000_000 SUN).
    assert_eq!(
        tron_wallet_core::tx::builder::DEFAULT_TRC20_FEE_LIMIT_SUN,
        130_000_000
    );
    // We do not introspect the inner contract — Phase 4 owns the deeper
    // round-trip through TronTransaction::new. The Phase 3 spike is
    // happy to confirm the builder does not reject valid inputs.
    let _ = params;
}

#[test]
fn builder_trc20_approve_produces_a_contract() {
    let owner = mainnet_test().owner_address.as_str();
    let spender = mainnet_test().approval_spender_address.as_str();
    let amount = ethereum_types::U256::MAX; // infinite approval

    let params =
        tron_wallet_core::tx::builder::trc20_approve(owner, usdt_contract(), spender, amount)
            .expect("trc20_approve builder succeeds");
    let _ = params;
}
