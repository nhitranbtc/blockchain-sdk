//! `sol-wallet-core` error type.
//!
//! Phase 1 extends the placeholder with the per-domain variants the
//! `Wallet` keypair constructors need (mnemonic parse / seed stretch /
//! SLIP-0010 derivation / base58 secret parse). Phases 5/6/7 add RPC +
//! persistence + CLI variants. Phase 6 + L13 step 10 review (Sept 11)
//! replaced `Error::Placeholder` reuse with per-cause variants:
//! `KdfFailed`, `CipherInit`, `CipherDecrypt`, `RecordCorrupt`,
//! `InvalidKdfParams`, `InvalidWalletId`.

use thiserror::Error;

/// Crate-wide error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// BIP-39 phrase failed to parse — wrong word count, unknown word,
    /// non-English wordlist, or checksum failure.
    ///
    /// Wraps `bip39::Error`. The `Phantom` UX surfaces the same error
    /// for all four sub-failures; here we keep them coalesced so callers
    /// don't pattern-match a private crate's enum.
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
    #[error("sol-wallet-core: base58 secret must decode to 64 bytes (32-byte secret + 32-byte pubkey); got {got} bytes")]
    InvalidBase58Secret { got: usize, expected: usize },

    /// User-supplied Solana address failed to parse.
    #[error("sol-wallet-core: invalid Solana address — {0}")]
    InvalidAddress(String),

    /// User-supplied SOL amount failed to parse.
    #[error("sol-wallet-core: invalid SOL amount — {0}")]
    InvalidAmount(String),

    /// SPL Token program detection rejected an unknown program ID.
    #[error("sol-wallet-core: invalid SPL token program — {0}")]
    InvalidTokenProgram(String),

    /// `spl_token::state::Mint::unpack` (or token-2022 variant) failed
    /// to decode raw mint account bytes.
    #[error("sol-wallet-core: failed to unpack SPL mint state — {0}")]
    InvalidTokenState(String),

    /// Phase 5 — reqwest connect/DNS/TLS error or HTTP non-success
    /// response bubbled up from `chain::RpcClient::post`.
    #[error("sol-wallet-core: RPC transport error — {0}")]
    Transport(String),

    /// Phase 5 — JSON-RPC error envelope returned by the cluster.
    #[error("sol-wallet-core: RPC error {code} — {message}")]
    Rpc { code: i32, message: String },

    /// Phase 5 — preflight found balance insufficient.
    #[error("sol-wallet-core: insufficient funds — needed {needed} lamports, have {have}")]
    InsufficientFunds { needed: u64, have: u64 },

    /// Phase 5 — `sendTransaction` returned an error.
    #[error("sol-wallet-core: broadcast failed — {kind} ({context})")]
    BroadcastFailed { kind: String, context: String },

    /// Phase 5 — `wait_for_confirm` polled for `timeout` without the
    /// signature reaching the requested commitment level.
    #[error("sol-wallet-core: confirm timeout — {signature} not seen after {waited_ms}ms")]
    ConfirmTimeout { signature: String, waited_ms: u64 },

    /// Phase 5 — `simulateTransaction` returned `units_consumed` that
    /// exceeds the CU limit set in the message's Compute Budget ix.
    #[error("sol-wallet-core: compute budget exceeded — needed {needed_cu} CU, available {available_cu} CU")]
    ComputeBudgetExceeded { needed_cu: u32, available_cu: u32 },

    /// Phase 5 — `wait_for_confirm` returned a status at `timeout / 2`
    /// but the requested commitment was not yet reached.
    #[error("sol-wallet-core: confirm pending — {signature} not yet {commitment:?} after {elapsed_ms}ms")]
    ConfirmPending {
        signature: String,
        commitment: solana_commitment_config::CommitmentConfig,
        elapsed_ms: u64,
    },

    /// Phase 5 — placeholder for WS subscribes (V0.1.5) and any
    /// not-yet-implemented method.
    #[allow(missing_docs)]
    #[doc(hidden)]
    #[error("sol-wallet-core: not yet implemented — {0}")]
    Unimplemented(&'static str),

    /// Phase 6 — `argon2` 0.5 / `aes-gcm` 0.10 / `getrandom` 0.2
    /// reported the OS RNG is unavailable.
    ///
    /// Per L13 step 10 review (Sept 11) finding: `WalletId::new` MUST
    /// call `getrandom::getrandom` directly (not `Uuid::new_v4`, which
    /// internally `unwrap_or_else(|err| panic!(...))` and bypasses
    /// this error variant). FFI exit code = 5.
    #[error("sol-wallet-core: OS RNG unavailable — {source}")]
    OsRngFailed {
        #[source]
        source: getrandom::Error,
    },

    /// Phase 6 — `atomic_write` failed.
    #[error("sol-wallet-core: file I/O error on {path} — {source}")]
    FileIo {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Phase 6 — `decrypt_wallet` rejected the envelope. Caller cannot
    /// distinguish wrong-passphrase / AAD / version (audit P6-9).
    /// FFI exit code = 5.
    #[error("sol-wallet-core: wallet decrypt failed for {id}")]
    WalletDecryptFailed { id: WalletId },

    /// Phase 6 — `WalletManager` called with an unknown `WalletId`.
    #[error("sol-wallet-core: wallet not found — {0}")]
    WalletNotFound(WalletId),

    /// Phase 6 — `import_from_pk_file` refused the source file (Unix
    /// mode exposes key to other users, or path resolves to a symlink
    /// per L13 review Sept 11).
    #[error("sol-wallet-core: insecure source file mode 0o{mode:o} on {path} — must be 0o600 or stricter, no symlinks")]
    InsecureSourceFile { path: std::path::PathBuf, mode: u32 },

    /// Phase 6 — encrypted-blob envelope carries a `version` outside
    /// the supported range. Gates future AEAD / KDF migration.
    #[error("sol-wallet-core: unsupported envelope version {found} (supported {lo}..={hi})")]
    UnsupportedBlobVersion { found: u32, lo: u32, hi: u32 },

    /// Phase 6 — Argon2id KDF returned an error (parameter shape
    /// invalid, memory exhausted). Per L13 step 10 Sept 11.
    /// `source` is a String (not a typed `#[source]`) because
    /// `argon2 0.5`'s `Error` does not implement `std::error::Error`
    /// via the path thiserror 2's `AsDynError` accepts.
    #[error("sol-wallet-core: KDF failed — {message}")]
    KdfFailed { message: String },

    /// Phase 6 — `Aes256Gcm::new_from_slice` rejected the 32-byte key
    /// length. Should be unreachable for our 32-byte derive output;
    /// surfaces if the KDF contract breaks. Per L13 step 10 Sept 11.
    #[error("sol-wallet-core: cipher init failed — key length mismatch")]
    CipherInit,

    /// Phase 6 — `WalletRecord` JSON serialize/deserialize failed
    /// (corrupt blob, schema mismatch, drift between V0.1.x versions).
    /// Per L13 step 10 Sept 11 — used to be `Error::Placeholder`.
    /// `source` is a String (not a typed `#[source]`) because the
    /// underlying error types vary (serde_json::Error, solana_sdk
    /// ParsePubkeyError, etc.) and we don't want one variant per
    /// crate-private error type.
    #[error("sol-wallet-core: wallet record corrupted ({context}) — {message}")]
    RecordCorrupt {
        context: &'static str,
        message: String,
    },

    /// Phase 6 — `KdfParams::argon2_params()` constructed an invalid
    /// Argon2 parameter set (m_cost < 8 × p_cost, iterations == 0, or
    /// memory_kb exceeds Argon2 spec cap of ~4 GB). Per L13 step 10
    /// Sept 11 — used to be `Error::Placeholder`.
    #[error("sol-wallet-core: invalid KDF parameters — memory_kb: {memory_kb}, iterations: {iterations}, parallelism: {parallelism}")]
    InvalidKdfParams {
        memory_kb: u32,
        iterations: u32,
        parallelism: u32,
    },

    /// Phase 6 — `WalletId::parse_str` could not parse the input as a
    /// hyphenated UUID. Per L13 step 10 Sept 11 — was previously
    /// coalesced into `WalletNotFound` (semantically wrong).
    #[error("sol-wallet-core: invalid wallet-id — {input:?}")]
    InvalidWalletId { input: String },
}

/// Crate-wide result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Phase 6 — wallet identifier (UUID v4). Inner field is private —
/// use `as_uuid()` for read-only access. Per L13 step 10 Sept 11.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct WalletId(pub(crate) uuid::Uuid);

impl WalletId {
    /// New random v4 UUID sourced from `getrandom 0.2` directly.
    /// RNG failure propagates as `Error::OsRngFailed` (no `unwrap()`
    /// or `expect()` — audit P6-7 / L13 review Sept 11). FFI exit 5.
    pub fn new() -> Result<Self> {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes).map_err(|source| Error::OsRngFailed { source })?;
        Ok(Self(uuid::Uuid::from_bytes(bytes)))
    }

    /// Parse from a hyphenated UUID string. Returns
    /// `Error::InvalidWalletId { input }` on parse failure (NOT
    /// `WalletNotFound` — coalescing was a L13 review finding Sept 11).
    pub fn parse_str(s: &str) -> core::result::Result<Self, Error> {
        uuid::Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| Error::InvalidWalletId {
                input: s.to_string(),
            })
    }

    /// Read-only access to the inner UUID. Inner field stays private.
    pub fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_id_new_uses_osrng_not_panic() {
        // Sanity: WalletId::new is the only entry point. RNG failure
        // path is exercised in the test suite via the OsRng grep
        // gate (tests/mnemonic_encrypt.rs::rng_path_no_unwrap_in_crypto_module).
        let id = WalletId::new().expect("osrng");
        let _ = id.as_uuid();
    }
}
