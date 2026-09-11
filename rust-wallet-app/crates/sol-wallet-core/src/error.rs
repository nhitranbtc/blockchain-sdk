//! `sol-wallet-core` error type.
//!
//! Phase 1 extends the placeholder with the per-domain variants the
//! `Wallet` keypair constructors need (mnemonic parse / seed stretch /
//! SLIP-0010 derivation / base58 secret parse). The remaining variants
//! for tx builder, RPC, persistence, FFI panic-utf8 boundary land in
//! Phase 5/6/7 — see plan §Phase 6 Task 6.3 and §Phase 7 Task 7.4 for
//! the full 21-variant breakdown.

use thiserror::Error;

/// Crate-wide error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// Placeholder. Replaced by per-domain variants in Phase 5+.
    #[error("sol-wallet-core: placeholder (Phase 0)")]
    Placeholder,

    /// BIP-39 phrase failed to parse — wrong word count, unknown word,
    /// non-English wordlist, or checksum failure.
    ///
    /// Wraps `bip39::Error`. The `Phantom` UX surfaces the same error
    /// for all four sub-failures (the wallet app owns the human-readable
    /// message); here we keep them coalesced so callers don't pattern-match
    /// a private crate's enum.
    #[error(
        "sol-wallet-core: invalid mnemonic — wrong word count, unknown word, or checksum failure"
    )]
    InvalidMnemonic,

    /// BIP-39 mnemonic parse succeeded but the SLIP-0010 derivation path
    /// (`m/44'/501'/{account}'/0'/{address_index}'`) was rejected by
    /// `ed25519-bip32`. Surfaces only when the seed bytes are unusable
    /// (e.g. zero-length) — never for valid Phantom-shaped inputs.
    #[error("sol-wallet-core: SLIP-0010 derivation rejected seed/path: {0}")]
    DerivationFailed(String),

    /// `solana_sdk::Keypair::try_from(seed_bytes)` rejected the derived
    /// 32-byte Ed25519 seed. Ed25519 keys of all-zero bytes are
    /// mathematically valid but disallowed by Solana's wire format to
    /// avoid the off-curve ambiguity.
    #[error("sol-wallet-core: derived seed produced no usable Ed25519 keypair")]
    InvalidSeed,

    /// Base58 secret passed to `Wallet::fromBase58` was not 32+32 = 64
    /// bytes after decoding, or the base58 alphabet was malformed.
    #[error("sol-wallet-core: base58 secret must decode to 64 bytes (32-byte secret + 32-byte pubkey); got {0} bytes")]
    InvalidBase58Secret(usize),

    /// User-supplied Solana address failed to parse — malformed base58,
    /// wrong length, or the resulting bytes are NOT on the Ed25519
    /// curve (i.e. a PDA-shaped address used as a wallet recipient).
    ///
    /// The Phantom wallet refuses to send to off-curve addresses
    /// because no private key exists for a PDA — sending to one would
    /// burn the funds. V0.1 mirrors that guard at the parser
    /// boundary so callers see `Error::InvalidAddress` instead of an
    /// `Err(PubkeyError)` from `solana_sdk`.
    #[error("sol-wallet-core: invalid Solana address — {0}")]
    InvalidAddress(String),

    /// User-supplied SOL amount failed to parse — NaN, ±Inf,
    /// negative, or beyond `u64::MAX` lamports.
    ///
    /// Wraps `f64`-to-`u64` overflow at the parser boundary so the
    /// wallet UI surfaces a single error rather than the validator's
    /// opaque `InvalidLamports` rejection. Mirrors Phantom's "Send"
    /// form's input validator.
    #[error("sol-wallet-core: invalid SOL amount — {0}")]
    InvalidAmount(String),

    /// SPL Token program detection rejected an unknown program ID —
    /// neither classic SPL (`TokenkegQ...`) nor Token-2022
    /// (`TokenzQdB...`). Raised by `disambig::TokenProgram::from_program_id`
    /// when `mint.owner` (as fetched by Phase 5 RPC) is not one of the
    /// two known SPL programs.
    #[error("sol-wallet-core: invalid SPL token program — {0}")]
    InvalidTokenProgram(String),

    /// `spl_token::state::Mint::unpack` (or token-2022 variant) failed
    /// to decode raw mint account bytes. Wraps the underlying
    /// `ProgramError` (e.g. `InvalidAccountData`) so callers see one
    /// crate-wide error rather than three crate-private ones.
    ///
    /// Triggered by truncated account data, wrong owner, or a
    /// non-mint account passed into the decimals-fetch path.
    #[error("sol-wallet-core: failed to unpack SPL mint state — {0}")]
    InvalidTokenState(String),

    /// Phase 5 — reqwest connect/DNS/TLS error or HTTP non-success
    /// response bubbled up from `chain::RpcClient::post`. Carries the
    /// verbatim `Display` of the underlying reqwest error or status.
    /// URL allowlist failures also land here (Tier 1 finding #1).
    #[error("sol-wallet-core: RPC transport error — {0}")]
    Transport(String),

    /// Phase 5 — JSON-RPC error envelope (`{"error": {"code": N, "message": "..."}}`)
    /// returned by the cluster. `code` is the raw i32 (Solana uses
    /// -32700..-32099 for protocol errors, -32005 for "Node is unhealthy",
    /// -32003 for "airdrop limit" on devnet). `message` is the verbatim
    /// cluster message. Phase 7 CLI renders as `"RPC error <code>: <message>"`.
    #[error("sol-wallet-core: RPC error {code} — {message}")]
    Rpc { code: i32, message: String },

    /// Phase 5 — `preflight::check_native_balance` (or similar) found
    /// the wallet's SOL balance insufficient to cover `needed` (transfer
    /// amount + tx fee). Surfaced before broadcast so the user gets a
    /// clear error rather than a `sendTransaction` rejection.
    #[error("sol-wallet-core: insufficient funds — needed {needed} lamports, have {have}")]
    InsufficientFunds { needed: u64, have: u64 },

    /// Phase 5 — `sendTransaction` returned an error (e.g. blockhash
    /// not found, account not found, signature verification failed).
    /// `kind` stores the Anza `ClientError` variant name (PascalCase
    /// per grilled decision Q13). CLI matches on these exact strings.
    #[error("sol-wallet-core: broadcast failed — {kind} ({context})")]
    BroadcastFailed { kind: String, context: String },

    /// Phase 5 — `wait_for_confirm` polled for `timeout` without the
    /// signature reaching the requested commitment level. The tx may
    /// still land; CLI surfaces "tx may or may not land — check
    /// <explorer>".
    #[error("sol-wallet-core: confirm timeout — {signature} not seen after {waited_ms}ms")]
    ConfirmTimeout { signature: String, waited_ms: u64 },

    /// Phase 5 — `simulateTransaction` returned `units_consumed` that
    /// exceeds the CU limit set in the message's Compute Budget ix.
    /// Surfaced before broadcast (Tier 4 finding #4 caveat applies:
    /// cluster state may have changed between simulate and send).
    #[error("sol-wallet-core: compute budget exceeded — needed {needed_cu} CU, available {available_cu} CU")]
    ComputeBudgetExceeded { needed_cu: u32, available_cu: u32 },

    /// Phase 5 — `wait_for_confirm` returned a status at `timeout / 2`
    /// but the requested commitment was not yet reached. The tx is
    /// still valid; CLI surfaces "tx pending — check <explorer>".
    /// Distinct from `ConfirmTimeout` (which fires at full `timeout`).
    #[error("sol-wallet-core: confirm pending — {signature} not yet {commitment:?} after {elapsed_ms}ms")]
    ConfirmPending {
        signature: String,
        commitment: solana_commitment_config::CommitmentConfig,
        elapsed_ms: u64,
    },

    /// Phase 5 — placeholder for WS subscribes (V0.1.5) and any
    /// not-yet-implemented method. The 5 WS subscribes in
    /// `chain::account` return this variant.
    #[allow(missing_docs)]
    #[error("sol-wallet-core: not yet implemented — {0}")]
    Unimplemented(&'static str),

    /// Phase 6 — `argon2` 0.5 / `aes-gcm` 0.10 / `getrandom` 0.2
    /// reported the OS RNG is unavailable (sandbox without
    /// `RANDOM_GET`, seccomp-restricted container, broken
    /// `/dev/urandom` fd). Terminal — `encrypt_wallet` cannot
    /// mint a fresh nonce/salt. Surfaced to FFI as exit 5.
    ///
    /// Per security audit
    /// `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`
    /// finding P6-7. No `unwrap()` / `expect()` on RNG paths.
    #[error("sol-wallet-core: OS RNG unavailable — {source}")]
    OsRngFailed {
        #[source]
        source: getrandom::Error,
    },

    /// Phase 6 — `atomic_write` failed: `.tmp` write, `fsync`, or
    /// `rename` returned an `io::Error`. The `.tmp` file is removed
    /// before returning (per audit finding P6-8). Path + verbatim
    /// OS error preserved for diagnostics.
    #[error("sol-wallet-core: file I/O error on {path} — {source}")]
    FileIo {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Phase 6 — `decrypt_wallet` rejected the envelope: wrong
    /// passphrase, JSON parse fail, AES-GCM tag mismatch, AAD
    /// mismatch (audit P6-1 — KDF metadata tampered), or unsupported
    /// version (audit P6-10). Caller cannot distinguish which
    /// (constant-time differential ≤ Argon2id cost + 10 ms — audit P6-9).
    #[error("sol-wallet-core: wallet decrypt failed for {id}")]
    WalletDecryptFailed { id: WalletId },

    /// Phase 6 — `WalletManager::unlock` / `delete` / `rename` /
    /// `summary` / `list` called with an unknown `WalletId`.
    #[error("sol-wallet-core: wallet not found — {0}")]
    WalletNotFound(WalletId),

    /// Phase 6 — `WalletManager::import_from_pk_file` refused the
    /// source file because its Unix mode exposes the key to other
    /// users (`mode & 0o077 != 0`). Surfaces before the read so the
    /// caller can `chmod 600` and retry. Windows ACL check deferred
    /// to V0.1.5 (audit P6-6).
    #[error("sol-wallet-core: insecure source file mode 0o{mode:o} on {path} — must be 0o600 or stricter")]
    InsecureSourceFile { path: std::path::PathBuf, mode: u32 },

    /// Phase 6 — encrypted-blob envelope carries a `version` outside
    /// the supported range. Per audit P6-10 — gates future migration
    /// to AEAD / KDF changes without silent corruption.
    #[error("sol-wallet-core: unsupported envelope version {found} (supported {lo}..={hi})")]
    UnsupportedBlobVersion { found: u32, lo: u32, hi: u32 },
}

/// Phase 6 — wallet identifier (UUID v4 string). Used as the
/// in-memory + on-disk key for `WalletManager` CRUD. `Display` /
/// `FromStr` provided via the inner `uuid::Uuid`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct WalletId(pub uuid::Uuid);

impl WalletId {
    /// New random v4 UUID. `OsRng` failure propagates as
    /// `Error::OsRngFailed` (no `unwrap()` on RNG paths — audit P6-7).
    pub fn new() -> Result<Self> {
        Ok(Self(uuid::Uuid::new_v4()))
    }

    /// Parse from a string slice (canonical hyphenated form).
    /// Returns `Error::WalletNotFound` on parse failure (the only
    /// caller that parses is `WalletManager::get_by_id_str` which
    /// already had a non-found id).
    pub fn parse_str(s: &str) -> core::result::Result<Self, Error> {
        uuid::Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| Error::WalletNotFound(Self(uuid::Uuid::nil())))
    }
}

impl core::fmt::Display for WalletId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::str::FromStr for WalletId {
    type Err = Error;
    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        Self::parse_str(s)
    }
}

/// Crate-wide result alias.
pub type Result<T> = core::result::Result<T, Error>;
