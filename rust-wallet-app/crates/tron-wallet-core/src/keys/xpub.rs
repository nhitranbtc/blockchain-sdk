//! Extended public key export (plan Task 1.6, Story 19).

use anychain_kms::bip32::{Prefix, XprvSecp256k1};

use super::{DerivationPath, Mnemonic};
use crate::error::{Error, Result};

/// Exports the SLIP-0132 `xpub` for `path`.
///
/// TRON reuses Bitcoin's `xpub` serialization, so a watch-only companion that
/// understands Bitcoin extended keys can consume this unchanged.
///
/// Pass an account-level path such as `m/44'/195'/0'`. Exporting at a deeper
/// path still works but yields a key that can only see one address chain,
/// which is rarely what a companion wants.
pub fn xpub(mnemonic: &Mnemonic, passphrase: &str, path: &DerivationPath) -> Result<String> {
    let seed = mnemonic.to_seed(passphrase);

    // Passed as a slice rather than `*seed`: dereferencing the `Zeroizing`
    // would hand this generic function a `Copy` array by value, leaving an
    // unwiped duplicate of the seed behind. See `derivation.rs` for the same
    // note.
    let xprv = XprvSecp256k1::new_from_path(seed.as_slice(), path)
        .map_err(|e| Error::Derivation(e.to_string()))?;

    Ok(xprv.public_key().to_string(Prefix::XPUB))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Language;

    const CANONICAL: &str = "abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon abandon abandon about";

    fn mnemonic() -> Mnemonic {
        Mnemonic::from_phrase(CANONICAL, Language::English).expect("valid phrase")
    }

    fn path(s: &str) -> DerivationPath {
        s.parse().expect("valid path")
    }

    #[test]
    fn exports_an_xpub_prefixed_key() {
        let exported = xpub(&mnemonic(), "", &path("m/44'/195'/0'")).expect("export");

        assert!(exported.starts_with("xpub"), "got {exported}");
    }

    #[test]
    fn export_is_deterministic() {
        let first = xpub(&mnemonic(), "", &path("m/44'/195'/0'")).expect("export");
        let second = xpub(&mnemonic(), "", &path("m/44'/195'/0'")).expect("export");

        assert_eq!(first, second);
    }

    #[test]
    fn accounts_and_passphrases_diverge() {
        let account_0 = xpub(&mnemonic(), "", &path("m/44'/195'/0'")).expect("export");
        let account_1 = xpub(&mnemonic(), "", &path("m/44'/195'/1'")).expect("export");
        let salted = xpub(&mnemonic(), "TREZOR", &path("m/44'/195'/0'")).expect("export");

        assert_ne!(account_0, account_1);
        assert_ne!(account_0, salted);
    }

    #[test]
    fn export_carries_no_private_material() {
        let exported = xpub(&mnemonic(), "", &path("m/44'/195'/0'")).expect("export");

        assert!(
            !exported.starts_with("xprv"),
            "an xprv leaked out of the xpub path: {exported}"
        );
    }
}
