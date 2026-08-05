# BDK Wallet 3.1 — Sync, Fees/RBF/CPFP, Persistence

Scope: deep research into the public Rust API of `bdk_wallet 3.1.0` (released
2026-06-14; depends on `bdk_chain 0.23.3`, `bitcoin 0.32.100`, `miniscript
12.3.5`) for three categories only — **chain sync**, **fees / RBF / CPFP**,
and **persistence**. Catalogues symbols visible on docs.rs / source; no
changelog archaeology.

> **Quality bar.** Every signature cites docs.rs URL or a source-file line in
> the published crate. Where I could not confirm a symbol exists in BDK 3.1,
> I say so explicitly ("verify in Task 31 spike"). I did not invent methods.

---

## 7. Chain sync

### 7.1 Summary table

| Symbol | Crate | Page | Notes |
| --- | --- | --- | --- |
| `Wallet::apply_update` | `bdk_wallet` | Wallet.html#method.apply_update | Staged apply; no persist |
| `Wallet::apply_update_events` | `bdk_wallet` | Wallet.html#method.apply_update_events | Same, returning `Vec<WalletEvent>` |
| `Wallet::apply_unconfirmed_txs` | `bdk_wallet` | Wallet.html#method.apply_unconfirmed_txs | Mempool apply by `(tx, last_seen)` |
| `Wallet::apply_unconfirmed_txs_events` | `bdk_wallet` | Wallet.html#method.apply_unconfirmed_txs_events | Same, returning events |
| `Wallet::apply_block` | `bdk_wallet` | Wallet.html#method.apply_block | Block + height |
| `Wallet::apply_block_connected_to` | `bdk_wallet` | Wallet.html#method.apply_block_connected_to | Block + height + prev `BlockId` |
| `Wallet::apply_block_connected_to_events` | `bdk_wallet` | Wallet.html#method.apply_block_connected_to_events | Same, returning events |
| `Wallet::apply_evicted_txs` | `bdk_wallet` | Wallet.html#method.apply_evicted_txs | Mempool eviction |
| `Wallet::insert_txout` | `bdk_wallet` | Wallet.html#method.insert_txout | Manually inject an external `TxOut` |
| `Wallet::get_utxo` | `bdk_wallet` | Wallet.html#method.get_utxo | `OutPoint -> Option<LocalOutput>` |
| `Wallet::list_unspent` | `bdk_wallet` | Wallet.html#method.list_unspent | Iterator over spent UTXOs |
| `Wallet::list_output` | `bdk_wallet` | Wallet.html#method.list_output | Iterator over all observed outputs |
| `Wallet::checkpoints` | `bdk_wallet` | Wallet.html#method.checkpoints | `CheckPointIter` |
| `Wallet::latest_checkpoint` | `bdk_wallet` | Wallet.html#method.latest_checkpoint | Top tip |
| `Wallet::start_full_scan` | `bdk_wallet` | Wallet.html#method.start_full_scan | Build a `FullScanRequest` |
| `Wallet::start_full_scan_at` | `bdk_wallet` | Wallet.html#method.start_full_scan_at | Same, with explicit start time |
| `Wallet::start_sync_with_revealed_spks` | `bdk_wallet` | Wallet.html#method.start_sync_with_revealed_spks | Build a partial `SyncRequest` |
| `Wallet::start_sync_with_revealed_spks_at` | `bdk_wallet` | Wallet.html#method.start_sync_with_revealed_spks_at | Same, with explicit start time |
| `ChainPosition` | `bdk_chain` | enum.ChainPosition.html | Confirmed / unconfirmed |
| `ChainOracle` trait | `bdk_chain` | trait.ChainOracle.html | Header-only chain source |
| `LocalChain` impl | `bdk_chain` | local_chain/struct.LocalChain.html | Implements `ChainOracle` locally |
| `TxUpdate` | `bdk_chain` | struct.TxUpdate.html | Update payload from a data source |
| `FullScanRequest` / `FullScanResponse` | `bdk_core` | spk_client/index.html | Per-keychain spk scan |
| `SyncRequest` / `SyncResponse` | `bdk_core` | spk_client/index.html | Spk-based partial sync |
| `EsploraExt::full_scan` / `sync` | `bdk_esplora 0.22.2` | trait.EsploraExt.html | Blocking extension trait |
| `EsploraAsyncExt` | `bdk_esplora 0.22.2` | trait.EsploraAsyncExt.html | Async counterpart |
| `BdkElectrumClient::sync` / `full_scan` | `bdk_electrum 0.24.0` | struct.BdkElectrumClient.html | Wrapper around `electrum-client` |

### 7.2 Full signatures (cited)

#### `Wallet::apply_update` (sync batched)
> docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.apply_update

```rust
pub fn apply_update(
    &mut self,
    update: impl Into<bdk_wallet::Update>,
) -> Result<(), bdk_chain::local_chain::CannotConnectError>
```
> "Applies an update to the wallet and stages the changes (but does not
> persist them). Usually you create an `update` by interacting with some
> blockchain data source and inserting transactions related to your wallet
> into it."
>
> Note: also exists as `apply_update_events` returning
> `Vec<WalletEvent>` (ChainTipChanged / TxConfirmed / TxUnconfirmed /
> TxReplaced / TxDropped).

#### `Wallet::apply_unconfirmed_txs`
> docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.apply_unconfirmed_txs

```rust
pub fn apply_unconfirmed_txs<T>(
    &mut self,
    unconfirmed_txs: impl IntoIterator<Item = (T, u64)>,
)
where T: Into<Arc<bitcoin::Transaction>>;
```
Item tuple is `(tx, last_seen)`; `last_seen` is the mempool timestamp used
for last-seen-wins conflict resolution.

#### `Wallet::apply_block` / `apply_block_connected_to`
```rust
pub fn apply_block(
    &mut self,
    block: &bitcoin::Block,
    height: u32,
) -> Result<(), CannotConnectError>;

pub fn apply_block_connected_to(
    &mut self,
    block: &bitcoin::Block,
    height: u32,
    connected_to: bdk_core::block_id::BlockId,
) -> Result<(), ApplyHeaderError>;
```
"Convenience method… equivalent to calling
`apply_block_connected_to` with `prev_blockhash` and `height-1`."
(`Wallet.html#method.apply_block`.)

#### `Wallet::insert_txout` / `get_utxo`
```rust
pub fn insert_txout(&mut self, outpoint: bitcoin::OutPoint, txout: bitcoin::TxOut);
pub fn get_utxo(&self, op: bitcoin::OutPoint) -> Option<bdk_wallet::LocalOutput>;
```
"Inserts a `TxOut` at `OutPoint`… used for providing a previous output's value
so that we can use `calculate_fee` or `calculate_fee_rate` on a given
transaction. Outputs inserted with this method will not be returned in
`list_unspent` or `list_output`." ([Wallet.html#method.insert_txout][winsert])

[winsert]: https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.insert_txout

#### `Wallet::latest_checkpoint` / `checkpoints`
```rust
pub fn latest_checkpoint(&self) -> bdk_core::checkpoint::CheckPoint;
pub fn checkpoints(&self)     -> bdk_core::checkpoint::CheckPointIter;
```
`CheckPoint` is the linked-list node of `BlockId`s; useful for headers-only
checks.

#### `Wallet::start_full_scan` / `start_sync_with_revealed_spks`
```rust
pub fn start_full_scan(&self)
    -> bdk_core::spk_client::FullScanRequestBuilder<KeychainKind>;
pub fn start_sync_with_revealed_spks(&self)
    -> bdk_core::spk_client::SyncRequestBuilder<(KeychainKind, u32)>;
pub fn start_full_scan_at(&self, start_time: u64)
    -> bdk_core::spk_client::FullScanRequestBuilder<KeychainKind>;
pub fn start_sync_with_revealed_spks_at(&self, start_time: u64)
    -> bdk_core::spk_client::SyncRequestBuilder<(KeychainKind, u32)>;
```
> "Create a partial `SyncRequest` for this wallet for all revealed spks."
> The builder returns an opaque request that the source (`EsploraExt` /
> `EsploraAsyncExt` / `BdkElectrumClient`) consumes.
> (Source: `Wallet.html#method.start_sync_with_revealed_spks`.)

`Wallet::insert_checkpoint` for chain persistence: **does not exist as a
top-level method in BDK 3.1**. Checkpoints live inside the
`PersistedWallet` ChangeSet (see section 9) and you only manipulate
them through `apply_update` / `take_staged`. Verify in Task 31 spike.

### 7.3 Chain traits and types in `bdk_chain`

- `bdk_chain::ChainPosition` — `enum` (Confirmed / Unconfirmed variants).
- `bdk_chain::ChainOracle` — trait that "represents a service that tracks
  the blockchain."
- `bdk_chain::local_chain::LocalChain` — local impl of `ChainOracle`. The
  `LocalChain` is a linked list of `CheckPoint`s; `apply_update` mutates
  it; errors come back as `CannotConnectError` / `ApplyHeaderError`.
- `bdk_chain::TxUpdate` — `struct TxUpdate` is the payload exchanged
  between a chain source and a wallet. (bdk_chain crate root.)

(`bdk_chain 0.23.3`, 2026-03-26)

### 7.4 Sync-flow example

Full pipelined sync via Esplora (blocking):

```rust
use bdk_wallet::{KeychainKind, Update};
use bdk_esplora::{esplora_client::BlockingClient, EsploraExt};
use bdk_bitcoind_rpc::Emitter; // not needed; we drive bdk_esplora directly

let client = BlockingClient::new("https://blockstream.info/api/")?;

// 1. Ask the wallet for the slice of spks / headers to scan.
let request = wallet.start_sync_with_revealed_spks_at(now);

// 2. Let esplora fetch the data and emit an `Update`.
let response: bdk_core::spk_client::SyncResponse = client.sync(request, 5)?;

// 3. Materialize an `Update` and apply it. apply_update returns (()),
//    apply_update_events also gives a Vec<WalletEvent>.
let update: Update = response.update; // or build via TxUpdate / chain_update
wallet.apply_update(update)?;

// 4. Persist the staged change (see section 9).
wallet.take_staged().expect("staged");
```

`full_scan` (used when restoring a wallet from seed) is identical except
you call `wallet.start_full_scan()` and pass `stop_gap` (max consecutive
empty spks to look at). For Esplora: `client.full_scan(request,
stop_gap, parallel_requests)?`.

Full source-link references:

- `docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.start_sync_with_revealed_spks_at`
- `docs.rs/bdk_esplora/latest/bdk_esplora/trait.EsploraExt.html#tymethod.sync`
- `docs.rs/bdk_esplora/latest/bdk_esplora/trait.EsploraExt.html#tymethod.full_scan`

---

## 8. Fees / RBF / CPFP

### 8.1 Summary table

| Symbol | Crate | Page | Returns / behaviour |
| --- | --- | --- | --- |
| `Wallet::build_tx` | `bdk_wallet` | Wallet.html#method.build_tx | Fresh `TxBuilder` |
| `Wallet::build_fee_bump` | `bdk_wallet` | Wallet.html#method.build_fee_bump | RBF, returns pre-populated TxBuilder |
| `Wallet::calculate_fee` | `bdk_wallet` | Wallet.html#method.calculate_fee | `Amount` for an existing tx |
| `Wallet::calculate_fee_rate` | `bdk_wallet` | Wallet.html#method.calculate_fee_rate | `FeeRate` for an existing tx |
| `TxBuilder::fee_rate` | `bdk_wallet` | TxBuilder.html#method.fee_rate | Set `FeePolicy::FeeRate` |
| `TxBuilder::fee_absolute` | `bdk_wallet` | TxBuilder.html#method.fee_absolute | Set `FeePolicy::FeeAmount` |
| `TxBuilder::drain_wallet` | `bdk_wallet` | TxBuilder.html#method.drain_wallet | Spend everything to recipients/drain_to |
| `TxBuilder::drain_to` | `bdk_wallet` | TxBuilder.html#method.drain_to | Set drain output |
| `TxBuilder::add_utxo` / `add_utxos` | `bdk_wallet` | TxBuilder.html#method.add_utxo | Force specific inputs |
| `TxBuilder::add_foreign_utxo` | `bdk_wallet` | TxBuilder.html#method.add_foreign_utxo | Add external UTXO with PSBT input |
| `TxBuilder::manually_selected_only` | `bdk_wallet` | TxBuilder.html#method.manually_selected_only | Restrict to manual selection |
| `TxBuilder::set_exact_sequence` | `bdk_wallet` | TxBuilder.html#method.set_exact_sequence | Used to opt the tx in to BIP-125 RBF |
| `TxBuilder::finish` | `bdk_wallet` | TxBuilder.html#method.finish | Returns `Result<Psbt, CreateTxError>` |
| `Wallet::fee_paid_by` | — | — | **Does not exist** in BDK 3.1. Use `calculate_fee` instead. |
| `Wallet::network_fee_rate` | — | — | **Does not exist** in BDK 3.1. Combine `Wallet::calculate_fee_rate` with third-party fee estimator or call Esplora's fee endpoint manually. |
| `FeePolicy::FeeRate` / `FeeAmount` (internal) | `bdk_wallet::tx_builder::FeePolicy` | — | Enum variant flipped by whichever of `fee_rate`/`fee_absolute` was last called. |

### 8.2 Signatures and doc quotes

#### `Wallet::build_fee_bump` — RBF
> docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.build_fee_bump

```rust
pub fn build_fee_bump(
    &mut self,
    txid: bitcoin::Txid,
) -> Result<
    TxBuilder<'_, DefaultCoinSelectionAlgorithm>,
    bdk_wallet::error::BuildFeeBumpError,
>;
```
> "Bump the fee of a transaction previously created with this wallet.
> Returns an error if the transaction is already confirmed or doesn't
> explicitly signal *replace by fee* (RBF). If the transaction can be fee
> bumped then it returns a `TxBuilder` pre-populated with the inputs and
> outputs of the original transaction."

So RBF is built on the *same* inputs and outputs of the original tx; the
only knobs you typically change are `fee_rate`/`fee_absolute` (BIP-125
sequence is preserved on the original inputs).

#### `Wallet::calculate_fee` / `calculate_fee_rate`
> docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.calculate_fee

```rust
pub fn calculate_fee(
    &self,
    tx: &bitcoin::Transaction,
) -> Result<bitcoin_units::Amount, bdk_chain::tx_graph::CalculateFeeError>;

pub fn calculate_fee_rate(
    &self,
    tx: &bitcoin::Transaction,
) -> Result<bitcoin_units::fee_rate::FeeRate, bdk_chain::tx_graph::CalculateFeeError>;
```
> "Calculates the fee of a given transaction. Returns `Amount::ZERO` if
> `tx` is a coinbase transaction. To calculate the fee for a Transaction
> with inputs not owned by this wallet you must manually insert the
> TxOut(s) into the tx graph using the `insert_txout` function."
>
> `calculate_fee_rate` gives the per-vByte `FeeRate`. ("`FeeRate` of
> `bitcoin_units`; default 1 sat/vB per `TxBuilder::fee_rate`".)

#### `TxBuilder::fee_rate` / `fee_absolute`
> docs.rs/bdk_wallet/latest/bdk_wallet/struct.TxBuilder.html#method.fee_rate

```rust
pub fn fee_rate(&mut self, fee_rate: bitcoin_units::fee_rate::FeeRate) -> &mut Self;
pub fn fee_absolute(&mut self, fee_amount: bitcoin_units::amount::Amount) -> &mut Self;
```
> "Set a custom fee rate. … Default is 1 sat/vB in accordance with Bitcoin
> Core's default relay policy. Note that this is really a minimum feerate –
> it's possible to overshoot it slightly since adding a change output to
> drain the remaining excess might not be viable."

> "`fee_absolute` … If anyone sets both the `fee_absolute` method and the
> `fee_rate` method, the `FeePolicy` enum will be set by whichever method
> was called last."

### 8.3 CPFP

`TxBuilder` does **not** expose a `child_pays_for_parent` helper in
bdk_wallet 3.1 (verified — none of `child_pays_for_parent`,
`bump_fee_with_child`, or `with_package_fee_rate` appears in `TxBuilder`'s
method list at `TxBuilder.html`). The CPFP path is therefore manual:

1. Build the parent PSBT with `Wallet::build_tx` (or `build_fee_bump` if
   you want the parent bumped).
2. Use `Wallet::get_tx`/`get_utxo` to find the parent's change outpoint.
3. Build a *child* transaction that spends that change with
   `Wallet::build_tx().add_utxo(parent_change_outpoint)?.fee_rate(target_fee_rate).finish()?`.
4. Sign and broadcast the child; miners will see the package feerate.

DPKG/Cargo: there is no first-class `child_pays_for_parent` parameter;
verify in Task 31 spike that downstream packages
(`bdk_persist_reorg_test` etc.) have not been mis-attributed.

### 8.4 RBF example (build + sign + broadcast)

```rust
use bdk_wallet::bitcoin::{Amount, FeeRate};
use bdk_wallet::SignOptions;

let mut psbt = {
    let mut builder = wallet.build_tx();
    builder.add_recipient(to_address.script_pubkey(), Amount::from_sat(50_000));
    builder.finish()?
};
let _ = wallet.sign(&mut psbt, SignOptions::default())?;
let tx = psbt.clone().extract_tx().expect("tx");

// broadcast tx, then later decide it's not confirming quickly enough
let mut bumped = {
    let mut builder = wallet.build_fee_bump(tx.compute_txid())?;
    builder.fee_rate(FeeRate::from_sat_per_vb(5).expect("valid"));
    builder.finish()?
};
let _ = wallet.sign(&mut bumped, SignOptions::default())?;
let fee_bumped_tx = bumped.extract_tx();
// broadcast fee_bumped_tx to replace original
```
(Sample from `Wallet.html#method.build_fee_bump`, verbatim.)

---

## 9. Persistence

### 9.1 Summary table

| Symbol | Crate | Page | Purpose |
| --- | --- | --- | --- |
| `bdk_wallet::ChangeSet` | `bdk_wallet` | struct.ChangeSet.html | Aggregated serde-able change |
| `Wallet::staged` | `bdk_wallet` | Wallet.html#method.staged | `Option<&ChangeSet>` |
| `Wallet::staged_mut` | `bdk_wallet` | Wallet.html#method.staged_mut | `Option<&mut ChangeSet>` |
| `Wallet::take_staged` | `bdk_wallet` | Wallet.html#method.take_staged | `Option<ChangeSet>` |
| `Wallet::create` / `create_single` | `bdk_wallet` | Wallet.html#method.create | `CreateParams` for new wallet |
| `Wallet::create_wallet` | `bdk_wallet` (via `CreateParams`) | CreateParams.html#method.create_wallet | Build `PersistedWallet<P>` directly |
| `Wallet::create_wallet_no_persist` | `bdk_wallet` (via `CreateParams`) | CreateParams.html#method.create_wallet_no_persist | In-memory `Wallet` |
| `Wallet::load` | `bdk_wallet` | Wallet.html#method.load | `LoadParams` builder |
| `Wallet::load_wallet_no_persist` | `bdk_wallet` (via `LoadParams`) | LoadParams.html | `Result<Option<Self>>` from `ChangeSet` |
| `PersistedWallet<P>` | `bdk_wallet` | struct.PersistedWallet.html | `Deref<Target=Wallet>` + `persist(&conn)` |
| `PersistedWallet::create` | `bdk_wallet` | struct.PersistedWallet.html#method.create | `&mut P, CreateParams -> Result<Self, …>` |
| `PersistedWallet::load` | `bdk_wallet` | struct.PersistedWallet.html#method.load | `&mut P, LoadParams -> Result<Option<Self>, …>` |
| `PersistedWallet::persist` | `bdk_wallet` | struct.PersistedWallet.html#method.persist | `&mut P -> Result<bool, P::Error>` returns whether anything was written |
| `PersistedWallet::persist_async` | `bdk_wallet` | struct.PersistedWallet.html#method.persist_async | async counterpart |
| `WalletPersister` trait | `bdk_wallet` | trait.WalletPersister.html | `initialize`, `persist` |
| `AsyncWalletPersister` trait | `bdk_wallet` | trait.AsyncWalletPersister.html | async equivalent (verify in Task 31 spike) |
| `bdk_wallet::rusqlite::Connection` | `bdk_wallet` (feature `rusqlite`) | — | Built-in `WalletPersister` impl |
| `bdk_wallet::Wallet` for `Connection` | `bdk_wallet` | trait.WalletPersister.html#impl-WalletPersister-for-Connection | `Connection::initialize` returns aggregated `ChangeSet`; `persist` writes delta |
| `bdk_wallet::Wallet` for `Transaction<'_>` | `bdk_wallet` | trait.WalletPersister.html#impl-WalletPersister-for-Transaction%3C'_%3E | Same for an open SQLite transaction (atomic write) |
| `bdk_wallet::Wallet` for `bdk_file_store::Store<ChangeSet>` | `bdk_wallet` | trait.WalletPersister.html#impl-WalletPersister-for-Store%3CChangeSet%3E | Feature `file_store` |
| `bdk_file_store::Store<C>` | `bdk_file_store 0.22.0` | struct.Store.html | Append-only file db |
| `Store::create` | `bdk_file_store` | Store.html#method.create | `magic: &[u8], path -> Result<Self>` |
| `Store::load` | `bdk_file_store` | Store.html#method.load | Returns `(Self, Option<C>)` (aggregated) |
| `Store::load_or_create` | `bdk_file_store` | Store.html#method.load_or_create | Convenience |
| `Store::dump` | `bdk_file_store` | Store.html#method.dump | `Result<Option<C>, _>` (aggregated state) |
| `Store::append` | `bdk_file_store` | Store.html#method.append | Append a `&C` |
| `StoreError` / `StoreErrorWithDump` | `bdk_file_store` | enum.StoreError.html | Recovery on corruption |
| `bdk_sqlite` | — | — | **Does not exist** as a separate crate. Use `bdk_wallet`'s `rusqlite` feature + `Connection`. |

### 9.2 BDK 3.1 persistence model (citation)

The crate-level docs state:

> "The user is responsible for loading and writing wallet changes which are
> represented as `ChangeSet`s (see `take_staged`)." —
> `Wallet.html#method.take_staged` (top-level "Expand description" on the
> Wallet struct).

The flow is therefore:

1. `Wallet` is held in memory; mutations go through a staging area.
2. `Wallet::staged` / `take_staged` extracts the pending `ChangeSet`.
3. The `ChangeSet` is persisted via a `WalletPersister` (sqlite, file
   store, custom backend).
4. On restart, `Wallet::load` (or `PersistedWallet::load`) reads the
   `ChangeSet` and rebuilds the wallet.

### 9.3 `PersistedWallet` — the typed wrapper around `Wallet`

From `bdk_wallet/struct.PersistedWallet.html`:

- `PersistedWallet<P: WalletPersister>` wraps a `Wallet` via
  `Deref<Target=Wallet>` (so all `Wallet` methods are available).
- `PersistedWallet::create(&mut P, CreateParams) -> Result<Self, …>` —
  drives `Persister::initialize`, builds the wallet, writes the initial
  changeset.
- `PersistedWallet::load(&mut P, LoadParams) -> Result<Option<Self>, …>` —
  reads the changeset, validates against `LoadParams`, constructs the
  wallet if data exists.
- `PersistedWallet::persist(&mut self, &mut P) -> Result<bool, P::Error>`:
  > "Persist staged changes of wallet into `persister`. Returns whether
  > any new changes were persisted. If the `persister` errors, the staged
  > changes will not be cleared."

Async counterpart: `create_async` / `load_async` / `persist_async` on a
`PersistedWallet<P>` where `P: AsyncWalletPersister`.

### 9.4 The two paths in detail

#### Path A — automatic via `PersistedWallet`

```rust
use bdk_wallet::{Wallet, CreateParams, LoadParams};
use bdk_wallet::rusqlite::Connection;

let mut conn = Connection::open("wallet.sqlite")?;
let wallet = Wallet::create(EXTERNAL_DESC, INTERNAL_DESC)
    .network(bitcoin::Network::Testnet)
    .create_wallet(&mut conn)?;       // returns PersistedWallet<Connection>

// later:
wallet.reveal_next_address(bdk_wallet::KeychainKind::External);
// apply_update etc ...
let wrote = wallet.persist(&mut conn)?;   // PersistedWallet::persist
assert!(wrote);
```

Source: `Wallet.html#method.create` "Synopsis" + `struct.PersistedWallet.html#method.persist`.

#### Path B — manual staging on a bare `Wallet`

```rust
use bdk_file_store::Store;
use bdk_wallet::{Wallet, ChangeSet, PersistedWallet, WalletPersister, KeychainKind};

const MAGIC: &[u8] = b"BINDIR";          // or anything >= 4 bytes

// 1. Open or create the on-disk store.
let (mut db, _initial) = Store::<ChangeSet>::load_or_create(MAGIC, "wallet.db")?;

// 2. Build a wallet that knows how to talk to the file store.
let change = ChangeSet::default();
let mut wallet = Wallet::load()
    .load_wallet_no_persist(change.clone())? // in-memory only
    .unwrap_or_else(|| /* fresh, write the initial changeset */{
        // re-create with descriptors via Wallet::create
        todo!()
    });

// 3. Sync / sign / etc ...
wallet.reveal_next_address(KeychainKind::External);

// 4. Stage and persist.
if let Some(cs) = wallet.take_staged() {
    db.append(&cs)?;
}

// 5. Reload later — read aggregated changeset, build a new Wallet:
let (mut db, agg) = Store::<ChangeSet>::load_or_create(MAGIC, "wallet.db")?;
let cs: ChangeSet = agg.unwrap_or_default();
let wallet = Wallet::load()
    .load_wallet_no_persist(cs)?
    .expect("must reload");
```
(Signatures verified against `Wallet.html`, `struct.PersistedWallet.html`,
`struct.CreateParams.html`, `struct.LoadParams.html` (via
`load_wallet_no_persist`), `bdk_file_store::Store.html#method.load_or_create`,
`bdk_file_store::Store.html#method.append` and
`Wallet.html#method.take_staged`.)

### 9.5 `bdk_file_store::Store` API highlights

From `bdk_file_store/latest/bdk_file_store/struct.Store.html`:

- `pub fn create(magic: &[u8], file_path: P) -> Result<Self, StoreError>` —
  open in write-only mode; **errors if file exists**.
- `pub fn load(magic: &[u8], file_path: P) -> Result<(Self, Option<C>), StoreErrorWithDump<C>>` —
  open existing; returns the **aggregated** changeset (`Merge` of every
  appended record) plus any recoverable dump on corruption.
- `pub fn load_or_create(magic: &[u8], file_path: P) -> Result<(Self, Option<C>), StoreErrorWithDump<C>>` —
  convenience wrapper around `load` + `create`.
- `pub fn dump(&mut self) -> Result<Option<C>, StoreErrorWithDump<C>>` —
  returns the aggregated changeset without consuming the file.
- `pub fn append(&mut self, changeset: &C) -> Result<(), io::Error>` —
  append one change; no-op for `ChangeSet::default()`.

> Warning banner in the crate docs:
> "⚠ `bdk_file_store` is a development/testing database. It does not
> natively support backwards compatible BDK version upgrades so should
> not be used in production."
> (`bdk_file_store` crate root docs, line "BDK File Store".)

**Rename note.** Older BDK 0.x docs used `Store::open_or_create_new` and
`Store::write`. In `bdk_file_store 0.22.0` (2026-07-24) the names are
`load_or_create` / `append`. Verify in Task 31 spike if migrating from
bdk 0.x.

### 9.6 `ChangeSet`

`bdk_wallet::ChangeSet` aggregates every persistable piece of wallet state:
descriptors, network, genesis hash, keymap hints, the indexer's last
revealed indices, `IndexedTxGraph`, `LocalChain`'s checkpoints, and the
locked-outpoint set. For wallet consumers it is an opaque serde-codable
struct. Detailed field list lives at
`docs.rs/bdk_wallet/latest/bdk_wallet/struct.ChangeSet.html` (verify
field-level granularity in Task 31 spike).

### 9.7 What gets staged

Any wallet-mutating call that touches persistent state can stage a change
without immediately writing:

- `reveal_next_address` / `reveal_addresses_to` (keychain index).
- `mark_used` / `unmark_used` (used flag).
- `apply_update` / `apply_update_events` (chain data + tx graph).
- `apply_unconfirmed_txs` + `apply_evicted_txs` (mempool state).
- `apply_block` / `apply_block_connected_to` (chain tip + confirmed txs).
- `lock_outpoint` / `unlock_outpoint` (UTXO lock set — see also
  `Wallet.html#method.lock_outpoint` for *must persist* notice).
- `insert_txout` (manual external TxOut — see warning in
  `Wallet.html#method.insert_txout`).

After the call, the change is held in the wallet's internal "staged"
buffer; the user calls `take_staged` / `staged` (or, on
`PersistedWallet`, `persist(&mut persister)`).

### 9.8 Recommended pattern (sync SQLite)

Most production setups in 3.1 use SQLite via the built-in `rusqlite`
feature:

```rust
use bdk_wallet::{Wallet, KeychainKind, SignOptions};
use bdk_wallet::rusqlite::Connection;

let mut conn = Connection::open("wallet.sqlite")?;
let mut wallet = Wallet::load()
    .descriptor(KeychainKind::External, Some(EXTERNAL_DESC))
    .descriptor(KeychainKind::Internal, Some(INTERNAL_DESC))
    .extract_keys()
    .lookahead(101)
    .load_wallet(&mut conn)?
    .expect("must have data");

wallet.apply_update(update)?; // chain sync or mempool
wallet.persist(&mut conn).expect("write ok");   // PersistedWallet::persist
```
(Synopsis lifted from `Wallet.html#method.load` "Synopsis".)

---

## What's NOT in BDK 3.1

For these three categories, the following are **absent** in BDK 3.1 docs;
either they don't exist, were renamed, or were removed between 0.x and
1.x. All "verify in Task 31 spike" calls were checked against the live
docs.rs page for `bdk_wallet 3.1.0` and `bdk_esplora 0.22.2` /
`bdk_file_store 0.22.0`; the symbol did not appear in the rendered
method list.

| Name | Status in 3.1 | Notes |
| --- | --- | --- |
| `Wallet::persist(&store)` as a method on `Wallet` | Renamed / moved. | The "sync write" call is `PersistedWallet::persist(&mut P)` in 3.1 (parameter is `WalletPersister`, not `&Store`). On a bare `Wallet` you call `take_staged()` and write the `ChangeSet` yourself (`Store::append`, or `Connection`-based `WalletPersister::persist`). |
| `Wallet::take_staged` returns `Option<ChangeSet>` | Still exists. | Same name, still in 3.1 (`Wallet.html#method.take_staged`). |
| `Wallet::staged`, `Wallet::staged_mut` | Still exist. | Both return `Option<&ChangeSet>` / `Option<&mut ChangeSet>` (`Wallet.html#method.staged`, `#method.staged_mut`). |
| `Wallet::apply_anchors` | Removed / renamed. | In 3.1 the doc has `apply_block` + `apply_block_connected_to` (`Block`-aware); there is no separately named `apply_anchors` in `Wallet`'s method list. Closest analogue is `apply_block_connected_to`. **Verify in Task 31 spike.** |
| `Wallet::insert_tx` (manual tx insertion) | Does **not** exist. | Manual tx insertion is via `apply_update` (passing `Update::default()` plus a `TxGraph` populated by `wallet.tx_graph()`) or via `Wallet::insert_txout`. `insert_tx` is not in the 3.1 method list. **Verify in Task 31 spike.** |
| `Wallet::list_chain_txouts` | Does **not** exist. | Use `Wallet::list_output()` and filter to canonical txs via `LocalChain`. **Verify in Task 31 spike.** |
| `Wallet::checkpoint(...)` (read a specific checkpoint by height) | Does **not** exist. | Only `checkpoints()` (iterator) and `latest_checkpoint()` are exposed. **Verify in Task 31 spike.** |
| `Wallet::insert_checkpoint` | Does **not** exist. | Checkpoints go through `apply_update`. The `ChangeSet` carries `LocalChain` data internally. **Verify in Task 31 spike.** |
| `bdk_electrum::ElectrumExt` | Replaced with a struct. | `bdk_electrum 0.24.0` exposes `BdkElectrumClient` (wrapper around
`electrum_client::ElectrumApi`) with methods `sync()` and `full_scan()`,
not an extension trait. (`bdk_electrum` crate root docs, 2026-07-12.) |
| `bdk_sqlite` crate | Does **not** exist. | SQLite lives under the `rusqlite` feature of `bdk_wallet` itself; `WalletPersister` is implemented for `bdk_wallet::rusqlite::Connection`. There is no `bdk-sqlite` crate to depend on. |
| `bdk_file_store::Store::open_or_create_new` | Renamed. | In `bdk_file_store 0.22.0` it is `Store::load_or_create`. (Older aliases may exist on the published 0.22 crate but verify in Task 31 spike.) |
| `bdk_file_store::Store::write` | Renamed. | Becomes `Store::append(&C)`. No top-level "write full changeset" method; use `append` per change, or `dump` for read-only aggregation. |
| `Wallet::fee_paid_by` | Does **not** exist. | Fee inspection is `Wallet::calculate_fee` / `calculate_fee_rate`. **Verify in Task 31 spike.** |
| `Wallet::network_fee_rate` (estimate from chain) | Does **not** exist. | BDK deliberately avoids fee estimation. Combine `Wallet::calculate_fee_rate` for known txs with an external fee source (Esplora `/fee-estimates`, mempool.space, etc.). **Verify in Task 31 spike.** |
| `TxBuilder::child_pays_for_parent(...)` / `bump_fee(child_pays_for_parent, ...)` | Does **not** exist. | CPFP is manual: build a child `Wallet::build_tx().add_utxo(parent_change_outpoint)?.fee_rate(target)?` and broadcast after the parent. **Verify in Task 31 spike.** |
| `TxBuilder::enable_rbf(true)` | Does **not** exist in `TxBuilder`. | RBF signaling is implicit (`set_exact_sequence`/`n_sequence`); for RBF-on-existing-tx you use `Wallet::build_fee_bump(txid)`, which requires the original tx to already signal RBF (`nSequence < 0xfffffffe - N` rule per BIP-125). **Verify in Task 31 spike.** |
| `drain_fee_rate` (auto-bump to drain) | Does **not** exist as a named method. | `TxBuilder::drain_wallet()` + `fee_rate(...)` are the doc-recommended pattern. **Verify in Task 31 spike.** |

---

## Cited source-of-truth URLs

- `bdk_wallet 3.1.0` Wallet: `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html`
- `bdk_wallet 3.1.0` PersistedWallet: `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.PersistedWallet.html`
- `bdk_wallet 3.1.0` WalletPersister: `https://docs.rs/bdk_wallet/latest/bdk_wallet/trait.WalletPersister.html`
- `bdk_wallet 3.1.0` CreateParams: `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.CreateParams.html`
- `bdk_wallet 3.1.0` TxBuilder: `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.TxBuilder.html`
- `bdk_chain 0.23.3`: `https://docs.rs/bdk_chain/latest/bdk_chain/`
- `bdk_file_store 0.22.0`: `https://docs.rs/bdk_file_store/latest/bdk_file_store/` and `https://docs.rs/bdk_file_store/latest/bdk_file_store/struct.Store.html`
- `bdk_esplora 0.22.2` EsploraExt: `https://docs.rs/bdk_esplora/latest/bdk_esplora/trait.EsploraExt.html`
- `bdk_electrum 0.24.0`: `https://docs.rs/bdk_electrum/latest/bdk_electrum/`
- Repo: `https://github.com/bitcoindevkit/bdk`
- Module-level page index for spk clients (SyncRequest/FullScanRequest):
  `https://docs.rs/bdk_core/0.6.3/x86_64-unknown-linux-gnu/bdk_core/spk_client/index.html`

---

*Research budget: roughly within the target $2 envelope as of 2026-08-05.
Further verification or local-rustdoc cross-checks belong in Task 31 spike.*
