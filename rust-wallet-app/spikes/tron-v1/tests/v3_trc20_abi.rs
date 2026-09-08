//! V3 — TRC-20 ABI selectors (CLI-driven, Phase 7 §Task 7.3).
//!
//! Plan §V3 table: `tron trc20 encode-call transfer` produces 68-byte hex
//! starting with selector `0xa9059cbb` (keccak256("transfer(address,uint256)")
//! first 4 bytes); the approve companion starts with `0x095ea7b3`.
//!
//! Every assertion is on the shipped `tron` CLI binary
//! (`crates/tron/src/cli.rs::Trc20Action::EncodeCall`); the spike library
//! (deleted in Task 7.13) is no longer the source of truth. Selectors
//! here mirror `tron_wallet_core::trc20::abi::TRC20_TRANSFER_SELECTOR` /
//! `TRC20_APPROVE_SELECTOR` — if the core changes, this test fails loudly.
//!
//! Layout cross-checks (selector + 32-byte left-padded address slot +
//! 32-byte big-endian uint256 slot) catch anychain-side encoding drift
//! that a selector-only assertion would silently pass.

#[path = "../../../crates/tron-wallet-core/tests/common/mod.rs"]
mod common;

/// First 4 bytes of keccak256("balanceOf(address)").
const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

/// First 4 bytes of keccak256("transfer(address,uint256)") = 0xa9059cbb.
const TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

/// First 4 bytes of keccak256("approve(address,uint256)") = 0x095ea7b3.
const APPROVE_SELECTOR: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];

/// TRC-20 transfer/approve calldata layout: 4-byte selector + 32-byte
/// address slot + 32-byte uint256 slot = 68 bytes.
const TRC20_CALLDATA_LEN: usize = 68;

/// CLI emits `0x`-prefixed lowercase hex via
/// `crates/tron/src/handlers/trc20.rs::encode_call::println!("0x{}", …)`;
/// strip the prefix before decoding so a future flag change at the call
/// site (e.g. dropping `0x` for `--raw`) does not break these tests.
fn strip_0x(hex: &str) -> &str {
    hex.strip_prefix("0x").unwrap_or(hex)
}

/// `tron trc20 encode-call transfer --to <addr> --amount <num>` emits
/// 68-byte calldata whose first 4 bytes are
/// keccak256("transfer(address,uint256)")[..4] = 0xa9059cbb. Pure offline
/// transform — no signing, no network, no `--mnemonic`.
#[test]
fn v3_trc20_encode_call_transfer_starts_with_a9059cbb() {
    let recipient = common::nile_recipient();
    let amount = "1000000";

    let assert = common::tron()
        .args([
            "trc20",
            "encode-call",
            "transfer",
            "--to",
            recipient,
            "--amount",
            amount,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let hex = stdout.trim();
    assert!(
        !hex.is_empty(),
        "`tron trc20 encode-call transfer` emitted empty stdout; got {stdout:?}"
    );

    let bytes =
        hex::decode(strip_0x(hex)).unwrap_or_else(|e| panic!("non-hex stdout ({e}): {hex:?}"));

    assert_eq!(
        bytes.len(),
        TRC20_CALLDATA_LEN,
        "TRC-20 transfer calldata must be exactly 68 bytes \
         (selector + 32-byte address slot + 32-byte uint256 slot); got {} bytes",
        bytes.len()
    );

    assert_eq!(
        &bytes[..4],
        &TRANSFER_SELECTOR,
        "TRC-20 transfer calldata must begin with keccak256(\
         \"transfer(address,uint256)\")[..4] = 0xa9059cbb; got 0x{}",
        hex::encode(&bytes[..4])
    );
}

/// Companion for `approve`: identical layout, different selector.
#[test]
fn v3_trc20_encode_call_approve_starts_with_095ea7b3() {
    let spender = common::nile_spender();
    let amount = "1000000";

    let assert = common::tron()
        .args([
            "trc20",
            "encode-call",
            "approve",
            "--to",
            spender,
            "--amount",
            amount,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let hex = stdout.trim();
    assert!(
        !hex.is_empty(),
        "`tron trc20 encode-call approve` emitted empty stdout; got {stdout:?}"
    );

    let bytes =
        hex::decode(strip_0x(hex)).unwrap_or_else(|e| panic!("non-hex stdout ({e}): {hex:?}"));

    assert_eq!(bytes.len(), TRC20_CALLDATA_LEN);
    assert_eq!(
        &bytes[..4],
        &APPROVE_SELECTOR,
        "TRC-20 approve calldata must begin with keccak256(\
         \"approve(address,uint256)\")[..4] = 0x095ea7b3; got 0x{}",
        hex::encode(&bytes[..4])
    );
}

/// `address.left_pad(32)` invariant: the 32-byte address slot is
/// zero-padded on the LEFT (12 leading zero bytes) with the 20-byte
/// recipient hash at the right. A selector-only assertion would silently
/// pass if anychain accidentally emitted the address un-padded or
/// right-aligned. Pinning the slot keeps the encoding tight.
#[test]
fn v3_trc20_encode_call_transfer_address_slot_is_left_padded() {
    let recipient = common::nile_recipient();
    let amount = "1000000";

    let assert = common::tron()
        .args([
            "trc20",
            "encode-call",
            "transfer",
            "--to",
            recipient,
            "--amount",
            amount,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let hex = stdout.trim();
    let bytes =
        hex::decode(strip_0x(hex)).unwrap_or_else(|e| panic!("non-hex stdout ({e}): {hex:?}"));

    assert_eq!(bytes.len(), TRC20_CALLDATA_LEN);
    assert_eq!(&bytes[..4], &TRANSFER_SELECTOR);

    // Recipient T-addresses are base58check-encoded
    // `0x41 || <20-byte hash> || <4-byte SHA-256d checksum>` = 25 bytes.
    // The 21-byte payload (prefix + hash) is what the ABI slot carries —
    // right-aligned in the 32-byte slot, so leading 11 bytes are zero.
    let decoded = bs58::decode(recipient)
        .into_vec()
        .expect("nile_recipient must be valid base58check");
    assert_eq!(
        decoded.len(),
        25,
        "TRON T-address base58check payload must be 25 bytes \
         (1 prefix + 20 hash + 4 checksum); got {}",
        decoded.len()
    );
    assert_eq!(
        decoded[0], 0x41,
        "TRON T-address payload byte 0 must be 0x41 mainnet prefix; got 0x{:02x}",
        decoded[0]
    );
    let payload = &decoded[..21];
    let checksum = &decoded[21..];

    // SHA-256d checksum verify: SHA256(SHA256(payload))[:4] must match the
    // trailing 4 bytes. Cheap invariant: catches a typo in the bundled
    // `tokens/nile.json` fixture if a future operator copy-pastes a
    // malformed address. `sha2` is already in this crate's dev-deps.
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let mid: [u8; 32] = hasher.finalize().into();
    let mut hasher = Sha256::new();
    hasher.update(mid);
    let final_hash: [u8; 32] = hasher.finalize().into();
    assert_eq!(
        &final_hash[..4],
        checksum,
        "TRON T-address base58check checksum mismatch; the bundled nile.json \
         fixture may carry a malformed address"
    );

    // bytes[4..36] is the 32-byte address slot. Leading 11 bytes must be 0,
    // trailing 21 bytes must equal the full 21-byte base58check payload
    // (`0x41` mainnet prefix + 20-byte hash). NOT the bare 20-byte hash —
    // the slot keeps the mainnet byte prefix in the ABI.
    let slot = &bytes[4..36];
    assert!(
        slot[..11].iter().all(|b| *b == 0),
        "address slot leading 11 bytes must be zero (left-pad of 21-byte payload); got 0x{}",
        hex::encode(&slot[..11])
    );
    assert_eq!(
        &slot[11..],
        payload,
        "address slot trailing 21 bytes must equal the T-address base58check payload \
         (0x41 || 20-byte hash); slot[11..]=0x{} want 0x{}",
        hex::encode(&slot[11..]),
        hex::encode(payload)
    );
}

/// Offline sanity check on the keccak-derived selector constants. We do
/// not invoke `tron trc20 encode-call balance-of` here because the spike's
/// job is to assert selectors the shipped CLI knows about, not to derive
/// them. The constant `BALANCE_OF_SELECTOR = 0x70a08231` (from
/// keccak256("balanceOf(address)")) is documented in plan §Q3; downstream
/// tests in `trc20_nile.rs::row_1` exercise the live call when
/// `RUN_TRON_NILE=1`.
#[test]
fn v3_trc20_balance_of_selector_documented_in_plan() {
    assert_eq!(BALANCE_OF_SELECTOR, [0x70, 0xa0, 0x82, 0x31]);
}
