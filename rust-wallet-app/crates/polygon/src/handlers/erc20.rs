//! ERC-20 handlers — Issue #426 / Batch E.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.6 + §6.5. Batch E (TDD): `guard_usdc_e` wrapper around
//! `polygon_wallet_core::disambig::reject_bridged_usdc_e` — Story 31
//! critical-tier guard against the native-vs-bridged USDC footgun.

use alloy_primitives::Address;

/// Reject bridged USDC.e addresses to prevent the native-vs-bridged
/// USDC footgun. Thin wrapper over
/// `polygon_wallet_core::disambig::reject_bridged_usdc_e` so the CLI's
/// `erc20 send --token USDC` path always invokes this guard before any
/// signing.
///
/// Returns `Err(Error::InvalidInput(_))` for any address in the bridged
/// USDC.e disallow list (Polygon mainnet only as of v0.1).
///
/// **Negative check only** — passing means "not on the disallow list,"
/// NOT "this address is native USDC." Callers that need a positive
/// identity assertion must compare against the canonical native USDC
/// address `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` themselves.
pub fn guard_usdc_e(token: Address) -> polygon_wallet_core::Result<()> {
    polygon_wallet_core::disambig::reject_bridged_usdc_e(token)
}

#[cfg(test)]
mod tests {
    //! Batch E tests (per design doc §6.5): USDC.e footgun guard.
    use super::guard_usdc_e;
    use alloy_primitives::Address;

    /// Polygon mainnet bridged USDC.e (legacy, pre-Circle native USDC).
    /// Bytes from `polygon-wallet-core/src/disambig.rs:40-43` —
    /// byte-equality with `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`.
    fn bridged_usdc_e() -> Address {
        Address::new([
            0x27, 0x91, 0xBC, 0xa1, 0xf2, 0xde, 0x46, 0x61, 0xED, 0x88, 0xA3, 0x0C, 0x99, 0xA7,
            0xa9, 0x44, 0x9A, 0xa8, 0x41, 0x74,
        ])
    }

    /// Polygon mainnet canonical native USDC (Circle issuance).
    fn native_usdc() -> Address {
        Address::new([
            0x3c, 0x49, 0x9c, 0x54, 0x2c, 0xEF, 0x5E, 0x38, 0x11, 0xe1, 0x19, 0x2c, 0xe7, 0x0d,
            0x8c, 0xC0, 0x3d, 0x5c, 0x33, 0x59,
        ])
    }

    /// Batch E test #1 (failing seed per design doc §6.5): bridged
    /// USDC.e address rejected with `Error::InvalidInput`.
    #[test]
    fn guard_usdc_e_rejects_bridged_usdce_address() {
        let r = guard_usdc_e(bridged_usdc_e());
        assert!(
            matches!(r, Err(polygon_wallet_core::Error::InvalidInput(ref msg)) if msg.contains("BRIDGED_USDC_REJECTED")),
            "bridged USDC.e must be rejected with BRIDGED_USDC_REJECTED marker; got {r:?}"
        );
    }

    /// Batch E test #2: native USDC (Circle) accepted — guard is
    /// negative-only, native USDC is not on the disallow list.
    #[test]
    fn guard_usdc_e_accepts_native_usdc_address() {
        assert!(guard_usdc_e(native_usdc()).is_ok());
    }

    /// Batch E test #3: zero address accepted (not a bridged issuance).
    #[test]
    fn guard_usdc_e_accepts_zero_address() {
        assert!(guard_usdc_e(Address::ZERO).is_ok());
    }

    /// Batch E test #4: arbitrary other token accepted (USDT, DAI, etc.
    /// don't match the bridged USDC.e bytes).
    #[test]
    fn guard_usdc_e_accepts_other_token_address() {
        let other = Address::new([
            0xde, 0xad, 0xbe, 0xef, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ]);
        assert!(guard_usdc_e(other).is_ok());
    }
}
