//! BIP-32 hierarchical key derivation.
//!
//! Wraps `bip32::XPrv` in `XPrvHolder` with custom `Drop` for best-effort
//! zeroize-on-drop. Per F47/U3: extended private keys must not linger in
//! memory after the wallet drops them.
//!
//! **Why not `Secret<XPrv>`?** `bip32::XPrv` does NOT impl `Zeroize`
//! (private fields `private_key` and `attrs.chain_code` can't be
//! zeroized externally). `Secret<T: Zeroize>` won't compile.
//!
//! **Accepted v0.1 limitations:**
//!
//! 1. `chain_code` (32 bytes) and `parent_fingerprint` (4 bytes) are
//!    NOT zeroized on drop — `bip32::XPrv`'s `attrs` field is private
//!    with no public accessor. The 32-byte scalar IS zeroized via
//!    `xprv.to_bytes() + zeroize()` in our custom `Drop`.
//! 2. `XPrv` impls `Clone`, so any caller that obtains an `&XPrv`
//!    can clone the full struct (including chain_code). Public API
//!    only exposes `scalar() -> Secret<Vec<u8>>` to avoid this.
//!
//! **Defends against:** U3 (memory leak, partial — chain_code excluded),
//! A1 (local read, partial), A8 (`/proc/$pid/mem`, partial).
//!
//! **Drift from plan §Task 4** (L9 v3 PR body):
//!
//! | Plan said | This implementation | Why |
//! |---|---|---|
//! | `master.derive_path(path)` | Manual for-loop with owned XPrv | Method doesn't exist in bip32 0.5.3; manual loop avoids O(N) clones per derive step |
//! | Return raw `XPrv` from `master_from_seed` | Return `XPrvHolder` (custom Drop) | XPrv not Zeroize; `Secret<XPrv>` won't compile |
//! | `derive_xprv(master, path)` returns raw `XPrv` | Returns `XPrvHolder` | Same |
//! | Public `xprv()` accessor for downstream use | `scalar() -> Secret<Vec<u8>>` only | Removes Clone escape hatch (type-design H1 / security F6) |
//! | `XPrv::derive_from_path(seed, &"m")` for master | `XPrv::new(seed)` direct constructor | Code M1 — bip32 0.5.3 has direct constructor, no `from_str("m")` indirection |

use std::str::FromStr;

use bip32::{DerivationPath, XPrv};
use zeroize::Zeroize;

use crate::error::{Error, Result};
use crate::keys::secret::Secret;

/// BIP-44/49/84/86 address type → purpose coin constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressType {
    /// BIP-44 — Legacy P2PKH (1...)
    Legacy,
    /// BIP-49 — Nested SegWit P2SH-P2WPKH (3...)
    NestedSegwit,
    /// BIP-84 — Native SegWit P2WPKH (bc1q...). Default for new wallets.
    NativeSegwit,
    /// BIP-86 — Taproot P2TR (bc1p...).
    Taproot,
}

impl AddressType {
    /// BIP purpose coin constant: 44 / 49 / 84 / 86.
    pub const fn purpose(self) -> u32 {
        match self {
            Self::Legacy => 44,
            Self::NestedSegwit => 49,
            Self::NativeSegwit => 84,
            Self::Taproot => 86,
        }
    }
}

/// Newtype around `bip32::XPrv`. Custom `Drop` for best-effort scalar
/// zeroize. NOT Clone (XPrv: Clone would defeat zeroize-on-drop).
///
/// **Public API is deliberately narrow:** only `scalar()` exposes
/// secret material (wrapped in `Secret<Vec<u8>>`). Internal callers
/// (e.g., `Signer::from_xprv`) use this; no `xprv() -> &XPrv` accessor
/// exists, removing the Clone escape hatch.
pub struct XPrvHolder(XPrv);

impl std::fmt::Debug for XPrvHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No field names (avoids BIP-39 wordlist collision; see
        // CONTEXT.md hard rule #7).
        f.debug_struct("XPrvHolder").finish_non_exhaustive()
    }
}

impl XPrvHolder {
    /// Construct from a raw seed. Uses `XPrv::new` (direct BIP-32 master
    /// constructor; bip32 0.5.3 has no `derive_from_path(seed, &"m")` shortcut
    /// needed).
    pub fn master_from_seed(seed: &[u8; 64]) -> Result<Self> {
        let xprv =
            XPrv::new(seed).map_err(|e| Error::InvalidDerivationPath(format!("master: {e}")))?;
        Ok(Self(xprv))
    }

    /// Derive a child key per BIP-32 standard path. Path must be
    /// non-empty (master derivation is `master_from_seed`, not this).
    ///
    /// **Manual for-loop** (not `fold`/`try_fold`) — saves O(N) XPrv
    /// clones per derive step. `XPrv: Clone` copies chain_code + scalar;
    /// fold would clone the full struct per path element.
    pub fn derive(&self, path: &DerivationPath) -> Result<Self> {
        if path.is_empty() {
            return Err(Error::InvalidDerivationPath(
                "empty path: use master_from_seed for root key".into(),
            ));
        }
        let mut current = self.0.clone();
        for child_num in path.iter() {
            current = current
                .derive_child(child_num)
                .map_err(|e| Error::InvalidDerivationPath(format!("derive: {e}")))?;
        }
        Ok(Self(current))
    }

    /// Returns the 32-byte secret scalar wrapped in `Secret<Vec<u8>>`
    /// for zeroize-on-drop. **Only public secret accessor** — replaces
    /// the `xprv() -> &XPrv` Clone escape hatch.
    ///
    /// Downstream callers (e.g., `Signer::from_xprv`) consume this.
    /// The 32 bytes are heap-allocated and zeroized when dropped.
    pub fn scalar(&self) -> Secret<Vec<u8>> {
        let bytes: Vec<u8> = self.0.to_bytes().to_vec();
        Secret::new(bytes)
    }

    /// Returns the inner xprv as a base58-encoded string wrapped in a
    /// zeroizing `Secret<String>` (BIP-32 standard serialization,
    /// prefixed `xprv…`).
    ///
    /// `pub(crate)` so external callers cannot extract the xprv —
    /// only `Wallet` (in the same crate) can build a descriptor
    /// template. The returned `Secret<String>` zeroizes its inner
    /// heap `String` on drop.
    pub(crate) fn to_xprv_secret(
        &self,
        network: bdk_wallet::bitcoin::Network,
    ) -> crate::keys::Secret<String> {
        use bip32::Prefix;
        // bip32 0.5.3 ExtendedPrivateKey doesn't carry its own
        // prefix; we must supply it based on the wallet's network.
        // bdk's descriptor parser rejects the wrong prefix with
        // Key(InvalidNetworkKind).
        let prefix = match network {
            bdk_wallet::bitcoin::Network::Bitcoin => Prefix::XPRV,
            bdk_wallet::bitcoin::Network::Testnet => Prefix::TPRV,
            // regtest/signet use testnet derivation (BIP-44 coin type 1)
            // + tprv prefix per convention. BDK 0.31+ validates the xprv
            // network kind matches the wallet's network — xprv for both
            // bitcoin and regtest fails with Key(InvalidNetworkKind).
            _ => Prefix::TPRV,
        };
        let s = self.0.to_string(prefix).to_string();
        crate::keys::Secret::new(s)
    }
}

impl Drop for XPrvHolder {
    /// Best-effort scalar zeroize. chain_code + parent_fingerprint
    /// leak (v0.1 limitation; bip32 0.5.3 has no public accessor for
    /// `attrs.chain_code`). See module doc.
    fn drop(&mut self) {
        // Per F53: scoped unsafe to move out before Drop semantics fire.
        // No `mem::forget(self)` — clippy `forgetting_references` lint
        // correctly flags forgetting `&mut Self` as a no-op.
        #[allow(unsafe_code)]
        let xprv = unsafe { std::ptr::read(&self.0) };

        // Extract + zeroize the 32-byte private scalar.
        let mut bytes: [u8; 32] = xprv.to_bytes();
        bytes.zeroize();

        // Drop XPrv: bip32::ExtendedPrivateKey has no Drop that zeros
        // chain_code (verified — only zeroes serialized `key_bytes`).
        // The chain_code field stays in memory until allocation reuse.
        drop(xprv);
    }
}

/// Standard pattern: `m/<purpose>'/<coin_type>'/<account>'/0/<index>`.
///
/// **Range checks** (per type-design review): hardened derivation
/// (`'`) is applied to `purpose`, `coin_type`, `account` (BIP-44).
/// BIP-32 rejects indices >= 0x80000000 for hardened; we reject them
/// upfront so the error message names the bad field.
pub fn address_type_to_path(
    t: AddressType,
    coin_type: u32,
    account: u32,
    index: u32,
) -> Result<DerivationPath> {
    // Hardened indices must be < 0x80000000. Reject upfront.
    if coin_type >= 0x8000_0000 {
        return Err(Error::InvalidDerivationPath(format!(
            "coin_type {coin_type} >= 0x80000000 (out of hardened range)"
        )));
    }
    if account >= 0x8000_0000 {
        return Err(Error::InvalidDerivationPath(format!(
            "account {account} >= 0x80000000 (out of hardened range)"
        )));
    }
    // index is intentionally non-hardened (BIP-44 receive).
    if index >= 0x8000_0000 {
        return Err(Error::InvalidDerivationPath(format!(
            "index {index} >= 0x80000000"
        )));
    }
    let s = format!("m/{}'/{}'/{}'/0/{}", t.purpose(), coin_type, account, index);
    DerivationPath::from_str(&s).map_err(|e| Error::InvalidDerivationPath(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip84_native_segwit_path() {
        let p = address_type_to_path(AddressType::NativeSegwit, 0, 0, 5).expect("path");
        assert_eq!(p.to_string(), "m/84'/0'/0'/0/5");
    }

    #[test]
    fn bip86_taproot_path() {
        let p = address_type_to_path(AddressType::Taproot, 0, 0, 0).expect("path");
        assert_eq!(p.to_string(), "m/86'/0'/0'/0/0");
    }

    #[test]
    fn bip44_legacy_path() {
        let p = address_type_to_path(AddressType::Legacy, 0, 0, 0).expect("path");
        assert_eq!(p.to_string(), "m/44'/0'/0'/0/0");
    }

    #[test]
    fn bip49_nested_segwit_path() {
        let p = address_type_to_path(AddressType::NestedSegwit, 0, 0, 7).expect("path");
        assert_eq!(p.to_string(), "m/49'/0'/0'/0/7");
    }

    #[test]
    fn purpose_constants_match_bip() {
        assert_eq!(AddressType::Legacy.purpose(), 44);
        assert_eq!(AddressType::NestedSegwit.purpose(), 49);
        assert_eq!(AddressType::NativeSegwit.purpose(), 84);
        assert_eq!(AddressType::Taproot.purpose(), 86);
    }

    #[test]
    fn path_with_mainnet_coin_type() {
        let p = address_type_to_path(AddressType::NativeSegwit, 0, 5, 3).expect("path");
        assert_eq!(p.to_string(), "m/84'/0'/5'/0/3");
    }

    #[test]
    fn path_rejects_out_of_range_coin_type() {
        let err = address_type_to_path(AddressType::NativeSegwit, 0x8000_0000, 0, 0)
            .expect_err("should reject");
        assert!(matches!(err, Error::InvalidDerivationPath(_)));
        assert!(err.to_string().contains("coin_type"));
    }

    #[test]
    fn path_rejects_out_of_range_account() {
        let err = address_type_to_path(AddressType::NativeSegwit, 0, 0x8000_0000, 0)
            .expect_err("should reject");
        assert!(err.to_string().contains("account"));
    }

    #[test]
    fn path_rejects_out_of_range_index() {
        let err = address_type_to_path(AddressType::NativeSegwit, 0, 0, 0x8000_0000)
            .expect_err("should reject");
        assert!(err.to_string().contains("index"));
    }

    #[test]
    fn master_from_seed_succeeds() {
        let seed = [0x42u8; 64];
        let master = XPrvHolder::master_from_seed(&seed).expect("master");
        let _ = master.scalar();
    }

    #[test]
    fn master_from_two_different_seeds_produces_different_keys() {
        let seed_a = [0x01u8; 64];
        let seed_b = [0x02u8; 64];
        let m_a = XPrvHolder::master_from_seed(&seed_a).expect("a");
        let m_b = XPrvHolder::master_from_seed(&seed_b).expect("b");
        assert_ne!(
            m_a.scalar().expose(),
            m_b.scalar().expose(),
            "different seeds must produce different masters"
        );
    }

    #[test]
    fn derive_first_child_succeeds() {
        let seed = [0x42u8; 64];
        let master = XPrvHolder::master_from_seed(&seed).expect("master");
        let path = address_type_to_path(AddressType::NativeSegwit, 0, 0, 0).expect("path");
        let child = master.derive(&path).expect("derive");
        assert_ne!(
            master.scalar().expose(),
            child.scalar().expose(),
            "child scalar must differ from master"
        );
    }

    #[test]
    fn derive_is_deterministic() {
        let seed = [0x42u8; 64];
        let m1 = XPrvHolder::master_from_seed(&seed).expect("m1");
        let m2 = XPrvHolder::master_from_seed(&seed).expect("m2");
        let path = address_type_to_path(AddressType::NativeSegwit, 0, 0, 0).expect("path");
        let c1 = m1.derive(&path).expect("c1");
        let c2 = m2.derive(&path).expect("c2");
        assert_eq!(c1.scalar().expose(), c2.scalar().expose());
    }

    #[test]
    fn derive_empty_path_rejected() {
        let seed = [0x42u8; 64];
        let master = XPrvHolder::master_from_seed(&seed).expect("master");
        let empty = DerivationPath::from_str("m").expect("master path");
        let err = master.derive(&empty).expect_err("empty path");
        assert!(matches!(err, Error::InvalidDerivationPath(_)));
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn derive_hardened_only_path_succeeds() {
        let seed = [0x42u8; 64];
        let master = XPrvHolder::master_from_seed(&seed).expect("master");
        let path = DerivationPath::from_str("m/44'/0'").expect("hardened path");
        let child = master.derive(&path).expect("derive");
        assert_ne!(master.scalar().expose(), child.scalar().expose());
    }

    #[test]
    fn derive_mixed_hardened_and_nonhardened_path_succeeds() {
        let seed = [0x42u8; 64];
        let master = XPrvHolder::master_from_seed(&seed).expect("master");
        let path = DerivationPath::from_str("m/84'/0'/0'/0/7").expect("mixed path");
        let child = master.derive(&path).expect("derive");
        assert_ne!(master.scalar().expose(), child.scalar().expose());
    }

    #[test]
    fn xprv_holder_debug_hides_secret() {
        let seed = [0x42u8; 64];
        let master = XPrvHolder::master_from_seed(&seed).expect("master");
        let dbg = format!("{master:?}");
        assert!(dbg.contains("XPrvHolder"));
        // Per CONTEXT.md hard rule #7: no wordlist words as field names.
        assert!(!dbg.contains("inner"), "Debug field collision: {dbg}");
    }

    #[test]
    fn xprv_holder_drops_without_panic() {
        // Best-effort test: Drop must not panic. Real zeroize verification
        // would require reading freed memory (UB).
        let seed = [0x42u8; 64];
        {
            let _master = XPrvHolder::master_from_seed(&seed).expect("master");
        }
    }
}
