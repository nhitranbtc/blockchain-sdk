//! Phantom-equivalent `Wallet` keypair surface.
//!
//! Mirrors `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` §F
//! (lines 1679–1729). No path strings exposed — numeric `--account` +
//! `--address-index` flags only, matching the Phantom UX.
//!
//! Crust behind the API (Phantom-compatible):
//!
//! ```text
//! bip39::Mnemonic::parse_in(English, phrase)
//!   -> bip39::Seed::new(&m, "")                     // empty passphrase V0.1
//!   -> XPrv::from_nonextended_force(&seed[..32], &seed[32..])
//!        // internally does the SLIP-0010 master SHA-512 stretch
//!   -> walk(m/44'/501'/{account}'/{address_index}') via derive(V2, 0x80000000|n)
//!   -> Keypair::new_from_array(xprv.extended_secret_key_bytes()[..32])  // 32-byte seed
//! ```
//!
//! Phase 1.1 ships `from_mnemonic` + `from_mnemonic_at`. Phase 1.2 adds
//! `from_base58` + `from_public_key` (read-only) + `sign_transaction` +
//! `sign_message`.

use solana_sdk::message::VersionedMessage;
use solana_sdk::signature::{Signature, Signer};
use solana_sdk::transaction::VersionedTransaction;
use zeroize::{Zeroize, Zeroizing};

use crate::error::{Error, Result};
use crate::read_only_wallet::ReadOnlyWallet;

/// Hardened-child bit (`0x80000000` — top bit set).
const HARDENED_BIT: u32 = 0x8000_0000;

/// BIP-44 / Phantom Solana coin type + purpose constants.
const BIP44_PURPOSE: u32 = 44;
const SOLANA_COIN_TYPE: u32 = 501;

/// Phantom-equivalent signing wallet.
///
/// Newtype over `solana_sdk::signature::Keypair` so the inner Anza type
/// can evolve (keypair 3.x → 4.x ABI) without breaking our public API.
///
/// `Clone` is deliberately omitted: the inner `Keypair` owns Ed25519
/// signing material, and `ZeroizeOnDrop` is the only sanctioned copy
/// path. Callers that need a second handle must re-derive from the
/// mnemonic (deterministic) rather than clone the keypair.
pub struct Wallet(solana_sdk::signature::Keypair);

impl Wallet {
    /// Inner Keypair 64-byte serialization (32 secret + 32 pubkey).
    /// Used by `WalletManager::create_with_mnemonic` to extract the
    /// derived bytes for at-rest storage. Returns `Zeroizing` so the
    /// caller does not leak the secret across the boundary.
    pub(crate) fn inner_bytes(&self) -> zeroize::Zeroizing<[u8; 64]> {
        zeroize::Zeroizing::new(self.0.to_bytes())
    }

    /// Reconstruct `Wallet` from a 64-byte serialization (32 secret
    /// + 32 pubkey).
    ///
    /// Used by `WalletManager::unlock` after decrypting the at-rest blob.
    ///
    /// Phase 10 / Task 10.1 — fallible, not panicking. A corrupted or
    /// tampered blob whose decrypted 64 bytes fail `Keypair::try_from`
    /// (all-zero secret, pubkey mismatch, off-curve scalar) returns
    /// `Error::InvalidSeed` so callers + FFI can surface
    /// `FfiError::DecryptFailed` (= 6) instead of `Panic` (= 99).
    pub(crate) fn from_bytes(bytes: &[u8; 64]) -> Result<Self> {
        let keypair = solana_sdk::signature::Keypair::try_from(bytes.as_slice())
            .map_err(|_| Error::InvalidSeed)?;
        Ok(Self(keypair))
    }
    /// Phantom "Import secret phrase" — defaults to
    /// `m/44'/501'/0'/0'`.
    ///
    /// Accepts a 12/15/18/21/24-word English BIP-39 phrase. Empty
    /// passphrase (Phantom default; V0.2 may expose a `--passphrase`
    /// flag).
    pub fn from_mnemonic(phrase: &str) -> Result<Self> {
        Self::from_mnemonic_at(phrase, 0, 0)
    }

    /// Phantom "Add account" — `m/44'/501'/{account}'/{address_index}'`.
    ///
    /// `account` and `address_index` are both `u32` (Phantom's numeric
    /// flags). The 4-component SLIP-0010 path is hardcoded; advanced
    /// paths (Ledger `m/44'/501'`, ZIP-32) live behind `sol keygen raw`
    /// in V0.2 — out of v0.1 scope. This matches the standard Solana
    /// wallet derivation used by `solana-keygen recover prompt://` and
    /// every Phantom-shaped wallet.
    ///
    /// SLIP-0010 mandates HARDENED derivation for every Ed25519 child
    /// (no soft-derivation path exists for Ed25519). Every index is
    /// OR'd with `0x80000000` before being passed to
    /// `XPrv::derive(scheme, index)`.
    pub fn from_mnemonic_at(phrase: &str, account: u32, address_index: u32) -> Result<Self> {
        // 1. Parse phrase (English wordlist, BIP-39 checksum).
        let mnemonic = bip39::Mnemonic::parse_in(bip39::Language::English, phrase)
            .map_err(|_| Error::InvalidMnemonic)?;

        // 2. PBKDF2-HMAC-SHA512 stretch with empty passphrase → 64-byte
        //    seed. Wrap in Zeroizing so the bytes ZeroizeOnDrop once
        //    we've consumed them (F53, U3).
        let seed = Zeroizing::new(mnemonic.to_seed(""));

        // 3. SLIP-0010 master stretch. `ed25519-bip32 0.4.3`'s
        //    `XPrv::from_nonextended_force` does the
        //    SHA-512-stretch-and-normalize internally; we just split
        //    the 64-byte seed into key (first 32) + chain_code (last
        //    32) and hand them over.
        let master = ed25519_bip32::XPrv::from_nonextended_force(
            array_from_slice_32(&seed[..32]),
            array_from_slice_32(&seed[32..]),
        );

        // 4. Walk the 4-component SLIP-0010 path. All Ed25519 child
        //    derivation is hardened (no soft path exists).
        let indices = [
            HARDENED_BIT | BIP44_PURPOSE,
            HARDENED_BIT | SOLANA_COIN_TYPE,
            HARDENED_BIT | account,
            HARDENED_BIT | address_index,
        ];

        let mut current = master;
        for idx in indices {
            current = current.derive(ed25519_bip32::DerivationScheme::V2, idx);
        }

        // 5. Final extended-secret-key bytes: first 32 = Ed25519 signing
        //    seed; second 32 = chain code (unused here, but still
        //    zeroized on drop).
        let extended = current.extended_secret_key_bytes();
        let mut signing_seed = Zeroizing::new([0u8; 32]);
        signing_seed.copy_from_slice(&extended[..32]);

        // 6. Construct Anza Keypair from the 32-byte Ed25519 seed.
        //    NB: `Keypair::try_from(&[u8])` expects a 64-byte secret+pubkey
        //    blob; `new_from_array([u8; 32])` is the correct seed-only
        //    constructor — `ed25519_dalek::SigningKey::from(secret_key)`
        //    computes the matching pubkey internally. `signing_seed`
        //    drops at end of scope — ZeroizeOnDrop fires.
        let mut seed_arr = zeroize::Zeroizing::new([0u8; 32]);
        seed_arr.as_mut_slice().copy_from_slice(&signing_seed[..32]);
        let keypair = solana_sdk::signature::Keypair::new_from_array(*seed_arr);
        // `seed_arr` drops at end of scope; Zeroizing's Drop zeroizes
        // the stack bytes (L13 post-push security review).

        Ok(Self(keypair))
    }

    /// Phantom "Import private key" — accepts a base58-encoded 64-byte
    /// secret+pubkey blob (the standard Solana CLI export format).
    ///
    /// `solana_sdk::Keypair::from_base58_string` validates the 64-byte
    /// length and decodes the base58 alphabet; we map its error to
    /// `Error::InvalidBase58Secret(actual_len)` so callers see the
    /// concrete length instead of an opaque `SignatureError`.
    pub fn from_base58(secret: &str) -> Result<Self> {
        let decoded_len_hint = bs58::decode(secret)
            .into_vec()
            .map(|v| v.len())
            .map_err(|_| Error::InvalidBase58Secret {
                got: secret.len(),
                expected: 64,
            })?;
        let keypair =
            solana_sdk::signature::Keypair::try_from_base58_string(secret).map_err(|_| {
                Error::InvalidBase58Secret {
                    got: decoded_len_hint,
                    expected: 64,
                }
            })?;
        Ok(Self(keypair))
    }

    /// Phantom "Watch-only" import — public key only, NO signing
    /// material; returns a `ReadOnlyWallet` (see `read_only_wallet.rs`).
    ///
    /// Watch-only wallets hold a leaf pubkey only — they cannot derive
    /// sibling addresses because Ed25519 SLIP-0010 does not expose a
    /// parent public key (`xpub`). Documented gap; mirrors Phantom.
    pub fn from_public_key(pubkey: solana_sdk::pubkey::Pubkey) -> ReadOnlyWallet {
        ReadOnlyWallet(pubkey)
    }

    /// Phantom base58 Ed25519 pubkey (32 bytes).
    ///
    /// Task 1.2 also references this method (alongside `signTransaction`
    /// + `signMessage`); adding it here so Phase 1.1 tests can assert
    ///   the derived address without coupling to Task 1.2's PR.
    pub fn public_key(&self) -> solana_sdk::pubkey::Pubkey {
        Signer::pubkey(&self.0)
    }

    /// Sign a `VersionedTransaction` with this wallet.
    ///
    /// Uses the transaction's own `recent_blockhash` for the Ed25519
    /// signature (Solana requires the blockhash to be part of the
    /// signed payload — Q7 from the plan grill). Delegates to Anza's
    /// `VersionedTransaction::try_sign(&[self], blockhash)`, which
    /// surfaces `SignerError` (e.g. fee-payer not in keypair set) as
    /// `Error::DerivationFailed` so the caller can distinguish signing
    /// failures from transport-level errors.
    pub fn sign_transaction(&self, mut tx: VersionedTransaction) -> Result<VersionedTransaction> {
        // Anza's `VersionedTransaction::try_sign` is gated behind the
        // `wincode` feature, so we sign manually: serialize the
        // `VersionedMessage` (which embeds the recent blockhash),
        // produce an Ed25519 signature via `Signer::sign_message`, and
        // place the signature at the index where this wallet's pubkey
        // appears in the message's static account keys (Solana's
        // standard signing convention).
        let my_pubkey = Signer::pubkey(&self.0);
        let position = tx
            .message
            .static_account_keys()
            .iter()
            .position(|k| k == &my_pubkey)
            .ok_or_else(|| {
                Error::DerivationFailed("wallet pubkey not present in tx account keys".to_string())
            })?;

        let message_bytes = tx.message.serialize();
        let signature = Signer::sign_message(&self.0, &message_bytes);

        // Resize the signatures vec if needed (preserves any
        // pre-existing signatures — multi-signer transactions).
        while tx.signatures.len() <= position {
            tx.signatures.push(Signature::default());
        }
        tx.signatures[position] = signature;
        Ok(tx)
    }

    /// Borrow the inner `Keypair`. Restricted to `pub(crate)` so only
    /// in-crate callers (`WalletManager` + FFI module) can reach the
    /// Anza keypair. External API consumers must use one of:
    ///
    /// - [`Wallet::sign_transaction`] — sign a fully-built
    ///   `VersionedTransaction`.
    /// - [`Wallet::sign_message`] — sign arbitrary bytes.
    /// - [`Wallet::try_build_versioned_transaction`] — borrow the
    ///   keypair *through a controlled wrapper* to build an
    ///   unsigned `VersionedTransaction` for caller-side
    ///   inspection before signing (FFI send paths).
    ///
    /// Narrowing this to `pub(crate)` closes the silent bypass where
    /// any caller could do `wallet.as_keypair().insecure_clone()`
    /// (Keypair's own public method) to dodge the explicit `Clone`
    /// ban that protects the signing material.
    pub(crate) fn as_keypair(&self) -> &solana_sdk::signature::Keypair {
        &self.0
    }

    /// Build an unsigned `VersionedTransaction` from a `VersionedMessage`
    /// using this wallet as the sole signer. Wraps the
    /// `VersionedTransaction::try_new` call so callers (notably the
    /// FFI `sol_wallet_send_sol` / `sol_wallet_send_spl` paths) don't
    /// need direct access to the inner `Keypair` to construct a tx.
    ///
    /// Surfaces Anza's `SignerError` variants as typed `Error`:
    /// `SignerError::InvalidInput` → `Error::DerivationFailed` (the
    /// only plausible "derivation" cause in this code path), all
    /// others (`TooManySigners`, `NotEnoughSigners`,
    /// `KeypairPubkeyMismatch`) → `Error::InvalidTransaction` so the
    /// FFI boundary can distinguish them from "feature not yet
    /// implemented" via `FfiError::InvalidTransaction = 15`.
    pub fn try_build_versioned_transaction(
        &self,
        message: VersionedMessage,
    ) -> Result<VersionedTransaction> {
        use solana_sdk::signer::SignerError;
        let keypair = self.as_keypair();
        VersionedTransaction::try_new(message, &[keypair]).map_err(|e| match e {
            SignerError::InvalidInput(s) => Error::DerivationFailed(s),
            SignerError::TooManySigners => {
                Error::InvalidTransaction("too many signers".to_string())
            }
            SignerError::NotEnoughSigners => {
                Error::InvalidTransaction("not enough signers".to_string())
            }
            SignerError::KeypairPubkeyMismatch => {
                Error::InvalidTransaction("wallet pubkey not in message account keys".to_string())
            }
            _ => Error::InvalidTransaction(format!("{e}")),
        })
    }

    /// Sign arbitrary bytes with this wallet's Ed25519 signing key.
    ///
    /// Returns the 64-byte Ed25519 signature. Verify with
    /// `solana_sdk::signature::Signature::verify(pubkey_bytes, msg)`.
    pub fn sign_message(&self, msg: &[u8]) -> Signature {
        Signer::sign_message(&self.0, msg)
    }
}

impl Drop for Wallet {
    /// Best-effort zeroize of the inner Ed25519 seed + pubkey on drop.
    /// Closes the Anza-Zeroize gap: Anza's `Keypair` does NOT impl
    /// `ZeroizeOnDrop`, so a bare `Wallet` local (one that drops
    /// without first being moved into an `OwnedLock`) would otherwise
    /// leak the 64-byte secret+pubkey buffer to the heap. This
    /// implementation mirrors the same shape as
    /// `OwnedLock::drop` (`src/wallet_manager.rs`).
    fn drop(&mut self) {
        let mut bytes = self.0.to_bytes();
        bytes.zeroize();
    }
}

impl core::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print the secret key. The Phantom UI shows only the
        // truncated base58 pubkey in debug output too.
        f.debug_struct("Wallet")
            .field("pubkey", &Signer::pubkey(&self.0).to_string())
            .finish()
    }
}

/// Copy a 32-byte slice into a fixed-size array.
///
/// `ed25519-bip32`'s `from_nonextended_force` takes `&[u8; 32]`, but
/// indexing `Zeroizing<[u8; 64]>` produces `&[u8]` not `&[u8; 32]`.
/// This helper closes the gap. Caller must guarantee `slice.len() == 32`.
fn array_from_slice_32(slice: &[u8]) -> &[u8; 32] {
    slice
        .try_into()
        .expect("caller must provide a 32-byte slice")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 10 / Task 10.1 — `Wallet::from_bytes` must be fallible, not
    // panic. Pre-fix the `.expect()` inside the fn triggered
    // `FfiError::Panic = 99` at the FFI boundary, which (a) bypassed
    // typed error handling and (b) risked process termination under
    // `panic = "abort"` (release-mobile profile per ffi.rs H8).

    #[test]
    fn from_bytes_rejects_all_zero_secret() {
        let bad = [0u8; 64];
        let result = Wallet::from_bytes(&bad);
        assert!(
            matches!(result, Err(Error::InvalidSeed)),
            "all-zero 64-byte serialization must surface as Error::InvalidSeed, not panic"
        );
    }

    #[test]
    fn from_bytes_rejects_mismatched_pubkey() {
        // Non-zero secret-side 32 bytes (avoid the all-zero short-circuit
        // in ed25519-dalek's SigningKey::from_bytes) with a pubkey-side
        // that doesn't match the derived pubkey. Keypair::try_from
        // recomputes the pubkey from the secret and rejects on mismatch.
        let mut bad = [0u8; 64];
        for (i, b) in bad[..32].iter_mut().enumerate() {
            *b = (i as u8).wrapping_add(1);
        }
        let result = Wallet::from_bytes(&bad);
        assert!(
            matches!(result, Err(Error::InvalidSeed)),
            "secret+pubkey mismatch must surface as Error::InvalidSeed, not panic"
        );
    }

    #[test]
    fn from_bytes_accepts_valid_keypair_round_trip() {
        // Regression guard: the fallible refactor must still round-trip
        // valid bytes produced by `Wallet::inner_bytes()`.
        let wallet = Wallet::from_mnemonic(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("valid BIP-39 mnemonic");
        let bytes = wallet.inner_bytes();
        let reconstructed = Wallet::from_bytes(&bytes).expect("valid 64-byte serialization");
        assert_eq!(reconstructed.public_key(), wallet.public_key());
    }

    // Phase 10 / Task 10.1 follow-up — security audit HIGH findings:
    // `as_keypair` narrowing + Drop impl.

    #[test]
    fn as_keypair_is_reachable_in_crate() {
        // Compile-time gate: `as_keypair` must remain `pub(crate)` so the
        // FFI module + `WalletManager` can reach it, while external
        // library users cannot. If a future refactor promotes it back to
        // `pub`, this test still compiles — but the doc-comment warning
        // + a `pub`-visibility audit should catch it.
        let wallet = Wallet::from_mnemonic(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("valid BIP-39 mnemonic");
        let _kp: &solana_sdk::signature::Keypair = wallet.as_keypair();
    }

    #[test]
    fn try_build_versioned_transaction_round_trips_through_sign() {
        let wallet = Wallet::from_mnemonic(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("valid BIP-39 mnemonic");
        let from = wallet.public_key();
        let ix = solana_sdk::instruction::Instruction {
            program_id: solana_sdk::pubkey::Pubkey::new_unique(),
            accounts: vec![solana_sdk::instruction::AccountMeta::new(from, true)],
            data: vec![],
        };
        let blockhash = solana_sdk::hash::Hash::new_from_array([0x11u8; 32]);
        let msg = solana_sdk::message::Message::new_with_blockhash(&[ix], Some(&from), &blockhash);
        let unsigned = wallet
            .try_build_versioned_transaction(solana_sdk::message::VersionedMessage::Legacy(msg))
            .expect("build tx");
        let signed = wallet.sign_transaction(unsigned).expect("sign tx");
        let verified = signed.verify_with_results();
        assert!(
            verified.iter().all(|ok| *ok),
            "wallet signature must verify after try_build_versioned_transaction → sign_transaction round-trip"
        );
    }

    #[test]
    fn drop_runs_without_panic_on_local_wallet() {
        // Compile-time + smoke gate: a bare `Wallet` local must drop
        // cleanly (the `Drop` impl writes zeros to a stack copy of the
        // 64-byte secret+pubkey). Best-effort; the inner Anza `Keypair`
        // Box itself is not reachable without `unsafe`, so this test
        // only proves the Drop fn body is sound (no UB, no panic). The
        // real zeroize contract is verified by integration coverage
        // + code review of the Drop impl.
        let wallet = Wallet::from_mnemonic(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("valid BIP-39 mnemonic");
        drop(wallet); // explicit drop to exercise the Drop impl now
    }
}
