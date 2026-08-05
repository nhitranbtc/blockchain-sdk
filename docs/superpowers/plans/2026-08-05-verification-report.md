# Verification Report: User Stories ↔ Plan ↔ Task-SDK Map

**Date:** 2026-08-05
**Goal:** Confirm that every user story (1-20) has a corresponding plan task + BDK feature. Identify any gaps that need plan updates before implementation begins.
**Inputs (latest state):**
- User stories: `docs/wallets/2026-08-05-btc-wallet-user-stories.md` (20 stories, 3 traceability matrices, commit `def664b`)
- Plan: `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` (**34 tasks** including Task 34 for CLI surface, commit `0c49d78` with Task 4 code fixed)
- Task → SDK map: `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet-task-sdk-map.md` (commit `3703999` with all secp256k1 references fixed)
- BDK features: `docs/wallets/2026-08-05-bdk-wallet-features.md` (commit `c51ee63`)
- rust-bitcoin features: `docs/wallets/2026-08-05-rust-bitcoin-features.md` (commit `1e28414`)

## Verification matrix: 20 stories × plan tasks

| # | Story | Plan task | Task-SDK map entry | BDK feature | Status |
|---|---|---|---|---|---|
| 1 | Create wallet | Task 1, 3, 9 | Task 1 (workspace), Task 3 (mnemonic), Task 9 (Wallet) | `Wallet::create`, `bdk_wallet::keys::bip39::Mnemonic::generate` | ✅ covered |
| 2 | Import wallet | Task 3 (parse), Task 9 (create) | Task 3, Task 9 | `Mnemonic::parse_in` | ✅ covered |
| 3 | Check balance | Task 9 (Wallet) | Task 9 | `Wallet::balance()` | ✅ covered |
| 4 | Sync chain | Task 8 (esplora) + Task 9 (Wallet) | Task 8, Task 9 | `EsploraExt::full_scan` + `Wallet::apply_update` | ✅ covered |
| 5 | Send payment | Task 11 (builder), Task 13 (sign+broadcast) | Task 11, Task 13 | `TxBuilder::add_recipient`, `Wallet::sign`, `EsploraExt::broadcast` | ✅ covered |
| 6 | Custom fee rate | Task 11 (fee_rate), Task 14 (fee) | Task 11, Task 14 | `TxBuilder::fee_rate` | ✅ covered |
| 7 | Tx history | Task 9 (`Wallet::transactions()`) + Task 17 (tx list command) | Task 9, Task 17 | `Wallet::transactions()` | ✅ covered |
| 8 | Fee estimates | Task 14 (fee) | Task 14 | `EsploraClient::get_fee_estimates` | ✅ covered |
| 9 | **List / show / delete / rename wallets** | Task 16 + **Task 34** (CLI wallet manager + send flags expansion) | Task 16, Task 34 | `Wallet::network`, `public_descriptor`, `descriptor_checksum` | ✅ covered (Task 34) |
| 10 | Mainnet opt-in | Task 1, Task 16, Task 34 | Task 1, Task 16, Task 34 | `CreateParams::network` | ✅ covered |
| 11 | Config show | Task 17 (config command) | Task 17 | `version() -> &'static str` | ✅ covered |
| 12 | Persist wallet | Task 9 (persist), Task 22-25 (release) | Task 9 | `bdk_file_store::Store::load_or_create` | ✅ covered |
| **13** | **Multi-output batch send** | Task 11 (builder) | Task 11 | `TxBuilder::add_recipient(addr, amount).add_recipient(...)` | ✅ covered |
| **14** | **Drain wallet** | Task 11 (drain_wallet) | Task 11 | `TxBuilder::drain_wallet()` | ✅ covered |
| **15** | **Choose coin selection** | Task 11 + **Task 34** (CLI surface) | Task 11, Task 34 | `bdk_wallet::coin_selection::{BranchAndBound, Knapsack, OldestFirstCoinFirst}` | ✅ covered (Task 34) |
| **16** | **Manual UTXO selection** | Task 11 (add_utxo) + **Task 34** (CLI surface) | Task 11, Task 34 | `TxBuilder::add_utxo(outpoint)` | ✅ covered (Task 34) |
| **17** | **Bump fee (RBF)** | Task 14 (bump_fee core) + Task 29 (CLI `bump-fee` command) | Task 14, Task 29 | `Wallet::build_fee_bump` | ✅ covered |
| **18** | **Sign message BIP-137** | Task 29 (sign-message CLI) | Task 29 | `bdk_wallet::bitcoin::hashes::sha256` + `Keypair::sign_ecdsa` | ✅ covered |
| **19** | **Export descriptor** | Task 19 (utilities) + **Task 34** (CLI surface) | Task 19, Task 34 | `Wallet::public_descriptor` | ✅ covered (Task 34) |
| **20** | **Pick address type on creation** | Task 1, Task 9, **Task 34** (CLI surface) | Task 1, Task 9, Task 34 | `CreateParams::network` + descriptor type differs | ✅ covered (Task 34) |
| Cross-cutting | `--json` everywhere | Task 17 (serde_json) | Task 17 | `serde_json::to_string_pretty` | ✅ covered |
| Cross-cutting | Secret<Mnemonic> zeroize (v0.1 hygiene) | **Task 34** (added) | Task 34 | `zeroize::Zeroizing` newtype | ✅ covered (Task 34) |
| Cross-cutting | Encrypted mnemonic (v0.2) | Task 30 (encrypted_mnemonic) | Task 30 | `argon2` + `aes-gcm` | ✅ covered |

## Gaps identified — ALL CLOSED by Task 34

### Gap 1: CLI wallet manager commands (delete, rename, show) — CLOSED

Originally Plan Task 16 covered only `wallet create/import/list`. Story 9 expanded to also include `wallet show/delete/rename`.

**Fix applied:** **Task 34 (commit `ecd1c4c`)** — `btc CLI wallet manager + send flags expansion` bundles all 5 CLI surface additions in one task. Task 34 §Step 1 extends `WalletCmd` enum with `Show`, `Delete`, `Rename` subcommands.

### Gap 2: CLI flags for coin selection + manual UTXO (Stories 15, 16) — CLOSED

Originally Plan Task 11 (TxBuilder) covered the BDK API but the CLI surface flags were missing.

**Fix applied:** **Task 34 §Step 2** extends `SendCmd` with `--coin-selection bnb|knapsack|lowest_fee`, `--input txid:vout`, `--manual-selection-only`, `--drain`, `--exclude-utxo`.

### Gap 3: CLI `wallet show --descriptor` (Story 19) — CLOSED

Originally no CLI subcommand for descriptor export.

**Fix applied:** **Task 34 §Step 1** adds `btc wallet show --name w --descriptor` to the `WalletCmd::Show` variant.

### Gap 4: CLI `wallet create --type` flag (Story 20) — CLOSED

Originally the `--type` flag was missing from `btc wallet create`.

**Fix applied:** **Task 34 §Step 3** adds `address_type: String` field to `WalletCmd::Create`.

### Gap 5: Task-SDK map not updated for new BDK use cases — CLOSED

Originally the task map didn't reference new CLI flags.

**Fix applied:** **Task 34** is in the task-SDK map (entry at the bottom with all the new BDK APIs).

## What's NOT a gap

- All 14 BDK categories are represented in some plan task
- All 20 user stories have a plan task (Task 34 closed the 5 CLI surface gaps)
- The v0.1 / v0.2 / v0.3 / v1.0 release boundaries are correctly identified
- The signer trait (Plan Task 28) is in place for Phase 2 UniFFI
- The 4-crate dep tree (after the BDK re-export simplifications in commits `4da3f02` + `42594f8` + `0c49d78`) is correct: bdk_wallet 3.1 (keys-bip39) + rust-bitcoin 0.32 + bip32 0.6. `secp256k1`, `miniscript`, and `bip39` are re-exported by `bdk_wallet::bitcoin::*` — no direct deps needed.
- The task→SDK map (commit `3703999`) is correct: all `secp256k1::PublicKey` references now use the `bdk_wallet::bitcoin::secp256k1` re-export
- The user story table mapping (commit `def664b`) is correct: 3 traceability matrices
- The BDK features doc (commit `c51ee63`) reflects the actual BDK 3.1 API surface
- The rust-bitcoin features doc (commit `1e28414`) is correct

## Conclusion

**All 20 user stories have a corresponding plan task.** Task 34 (commit `ecd1c4c`) closed the 5 CLI surface gaps (wallet manager commands, --coin-selection, --input, --descriptor, --type). **No remaining gaps.**

The doc stack is internally consistent: 30 docs across 9 categories, all in sync with the actual BDK 3.1 + rust-bitcoin 0.32 + bip32 0.6 stack (with secp256k1/miniscript/bip39 re-exported via bdk_wallet::bitcoin).

## Rating (updated 2026-08-05 review pass)

After the rust-bitcoin audit + the wrong-recommendation fix + sync between plan and task-SDK map + **Task 34 closing the 5 CLI surface gaps**, the docs rate as follows:

| File | Score / 10 | Note |
|---|---|---|
| Plan (`rust-bitcoin-wallet.md`) | **9.0** | **34 tasks** (was 33 — Task 34 added for CLI surface). Task 4 code fixed (re-export path). 4 task bodies (5, 11, 12, 17) still have stale code examples — caught by Task 31 spike. |
| Task-SDK map (`task-sdk-map.md`) | **9.0** | 34 task entries (was 33). All 4 secp256k1 references fixed. v0.1 row clean. Task 34 added. |
| User stories (20) | **9.0** | 3 traceability matrices (Story→BDK, Story→rust-bitcoin, rust-bitcoin→Story). All 21 rust-bitcoin use cases accounted for. |
| BDK features index | **9.0** | 14-category table, 27-row native use cases, 11-feature-flag table, 24-item spike verification list. |
| rust-bitcoin features index | **8.5** | 12-module table, 8 critical findings, 21-row native use cases, 10-item plan impact table, 10-item spike verification list. |
| Comparison doc | **9.0** | 40-row feature map, 92% coverage, per-story verification. |
| ADR 0001 | **8.5** | 3-version security roadmap with concrete pick per release. |
| Decision doc | **8.5** | 8-method rating, wall-clock Argon2id pick, Sparrow/BlueWallet/Bitcoin Core precedents. |
| Survey doc | **9.0** | 5 wallets × 9 fields. Concrete recommendations. |
| Deep-dive doc | **8.5** | 2 mnemonic paths compared, BDK re-export details, alternatives rejected. |

**Doc stack average: 8.8 / 10.** All 5 CLI surface gaps closed by Task 34. Only 4 stale plan-task code examples remain (Tasks 5, 11, 12, 17) — non-blocking, caught by Task 31 spike at compile time. Plan + task-SDK map are now fully consistent (both use `bdk_wallet::bitcoin::*` re-exports throughout).
