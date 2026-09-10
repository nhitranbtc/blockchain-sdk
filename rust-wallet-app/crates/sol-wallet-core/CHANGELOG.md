# Changelog — `sol-wallet-core`

All notable changes to `rust-wallet-app/crates/sol-wallet-core/`. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer 2.0.0](https://semver.org/).

Conventions: `Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security`. Phase markers per [`docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`](../../../docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md).

---

## [Unreleased]

### Planned (v0.1.0)

- Phase 0 — crate scaffold + compile/`--help` check (no crate source exists yet)
- Phase 1 — Phantom-equivalent `Wallet` keypair (`fromMnemonic`, `fromMnemonicAt`, `fromBase58`, `fromPublicKey`, sign APIs)
- Phase 2 — address surface (base58, `is_on_curve`, PDA)
- Phase 3 — `tx::builder` native SOL transfer + Compute Budget prepend
- Phase 4 — SPL `transfer_checked` + ATA lifecycle + Token-2022 disambiguation
- Phase 5 — RPC client + `send_with_retry` + `wait_for_confirm`
- Phase 6 — wallet persistence (Argon2id + AES-GCM) + `WalletManager`
- Phase 6.2 — library completeness verification (33 in-scope deep-dive rows)
- Phase 7 — `sol` CLI, 22 commands
- Phase 8 — FFI cdylib, 12 C functions + panic-message scrubber
- Phase 9 — mainnet self-send smoke gate + release cut
- v0.1 release cut: branch `rust-sol-core` → `main`

---

## Phase Set Up — 2026-09-10 — repo plumbing (no crate code)

Branch, tracker vocabulary, and CI gate established before any Rust code. This entry exists per L24; the crate directory holds only this file until Phase 0 lands `Cargo.toml` and `src/`.

### Added

- Integration branch `rust-sol-core`, cut from `main` at `9b118eb07014639ba1f8e5271be47b3322339bc2`. Every v0.1 task branches from it and PRs back into it; the single exception is the Phase 9 cut PR to `main`.
- Tracker labels `rust-sol-core` (`#c41e3a`) and `rust-sol-cli` (`#1f6feb`). Existing repo-wide `priority/p0`…`priority/p3` reused — no parallel priority scale created.
- Milestone `sol-wallet-core v0.1` (#2). No due date: the gate is the acceptance criteria, not a calendar.
- `.github/workflows/rust-sol-core-ci.yml` — six jobs at parity with umbrella `ci.yml`: `rust-lint` (fmt + clippy), `rust-test`, `rust-deps` (dedup + audit + deny), `rust-ffi-cdylib`, `rust-geiger`, plus `mobile-check` (iOS + Android arm64, not in `ci.yml`). Triggered on push/PR to `rust-sol-core` only. Crate-scoped jobs skip while `crates/sol-wallet-core/Cargo.toml` is absent, so the Phase Set Up commit can go green before Phase 0 exists; dep-tree jobs run unguarded because they read the shared workspace lockfile.

### Changed

- `rust-wallet-app/deny.toml` — `[bans]` now denies `mpl-token-metadata` and `mpl-core`, enforcing Q12 (no Metaplex surface in v0.1). A future `cargo add` that pulls either in fails `cargo deny check` rather than landing unnoticed.

### Fixed

- CI skip-guard tested `[ -d crates/sol-wallet-core ]`, but this phase's own `CHANGELOG.md` creates that directory — so the guard passed while the cargo package still did not exist, and `rust-lint`, `rust-test`, and `mobile-check` all failed run `34433048858` with `error: package ID specification 'sol-wallet-core' did not match any packages`. Guard now tests `[ -f crates/sol-wallet-core/Cargo.toml ]`, which is the condition the surrounding comment always claimed. `rust-deny` was unaffected — it is workspace-scoped and carries no guard.

### Notes — branch name vs crate name

The integration branch is **`rust-sol-core`**; the crate is **`sol-wallet-core`** at `crates/sol-wallet-core/`. They deliberately differ: the branch follows the repo's `rust-<chain>-core` convention (matching the `rust-sol-core` label and the `rust-sol-core-ci.yml` workflow, alongside `rust-tron-core` and `rust-eth-core`), while the crate name is the published library name.

Do not "unify" them with a find-replace. In this plan and CHANGELOG the string `sol-wallet-core` appears in both senses — roughly 109 crate-sense occurrences against 34 branch-sense at the time of the rename — and every `cargo -p sol-wallet-core`, `crates/sol-wallet-core/`, and `libsol_wallet_core.so` refers to the crate.

### Notes — plan drift recorded at execution time

Five deltas between the plan text and live repo state, resolved as follows:

1. **`origin/docs/2026-09-08-solana-rust-sdks-deep-dive` does not exist.** Task S.1's note assumed a docs branch holding the research + planning docs. `git ls-remote` shows no such ref; the four Solana documents were untracked in the `main` worktree. They land on `rust-sol-core` instead.
2. **MSRV pinned to 1.98.1, not 1.89.0.** Task S.5 asked for `1.89.0` (Anza's declared `rust-version`). `rust-wallet-app/rust-toolchain.toml` hard-pins channel `1.98.1` and overrides any toolchain the CI action installs for cargo invocations inside that directory — a `1.89.0` declaration in YAML would be inert. 1.98.1 satisfies the Anza floor.
3. **"Copy the structure of `rust-tron-core-ci.yml`" is stale.** That file lost its fmt/clippy/test jobs in `998cc1fc`, which consolidated them into umbrella `ci.yml`. Umbrella triggers on `main` only, so a PR into `rust-sol-core` would receive no gate at all if this file deferred to it. The plan's *job list* stands; the "copy tron" pointer does not. fmt and clippy share one runner per the post-`998cc1fc` repo convention rather than splitting into separate `rust-fmt` / `rust-clippy` jobs.
4. **Labels were absent, not pre-existing.** Both created fresh.
5. **Milestone was absent.** Only `tron-v0.1` existed.

### Notes — Task S.6 (branch protection) applied

Protection is live on `rust-sol-core`, mirroring `main`'s policy: six required status checks (`Rust lint (fmt + clippy)`, `Rust test (sol-wallet-core)`, `Rust dep checks (dedup + audit + deny)`, `Rust FFI cdylib (sol-wallet-core)`, `Rust unsafe-code audit (geiger)`, `Mobile compile-only (iOS + Android arm64)`), strict up-to-date branches, linear history, one approving review, stale reviews dismissed, no force-push, no deletion.

`enforce_admins` is `false`, matching `main`. That is deliberate on a solo-maintainer repo: a required review that an author cannot supply themselves would otherwise hard-block every merge. The admin bypass is the release valve, not an oversight.

Ordering note: protection could not be applied until after the first green run — a required status check that has never produced a run can never be satisfied, so configuring it earlier would have blocked the very push that delivered the workflow. Sequence used: push → run `34433375026` green → protect.

---

## Phase 0 — 2026-09-10 — crate scaffold (no behaviour)

Repo plumbing landed in Phase Set Up; this phase turns the empty `crates/sol-wallet-core/` directory into a compiling library + CLI binary, with the full dep graph declared but most of it deliberately unwired from the crate until Phase 1 has picked a compatible exact-pin set.

### Added

- `rust-wallet-app/crates/sol-wallet-core/Cargo.toml` — package metadata, `[lib] crate-type = ["rlib"]` (cdylib lands in Phase 8; the `rust-sol-core-ci.yml` `rust-ffi-cdylib` job is explicitly guarded on the `cdylib` literal appearing here), and a minimal `[dependencies]` block containing only `thiserror` (needed by the `error.rs` stub). The plan's full Anza/SPL/crypto dep list is annotated as future blocks above the `[dependencies]` header; each lands in its owning phase.
- `rust-wallet-app/crates/sol-wallet-core/src/lib.rs` — three empty module declarations (`pub mod address; pub mod error; pub mod wallet;`), `pub use error::{Error, Result}`, `#![deny(unsafe_code)]` + `#![warn(missing_docs)]`, one compile-only smoke test (`facade_compiles`). No `pub use solana_sdk::*` re-exports yet — those land in Phase 1 alongside the Wallet keypair.
- `rust-wallet-app/crates/sol-wallet-core/src/error.rs` — placeholder `Error::Placeholder` enum + `Result<T>` alias. Full 21-variant enum lands in Phase 5/6/7 per plan §Phase 6 Task 6.3.
- `rust-wallet-app/crates/sol-wallet-core/src/address.rs` + `src/wallet.rs` — empty doc-only modules. Phase 2 (address) and Phase 1 (wallet) fill them in.
- `rust-wallet-app/crates/sol/Cargo.toml` + `src/main.rs` — `sol` CLI binary skeleton, clap-driven `--help` only. Hidden `placeholder` subcommand prints a Phase 0 notice. The 22 subcommands land in Phase 7 per plan §Phase 7 Task 7.1.
- `rust-wallet-app/Cargo.toml` `members` — added `crates/sol-wallet-core` and `crates/sol`. `crates/sol-wallet-core/` already existed (held the Phase Set Up CHANGELOG); the manifest makes it a real workspace member.
- `rust-wallet-app/Cargo.toml` `[workspace.dependencies]` — added Anza stack (`solana-sdk/program/keypair/signer/message/transaction/instruction/client/rpc-client/compute-budget-program`), SPL (`spl-token/2022/associated-token-account/memo`), crypto (`ed25519-dalek`, `ed25519-bip32`), and Solana-only helpers (`sha3`, `hmac`, `chrono`, `once_cell`, `regex`). `sol-wallet-core` + `sol` workspace deps added so `crates/sol`'s `sol-wallet-core = { workspace = true }` resolves.

### Changed

- Workspace `Cargo.toml` is now wider by 9 dep lines that exist in `[workspace.dependencies]` but are not yet consumed by `sol-wallet-core` or `sol`. The Anza pins will start resolving only when Phase 1 uncomments the relevant block in `sol-wallet-core/Cargo.toml`. Until then `cargo tree -p sol-wallet-core` shows the bare lib only.

### Drift recorded at execution time

- **Anza exact-pin drift (Step 6 deferred to Phase 1).** Plan §Task 0.1 Step 2 specified exact pins for nine Anza crates — including `solana-rpc-client = "=4.2.2"` and `solana-instruction = "=3.5.0"`. On crates.io today, `solana-rpc-client` has no `4.2.2` (only `4.4.0-alpha.3` is published), and the `4.x` line's manifest pins `solana-instruction >=3.4.0, <3.5.0` — incompatible with `=3.5.0`. Following the plan literally produces a workspace that does not resolve. Phase 0 keeps the Anza pins declared in `[workspace.dependencies]` so Phase 1 can pick a compatible set after `cargo tree` shows the constraint graph; the crate itself does not depend on them yet, so the build is green. Phase 1 re-runs plan §Task 0.1 Step 6 (Anza exact-pin verification) on its own build before claiming done.
- **Module placeholder count.** Plan §Task 0.1 Step 3 said `pub mod address; pub mod wallet; pub mod error;`. Delivered exactly. The plan did not ask for `src/{address,wallet,error}.rs` to exist — but the Rust 2021 module resolver requires them once the `pub mod` declaration is present, otherwise `cargo build` errors with `file not found for module`. Three empty doc-only files were added to satisfy the resolver. Each file carries a doc comment naming the phase that fills it in.
- **`crates/sol-wallet-core/` already existed.** Phase Set Up created this directory for `CHANGELOG.md`; Phase 0 turns it into a workspace member. No `mkdir` or `git mv` needed.

