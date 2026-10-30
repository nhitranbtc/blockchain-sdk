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
- **`wallet-errors` has zero dependencies** — not even `thiserror`. This keeps `Cargo.lock` stable after Task 1 (no merge conflicts across the eight sub-task branches) and adds no supply-chain surface.
- **No engine crate changes behaviour.** `bitcoin-wallet-core`, `evm-wallet-core`, `eth-wallet-core`, `polygon-wallet-core`, `tron-wallet-core` and `sol-wallet-core` are untouched except for the **deletion** of `evm_wallet_core::Error::exit_code()` in Task 4, which the spec sanctions ("Phase 1 deletes or delegates it"). Delegation is not an option: it would require an engine → `wallet-errors` edge, which the spec forbids ("no engine edge changes"), and a retained shim would be the *second competing table* that finding F5 is about.
- **No engine gains a `wallet-errors` dependency.** The mapping lives in the CLI layer.
- **The per-chain error enums stay.** Their granularity is a security feature (`bitcoin-wallet-core/src/error.rs:109-120` keeps `WalletStore` separate from `Storage` to preserve a file-existence oracle closure). This phase unifies the *category*, never the enum.
- **Wrong password and corrupt blob stay indistinguishable** — both map to `Signing` (exit 4) in every chain, and must not be distinguishable by exit code.
- **`cargo test -p <crate>` only** in the local verify gate. Never `--workspace` (L55: the workspace-wide run crosses the 300 s Bash timeout on `bitcoin-wallet-core`'s integration tests).
- **Rust edition / toolchain:** workspace `edition = "2021"`, `rust-toolchain.toml` pins 1.98.1. Do not bump `rust-version` — the declared MSRV 1.85 is already unachievable because the vendored anychain stack needs 1.98.1; that is a separate, pre-existing issue.
- **Error enums marked `#[non_exhaustive]`** (`bitcoin-wallet-core/src/error.rs:28`, `evm-wallet-core/src/error.rs:26`, `tron-wallet-core/src/error.rs:10`) force a wildcard arm in a downstream match. The wildcard maps to `ErrorCategory::Signing` in every case; every currently-existing variant is pinned by a table-driven test, and Task 8's CI job closes the loop on future variants. `sol_wallet_core::Error` is **not** `#[non_exhaustive]`, so the Solana classifier is exhaustive — a new Solana variant is a compile error until classified.
- **L24 doc set travels with every sub-task PR** — a `CHANGELOG.md` `[Unreleased]` bullet (Keep a Changelog; `### Changed` with a `**BREAKING**:` prefix for the exit-code change) plus a User Stories row flip where a story completes.
- **Never auto-commit** (house rule). Every task ends at a PAUSE step; the commit happens only after the maintainer approves.
- **L25 sub-task workflow** applies — eight sub-tasks, so a parent branch `task/<N>-wallet-errors` is the integration point; each sub-task branches from the parent, PRs to the parent, and merges in order with `--no-ff`.

---

## Review Focus

Five input classes the spec implies that no task's happy-path test exercises. Each names the input and the behaviour a reasonable person would expect; the owning task pins it with a test.

1. **A wallet file that is corrupt on disk.** The operator must see exit 4 (`Signing`) and the *same* message a wrong password produces — not a distinguishable error. Bitcoin's `WalletStore` deliberately collapses not-found, wrong-password, wrong-network and corrupt-blob into one variant (`bitcoin-wallet-core/src/error.rs:110-120`); EVM's `WalletCorrupt` and `DecryptionFailed` must land on the same category. Owned by Tasks 2, 4, 5, 6.
2. **An empty passphrase.** Exit 1 (`User`) — the operator supplied something unusable — and *distinct* from a wrong passphrase's exit 4 (`Signing`). These are different conditions (`eth/src/handlers.rs:127` constructs `InvalidPassword("password must be non-empty")`; a wrong passphrase surfaces as `DecryptionFailed`), so they get different codes on purpose. A regression here re-opens exactly the ambiguity `CONTEXT.md:96` records. Owned by Task 4.
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
| invalid input / bad mnemonic | 1 or 2 | 1 |
| wallet not found | 4 or 1 | 1 |
| empty passphrase | 2 | 1 |
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

`CHANGELOG.md` `[Unreleased]` → `### Added`:

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
git push -u origin task/1a-wallet-errors-crate
gh pr create --base task/1-wallet-errors --head task/1a-wallet-errors-crate \
  --title "feat(errors): add wallet-errors crate with the shared category and exit-code table" \
  --body-file /tmp/pr-body-1a.md
```

The PR body carries: Context, the six-category table, the verification output, and a **Drift** section (L13 step 4a) noting that the new crate has no dependents yet — Tasks 2–6 add them.

Merge into the parent when the PR is approved:

```bash
git checkout task/1-wallet-errors
git merge task/1a-wallet-errors-crate --no-ff -m "Merge PR #<N> (task/1a-wallet-errors-crate): wallet-errors crate"
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
        assert_eq!(cases.len(), 29, "the Solana error enum has 29 variants");
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

`CHANGELOG.md` `[Unreleased]` → `### Changed`:

```
- **`[user-facing]`** `rust-wallet-app/crates/sol/` — **BREAKING:** the `sol` CLI now uses the shared cross-chain exit-code table. Exit codes change: invalid input and missing wallet 2→1, transport/RPC/timeout 3→2, insufficient funds 3→3 (was reported as a transport error, now distinct), wallet errors 4→1, sign/persistence/config/internal 5→4, cluster rejection 3→5. Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/1b-sol-migration` off `task/1-wallet-errors`. Same commit/PAUSE/PR/`--no-ff`-merge shape as Task 1 Step 9.

---

### Task 3: TRON CLI migration

**Files:**
- Modify: `rust-wallet-app/crates/tron/src/handlers/mod.rs`
- Modify: `rust-wallet-app/crates/tron/src/main.rs`
- Modify: `rust-wallet-app/crates/tron/src/handlers/tx.rs`
- Modify: `rust-wallet-app/crates/tron/src/handlers/wallet.rs`
- Modify: `rust-wallet-app/crates/tron/Cargo.toml`

**Interfaces:**
- Consumes: `wallet_errors::ErrorCategory` from Task 1.
- Produces: `tron::handlers::classify(err: &CliError) -> ErrorCategory` and `tron::handlers::classify_core(err: &CoreError) -> ErrorCategory`. `tron` has a `lib.rs`, so these are public API of the `tron` library target.

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
        assert_eq!(cases.len(), 12, "the TRON core error enum has 12 variants");
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

`CHANGELOG.md` `[Unreleased]` → `### Changed`:

```
- **`[user-facing]`** `rust-wallet-app/crates/tron/` — **BREAKING:** the `tron` CLI now uses the shared cross-chain exit-code table. Exit codes change: operator input 2→1, wallet lookup 4→1, node unreachable 3→2, signing/encryption 5→4, declined confirmation 1→1 (unchanged). Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/1c-tron-migration`.

---

### Task 4: Ethereum CLI migration + delete `evm_wallet_core::Error::exit_code()`

**Files:**
- Create: `rust-wallet-app/crates/eth/src/error.rs`
- Modify: `rust-wallet-app/crates/eth/src/main.rs`
- Modify: `rust-wallet-app/crates/eth/src/handlers.rs` (doc comment only)
- Modify: `rust-wallet-app/crates/evm-wallet-core/src/error.rs` (delete `exit_code()` + its doc block; delete the two `exit_code()` assertions in that crate's own test module)
- Modify: `rust-wallet-app/crates/eth/Cargo.toml`

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
        assert_eq!(cases.len(), 19, "the EVM error enum has 19 variants");
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
5. In that file's own `#[cfg(test)] mod tests`, the assertions `assert_eq!(Error::InvalidPassword("x".into()).exit_code(), 2);` and `assert_eq!(Error::DecryptionFailed("x".into()).exit_code(), 5);` no longer compile — delete them, and replace the two exit-code assertions with one that pins the *variant set* rather than the code:

```rust
    #[test]
    fn exit_code_table_lives_in_wallet_errors() {
        // The per-variant exit-code table moved to the `wallet-errors` crate
        // (design spec §Components 1). `evm-wallet-core` no longer exposes
        // `Error::exit_code()` — the mapping lives in the CLI layer so the
        // engines gain no `wallet-errors` edge.
        let _ = Error::InvalidPassword("x".into());
    }
```

6. Update the doc comments that point at the deleted method:
   - `rust-wallet-app/crates/eth/src/main.rs:19` — "Forwarded from `eth_wallet_core::error::Error::exit_code()` per #297 M11." → "Categories come from `wallet_errors::ErrorCategory`; the exit-code table is shared across all five CLIs."
   - `rust-wallet-app/crates/eth/src/handlers.rs:5` and `:59` — same correction.
   - `rust-wallet-app/crates/eth-wallet-core/src/error.rs`, if it carries a copy of the doc block.
7. Add to `rust-wallet-app/crates/eth/Cargo.toml`:

```toml
wallet-errors = { workspace = true }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p eth`
Expected: PASS — the three new tests pass and the existing `eth` suite passes. `cargo build -p eth-wallet-core -p polygon-wallet-core` must also succeed: nothing else called the deleted method.

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

`CHANGELOG.md` `[Unreleased]` → two bullets, one under `### Changed` and one under `### Removed`:

```
- **`[user-facing]`** `rust-wallet-app/crates/eth/` — **BREAKING:** the `eth` CLI now uses the shared cross-chain exit-code table. Exit codes change: invalid input 2→1, wallet lookup 4→1, empty passphrase 2→1, RPC/gas-estimate 3→2, receipt timeout 5→2, insufficient funds 4→3, wallet-corrupt/decryption 5→4, broadcast failure 5→5 (unchanged). Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
- **`[internal]`** `rust-wallet-app/crates/evm-wallet-core/` — **BREAKING:** removed `Error::exit_code()` (and the `eth-wallet-core` / `polygon-wallet-core` re-exports of it). The exit-code table now lives in the `wallet-errors` crate and the variant→category mapping lives in each CLI, so the engines gain no `wallet-errors` edge. Callers of `evm_wallet_core::Error::exit_code()` must call `wallet_errors::ErrorCategory::exit_code()` with the category their CLI derives.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/1d-eth-migration`.

---

### Task 5: Polygon CLI migration

**Files:**
- Create: `rust-wallet-app/crates/polygon/src/error.rs`
- Modify: `rust-wallet-app/crates/polygon/src/main.rs`
- Modify: `rust-wallet-app/crates/polygon/src/handlers/mod.rs` (doc comment only)
- Modify: `rust-wallet-app/crates/polygon/Cargo.toml`

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

1. Add `mod error;` to `rust-wallet-app/crates/polygon/src/main.rs` and `use error::classify;` plus `use wallet_errors::ErrorCategory;`.
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

3. Update the doc comment at `rust-wallet-app/crates/polygon/src/handlers/mod.rs:53-59` — it describes the old per-chain `Error::exit_code()` table; it now describes the shared table.
4. Add to `rust-wallet-app/crates/polygon/Cargo.toml`:

```toml
wallet-errors = { workspace = true }
```

**Drift note for the PR body (L13 step 4a):** the Polygon classifier duplicates the Ethereum one because `polygon_wallet_core::Error` and `eth_wallet_core::Error` are the same type but neither CLI depends on the other, and the engines may not gain a `wallet-errors` edge. A change to one table must be applied to the other; Task 8's CI parity job checks the two files are identical modulo the import path.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p polygon`
Expected: PASS — the three new tests pass and the 12 existing Polygon tests still pass.

- [ ] **Step 5: L12 review pair + fmt re-check + test-coverage gap analysis.** As Task 1 Step 5. The reviewer must confirm the Polygon table is identical to the Ethereum one.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p polygon --all-targets -- -D warnings
cargo test -p polygon
```

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` → `### Changed`:

```
- **`[user-facing]`** `rust-wallet-app/crates/polygon/` — **BREAKING:** the `polygon` CLI now uses the shared cross-chain exit-code table instead of mapping every error to exit 1. Exit codes now distinguish invalid input (1), node unreachable (2), insufficient funds (3), signing/credential failure (4), and node rejection (5). Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/1e-polygon-migration`.

---

### Task 6: Bitcoin CLI migration

**Files:**
- Create: `rust-wallet-app/crates/btc/src/error.rs`
- Modify: `rust-wallet-app/crates/btc/src/main.rs`
- Modify: `rust-wallet-app/crates/btc/Cargo.toml`

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
        assert_eq!(cases.len(), 21, "the Bitcoin error enum has 21 variants");
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

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p btc`
Expected: PASS — the three new tests pass. The `btc` integration test `btc-regtest-smoke.rs` is `#[ignore]`d and needs a regtest container; run the full `cargo test -p btc` (not `--lib`) to confirm nothing else broke.

- [ ] **Step 5: L12 review pair + fmt re-check + test-coverage gap analysis.** As Task 1 Step 5. The reviewer must specifically confirm the `WalletStore` → `Signing` decision, since it makes Bitcoin's "wallet not found" exit 4 while the other four chains exit 1.

- [ ] **Step 6: Verify gate**

```bash
cd rust-wallet-app
cargo fmt --all -- --check
cargo clippy -p btc --all-targets -- -D warnings
cargo test -p btc
```

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` → `### Changed`:

```
- **`[user-facing]`** `rust-wallet-app/crates/btc/` — **BREAKING:** the `btc` CLI now uses the shared cross-chain exit-code table. Exit codes change: invalid mnemonic 2→1, backend unreachable 1→2, insufficient funds 1→3, signing/wallet-store failure 1→4. Note that `btc` reports "wallet not found" as exit 4, not 1, because `bitcoin-wallet-core`'s `WalletStore` variant deliberately collapses not-found with wrong-password and corrupt-blob to close the file-existence oracle. Automation that keys on exit codes must be updated. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/1f-btc-migration`.

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
| endpoint override | `--esplora-url` | `--rpc-url` | `--rpc-url` | `--rpc-url`, `--rpc` | `--url` |
| SPKI pin override | `--pin-spki`, `--esplora-spki-pin` | — | — | — | `--spki-pin` |

Notes that must accompany the table:

- **Seven flags are common to all five CLIs:** `--address`, `--data-dir`, `--dry-run`, `--id`, `--json`, `--mnemonic`, `--to`. (The grilling of the recommendations document recorded **six** and omitted `--mnemonic`; measuring against the built binaries gives seven. Solana has both `--mnemonic` and `--mnemonic-file`.)
- **The endpoint override has four different spellings** across the five CLIs and means something slightly different on each — this is genuinely per-chain and is *not* unified by this phase.
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

Two intentional divergences from the categories above:

- **Bitcoin's "wallet not found" exits 4, not 1.** `bitcoin-wallet-core`'s
  `WalletStore` variant deliberately collapses not-found, wrong-password,
  wrong-network and corrupt-blob into one observable outcome to close the
  file-existence oracle (`crates/bitcoin-wallet-core/src/error.rs:110-120`).
  Because not-found and wrong-password share the variant, they must share the
  code.
- **An empty passphrase exits 1, a wrong passphrase exits 4.** The first is
  operator input (`eth/src/handlers.rs:127` raises
  `InvalidPassword("password must be non-empty")`); the second is a credential
  failure (`DecryptionFailed`).

**This changed the exit codes of all five CLIs.** Automation that keys on exit
codes must be updated — see the `**BREAKING**` entries in `CHANGELOG.md`.
```

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

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/1g-context-doc`.

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
              if ! grep -q "Error::$variant" "$cli"; then
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

Expected for the first check: it passes if Tasks 2–6 classified every variant. **Verify the check can fail** — add a dummy variant to a *copy* of an engine error file and confirm the check reports it, then discard the copy. A check that cannot fail is not a check.

Expected for the second check: `diff` produces no output and exits 0. If it reports a difference, fix the table (a real drift) rather than the check.

- [ ] **Step 3: Add the job to the workflow**

Insert the `error-mapping-parity` job into `.github/workflows/workspace.yml` alongside `rust-lint` and `rust-test`. It runs no Rust toolchain and invokes no `cargo` command — it is a source scan — so it needs only the checkout.

- [ ] **Step 4: Verify the workflow parses**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/workspace.yml'))"`
Expected: exits 0 with no output (valid YAML).

- [ ] **Step 5: L12 review pair + fmt re-check.** The reviewer must confirm the `sed` range `/pub enum Error {/,/^}/p` extraction is correct for all five engine files, and that the check fails loudly when the extraction yields zero variants (a vacuous pass is worse than no check).

- [ ] **Step 6: Verify gate**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/workspace.yml'))"
```

CI-only change — the Rust cargo gates are N/A (trivial tier for the Rust workspace).

- [ ] **Step 7: L24 doc set**

`CHANGELOG.md` `[Unreleased]` → `### Added`:

```
- **`[internal]`** `.github/workflows/workspace.yml` — new `error-mapping-parity` CI job. Fails the build when a chain engine's error enum grows a variant that its CLI has not classified, and when the `eth` and `polygon` classifiers drift apart. Closes the silent-divergence path that finding F5 (four taxonomies that disagree) describes. Refs `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md` §Components 1.
```

- [ ] **Step 8: PAUSE — request approval.** Do not commit until approved.

- [ ] **Step 9: Commit, push, PR to the parent, merge into the parent.** Branch `task/1h-ci-parity`.

---

## Completion

When all eight sub-tasks are merged into `task/1-wallet-errors`:

```bash
git checkout main
git merge task/1-wallet-errors --no-ff -m "Task 1: wallet-errors — unified CLI error categories and exit codes"
git push origin main
```

Then the L13 post-merge cascade: L24 (already travelled with each sub-task PR), L21 (estimate report + AI cost report), L14 (`progress.md` ledger entry), and a release cut if the maintainer wants `wallet-errors` to ship as its own release — the spec's process-compliance note says it should ("`wallet-errors` ships alone at the end of Phase 1").

## Deferred, with tracking

These fall out of this phase and must be filed as backlog issues (`backlog` + `priority/pN` + chain label + `task`, per L13 step 11a and PL22–PL26 — **PAUSE before `gh issue create`**):

1. **EVM wallet-file I/O failures surface as `Network` (exit 2).** `eth/src/handlers.rs:59-68` folds `WalletError::Io` and `WalletError::Path` into `Error::rpc(...)`, and `evm-wallet-core::Error` has no I/O variant, so the CLI cannot classify it as `User` without an engine change. Fix it in Phase 3 (`wallet-storage`), where the storage layer lands.
2. **Bitcoin's "wallet not found" exits 4 while the other four chains exit 1.** Deliberate and documented — but a caller keying on "wallet missing" across chains needs to know. If a future `wallet-storage` trait introduces a shared `WalletId`, revisit it.
3. **The `rust-wallet-app` workspace version is `0.2.0` while `CHANGELOG.md` already documents `v0.3.0`.** A pre-existing inconsistency, not introduced here. Task 4 removes public API from three library crates, so the next release needs a version bump to `0.3.0` — a release decision, not a task in this plan.
4. **The declared MSRV `rust-version = "1.85"` is unachievable** — the vendored anychain stack requires 1.98.1. Pre-existing; fix separately.
