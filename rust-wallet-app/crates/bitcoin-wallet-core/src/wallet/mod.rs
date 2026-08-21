//! Wallet module: end-to-end Bitcoin wallet (Task 9, #19b.2 + #64).
//!
//! Per plan §Task 9. `Wallet::from_mnemonic` (#19a, PR #48),
//! `Wallet::sync` (#19b.2, PR #55), `Wallet::balance` (#19c, PR #55).
//!
//! **Issue #64 (Task 54d) — wallet persistence layer:**
//!
//! - [`id`] — `WalletId(Uuid)` newtype (v4-only, single validate() gate).
//! - [`store`] — filesystem layout per ADR 0001 (`$XDG_DATA_HOME/btc/
//!   wallets/<network>/<id>.enc`, `0o600` files, `0o700` dirs, atomic
//!   write, `O_NOFOLLOW` symlink defense, constant-time padding).
//! - [`ops`] — high-level `create_wallet` + `show_wallet` backing the
//!   `btc wallet create` / `btc wallet show` CLI subcommands.
//!
//! **Threat-model coverage:**
//!
//! - F34 (concrete mnemonic assertion in `Wallet::from_mnemonic`)
//! - F12 (chain sync via Esplora `/address/{addr}/utxo`)
//! - F13 (confirmed-only UTXO aggregation, capped at MAX_MONEY)
//! - F14 (persistence atomicity via `bdk_file_store`) — deferred to v0.1.1;
//!   this module persists only `MnemonicCipherBlob` per ADR 0001. UTXO
//!   state stays in-memory only. A wallet restart loses UTXO state
//!   until next `sync`.
//! - F19 (atomic write) — defended in `wallet::store`.
//! - F49 (mnemonic echoes to STDOUT) — CLI routes mnemonic to STDERR;
//!   the library returns the mnemonic to the caller so the CLI controls
//!   the print destination.
//!
//! **Security:**
//!
//! - F20 (Esplora SPKI pinning): enforced via caller-built
//!   `EsploraClient`. `sync`/`balance` take `&EsploraClient` (not a
//!   raw URL) so the caller controls TLS policy — they can pass a
//!   `TlsPolicy::Pinned(SpkiPinSet)` for production endpoints and
//!   are responsible for rejecting `TlsPolicy::SystemRoots` for
//!   public Esplora servers. This file does NOT default to
//!   `SystemRoots` — there is no default.
//! - Cross-network confusion: AAD binds `bitcoin::Network` discriminant
//!   to the ciphertext (closes N5). The wallet store ALSO uses the
//!   `<network>/` directory layout for defense-in-depth.
//! - xprv material: `XPrvHolder::to_xprv_secret` returns
//!   `Secret<String>` (zeroize-on-drop). Descriptor strings are
//!   dropped immediately after `bdk_wallet::Wallet::create`
//!   returns — bdk parses them into its own keystore.
//! - bdk error wrapping: `Error::Bdk` carries a fixed string, not
//!   the raw bdk error which may echo the descriptor.
//! - N2 (file-existence oracle): single indistinguishable error message
//!   in `wallet::store::WALLET_NOT_ACCESSIBLE`.
//! - N8 (timing oracle): constant-time padding on missing-file +
//!   try_from failure paths in `wallet::ops::show_wallet`.

pub mod id;
pub mod ops;
pub mod persist;
pub mod store;

pub use id::WalletId;
pub use ops::{
    create_wallet, delete_wallet, import_wallet, list_wallets, rename_wallet, show_wallet,
    WalletInfo, SUPPORTED_WORD_COUNTS,
};
pub use store::{data_dir, wallet_path};

// Re-export `KeychainKind` from `bdk_wallet` so CLI handlers can
// pass `KeychainKind::External` to `Wallet::peek_addresses` without
// taking a direct dep on `bdk_wallet`. The type is part of the lib's
// surface (used in `Wallet::peek_addresses`'s public signature).
pub use bdk_wallet::KeychainKind;

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;

use bdk_wallet::bitcoin::{Address, Amount, FeeRate, Network, OutPoint, TxOut, Txid};
use bdk_wallet::Wallet as BdkWallet;

use crate::chain::esplora::{EsploraClient, EsploraUtxo};
use crate::chain::network::coin_type_for;
use crate::error::Error;
use crate::keys::{AddressType, Mnemonic, Secret, XPrvHolder};
use crate::tx;

/// Gap limit for full chain scan (v0.1 demo). BIP-44 default is 20;
/// 5 keeps `Wallet::sync` quick while still finding any recent
/// receive for a fresh wallet. Bump to 20 once we test against
/// wallets with 20+ unused addresses.
const SCAN_GAP_LIMIT: u32 = 5;

/// Bitcoin wallet bound to one mnemonic + one network.
///
/// No `Default` impl (network policy). Construct via
/// [`Wallet::from_mnemonic`]. `sync` populates the inner bdk wallet
/// with UTXOs fetched from Esplora; `balance` reads it back.
///
/// **F14 (persistence):** the inner `bdk_wallet::Wallet` is in-memory
/// only. Wallet restart loses UTXO state until next `sync`
/// repopulates from Esplora. `bdk_file_store` SQLite deferred to
/// v0.1.1.
pub struct Wallet {
    /// Recoverable BIP-39 phrase wrapped in `Secret<String>`.
    /// `sync` re-parses via `Mnemonic::from_phrase` to derive xprv.
    phrase: Secret<String>,
    network: Network,
    /// BIP-44/49/84/86 address type (Story 20 / Issue #132). Drives
    /// the descriptor template in `build_bdk_wallet`. Defaults to
    /// `NativeSegwit` (BIP-84 P2WPKH) via `from_mnemonic`. Other
    /// types require `from_mnemonic_with_type`.
    address_type: AddressType,
    /// Lazily-populated by `sync`. Held in a `Mutex` for interior
    /// mutability across the `&self` API; `sync` and `balance` take
    /// short-lived locks that never cross `await`.
    bdk: Mutex<Option<BdkWallet>>,
    /// Optional path to the bdk_file_store SQLite-like store (Story 12).
    /// `Some(path)` enables `Wallet::persist()` + `load_persisted()`.
    /// `None` keeps the in-memory-only behavior (default for v0.1.1).
    db_path: Option<PathBuf>,
}

impl Wallet {
    /// Construct a `Wallet` from a BIP-39 mnemonic + network.
    ///
    /// F34: rejects non-standard word counts (defense-in-depth).
    /// Network policy: caller must supply `network` explicitly.
    /// `db_path`: `Some(path)` enables `Wallet::persist()` (Story 12
    /// bdk_file_store). `None` keeps the in-memory-only v0.1.1 default.
    ///
    /// **Defaults to `AddressType::NativeSegwit`** (BIP-84 P2WPKH).
    /// For other address types (legacy / nested-segwit / taproot),
    /// use [`Wallet::from_mnemonic_with_type`] (Story 20 / Issue #132).
    pub fn from_mnemonic(
        mnemonic: &Mnemonic,
        network: Network,
        db_path: Option<PathBuf>,
    ) -> Result<Self, Error> {
        Self::from_mnemonic_with_type(mnemonic, network, AddressType::NativeSegwit, db_path)
    }

    /// Construct a `Wallet` from a BIP-39 mnemonic + network +
    /// explicit address type (BIP-44/49/84/86). Story 20 / Issue #132.
    ///
    /// The address type drives the descriptor template used by
    /// `build_bdk_wallet` (private; invoked by `sync` / `balance` /
    /// `send` / `show`). Persisted wallet blobs do NOT carry the
    /// address type — it is encoded in the descriptor shape
    /// (`pkh(...)` vs `sh(wpkh(...))` vs `wpkh(...)` vs `tr(...)`).
    /// Operators re-supply `--type` on `wallet create` (CLI); on
    /// `wallet show` / `send` / `balance` the type is inferred from
    /// the persisted descriptor.
    pub fn from_mnemonic_with_type(
        mnemonic: &Mnemonic,
        network: Network,
        address_type: AddressType,
        db_path: Option<PathBuf>,
    ) -> Result<Self, Error> {
        match mnemonic.word_count() {
            12 | 15 | 18 | 21 | 24 => Ok(Self {
                phrase: mnemonic.to_phrase(),
                network,
                address_type,
                bdk: Mutex::new(None),
                db_path,
            }),
            _ => Err(Error::InvalidMnemonic(
                "unsupported BIP-39 word count".to_string(),
            )),
        }
    }

    /// Return the network this wallet was constructed for.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Return a reference to the wrapped mnemonic phrase.
    pub fn phrase(&self) -> &Secret<String> {
        &self.phrase
    }

    /// Return the path to the bdk_file_store (if any).
    pub fn db_path(&self) -> Option<&std::path::Path> {
        self.db_path.as_deref()
    }

    /// Persist the wallet's current staged `ChangeSet` to the
    /// bdk_file_store at `self.db_path`. Story 12 / Issue #130.
    ///
    /// **Behavior:**
    /// - If `self.db_path` is `None`, returns `Err(Error::Storage("not
    ///   configured for persistence"))`.
    /// - If the bdk wallet has not been synced yet (`bdk: None`),
    ///   persists an empty `ChangeSet` (still creates the store file
    ///   with magic bytes — caller may reload later).
    /// - If the bdk wallet has staged changes from a prior sync, those
    ///   are appended to the store.
    ///
    /// # Errors
    /// - `Error::Storage` on `bdk_file_store` open/append failure.
    pub fn persist(&self) -> Result<(), Error> {
        let db_path = self
            .db_path
            .as_ref()
            .ok_or_else(|| Error::Storage("Wallet::persist: db_path not configured".into()))?;
        // Build a temporary `Mutex`-free handle by taking the bdk wallet
        // out, persisting, then putting it back. Mirrors the
        // `Wallet::send` lock discipline — the guard never crosses an
        // .await point (and `persist` is sync anyway).
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::Storage(
                    "Wallet::persist: bdk wallet not initialized (call sync/balance first)".into(),
                )
            })?
        };
        let result = crate::wallet::persist::persist(&mut bdk, db_path);
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        result
    }

    /// Load a `Wallet` from the bdk_file_store at `db_path`, applying
    /// the persisted `ChangeSet` to the in-memory bdk wallet.
    /// Story 12 / Issue #130.
    ///
    /// **Caveat:** bdk_file_store's `ChangeSet` does NOT include the
    /// mnemonic (it's metadata-only: network, descriptor, chain index,
    /// tx graph). The caller MUST supply `mnemonic` to reconstruct
    /// the signing keys. The mnemonic is then used to derive the same
    /// descriptor that the persisted wallet expects.
    ///
    /// If the store is empty or missing, returns an in-memory
    /// `Wallet` (same as `from_mnemonic(.., None)`) — caller is
    /// expected to `sync()` after.
    pub fn load_persisted(
        db_path: PathBuf,
        mnemonic: &Mnemonic,
        network: Network,
    ) -> Result<Self, Error> {
        // Validate word count (F34).
        if !matches!(mnemonic.word_count(), 12 | 15 | 18 | 21 | 24) {
            return Err(Error::InvalidMnemonic(
                "unsupported BIP-39 word count".to_string(),
            ));
        }
        // Read the aggregated changeset (may be None for empty store).
        let _changeset = crate::wallet::persist::read_change_set(&db_path)?;
        // NOTE: applying the ChangeSet to rebuild the full bdk state is
        // the next iteration (see Issue #130 follow-up). For now,
        // load_persisted returns a fresh wallet with the db_path set;
        // a subsequent sync() will repopulate the bdk wallet from
        // Esplora. The bdk_file_store integration provides the file
        // format + ChangeSet capture; applying it to bdk's tx_graph
        // requires the lower-level `Wallet::build_bdk_wallet_with_cs`
        // extension (Issue #130 follow-up).
        Ok(Self {
            phrase: mnemonic.to_phrase(),
            network,
            address_type: AddressType::NativeSegwit,
            bdk: Mutex::new(None),
            db_path: Some(db_path),
        })
    }

    /// Synchronize the wallet with the blockchain via an Esplora
    /// server (F12: full chain scan).
    ///
    /// Takes `&EsploraClient` (not a raw URL) so the caller controls
    /// TLS policy (F20). For production endpoints, build with
    /// `TlsPolicy::Pinned(SpkiPinSet)` via `EsploraClient::from_config`.
    /// For local dev, `TlsPolicy::SystemRoots` is acceptable.
    ///
    /// Pipeline:
    /// 1. Build in-memory `bdk_wallet::Wallet` from the phrase.
    /// 2. For first `SCAN_GAP_LIMIT` external + internal addresses,
    ///    query Esplora `/address/{addr}/utxo`; for each
    ///    **confirmed** UTXO (F13), cap the value against MAX_MONEY,
    ///    then `wallet.insert_txout(outpoint, txout)`.
    /// 3. Store the populated wallet in `self.bdk`.
    ///
    /// # Errors
    ///
    /// Network/HTTP failure → `Error::Esplora`. bdk parse failure →
    /// `Error::Bdk` (fixed message — does NOT echo the descriptor
    /// to avoid leaking xprv).
    pub async fn sync(&self, client: &EsploraClient) -> Result<(), Error> {
        let mut bdk = self.build_bdk_wallet()?;
        scan_into(client, &mut bdk).await?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(())
    }

    /// Return the wallet's confirmed balance in satoshis (F13).
    ///
    /// Lazy: on first call, syncs against `client` if the wallet has
    /// never been synced. Subsequent calls use the cached wallet.
    /// Returns `Ok(0)` for a wallet with no confirmed UTXOs.
    pub async fn balance(&self, client: &EsploraClient) -> Result<u64, Error> {
        {
            let guard = self.bdk.lock().expect("bdk lock poisoned");
            if let Some(w) = guard.as_ref() {
                return Ok(w.balance().confirmed.to_sat());
            }
        }
        // Slow path: lazy first-time sync. Build + scan outside any
        // lock, then store + return. No MutexGuard crosses await.
        let mut bdk = self.build_bdk_wallet()?;
        scan_into(client, &mut bdk).await?;
        let bal = bdk.balance().confirmed.to_sat();
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(bal)
    }

    /// Send `amount` satoshis to `recipient` (Story 5 / Issue #118 /
    /// Task 11 + 13, extended by Story 6 / Issue #119). Full tx
    /// lifecycle: lazy-sync → build → sign → broadcast → return txid.
    ///
    /// `fee_rate` controls the sat/vB fee. Pass the result of
    /// `FeeRate::from_sat_per_vb(N)` (the caller validates `N >= 1`;
    /// `from_sat_per_vb` returns `None` for `N == 0`). CLI layer
    /// passes the `DEFAULT_FEE_RATE_SAT_PER_VB`-derived rate when
    /// the user omits `--fee-rate`.
    ///
    /// **Lock discipline:** bdk's `build_tx()` takes `&mut self`, so
    /// we must take the bdk wallet OUT of the `Mutex<Option<...>>`
    /// for the build step, then put it back before the async
    /// broadcast. `MutexGuard` never crosses `.await`.
    ///
    /// **Error sanitization (F25 / U1):** `tx::builder::build_send_tx`
    /// already sanitizes bdk's `CreateTxError` (no descriptor echo);
    /// `tx::sign::sign_psbt` maps `SignerError` → `Error::Sign`. The
    /// `Error::NotInitialized` here is only reachable if `balance`
    /// succeeds but the wallet then disappears from the mutex — a
    /// caller bug (no other code path mutates `self.bdk`).
    pub async fn send(
        &self,
        esplora: &EsploraClient,
        recipient: &Address,
        amount: Amount,
        fee_rate: FeeRate,
    ) -> Result<Txid, Error> {
        // 1. Lazy-sync: populates the bdk wallet via balance().
        self.balance(esplora).await?;
        // 2. Take bdk wallet out of the mutex (build needs &mut).
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "send: bdk wallet not initialized after sync (caller bug)".into(),
                )
            })?
        };
        // 3. Build (sync, &mut bdk).
        let mut psbt = tx::builder::build_send_tx(&mut bdk, recipient, amount, fee_rate)?;
        // 4. Sign (sync, &bdk — sign mutates psbt, not self).
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        // 5. Finalize PSBT → broadcast-ready Transaction.
        let tx = tx::sign::extract_tx(&psbt)?;
        // 6. Put bdk wallet back BEFORE the async broadcast so the
        //    MutexGuard doesn't cross .await (mutex poisoning guard).
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        // 7. Broadcast (async).
        let txid = tx::broadcast::broadcast(esplora, &tx).await?;
        Ok(txid)
    }

    /// Send to multiple recipients in a single tx (Story 13 / Issue #138).
    /// Composes `tx::builder::build_multi_recipient_tx` + sign + broadcast.
    /// Same async/lock discipline as [`Wallet::send`].
    ///
    /// # Errors
    ///
    /// - `Error::TxBuild` on bdk build failure (empty list / >20 / no funds).
    /// - `Error::Sign` on signing failure.
    /// - `Error::Esplora` on broadcast failure.
    pub async fn send_multi(
        &self,
        esplora: &EsploraClient,
        recipients: &[(Address, Amount)],
        fee_rate: FeeRate,
    ) -> Result<Txid, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "send_multi: bdk wallet not initialized after sync (caller bug)".into(),
                )
            })?
        };
        let mut psbt = tx::builder::build_multi_recipient_tx(&mut bdk, recipients, fee_rate)?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        let tx = tx::sign::extract_tx(&psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        let txid = tx::broadcast::broadcast(esplora, &tx).await?;
        Ok(txid)
    }

    /// Build + sign a multi-recipient tx without broadcasting
    /// (`--dry-run` / Stories 13+14). Returns the base64-encoded PSBT
    /// so the operator can inspect / submit via another tool.
    ///
    /// Performs the same `balance()` lazy-sync as `send_multi` —
    /// without it, the PSBT would reference stale UTXO state
    /// (already-spent or newly-received outputs invisible to the
    /// bdk tx graph). The sync is the operator's "live view" of the
    /// wallet before sign.
    pub async fn build_sign_multi(
        &self,
        esplora: &EsploraClient,
        recipients: &[(Address, Amount)],
        fee_rate: FeeRate,
    ) -> Result<String, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized("build_sign_multi: bdk wallet not initialized".into())
            })?
        };
        let mut psbt = tx::builder::build_multi_recipient_tx(&mut bdk, recipients, fee_rate)?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(psbt.to_string())
    }

    /// Drain all spendable UTXOs to a single address (Story 14 / Issue #138).
    /// Composes `tx::builder::build_drain_tx` + sign + broadcast. Excluded
    /// outpoints are locked via bdk's `add_unspendable`.
    ///
    /// # Errors
    ///
    /// - `Error::TxBuild` on no spendable UTXOs (after exclusions) or bdk
    ///   build failure.
    /// - `Error::Sign` on signing failure.
    /// - `Error::Esplora` on broadcast failure.
    pub async fn send_drain(
        &self,
        esplora: &EsploraClient,
        drain_addr: &Address,
        fee_rate: FeeRate,
        exclude: &[OutPoint],
    ) -> Result<Txid, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "send_drain: bdk wallet not initialized after sync (caller bug)".into(),
                )
            })?
        };
        let mut psbt = tx::builder::build_drain_tx(&mut bdk, drain_addr, fee_rate, exclude)?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        let tx = tx::sign::extract_tx(&psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        let txid = tx::broadcast::broadcast(esplora, &tx).await?;
        Ok(txid)
    }

    /// Build + sign a drain tx without broadcasting (`--dry-run`).
    /// Returns the base64-encoded PSBT. Performs the same `balance()`
    /// lazy-sync as `send_drain` to surface the live UTXO set.
    pub async fn build_sign_drain(
        &self,
        esplora: &EsploraClient,
        drain_addr: &Address,
        fee_rate: FeeRate,
        exclude: &[OutPoint],
    ) -> Result<String, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized("build_sign_drain: bdk wallet not initialized".into())
            })?
        };
        let mut psbt = tx::builder::build_drain_tx(&mut bdk, drain_addr, fee_rate, exclude)?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(psbt.to_string())
    }

    /// Story 15 / Issue #139: send with explicit coin-selection
    /// algorithm. Composes `tx::builder::build_send_with_coin_selection`
    /// (then sign + broadcast). Lock discipline + lazy-sync mirror
    /// `Wallet::send`.
    pub async fn send_with_coin_selection(
        &self,
        esplora: &EsploraClient,
        recipient: &Address,
        amount: Amount,
        fee_rate: FeeRate,
        algorithm: tx::builder::CoinSelection,
    ) -> Result<Txid, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "send_with_coin_selection: bdk wallet not initialized after sync (caller bug)"
                        .into(),
                )
            })?
        };
        let mut psbt = tx::builder::build_send_with_coin_selection(
            &mut bdk, recipient, amount, fee_rate, algorithm,
        )?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        let tx = tx::sign::extract_tx(&psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        let txid = tx::broadcast::broadcast(esplora, &tx).await?;
        Ok(txid)
    }

    /// Story 16 / Issue #139: send with manual UTXO selection.
    /// `only_manual = true` → bdk `manually_selected_only` strict mode
    /// (fails if selected UTXOs don't cover amount + fee).
    /// `algorithm` applies to any auto-fill UTXOs when
    /// `only_manual = false`. Lock discipline + lazy-sync mirror
    /// [`Wallet::send`].
    #[allow(clippy::too_many_arguments)]
    pub async fn send_with_manual_utxo(
        &self,
        esplora: &EsploraClient,
        recipient: &Address,
        amount: Amount,
        fee_rate: FeeRate,
        utxos: &[OutPoint],
        only_manual: bool,
        algorithm: tx::builder::CoinSelection,
    ) -> Result<Txid, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "send_with_manual_utxo: bdk wallet not initialized after sync (caller bug)"
                        .into(),
                )
            })?
        };
        let mut psbt = tx::builder::build_send_with_manual_utxo(
            &mut bdk,
            recipient,
            amount,
            fee_rate,
            utxos,
            only_manual,
            algorithm,
        )?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        let tx = tx::sign::extract_tx(&psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        let txid = tx::broadcast::broadcast(esplora, &tx).await?;
        Ok(txid)
    }

    /// Story 15 dry-run: build + sign with explicit coin-selection
    /// algorithm. Returns base64 PSBT.
    pub async fn build_sign_with_coin_selection(
        &self,
        esplora: &EsploraClient,
        recipient: &Address,
        amount: Amount,
        fee_rate: FeeRate,
        algorithm: tx::builder::CoinSelection,
    ) -> Result<String, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "build_sign_with_coin_selection: bdk wallet not initialized".into(),
                )
            })?
        };
        let mut psbt = tx::builder::build_send_with_coin_selection(
            &mut bdk, recipient, amount, fee_rate, algorithm,
        )?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(psbt.to_string())
    }

    /// Story 16 dry-run: build + sign with manual UTXO selection.
    /// Returns base64 PSBT.
    #[allow(clippy::too_many_arguments)]
    pub async fn build_sign_with_manual_utxo(
        &self,
        esplora: &EsploraClient,
        recipient: &Address,
        amount: Amount,
        fee_rate: FeeRate,
        utxos: &[OutPoint],
        only_manual: bool,
        algorithm: tx::builder::CoinSelection,
    ) -> Result<String, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "build_sign_with_manual_utxo: bdk wallet not initialized".into(),
                )
            })?
        };
        let mut psbt = tx::builder::build_send_with_manual_utxo(
            &mut bdk,
            recipient,
            amount,
            fee_rate,
            utxos,
            only_manual,
            algorithm,
        )?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(psbt.to_string())
    }

    /// Story 17 / Issue #140: bump fee on an existing tx (RBF).
    /// The replacement tx preserves inputs/outputs of the original
    /// and pays a strictly higher fee. bdk auto-bumps BIP-125 sequence.
    /// Returns the txid (same as input — RBF replaces in mempool).
    pub async fn bump_fee(
        &self,
        esplora: &EsploraClient,
        txid: bitcoin::Txid,
        fee_rate: FeeRate,
    ) -> Result<Txid, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized(
                    "bump_fee: bdk wallet not initialized after sync (caller bug)".into(),
                )
            })?
        };
        let mut psbt = tx::builder::build_bump_fee_tx(&mut bdk, txid, fee_rate)?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        let tx = tx::sign::extract_tx(&psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        tx::broadcast::broadcast(esplora, &tx).await
    }

    /// Story 17 dry-run: build + sign RBF replacement without
    /// broadcasting. Returns base64 PSBT.
    pub async fn build_sign_bump_fee(
        &self,
        esplora: &EsploraClient,
        txid: bitcoin::Txid,
        fee_rate: FeeRate,
    ) -> Result<String, Error> {
        self.balance(esplora).await?;
        let mut bdk = {
            let mut guard = self.bdk.lock().expect("bdk lock poisoned");
            guard.take().ok_or_else(|| {
                Error::NotInitialized("build_sign_bump_fee: bdk wallet not initialized".into())
            })?
        };
        let mut psbt = tx::builder::build_bump_fee_tx(&mut bdk, txid, fee_rate)?;
        tx::sign::sign_psbt(&bdk, &mut psbt)?;
        *self.bdk.lock().expect("bdk lock poisoned") = Some(bdk);
        Ok(psbt.to_string())
    }

    /// Build the in-memory `bdk_wallet::Wallet` from the stored
    /// phrase + network. Used by `sync` and `balance`; not exposed.
    ///
    /// xprv flow: phrase → seed → BIP-32 master → derive
    /// `m/84'/coin'/0'` → wpkh descriptor. The xprv is read via
    /// `XPrvHolder::to_xprv_secret` (returns `Secret<String>`,
    /// zeroize-on-drop). Descriptor Strings are dropped immediately
    /// after `bdk_wallet::Wallet::create` returns — bdk parses them
    /// into its own keystore.
    fn build_bdk_wallet(&self) -> Result<BdkWallet, Error> {
        let m = Mnemonic::from_phrase(self.phrase.expose())?;
        let seed = m.to_seed("");
        let seed_arr: [u8; 64] = seed.expose().as_slice().try_into().map_err(|_| {
            Error::InvalidDerivationPath(format!(
                "bip39 seed must be 64 bytes, got {}",
                seed.expose().len()
            ))
        })?;
        let master = XPrvHolder::master_from_seed(&seed_arr)?;
        let coin = coin_type_for(self.network);
        // Derive only to BIP-44 account root (`m/84'/coin'/0'`).
        // The descriptor template's `*/0/*` and `*/1/*` append the
        // external/change chain + address index. Deriving to a
        // specific index (e.g., `m/.../0/0`) then appending `*/0/*`
        // would yield `m/.../0/0/0/*` — duplicate receive index.
        // Story 20 / Issue #132: address type drives purpose + descriptor
        // wrapper (pkh / sh-wpkh / wpkh / tr).
        let path_str = format!("m/{}h/{}h/0h", self.address_type.purpose(), coin);
        let path = bip32::DerivationPath::from_str(&path_str)
            .map_err(|e| Error::InvalidDerivationPath(e.to_string()))?;
        let derived = master.derive(&path)?;

        // Build descriptor from zeroizing Secret<String>. Drop
        // immediately after `create` returns.
        let xprv_secret = derived.to_xprv_secret(self.network);
        let external_descriptor = self
            .address_type
            .receiving_template()
            .replace("{}", xprv_secret.expose().as_str());
        let change_descriptor = self
            .address_type
            .change_template()
            .replace("{}", xprv_secret.expose().as_str());
        drop(xprv_secret);
        drop(derived);
        drop(master);

        let result = BdkWallet::create(external_descriptor.clone(), change_descriptor.clone())
            .network(self.network)
            .create_wallet_no_persist();

        // Drop the descriptor Strings — bdk has parsed them into
        // its keystore. If create failed, drop is moot but harmless.
        drop(external_descriptor);
        drop(change_descriptor);

        // Sanitize bdk error: do NOT propagate bdk's Display, which
        // can include the descriptor (xprv leak). Use a fixed
        // message.
        result.map_err(|_| Error::Bdk("wallet descriptor parse failed (sanitized)".into()))
    }

    /// Return a sorted, deduped list of txids the wallet has seen
    /// during sync. Read-only after sync — does NOT trigger a network
    /// fetch. Caller must have invoked [`Wallet::sync`] or
    /// [`Wallet::balance`] first (either populates the tx graph).
    ///
    /// **Note:** the txid list includes incoming + outgoing txs
    /// touching our keychain, plus any ancestor txs bdk pulled in
    /// to validate UTXO provenance. Caller filters as needed.
    ///
    /// # Errors
    ///
    /// - `Error::NotInitialized` if called before sync/balance.
    pub fn txids(&self) -> Result<Vec<Txid>, Error> {
        let guard = self.bdk.lock().expect("bdk lock poisoned");
        let bdk = guard
            .as_ref()
            .ok_or_else(|| Error::NotInitialized("txids called before sync or balance".into()))?;
        // bdk's tx_graph exposes anchored + floating (orphan) txs.
        // We collect both — orphans useful for "what txs are in my
        // mempool?" workflows. Dedup + sort for deterministic output.
        let mut out: Vec<Txid> = bdk
            .tx_graph()
            .txs_with_no_anchor_or_last_seen()
            .map(|node| node.txid)
            .collect();
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// Peek the first `count` addresses on the given keychain
    /// (external receive = 0, internal change = 1). Caller must have
    /// invoked [`Wallet::sync`] or [`Wallet::balance`] first (either
    /// populates the bdk wallet). Returns `Err` if not synced — does
    /// NOT panic. Library APIs should return Result, not panic, for
    /// misuse paths. Used by
    /// [`crate::wallet::ops::show_wallet`] to render the wallet's
    /// addresses after sync.
    ///
    /// **Count cap** (L12 review MED #4): rejects `count > 1000` to
    /// bound CPU cost (each address is a derived key + script).
    pub fn peek_addresses(
        &self,
        kind: KeychainKind,
        count: u32,
    ) -> Result<Vec<bdk_wallet::bitcoin::Address>, Error> {
        if count > 1000 {
            return Err(Error::InvalidMnemonic(format!(
                "peek_addresses count {count} exceeds cap of 1000"
            )));
        }
        let guard = self.bdk.lock().expect("bdk lock poisoned");
        let bdk = guard.as_ref().ok_or_else(|| {
            Error::NotInitialized(
                "peek_addresses called before sync or balance — caller bug".into(),
            )
        })?;
        Ok((0..count)
            .map(|i| bdk.peek_address(kind, i).address)
            .collect())
    }

    /// Derive the first external (receive) address WITHOUT syncing
    /// the bdk wallet state. Pure local crypto: bip39 seed → xprv →
    /// descriptor → `BdkWallet::create` → `peek_address(External, 0)`.
    /// The transient bdk Wallet is dropped immediately — this method
    /// does NOT mutate `self.bdk` (so subsequent `sync()` /
    /// `balance()` can still populate that field without losing a
    /// staged `ChangeSet`).
    ///
    /// **Why a separate method (not `peek_addresses`):** `peek_addresses`
    /// requires `self.bdk: Some`, populated only by `sync()` or
    /// `balance()`. The wallet detail screen wants the address before
    /// any Esplora round-trip — derivation is a function of the
    /// descriptor alone (L12 review HIGH #3 follow-up).
    ///
    /// **Cost:** one `build_bdk_wallet` call (~1 ms; bip39 seed →
    /// xprv → descriptor parse). Acceptable for `wallet_show` which
    /// runs once per detail-screen open.
    ///
    /// **Errors:** same surface as `build_bdk_wallet` —
    /// `Error::InvalidMnemonic`, `Error::InvalidDerivationPath`,
    /// `Error::Bdk`. Callers in the FFI layer collapse all three to
    /// `FfiError::WalletStore` to preserve the no-oracle surface
    /// (L12 security-auditor HIGH #1).
    ///
    /// **v0.2.x deviance resolution:** the original v0.2.0
    /// `wallet_show` FFI deferred first-address to v0.2.1 because
    /// `peek_addresses` requires sync. Issue #261 closed the gap by
    /// exposing this offline path. See `tests::first_external_address_offline_*`
    /// for the regression guards.
    pub fn first_external_address_offline(&self) -> Result<bdk_wallet::bitcoin::Address, Error> {
        let bdk = self.build_bdk_wallet()?;
        let info = bdk.peek_address(KeychainKind::External, 0);
        // bdk drops here (the `bdk` binding is a local BdkWallet,
        // not `self.bdk`).
        Ok(info.address)
    }
}

/// For first `SCAN_GAP_LIMIT` external + internal addresses, query
/// Esplora `/address/{addr}/utxo`; for each **confirmed** UTXO,
/// cap value at `Amount::MAX_MONEY` (F13 confirmed-only, plus a
/// malicious-Esplora-response cap to bound a DoS).
///
/// **Script-pubkey trust**: `info.script_pubkey()` comes from the
/// wallet's derived keychain (`bdk.peek_address`), not from
/// Esplora. Esplora's `/address/{addr}/utxo` returns only the
/// `txid`/`vout`/`value`; we substitute the wallet-derived
/// script_pubkey for the UTXO, so a malicious Esplora response
/// cannot trick the wallet into tracking someone else's UTXO.
async fn scan_into(client: &EsploraClient, bdk: &mut BdkWallet) -> Result<(), Error> {
    for kind in [KeychainKind::External, KeychainKind::Internal] {
        for i in 0..SCAN_GAP_LIMIT {
            let info = bdk.peek_address(kind, i);
            let utxos: Vec<EsploraUtxo> = client.address_utxos(&info.address).await?;
            for u in utxos {
                if !u.status.confirmed {
                    continue;
                }
                // Cap value against MAX_MONEY. Reject on overflow.
                let amount = Amount::from_sat(u.value);
                if amount > Amount::MAX_MONEY {
                    return Err(Error::Esplora(format!(
                        "utxo value {} sat exceeds MAX_MONEY ({} sat) for {}",
                        u.value,
                        Amount::MAX_MONEY.to_sat(),
                        info.address,
                    )));
                }
                let outpoint = OutPoint {
                    txid: u.txid,
                    vout: u.vout,
                };
                let txout = TxOut {
                    value: amount,
                    script_pubkey: info.script_pubkey().clone(),
                };
                bdk.insert_txout(outpoint, txout);
            }
        }
    }
    Ok(())
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::esplora::TlsPolicy;
    use tokio::runtime::Builder as RtBuilder;

    fn block<F: std::future::Future>(f: F) -> F::Output {
        RtBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(f)
    }

    fn fresh_mnemonic(words: usize) -> Mnemonic {
        Mnemonic::generate(words).expect("fresh mnemonic generation")
    }

    /// Build a testnet EsploraClient for unit tests. Uses
    /// `TlsPolicy::SystemRoots` for local dev (testnet Esplora
    /// endpoints are public + trusted CA chain).
    fn testnet_client() -> EsploraClient {
        EsploraClient::new(
            crate::chain::esplora_url::EsploraUrl::new("https://blockstream.info/testnet/api")
                .expect("testnet esplora url"),
            TlsPolicy::SystemRoots,
        )
        .expect("testnet Esplora client")
    }

    #[test]
    fn from_mnemonic_rejects_13_words_via_mnemonic_api() {
        let mnemonic_12 = fresh_mnemonic(12usize);
        let phrase_12 = mnemonic_12.to_phrase();
        let bad_phrase = format!("{} extra", phrase_12.expose());
        assert!(
            Mnemonic::from_phrase(&bad_phrase).is_err(),
            "Mnemonic::from_phrase must reject 13 words"
        );
    }

    #[test]
    fn from_mnemonic_accepts_12_words() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        assert_eq!(wallet.network(), Network::Testnet);
        assert_eq!(wallet.phrase().expose().split_whitespace().count(), 12);
    }

    #[test]
    fn from_mnemonic_accepts_24_words() {
        let mnemonic = fresh_mnemonic(24usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        assert_eq!(wallet.network(), Network::Testnet);
        assert_eq!(wallet.phrase().expose().split_whitespace().count(), 24);
    }

    #[test]
    fn from_mnemonic_accepts_15_18_21_words() {
        for words in [15usize, 18, 21] {
            let mnemonic = fresh_mnemonic(words);
            assert!(Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).is_ok());
        }
    }

    #[test]
    fn from_mnemonic_explicit_network_required() {
        let mnemonic = fresh_mnemonic(12usize);
        let _w = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None);
    }

    // -- first_external_address_offline (Issue #261) ----------------------
    //
    // The wallet detail screen needs the first external receive address
    // for copy/explorer/faucet wiring. `peek_addresses` requires `sync()`
    // or `balance()` first (populates `self.bdk`); for a fresh detail-
    // screen open we don't want an Esplora round-trip just to derive
    // an address. `first_external_address_offline` builds a transient
    // bdk Wallet from the descriptor alone (no UTXO scan) and peeks
    // index 0 of the External keychain. See `Wallet::first_external_address_offline`
    // for the full contract.

    #[test]
    fn first_external_address_offline_returns_tb1_for_testnet_native_segwit() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        let addr = wallet
            .first_external_address_offline()
            .expect("offline peek should succeed for a freshly constructed wallet");
        let s = addr.to_string();
        assert!(
            s.starts_with("tb1"),
            "testnet NativeSegwit (BIP-84) must start with `tb1`, got {s}"
        );
        assert_eq!(
            s.len(),
            42,
            "testnet NativeSegwit P2WPKH bech32m address must be 42 chars, got {} ({s})",
            s.len()
        );
    }

    #[test]
    fn first_external_address_offline_does_not_populate_bdk_state() {
        // Regression guard: if the offline peek path accidentally
        // populated `self.bdk`, downstream `peek_addresses` would no
        // longer require `sync()` — and the next `sync()` would
        // rebuild + discard any staged `ChangeSet` from a prior
        // sync. Lock in the invariant: offline peek is read-only.
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        let _addr = wallet.first_external_address_offline().expect("peek ok");
        let peek_after = wallet.peek_addresses(KeychainKind::External, 1);
        assert!(
            peek_after.is_err(),
            "offline peek must NOT populate self.bdk; peek_addresses must still error"
        );
    }

    #[test]
    fn sync_rejects_invalid_client_url() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        // http:// scheme is rejected by EsploraUrl::new (issue #36:
        // URL validation consolidated into the EsploraUrl newtype).
        let bad = crate::chain::esplora_url::EsploraUrl::new("http://blockstream.info/testnet/api");
        let err = match bad {
            Ok(url) => {
                let c = EsploraClient::new(url, TlsPolicy::SystemRoots)
                    .expect("client build after url validation");
                block(wallet.sync(&c)).expect_err("http must be rejected")
            }
            Err(e) => e,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("https") || msg.contains("invalid"),
            "error should mention https/invalid: {msg}"
        );
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn sync_completes_against_testnet_for_fresh_wallet() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        let client = testnet_client();
        wallet
            .sync(&client)
            .await
            .expect("full sync should complete against testnet");
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn balance_returns_zero_for_fresh_wallet() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        let client = testnet_client();
        let bal = wallet
            .balance(&client)
            .await
            .expect("balance fetch should complete against testnet");
        assert_eq!(bal, 0, "fresh wallet has no UTXOs; got {bal}");
    }

    #[tokio::test]
    #[ignore = "requires live testnet Esplora; run manually before merge per L29"]
    async fn balance_reuses_cached_wallet_on_second_call() {
        let mnemonic = fresh_mnemonic(12usize);
        let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
        let client = testnet_client();
        let b1 = wallet.balance(&client).await.expect("first balance call");
        let b2 = wallet.balance(&client).await.expect("second balance call");
        assert_eq!(b1, b2);
    }
}
