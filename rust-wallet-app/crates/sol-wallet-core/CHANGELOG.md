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

