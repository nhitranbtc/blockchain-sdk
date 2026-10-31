//! Error classifier — maps the CLI's core error variants to the shared
//! cross-chain categories; the shared table maps categories to exit codes.
//!
//! The variant→category mapping is this CLI's own: chain-specific granularity
//! is a security feature and is deliberately not unified across chains. What
//! IS unified is the exit code — `wallet_errors::ErrorCategory::exit_code()`
//! is the only exit-code table in the workspace.
//!
//! Exit codes (from `wallet-errors`):
//!   0 — success
//!   1 — user error      (invalid input, unknown wallet, empty passphrase)
//!   2 — network error   (transport, RPC, timeout, malformed response)
//!   3 — insufficient funds
//!   4 — signing error   (key material, credential, crypto, internal)
//!   5 — broadcast error (the node rejected the transaction)

use polygon_wallet_core::Error;
use wallet_errors::ErrorCategory;

/// Maps a core error to its cross-chain category.
pub fn classify(err: &Error) -> ErrorCategory {
    match err {
        // Network — the node did not answer usefully.
        Error::Rpc(_) | Error::GasEstimateFailed(_) | Error::ReceiptTimeout { .. } => {
            ErrorCategory::Network
        }
        // User — the operator supplied something unusable.
        Error::InvalidMnemonic(_)
        | Error::InvalidPrivateKey(_)
        | Error::InvalidInput(_)
        | Error::AbiDecodeFailed { .. }
        | Error::NonceMismatch { .. }
        | Error::FeeTooLow { .. }
        | Error::WalletNotFound { .. }
        | Error::WalletNotFoundByName { .. }
        | Error::WalletExists { .. }
        | Error::InvalidPassword(_) => ErrorCategory::User,
        // Insufficient funds.
        Error::InsufficientFunds { .. } | Error::InsufficientTokenBalance { .. } => {
            ErrorCategory::InsufficientFunds
        }
        // Signing — credential, key material, or a tampered blob.
        Error::WalletCorrupt { .. } | Error::EncryptionFailed(_) | Error::DecryptionFailed(_) => {
            ErrorCategory::Signing
        }
        // Broadcast — the node rejected the transaction.
        Error::TxBroadcastFailed(_) => ErrorCategory::Broadcast,
        // Future variants are internal failures, never operator input.
        _ => ErrorCategory::Signing,
    }
}

#[cfg(test)]
mod tests {
    use polygon_wallet_core::Error;
    use wallet_errors::ErrorCategory;

    use super::classify;

    #[test]
    fn every_evm_variant_maps_to_its_documented_category() {
        let cases: Vec<(Error, ErrorCategory)> = vec![
            (Error::Rpc("down".into()), ErrorCategory::Network),
            (Error::GasEstimateFailed("x".into()), ErrorCategory::Network),
            (
                Error::ReceiptTimeout {
                    secs: 1,
                    tx_hash: "h".into(),
                },
                ErrorCategory::Network,
            ),
            (Error::InvalidMnemonic("x".into()), ErrorCategory::User),
            (Error::InvalidPrivateKey("x".into()), ErrorCategory::User),
            (Error::InvalidInput("x".into()), ErrorCategory::User),
            (
                Error::AbiDecodeFailed {
                    context: "c".into(),
                    reason: "r".into(),
                },
                ErrorCategory::User,
            ),
            (
                Error::NonceMismatch {
                    submitted: 1,
                    current: 2,
                },
                ErrorCategory::User,
            ),
            (
                Error::FeeTooLow {
                    max_fee_per_gas: 1,
                    min_required: 2,
                },
                ErrorCategory::User,
            ),
            (
                Error::WalletNotFound {
                    wallet_id: uuid::Uuid::nil(),
                },
                ErrorCategory::User,
            ),
            (
                Error::WalletNotFoundByName {
                    name: "n".into(),
                    network: "sepolia".into(),
                },
                ErrorCategory::User,
            ),
            (
                Error::WalletExists {
                    name: "n".into(),
                    network: "sepolia".into(),
                },
                ErrorCategory::User,
            ),
            (Error::InvalidPassword("x".into()), ErrorCategory::User),
            (
                Error::InsufficientFunds {
                    needed_wei: "2".into(),
                    have_wei: "1".into(),
                },
                ErrorCategory::InsufficientFunds,
            ),
            (
                Error::InsufficientTokenBalance {
                    token_symbol: "USDC".into(),
                    needed_base_units: "2".into(),
                    have_base_units: "1".into(),
                },
                ErrorCategory::InsufficientFunds,
            ),
            (
                Error::WalletCorrupt {
                    path: "/tmp/w".into(),
                    reason: "r".into(),
                },
                ErrorCategory::Signing,
            ),
            (Error::EncryptionFailed("x".into()), ErrorCategory::Signing),
            (Error::DecryptionFailed("x".into()), ErrorCategory::Signing),
            (
                Error::TxBroadcastFailed("x".into()),
                ErrorCategory::Broadcast,
            ),
        ];
        assert_eq!(cases.len(), 19, "this table pins 19 EVM variants — the enum is #[non_exhaustive], so a new variant must add a row here");
        for (err, expected) in cases {
            assert_eq!(classify(&err), expected, "wrong category for {err:?}");
        }
    }

    #[test]
    fn wrong_password_and_corrupt_blob_are_indistinguishable() {
        // Review Focus #1.
        let wrong_pw = classify(&Error::DecryptionFailed("bad pw".into()));
        let corrupt = classify(&Error::WalletCorrupt {
            path: "/tmp/w".into(),
            reason: "r".into(),
        });
        assert_eq!(wrong_pw, corrupt);
        assert_eq!(wrong_pw.exit_code(), 4);
    }

    #[test]
    fn empty_passphrase_is_a_user_error_not_a_signing_error() {
        // Review Focus #2: an empty passphrase is operator input; a wrong
        // passphrase is a credential failure. They must not share a code.
        let empty = classify(&Error::InvalidPassword("password must be non-empty".into()));
        assert_eq!(empty, ErrorCategory::User);
        assert_eq!(empty.exit_code(), 1);
        assert_ne!(
            empty.exit_code(),
            classify(&Error::DecryptionFailed("bad pw".into())).exit_code()
        );
    }

    #[test]
    fn a_rejected_transaction_is_distinguishable_from_an_unreachable_node() {
        // Review Focus #5: automation must not retry a transaction the node
        // will never accept.
        let rejected = classify(&Error::TxBroadcastFailed("reverted".into()));
        let unreachable = classify(&Error::Rpc("down".into()));
        assert_eq!(rejected, ErrorCategory::Broadcast);
        assert_eq!(rejected.exit_code(), 5);
        assert_ne!(rejected.exit_code(), unreachable.exit_code());
    }
}
