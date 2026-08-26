//! Canonical `eth-wallet-core` Error type + Result alias.
//!
//! Per Issue #303 Task 4 acceptance:
//! - 17-variant Error enum mirroring the BTC `bitcoin-wallet-core::error::Error` schema
//! - `thiserror` impl with `Display + Error + From<...>` for the 5 external-error sources
//! - Stable exit-code mapping per #297 M11: 0/1/2/3/4/5 (success / user abort /
//!   bad input / upstream-RPC / wallet-balance / signing-RPC-broadcast)
//! - `pub type Result<T> = std::result::Result<T, Error>;` alias used across the crate
//!
//! Module-local error types (`crypto::CryptoError`, `wallet::WalletError`,
//! `signer::SignError`) stay where they are for v0.2. Future Tasks 5+ add
//! `From` impls so they convert cleanly into the canonical `Error`.

use thiserror::Error;

/// Canonical `eth-wallet-core` error type.
///
/// Each variant maps to a stable exit code via [`crate::error::Error::exit_code`]
/// per the #297 M11 taxonomy. CLI consumers (Task 10) translate exit codes
/// 0..=5 directly into `std::process::ExitCode`.
///
/// `#[non_exhaustive]` added per Issue #357 — downstream code matching
/// exhaustively on `Error` must add a wildcard arm. In-crate callers
/// (e.g. `exit_code()` itself) still see all variants.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Upstream RPC error: connection refused, HTTP failure, chain-id mismatch,
    /// malformed response. Exit 3.
    #[error("rpc: {0}")]
    Rpc(String),

    /// Mnemonic parse failure: invalid checksum, wrong word count, unexpected
    /// characters. Exit 2.
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    /// Raw private key parse failure: not 32 bytes, scalar out of range,
    /// hex decode error. Exit 2.
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),

    /// Operation references a `wallet_id` that doesn't exist on disk.
    /// Exit 4 (wallet/balance issue).
    #[error("wallet {wallet_id} not found")]
    WalletNotFound { wallet_id: uuid::Uuid },

    /// `create_wallet` or `import_wallet` collisions with an existing
    /// (name, network) pair. Exit 4.
    #[error("wallet '{name}' already exists on {network}")]
    WalletExists { name: String, network: String },

    /// Encrypted blob file exists but decrypt failed or JSON malformed.
    /// Exit 5 (signing/RPC/broadcast error — chosen for "post-on-disk tamper").
    #[error("corrupt wallet file at {path}: {reason}")]
    WalletCorrupt { path: String, reason: String },

    /// Empty password or otherwise unusable passphrase. Exit 2.
    #[error("invalid password: {0}")]
    InvalidPassword(String),

    /// Argon2id KDF failure (rare; typically means params out of range).
    /// Exit 5.
    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    /// AES-GCM decrypt failed (AEAD tag mismatch = wrong password OR tampered blob).
    /// Exit 5.
    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    /// Native-ETH transfer amount > sender balance minus gas. Exit 4.
    #[error("insufficient ETH for amount + gas: need {needed_wei} wei, have {have_wei} wei")]
    InsufficientFunds {
        needed_wei: String,
        have_wei: String,
    },

    /// ERC-20 transfer amount > token balance. Exit 4.
    #[error("insufficient token balance for {token_symbol}: need {needed_base_units}, have {have_base_units}")]
    InsufficientTokenBalance {
        token_symbol: String,
        needed_base_units: String,
        have_base_units: String,
    },

    /// Manual nonce submitted is < current on-chain nonce. Exit 2.
    #[error("nonce mismatch: submitted {submitted}, current {current}")]
    NonceMismatch { submitted: u64, current: u64 },

    /// Submitted EIP-1559 fee cap doesn't meet the network minimum or doesn't
    /// satisfy `max_priority_fee_per_gas <= max_fee_per_gas`. Exit 2.
    #[error("fee too low: max_fee_per_gas={max_fee_per_gas}, min_required={min_required}")]
    FeeTooLow {
        max_fee_per_gas: u128,
        min_required: u128,
    },

    /// `provider.estimate_gas` failed (contract reverts, gas-limit too low
    /// relative to introspection query, etc). Exit 3.
    #[error("gas estimation failed: {0}")]
    GasEstimateFailed(String),

    /// `Provider::send_transaction` rejected the broadcast (nonce taken,
    /// replacement-underpriced, etc.). Exit 5.
    #[error("tx broadcast failed: {0}")]
    TxBroadcastFailed(String),

    /// `pending.get_receipt()` polled past the deadline without inclusion.
    /// Exit 5.
    #[error("receipt timeout after {secs}s for tx_hash {tx_hash}")]
    ReceiptTimeout { secs: u64, tx_hash: String },

    /// CLI input failed to parse: bad address hex, bad tx-hash, bad unit,
    /// missing/incompatible flag combos. Distinct from `InvalidMnemonic` /
    /// `InvalidPrivateKey` which are library-domain-specific. Exit 2.
    /// Added in #337 (Task 13 follow-up to #309) so the CLI can surface
    /// its own parse failures through the same `exit_code()` table.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Wallet lookup by `(name, network)` failed. Distinct from
    /// `WalletNotFound` (which carries a `wallet_id`). Exit 4 — same as
    /// `WalletNotFound` per M11 wallet/balance category.
    /// Added in #337 fix for type-design CRITICAL: the previous
    /// `WalletNotFound { wallet_id: uuid::Uuid::nil() }` sentinel lost
    /// the user-supplied name.
    #[error("wallet '{name}' not found on {network}")]
    WalletNotFoundByName { name: String, network: String },

    /// ABI decoding failed on a response we did receive over the wire.
    /// Distinct from `Error::Rpc(_)` which means the RPC itself was
    /// unreachable or returned HTTP/transport failure. This variant means:
    /// the RPC answered `200 OK` with bytes that don't decode into the
    /// expected Solidity return type — typically because the target
    /// contract is not actually ERC-20 (or the function being queried
    /// doesn't match the selector). Exit 2 (bad input category — operator
    /// passed a token address that isn't what they thought).
    /// Added in Issue #357 (follow-up to #356) so operators can tell
    /// "contract doesn't speak ERC-20" apart from "RPC unreachable".
    #[error("ABI decode failed for {context}: {reason}")]
    AbiDecodeFailed { context: String, reason: String },
}

impl Error {
    /// Construct an `Error::Rpc` from any `Display` impl, routing the
    /// formatted string through `redact_rpc_url` so upstream RPC URLs
    /// (paths, query strings) never leak into operator-visible error
    /// messages. Issue #365 AC #1 — centralizes the redaction
    /// discipline that previously required every call site to remember
    /// `redact_rpc_url(&e)` (and which the security-guidance post-push
    /// sweep found 4+ sites still forgetting).
    ///
    /// Use this constructor for any error that flows from an RPC call
    /// (provider, transport, reqwest, alloy transport). For non-RPC
    /// errors (parse, validation), prefer the existing `Error::Rpc(s)`
    /// variant directly when redaction is irrelevant.
    ///
    /// Backward-compatible: old call sites using `Error::rpc(format!(...))`
    /// still compile, but their redaction responsibility is now on the
    /// author. New code should always use this constructor.
    ///
    /// Enforced by CI audit-gate `rust-error-grep` job in
    /// `.github/workflows/rust-eth-core-ci.yml` — any `Error::Rpc(format`
    /// match in `eth/` or `eth-wallet-core/` fails the workflow. See
    /// Issue #382 for the rationale.
    pub fn rpc(e: impl std::fmt::Display) -> Self {
        Error::Rpc(crate::redact_rpc_url(e))
    }
}

impl Error {
    /// Stable exit-code mapping per #297 M11.
    ///
    /// | Variant                            | Exit |
    /// |------------------------------------|-----:|
    /// | Rpc / GasEstimateFailed            |   3  |
    /// | InvalidMnemonic / InvalidPrivateKey / InvalidPassword / NonceMismatch / FeeTooLow | 2 |
    /// | WalletNotFound / WalletExists / InsufficientFunds / InsufficientTokenBalance | 4 |
    /// | WalletCorrupt / EncryptionFailed / DecryptionFailed / TxBroadcastFailed / ReceiptTimeout | 5 |
    ///
    /// User abort (exit 1) is generated by the CLI when `confirm_prompt`
    /// declines — never from this enum. Success (exit 0) likewise.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Rpc(_) | Error::GasEstimateFailed(_) => 3,

            Error::InvalidMnemonic(_)
            | Error::InvalidPrivateKey(_)
            | Error::InvalidPassword(_)
            | Error::NonceMismatch { .. }
            | Error::FeeTooLow { .. }
            | Error::InvalidInput(_)
            | Error::AbiDecodeFailed { .. } => 2,

            Error::WalletNotFound { .. }
            | Error::WalletNotFoundByName { .. }
            | Error::WalletExists { .. }
            | Error::InsufficientFunds { .. }
            | Error::InsufficientTokenBalance { .. } => 4,

            Error::WalletCorrupt { .. }
            | Error::EncryptionFailed(_)
            | Error::DecryptionFailed(_)
            | Error::TxBroadcastFailed(_)
            | Error::ReceiptTimeout { .. } => 5,
        }
    }
}

/// Crate-wide Result alias — every Task uses `Result<T>` rather than the
/// module-local aliases so error-conversion ergonomics stay uniform.
pub type Result<T> = std::result::Result<T, Error>;

// ===========================================================================
// Auto-generated `From` impls for the 5 external error sources (per Plan).
// Each conversion maps to the variant whose exit-code-correct category the
// external error most naturally falls under; CLI surfaces exit_code() either
// way.
// ===========================================================================

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::rpc(format!("io: {e}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::WalletCorrupt {
            path: "<serde path unknown>".to_string(),
            reason: format!("json: {e}"),
        }
    }
}

impl From<bip39::Error> for Error {
    fn from(e: bip39::Error) -> Self {
        Error::InvalidMnemonic(format!("bip39: {e}"))
    }
}

impl From<alloy_transport::TransportError> for Error {
    fn from(e: alloy_transport::TransportError) -> Self {
        Error::rpc(format!("alloy transport: {e}"))
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::rpc(format!("reqwest: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_variants_map_to_exit_3() {
        assert_eq!(Error::Rpc("x".into()).exit_code(), 3);
        assert_eq!(Error::GasEstimateFailed("x".into()).exit_code(), 3);
    }

    // Issue #365 AC #1 — `Error::rpc(impl Display)` constructor centralizes
    // redaction. Without it, `Error::rpc(format!("...{url}..."))` would
    // embed the RPC URL in the error string. This test fails to compile
    // until the constructor lands.
    #[test]
    fn error_rpc_constructor_redacts_rpc_url_from_display_impl() {
        use std::fmt;
        struct FakeError(&'static str);
        impl fmt::Display for FakeError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.0)
            }
        }
        let secret_url = "https://rpc.example.com/secret-api-key-abcdef12345?token=x";
        let err = Error::rpc(FakeError(secret_url));
        let s = format!("{err}");
        assert!(
            !s.contains("secret-api-key"),
            "Error::rpc must redact RPC URL secrets via Display impl, got: {s}"
        );
        assert!(
            !s.contains("token=x"),
            "Error::rpc must redact URL query strings, got: {s}"
        );
        assert!(
            s.contains("rpc:") && s.contains("<rpc-url-redacted>"),
            "expected `rpc:` format prefix + `<rpc-url-redacted>` marker, got: {s}"
        );
    }

    #[test]
    fn invalid_input_variants_map_to_exit_2() {
        assert_eq!(Error::InvalidMnemonic("x".into()).exit_code(), 2);
        assert_eq!(Error::InvalidPrivateKey("x".into()).exit_code(), 2);
        assert_eq!(Error::InvalidPassword("x".into()).exit_code(), 2);
        assert_eq!(
            Error::NonceMismatch {
                submitted: 1,
                current: 2
            }
            .exit_code(),
            2
        );
        assert_eq!(
            Error::FeeTooLow {
                max_fee_per_gas: 1,
                min_required: 2
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn wallet_balance_variants_map_to_exit_4() {
        let id = uuid::Uuid::new_v4();
        assert_eq!(Error::WalletNotFound { wallet_id: id }.exit_code(), 4);
        assert_eq!(
            Error::WalletExists {
                name: "x".into(),
                network: "sepolia".into()
            }
            .exit_code(),
            4
        );
        assert_eq!(
            Error::InsufficientFunds {
                needed_wei: "1".into(),
                have_wei: "0".into()
            }
            .exit_code(),
            4
        );
        assert_eq!(
            Error::InsufficientTokenBalance {
                token_symbol: "USDC".into(),
                needed_base_units: "1".into(),
                have_base_units: "0".into()
            }
            .exit_code(),
            4
        );
    }

    #[test]
    fn signing_rpc_variants_map_to_exit_5() {
        assert_eq!(
            Error::WalletCorrupt {
                path: "/x".into(),
                reason: "y".into()
            }
            .exit_code(),
            5
        );
        assert_eq!(Error::EncryptionFailed("x".into()).exit_code(), 5);
        assert_eq!(Error::DecryptionFailed("x".into()).exit_code(), 5);
        assert_eq!(Error::TxBroadcastFailed("x".into()).exit_code(), 5);
        assert_eq!(
            Error::ReceiptTimeout {
                secs: 1,
                tx_hash: "0x".into()
            }
            .exit_code(),
            5
        );
    }

    #[test]
    fn external_error_conversions_map_to_expected_variants() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Rpc(_)), "got: {err:?}");
        assert_eq!(err.exit_code(), 3);

        let json_err: serde_json::Error =
            serde_json::from_slice::<serde_json::Value>(b"not-json").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::WalletCorrupt { .. }), "got: {err:?}");
        assert_eq!(err.exit_code(), 5);

        let bip_err = "not a real mnemonic phrase at all".parse::<bip39::Mnemonic>();
        assert!(bip_err.is_err());
        let bip_err = bip_err.unwrap_err();
        let err: Error = bip_err.into();
        assert!(matches!(err, Error::InvalidMnemonic(_)), "got: {err:?}");
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn abidecode_failed_maps_to_exit_2() {
        assert_eq!(
            Error::AbiDecodeFailed {
                context: "balanceOf".into(),
                reason: "decode err".into(),
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn all_19_variants_exist_at_compile_time() {
        // Compile-time witness: every Error variant is constructed.
        // Adding a new variant requires a test update (per L8/L14 audit).
        // v0.2.0 dropped SpkiKeyPinMismatch (Task 5/Issue #304 deferred).
        // Issue #357 added AbiDecodeFailed for contract-revert vs transport-fail
        // distinction (#356 follow-up).
        let _v01 = Error::Rpc("x".into());
        let _v02 = Error::InvalidMnemonic("x".into());
        let _v03 = Error::InvalidPrivateKey("x".into());
        let _v04 = Error::WalletNotFound {
            wallet_id: uuid::Uuid::new_v4(),
        };
        let _v05 = Error::WalletExists {
            name: "x".into(),
            network: "sepolia".into(),
        };
        let _v06 = Error::WalletCorrupt {
            path: "/x".into(),
            reason: "y".into(),
        };
        let _v07 = Error::InvalidPassword("x".into());
        let _v08 = Error::EncryptionFailed("x".into());
        let _v09 = Error::DecryptionFailed("x".into());
        let _v10 = Error::InsufficientFunds {
            needed_wei: "1".into(),
            have_wei: "0".into(),
        };
        let _v11 = Error::InsufficientTokenBalance {
            token_symbol: "USDC".into(),
            needed_base_units: "1".into(),
            have_base_units: "0".into(),
        };
        let _v12 = Error::NonceMismatch {
            submitted: 1,
            current: 2,
        };
        let _v13 = Error::FeeTooLow {
            max_fee_per_gas: 1,
            min_required: 2,
        };
        let _v14 = Error::GasEstimateFailed("x".into());
        let _v15 = Error::TxBroadcastFailed("x".into());
        let _v16 = Error::ReceiptTimeout {
            secs: 120,
            tx_hash: "0xab..".into(),
        };
        let _v17 = Error::InvalidInput("x".into());
        let _v18 = Error::WalletNotFoundByName {
            name: "x".into(),
            network: "sepolia".into(),
        };
        let _v19 = Error::AbiDecodeFailed {
            context: "balanceOf".into(),
            reason: "y".into(),
        };
        let _extra = Error::Rpc("ensure this is the last".into());
        drop((
            _v01, _v02, _v03, _v04, _v05, _v06, _v07, _v08, _v09, _v10, _v11, _v12, _v13, _v14,
            _v15, _v16, _v17, _v18, _v19, _extra,
        ));
    }
}
