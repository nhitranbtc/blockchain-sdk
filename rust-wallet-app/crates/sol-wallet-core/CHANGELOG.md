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
- v0.1 release cut: branch `sol-wallet-core` → `main`

---

## Phase Set Up — 2026-09-10 — repo plumbing (no crate code)

Branch, tracker vocabulary, and CI gate established before any Rust code. This entry exists per L24; the crate directory holds only this file until Phase 0 lands `Cargo.toml` and `src/`.

### Added

- Integration branch `sol-wallet-core`, cut from `main` at `9b118eb07014639ba1f8e5271be47b3322339bc2`. Every v0.1 task branches from it and PRs back into it; the single exception is the Phase 9 cut PR to `main`.
- Tracker labels `rust-sol-core` (`#c41e3a`) and `rust-sol-cli` (`#1f6feb`). Existing repo-wide `priority/p0`…`priority/p3` reused — no parallel priority scale created.
- Milestone `sol-wallet-core v0.1` (#2). No due date: the gate is the acceptance criteria, not a calendar.
- `.github/workflows/sol-wallet-core-ci.yml` — four jobs (`rust-lint` fmt+clippy, `rust-test`, `rust-deny`, `mobile-check`), triggered on push/PR to `sol-wallet-core` only. Each sol-specific step skips while `crates/sol-wallet-core` has no crate source, so the Phase Set Up no-op PR can go green before Phase 0 exists.

### Changed

- `rust-wallet-app/deny.toml` — `[bans]` now denies `mpl-token-metadata` and `mpl-core`, enforcing Q12 (no Metaplex surface in v0.1). A future `cargo add` that pulls either in fails `cargo deny check` rather than landing unnoticed.

### Notes — plan drift recorded at execution time

Five deltas between the plan text and live repo state, resolved as follows:

1. **`origin/docs/2026-09-08-solana-rust-sdks-deep-dive` does not exist.** Task S.1's note assumed a docs branch holding the research + planning docs. `git ls-remote` shows no such ref; the four Solana documents were untracked in the `main` worktree. They land on `sol-wallet-core` instead.
2. **MSRV pinned to 1.98.1, not 1.89.0.** Task S.5 asked for `1.89.0` (Anza's declared `rust-version`). `rust-wallet-app/rust-toolchain.toml` hard-pins channel `1.98.1` and overrides any toolchain the CI action installs for cargo invocations inside that directory — a `1.89.0` declaration in YAML would be inert. 1.98.1 satisfies the Anza floor.
3. **"Copy the structure of `rust-tron-core-ci.yml`" is stale.** That file lost its fmt/clippy/test jobs in `998cc1fc`, which consolidated them into umbrella `ci.yml`. Umbrella triggers on `main` only, so a PR into `sol-wallet-core` would receive no gate at all if this file deferred to it. The plan's *job list* stands; the "copy tron" pointer does not. fmt and clippy share one runner per the post-`998cc1fc` repo convention rather than splitting into separate `rust-fmt` / `rust-clippy` jobs.
4. **Labels were absent, not pre-existing.** Both created fresh.
5. **Milestone was absent.** Only `tron-v0.1` existed.

### Notes — Task S.6 (branch protection) deferred

Branch protection is available on this repo (`main` already requires the `CI` context). It is not yet applied to `sol-wallet-core`: a required status check that has never produced a run can never turn green, so protection configured before the first CI run would block the very push that delivers the workflow. Sequence is push → first green run → then require the four contexts (`Rust lint (fmt + clippy)`, `Rust test (sol-wallet-core)`, `Rust dep policy (cargo-deny)`, `Mobile compile-only (iOS + Android arm64)`) plus one review. Until then the L13 PAUSE points are the gate, per Task S.6's own soft-gate fallback.
