# BDK 3.1 — Public API for Wallet Construction, Reading, and Address Generation

- **Crate:** `bdk_wallet`
- **Version verified:** `3.1.0` (published 14 June 2026)
- **Source of truth:** docs.rs pages for `Wallet`, `PersistedWallet`, `CreateParams`, `LoadParams`, `KeychainKind`, `AddressInfo`, `Balance`, plus the crate root and feature-flags page.
- **In-scope categories:** wallet construction, wallet reading, address generation.
- **Out of scope (this document):** transaction building / signing / PSBT, coin selection, descriptors, signers, persistence trait details, blockchain sync, exports, lock/unlock, scripts.

Where a method exists in `Wallet` and is exposed through `PersistedWallet` via `Deref<Target = Wallet>`, the doc line on the `Wallet` page applies unchanged — only the signature and any `&mut self` semantics differ. The `PersistedWallet`-only methods (which compose `Wallet` with a `WalletPersister`) are listed separately under category 1.

---

## 1. Wallet construction

### 1.1 Summary table

| Function | Signature | BDK 3.1 status | Notes |
| --- | --- | --- | --- |
| `Wallet::create` | `fn create<D: IntoWalletDescriptor + Send + 'static>(descriptor: D, change_descriptor: D) -> CreateParams` | **Present** | Builder entry point. Returns `CreateParams`, not a `Wallet`. |
| `Wallet::create_single` | `fn create_single<D: IntoWalletDescriptor + Send + Clone + 'static>(descriptor: D) -> CreateParams` | **Present** | Single-keychain wallet. No `Internal` keychain — some features are unavailable (see doc note). |
| `Wallet::create_with_params` | `fn create_with_params(params: CreateParams) -> Result<Self, DescriptorError>` | **Present** | Terminal step on `CreateParams` that produces a `Wallet` without a persister. |
| `Wallet::create_from_two_path_descriptor` | `fn create_from_two_path_descriptor<D: IntoWalletDescriptor + Send + Clone + 'static>(two_path_descriptor: D) -> CreateParams` | **Present** | BIP-389 multipath descriptor (`<0;1>`). Watch-only (xpubs only). |
| `Wallet::create_with_persist` | — | **Absent** | No such method. Persistence is added by routing through `CreateParams::create_wallet` / `CreateParams::create_wallet_async`, not through a method on `Wallet`. |
| `PersistedWallet::create` | `fn create<P: WalletPersister>(persister: &mut P, params: CreateParams) -> Result<PersistedWallet<P>, CreateWithPersistError<P::Error>>` | **Present** | `PersistedWallet<P>` where `P: WalletPersister`. |
| `PersistedWallet::create_async` | `async fn create_async<P: AsyncWalletPersister>(persister: &mut P, params: CreateParams) -> Result<PersistedWallet<P>, CreateWithPersistError<P::Error>>` | **Present** | Async twin of `create`. |
| `Wallet::load` | `fn load() -> LoadParams` | **Present** | Builder entry point. Returns `LoadParams`. |
| `Wallet::load_with_params` | `fn load_with_params(changeset: ChangeSet, params: LoadParams) -> Result<Option<Self>, LoadError>` | **Present** | Terminal step that takes a `ChangeSet` (no persister). Returns `Ok(None)` when the changeset is empty. |
| `PersistedWallet::load` | `fn load<P: WalletPersister>(persister: &mut P, params: LoadParams) -> Result<Option<PersistedWallet<P>>, LoadWithPersistError<P::Error>>` | **Present** | Returns `Ok(None)` if no persisted data. |
| `PersistedWallet::load_async` | `async fn load_async<P: AsyncWalletPersister>(persister: &mut P, params: LoadParams) -> Result<Option<PersistedWallet<P>>, LoadWithPersistError<P::Error>>` | **Present** | Async twin of `load`. |
| `Wallet::load_with_persist` | — | **Absent** | Same routing as `create_with_persist` — use `LoadParams::load_wallet` / `load_wallet_async`. |
| `CreateParams::new` | `fn new<D: IntoWalletDescriptor + Send + 'static>(descriptor: D, change_descriptor: D) -> Self` | **Present** | Defaults: `Network::Bitcoin`, `DEFAULT_LOOKAHEAD`, `genesis_hash = None`. |
| `CreateParams::new_single` | `fn new_single<D: IntoWalletDescriptor + Send + 'static>(descriptor: D) -> Self` | **Present** | `change_descriptor = None`. |
| `CreateParams::new_two_path` | `fn new_two_path<D: IntoWalletDescriptor + Send + Clone + 'static>(two_path_descriptor: D) -> Self` | **Present** | Mirrors `Wallet::create_from_two_path_descriptor`. |
| `CreateParams::keymap` | `fn keymap(self, keychain: KeychainKind, keymap: KeyMap) -> Self` | **Present** | |
| `CreateParams::network` | `fn network(self, network: Network) -> Self` | **Present** | |
| `CreateParams::genesis_hash` | `fn genesis_hash(self, genesis_hash: BlockHash) -> Self` | **Present** | Custom genesis (e.g. for a regtest/signet variant). |
| `CreateParams::lookahead` | `fn lookahead(self, lookahead: u32) -> Self` | **Present** | |
| `CreateParams::use_spk_cache` | `fn use_spk_cache(self, use_spk_cache: bool) -> Self` | **Present** | Must also be set on `LoadParams` for cross-restart persistence. |
| `CreateParams::create_wallet` | `fn create_wallet<P: WalletPersister>(self, persister: &mut P) -> Result<PersistedWallet<P>, CreateWithPersistError<P::Error>>` | **Present** | Terminal step with a persister. |
| `CreateParams::create_wallet_async` | `async fn create_wallet_async<P: AsyncWalletPersister>(self, persister: &mut P) -> Result<PersistedWallet<P>, CreateWithPersistError<P::Error>>` | **Present** | Async terminal step. |
| `CreateParams::create_wallet_no_persist` | `fn create_wallet_no_persist(self) -> Result<Wallet, DescriptorError>` | **Present** | Same as `Wallet::create_with_params`. |
| `LoadParams::new` | `fn new() -> Self` | **Present** | `lookahead = DEFAULT_LOOKAHEAD`. |
| `LoadParams::keymap` | `fn keymap(self, keychain: KeychainKind, keymap: KeyMap) -> Self` | **Present** | |
| `LoadParams::descriptor` | `fn descriptor<D: IntoWalletDescriptor + Send + 'static>(self, keychain: KeychainKind, expected_descriptor: Option<D>) -> Self` | **Present** | Validates the on-disk descriptor matches. |
| `LoadParams::two_path_descriptor` | `fn two_path_descriptor<D: IntoWalletDescriptor + Send + Clone + 'static>(self, expected_descriptor: D) -> Self` | **Present** | |
| `LoadParams::check_network` | `fn check_network(self, network: Network) -> Self` | **Present** | |
| `LoadParams::check_genesis_hash` | `fn check_genesis_hash(self, genesis_hash: BlockHash) -> Self` | **Present** | |
| `LoadParams::lookahead` | `fn lookahead(self, lookahead: u32) -> Self` | **Present** | |
| `LoadParams::extract_keys` | `fn extract_keys(self) -> Self` | **Present** | Adds signers from secret-key-bearing descriptors. |
| `LoadParams::use_spk_cache` | `fn use_spk_cache(self, use_spk_cache: bool) -> Self` | **Present** | Must be `true` only if a cache was previously persisted with `CreateParams::use_spk_cache(true)`. |
| `LoadParams::load_wallet` | `fn load_wallet<P: WalletPersister>(self, persister: &mut P) -> Result<Option<PersistedWallet<P>>, LoadWithPersistError<P::Error>>` | **Present** | Terminal step. |
| `LoadParams::load_wallet_async` | `async fn load_wallet_async<P: AsyncWalletPersister>(self, persister: &mut P) -> Result<Option<PersistedWallet<P>>, LoadWithPersistError<P::Error>>` | **Present** | |
| `LoadParams::load_wallet_no_persist` | `fn load_wallet_no_persist(self, changeset: ChangeSet) -> Result<Option<Wallet>, LoadError>` | **Present** | |
| `PersistedWallet::persist` | `fn persist<P: WalletPersister>(&mut self, persister: &mut P) -> Result<bool, P::Error>` | **Present** | Returns `true` if new staged changes were persisted; staged changes are only cleared on success. |
| `PersistedWallet::persist_async` | `async fn persist_async<P: AsyncWalletPersister>(&mut self, persister: &mut P) -> Result<bool, P::Error>` | **Present** | |

### 1.2 Per-function source links (docstring + source URL)

The full signatures are above; the docs.rs method pages also carry short docstrings and a "Source" link to the crate's source. Sources cited below all resolve to docs.rs for `bdk_wallet 3.1.0`:

- `Wallet::create` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.create) — *docstring:* "Build a new `Wallet`." Recommends `Wallet::create` over `Wallet::create_single` unless single-descriptor design.
- `Wallet::create_single` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.create_single) — *docstring:* "Build a new single descriptor `Wallet`." Note: no internal keychain; `change_policy` and `do_not_spend_change` are unavailable.
- `Wallet::create_with_params` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.create_with_params) — *docstring:* "Create a new `Wallet` with given `params`. Refer to `Wallet::create` for more."
- `Wallet::create_from_two_path_descriptor` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.create_from_two_path_descriptor) — *docstring:* "Build a new `Wallet` from a two-path descriptor." BIP-389, public-only xpubs.
- `Wallet::load` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.load) — *docstring:* "Build `Wallet` by loading from persistence or `ChangeSet`. Note that the descriptor secret keys are not persisted to the db."
- `Wallet::load_with_params` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.load_with_params) — *docstring:* "Load `Wallet` from the given previously persisted `ChangeSet` and `params`. Returns `Ok(None)` if the changeset is empty."
- `CreateParams` and its builders — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.CreateParams.html). Source pointer: `bdk_wallet/wallet/params.rs#61-211`.
- `LoadParams` and its builders — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.LoadParams.html). Source pointer: `bdk_wallet/wallet/params.rs#215-360`.
- `PersistedWallet::create` / `load` / `persist` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.PersistedWallet.html). Source pointer for the `WalletPersister` impl block lives in the same `wallet/mod.rs` file as the rest of `PersistedWallet`.
- `PersistedWallet::create_async` / `load_async` / `persist_async` — same docs.rs page, separate `impl<P: AsyncWalletPersister>` block.

### 1.3 Source file:line references (verified against docs.rs "Source" link anchors)

- `bdk_wallet::Wallet` impl block: <https://docs.rs/bdk_wallet/latest/src/bdk_wallet/wallet/mod.rs.html> (the `Wallet.html` page links here).
- `bdk_wallet::CreateParams` and `bdk_wallet::LoadParams`: <https://docs.rs/bdk_wallet/latest/src/bdk_wallet/wallet/params.rs.html> — `CreateParams` lines 61–211, `LoadParams` lines 215–360 per docs.rs anchors.
- `bdk_wallet::KeychainKind`: <https://docs.rs/bdk_wallet/latest/src/bdk_wallet/types.rs.html#24-29>.
- `bdk_wallet::AddressInfo`: <https://docs.rs/bdk_wallet/latest/src/bdk_wallet/wallet/mod.rs.html#161-168>.

For exact line numbers in the GitHub checkout `bitcoindevkit/bdk/tree/main/crates/wallet`, open the corresponding relative paths: `crates/wallet/src/wallet/mod.rs`, `crates/wallet/src/wallet/params.rs`, `crates/wallet/src/types.rs`.

---

## 2. Wallet reading

### 2.1 Summary table

| Function | Signature | BDK 3.1 status | Notes |
| --- | --- | --- | --- |
| `Wallet::network` | `fn network(&self) -> Network` | **Present** | Returns the `bitcoin::Network` the wallet is bound to. |
| `Wallet::balance` | `fn balance(&self) -> Balance` | **Present** | Sums over all keychains. |
| `Wallet::transactions` | `fn transactions(&self) -> impl Iterator<Item = WalletTx<'_>> + '_` | **Present** | Relevant and canonical transactions only. |
| `Wallet::full_txs` | `fn full_txs(&self) -> impl Iterator<Item = TxNode<'_>> + '_` | **Present** | All transactions (incl. irrelevant / non-canonical). Delegates to `TxGraph::full_txs`. |
| `Wallet::transactions_sort_by` | `fn transactions_sort_by(&self) -> impl Iterator<Item = WalletTx<'_>> + '_` | **Present** | Same iterator as `transactions` per the docstring on the scraped page (verify in Task 31 spike — name suggests sorting, docs.rs alias). |
| `Wallet::list_unspent` | `fn list_unspent(&self) -> impl Iterator<Item = LocalOutput> + '_` | **Present** | Unspent outputs only (UTXOs). |
| `Wallet::list_output` | `fn list_output(&self) -> impl Iterator<Item = LocalOutput> + '_` | **Present** | All relevant outputs (spent + unspent, confirmed + unconfirmed). |
| `Wallet::tx_details` | `fn tx_details(&self, txid: Txid) -> Option<TxDetails>` | **Present** | `None` if the wallet has no record of `txid`. |
| `Wallet::list_canonical_txs` | `fn list_canonical_txs(&self) -> impl Iterator<Item = CanonicalTx<'_>> + '_` | **Present** | All canonical transactions, including irrelevant ones (delegates to `TxGraph::list_canonical_txs`). |
| `Wallet::list_tx` | — | **Absent** | No `list_tx` method on `Wallet`. Closest: `transactions`, `full_txs`, `list_canonical_txs`. (Verify in Task 31 spike — may exist as an alias not surfaced on docs.rs index.) |
| `Wallet::list_canonical_txids` | — | **Absent** | No method by this exact name on `Wallet`. `tx_details(txid)` queries by txid; iterating canonical txids happens via `list_canonical_txs` + `.tx_node.txid`. |
| `Wallet::public_descriptor` | `fn public_descriptor(&self, keychain: KeychainKind) -> &ExtendedDescriptor` | **Present** | Public-only descriptor (no secrets) for watch-only export. |
| `Wallet::descriptor_checksum` | `fn descriptor_checksum(&self, keychain: KeychainKind) -> String` | **Present** | Calls `public_descriptor` and returns its checksum. |
| `Wallet::master_fingerprint` | — | **Absent** | **No such method on `Wallet` in BDK 3.1.** Fingerprints live on `Descriptor`/`ExtendedDescriptor`; iterate `Wallet::keychains()` to access descriptors. Verify in Task 31 spike. |
| `Wallet::public_key` | — | **Absent** | **No such top-level `Wallet::public_key(keychain)` method.** Public keys are reachable via `Wallet::public_descriptor(keychain).derived_public_keys(...)` or via the descriptor's own key iterator. Verify in Task 31 spike. |
| `Wallet::latest_checkpoint` | `fn latest_checkpoint(&self) -> CheckPoint` | **Present** | Returns the latest checkpoint (chain tip in the wallet's view). |
| `Wallet::checkpoints` | `fn checkpoints(&self) -> CheckPointIter` | **Present** | All checkpoints currently stored, indexed by height. |
| `Wallet::keychains` | `fn keychains(&self) -> impl Iterator<Item = (KeychainKind, &ExtendedDescriptor)>` | **Present** | Iterator over all keychains + their public descriptors. |
| `Wallet::secp_ctx` | `fn secp_ctx(&self) -> &Secp256k1<All>` | **Present** | Shared secp256k1 context used for signing. |
| `Wallet::get_utxo` | `fn get_utxo(&self, op: OutPoint) -> Option<LocalOutput>` | **Present** | `None` if the wallet doesn't know about `op`. |
| `Wallet::insert_txout` | `fn insert_txout(&mut self, outpoint: OutPoint, txout: TxOut)` | **Present** | Inserts a foreign `TxOut` for fee math; not returned by `list_unspent`/`list_output`. WARNINGS: only insert `TxOut`s whose values you trust. |

### 2.2 Supporting types (read-side)

- **`Balance`** — `bdk_wallet::Balance` re-exported from `bdk_chain::Balance`.
  - Fields: `confirmed: Amount`, `trusted_pending: Amount`, `untrusted_pending: Amount`, `immature: Amount`.
  - Methods: `trusted_spendable(&self) -> Amount` (`trusted_pending + confirmed`), `total(&self) -> Amount` (sum of all four).
  - Source: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Balance.html> (re-exports `bdk_chain 0.23.3` `src/bdk_chain/balance.rs#6`).
- **`TxDetails`** — single transaction metadata struct returned by `Wallet::tx_details(txid)`. See [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.TxDetails.html). Definition is re-exported from `bdk_chain` (verify in Task 31 spike for exact field set — docs.rs lists it but the page contents were not in our scrape).
- **`WalletTx`** — `bdk_wallet::WalletTx` is a type alias for `CanonicalTx` managed by a `Wallet`. Returns the full tx alongside anchors + `ChainPosition`.
- **`CanonicalTx`** — re-exported from `bdk_chain::CanonicalTx` (verify field set in Task 31 spike).
- **`LocalOutput`** — `bdk_wallet::LocalOutput` "An unspent output owned by a `Wallet`." Iterators `list_unspent` and `list_output` yield this.
- **`WeightedUtxo`** — `bdk_wallet::WeightedUtxo` is a `Utxo` paired with its `satisfaction_weight` for fee-bumping (used in `add_utxo` / coin selection — *not in our scope but referenced by reading APIs*).
- **`Utxo`** — `bdk_wallet::Utxo` enum "An unspent transaction output (UTXO)." (Re-exported from `bdk_chain`.)

### 2.3 Source pages

- `Wallet` reading-method docs: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html> (read side anchors: `#method.network`, `#method.balance`, `#method.transactions`, `#method.full_txs`, `#method.list_unspent`, `#method.list_output`, `#method.tx_details`, `#method.list_canonical_txs`, `#method.public_descriptor`, `#method.descriptor_checksum`, `#method.latest_checkpoint`, `#method.checkpoints`, `#method.keychains`, `#method.secp_ctx`, `#method.get_utxo`, `#method.insert_txout`).
- `Balance`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Balance.html>.
- `WalletTx`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/type.WalletTx.html>.
- `LocalOutput`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.LocalOutput.html> (definition re-exported from `bdk_chain`; verify in Task 31 spike for exact fields).
- `WeightedUtxo`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.WeightedUtxo.html>.

### 2.4 Per-function source links (docstring + source URL)

- `Wallet::network` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.network) — *docstring:* "Get the `Network` the wallet is using."
- `Wallet::balance` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.balance) — *docstring:* "Return the balance, separated into available, trusted-pending, untrusted-pending, and immature values."
- `Wallet::transactions` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.transactions) — *docstring:* "Iterate over relevant and canonical transactions in the wallet."
- `Wallet::full_txs` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.full_txs) — *docstring:* Delegates to `TxGraph::full_txs`.
- `Wallet::list_unspent` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.list_unspent) — *docstring:* "Return the list of unspent outputs of this wallet."
- `Wallet::list_output` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.list_output) — *docstring:* "List all relevant outputs (includes both spent and unspent, confirmed and unconfirmed)."
- `Wallet::tx_details` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.tx_details) — *docstring:* "Get the `TxDetails` of a wallet transaction."
- `Wallet::list_canonical_txs` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.list_canonical_txs) — *docstring:* Delegates to `TxGraph::list_canonical_txs`.
- `Wallet::public_descriptor` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.public_descriptor) — *docstring:* "Returns the descriptor used to create addresses for a particular `keychain`." Public-only.
- `Wallet::descriptor_checksum` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.descriptor_checksum) — *docstring:* "Return the checksum of the public descriptor associated to the `keychain`."
- `Wallet::latest_checkpoint` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.latest_checkpoint) — *docstring:* "Returns the latest checkpoint."
- `Wallet::checkpoints` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.checkpoints) — *docstring:* "Get all the checkpoints the wallet is currently storing indexed by height."

---

## 3. Address generation

### 3.1 Summary table

| Function | Signature | BDK 3.1 status | Notes |
| --- | --- | --- | --- |
| `Wallet::peek_address` | `fn peek_address(&self, keychain: KeychainKind, index: u32) -> AddressInfo` | **Present** | Does not advance the keychain. **Panics** when `index` exceeds BIP32 max. |
| `Wallet::reveal_next_address` | `fn reveal_next_address(&mut self, keychain: KeychainKind) -> AddressInfo` | **Present** | Increments derivation index; returns the last revealed address if the descriptor has no wildcard or every address up to BIP32 max is already revealed. Requires `persist` to keep state. |
| `Wallet::next_unused_address` | `fn next_unused_address(&mut self, keychain: KeychainKind) -> AddressInfo` | **Present** | Lowest unused index; reveals a new one if all previously revealed are used. Requires `persist`. |
| `Wallet::reveal_addresses_to` | `fn reveal_addresses_to(&mut self, keychain: KeychainKind, index: u32) -> impl Iterator<Item = AddressInfo> + '_` | **Present** | Reveals up to `index`, best-effort, returns newly revealed. Requires `persist`. |
| `Wallet::list_unused_addresses` | `fn list_unused_addresses(&self, keychain: KeychainKind) -> impl DoubleEndedIterator<Item = AddressInfo> + '_` | **Present** | Already-revealed + unused addresses. |
| `Wallet::mark_used` | `fn mark_used(&mut self, keychain: KeychainKind, index: u32) -> bool` | **Present** | Marks the address at `(keychain, index)` as used. Returns whether the index was present and removed from the unused set. |
| `Wallet::unmark_used` | `fn unmark_used(&mut self, keychain: KeychainKind, index: u32) -> bool` | **Present** | Inverse of `mark_used`. No effect if the address is actually spent on chain. |
| `Wallet::is_mine` | `fn is_mine(&self, script: ScriptBuf) -> bool` | **Present** | True iff the script is owned by either keychain. |
| `Wallet::derivation_of_spk` | `fn derivation_of_spk(&self, spk: ScriptBuf) -> Option<(KeychainKind, u32)>` | **Present** | `Some((keychain, index))` only for spks the wallet has given out. |
| `Wallet::next_derivation_index` | `fn next_derivation_index(&self, keychain: KeychainKind) -> u32` | **Present** | Index of the address the next `reveal_next_address` will return. |
| `Wallet::derivation_index` | `fn derivation_index(&self, keychain: KeychainKind) -> Option<u32>` | **Present** | Highest index this wallet has actually derived, or `None` if it hasn't derived any. |
| `Wallet::list_addresses` | — | **Absent** | **No `list_addresses` method on `Wallet` in BDK 3.1.** Use `list_unused_addresses` to iterate revealed addresses; to iterate *all* derived addresses use `Wallet::unbounded_spk_iter` / `Wallet::all_unbounded_spk_iters` (out of category scope but the closest analog — verify in Task 31 spike). |
| `KeychainKind::External` | variant | **Present** | `External = 0`. "External keychain, used for deriving recipient addresses." |
| `KeychainKind::Internal` | variant | **Present** | `Internal = 1`. "Internal keychain, used for deriving change addresses." |
| `KeychainKind::as_byte` | `fn as_byte(&self) -> u8` | **Present** | Returns 0 for External, 1 for Internal. |

### 3.2 Supporting type — `AddressInfo`

`bdk_wallet::AddressInfo` (source: `bdk_wallet/wallet/mod.rs#161-168` per docs.rs) is the struct returned by every address-derivation method. Fields:

```rust
pub struct AddressInfo {
    pub index: u32,           // Child index of this address
    pub address: Address,     // The derived bitcoin::Address
    pub keychain: KeychainKind,
}
```

It implements `Deref<Target = Address>`, so every `Address` method (`to_qr_uri`, `script_pubkey`, `is_valid_for_network`, etc.) is reachable on `AddressInfo` without an explicit dereference.

Source: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.AddressInfo.html>.

### 3.3 Per-function source links (docstring + source URL)

- `Wallet::peek_address` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.peek_address) — *docstring:* "Peek an address of the given `keychain` at `index` without revealing it." For non-wildcard descriptors the same address is returned for every index.
- `Wallet::reveal_next_address` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.reveal_next_address) — *docstring:* "Attempt to reveal the next address of the given `keychain`." Increments index; returns the last revealed if the descriptor has no wildcard or BIP32 max is reached. WARNING: persist before closing.
- `Wallet::next_unused_address` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.next_unused_address) — *docstring:* "Get the next unused address for the given `keychain` ... will attempt to reveal a new address if all previously revealed addresses have been used."
- `Wallet::reveal_addresses_to` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.reveal_addresses_to) — *docstring:* "Reveal addresses up to and including the target `index` and return an iterator of newly revealed addresses." Best-effort.
- `Wallet::list_unused_addresses` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.list_unused_addresses) — *docstring:* "List addresses that are revealed but unused."
- `Wallet::mark_used` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.mark_used) — *docstring:* "Marks an address used of the given `keychain` at `index`. Returns whether the given index was present and then removed from the unused set."
- `Wallet::unmark_used` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.unmark_used) — *docstring:* "Undoes the effect of `mark_used` ... no effect if the address at the given `index` was actually used."
- `Wallet::is_mine` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.is_mine) — *docstring:* "Return whether or not a `script` is part of this wallet (either internal or external)."
- `Wallet::derivation_of_spk` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.derivation_of_spk) — *docstring:* "Finds how the wallet derived the script pubkey `spk`. Will only return `Some(_)` if the wallet has given out the spk."
- `Wallet::next_derivation_index` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.next_derivation_index) — *docstring:* "The index of the next address that you would get if you were to ask the wallet for a new address."
- `Wallet::derivation_index` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html#method.derivation_index) — *docstring:* "The derivation index of this wallet. It will return `None` if it has not derived any addresses."
- `KeychainKind` — [docs.rs](https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/enum.KeychainKind.html) — variants `External = 0`, `Internal = 1`. Source pointer: `bdk_wallet/types.rs#24-29`.

---

## 4. Feature flags and what they enable

Source: <https://docs.rs/crate/bdk_wallet/latest/features> (11 total, 1 enabled by default).

| Flag | Default | Enables | Source |
| --- | --- | --- | --- |
| `std` | yes | Pulls `std` on `bdk_chain`, `bitcoin`, `miniscript`. Required for the typical std-binary use case. | docs.rs features page |
| `default` | n/a | Implies `std`. The "default feature set". | docs.rs features page |
| `keys-bip39` | no | Pulls in `bip39` for mnemonic support. Exposes BIP-39 mnemonics as a key input. | docs.rs features page |
| `all-keys` | no | Convenience umbrella: `keys-bip39`. | docs.rs features page |
| `bip39` | no | Direct dep on `bip39 ^2.2.2`. Lower-level than `keys-bip39` (which re-exports mnemonic types). | docs.rs features page |
| `rusqlite` | no | Re-exports `bdk_chain::rusqlite`, which provides `rusqlite`-based persistence for `PersistedWallet` (also enables the `rusqlite_impl` module). Needed for `Wallet::load`/`Wallet::create` against a SQLite database. | docs.rs features page |
| `file_store` | no | Alias for `bdk_file_store`. Pulls `bdk_file_store ^0.22.0` as the file-based `WalletPersister` implementation. | docs.rs features page |
| `bdk_file_store` | no | Same dependency as `file_store` but exposed as `bdk_file_store` feature name. | docs.rs features page |
| `anyhow` | no | Adds `anyhow ^1` as an optional dependency for convenience in examples. | docs.rs features page |
| `compiler` | no | Forwards `compiler` feature on `miniscript ^12.3.5`. Enables runtime descriptor compilation via miniscript's compiler. | docs.rs features page |
| `tempfile` | no | Adds `tempfile ^3.26.0` for temp-file paths used by some test/example helpers. | docs.rs features page |
| `test-utils` | no | Pulls `anyhow` + `tempfile`. Exposes `test_utils` and `persist_test_utils` modules. | docs.rs features page |

Practical combinations for the categories in this doc:

- Pure in-memory wallet construction/reading/addresses: `default` (just `std`).
- Mnemonic-driven wallet: add `keys-bip39` (or `all-keys`).
- SQLite-backed `PersistedWallet`: add `rusqlite`.
- Flat-file-backed `PersistedWallet`: add `file_store` (alias `bdk_file_store`).

---

## 5. What's NOT in BDK 3.1 (for these three categories)

Items from the task prompt that are **explicitly absent** in `bdk_wallet 3.1.0`. Each one should be re-verified in the Task 31 spike against the live `bitcoindevkit/bdk` `main` branch before any "missing feature" claim is shipped downstream.

- **`Wallet::create_with_persist`** — *absent*. Persistence is composed by routing `CreateParams` into `CreateParams::create_wallet` / `create_wallet_async` (or by `PersistedWallet::create`). There is no `Wallet::create_with_persist` method.
- **`Wallet::load_with_persist`** — *absent*. Same pattern via `LoadParams::load_wallet` / `load_wallet_async` (or `PersistedWallet::load`).
- **`Wallet::master_fingerprint(keychain)`** — *absent*. No top-level `master_fingerprint` method on `Wallet`. Fingerprints live on descriptors; access via `Wallet::keychains()` or `Wallet::public_descriptor(keychain)`.
- **`Wallet::public_key(keychain)`** — *absent*. No top-level `public_key` method on `Wallet`. Public keys are reachable through the descriptor iterator (`public_descriptor(k).derived_public_keys(...)` or via `miniscript::Descriptor` APIs).
- **`Wallet::list_addresses(keychain)`** — *absent*. The closest in-spirit method is `list_unused_addresses` (revealed + unused). For *all* addresses derived or derivable up to lookahead, see `Wallet::unbounded_spk_iter(keychain)` / `Wallet::all_unbounded_spk_iters()` (out of category scope; re-examine during the Task 31 spike).
- **`Wallet::list_canonical_txids`** — *absent* as a named method. Iterate `Wallet::list_canonical_txs()` and read `.tx_node.txid` instead.
- **`Wallet::list_tx`** — *absent* as a named method. The closest equivalents are `Wallet::transactions` (relevant + canonical), `Wallet::list_canonical_txs` (canonical), and `Wallet::full_txs` (everything).

Two minor docs.rs-name gotchas worth noting for the spike:

- The docs.rs HTML for `Wallet::create_single` reuses the same anchor as `Wallet::create` on the page; the actual signature is `create_single(descriptor) -> CreateParams` (not a `Wallet`). See `Wallet.html#method.create_single`.
- `Wallet::transactions` and `Wallet::transactions_sort_by` both render an iterator of `WalletTx<'_>` in the docs we read; the distinction needs confirmation against the source (the latter is likely a sort hook that's currently aliased — re-check `wallet/mod.rs`).

---

## 6. Source-of-truth links (one-stop)

- Crate root: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/>
- `Wallet`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Wallet.html>
- `PersistedWallet`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.PersistedWallet.html>
- `CreateParams`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.CreateParams.html>
- `LoadParams`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.LoadParams.html>
- `KeychainKind`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/enum.KeychainKind.html>
- `AddressInfo`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.AddressInfo.html>
- `Balance`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.Balance.html>
- `LocalOutput`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.LocalOutput.html>
- `WeightedUtxo`: <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/struct.WeightedUtxo.html>
- `WalletTx` (type alias): <https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/type.WalletTx.html>
- Feature flags: <https://docs.rs/crate/bdk_wallet/latest/features>
- GitHub source: <https://github.com/bitcoindevkit/bdk/tree/main/crates/wallet>

---

## 7. Methodology and confidence

- **Verified directly against docs.rs** (HTML pages, scraped 2026-08-05): crate root, `Wallet`, `PersistedWallet`, `CreateParams`, `LoadParams`, `KeychainKind`, `AddressInfo`, `Balance`, feature-flags page. 100% of the `bdk_wallet` crate is reported as documented.
- **Signature confidence:** high for methods listed in section 1.1, 2.1, 3.1 — every entry has a docs.rs page that renders the function. Names normalized from rustdoc display format (e.g. `pub fn create_single <D>(descriptor: D) -> CreateParams`).
- **Docstring quotes:** taken verbatim or near-verbatim from the scraped docs.rs HTML; minor whitespace/punctuation edits only.
- **Items marked "Absent"**: presence of the *absence* is verified by grepping the docs.rs index page for each missing name — none surfaced as documented methods. Still flag with "verify in Task 31 spike" because docs.rs's index can omit a method if it's a re-export from `bdk_chain` aliased under a different name.
- **Items marked "Verify in Task 31 spike"**: `WalletTx`/`TxDetails`/`CanonicalTx`/`LocalOutput` field sets, `transactions_sort_by` semantics, and any method we couldn't fully verify from the docs.rs page contents.