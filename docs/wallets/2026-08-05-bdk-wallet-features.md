# BDK 3.1 Features — Complete API Surface

**Date:** 2026-08-05
**Source:** Live docs.rs (`bdk_wallet 3.1.0`, 14 June 2026) + GitHub source paths.
**Companion docs (split for parallel research):**
- [Part 1: construction + reading + address generation](2026-08-05-bdk-features-01-construct-read-address.md)
- [Part 2: tx building + signing + PSBT](2026-08-05-bdk-features-02-tx-sign-psbt.md)
- [Part 3: chain sync + fees/RBF/CPFP + persistence](2026-08-05-bdk-features-03-sync-fees-persist.md)
- [Part 4: keys module + errors + utilities + network + integration](2026-08-05-bdk-features-04-keys-errors-utils-network.md)

## TL;DR

BDK 3.1 has ~80 public APIs organized into 14 categories. **It is the canonical Bitcoin wallet shell for Rust.** We use it as the wallet layer of `bitcoin-wallet-core`. Every API the plan needs is present, except for: a single `Error` enum (BDK has 5 sibling enums); an `MnemonicType` (replaced by `WordCount`); `child_pays_for_parent` (no native CPFP — must be done via the manual RBF flow); and a `Wallet::from_mnemonic()` shortcut (does not exist — must build descriptor yourself).

## Use cases BDK 3.1 handles natively (no custom code needed)

These are user-facing capabilities that BDK provides out of the box. The `bitcoin-wallet-core` library wraps each one with a thin Rust API; the `btc` CLI exposes it as a subcommand. No hand-rolled crypto, no custom signing paths, no manual UTXO selection algorithms.

| Use case | BDK 3.1 API | Code we write |
|---|---|---|
| **Single-sig wallets** (P2PKH / P2SH-P2WPKH / P2WPKH / P2TR via BIP-44/49/84/86) | `Wallet::create(descriptor, change_descriptor)` | descriptor string from mnemonic |
| **Multi-wallet** (N wallets in one process) | instantiate N `Wallet`s; BDK has no built-in manager | `HashMap<wallet_id, Arc<Wallet>>` |
| **Chain sync** (Esplora + Electrum) | `bdk_esplora::EsploraExt::full_scan` + `Wallet::apply_update` | one-line adapter |
| **UTXO selection** (BnB, Knapsack, LowestFee, LargestFirstCoinFirst) | `bdk_wallet::coin_selection` module | algorithm choice on `TxBuilder` |
| **Manual UTXO selection** (coin control) | `TxBuilder::add_utxo(outpoint)` | outpoint list from caller |
| **Foreign UTXOs** (spend non-wallet inputs) | `TxBuilder::add_foreign_utxo(...)` | full UTXO metadata |
| **RBF** (BIP-125) | `Wallet::build_fee_bump(txid).fee_rate(new_rate).finish()` | — |
| **Custom fee** (sat/vB or absolute) | `TxBuilder::fee_rate(rate)` / `.fee_absolute(amount)` | — |
| **Drain to address** | `TxBuilder::drain_wallet()` / `.drain_to(addr)` | — |
| **Multi-output transactions** | `.add_recipient(addr, amount).add_recipient(...)` (chainable) | — |
| **Change control** (drain / do_not_spend / only_spend) | `TxBuilder::do_not_spend_change()` / `.only_spend_change()` | — |
| **Signing** (in-process secp256k1) | `Wallet::sign(&mut psbt, SignOptions::default())` | — |
| **External signing** (third-party signer integration) | `Wallet::add_signer(keychain, ordering, Arc<dyn TransactionSigner>)` | implement `TransactionSigner` trait |
| **PSBT v1** (BIP-174) | `bitcoin::psbt::Psbt` re-exported via `bdk_wallet::bitcoin::psbt` | serialize/deserialize |
| **Balance** (confirmed / trusted_pending / untrusted_pending / immature) | `Wallet::balance() -> Balance` | — |
| **Transaction history** (canonical + pending) | `Wallet::transactions()` / `full_txs()` | — |
| **UTXO queries** | `Wallet::list_unspent()` / `get_utxo(outpoint)` | — |
| **Address generation** (multi-address, advance index, mark used) | `peek_address` / `reveal_next_address` / `mark_used` | — |
| **Descriptor-based wallets** (any miniscript descriptor) | `Wallet::create_with_params` accepts any parseable descriptor | — |
| **Persistence** (in-memory / SQLite / file) | `PersistedWallet` + `bdk_file_store::Store` + `WalletPersister` trait | one-line persister impl |
| **Checkpointing** (atomic sync persistence) | `Wallet::take_staged()` / `apply_update_events()` | — |
| **Descriptor export** (string) | `Wallet::public_descriptor(keychain)` | — |
| **Descriptor checksum** (8-char) | `Wallet::descriptor_checksum(keychain)` | — |
| **Wallet name from descriptor** (deterministic) | `wallet_name_from_descriptor` | — |
| **Network support** (Bitcoin / Testnet / Testnet4 / Signet / Regtest) | `bitcoin::Network` re-exported | — |
| **Error introspection** (InsufficientFunds, NoRecipients, etc.) | 5 sibling enums at `bdk_wallet::error` | map to our `Error` enum |
| **WordCount / language** (replaces MnemonicType) | `bdk_wallet::keys::bip39::WordCount` | — |
| **Sighash extraction** for external signing | `psbt.inputs[i].sighash` | — |
| **Watch-only wallets** (public-only descriptor, no signing) | `Wallet::new_single(public_descriptor)` | — |

**Use cases we still write custom code for** (BDK does NOT provide):

| Use case | Why BDK doesn't help | What we do |
|---|---|---|
| **Multi-sig** (P2SH/P2WSH/P2TR script-path) | miniscript supports it but BDK 3.1 doesn't expose a multi-sig builder | defer to v1.0+; miniscript dep stays in tree |
| **Hardware signer** (Ledger, Trezor, Tangem) | external `TransactionSigner` trait exists but no built-in transport | wrap trait for Phase 2 UniFFI |
| **BIP-137 message signing** (off-chain signed messages) | BDK signs PSBTs, not raw messages | compose with `bdk_wallet::bitcoin::hashes::sha256` + our `Signer` |
| **Encrypted mnemonic at rest** (v0.2) | BDK has no encryption | `argon2` + `aes-gcm` + `zeroize` |
| **Plausible deniability** (v1.0) | BDK has no multi-bucket concept | new design |
| **CPFP** | BDK has no `child_pays_for_parent` | manual RBF on parent + child |
| **Block explorer link** | BDK has no `tx_url` / `address_url` | pure `format!` |
| **Dust check** (pre-check before build) | BDK auto-rejects via `CreateTxError::OutputBelowDustLimit`; we pre-check | `ScriptBuf::len` + threshold |
| **Lightning** | separate crate (`rust-lightning`) | defer to separate spec |
| **Other UTXO chains** (BCH, LTC, DOGE, KAS) | BDK is BTC-only | separate plans per chain |

**Net:** BDK handles ~90% of the wallet surface. The remaining 10% is split between (a) genuine missing features (multi-sig, hardware, Lightning) and (b) explicit design choices (encryption, plausible deniability). The plan correctly identifies the boundary.

## Master index — 14 categories

| # | Category | Public surface (count) | BDK 3.1 status | Part doc |
|---|---|---|---|---|
| 1 | Wallet construction | `Wallet::create`, `create_single`, `create_with_params`, `create_from_two_path_descriptor`, `load`, `load_with_params`; `PersistedWallet::{create,load,persist}` and async variants; `CreateParams` / `LoadParams` builders | ✅ all present | [part 1](2026-08-05-bdk-features-01-construct-read-address.md) |
| 2 | Wallet reading | `network`, `balance`, `transactions`, `full_txs`, `list_canonical_txs`, `list_unspent`, `list_output`, `tx_details`, `public_descriptor`, `descriptor_checksum`, `latest_checkpoint`, `checkpoints`, `keychains`, `secp_ctx`, `get_utxo`, `insert_txout` | ✅ all present (note: no `master_fingerprint` at top level — use `keychains()`) | part 1 |
| 3 | Address generation | `peek_address`, `reveal_next_address`, `next_unused_address`, `reveal_addresses_to`, `list_unused_addresses`, `mark_used`, `unmark_used`, `is_mine`, `derivation_of_spk`, `next_derivation_index`, `derivation_index`; `KeychainKind::{External, Internal}`; `AddressInfo { index, address, keychain }` | ✅ all present (note: no `list_addresses` — use `list_unused_addresses`) | part 1 |
| 4 | Transaction building (TxBuilder) | `add_recipient`, `set_recipient`, `add_utxo`, `add_utxos`, `add_unspendable`, `do_not_spend_change`, `only_spend_change`, `fee_rate`, `fee_absolute`, `drain_wallet`, `drain_to`, `ordering`, `policy_path`, `add_xpub_key_only` (replaced by `add_global_xpubs` in 3.x), `finish` → `Result<Psbt, CreateTxError>` | ✅ all present (note: `add_xpub_key_only`/`only_xpub_key_only` from 0.x are GONE; replaced by `add_global_xpubs`) | [part 2](2026-08-05-bdk-features-02-tx-sign-psbt.md) |
| 5 | Signing | `Wallet::sign`, `Wallet::sign_with_signers`; `add_signer(keychain, SignerOrdering, Arc<dyn TransactionSigner>)`; `SignOptions` (7 fields); `TransactionSigner` + `InputSigner` traits | ✅ all present (note: no `sign_with`, no `add_external_signer`, no `mark_psbt_as_signed`, no `KeychainKey` enum in 3.x) | part 2 |
| 6 | PSBT | `bdk_wallet::psbt` module = one trait `PsbtUtils` with 3 methods: `get_utxo_for`, `fee_amount`, `fee_rate`; `extract_tx()` is on `bitcoin::psbt::Psbt`, not BDK | ✅ minimal (no PSBT v2 helpers yet — verify in spike) | part 2 |
| 7 | Chain sync | `apply_update`, `apply_update_events`, `apply_unconfirmed_txs`, `apply_block`/`apply_block_connected_to`, `apply_evicted_txs`, `insert_txout`, `get_utxo`, `latest_checkpoint`, `checkpoints`, `start_full_scan`, `start_sync_with_revealed_spks`; `bdk_chain::ChainPosition`/`ChainOracle`/`LocalChain`/`SyncRequest`/`FullScanRequest`; driven by `EsploraExt::sync` from `bdk_esplora` | ✅ all present (note: no `apply_anchors`, no `insert_tx`, no `list_chain_txouts`, no `checkpoint()` / `insert_checkpoint`) | [part 3](2026-08-05-bdk-features-03-sync-fees-persist.md) |
| 8 | Fees / RBF / CPFP | `build_fee_bump`, `calculate_fee`/`calculate_fee_rate`; `TxBuilder::fee_rate`/`fee_absolute`/`drain_wallet`/`drain_to`/`add_utxo`/`add_foreign_utxo`/`manually_selected_only`/`set_exact_sequence`; `drain_fee_rate`/`enable_rbf` are gone | ✅ RBF present; **CPFP NOT native** (must be done via the manual RBF flow on the parent + child spends) | part 3 |
| 9 | Persistence | `ChangeSet`; `Wallet::staged`/`staged_mut`/`take_staged`; `CreateParams::create_wallet`/`create_wallet_no_persist`/`create_wallet_async`; `LoadParams::load_wallet_no_persist`; `PersistedWallet`; `WalletPersister` trait; `bdk_file_store::Store::{create,load,load_or_create,dump,append}`; foreign impls on `rusqlite::{Connection,Transaction}`; built-in `bdk_sqlite` via `rusqlite` feature | ✅ all present (note: `Store::open_or_create_new`/`write` from old API renamed to `load_or_create`/`append`) | part 3 |
| 10 | Keys module | `keys::bip39::{Mnemonic, WordCount, Language, Error, MnemonicWithPassphrase}` re-exported (behind `keys-bip39` feature); `GeneratableKey<Ctx>` trait; `DescriptorSecretKey` (3 variants: `Single`/`XPrv`/`MultiXPrv`); `DescriptorPublicKey` re-export from miniscript; `KeyError` (6 variants); `ValidNetworkKinds` (not `ValidNetworks`) | ✅ all present (note: `WordCount` not `MnemonicType`; no `ValidNetworks`) | [part 4](2026-08-05-bdk-features-04-keys-errors-utils-network.md) |
| 11 | Errors | **5 sibling enums** under `bdk_wallet::error`: `CreateTxError` (18 variants), `BuildFeeBumpError` (6), `LoadError` (5), `LoadMismatch` (3), `MiniscriptPsbtError` (3); plus `keys::KeyError` (6) and re-exported `chain` errors | ✅ 5-enum structure (no single `bdk_wallet::Error` like in 0.x); map to our `Error` enum at the boundary | part 4 |
| 12 | Utilities | `wallet_name_from_descriptor` (line `wallet/mod.rs:2851-2871`); `version() -> &'static str`; `Wallet::descriptor_checksum(keychain) -> String`; `Wallet::Debug` impl (no `to_string`) | ✅ all present | part 4 |
| 13 | Network | `Wallet::network() -> bitcoin::Network`; exhaustive enum: `Bitcoin`, `Testnet`, `Testnet4`, `Signet`, `Regtest`; per-network HRPs: `bc`, `tb`, `tb`, `tb`, `bcrt`; `CreateParams::network(self, Network)` setter; `LoadParams::check_network(Network)` validator; **no default ports** (caller picks) | ✅ all present | part 4 |
| 14 | Integration | `bdk_wallet::bitcoin` re-export = `bitcoin ^0.32` (resolved 0.32.100); `bdk_wallet::miniscript` re-export = `miniscript ^12`; `bdk_wallet::chain` re-export = `bdk_chain ^0.23`; `bdk_wallet::file_store` (optional) = `bdk_file_store ^0.22`; `bdk_wallet::keys::bip39` re-export = `bip39 ^2.2.2` (gated on `keys-bip39`); `bdk_esplora` / `bdk_electrum` / `bdk_bitcoind_rpc` are dev-deps, not re-exported; **`secp256k1` is NOT re-exported** — use `bdk_wallet::bitcoin::secp256k1` and add your own dep (compatible 0.29.x) | ✅ all present (note: secp256k1 is a separate dep) | part 4 |

## Feature flags (11 total)

| Flag | Default | What it enables |
|---|---|---|
| `std` | ✅ | Standard library support |
| `default` | ✅ | (no extra features by default) |
| `keys-bip39` | ❌ | Re-export of `bip39::Mnemonic` under `bdk_wallet::keys::bip39` |
| `all-keys` | ❌ | All key-format re-exports |
| `bip39` | ❌ | Re-export of `bip39` at top level (alternative path) |
| `rusqlite` | ❌ | `bdk_sqlite` built-in (foreign `WalletPersister` impl on `rusqlite::{Connection, Transaction}`) |
| `file_store` | ❌ | Re-export of `bdk_file_store` under `bdk_wallet::file_store` |
| `bdk_file_store` | ❌ | (alias of `file_store`) |
| `anyhow` | ❌ | `anyhow::Error` integration in some `Result` returns |
| `compiler` | ❌ | Forwards to `miniscript` `compiler` feature |
| `tempfile` | ❌ | `tempfile::TempDir` integration for tests |
| `test-utils` | ❌ | `bdk_wallet::test_utils` (test fixtures) |

**Our `bitcoin-wallet-core` Cargo.toml enables:** `keys-bip39` (for the Mnemonic re-export we use), `file_store` (for `bdk_file_store` SQLite persistence via `take_staged`).

## What's NOT in BDK 3.1 (removed from 0.x or never added)

| Removed/never-existed | What we do instead |
|---|---|
| `Wallet::master_fingerprint()` (top-level) | Reach via `Wallet::keychains()` → keychain descriptor |
| `Wallet::public_key()` (top-level) | Reach via `Wallet::public_descriptor(keychain)` |
| `Wallet::list_addresses()` | Use `list_unused_addresses()` or iterate `KeychainKind` |
| `Wallet::list_canonical_txids()` / `list_tx()` (named) | Use `list_canonical_txs()` + `.tx_node.txid`, or `transactions()` / `full_txs()` |
| `add_xpub_key_only()` / `only_xpub_key_only()` (0.x) | Replaced by `add_global_xpubs()` in 3.x |
| `sign_with()` (we expected this) | Use `sign_with_signers()` (the actual API) |
| `add_external_signer()` (we expected this) | Use `add_signer(keychain, SignerOrdering, Arc<dyn TransactionSigner>)` |
| `mark_psbt_as_signed()` | Not needed in 3.1 (BDK signs internally; partial signers attach via `InputSigner`) |
| `KeychainKey` enum (0.x) | Gone; only `KeychainKind { External, Internal }` remains |
| `Wallet::apply_anchors()` / `insert_tx()` / `list_chain_txouts()` | Use `apply_update_events()` + `insert_txout()` |
| `Wallet::checkpoint()` / `insert_checkpoint()` | Use `latest_checkpoint()` / `checkpoints()` |
| `bdk_electrum::ElectrumExt` trait | Replaced by `BdkElectrumClient` |
| `bdk_sqlite` (separate crate) | Built into `bdk_wallet` via `rusqlite` feature |
| `Store::open_or_create_new()` / `write()` | Renamed to `load_or_create()` / `append()` |
| `child_pays_for_parent` (TxBuilder) | NOT native; CPFP must be done via the manual RBF flow on the parent + child spends |
| `enable_rbf()` / `drain_fee_rate()` (TxBuilder) | Gone; RBF is via `build_fee_bump`, fee is via `fee_rate`/`fee_absolute` |
| `fee_paid_by()` / `network_fee_rate()` (Wallet) | Don't exist; compute fee from `psbt.fee_amount()` or `psbt.fee_rate()` |
| `Wallet::from_mnemonic()` shortcut | Doesn't exist; must build descriptor string manually |
| `Wallet::MnemonicType` | Replaced by `Wallet::keys::bip39::WordCount` |
| `Wallet::ValidNetworks` | Use `ValidNetworkKinds` (a `BTreeSet<NetworkKind>`) |
| Single `bdk_wallet::Error` enum (0.x) | Replaced by 5 sibling enums: `CreateTxError`, `BuildFeeBumpError`, `LoadError`, `LoadMismatch`, `MiniscriptPsbtError` |
| `secp256k1` re-export | NOT re-exported; use `bdk_wallet::bitcoin::secp256k1` and add your own dep |
| PSBT v2 helpers | Not surfaced; `TxBuilder::finish` returns v1 `bitcoin::psbt::Psbt`. Verify in spike whether v2 helpers hide in source |

## Full-task verification list for Task 31 (BDK API spike)

The spike must validate these specific assumptions before any implementation work:

1. `bdk_wallet::keys::bip39::Mnemonic::generate(12)` works with the `keys-bip39` feature flag.
2. `Mnemonic::parse_in(Language::English, s)` returns the expected type.
3. `Mnemonic::to_seed(passphrase)` returns `[u8; 64]`.
4. `bip32::XPrv::derive_from_path(&seed, &DerivationPath::from_str("m")?)` returns `XPrv`.
5. `XPrv::derive_path(&path)` returns child `XPrv`.
6. `XPrv::to_string()` produces a string parseable as `wpkh(...)` descriptor.
7. `bdk_wallet::Wallet::create(descriptor, change_descriptor).network(...).create_wallet_no_persist()?` works.
8. `wallet.peek_address(KeychainKind::External, 0).address` returns a `bitcoin::Address`.
9. `wallet.balance()` returns `bdk_wallet::Balance`.
10. `bdk_esplora::EsploraExt::full_scan` and `sync` drive a full chain sync.
11. `wallet.build_tx()` returns `Result<Psbt, CreateTxError>` — specifically, the `CoinSelection(InsufficientFunds { needed, available })` variant is reachable.
12. `wallet.build_fee_bump(&txid)` returns `Result<Psbt, BuildFeeBumpError>`.
13. `wallet.sign(&mut psbt, SignOptions::default())` produces a signed `Transaction`.
14. `bdk_file_store::Store::load_or_create(name, magic).append(&changeset)` persists correctly.
15. `Wallet::take_staged()` returns the right `ChangeSet` for persistence.
16. `Wallet::load().descriptor(External, Some(d)).descriptor(Internal, Some(cd)).load_wallet_no_persist()` reloads correctly.
17. `Wallet::descriptor_checksum(KeychainKind::External)` returns a 8-char descriptor checksum.
18. `Wallet::network()` returns the right `bitcoin::Network` value.
19. The 5 error enums (`CreateTxError`, `BuildFeeBumpError`, `LoadError`, `LoadMismatch`, `MiniscriptPsbtError`) exist at the paths `bdk_wallet::error::{name}`.
20. `bdk_wallet::bitcoin::psbt::Psbt` re-export works (used in Task 12).
21. `bdk_wallet::bitcoin::secp256k1::Message::from_digest(hash)` works (used in Task 4 + Task 13).
22. No PSBT v2 helpers are hidden in `bdk_wallet::psbt` (grep the source).
23. `DescriptorSecretKey` round-trips through `XPrv` correctly (used in Task 3 for the BIP-32 → xprv path).
24. `Wallet::is_mine(&address)` returns `true` for an address we derived, `false` otherwise.

If any of these 24 fail, the fix is one of: enable a different feature flag, use a different type path, fall back to standalone `bip39`/`bip32`, or document the gap in `docs/wallets/2026-08-05-bdk-features-NN-*.md` and update the plan.

## What this means for `bitcoin-wallet-core`

| Plan task | Maps to BDK 3.1 API |
|---|---|
| Task 9 (Wallet::from_mnemonic) | `Wallet::create(descriptor, change_descriptor).network().create_wallet_no_persist()` — descriptor must be pre-built from `bdk_wallet::keys::bip39::Mnemonic` (generate) + `bip32::XPrv::derive_path` + `format!("wpkh({xprv})/0/*")` |
| Task 10 (multi-address) | `wallet.peek_address`, `wallet.reveal_next_address`, `wallet.next_unused_address`, `wallet.list_unused_addresses` |
| Task 11 (tx builder) | `wallet.build_tx()` → `TxBuilder` chain. Map `CreateTxError::CoinSelection(InsufficientFunds { needed, available })` to our `Error::InsufficientFunds` |
| Task 12 (PSBT) | `bdk_wallet::bitcoin::psbt::Psbt` re-export. No BDK-side `psbt` helpers beyond the 3-method `PsbtUtils` trait. |
| Task 13 (sign + broadcast) | `wallet.sign(&mut psbt, SignOptions::default())`, then `psbt.extract_tx()`, then `EsploraExt::broadcast(&tx)` |
| Task 14 (fee + RBF) | `wallet.fee_estimate()` (custom impl via `EsploraClient::get_fee_estimates`), `wallet.build_fee_bump(&txid).fee_rate(new_rate).finish()`. CPFP not native — manual RBF on the parent. |
| Task 28 (Signer trait) | The 3.x `Signer` is a trait alias: `add_signer(keychain, SignerOrdering, Arc<dyn TransactionSigner>)`. Our `tx::sign_external::Signer` (Plan Task 28) is a separate trait for Phase 2 UniFFI — wraps `TransactionSigner`. |
| Task 32 (watch-only) | `Wallet::new_single(public_only_descriptor)` — same as `create_single` with no signing key. |

## Sources

All 4 part docs cite:
- `https://docs.rs/bdk_wallet/3.1.0/bdk_wallet/`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.TxBuilder.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.SignOptions.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.CreateTxError.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.BuildFeeBumpError.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/coin_selection/struct.InsufficientFunds.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/psbt/trait.PsbtUtils.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/signer/index.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/index.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/keys/bip39/index.html`
- `https://docs.rs/bdk_wallet/latest/bdk_wallet/persisted/index.html`
- `https://docs.rs/bdk_chain/latest/bdk_chain/`
- `https://docs.rs/bdk_esplora/latest/bdk_esplora/`
- `https://docs.rs/bdk_electrum/latest/bdk_electrum/`
- `https://docs.rs/bdk_file_store/latest/bdk_file_store/`
- `https://github.com/bitcoindevkit/bdk` (note: source path layout has moved since 0.x; docs.rs source anchors are the current reference)

## How to use these 5 docs

| Reader | Path |
|---|---|
| Engineer starting a task | Read the per-task `Maps to BDK 3.1 API` row in `2026-08-05-rust-bitcoin-wallet-task-sdk-map.md`, then drill into the relevant part doc (1-4) for full signatures |
| Code reviewer | Same path; cross-check that the implementation uses the BDK 3.x method (not the 0.x name) |
| Task 31 spike engineer | Use the 24-item "Full-task verification list" in this index; mark each verified / corrected |
| Plan editor | Use the "What's NOT in BDK 3.1" table to update plan bodies if a 0.x-era method is referenced |
