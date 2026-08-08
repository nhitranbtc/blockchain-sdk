//! secp256k1 signer for ECDSA signatures.
//!
//! Holds the 32-byte secret scalar in `Secret<Vec<u8>>` (heap, non-Copy,
//! per L16 anti-pattern). `SecretKey` is reconstructed on each
//! `sign_ecdsa` / `public_key` call (microsecond cost; signing is rare
//! and not on a hot path). Reconstructed `SecretKey` is explicitly
//! zeroized via `secp256k1::SecretKey::non_secure_erase` immediately
//! after the FFI call returns.
//!
//! **Defends against:** U3 (memory leak, partial), A1 (local read,
//! partial), A8 (`/proc/$pid/mem`, partial).
//!
//! **Threat-model notes:**
//!
//! - F7 (U5 mitigation): `sign_ecdsa(&[u8; 32])` narrow API is **API
//!   hygiene only** — a real Bitcoin sighash IS 32 bytes, so the API
//!   doesn't refuse phishing inputs. Caller must declare intent via
//!   `threat::Sighash` / `MessageClass` enums (added in Task 9).
//! - T3 (timing side-channel): signatures use `secp256k1`'s constant-time
//!   implementation. No custom scalar math.
//!
//! **Drift from plan §Task 4** (L9 v3 PR body):
//!
//! | Plan said | This implementation | Why |
//! |---|---|---|
//! | `sign_ecdsa(&[u8; 32]) -> Signature` | Same; doc corrected — narrow API is hygiene, not U5 mitigation | Plan over-claimed F7 |
//! | `from_secret_key(SecretKey) -> Self` | Same; now calls `non_secure_erase` on input | `SecretKey: Copy`; caller's frame retains a copy. Documented obligation. |
//! | `keypair: Secret<Keypair>` | Replaced by `secret_bytes: Secret<Vec<u8>>` | `Keypair` requires `DefaultIsZeroes` for `Zeroize` (zeroize 1.9); wrapping fails to compile |
//!
//! **Caller responsibility (documented in module doc):**
//! - Prefer `from_secret_bytes(Secret<Vec<u8>>)` for full zeroize control.
//! - `from_secret_key(SecretKey)` zeroizes the LOCAL `SecretKey` but the
//!   caller's original `SecretKey` variable retains a copy (Copy type).
//!   Caller should `sk.non_secure_erase()` before/after passing.

use bdk_wallet::bitcoin::secp256k1::ecdsa::{RecoverableSignature, RecoveryId, Signature};
use bdk_wallet::bitcoin::secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use zeroize::Zeroize;

use crate::error::{Error, Result};
use crate::keys::derivation::XPrvHolder;
use crate::keys::secret::Secret;

/// secp256k1 signer. Holds the 32-byte secret scalar in heap-allocated
/// `Secret<Vec<u8>>` so it zeroizes on drop (avoids L16 Copy-type defeat).
///
/// `Secp256k1<All>` is not zeroized on drop (it's precomputed-table data,
/// not secret material). Manual `Drop` impl handles only the secret field.
pub struct Signer {
    secret_bytes: Secret<Vec<u8>>,
    secp: Secp256k1<bdk_wallet::bitcoin::secp256k1::All>,
}

impl Drop for Signer {
    fn drop(&mut self) {
        // Secret<Vec<u8>> zeroizes its inner on drop automatically.
        // No explicit action needed here; the field's Drop impl runs.
    }
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer").finish_non_exhaustive()
    }
}

impl Signer {
    /// Construct a `Signer` from a raw `SecretKey`. Defensive: zeroizes
    /// the LOCAL `SecretKey` via `non_secure_erase` before dropping.
    ///
    /// **Caller obligation:** because `SecretKey: Copy`, the caller's
    /// original `SecretKey` variable retains a copy on their stack.
    /// Caller should call `sk.non_secure_erase()` on their side before
    /// and/or after this call. For full zeroize control, prefer
    /// `from_secret_bytes(Secret<Vec<u8>>)`.
    pub fn from_secret_key(mut sk: SecretKey) -> Self {
        let secret_bytes: Vec<u8> = sk.secret_bytes().to_vec();
        sk.non_secure_erase();
        Self {
            secret_bytes: Secret::new(secret_bytes),
            secp: Secp256k1::new(),
        }
    }

    /// Construct a `Signer` from heap-zeroizable bytes. **Preferred
    /// entry point** — caller retains full control of the input bytes
    /// (which zeroize on drop).
    pub fn from_secret_bytes(bytes: Secret<Vec<u8>>) -> Self {
        Self {
            secret_bytes: bytes,
            secp: Secp256k1::new(),
        }
    }

    /// Construct a `Signer` from an `XPrvHolder` (Task 4 internal
    /// wiring; used by Task 9 wallet). Zeroizes the stack-local 32-byte
    /// scalar after extracting the `SecretKey`.
    pub fn from_xprv(xprv: &XPrvHolder) -> Result<Self> {
        let mut bytes: [u8; 32] = {
            let secret_vec = xprv.scalar();
            let slice = secret_vec.expose();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(slice);
            arr
        };
        let sk = SecretKey::from_slice(&bytes)
            .map_err(|e| Error::Sign(format!("xprv -> secret_key: {e}")))?;
        bytes.zeroize(); // zeroize the stack-resident [u8; 32]
        Ok(Self::from_secret_key(sk))
    }

    /// Returns the 33-byte compressed secp256k1 public key. The
    /// reconstructed `SecretKey` is zeroized immediately after use.
    pub fn public_key(&self) -> PublicKey {
        let mut sk = self.secret_key();
        let pk = PublicKey::from_secret_key(&self.secp, &sk);
        sk.non_secure_erase();
        pk
    }

    /// Sign a 32-byte digest. Returns a 64-byte compact ECDSA signature.
    /// The reconstructed `SecretKey` is zeroized immediately after the
    /// FFI call.
    pub fn sign_ecdsa(&self, hash: &[u8; 32]) -> Result<Signature> {
        let msg = Message::from_digest(*hash);
        let mut sk = self.secret_key();
        let sig = self.secp.sign_ecdsa(&msg, &sk);
        sk.non_secure_erase();
        Ok(sig)
    }

    /// Reconstruct the `SecretKey` from stored bytes. Caller MUST call
    /// `non_secure_erase` on the result after use (it is `Copy` and does
    /// not zeroize on drop).
    fn secret_key(&self) -> SecretKey {
        let bytes = self.secret_bytes.expose();
        SecretKey::from_slice(bytes)
            .expect("stored secret bytes are valid (set by from_secret_key/from_xprv)")
    }

    /// Sign a 32-byte digest recoverably. Returns `(RecoveryId, [u8; 64])`.
    ///
    /// **Task 6 (BIP-137):** `pub(crate)` only — public callers must use
    /// `crypto::bip137::sign_message` so the narrow F7 API stays the
    /// only signing entrypoint. Per security-auditor L12 review (F7
    /// hygiene: every `pub` 32-byte-signing method is a phishing vector).
    ///
    /// The local `SecretKey` is `non_secure_erase`-ed immediately after
    /// the FFI call returns. The returned `[u8; 64]` is the compact
    /// signature bytes (public material, not secret).
    pub(crate) fn sign_recoverable(&self, hash: &[u8; 32]) -> Result<(RecoveryId, [u8; 64])> {
        let msg = Message::from_digest(*hash);
        let mut sk = self.secret_key();
        let rec_sig: RecoverableSignature = self.secp.sign_ecdsa_recoverable(&msg, &sk);
        sk.non_secure_erase();
        Ok(rec_sig.serialize_compact())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::derivation::{address_type_to_path, AddressType};

    #[test]
    fn sign_ecdsa_produces_64_byte_signature() {
        let sk_bytes = [0x01u8; 32];
        let signer = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let hash = [0u8; 32];
        let sig = signer.sign_ecdsa(&hash).expect("sign");
        assert_eq!(sig.serialize_compact().len(), 64);
    }

    #[test]
    fn sign_ecdsa_is_deterministic_for_same_key_and_hash() {
        let sk_bytes = [0x42u8; 32];
        let s1 = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let s2 = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let hash = [0xaau8; 32];
        let sig1 = s1.sign_ecdsa(&hash).expect("sign1");
        let sig2 = s2.sign_ecdsa(&hash).expect("sign2");
        assert_eq!(
            sig1.serialize_compact(),
            sig2.serialize_compact(),
            "RFC 6979 deterministic ECDSA must match"
        );
    }

    #[test]
    fn sign_ecdsa_changes_with_hash() {
        let sk_bytes = [0x42u8; 32];
        let signer = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let hash_a = [0x01u8; 32];
        let hash_b = [0x02u8; 32];
        let sig_a = signer.sign_ecdsa(&hash_a).expect("a");
        let sig_b = signer.sign_ecdsa(&hash_b).expect("b");
        assert_ne!(sig_a.serialize_compact(), sig_b.serialize_compact());
    }

    #[test]
    fn sign_ecdsa_verifies_against_matching_pubkey() {
        let sk_bytes = [0x07u8; 32];
        let signer = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let pk = signer.public_key();
        let secp = Secp256k1::new();
        let hash = [0xbbu8; 32];
        let msg = Message::from_digest(hash);
        let sig = signer.sign_ecdsa(&hash).expect("sign");
        assert!(
            secp.verify_ecdsa(&msg, &sig, &pk).is_ok(),
            "signature must verify against matching pubkey"
        );
    }

    #[test]
    fn public_key_is_deterministic() {
        let sk_bytes = [0x07u8; 32];
        let s1 = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let s2 = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        assert_eq!(s1.public_key(), s2.public_key());
    }

    #[test]
    fn public_key_is_33_bytes_compressed() {
        let sk_bytes = [0x42u8; 32];
        let signer = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let pk = signer.public_key();
        let serialized = pk.serialize(); // compressed by default
        assert_eq!(
            serialized.len(),
            33,
            "secp256k1 default pubkey is 33-byte compressed"
        );
    }

    #[test]
    fn different_keys_produce_different_signatures() {
        let sk_a = Signer::from_secret_bytes(Secret::new(vec![0x01u8; 32]));
        let sk_b = Signer::from_secret_bytes(Secret::new(vec![0x02u8; 32]));
        let hash = [0u8; 32];
        let sig_a = sk_a.sign_ecdsa(&hash).unwrap();
        let sig_b = sk_b.sign_ecdsa(&hash).unwrap();
        assert_ne!(sig_a.serialize_compact(), sig_b.serialize_compact());
    }

    #[test]
    fn signer_debug_hides_secret() {
        let signer = Signer::from_secret_bytes(Secret::new(vec![0x42u8; 32]));
        let dbg = format!("{signer:?}");
        assert!(dbg.contains("Signer"));
        assert!(!dbg.contains("inner"), "Debug field collision: {dbg}");
    }

    #[test]
    fn signer_zeroizes_on_drop() {
        // Compile-time witness: Signer holds `Secret<Vec<u8>>` which
        // derives ZeroizeOnDrop. Verify the inner field zeroizes on drop
        // (this is the actual security guarantee; the manual Drop on
        // Signer itself is a no-op that lets the field's Drop run).
        fn assert_zod<T: zeroize::ZeroizeOnDrop>() {}
        assert_zod::<Secret<Vec<u8>>>();
    }

    #[test]
    fn from_xprv_round_trip() {
        let seed = [0x42u8; 64];
        let master = XPrvHolder::master_from_seed(&seed).expect("master");
        let path = address_type_to_path(AddressType::NativeSegwit, 0, 0, 0).expect("path");
        let child = master.derive(&path).expect("child");
        let signer = Signer::from_xprv(&child).expect("signer");

        let direct = Signer::from_secret_bytes(Secret::new(child.scalar().expose().clone()));
        assert_eq!(signer.public_key(), direct.public_key());
    }
}
