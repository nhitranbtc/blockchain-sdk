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

use solana_sdk::signature::{Signature, Signer};
use solana_sdk::transaction::VersionedTransaction;
use zeroize::Zeroizing;

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
    pub(crate) fn from_bytes(bytes: &[u8; 64]) -> Self {
        let keypair = solana_sdk::signature::Keypair::try_from(bytes.as_slice())
            .expect("64-byte secret+pubkey serialization");
        Self(keypair)
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

    /// Sign arbitrary bytes with this wallet's Ed25519 signing key.
    ///
    /// Returns the 64-byte Ed25519 signature. Verify with
    /// `solana_sdk::signature::Signature::verify(pubkey_bytes, msg)`.
    pub fn sign_message(&self, msg: &[u8]) -> Signature {
        Signer::sign_message(&self.0, msg)
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
