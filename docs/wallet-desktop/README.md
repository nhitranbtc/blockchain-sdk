# Flutter Desktop UI for `rust-wallet-app` (`wallet-desktop`)

Cross-platform Flutter desktop host layer for the **`rust-wallet-app`** umbrella (sibling role to `rust-wallet-app/crates/btc` CLI — same engine, different skin). v1 wraps the existing `btc` binary directly; future revisions may drive the umbrella via UniFFI / `chain-traits` (Phase 3 per ADR 0001).

Project directory: `wallet-desktop/` (renamed from `flutter-btc-wallet/` to drop BTC coupling ahead of v0.2 multi-chain). Bundles `btc` per-arch, spawns it via `dart:io` `Process.start`, parses `--json` stdout.

**Status:** v1 spec + plan committed (design §11 added for umbrella alignment). Implementation not started — see [Plan §Task Index](./plan.md#task-index) for the 26-task breakdown.

## Documents

- [`design.md`](./design.md) — Full design spec (architecture, data flow, components, security model, CI, §11 umbrella alignment)
- [`plan.md`](./plan.md) — Implementation plan (26 tasks, TDD per task)

Canonical locations:

- `docs/superpowers/specs/2026-08-15-flutter-btc-wallet-design.md`
- `docs/superpowers/plans/2026-08-15-flutter-btc-wallet.md`

## Relationship to rust-wallet-app

Per `docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md` §3.1, the umbrella defines a HOST LAYER (currently CLI only) above per-chain crates. This Flutter UI is a second host — same engine (`bitcoin-wallet-core`), different surface.

**Deviation from umbrella §3.1:** the umbrella spec says host layer lives "not in this repo" (iOS/Android/CLI external). The Flutter UI is in this repo (release-coupling rationale). Tracked as ADR-candidate; treated as in-repo host-layer deviation until amended.

Layer diagram:

```text
┌──────────────────────────────────────────┐
│ HOST LAYER (this repo — in-repo deviation)│
│   rust-wallet-app/crates/btc  (CLI)     │  ← existing v0.1
│   wallet-desktop/              (UI)      │  ← this project (v1 BTC-only)
└──────────────────────────────────────────┘
                  │
                  ▼
┌──────────────────────────────────────────┐
│ PER-CHAIN CRATES                          │
│   bitcoin-wallet-core  (v0.1 done)       │
│   ethereum-wallet-core  (v0.2 planned)   │
│   solana-wallet-core    (v0.2 planned)   │
└──────────────────────────────────────────┘
```

v1 ships BTC-only (matches the umbrella's per-chain rollout order). v0.2+ adds ETH + SOL; design §11.2–§11.4 sketch migration paths to `ChainInvoker` interface + `MnemonicSession` (single mnemonic → N chains). v1.0 (per ADR 0001) replaces software signing with hardware `Signer` trait via UniFFI — UI secret-handling contract unchanged.

## Source-of-truth

- Wrapped binary: `rust-wallet-app/crates/btc/`
- Wrapped library: `rust-wallet-app/crates/bitcoin-wallet-core/`
- Umbrella arch: `docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md`
- BTC design spec: `docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md`
- BTC user stories: `docs/wallets/2026-08-05-btc-wallet-user-stories.md`
- ADR signing model: `docs/wallets/2026-08-05-adr-0001-signing-model.md`
- Migration table: `design.md` §12

## Issue tracking

26 tasks → GitHub issues #149–#174 in `nhitranbtc/blockchain-sdk`, labeled `ui` + `P2`.
