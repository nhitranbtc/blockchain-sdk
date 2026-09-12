---
title: sol-wallet-core Phase 9 security audit (ship-gate)
tracker: https://github.com/nhitranbtc/blockchain-sdk/issues/563
plan: ../superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md
deep-dive: ../wallets/2026-09-08-solana-rust-sdks-deep-dive.md
date: 2026-09-12
status: open
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening
---

# sol-wallet-core Phase 9 — Security audit + hack-scenario review

## Intro

Phase 9 closes the prior-session gap ("why in sol-wallet-core don't support create wallet?") by surfacing `generate_12_word_english` as a library-level public API and adding an end-to-end devnet example binary. This audit walks the Phase 9 plan section (lines 1749-1856 of [the plan](../superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md)) for security/hack-scenario threats.

The audit covers only Phase 9 — library changes are minimal (one-line `pub use` re-export) and `generate_12_word_english` was already audited in Phase 8.1 ([companion audit](2026-09-11-sol-wallet-core-phase8-security-review.md)). The example binary introduces 2 🔴 + 3 🟠 + 3 🟡 + 2 🔵 findings; all are example-side, none are library-side.

## TL;DR

The two 🔴 findings (P9-1 plaintext password + P9-2 mnemonic to stdout) are easy fixes at the example layer — env-var password + `eprintln!` mnemonic. The 🟠 findings (C-P9-1 defense-in-depth parity, P9-4 RPC URL warning, P9-8 CI orphan state) require README + UX changes. None of the 8 ship-gate items block the v0.1 release cut — they are Phase 9 PR blockers, not Phase 10 blockers.

## Drift scan (L13 step 4a)

| Cited | Found | State |
|-------|-------|-------|
| `rust-wallet-app/crates/sol-wallet-core/src/lib.rs` | ✅ exists | OK |
| `rust-wallet-app/crates/sol-wallet-core/src/ffi_mnemonic.rs` | ✅ exists | OK (Phase 8.1, PR #562, commit `f2ebeca2`) |
| `rust-wallet-app/crates/sol-wallet-core/examples/` | ❌ not yet | Phase 9 creates it — expected |
| `rust-wallet-app/crates/sol-wallet-core/CHANGELOG.md` | ✅ exists | OK |
| `rust-wallet-app/crates/sol-wallet-core/README.md` | 🟡 present (single-line) | Phase 9 Task 9.3 expands to "Quick Start" section |
| `https://api.devnet.solana.com` | ✅ DNS-resolves | OK |
| `https://explorer.solana.com/?cluster=devnet` | ✅ valid | OK |
| `tokio::runtime::Runtime` for sync block_on | 🟡 workspace dep | OK if `[dev-dependencies]` declared |

## Cross-cutting controls

| ID | Threat | Severity | Required control | Ships in |
|----|--------|----------|------------------|----------|
| C-P9-1 | Example code introduces weaker security patterns than the audited library | 🟠 high | Example must NOT use plaintext password, plaintext mnemonic to stdout, or skip `FileWalletStorage`'s 0o600 enforcement | Phase 9.2 |
| C-P9-2 | Devnet airdrop becomes a real-funds path via env-var misuse | 🟠 high | Default `SOL_RPC_URL` to `api.devnet.solana.com`; reject override pointing at non-devnet host with loud warning; require explicit `RUN_SOL_DEVNET_AIRDROP=1` opt-in | Phase 9.2 |

## Per-phase controls (Phase 9)

| ID | Finding | Severity | Detail | Required control | Ships in Task |
|----|---------|----------|--------|------------------|---------------|
| P9-1 | Hardcoded `"example-password"` in example pseudocode | 🔴 critical | Plaintext password in version-controlled file; users copying the pattern land it in shell history + git history | Read password from `std::env::var("EXAMPLE_WALLET_PASSWORD")` with explicit `eprintln!` warning if unset + default to `change-me-<timestamp>` | Phase 9.2 Step 1 |
| P9-2 | Mnemonic printed to `println!` (stdout) | 🔴 critical | stdout persists in shell scrollback, CI logs, terminal multiplexers (tmux/screen); mnemonic in clear is loss-of-funds path | Print mnemonic to `stderr` (separate stream) OR write to `0o600` file in `data_dir/mnemonic.txt` with explicit user prompt + offer to print | Phase 9.2 Step 1 |
| P9-3 | `data_dir` from `argv[1]` unvalidated | 🟡 medium | Path traversal: `cargo run --example -- /etc` → `FileWalletStorage::open("/etc")` creates dir at `/etc/sol-wallet-example` | Validate `data_dir` is absolute path + reject symlink parent + reject if path canonicalizes to system dir | Phase 9.2 Step 1 |
| P9-4 | `SOL_RPC_URL` override unverified | 🟠 high | Default `devnet` is safe; user-overridden to `mainnet-beta` is fine for non-airdrop ops but `request_airdrop` host allowlist would reject → confusing user error | Add `if rpc_url host != api.devnet.solana.com && env::var("RUN_SOL_DEVNET_AIRDROP").is_ok() { eprintln!("WARN: airdrop to non-devnet cluster"); }` | Phase 9.2 Step 1 |
| P9-5 | README Quick Start hardcodes `devnet` URL + env var names | 🟡 medium | Docs cite cluster without linking to `sol config set-cluster`; risk of user running example with prod envs in same shell | Add explicit "devnet only" banner + link to Phase 7 cluster switching; list env vars in a table | Phase 9.3 Step 2 |
| P9-6 | `tokio::runtime::Runtime::new()` leaks background threads on early-exit | 🔵 low | Airdrop failure path leaves runtime orphaned | Use `tokio::runtime::Builder::new_current_thread().enable_all().build()?` + explicit `runtime.shutdown_timeout(Duration::from_secs(2))` in a Drop guard | Phase 9.2 Step 1 |
| P9-7 | `request_airdrop(1_000_000_000)` may exceed devnet per-request cap (5 SOL historic, varies) | 🔵 low | Airdrop may succeed with 1 SOL on devnet (1 SOL = 1e9 lamports) but rate-limit hit on retry | Drop to `500_000_000` (0.5 SOL) + handle `Rpc { code: -32003 }` (airdrop limit) with retry-after print | Phase 9.2 Step 1 |
| P9-8 | Example runs in CI without `RUN_SOL_DEVNET_AIRDROP` → leaves orphan encrypted blob on disk | 🟠 high | CI scratch space fills up + persistent state from failed runs | Wrap example in `if env::var("CI").is_ok() { eprintln!("SKIP: CI env"); return Ok(()); }` early-return | Phase 9.2 Step 1 |

## Minimum ship-gate checklist (8 items)

- [ ] **P9-1** No plaintext password anywhere in `examples/create_wallet_devnet.rs` — env var only
- [ ] **P9-2** Mnemonic writes to `0o600` file in `data_dir` OR stderr, never stdout
- [ ] **P9-3** `data_dir` path validation (absolute + non-symlink + non-system path)
- [ ] **P9-4** Non-devnet `SOL_RPC_URL` + airdrop opt-in prints loud `WARN:` banner
- [ ] **P9-5** README Quick Start marks devnet-only + lists env vars in table
- [ ] **P9-8** Example early-returns in CI env without writing orphan state
- [ ] **C-P9-1** All Phase 6 audit controls (AAD binding, 0o600 chmod, AAD tamper detect, zeroize) inherited by example
- [ ] **C-P9-2** Airdrop requires explicit `RUN_SOL_DEVNET_AIRDROP=1` env opt-in (default = no RPC call)

## Out-of-scope (deferred items from plan)

- Production example (mainnet) — not in v0.1; only devnet
- Hardware wallet integration (Ledger) — V0.2+
- Multi-account bulk create — Phase 9 ships single-wallet create only
- Mnemonic strength selector (12/15/18/21/24 words) — Phase 9 hardcodes 12 (Phantom UX)

## References

- **Issue tracker:** [#563](https://github.com/nhitranbtc/blockchain-sdk/issues/563)
- **Plan:** [docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md](../superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md) — Phase 9 lines 1749-1856
- **Deep-dive:** [docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md](../wallets/2026-09-08-solana-rust-sdks-deep-dive.md)
- **Phase 6 audit:** [2026-09-11-sol-wallet-core-phase6-security-review.md](2026-09-11-sol-wallet-core-phase6-security-review.md) — wallet persistence controls
- **Phase 7 audit:** [2026-09-11-sol-wallet-core-phase7-security-review.md](2026-09-11-sol-wallet-core-phase7-security-review.md) — CLI surface controls
- **Phase 8 audit:** [2026-09-11-sol-wallet-core-phase8-security-review.md](2026-09-11-sol-wallet-core-phase8-security-review.md) — FFI cdylib controls
- **Source files affected by Phase 9 controls:**
  - `rust-wallet-app/crates/sol-wallet-core/src/lib.rs` (P9-1 / P9-2 fix site — re-export)
  - `rust-wallet-app/crates/sol-wallet-core/src/ffi_mnemonic.rs` (P9-callsite — `generate_12_word_english` consumer)
  - `rust-wallet-app/crates/sol-wallet-core/src/wallet_manager.rs` (P9-3 — `FileWalletStorage::open` invoker)
  - `rust-wallet-app/crates/sol-wallet-core/src/chain/account.rs::request_airdrop` (P9-4 / P9-7 — host allowlist + rate-limit hint)
  - `rust-wallet-app/crates/sol-wallet-core/examples/create_wallet_devnet.rs` (new in Phase 9)
  - `rust-wallet-app/crates/sol-wallet-core/CHANGELOG.md` (Phase 9 entry)
  - `rust-wallet-app/crates/sol-wallet-core/README.md` (Phase 9 Quick Start)
