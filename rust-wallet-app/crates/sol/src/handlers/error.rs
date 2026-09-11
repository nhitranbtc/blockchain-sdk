//! Error classifier — maps `sol_wallet_core::Error` variants to stable
//! exit codes (0/1/2/3/4/5) per Phase 7 audit `P5-1` corrected mapping.
//!
//! DO NOT use deep-dive §J L1825 (spec table is inverted vs code).
//! Use deep-dive §L L2738–2744 + output-conventions L2843 (these two agree).
//!
//! Exit code map:
//!   0 — success
//!   1 — anyhow default (unclassified)
//!   2 — invalid input (mnemonic / address / amount / token program / token state
//!       / derivation / seed / base58 secret)
//!   3 — transport / RPC / broadcast (Transport, Rpc, BroadcastFailed,
//!       ConfirmTimeout, ComputeBudgetExceeded, ConfirmPending, InsufficientFunds)
//!   4 — wallet errors (WalletNotFound, WalletDecryptFailed, InsecureSourceFile,
//!       UnsupportedBlobVersion, InvalidKdfParams, InvalidWalletId)
//!   5 — sign / persistence / config / internal (OsRngFailed, FileIo, KdfFailed,
//!       CipherInit, RecordCorrupt, Unimplemented)
//!
//! Note: deep-dive §L L2738–2744 references `SignFailed`, `ConfigInvalid`, `PalError`,
//! `InvalidCluster`, `InvalidDerivationPath`, `InvalidTokenMint` — these were
//! aspirational spec names. The shipped `sol_wallet_core::Error` enum (Phase 6
//! landed commit `56a5bfe3`) uses different names; the CLI classifies whatever
//! variants exist today. Unmapped variants fall through to exit 1.

use sol_wallet_core::Error;

pub fn classify(err: &anyhow::Error) -> i32 {
    for cause in err.chain() {
        if let Some(lib_err) = cause.downcast_ref::<Error>() {
            return match lib_err {
                // Exit 2 — invalid input.
                Error::InvalidMnemonic
                | Error::DerivationFailed(_)
                | Error::InvalidSeed
                | Error::InvalidBase58Secret { .. }
                | Error::InvalidAddress(_)
                | Error::InvalidAmount(_)
                | Error::InvalidTokenProgram(_)
                | Error::InvalidTokenState(_) => 2,

                // Exit 3 — transport / RPC / broadcast.
                Error::Transport(_)
                | Error::Rpc { .. }
                | Error::InsufficientFunds { .. }
                | Error::BroadcastFailed { .. }
                | Error::ConfirmTimeout { .. }
                | Error::ComputeBudgetExceeded { .. }
                | Error::ConfirmPending { .. } => 3,

                // Exit 4 — wallet errors.
                Error::WalletNotFound(_)
                | Error::WalletDecryptFailed { .. }
                | Error::InsecureSourceFile { .. }
                | Error::UnsupportedBlobVersion { .. }
                | Error::InvalidKdfParams { .. }
                | Error::InvalidWalletId { .. } => 4,

                // Exit 5 — sign / persistence / config / internal.
                Error::OsRngFailed { .. }
                | Error::FileIo { .. }
                | Error::KdfFailed { .. }
                | Error::CipherInit
                | Error::RecordCorrupt { .. }
                | Error::Unimplemented(_) => 5,
            };
        }
    }
    1 // anyhow default (unclassified)
}
