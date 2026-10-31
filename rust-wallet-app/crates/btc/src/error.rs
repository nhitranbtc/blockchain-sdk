//! Error classifier for the `btc` CLI.
//!
//! Bitcoin's `WalletStore` variant deliberately collapses not-found,
//! wrong-password, wrong-network and corrupt-blob into one observable
//! outcome (`bitcoin-wallet-core/src/error.rs:110-120`). That closure is why
//! `WalletStore` maps to `Signing` and not to `User`: not-found and
//! wrong-password must not be distinguishable by exit code.
//!
//! The variant→category mapping is Bitcoin's own: chain-specific granularity
//! is a security feature and is deliberately not unified across chains. What
//! IS unified is the exit code — `wallet_errors::ErrorCategory::exit_code()`
//! is the only exit-code table in the workspace.
//!
//! Exit codes (from `wallet-errors`):
//!   0 — success
//!   1 — user error      (invalid input, malformed descriptor, filesystem)
//!   2 — network error   (transport, RPC, timeout, malformed response)
//!   3 — insufficient funds
//!   4 — signing error   (key material, credential, crypto, internal)
//!   5 — broadcast error (the node rejected the transaction)

use bitcoin_wallet_core::error::Error;
use wallet_errors::ErrorCategory;

/// Maps a Bitcoin core error to its cross-chain category.
pub fn classify(err: &Error) -> ErrorCategory {
    match err {
        // Network — the backend did not answer usefully.
        Error::Network(_) | Error::Esplora(_) | Error::Electrum(_) => ErrorCategory::Network,
        // User — the operator supplied something unusable.
        Error::InvalidMnemonic(_)
        | Error::InvalidDerivationPath(_)
        | Error::Storage(_)
        | Error::NotInitialized(_)
        | Error::Bdk(_)
        | Error::Io(_)
        | Error::SpkiPin(_) => ErrorCategory::User,
        // Insufficient funds.
        Error::InsufficientFunds { .. } => ErrorCategory::InsufficientFunds,
        // Signing, transaction construction, and the collapsed
        // wallet-store outcome.
        Error::TxBuild(_)
        | Error::Sign(_)
        | Error::Psbt(_)
        | Error::AddressDerivation(_)
        | Error::ScriptBuild(_)
        | Error::Encryption(_)
        | Error::Bitcoin(_)
        | Error::Bip137(_)
        | Error::MnemonicCipher(_)
        | Error::WalletStore(_) => ErrorCategory::Signing,
        // Future variants are internal failures, never operator input.
        _ => ErrorCategory::Signing,
    }
}

/// Classifies an anyhow error chain.
///
/// Walks the chain for a `bitcoin_wallet_core::Error`; an error that never
/// reached the core (a CLI-level rejection) is a user error.
pub fn classify_anyhow(err: &anyhow::Error) -> ErrorCategory {
    for cause in err.chain() {
        if let Some(lib_err) = cause.downcast_ref::<Error>() {
            return classify(lib_err);
        }
    }
    ErrorCategory::User
}

#[cfg(test)]
mod tests {
    use bitcoin_wallet_core::error::Error;
    use wallet_errors::ErrorCategory;

    use super::{classify, classify_anyhow};

    #[test]
    fn every_bitcoin_variant_maps_to_its_documented_category() {
        let cases: Vec<(Error, ErrorCategory)> = vec![
            (Error::Network("x".into()), ErrorCategory::Network),
            (Error::Esplora("x".into()), ErrorCategory::Network),
            (Error::Electrum("x".into()), ErrorCategory::Network),
            (Error::InvalidMnemonic("x".into()), ErrorCategory::User),
            (
                Error::InvalidDerivationPath("x".into()),
                ErrorCategory::User,
            ),
            (Error::Storage("x".into()), ErrorCategory::User),
            (Error::NotInitialized("x".into()), ErrorCategory::User),
            (Error::Bdk("x".into()), ErrorCategory::User),
            (Error::Io(std::io::Error::other("x")), ErrorCategory::User),
            (Error::SpkiPin("x".into()), ErrorCategory::User),
            (
                Error::InsufficientFunds {
                    needed: 2,
                    available: 1,
                },
                ErrorCategory::InsufficientFunds,
            ),
            (Error::TxBuild("x".into()), ErrorCategory::Signing),
            (Error::Sign("x".into()), ErrorCategory::Signing),
            (Error::Psbt("x".into()), ErrorCategory::Signing),
            (Error::AddressDerivation("x".into()), ErrorCategory::Signing),
            (Error::ScriptBuild("x".into()), ErrorCategory::Signing),
            (Error::Encryption("x".into()), ErrorCategory::Signing),
            (
                Error::Bitcoin(bitcoin::consensus::encode::Error::ParseFailed("x")),
                ErrorCategory::Signing,
            ),
            (Error::Bip137("x".into()), ErrorCategory::Signing),
            (Error::MnemonicCipher("x".into()), ErrorCategory::Signing),
            (Error::WalletStore("x".into()), ErrorCategory::Signing),
        ];
        assert_eq!(cases.len(), 21, "this table pins 21 Bitcoin variants — the enum is #[non_exhaustive], so a new variant must add a row here");
        for (err, expected) in cases {
            assert_eq!(classify(&err), expected, "wrong category for {err:?}");
        }
    }

    #[test]
    fn wallet_store_is_not_distinguishable_from_a_wrong_password() {
        // Review Focus #1: the wallet-store variant collapses not-found,
        // wrong-password, wrong-network and corrupt-blob.
        assert_eq!(
            classify(&Error::WalletStore("x".into())),
            ErrorCategory::Signing
        );
    }

    #[test]
    fn unclassified_anyhow_error_is_a_user_error() {
        let err = anyhow::anyhow!("--password-file is empty");
        assert_eq!(classify_anyhow(&err), ErrorCategory::User);
    }
}
