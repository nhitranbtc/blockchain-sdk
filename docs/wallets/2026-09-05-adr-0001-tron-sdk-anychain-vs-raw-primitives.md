# ADR-0001: TRON SDK Stack — `anychain-tron` + `anychain-kms` vs Raw Primitives

**Date:** 2026-09-05
**Status:** Accepted → **Amended 2026-09-06** (see `## 2026-09-06 Revision` below)
**Supersedes:** 2026-09-04 raw-primitives decision (informal)
**Scope:** `tron-wallet-core` v0.1 wire-format + HD + signing layer
**Author:** deep-dive research session (2026-08-27 → 2026-09-05); amended 2026-09-06 per issue #540 + grill Round-1 (plan `2026-09-05-tron-wallet-core-v0.1-anychain.md`)
**Companion:** `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md`

## Context

`tron-wallet-core` v0.1 needs a Rust stack covering: T-base58check address derivation, protobuf `Transaction` envelope, TRC-20 ABI encoding, Stake 2.0 contract builders, BIP-39/32 HD wallet, secp256k1 signing, TronGrid JSON-RPC client. All under mobile constraints (no Docker, no system cert store, ~3-5 MB binary budget).

Three candidate stacks were considered across two passes:

| Pass | Date | Decision | Rationale at the time |
|---|---|---|---|
| 1 | 2026-09-04 | Raw `prost` + `reqwest` + `k256` + `bs58` + `sha3` | Avoid anychain umbrella scope; hand-roll ~1000 LOC of protobuf + Keccak256 + base58check + ABI encoder |
| 2 | 2026-09-05 | `anychain-tron 0.2.14` + `anychain-kms 0.1.23` | Trade 250 LOC RPC glue for ~1000 LOC saved on protobuf + Keccak256 + base58check + ABI + Stake 2.0 contract builders |

## Decision

**Adopt `anychain-tron 0.2.14` + `anychain-kms 0.1.23`** as the primary TRON stack. `anychain-core 0.1.8` for shared traits + crypto utilities.

## Considered Alternatives

### A. Raw primitives (`prost` + `reqwest` + `k256` + `bs58` + `sha3`)

**Pros:**
- Zero upstream bus-factor risk (we own all code paths)
- No multi-chain umbrella scope creep
- Smaller transitive dep tree

**Cons:**
- ~1000 LOC of hand-rolled protobuf encode/decode for `core/Tron.proto` + 13 `core/contract/*.proto` files
- Hand-roll Keccak-256 base58check T-address derivation (small but error-prone)
- Hand-roll TRC-20 ABI encoder (EIP-20 compatible) — Solidity function selectors, type encoding
- Hand-roll 13 contract builders (TRX transfer, TRC-20 transfer/approve, Stake 2.0 freeze/unfreeze/delegate/cancel/withdraw, witness vote, withdraw vote)
- Maintenance burden for protobuf schema evolution when TRON ships new contract types

**Rejected because:** scope underestimation. The 13 contract builders + protobuf schema maintenance = recurring cost larger than the bus-factor risk discount.

### B. `tronz 0.5.2` (`throgxyz/tronz`)

**Pros:**
- 52 stars, 9-crate monorepo, MSRV 1.91.1, sponsor CatFee.IO
- Includes gRPC + JSON-RPC clients

**Cons:**
- gRPC-only binding (no JSON-RPC), wrong transport for TronGrid HTTP-only
- MSRV 1.91.1 blocker if workspace pinned differently
- 9-crate bloat outweighs ~1000-line code saving
- Single primary maintainer + dependabot

**Rejected because:** gRPC-only binding wrong transport; MSRV incompatibility risk; same single-maintainer trust issue as anychain-tron but with worse dependency surface.

### C. `tronic 0.6.1` (`39george/tronic`)

**Pros:** Alloy-inspired; `alloy_sol_types` for TRC-20 ABI.

**Cons:** gRPC-only (JSON-RPC WIP per maintainer); single maintainer.

**Rejected because:** gRPC-only binding wrong transport; JSON-RPC still WIP.

### D. `0xcregis/anychain` umbrella (full)

**Pros:** 252 stars, 10-chain umbrella, 8 contributors, Rust 1.98.0 toolchain, MIT.

**Cons:** Multi-chain scope too broad for TRON-only v0.1; pulls deps for BTC/ETH/Solana/Filecoin/Ripple/Polkadot/TON/Neo.

**Rejected because:** umbrella scope = bloat. We don't need BTC/EVM utilities inside the TRON crate.

### E. `anychain-tron` (subcrate only) — CHOSEN

**Pros:**
- TRON-specialized subcrate: 25 wire-format features (T-base58check, protobuf, 13+ contract builders, ABI)
- Pairs with `anychain-kms` for BIP-39/32 + secp256k1 signing
- Pairs with `anychain-core` for shared traits (`Address`, `PublicKey`, `Transaction`, `Network`, `Format`)
- MIT OR Apache-2.0 license
- 256/month DL, 107/week DL = real adoption signal

**Cons (accepted risks):**
- **Single-maintainer risk** (mitigation: see Risks below)
- **2 known bugs requiring workarounds** (mitigation: see Risks below)
- **MSRV 1.98.1** anychain workspace pin (mitigation: see MSRV below)

## Risks Accepted

### R1. Single-maintainer trust (Q3 finding, 2026-09-05 audit)

`0xcregis/anychain` author diversity over trailing 12 months:

- `anychain-tron/`: **1 author (`loki-cmu`), 3 commits** — single-maintainer trust, worse than rejected `tronic` and `tronz`
- `anychain-kms/`: **2 authors (`Brian`, `loki-cmu`), 8 commits** — marginal
- umbrella total: **2 distinct authors**

**Mitigation:** Pin `anychain-tron = "=0.2.14"` (exact, not `^`). Before v0.1 ships, vendor `anychain-tron` + `anychain-kms` source into `rust-wallet-app/crates/anychain-vendored/` (fork-with-citation pattern, NOT public fork). Cite per file. Track upstream for security fixes — apply manually to vendored copy.

### R2. Two known bugs requiring workarounds (Q2)

**Bug 1:** `anychain-tron::TronTransaction::to_transaction_id()` returns single SHA-256, not the chain-spec `SHA256(SHA256(raw_bytes))` double-hash. Display txid would be wrong.

**Workaround:** In `tron-wallet-core::tx::sign`, compute `let txid = Sha256::digest(&Sha256::digest(&tx.to_bytes()))` manually. Add unit test asserting this matches TronScan explorer txid for known tx.

**Bug 2:** `trx::build_contract` formats `type_url` via `{:?}` Debug derive. Works today but fragile if anychain changes internal Debug impl.

**Workaround:** When serializing for broadcast, encode `type_url` manually via `hex::encode` instead of relying on Debug-derived string.

**Mitigation:** File upstream issues with reproduction + proposed fix. Pin exact version. Unit tests assert txid matches `SHA256(SHA256(raw_bytes))` so any upstream "fix" gets caught in CI.

### R3. MSRV 1.98.1 forced by anychain umbrella (Q1)

Anychain workspace pins `rust-toolchain = "1.98.1"`. Workspace MSRV impact.

**Mitigation:** Pin `rust-toolchain.toml` to 1.98.1. Advertise MSRV 1.94 ONLY after `cargo +1.94 check -p anychain-tron -p anychain-kms` passes end-to-end. If 1.94 check fails, advertise 1.98.1 honestly — no fake MSRV.

### R4. Mainnet spike deferred (Q4)

V0.1 ships with Local + Nile PASS evidence only. No real-value mainnet smoke.

**Mitigation:** Gate v0.1 release on ONE mainnet self-send — $0.001 USDT to self (recipient == sender), real value, real network. Without this, V1-V10 PASS evidence is "looks like real network" not "real network". Add to acceptance criteria NOW, not post-Phase 4. Mainnet self-send uses pre-check audit hook (refuse if recipient != operator_wallet) + `RUN_TRON_MAINNET=1` env gate.

## Consequences

### Positive

- ~1000 LOC saved vs raw primitives (protobuf encode + Keccak256 + base58check + 13 contract builders)
- All 13 Stake 2.0 contract builders available out-of-box (`trx::build_freeze_balance_v2_contract` etc.)
- BIP-39/32 + secp256k1 signing via single `anychain-kms` crate
- Shared traits `Address`, `PublicKey`, `Transaction`, `Network`, `Format` enable future cross-chain FFI parity (Polygon ETH/Tron already need shared shape)

### Negative

- MSRV bump to 1.98.1 affects workspace `rust-toolchain.toml`
- Bus-factor 1 for `anychain-tron` — vendor-before-ship mandatory
- 2 known bugs require caller-side workarounds (R2)
- Mainnet smoke deferred but mandatory pre-release gate (R4)

### Implementation

- Pin `anychain-tron = "=0.2.14"`, `anychain-kms = "=0.1.23"`, `anychain-core = "=0.1.8"` in workspace `Cargo.toml`
- Pin `rust-toolchain.toml` to 1.98.1
- Add `tron-wallet-core::tx::sign` dual-SHA256 workaround + unit test
- Add vendor plan as V0.1 spike before release

## 2026-09-06 Revision

### Trigger

Issue #540 surfaced in production during v0.1 integration testing on Nile testnet: `anychain-tron 0.2.14`'s `Raw` proto serialization emits a spurious leading `0x01` byte before the canonical varint for `fee_limit >= 2^27` (observed: `130_000_000` encodes as `90 01 80 c9 fe 3d` instead of canonical `90 80 c9 fe 3d`). TronGrid's Java gateway reads `fee_limit = 1` and trips on unknown field 16, returning `{"Error": "class java.lang.NullPointerException : null"}` for every signed TRC-20 broadcast. Reproduction verified via raw curl against Nile mainnet endpoint + Python protobuf 7.36.1 cross-check. See `gh issue view 540` for full observed/expected bytes.

The bug combines with the bus-factor accepted risk recorded above (R1: 1 author, 3 commits trailing 12 months for `anychain-tron`): upstream fix turnaround is unknown, and Q4 mainnet smoke gate plus the entire live-broadcast path cannot close without it.

### Revised Decision (flip)

The 2026-09-05 mitigation note under R1 ("Before v0.1 ships, vendor `anychain-tron` + `anychain-kms` source into `rust-wallet-app/crates/anychain-vendored/`") was previously listed as a pre-release task. The 2026-09-06 amendment **promotes vendoring from pre-release-task to primary decision**, executed at Phase 0 Task 0.2 instead of deferred to v0.1 release week.

**Vendor location:** `rust-wallet-app/crates/anychain-vendored/{anychain-core,anychain-tron,anychain-kms}/` — three separate vendored crates, NOT a public fork.

**Source provenance:** Each vendored crate carries a `SOURCE.md` recording the pinned upstream commit SHA (tag `v0.1.8` / `v0.2.14` / `v0.1.23`). Per-file `SPDX-License-Identifier: MIT OR Apache-2.0` preserved from upstream. Vendored versions bumped to `0.X.Y-local.0` to distinguish from crates.io.

**Workspace wiring:** `[workspace.dependencies]` uses `path = "crates/anychain-vendored/..."` instead of crates.io pins. NO `[patch.crates-io]` — local-path resolution short-circuits crates.io entirely for these names.

**Local patches (applied to vendored `src/**` in-place per grill Round-1 Q2):**

| Patch | Source | File(s) modified | Test citation |
|---|---|---|---|
| **Q13 varint fix** | Issue #540 | `anychain-tron/src/protocol/Tron.rs::Raw::write_to_with_cached_sizes` — `os.write_int64(18, self.fee_limit)` patched to emit canonical varint (strip spurious `0x01` prefix) | `tests/varint_and_txid.rs::fee_limit_canonical_varint` asserts trailing bytes `9080c9fe3d` for `fee_limit = 130_000_000` |
| **Q2 dual-SHA256 txid fix** | R2 Bug 1 (was caller-side workaround) | `anychain-tron/src/transaction.rs::TronTransaction::to_transaction_id` patched to return `SHA256(SHA256(raw_bytes))` (was single SHA-256) | `tests/varint_and_txid.rs::txid_is_double_sha256` |
| **Zeroizing gap fix** | R2 (caller-side mitigation in `tx/sign.rs`) | `anychain-kms/src/sign.rs::secp256k1_sign` — `sk` wrapped in `Zeroizing` for function scope, zeroize on return | Belt-and-suspenders: caller-side `Zeroizing<Vec<u8>>` wrap (Task 1.2) |

Per-dep changelog in `anychain-vendored/anychain-tron/CHANGELOG.md` records each patch with issue number, observed vs expected bytes (for #540), and test citation.

### Grill Round 1 (2026-09-06) — locked decisions

The following were settled against the grill frontier and are now canonical:

| Q | Decision |
|---|---|
| Q1 (vendor scope) | **(a) all three** (`anychain-core` + `anychain-tron` + `anychain-kms`). Bus-factor is project-wide; partial vendor creates false mitigation. |
| Q2 (patch form) | **(a) in-tree edits** to vendored `src/**`. Agents reading vendored source see fix in place; no second-file hunt. `cargo-patch` machinery buys nothing when source is already local. |
| Q3 (sync cadence) | **(a) quarterly** — `vendor/0xcregis-upstream` fetch + `cargo test -p tron-wallet-core` PASS + manual diff review. Mirrors crypto-deps CVE cadence. |
| Q4 (ADR form) | **(a) amend** this ADR (you are reading the amendment). Single-document audit trail. |

### Updated Risk Posture

| Old (2026-09-05) | New (2026-09-06) |
|---|---|
| R1 bus-factor = 1, ACCEPTED with pre-release-vendor-mitigation | R1 bus-factor = 1, **MITIGATED** via vendoring at Phase 0. Local patches land without upstream turnaround. |
| R2 Bug 1 (dual-SHA256 txid), caller-side workaround | R2 Bug 1, **FIXED in vendored copy**. Caller no longer computes workaround. Regression test guards against revert. |
| R2 Zeroizing gap, caller-side `Zeroizing` wrap | R2 Zeroizing gap, **FIXED in vendored kms copy** + caller-side wrap retained as belt-and-suspenders. |
| R4 mainnet smoke deferred, gated on `RUN_TRON_MAINNET=1` | R4 mainnet smoke UNBLOCKED — Q13 varint fix removes the upstream blocker. Gate still applies. |

### New Risk (R5) — vendor-upstream divergence lag

MEDIUM. Vendored copy will drift from `0xcregis/anychain main`. Quarterly sync gate (R5 above) keeps the gap bounded; security-fix back-port mandatory on advisory.

### Why amend instead of supersede (per grill Round-1 Q4)

The reasoning chain belongs together in one document: raw-primitives rejected (Pass 1) → crates.io accepted (Pass 2) → vendoring chosen (Pass 3, 2026-09-06 amendment). A separate ADR-0002 would split context across files and force readers to chase cross-references to reconstruct the decision. Single-document trail is the audit-friendly shape for this decision tree.

## References

- `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md` — full research, rejection rationale, version pins
- `docs/wallets/2026-09-05-anychain-tron-technical-deep-dive.md` — companion technical deep-dive
- `docs/wallets/2026-09-05-anychain-kms-technical-deep-dive.md` — companion KMS deep-dive
- `~/.claude/projects/-home-nhitran-Projects-blockchain-sdk/memory/polygon-phase-0-landed.md` — pattern reference for v0.1 release flow
- `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md` — implementation plan with Q13 + Phase 0 Task 0.2/0.7
- `gh issue view 540` — non-canonical varint in `fee_limit` (NPE on TronGrid broadcast) — driver for the 2026-09-06 amendment
