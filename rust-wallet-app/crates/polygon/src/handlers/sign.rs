//! EIP-191 + EIP-712 signing handlers — Issue #426 / Batch D.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.10 + §6.4. Batch D (TDD): `assert_polygon_chain_id` (Q7 critical-tier
//! gate against cross-chain replay on EIP-712 typed-data signing).

use polygon_wallet_core::{Error, PolygonChain};

/// Q7 + C1 enforcement: EIP-712 `chain_id` must be a Polygon chain
/// (`PolygonChain::Mainnet` = 137 or `PolygonChain::Amoy` = 80002).
///
/// Single chokepoint for cross-chain replay protection on EIP-712 typed-data
/// signing. Both `sign_typed_data` (explicit arg) and any future EIP-712
/// path (Permit2, route handlers, etc.) call this before signing.
///
/// Returns `Error::InvalidInput` for any chain_id not in {137, 80002}.
pub fn assert_polygon_chain_id(chain_id: u64) -> polygon_wallet_core::Result<()> {
    match chain_id {
        137 | 80002 => Ok(()),
        other => Err(Error::InvalidInput(format!(
            "EIP-712 chain_id {other} is not a polygon chain (expected 137|80002)"
        ))),
    }
}

/// Resolve a `--chain-id` u64 to a `PolygonChain` enum variant. Inverse
/// of `PolygonChain::chain_id()`. Returns `Error::InvalidInput` for
/// unknown chain_ids (defense-in-depth alongside `assert_polygon_chain_id`).
pub fn polygon_chain_from_id(chain_id: u64) -> polygon_wallet_core::Result<PolygonChain> {
    match chain_id {
        137 => Ok(PolygonChain::Mainnet),
        80002 => Ok(PolygonChain::Amoy),
        other => Err(Error::InvalidInput(format!(
            "unknown polygon chain_id {other} (expected 137|80002)"
        ))),
    }
}

#[cfg(test)]
mod tests {
    //! Batch D tests (per design doc §6.4): EIP-712 chain_id gate.
    use super::{assert_polygon_chain_id, polygon_chain_from_id};
    use polygon_wallet_core::{Error, PolygonChain};

    /// Batch D test #1 (failing seed per design doc §6.4): chain_id=1
    /// (Ethereum mainnet) rejected — cross-chain replay blocked at the
    /// type level.
    #[test]
    fn assert_polygon_chain_id_rejects_chain_id_1() {
        let r = assert_polygon_chain_id(1);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=1 (Ethereum mainnet) must be rejected; got {r:?}"
        );
    }

    /// Batch D test #2: chain_id=11155111 (Sepolia) rejected.
    #[test]
    fn assert_polygon_chain_id_rejects_chain_id_sepolia() {
        let r = assert_polygon_chain_id(11155111);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=11155111 (Sepolia) must be rejected; got {r:?}"
        );
    }

    /// Batch D test #3: chain_id=137 (Polygon mainnet) accepted.
    #[test]
    fn assert_polygon_chain_id_accepts_chain_id_137() {
        assert!(assert_polygon_chain_id(137).is_ok());
    }

    /// Batch D test #4: chain_id=80002 (Polygon amoy) accepted.
    #[test]
    fn assert_polygon_chain_id_accepts_chain_id_80002() {
        assert!(assert_polygon_chain_id(80002).is_ok());
    }

    /// Batch D test #5: unknown chain_id rejected.
    #[test]
    fn assert_polygon_chain_id_rejects_unknown_chain_id() {
        let r = assert_polygon_chain_id(99999);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=99999 must be rejected; got {r:?}"
        );
    }

    /// Batch D test #6 (companion): polygon_chain_from_id round-trips
    /// with PolygonChain::chain_id().
    #[test]
    fn polygon_chain_from_id_round_trips() {
        assert_eq!(
            polygon_chain_from_id(PolygonChain::Mainnet.chain_id()).unwrap(),
            PolygonChain::Mainnet
        );
        assert_eq!(
            polygon_chain_from_id(PolygonChain::Amoy.chain_id()).unwrap(),
            PolygonChain::Amoy
        );
    }
}
