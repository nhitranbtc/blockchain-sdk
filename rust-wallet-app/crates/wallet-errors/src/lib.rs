//! Shared error categories and the one exit-code table for all five CLIs.
//!
//! One category enum, one `ErrorCategory -> i32` table, one source of truth.
//! The per-chain error enums stay where they are — their granularity is a
//! security feature — and each CLI maps its own variants onto these six
//! categories in its own layer.
//!
//! The exit codes are the ones documented in `docs/wallets/CONTEXT.md`; the
//! `context_doc_parity` test asserts the document and this table agree.
//!
//! The CLI-local tables are migrated onto this one across Phase 1; until a
//! given CLI is migrated, it still carries its own copy.

#![warn(missing_docs)]

/// The six cross-chain failure categories.
///
/// The set is fixed to the six documented in `docs/wallets/CONTEXT.md` — six,
/// not eight. `Storage` and `Crypto` are deliberately *not* categories:
/// filesystem and permission failures are `User`, and credential / crypto /
/// internal-machinery failures are `Signing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// The operation completed.
    Success,
    /// Operator-supplied input or environment: malformed input, missing
    /// wallet, bad config, filesystem or permission failure, empty
    /// passphrase, declined confirmation.
    User,
    /// The node or transport failed to answer, timed out, or answered
    /// unintelligibly.
    Network,
    /// The balance will not cover the amount plus the fee.
    InsufficientFunds,
    /// Key material is unusable or the operation consuming it failed:
    /// wrong passphrase, decryption failure, corrupt blob, derivation,
    /// transaction construction, signature failure, internal machinery.
    Signing,
    /// The network accepted the request and rejected the transaction.
    Broadcast,
}

impl ErrorCategory {
    /// Every category, in exit-code order. Used by the doc-parity test.
    pub const ALL: [ErrorCategory; 6] = [
        ErrorCategory::Success,
        ErrorCategory::User,
        ErrorCategory::Network,
        ErrorCategory::InsufficientFunds,
        ErrorCategory::Signing,
        ErrorCategory::Broadcast,
    ];

    /// The process exit code for this category.
    ///
    /// This is the one `ErrorCategory -> i32` table in the workspace. The
    /// CLI-local tables (`sol`, `tron`, `evm-wallet-core`) are migrated onto
    /// it across Phase 1; until a given CLI is migrated, it still carries its
    /// own copy.
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorCategory::Success => 0,
            ErrorCategory::User => 1,
            ErrorCategory::Network => 2,
            ErrorCategory::InsufficientFunds => 3,
            ErrorCategory::Signing => 4,
            ErrorCategory::Broadcast => 5,
        }
    }

    /// The label used in the `docs/wallets/CONTEXT.md` category table.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCategory::Success => "Success",
            ErrorCategory::User => "User error",
            ErrorCategory::Network => "Network error",
            ErrorCategory::InsufficientFunds => "Insufficient funds",
            ErrorCategory::Signing => "Signing error",
            ErrorCategory::Broadcast => "Broadcast error",
        }
    }
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
