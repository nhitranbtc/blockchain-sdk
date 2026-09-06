---
title: PR #541 — Phase 0 anychain vendoring + Q2 / Zeroizing patches + #540 endpoint fix (ship-gate)
tracker: https://github.com/nhitranbtc/blockchain-sdk/issues/542
plan: docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md
deep-dive: docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md
pr: https://github.com/nhitranbtc/blockchain-sdk/pull/541
date: 2026-09-06
status: open
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening
---

# PR #541 — Phase 0 anychain vendoring — security audit (ship-gate)

Audit of `feat(tron): Phase 0 — vendor anychain + apply patches + fix #540 (endpoint switch)`
(PR #541, branch `tron/phase0-vendoring` → `rust-tron-core`, 78 files / +57,392 / −40).

## Scope

- Vendoring of `anychain-{core,tron,kms}` (75 files, `cf3aa2d` HEAD of `main`).
- Two local patches applied: **Q2** (`sha256(sha256)` txid) + **Risk #3** (Zeroizing sk buffer).
- Q13 reverted (fee_limit varint hypothesis disproved by live broadcast).
- Issue #540 actual fix: broadcast endpoint switched `/wallet/broadcasttransaction` → `/wallet/broadcasthex`.
- Regression tests: 4 new in `varint_and_txid.rs` + 1 updated in `v10_broadcast.rs`.

Out of scope (deferred per PR body): ADR-0001 revision, plan §126-138 endpoint doc update, plan §0.2 vendor-tracking procedure, Phase 1-7 deliverables, plan-amendment follow-up issue filing.

## Drift scan (L13 step 4a)

| ID | Severity | Claim | Verified |
|---|---|---|---|
| D1 | ✅ | Plan pins upstream tags `v0.1.8 / v0.2.14 / v0.1.23` for anychain-* | Tags do NOT exist on remote (`git ls-remote --tags` returned `0.0.2` and `0.1.5` only — recorded in all 3 SOURCE.md files). Plan deviation = documented ✅ |
| D2 | ✅ | `.local/anychain` HEAD = `cf3aa2d59afb2c50dc961919fca011c401238ed6` (moved from repo root `/anychain` → `.local/anychain` 2026-09-06, HEAD preserved) | Matches PR + all 3 SOURCE.md exactly. `git -C .local/anychain log -1` confirms subject `build: rustup toolchain update to 1.98.0 (#465)` 2026-09-04 ✅ |
| D3 | ✅ | 56 .rs files vendored, all with `SPDX-License-Identifier: MIT OR Apache-2.0` | `find ... -name '*.rs' \| wc -l` = 56. `grep -rL 'SPDX-License-Identifier' --include='*.rs'` returned 0 files without ✅ |
| D4 | ✅ | Vendored versions = `0.1.0-local.0` / `0.2.0-local.0` | `rust-wallet-app/Cargo.lock` entries match exactly; **no `source = "registry+..."`** field on any anychain-* entry (path-only entries) ✅ |
| D5 | ✅ | Workspace deps flipped to local `path =` | `rust-wallet-app/Cargo.toml`: `anychain-core = { path = "crates/anychain-vendored/anychain-core" }` (+ tron, kms) ✅ |
| D6 | ✅ | `cargo check -p tron-wallet-core` succeeds today | Re-checked 2026-09-06: `Finished dev profile ... in 2.27s` ✅ — zeroize 1.9.0 IS in Cargo.lock (initial grep `"zeroizing"` failed; correct crate name is `"zeroize"`) ✅ |
| D7 | ✅ | Round-trip guard at `sign.rs:275` | `grep 'from_bytes(&signed_bytes)'` in `tron-wallet-core/src/tx/sign.rs` line 275 — `TronTransaction::from_bytes(&signed_bytes)` re-parse present. Doc comment L231 names it "the round-trip guard" ✅ |
| D8 | ✅ | Mnemonic Zeroizing-wrapped at vendor level | `anychain-kms/src/bip39/mnemonic.rs:39-44`: `pub struct Mnemonic { phrase: Zeroizing<String>, lang: Language, entropy: Zeroizing<Vec<u8>> }` — NOT plaintext ✅ |
| D9 | 🟠 | Plan-amendment follow-up issue #541 filed | **`gh issue list --state all` returns no #541 issue** — GitHub `api /issues/541` returns PR body. PR body promises "Issues: ... #541 (plan amendment follow-up)" but #541 is the PR number itself, not a separate tracker item. **3 follow-up items (ADR-0001 revision, plan §126-138 broadcast path update, plan §0.2 vendor-tracking) currently have NO open tracker item holding them accountable.** |
| D10 | 🟡 | SOURCE.md claims Q13 varint patch applied in `Tron.rs::write_to_with_cached_sizes` | **PR body says Q13 REVERTED.** SOURCE.md (anychain-tron) predates the 2026-09-06 revert decision and still lists Q13 under "Local patches". Test `fee_limit_canonical_varint` pins `90 01 80 c9 fe 3d` (standard protobuf tag-18 encoding), consistent with revert narrative. Documentation drift, not code drift. |
| D11 | 🟡 | SOURCE.md (anychain-kms) names future Zeroizing patch site as `src/sign.rs::secp256k1_sign` | Actual patch site is `src/lib.rs::secp256k1_sign` — path mismatch in SOURCE.md. |
| D12 | — | REMOVED 2026-09-06 (config drift, not security finding) | `/anychain` not gitignored — moved out of ship-gate per review. |

## Cross-cutting controls (apply to every phase)

| ID | Severity | Control | Ships in |
|---|---|---|---|
| C1 | 🔴 | `TronGridClient::rpc_url` MUST be validated against an allowlist of Triton RPC hosts (`*.trongrid.io`, `*.shasta.trongrid.io`, etc.) before any signed-envelope POST. Current code accepts arbitrary string, optional SPKI pin only — attacker config steers signed envelopes to attacker host. | Phase 1 client wiring |
| C2 | 🟠 | `SignedTransaction::signed_envelope_hex` MUST be flushed / zeroed from process memory after successful broadcast (not just held in struct field). Loss-of-funds surface when envelopes cached for retry-queue survive process crash. | Phase 1 + Phase 4 (retry) |
| C3 | 🟠 | Vendored `anychain-*` MUST ship with a `SOURCE.md` that records BOTH the local-clone SHA AND `git ls-remote https://github.com/0xcregis/anychain cf3aa2d` output, with an archived tarball sha256. Today SOURCE.md records only the local clone pin — if `/anychain` clone is wiped, SHA pin is meaningless. | Phase 0 follow-up |
| C4 | 🟠 | Big-bang 75-file vendor drops MUST be replaced by an incremental sync mechanism: `cargo vendor` (re-issue on upstream bump), OR a vendoring tool that records per-file provenance (e.g. `cargo vendor` + `cargo-deny` provenance table). Future upstream pulls cannot be review-by-PR atomic. | Q3 cadence (post-Phase 7) |
| C5 | ✅ | **RESOLVED 2026-09-06** — `/anychain` clone relocated to `.local/anychain/` (under `.local/` which is convention-gitignored). No more `git add .` risk. | was Phase 0 follow-up; closed by operator move |
| C6 | 🟡 | Generated protobuf code (`anychain-tron/src/protocol/Tron.rs` = 20,060 lines, `Discover.rs` = 1,120, plus 14 contract `protocol/*.rs` files) MUST ship with a documented regeneration step (which `*.proto` + `protoc` invocation), so future schema updates flow through a reviewable diff path instead of becoming another big-bang drop. | Phase 0 follow-up (SOURCE.md addition) |

## Per-phase controls

### Phase 0 (this PR)

| ID | Severity | Control | Plan task |
|---|---|---|---|
| P0-1 | 🔴 | Plan-amendment follow-up issue must be filed as a real tracker item (NOT as the PR #). Plan amendment scope covers: (a) ADR-0001 pin strings → `cf3aa2d` + broadcast path = `/wallet/broadcasthex` + Q13 diagnosis correction, (b) plan §126-138 broadcast path documentation update, (c) plan §0.2 vendor-tracking procedure update. | Plan §S.1, L13 step 5 |
| P0-2 | 🟠 | Resolve the SOURCE.md ↔ PR body contradiction on Q13: edit `anychain-tron/SOURCE.md` to drop Q13 from "Local patches" section (preferred — matches current vendored bytes + PR body narrative). | Plan §0.7 |
| P0-3 | 🟠 | Fix SOURCE.md (anychain-kms) future-patch-site reference: `src/sign.rs::secp256k1_sign` → `src/lib.rs::secp256k1_sign` (the actual patched file). | Plan §0.7 |
| P0-4 | 🟡 | Live broadcast test (`v10_broadcast.rs`) gated `#[ignore]` — default `cargo test` skips. Add a synthetic-broadcast test that exercises `TronGridClient::broadcast` against a local mock server (or `wiremock`) so regression on wire-format is caught by `cargo test` without `RUN_TRON_NILE=1`. | Plan §Q3 cadence |
| P0-5 | 🟡 | `secp256k1_sign_smoke_after_zeroizing_patch` is structural-only (asserts `sig.len() == 64`). Doesn't verify the Zeroizing buffer lifecycle. Consider an ASAN test or inspector-callback test that asserts secret bytes are zeroed post-call. | Plan §Task 0.7 |
| P0-6 | ✅ | `zeroize` is a top-level transitive dep of `anychain-kms`, resolved from crates.io (1.9.0, with `derive` feature). `Cargo.lock` entry is correct. No drift. | Drift D6 |

### Phase 1–7 controls (out of scope — listed for downstream audit)

| ID | Severity | Control | Plan task |
|---|---|---|---|
| P1-N | 🔴 | Address derivation (`anychain_kms::bip32`) MUST be re-vetted for side-channel (constant-time HMAC-SHA512); P0 only covered signing-key `sk` Zeroizing. | Plan §1.2 |
| P2-N | 🟠 | TRC-20 ABI sign-then-broadcast MUST propagate `SignedTransaction::signed_envelope_hex` (not `raw_data_hex` + `signature_hex`) to all callers; current `broadcast()` signature already takes the right field, but `phase_2` must audit call sites. | Plan §2 |
| P7-N | 🟠 | Mainnet `$0.001 USDT` self-send gate (RUN_TRON_MAINNET=1) MUST verify that the live broadcast flow uses ONLY the canonical `/wallet/broadcasthex` endpoint — no fallback to `/wallet/broadcasttransaction`. | Plan §7 |
| P7-N | 🟠 | Mobile PAL wiring (`targets/ios` + `targets/android`) MUST re-vendor anychain-* rather than link against workspace `path =` — local-path deps do not cross crate boundaries when published as a `cdylib`. | Plan §4 |

## Threat catalog (supply-chain / replay / key / RPC)

| Threat | Severity | Phase | Control |
|---|---|---|---|
| Vendored supply-chain attack: `.local/anychain` clone is the only bit-source for vendored bytes | 🟠 | Phase 0 | C3 |
| RPC endpoint spoofing: `rpc_url` accepts any string, optional SPKI pin only | 🔴 | Phase 0 | C1 |
| Signed envelope disclosure: `signed_envelope_hex` kept in struct field past broadcast success | 🟠 | Phase 1+ | C2 |
| Mnemonic plaintext in transport (network send, FFI, config file) | 🟡 | Phase 1+ | (out of scope for Phase 0 audit) |
| Future-patch rebasing hazard: vendor + local patches must be re-applied on every upstream sync | 🟠 | Q3 cadence | C4 |
| Generated proto code stale: schema changes upstream not reflected in vendored `Tron.rs` | 🟡 | Q3 cadence | C6 |
| Address-derivation side-channel (HMAC-SHA512) | 🔴 | Phase 1 | P1-N |
| `cargo vendor` provenance not recorded | 🟡 | Phase 0 follow-up | C3 |
| `git add .` sweeps `/anychain` into staging | 🟡 | Phase 0 follow-up | C5 |
| Test pins 6-byte form per revert; future regulator demand for canonical 5-byte would silently regress | 🔵 | Phase 0 | (deferred — wait for upstream / canonical wire-format adjudication) |

## Audit axes score (0–3, sum 0–24, any 0 = BLOCK ship)

| Axis | Score | Note |
|---|---|---|
| A1 Drift scan (L13 4a) | 2 | D9/D10/D11/D12 open; D2/D3/D4/D5/D6/D7/D8 closed |
| A2 Threat model | 2 | C1/C2/C4 surfaced; C3/C5/C6 hardening |
| A3 Test coverage | 2 | 4 new tests pin varint/txid/zeroizing. Live broadcast test gated `#[ignore]` — default `cargo test` skips |
| A4 Iron-law compliance | 3 | Q2 + Risk #3 + #540 fix all TDD-flavored: write failing test, fix code, verify live |
| A5 Tracker alignment | 1 | D9 — promised #541 follow-up issue does not exist as tracker item |
| A6 Spec-implementation distance | 2 | Vendored crates are thin (sync layer) — module-shape good. `SignedTransaction` adds field additively (no break). Round-trip guard restored. |
| A7 Iron anti-patterns | 3 | No "verify at end"; no "tests optional"; no "agent knows"; design decisions documented in plan + ADRs |
| A8 Worktree + branch discipline | 3 | Branch `tron/phase0-vendoring` (correct prefix), targets `rust-tron-core` (correct, per plan §S.2), NOT `main` |
| **Sum** | **18** | (Ship-blocker until D9 + A5 > 1; ship-blocker until C1 closed in Phase 1) |

## Ship-gate checklist

Implementer MUST clear before tagging release:

- [ ] **D9 resolved** — open separate issue for plan-amendment scope (ADR-0001 revision + plan §126-138 + plan §0.2). DO NOT block this PR on it but DO commit to filing in this PR's merge squash.
- [ ] **D10 resolved** — delete "Q13 varint fix" line from `anychain-tron/SOURCE.md` "Local patches" section (matches current vendored bytes + PR body revert narrative).
- [ ] **D11 resolved** — fix `anychain-kms/SOURCE.md` future-patch site reference to `src/lib.rs::secp256k1_sign`.
- [ ] **C1 documented / scoped** — `TronGridClient::rpc_url` accepts an allowlist OR is flagged "host validation deferred to Phase 1" (your call: either accepted with human sign-off, or implemented).
- [ ] **C2 documented / scoped** — `signed_envelope_hex` zeroing-on-success has a doc note OR is implemented in Phase 1.
- [ ] **C3** — `SOURCE.md` per crate records BOTH local-clone SHA AND `git ls-remote` output, plus an archived tarball sha256.
- [ ] **P0-4** — accept gating of live broadcast test, OR add a `wiremock`-backed unit test for the broadcast endpoint.
- [ ] ADR-0001 revision follow-up tracked (via issue per D9).
- [ ] Plan §126-138 revision follow-up tracked (via issue per D9).
- [ ] `cargo build --workspace` PASS (re-verified 2026-09-06: 2.27s).
- [ ] `cargo test -p tron-wallet-core` PASS (139 tests incl. 4 varint_and_txid).
- [ ] `cargo clippy -p tron-wallet-core -- -D warnings` PASS (run before merge).
- [ ] `cargo geiger -p tron-wallet-core` shows no NEW `unsafe` introduced vs pre-vendor state.

## Out-of-scope (deferred per PR body + audit)

- Plan §1-7 phase deliverables (each in its own Phase ticket per plan).
- Live mainnet self-send gate (`RUN_TRON_MAINNET=1`, Phase 7).
- Plan amendment updates — blocked on D9 fix.
- Future Q3 quarterly anychain sync — cadence only; not in this PR.
- `Cargo.lock` audit for non-anychain registry deps — out of scope for this vendoring-focused audit.

## References

- PR #541: <https://github.com/nhitranbtc/blockchain-sdk/pull/541> (state: OPEN)
- Issue #540 (root cause / fixed-by-endpoint-switch): <https://github.com/nhitranbtc/blockchain-sdk/issues/540>
- Issue #399 (umbrella): <https://github.com/nhitranbtc/blockchain-sdk/issues/399>
- Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`
- ADR-0001 (pre-revision): `docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md`
- Deep-dive: `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md`
- Vendored source clone: `.local/anychain` (HEAD = `cf3aa2d59afb2c50dc961919fca011c401238ed6`, tracking `origin/main`; relocated from `/anychain` at repo root 2026-09-06)
- Upstream: <https://github.com/0xcregis/anychain>
- `git ls-remote --tags https://github.com/0xcregis/anychain.git` (executed 2026-09-06): returned only `0.0.2` and `0.1.5` tags.
- Reference impl (Tangem Swift): <https://github.com/tangem/blockchain-sdk-swift/blob/develop/BlockchainSdk/Blockchains/Tron/TronTarget.swift>
- Audit methodology (companion guide): `.local/plugins-docs/2026-09-06-mattpocock-vs-superpowers-audit-guide.md`
- Reference audit (precedent): `docs/audit/2026-08-27-tron-wallet-core-security-audit.md`

## Filename and label matrix (for `gh issue create`)

```bash
gh issue create \
  --repo nhitranbtc/blockchain-sdk \
  --title "Security audit + drift scan: PR #541 (Phase 0 anychain vendoring + Q2/Zeroizing patches + #540 endpoint fix)" \
  --label "rust-tron-core,backlog,task" \
  --body-file /tmp/audit-2026-09-06-pr541-tron-phase0-vendoring.md
```

Labels: `rust-tron-core` (chain-specific, per repo convention) + `backlog` + `task`. Per GateGuard: `gh issue create` body written to `/tmp` file before issue creation.
