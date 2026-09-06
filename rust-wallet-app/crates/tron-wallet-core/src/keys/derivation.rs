//! BIP-32 derivation to a signing keypair (plan Task 1.2).

use anychain_core::PublicKey as _;
use anychain_kms::bip32::{PrivateKey as _, XprvSecp256k1};
use anychain_tron::TronPublicKey;
use core::fmt;
use zeroize::Zeroizing;

use super::{DerivationPath, Mnemonic};
use crate::error::{Error, Result};

/// Length in bytes of a secp256k1 secret scalar.
pub const SECRET_KEY_LEN: usize = 32;

/// A derived TRON signing key and its public half.
///
/// The secret is stored as raw bytes inside [`Zeroizing`] rather than as a
/// `libsecp256k1::SecretKey`, for two reasons: that type has no zeroizing
/// `Drop`, and `anychain_kms::secp256k1_sign` wants a `&[u8]` anyway, so
/// keeping bytes avoids a re-serialization at every signature.
#[derive(Clone)]
pub struct KeyPair {
    secret: Zeroizing<[u8; SECRET_KEY_LEN]>,
    public: TronPublicKey,
}

impl KeyPair {
    /// The public key, for address derivation.
    pub fn public_key(&self) -> &TronPublicKey {
        &self.public
    }

    /// The raw secret scalar, for `tx::sign`.
    ///
    /// Callers must keep it inside `Zeroizing`; copying the bytes back out
    /// into a plain array defeats the wrapper.
    pub fn secret_bytes(&self) -> &Zeroizing<[u8; SECRET_KEY_LEN]> {
        &self.secret
    }
}

/// Keeps the secret out of logs and panic messages.
impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("secret", &"<redacted>")
            .field("public", &self.public.to_string())
            .finish()
    }
}

/// Derives a keypair at `path` from `mnemonic`.
///
/// `passphrase` is the BIP-39 passphrase; pass `""` for the common case. The
/// plan sketched this without the parameter, but the passphrase is part of the
/// seed — omitting it here would mean passphrase wallets need a second entry
/// point later, and two derivation paths through one crate is how key-handling
/// bugs start.
pub fn derive_keypair(
    mnemonic: &Mnemonic,
    passphrase: &str,
    path: &DerivationPath,
) -> Result<KeyPair> {
    let seed = mnemonic.to_seed(passphrase);

    // `xprv` holds a non-zeroizing `libsecp256k1::SecretKey`; it is read once
    // here and dropped at the end of this function.
    //
    // The seed is passed as a slice, not as `*seed`. `new_from_path` is generic
    // over `AsRef<[u8]>`, and dereferencing the `Zeroizing` would hand it a
    // `Copy` array by value — a second, unwiped copy of the seed. Clippy's
    // `needless_borrows_for_generic_args` suggests exactly that; a slice
    // satisfies the lint without duplicating secret material.
    let xprv = XprvSecp256k1::new_from_path(seed.as_slice(), path)
        .map_err(|e| Error::Derivation(e.to_string()))?;

    let public = TronPublicKey::from_secret_key(xprv.private_key());

    // Built inside the wrapper rather than as a bare `[u8; 32]` that is then
    // handed to `Zeroizing::new`. An array is `Copy`, so that shape would copy
    // the scalar and leave the original binding on the stack, unwiped, for the
    // rest of the frame — the exact leak `Zeroizing` is here to prevent.
    let mut secret = Zeroizing::new([0u8; SECRET_KEY_LEN]);
    {
        let serialized = Zeroizing::new(xprv.private_key().to_bytes());
        if serialized.len() != SECRET_KEY_LEN {
            return Err(Error::Derivation(format!(
                "expected a {SECRET_KEY_LEN}-byte secret, got {}",
                serialized.len()
            )));
        }
        secret.copy_from_slice(serialized.as_slice());
    }

    Ok(KeyPair { secret, public })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{Language, DEFAULT_DERIVATION_PATH};

    const CANONICAL: &str = "abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon abandon abandon about";

    fn mnemonic() -> Mnemonic {
        Mnemonic::from_phrase(CANONICAL, Language::English).expect("valid phrase")
    }

    fn path(s: &str) -> DerivationPath {
        s.parse().expect("valid path")
    }

    #[test]
    fn derives_at_the_default_path() {
        let keypair = derive_keypair(&mnemonic(), "", &path(DEFAULT_DERIVATION_PATH))
            .expect("derivation must succeed");

        assert_eq!(keypair.secret_bytes().len(), SECRET_KEY_LEN);
        assert_ne!(**keypair.secret_bytes(), [0u8; SECRET_KEY_LEN]);
    }

    #[test]
    fn derivation_is_deterministic() {
        let a = derive_keypair(&mnemonic(), "", &path(DEFAULT_DERIVATION_PATH)).expect("derive");
        let b = derive_keypair(&mnemonic(), "", &path(DEFAULT_DERIVATION_PATH)).expect("derive");

        assert_eq!(**a.secret_bytes(), **b.secret_bytes());
        assert_eq!(a.public_key(), b.public_key());
    }

    #[test]
    fn sibling_indices_give_different_keys() {
        let first = derive_keypair(&mnemonic(), "", &path("m/44'/195'/0'/0/0")).expect("derive");
        let second = derive_keypair(&mnemonic(), "", &path("m/44'/195'/0'/0/1")).expect("derive");

        assert_ne!(**first.secret_bytes(), **second.secret_bytes());
        assert_ne!(first.public_key(), second.public_key());
    }

    #[test]
    fn passphrase_changes_the_key() {
        let plain = derive_keypair(&mnemonic(), "", &path(DEFAULT_DERIVATION_PATH));
        let salted = derive_keypair(&mnemonic(), "TREZOR", &path(DEFAULT_DERIVATION_PATH));

        assert_ne!(
            **plain.expect("derive").secret_bytes(),
            **salted.expect("derive").secret_bytes()
        );
    }

    #[test]
    fn master_path_derives() {
        // "m" is the root with no children; derivation must still work.
        assert!(derive_keypair(&mnemonic(), "", &path("m")).is_ok());
    }

    #[test]
    fn debug_does_not_leak_the_secret() {
        let keypair = derive_keypair(&mnemonic(), "", &path(DEFAULT_DERIVATION_PATH))
            .expect("derivation must succeed");

        let secret_hex = hex::encode(**keypair.secret_bytes());
        let rendered = format!("{keypair:?}");

        assert!(!rendered.contains(&secret_hex), "leaked: {rendered}");
        assert!(rendered.contains("redacted"));
    }
}
