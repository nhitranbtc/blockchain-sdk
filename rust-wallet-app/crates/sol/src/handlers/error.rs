//! Error classifier — maps `sol_wallet_core::Error` variants to the shared
//! cross-chain categories; the shared table maps categories to exit codes.
//!
//! The variant→category mapping is Solana's own: chain-specific granularity
//! is a security feature and is deliberately not unified across chains. What
//! IS unified is the exit code — `wallet_errors::ErrorCategory::exit_code()`
//! is the only exit-code table in the workspace.
//!
//! Exit codes (from `wallet-errors`):
//!   0 — success
//!   1 — user error      (invalid input, missing file, empty passphrase, ...)
//!   2 — network error   (transport, RPC, timeout, malformed response)
//!   3 — insufficient funds
//!   4 — signing error   (key material, credential, crypto, internal)
//!   5 — broadcast error (the cluster rejected the transaction)

use sol_wallet_core::Error;
use wallet_errors::ErrorCategory;

/// Classify a Solana core error into its cross-chain category.
pub fn classify_core(err: &Error) -> ErrorCategory {
    match err {
        // Network — the cluster did not answer usefully.
        Error::Transport(_)
        | Error::Rpc { .. }
        | Error::ConfirmTimeout { .. }
        | Error::ConfirmPending { .. } => ErrorCategory::Network,

        // User — the operator supplied something unusable.
        Error::InvalidMnemonic
        | Error::InvalidBase58Secret { .. }
        | Error::InvalidAddress(_)
        | Error::InvalidAmount(_)
        | Error::InvalidTokenProgram(_)
        | Error::InvalidTokenState(_)
        | Error::InvalidWalletId { .. }
        | Error::InvalidStorageName { .. }
        | Error::InsecureSourceFile { .. }
        | Error::ComputeBudgetExceeded { .. }
        | Error::FileIo { .. }
        | Error::WalletNotFound(_) => ErrorCategory::User,

        // Insufficient funds — the balance will not cover amount + fee.
        Error::InsufficientFunds { .. } => ErrorCategory::InsufficientFunds,

        // Broadcast — the cluster accepted the request and rejected the tx.
        Error::BroadcastFailed { .. } => ErrorCategory::Broadcast,

        // Signing — key material, credentials, crypto, internal machinery.
        Error::DerivationFailed(_)
        | Error::InvalidSeed
        | Error::WalletDecryptFailed { .. }
        | Error::UnsupportedBlobVersion { .. }
        | Error::OsRngFailed { .. }
        | Error::KdfFailed { .. }
        | Error::CipherInit
        | Error::RecordCorrupt { .. }
        | Error::InvalidKdfParams { .. }
        | Error::InvalidTransaction(_)
        | Error::Unimplemented(_) => ErrorCategory::Signing,
    }
}

/// Classify an anyhow error chain into its cross-chain category.
///
/// Walks the chain for a `sol_wallet_core::Error`; an error that never
/// reached the core (a CLI-level rejection) is a user error.
pub fn classify(err: &anyhow::Error) -> ErrorCategory {
    for cause in err.chain() {
        if let Some(lib_err) = cause.downcast_ref::<Error>() {
            return classify_core(lib_err);
        }
    }
    ErrorCategory::User
}

#[cfg(test)]
mod tests {
    use super::*;
    use sol_wallet_core::Error as CoreError;
    use wallet_errors::ErrorCategory;

    /// The `WalletId` inner field is private, so the id is built through
    /// its public parser.
    fn test_wallet_id() -> sol_wallet_core::WalletId {
        sol_wallet_core::WalletId::parse_str("00000000-0000-0000-0000-000000000000")
            .expect("a valid nil UUID parses")
    }

    #[test]
    fn every_solana_variant_maps_to_its_documented_category() {
        let cases: Vec<(CoreError, ErrorCategory)> = vec![
            // Network.
            (CoreError::Transport("down".into()), ErrorCategory::Network),
            (
                CoreError::Rpc {
                    code: -32000,
                    message: "x".into(),
                },
                ErrorCategory::Network,
            ),
            (
                CoreError::ConfirmTimeout {
                    signature: "s".into(),
                    waited_ms: 1,
                },
                ErrorCategory::Network,
            ),
            (
                CoreError::ConfirmPending {
                    signature: "s".into(),
                    commitment: solana_commitment_config::CommitmentConfig::processed(),
                    elapsed_ms: 1,
                },
                ErrorCategory::Network,
            ),
            // User.
            (CoreError::InvalidMnemonic, ErrorCategory::User),
            (
                CoreError::InvalidBase58Secret {
                    got: 1,
                    expected: 64,
                },
                ErrorCategory::User,
            ),
            (CoreError::InvalidAddress("x".into()), ErrorCategory::User),
            (CoreError::InvalidAmount("x".into()), ErrorCategory::User),
            (
                CoreError::InvalidTokenProgram("x".into()),
                ErrorCategory::User,
            ),
            (
                CoreError::InvalidTokenState("x".into()),
                ErrorCategory::User,
            ),
            (
                CoreError::InvalidWalletId { input: "x".into() },
                ErrorCategory::User,
            ),
            (
                CoreError::InvalidStorageName { name: "x".into() },
                ErrorCategory::User,
            ),
            (
                CoreError::InsecureSourceFile {
                    path: "/tmp/x".into(),
                    mode: 0o644,
                },
                ErrorCategory::User,
            ),
            (
                CoreError::ComputeBudgetExceeded {
                    needed_cu: 2,
                    available_cu: 1,
                },
                ErrorCategory::User,
            ),
            (
                CoreError::FileIo {
                    path: "/tmp/x".into(),
                    source: std::io::Error::other("x"),
                },
                ErrorCategory::User,
            ),
            (
                CoreError::WalletNotFound(test_wallet_id()),
                ErrorCategory::User,
            ),
            // Insufficient funds.
            (
                CoreError::InsufficientFunds { needed: 2, have: 1 },
                ErrorCategory::InsufficientFunds,
            ),
            // Broadcast.
            (
                CoreError::BroadcastFailed {
                    kind: "k".into(),
                    context: "c".into(),
                },
                ErrorCategory::Broadcast,
            ),
            // Signing.
            (
                CoreError::DerivationFailed("x".into()),
                ErrorCategory::Signing,
            ),
            (CoreError::InvalidSeed, ErrorCategory::Signing),
            (
                CoreError::WalletDecryptFailed {
                    id: test_wallet_id(),
                },
                ErrorCategory::Signing,
            ),
            (
                CoreError::UnsupportedBlobVersion {
                    found: 9,
                    lo: 1,
                    hi: 2,
                },
                ErrorCategory::Signing,
            ),
            (
                CoreError::OsRngFailed {
                    source: getrandom::Error::UNSUPPORTED,
                },
                ErrorCategory::Signing,
            ),
            (
                CoreError::KdfFailed {
                    message: "x".into(),
                },
                ErrorCategory::Signing,
            ),
            (CoreError::CipherInit, ErrorCategory::Signing),
            (
                CoreError::RecordCorrupt {
                    context: "ctx",
                    message: "msg".into(),
                },
                ErrorCategory::Signing,
            ),
            (
                CoreError::InvalidKdfParams {
                    memory_kb: 8,
                    iterations: 1,
                    parallelism: 1,
                },
                ErrorCategory::Signing,
            ),
            (
                CoreError::InvalidTransaction("x".into()),
                ErrorCategory::Signing,
            ),
            (CoreError::Unimplemented("x"), ErrorCategory::Signing),
        ];
        assert_eq!(
            cases.len(),
            29,
            "the table has one row per Solana error variant"
        );
        for (err, expected) in cases {
            assert_eq!(classify_core(&err), expected, "wrong category for {err:?}");
        }
    }

    #[test]
    fn unclassified_anyhow_error_is_a_user_error() {
        let err = anyhow::anyhow!("something the CLI rejected before the core");
        assert_eq!(classify(&err), ErrorCategory::User);
    }

    #[test]
    fn insufficient_funds_is_not_reported_as_a_transport_error() {
        // Review Focus #4: Solana today maps InsufficientFunds to the same
        // code as Transport. The two must be distinguishable.
        let err = anyhow::Error::new(CoreError::InsufficientFunds { needed: 2, have: 1 });
        assert_eq!(classify(&err), ErrorCategory::InsufficientFunds);
        assert_eq!(classify(&err).exit_code(), 3);
        assert_ne!(
            classify(&err).exit_code(),
            classify(&anyhow::Error::new(CoreError::Transport("down".into()))).exit_code()
        );
    }

    #[test]
    fn wrong_password_and_corrupt_blob_are_indistinguishable() {
        // Review Focus #1: a wrong passphrase and a corrupt blob must land
        // on the same category, so the two cannot be told apart by exit code.
        let wrong_pw = classify_core(&CoreError::WalletDecryptFailed {
            id: test_wallet_id(),
        });
        let corrupt = classify_core(&CoreError::RecordCorrupt {
            context: "ctx",
            message: "msg".into(),
        });
        assert_eq!(wrong_pw, ErrorCategory::Signing);
        assert_eq!(wrong_pw, corrupt);
        assert_eq!(wrong_pw.exit_code(), 4);
    }

    #[test]
    fn a_rejected_transaction_is_distinguishable_from_an_unreachable_node() {
        // Review Focus #5: automation must not retry a transaction the
        // cluster will never accept.
        let rejected = classify_core(&CoreError::BroadcastFailed {
            kind: "k".into(),
            context: "c".into(),
        });
        let unreachable = classify_core(&CoreError::Transport("down".into()));
        assert_eq!(rejected, ErrorCategory::Broadcast);
        assert_eq!(rejected.exit_code(), 5);
        assert_ne!(rejected.exit_code(), unreachable.exit_code());
    }
}
