//! Sign-only path (plan Task 1.5, spike V8).
//!
//! Two upstream behaviours are worked around here rather than upstream, per the
//! plan's decision to pin `anychain` exactly instead of forking it:
//!
//! * **Risk 2** — `anychain_tron`'s transaction id is a single SHA-256, but the
//!   TRON wire format hashes twice. [`txid`] does the second pass caller-side.
//! * **Risk 3** — `anychain_kms::secp256k1_sign` takes a plain `&[u8]` and
//!   never clears it. [`sign_hash`] therefore requires the caller to hold the
//!   secret in `Zeroizing` and only lends it for the duration of the call.

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::keys::SECRET_KEY_LEN;

/// Length of a compact secp256k1 signature: `r || s`, 32 bytes each.
pub const SIGNATURE_LEN: usize = 64;

/// Length of the message digest TRON signs.
pub const MESSAGE_LEN: usize = 32;

/// A compact `r || s` signature.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature([u8; SIGNATURE_LEN]);

impl Signature {
    /// The 64 compact bytes.
    pub fn as_bytes(&self) -> &[u8; SIGNATURE_LEN] {
        &self.0
    }
}

impl core::fmt::Debug for Signature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Signature")
            .field(&hex::encode(self.0))
            .finish()
    }
}

/// A secp256k1 recovery id in the range TRON accepts.
///
/// A newtype rather than a bare `u8` because the two neighbouring conventions
/// are easy to confuse: Ethereum offsets the same value by 27, and libsecp256k1
/// can in principle report 2 or 3. Both produce a transaction a TRON node
/// rejects. Making the only constructor validating means a wrong value cannot
/// reach the wire form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecoveryId(u8);

impl RecoveryId {
    /// Accepts 0 or 1; rejects everything else, including Ethereum's 27/28.
    pub fn new(v: u8) -> Result<Self> {
        if v > 1 {
            return Err(Error::Signing(format!(
                "recovery id {v} is outside TRON's accepted range of 0..=1"
            )));
        }
        Ok(Self(v))
    }

    /// The raw value, for serialization.
    pub fn to_u8(self) -> u8 {
        self.0
    }
}

/// A signature together with the recovery id that belongs to it.
///
/// The two travel as one value rather than a `(Signature, u8)` tuple so a
/// caller cannot drop the recovery id, or pair a signature with the id from a
/// different message. Both mistakes produce a transaction that a node refuses
/// for reasons that are hard to trace back to the call site.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Signed {
    signature: Signature,
    recovery_id: RecoveryId,
}

impl Signed {
    /// The compact `r || s` half.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// The recovery id.
    pub fn recovery_id(&self) -> RecoveryId {
        self.recovery_id
    }

    /// The 65-byte `r || s || v` form TRON puts on the wire.
    pub fn to_tron_bytes(&self) -> [u8; SIGNATURE_LEN + 1] {
        let mut out = [0u8; SIGNATURE_LEN + 1];
        out[..SIGNATURE_LEN].copy_from_slice(&self.signature.0);
        out[SIGNATURE_LEN] = self.recovery_id.0;
        out
    }
}

/// Signs a 32-byte digest.
///
/// `msg32` must already be the digest to sign — this function does not hash its
/// input.
///
/// # Residual exposure
///
/// The `Zeroizing` boundary is tight on this side of the call and no tighter.
/// `anychain_kms::secp256k1_sign` takes a `&[u8]` and immediately copies it
/// into a `libsecp256k1::SecretKey`, which has no zeroizing `Drop`. That copy
/// lives on a stack frame this crate does not control and is not wiped when the
/// call returns. Closing that window would mean forking `anychain-kms` or
/// hand-rolling the signature; neither is in v0.1 scope, so the honest
/// statement is that the secret is wiped everywhere *this crate* holds it.
pub fn sign_hash(
    secret: &Zeroizing<[u8; SECRET_KEY_LEN]>,
    msg32: &[u8; MESSAGE_LEN],
) -> Result<Signed> {
    let (raw, v) = anychain_kms::secp256k1_sign(secret.as_slice(), msg32)
        .map_err(|e| Error::Signing(e.to_string()))?;

    let bytes: [u8; SIGNATURE_LEN] = raw.as_slice().try_into().map_err(|_| {
        Error::Signing(format!(
            "expected a {SIGNATURE_LEN}-byte compact signature, got {}",
            raw.len()
        ))
    })?;

    // libsecp256k1 normalizes `s` into the low half of the curve order and
    // re-drives the nonce when `r` overflows, so 2 and 3 are unreachable in
    // practice. Kept as a guard against an upstream refactor: if one ever
    // surfaces, erroring is right — a caller cannot repair it by retrying,
    // because the nonce is a deterministic function of (secret, message).
    Ok(Signed {
        signature: Signature(bytes),
        recovery_id: RecoveryId::new(v)?,
    })
}

/// Computes a TRON transaction id: SHA-256 applied twice to the raw bytes.
///
/// `anychain_tron::TronTransaction::to_transaction_id` hashes once, which does
/// not match what the network indexes. Any caller that needs an id must use
/// this function; `txid_is_double_sha256` in `tests/v8_sign_only.rs` fails if
/// an `anychain` bump ever changes that behaviour underneath us.
pub fn txid(raw_bytes: &[u8]) -> [u8; MESSAGE_LEN] {
    Sha256::digest(Sha256::digest(raw_bytes)).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid scalar for signing tests. Not derived from any real wallet.
    fn test_secret() -> Zeroizing<[u8; SECRET_KEY_LEN]> {
        Zeroizing::new([7u8; SECRET_KEY_LEN])
    }

    #[test]
    fn produces_a_compact_signature_with_a_tron_recovery_id() {
        let signed = sign_hash(&test_secret(), &[0x11; MESSAGE_LEN]).expect("sign");

        assert_eq!(signed.signature().as_bytes().len(), SIGNATURE_LEN);
        assert!(signed.recovery_id().to_u8() <= 1);
    }

    #[test]
    fn wire_form_appends_the_recovery_id() {
        let signed = sign_hash(&test_secret(), &[0x11; MESSAGE_LEN]).expect("sign");
        let wire = signed.to_tron_bytes();

        assert_eq!(wire.len(), 65);
        assert_eq!(&wire[..SIGNATURE_LEN], signed.signature().as_bytes());
        assert_eq!(wire[SIGNATURE_LEN], signed.recovery_id().to_u8());
    }

    #[test]
    fn recovery_id_rejects_values_tron_will_not_accept() {
        assert!(RecoveryId::new(0).is_ok());
        assert!(RecoveryId::new(1).is_ok());

        // 2 and 3 are libsecp256k1's overflow ids; 27 and 28 are Ethereum's.
        for rejected in [2u8, 3, 27, 28, 255] {
            assert!(
                RecoveryId::new(rejected).is_err(),
                "accepted recovery id {rejected}"
            );
        }
    }

    #[test]
    fn rejects_an_all_zero_secret() {
        let zero = Zeroizing::new([0u8; SECRET_KEY_LEN]);

        assert!(matches!(
            sign_hash(&zero, &[0x11; MESSAGE_LEN]),
            Err(Error::Signing(_))
        ));
    }

    #[test]
    fn rejects_a_secret_at_or_above_the_curve_order() {
        // All-0xff is larger than secp256k1's group order.
        let too_large = Zeroizing::new([0xffu8; SECRET_KEY_LEN]);

        assert!(sign_hash(&too_large, &[0x11; MESSAGE_LEN]).is_err());
    }

    #[test]
    fn signing_is_deterministic() {
        let msg = [0x42; MESSAGE_LEN];

        let a = sign_hash(&test_secret(), &msg).expect("sign");
        let b = sign_hash(&test_secret(), &msg).expect("sign");

        assert_eq!(a, b);
    }

    #[test]
    fn debug_shows_hex_not_raw_bytes() {
        let signed = sign_hash(&test_secret(), &[0x11; MESSAGE_LEN]).expect("sign");
        let rendered = format!("{:?}", signed.signature());

        assert!(rendered.starts_with("Signature("));
        assert!(rendered.contains(&hex::encode(signed.signature().as_bytes())));
    }

    /// Independently confirmed:
    ///
    /// ```text
    /// $ printf '' | sha256sum | cut -d' ' -f1 | xxd -r -p | sha256sum
    /// 5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456
    /// ```
    #[test]
    fn txid_hashes_twice() {
        assert_eq!(
            hex::encode(txid(b"")),
            "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456"
        );
    }

    #[test]
    fn txid_is_not_a_single_hash() {
        let raw = b"some raw transaction bytes";
        let single: [u8; MESSAGE_LEN] = Sha256::digest(raw).into();

        assert_ne!(txid(raw), single);
    }
}
