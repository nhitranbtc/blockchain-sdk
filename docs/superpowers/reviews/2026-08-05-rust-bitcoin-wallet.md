# Rust Bitcoin Wallet Plan — Review Findings

> **Source:** `compound-engineering:ce-doc-review` walkthrough session 2026-08-06.
> **Source plan:** [2026-08-05-rust-bitcoin-wallet.md](2026-08-05-rust-bitcoin-wallet.md) (4437 lines).
> **Personas dispatched:** coherence, feasibility, scope-guardian, security, adversarial (5 in-process).
> **Cross-model peer pass:** skipped (no host model-routing config).
> **Verdict:** architecture-ready, implementation-not-ready.

This file is the canonical record of the doc-review session. Consumed by
`superpowers:writing-plans` (Step 2) as input to the clean plan rewrite.

## Summary

- **Total findings (raw):** 65
- **Post-anchor-50 gate:** ~50 actionable
- **Post cross-reviewer dedup:** 38 unique issues
- **Walkthrough decisions:** 50 applied, 2 deferred, 0 skipped

| Severity | Count |
| -------- | ----- |
| P0       | 11    |
| P1       | 14    |
| P2       | 9     |
| P3       | 4     |

## Walkthrough Decisions

### P0 (must fix)

| # | Title | Section | Decision | Action |
| - | ----- | ------- | -------- | ------ |
| F1 | Duplicate scope 18.5/18.6 vs Tasks 29/34 | Week 5 vs Add-on Tasks | Applied | Delete Tasks 29 + 34 entirely |
| F2 | `bitcoind.esplora_url()` does not exist | Task 15:2282 | Applied | Use `bdk_testenv::TestEnv` instead |
| F3 | `regtest-tests` feature never declared | Task 15 | Applied | Add `[features] regtest-tests = []` to `bitcoin-wallet-core/Cargo.toml` |
| F4 | Task 13 wrong Esplora trait import | Task 13:2126-2140 | Applied | Change to `use bdk_esplora::EsploraAsyncExt;` + gate `async` feature on `bdk_esplora` |
| F5 | Argon2 calibration drift Step 1 vs Step 2 | Task 30 | Applied | Use 256 MiB / t=10 / p=4 (Step 1 spec); update Step 2 to match |
| F6 | Mnemonic plaintext-on-disk is v0.1 default | Global Constraint + Task 16 | Applied | Promote Task 30 (encrypted mnemonic) to v0.1 core; `wallet create` refuses plaintext without `--plaintext --yes` |
| F7 | `first_private_key` returns raw-key Signer | Task 18.5 + Task 29 | Applied | Narrow to `sign_message_bip137(text: &str) -> Signature` API |
| F8 | Watch-only descriptor guard bypassable | Task 32 | Applied | Drop `has_wildcard()` check; walk parsed descriptor tree; case-insensitive prefix match against `xprv/XPRV/yprv/YPRV/zprv/ZPRV/tprv/TPRV` set |
| F9 | BIP-137 length prefix 1 byte not varint | Task 29:3806 | Applied | Replace with Bitcoin varint encoding (`1B` for <253, `0x4c+u16 LE` for ≤65535, `0x4d+u32 LE` otherwise); add cross-verification test against Bitcoin Core `signmessage` |
| F10 | Goal lacks v0.1 consumer | Plan header | Deferred | Add to Open Questions |
| F11 | UniFFI binding path missing | Cross-cutting gap | Deferred | Add to Open Questions |

### P1 (high impact)

| # | Title | Section | Decision | Action |
| - | ----- | ------- | -------- | ------ |
| F12 | Task 9 sync uses bare `FullScanRequest::new()` | Task 9:1733-1749 | Applied | Replace with `guard.start_full_scan()` pattern from upstream `bdk_wallet/examples/esplora_blocking.rs`; acquire lock briefly, drop before `.await` |
| F13 | `FeeRate::from_sat_per_vb` Result not handled | Task 9 + Task 15 + Task 11 test | Applied | Add `.unwrap()` (or `?` propagation) at every call site |
| F14 | Task 32 uses BDK 1.x `Wallet::new_single` | Task 32:4228 | Applied | Replace with `BdkWallet::create(d, d).network(...).create_wallet(&mut conn)?` |
| F15 | Task 17 `load_wallet` calls undefined `detect_network_from_dir` | Task 17 Step 6 | Applied | Define via `network.txt` sidecar file written by Task 17 Step 7 |
| F16 | Global Constraint "no unsafe" contradicts Task 1.5 | Global Constraints vs Task 1.5 | Applied | Update Global Constraint line 25 to acknowledge `Secret::into_inner` exception with rationale |
| F17 | Sub-task numbering non-monotonic | W5 task ordering | Applied | Reorder W5 tasks to monotonic numeric: 17 → 17.5 → 17.6 → 18 → 18.5 → 18.6 → 18.7 |
| F18 | Coverage matrix claims "core" but stories live in sub-tasks | Coverage matrix vs plan summary | Applied | Update plan summary to "25 core + 5 sub-tasks (17.5/17.6/18.5/18.6/18.7) + 7 add-on tasks (26-32 after F1 deletion)" |
| F19 | `atomic_write` never called at mnemonic write site | Task 1.5 vs Task 16/17.6 | Applied | Wire `atomic_write` + `OpenOptions::create_new(...).mode(0o600)` + `refuse_world_writable` at mnemonic write + load sites |
| F20 | No certificate pinning for Esplora/Electrum | Task 7 + Task 8 | Applied | Add `--esplora-pinned-pubkey` + `--electrum-pinned-pubkey` flags + rustls SPKI pinning + SECURITY.md disclosure |
| F21 | External Signer trait accepts arbitrary 32-byte hashes | Task 28 | Applied | Take typed `Sighash` wrapper or `MessageClass` enum (Transaction/TapScript/Bip137Message) |
| F22 | Passphrase via CLI leaks via ps/top | All CLI surface | Applied | TTY-only entry; reject `--passphrase` CLI flag with build error |
| F23 | Dust MIN_RELAY_FEE = 3000 sat/kvB | Task 26 | Applied | Set to 1000; accept constructor param to track network policy |
| F24 | No top-level threat model | Task 23 | Applied | Add Task 0 producing `docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md` (assets, adversaries, trust boundaries, abuse cases) |
| F25 | PSBT signed without per-output review | Task 12 + 13 + 17 | Applied | Decode + display every output (address, amount, fee rate, change) + interactive confirm unless `--yes` |
| F26 | BDK version pins unverified | Workspace Cargo.toml | Applied | Add fallback chain (if `bdk_wallet 3.1` fails, fall back to 2.1 + 0.22); require Task 31 spike compatibility matrix |
| F27 | Task 28 external Signer trait zero v0.1 consumers | Task 28 | Applied | Drop from v0.1; re-add in v0.2 when UniFFI consumer concrete |
| F28 | Task 32 WatchOnlyWallet no user story | Task 32 | Applied | Delete Task 32; defer watch-only to v0.2 |
| F29 | Task 27 `chain::explorer` no consumer | Task 27 | Applied | Drop Task 27 from v0.1 |
| F30 | Task 30 encryption mislabeled v0.2/v1.1 | Task 30 vs Global Constraint | Superseded by F6 | (F6 promotes Task 30 to v0.1 core) |
| F31 | MSRV 1.85 unexplained | Plan header | Applied | Document which crate requires 1.85 and why |
| F32 | MIT license without copyright holder | Plan header | Applied | Add explicit `Copyright (c) [year] [holder]`; require CLA before merge |
| F33 | 7-week timeline no buffer for BDK spike | Plan header | Applied | Add MVP fallback: if spike reveals >5 API edits needed, trim to MVP (Tasks 1-9 + minimal CLI) for v0.1.1; rest defer to v0.1.2 |
| F34 | Coverage claim rests on vacuous test assertions | Story Coverage matrix | Applied | Add AC test step per Task 17+ story (negative cases, error paths, specific values); replace tautological asserts (`is_ok() || is_err()`) with concrete expectations |
| F35 | 25-task scope vs MVP unmotivated | Plan header | Applied | Trim to MVP (Tasks 1-9 + minimal CLI), ship faster |

### P2 (worth-noting)

| # | Title | Section | Decision | Action |
| - | ----- | ------- | -------- | ------ |
| F36 | CLI smoke test only exercises `--help` | Task 18 | Applied | Replace with behavior test: `btc wallet create --name smoke-$$ --network regtest --type native-segwit` against tempdir; assert exit 0 + mnemonic line + file written owner-only |
| F37 | Tasks 17.5/17.6 are post-hoc patches | Tasks 17.5/17.6 | Applied | Re-tag as core; add AC-tracking step to Task 17 review criteria |
| F38 | Task 26 dust math duplicates BDK built-in | Task 26 | Applied | Drop Task 26 entirely (BDK `dust_limit()` already wired in `build_tx.finish()`) |
| F39 | Signer name collision (struct vs trait) | Task 4 vs Task 28 | Applied | Verify F27 Task 28 deletion also removes `pub use sign_external::Signer` re-export |
| F40 | `tests/vectors.rs` listed but never created | File Structure line 60 | Applied | Remove `vectors.rs` from File Structure |
| F41 | Task 17 `commands/mod.rs` wiring skipped | Task 17 Step 5 | Applied | Add explicit `mod send; mod tx; mod fee; mod config;` declarations before Step 5 |
| F42 | Task 17.6 import path wrong | Task 17.6:2782 | Applied | Replace with `bdk_wallet::bitcoin::FeeRate::from_sat_per_vb(rate)` + add `use bdk_wallet::bitcoin::FeeRate;` |
| F43 | Task 5 `p2wpkh.unwrap()` violates no-unwrap rule | Task 5:1160 | Applied | Return `Result<ScriptBuf, Error>` from `p2wpkh`/`p2pkh`/`p2tr_key_path`; propagate `wpubkey_hash()` error |
| F44 | Task 9 `derive_xprv` called twice | Task 9:1660, 1662 | Applied | Drop unused first call; rename second to `let xprv = ...; let xprv_str = xprv.to_string();` |
| F45 | Coin-selection enum names unverified | Task 18.6 | Applied | Task 31 spike verifies exact names (`BranchAndBoundCoinSelection` vs `BranchAndBound`; `OldestFirstCoinSelection` vs `OldestFirst`) against BDK 3.1 |
| F46 | `bip32 0.6` dep existence unverified | Workspace Cargo.toml:193 | Applied | `cargo search bip32 --limit 5` verification in Task 1; add fallback version |
| F47 | Signer struct no explicit ZeroizeOnDrop | Task 4 | Applied | Wrap `keypair` in `Secret<Keypair>` using Task 1.5 wrapper |
| F48 | `atty` crate unmaintained | Task 18.7 | Applied | Replace with `std::io::stdin().is_terminal()` + `use std::io::IsTerminal;`; drop `atty` dep |
| F49 | Wallet create echoes mnemonic to STDOUT | Task 16:2448 | Applied | Write to 0o600 file via `atomic_write`; print only path; require `--show-mnemonic` for STDOUT echo |
| F50 | BIP-137 omits recovery flag byte | Task 29:3812-3816 | Applied | Prepend `27 + rec_id + 4` to compact signature; emit full 65-byte base64 |

### P3 (low-signal)

| # | Title | Section | Decision | Action |
| - | ----- | ------- | -------- | ------ |
| F51 | `zeroize` crate preemptive | Workspace Cargo.toml | Resolved by F6 | (Task 30 promotion justifies dep) |
| F52 | BDK vs `rust-bitcoin` choice not defended | Plan preamble | Applied | Document BDK trade-off: saves 1000s LOC vs writing wallet from scratch, at cost of API lock-in |
| F53 | Task 1.5 `unsafe_code` allow scope | Task 1.5:418-429 | Applied | Move `#[allow(unsafe_code)]` to `unsafe` block; add `cargo geiger` CI check; doc exception in crate root |

## Open Questions (Deferred)

The following decisions require further input before plan rewrite:

- **F10: v0.1 consumer path.** Plan produces a library but no v0.1 consumer exists. Options:
  - Ship UniFFI Swift binding as part of v0.1 (adds Task 0 + binding scaffolding).
  - Reconsider deferred HTTP server; ship v0.1.5 Rust daemon.
  - Accept library-only ship; document as foundational layer for v0.2 umbrella.
- **F11: UniFFI binding scaffolding.** Original goal is to replace Swift code in Tangem iOS. Without UniFFI consumer, Rust library cannot be consumed by Tangem iOS. Options:
  - Add Task 0 to scaffold UniFFI proc-macro + UDL file + Swift/C++/Kotlin glue generation.
  - Defer to v0.2 umbrella integration.

## FYI / Residual Concerns (Anchor 50, Not Actioned)

These surfaced but were routed to FYI; not part of the Apply set:

- BDK version pins unverified against current published versions (`bdk_wallet 3.1` + `bdk_esplora 0.22` + `bdk_file_store 0.15` + `bip32 0.6`). Book of BDK example uses `bdk_wallet 2.1.0`.
- Self-review placeholder scan claim contradicts many `todo!()` blocks in code samples.
- Integration-testnet CI job will be no-op until test wallet funded from faucet.
- Version-label confusion: v0.1 / v0.2 / v1 / v1.1 used interchangeably across the plan.
- mlock / secure-memory pools for future daemon mode (deferred to v0.2).
- `atomic_write` does not fsync parent directory after rename — incomplete crash safety on COW/NFS filesystems.
- `load_wallet` does not verify on-disk wallet network matches CLI `--network` flag (UX failure, not theft).
- `load_wallet` hardcodes `NativeSegwit` regardless of wallet's stored `address_type` — Taproot wallets silently fail to find UTXOs on reload.
- Argon2 vs scrypt choice not defended.
- `crates.io` publish policy + yank strategy for post-publish CVE response.
- `cargo geiger` only audits build-time dep tree, not runtime FFI risk surface.

## Persona Coverage

| Persona | Findings | Severity Mix |
| ------- | -------- | ------------ |
| coherence-reviewer | 13 | 2 P0, 4 P1, 5 P2, 2 P3 |
| feasibility-reviewer | 12 | 3 P0, 4 P1, 5 P2 |
| scope-guardian-reviewer | 9 | 1 P0, 4 P1, 3 P2, 1 P3 |
| security-lens-reviewer | 15 | 4 P0, 6 P1, 4 P2, 1 P3 |
| adversarial-document-reviewer | 16 | 3 P0, 5 P1, 4 P2, 2 P3 |

**Cross-reviewer duplicates (resolved during synthesis):**

- Tasks 18.5/18.6 vs 29/34 scope overlap — flagged by scope-guardian + coherence; consolidated to F1.
- Task 9 `derive_xprv` called twice — flagged by coherence + feasibility; consolidated to F44.

## Step 2 (writing-plans) Recommendations

When `superpowers:writing-plans` re-emits the plan, prioritize in this order:

1. **Drop deleted tasks first** (F1: Tasks 29 + 34, F27: Task 28, F28: Task 32, F29: Task 27, F38: Task 26). Plan summary count recalibration (F18).
2. **Apply mechanical fixes (safe_auto-class items)** that have no design choice: F4, F5, F13, F40, F41, F42, F44, F46, F48, F50.
3. **Apply substantive code rewrites** (gated_auto-class): F8 (watch-only guard), F9 + F50 (BIP-137 fix pair), F12 (FullScanRequest), F14 (BDK 3.x Wallet::create), F15 (network.txt sidecar), F19 (atomic_write wiring), F21 (typed Sighash), F25 (PSBT review), F43 (Result propagation).
4. **Apply policy / structural additions**: F6 (Task 30 promotion), F24 (threat model), F16 (Global Constraint update), F17 (sub-task reorder), F22 (TTY-only passphrase), F23 (MIN_RELAY_FEE 1000), F34 (AC test step), F35 (MVP scope justification).
5. **Apply documentation updates**: F31 (MSRV rationale), F32 (copyright holder), F52 (BDK trade-off).
6. **Verify deletions propagate**: F18 (plan summary count) depends on F1, F27, F28, F29, F38; F39 depends on F27.

## Cross-Model Pass

**Skipped.** This session did not have a configured cross-model routing policy for the activated trio (adversarial + security). Per the skill, agreement promotion requires artifact `independence_verified: true`, which was not available. Findings from the in-process reviewers stand as documented; cross-model corroboration was not produced.

If a future session has cross-model routing configured, the activated trio (`adversarial-document-reviewer`, `security-lens-reviewer`) plus a `whole-doc` reviewer should be re-run with explicit targets.
