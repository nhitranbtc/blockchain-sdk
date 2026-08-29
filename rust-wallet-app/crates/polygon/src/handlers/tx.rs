//! `polygon tx` handlers — Issue #426 / T6d-3.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.5 + §6.x. Batch F (TDD): `tx_get` + `tx_list`.
//!
//! **Live RPC deferred to T7** per L29 (operator-driven smoke on real
//! Amoy). T6d-3 owns argument validation + the Q7 chain_id gate; the
//! `provider.get_transaction_by_hash` / `provider.get_logs` calls return
//! `Error::Rpc("deferred to T7 (L29 operator-driven)")` until the
//! operator session ships the live path.

use alloy_primitives::{Address, B256};
use std::str::FromStr;

/// T6d-3 handler: `polygon tx list <address> [--since-block] [--limit] [--json]`.
///
/// Per design doc §5.5. Pure arg validation lives here; the live
/// `provider.get_logs` scan is deferred to T7 (L29 operator session).
///
/// Address must parse as 20-byte EIP-55 hex (clap's `value_parser`
/// already filters malformed input — defensive re-parse in the handler
/// guards against direct programmatic callers). `since_block` is
/// optional (defaults to "from latest"); `limit` clamps to `[1, 10000]`
/// to prevent unbounded scans.
pub async fn tx_list(
    address: &str,
    since_block: Option<u64>,
    limit: Option<u32>,
    _json: bool,
) -> polygon_wallet_core::Result<()> {
    // Arg validation (pure — no RPC). Defensive re-parse of the address
    // catches programmatic callers bypassing clap's `value_parser`.
    let _addr = parse_address_strict(address)?;
    if let Some(lim) = limit {
        if lim == 0 || lim > 10_000 {
            return Err(polygon_wallet_core::Error::InvalidInput(format!(
                "tx list --limit must be in [1, 10000]; got {lim}"
            )));
        }
    }
    let _ = since_block; // accepted; default-resolved at T7 live scan
                         // Live RPC deferred to T7 (L29 operator-driven Amoy smoke).
    Err(polygon_wallet_core::Error::Rpc(
        "tx list live RPC deferred to T7 (L29 operator-driven)".into(),
    ))
}

/// T6d-3 handler: `polygon tx get <tx_hash> [--json]`.
///
/// Per design doc §5.5. Parses the tx_hash as a 32-byte B256 hex
/// (EIP-55 insensitive — `B256::from_str` accepts both cased and
/// lowercased). Invalid format → `Error::InvalidInput` (exit 2).
/// Live `provider.get_transaction_by_hash` deferred to T7.
pub async fn tx_get(tx_hash: &str, _json: bool) -> polygon_wallet_core::Result<()> {
    // Validate hash format — defensive even though clap already does it,
    // because programmatic callers can bypass clap.
    let _hash = B256::from_str(tx_hash).map_err(|e| {
        polygon_wallet_core::Error::InvalidInput(format!(
            "tx hash must be 32-byte hex (0x + 64 chars); got {tx_hash:?}: {e}"
        ))
    })?;
    // Live RPC deferred to T7 (L29 operator-driven Amoy smoke).
    Err(polygon_wallet_core::Error::Rpc(
        "tx get live RPC deferred to T7 (L29 operator-driven)".into(),
    ))
}

/// T6d-3 helper (pure): validate that a `--address` string parses as
/// a 20-byte EIP-55 hex. Returns `Ok(Address)` on success; `Err(InvalidInput)`
/// on parse failure. Used by `tx_list` and any future address-bearing
/// handler — single chokepoint for address-format validation.
#[allow(dead_code)] // exported for future batch reuse
pub fn parse_address_strict(s: &str) -> polygon_wallet_core::Result<Address> {
    Address::from_str(s).map_err(|e| {
        polygon_wallet_core::Error::InvalidInput(format!("invalid address {s:?}: {e}"))
    })
}

#[cfg(test)]
mod tests {
    //! Batch F tests (per design doc §6.x extension for T6d-3):
    //! tx_get + tx_list — pure arg validation. Live RPC paths verified
    //! by T7 operator-driven smoke per L29.
    use super::{parse_address_strict, tx_get, tx_list};
    use polygon_wallet_core::Error;

    /// Batch F test #1: tx_get with empty string → Error::InvalidInput.
    #[tokio::test]
    async fn tx_get_rejects_empty_string() {
        let r = tx_get("", false).await;
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "empty tx_hash must be rejected; got {r:?}"
        );
    }

    /// Batch F test #2: tx_get with too-short hex → Error::InvalidInput.
    #[tokio::test]
    async fn tx_get_rejects_short_hex() {
        let r = tx_get("0xdeadbeef", false).await;
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "short hex must be rejected; got {r:?}"
        );
    }

    /// Batch F test #3: tx_get with well-formed 32-byte hex → reaches
    /// the live-RPC gate (returns `Error::Rpc("deferred to T7")`).
    #[tokio::test]
    async fn tx_get_well_formed_hash_reaches_live_rpc_gate() {
        let valid_hash = "0x".to_string() + &"0".repeat(64); // 32 zero bytes
        let r = tx_get(&valid_hash, false).await;
        match r {
            Err(Error::Rpc(ref msg)) if msg.contains("T7") => {}
            other => panic!("well-formed hash must reach T7 deferral gate; got {other:?}"),
        }
    }

    /// Batch F test #4: tx_list with limit=0 → Error::InvalidInput.
    #[tokio::test]
    async fn tx_list_rejects_zero_limit() {
        let r = tx_list(
            "0x0000000000000000000000000000000000000001",
            None,
            Some(0),
            false,
        )
        .await;
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "limit=0 must be rejected; got {r:?}"
        );
    }

    /// Batch F test #5: tx_list with limit > 10000 → Error::InvalidInput.
    #[tokio::test]
    async fn tx_list_rejects_excessive_limit() {
        let r = tx_list(
            "0x0000000000000000000000000000000000000001",
            None,
            Some(10_001),
            false,
        )
        .await;
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "limit > 10000 must be rejected; got {r:?}"
        );
    }

    /// Batch F test #6: tx_list with valid args → reaches T7 deferral.
    #[tokio::test]
    async fn tx_list_valid_args_reach_live_rpc_gate() {
        let r = tx_list(
            "0x0000000000000000000000000000000000000001",
            Some(100),
            Some(50),
            false,
        )
        .await;
        match r {
            Err(Error::Rpc(ref msg)) if msg.contains("T7") => {}
            other => panic!("valid args must reach T7 deferral gate; got {other:?}"),
        }
    }

    /// Batch F test #7: parse_address_strict accepts valid hex.
    #[test]
    fn parse_address_strict_accepts_valid_hex() {
        let a = parse_address_strict("0x0000000000000000000000000000000000000001").expect("valid");
        assert_eq!(format!("{a}"), "0x0000000000000000000000000000000000000001");
    }

    /// Batch F test #8: parse_address_strict rejects non-hex.
    #[test]
    fn parse_address_strict_rejects_non_hex() {
        let r = parse_address_strict("not-an-address");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "non-hex must be rejected; got {r:?}"
        );
    }

    /// Batch F test #9: parse_address_strict rejects short hex.
    #[test]
    fn parse_address_strict_rejects_short_hex() {
        let r = parse_address_strict("0x1234");
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "short hex must be rejected; got {r:?}"
        );
    }
}
