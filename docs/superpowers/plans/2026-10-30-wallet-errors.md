# wallet-errors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace five hand-written, mutually contradictory CLI error→exit-code tables with one shared category enum and one exit-code table, so that every one of the five CLIs reports the same condition with the same exit code.

**Architecture:** A new zero-dependency crate `wallet-errors` owns a six-variant `ErrorCategory` enum and the single `ErrorCategory → i32` table. Each CLI keeps its **own** variant→category classifier — chain-specific error granularity is a security feature and is deliberately not unified — and calls the shared table for the exit code. No engine crate changes behaviour; the mapping lives entirely in the CLI layer.

**Tech Stack:** Rust 2021, workspace `rust-wallet-app/`, toolchain 1.98.1 (`rust-toolchain.toml`), `cargo` workspace with 15 members.

**Spec:** `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` — §Components 1 (`wallet-errors`), §Error handling, §Testing, §Sequenced plan Phase 1.

**ADRs:** `docs/adr/2026-10-30-adr-0003-cross-chain-abstraction-strategy.md` (strategy) · `docs/adr/2026-10-30-adr-0004-chain-traits-deprecation.md` (amends 0003; not touched by this plan).

---

## Scope

This plan implements **Phase 1 only**. It does not implement Phase 2 (`wallet-facade`), Phase 3 (`wallet-storage`) or Phase 4 (`wallet-crypto`), and it does not fix the Phase 0 defects (release-build break F1, Solana FFI panic barrier F3, Solana FFI error collapse F4, #579, #580). Those have their own plans; Phase 1 is independent of all of them.

**Prerequisite (executor, before Task 1):** Phase 1 needs a tracker issue. File it per L13 step 11a / PL22 — title `Backlog: unify CLI error categories and exit codes across all five CLIs`, labels `backlog`, `priority/p1`, `task`, chain labels `rust-btc-core` `rust-eth-core` `rust-evm-core` `rust-tron-core` `rust-sol-core` (**not** `rust-polygon-core` — that label does not exist in the tracker; Polygon work carries `rust-evm-core`), no milestone (the repo's only milestones are `tron-v0.1` and `sol-wallet-core v0.1`, neither of which covers this work), no assignee. Body carries the acceptance criteria below and `Refs` the design spec, ADR 0003 and the plan. **PAUSE before `gh issue create`** (PL22). The issue number becomes the L25 parent-branch prefix: `task/<N>-wallet-errors`.

---

## Global Constraints

- **The category set is exactly six** — `Success`, `User`, `Network`, `InsufficientFunds`, `Signing`, `Broadcast`. This is fixed to `docs/wallets/CONTEXT.md:91-98`. Do not add `Storage` or `Crypto`; the spec explicitly rejected an eight-category set because it would contradict the document this phase promotes to the tested spec.
- **The exit-code table is exactly** `Success=0, User=1, Network=2, InsufficientFunds=3, Signing=4, Broadcast=5`.
- **A clap usage error exits 1 (`User`), not 2.** Every CLI uses clap, and clap's default exit code for a usage error is 2 — which this table assigns to `Network`. Left alone, every CLI would report "network error, retry later" for a bad command line, and an automation rule keying on 2 could loop on a usage error. Each CLI therefore parses with `Cli::try_parse()`, prints clap's error, and exits `ErrorCategory::User.exit_code()` (1); `--help` and `--version` keep clap's exit 0.

  Migrating a CLI's assertions to the new codes touches **four** distinct classes, and they need different handling — **read all four**:

  1. **Assertions on a clap usage error** (the CLI rejected the arguments) become `.code(1)`.
  2. **Assertions on a handler-level bad-input error** also move 2→1, because bad input is now `User` — but these were never clap assertions, so identify them by what they exercise, not by their old exit code.
  3. **`assert_ne!(code, Some(2))` "lock-down" guards** — written to catch *clap* rejecting the arguments — become **vacuous**, not renumbered: a clap rejection now exits 1, so the guard can never fire. They also stop meaning what they said, because a genuine Network failure now exits 2. Rewrite them to assert clap's `Usage:` banner on stderr, the way `sol` does — **not** to `assert_ne!(code, Some(1))`, which would replace one dead guard with another, since a handler-level bad input also exits 1.
  4. **Any other `assert_ne!(code, Some(N))` "lock-down" guard** keyed on an old code for a non-clap condition must be re-keyed on the **new** code for the condition it names, or dropped as redundant if the test's own final assertion already covers it. Leaving it in place leaves a guard that no longer detects what its message claims.
- **`wallet-errors` has zero dependencies** — not even `thiserror`. This keeps `Cargo.lock` stable after Task 1 (no merge conflicts across the eight sub-task branches) and adds no supply-chain surface.
- **No engine crate changes behaviour.** `bitcoin-wallet-core`, `evm-wallet-core`, `eth-wallet-core`, `polygon-wallet-core`, `tron-wallet-core` and `sol-wallet-core` are untouched except for the **deletion** of `evm_wallet_core::Error::exit_code()` in Task 4, which the spec sanctions ("Phase 1 deletes or delegates it"). Delegation is not an option: it would require an engine → `wallet-errors` edge, which the spec forbids ("no engine edge changes"), and a retained shim would be the *second competing table* that finding F5 is about.
- **No engine gains a `wallet-errors` dependency.** The mapping lives in the CLI layer.
- **The per-chain error enums stay.** Their granularity is a security feature (`bitcoin-wallet-core/src/error.rs:109-120` keeps `WalletStore` separate from `Storage` to preserve a file-existence oracle closure). This phase unifies the *category*, never the enum.
- **Wrong password and corrupt blob stay indistinguishable** — both map to `Signing` (exit 4) in every chain, and must not be distinguishable by exit code.
- **The `Signing` bucket covers credential failures *and* internal-machinery failures.** A variant meaning *the machine could not do its job* — an OS RNG failure, a KDF failure, a cipher-init failure, a transaction-construction failure, a runtime that could not be built, a feature that is not implemented — is `Signing` (4). `User` (1) means the **operator** supplied something unusable. That is why Solana's `Unimplemented` and TRON's async-runtime-construction failure are both 4. The `Examples` cell in `docs/wallets/CONTEXT.md` names this scope (Task 7); the parity test asserts only the label and the exit code, so that cell is free text.
- **`cargo test -p <crate>` only** in the local verify gate. Never `--workspace` (L55: the workspace-wide run crosses the 300 s Bash timeout on `bitcoin-wallet-core`'s integration tests).
- **`rust-wallet-app/Cargo.lock` is part of every CLI task's commit.** Each task adds a `wallet-errors = { workspace = true }` dependency to a crate manifest, and a manifest change without the lock desyncs it and breaks `--locked` builds. Stage the lock file with the manifest in the same commit.
- **Rust edition / toolchain:** workspace `edition = "2021"`, `rust-toolchain.toml` pins 1.98.1. Do not bump `rust-version` — the declared MSRV 1.85 is already unachievable because the vendored anychain stack needs 1.98.1; that is a separate, pre-existing issue.
- **Error enums marked `#[non_exhaustive]`** (`bitcoin-wallet-core/src/error.rs:28`, `evm-wallet-core/src/error.rs:26`, `tron-wallet-core/src/error.rs:10`) force a wildcard arm in a downstream match. The wildcard maps to `ErrorCategory::Signing` in every case; every currently-existing variant is pinned by a table-driven test, and Task 8's CI job closes the loop on future variants. `sol_wallet_core::Error` is **not** `#[non_exhaustive]`, so the Solana classifier is exhaustive — a new Solana variant is a compile error until classified.
- **L24 doc set travels with every sub-task PR** — one `CHANGELOG.md` `[Unreleased]` bullet **per changed CLI** — a task that changes two crates carries two bullets, both in the same flat list — with a `**BREAKING**:` prefix where the change is user-visible. The `[Unreleased]` section is a **flat, newest-first bullet list**: the new entry goes immediately after the `## [Unreleased]` heading, with the `[internal]` / `[user-facing]` tag carrying the classification. Do **not** add a `### Added` / `### Changed` heading — those belong to the *versioned* sections and are applied at release cut, and a heading here would reclassify every existing flat bullet. **No User Stories row is flipped in Phase 1** — there is no User Stories table inside `[Unreleased]` (the tables live under the released version sections), and no Phase-1 task completes a user story; these tasks change how errors are *reported*, not what the software can do.
- **Every per-chain `CHANGELOG.md` bullet in this plan must be verified against that CLI's pre-migration classifier before it is used.** The bullet text in each task below is written by hand and has been wrong — the Solana bullet mis-stated two old→new arrows, and the TRON bullet mis-stated three. Read the CLI's old classifier out of git and check every arrow in the bullet against it. The pre-migration classifier lives at a different path per chain:

  | CLI | pre-migration classifier | the CLI's own old exit path |
  |---|---|---|
  | `sol` | `rust-wallet-app/crates/sol/src/handlers/error.rs` | `rust-wallet-app/crates/sol/src/main.rs` |
  | `tron` | `rust-wallet-app/crates/tron/src/handlers/mod.rs` | `rust-wallet-app/crates/tron/src/main.rs` |
  | `eth` | `rust-wallet-app/crates/evm-wallet-core/src/error.rs` | `rust-wallet-app/crates/eth/src/main.rs` |
  | `polygon` | `rust-wallet-app/crates/polygon/src/main.rs` — every error mapped to exit 1; the EVM engine table was never consulted | `rust-wallet-app/crates/polygon/src/main.rs` |
  | `btc` | `rust-wallet-app/crates/btc/src/main.rs` — bespoke: invalid mnemonic → exit 2, anything else → exit 1; there is no engine classifier | `rust-wallet-app/crates/btc/src/main.rs` |

  The `sol` row points at the file that now holds the **new** classifier — it is the reference implementation. To see sol's pre-migration table, read it at the base that predates the sol migration: `git show 4e56009:rust-wallet-app/crates/sol/src/handlers/error.rs`.

  e.g. `git show <task-base>:rust-wallet-app/crates/tron/src/handlers/mod.rs`.

  A bullet that has not been checked is not ready to ship.
- **Never auto-commit** (house rule). Every task ends at a PAUSE step; the commit happens only after the maintainer approves.
- **L25 sub-task workflow** applies — eight sub-tasks, so a parent branch `task/<N>-wallet-errors` is the integration point; each sub-task branches from the parent, PRs to the parent, and merges in order with `--no-ff`.

---

## Review Focus

Five input classes the spec implies that no task's happy-path test exercises. Each names the input and the behaviour a reasonable person would expect; the owning task pins it with a test.

1. **A wallet file that is corrupt on disk.** The operator must see exit 4 (`Signing`) and the *same* message a wrong password produces — not a distinguishable error. Bitcoin's `WalletStore` deliberately collapses not-found, wrong-password, wrong-network and corrupt-blob into one variant (`bitcoin-wallet-core/src/error.rs:110-120`); EVM's `WalletCorrupt` and `DecryptionFailed` must land on the same category. Owned by Tasks 2, 4, 5, 6.
2. **An empty passphrase.** Exit 1 (`User`) — the operator supplied something unusable — and *distinct* from a wrong passphrase's exit 4 (`Signing`). These are different conditions (`eth/src/handlers.rs:127` constructs `InvalidPassword("password must be non-empty")`; a wrong passphrase surfaces as `DecryptionFailed`), so they get different codes on purpose. A regression here re-opens exactly the ambiguity `CONTEXT.md:96` records. Owned by Task 4. **Scope: the paths where the CLI refuses the passphrase before the library call — create and import. Where the passphrase opens an *existing* wallet, an empty passphrase is indistinguishable from a wrong one and exits 4; the per-chain shape is set out in Task 7's divergences.**
3. **A node that is unreachable.** Exit 2 (`Network`) on every chain — Bitcoin's `Network`/`Esplora`/`Electrum`, Solana's `Transport`/`Rpc`, TRON's `Node`, EVM's `Rpc` — so one automation rule can retry on a single code across all five CLIs. Owned by Tasks 2, 3, 4, 5, 6.
4. **A transfer the balance cannot cover.** Exit 3 (`InsufficientFunds`) on every chain. Solana today reports this as a *transport* error (`sol/src/handlers/error.rs` maps `InsufficientFunds` to the same code as `Transport`), which under the old taxonomy collided with the network bucket; the new taxonomy separates them. Owned by Task 2.
5. **A transaction the node rejects.** Exit 5 (`Broadcast`), distinct from a network failure, so automation does not retry a transaction that will never be accepted. Owned by Tasks 2 and 4.

---

## The contract

### The six categories

| Category | Exit | Scope (the rule a classifier applies) |
|---|---:|---|
| `Success` | 0 | The operation completed. Never produced by an error classifier. |
| `User` | 1 | Determined by something the operator supplied or owns and can correct: malformed mnemonic / address / amount / key, bad flag combination, missing wallet, malformed config, an unsafe source-file mode, a refused path, a filesystem or permission failure, an **empty** passphrase, an operator-declined confirmation. |
| `Network` | 2 | The node or transport failed to answer, timed out, or answered unintelligibly: unreachable, DNS/TLS failure, HTTP non-success, JSON-RPC error envelope, malformed response, receipt timeout. |
| `InsufficientFunds` | 3 | The balance will not cover the amount plus the fee. |
| `Signing` | 4 | The key material is unusable or the operation that consumes it failed: wrong passphrase, decryption failure, corrupt blob, key derivation failure, PSBT/transaction construction, signature failure, and — as the house fallback for an internal fault — a crypto-primitive or internal-machinery failure. |
| `Broadcast` | 5 | The network accepted the request and rejected the transaction: node rejected, reverted, fee below the node's floor at submission time. |

### Per-chain variant → category

These four tables are the specification. Each classifier is a `match` over its engine's error enum with **no catch-all for a known variant**. Where the enum is `#[non_exhaustive]` a wildcard arm is required by the compiler and maps to `Signing`; every currently-existing variant is pinned by a test in the owning task.

**EVM — `eth` and `polygon` (19 variants, `evm_wallet_core::Error`, `#[non_exhaustive]`)**

| Variant | Category | Exit |
|---|---|---:|
| `Rpc(_)` | Network | 2 |
| `GasEstimateFailed(_)` | Network | 2 |
| `ReceiptTimeout { .. }` | Network | 2 |
| `InvalidMnemonic(_)` | User | 1 |
| `InvalidPrivateKey(_)` | User | 1 |
| `InvalidInput(_)` | User | 1 |
| `AbiDecodeFailed { .. }` | User | 1 |
| `NonceMismatch { .. }` | User | 1 |
| `FeeTooLow { .. }` | User | 1 |
| `WalletNotFound { .. }` | User | 1 |
| `WalletNotFoundByName { .. }` | User | 1 |
| `WalletExists { .. }` | User | 1 |
| `InvalidPassword(_)` | User | 1 |
| `InsufficientFunds { .. }` | InsufficientFunds | 3 |
| `InsufficientTokenBalance { .. }` | InsufficientFunds | 3 |
| `WalletCorrupt { .. }` | Signing | 4 |
| `EncryptionFailed(_)` | Signing | 4 |
| `DecryptionFailed(_)` | Signing | 4 |
| `TxBroadcastFailed(_)` | Broadcast | 5 |
| *(wildcard — future variant)* | Signing | 4 |

**Solana — `sol` (29 variants, `sol_wallet_core::Error`, exhaustive — no wildcard)**

| Variant | Category | Exit |
|---|---|---:|
| `Transport(_)` | Network | 2 |
| `Rpc { .. }` | Network | 2 |
| `ConfirmTimeout { .. }` | Network | 2 |
| `ConfirmPending { .. }` | Network | 2 |
| `InvalidMnemonic` | User | 1 |
| `InvalidBase58Secret { .. }` | User | 1 |
| `InvalidAddress(_)` | User | 1 |
| `InvalidAmount(_)` | User | 1 |
| `InvalidTokenProgram(_)` | User | 1 |
| `InvalidTokenState(_)` | User | 1 |
| `InvalidWalletId { .. }` | User | 1 |
| `InvalidStorageName { .. }` | User | 1 |
| `InsecureSourceFile { .. }` | User | 1 |
| `ComputeBudgetExceeded { .. }` | User | 1 |
| `FileIo { .. }` | User | 1 |
| `WalletNotFound(_)` | User | 1 |
| `InsufficientFunds { .. }` | InsufficientFunds | 3 |
| `BroadcastFailed { .. }` | Broadcast | 5 |
| `DerivationFailed(_)` | Signing | 4 |
| `InvalidSeed` | Signing | 4 |
| `WalletDecryptFailed { .. }` | Signing | 4 |
| `UnsupportedBlobVersion { .. }` | Signing | 4 |
| `OsRngFailed { .. }` | Signing | 4 |
| `KdfFailed { .. }` | Signing | 4 |
| `CipherInit` | Signing | 4 |
| `RecordCorrupt { .. }` | Signing | 4 |
| `InvalidKdfParams { .. }` | Signing | 4 |
| `InvalidTransaction(_)` | Signing | 4 |
| `Unimplemented(_)` | Signing | 4 |

**TRON — `tron` (12 core variants + `CliError`, `tron_wallet_core::Error`, `#[non_exhaustive]`)**

| Variant | Category | Exit |
|---|---|---:|
| `Node(_)` | Network | 2 |
| `NodeResponse(_)` | Network | 2 |
| `Mnemonic(_)` | User | 1 |
| `Address(_)` | User | 1 |
| `Config(_)` | User | 1 |
| `SpkiPin(_)` | User | 1 |
| `Disambiguation(_)` | User | 1 |
| `Wallet(_)` | User | 1 |
| `Derivation(_)` | Signing | 4 |
| `Signing(_)` | Signing | 4 |
| `TransactionBuild(_)` | Signing | 4 |
| `Encryption(_)` | Signing | 4 |
| `CliError::Abort` | User | 1 |
| `CliError::BadInput(_)` | User | 1 |
| `CliError::Core(e)` | → recurse into the core table | — |
| *(wildcard — future variant)* | Signing | 4 |

**Bitcoin — `btc` (21 variants, `bitcoin_wallet_core::Error`, `#[non_exhaustive]`)**

| Variant | Category | Exit |
|---|---|---:|
| `Network(_)` | Network | 2 |
| `Esplora(_)` | Network | 2 |
| `Electrum(_)` | Network | 2 |
| `InvalidMnemonic(_)` | User | 1 |
| `InvalidDerivationPath(_)` | User | 1 |
| `Storage(_)` | User | 1 |
| `NotInitialized(_)` | User | 1 |
| `Bdk(_)` | User | 1 |
| `Io(_)` | User | 1 |
| `SpkiPin(_)` | User | 1 |
| `InsufficientFunds { .. }` | InsufficientFunds | 3 |
| `TxBuild(_)` | Signing | 4 |
| `Sign(_)` | Signing | 4 |
| `Psbt(_)` | Signing | 4 |
| `AddressDerivation(_)` | Signing | 4 |
| `ScriptBuild(_)` | Signing | 4 |
| `Encryption(_)` | Signing | 4 |
| `Bitcoin(_)` | Signing | 4 |
| `Bip137(_)` | Signing | 4 |
| `MnemonicCipher(_)` | Signing | 4 |
| `WalletStore(_)` | Signing | 4 |
| *(wildcard — future variant)* | Signing | 4 |

Three entries need their rationale, because they look wrong at a glance and a reviewer will ask:

- **`WalletStore` → `Signing` (4), not `User` (1).** `WalletStore` is the deliberately collapsed variant: not-found, wrong-password, wrong-network and corrupt-blob all produce it, and its `Display` is intentionally generic so the file-existence oracle stays closed (`bitcoin-wallet-core/src/error.rs:110-120`). Because not-found and wrong-password share the variant, they must share the code. Bitcoin's "wallet not found" therefore exits 4 while EVM's exits 1 — that divergence is the price of the oracle closure, not drift.
- **`Bdk` → `User` (1).** `Error::Bdk` is constructed for a wallet-descriptor parse failure with a sanitised message (`bitcoin-wallet-core/src/wallet/mod.rs:726`), i.e. a malformed descriptor the operator supplied — not a transport failure.
- **`FileIo` (Solana) → `User` (1).** A failed `atomic_write` is the operator's filesystem rejecting the operation (permissions, read-only mount, full disk), and `CONTEXT.md`'s `User` row already lists "missing file" as a User example. It is not a credential problem, so it is not `Signing`.

### What changes, and what it breaks

This is a **breaking change to the CLI exit-code contract**, in every chain. It is not a no-op refactor: the old codes were mutually inconsistent, so no assignment of six codes preserves all five CLIs' current behaviour. The taxonomy was chosen because it is substantively better — Solana currently reports insufficient funds as a transport error, and EVM reports "wallet not found" under the same code as "insufficient funds".

| Condition | Old (varies by chain) | New (all chains) |
|---|---|---|
| success | 0 | 0 |
| clap usage error (bad flag, missing argument) | 2 | 1 |
| invalid input / bad mnemonic | 1 or 2 | 1 |
| wallet not found | 4 or 1 | 1 |
| empty passphrase | 2 | 1 on create/import; 4 where the passphrase opens an existing wallet |
| wrong passphrase | 2 or 5 | 4 |
| corrupt wallet file | 5 | 4 |
| insufficient funds | 4 or 3 | 3 |
| node unreachable | 3 or 2 | 2 |
| node rejected the transaction | 5 or 3 | 5 |
| signing failure | 5 | 4 |

The Flutter desktop host does **not** parse CLI exit codes (the only `exitCode` read in `wallet-desktop/` is a `chmod` result in `wallet-desktop/lib/core/secrets/temp_secret_file.dart:84`), so no host change is required. Every `CHANGELOG.md` bullet in this phase carries a `**BREAKING**:` prefix.

---

## File structure

```
rust-wallet-app/crates/
├── wallet-errors/                    NEW  zero-dependency shared crate
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs                    ErrorCategory, exit_code(), as_str()
│   └── tests/
│       └── context_doc_parity.rs     CONTEXT.md table == the code table
├── eth/src/error.rs                  NEW  classify(&eth_wallet_core::Error)
├── eth/src/main.rs                   MOD  drop Error::exit_code() call
├── eth/src/handlers.rs               MOD  doc comment referencing the old table
├── evm-wallet-core/src/error.rs      MOD  DELETE Error::exit_code()
├── polygon/src/error.rs               NEW  classify(&polygon_wallet_core::Error)
├── polygon/src/main.rs                MOD  map errors instead of exit 1
├── btc/src/error.rs                   NEW  classify(&bitcoin_wallet_core::Error)
├── btc/src/main.rs                    MOD  replace the InvalidMnemonic special case
├── sol/src/handlers/error.rs          MOD  classify returns ErrorCategory
├── sol/src/main.rs                    MOD  call the shared table
├── tron/src/handlers/mod.rs           MOD  exit_code() -> classify()
├── tron/src/main.rs                   MOD  call the shared table
└── <chain>/Cargo.toml                 MOD  + wallet-errors = { workspace = true }

rust-wallet-app/Cargo.toml             MOD  members + [workspace.dependencies]
docs/wallets/CONTEXT.md                MOD  the tested spec + corrected flag table
CHANGELOG.md                           MOD  L24 [Unreleased] bullets (one per sub-task)
.github/workflows/workspace.yml        MOD  error-mapping parity job
```

---

## Tasks

### Task 1: `wallet-errors` crate — the category enum and the exit-code table

**Files:**
- Create: `rust-wallet-app/crates/wallet-errors/Cargo.toml`
- Create: `rust-wallet-app/crates/wallet-errors/src/lib.rs`
- Create: `rust-wallet-app/crates/wallet-errors/tests/context_doc_parity.rs`
- Modify: `rust-wallet-app/Cargo.toml` (workspace `members` + `[workspace.dependencies]`)

**Interfaces:**
- Consumes: nothing. This task is the root of the phase.
- Produces:
  - `wallet_errors::ErrorCategory` — `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum ErrorCategory { Success, User, Network, InsufficientFunds, Signing, Broadcast }`
  - `wallet_errors::ErrorCategory::ALL` — `pub const ALL: [ErrorCategory; 6]`
  - `wallet_errors::ErrorCategory::exit_code(self) -> i32`
  - `wallet_errors::ErrorCategory::as_str(self) -> &'static str` — returns the exact label used in `docs/wallets/CONTEXT.md`: `"Success"`, `"User error"`, `"Network error"`, `"Insufficient funds"`, `"Signing error"`, `"Broadcast error"`.

- [ ] **Step 1: Create the crate skeleton and the failing parity test**

`rust-wallet-app/crates/wallet-errors/Cargo.toml`:

```toml
[package]
name = "wallet-errors"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
# Intentionally empty. The category enum and the exit-code table are the
# whole crate; a dependency here would put a supply-chain surface into the
# one crate every CLI depends on.
```

`rust-wallet-app/crates/wallet-errors/tests/context_doc_parity.rs`:

```rust
//! The documented table in `docs/wallets/CONTEXT.md` IS the specification.
//!
//! This test parses the "Common Error Categories (cross-chain)" table out of
//! the document and asserts it matches `wallet_errors::ErrorCategory`
//! exactly — same rows, same order, same labels, same exit codes.
//!
//! The document lives at `<repo>/docs/wallets/CONTEXT.md`; from this crate's
//! manifest directory that is three levels up.

use std::path::PathBuf;

use wallet_errors::ErrorCategory;

fn context_md_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/wallets/CONTEXT.md")
}

fn documented_rows() -> Vec<(String, i32)> {
    let text = std::fs::read_to_string(context_md_path()).expect("read CONTEXT.md");
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("| Category | Exit code") {
            in_table = true;
            continue;
        }
        if in_table {
            if line.starts_with('|') {
                let mut cells = line.split('|').map(str::trim);
                cells.next();
                let Some(label) = cells.next() else { continue };
                let Some(code) = cells.next() else { continue };
                if label.is_empty() || !code.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                rows.push((label.to_string(), code.parse().expect("numeric exit code")));
            } else if !line.is_empty() {
                in_table = false;
            }
        }
    }
    rows
}

#[test]
fn context_md_table_matches_the_code_table() {
    let documented = documented_rows();
    assert_eq!(
        documented.len(),
        ErrorCategory::ALL.len(),
        "CONTEXT.md must document exactly one row per ErrorCategory"
    );
    for (category, (label, code)) in ErrorCategory::ALL.iter().zip(documented.iter()) {
        assert_eq!(category.as_str(), label, "category label drift");
        assert_eq!(category.exit_code(), *code, "exit code drift for {label}");
    }
}

#[test]
fn exit_codes_are_the_documented_six() {
    assert_eq!(ErrorCategory::Success.exit_code(), 0);
    assert_eq!(ErrorCategory::User.exit_code(), 1);
    assert_eq!(ErrorCategory::Network.exit_code(), 2);
    assert_eq!(ErrorCategory::InsufficientFunds.exit_code(), 3);
    assert_eq!(ErrorCategory::Signing.exit_code(), 4);
    assert_eq!(ErrorCategory::Broadcast.exit_code(), 5);
}
```

- [ ] **Step 2: Register the crate and run the test to verify it fails**

Add to `rust-wallet-app/Cargo.toml`:

```toml
members = [
    "crates/wallet-errors",
    "crates/chain-traits",
    ...
]
```

and to `[workspace.dependencies]`:

```toml
wallet-errors = { path = "crates/wallet-errors" }
```

Run: `cargo test -p wallet-errors`
Expected: FAIL — `error[E0432]: unresolved import wallet_errors` / the crate has no `src/lib.rs` yet.

- [ ] **Step 3: Implement `src/lib.rs`**

```rust
//! Shared error categories and the one exit-code table for all five CLIs.
//!
//! One category enum, one `ErrorCategory -> i32` table, one source of truth.
//! The per-chain error enums stay where they are — their granularity is a
//! security feature — and each CLI maps its own variants onto these six
//! categories in its own layer.
//!
//! The exit codes are the ones documented in `docs/wallets/CONTEXT.md`; the
//! `context_doc_parity` test asserts the document and this table agree.

/// The six cross-chain failure categories.
///
/// The set is fixed to the six documented in `docs/wallets/CONTEXT.md` — six,
/// not eight. `Storage` and `Crypto` are deliberately *not* categories:
/// filesystem and permission failures are `User`, and credential / crypto /
/// internal-machinery failures are `Signing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// The operation completed.
    Success,
    /// Operator-supplied input or environment: malformed input, missing
    /// wallet, bad config, filesystem or permission failure, empty
    /// passphrase, declined confirmation.
    User,
    /// The node or transport failed to answer, timed out, or answered
    /// unintelligibly.
    Network,
    /// The balance will not cover the amount plus the fee.
    InsufficientFunds,
    /// Key material is unusable or the operation consuming it failed:
    /// wrong passphrase, decryption failure, corrupt blob, derivation,
    /// transaction construction, signature failure, internal machinery.
    Signing,
    /// The network accepted the request and rejected the transaction.
    Broadcast,
}

impl ErrorCategory {
    /// Every category, in exit-code order. Used by the doc-parity test.
    pub const ALL: [ErrorCategory; 6] = [
        ErrorCategory::Success,
        ErrorCategory::User,
        ErrorCategory::Network,
        ErrorCategory::InsufficientFunds,
        ErrorCategory::Signing,
        ErrorCategory::Broadcast,
    ];

    /// The process exit code for this category.
    ///
    /// This is the only exit-code table in the workspace. Every CLI calls
    /// it; no CLI keeps a second copy.
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorCategory::Success => 0,
            ErrorCategory::User => 1,
            ErrorCategory::Network => 2,
            ErrorCategory::InsufficientFunds => 3,
            ErrorCategory::Signing => 4,
            ErrorCategory::Broadcast => 5,
        }
    }

    /// The label used in the `docs/wallets/CONTEXT.md` category table.
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCategory::Success => "Success",
            ErrorCategory::User => "User error",
            ErrorCategory::Network => "Network error",
            ErrorCategory::InsufficientFunds => "Insufficient funds",
            ErrorCategory::Signing => "Signing error",
            ErrorCategory::Broadcast => "Broadcast error",
        }
    }
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p wallet-errors`
Expected: PASS — 2 passed, 0 failed. The doc-parity test passes on the **current** `CONTEXT.md` table, because that table already states the target codes; what was wrong was the code, not the document.

- [ ] **Step 5: L12 review pair**

Run `superpowers:requesting-code-review` on the squash-candidate state with two parallel sub-agents (`pr-review-toolkit:type-design-analyzer` for the enum's invariants, `pr-review-toolkit:code-reviewer` for correctness and test quality). Then run the review-paired fmt re-check (`cargo fmt -p wallet-errors -- --check`). Then `pr-review-toolkit:pr-test-analyzer` for coverage gaps. Fix every confirmed finding in one pass (L13 convergent-finding rule) before the verify gate.

- [ ] **Step 6: Verify gate (L13 step 11)**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p wallet-errors --all-targets -- -D warnings
cargo test -p wallet-errors
```

Expected: fmt clean, clippy clean, 2 tests pass. (L55: `-p` only.)

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` — one flat bullet per changed CLI, each immediately after the `## [Unreleased]` heading, with no `###` heading and no User Stories row (see the Global Constraints for the full L24 doc-set rule):

```
- **`[internal]`** `rust-wallet-app/crates/wallet-errors/` — new shared crate owning the cross-chain `ErrorCategory` enum and the single `ErrorCategory -> i32` exit-code table (`Success=0, User=1, Network=2, InsufficientFunds=3, Signing=4, Broadcast=5`). Zero dependencies. The table is asserted against `docs/wallets/CONTEXT.md` by `wallet-errors/tests/context_doc_parity.rs`. Refs the design spec `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1 and ADR 0003.
```

No User Stories row flips — this task adds no user-facing capability on its own.

- [ ] **Step 8: PAUSE — request approval**

Present the diff (`git diff --stat` plus the full diff of the four files) and the verify-gate output. **Do not commit until the maintainer approves.** House never-auto-commit rule and L13 step 12.

- [ ] **Step 9: Commit, push, open the PR to the parent branch**

```bash
git add rust-wallet-app/Cargo.toml rust-wallet-app/crates/wallet-errors CHANGELOG.md
git commit -m "feat(errors): add wallet-errors crate with the shared category and exit-code table

Co-Authored-By: Claude Code <noreply@anthropic.com>"
git push -u origin task/581a-wallet-errors-crate
gh pr create --base task/581-wallet-errors --head task/581a-wallet-errors-crate \
  --title "feat(errors): add wallet-errors crate with the shared category and exit-code table" \
  --body-file /tmp/pr-body-1a.md
```

The PR body carries: Context, the six-category table, the verification output, and a **Drift** section (L13 step 4a) noting that the new crate has no dependents yet — Tasks 2–6 add them.

Merge into the parent when the PR is approved:

```bash
git checkout task/581-wallet-errors
git merge task/581a-wallet-errors-crate --no-ff -m "Merge PR #<N> (task/581a-wallet-errors-crate): wallet-errors crate"
```

---

### Task 2: Solana CLI migration

**Files:**
- Modify: `rust-wallet-app/crates/sol/src/handlers/error.rs`
- Modify: `rust-wallet-app/crates/sol/src/main.rs`
- Modify: `rust-wallet-app/crates/sol/Cargo.toml`
- Modify: `rust-wallet-app/crates/sol/tests/cli_integration_surfpool.rs` (the two exit-code assertions)

**Interfaces:**
- Consumes: `wallet_errors::ErrorCategory` and `ErrorCategory::exit_code()` from Task 1.
- Produces: `sol::handlers::error::classify(err: &anyhow::Error) -> ErrorCategory` and `classify_core(err: &sol_wallet_core::Error) -> ErrorCategory` (the signature changes from `-> i32`). `sol` has no `lib.rs`, so this is crate-internal — nothing outside the `sol` binary consumes it.

- [ ] **Step 1: Write the failing test**

In `rust-wallet-app/crates/sol/src/handlers/error.rs`, keep the existing `classify` body for now and add a table-driven test module at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sol_wallet_core::Error as CoreError;
    use wallet_errors::ErrorCategory;

    #[test]
    fn every_solana_variant_maps_to_its_documented_category() {
        let cases: Vec<(CoreError, ErrorCategory)> = vec![
            // Network.
            (CoreError::Transport("down".into()), ErrorCategory::Network),
            (CoreError::Rpc { code: -32000, message: "x".into() }, ErrorCategory::Network),
            (CoreError::ConfirmTimeout { signature: "s".into(), waited_ms: 1 }, ErrorCategory::Network),
            (CoreError::ConfirmPending { signature: "s".into(), commitment: solana_commitment_config::CommitmentConfig::processed(), elapsed_ms: 1 }, ErrorCategory::Network),
            // User.
            (CoreError::InvalidMnemonic, ErrorCategory::User),
            (CoreError::InvalidBase58Secret { got: 1, expected: 64 }, ErrorCategory::User),
            (CoreError::InvalidAddress("x".into()), ErrorCategory::User),
            (CoreError::InvalidAmount("x".into()), ErrorCategory::User),
            (CoreError::InvalidTokenProgram("x".into()), ErrorCategory::User),
            (CoreError::InvalidTokenState("x".into()), ErrorCategory::User),
            (CoreError::InvalidWalletId { input: "x".into() }, ErrorCategory::User),
            (CoreError::InvalidStorageName { name: "x".into() }, ErrorCategory::User),
            (CoreError::InsecureSourceFile { path: "/tmp/x".into(), mode: 0o644 }, ErrorCategory::User),
            (CoreError::ComputeBudgetExceeded { needed_cu: 2, available_cu: 1 }, ErrorCategory::User),
            (CoreError::FileIo { path: "/tmp/x".into(), source: std::io::Error::other("x") }, ErrorCategory::User),
            (CoreError::WalletNotFound(test_wallet_id()), ErrorCategory::User),
            // Insufficient funds.
            (CoreError::InsufficientFunds { needed: 2, have: 1 }, ErrorCategory::InsufficientFunds),
            // Broadcast.
            (CoreError::BroadcastFailed { kind: "k".into(), context: "c".into() }, ErrorCategory::Broadcast),
            // Signing.
            (CoreError::DerivationFailed("x".into()), ErrorCategory::Signing),
            (CoreError::InvalidSeed, ErrorCategory::Signing),
            (CoreError::WalletDecryptFailed { id: test_wallet_id() }, ErrorCategory::Signing),
            (CoreError::UnsupportedBlobVersion { found: 9, lo: 1, hi: 2 }, ErrorCategory::Signing),
            (CoreError::OsRngFailed { source: getrandom::Error::UNSUPPORTED }, ErrorCategory::Signing),
            (CoreError::KdfFailed { message: "x".into() }, ErrorCategory::Signing),
            (CoreError::CipherInit, ErrorCategory::Signing),
            (CoreError::RecordCorrupt { context: "ctx", message: "msg".into() }, ErrorCategory::Signing),
            (CoreError::InvalidKdfParams { memory_kb: 8, iterations: 1, parallelism: 1 }, ErrorCategory::Signing),
            (CoreError::InvalidTransaction("x".into()), ErrorCategory::Signing),
            (CoreError::Unimplemented("x"), ErrorCategory::Signing),
        ];
        assert_eq!(cases.len(), 29, "this table pins 29 Solana variants — a new variant must add a row here");
        for (err, expected) in cases {
            assert_eq!(classify_core(&err), expected, "wrong category for {err:?}");
        }
    }

    #[test]
    fn unclassified_anyhow_error_is_a_user_error() {
        let err = anyhow::anyhow!("something the CLI rejected before the core");
        assert_eq!(classify(&err), ErrorCategory::User);
    }

    #[test]
    fn insufficient_funds_is_not_reported_as_a_transport_error() {
        // Review Focus #4: Solana today maps InsufficientFunds to the same
        // code as Transport. The two must be distinguishable.
        let err = anyhow::Error::new(CoreError::InsufficientFunds { needed: 2, have: 1 });
        assert_eq!(classify(&err), ErrorCategory::InsufficientFunds);
        assert_eq!(classify(&err).exit_code(), 3);
        assert_ne!(
            classify(&err).exit_code(),
            classify(&anyhow::Error::new(CoreError::Transport("down".into()))).exit_code()
        );
    }

    #[test]
    fn wrong_password_and_corrupt_blob_are_indistinguishable() {
        // Review Focus #1: a wrong passphrase and a corrupt blob must land
        // on the same category, so the two cannot be told apart by exit code.
        let wrong_pw = classify_core(&CoreError::WalletDecryptFailed { id: test_wallet_id() });
        let corrupt = classify_core(&CoreError::RecordCorrupt {
            context: "ctx",
            message: "msg".into(),
        });
        assert_eq!(wrong_pw, ErrorCategory::Signing);
        assert_eq!(wrong_pw, corrupt);
        assert_eq!(wrong_pw.exit_code(), 4);
    }

    #[test]
    fn a_rejected_transaction_is_distinguishable_from_an_unreachable_node() {
        // Review Focus #5: automation must not retry a transaction the
        // cluster will never accept.
        let rejected = classify_core(&CoreError::BroadcastFailed {
            kind: "k".into(),
            context: "c".into(),
        });
        let unreachable = classify_core(&CoreError::Transport("down".into()));
        assert_eq!(rejected, ErrorCategory::Broadcast);
        assert_eq!(rejected.exit_code(), 5);
        assert_ne!(rejected.exit_code(), unreachable.exit_code());
    }
}
```

Add this helper above the test module (the `WalletId` inner field is private, so the id is built through its public parser):

```rust
    fn test_wallet_id() -> sol_wallet_core::WalletId {
        sol_wallet_core::WalletId::parse_str("00000000-0000-0000-0000-000000000000")
            .expect("a valid nil UUID parses")
    }
```

`OsRngFailed { source: getrandom::Error }` needs `getrandom` in the `sol` crate's dependency set. Add it under `[dev-dependencies]` in `rust-wallet-app/crates/sol/Cargo.toml`, pinned to the same version `sol-wallet-core` uses:

```toml
getrandom = "=0.2.15"
```

`getrandom::Error` is a newtype over a `NonZeroU32` with public associated constants, so `getrandom::Error::UNSUPPORTED` is constructible without a real OS failure.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p sol handlers::error`
Expected: FAIL — `classify_core` does not exist, and `classify` returns `i32` so `assert_eq!(classify_core(&err), expected)` does not compile.

- [ ] **Step 3: Implement the classifier**

Replace the body of `rust-wallet-app/crates/sol/src/handlers/error.rs` with:

```rust
//! Error classifier — maps `sol_wallet_core::Error` variants to the shared
//! cross-chain categories; the shared table maps categories to exit codes.
//!
//! The variant→category mapping is Solana's own: chain-specific granularity
//! is a security feature and is deliberately not unified across chains. What
//! IS unified is the exit code — `wallet_errors::ErrorCategory::exit_code()`
//! is the only exit-code table in the workspace.
//!
//! Exit codes (from `wallet-errors`):
//!   0 — success
//!   1 — user error      (invalid input, missing file, empty passphrase, ...)
//!   2 — network error   (transport, RPC, timeout, malformed response)
//!   3 — insufficient funds
//!   4 — signing error   (key material, credential, crypto, internal)
//!   5 — broadcast error (the cluster rejected the transaction)

use sol_wallet_core::Error;
use wallet_errors::ErrorCategory;

/// Classify a Solana core error into its cross-chain category.
pub fn classify_core(err: &Error) -> ErrorCategory {
    match err {
        // Network — the cluster did not answer usefully.
        Error::Transport(_)
        | Error::Rpc { .. }
        | Error::ConfirmTimeout { .. }
        | Error::ConfirmPending { .. } => ErrorCategory::Network,

        // User — the operator supplied something unusable.
        Error::InvalidMnemonic
        | Error::InvalidBase58Secret { .. }
        | Error::InvalidAddress(_)
        | Error::InvalidAmount(_)
        | Error::InvalidTokenProgram(_)
        | Error::InvalidTokenState(_)
        | Error::InvalidWalletId { .. }
        | Error::InvalidStorageName { .. }
        | Error::InsecureSourceFile { .. }
        | Error::ComputeBudgetExceeded { .. }
        | Error::FileIo { .. }
        | Error::WalletNotFound(_) => ErrorCategory::User,

        // Insufficient funds — the balance will not cover amount + fee.
        Error::InsufficientFunds { .. } => ErrorCategory::InsufficientFunds,

        // Broadcast — the cluster accepted the request and rejected the tx.
        Error::BroadcastFailed { .. } => ErrorCategory::Broadcast,

        // Signing — key material, credentials, crypto, internal machinery.
        Error::DerivationFailed(_)
        | Error::InvalidSeed
        | Error::WalletDecryptFailed { .. }
        | Error::UnsupportedBlobVersion { .. }
        | Error::OsRngFailed { .. }
        | Error::KdfFailed { .. }
        | Error::CipherInit
        | Error::RecordCorrupt { .. }
        | Error::InvalidKdfParams { .. }
        | Error::InvalidTransaction(_)
        | Error::Unimplemented(_) => ErrorCategory::Signing,
    }
}

/// Classify an anyhow error chain into its cross-chain category.
///
/// Walks the chain for a `sol_wallet_core::Error`; an error that never
/// reached the core (a CLI-level rejection) is a user error.
pub fn classify(err: &anyhow::Error) -> ErrorCategory {
    for cause in err.chain() {
        if let Some(lib_err) = cause.downcast_ref::<Error>() {
            return classify_core(lib_err);
        }
    }
    ErrorCategory::User
}
```

`sol_wallet_core::Error` is **not** `#[non_exhaustive]`, so this match is exhaustive — adding a variant to the Solana error enum is a compile error here until it is classified. That is the intended discipline.

Update `rust-wallet-app/crates/sol/src/main.rs:45`:

```rust
let exit_code = handlers::error::classify(&e).exit_code();
```

Add to `rust-wallet-app/crates/sol/Cargo.toml` under `[dependencies]`:

```toml
wallet-errors = { workspace = true }
```

Update the two exit-code assertions in `rust-wallet-app/crates/sol/tests/cli_integration_surfpool.rs`:
- `classify_exit_code_3_tx_wait_transport_audit_p7_3` asserted exit 3 for a transport failure — a transport failure is now exit **2** (Network). Rename to `classify_exit_code_2_tx_wait_transport_audit_p7_3` and change the expected code to 2.
- `classify_exit_code_5_wallet_create_file_io_audit_p7_3` asserted exit 5 for a `FileIo` failure — `FileIo` is now exit **1** (User). Rename to `classify_exit_code_1_wallet_create_file_io_audit_p7_3` and change the expected code to 1.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p sol`
Expected: PASS — the three new tests pass and the 53 existing CLI tests still pass. The two renamed surfpool tests are env-gated (`RUN_SOL_SURFPOOL=1`) and skip without it.

- [ ] **Step 5: L12 review pair + fmt re-check + test-coverage gap analysis**

As Task 1 Step 5. The reviewer must specifically check that `FileIo` → `User` and `ComputeBudgetExceeded` → `User` are deliberate and documented, and that no arm was left implicit.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p sol --all-targets -- -D warnings
cargo test -p sol
```

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` — one flat bullet per changed CLI, each immediately after the `## [Unreleased]` heading, with no `###` heading and no User Stories row (see the Global Constraints for the full L24 doc-set rule):

```
- **`[user-facing]`** `rust-wallet-app/crates/sol/` — **BREAKING:** the `sol` CLI now uses the shared cross-chain exit-code table. Exit codes change: clap usage errors (bad flag, missing argument) 2→1; invalid mnemonic, address, amount, token program, token state and base58 secret 2→1; missing wallet, unknown wallet id, unsafe source file and invalid storage name 4→1; file-write failures 5→1; compute-budget exceeded 3→1; derivation and unusable-seed failures 2→4; transport, RPC and confirmation timeout 3→2; insufficient funds 3→3 (previously reported as a transport error, now distinct); cluster rejection of a broadcast 3→5; wallet decryption, unsupported blob version, invalid KDF parameter and transaction-construction failures 4→4 (unchanged); OS RNG failure, KDF failure, cipher init, corrupt record and not-yet-implemented failures 5→4. Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/581b-sol-migration` off `task/581-wallet-errors`. Same commit/PAUSE/PR/`--no-ff`-merge shape as Task 1 Step 9.

---

### Task 3: TRON CLI migration

**Files:**
- Modify: `rust-wallet-app/crates/tron/src/handlers/mod.rs`
- Modify: `rust-wallet-app/crates/tron/src/main.rs`
- Modify: `rust-wallet-app/crates/tron/src/handlers/tx.rs` (classifier + a stale exit-code comment)
- Modify: `rust-wallet-app/crates/tron/src/handlers/wallet.rs` (classifier + a stale exit-code comment)
- Modify: `rust-wallet-app/crates/tron/src/cli.rs` (stale exit-code comment only)
- Modify: `rust-wallet-app/crates/tron/tests/cli.rs` (stale exit-code comment only)
- Modify: `rust-wallet-app/crates/tron/Cargo.toml`
- Modify: `rust-wallet-app/crates/tron-wallet-core/src/tx/submit.rs` (comment-only: a reference to the removed `handlers::exit_code` and the old exit 2; no behaviour change)

**Interfaces:**
- Consumes: `wallet_errors::ErrorCategory` from Task 1.
- Produces: `classify(err: &CliError) -> ErrorCategory` and `classify_core(err: &CoreError) -> ErrorCategory` in `tron/src/handlers/mod.rs`. **Note:** `tron/src/lib.rs` exposes only `pub mod cli;` — `handlers` is a private module of the **binary** target, so neither function is reachable from the library target or from integration tests. The classifier is exercised by the in-crate unit test in `handlers/mod.rs`.

- [ ] **Step 1: Write the failing test**

In `rust-wallet-app/crates/tron/src/handlers/mod.rs`, replace the existing `exit_codes_match_the_plan_table` test (around line 526) with:

```rust
    #[test]
    fn every_tron_variant_maps_to_its_documented_category() {
        use wallet_errors::ErrorCategory;

        let cases: Vec<(CoreError, ErrorCategory)> = vec![
            (CoreError::Node("down".into()), ErrorCategory::Network),
            (CoreError::NodeResponse("bad".into()), ErrorCategory::Network),
            (CoreError::Mnemonic("bad".into()), ErrorCategory::User),
            (CoreError::Address("bad".into()), ErrorCategory::User),
            (CoreError::Config("bad".into()), ErrorCategory::User),
            (CoreError::SpkiPin("bad".into()), ErrorCategory::User),
            (CoreError::Disambiguation("bad".into()), ErrorCategory::User),
            (CoreError::Wallet("missing".into()), ErrorCategory::User),
            (CoreError::Derivation("bad".into()), ErrorCategory::Signing),
            (CoreError::Signing("bad".into()), ErrorCategory::Signing),
            (CoreError::TransactionBuild("bad".into()), ErrorCategory::Signing),
            (CoreError::Encryption("bad pw".into()), ErrorCategory::Signing),
        ];
        assert_eq!(cases.len(), 12, "this table pins 12 TRON variants — the enum is #[non_exhaustive], so a new variant must add a row here");
        for (err, expected) in cases {
            assert_eq!(classify(&CliError::Core(err)), expected, "wrong category");
        }

        assert_eq!(classify(&CliError::Abort), ErrorCategory::User);
        assert_eq!(
            classify(&CliError::BadInput("x".into())),
            ErrorCategory::User
        );
    }

    #[test]
    fn wrong_password_and_corrupt_blob_are_indistinguishable() {
        use wallet_errors::ErrorCategory;
        // Review Focus #1: a wrong passphrase and a corrupt blob must land
        // on the same category, so the two cannot be told apart by exit code.
        //
        // What this pins: TRON's engine produces the *same* `Error::Encryption`
        // variant for a GCM-tag mismatch (wrong passphrase) and for a
        // truncated blob (`tron-wallet-core/src/crypto/mod.rs`), so the two
        // conditions are indistinguishable **by construction** and this test
        // pins the exit code they share. It does not — and cannot — compare
        // two distinct errors, because the engine never produces two.
        let wrong_pw = classify(&CliError::Core(CoreError::Encryption("bad pw".into())));
        assert_eq!(wrong_pw, ErrorCategory::Signing);
        assert_eq!(wrong_pw.exit_code(), 4);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p tron handlers`
Expected: FAIL — `classify` does not exist (only `exit_code` does).

- [ ] **Step 3: Implement the classifier**

In `rust-wallet-app/crates/tron/src/handlers/mod.rs`, replace `pub fn exit_code(err: &CliError) -> i32` with:

```rust
/// Maps an error to its cross-chain category.
///
/// The category set and the exit-code table live in `wallet-errors`; this
/// function owns only the TRON-specific variant→category mapping.
pub fn classify(err: &CliError) -> wallet_errors::ErrorCategory {
    use wallet_errors::ErrorCategory;
    match err {
        // Operator declined a confirmation prompt.
        CliError::Abort => ErrorCategory::User,
        // Argument shape the CLI rejects before reaching the core.
        CliError::BadInput(_) => ErrorCategory::User,
        CliError::Core(e) => classify_core(e),
    }
}

/// Maps a `tron-wallet-core` error to its cross-chain category.
pub fn classify_core(e: &CoreError) -> wallet_errors::ErrorCategory {
    use wallet_errors::ErrorCategory;
    match e {
        // Transport / node reachability.
        CoreError::Node(_) | CoreError::NodeResponse(_) => ErrorCategory::Network,
        // Operator input: bad mnemonic, bad address, bad config, bad pin,
        // cross-network guard, wallet lookup.
        CoreError::Mnemonic(_)
        | CoreError::Address(_)
        | CoreError::Config(_)
        | CoreError::SpkiPin(_)
        | CoreError::Disambiguation(_)
        | CoreError::Wallet(_) => ErrorCategory::User,
        // Signing, envelope construction, at-rest encryption.
        CoreError::Derivation(_)
        | CoreError::Signing(_)
        | CoreError::TransactionBuild(_)
        | CoreError::Encryption(_) => ErrorCategory::Signing,
        // Future variants are treated as internal failures, never as
        // operator input, so automation does not loop on a bad command line.
        _ => ErrorCategory::Signing,
    }
}
```

`tron_wallet_core::Error` is `#[non_exhaustive]` (`tron-wallet-core/src/error.rs:10`), so the wildcard arm is required by the compiler.

In the module doc at the top of `rust-wallet-app/crates/tron/src/handlers/mod.rs` (the shared exit-code table), add a clause after the table — the table itself is the shared one and is accurate, but rows 3 and 5 are **unreachable from this CLI**, and a reader of this module would otherwise assume the CLI can produce them:

```
//! Rows 3 and 5 are **unreachable from this CLI**: `tron-wallet-core`'s
//! `Error` has no insufficient-funds and no broadcast-rejection variant, so
//! a node-side transaction rejection is reported as `Network` (2), the same
//! code as an unreachable node.
```

The same fact is stated for operators in `docs/wallets/CONTEXT.md` by Task 7.

Update `rust-wallet-app/crates/tron/src/main.rs`:
- line 16: `use handlers::{classify, Result};`
- line 27: `std::process::exit(5);` (runtime construction failure) → `std::process::exit(wallet_errors::ErrorCategory::Signing.exit_code());` — an internal-machinery failure.
- line 34: `std::process::exit(exit_code(&err));` → `std::process::exit(classify(&err).exit_code());`

Update the two existing assertions:
- `rust-wallet-app/crates/tron/src/handlers/tx.rs:146` — `assert_eq!(super::super::exit_code(&err), 2);` → `assert_eq!(super::super::classify(&err), wallet_errors::ErrorCategory::User);` (the `Error::Config` case is a user error).
- `rust-wallet-app/crates/tron/src/handlers/wallet.rs:744` — `assert_eq!(super::super::exit_code(&err), 3);` → `assert_eq!(super::super::classify(&err), wallet_errors::ErrorCategory::Network);` (the node case).

Add to `rust-wallet-app/crates/tron/Cargo.toml`:

```toml
wallet-errors = { workspace = true }
```

1. In `rust-wallet-app/crates/tron/src/main.rs`, replace the plain `Cli::parse()` with one that maps a clap usage error to `User`:

```rust
let cli = match Cli::try_parse() {
    Ok(cli) => cli,
    Err(e) => {
        // A clap usage error is a user error (exit 1), not a network error
        // (exit 2). `--help` and `--version` keep clap's exit 0.
        let code = e.exit_code();
        // `print()` returns `io::Result<()>` and `Result` is `#[must_use]` —
        // the write result is dropped deliberately: the process exits either way.
        let _ = e.print();
        std::process::exit(if code == 0 {
            0
        } else {
            wallet_errors::ErrorCategory::User.exit_code()
        });
    }
};
```

**The TRON test suite contains no exit-code assertions, so nothing in CI will fail if this step is skipped — yet the CHANGELOG bullet below claims "clap usage errors 2→1". Skipping it makes that claim false.**

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p tron`
Expected: PASS — the two new tests pass and the 149 existing TRON tests still pass.

- [ ] **Step 5: L12 review pair + fmt re-check + test-coverage gap analysis.** As Task 1 Step 5.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p tron --all-targets -- -D warnings
cargo test -p tron
```

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` — one flat bullet per changed CLI, each immediately after the `## [Unreleased]` heading, with no `###` heading and no User Stories row (see the Global Constraints for the full L24 doc-set rule):

```
- **`[user-facing]`** `rust-wallet-app/crates/tron/` — **BREAKING:** the `tron` CLI now uses the shared cross-chain exit-code table. Exit codes change: clap usage errors (bad flag, missing argument) 2→1; declined confirmation 1→1 (unchanged); bad mnemonic, address, config, SPKI pin and cross-network guard 2→1; argument shapes the CLI rejects before reaching the core 2→1; wallet lookup 4→1; node unreachable 3→2; malformed node response 5→2; key derivation 2→4; signing and transaction construction 5→4; wrong passphrase and at-rest encryption failures 4→4 (unchanged). Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/581c-tron-migration`.

---

### Task 4: Ethereum CLI migration + delete `evm_wallet_core::Error::exit_code()`

**Files:**
- Create: `rust-wallet-app/crates/eth/src/error.rs`
- Modify: `rust-wallet-app/crates/eth/src/main.rs` (the exit path, the clap path, and the stale exit-code comments at `:19`, `:166`, `:219`, `:224`, `:379`, `:413`, `:773`)
- Modify: `rust-wallet-app/crates/eth/src/handlers.rs` (doc comments only — `:5`, `:59-67`, `:93`, `:313`, `:349-351`, `:568`, `:648`, `:768-786`, `:1041`, `:1049`)
- Modify: `rust-wallet-app/crates/evm-wallet-core/src/error.rs` (delete `exit_code()` + its doc block; delete the five exit-code test functions and the three `exit_code()` lines in the sixth; correct the doc comments at `:5-6`, `:24`, `:118`, `:218`)
- Modify: `rust-wallet-app/crates/eth/Cargo.toml`
- Modify: `rust-wallet-app/crates/eth/tests/cli_localnet.rs` (24 non-zero `assert_eq!` assertions, 5 `assert_ne!` guards, and 66 stale exit-code references)
- Modify: `rust-wallet-app/crates/eth/tests/wallet_send_speedup.rs` (2 non-zero `assert_eq!` assertions, 10 stale exit-code references — both assertions sit inside `#[ignore]`d operator-driven tests)
- Modify: `rust-wallet-app/crates/polygon/src/handlers/mod.rs` (doc comments only — `:53`, `:59` name the method this task deletes)
- Modify: `rust-wallet-app/crates/polygon/tests/local_testnet_smoke.rs` (doc comments only — `:2755`, `:2962`)
- Modify: `rust-wallet-app/Cargo.lock`
- **Unchanged:** `rust-wallet-app/crates/eth/tests/cli_sepolia.rs` — all six of its `.status.code()` call sites assert `Some(0)`, so it needs no edit. Do not touch it.
- **Unchanged:** `rust-wallet-app/crates/eth-wallet-core/` and `rust-wallet-app/crates/polygon-wallet-core/` — their `lib.rs` files re-export the `Error` type by glob and by name; they contain no `exit_code` reference of their own, so deleting the method needs no edit there.

**Interfaces:**
- Consumes: `wallet_errors::ErrorCategory` from Task 1.
- Produces: `eth::error::classify(err: &eth_wallet_core::Error) -> ErrorCategory` (crate-internal; `eth` has no `lib.rs`).
- Removes: `evm_wallet_core::Error::exit_code()` — and with it `eth_wallet_core::Error::exit_code()` (re-exported by `eth-wallet-core`) and `polygon_wallet_core::Error::exit_code()` (re-exported by `polygon-wallet-core`). **This is a breaking change to three library crates' public API.**

- [ ] **Step 1: Write the failing test**

Create `rust-wallet-app/crates/eth/src/error.rs` containing **only the test module** — the classifier is added in Step 3, so Step 2 is a genuine RED:

```rust
//! Error classifier for the `eth` CLI.
//!
//! The variant→category mapping is Ethereum-specific; the category→exit-code
//! mapping is the shared table in `wallet-errors`.

#[cfg(test)]
mod tests {
    use eth_wallet_core::Error;
    use wallet_errors::ErrorCategory;

    use super::classify;

    #[test]
    fn every_evm_variant_maps_to_its_documented_category() {
        let cases: Vec<(Error, ErrorCategory)> = vec![
            (Error::Rpc("down".into()), ErrorCategory::Network),
            (Error::GasEstimateFailed("x".into()), ErrorCategory::Network),
            (Error::ReceiptTimeout { secs: 1, tx_hash: "h".into() }, ErrorCategory::Network),
            (Error::InvalidMnemonic("x".into()), ErrorCategory::User),
            (Error::InvalidPrivateKey("x".into()), ErrorCategory::User),
            (Error::InvalidInput("x".into()), ErrorCategory::User),
            (Error::AbiDecodeFailed { context: "c".into(), reason: "r".into() }, ErrorCategory::User),
            (Error::NonceMismatch { submitted: 1, current: 2 }, ErrorCategory::User),
            (Error::FeeTooLow { max_fee_per_gas: 1, min_required: 2 }, ErrorCategory::User),
            (Error::WalletNotFound { wallet_id: uuid::Uuid::nil() }, ErrorCategory::User),
            (Error::WalletNotFoundByName { name: "n".into(), network: "sepolia".into() }, ErrorCategory::User),
            (Error::WalletExists { name: "n".into(), network: "sepolia".into() }, ErrorCategory::User),
            (Error::InvalidPassword("x".into()), ErrorCategory::User),
            (Error::InsufficientFunds { needed_wei: "2".into(), have_wei: "1".into() }, ErrorCategory::InsufficientFunds),
            (Error::InsufficientTokenBalance { token_symbol: "USDC".into(), needed_base_units: "2".into(), have_base_units: "1".into() }, ErrorCategory::InsufficientFunds),
            (Error::WalletCorrupt { path: "/tmp/w".into(), reason: "r".into() }, ErrorCategory::Signing),
            (Error::EncryptionFailed("x".into()), ErrorCategory::Signing),
            (Error::DecryptionFailed("x".into()), ErrorCategory::Signing),
            (Error::TxBroadcastFailed("x".into()), ErrorCategory::Broadcast),
        ];
        assert_eq!(cases.len(), 19, "this table pins 19 EVM variants — the enum is #[non_exhaustive], so a new variant must add a row here");
        for (err, expected) in cases {
            assert_eq!(classify(&err), expected, "wrong category for {err:?}");
        }
    }

    #[test]
    fn wrong_password_and_corrupt_blob_are_indistinguishable() {
        // Review Focus #1.
        let wrong_pw = classify(&Error::DecryptionFailed("bad pw".into()));
        let corrupt = classify(&Error::WalletCorrupt { path: "/tmp/w".into(), reason: "r".into() });
        assert_eq!(wrong_pw, corrupt);
        assert_eq!(wrong_pw.exit_code(), 4);
    }

    #[test]
    fn empty_passphrase_is_a_user_error_not_a_signing_error() {
        // Review Focus #2: an empty passphrase is operator input; a wrong
        // passphrase is a credential failure. They must not share a code.
        let empty = classify(&Error::InvalidPassword("password must be non-empty".into()));
        assert_eq!(empty, ErrorCategory::User);
        assert_eq!(empty.exit_code(), 1);
        assert_ne!(
            empty.exit_code(),
            classify(&Error::DecryptionFailed("bad pw".into())).exit_code()
        );
    }

    #[test]
    fn a_rejected_transaction_is_distinguishable_from_an_unreachable_node() {
        // Review Focus #5: automation must not retry a transaction the node
        // will never accept.
        let rejected = classify(&Error::TxBroadcastFailed("reverted".into()));
        let unreachable = classify(&Error::Rpc("down".into()));
        assert_eq!(rejected, ErrorCategory::Broadcast);
        assert_eq!(rejected.exit_code(), 5);
        assert_ne!(rejected.exit_code(), unreachable.exit_code());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p eth error`
Expected: FAIL to compile — `cannot find function classify in this scope`, because the classifier does not exist yet. This is the expected RED state.

- [ ] **Step 3: Implement the classifier and wire it up**

Add the classifier to `rust-wallet-app/crates/eth/src/error.rs`, above the test module:

```rust
use eth_wallet_core::Error;
use wallet_errors::ErrorCategory;

/// Maps an Ethereum core error to its cross-chain category.
pub fn classify(err: &Error) -> ErrorCategory {
    match err {
        // Network — the node did not answer usefully.
        Error::Rpc(_) | Error::GasEstimateFailed(_) | Error::ReceiptTimeout { .. } => {
            ErrorCategory::Network
        }
        // User — the operator supplied something unusable.
        Error::InvalidMnemonic(_)
        | Error::InvalidPrivateKey(_)
        | Error::InvalidInput(_)
        | Error::AbiDecodeFailed { .. }
        | Error::NonceMismatch { .. }
        | Error::FeeTooLow { .. }
        | Error::WalletNotFound { .. }
        | Error::WalletNotFoundByName { .. }
        | Error::WalletExists { .. }
        | Error::InvalidPassword(_) => ErrorCategory::User,
        // Insufficient funds.
        Error::InsufficientFunds { .. } | Error::InsufficientTokenBalance { .. } => {
            ErrorCategory::InsufficientFunds
        }
        // Signing — credential, key material, or a tampered blob.
        Error::WalletCorrupt { .. }
        | Error::EncryptionFailed(_)
        | Error::DecryptionFailed(_) => ErrorCategory::Signing,
        // Broadcast — the node rejected the transaction.
        Error::TxBroadcastFailed(_) => ErrorCategory::Broadcast,
        // Future variants are internal failures, never operator input.
        _ => ErrorCategory::Signing,
    }
}
```

1. Add `mod error;` to `rust-wallet-app/crates/eth/src/main.rs` (near the other `mod` declarations) and `use error::classify;` plus `use wallet_errors::ErrorCategory;`.
2. In `main.rs:392-399`, replace:

```rust
    let exit_code = match run(cli) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            e.exit_code()
        }
    };
    std::process::exit(exit_code);
```

with:

```rust
    let exit_code = match run(cli) {
        Ok(()) => ErrorCategory::Success.exit_code(),
        Err(e) => {
            eprintln!("error: {e}");
            classify(&e).exit_code()
        }
    };
    std::process::exit(exit_code);
```

3. In `main.rs:385`, the `.env` parse failure currently exits 2 — a malformed `.env` is operator input, so it becomes exit 1:

```rust
            std::process::exit(ErrorCategory::User.exit_code());
```

4. Delete `pub fn exit_code(&self) -> i32` and its doc block (beginning `/// Stable exit-code mapping per #297 M11.`) from `rust-wallet-app/crates/evm-wallet-core/src/error.rs`.
5. In that file's own `#[cfg(test)] mod tests`, **20** assertions call the deleted method and will no longer compile. They are not two — they are five whole tests plus three assertions inside a sixth:

   | test | `.exit_code()` sites | what it does now |
   |---|---|---|
   | `rpc_variants_map_to_exit_3` | 2 | asserts the deleted table's code for `Rpc` / `GasEstimateFailed` — **delete the whole test** |
   | `invalid_input_variants_map_to_exit_2` | 5 | same, for the seven bad-input variants — **delete the whole test** |
   | `wallet_balance_variants_map_to_exit_4` | 4 | same, for the wallet/balance variants — **delete the whole test** |
   | `signing_rpc_variants_map_to_exit_5` | 5 | same, for the signing/broadcast/timeout variants — **delete the whole test** |
   | `abidecode_failed_maps_to_exit_2` | 1 | same, for `AbiDecodeFailed` — **delete the whole test** |
   | `external_error_conversions_map_to_expected_variants` | 3 | tests the `From` impls, **not** the exit table — **keep the three `matches!` assertions and delete only the three `assert_eq!(err.exit_code(), N)` lines** |

   **Keep `all_19_variants_exist_at_compile_time` unchanged** — it is the compile-time witness that every variant is constructible, it does not call `exit_code()`, and it is the *variant-set* pin the deleted tests were standing in for. **Do not add a replacement test.** A test whose body is `let _ = Error::InvalidPassword("x".into());` asserts nothing and is a defect, not a replacement — the coverage those five tests provided now lives in the CLI's table-driven classifier test (Step 1) and in the `wallet-errors` parity test.

6. Update the doc comments that name the deleted method — **every one of them, across the workspace**, because the method is gone:

   | file:line | current text | replacement |
   |---|---|---|
   | `evm-wallet-core/src/error.rs:5-6` | module doc: "Stable exit-code mapping per #297 M11: 0/1/2/3/4/5 …" | drop the exit-code bullet; state that the variant→category mapping lives in each CLI and the category→exit-code table in `wallet-errors` |
   | `evm-wallet-core/src/error.rs:24` | "In-crate callers (e.g. `exit_code()` itself) still see all variants." | "In-crate callers still see all variants." |
   | `evm-wallet-core/src/error.rs:118` | "its own parse failures through the same `exit_code()` table." | "its own parse failures through the same canonical error type." |
   | `evm-wallet-core/src/error.rs:218` | "CLI surfaces exit_code() either way." | "the CLI classifies the variant either way." |
   | `eth/src/main.rs:19` | "Forwarded from `eth_wallet_core::error::Error::exit_code()` per #297 M11." | "Categories come from `wallet_errors::ErrorCategory`; the exit-code table is shared across all five CLIs." |
   | `eth/src/handlers.rs:5` and `:59` | "so all error→exit-code mapping flows through `Error::exit_code()` per …" | "so all error→exit-code mapping flows through the shared `wallet_errors::ErrorCategory` table …" |
   | `polygon/src/handlers/mod.rs:53` and `:59` | "so the CLI's `Error::exit_code()` table applies" / "distinct exit code per the canonical `evm-wallet-core::Error::exit_code()` table" | drop the exit-code-specific claims; polygon's own classification is Task 5's scope, so say the classification is the CLI's own and leave the numbers to Task 5 |
   | `polygon/tests/local_testnet_smoke.rs:2755` and `:2962` | "the `Error::exit_code()` mapping is not yet applied" | "the per-variant exit-code mapping is not yet applied" |

   **All of these are comment-only.** The two `polygon` entries are the only non-`eth`/`evm` files in this task, and they are here because Task 4 deletes the method they name — the same reason TRON's engine comment was corrected in Task 3. Do not change polygon's classification, exit numbers, or behaviour: that is Task 5.

7. Update to the new table **every exit-code reference in the `eth` CLI crate and its tests.** By one consistent grep (`grep -cE "exit [0-9]|exit_code|exit code \([0-9]\)|\(exit [0-9]\)|→ [A-Za-z]+ \([0-9]\)"`), the ETH crate names an exit code in **111 places**: `src/main.rs` 10, `src/handlers.rs` 22, `tests/cli_localnet.rs` 66, `tests/wallet_send_speedup.rs` 10, `tests/cli_sepolia.rs` 3. Two of the ten in `src/main.rs` are a legitimate *variable* named `exit_code`; the three in `cli_sepolia.rs` all say "exit 0" and are **correct as they stand**. The remaining **106** name a code that changes, so they are all wrong after this migration.

   **Do not find-and-replace the numbers.** The old→new mapping is *not* uniform — old 5 maps to 2, 4 **or** 5 depending on the variant, and old 4 maps to 1 **or** 3 — so key each reference on the condition it names:

   | condition named by the text | old | new |
   |---|---|---|
   | a clap usage error (bad flag, missing argument) | 2 | 1 |
   | `Error::InvalidInput` / `InvalidMnemonic` / `InvalidPrivateKey` / `InvalidPassword` / `NonceMismatch` / `FeeTooLow` / `AbiDecodeFailed` | 2 | 1 |
   | `Error::WalletNotFound` / `WalletNotFoundByName` / `WalletExists` | 4 | 1 |
   | `Error::InsufficientFunds` / `InsufficientTokenBalance` | 4 | 3 |
   | `Error::Rpc` / `GasEstimateFailed` (unreachable node, gas estimate failed) | 3 | 2 |
   | `Error::ReceiptTimeout` | 5 | 2 |
   | `Error::WalletCorrupt` / `EncryptionFailed` / `DecryptionFailed` | 5 | 4 |
   | `Error::TxBroadcastFailed` | 5 | 5 |
   | the CLI's own `.env` parse failure (`main.rs:385`) | 2 | 1 |
   | success | 0 | 0 |

   The **29 assertion-message strings** are the load-bearing part — 24 equality-assertion messages plus the 5 guard messages. They print on failure, so a message that says "must yield exit 4" while the assertion says `Some(1)` is a self-contradicting diagnostic. Update each message together with its assertion.

   `eth/src/handlers.rs:59-67` is a whole doc block listing the old per-variant codes (`Rpc (3)`, `WalletCorrupt (5)`, `InvalidMnemonic (2)`, `InvalidPrivateKey (2)`, `WalletNotFound (4)`, `WalletExists (4)`). Rewrite it to name the **category** rather than the number, so the doc does not restate a table that lives in `wallet-errors` and is tested there:

   ```
   /// Map `WalletError` → canonical `Error` so the CLI's shared category
   /// table applies (the exit codes are `wallet-errors`'):
   /// - Io/Path → Rpc (Network)
   /// - Json/Corrupt → WalletCorrupt (Signing)
   /// - Crypto/Mnemonic → InvalidMnemonic (User)
   /// - PrivateKey → InvalidPrivateKey (User)
   /// - NotFound → WalletNotFound (User)
   /// - NotFoundByName → WalletNotFound (User) + tracing event with the name
   /// - AlreadyExists → WalletExists (User)
   ```

8. Re-key the ETH test suite. `eth/tests/` carries **24** non-zero `assert_eq!` exit-code assertions and **5** `assert_ne!` lock-down guards. `cli_sepolia.rs` has **none** — all six of its call sites assert `Some(0)`, so that file needs no change.

   **The 24 equality assertions** — keyed on the condition each one exercises:

   | site | old | condition | new |
   |---|---|---|---|
   | `cli_localnet.rs` 263, 357, 385, 516, 616, 886, 931, 973, 1009, 2024, 2061, 2090, 2991, 3058, 3128, 3185 | 2 | handler-level bad input | **1** |
   | `cli_localnet.rs` 290, 322, 2126 | 4 | wallet lookup (`WalletNotFoundByName` / `WalletExists` / `WalletNotFound`) | **1** |
   | `cli_localnet.rs` 731, 2415 | 3 | unreachable node (`Error::Rpc`) | **2** |
   | `cli_localnet.rs` 2178 | 5 | wrong password (`Error::DecryptionFailed`) | **4** |
   | `wallet_send_speedup.rs` 339, 482 | 2 | `Error::FeeTooLow` | **1** |

   **Three of the 24 are clap usage errors** — `cli_localnet.rs:616` and `:931` (clap's `value_parser` on `--token`) and `:3128` (clap's `u128` parse on `--max-fee-per-gas`). All three land on 1 either way, so the renumbering is the same; only their *class* differs (class 1, a clap rejection, rather than class 2, a handler-level rejection). The other 21 are handler-level bad input. **None of the 24 becomes a banner check** — the banner replaces a lock-down *guard*, not an equality assertion.

   **The 5 guards** — `assert_ne!` on an exit code:

   | site | old | condition named | resolution |
   |---|---|---|---|
   | `cli_localnet.rs:559` | `Some(2)` | "clap rejected `--token`" | **drop it.** The clap-marker check immediately below it (`:565-570`: no `"unexpected argument"`, no `"Usage:"`, no `"unrecognized"`) already asserts clap accepted the flag. The block's own final assertion then becomes `Some(2)` (see below), so keeping `ne(2)` would contradict it |
   | `cli_localnet.rs:671` | `Some(2)` | "clap rejected `--all`" | **drop it** — same, with the clap-marker check at `:678-683` |
   | `cli_localnet.rs:1959` | `Some(2)` | "parsing rejected / must reach the network layer" | **replace it with the clap-marker stderr check** in the file's own idiom (`!stderr.contains("unexpected argument") && !stderr.contains("Usage:") && !stderr.contains("unrecognized")`). This block has no marker check today, and its first guard would contradict the block's final assertion once that becomes `Some(2)` |
   | `cli_localnet.rs:1964` | `Some(4)` | "wallet lookup rejected" | **re-key to `Some(1)`** — wallet lookup is now `User` (class 4: re-key on the new code for the condition it names) |
   | `cli_localnet.rs:1969` | `Some(5)` | "signer unlock rejected / wrong password" | **re-key to `Some(4)`** — a wrong password is now `Signing` |

   The block at `cli_localnet.rs:1955-1985` ends with `assert_eq!(code, Some(3), "erc20 send should reach send_raw_transaction and report RPC error")` — `Error::Rpc` is now `Network`, so that becomes `Some(2)`. The same change applies to the final assertions of the two blocks at `:559` and `:671`, whose `Some(3)` becomes `Some(2)`.

   `eth` does **not** depend on `assert_cmd` or `predicates` — its tests use `Command::new(env!("CARGO_BIN_EXE_eth"))` → `std::process::Output`, with `run_eth(data_dir, args)` as the harness helper (`cli_localnet.rs:156`). Use that existing style: `String::from_utf8_lossy(&out.stderr).contains("Usage:")`. **Do not add a dev-dependency to do this.**

9. Add to `rust-wallet-app/crates/eth/Cargo.toml`:

```toml
wallet-errors = { workspace = true }
```

10. In `rust-wallet-app/crates/eth/src/main.rs`, replace the plain `Cli::parse()` with one that maps a clap usage error to `User`:

```rust
let cli = match Cli::try_parse() {
    Ok(cli) => cli,
    Err(e) => {
        // A clap usage error is a user error (exit 1), not a network error
        // (exit 2). `--help` and `--version` keep clap's exit 0.
        let code = e.exit_code();
        // `print()` returns `io::Result<()>` and `Result` is `#[must_use]` —
        // the write result is dropped deliberately: the process exits either way.
        let _ = e.print();
        std::process::exit(if code == 0 {
            0
        } else {
            wallet_errors::ErrorCategory::User.exit_code()
        });
    }
};
```

**The ETH suite's exit-code equality assertions will fail loudly until this is done — that is the forcing function, and it is what stops you adjusting them back to the old codes. Note that ETH's clap lock-down guards (`assert_ne!(code, Some(2))` and the `Some(4)` / `Some(5)` ones) will NOT fail: they go vacuous, because a clap rejection now exits 1 and a wrong password now exits 4. Re-key those guards per the Global Constraints' assertion classes.**

11. Rename the ETH integration tests whose **names** encode a pre-migration exit code. **Twenty** `fn` names state a code the test no longer asserts — nineteen end in `_yields_exit_2` / `_yields_exit_4` / `_yields_exit_5` and one is `wallet_balance_all_flag_against_unreachable_rpc_exits_3_with_balanceof` — e.g. `send_command_with_wrong_password_yields_exit_5`, which now asserts `Some(4)`. A test whose name states a code it does not assert lies in its identifier, and it is the same defect class as the 106 stale comment references above. Rename each to the code it now asserts, keeping the house `..._yields_exit_N` convention. **Verify each new suffix against the `Some(N)` assertion inside its own test body** rather than against a table — a rename to the wrong number is invisible to the compiler. The names are distinct after renaming — no two collide. **Historical `CHANGELOG.md` entries quote some of the old names; leave those entries alone** — they describe what shipped in a past release and stand on their own — but add one clause to the `eth` `[user-facing]` CHANGELOG bullet below noting that the ETH test names were renamed, so a reader who greps for an old name is not left guessing.

12. Fix the one remaining reference to the deleted method that Task 4 leaves behind: `rust-wallet-app/crates/polygon/src/main.rs:108-109` — the comment above `std::process::ExitCode::from(1)` reads "T6b: all errors map to exit 1. Per-error mapping via `Error::exit_code()` lands in T6c when the error table becomes meaningful (currently every error is `Error::Rpc`)." It names the method this task deletes. **Comment-only.** Task 5 replaces that whole block, so this is a one-line correction that closes the gap for the length of one PR.

13. Fix two ETH doc comments that name a **category the code does not produce**:
    - `rust-wallet-app/crates/eth/src/error.rs:11` — the module doc lists "missing file" as a `User` (1) example, carried over verbatim from Solana's module doc where it is accurate. **For ETH it is wrong**: `eth/src/handlers.rs:61-62` folds `WalletError::Io` and `WalletError::Path` into `Error::rpc(...)`, which is `Network` (2). That behaviour is a known deferral (it needs an engine variant for I/O), so the **doc** is what must change — replace "missing file" with an example ETH actually produces, e.g. "invalid input, unknown wallet, empty passphrase".
    - `rust-wallet-app/crates/eth/src/handlers.rs:63` — "Crypto/Mnemonic → InvalidMnemonic (User)" is wrong for **Crypto**: the code at `eth/src/handlers.rs:75` maps `WalletError::Crypto(_)` to `Error::DecryptionFailed(...)`, which is `Signing` (4). Split the line: `Crypto → DecryptionFailed (Signing)` and `Mnemonic → InvalidMnemonic (User)`. **Note the divergence to carry forward (resolved in Task 5):** when this plan was written, Polygon's `map_wallet_err` mapped `WalletError::Crypto` to `Error::InvalidInput`, *not* to `InvalidMnemonic` — so Task 5 was told to describe **Polygon's** mapping, not Ethereum's. Task 5 then found that divergence to be the defect itself and **aligned Polygon's mapping with ETH's, arm for arm**; the `Crypto` arm is now `Error::DecryptionFailed` (`polygon/src/handlers/mod.rs:101`).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p eth`
Expected: PASS — the three new tests pass and the existing `eth` suite passes **after its exit-code assertions are updated to the new table. That suite carries 62 `.status.code()` call sites — 60 of them assertions, the other two repeated inside `assert_eq!` message arguments — of which 24 assert a non-zero code, and every one of those keys on an old code, so the suite cannot pass as written. 20 of the 24 are in always-on tests, so the suite breaks loudly; the other 4 sit inside `#[ignore]`d operator-driven tests (`RUN_ANVIL_E2E=1` / `SEPOLIA_E2E=1`) that CI never runs, so a missed update there can pass CI silently. `cli_sepolia.rs` asserts only `Some(0)` and needs no change. See Step 3 item 8 for the per-site table and the Global Constraints for the four assertion classes.** `cargo build -p eth-wallet-core -p polygon-wallet-core` must also succeed: nothing else called the deleted method.

- [ ] **Step 5: L12 review pair + fmt re-check + test-coverage gap analysis.**

The reviewer must check the deletion's blast radius. Run `grep -rn "exit_code()" rust-wallet-app/crates/` and confirm that only `wallet-errors` and the CLI call sites remain — no engine or test still calls the deleted method.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p eth -p eth-wallet-core --all-targets -- -D warnings
cargo test -p eth -p eth-wallet-core
```

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` — one flat bullet per changed CLI, each immediately after the `## [Unreleased]` heading, with no `###` heading and no User Stories row (see the Global Constraints for the full L24 doc-set rule):

```
- **`[user-facing]`** `rust-wallet-app/crates/eth/` — **BREAKING:** the `eth` CLI now uses the shared cross-chain exit-code table. Exit codes change: clap usage errors (bad flag, missing argument) 2→1; invalid input 2→1, ABI decode failure 2→1, wallet lookup 4→1, empty passphrase 2→1, RPC/gas-estimate 3→2, receipt timeout 5→2, wallet-corrupt/decryption/encryption-failure 5→4. **The `eth` CLI classifies all six categories, but it constructs no insufficient-funds and no broadcast-rejection error — neither `evm-wallet-core` nor `eth/src/` has a production construction site for either, only the engine's variant-coverage tests — so the insufficient-funds (3) and broadcast-rejection (5) codes are classified but not yet reachable in this CLI.** Automation that keys on exit codes must be updated. The ETH integration tests whose names encoded an exit code were renamed to match the code they now assert; historical entries above still quote the old names. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
- **`[internal]`** `rust-wallet-app/crates/evm-wallet-core/` — **BREAKING:** removed `Error::exit_code()` (and the `eth-wallet-core` / `polygon-wallet-core` re-exports of it). The exit-code table now lives in the `wallet-errors` crate and the variant→category mapping lives in each CLI, so the engines gain no `wallet-errors` edge. Callers of `evm_wallet_core::Error::exit_code()` must call `wallet_errors::ErrorCategory::exit_code()` with the category their CLI derives.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/581d-eth-migration`.

---

### Task 5: Polygon CLI migration

**Files:**
- Create: `rust-wallet-app/crates/polygon/src/error.rs`
- Modify: `rust-wallet-app/crates/polygon/src/main.rs` (the exit path, the clap path, and 11 stale exit-code references — including two inside `panic!` messages in its own unit tests)
- Modify: `rust-wallet-app/crates/polygon/src/handlers/mod.rs` (doc comment only)
- Modify: `rust-wallet-app/crates/polygon/src/handlers/wallet.rs` (doc comments only — 9 stale exit-code references)
- Modify: `rust-wallet-app/crates/polygon/src/handlers/sign.rs` (doc comments only — 9 stale exit-code references)
- Modify: `rust-wallet-app/crates/polygon/src/handlers/fee.rs` (doc comments only — 2)
- Modify: `rust-wallet-app/crates/polygon/src/handlers/erc20.rs` (doc comments only — 2)
- Modify: `rust-wallet-app/crates/polygon/src/handlers/tx.rs` (doc comments only — 1)
- Modify: `rust-wallet-app/crates/polygon/tests/local_testnet_smoke.rs` (doc comments only — 25 stale exit-code references; **no assertion in it asserts a non-zero exit code**, so none can break)
- Modify: `rust-wallet-app/crates/polygon/tests/amoy_sign_only.rs` (doc comments only — 7)
- Modify: `rust-wallet-app/crates/polygon/tests/amoy_p9_pk_import.rs` (doc comments only — 6)
- Modify: `rust-wallet-app/crates/polygon/tests/polygon_wallet_scenario.rs` (doc comments only — 1)
- Modify: `rust-wallet-app/crates/polygon/tests/amoy_error_paths.rs` (doc comments only — 1)
- Modify: `rust-wallet-app/crates/polygon/Cargo.toml`
- Modify: `rust-wallet-app/Cargo.lock`
- Test: rust-wallet-app/crates/polygon/tests/ — **no exit-code assertion in the suite asserts a non-zero code**, so none of them can break across this migration; do not "fix" exit codes it does not assert. The files above are touched for their *comments* only.

**Interfaces:**
- Consumes: `wallet_errors::ErrorCategory` from Task 1.
- Produces: `polygon::error::classify(err: &polygon_wallet_core::Error) -> ErrorCategory` (crate-internal; `polygon` has no `lib.rs`).
- Note: `polygon_wallet_core::Error` **is** `evm_wallet_core::Error` (`polygon-wallet-core/src/lib.rs:24` re-exports it), so this classifier is the same 19-variant table as Task 4's, written out again in the Polygon CLI. See the drift note below.

- [ ] **Step 1: Write the failing test**

Create `rust-wallet-app/crates/polygon/src/error.rs` containing **only the test module** — the same four tests as Task 4's `eth/src/error.rs`, with `polygon_wallet_core::Error` in place of `eth_wallet_core::Error`. The classifier is added in Step 3, so Step 2 is a genuine RED.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p polygon error`
Expected: FAIL to compile — `cannot find function classify in this scope`.

- [ ] **Step 3: Implement the classifier and wire it up**

Add the same 19-variant classifier as Task 4's, with `polygon_wallet_core::Error` in place of `eth_wallet_core::Error`. Then:

1. Add `mod error;` to `rust-wallet-app/crates/polygon/src/main.rs` and `use error::classify;`. **Do not add `use wallet_errors::ErrorCategory;`** — the code block below uses the fully-qualified `wallet_errors::ErrorCategory::User.exit_code()`, so the import would be unused and would fail `clippy -D warnings`. This matches `sol/src/main.rs:58` and `tron/src/main.rs:36`, which use the fully-qualified form with no such import.
2. Replace `rust-wallet-app/crates/polygon/src/main.rs:104-113`:

```rust
        Err(e) => {
            eprintln!("error: {e}");
            // T6b: all errors map to exit 1. Per-error mapping via
            // `Error::exit_code()` lands in T6c when the error table
            // becomes meaningful (currently every error is `Error::Rpc`).
            std::process::ExitCode::from(1)
        }
```

with:

```rust
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::from(classify(&e).exit_code() as u8)
        }
```

3. Update the doc comment at `rust-wallet-app/crates/polygon/src/handlers/mod.rs:53-59` — it describes the old per-chain `Error::exit_code()` table; it now describes the shared table. **Note:** Task 4 already removed the references to the deleted `exit_code()` method from these lines, because Task 4 is what deletes it; Task 5 adds the *new* numbers back, so it is the same lines twice, not a conflict. Rewrite the whole block to name the shared categories and their codes:

   ```
   /// Map `WalletError` (lib-level) onto `polygon_wallet_core::Error`
   /// (CLI-canonical). Called by every handler that touches `WalletManager`
   /// so the CLI's shared category table applies (design §3.5 cross-cutting
   /// line 293). Localities:
   ///
   ///   `InvalidInput` (User, exit 1) for caller-side errors (password, name,
   ///       mnemonic, duplicate name, missing wallet).
   ///   `Rpc` (Network, exit 2) for filesystem + serialization + corruption —
   ///       distinct exit code per the shared `wallet-errors` table.
   ```

4. Update to the new table **every exit-code reference in the `polygon` CLI crate and its tests.** By one consistent grep (`grep -cE "exit [0-9]|exit_code|exit code \([0-9]\)|\(exit [0-9]\)|→ [A-Za-z]+ \([0-9]\)"`), the Polygon crate named an exit code in **78 places** at the base of this task — but Task 4 removed five of them (two in `src/handlers/mod.rs`, two in `tests/local_testnet_smoke.rs`, one in `src/main.rs`), so **73 remain to re-key**. The grep also cannot match `ExitCode::from(1)`, `ExitCode 1`, or a phrase like "exit code from 2 to 3" — **at least eight more real sites exist that the pattern misses**, including the pre-migration-behaviour claims at `tests/local_testnet_smoke.rs:62/65/2754/2879/2961` and `src/main.rs:229/1509` that this change would otherwise falsify. **Sweep for the condition, not for the pattern.** The per-file figures (before Task 4): `src/main.rs` 11, `src/handlers/wallet.rs` 9, `src/handlers/sign.rs` 9, `tests/local_testnet_smoke.rs` 25, `tests/amoy_sign_only.rs` 7, `tests/amoy_p9_pk_import.rs` 6, `src/handlers/mod.rs` 4, `src/handlers/fee.rs` 2, `src/handlers/erc20.rs` 2, `src/handlers/tx.rs` 1, `tests/polygon_wallet_scenario.rs` 1, `tests/amoy_error_paths.rs` 1. **Most of them name the *engine's* old table** (`InvalidInput` → 2, `Rpc` → 3, `WalletCorrupt` → 5), which this migration replaces. Key each one on the condition it names, using the same per-condition mapping as Task 4's Step 3 item 7:

   | condition named by the text | old | new |
   |---|---|---|
   | a clap usage error | 2 | 1 |
   | `Error::InvalidInput` / `InvalidMnemonic` / `InvalidPrivateKey` / `InvalidPassword` / `NonceMismatch` / `FeeTooLow` / `AbiDecodeFailed` | 2 | 1 |
   | `Error::WalletNotFound` / `WalletNotFoundByName` / `WalletExists` | 4 | 1 |
   | `Error::InsufficientFunds` / `InsufficientTokenBalance` | 4 | 3 |
   | `Error::Rpc` / `GasEstimateFailed` | 3 | 2 |
   | `Error::ReceiptTimeout` | 5 | 2 |
   | `Error::WalletCorrupt` / `EncryptionFailed` / `DecryptionFailed` | 5 | 4 |
   | `Error::TxBroadcastFailed` | 5 | 5 |

   Two of them are inside `panic!` messages in Polygon's own unit tests (`src/main.rs:1322` and `:1501` — "wrong password must surface as InvalidInput (exit 2)"), which print on failure; update them with the code they now assert. **Do not find-and-replace the numbers** — old 5 maps to 2, 4 **or** 5 and old 4 maps to 1 **or** 3, so a blind replace would corrupt them.

   Note the same rule Task 4 applied: `polygon/tests/local_testnet_smoke.rs:2755` and `:2962` were already corrected by Task 4 because they named the deleted method; the remaining references in that file are ordinary exit-code claims and are this item's work.

5. Add to `rust-wallet-app/crates/polygon/Cargo.toml`:

```toml
wallet-errors = { workspace = true }
```

6. In `rust-wallet-app/crates/polygon/src/main.rs`, replace the plain `Cli::parse()` with one that maps a clap usage error to `User`:

```rust
let cli = match Cli::try_parse() {
    Ok(cli) => cli,
    Err(e) => {
        // A clap usage error is a user error (exit 1), not a network error
        // (exit 2). `--help` and `--version` keep clap's exit 0.
        let code = e.exit_code();
        // `print()` returns `io::Result<()>` and `Result` is `#[must_use]` —
        // the write result is dropped deliberately: the process exits either way.
        let _ = e.print();
        std::process::exit(if code == 0 {
            0
        } else {
            wallet_errors::ErrorCategory::User.exit_code()
        });
    }
};
```

**The Polygon suite asserts only zero versus non-zero — it asserts no specific non-zero exit code anywhere — and `polygon/src/main.rs` currently maps every error to exit 1. Success stays 0 and every failure stays non-zero, so no assertion in this suite can break across the migration: the suite will be green whether or not this step is done. Do not read a green Polygon suite as evidence that the new classification is right — the classifier's own unit tests are the evidence.**

**Drift note for the PR body (L13 step 4a):** the Polygon classifier duplicates the Ethereum one because `polygon_wallet_core::Error` and `eth_wallet_core::Error` are the same type but neither CLI depends on the other, and the engines may not gain a `wallet-errors` edge. A change to one table must be applied to the other; Task 8's CI parity job checks the two files are identical modulo the import path.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p polygon`
Expected: PASS — the three new tests pass, and the existing Polygon tests still pass unchanged: the suite asserts only zero versus non-zero and polygon's CLI currently maps every error to exit 1, so nothing in it can break across this migration. **A green Polygon suite is NOT evidence that the classification is correct — the classifier's own unit tests are.**

- [ ] **Step 5: L12 review pair + fmt re-check + test-coverage gap analysis.** As Task 1 Step 5. The reviewer must confirm the Polygon table is identical to the Ethereum one.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p polygon --all-targets -- -D warnings
cargo test -p polygon
```

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` — one flat bullet per changed CLI, each immediately after the `## [Unreleased]` heading, with no `###` heading and no User Stories row (see the Global Constraints for the full L24 doc-set rule):

```
- **`[user-facing]`** `rust-wallet-app/crates/polygon/` — **BREAKING:** the `polygon` CLI now uses the shared cross-chain exit-code table instead of mapping every error to exit 1. Exit codes change: clap usage errors (bad flag, missing argument) 2→1; invalid input 1→1 (unchanged); node unreachable, RPC failure and local I/O 1→2; a wrong passphrase or a corrupt wallet file 1→4. **The `polygon` CLI classifies all six categories, but it constructs no insufficient-funds and no broadcast-rejection error, so the insufficient-funds (3) and broadcast-rejection (5) codes are classified but not yet reachable in this CLI; they become observable when the CLI's own error construction is extended.** Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/581e-polygon-migration`.

---

### Task 6: Bitcoin CLI migration

**Files:**
- Create: `rust-wallet-app/crates/btc/src/error.rs`
- Modify: `rust-wallet-app/crates/btc/src/main.rs` (the invalid-mnemonic special case, the clap path, and the comment at `:39` that the special case makes obsolete)
- Modify: `rust-wallet-app/crates/btc/src/cli.rs` (two stale exit-code claims in test-message strings — `:1173`, `:2377`)
- Modify: `rust-wallet-app/crates/btc/Cargo.toml`
- Modify: `rust-wallet-app/crates/btc/tests/cli.rs` (the invalid-mnemonic assertion at `:305-324` still expects exit 2; the seven "exit 0" assertions in the file are correct as they stand and must not change)
- Modify: `rust-wallet-app/Cargo.lock`

**Interfaces:**
- Consumes: `wallet_errors::ErrorCategory` from Task 1.
- Produces: `btc::error::classify(err: &bitcoin_wallet_core::Error) -> ErrorCategory` and `btc::error::classify_anyhow(err: &anyhow::Error) -> ErrorCategory` (crate-internal; `btc` has no `lib.rs`).

- [ ] **Step 1: Write the failing test**

Create `rust-wallet-app/crates/btc/src/error.rs` containing **only the test module** — the classifiers are added in Step 3, so Step 2 is a genuine RED:

```rust
//! Error classifier for the `btc` CLI.
//!
//! Bitcoin's `WalletStore` variant deliberately collapses not-found,
//! wrong-password, wrong-network and corrupt-blob into one observable
//! outcome (`bitcoin-wallet-core/src/error.rs:110-120`). That closure is why
//! `WalletStore` maps to `Signing` and not to `User`: not-found and
//! wrong-password must not be distinguishable by exit code.

#[cfg(test)]
mod tests {
    use bitcoin_wallet_core::error::Error;
    use wallet_errors::ErrorCategory;

    use super::{classify, classify_anyhow};

    #[test]
    fn every_bitcoin_variant_maps_to_its_documented_category() {
        let cases: Vec<(Error, ErrorCategory)> = vec![
            (Error::Network("x".into()), ErrorCategory::Network),
            (Error::Esplora("x".into()), ErrorCategory::Network),
            (Error::Electrum("x".into()), ErrorCategory::Network),
            (Error::InvalidMnemonic("x".into()), ErrorCategory::User),
            (Error::InvalidDerivationPath("x".into()), ErrorCategory::User),
            (Error::Storage("x".into()), ErrorCategory::User),
            (Error::NotInitialized("x".into()), ErrorCategory::User),
            (Error::Bdk("x".into()), ErrorCategory::User),
            (Error::Io(std::io::Error::other("x")), ErrorCategory::User),
            (Error::SpkiPin("x".into()), ErrorCategory::User),
            (Error::InsufficientFunds { needed: 2, available: 1 }, ErrorCategory::InsufficientFunds),
            (Error::TxBuild("x".into()), ErrorCategory::Signing),
            (Error::Sign("x".into()), ErrorCategory::Signing),
            (Error::Psbt("x".into()), ErrorCategory::Signing),
            (Error::AddressDerivation("x".into()), ErrorCategory::Signing),
            (Error::ScriptBuild("x".into()), ErrorCategory::Signing),
            (Error::Encryption("x".into()), ErrorCategory::Signing),
            (Error::Bitcoin(bitcoin::consensus::encode::Error::ParseFailed("x")), ErrorCategory::Signing),
            (Error::Bip137("x".into()), ErrorCategory::Signing),
            (Error::MnemonicCipher("x".into()), ErrorCategory::Signing),
            (Error::WalletStore("x".into()), ErrorCategory::Signing),
        ];
        assert_eq!(cases.len(), 21, "this table pins 21 Bitcoin variants — the enum is #[non_exhaustive], so a new variant must add a row here");
        for (err, expected) in cases {
            assert_eq!(classify(&err), expected, "wrong category for {err:?}");
        }
    }

    #[test]
    fn wallet_store_is_not_distinguishable_from_a_wrong_password() {
        // Review Focus #1: the wallet-store variant collapses not-found,
        // wrong-password, wrong-network and corrupt-blob.
        assert_eq!(
            classify(&Error::WalletStore("x".into())),
            ErrorCategory::Signing
        );
    }

    #[test]
    fn unclassified_anyhow_error_is_a_user_error() {
        let err = anyhow::anyhow!("--password-file is empty");
        assert_eq!(classify_anyhow(&err), ErrorCategory::User);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p btc error`
Expected: FAIL to compile — `cannot find function classify in this scope`.

- [ ] **Step 3: Implement the classifier and wire it up**

Add the two classifiers to `rust-wallet-app/crates/btc/src/error.rs`, above the test module:

```rust
use bitcoin_wallet_core::error::Error;
use wallet_errors::ErrorCategory;

/// Maps a Bitcoin core error to its cross-chain category.
pub fn classify(err: &Error) -> ErrorCategory {
    match err {
        // Network — the backend did not answer usefully.
        Error::Network(_) | Error::Esplora(_) | Error::Electrum(_) => ErrorCategory::Network,
        // User — the operator supplied something unusable.
        Error::InvalidMnemonic(_)
        | Error::InvalidDerivationPath(_)
        | Error::Storage(_)
        | Error::NotInitialized(_)
        | Error::Bdk(_)
        | Error::Io(_)
        | Error::SpkiPin(_) => ErrorCategory::User,
        // Insufficient funds.
        Error::InsufficientFunds { .. } => ErrorCategory::InsufficientFunds,
        // Signing, transaction construction, and the collapsed
        // wallet-store outcome.
        Error::TxBuild(_)
        | Error::Sign(_)
        | Error::Psbt(_)
        | Error::AddressDerivation(_)
        | Error::ScriptBuild(_)
        | Error::Encryption(_)
        | Error::Bitcoin(_)
        | Error::Bip137(_)
        | Error::MnemonicCipher(_)
        | Error::WalletStore(_) => ErrorCategory::Signing,
        // Future variants are internal failures, never operator input.
        _ => ErrorCategory::Signing,
    }
}

/// Classifies an anyhow error chain.
///
/// Walks the chain for a `bitcoin_wallet_core::Error`; an error that never
/// reached the core (a CLI-level rejection) is a user error.
pub fn classify_anyhow(err: &anyhow::Error) -> ErrorCategory {
    for cause in err.chain() {
        if let Some(lib_err) = cause.downcast_ref::<Error>() {
            return classify(lib_err);
        }
    }
    ErrorCategory::User
}
```

1. Add `mod error;` to `rust-wallet-app/crates/btc/src/main.rs` and `use error::classify_anyhow;`.
2. Replace `rust-wallet-app/crates/btc/src/main.rs:220-233`:

```rust
    // Per Story 2 AC: invalid mnemonic → exit code 2 (not anyhow default 1).
    // Walk the error chain looking for the lib-level InvalidMnemonic variant
    // (wrapped by handle_import's .context(...)? call).
    if let Err(e) = dispatch_result {
        let is_invalid_mnemonic = e.chain().any(|c| {
            matches!(
                c.downcast_ref::<LibError>(),
                Some(LibError::InvalidMnemonic(_))
            )
        });
        if is_invalid_mnemonic {
            eprintln!("{e:?}");
            std::process::exit(2);
        }
        return Err(e);
    }
```

with:

```rust
    if let Err(e) = dispatch_result {
        eprintln!("{e:?}");
        std::process::exit(classify_anyhow(&e).exit_code());
    }
```

3. Remove the now-unused `use bitcoin_wallet_core::error::Error as LibError;` import if nothing else in `main.rs` uses it (clippy will flag it).
4. Add to `rust-wallet-app/crates/btc/Cargo.toml`:

```toml
wallet-errors = { workspace = true }
```

5. In `rust-wallet-app/crates/btc/src/main.rs`, replace the plain `Cli::parse()` with one that maps a clap usage error to `User`:

```rust
let cli = match Cli::try_parse() {
    Ok(cli) => cli,
    Err(e) => {
        // A clap usage error is a user error (exit 1), not a network error
        // (exit 2). `--help` and `--version` keep clap's exit 0.
        let code = e.exit_code();
        // `print()` returns `io::Result<()>` and `Result` is `#[must_use]` —
        // the write result is dropped deliberately: the process exits either way.
        let _ = e.print();
        std::process::exit(if code == 0 {
            0
        } else {
            wallet_errors::ErrorCategory::User.exit_code()
        });
    }
};
```

**The BTC suite's invalid-mnemonic assertion must be updated to exit 1 in the same change.**

6. Update the remaining stale exit-code references in the `btc` crate. The Bitcoin crate names an exit code in **12** places, and only **four** of them are wrong:

   | site | current | new | why |
   |---|---|---|---|
   | `btc/tests/cli.rs:314` | `Some(2)` | **`Some(1)`** | `Error::InvalidMnemonic` is `User` (1) — the assertion itself, and its message string |
   | `btc/src/cli.rs:1173` | "exit 2" | "exit 1" | a clap parse rejection is now `User` |
   | `btc/src/cli.rs:2377` | "exit 2" | "exit 1" | a clap parse rejection is now `User` |
   | `btc/src/main.rs:37-40` | "invalid mnemonic → exit code 2 (not anyhow's default exit 1)" | *removed* | the special case it describes is replaced by `classify_anyhow` in item 2 |

   The other eight are "exit 0" claims — in `btc/src/handlers.rs:428`, `btc/src/cli.rs` and six assertion messages in `btc/tests/cli.rs` — and **exit 0 does not change**. Leave them alone.

   `btc/src/cli.rs:408` says a fee-rate that does not exceed the original's effective rate "exit 4 otherwise". That stays 4: the path surfaces `Error::TxBuild`, which is `Signing` (4) under the new table. Leave it alone.

   **The "classified vs fell through" distinction is lost by design.** Once `InvalidMnemonic` maps to `User` (1), it shares its code with the anyhow fallback. The stderr pin — `stderr.contains("checksum mismatch")` at `btc/tests/cli.rs:320-324` — is what still proves the error was *classified* as an invalid mnemonic rather than falling through, so that assertion must survive the change.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p btc`
Expected: PASS — the three new tests pass, and `btc/tests/cli.rs` passes after its invalid-mnemonic assertion is updated from exit 2 to exit 1. The `btc` integration test `btc-regtest-smoke.rs` is `#[ignore]`d and needs a regtest container; run the full `cargo test -p btc` (not `--lib`) to confirm nothing else broke.

- [ ] **Step 5: L12 review pair + fmt re-check + test-coverage gap analysis.** As Task 1 Step 5. The reviewer must specifically confirm the `WalletStore` → `Signing` decision, since it makes Bitcoin's "wallet not found" exit 4 while the other four chains exit 1.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p btc --all-targets -- -D warnings
cargo test -p btc
```

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` — one flat bullet per changed CLI, each immediately after the `## [Unreleased]` heading, with no `###` heading and no User Stories row (see the Global Constraints for the full L24 doc-set rule):

```
- **`[user-facing]`** `rust-wallet-app/crates/btc/` — **BREAKING:** the `btc` CLI now uses the shared cross-chain exit-code table. Exit codes change: clap usage errors (bad flag, missing argument) 2→1; invalid mnemonic 2→1, backend unreachable 1→2, insufficient funds 1→4, signing/wallet-store failure 1→4. Note that `btc` reports "wallet not found" as exit 4, not 1, because `bitcoin-wallet-core`'s `WalletStore` variant deliberately collapses not-found with wrong-password and corrupt-blob to close the file-existence oracle. **The `btc` CLI classifies all six categories, but it constructs no insufficient-funds and no broadcast-rejection error, so the insufficient-funds (3) and broadcast-rejection (5) codes are classified but not yet reachable in this CLI: the Bitcoin engine surfaces a funds shortage as `Error::TxBuild` (there is no `Error::InsufficientFunds` construction site outside the engine's own tests), which is `Signing` (4).** **Error output no longer carries Rust's `Error: ` prefix on stderr; automation that greps stderr for `Error: ` must be updated.** Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/581f-btc-migration`.

---

### Task 7: `docs/wallets/CONTEXT.md` — the tested spec and the corrected flag table

**Files:**
- Modify: `docs/wallets/CONTEXT.md`

**Interfaces:**
- Consumes: the six-category table (Task 1) and the five classifiers (Tasks 2–6).
- Produces: the document that Task 1's `context_doc_parity` test asserts against, plus the corrected cross-chain flag table.

- [ ] **Step 1: Write the failing check**

The parity test from Task 1 already guards the category table. The flag table has no guard — this task corrects it, and Task 8 adds the guard.

The rows that are wrong today (`docs/wallets/CONTEXT.md:79-87`):

```
| `--network <name>` | Network selection | ✓ |
| `--rpc <url>`      | Override default RPC URL | ✓ |
| `--spki-pin <hex>` | Override default SPKI pin | ✓ |
| `--sign-only`      | Sign but don't broadcast | partial |
| `--wait`           | Wait for confirmation | ✓ |
| `--mnemonic-file <path>` | Read mnemonic from file (not argv) | ✓ |
```

- [ ] **Step 2: Replace the flag table with the verified one**

The matrix below was produced by building all five CLIs and crawling `--help` across every subcommand — not by reading the old document, and not by reading the clap attributes. Re-verify it if any CLI's argument definitions change before this task runs.

| Flag | btc | eth | polygon | tron | sol |
|---|:-:|:-:|:-:|:-:|:-:|
| `--address` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--data-dir` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--dry-run` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--id` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--json` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--mnemonic` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--to` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--network` | ✓ | ✓ | ✓ | ✓ | — |
| `--cluster` | — | — | — | — | ✓ |
| `--amount` | — | ✓ | ✓ | ✓ | ✓ |
| `--amount-sat` | ✓ | — | — | — | — |
| `--mnemonic-file` | — | — | ✓ | ✓ | ✓ |
| `--password` | ✓ | ✓ | ✓ | ✓ | — |
| `--password-file` | ✓ | — | — | — | — |
| `--private-key-file` | — | — | ✓ | ✓ | ✓ |
| `--name` | — | ✓ | ✓ | ✓ | ✓ |
| `--unit` | — | ✓ | ✓ | ✓ | ✓ |
| `--token` | — | ✓ | ✓ | ✓ | ✓ |
| `--sign-only` | — | — | ✓ | ✓ | ✓ |
| `--wait` | — | ✓ | ✓ | ✓ | ✓ |
| `--limit` | ✓ | ✓ | ✓ | — | — |
| `--spender` | — | ✓ | ✓ | ✓ | — |
| `--fee-limit` | — | — | — | ✓ | — |
| `--gas-limit` | — | ✓ | ✓ | — | — |
| `--max-fee-gwei` | — | ✓ | ✓ | — | — |
| `--fee-rate` | ✓ | — | — | — | — |
| `--priority-fee` | — | — | — | — | ✓ |
| endpoint override | `--esplora-url` | `--rpc-url` | `--rpc-url` | `--rpc-url`, `--rpc` | `--rpc`, `--url` |
| SPKI pin override | `--pin-spki`, `--esplora-spki-pin` | — | — | — | `--spki-pin` |

Notes that must accompany the table:

- **Seven flags are common to all five CLIs:** `--address`, `--data-dir`, `--dry-run`, `--id`, `--json`, `--mnemonic`, `--to`. (The grilling of the recommendations document recorded **six** and omitted `--mnemonic`; measuring against the built binaries gives seven. Solana has both `--mnemonic` and `--mnemonic-file`.)
- **The endpoint override has four different spellings** across the five CLIs and means something slightly different on each — this is genuinely per-chain and is *not* unified by this phase. On `sol` the two are genuinely different things: `--rpc` is the **global runtime override** (present on every subcommand's `--help`), while `--url` is the argument of **`sol config set-rpc`**, which writes the endpoint into the config instead of overriding it for one invocation.
- **The SPKI pin has three names over two chains.** `eth`, `polygon` and `tron` take their pin from configuration, not from a flag.
- `--network` is not universal: Solana selects a `--cluster`.
- `--mnemonic-file` exists on three CLIs, not all five; Bitcoin uses `--password-file` / `--password-stdin`, and `eth` has no mnemonic-file flag.
- `--wait` exists on four CLIs, not all five.
- `--sign-only` exists on three CLIs (polygon, tron, sol), not two — the old table said "partial", which was both imprecise and wrong.

- [ ] **Step 3: Rewrite the note under the error table**

Replace `docs/wallets/CONTEXT.md:100`:

```
(Pattern implemented in `sol/src/handlers/error.rs:27` `classify()` + `tron/src/handlers/mod.rs:75` `exit_code()`. `btc` deviates — uses bespoke logic that exits 2 only on `LibError::InvalidMnemonic` at `btc/src/main.rs:229`, no general classifier.)
```

with:

```
**Implementation (2026-10-30, unified).** The category→exit-code table lives in one
place: the `wallet-errors` crate (`rust-wallet-app/crates/wallet-errors/`), and
this table is asserted against it by
`wallet-errors/tests/context_doc_parity.rs` — the table is the specification,
not a description of it.

Each CLI keeps its own variant→category mapping (chain-specific error granularity
is a security feature and is deliberately not unified):

| CLI | Classifier |
|---|---|
| `btc` | `crates/btc/src/error.rs::classify` |
| `eth` | `crates/eth/src/error.rs::classify` |
| `polygon` | `crates/polygon/src/error.rs::classify` |
| `tron` | `crates/tron/src/handlers/mod.rs::classify` |
| `sol` | `crates/sol/src/handlers/error.rs::classify` |

Four intentional divergences from the categories above:

- **Bitcoin's "wallet not found" exits 4, not 1.** `bitcoin-wallet-core`'s
  `WalletStore` variant deliberately collapses not-found, wrong-password,
  wrong-network and corrupt-blob into one observable outcome to close the
  file-existence oracle (`crates/bitcoin-wallet-core/src/error.rs:110-120`).
  Because not-found and wrong-password share the variant, they must share the
  code.
- **An empty passphrase exits 1 where the CLI refuses it up front, and 4 on
  the paths that open an existing wallet.** The library cannot tell an empty
  passphrase from a wrong one — both are a failed KDF, both surface as
  `WalletError::Crypto` — so only the CLI can make the distinction, and the
  CLIs draw that line at different points:
  - **`eth` and `polygon`** refuse it on **create and import**
    (`eth/src/handlers.rs:127`, `polygon/src/handlers/wallet.rs:373` →
    `InvalidPassword` → **1**). On the paths that **open an existing wallet**
    the passphrase is a credential for a wallet that already exists, so an
    empty one is indistinguishable from a wrong one and exits **4**, exactly
    as a wrong one does. Which paths those are differs per CLI: in `eth`
    they are native send, ERC-20 send and speed-up; in `polygon` they are
    sign-message, sign-typed, ERC-20 send and ERC-20 approve. `polygon`
    additionally refuses it on its **native-send** and **speed-up** paths
    (**1**), where `eth` does not.
  - **`eth`'s sign-message, sign-typed and ERC-20 approve are deferred stubs**
    that return `Error::Rpc` (**2**) without touching the wallet, so on those
    commands the question does not arise.
  - **`tron`** refuses it **on every path** — `tron/src/handlers/mod.rs:302`
    rejects an empty passphrase centrally, from argv, the env var and the TTY
    prompt — so it exits **1** everywhere, sign included. That is TRON being
    stricter, not a different taxonomy.
  - **`btc` accepts one where the other three chains refuse it.** Its
    `password_or_prompt` (`btc/src/handlers.rs:84`) treats an empty
    `--password` argv as "not supplied" and falls through to the TTY prompt,
    and argon2 0.5.3 derives a key from an empty password without complaint
    (`bitcoin-wallet-core/src/crypto/argon2.rs:104` calls
    `Argon2::hash_password_into`, whose `verify_inputs` check rejects only
    passwords *longer than* `u32::MAX`) — so `btc wallet create` **succeeds
    with exit 0** and stores a wallet encrypted under an empty passphrase. On
    the paths that open a wallet **created with a non-empty passphrase** the
    empty passphrase derives a *different* key, so the **AES-GCM tag check**
    fails and the error lands on `WalletStore` → **4**, like a wrong
    passphrase; a wallet that was itself created under an empty passphrase
    re-derives the *same* key and opens. **`sol` accepts one the same way**
    — its only password source is the `SOL_WALLET_PASSWORD` env var, and
    nothing downstream checks emptiness. This is pre-existing, unchanged by
    the migration, and recorded as a deferred finding.

  The EVM CLIs' differences on the wallet-open paths are a **recorded deferred
  finding**: closing them means changing `eth` and `polygon` together, against
  the already-merged ETH migration, with both `CHANGELOG.md` bullets
  corrected.
- **A chain reaches only the categories its engine can express. Solana reaches
  all six; `eth`, `polygon`, `btc` and `tron` each reach four** — 0, 1, 2 and 4.
  None of the EVM, Bitcoin or TRON engines constructs an insufficient-funds or
  a broadcast-rejection error outside its own variant-coverage tests, so **3
  and 5 are unreachable in all four**; a node-side transaction rejection is
  reported as `Network` (2), the same code as an unreachable node. This is a
  property of the engines, not of the taxonomy — the table above is the
  specification for every chain that *can* reach a category.
- **The `polygon` CLI translates the wallet library's errors the same way `eth`
  does.** Its `map_wallet_err` maps the library's errors into `Rpc`,
  `WalletCorrupt`, `DecryptionFailed`, `InvalidMnemonic`, `InvalidPrivateKey`,
  `WalletNotFound`, `WalletNotFoundByName` and `WalletExists`, and the CLI
  itself raises `InvalidInput` and `InvalidPassword` — so it exits **1**
  (operator input), **2** (node unreachable, RPC failure, local I/O) or **4**
  (a wrong passphrase or a corrupt wallet file), exactly as `eth` does. A
  wrong passphrase exits 4 rather than 1 so that it stays indistinguishable
  from a corrupt wallet file, exactly as in the other four chains.

**This changed the exit codes of all five CLIs.** Automation that keys on exit
codes must be updated — see the `**BREAKING**` entries in `CHANGELOG.md`.
```

Also extend the **Examples** column of the error table itself
(`docs/wallets/CONTEXT.md:91-98`), because the categories now cover more
conditions than the two-word examples name. The parity test reads only the
**label** and the **exit-code** columns, so the Examples cell is free text —
but it is the cell a reader consults:

| Category | Exit code | Examples |
|---|---|---|
| Success | 0 | OK |
| User error | 1 | Invalid args, missing file, unknown wallet, invalid mnemonic or address, ABI decode failure |
| Network error | 2 | RPC unreachable, gas estimate failed, receipt timeout |
| Insufficient funds | 3 | Balance below amount + fee |
| Signing error | 4 | Wrong password, key derivation failure, corrupt or tampered wallet file, encryption or KDF failure, transaction construction failure, internal machinery failure, feature not yet implemented |
| Broadcast error | 5 | Tx REVERTED, node rejected |

The `Signing` row is the one that needs extending: "Wrong password, invalid
key" does not mention a corrupt blob, and under this taxonomy a corrupt blob
**must** be indistinguishable from a wrong password by exit code. The
`Broadcast error` row already reads "Tx REVERTED, node rejected" and needs no
change.

**Do not change the label or the exit-code cells.** `wallet-errors/tests/context_doc_parity.rs`
asserts them row-for-row against `ErrorCategory::ALL`, and changing either
breaks that test.

- [ ] **Step 4: Run the parity test to verify it still passes**

Run: `cargo test -p wallet-errors`
Expected: PASS — the category table itself was not edited, so the parity test is unaffected. If it fails, the table was edited by accident; restore it.

- [ ] **Step 5: L12 review pair + fmt re-check.**

Doc-only change — the L13 *trivial tier* applies to the cargo gates (skipped for doc-only commits), but the review still runs because the content is a contract. The reviewer verifies every ✓/— cell in the flag table against the built binaries' `--help` output.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo test -p wallet-errors
```

(The doc change is outside the Rust workspace, so `cargo fmt` / `clippy` are N/A for this task — trivial tier per L13's amendment.)

- [ ] **Step 7: L24 doc set**

This task *is* the doc set. No CHANGELOG bullet is needed for a correction to an internal reference document — the exit-code migration is already announced by the five per-CLI bullets in Tasks 2–6.

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/581g-context-doc`.

---

### Task 8: CI — the error-mapping parity gate

**Files:**
- Modify: `.github/workflows/workspace.yml` (add a job)

**Interfaces:**
- Consumes: the five classifiers (Tasks 2–6) and the engine error enums.
- Produces: a CI job named `error-mapping-parity` that fails the build when a chain's error enum grows a variant that its CLI has not classified, or when the Ethereum and Polygon tables drift apart.

- [ ] **Step 1: Write the failing check**

The check does not exist yet, so it is written as a script the job runs. Add this job to `.github/workflows/workspace.yml`:

```yaml
  error-mapping-parity:
    name: Error mapping parity
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
      - name: Every engine error variant is classified by its CLI
        run: |
          set -euo pipefail
          cd rust-wallet-app/crates
          check() {
            engine="$1"; cli="$2"
            variants=$(sed -n '/pub enum Error {/,/^}/p' "$engine" \
                        | grep -oE '^    [A-Z][A-Za-z0-9]+' \
                        | tr -d ' ')
            if [ -z "$variants" ]; then
              echo "::error file=$engine::could not extract variants — the check is vacuous"
              return 1
            fi
            missing=0
            for variant in $variants; do
              # The boundary matters: without it `Error::Node` matches
              # `Error::NodeResponse` and `Error::WalletNotFound` matches
              # `Error::WalletNotFoundByName`, so an unclassified variant can
              # be masked by a longer sibling that *is* classified. Both of
              # those pairs exist today (tron `Node`/`NodeResponse`, evm
              # `WalletNotFound`/`WalletNotFoundByName`).
              if ! grep -qE "Error::${variant}([^A-Za-z0-9_]|$)" "$cli"; then
                echo "::error file=$cli::unclassified engine variant Error::$variant"
                missing=1
              fi
            done
            [ "$missing" -eq 0 ]
          }
          check bitcoin-wallet-core/src/error.rs btc/src/error.rs
          check evm-wallet-core/src/error.rs eth/src/error.rs
          check evm-wallet-core/src/error.rs polygon/src/error.rs
          check tron-wallet-core/src/error.rs tron/src/handlers/mod.rs
          check sol-wallet-core/src/error.rs sol/src/handlers/error.rs
      - name: The Ethereum and Polygon tables are identical
        run: |
          set -euo pipefail
          cd rust-wallet-app/crates
          norm() { sed -e 's/eth_wallet_core::Error/Error/g' -e 's/polygon_wallet_core::Error/Error/g' "$1"; }
          diff <(norm eth/src/error.rs | sed -n '/^pub fn classify/,/^}/p') \
               <(norm polygon/src/error.rs | sed -n '/^pub fn classify/,/^}/p')
```

- [ ] **Step 2: Run the check to verify it fails**

Run the two shell bodies locally from `rust-wallet-app/crates`.

Expected for the first check: it passes if Tasks 2–6 classified every variant. **Verify the check can fail** — add a dummy variant to a real engine error file, run the check, confirm it reports that variant as unclassified, then restore the file and confirm it is clean:

```bash
cd rust-wallet-app/crates
# 1. add a variant, e.g. `DummyUnclassified(String),` inside the enum in
#    tron-wallet-core/src/error.rs
# 2. run the check body → it must report
#    "::error file=tron/src/handlers/mod.rs::unclassified engine variant Error::DummyUnclassified"
# 3. restore:
git checkout -- tron-wallet-core/src/error.rs
git status --short rust-wallet-app/crates/tron-wallet-core/src/error.rs   # must be empty
```

A check that cannot fail is not a check.

Expected for the second check: `diff` produces no output and exits 0. If it reports a difference, fix the table (a real drift) rather than the check.

- [ ] **Step 3: Add the job to the workflow**

Insert the `error-mapping-parity` job into `.github/workflows/workspace.yml` alongside `rust-lint` and `rust-test`. It runs no Rust toolchain and invokes no `cargo` command — it is a source scan — so it needs only the checkout.

- [ ] **Step 4: Verify the workflow parses**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/workspace.yml'))"`
Expected: exits 0 with no output (valid YAML).

- [ ] **Step 5: L12 review pair + fmt re-check.** The reviewer must confirm the `sed` range `/pub enum Error {/,/^}/p` extraction is correct for the **four** engine files that declare their own enum (`bitcoin-wallet-core`, `evm-wallet-core`, `tron-wallet-core`, `sol-wallet-core` — `eth-wallet-core` and `polygon-wallet-core` only re-export it), and that the check fails loudly when the extraction yields zero variants (a vacuous pass is worse than no check). The controller has already run the extraction against all four and it yields **21 / 19 / 12 / 29** variants respectively — matching the four per-chain tables in this plan.

- [ ] **Step 6: Verify gate**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/workspace.yml'))"
```

CI-only change — the Rust cargo gates are N/A (trivial tier for the Rust workspace).

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` — one flat bullet per changed CLI, each immediately after the `## [Unreleased]` heading, with no `###` heading and no User Stories row (see the Global Constraints for the full L24 doc-set rule):

```
- **`[internal]`** `.github/workflows/workspace.yml` — new `error-mapping-parity` CI job. Fails the build when a chain engine's error enum grows a variant that its CLI has not classified, and when the `eth` and `polygon` classifiers drift apart. Closes the silent-divergence path that finding F5 (four taxonomies that disagree) describes. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/581h-ci-parity`.

---

## Completion

When all eight sub-tasks are merged into `task/581-wallet-errors`:

```bash
git checkout main
git merge task/581-wallet-errors --no-ff -m "Task 1: wallet-errors — unified CLI error categories and exit codes"
git push origin main
```

Then the L13 post-merge cascade: L24 (already travelled with each sub-task PR), L21 (estimate report + AI cost report), L14 (`progress.md` ledger entry), and a release cut if the maintainer wants `wallet-errors` to ship as its own release — the spec's process-compliance note says it should ("`wallet-errors` ships alone at the end of Phase 1").

## Deferred, with tracking

These fall out of this phase and must be filed as backlog issues (`backlog` + `priority/pN` + chain label + `task`, per L13 step 11a and PL22–PL26 — **PAUSE before `gh issue create`**):

1. **EVM wallet-file I/O failures surface as `Network` (exit 2).** `eth/src/handlers.rs:59-68` folds `WalletError::Io` and `WalletError::Path` into `Error::rpc(...)`, and `evm-wallet-core::Error` has no I/O variant, so the CLI cannot classify it as `User` without an engine change. Fix it in Phase 3 (`wallet-storage`), where the storage layer lands.
2. **Bitcoin's "wallet not found" exits 4 while the other four chains exit 1.** Deliberate and documented — but a caller keying on "wallet missing" across chains needs to know. If a future `wallet-storage` trait introduces a shared `WalletId`, revisit it.
3. **The `rust-wallet-app` workspace version is `0.2.0` while `CHANGELOG.md` already documents `v0.3.0`.** A pre-existing inconsistency, not introduced here. Task 4 removes public API from three library crates, so the next release needs a version bump to `0.3.0` — a release decision, not a task in this plan.
4. **The declared MSRV `rust-version = "1.85"` is unachievable** — the vendored anychain stack requires 1.98.1. Pre-existing; fix separately.
5. **The two EVM CLIs disagree with each other about an empty passphrase on the wallet-open paths.** `polygon` refuses it on its native-send and speed-up paths (**1**) but not on ERC-20 send and approve (**4**), while `eth` refuses it nowhere but create and import — so `eth`'s native send, ERC-20 send and speed-up all exit **4**. ETH's ERC-20 *approve*, sign-message and sign-typed are deferred stubs that never reach a wallet and exit **2**. Closing the gap means changing `eth` and `polygon` together, against the already-merged ETH migration, with both `CHANGELOG.md` bullets corrected — so it is a separate cross-chain task, not a Task 5 fix.
6. **`btc` accepts an empty passphrase where the other three chains (`eth`, `polygon`, `tron`) refuse it.** On a TTY, a blank response yields an empty password, and argon2 0.5.3 derives a key from it (`bitcoin-wallet-core/src/crypto/argon2.rs:104` calls `Argon2::hash_password_into`, whose `verify_inputs` check rejects only passwords *longer than* `u32::MAX`), so `btc wallet create` **succeeds with exit 0** and stores a wallet encrypted under an empty passphrase — anyone holding the file can decrypt it, and a wallet created that way re-opens with the same empty passphrase. Pre-existing and unchanged by this migration (it is not an exit-code defect; the operation succeeds), but it is a security-relevant divergence from `eth`, `polygon` and `tron`, which all refuse an empty passphrase at the CLI boundary. **File it with the security label, not `backlog` alone.**
7. **`sol` accepts an empty passphrase too.** `sol/src/handlers/wallet.rs:740-745` reads `SOL_WALLET_PASSWORD` and returns `Ok("")` for an empty env var, and nothing downstream checks emptiness (`sol-wallet-core/src/crypto.rs`'s `derive_key` calls `a2.hash_password_into(password, salt, …)` with no length guard). Measured: `SOL_WALLET_PASSWORD="" sol wallet create …` → **exit 0**, wallet written. Same security shape as item 6 and the same remedy — refuse the empty passphrase at the CLI boundary, as `eth`, `polygon` and `tron` already do. **File it with the security label too.**
8. **Documents under `docs/wallets/` outside `CONTEXT.md` still state the pre-migration exit codes.** Task 7 owns `CONTEXT.md` only. The rest were found by the Task 6 review: `docs/wallets/2026-08-05-btc-wallet-user-stories.md:122` ("Invalid checksum returns exit code 2") and the per-story acceptance claims around it, and `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md:1430`/`:1853`/`:2110` and `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md:1021`, which describe a "matches `btc/src/main.rs:151-169` exit-2 pattern" that no longer exists. **Task 7's survey found three more, and they are the most stale of all: `docs/wallets/2026-08-23-eth-wallet-user-stories.md:129` and `:148`, `docs/wallets/2026-08-27-tron-wallet-user-stories.md:155` and `:175`, and `docs/wallets/2026-08-27-polygon-wallet-user-stories.md:137` each state that an existing wallet "fails with exit code 2" (now **1**) — the first two also state "Invalid checksum returns exit code 2" (now **1**) — and each of the three reproduces the whole pre-migration taxonomy in prose: `2026-08-23-eth-wallet-user-stories.md:615`, `2026-08-27-tron-wallet-user-stories.md:610` and `2026-08-27-polygon-wallet-user-stories.md:560` spell out "0 = success, 1 = user abort, 2 = bad input, 3 = upstream/RPC transport failure, 4 = wallet/balance issue, 5 = signing/RPC/broadcast error", which shares no category with the shared table but the six numbers.** The deep-dive files are historical research records and arguably stand on their own; the four user-stories files are live spec documents and are genuinely stale.
