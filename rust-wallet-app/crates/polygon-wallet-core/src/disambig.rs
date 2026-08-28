//! Polygon-specific disambiguation helpers — Phase 3 Task 5 of #425.
//!
//! Two responsibilities:
//!
//! 1. **Bridged USDC.e footgun guard.** Polygon hosts two distinct USDC
//!    deployments: Circle's native USDC (`0x3c499c…3359`) and the
//!    legacy bridged USDC.e (`0x2791Bca1…4174`). Sending the wrong
//!    one is a common, irreversible mistake — `reject_bridged_usdc_e`
//!    rejects the bridged address and forces the caller to opt out
//!    explicitly. The bridged address list lives as a compile-time
//!    `const` slice — extend it as new bridged issuances appear.
//!    NOTE: this is a *negative* check — it does NOT positively
//!    identify an address as native USDC; passing means "not on the
//!    disallow list," not "is native USDC."
//!
//! 2. **POL / MATIC legacy alias.** Polygon rebranded MATIC → POL on
//!    2024-09-04. `gas_token_label(use_legacy)` returns the appropriate
//!    display string for wallet output. Source-of-truth constants
//!    already live in `network.rs`; this selector picks one.
//!
//! Both helpers are pure (no I/O, no globals) — safe to call from
//! any layer (CLI handler, library consumer, tests).

use alloy_primitives::Address;
use evm_wallet_core::{Error, Result};

/// Canonical bridged USDC.e address on Polygon mainnet (historical
/// issuance, pre-Circle native USDC). Extend this slice as new
/// bridged issuances appear; `reject_bridged_usdc_e` rejects any
/// address present here.
///
/// Mainnet-only as of v0.1. Amoy testnet has no bridged USDC.e
/// issuance — if Amoy ever ships one, widen this slice with care.
///
/// L13 Round 1 review fix #3: `pub` (wider than `pub(crate)` — the
/// polygon CLI is a separate crate and needs to import this slice
/// directly so its USDC.e guard test couples to the lib's source of
/// truth instead of duplicating bytes; kills the silent-drift risk
/// when this slice extends). The slice contains public on-chain
/// contract addresses — no secret leak; the `pub` boundary is
/// appropriate for "canonical disallow list" data.
pub const BRIDGED_USDC_E_ADDRESSES: &[Address] = &[
    // Polygon mainnet USDC.e (legacy bridged from Ethereum).
    // Bytes match the hex string `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`
    // byte-for-byte; the EIP-55 checksum happens to be that exact
    // string. Do not edit the byte values without re-deriving the
    // EIP-55 checksum against the changed bytes.
    Address::new([
        0x27, 0x91, 0xBC, 0xa1, 0xf2, 0xde, 0x46, 0x61, 0xED, 0x88, 0xA3, 0x0C, 0x99, 0xA7, 0xa9,
        0x44, 0x9A, 0xa8, 0x41, 0x74,
    ]),
];

/// L13 Round 1 review fix #3: compile-time assertion that the slice is
/// non-empty. Future regressions where the slice gets emptied (would
/// silently accept all addresses via `reject_bridged_usdc_e`) fail loud
/// at compile time rather than at runtime.
const _: () = assert!(
    !BRIDGED_USDC_E_ADDRESSES.is_empty(),
    "BRIDGED_USDC_E_ADDRESSES must not be empty — empty slice accepts every address"
);

/// Returns the display label for Polygon's native gas token.
///
/// - `use_legacy = false` → `"POL"` (current canonical label, post
///   2024-09-04 MATIC → POL rebrand).
/// - `use_legacy = true` → `"MATIC"` (preserved for wallets that
///   pre-date the rebrand and for explicit operator opt-in via
///   `--legacy-token-symbol` in the Phase 4 `polygon` CLI).
pub fn gas_token_label(use_legacy: bool) -> &'static str {
    if use_legacy {
        crate::network::LEGACY_GAS_TOKEN_LABEL
    } else {
        crate::network::GAS_TOKEN_LABEL
    }
}

/// Reject bridged USDC.e addresses to prevent the native-vs-bridged
/// footgun. Returns `Err(Error::InvalidInput(_))` for any address in
/// [`BRIDGED_USDC_E_ADDRESSES`].
///
/// This is a *negative* check only — passing means "not on the disallow
/// list," NOT "this address is native USDC." Callers that need a
/// positive identity assertion must compare against the canonical
/// native USDC address `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359`
/// themselves.
///
/// Comparison is byte-equality on the 20-byte payload — both forms of
/// the same address (mixed-case / lowercase / uppercase hex) parse
/// to the same `Address` via `FromStr`, so `==` returns true
/// regardless of EIP-55 form.
pub fn reject_bridged_usdc_e(address: Address) -> Result<()> {
    if BRIDGED_USDC_E_ADDRESSES.contains(&address) {
        Err(Error::InvalidInput(format!(
            "BRIDGED_USDC_REJECTED addr={:#x} reason=polygon-bridged-usdc-e-disallow-list",
            address
        )))
    } else {
        Ok(())
    }
}
