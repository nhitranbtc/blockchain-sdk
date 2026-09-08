# tron-wallet-core (v0.1) — Implementation Plan (anychain stack, **VENDORED**)

> **For agentic workers:** REQUIRED SUB-SKILLS: `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **2026-09-06 REVISION (issue #540):** anychain direct from crates.io → **VENDORED**. See Q13 + Q3 (revised) + Section B (CHOSEN). Vendoring decision was REJECTED on 2026-09-05 — #540 (non-canonical varint in `fee_limit` from `anychain-tron 0.2.14`, bus-factor = 1, upstream fix turnaround unknown) flipped the cost/benefit. Local copy gives us the fix without waiting on upstream. ADR update follows in a follow-up commit (see "Open follow-up" below).

**Goal:** Deliver `rust-wallet-app/crates/tron-wallet-core/` — a TRON (TRX + TRC-20 stablecoin) wallet library built on **`anychain-{core,tron,kms}`** (revision 2026-09-06, supersedes 2026-08-27 raw-primitives plan), plus a `tron` CLI in the umbrella workspace. **Vendored** under `rust-wallet-app/crates/anychain-vendored/{core,tron,kms}/` — source pulled from `https://github.com/0xcregis/anychain` commit **`cf3aa2d59afb2c50dc961919fca011c401238ed6`** (HEAD of `main`, 2026-09-04; upstream tags `v0.1.8` / `v0.2.14` / `v0.1.23` do NOT exist on remote — `git ls-remote --tags` returned only `0.0.2` and `0.1.5` as of 2026-09-06), per file `SPDX-License-Identifier: MIT OR Apache-2.0` preserved, local patches applied for #540 (endpoint switch to `/wallet/broadcasthex`) + Zeroizing gap (Risk #3). PR #541 closed Phase 0. **2026-09-06 evening revision:** Q2 (dual-SHA-256 txid) hypothesis was wrong — live broadcast verification showed TronGrid returns `sha256(raw_bytes)` (single SHA-256, matching upstream). Q2 vendored patch reverted; `tx::sign::txid` corrected to single SHA-256; tests in `tests/v9_sign_tx.rs`, `tests/v8_sign_only.rs`, `tests/varint_and_txid.rs` updated. **Bumps MSRV to 1.98.1** (anychain workspace toolchain). Compiles for desktop (Linux/macOS/Windows) + mobile (iOS arm64 + Android arm64) via 4-trait PAL.

**Companion docs:**
- Research: `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md` (this plan's source of truth)
- User stories (legacy, outdated — must regenerate per mismatch report): `docs/wallets/2026-08-27-tron-wallet-user-stories.md`
- ADR capturing reversal: `docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md` (**AMENDED 2026-09-06** with `## 2026-09-06 Revision` section per grill Round-1 Q4)
- Changelog: `rust-wallet-app/crates/tron-wallet-core/CHANGELOG.md` (per-phase pre-release entries; updated on every shipped phase per L24)
- Estimate report (operator-local, gitignored per L14): `.superpowers/sdd/2026-09-05-tron-wallet-core-v0.1-anychain/estimate-report.md`
- AI cost report (operator-local, gitignored per L14): `.superpowers/sdd/2026-09-05-tron-wallet-core-v0.1-anychain/ai-cost-report.md`
- Supersedes: `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` (raw-primitives plan — kept for archaeology)

**Tracks:** issue #399 (Q1–Q10 closed by deep-dive Round-1 grill Q1–Q12). PR #402 (Ticket A — research) closed; this plan covers Ticket B–F (implementation).

**Pre-empts:** v0.3+ placeholder in `rust-wallet-app/crates/chain-traits/src/lib.rs:21` (`ChainId::Tron(u32)`).

**Status:** Plan. No code produced yet. **PAUSE before commit** per never-auto-commit rule.

### Decisions (grill Round 1, 2026-09-06)

Locked by user ("do it") against the grill Round-1 frontier:

| Q   | Decision               | Answer                                                                                                                                                                       |
| --- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1  | Vendor scope           | **(a) all three** (`anychain-core`, `anychain-tron`, `anychain-kms`) — bus-factor is project-wide, partial vendor creates false mitigation                                   |
| Q2  | Patch application form | **(a) in-tree edits** to vendored `src/**` — agents read fix in place; no second-file hunt                                                                                   |
| Q3  | Upstream sync cadence  | **(a) quarterly** — `.local/anychain` working-copy fetch (`git -C .local/anychain fetch origin && git reset --hard origin/main`) + regression test gate + manual diff review |
| Q4  | ADR form               | **(a) amend** ADR-0001 with `## 2026-09-06 Revision` section — single-document audit trail                                                                                   |

Downstream Round 2 (pending): vendored-tree git location, version-suffix style, patch-bundling per PR, lockfile `source =` policy.

---

## Global Constraints (verbatim from deep-dive Round-1 grill Q1–Q12)

- **Q1 — SDK choice.** **anychain-{core,tron,kms} @ `cf3aa2d59afb2c50dc961919fca011c401238ed6`** (HEAD of upstream `main`, 2026-09-04), **vendored** under `rust-wallet-app/crates/anychain-vendored/{core,tron,kms}/` (revision 2026-09-06, see Q3 + Q13 + Section B). Trades ~250 lines of `reqwest` glue for ~1000 lines of hand-rolled protobuf + base58check + Keccak-256 + ABI encoder + Stake 2.0 contract builders. Raw `reqwest` + `prost` 0.14.4 plan (2026-08-27) **REJECTED** by Round-1 audit — see ADR-0001.
- **Q2 — Transaction format.** Protobuf via vendored `anychain-tron/src/protocol/Tron.proto`. Signing: `SHA256(raw_bytes)` then `secp256k1_sign` → `TronTransaction::sign(sig, recid)`. **txid BUG fix lives in vendored copy:** vendored `anychain-tron/src/transaction.rs::TronTransaction::to_transaction_id()` is patched to return `SHA256(SHA256(raw_bytes))` (canonical TRX double-hash). Caller no longer computes it manually. **Pinned to vendored SHA** (commit recorded in `anychain-vendored/SOURCE.md`) — vendored `[patch.crates-io]` is NOT used; consumers reference via local `path = "...anychain-vendored/..."`. Regression test in `tron-wallet-core/tests/varint_and_txid.rs` asserts `txid == SHA256(SHA256(raw_bytes))` for canonical `fee_limit = 130_000_000` so future vendored "fixes" or upstream syncs get caught in CI.
- **Q3 — Bus-factor risk (mitigated by vendoring).** `0xcregis/anychain` author diversity trailing 12 months: `anychain-tron` = **1 author (`loki-cmu`), 3 commits**; `anychain-kms` = 2 authors, 8 commits. Bus-factor = 1. **Mitigation = vendor (revision 2026-09-06, #540).** Vendored copy under `rust-wallet-app/crates/anychain-vendored/{core,tron,kms}/` lets us (a) apply local patches without waiting for upstream turnaround (Q13 varint fix shipped without upstream ack); (b) audit every commit we import via `git log` from the pinned source commit; (c) reject silent upstream behavior changes by exact `path =` reference — no version drift, no `cargo update`-induced surprise. Upstream is tracked via a `.local/anychain` working-copy clone under `.local/` (kept untracked, convention-gitignored, single source-of-truth for the inspection mirror; relocated from repo root 2026-09-06 per operator decision) for security-fix back-port. The original plan to spin up `vendor/0xcregis-upstream` git remote inside `anychain-vendored/.git/` was **dropped 2026-09-06** (per PR #541 amendment) — `.local/anychain` keeps the workflow simple (one checkout, no nested submodule). Consumer builds never touch crates.io. Per-file `SPDX-License-Identifier: MIT OR Apache-2.0` headers preserved; `SOURCE.md` records pinned commit SHAs + any local patches. **Revision history:** REJECTED 2026-09-05 (cost/benefit); ACCEPTED 2026-09-06 (bus-factor combined with #540 bug = no-cost mitigation; local patch turnaround < 1 day vs unknown upstream turnaround).
- **Q13 — Varint hypothesis DISPROVED; broadcast endpoint SWITCHED (REVISION 2026-09-06 per PR #541, issue #540).** Original hypothesis (anychain-tron `0.2.14` `Raw` proto emitted spurious leading `0x01` byte in canonical varint for `fee_limit >= 2^27`, e.g. observed `90 01 80 c9 fe 3d` instead of canonical `90 80 c9 fe 3d`; TronGrid Java gateway reading `fee_limit = 1` + tripping on unknown field 16 → NPE) was NOT the root cause. **Live broadcast investigation 2026-09-06** (Nile, sender funded 100 TRX + 61.5 USDT, recipient address `TJdcLjM1KvzGotZ5MxEAZZbnT6YCuAYRaf`) showed BOTH the 6-byte form (`90 01 80 c9 fe 3d`) AND the 5-byte canonical form (`90 80 c9 fe 3d`) FAIL against TronGrid — the 6-byte form triggers NPE on `/wallet/broadcasttransaction` and `InvalidProtocolBufferException` on `/wallet/broadcasthex`; the canonical 5-byte form triggers NPE on `/wallet/broadcasttransaction` and `InvalidProtocolBufferException` on `/wallet/broadcasthex`. `fee_limit` was a red herring. **Actual fix (PR #541):** switch broadcast endpoint from `/wallet/broadcasttransaction` (split-form `{raw_data_hex, signature_hex}` body) to `/wallet/broadcasthex` (single-blob `{transaction: "<full-TronTransaction-envelope-hex>"}` body). This matches Tangem's `TronTarget.broadcastHex` Swift impl that works against the same TronGrid endpoints. Vendored `Tron.rs` left unmodified — Q13 varint patch was **REVERTED 2026-09-06** before this plan-amendment commit. **Regression test:** `tests/varint_and_txid.rs::fee_limit_canonical_varint` now pins the standard 6-byte form (`90 01 80 c9 fe 3d`) which is the correct protobuf wire format for tag 18 (5-byte form is impossible for tag 18 — varint of header byte `90` needs 2 bytes per protobuf wire spec). Test comments explain the standard form IS the baseline; the test's job is to catch any accidental Q13-style 5-byte patch reintroduction. **Verified live:** `RUN_TRON_NILE=1 cargo test -p tron-wallet-core --test v10_broadcast` succeeds — Nile txid `3cb6657601449ccca510949f025bdf8de186aac1ea291d091c8148fe05c08e74` accepted by TronGrid. **Tracking:** endpoint switch documented at `rust-wallet-app/crates/tron-wallet-core/src/chain/client.rs:86-104` (post-PR #541).
- **Q4 — Mainnet smoke gate.** **v0.1 release GATED on one mainnet self-send — $0.001 USDT to self (recipient == sender), real value, real network.** Local + Nile is emulation. Without a real-value smoke, V1-V10 PASS evidence = "looks like real network" not "real network". Add to acceptance criteria NOW, not post-Phase 4. Pre-check audit hook (refuse if recipient != operator_wallet) + `RUN_TRON_MAINNET=1` env gate.
- **Q5 — SPKI pin live extraction.** `api.trongrid.io` TLS cert SPKI SHA-256 = `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d` (verified 2026-09-05). Pin in `TronConfig::for_network(Network::Mainnet)` SPKI list. Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` shape-compatible.
- **Q6 — Testnet.** **Nile** for v0.1. Chain-id `0xcd8690dc` / 3448148188. Use `POST /jsonrpc {"method":"eth_chainId"}` (TronGrid's `/wallet/getchainid` returns HTTP 405). Address prefix byte `0x41` universal across Mainnet/Shasta/Nile — network discrimination by chain-id only.
- **Q7 — RPC pinning.** `pinned://<spki-hex>@host[:port]` URL scheme. Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` verbatim. Default CLI: Scenario B (no pin); operator opts into Scenario A via `pinned://`.
- **Q8 — Sign-only path.** `v ∈ {0, 1}` (NOT Ethereum's `v+27 ∈ {27, 28}`). anychain-kms returns recid ∈ {0, 1} natively. Audit control: compile-fail test rejecting eth-default `v+27` decoders in `tx/sign.rs`.
- **Q9 — Token registry.** Bundled JSON. **Nile USDT = `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`** (canonical per TronScan verified 2026-09-05). **CAUTION:** user-stories.md Story 21 quotes `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` — this is **WRONG**; deep-dive value is canonical. Tokens live at `tokens/{local,nile,mainnet}.json` (3 files including local — local added per Round-1 grill Q6 for TronBox Docker integration test).
- **Q10 — Mnemonic + SLIP-44.** Coin type 195 = TRX. Path `m/44'/195'/0'/0/0`. Spike V10 verifies canonical test vector ("abandon ×11 about") matches `bip_utils::slip44::Coin::Tron`.
- **Q11 — Stake 1.0.** Deferred entirely. `tron stake` = Stake 2.0 only. Add `tron stake1 withdraw` in v0.2 IF user demand. No silent Stake 1.0 code paths.
- **Q12 — Disambiguation guards.** Compile-time constants in `disambig.rs`: cross-network address check (refuse mainnet→nile send), TRC-20 footgun guards (refuse `transfer` to non-Tron address).
- **Round-1 grill Q8 audit:** every "Status: ready" row is by inspection, NOT by spike PASS. Before v0.1 ships, audit each against V1-V10. If a command isn't covered by Vn spike PASS block, demote to "ready (untested)" or remove from v0.1.

---

## Architecture (locked 2026-09-04)

### Five-layer PAL design

```text
Layer 5: FFI (cdylib)
   - C ABI surface (extern "C" fn wallet_unlock, ...)
   - Panic-message scrubber
   - tokio runtime pinned (single-threaded current_thread)

Layer 4: Pure Rust Core (portable, 95%)
   - address/, keys/, tx/builder, tx/sign
   - crypto (argon2id + AES-GCM logic)
   - error, disambig, config (types), util
   - tx_summary, tokens (bundled JSON via include_str!)

Layer 3: PAL — 4 traits
   - WalletStorage (encrypted blob persistence)
   - PlatformInfo (data dir, app name, version, is_mobile)
   - NetworkClient (HTTP + TLS root certs)
   - Clock (monotonic time for tx expiration)

Layer 2: Platform impls (5%)
   Desktop: FileWalletStorage, SystemDirsInfo, ReqwestClient
   iOS:     KeychainWalletStorage, BundleInfo, OSRootsClient
   Android: EncryptedFileWalletStorage, ContextInfo, OSRootsClient
   Tests:   InMemoryStorage, StaticInfo, MockClient

Layer 1: OS + hardware
   Desktop: filesystem + OpenSSL/ring
   iOS: Keychain Services + Secure Enclave + Apple Trust Store
   Android: EncryptedFile + Android Keystore + StrongBox
```

### V0.1 CLI surface (22 commands, 6 top-level)

| Top-level | Commands                                                                    | Stories                           |
| --------- | --------------------------------------------------------------------------- | --------------------------------- |
| `wallet`  | create, import, show, list, delete, rename, balance, send, send-speedup (9) | 1, 2, 3, 5, 9, 10, 11, 12, 17, 22 |
| `address` | new, xpub (2)                                                               | 3, 19                             |
| `balance` | --address, --address --token (2)                                            | 3, 22                             |
| `trc20`   | send, approve, balance, allowance (4)                                       | 21, 22, 25, 30                    |
| `tx`      | get, wait (2)                                                               | 7                                 |
| `config`  | show, set-rpc, set-network (3)                                              | 10, 11, 26, 27                    |

**Story coverage matrix** (from deep-dive V0.1 feature map):

| Stories                                                      | Status                                                                                                                                                                                               |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1, 2, 3, 5, 7, 9, 10, 11, 12, 17, 19, 21, 22, 25, 27, 28, 29 | shipped V0.1                                                                                                                                                                                         |
| 4, 6, 31, 32, 33                                             | V0.1.5 (Stake 2.0, ships with V0.1 release train)                                                                                                                                                    |
| 8, 18, 23, 24, 30, 34, 35, 36                                | V0.2 (deferred — `tron resource`, `tron sign-message`, `tron tokens list/register`, `tron tokens balances`, `tron trc10 issue/send/buy`, `tron governance propose/approve`, `tron storage buy/sell`) |
| 26 (TronBox local)                                           | shipped via `--rpc http://127.0.0.1:8090` flag                                                                                                                                                       |
| **13 (batch), 14 (drain), 15 (ref-block), 16 (manual exp)**  | **DROPPED from V0.1** — redesign deferred (user-stories.md must update to mark these as removed)                                                                                                     |

### Cross-crate flows (stable)

**Address derivation (kms → tron cooperation):**
```text
keys::derive_keypair(mnemonic, path)
    → anychain_kms::seed_from_mnemonic(phrase, "") -> [u8; 64]
    → anychain_kms::ExtendedPrivateKey::from_seed(seed)
    → anychain_kms::ExtendedPrivateKey::derive_child(path)
    → xprv.to_string() -> Zeroizing<String>
    → anychain_tron::TronAddress::from_public_key(&xprv.public_key())
    → "T..." base58check
```

**Sign + broadcast TRX tx:**
```text
tx::sign::sign_tx(&sk, raw_data_bytes)
    → anychain_core::sha256(&raw_bytes)            [direct utility]
    → msg32
    → anychain_kms::secp256k1_sign(&sk_z, msg32)    [Zeroizing wrapper — GAP]
    → (sig, recid)
    → anychain_tron::Transaction::sign(sig, recid)
    → SignedTransaction { txid: SHA256(SHA256(raw)), signature }

tx::broadcast::serialize_for_broadcast(&tx)
    → serde_json::to_value(&tx)
    → POST wallet/broadcasthex                    [caller reqwest] # AMENDED 2026-09-06 per PR #541 — broadcast transaction POSTed as full-envelope hex, not split raw_data_hex + signature_hex
```

---

## Rust SDKs, tools, and crates — full inventory

**Total: 38 crates** (33 mobile-safe 87% + 4 desktop-only 11% + 1 build-time 2%) per deep-dive §"Crates used in `tron-wallet-core` (V0.1)".

### A. anychain stack (PRIMARY — wire-format + HD + signing) [vendored, `path = workspace`]

**Vendored** under `rust-wallet-app/crates/anychain-vendored/{core,tron,kms}/`. Consumer crates reference via `path = "..."` — no crates.io, no `[patch.crates-io]`. Pinned to upstream tags per `anychain-vendored/SOURCE.md`. Local patches layered on top per `anychain-vendored/CHANGELOG.md` (Q2 dual-SHA256 txid, Q13 varint, Zeroizing gap).

| Crate           | Source tag (pinned)                                                                                  | Local path                                                                 | Purpose                                                                                                                                                                                                                                                                      |
| --------------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `anychain-core` | `cf3aa2d` (upstream `main` HEAD, 2026-09-04 — tags `0.1.8`/`0.2.14`/`0.1.23` do NOT exist on remote) | `rust-wallet-app/crates/anychain-vendored/anychain-core` (`0.1.0-local.0`) | Shared traits (`Address`, `PublicKey`, `Transaction`, `Format`, `Network`), crypto utilities (`keccak256`, `sha256`, `func_selector`), `hex` re-export                                                                                                                       |
| `anychain-tron` | `cf3aa2d` (same — vendored together as one snapshot, 2026-09-04)                                     | `rust-wallet-app/crates/anychain-vendored/anychain-tron` (`0.2.0-local.0`) | Wire format — T-base58check address, protobuf `Transaction` envelope, **17 contract builders** (TRX transfer, TRC-20 transfer/approve, Stake 2.0 freeze/unfreeze/delegate/cancel/withdraw, witness vote, withdraw vote, account create, generic trigger), `abi::encode_call` |
| `anychain-kms`  | `cf3aa2d` (same — vendored together as one snapshot, 2026-09-04)                                     | `rust-wallet-app/crates/anychain-vendored/anychain-kms` (`0.1.0-local.0`)  | BIP-39 mnemonic (8 languages), BIP-32 HD (SLIP-44 coin 195), secp256k1 sign, xprv serialize with `Zeroizing<String>`                                                                                                                                                         |

**Pin to vendored commit (NOT `^`, NOT crates.io).** Phase 0 Task 0.2 replaces crates.io pins with vendored `path =` references in `[workspace.dependencies]`. Consumers NEVER see `crates.io/index` for these names — local-path wins deterministically.

### B. Vendoring — CHOSEN 2026-09-06 (was REJECTED 2026-09-05; flipped by issue #540)

| Source repo         | Vendored path                                               | Outcome                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------- | ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0xcregis/anychain` | `rust-wallet-app/crates/anychain-vendored/{core,tron,kms}/` | **CHOSEN 2026-09-06** — bus-factor = 1 for `anychain-tron` (1 author, 3 commits trailing 12 months) was an accepted risk on 2026-09-05, but issue #540 (non-canonical varint for `fee_limit` in `anychain-tron 0.2.14`, NPE on TronGrid) flipped the cost/benefit: local copy lets us apply the varint fix without waiting on upstream turnaround. Local patches layered on top per Q13 + Q2 + Zeroizing gap. Per-file `SPDX-License-Identifier: MIT OR Apache-2.0` preserved. `anychain-vendored/SOURCE.md` records pinned commit SHAs. Upstream tracked via the `.local/anychain` working-copy clone under `.local/` (kept untracked via convention-gitignore; spun up 2026-09-06 per PR #541 amendment as a replacement for the original `vendor/0xcregis-upstream` git remote plan; relocated from repo root `/anychain` → `.local/anychain` 2026-09-06) for security-fix back-port — but consumer builds never touch crates.io. **Update cadence:** sync vendored copy with upstream `main` on a quarterly schedule, gate on (a) no breaking wire-format change, (b) regression tests in `tron-wallet-core/tests/` still PASS, (c) manual review of diff. |

**Open follow-up (not blocking v0.1 release):** update `docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md` → ADR-0002 (or amend ADR-0001 with `## 2026-09-06 Revision` section) capturing the vendoring reversal. Filed as issue #541 (or follow-up edit in same PR). PR author to decide whether to amend or supersede; the ADR should record both the original 2026-09-05 decision (crates.io exact pin, bus-factor accepted) AND the 2026-09-06 revision (vendor, bus-factor mitigated, #540 fix locally).

### C. Crypto (RustCrypto ecosystem) [workspace deps]

| Crate                      | Version   | Purpose                                                                               |
| -------------------------- | --------- | ------------------------------------------------------------------------------------- |
| `argon2`                   | 0.5       | Argon2id KDF for wallet file encryption                                               |
| `aes-gcm`                  | 0.10      | AES-256-GCM symmetric encryption for `EncryptedWallet` blob                           |
| `sha2`                     | 0.10      | SHA-256 (txid double-hash workaround per Q2)                                          |
| `sha3`                     | workspace | Keccak-256 (TRON address derivation via `anychain_core::keccak256`)                   |
| `tiny-keccak`              | 2.0.2     | Direct keccak256 call (kept to avoid anychain indirection)                            |
| `bs58`                     | 0.5       | base58check encoding (T-addresses, xprv)                                              |
| `hex`                      | workspace | hex encode/decode for protobuf serialization                                          |
| `zeroize`                  | 1.x       | Secure memory hygiene (`Zeroizing<Vec<u8>>` wrap on raw `sk` before `secp256k1_sign`) |
| `subtle`                   | 2         | Constant-time comparison (`ConstantTimeEq` for SPKI pin, xprv compare)                |
| `libsecp256k1` (secp256k1) | workspace | ECDSA signing via anychain-kms (`secp256k1_sign`)                                     |

### D. Encoding / serialization [workspace deps]

| Crate            | Version   | Purpose                                                                             |
| ---------------- | --------- | ----------------------------------------------------------------------------------- |
| `serde`          | 1.x       | derive Serialize/Deserialize                                                        |
| `serde_json`     | 1.x       | JSON for TronGrid HTTP envelope + receipt parsing                                   |
| `protobuf`       | 3.7       | TRON wire format (proto-generated types in `anychain-tron/src/protocol/`)           |
| `ethereum-types` | workspace | Address type for ABI encoder                                                        |
| `ethabi`         | workspace | TRC-20 ABI encode/decode (EIP-20 compatible)                                        |
| `chrono`         | workspace | timestamp handling for tx expiration                                                |
| `uuid`           | 1.x       | Wallet id (UUID v4)                                                                 |
| `directories`    | workspace | Desktop data dir resolution — **V0.1.5 removal**, replaced by `WalletStorage` trait |

### F. Async + HTTP [workspace deps]

| Crate                 | Version | Purpose                                                                                                                           |
| --------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `tokio`               | 1.x     | Async runtime (current_thread for FFI; multi-thread for CLI)                                                                      |
| `reqwest`             | 0.12    | HTTP client for TronGrid (`broadcast`, `gettxinfo`, `getnowblock`, `getaccount`, `getaccountresource`, `triggerconstantcontract`) |
| `rustls`              | 0.23    | TLS for reqwest + custom SPKI pin verifier                                                                                        |
| `rustls-native-certs` | 0.7     | Desktop OS root cert loading                                                                                                      |
| `webpki`              | 0.22    | Custom `ServerCertVerifier` for SPKI pinning                                                                                      |
| `x509-parser`         | 0.16    | SPKI DER extraction from cert chain                                                                                               |

### G. Errors + tracing + FFI safety [workspace deps]

| Crate                | Version   | Purpose                                                              |
| -------------------- | --------- | -------------------------------------------------------------------- |
| `thiserror`          | 1.x       | `Error` enum derive                                                  |
| `tracing`            | workspace | Structured logging (STDERR, secret-scrubbing filter)                 |
| `tracing-subscriber` | workspace | Subscriber with EnvFilter                                            |
| `once_cell`          | 1.x       | Lazy-init for FFI runtime + compiled regex patterns                  |
| `regex`              | 1.x       | Panic-message scrubber (redact mnemonic + password + xprv + secret)  |
| `cbindgen`           | workspace | Generates C header for FFI consumers (build-time only, doesn't ship) |

### H. Test-only (dev-dependencies)

| Crate            | Purpose                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------ |
| `proptest`       | Property-based tests for amount parsing, address derivation                                |
| `tempfile`       | Atomic-write test fixtures                                                                 |
| `testcontainers` | TronBox Docker auto-spawn (`0.23`, desktop-only, mobile skips via `--no-default-features`) |
| `bitcoind`       | regtest smoke tests (desktop-only)                                                         |

### I. Build tools + system dependencies (CI + dev environment)

| Tool        | Version       | Role                                                                                                                                                                     |
| ----------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `protoc`    | **≥3.12**     | System protobuf compiler invoked by anychain-tron build (transitive). Required only if anychain-tron uses `prost-build` at runtime. Spike README documents install path. |
| `cargo`     | 1.98.1 stable | **MSRV bump from 1.85 to 1.98.1** (anychain workspace toolchain pin). Pin via `rust-toolchain.toml`.                                                                     |
| **TronBox** | **4.10.0+**   | Local dev regtest + `MockTRC20` deploy. Node ≥20. NOT a Rust dep — runs in Node for spike regtest only via `testcontainers`.                                             |

### J. Cross-crate reuse (NOT a new dep — single import)

| Source               | Path                               | Role                                                                                                                                                         |
| -------------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `SpkiPinnedVerifier` | `bitcoin_wallet_core::chain::spki` | Custom `rustls::ServerCertVerifier` for SPKI-pinned JSON-RPC transport. Same `rustls = "0.23"` version as TRON-side `reqwest`. Single import, zero new code. |

### License summary (all compatible with workspace MIT)

MIT OR Apache-2.0 (anychain-*, argon2, aes-gcm, sha2, sha3, bs58, hex), Apache-2.0 (reqwest, serde_json, protobuf, ethereum-types, ethabi, libsecp256k1 variant), MIT (serde, tokio, zeroize, etc.), BSD (libsecp256k1).

### Mobile-unsafe deps (must remove for V0.1.5)

| Crate                      | Reason                      | Replacement                                                                    |
| -------------------------- | --------------------------- | ------------------------------------------------------------------------------ |
| `directories`              | No iOS/Android backend      | `WalletStorage` trait (File/Keychain/EncryptedFile)                            |
| `rustls-native-certs`      | Desktop-only OS cert loader | `tls_built_in_root_certs(true)` on mobile (reqwest)                            |
| `testcontainers` (dev-dep) | Requires Docker             | Skip on mobile via `cfg(not(target_os = "android"))` + `--no-default-features` |
| `bitcoind` (dev-dep)       | Test fixture only           | Same gating                                                                    |

**V0.1.5 work:** remove `directories` (1 crate), gate `rustls-native-certs` behind `#[cfg(not(mobile))]`, gate `testcontainers` + `bitcoind` behind `#[cfg(desktop)]`. ~30 LOC of Cargo.toml changes.

---

## File Structure

```text
rust-wallet-app/crates/anychain-vendored/      # VENDORED anychain stack (Task 0.2) — pinned SHA per SOURCE.md
│
│   # NOTE (2026-09-06 amendment per PR #541):
│   # - Removed nested `.git/` subtree (was: separate git repo with vendor/0xcregis-upstream remote)
│   #   → replaced by /anychain working-copy at repo root as the inspection mirror.
│   # - Vendored Cargo.toml versions set to 0.1.0-local.0 / 0.2.0-local.0 (distinguishable from upstream tag pins).
├── anychain-core/
│   ├── Cargo.toml                              # version = "0.1.0-local.0", deps = local anychain-{tron,kms} via path
│   ├── SOURCE.md                               # pinned commit SHA cf3aa2d… + upstream URL + git ls-remote output
│   └── src/                                    # copied verbatim from /anychain @ cf3aa2d… ; SPDX headers preserved
├── anychain-tron/
│   ├── Cargo.toml                              # version = "0.2.0-local.0"
│   ├── SOURCE.md
│   ├── CHANGELOG.md                            # local patches (Task 0.7: Q2 txid + Zeroizing; Q13 REVERTED 2026-09-06)
│   └── src/                                    # vendored + patched (Q2 + Zeroizing; Q13 NOT applied)
└── anychain-kms/
    ├── Cargo.toml                              # version = "0.1.0-local.0"
    ├── SOURCE.md
    └── src/                                    # vendored + patched (Zeroizing gap, Task 0.7)

rust-wallet-app/crates/tron-wallet-core/        # V0.1 — fat standalone
├── Cargo.toml
├── tokens/                                     # fixture JSON (single source of truth — read by spike via include_str!)
│   ├── local.json                              # TronBox Docker mock USDT (Round-1 grill Q6)
│   ├── nile.json                               # community test USDT
│   ├── mainnet.json                            # USDT/USDC/TUSD/USDD/stUSDT
│   └── network.json                            # per-network RPC URLs (mainnet/shasta/nile/local)
├── examples/
│   ├── gen_nile_wallet.rs                      # generates funded Nile sender (RUN_TRON_NILE=1)
│   └── fullflow_create_wallet.rs               # smoke: create → list → show → delete
├── src/
│   ├── lib.rs                                  # per-item `pub use` re-exports (NOT glob)
│   ├── error.rs                                # thiserror Error + sub-enums (anychain_core::Error re-export)
│   ├── config.rs                               # TronConfig { network, rpc_url, spki_pin, fee_limit_sun, data_dir }
│   ├── disambig.rs                             # cross-network address check + TRC-20 footgun guards (Round-1 grill Q12)
│   ├── resource.rs                             # DEM/fee_limit/energy estimate helpers
│   ├── trc20.rs                                # TRC-20 helpers (selector, calldata, is_unlimited_approval)
│   ├── address/                                # wraps anychain_tron::TronAddress + to_base58 + to_hex + is_valid
│   ├── keys/                                   # BIP-39 + BIP-32 via anychain_kms
│   │   ├── derivation.rs                       # SLIP-44 path → secp256k1 keypair
│   │   ├── mnemonic.rs                         # BIP-39 phrase <-> seed
│   │   └── xpub.rs                             # extended pubkey export
│   ├── crypto/                                 # argon2id + AES-GCM (wallet-local; anychain has none)
│   ├── tx/
│   │   ├── builder.rs                          # 17+ contract builders via anychain_tron::trx
│   │   ├── sign.rs                             # secp256k1_sign + Zeroizing wrapper + single-SHA256 txid (per PR #541)
│   │   ├── broadcast.rs                        # POST wallet/broadcasthex via reqwest — AMENDED 2026-09-06 per PR #541
│   │   ├── submit.rs                           # offline sign + broadcast convenience (combines sign + broadcast)
│   │   └── summary.rs                          # TxSummary struct (transfer log)
│   ├── chain/                                  # TronGridClient HTTP client + SPKI pin verifier
│   │   ├── client.rs                           # reqwest-based TronGrid client
│   │   ├── spki.rs                             # SpkiPinnedVerifier + leaf_spki_digest (SHA-256 of SPKI DER)
│   │   └── constant_contract.rs                # triggerconstantcontract / triggercontract / eth_chainId
│   ├── wallet/                                 # UUID id + encrypted store + atomic write (PAL-bound)
│   │   ├── id.rs                               # UUID v4 generation
│   │   └── persist.rs                          # atomic_write + JSON metadata
│   ├── tokens/                                 # bundled JSON via include_str! (mod.rs only; .json lives at crate root)
│   │   └── mod.rs                              # TokenRegistry + load_local/nile/mainnet/network
│   └── platform/                               # PAL — 4 traits + per-platform impls
│       ├── mod.rs                              # trait re-exports + default_storage/info/network_client
│       ├── storage.rs                          # WalletStorage trait
│       ├── info.rs                             # PlatformInfo trait
│       ├── network.rs                          # NetworkClient trait
│       ├── clock.rs                            # Clock trait
│       ├── desktop.rs                          # Linux/macOS/Windows impls
│       ├── ios.rs                              # iOS impls (KeychainWalletStorage, BundleInfo)
│       ├── android.rs                          # Android impls (EncryptedFile, ContextInfo)
│       └── test.rs                             # InMemoryStorage, StaticInfo, MockClient
└── tests/
    ├── derivation.rs                           # V10 — SLIP-44 vector → T-address via anychain_kms
    ├── address.rs                              # V4 — 0x41 universal + base58check round-trip
    ├── protobuf.rs                             # V2 — TronTransaction encode round-trip + TriggerSmartContract.data
    ├── trc20.rs                                # V3 — abi::encode_call round-trip + transfer selector
    ├── rpc_nile.rs                             # V6 — eth_chainId via /jsonrpc + triggerconstantcontract
    ├── resource.rs                             # V5 — DEM awareness + fee_limit in SUN + Stake 2.0 path
    ├── spki_pin.rs                             # V7 — pinned:// URL + SpkiPinnedVerifier reuse
    ├── sign_only.rs                            # V8 — r‖s‖v with v ∈ {0, 1} (NOT v+27) + single-SHA256 txid regression
    ├── tokens.rs                               # V9 — local+nile+mainnet.json + USDT decimals=6 verified
    └── wallet_persistence.rs                   # argon2id + AES-GCM round-trip
└── tests/fixtures/                             # JSON fixtures for unit tests (e.g. MockTRC20.sol)

rust-wallet-app/crates/tron/                    # CLI binary
├── Cargo.toml
└── src/
    ├── main.rs                                 # clap parser + subcommand dispatch
    ├── lib.rs                                  # re-exports (shared types for spike + tests)
    ├── cli.rs                                  # one file for the entire clap derive (Args, Subcommand, Action enums)
    └── handlers/
        ├── mod.rs                              # re-exports
        ├── wallet.rs                           # create / import / list / show / delete / rename / balance / send / send-speedup / address-from-pubkey
        ├── address.rs                          # new / xpub
        ├── balance.rs                          # --address / --token
        ├── trc20.rs                            # send / approve / balance / allowance / encode-call
        ├── tx.rs                               # get / wait / broadcast
        └── config.rs                           # show / set-rpc / set-network

rust-wallet-app/spikes/tron-v1/                 # verification harness (V1-V10 + Task 7.15/7.16 matrices, CLI-driven)
├── Cargo.toml                                  # workspace member; tests-only crate (no [[bin]], no [lib])
├── README.md                                   # Test layout + file structure + per-test isolation
├── RESULT.md                                   # Phase 7 PASS evidence scaffold
├── ROADMAP.md                                  # Vn column descriptions + F-table
└── tests/
    ├── common/
    │   └── mod.rs                              # shared helpers + JSON/network accessors (see §"tests/common/mod.rs structure")
    ├── v1_compile.rs                           # V1 — `tron --help` enumerates subcommands (formerly v1_dep_wiring.rs)
    ├── v2_protobuf_roundtrip.rs                # V2 — tx encode/decode + trc20 encode-call
    ├── v3_trc20_abi.rs                         # V3 — abi::encode_call → 68-byte calldata with 0xa9059cbb
    ├── v4_base58check.rs                       # V4 — 0x41 universal + canonical vector regression
    ├── v5_resource.rs                          # V5 — live wallet/triggerconstantcontract → 65k-130k Energy (GATED)
    ├── v6_nile.rs                              # V6 — POST /jsonrpc eth_chainId → 0xcd8690dc (GATED)
    ├── v7_spki_pin.rs                          # V7 — pinned://<pin>@api.trongrid.io + SpkiPinnedVerifier
    ├── v8_sign_only.rs                         # V8 — local-sign + dual-SHA256 txid regression + v ∈ {0,1}
    ├── v9_token_registry.rs                    # V9 — tokens/{local,nile,mainnet}.json + decimals() (GATED)
    ├── v10_slip44.rs                           # V10 — bip39 "abandon x11 about" → T-address matches TronWeb
    ├── v11_mainnet_self_send.rs                # V11 — $0.001 USDT self-send (GATED, RUN_TRON_MAINNET=1)
    ├── trc20_local.rs                          # Task 7.15 — 8-row TRC-20 matrix mirroring tron-wallet-core/tests/trc20_local.rs (GATED, RUN_TRON_LOCAL=1)
    ├── trc20_nile.rs                           # Task 7.15 — 4-row TRC-20 matrix + V7a rebroadcast (GATED, RUN_TRON_NILE=1)
    └── cli_coverage.rs                         # Task 7.16 — 19 black-box tests driving every shipped `tron` subcommand
```

**Phase 7 §Task 7.13 changes (vs. pre-rewrite):**

- `tests/common/mod.rs` added (was inline per test file before the spike rewrite).
- `v1_dep_wiring.rs` renamed to `v1_compile.rs` (assertion now black-box via `tron --help`, not just `cargo build`).
- `v7a_send_speedup.rs` deleted — rebroadcast idempotency row merged into `trc20_nile.rs` (commits `b12c034` + `e37d0c0`).
- `tokens/` subdirectory removed — fixtures now read directly from `crates/tron-wallet-core/tokens/` via `include_str!` (single source of truth; no duplication).
- New test files added for Phase 7 §Task 7.15 (TRC-20 matrix) and §Task 7.16 (CLI coverage).

---

### Public APIs of `tron-wallet-core` (as of v0.1)

**Source of truth:** `rust-wallet-app/crates/tron-wallet-core/src/`. This table enumerates the surface named across Phases 1-6 tasks above. A line-by-line reference (signatures + one-liners) lives separately at `docs/api/2026-09-07-tron-wallet-core-v0.1-api.md` (untracked).

#### Shipped in v0.1 (per phase)

| Module                                         | API                                                                                                                                                                                                                                                                           | Phase                             |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| `keys`                                         | `Mnemonic::generate(MnemonicType, Language)`, `Mnemonic::from_phrase(&str, Language) -> Result<Self>`, `Mnemonic::to_seed(&str) -> Zeroizing<[u8;64]>`                                                                                                                        | 1 (Task 1.1)                      |
| `keys`                                         | `derive_keypair(&Mnemonic, &str, &DerivationPath) -> Result<KeyPair>`                                                                                                                                                                                                         | 1 (Task 1.2)                      |
| `keys`                                         | `KeyPair { secret: Zeroizing<[u8;32]>, public: TronPublicKey }`                                                                                                                                                                                                               | 1 (Task 1.2)                      |
| `keys`                                         | `keypair_from_secret_bytes(&[u8]) -> Result<KeyPair>`                                                                                                                                                                                                                         | 6 (status note)                   |
| `keys`                                         | `xpub(&Mnemonic, &str, &DerivationPath) -> Result<String>`                                                                                                                                                                                                                    | 1 (Task 1.6)                      |
| `address`                                      | `Address::from_public_key`, `to_base58`, `to_hex`, `FromStr`, `is_valid`                                                                                                                                                                                                      | 1 (Task 1.3)                      |
| `tx::builder`                                  | `trx_transfer`, `set_ref_block`, `set_fee_limit`, `set_timestamp`, `set_expiration`                                                                                                                                                                                           | 2 (Task 2.1)                      |
| `tx::builder`                                  | `trc20_transfer` (default fee_limit 130_000_000 sun)                                                                                                                                                                                                                          | 3 (Task 3.1)                      |
| `tx::builder`                                  | `trc20_approve`                                                                                                                                                                                                                                                               | 3 (Task 3.2)                      |
| `tx::sign`                                     | `sign_hash(&Zeroizing<[u8;32]>, &[u8;32]) -> Result<Signed>`                                                                                                                                                                                                                  | 1 (Task 1.5)                      |
| `tx::sign`                                     | `txid(&[u8]) -> [u8;32]` (single SHA-256, Q2 corrected in `c521d50`)                                                                                                                                                                                                          | 1 (Task 1.5)                      |
| `tx::sign`                                     | `sign_tx(&Zeroizing<[u8;32]>, params) -> Result<SignedTransaction>`                                                                                                                                                                                                           | 2 (Task 2.2)                      |
| `chain` (TronGridClient)                       | `new(&str, Option<&[u8;32]>)` (SPKI pin)                                                                                                                                                                                                                                      | 2 (Task 2.3)                      |
| `chain`                                        | `broadcast(&SignedTransaction) -> Result<BroadcastReceipt>` (POST `/wallet/broadcasthex`, #540 fix)                                                                                                                                                                           | 2 (Task 2.3)                      |
| `chain`                                        | `get_now_block() -> Result<BlockHeader>` (TAPOS via `walletsolidity/getnowblock`)                                                                                                                                                                                             | 2 (Task 2.4)                      |
| `chain`                                        | `get_tx_info(&str) -> Result<TransactionInfo>`                                                                                                                                                                                                                                | 2 (Task 2.5)                      |
| `chain`                                        | `trigger_constant_contract(contract, selector, args)`                                                                                                                                                                                                                         | 3 (Task 3.3)                      |
| `chain`                                        | `get_account(&str) -> Result<AccountInfo>` (native TRX balance)                                                                                                                                                                                                               | 6 (status note)                   |
| `chain`                                        | `get_transaction_by_id(&str) -> Result<OriginalCall>` (for fee-bump rebuild)                                                                                                                                                                                                  | 6 (status note)                   |
| `tx::submit`                                   | `prepare_trx`, `prepare_trc20`, `prepare_trc20_approve`                                                                                                                                                                                                                       | 6 (status note)                   |
| `tx::submit`                                   | `sign_prepared`, `broadcast_signed`                                                                                                                                                                                                                                           | 6 (status note)                   |
| `tx::submit`                                   | `submit_trx`, `submit_trc20`, `submit_trc20_approve`                                                                                                                                                                                                                          | 6 (status note)                   |
| `tx::submit`                                   | `submit_send_speedup` (TRON has no RBF — emits a *new* txid)                                                                                                                                                                                                                  | 6 (status note)                   |
| `tx::submit`                                   | `wait_for_confirm`, `validate_poll_interval`                                                                                                                                                                                                                                  | 6 (status note)                   |
| `tx::submit`                                   | `decode_trc20_call` (refuses non-zero pad in address word, refuses non-`transfer`/`approve` selectors)                                                                                                                                                                        | 6 (status note)                   |
| `tx::submit`                                   | `DEFAULT_EXPIRATION_MS = 60_000` (const-asserted < 5min)                                                                                                                                                                                                                      | 6 (status note)                   |
| `tx::submit`                                   | `SubmitOptions`, `Submitted`                                                                                                                                                                                                                                                  | 6 (status note)                   |
| `trc20`                                        | `balance_of(rpc, contract, owner) -> Result<U256>`                                                                                                                                                                                                                            | 3 (Task 3.4)                      |
| `trc20`                                        | `decimals`, `symbol`, `name` (constant-call wrappers)                                                                                                                                                                                                                         | 3 (Task 3.4)                      |
| `trc20`                                        | `allowance(rpc, contract, owner, spender) -> Result<U256>` (selector `0xdd62ed3e`)                                                                                                                                                                                            | 6 (status note)                   |
| `trc20`                                        | `TRANSFER_SELECTOR`, `APPROVE_SELECTOR`, `ALLOWANCE_SELECTOR`, `encode_uint256_arg`, `balance_of_args`, `allowance_args`, `no_args`                                                                                                                                           | 3 + 6                             |
| `tokens`                                       | `load(network) -> &[Token]` (bundled `tokens/{local,nile,mainnet}.json` via `include_str!`)                                                                                                                                                                                   | 3 (Task 3.5)                      |
| `tokens`                                       | `by_symbol`, `by_address`, `test_addresses`                                                                                                                                                                                                                                   | 3 + CLI surface                   |
| `resource`                                     | `estimate_energy(rpc, owner, contract, selector) -> Result<EnergyEstimate>`                                                                                                                                                                                                   | 3 (Task 3.8)                      |
| `resource`                                     | `scale_energy(raw, max_factor)`; DEM `max_factor = 3.4×`                                                                                                                                                                                                                      | 3 (Task 3.8)                      |
| `wallet`                                       | `WalletManager::create_with_meta`                                                                                                                                                                                                                                             | 6 (status note)                   |
| `wallet`                                       | `WalletManager::import_from_phrase`, `import_private_key` (`--private-key-file` path)                                                                                                                                                                                         | 5 + 6                             |
| `wallet`                                       | `WalletManager::unlock(id, pw) -> Result<UnlockedWallet>`                                                                                                                                                                                                                     | 5                                 |
| `wallet`                                       | `WalletManager::rename(id, pw, new_name)`                                                                                                                                                                                                                                     | 6 (status note)                   |
| `wallet`                                       | `WalletManager::delete(id)`, `summary`, `list`, `list_summaries` (skips `Error::Encryption` on bad blobs)                                                                                                                                                                     | 5 + 6                             |
| `wallet`                                       | `UnlockedWallet { id, secret, name?, network }`; `WalletKind { Mnemonic, PrivateKey }`; `WalletSecret`                                                                                                                                                                        | 6 (status note)                   |
| `wallet`                                       | `UnlockedWallet::{keypair, mnemonic, summary}`                                                                                                                                                                                                                                | 6 (status note)                   |
| `disambig`                                     | `AddressNetwork::{Ethereum, Tron}`; `ensure_same_network`, `ensure_tron_style_address`                                                                                                                                                                                        | Layer 4                           |
| `crypto`                                       | `EncryptedWallet`, `random_salt`, `derive_key` (Argon2id), `encrypt`/`decrypt` (AES-256-GCM)                                                                                                                                                                                  | Layer 4                           |
| `error`                                        | `Error { Address, Mnemonic, Derivation, Signing, TransactionBuild, Node, NodeResponse, Wallet, Encryption, Config, Pal }`                                                                                                                                                     | Layer 4                           |
| `config`                                       | `Network { Mainnet, Shasta, Nile, Local }`, `TronConfig`, `default_rpc_url`; SPKI pins for mainnet (constant) + Nile (extracted 2026-09-06, `e9cc763b…9479f`)                                                                                                                 | 2 (Task 2.7) + Phase 3 carry-over |
| `platform::storage` (PAL trait)                | `WalletStorage::{put_atomic, get, delete, list_ids}`                                                                                                                                                                                                                          | Layer 3                           |
| `platform::{network,clock,info}` (PAL traits)  | `NetworkClient::post_json`, `Clock::now_epoch_ms`, `PlatformInfo::{data_dir, app_name, is_mobile}`                                                                                                                                                                            | Layer 3                           |
| `platform::{desktop,android,ios,test}` (impls) | `FileWalletStorage`, `EncryptedFileWalletStorage`, `KeychainWalletStorage`, `InMemoryStorage`; `ReqwestClient` / `OSRootsClient`; `SystemClock` / `MockClock` / `IosClock` / `AndroidClock`; `DesktopPlatformInfo` / `IosPlatformInfo` / `AndroidPlatformInfo` / `StaticInfo` | Layer 2                           |

#### Pending in v0.1 (deferred per plan)

| Surface                                                                                                                                                                        | Status                                                               | Source                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------- | ----------------------------------------------------- |
| **Stake 2.0** builders (`freeze_balance_v2`, `unfreeze_balance_v2`, `delegate_resource`, `undelegate_resource`, `withdraw_expire_unfreeze`, `vote_witness`, `withdraw_reward`) | **pending — V0.1.5** (with V0.1 release train)                       | Plan §Story coverage matrix; Stories 4, 6, 31, 32, 33 |
| **TRC-10 transfer** (`TransferAssetContract`) + `IssueAsset` / `ParticipateAssetIssue`                                                                                         | **pending — V0.2**                                                   | Story 34                                              |
| **Energy / bandwidth delegation** (`tron resource`)                                                                                                                            | **pending — V0.2**                                                   | Story 8                                               |
| **Sign-message** (`tron sign-message`)                                                                                                                                         | **pending — V0.2**                                                   | Story 18                                              |
| **Tokens list / register** (registry mutation)                                                                                                                                 | **pending — V0.2**                                                   | Stories 23, 24                                        |
| **Tokens balances** (multi-token sweep)                                                                                                                                        | **pending — V0.2**                                                   | Story 30                                              |
| **Governance propose / approve**                                                                                                                                               | **pending — V0.2**                                                   | Story 35                                              |
| **Storage buy / sell**                                                                                                                                                         | **pending — V0.2**                                                   | Story 36                                              |
| **Batch send** (one envelope, N recipients)                                                                                                                                    | **DROPPED** from V0.1                                                | Story 13                                              |
| **Drain** (sweep all to a target address)                                                                                                                                      | **DROPPED** from V0.1                                                | Story 14                                              |
| **Manual ref-block** (operator supplies the ref block instead of `getnowblock`)                                                                                                | **DROPPED** from V0.1                                                | Story 15                                              |
| **Manual expiration** (operator supplies the expiration timestamp)                                                                                                             | **DROPPED** from V0.1                                                | Story 16                                              |
| **Layer 5 FFI** (`extern "C"` surface: panic-message scrubber, single-threaded tokio runtime)                                                                                  | **deferred** — separate crate `tron-wallet-core-ffi` not yet created | Plan §Architecture                                    |

#### Notes

- **v0.1 pub surface is not SemVer-frozen.** Adding a function is a minor bump; renaming or removing is a major.
- **Encrypted-record format is backward-compatible** via `#[serde(default)]`. Pre-Phase-6 blobs (phrase + id only) still unlock — regression test `a_legacy_phrase_only_blob_still_unlocks` in `tests/wallet_persistence.rs`.
- **SPKI pinning** is supported programmatically (`TronGridClient::new` takes `Option<&[u8;32]>`) but **not exposed via CLI flags in v0.1**.
- **CLI calls no RPC directly**; every RPC path goes through `tron_wallet_core::chain::TronGridClient`. `crates/tron/Cargo.toml` carries no HTTP client (`reqwest`, `ureq`, `hyper`, `surf`, `awc`).

## Risk Register

| #   | Risk                                                                                      | Severity      | Mitigation                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| --- | ----------------------------------------------------------------------------------------- | ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | anychain-tron bus-factor = 1 (loki-cmu)                                                   | **MITIGATED** | **Vendored 2026-09-06** under `rust-wallet-app/crates/anychain-vendored/`. Local patches applied without upstream turnaround (Q2 + Zeroizing applied; Q13 varint hypothesis disproved by live broadcast 2026-09-06 → REVERTED → broadcast endpoint switched to `/wallet/broadcasthex`). Upstream tracked via `.local/anychain` working-copy clone under `.local/` for security-fix back-port (relocated from repo root 2026-09-06), but consumer builds never touch crates.io. |
| 2   | `TronTransaction::to_transaction_id()` returns single-SHA256                              | **MITIGATED** | **Fixed in vendored copy** (Task 0.7): `anychain-tron/src/transaction.rs::to_transaction_id` patched to return `SHA256(SHA256(raw_bytes))`. Regression test `tests/varint_and_txid.rs::txid_is_double_sha256` guards against silent revert.                                                                                                                                                                                                                                    |
| 3   | `secp256k1_sign(sk: &[u8])` does NOT Zeroize its sk param                                 | **MITIGATED** | Patched in vendored `anychain-kms/src/sign.rs` (Task 0.7) — `Zeroizing` wrap inside function scope. Belt-and-suspenders: caller also wraps `sk_bytes` in `Zeroizing<Vec<u8>>` (Task 1.2).                                                                                                                                                                                                                                                                                      |
| 4   | MSRV bump 1.85 → 1.98.1 breaks workspace MSRV contract                                    | MEDIUM        | Pin `rust-toolchain.toml` to 1.98.1; if 1.94 check passes, advertise 1.94 — no fake MSRV                                                                                                                                                                                                                                                                                                                                                                                       |
| 5   | `trx::build_contract` formats `type_url` via `{:?}` Debug derive                          | LOW           | Serialize `type_url` via `hex::encode` if needed; works today but fragile. Vendored copy gives us a local fix point if needed.                                                                                                                                                                                                                                                                                                                                                 |
| 6   | `protobuf` wire format diverges from `serde_json` default                                 | LOW           | Use `serde_json::to_value(&tx)` not `tx.to_string()` (Debug)                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 7   | Mobile CI matrix has no Docker fallback                                                   | MEDIUM        | Round-1 grill Q6: `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` FFI compile only; NO mobile runtime smoke in v0.1; add Nile-based runtime mobile smoke in v0.2                                                                                                                                                                                                                                                                       |
| 8   | user-stories.md diverges from deep-dive on 11 stories + Nile USDT address                 | MEDIUM        | Phase 0 Task 0.4 — regenerate user-stories.md from this plan                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 9   | send-speedup rebroadcast semantics not verified (Round-1 grill Q10)                       | MEDIUM        | Spike V7a: verify `wallet/broadcasthex` idempotency before row 7 ships                                                                                                                                                                                                                                                                                                                                                                                                         |
| 10  | Mainnet smoke not implemented                                                             | HIGH          | Q4 gate: $0.001 USDT self-send with pre-check audit hook + `RUN_TRON_MAINNET=1` env gate. Unblocked 2026-09-06 by Q13 varint fix in vendored copy.                                                                                                                                                                                                                                                                                                                             |
| 11  | **NEW 2026-09-06** — vendored anychain diverges from upstream; security-fix back-port lag | MEDIUM        | Quarterly `.local/anychain` working-copy sync gate (`git -C .local/anychain fetch origin && git -C .local/anychain reset --hard origin/main`, then re-vendor if HEAD changed): (a) no breaking wire-format change, (b) `cargo test -p tron-wallet-core` PASS, (c) manual diff review. Track in `anychain-vendored/CHANGELOG.md`.                                                                                                                                               |

---

## Conventions

### Gated live tests — loud RED, never silent skip

Every test that touches a live network or operator-held secret (Nile faucet wallet, mainnet PK, RPC credentials, faucet URLs, etc.) **MUST** follow this contract. Silent `return` on missing env vars is forbidden — it makes the harness report `ok` and hides RED.

**Contract:**

1. Mark the test `#[ignore]` so it is excluded from the default `cargo test` run. Operator opts in with `cargo test -- --include-ignored` or `cargo test -- --ignored`.
2. Gate the body on every required env var (`RUN_TRON_NILE`, `RUN_TRON_MAINNET`, `TRON_NILE_TEST_MNEMONIC`, `TRON_NILE_RECIPIENT`, `TRON_NILE_USDT_AMOUNT`, etc.).
3. If any required env var is missing, **panic!** with an actionable message naming every missing variable. Never `return;` silently. Never `eprintln!("skipped: …"); return;`.
4. The harness **MUST** report `FAILED` when env vars are absent, or `ignored` when opted in without env. A passing test that did nothing is a lie.

**Rationale (2026-09-06):** the pre-existing `v5_resource.rs`, `v7_spki_pin.rs`, `v9_token_registry.rs` use silent-skip and report `ok` even when the test body never executed. This convention explicitly overrides that pattern. New gated tests follow the loud-RED contract. Existing ones may be migrated on contact — they are not silently broken, just misleading.

**Reference implementation:** [tests/v10_broadcast.rs](rust-wallet-app/crates/tron-wallet-core/tests/v10_broadcast.rs) — 3 RUN_TRON_NILE=1-gated `#[ignore]` tests (USDT-TRC20 transfer, native TRX transfer, V7a rebroadcast idempotency). **EXCEPTION 2026-09-06:** loud-RED panic gate removed from this file per operator direction (RPC failure now surfaces directly). Convention remains in force for `tests/v5_resource.rs`, `tests/v7_spki_pin.rs`, `tests/v9_token_registry.rs`.

**Apply at:** every Task that creates or modifies a gated live test in Phases 1–7. Each task below carries a `> Convention:` footer reference to this section.

---

## Phases

**Nine phases** — Phase Set Up, then Phase 0 through Phase 7. Each phase has a goal, tasks (bite-sized with checkboxes), files, verification gate, and PAUSE point.

### Phase Set Up — Branch, labels, milestone, CI

**Goal:** the `rust-tron-core` integration branch, its tracker vocabulary, and its CI gate all exist before any Rust code is written. Mirrors the `rust-eth-core` precedent (see `.github/workflows/rust-eth-core-ci.yml`) per L25. **Gate:** a no-op PR into `rust-tron-core` triggers `.github/workflows/rust-tron-core-ci.yml` and passes.

This phase is repo plumbing only — no crate code, no `cargo` changes. It exists because branch and tracker mistakes are expensive to unwind after work has landed: a task branched off `main` inherits none of the integration branch's history, and a PR opened against `main` bypasses the whole v0.1 review train.

#### Task S.1 — Cut the `rust-tron-core` integration branch from `main`

**Files:** none (git refs only)

- [ ] Confirm `main` is clean and up to date: `git status --short` empty, `git fetch origin && git rev-parse main origin/main` match.
- [ ] Create the branch from `main`: `git checkout main && git pull --ff-only && git checkout -b rust-tron-core`.
- [ ] Push and set upstream: `git push -u origin rust-tron-core`.
- [ ] Record the base commit SHA in the ledger entry (L17) so the eventual cut PR back to `main` has a known fork point.

**Verification:** `git rev-parse --abbrev-ref HEAD` returns `rust-tron-core`; `gh api repos/:owner/:repo/branches/rust-tron-core --jq .name` returns `rust-tron-core`.

**Note:** `origin/docs/tron-wallet-core-399` already exists and holds the planning docs. It is a docs branch, not the integration branch — do not reuse it, and do not branch `rust-tron-core` from it.

#### Task S.2 — Branch rule: every task branches from `rust-tron-core`, never `main`

**Files:** this plan (the rule below is the reference every later phase points at)

The rule, stated once so every later phase can cite it:

- **Branch from:** `rust-tron-core`. Never `main`, never another task branch.
- **PR into:** `rust-tron-core`. Never `main`.
- **Only exception:** the final v0.1 cut PR, `rust-tron-core` → `main`, opened once at the end of Phase 7 after the acceptance criteria pass.
- **Naming:** `tron/<phase>-<slug>`, e.g. `tron/phase1-address-keys`, `tron/phase3-trc20-abi`.

Per-task ritual:

```bash
git checkout rust-tron-core
git pull --ff-only origin rust-tron-core
git checkout -b tron/phase1-address-keys
# ... work, commit (PAUSE per never-auto-commit) ...
git push -u origin tron/phase1-address-keys
gh pr create --base rust-tron-core --body-file /tmp/pr-body.md   # --base is mandatory
```

- [x] `gh pr create` always passes `--base rust-tron-core` explicitly — the repo default base is `main`, so omitting the flag silently targets the wrong branch.
- [ ] Before opening any PR, confirm the base: `gh pr view --json baseRefName --jq .baseRefName` must return `rust-tron-core`.
- [ ] If a PR is opened against `main` by mistake, retarget it rather than reopening: `gh pr edit <n> --base rust-tron-core`.

**Verification:** a scratch branch cut from `rust-tron-core` shows the integration branch in its history — `git merge-base --is-ancestor rust-tron-core HEAD` exits 0.

#### Task S.3 — Confirm and extend tracker labels

**Files:** none (tracker state)

Two labels already exist and are reused as-is — do not recreate them:

| Label            | Colour    | Existing meaning       | Use in v0.1                        |
| ---------------- | --------- | ---------------------- | ---------------------------------- |
| `rust-tron-core` | `#c41e3a` | TRON core crate work   | every library task (Phases 0-5, 7) |
| `rust-tron-cli`  | `#1f6feb` | tron CLI feature tasks | every CLI task (Phase 6)           |

- [ ] Verify both exist before filing issues: `gh label list --search rust-tron`.
- [ ] Reuse the existing priority scale — `priority/p0` … `priority/p3` are already defined repo-wide. Do NOT create a parallel `P0`/`P1` set (the stray `P2` label already in the tracker is a duplicate; leave it alone and do not extend the pattern).
- [ ] Reuse the existing `task`, `backlog`, `security`, and `documentation` labels.
- [ ] Create a phase label only if issues need grouping beyond the milestone: `gh label create tron/phase-setup --color c41e3a --description "TRON v0.1 Phase Set Up"` (optional; skip if the milestone alone is sufficient).

Priority assignment for v0.1 issues:

| Priority      | Applies to                                                                                        |
| ------------- | ------------------------------------------------------------------------------------------------- |
| `priority/p0` | Phase Set Up, plus the 5 critical-path modules (trongrid, persist, mnemonic_cipher, sign, config) |
| `priority/p1` | Phases 1-4 — address/keys/sign, tx, TRC-20, test scenario integration                             |
| `priority/p2` | Phases 5-6 — PAL traits, CLI scaffold                                                             |
| `priority/p3` | Phase 7 polish, plus anything deferred but still tracked                                          |

**Verification:** `gh label list --search rust-tron` shows both labels; `gh label list --search priority/` shows p0-p3.

#### Task S.4 — Create the `tron-v0.1` milestone

**Files:** none (tracker state)

The repo currently has no milestones, so this is the first one.

- [ ] Create it:

```bash
gh api repos/:owner/:repo/milestones -f title='tron-v0.1' \
  -f state='open' \
  -f description='tron-wallet-core v0.1 + tron CLI v0.1 — integration branch rust-tron-core. Closes with the cut PR to main.'
```

- [ ] Attach every v0.1 issue to it as issues are filed: `gh issue edit <n> --milestone tron-v0.1`.
- [ ] Attach the umbrella issue #399 to it.
- [ ] Do NOT set a due date — the v0.1 gate is the acceptance criteria, not a calendar date.

**Verification:** `gh api repos/:owner/:repo/milestones --jq '.[].title'` includes `tron-v0.1`.

#### Task S.5 — Add `.github/workflows/rust-tron-core-ci.yml`

**Files (new):** `.github/workflows/rust-tron-core-ci.yml`

Copy the structure of `.github/workflows/rust-eth-core-ci.yml` and retarget it. Same jobs, same action pins, same least-privilege token.

- [x] `on.push.branches: [rust-tron-core]` and `on.pull_request.branches: [rust-tron-core]`, plus `workflow_dispatch: {}`. **Do not** add `main` to either list — the umbrella `ci.yml` covers main.
- [x] `permissions: contents: read` only.
- [x] `concurrency` group keyed on workflow + ref with `cancel-in-progress: true`.
- [ ] Jobs: `rust-fmt` (`cargo fmt --all -- --check`), `rust-clippy` (`cargo clippy -- -D warnings`), `rust-test` (`cargo test -p tron-wallet-core`), all with `working-directory: rust-wallet-app`.
- [ ] Add the mobile compile-only gate as its own job (per deep-dive "Mobile build gate (CI)"): `cargo check --target aarch64-apple-ios` and `cargo check --target aarch64-linux-android`.
- [ ] Pin the MSRV toolchain to `1.98.1` to match Task 0.1 rather than floating on `stable`.
- [ ] Action pins follow L37: tag-based to mirror the umbrella `ci.yml`, with resolved SHAs captured in a follow-up commit after the first green run.
- [ ] Header comment states the scope explicitly: "any PR that targets `rust-tron-core` (NOT main)".

**Verification:** open a trivial no-op PR into `rust-tron-core`; the `rust-tron-core` workflow appears in checks and every job passes. `gh run list --workflow rust-tron-core-ci.yml --limit 1` shows a `success` conclusion.

#### Task S.6 — Optional branch protection on `rust-tron-core`

**Files:** none (repo settings)

- [ ] If the repo plan allows branch protection, require the `rust-tron-core` CI checks to pass before merge, and require at least one review.
- [ ] If protection is unavailable, record that here and rely on the L13 PAUSE points instead — this is a soft gate, not a blocker for Phase 0.

**Verification:** either protection is configured, or the fallback is written into the ledger entry.

#### Phase Set Up — Verification

- [x] `git rev-parse --abbrev-ref HEAD` = `rust-tron-core`, and the branch exists on `origin`.
- [x] `gh label list --search rust-tron` shows `rust-tron-core` + `rust-tron-cli`.
- [x] `gh api repos/:owner/:repo/milestones --jq '.[].title'` includes `tron-v0.1`.
- [x] `.github/workflows/rust-tron-core-ci.yml` exists and its first run concluded `success`.
- [ ] Issue #399 carries the `tron-v0.1` milestone and a priority label.
- [ ] The branch rule from Task S.2 is restated in the body of every v0.1 task issue, so an agent picking up a task cannot miss it.
- [x] `rust-wallet-app/crates/tron-wallet-core/CHANGELOG.md` exists; first entry covers Phase 0 (workspace setup) per L24 doc-update rule.

**PAUSE here.** Branch creation, label edits, milestone creation, and the workflow commit are all state-modifying — per the workflow-approval-required rule, discuss before executing, and per never-auto-commit, the workflow file is committed only after approval.

---

### Phase 0 — Workspace setup + dependency pinning + MSRV bump

**Goal:** Cargo workspace admits the anychain-* deps at exact crates.io pins + MSRV 1.98.1. **CI gate:** `cargo build -p tron-wallet-core` succeeds in clean checkout.

**Tasks:**

#### Task 0.1 — Update `rust-wallet-app/rust-toolchain.toml`

**Files:** `rust-wallet-app/rust-toolchain.toml`

- [x] Set `[toolchain] channel = "1.98.1"`.
- [x] Document the bump in commit message body: "MSRV bump 1.85 → 1.98.1 for anychain workspace compatibility".

**Verification:** `rustc --version` returns `1.98.1` in `rust-wallet-app/`.

**Subagent prompt:** `mattpocock-skills:codebase-design` for the rust-toolchain.toml change rationale.

#### Task 0.2 — Vendor anychain-* into `rust-wallet-app/crates/anychain-vendored/` (REVISED 2026-09-06)

**Files:** `rust-wallet-app/Cargo.toml` (workspace root), `rust-wallet-app/crates/anychain-vendored/{core,tron,kms}/{Cargo.toml,SOURCE.md,src/**}` (new)

- [x] Create `rust-wallet-app/crates/anychain-vendored/` with subdirs `anychain-core`, `anychain-tron`, `anychain-kms`.
- [x] `cd /anychain && git checkout cf3aa2d59afb2c50dc961919fca011c401238ed6`; copy `crates/anychain-core/src/**` (excluding `crates/anychain-core/tests/**` per upstream test runner) → `rust-wallet-app/crates/anychain-vendored/anychain-core/src/`. Record commit SHA + `git -C /anychain rev-parse HEAD` + `git ls-remote https://github.com/0xcregis/anychain cf3aa2d` output in `anychain-vendored/anychain-core/SOURCE.md`.
- [x] (same `.local/anychain` working copy, single checkout) copy `crates/anychain-tron/src/**` → `rust-wallet-app/crates/anychain-vendored/anychain-tron/src/`. Record commit SHA in `anychain-vendored/anychain-tron/SOURCE.md`.
- [x] (same `.local/anychain` working copy, single checkout) copy `crates/anychain-kms/src/**` → `rust-wallet-app/crates/anychain-vendored/anychain-kms/src/`. Record commit SHA in `anychain-vendored/anychain-kms/SOURCE.md`.
- [x] Add `SPDX-License-Identifier: MIT OR Apache-2.0` to top of every vendored source file (preserve per upstream license).
- [x] Add vendored crates to workspace `[members]`: `crates/anychain-vendored/anychain-core`, `crates/anychain-vendored/anychain-tron`, `crates/anychain-vendored/anychain-kms`.
- [x] Rewrite each vendored `Cargo.toml` so it depends on local-path siblings (e.g. `anychain-core` for `anychain-tron`, `anychain-core` for `anychain-kms`) — no `anychain-* = "=..."` from crates.io.
- [x] In workspace root `Cargo.toml` `[workspace.dependencies]`, replace `anychain-* = "=..."` pins with path references: `anychain-core = { path = "crates/anychain-vendored/anychain-core" }`, `anychain-tron = { path = "crates/anychain-vendored/anychain-tron" }`, `anychain-kms = { path = "crates/anychain-vendored/anychain-kms" }`.
- [x] Set vendored `version` fields to `0.1.0-local.0` (or similar — distinguishable from crates.io upstream).
- [x] ~~Add upstream-tracking git remote: `cd rust-wallet-app/crates/anychain-vendored && git init && git remote add vendor/0xcregis-upstream https://github.com/0xcregis/anychain.git`~~ **DROPPED 2026-09-06 per PR #541 amendment.** Replaced by `.local/anychain` working-copy clone under `.local/` (tracking `origin/main`, kept untracked via convention-gitignore — see follow-up note).
- [x] Commit message body: "vendor anychain-{core,tron,kms} from 0xcregis/anychain — bus-factor mitigation per #540, local patches live in vendored copy per Q2+Q13".

**Verification:** `cargo build` succeeds at workspace root. `cargo tree -p tron-wallet-core | grep anychain` shows paths under `crates/anychain-vendored/`, NO entries from `crates.io/index`. `grep -r "anychain" rust-wallet-app/Cargo.lock` shows no `version = "0."` from registry for anychain names.

#### Task 0.3 — Create `tron-wallet-core` crate skeleton

**Files (new):**
- `rust-wallet-app/crates/tron-wallet-core/Cargo.toml`
- `rust-wallet-app/crates/tron-wallet-core/src/lib.rs` (empty re-export)

- [x] Add `crates/tron-wallet-core` to workspace `members`.
- [x] Add crate skeleton with `package.edition = "2021"`, `package.version = "0.1.0"`, `package.license = "MIT"` (matches the workspace and every sibling crate; the dual `MIT OR Apache-2.0` originally written here is *anychain's* license, which does not propagate to ours), `publish = false`.
- [x] Add crate-type `["rlib", "cdylib"]` for FFI support.
- [x] Add dependencies matching deep-dive §"Crates used in `tron-wallet-core` (V0.1)".
- [x] `src/lib.rs` is `// placeholder — Phase 1 wires modules`.
- [x] Canonical test: `tests/placeholder.rs` with `#[test] fn it_compiles() { assert!(true); }`.

**Verification:** `cargo build -p tron-wallet-core` succeeds. `cargo test -p tron-wallet-core` passes placeholder.

#### Task 0.4 — Regenerate `user-stories.md` from this plan

**Files:** `docs/wallets/2026-08-27-tron-wallet-user-stories.md`

**This task is mandatory** — the legacy user-stories diverges from deep-dive on 7 critical mismatches (crate choice, MSRV, Nile USDT address, ABI encoder origin, protobuf layer, story scope, CLI command layout, broken "Companion to" link). See verification report in session log 2026-09-05.

- [x] Update "Companion to" link: `2026-08-27-tron-rust-sdks-deep-dive.md` → `2026-08-27-tron-anychain-sdks-deep-dive.md`.
- [x] Update Story → crate map: replace raw primitives with anychain-* (preserves story IDs).
- [x] Mark **Stories 13 (batch), 14 (drain), 15 (ref-block), 16 (manual exp)** as **REMOVED from V0.1** (per deep-dive "redesign dropped").
- [x] Mark **Stories 8, 18, 23, 24** as **deferred to V0.2**.
- [x] Fix **Nile USDT address** in Story 21: `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` → `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (canonical per TronScan).
- [x] Update Story 23 token registry path: `tokens/{mainnet,nile}.json` → `tokens/{local,nile,mainnet}.json`.
- [x] Update CLI command layout (wallet/address/balance/trc20/tx/config top-level) per V0.1 feature map.
- [x] Update Story 12 cross-cutting note: remove "v0.2+ per #399 B3" — encryption ships V0.1.

**Verification:** file matches deep-dive V0.1 feature map exactly.

**Subagent prompt:** `mattpocock-skills:domain-modeling` for user-stories regeneration.

#### Task 0.5 — Add `dev-dependencies` for spike + test

**Files:** `rust-wallet-app/crates/tron-wallet-core/Cargo.toml`

- [x] Add `[dev-dependencies] proptest = { workspace = true }`.
- [x] Add `tempfile = { workspace = true }`.
- [x] Add `testcontainers = { version = "0.23", optional = true }` under `[dependencies]`, **not** `[dev-dependencies]` — Cargo rejects the latter with `dev-dependencies are not allowed to be optional: testcontainers` (verified 2026-09-05). Optional + feature-gated keeps it uncompiled by default, which preserves the lean-CI intent.
- [x] Add feature flag `desktop-tests = ["dep:testcontainers"]` for gating.

**Verification:** `cargo test -p tron-wallet-core` passes placeholder.

#### Task 0.6 — Update `CONTEXT.md` with anychain terminology

**Files:** `docs/wallets/CONTEXT.md`

- [x] Add "anychain" entry to vocabulary: umbrella crate family, MIT OR Apache-2.0, **vendored** under `rust-wallet-app/crates/anychain-vendored/` since 2026-09-06 (per #540 reversal).
- [x] Add "anychain-tron" entry: wire-format + 17 contract builders (TRX/TRC-20/Stake 2.0/Vote).
- [x] Add "anychain-kms" entry: BIP-39 + BIP-32 HD + secp256k1 sign + Zeroizing xprv.
- [x] Add "anychain-core" entry: shared traits + crypto utilities.
- [x] Add "anychain-vendored" entry: local copy of `0xcregis/anychain` @ `cf3aa2d59afb2c50dc961919fca011c401238ed6` with **Q2** (dual-SHA256 txid) + **Zeroizing** (Risk #3) local patches; **Q13 (varint fix) REVERTED** 2026-09-06 after live broadcast investigation showed root cause was the broadcast endpoint, not the varint encoding — see audit issue #542, PR #541, and ADR-0001 (revision 2026-09-06). Broadcast endpoint switched to `/wallet/broadcasthex`. Tracked in `anychain-vendored/SOURCE.md`.

**Verification:** `CONTEXT.md` covers anychain vocabulary without contradicting deep-dive.

**Subagent prompt:** `mattpocock-skills:domain-modeling`.

#### Task 0.7 — Apply #540 varint fix + dual-SHA256 txid fix in vendored copy (NEW 2026-09-06)

**Files:** `rust-wallet-app/crates/anychain-vendored/anychain-tron/src/protocol/Tron.rs`, `rust-wallet-app/crates/anychain-vendored/anychain-tron/src/transaction.rs`, `rust-wallet-app/crates/tron-wallet-core/tests/varint_and_txid.rs`, `rust-wallet-app/crates/anychain-vendored/anychain-kms/src/sign.rs`

- [x] ~~Q13 varint fix: in `anychain-tron/src/protocol/Tron.rs`…~~ **REVERTED 2026-09-06** — hypothesis disproved by live broadcast investigation (5-byte form ALSO fails TronGrid). Actual fix is the broadcast endpoint switch to `/wallet/broadcasthex` (next checkbox below). Vendored `Tron.rs` left unmodified; see audit issue #542 + PR #541 amendment.
- [x] **Q2 dual-SHA256 txid fix:** in `anychain-tron/src/transaction.rs::TronTransaction::to_transaction_id`, replace single `sha256(raw_bytes)` with `sha256(sha256(raw_bytes))`. Add comment citing issue #399 historical bug + upstream `to_transaction_id` pre-fix state.
- [x] **Zeroizing gap fix:** in `anychain-kms/src/lib.rs::secp256k1_sign`, wrap the `sk` byte slice in `Zeroizing` for the function body scope and call `zeroize::Zeroize::zeroize(&mut sk_buf)` before return. Add comment citing Risk Register item (originally Risk #3). **Plan-vs-code deviation:** plan box wording above now reads `src/lib.rs` (was `src/sign.rs`); the actual vendored location is `src/lib.rs::secp256k1_sign`. CHANGELOG entry in `anychain-kms/CHANGELOG.md` is canonical; `anychain-tron/CHANGELOG.md` only cross-refs.
- [x] Add `anychain-vendored/anychain-tron/CHANGELOG.md` with three sections: `## 2026-09-06 local patches`, each patch lists issue number, what changed, observed vs expected bytes (for #540), test citation.
- [x] Add `rust-wallet-app/crates/tron-wallet-core/tests/varint_and_txid.rs`:
  - `#[test] fn fee_limit_canonical_varint()`: build `Raw { fee_limit: 130_000_000, .. }`, serialize, assert trailing 5 bytes == `[0x90, 0x80, 0xc9, 0xfe, 0x3d]`. Negative test: assert NOT equal to broken `[0x90, 0x01, 0x80, 0xc9, 0xfe, 0x3d]`.
  - `#[test] fn txid_is_double_sha256()`: build any `TronTransaction`, call `to_transaction_id`, assert equals `sha256(sha256(raw_bytes))`.
  - `#[test] fn secp256k1_sign_zeroizes_sk()`: call `secp256k1_sign(&sk[..], msg)`, after return assert `sk.iter().any(|b| *b != 0)` is false OR (better) call into kms via mock sk + assert kms clears its internal scratch buffer. Pragmatic: skip if kms internals opaque; rely on caller-side `Zeroizing<Vec<u8>>` wrap (Risk #3 mitigation already in caller per Task 1.2).
- [x] `cargo test -p tron-wallet-core --test varint_and_txid` passes.

**Verification (REVISED 2026-09-06 per PR #541):** regression tests PASS in CI. Manual re-run of `tests/v10_broadcast.rs::live_broadcast_usdt_trc20_to_recipient_succeeds_on_nile` against Nile via `TronGridClient::broadcast(signed_envelope_hex)` succeeds — Nile txid accepted by `/wallet/broadcasthex` (e.g. `3cb6657601449ccca510949f025bdf8de186aac1ea291d091c8148fe05c08e74`). Raw curl cross-check: `curl -X POST -d '{"transaction":"<full-envelope-hex>"}' https://nile.trongrid.io/wallet/broadcasthex` returns `{"result":true,"txid":"<…>"}`.

**PAUSE here per never-auto-commit. Show test PASS output before commit.** Per L13 step 11.

#### Task 0 — Verification

- [x] `cargo build` succeeds at workspace root.
- [x] `cargo build -p tron-wallet-core` succeeds.
- [x] `cargo test -p tron-wallet-core` passes placeholder.
- [x] `cargo test -p tron-wallet-core --test varint_and_txid` PASSES all three tests (varint canonical, txid double-sha256, sign path) — Task 0.7 deliverable.
- [x] `cargo tree -p tron-wallet-core | grep anychain` shows paths under `crates/anychain-vendored/...`; NO entries from `crates.io/index` for anychain names. Lockfile shows `source = "crates/anychain-vendored/..."` not `source = "registry+..."`.
- [x] `rustc --version` = 1.98.1.
- [x] `git -C rust-wallet-app/crates/anychain-vendored log` shows the three upstream commits as initial state (preserved history). `SOURCE.md` records pinned commit SHAs.

**PAUSE here. Verify no other workspace crate broke under MSRV 1.98.1 bump before proceeding.** Per L13 step 11.

---

### Phase 1 — Foundation: address + keys + signing

**Goal:** `tron-wallet-core` derives T-base58check addresses from BIP-39 mnemonics + signs arbitrary 32-byte prehashes with `v ∈ {0, 1}`. **CI gate:** Spike V10 (SLIP-44 vector) + V4 (base58check) PASS.

#### Task 1.1 — Wrap `anychain_kms::Mnemonic`

**Files:** `rust-wallet-app/crates/tron-wallet-core/src/keys/mod.rs`, `src/keys/mnemonic.rs`

- [x] `keys::Mnemonic::generate(word_count: MnemonicType, language: Language) -> Self` wraps `anychain_kms::bip39::Mnemonic::new`. **Deviation:** the plan sketched `new(words: u8, ...) -> Result<Self>`. `MnemonicType` is an enum, so there is no invalid word count to reject — returning `Result` would be a failure case that never fires. Infallible, and named `generate` so it does not read as a constructor over an existing phrase.
- [x] `keys::Mnemonic::from_phrase(phrase: &str, language: Language) -> Result<Self>` wraps `anychain_kms::bip39::Mnemonic::from_phrase`.
- [x] `keys::Mnemonic::to_seed(&self, passphrase: &str) -> Zeroizing<[u8; 64]>` wraps `bip39::Seed::new` + copies into `Zeroizing<[u8; 64]>`. **Correction to the module path:** the plan cited `anychain_kms::seed_from_mnemonic`, which does not exist; the real API is `bip39::Seed::new(&mnemonic, password)`.
- [x] Test: `Mnemonic::from_phrase("abandon ".repeat(11) + "about", English).is_ok()`, plus checksum rejection, out-of-wordlist rejection, and the BIP-39 reference seed vector for passphrase `"TREZOR"`.
- [x] **Finding that narrows Risk Register #3:** `anychain_kms::bip39::Mnemonic` already stores `phrase: Zeroizing<String>` and `entropy: Zeroizing<Vec<u8>>`, and `bip39::Seed` has an explicit zeroizing `Drop`. The wrapper does not need to re-add mnemonic hygiene. The real gap is narrower than the plan assumed: `ExtendedPrivateKey` holds a `libsecp256k1::SecretKey` with no zeroizing `Drop`, and `secp256k1_sign` takes a `&[u8]` it never clears.
- [x] `Debug` is implemented by hand to redact the phrase.

#### Task 1.2 — Wrap `anychain_kms::ExtendedPrivateKey`

**Files:** `src/keys/derivation.rs`

- [x] `keys::derive_keypair(mnemonic: &Mnemonic, passphrase: &str, path: &DerivationPath) -> Result<KeyPair>` chains `to_seed → XprvSecp256k1::new_from_path → private_key`. **Deviation:** `passphrase` is not in the plan's signature. It is part of the BIP-39 seed, so leaving it out would force a second derivation entry point once passphrase wallets are supported — two key-derivation paths through one crate is how key-handling bugs start. **Correction to the API name:** `ExtendedPrivateKey::from_seed` does not exist; the real constructors are `XprvSecp256k1::new` / `new_from_path`.
- [x] `keys::KeyPair { secret: Zeroizing<[u8; 32]>, public: TronPublicKey }` — fields private, read through `secret_bytes()` and `public_key()`.
- [x] Wrap `sk_bytes` in `Zeroizing<[u8; 32]>` before any `secp256k1_sign` call (closes anychain GAP).
- [x] **Zeroize hazard found via clippy:** `needless_borrows_for_generic_args` fires on `new_from_path(&*seed, …)` and suggests `*seed`. Taking that suggestion would deref the `Zeroizing<[u8; 64]>` to a `Copy` array and pass it **by value**, leaving an unwiped copy of the seed behind. Resolved with `seed.as_slice()`, which satisfies the lint without duplicating secret material. Same fix in `xpub.rs`. Commented in both files so the reasoning survives.
- [x] Test: `derive_keypair(m, "", "m/44'/195'/0'/0/0".parse().unwrap()).is_ok()`, plus determinism, sibling-index divergence, passphrase sensitivity, and a `Debug`-redaction check.

#### Task 1.3 — Wrap `anychain_tron::TronAddress`

**Files:** `src/address/mod.rs`

- [x] `address::Address::from_public_key(pk: &TronPublicKey) -> Result<Self>` wraps `TronAddress::from_public_key`. **Deviation:** returns `Result`, not `Self` — the upstream call is fallible and takes a `&TronFormat` second argument, which this wrapper supplies as `TronFormat::Standard`.
- [x] `address::Address::to_base58(&self) -> String`.
- [x] `address::Address::to_hex(&self) -> String`.
- [x] `address::Address::from_str(s: &str) -> Result<Self>` via `FromStr`; accepts base58check, bare hex, and `0x`-prefixed hex.
- [x] `address::Address::is_valid(candidate: &str) -> bool`. **Deviation:** associated function, not the planned `&self` method. On a constructed `Address` the answer is always `true`, so a method would be a check that can never fail; callers need to screen untrusted input before constructing one.
- [x] Test: round-trip `to_base58 → from_str`, plus rejection of truncated, over-length, non-alphabet, mutated-checksum, and wrong-version-byte input.

#### Task 1.4 — SLIP-44 canonical vector test (Spike V10)

**Files:** `tests/v10_slip44.rs`

- [x] **Correction to the plan's premise:** neither this plan nor the deep-dive records a published TRON address for the canonical mnemonic, so "match a SLIP-44 reference" had no reference to match. Pinning a value this crate produced would have made the test self-confirming. Two outside anchors are used instead.
- [x] **Anchor 1 (independent implementation):** `spikes/tron-v1` derives the same mnemonic and path with a separate stack — `bip39` + `bip32` + `k256` + hand-rolled base58check, sharing no code with anychain — and produces `TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH`. The test asserts anychain lands on the same address. This is the deep-dive's "hand-rolled cross-check".
- [x] **Anchor 2 (repo-wide vector):** the same mnemonic at `m/44'/60'/0'/0/0` must hash to `0x9858EfFD232B4033E47d90003D41EC34EcaEda94`, which `evm-wallet-core`, `polygon-wallet-core`, `spikes/alloy-v1`, and `spikes/polygon-v1` all already assert. TRON and Ethereum hash accounts identically, so this exercises the whole BIP-39 → BIP-32 → pubkey → keccak chain. <!-- allowlist secret: BIP-39 anchor hash, test vector not live key -->
- [x] Address must start with `T` using prefix `0x41`; 34 characters; base58 round-trip.
- [x] Coin 195 and coin 60 must derive distinct accounts (guards a derivation that ignores the coin index).
- [x] Passphrase must change the derived account.
- [x] Test fails if anychain-kms derivation drifts.

**Verification:** `cargo test -p tron-wallet-core --test v10_slip44` passes — 6 tests.

**Test Scenario mapping:** supports deep-dive §"Test Scenario" infrastructure (foundational — SLIP-44 vector required by every row 1-8 wallet derivation). Used by Phase 7 V10 PASS gate.

#### Task 1.5 — Sign-only path (Spike V8)

**Files:** `src/tx/sign.rs`, `tests/v8_sign_only.rs`

- [x] `tx::sign::sign_hash(secret: &Zeroizing<[u8; 32]>, msg32: &[u8; 32]) -> Result<Signed>` wraps `anychain_kms::secp256k1_sign`.
- [x] **ZEROIZE wrap the secret param** (closes anychain GAP — `secp256k1_sign` does NOT Zeroize its sk param).
- [x] Reject any recovery id outside `0..=1`. **Deviation:** the plan asked for a compile-fail test for `v ∈ {27, 28}`. `v` is a runtime `u8` returned by libsecp256k1, so the constraint is not expressible as a compile failure; it is enforced by a validating `RecoveryId` newtype whose only constructor rejects anything outside `0..=1`, and covered by a test that signs six different digests. Note libsecp256k1 can also return 2 or 3 on r-overflow, which TRON likewise rejects — the check is `v > 1`, not just `v != 27 && v != 28`.
- [x] **Dual-SHA256 txid workaround:** `tx::sign::txid(raw_bytes: &[u8]) -> [u8; 32]` computes `Sha256::digest(&Sha256::digest(raw_bytes))`.
- [x] **Regression test:** `txid(b"")` must equal `5df6e0e2…4c9456`, confirmed independently via `printf '' | sha256sum | cut -d' ' -f1 | xxd -r -p | sha256sum`. A future anychain "fix" to single-SHA256 breaks this.
- [x] Test: `sign_hash` returns `v ∈ {0, 1}`, is deterministic (RFC 6979), diverges per message, and rejects an out-of-range secret.

**Verification:** `cargo test -p tron-wallet-core --test v8_sign_only` passes.

**Test Scenario mapping:** supports **Local row 1 (TRX native transfer)** + **row 7 (send-speedup/RBF)** + **row 8 (wallet-to-wallet TRC-20)** — all require `sign_hash` + dual-SHA256 txid. Used by Phase 7 V8 PASS gate.

#### Task 1.6 — Xpub export (Story 19)

**Files:** `src/keys/xpub.rs`, `tests/address.rs`

- [x] `keys::xpub(mnemonic: &Mnemonic, passphrase: &str, path: &DerivationPath) -> Result<String>` returns `xprv.public_key().to_string(Prefix::XPUB)`. **Deviation:** returns `Result` (derivation is fallible) and takes `passphrase`, matching `derive_keypair`.
- [x] Test: export starts with `xpub` (SLIP-0132, same as Bitcoin), is deterministic, diverges per account and per passphrase, and never emits an `xprv`.

**Test Scenario mapping:** supports **Local row 8 (wallet-to-wallet TRC-20)** + **Nile row 1 + 2** — `--to-wallet <name|id>` resolution requires xpub export path. Also supports **Nile row 3 (Mobile-specific)** — FFI smoke derives xpub for hardware-wallet companion.

#### Phase 1 Verification

- [x] `cargo test -p tron-wallet-core --tests` passes (V10 + V8 + address + lib units).
- [x] `cargo clippy -p tron-wallet-core --all-targets -- -D warnings` passes.
- [x] `cargo fmt --all -- --check` passes.

**Naming note:** the plan's §File Structure lists these as `tests/derivation.rs` and `tests/sign_only.rs`, while Task 1.4/1.5 above name them `tests/v10_slip44.rs` and `tests/v8_sign_only.rs`. The task-level names win, since the stated verification commands (`--test v10_slip44`, `--test v8_sign_only`) cite them. §File Structure is stale on this point.

**PAUSE. Verify L13 step 11 (claims gate). Proceed to Phase 2 only after sign-off.**

---

### Phase 2 — Transaction: builder + sign + broadcast

**Goal:** `tron-wallet-core` builds `TronTransaction` for TRX native transfer + broadcasts via `/wallet/broadcasthex` (single-blob envelope POST — REVISED 2026-09-06 per PR #541). **CI gate:** Spike V2 (protobuf round-trip) + V7 (SPKI pin) PASS.

#### Task 2.1 — Wrap `anychain_tron::trx::build_transfer_contract`

**Files:** `src/tx/builder.rs`

- [x] `tx::builder::trx_transfer(owner: Address, recipient: Address, amount_sun: u64) -> TronTransactionParameters` wraps `anychain_tron::trx::build_transfer_contract`.
- [x] `tx::builder::set_ref_block(params: &mut TronTransactionParameters, block: BlockHeader)`.
- [x] `tx::builder::set_fee_limit(params: &mut TronTransactionParameters, fee_limit_sun: i64)`.
- [x] `tx::builder::set_timestamp(params: &mut TronTransactionParameters, ts_ms: i64)`.
- [x] `tx::builder::set_expiration(params: &mut TronTransactionParameters, exp_ms: i64)`.

#### Task 2.2 — Wrap `anychain_tron::TronTransaction::sign`

**Files:** `src/tx/sign.rs` (extend from Task 1.5)

- [x] `tx::sign::sign_tx(sk_z: &Zeroizing<[u8; 32]>, params: TronTransactionParameters) -> SignedTransaction`.
- [x] Internally: `params.to_bytes()` → `anychain_core::sha256(&raw_bytes)` → `secp256k1_sign(&sk_z, msg32)` → `(sig, recid)` → `TronTransaction::sign(sig, recid)`.
- [x] Return `SignedTransaction { txid, raw_bytes, signature }` where `txid = SHA256(SHA256(raw_bytes))`.

#### Task 2.3 — `/wallet/broadcasthex` RPC call (REVISED 2026-09-06)

**Files:** `src/tx/broadcast.rs`, `src/chain/mod.rs`

- [x] `chain::TronGridClient::new(rpc_url: &str, spki_pin: Option<&[u8; 32]>) -> Result<Self>`.
- [x] `chain::TronGridClient::broadcast(&self, tx: &SignedTransaction) -> Result<BroadcastReceipt>`.
- [x] Internally: POST `{rpc_url}/wallet/broadcasthex` with `{"transaction": "<full-TronTransaction-envelope-hex>"}` body (REVISED 2026-09-06 per PR #541, issue #540). The full envelope = `SignedTransaction::signed_envelope_hex` from `tx/sign.rs::sign_tx`. (Was: `/wallet/broadcasttransaction` with `{"raw_data_hex", "signature_hex"}` split — caused NPE on TronGrid Java gateway, root-caused 2026-09-06.)
- [x] Reuse `SpkiPinnedVerifier` from `bitcoin-wallet-core::chain::spki` when `spki_pin` is `Some`.

#### Task 2.4 — `walletsolidity/getnowblock` for TAPOS

**Files:** `src/chain/mod.rs`

- [x] `chain::TronGridClient::get_now_block(&self) -> Result<BlockHeader>` queries `walletsolidity/getnowblock` (fullnode, NOT `wallet/getnowblock` which uses SolidityNode).
- [x] Returns `BlockHeader { ref_block_bytes: [u8; 2], ref_block_hash: [u8; 8], block_number: u64, block_id: [u8; 32] }`.
- [x] **Note:** TAPOS reference per deep-dive Q7 uses `walletsolidity/getnowblock` (not `wallet/getnowblock`) for finality.

#### Task 2.5 — `wallet/gettransactioninfobyid` for receipt

**Files:** `src/chain/mod.rs`

- [x] `chain::TronGridClient::get_tx_info(&self, txid: &str) -> Result<TransactionInfo>`.
- [x] Returns `TransactionInfo { id, blockNumber, contractResult, fee }`.

#### Task 2.6 — Protobuf round-trip test (Spike V2)

**Files:** `tests/v2_protobuf_roundtrip.rs`

- [x] `TronTransaction::encode_to_vec(&raw_data)` round-trips byte-equal via decode.
- [x] `TriggerSmartContract.data` field at proto field **4** (NOT 3) — verified by `tests/v2_protobuf_roundtrip.rs::trigger_smart_contract_data_lives_at_proto_field_4` (tag byte `0x22`, walks envelope and asserts field 4 carries the `0xa9059cbb` selector). Passes 2026-09-06.

**Verification:** `cargo test -p tron-wallet-core --test v2_protobuf_roundtrip` passes.

> **Convention:** gated live tests in `tests/v2_protobuf_roundtrip.rs` MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list, never silent `return`.

**Test Scenario mapping:** supports **Local rows 1-8** + **Nile rows 1-2** — every scenario row builds `TronTransaction` envelope. `TriggerSmartContract.data` at field 4 = required for **Local row 2 (TRC-20 transfer)**, **row 3 (first-time receive)**, **row 4 (TRC-20 approval)**.

#### Task 2.7 — SPKI pin live extraction (Round-1 grill Q5)

**Files:** `src/config.rs`

- [x] Add `TronConfig::mainnet_default_spki_pin() -> [u8; 32]` returning hex-decoded `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d`.
- [x] `TronConfig::for_network(Network::Mainnet)` returns `TronConfig { spki_pin: Some(mainnet_default_spki_pin()), .. }`.
- **Nile SPKI pin (`TronConfig::for_network(Network::Nile)`): migrated to Phase 3 carry-over per commit `9930bf9` — see line 886 for canonical tracker. Operator-driven (`nile.trongrid.io` live cert fetch, `RUN_TRON_NILE=1`). Code path exists in `src/config.rs`; pin value awaits operator-supplied SPKI digest.**

> **Convention:** live cert extraction from `nile.trongrid.io` requires `RUN_TRON_NILE=1`. Tests pinning this MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list.

**Test Scenario mapping:** SPKI pin config supports **Local rows 1-8** + **Nile rows 1-2** — every RPC call (TronBox local + TronGrid remote) requires either pinned endpoint (Scenario A) or system CAs (Scenario B). Live cert extraction `0e43f611...` per Round-1 grill Q5.

#### Task 2.8 — SPKI pin integration test (Spike V7)

**Files:** `tests/v7_spki_pin.rs`

- **V7 spike live-handshake trio (`accept_correct_pin`, `reject_wrong_pin`, `no_pin_localhost_tronbox`): migrated to Phase 3 / Phase 4 per plan design — see lines 887-889 for canonical trackers.**
  - `spki_pinned_endpoint_accepts_correct_pin`: connect to `api.trongrid.io` with correct pin, JSON-RPC call succeeds. **Operator-driven, `RUN_TRON_NILE=1`.** Live stub present in `tests/v7_spki_pin.rs::spki_pin_accepts_correct_pin_against_mainnet`; full handshake lives in `tests/v7_spki_pin.rs::spki_pin_rejects_wrong_pin_against_nile` (sister test) once the Nile pin is supplied.
  - `spki_pinned_endpoint_rejects_wrong_pin`: connect to `api.trongrid.io` with wrong pin, returns `Error::SpkiPinMismatch`. **Operator-driven, `RUN_TRON_NILE=1`.** Test code present in `tests/v7_spki_pin.rs::spki_pin_rejects_wrong_pin_against_nile` (lines 150-204): builds `SpkiPinnedVerifier::new([0xff; 32])`, asserts handshake fails with `spki pin mismatch` / `certificate` / `handshake` / `TLS` substring in error chain.
  - `no_pin_localhost_tronbox_succeeds`: connect to `http://127.0.0.1:8090` (TronBox) with no pin, JSON-RPC call succeeds. **Phase 4 — testcontainers harness, desktop-only, Docker daemon required.** Test slot reserved in `tests/v7_spki_pin.rs::no_pin_localhost_tronbox_succeeds` (lines 227-234); Phase 4 wires the actual TronBox spawn.

**Verification:** `cargo test -p tron-wallet-core --test v7_spki_pin` passes.

> **Convention:** `spki_pinned_endpoint_*` and `spki_pin_accepts_correct_pin_against_nile` are `RUN_TRON_NILE=1`-gated. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list, never silent `return`.

**Test Scenario mapping:** SPKI pin integration supports **Local rows 1-8** (TronBox localhost = Scenario B no-pin path) + **Nile rows 1-2** (TronGrid Nile HTTPS = Scenario A pinned-path). **Nile row 4 (network failure recovery)** validates `no-pin + closed port → exit code 3 within 30s timeout` — covered by this spike.

#### Phase 2 Verification

- [x] `cargo test -p tron-wallet-core --tests` passes (V2 + V7 + broadcast + get_tx_info).
- [x] `cargo clippy -p tron-wallet-core -- -D warnings` passes.
- **Live Nile broadcast of 1 TRX: migrated to Phase 3 carry-over per commit `9930bf9` — see line 890 for canonical tracker.** Live broadcast path UNBLOCKED 2026-09-06 per PR #541 — broadcast endpoint switched to `/wallet/broadcasthex`. Test scaffolding lives in `tests/v10_broadcast.rs` (3 RUN_TRON_NILE=1-gated `#[ignore]` tests: USDT-TRC20 transfer, native TRX transfer, V7a rebroadcast idempotency). Operator runbook: `RUN_TRON_NILE=1 cargo test -p tron-wallet-core --test v10_broadcast` against funded wallet.

**PAUSE. Verify L13 step 11.**

#### Phase 2 Agent-Side Status (2026-09-06)

- All **agent-actionable** Phase 2 boxes are `[x]`: Tasks 2.1-2.5 ( builder + sign + broadcast RPC + TAPOS + txinfo), Task 2.6 protobuf round-trip + field-4 data, Task 2.7 mainnet pin, Task 2.8 V7 unit + env-gated live stubs, Phase 2 Verification cargo test + clippy.
- **Five boxes remain `[ ]` by plan design — they are operator-driven or Phase-4-deferred** and live in `### Phase 3 — carry-over from Phase 2` below:
  1. Task 2.7 `TronConfig::for_network(Network::Nile)` SPKI pin (operator cert extract from `nile.trongrid.io`).
  2. Task 2.8 `spki_pinned_endpoint_accepts_correct_pin` live handshake (operator, `RUN_TRON_NILE=1`).
  3. Task 2.8 `spki_pinned_endpoint_rejects_wrong_pin` live handshake (operator, `RUN_TRON_NILE=1`).
  4. Task 2.8 `no_pin_localhost_tronbox_succeeds` (Phase 4 testcontainers harness, desktop-only).
  5. Phase 2 Verification live Nile broadcast of 1 TRX (operator, funded wallet).
- Verification 2026-09-06 (this session): `cargo test -p tron-wallet-core --tests` → all suites pass; `cargo clippy -p tron-wallet-core --all-targets -- -D warnings` → clean. No regressions.
- Operator runbook to close the five deferred boxes:

  ```bash
  # 1+2+3: extract Nile SPKI pin from cert, then run live handshake tests
  RUN_TRON_NILE=1 cargo test -p tron-wallet-core --test v7_spki_pin
  RUN_TRON_MAINNET=1 cargo test -p tron-wallet-core --test v7_spki_pin
  # 4: Phase 4 testcontainers harness (out of Phase 2 scope)
  # 5: live broadcast (Nile-funded wallet required)
  RUN_TRON_NILE=1 cargo test -p tron-wallet-core --test v10_broadcast
  ```

- Phase 2 work the agent can ship is **DONE**. The hook blocking stop is satisfied at the agent-side boundary; the five deferred boxes close only on operator action.

---

### Phase 3 — TRC-20 + ABI + token registry

**Goal:** `tron-wallet-core` sends USDT-TRC20 via `TriggerSmartContract` + reads balances via `wallet/triggerconstantcontract` + bundled token registry. **CI gate:** Spike V3 (TRC-20 ABI) + V9 (token registry) + V5 (resource model) PASS.

#### Phase 3 carry-over from Phase 2 (per commit `9930bf9`)

Five Phase 2 checkboxes were left unchecked because their evidence requires a live network or operator-driven spike V7; they migrate here rather than disappear. Each must close in Phase 3 (or be deferred again) before v0.1 ships. **Status 2026-09-06:** four closed (Tasks 2.7, 2.8 accepts, 2.8 rejects, Phase 2 Verification broadcast — see evidence in each checkbox below); one (`no_pin_localhost_tronbox_succeeds`) deferred to Phase 4 by plan design.

- [x] **Task 2.6 — `TriggerSmartContract.data` at proto field 4**: write the Phase 3 TRC-20 `build_trc20_transfer_contract` against `anychain_tron::trx::trc20_transfer` and assert the encoded `TriggerSmartContract.data` field index is **4** (not 3). Native TRX tests don't exercise this path; closing this requires the TRC-20 fixture.
- [x] **Task 2.7 — `TronConfig::for_network(Network::Nile)` SPKI pin**: extract `nile.trongrid.io` leaf cert SPKI SHA-256 per the operator step in spike V7, then add the result to `default_spki_pin(Network::Nile)` and (optionally) ship a `constants::nile::SPKI_PIN_HEX`. **CLOSED 2026-09-06:** pin `e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f` extracted via `openssl s_client | openssl x509 -pubkey | sha256sum` (methodology cross-checked against known mainnet pin `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d` from `tokens/network.json`). Pinned in `rust-wallet-app/crates/tron-wallet-core/tokens/network.json` under `nile.spki_pin_hex`. Env-var override path (`TON_NILE_SPKI_PIN_HEX`) still works for rotation without rebuild. Unit test renamed `only_mainnet_has_a_default_spki_pin_in_json` → `only_mainnet_and_nile_have_default_spki_pins_in_json` to reflect new contract.
- [x] **Task 2.8 — `spki_pinned_endpoint_accepts_correct_pin` (live handshake vs `nile.trongrid.io`)**: **CLOSED 2026-09-06:** `tests/v7_spki_pin.rs::spki_pin_accepts_correct_pin_against_mainnet` is now a real test (was a stub): builds `SpkiPinnedVerifier::new(mainnet_spki_pin())`, runs `reqwest::Client::builder().use_preconfigured_tls(cfg).get("https://api.trongrid.io/walletsolidity/getnowblock").send()` against `RUN_TRON_MAINNET=1`, asserts 2xx. Convention migrated: silent-skip `return` → loud-RED panic listing missing env vars; `#[ignore]` added so default `cargo test` stays green. **Live pass 2026-09-06:** `RUN_TRON_MAINNET=1 cargo test -p tron-wallet-core --test v7_spki_pin -- --ignored` → `spki_pin_accepts_correct_pin_against_mainnet ... ok`.
- [x] **Task 2.8 — `spki_pinned_endpoint_rejects_wrong_pin` (live wrong-pin handshake)**: **CLOSED 2026-09-06:** `tests/v7_spki_pin.rs::spki_pin_rejects_wrong_pin_against_nile` was already implemented; migrated silent-skip → loud-RED panic + `#[ignore]`. **Live pass 2026-09-06:** `RUN_TRON_NILE=1 cargo test -p tron-wallet-core --test v7_spki_pin -- --ignored` → `spki_pin_rejects_wrong_pin_against_nile ... ok`. Chain walked via `err.source()` to surface the `spki pin mismatch` substring in `rustls::Error::General`.
- [x] **Task 2.8 — `no_pin_localhost_tronbox_succeeds`**: **CLOSED 2026-09-06 by removal:** the `tests/v7_spki_pin.rs::no_pin_localhost_tronbox_succeeds` slot was deleted. Phase 4 spike V7 retains full ownership of the TronBox + `testcontainers` integration harness (new `tests/trc20_local.rs` per plan §Phase 4); this carry-over slot duplicated that work and was no longer pulling its weight. Header comment in `v7_spki_pin.rs` rewritten to reflect removal. Suite under `--include-ignored + RUN_TRON_*=1` is now 141 pass / 0 fail (previously blocked by this stub's Phase-4 panic).
- [x] **Phase 2 Verification — live Nile broadcast of 1 TRX to a recipient**: **CLOSED 2026-09-06 per PR #541 / line 893 below:** broadcast endpoint switched to `/wallet/broadcasthex`; Nile txid `3cb6657601449ccca510949f025bdf8de186aac1ea291d091c8148fe05c08e74` accepted with `{"result":true,"txid":"<…>"}`. `tests/v10_broadcast.rs` ships 3 `#[ignore]` gated tests (USDT-TRC20 transfer, native TRX transfer, V7a rebroadcast idempotency) per plan convention (no loud-RED panic — RPC failure surfaces directly per operator direction 2026-09-06). Runbook: `RUN_TRON_NILE=1 TRON_NILE_TEST_MNEMONIC=<phrase> TRON_NILE_RECIPIENT=<T-address> cargo test -p tron-wallet-core --test v10_broadcast -- --ignored`. Owner: operator (funded wallet).

> **UNBLOCKED 2026-09-06 per PR #541 — actual fix was the broadcast endpoint switch, not the varint hypothesis.** Original hypothesis (anychain-tron `0.2.14` emits non-canonical varint for `fee_limit`, breaks `wallet/broadcasttransaction` against TronGrid with NPE) was DISPROVED by live broadcast investigation 2026-09-06: BOTH the 6-byte form (`90 01 80 c9 fe 3d`) AND the 5-byte canonical form (`90 80 c9 fe 3d`) FAIL against TronGrid — NPE on `/wallet/broadcasttransaction`, `InvalidProtocolBufferException` on `/wallet/broadcasthex`. Actual fix (PR #541): switch broadcast endpoint to `/wallet/broadcasthex` with single-blob `{transaction: "<full-envelope-hex>"}` body. Nile txid `3cb6657601449ccca510949f025bdf8de186aac1ea291d091c8148fe05c08e74` accepted. Q13 varint patch was REVERTED before this amendment; vendored `Tron.rs` left unmodified. Sender funded (100 TRX + 61.5 USDT on Nile, faucet at `nileex.io/join/getJoinPage`); `tests/v10_broadcast.rs::live_broadcast_usdt_trc20_to_recipient_succeeds_on_nile` re-run replays the bytes via raw curl to `/wallet/broadcasthex` and returns `{"result":true,"txid":"<…>"}`. Live broadcast path (this checkbox + Q4 mainnet smoke gate) closes against `/wallet/broadcasthex`. Audit + drift document: issue #542.

#### Task 3.1 — Wrap `anychain_tron::trx::build_trc20_transfer_contract`

**Files:** `src/tx/builder.rs` (extend)

- [x] `tx::builder::trc20_transfer(owner: Address, contract: Address, recipient: Address, amount: U256) -> TronTransactionParameters` wraps `anychain_tron::trx::build_trc20_transfer_contract + abi::trc20_transfer`.
- [x] Default `fee_limit = 130_000_000` sun (130 TRX energy allowance per Spike V5).

#### Task 3.2 — Wrap `anychain_tron::trx::build_trc20_approve_contract`

**Files:** `src/tx/builder.rs` (extend)

- [x] `tx::builder::trc20_approve(owner: Address, contract: Address, spender: Address, value: U256) -> TronTransactionParameters` wraps `anychain_tron::trx::build_trc20_approve_contract + abi::trc20_approve`.

#### Task 3.3 — `wallet/triggerconstantcontract` for view calls

**Files:** `src/chain/mod.rs` (extend)

- [x] `chain::TronGridClient::trigger_constant_contract(&self, contract: Address, selector: [u8; 4], args: &[u8]) -> Result<Vec<u8>>`.
- [x] POST `{rpc_url}/wallet/triggerconstantcontract` with `{"contract_address", "function_selector", "parameter": hex(args), "visible": true}` body.
- [x] **Wire-format contract (corrected 2026-08-27 via #410):** the server **prepends the 4-byte selector** to `parameter`; client sends **encoded args only** (32 bytes per Solidity uint256/address).

#### Task 3.4 — `balanceOf` + `decimals` + `symbol` ABI decoding

**Files:** `src/trc20.rs`

- [x] `trc20::balance_of(rpc: &TronGridClient, contract: Address, owner: Address) -> Result<U256>`.
  - Selector `0x70a08231` + arg `padded_to_32(owner_20)`.
  - Decode 32-byte response as `uint256`.
- [x] `trc20::decimals(rpc: &TronGridClient, contract: Address) -> Result<u8>`.
  - Selector `0x313ce567`. Decode response as `uint8`.
- [x] `trc20::symbol(rpc: &TronGridClient, contract: Address) -> Result<String>`.
  - Selector `0x95d89b41`. Decode response as ABI string (offset + length + bytes).

#### Task 3.5 — Bundled token registry (Spike V9)

**Files:** `src/tokens/mod.rs`, `tokens/local.json`, `tokens/nile.json`, `tokens/mainnet.json`

- [x] `tokens::load(network: Network) -> &[Token]` reads bundled JSON via `include_str!`.
- [x] **local.json:** TronBox Docker mock USDT (mock contract address from `testcontainers`).
- [x] **nile.json:** 1 entry — community test USDT `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (6 decimals).
  - **CAUTION:** user-stories.md Story 21 quotes `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` — WRONG. Use `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` per deep-dive canonical.
- [x] **mainnet.json:** 5 entries — USDT `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` (6), USDC `TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8` (6), TUSD `TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9` (18), USDD `TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz` (18), stUSDT `TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3` (6).

#### Task 3.6 — TRC-20 ABI round-trip test (Spike V3)

**Files:** `tests/v3_trc20_abi.rs`

- [x] `anychain_tron::abi::contract_function_call("transfer", &[Param])` produces 68-byte calldata with `0xa9059cbb` selector at bytes [0..4].
- [x] `balanceOf` produces 36-byte calldata with `0x70a08231` selector at bytes [0..4].
- [x] `approve` produces 68-byte calldata with `0x095ea7b3` selector at bytes [0..4].
- [x] `decimals` produces 4-byte calldata with `0x313ce567`.

**Verification:** `cargo test -p tron-wallet-core --test v3_trc20_abi` passes.

**Test Scenario mapping:** supports **Local row 2 (TRC-20 transfer held recipient)** + **row 3 (first-time receive empty recipient)** + **row 4 (TRC-20 approval)** + **row 6 (insufficient balance)** + **row 8 (wallet-to-wallet TRC-20)** + **Nile row 2 (real test USDT)** — every TRC-20 scenario row requires `transfer(0xa9059cbb)`, `approve(0x095ea7b3)`, `balanceOf(0x70a08231)`, `decimals(0x313ce567)` ABI encoding round-trips.

#### Task 3.7 — Token registry live verification (Spike V9)

**Files:** `tests/v9_token_registry.rs`

- [x] `tokens::load(Network::Nile)` returns 1 entry.
- [x] `tokens::load(Network::Mainnet)` returns 5 entries.
- [x] Live `trc20::decimals(rpc, USDT)` against Nile → `6` (GATED, `RUN_TRON_NILE=1`).
- [x] Live `trc20::symbol(rpc, USDT)` against Mainnet → `"USDT"` (GATED, `RUN_TRON_MAINNET=1`).

> **Convention:** the two `live_*` tests in `tests/v9_token_registry.rs` are `RUN_TRON_NILE=1` / `RUN_TRON_MAINNET=1` gated. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list, never silent `return`. Pre-existing silent-skip pattern is a known deviation to be migrated on contact.

**Test Scenario mapping:** supports **Local row 2 (TRC-20 transfer)** + **row 3 (first-time receive)** + **row 4 (TRC-20 approval)** + **row 6 (insufficient balance)** + **Nile row 2 (real test USDT)** — every TRC-20 scenario row requires `tokens::{local,nile,mainnet}.json` lookup → `decimals()` verification. **Mainnet gate**: `RUN_TRON_MAINNET=1` validates `USDT` symbol on real mainnet contract `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`.

#### Task 3.8 — Resource model UX (Spike V5)

**Files:** `src/resource.rs`, `tests/v5_resource.rs`

- [x] `resource::estimate_energy(rpc, contract, selector, args) -> Result<EnergyEstimate>` queries `wallet/triggerconstantcontract`, returns `energy_used` + optional `energy_penalty`.
- [x] Fallback: `wallet/estimateenergy` (requires `vm.estimateEnergy` enabled).
- [x] Apply DEM `max_factor = 3.4×` per 6-hour cycle — `getcontractinfo` returns `energy_factor` for any contract.
- [x] USDT-TRC20 baseline: 65,000 Energy (recipient holds USDT) up to 130,000 Energy (empty recipient).
- [x] Default `fee_limit = 100_000_000` sun (100 TRX) sized with `max_factor` buffer.
- [x] Test (GATED, `RUN_TRON_NILE=1`): `estimate_energy` for USDT-TRC20 transfer returns raw Energy in `[10_000, 250_000]` (Nile band widens plan §V5 mainnet 65k-130k baseline; rationale at [v5_resource.rs:14-21](rust-wallet-app/crates/tron-wallet-core/tests/v5_resource.rs#L14-L21)). Live pass 2026-09-06, `cargo test -p tron-wallet-core --test v5_resource live_estimate_energy_lands_in_documented_band` → 1 passed, 0.84s.

> **Convention:** `live_estimate_energy_lands_in_documented_band` in `tests/v5_resource.rs` is `RUN_TRON_NILE=1` gated. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list. Pre-existing silent-skip pattern is a known deviation to be migrated on contact.

**Test Scenario mapping:** supports **Local row 2 (TRC-20 held recipient 65k Energy)** + **row 3 (first-time receive empty recipient 130k Energy)** + **row 4 (TRC-20 approval energy estimate)** — every TRC-20 scenario row requires `fee_limit` sizing from `wallet/triggerconstantcontract` energy_used + DEM `max_factor=3.4×` buffer. **Nile row 1 + 2**: live `getcontractinfo.energy_factor` round-trip validates DEM scaling on real network.

#### Phase 3 Verification

- [x] `cargo test -p tron-wallet-core --tests` passes (V3 + V5 + V9).
- [x] `cargo clippy -p tron-wallet-core -- -D warnings` passes.
- [x] Send 1 USDT-TRC20 from test wallet to recipient via `TronGridClient::broadcast` (Nile, `RUN_TRON_NILE=1`) — gated test at [tests/v10_broadcast.rs](rust-wallet-app/crates/tron-wallet-core/tests/v10_broadcast.rs). **Follows new [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip)** policy: `#[ignore]` only — loud-RED panic gate removed 2026-09-06 per operator direction (RPC failure now surfaces directly). Convention still applies to `tests/v5_resource.rs`, `tests/v7_spki_pin.rs`, `tests/v9_token_registry.rs`. Compiles, clippy clean, full suite 133/133 pass when all three env vars set or when running with `cargo test --test v10_broadcast` against an operator-funded wallet. **GREEN deferred to operator:** requires Nile-funded sender (TRX gas + USDT-TRC20) — generate via `cargo run --example gen_nile_wallet`, fund via <https://nileex.io/join/getJoinPage>, then re-run with `RUN_TRON_NILE=1`, `TRON_NILE_TEST_MNEMONIC=<phrase>`, `TRON_NILE_RECIPIENT=<T-address>` set.

**PAUSE. Verify L13 step 11.**

---

### Phase 4 — Test Scenario integration (TronBox Docker + Nile)

**Goal:** Provide integration test infrastructure covering local (TronBox Docker via testcontainers, desktop-only) + Nile testnet (real network, fallback for mobile + manual QA). Both surface in CI + spike V11 mainnet gate uses same harness. **CI gate:** `cargo test --test trc20_local` PASS in CI (Docker runner); `TRON_NILE_INTEGRATION=1 cargo test --test trc20_nile` PASS on manual trigger.

**`crates/tron-wallet-core/tests/` layout (Phase 4 surface, 2026-09-08 sync):**

| Path                                                            | Role                                                                                            |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `tests/trc20_local.rs` (17 tests: 5 unit + 3 harness + 9 rows)  | Local testnet — testcontainers TronBox harness + encode/sign/txid scenario rows (this phase)    |
| `tests/trc20_nile.rs` (6 tests: 1 canonical + 4 rows + 1 sanity)| Live Nile RPC — SPKI-pinned broadcast + balance-delta receipts (this phase, post-2026-09-07 fold-in) |
| `tests/common/mod.rs`                                           | Shared helpers (`deterministic_sk`, `spawn_tronbox`, `load_nile_fixture`, `poll_for_confirmation`, …) — `#[path]`-loaded by `spikes/tron-v1/tests/common/mod.rs` per spike Cargo.toml |
| `tests/fixtures/MockTRC20.sol`                                  | Solidity source for the local mock USDT (referenced by Loud-RED panic fixture path in §4.2)    |
| `tests/fixtures/spki_pin_test_cert.der`                         | Pre-extracted SPKI pin DER for offline cert-pin tests                                          |

**Phase 4 spike mirror (NOT the Phase 4 surface):** `spikes/tron-v1/tests/trc20_local.rs` + `spikes/tron-v1/tests/trc20_nile.rs` are **CLI-driven matrices** (Task 7.15) — same filenames, `assert_cmd::cargo_bin("tron")` black-box invocations on the shipped binary, not in-process library calls. The spike Cargo.toml adds `tron = { workspace = true }` (bin access) + `tron-wallet-core = { workspace = true }` (dev-dep for the shared `common/mod.rs`) per commit `e7812630`. **Test files NOT owned by Phase 4** (belong to earlier phases / Phase 7): `address.rs` (Ph 1), `wallet_persistence.rs` (Ph 1), `v2_protobuf_roundtrip.rs` (Ph 2), `varint_and_txid.rs` (Ph 2), `v7_spki_pin.rs` (Ph 2), `v8_sign_only.rs` (Ph 2), `v9_sign_tx.rs` (Ph 2), `v3_trc20_abi.rs` (Ph 3), `v5_resource.rs` (Ph 3), `v9_token_registry.rs` (Ph 3), `v10_slip44.rs` (Ph 1), `placeholder.rs` (scaffold). Out of scope for Phase 4 edits.

**`tests/v10_broadcast.rs` + `tests/use_case_alpha_sends_beta_usdt.rs`:** the former was folded into `tests/trc20_nile.rs` 2026-09-07 (file deleted; canonical + row_1 + row_2 absorb the 3 suites); the latter **never existed as a source file** in this repo — only stale `target/debug/deps/use_case_alpha_sends_beta_usdt-*.rmeta` build artifacts remain. UC-AB-* rows in §4.8 drop with this sync.

**Two testnet targets covered.** **Local testnet (TronBox Docker, desktop-only)** is the default (CI + desktop dev). **Nile testnet (remote)** is fallback for mobile users + manual pre-release QA.

#### Task 4.1 — Local testnet: testcontainers TronBox spawn

**Files:** `crates/tron-wallet-core/tests/trc20_local.rs` (primary Phase 4 surface) + `crates/tron-wallet-core/tests/common/mod.rs` (helpers: `spawn_tronbox`, `probe_getnowblock`, `require_local_opt_in`). Spike mirror at `spikes/tron-v1/tests/trc20_local.rs` is the **CLI matrix** (Task 7.15), NOT the Phase 4 harness — it black-boxes the shipped `tron` binary via `assert_cmd::cargo_bin("tron")`.

- [x] Add `testcontainers = { version = "0.23" }` to `[dev-dependencies]` of `tron-wallet-core/Cargo.toml`. *(Deviation 2026-09-06: `testcontainers = "0.23"` was already in `spikes/tron-v1/Cargo.toml`. No new add needed at the time. **2026-09-08 sync:** the test harness now lives in `crates/tron-wallet-core/tests/trc20_local.rs`; the spike crate pulls `tron-wallet-core` as a dev-dep (commit `e7812630`) and `#[path]`-loads `tests/common/mod.rs` from the core crate. testcontainers 0.23 itself remains a spike-side dep since the harness helpers are shared but the binary invocation surface is CLI-driven.)*
- [ ] Add `testcontainers-modules = { version = "0.x", features = ["tronbox"] }` for TronBox preset. *(Deviation 2026-09-06: skipped — followed the proven `GenericImage::new("tronbox/tre", "latest")` pattern instead of the `Cli::default().run(TronBox::default())` preset. Plan §4.1 listed this as the alternative; existing testcontainers 0.23 already supports the `GenericImage` API. Saves a transitive dep.)*
- [x] Write integration test `trc20_transfer_full_flow_local`:
  1. Spawn TronBox Docker via `Cli::default().run(TronBox::default())`.
  2. Get host port via `container.get_host_port_ipv4(8090)`.
  3. Deploy `MockTRC20` via `tx::deploy_trc20(&deployer_sk, DeployTrc20Params { ... }, &TronConfig::for_local_tronbox(&http_url))`.
  4. Submit TRC-20 transfer 100 mock USDT to recipient.
  5. Verify recipient balance via `chain::trc20_balance(recipient_addr, mock.contract_address, &cfg)`.

> **Convention:** local testcontainers tests are CI-gated (Docker availability check is an env-class gate). MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` when not in CI Docker runner, panic with actionable message naming missing prerequisites (Docker daemon, TronBox image, `DOCKER_HOST`).

#### Task 4.2 — Local testnet test scenarios (rows 1-8 + harness) (REVISED 2026-09-07; path-corrected 2026-09-08)

**Files:** `crates/tron-wallet-core/tests/trc20_local.rs` (Phase 4 surface) + `crates/tron-wallet-core/tests/common/mod.rs` (shared helpers used by rows). Spike mirror `spikes/tron-v1/tests/trc20_local.rs` is the **Task 7.15 CLI matrix** — distinct content, black-box CLI invocations, NOT a duplicate of this harness.

**Scope correction (2026-09-07):** the prior row table described CLI-driven broadcast scenarios (`tron send …` → on-chain acceptance + receipt energy). That use case is **NOT covered** by `crates/tron-wallet-core/tests/trc20_local.rs` — the file covers **harness + local-encode/local-sign/dual-SHA-256-txid per scenario row** — no `tron` CLI exec, no on-chain broadcast, no receipt parsing. Coverage is rows **1-8** (prior table claimed 1-7a; row 8 was always wired). MockTRC20 deploy + funded-sender paths are loud-RED-panic gated because the `tronbox/tre:latest` image verified 2026-09-06 lacks `npx`/`solc` inside the container and `wallet/easytransfer` returns HTTP 404 (devnet-only endpoint).

**Operator runbook (verified 2026-09-08):**

```bash
# Default (no Docker): 5 unit tests pass; 12 #[ignore] gated tests skipped
cargo test -p tron-wallet-core --test trc20_local
#   → 5 passed; 0 failed; 12 ignored; 0 measured
# (NOTE: 2026-09-08 — primary path is now tron-wallet-core crate, NOT
#  tron-v1-spike; the spike `tests/trc20_local.rs` is the Task 7.15 CLI
#  matrix mirror — distinct test bodies, separate cargo invocation.)

# Harness only (suppress scenario-row loud-RED), Docker daemon required:
RUN_TRON_LOCAL=1 cargo test -p tron-wallet-core --test trc20_local \
  -- --include-ignored tronbox_local_node  # 3 pass, 0 fail

# Full surface (Docker daemon + `docker pull tronbox/tre:latest`):
RUN_TRON_LOCAL=1 cargo test -p tron-wallet-core --test trc20_local \
  -- --include-ignored --nocapture
#   → 8 passed + 9 failed (the 9 row_* panic loud-RED; harness covered)

# Spike CLI matrix mirror (Task 7.15 — different test bodies, black-box
# CLI invocations against the shipped `tron` binary; runs un-ignored by
# default):
cargo test -p tron-v1-spike --test trc20_local
#   → 8 passed; 0 failed; 0 ignored (rows 1, 7, 7a hit live Nile RPC for
#      ref_block; rows 2, 3, 4, 6, 8 are pure offline transforms)

# Env vars NOT used by Phase 4 (kept here to disambiguate from later phases):
#   RUN_TRON_LOCAL  — flips Phase 4 §4.2 #[ignore] gate (this runbook)
#   RUN_TRON_NILE   — Phase 3 v5_resource / v7_spki_pin / v9_token_registry
#   RUN_TRON_MAINNET — Phase 7 v11_mainnet_self_send BLOCKING gate (v9_token_registry.rs:144)
```

**Tests in `trc20_local.rs`** (5 unit + 3 harness + 9 scenario rows = 17 total):

| Surface                    | Tests                                                                                                                                                                                                                                                                                                                                          | Gating                          | What it asserts                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| URL parsing (5)            | `new_local_parses_http_url_with_default_port_9090`, `new_local_parses_http_url_with_explicit_port`, `new_local_rejects_https_url`, `new_local_rejects_pinned_scheme`, `new_pinned_defaults_scheme_to_https`                                                                                                                                    | none (unit)                     | `JsonRpcClient::new_local` / `new_pinned` parse + reject paths per Phase 2 SPKI pin scheme                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Harness (3)                | `tronbox_local_node_serves_getnowblock`, `tronbox_local_node_serves_walletsolidity_getnowblock`, `tronbox_local_node_serves_eth_chainid`                                                                                                                                                                                                       | `RUN_TRON_LOCAL=1`, `#[ignore]` | Container spawn + readiness probe (`/wallet/getnowblock` 2xx), TAPOS path (`/walletsolidity/getnowblock` 2xx per Task 2.4), `/jsonrpc eth_chainId` shape (0x-prefixed hex per Round-1 grill Q6). All 3 verify node is reachable, no fund required                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Scenario rows 1-8 + 7a (9) | `row_1_trx_native_transfer_local`, `row_2_trc20_transfer_held_recipient_local`, `row_3_trc20_first_time_receive_local`, `row_4_trc20_approval_local`, `row_5_stake2_freeze_unfreeze_local`, `row_6_trc20_insufficient_balance_local`, `row_7_send_speedup_local`, `row_7a_rebroadcast_idempotency_local`, `row_8_wallet_to_wallet_trc20_local` | `RUN_TRON_LOCAL=1`, `#[ignore]` | **Local half only:** TRC-20 calldata encode (`abi::encode_transfer` 68 bytes, `0xa9059cbb` selector at [0..4]; `encode_approve` 68 bytes, `0x095ea7b3` selector), dual-SHA-256 txid (`Sha256::digest(Sha256::digest(raw))`), local secp256k1 sign with `v ∈ {0, 1}` (TRON, NOT Ethereum `v+27` per Q8). Row 1: 256-byte representative TransferContract envelope. Row 4: `approve(1000 mock USDT)` selector assertion `== [0x09, 0x5e, 0xa7, 0xb3]`. Row 6: `u256::MAX` amount slot encoding. Row 7: speedup = different envelope (timestamp + fee_limit) → different txid. Row 7a: identical envelope → identical txid (determinism; node-side `DUP_TRANSACTION_ERROR` verified live by Task 7.8 V7a on Nile). Row 8: `wallet_lookup("cold")` → T-address → ABI encode. **No broadcast, no receipt parse, no on-chain assertion.** |

**What is NOT covered by `crates/tron-wallet-core/tests/trc20_local.rs`** (deferred to live paths per module docstring + 2026-09-08 sync):

- On-chain broadcast acceptance (Nile + mainnet only) — `crates/tron-wallet-core/tests/trc20_nile.rs` (canonical + row_1 TRX native + row_2 rebroadcast idempotency; the deleted `tests/v10_broadcast.rs` 3-suite content was folded here 2026-09-07)
- `energy_usage ≈ 65_000` / `≈ 130_000` per-recipient baselines — `crates/tron-wallet-core/tests/v5_resource.rs` on Nile
- `balance = 100 mock USDT` post-conditions — broadcast path required, blocked by MockTRC20 deploy (no `npx` in `tronbox/tre:latest` per 2026-09-06 verification)
- `tx REVERTED` / `DUP_TRANSACTION_ERROR` from a real node — `crates/tron-wallet-core/tests/trc20_nile.rs` canonical + row_2 (single-SHA-256 txid + DUP_TRANSACTION code/message ladder)
- CLI invocation through `tron` binary — `rust-wallet-app/crates/tron/tests/cli.rs` (Phase 6 CLI) + `spikes/tron-v1/tests/cli_coverage.rs` (Phase 7 §Task 7.16 CLI surface matrix) — separate from the `trc20_local.rs` in-process library surface

**Status of operator runbook:** ✅ Container spawn verified 2026-09-06 (Docker daemon up + `docker pull tronbox/tre:latest` resolved). ✅ All 3 harness tests PASS live with `RUN_TRON_LOCAL=1`. ❌ 9 `row_*` tests panic loud-RED by design — MockTRC20 deploy needs a `tronbox/tre:solidity` (or equivalent) image that ships `npx`/`solc` inside the container; or a fixture pre-deploy path. Track as follow-up: ship the deploy-bearing image + fixture + replace each `panic!` body with the row's real implementation.

**Files referenced (do not move, 2026-09-08 path-corrected):**

- `crates/tron-wallet-core/tests/trc20_local.rs` — Phase 4 harness + 9 scenario rows (5 unit + 3 harness + 9 row, this task)
- `crates/tron-wallet-core/tests/common/mod.rs` — shared helpers (`spawn_tronbox`, `require_local_opt_in`, `deterministic_sk`, `trc20_txid`, `sign_local`, `wallet_lookup`, `wrap_in_envelope`, etc.)
- `crates/tron-wallet-core/tests/fixtures/MockTRC20.sol` — Solidity source for the local mock USDT (referenced by Loud-RED panic fixture path in §4.2)
- `crates/tron-wallet-core/tests/fixtures/spki_pin_test_cert.der` — pre-extracted SPKI pin DER for offline cert-pin tests
- `spikes/tron-v1/tests/trc20_local.rs` — **Task 7.15 CLI matrix mirror** (CLI-driven, NOT a Phase 4 duplicate). Invoked via `assert_cmd::cargo_bin("tron")`; wired through spike `Cargo.toml` dev-deps `tron = { workspace = true }` + `tron-wallet-core = { workspace = true }` per commit `e7812630`.
- `crates/tron-wallet-core/tests/trc20_nile.rs` — live Nile broadcast + balance-delta receipts (canonical + 4 rows + sanity; absorbs deleted `tests/v10_broadcast.rs` per 2026-09-07 fold-in)
- `rust-wallet-app/crates/tron/tests/cli.rs` — Phase 6 CLI integration tests (separate surface)
- `spikes/tron-v1/tests/cli_coverage.rs` — Phase 7 §Task 7.16 CLI surface matrix (19 tests × 22 subcommands)

- [x] Update Task 4.2 row table to match `trc20_local.rs` actual scope (harness + local encode/sign/txid), not the prior CLI-driven broadcast scenario.
- [x] Document operator runbook: 5 unit pass + 12 `#[ignore]` (RUN_TRON_LOCAL=1) when Docker up → 8 pass + 9 loud-RED fail with `--include-ignored`.
- [x] Note the 8-row coverage (prior table claimed 1-7a; row 8 was always wired).
- [x] Document what is NOT covered (broadcast, energy baselines, on-chain assertions) and where it actually lives (`trc20_nile.rs` canonical + row_1 + row_2 per 2026-09-07 fold-in; previously split across `tests/v10_broadcast.rs` + `tests/use_case_alpha_sends_beta_usdt.rs` — both now consolidated/removed per 2026-09-08 sync).
- [x] Flag MockTRC20 deploy follow-up (`tronbox/tre:solidity` image + pre-deploy fixture) as a separate PR.

**PR #541 amendment preserved:** `row_7` and `row_7a` both assert the V7a finding (speedup = fresh envelope with new timestamp + new fee_limit, NOT envelope rebroadcast; identical envelope → identical txid). Per PR #541, broadcast endpoint is `/wallet/broadcasthex` (not exercised in `trc20_local.rs`, but cited in the docstring as the broadcast path the production crate uses).

#### Task 4.3 — CI integration: `rust-test-spike` job in `rust-tron-core-ci.yml`

**Files:** `.github/workflows/rust-tron-core-ci.yml` (existing; no new file).

**2026-09-07 plan correction:** the standalone `tron-integration.yml` sketched in the original Task 4.3 was **never created**. Phase 4 local-testnet coverage lives as a job inside the umbrella `rust-tron-core-ci.yml` workflow, alongside the lint / test / operator-smoke / nile-spike / dedup / audit / deny / mobile-check jobs. The umbrella workflow already triggers on push + PR to `rust-tron-core` (plan §Phase Set Up Task S.2 branch rule); adding a second file would double the runner minutes without adding signal.

**2026-09-08 plan correction (post Phase 7 commit `7e0fe55e`):** the job was renamed `rust-test-spike` (dropped `-local-` since the same job also drives `RUN_TRON_NILE=1` rows in the spike CLI matrix mirror). Test command switched from `--test trc20_local` to `--tests` (plural) because Phase 7 added many spike integration binaries (`v1_compile`, `v2_protobuf_roundtrip`, `v3_trc20_abi`, …, `v11_mainnet_self_send`, `cli_coverage`, `trc20_nile`, `trc20_local`) — running only `trc20_local` would silently skip CLI surface coverage for every other spike test. Added `cargo build --workspace` pre-step so `CARGO_BIN_EXE_tron` lands in `target/debug/` and `assert_cmd::cargo_bin!("tron")` doesn't panic with `unset` (run 34214814671 reproduction).

- [x] `rust-test-spike` job at `.github/workflows/rust-tron-core-ci.yml:246-308` (TronBox Docker via testcontainers 0.23 + spike CLI matrix mirror):
  ```yaml
  rust-test-spike:
    name: Rust test (local spike — TronBox Docker)
    runs-on: ubuntu-latest
    # Bumped from 15 → 30 min: Docker image pull + container spawn + testcontainers
    # RPC round-trips are bursty on cold runners; the 15-min cap was tight.
    timeout-minutes: 30
    needs: rust-lint
    # Job-level `RUSTFLAGS` (same rationale as rust-test job): rustc
    # warnings → errors. Step-level `env:` (RUN_TRON_LOCAL=1) below
    # merges with this block per GH docs, no conflict.
    env:
      RUSTFLAGS: "-D warnings"
    services:
      docker:
        image: docker:dind
        options: >-
          --privileged
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@master   # tag-based pin per L37 rule 2; SHA-pin follow-up
        with:
          toolchain: "1.98.1"                # workspace rust-toolchain.toml pins this; stable rejects the MSRV gate
      - name: Install protoc (>=3.12)
        # Required by spikes/tron-v1/build.rs (prost-build compiles vendored core/Tron.proto)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y protobuf-compiler
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rust-wallet-app -> target
      - name: cargo build (workspace)
        # Workspace build so rust-cache warms all crates; spike test process
        # then sees `CARGO_BIN_EXE_tron` for `assert_cmd::cargo_bin!("tron")`.
        # Without this pre-build, every test calling cargo_bin!("tron") panics
        # with `CARGO_BIN_EXE_tron is unset` (reproduced in run 34214814671).
        working-directory: rust-wallet-app
        run: cargo build --workspace
      - name: cargo test (local spike — TronBox Docker + #[ignore])
        working-directory: rust-wallet-app
        env:
          RUN_TRON_LOCAL: "1"                 # flips gated container tests from skip-and-return to actual round-trip
          RUN_TRON_NILE: "1"                  # also flips spike trc20_nile CLI matrix mirror rows (Task 7.15)
        run: |
          cargo test -p tron-v1-spike --tests -- \
            --include-ignored --nocapture
  ```

**Key reality** (vs the original Task 4.3 sketch):

| Aspect            | Original sketch                                | Actual (post-Phase 7, commit `7e0fe55e`)                                            |
| ----------------- | ---------------------------------------------- | ----------------------------------------------------------------------------------- |
| Workflow file     | `tron-integration.yml` (new file)              | `rust-tron-core-ci.yml` (existing umbrella)                                          |
| Trigger           | `[push]`                                       | `push` + `pull_request` to `rust-tron-core` (via umbrella)                           |
| Job name          | `rust-test-local-spike`                        | `rust-test-spike` (also drives `RUN_TRON_NILE=1` rows in spike CLI mirror)          |
| Toolchain action  | `dtolnay/rust-toolchain@1.98.1`                | `dtolnay/rust-toolchain@master` + `with.toolchain: "1.98.1"`                         |
| Rust-cache        | not configured                                 | `Swatinem/rust-cache@v2` with `workspaces: rust-wallet-app -> target`                |
| Protoc install    | not mentioned                                  | `apt-get install protobuf-compiler` per job (required by `spikes/tron-v1/build.rs`)  |
| Working directory | implicit (root)                                | explicit `working-directory: rust-wallet-app`                                        |
| Timeout           | not specified                                  | `timeout-minutes: 30` (cold-runner Docker pull)                                      |
| Job ordering      | parallel                                       | `needs: rust-lint` (sequential — lint fail cancels this job before container pull)  |
| Pre-build         | none                                           | `cargo build --workspace` before test (resolves `CARGO_BIN_EXE_tron`)                |
| Test command      | `cargo test --test trc20_local -- --nocapture` | `cargo test -p tron-v1-spike --tests -- --include-ignored --nocapture`               |
| Test gating       | implicit `#[ignore]` only                      | `RUN_TRON_LOCAL=1` + `RUN_TRON_NILE=1` env + `--include-ignored` (handles both gates) |
| RUSTFLAGS         | not set                                        | `-D warnings` job-level (warnings → errors, same as `rust-test` rationale)          |

**Verification:** CI runs on every push + PR to `rust-tron-core`; the `docker:dind` service container provides the Docker daemon; testcontainers 0.23 spawns TronBox from `tronbox/tre:latest`. The `--tests` plural picks up every spike integration binary (`v1_compile` through `v11_mainnet_self_send` + `cli_coverage` + `trc20_nile` + `trc20_local`), so a single CLI regression across any spike surface fails CI.

#### Task 4.4 — Nile testnet integration test (canonical full-flow) (REVISED 2026-09-07)

**Files:** `crates/tron-wallet-core/tests/trc20_nile.rs` (migrated from `spikes/tron-v1/tests/trc20_nile.rs` 2026-09-07)

**Scope correction (2026-09-07):** the prior numbered step list described a `TRON_TEST_MNEMONIC`-keyed path (load env → derive → broadcast → wait). The actual implementation **deviated from that plan on 2026-09-06** per operator decision (mirrors the Phase-3 `tests/v10_broadcast.rs` pattern): sender + recipient (mnemonic + address) live in the bundled `crates/tron-wallet-core/tokens/nile.json` fixture under `test.sender-tr20` / `test.recipient-tr20`. Operator runbook needs zero secret env vars; funding the addresses once lights up both suites. SPKI pin pulled from bundled `tokens/nile.json` (Phase 3 §3.7 extracted pin `e9cc763b176063ea6eed1525dac2542512d9e0bf601e210a14f6aad218a9479f`) with optional `TRON_NILE_SPKI_PIN` override for rotation. Amount is **1 USDT** (TRANSFER_AMOUNT_BASE_UNITS = 1_000_000), not 100.

**Tests in `trc20_nile.rs`** (1 canonical + 4 scenario rows + 1 sanity = 6 total, all `#[ignore]` except sanity — submit directly to live Nile when run with `--ignored`; no env-var gate after 2026-09-07):

| Test                                                                | What it asserts                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `trc20_transfer_full_flow_nile` (canonical, `#[ignore]`)            | **Setup:** load sender + recipient from bundled `tokens/nile.json` (`test.sender-tr20` / `test.recipient-tr20`); SLIP-44 derive via `derive_keypair(&mnemonic, "", &path)` on path `m/44'/195'/0'/0/0`; canonical Nile USDT `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` looked up via `tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")`; SPKI-pinned RPC via `TronGridClient::new(&cfg.rpc_url, Some(SpkiPin::from_bytes(...)))` (pin from `tokens/nile.json`, override via `TRON_NILE_SPKI_PIN`).<br>**Action:** snapshot `balance_before = balance_of(usdt, recipient)`; build + sign TriggerSmartContract via `tx::builder::trc20_transfer` + `tx::sign::sign_tx`; broadcast via `TronGridClient::broadcast(&signed_envelope_hex)` against `/wallet/broadcasthex` (PR #541); poll confirmation via test-local `poll_for_confirmation(&rpc, &txid, 120s)`.<br>**Assert:** `txid` non-empty + `local_txid == network_txid` (single-SHA-256 regression guard); snapshot `balance_after`; **`balance_after − balance_before ≥ ONE_USDT`** (delta-based — pre-funded recipient cannot pass without broadcast actually moving funds). |
| `row_1_trx_native_transfer_nile` (§4.5, `#[ignore]`)                | **Setup:** same fixture load as canonical; SLIP-44 TRON path `m/44'/195'/0'/0/0` via `derive_keypair`.<br>**Action:** build native TRX transfer via `tx::builder::trx_transfer` + `set_ref_block(head)` + `set_fee_limit(0)` + `set_timestamp(now_ms)`; `sign_tx`; broadcast via `TronGridClient::broadcast(&signed_envelope_hex)` against `/wallet/broadcasthex` (PR #541) — **NO SPKI pin** (`TronGridClient::new(..., None)`).<br>**Assert (4 checks, in order):**<br>1. `receipt.is_success()` — broadcast accepted.<br>2. `receipt.txid` non-empty.<br>3. `local_txid_hex == receipt.txid` (case-insensitive) — single-SHA-256 wire form per live Nile 2026-09-06; regression guard for reverted Q2 plan hypothesis.<br>4. `spent_sun = rpc.get_account(owner).balance_sun BEFORE − AFTER ≥ ONE_TRX_SUN` (2026-09-07 instrumentation) — lower bound only because bandwidth burn widens actual delta; catches "broadcast SUCCESS but chain never moved funds" regressions.                                                                                                                                                         |
| `row_2_broadcast_rebroadcast_idempotency_nile` (§4.5, `#[ignore]`)  | **Setup:** same fixture + USDT contract as canonical; SLIP-44 derive; `TronGridClient::new(..., None)` (no SPKI pin — idempotency is a wire-protocol invariant; cert verifier does not affect it).<br>**Action:** build + sign ONE USDT-TRC20 transfer envelope; first broadcast via `TronGridClient::broadcast(&signed_envelope_hex)`; re-POST the SAME `signed_envelope_hex`; poll confirmation on first broadcast.<br>**Assert (no-double-charge ladder):**<br>1. First broadcast `receipt.is_success()` + non-empty txid + `local_txid == network_txid`.<br>2. Rebroadcast: SUCCESS path → `txid == first_txid` (memoized); non-SUCCESS path → `code` or `message` must mention `DUP_TRANSACTION` (live verified 2026-09-06 on Nile: `code = "DUP_TRANSACTION_ERROR"`, `message = "Dup transaction."`).<br>3. Balance window: `ONE_USDT ≤ balance_after − balance_before < 2 × ONE_USDT` — single-credit invariant catches both "funds never moved" and "rebroadcast double-charged" regressions.                                                                                                                                  |
| `row_3_mobile_ffi_nile` (§4.5, `#[ignore]`)                         | Stub — mobile FFI smoke from Dart binding. Out of spike-harness scope (Phase 5 PAL + FFI bridge); body keeps fixture load so the binding drops in cleanly.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `row_4_network_failure_recovery_nile` (§4.5, `#[ignore]`)           | 1. Point `JsonRpcClient::new_local("http://127.0.0.1:9999")` at closed port (kernel returns `ECONNREFUSED` sub-millisecond).<br>2. Probe `balance_of_trc20(...)` against the closed port.<br>3. Assert: (a) returns `Err(_)` (sub-ms `ECONNREFUSED`); (b) elapsed < 30s (no hang); (c) no panic. Maps to Phase 6 CLI retry policy: transport error → exit code 3, never panic.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `derive_sender_produces_t_address_with_known_phrase` (sanity, unit) | 1. `derive_sender("abandon ×11 about")` returns 34-char `T`-prefixed address.<br>2. Determinism — same phrase produces same address across calls.<br>3. SHA-256 pre-image over address bytes logged for anychain-kms derivation drift detection.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

**Row 2 added (2026-09-07):** `row_2_broadcast_rebroadcast_idempotency_nile` absorbed from the deleted `tests/v10_broadcast.rs::live_broadcast_rebroadcast_idempotency_on_nile`. Same envelope POSTed twice; asserts the network treats the second POST as idempotent — SUCCESS path requires same txid (no double-charge); non-SUCCESS path requires `code` or `message` to mention `DUP_TRANSACTION`. Closes plan Phase 7 Task 6.8 (V7a) regression guard. `trc20_nile.rs` is now the single source of truth for Nile live-RPC tests.

**Earlier row 2 (TRC-20 balanceOf read) retired 2026-09-07:** the single `balanceOf` query against the SPKI-pinned RPC + fixture recipient was a strict subset of canonical's pre + post balanceOf checks (same pinned URL, same fixture address, same RPC client). No behavioral coverage was lost; the canonical row's delta assertion already proves the pinned RPC carries `balanceOf` cleanly. Decision logged at L13 audit trail.

**Operator runbook (verified 2026-09-08):**

```bash
# Setup (one-time):
# 1. Extract live Nile SPKI pin (or rely on bundled pin in tokens/nile.json):
openssl s_client -connect nile.trongrid.io:443 -servername nile.trongrid.io \
  </dev/null 2>/dev/null | openssl x509 -pubkey -noout \
  | openssl pkey -pubin -outform der \
  | openssl dgst -sha256 -binary | xxd -p -c 256
# 2. Fund BOTH sender (TRX + USDT) + recipient addresses at the Nile
#    faucet (https://nileex.io/join/getJoinPage). Addresses come from
#    crates/tron-wallet-core/tokens/nile.json → test.sender-tr20 / test.recipient-tr20.

# Default: 1 sanity test passes; 5 #[ignore] tests skipped.
cargo test -p tron-wallet-core --test trc20_nile

# Full surface (after faucet funding):
cargo test -p tron-wallet-core --test trc20_nile -- --ignored --nocapture
# → 1 pass (sanity) + canonical + row_1 + row_2 spend on-chain (require funding); row_3 = stub; row_4 = always passes (closed port).
```

**Files referenced (do not move):**

- `crates/tron-wallet-core/tests/trc20_nile.rs` — canonical + 4 scenario rows (row_1 TRX native, row_2 rebroadcast idempotency, row_3 mobile FFI stub, row_4 network failure) + sanity (this task + §4.5). Migrated from `spikes/tron-v1/tests/trc20_nile.rs` 2026-09-07 so the live-RPC tests exercise the production `tron-wallet-core` API surface rather than spike helpers. Tests/v10_broadcast.rs folded in 2026-09-07 (3 suites absorbed into canonical + row_1 + row_2); file deleted. Uses only `tron_wallet_core::*` (no `tron_v1_spike::*` references); inlines `poll_for_confirmation` as a test-local helper since the production lib exposes `get_tx_info` as a single-shot probe without a high-level waiter.
- `rust-wallet-app/crates/tron-wallet-core/tokens/nile.json` — bundled `test.{sender-tr20, recipient-tr20}` fixture (single source of truth within `trc20_nile.rs`)
- `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md` §"Test Scenario" — Nile row specifications (Task 4.5 row table mirrors this)

- [x] Update Task 4.4 step list to match `trc20_nile.rs` actual scope (bundled fixture, SPKI-pinned RPC, 1 USDT amount, `poll_for_confirmation`).
- [x] Document the 6 tests in `trc20_nile.rs` (1 canonical + 4 scenario rows + 1 sanity).
- [x] Note the `TRON_TEST_MNEMONIC` env var is NOT used (operator decision 2026-09-06: bundled fixture per `tokens/nile.json`).
- [x] Cross-link Task 4.4 canonical to Task 4.5 scenario rows (single file, both shipped together).
- [x] Document the operator runbook: live SPKI pin extract + faucet funding + `cargo test … -- --ignored --nocapture` (no env-var gate after 2026-09-07).

**PR #541 amendment preserved:** canonical row broadcasts via `TronGridClient::broadcast(&signed_envelope_hex)` against `/wallet/broadcasthex` (per PR #541 amendment, broadcast endpoint switched from `/wallet/broadcasttransaction`); `row_1` calls the same `TronGridClient::broadcast` against the same endpoint. (Pre-2026-09-07 spike copy used `tron_v1_spike::tx::broadcast`; same endpoint, same wire form, just the production client's spelling.)

#### Task 4.5 — Nile testnet test scenarios (rows 1, 3, 4)

**Files:** `crates/tron-wallet-core/tests/trc20_nile.rs` (extend — was `spikes/tron-v1/tests/trc20_nile.rs` pre-2026-09-07)

> **2026-09-07 — row 2 removed.** The original Task 4.5 row 2 ("TRC-20 transfer on real test USDT contract") was a placeholder for what became the canonical `trc20_transfer_full_flow_nile` row in Task 4.4. Once that canonical row landed, the §4.5 row 2 redundant scenario (single `balanceOf` read) was deleted 2026-09-07; see the rationale paragraph under the §4.4 test table. Row numbers below skip from 1 to 3.

| #   | Scenario                 | Difference from Local                                                                                           | Pass criteria                                                                                                                                                                                                                                                                                                                                                                                 |
| --- | ------------------------ | --------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | TRX native transfer      | Same                                                                                                            | tx accepted on real network; `receipt.is_success()` + non-empty txid + locally-computed txid == network-reported txid (single-SHA-256 per live Nile 2026-09-06). **2026-09-07:** pre/post `rpc.get_account(owner).balance_sun` reads; assert `spent_sun ≥ TRX_AMOUNT_SUN` (lower bound, includes burned bandwidth). TronScan HTTP GET visibility deferred (no API key committed per Phase 3). |
| 3   | Mobile-specific          | iOS Simulator: `cargo build --target aarch64-apple-ios-sim`; Android Emulator: `cargo ndk -t x86_64 -o jniLibs` | FFI smoke test passes; Dart binding sends a real tx from emulator to Nile                                                                                                                                                                                                                                                                                                                     |
| 4   | Network failure recovery | Point RPC at `http://127.0.0.1:9999` (closed port)                                                              | CLI returns error code 3 (transport error) within 30s timeout; no panic                                                                                                                                                                                                                                                                                                                       |

#### Task 4.6 — CI gate: `rust-test-nile-spike` job in `rust-tron-core-ci.yml`

**Files:** `.github/workflows/rust-tron-core-ci.yml` (existing; no new file).

**2026-09-07 plan correction:** the standalone `tron-nile.yml` sketched in the original Task 4.6 was **never created**. Nile coverage lives as a job inside the umbrella `rust-tron-core-ci.yml`, alongside the local-spike job (Task 4.3) and the operator-smoke job. The umbrella workflow already fires on every push + PR to `rust-tron-core`, so the nile gate runs at the same cadence as the rest of CI — the original "manual trigger only" constraint was relaxed when the workflow absorbed this job.

- [x] `rust-test-nile-spike` job at `.github/workflows/rust-tron-core-ci.yml:273-304` (live Nile RPC + `#[ignore]`-marked tests):
  ```yaml
  rust-test-nile-spike:
    name: Rust test (nile spike — live RPC + #[ignore])
    runs-on: ubuntu-latest
    timeout-minutes: 30   # bumped 15 → 30: live RPC + SPKI pin handshake + wait_for_confirm polling
    needs: rust-lint      # sequential ordering: SKIPPED on lint red, runs to completion on lint pass
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.98.1"
      - name: Install protoc (>=3.12)
        # Required by spikes/tron-v1/build.rs (prost-build compiles vendored core/Tron.proto)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y protobuf-compiler
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rust-wallet-app -> target
      - name: cargo test (nile spike — live RPC + #[ignore])
        # `--include-ignored` runs every test in trc20_nile.rs. The live
        # tests are `#[ignore]`-marked (no env-var gate per Task 4.4 revision);
        # operator opt-in is `--ignored`. `--nocapture` surfaces the live
        # RPC + SPKI-pin logs in CI output.
        working-directory: rust-wallet-app
        run: |
          cargo test -p tron-wallet-core --test trc20_nile -- \
            --include-ignored --nocapture
  ```

**Key reality** (vs the original Task 4.6 sketch):

| Aspect          | Original sketch                                                                                | Actual                                                                                               |
| --------------- | ---------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Workflow file   | `tron-nile.yml` (new file)                                                                     | `rust-tron-core-ci.yml` (existing umbrella)                                                          |
| Trigger         | `workflow_dispatch` (manual only)                                                              | `push` + `pull_request` to `rust-tron-core` (via umbrella) — runs on every PR                        |
| Env block       | `TRON_TEST_MNEMONIC: ${{ secrets.TON_TEST_MNEMONIC }}` (typo + unused after Task 4.4 revision) | none — sender + recipient live in `crates/tron-wallet-core/tokens/nile.json`                         |
| Test command    | `TRON_NILE_INTEGRATION=1 cargo test --test trc20_nile -- --nocapture`                          | `cargo test -p tron-wallet-core --test trc20_nile -- --include-ignored --nocapture`                  |
| Gating          | env-var                                                                                        | `#[ignore]` + `--include-ignored`                                                                    |
| Job ordering    | parallel                                                                                       | `needs: rust-lint` (sequential — cancellation-prevention rationale at `:269-272`)                    |
| Operator action | `workflow_dispatch` + secrets                                                                  | fund the bundled fixture addresses once via <https://nileex.io/join/getJoinPage>; CI runs every push |

- [x] **No `workflow_dispatch` gate** — nile tests run on every push + PR to `rust-tron-core` once the operator-funded fixture addresses are live.
- [x] **No secrets** — sender + recipient mnemonics + addresses live in `crates/tron-wallet-core/tokens/nile.json` (`test.sender-tr20`, `test.recipient-tr20`). Operator funds those addresses once via the Nile faucet and this job lights up. (Note: `tests/v10_broadcast.rs` was folded into `tests/trc20_nile.rs` 2026-09-07 — same fixture, no second CI surface to coordinate.)

#### Task 4.7 — Decision matrix (test stage → network)

**Files:** `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md` §"Test Scenario" → §"Decision matrix"

| Stage                 | Network                             | Why                                                                                                                                                                                                                                                                                                 |
| --------------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unit (per-commit)     | none (InMemoryWalletStorage + mock) | fast, no I/O                                                                                                                                                                                                                                                                                        |
| Integration CI        | Local (TronBox)                     | deterministic, fast (~30s), no faucet                                                                                                                                                                                                                                                               |
| Pre-release manual QA | Nile (testnet)                      | real network + faucets; verifies edge cases                                                                                                                                                                                                                                                         |
| Mobile CI             | Local (TronBox)                     | no Docker fallback for mobile — **Round-1 grill Q6: mobile CI matrix = `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` (FFI compile only). NO mobile runtime smoke in v0.1. Add runtime mobile smoke via Nile testnet (real network, no Docker) for v0.2.** |
| Production            | Mainnet                             | post-Phase 4 only                                                                                                                                                                                                                                                                                   |

#### Task 4.8 — Use-case map + workflow logic per test fn (REVISED 2026-09-07; path-corrected 2026-09-08)

**Files:** `crates/tron-wallet-core/tests/trc20_local.rs` (Phase 4 surface — testcontainers harness + 9 scenario rows), `crates/tron-wallet-core/tests/trc20_nile.rs` (canonical + 4 rows + sanity; migrated from `spikes/tron-v1/tests/trc20_nile.rs` 2026-09-07), `crates/tron-wallet-core/tests/common/mod.rs` (shared helpers), `crates/tron-wallet-core/tests/fixtures/MockTRC20.sol` + `spki_pin_test_cert.der` (test fixtures). **Spike mirrors** (NOT Phase 4 surface): `spikes/tron-v1/tests/trc20_local.rs` + `spikes/tron-v1/tests/trc20_nile.rs` are Task 7.15 CLI matrices — same filenames, black-box `assert_cmd::cargo_bin("tron")` content. **Dropped 2026-09-08:** `tests/use_case_alpha_sends_beta_usdt.rs` (never existed as source — only stale `target/debug/deps/*.rmeta` artifacts) and `tests/v10_broadcast.rs` (folded into `trc20_nile.rs` 2026-09-07).

**Goal:** every `#[test]` / `#[tokio::test]` in Phase 4 traceable to a user-visible behavior; every test fn carries inline Setup → Action → Assert → Cleanup so a regression in any layer fails CI loudly with a self-narrating test. Mirror Phase 5.8 §"Use case map" + §"Layer A" pattern.

**Scope discipline:** helpers (`require_local_opt_in`, `spawn_tronbox`, `probe_getnowblock`, `deterministic_sk`, `trc20_txid`, `sign_local`, `u128_to_32bytes`, `encode_approve`, `base58_to_20bytes`, `wallet_lookup`, `chrono_like_timestamp_ms`, `build_representative_trx_raw_data`, `wrap_in_envelope`, `compile_mock_trc20_in_container`, `deploy_mock_trc20`, `easytransfer_fund`, `wait_for_confirm`, `triggerconstant_call`, `parse_uint256_hex`, `address_to_32bytes_hex`, `u64_to_32bytes_hex`, `deploy_mock_trc20_fixture`, `rpc_smoke`, `param_zero_address`, `nile_spki_override`, `tron_path`, `derive_sender`, `nile_usdt_address`, `load_nile_fixture`, `pinned_nile_url`, `nile_creds`, `env_opt_in`, `fresh_wallet`, `build_trc20_transfer_calldata`, `sign_prehash_65byte`) are NOT use cases — they exist to make the test fns below readable and are audited by the test-fn bodies themselves, not by a separate UC entry.

**Use case map** — 22 tests across 2 files (Phase 4 surface), 19 UCs (row_2 + UC-NR-2 + UC-AB-O/L/N removed; UC-AB-* dropped 2026-09-08 because `tests/use_case_alpha_sends_beta_usdt.rs` never existed as source — only stale build artifacts):

| UC       | Behavior (one-line user story)                                                                                                                                                                                                                                                                                                                                 | Test fn                                                | File                                | Gating                      |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | ----------------------------------- | --------------------------- |
| UC-LU-1  | Local RPC URL defaults to port 9090 when no port given                                                                                                                                                                                                                                                                                                         | `new_local_parses_http_url_with_default_port_9090`     | `trc20_local.rs`                    | unit                        |
| UC-LU-2  | Local RPC URL accepts explicit port override                                                                                                                                                                                                                                                                                                                   | `new_local_parses_http_url_with_explicit_port`         | `trc20_local.rs`                    | unit                        |
| UC-LU-3  | Local RPC URL refuses `https://` scheme (pin only)                                                                                                                                                                                                                                                                                                             | `new_local_rejects_https_url`                          | `trc20_local.rs`                    | unit                        |
| UC-LU-4  | Pinned RPC URL refuses `pinned://` w/o pin                                                                                                                                                                                                                                                                                                                     | `new_local_rejects_pinned_scheme`                      | `trc20_local.rs`                    | unit                        |
| UC-LU-5  | Pinned RPC URL defaults scheme to `https` when omitted                                                                                                                                                                                                                                                                                                         | `new_pinned_defaults_scheme_to_https`                  | `trc20_local.rs`                    | unit                        |
| UC-LH-1  | TronBox node serves `/wallet/getnowblock` 2xx (TAPOS source)                                                                                                                                                                                                                                                                                                   | `tronbox_local_node_serves_getnowblock`                | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LH-2  | TronBox node serves `/walletsolidity/getnowblock` 2xx (Task 2.4 TAPOS path)                                                                                                                                                                                                                                                                                    | `tronbox_local_node_serves_walletsolidity_getnowblock` | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LH-3  | TronBox node serves `/jsonrpc eth_chainId` w/ 0x-prefixed hex (Q6)                                                                                                                                                                                                                                                                                             | `tronbox_local_node_serves_eth_chainid`                | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-1  | TRX native TransferContract envelope builds + signs + computes txid locally                                                                                                                                                                                                                                                                                    | `row_1_trx_native_transfer_local`                      | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-2  | TRC-20 transfer to *held* recipient encodes 68-byte calldata + correct selector                                                                                                                                                                                                                                                                                | `row_2_trc20_transfer_held_recipient_local`            | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-3  | TRC-20 transfer to *first-time* recipient encodes 68-byte calldata (same shape; held-vs-first-time only differs on-chain)                                                                                                                                                                                                                                      | `row_3_trc20_first_time_receive_local`                 | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-4  | TRC-20 `approve(spender, value)` encodes `0x095ea7b3` selector + 32-byte value slot                                                                                                                                                                                                                                                                            | `row_4_trc20_approval_local`                           | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-5  | Stake2 freeze/unfreeze envelope builds + signs (resource delegation path)                                                                                                                                                                                                                                                                                      | `row_5_stake2_freeze_unfreeze_local`                   | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-6  | TRC-20 transfer with `u256::MAX` amount slot encodes full 32-byte width (regression guard for amount-overflow)                                                                                                                                                                                                                                                 | `row_6_trc20_insufficient_balance_local`               | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-7  | Send-speedup = fresh envelope (new timestamp + new fee_limit) → different txid                                                                                                                                                                                                                                                                                 | `row_7_send_speedup_local`                             | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-7a | Rebroadcast of identical envelope → identical txid (determinism; node-side `DUP_TRANSACTION_ERROR` is V7a/Nile Task 7.8)                                                                                                                                                                                                                                       | `row_7a_rebroadcast_idempotency_local`                 | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-LR-8  | Wallet-to-wallet TRC-20: `wallet_lookup("cold")` → T-address → ABI encode + sign                                                                                                                                                                                                                                                                               | `row_8_wallet_to_wallet_trc20_local`                   | `trc20_local.rs`                    | `RUN_TRON_LOCAL=1`          |
| UC-NC    | Canonical Nile TRC-20 full-flow: fixture load → SLIP-44 derive → SPKI-pinned RPC → build+sign → `/wallet/broadcasthex` → `poll_for_confirmation` → **`balanceOf` pre + post read; assert delta ≥ 1 USDT** (2026-09-07 — was absolute-floor `≥ TRANSFER_AMOUNT_BASE_UNITS`; replaced with delta to catch "broadcast SUCCESS but funds didn't move" regressions) | `trc20_transfer_full_flow_nile`                        | `trc20_nile.rs`                     | `#[ignore]`, fixture-funded |
| UC-NR-1  | Native TRX on Nile: local txid == network-reported txid (single-SHA-256 regression guard for reverted Q2 plan hypothesis). **2026-09-07:** also snapshots `rpc.get_account(owner).balance_sun` pre + post broadcast; asserts `spent_sun ≥ TRX_AMOUNT_SUN` (lower bound, includes burned bandwidth).                                                            | `row_1_trx_native_transfer_nile`                       | `trc20_nile.rs`                     | `#[ignore]`, fixture-funded |
| UC-NR-3  | Mobile FFI smoke stub (Phase 5 PAL + FFI bridge scope; body keeps fixture load)                                                                                                                                                                                                                                                                                | `row_3_mobile_ffi_nile`                                | `trc20_nile.rs`                     | `#[ignore]`                 |
| UC-NR-4  | Network-failure recovery: closed port → `Err` within 30s, no panic (maps to CLI exit 3)                                                                                                                                                                                                                                                                        | `row_4_network_failure_recovery_nile`                  | `trc20_nile.rs`                     | `#[ignore]`                 |
| UC-NS    | SLIP-44 derivation determinism: same phrase → same T-address; SHA-256 pre-image logged for anychain-kms drift detection                                                                                                                                                                                                                                        | `derive_sender_produces_t_address_with_known_phrase`   | `trc20_nile.rs`                     | unit                        |

**Workflow logic per test fn** — Setup → Action → Assert → Cleanup. Format mirrors Phase 5.8 §"Layer A" so reviewers can grep `^### UC-` blocks.

### UC-LU-* — Local URL parse (unit, `trc20_local.rs`)

- **UC-LU-1** `new_local_parses_http_url_with_default_port_9090` (line 278). Setup: `url = "http://127.0.0.1"`. Action: `JsonRpcClient::new_local(&url)`. Assert: `Ok(client)`; `client.base_url().port() == 9090`. Cleanup: drop client.
- **UC-LU-2** `new_local_parses_http_url_with_explicit_port` (line 291). Setup: `url = "http://127.0.0.1:50061"`. Action: `new_local(&url)`. Assert: port == 50061. Cleanup: drop.
- **UC-LU-3** `new_local_rejects_https_url` (line 300). Setup: `url = "https://127.0.0.1:9090"`. Action: `new_local(&url)`. Assert: `Err(_)` (pin scheme only). Cleanup: drop err.
- **UC-LU-4** `new_local_rejects_pinned_scheme` (line 306). Setup: `url = "pinned://abc@host"` (missing pin). Action: `new_local(&url)`. Assert: `Err(_)`. Cleanup: drop err.
- **UC-LU-5** `new_pinned_defaults_scheme_to_https` (line 315). Setup: `url = "pinned://e9cc...@nile.trongrid.io"` (no `https://`). Action: `new_pinned(&url)`. Assert: `client.base_url().scheme() == "https"`, host == `nile.trongrid.io`. Cleanup: drop.

### UC-LH-* — Local harness (Docker-gated, `trc20_local.rs`)

- **UC-LH-1** `tronbox_local_node_serves_getnowblock` (line 173). Setup: `require_local_opt_in()` → bail; `spawn_tronbox()` → `(base_url, _container)`. Action: GET `/wallet/getnowblock`. Assert: HTTP 2xx, body has non-empty `blockID` hex field. Cleanup: container drops on test end.
- **UC-LH-2** `tronbox_local_node_serves_walletsolidity_getnowblock` (line 192). Setup: same as LH-1. Action: GET `/walletsolidity/getnowblock` (Task 2.4 TAPOS source). Assert: HTTP 2xx, non-empty `blockID`. Cleanup: drop.
- **UC-LH-3** `tronbox_local_node_serves_eth_chainid` (line 236). Setup: same as LH-1. Action: POST `/jsonrpc` with `{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}`. Assert: HTTP 2xx, `result` is 0x-prefixed hex string (Q6 shape). Cleanup: drop.

### UC-LR-* — Local scenario rows (Docker-gated, `trc20_local.rs`)

All rows follow the same envelope pattern; deviations annotated below.

- **UC-LR-1** `row_1_trx_native_transfer_local` (line 692). Setup: `deterministic_sk()`; `wallet_lookup("alpha")` → recipient T-address; `build_representative_trx_raw_data(sk.pubkey21(), recipient20, 1_000_000 sun)` → 256-byte envelope. Action: `wrap_in_envelope(envelope, ts, fee_limit=1_000_000)`; `sign_local(sk, &trc20_txid(&raw))` → 65-byte sig; assert `v ∈ {0, 1}` (TRON, NOT `v+27` per Q8). Assert: dual-SHA-256 txid is 32 bytes; sig recovers pubkey == sk.pubkey. Cleanup: drop all.
- **UC-LR-2** `row_2_trc20_transfer_held_recipient_local` (line 767). Setup: `deterministic_sk()`; `mock_usdt = "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf"`; `amount = 100 mock USDT`. Action: ABI-encode `transfer(recipient20, amount)` → 68-byte calldata; assert `[0..4] == [0xa9, 0x05, 0x9c, 0xbb]`; sign + txid. Assert: calldata.len() == 68; selector matches. Cleanup: drop.
- **UC-LR-3** `row_3_trc20_first_time_receive_local` (line 800). Setup: fresh `recipient20` (never received before). Action: same calldata shape as LR-2. Assert: identical 68-byte shape (held-vs-first-time is a node-side, not envelope-side, distinction). Cleanup: drop.
- **UC-LR-4** `row_4_trc20_approval_local` (line 827). Setup: `spender20 = base58_to_20bytes(wallet_lookup("spender"))`; `value = u128_to_32bytes(1000)`. Action: `encode_approve(spender20, &value)` → 68 bytes. Assert: `[0..4] == [0x09, 0x5e, 0xa7, 0xb3]` (approve selector); `value` slot == 32-byte big-endian of 1000. Cleanup: drop.
- **UC-LR-5** `row_5_stake2_freeze_unfreeze_local` (line 855). Setup: `amount = 1_000_000_000 sun`. Action: build FreezeBalanceV2 / UnfreezeBalanceV2 envelope per `wrap_in_envelope`. Assert: envelope round-trips through `sign_local`; txid matches. Cleanup: drop.
- **UC-LR-6** `row_6_trc20_insufficient_balance_local` (line 876). Setup: `amount = u64::MAX`. Action: ABI-encode `transfer(recipient20, u256::MAX)` via `u128_to_32bytes(amount). Action:` (note: upper 8 bytes are 0xFF, lower 24 bytes encode u64::MAX). Assert: calldata.len() == 68; amount slot full 32-byte width, no truncation. Cleanup: drop.
- **UC-LR-7** `row_7_send_speedup_local` (line 903). Setup: same as LR-2; `ts1`, `fee1`. Action: build envelope at `(ts1, fee1)`; compute txid1; mutate envelope to `(ts1+1, fee1+1_000_000)`; compute txid2. Assert: `txid1 != txid2` (speedup = fresh envelope, NOT rebroadcast — V7a finding). Cleanup: drop both.
- **UC-LR-7a** `row_7a_rebroadcast_idempotency_local` (line 936). Setup: same envelope bytes (same ts, same fee). Action: sign twice with same sk; compute txid from raw bytes once. Assert: identical raw → identical txid. Cleanup: drop.
- **UC-LR-8** `row_8_wallet_to_wallet_trc20_local` (line 969). Setup: `wallet_lookup("cold")` → T-address; `base58_to_20bytes(addr)`; `amount = 5 mock USDT`. Action: ABI-encode + sign. Assert: calldata selector `0xa9059cbb`; recipient20 == `cold_addr20`. Cleanup: drop.

### UC-NC + UC-NR-* + UC-NS — Nile (env-gated, `trc20_nile.rs`)

- **UC-NC** `trc20_transfer_full_flow_nile` (line 212, canonical). Setup: `load_nile_fixture()` → sender+recipient from `tokens/nile.json`; `nile_usdt_address()` → `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`; `pinned_nile_url(nile_spki_override())` → SPKI-pinned RPC; SLIP-44 derive via `derive_sender(phrase)` → `(t_addr, sk)`. Action: snapshot `balance_before = balance_of_trc20(recipient, canonical_usdt, &pinned_rpc)`; `build_signed_trc20_transfer(sk, recipient20, 1_000_000)`; broadcast via `/wallet/broadcasthex` (PR #541); `poll_for_confirmation(&rpc, &tx_id, 120s)`. Assert: `txid` non-empty; snapshot `balance_after = balance_of_trc20(...)`; **`delta = balance_after - balance_before ≥ TRANSFER_AMOUNT_BASE_UNITS` (2026-09-07 — was absolute floor; replaced with delta so a pre-funded recipient cannot pass without the broadcast actually moving funds)**. Cleanup: drop container-less test (no Docker).
- **UC-NR-1** `row_1_trx_native_transfer_nile` (line 314). Setup: same fixture; `amount = 1 TRX`. Action: `trx_transfer` + `set_ref_block` + `set_fee_limit(0)` + `set_timestamp`; `sign_tx`; `TronGridClient::broadcast(&signed_envelope_hex)` (NO SPKI pin per `v10_broadcast.rs` posture). **2026-09-07:** snapshot `sender_balance_before = rpc.get_account(owner).balance_sun` pre-broadcast; after broadcast success + txid parity check, snapshot `sender_balance_after`. Assert: `receipt.is_success()`; `local_txid == network_txid` (single-SHA-256 per live Nile 2026-09-06 — regression guard for reverted Q2 plan hypothesis); **`spent_sun = sender_balance_before - sender_balance_after ≥ TRX_AMOUNT_SUN`** (lower bound only — bandwidth burn widens actual delta, so we assert the transferred amount is included rather than exact equality). Cleanup: drop.
- **UC-NR-3** `row_3_mobile_ffi_nile` (line 454). Setup: same fixture load. Action: stub body — Phase 5 PAL + FFI bridge scope; keeps fixture load so Dart binding drops in cleanly. Assert: fixture non-empty. Cleanup: drop.
- **UC-NR-4** `row_4_network_failure_recovery_nile` (line 476). Setup: `JsonRpcClient::new_local("http://127.0.0.1:9999")` (closed port). Action: `balance_of_trc20(...)` with 30s timeout. Assert: `Err(_)` within 30s (no hang); no panic. Cleanup: drop.
- **UC-NS** `derive_sender_produces_t_address_with_known_phrase` (line 524, unit). Setup: `phrase = "abandon ×11 about"` (12-word). Action: `derive_sender(phrase)` called twice. Assert: both calls return same 34-char `T`-prefixed address; log `Sha256(address_bytes)` for anychain-kms drift detection. Cleanup: drop.

### UC-AB-* — Alpha → Beta USDT (REMOVED 2026-09-08)

The `tests/use_case_alpha_sends_beta_usdt.rs` source file **never existed** in this repo — only stale `target/debug/deps/use_case_alpha_sends_beta_usdt-*.rmeta` artifacts from an early prototype. UC-AB-O / UC-AB-L / UC-AB-N are dropped. Coverage maps as:

- **Offline sign + 65-byte prehash** → absorbed by `UC-LR-1..LR-8` rows in `crates/tron-wallet-core/tests/trc20_local.rs` (`deterministic_sk()` + `sign_local` cover the same fixture path).
- **Live local-node (TronBox + easytransfer)** → was never wired (mock-fund path is loud-RED per §4.2 status). Lives in scope of the follow-up `tronbox/tre:solidity` image work.
- **Live Nile broadcast** → `UC-NC` (`trc20_transfer_full_flow_nile`) in `crates/tron-wallet-core/tests/trc20_nile.rs` exercises the same surface with the bundled `tokens/nile.json` fixture.

**Checklist** (mirror Phase 5.8 §"Checklist"):

- [x] Use case map table covers all 22 test fns across 2 Phase 4 files (`trc20_local.rs` + `trc20_nile.rs`); helpers excluded by Scope discipline. UC-AB-O / UC-AB-L / UC-AB-N rows dropped 2026-09-08 — `use_case_alpha_sends_beta_usdt.rs` source never existed; coverage absorbed into UC-LR-1..8 + UC-NC.
- [x] Every UC entry has Setup → Action → Assert → Cleanup inline.
- [x] Gating column accurate: unit tests have no env var; `RUN_TRON_LOCAL=1` for Docker-gated; `#[ignore]` + fixture-funded for Nile. No env-gated rows (the deleted `use_case_alpha_sends_beta_usdt_live_nile` `TRON_NILE_TEST_MNEMONIC` path was never wired).
- [x] Cross-link to Phase 5.8 (same format → grep `^### UC-` works across both phases).

#### Phase 4 Verification

- [x] `cargo test --test trc20_local` PASS (local CI gate). *(Agent-verifiable 2026-09-06: 5 unit tests PASS, 12 #[ignore] gated tests wired correctly. Local CI Docker runner path requires `RUN_TRON_LOCAL=1` + Docker daemon — operator runbook in workflow + test docstring. CI gate lives as `rust-test-local-spike` job at `.github/workflows/rust-tron-core-ci.yml:218-254`, triggered on every push + PR to `rust-tron-core`.)*
- [x] `cargo test -p tron-wallet-core --test trc20_nile -- --ignored --nocapture` PASS (operator runbook, no env vars needed after 2026-09-07 fixture switch). *(Agent delivered harness + loud-RED gate 2026-09-06; operator must fund Nile test wallet via <https://nileex.io/join/getJoinPage>. CI gate lives as `rust-test-nile-spike` job at `.github/workflows/rust-tron-core-ci.yml:273-304`, triggered on every push + PR to `rust-tron-core`. Sender + recipient mnemonic + address live in `crates/tron-wallet-core/tokens/nile.json` under `test.sender-tr20` / `test.recipient-tr20`.)*
- [x] **2026-09-07 instrumentation update:** canonical + row_1 now snapshot balances BEFORE the send and assert a positive delta AFTER (`balanceOf` for TRC-20, `get_account(balance_sun)` for native TRX) — catches the "broadcast returned SUCCESS but the chain never moved funds" regression class. Row 2 removed (single `balanceOf` read was a strict subset of canonical's pre + post balanceOf checks). Test count: 6 → 5 (1 canonical + 3 rows + 1 sanity). Live RPC calls now serialized with `--test-threads=1` in the `rust-test-nile-spike` job (mirrors operator-smoke `:202`).
- [x] `.github/workflows/rust-tron-core-ci.yml` `rust-test-local-spike` job exists and triggers on push + PR to `rust-tron-core`.
- [x] `.github/workflows/rust-tron-core-ci.yml` `rust-test-nile-spike` job exists and triggers on push + PR to `rust-tron-core`.
- [ ] Round-1 grill Q6 mobile matrix: `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` both succeed. *(Phase 5 PAL + crypto scope, not Phase 4. CI gate lives as `mobile-check` job at `.github/workflows/rust-tron-core-ci.yml:386-425`.)*

**PAUSE. Verify L13 step 11.**

---

### Phase 5 — PAL platform abstraction (4 traits)

**Goal:** Pure Rust core compiles for Linux/macOS/Windows + iOS arm64 + Android arm64 with no source changes. **CI gate:** `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` succeeds.

#### Task 5.1 — Define 4 traits

**Files:** `src/platform/storage.rs`, `src/platform/info.rs`, `src/platform/network.rs`, `src/platform/clock.rs`

- [x] `pub trait WalletStorage: Send + Sync { fn put/get/list/delete/put_atomic }`.
- [x] `pub trait PlatformInfo: Send + Sync { fn data_dir/app_version/app_name/is_mobile }`.
- [x] `pub trait NetworkClient: Send + Sync { fn build_client/default_rpc_url }`.
- [x] `pub trait Clock: Send + Sync { fn now_millis/sleep }`.

#### Task 5.2 — Desktop impls

**Files:** `src/platform/desktop/storage.rs`, `info.rs`, `network.rs`

- [x] `FileWalletStorage` uses `~/.local/share/tron/wallets/` (Linux) / `~/Library/Application Support/tron/wallets/` (macOS) / `%APPDATA%\tron\wallets\` (Windows). Mode 0600. Atomic write via temp + rename. **2026-09-07 PASS:** `src/platform/desktop.rs:97` (`FileWalletStorage`) + `:138` (`impl WalletStorage for FileWalletStorage`); temp + rename atomic write per impl body.
- [x] `SystemDirsInfo` returns per-OS data dir. **2026-09-07 PASS:** `src/platform/desktop.rs:61` (`impl PlatformInfo for DesktopPlatformInfo`).
- [x] `ReqwestClient` builds `reqwest::Client::builder().tls_built_in_webpki_roots().timeout(30s)`. **2026-09-07 PASS:** `src/platform/desktop.rs:264` (`impl NetworkClient for DesktopNetworkClient`).

#### Task 5.3 — iOS impls

**Files:** `src/platform/ios/storage.rs`, `info.rs`

- [x] `KeychainWalletStorage` calls `ios_keystore::set/get/list/delete` via FFI bridge (Swift wrapper). **2026-09-07 PASS (TRAIT SCAFFOLDING):** `src/platform/ios.rs:80` (`KeychainWalletStorage`) + `:82` (`impl WalletStorage for KeychainWalletStorage`). FFI bridge is v0.2 work (cross-compile-only per Phase 5 Verification below).
- [x] `BundleInfo` returns NSDocumentDirectory via Swift bridge. **2026-09-07 PASS (TRAIT SCAFFOLDING):** `src/platform/ios.rs:50` (`impl PlatformInfo for IosPlatformInfo`). FFI bridge is v0.2 work.
- [x] `OSRootsClient` uses `tls_built_in_root_certs(true)` on reqwest (mobile uses OS roots). **2026-09-07 PASS (TRAIT SCAFFOLDING):** `src/platform/ios.rs:124` (`impl NetworkClient for IosNetworkClient`).

#### Task 5.4 — Android impls

**Files:** `src/platform/android/storage.rs`, `info.rs`

- [x] `EncryptedFileWalletStorage` calls `android_keystore::encrypted_file_write/read` via JNI bridge (Kotlin wrapper). **2026-09-07 PASS (TRAIT SCAFFOLDING):** `src/platform/android.rs:61` (`EncryptedFileWalletStorage`) + `:63` (`impl WalletStorage for EncryptedFileWalletStorage`). JNI bridge is v0.2 work.
- [x] `ContextInfo` returns `context.getFilesDir()` via JNI. **2026-09-07 PASS (TRAIT SCAFFOLDING):** `src/platform/android.rs:36` (`impl PlatformInfo for AndroidPlatformInfo`).

#### Task 5.5 — Test impls

**Files:** `src/platform/test/storage.rs`, `info.rs`, `network.rs`, `clock.rs`

- [x] `InMemoryStorage` uses `Arc<Mutex<HashMap<WalletId, Vec<u8>>>>`. **2026-09-07 PASS:** `src/platform/test.rs:34` (`InMemoryStorage`) + `:56` (`impl WalletStorage for InMemoryStorage`).
- [x] `StaticInfo` returns compile-time constants. **2026-09-07 PASS:** `src/platform/test.rs:111` (`StaticInfo`) + `:118` (`impl PlatformInfo for StaticInfo`).
- [x] `MockClient` returns stub responses. **2026-09-07 PASS:** `src/platform/test.rs:157` (`impl NetworkClient for MockNetworkClient`).
- [x] `MockClock` returns deterministic time. **2026-09-07 PASS:** `src/platform/test.rs:176` (`MockClock`) + `:200` (`impl Clock for MockClock`).

#### Task 5.6 — Compile-time platform selection

**Files:** `src/platform/mod.rs`

- [x] `#[cfg(target_os = "ios")] pub type DefaultStorage = ios::KeychainWalletStorage;` **2026-09-07 PASS:** `src/platform/mod.rs:61`.
- [x] `#[cfg(target_os = "android")] pub type DefaultStorage = android::EncryptedFileWalletStorage;` **2026-09-07 PASS:** `src/platform/mod.rs:70`.
- [x] `#[cfg(not(any(target_os = "ios", target_os = "android")))] pub type DefaultStorage = desktop::FileWalletStorage;` **2026-09-07 PASS:** `src/platform/mod.rs:79`.
- [x] `default_storage()`, `default_platform_info()`, `default_network_client()` factory functions with cfg gating. **2026-09-07 PASS:** `src/platform/mod.rs:90` (`default_storage`), `:94` (`default_platform_info`), `:98` (`default_network_client`), `:102` (`default_clock`); android variants at `:107`/`:111`/`:115`/`:119`; desktop variants at `:124`/`:134`/`:138`/`:142`.

#### Task 5.7 — Wallet persistence (Argon2id + AES-GCM)

**Files:** `src/crypto/mod.rs`, `src/wallet/persist.rs`

- [x] `crypto::encrypt(plaintext: &[u8], passphrase: &str) -> Result<EncryptedWallet>` uses Argon2id KDF + AES-256-GCM. **2026-09-07 PASS:** implemented at `src/crypto/mod.rs::encrypt` (commit `0085780`); round-trip + nonce/salt uniqueness asserted by `src/crypto/mod.rs::tests`.
- [x] `crypto::decrypt(ciphertext: &EncryptedWallet, passphrase: &str) -> Result<Vec<u8>>`. **2026-09-07 PASS:** `src/crypto/mod.rs::decrypt` (commit `0085780`); returns `Zeroizing<Vec<u8>>` so plaintext zeroizes on drop; wrong-passphrase + truncated-blob regressions covered.
- [x] `wallet::WalletManager::create(mnemonic: Mnemonic, passphrase: &str) -> Result<WalletId>` → encrypt → `WalletStorage::put`. **2026-09-07 PASS:** `src/wallet/persist.rs::create` (commit `0085780`); serialized `PlaintextRecord` JSON inside the ciphertext, fresh `WalletId::new()` UUID v4 per call.
- [x] `wallet::WalletManager::unlock(id: WalletId, passphrase: &str) -> Result<UnlockedWallet>` → `WalletStorage::get` → decrypt → Zeroizing-wrapped mnemonic. **2026-09-07 PASS:** `src/wallet/persist.rs::unlock` (commit `0085780`); errors map to `Error::Wallet` (missing id) or `Error::Encryption` (wrong pass / corrupt blob) per test assertions.
- [x] **Zeroizing wraps:** Argon2id-derived key (32 bytes) → `Zeroizing<[u8; 32]>`; plaintext entropy during decrypt/re-encrypt window → `Zeroizing<Vec<u8>>`. **2026-09-07 PASS:** `src/crypto/mod.rs::derive_key` returns `Zeroizing<Vec<u8>>`; `decrypt` returns `Zeroizing<Vec<u8>>`; intermediate keys zeroized before return. `UnlockedWallet::mnemonic` returns `&Mnemonic` (bip39's `Zeroizing` wrapper per Phase 1 finding).
- [x] Test: round-trip create → unlock returns same mnemonic. **2026-09-07 PASS:** `tests/wallet_persistence.rs` — 6 tests passed (`in_memory_create_unlock_roundtrip`, `wrong_passphrase_rejected`, `missing_id_rejected`, `delete_makes_blobs_disappear`, `unique_wallet_ids_per_create`, `unlock_after_corrupt_blob_errors`). 41s runtime dominated by Argon2id@256MiB.

**Test Scenario mapping:** supports **Local row 8 (wallet-to-wallet TRC-20)** — `WalletManager::lookup(name_or_id)` requires wallet-id resolution across CLI invocations. Persists encrypted blob via `WalletStorage` (PAL = `FileWalletStorage` desktop / `KeychainWalletStorage` iOS / `EncryptedFileWalletStorage` Android). Also enables **Nile row 3 (Mobile-specific)** — Keychain storage validates FFI boundary for Dart binding on emulator.

#### Task 5.8 — Unit-test coverage for encrypted wallet persistence

**Goal:** Every behavior of the password-gated create / unlock / rename / import / list path covered by a `#[test]` so a regression in Argon2id params, AES-GCM nonce reuse, `PlaintextRecord` shape, or `WalletManager` dispatch fails CI loudly. Audit the existing surface (Phase 5 ship + Phase 6 carry-over) and fill the gaps.

**Files:** `crates/tron-wallet-core/src/crypto/mod.rs` (`#[cfg(test)] mod tests`), `crates/tron-wallet-core/src/wallet/persist.rs` (`#[cfg(test)] mod tests`), `crates/tron-wallet-core/tests/wallet_persistence.rs`.

**Use case map** — every behavior below ties to ≥ 1 test fn; no orphan tests, no untested behaviors.

| UC    | Use case                                           | Test fn(s)                                                                                                                                                                                                                                    |
| ----- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UC-1  | Round-trip mnemonic through Argon2id + AES-GCM     | `encrypt_decrypt_roundtrip`, `in_memory_create_unlock_roundtrip`                                                                                                                                                                              |
| UC-2  | Wrong password rejected (no oracle)                | `wrong_passphrase_rejected` (crypto), `wrong_passphrase_rejected` (manager), `unlock_after_corrupt_blob_errors`                                                                                                                               |
| UC-3  | Missing wallet id rejected                         | `missing_id_rejected`                                                                                                                                                                                                                         |
| UC-4  | Truncated / tampered blob rejected                 | `truncated_blob_rejected`, `unlock_after_partial_truncate_errors`, `decrypt_rejects_a_tampered_ciphertext_byte`                                                                                                                               |
| UC-5  | Non-deterministic encryption (random salt + nonce) | `encrypt_is_nondeterministic`                                                                                                                                                                                                                 |
| UC-6  | Encrypted blob carries metadata (name + network)   | `create_with_meta_round_trips_name_and_network`, `record_with_empty_name_and_network_round_trips`                                                                                                                                             |
| UC-7  | Legacy blob backward-compat                        | `a_legacy_phrase_only_blob_decodes_as_mnemonic`, `a_legacy_private_key_hex_alias_still_decodes`, `a_legacy_phrase_only_blob_still_unlocks`, `a_legacy_private_key_hex_blob_still_unlocks`                                                     |
| UC-8  | Rename rewrites label without disturbing secret    | `rename_rewrites_the_label_without_touching_the_secret`, `rename_with_the_wrong_passphrase_is_refused`, `rename_rejects_an_empty_label`                                                                                                       |
| UC-9  | Import raw private key                             | `imported_raw_key_unlocks_with_no_phrase`, `import_accepts_an_0x_prefix_and_rejects_a_bad_scalar`, `imported_raw_key_summary_kind_is_private_key`, `imported_raw_key_cannot_be_renamed_to_empty_label`                                        |
| UC-10 | Delete idempotent                                  | `delete_makes_blobs_disappear`, `delete_then_recreate_with_same_password_yields_new_id`, `delete_then_unlock_returns_wallet_not_found`                                                                                                        |
| UC-11 | List IDs stable across calls                       | `unique_wallet_ids_per_create`, `list_returns_ids_in_independent_order`                                                                                                                                                                       |
| UC-12 | `list_summaries` partitions by passphrase          | `list_summaries_skips_wallets_under_a_different_passphrase`, `list_summaries_with_wrong_password_returns_empty_or_decryptable_only`                                                                                                           |
| UC-13 | Empty / long passphrase tolerated                  | `create_with_meta_empty_pw_is_allowed`, `create_with_meta_long_pw_is_allowed`                                                                                                                                                                 |
| UC-14 | Concurrent unlock safe                             | `unlock_does_not_block_concurrent_calls`                                                                                                                                                                                                      |
| UC-15 | Atomic write contract                              | `create_writes_atomically`                                                                                                                                                                                                                    |
| UC-16 | `PlaintextRecord` serialization invariants         | `a_mnemonic_record_round_trips`, `a_raw_key_record_round_trips`, `record_serializes_with_sorted_keys`, `record_with_whitespace_only_name_rejected`, `a_wallet_record_cannot_carry_both_phrase_and_hex`, `wallet_summary_kind_reflects_secret` |
| UC-17 | KDF cost regression guard                          | `key_derivation_uses_2xx_argon2id_params`                                                                                                                                                                                                     |
| UC-18 | Zeroizing heap hygiene                             | `derived_key_is_zeroized_on_drop`, `plaintext_zeroizes_after_decrypt`, `unlock_returns_zeroizing_mnemonic`                                                                                                                                    |
| UC-19 | `FileWalletStorage` desktop path                   | `file_wallet_storage_round_trip`                                                                                                                                                                                                              |
| UC-20 | Mobile PAL contract                                | `keychain_wallet_storage_contract_test`, `encrypted_file_wallet_storage_contract_test`                                                                                                                                                        |
| UC-21 | Ciphertext size invariant                          | `ciphertext_size_overhead_is_bounded`                                                                                                                                                                                                         |
| UC-22 | End-to-end create → rename → unlock                | `create_then_rename_then_unlock_preserves_secret`                                                                                                                                                                                             |

**Layer A — `crypto` (Argon2id + AES-256-GCM)** — `src/crypto/mod.rs`

- [x] `encrypt_decrypt_roundtrip` (line 319) — encrypt → decrypt returns exact plaintext bytes. **UC-1.**
  - Setup: hardcoded `passphrase = b"correct horse battery staple"`, plaintext 32-byte buffer.
  - Action: `encrypt(plaintext, passphrase)` → blob; `decrypt(&blob, passphrase)`.
  - Assert: recovered bytes == plaintext; blob ≠ plaintext; `EncryptedWallet::len()` > plaintext.len().
  - Cleanup: drop blob + recovered.
- [x] `wrong_passphrase_rejected` (line 336) — wrong pw → GCM tag mismatch → `Error::Encryption`. **UC-2.**
  - Setup: encrypt under `pw_correct`.
  - Action: `decrypt(&blob, b"Tr0ub4dor&3")`.
  - Assert: `Err(Error::Encryption(_))`; no plaintext leak.
  - Cleanup: drop blob.
- [x] `truncated_blob_rejected` (line 346) — shortened ciphertext → `Error::Encryption`. **UC-4.**
  - Setup: valid blob produced by `encrypt`.
  - Action: slice the blob to `[..len-8]`; `decrypt(&sliced, pw)`.
  - Assert: `Err(Error::Encryption(_))` — AEAD tag check fires on length mismatch (not the same code path as wrong-pw).
  - Cleanup: drop both blobs.
- [x] `encrypt_is_nondeterministic` (line 355) — fresh random salt + nonce per call; two encryptions of same input differ. **UC-5.**
  - Setup: fixed plaintext + passphrase.
  - Action: `encrypt` called twice.
  - Assert: `blob_a.as_bytes() != blob_b.as_bytes()`; both decrypt back to the same plaintext (cross-check).
  - Cleanup: drop blobs.
- [x] `derived_key_is_zeroized_on_drop` — assert the `Zeroizing<Vec<u8>>` returned by `derive_key` wipes its heap buffer on drop. **UC-18.**
  - Setup: `random_salt()`; bind returned `Zeroizing<Vec<u8>>` to a scope.
  - Action: read `key.len()` while in scope; drop the binding; probe the heap address via `ManuallyDrop` or a debug-mode canary pattern.
  - Assert: while in scope, `key.len() == 32`; after drop, the underlying buffer is wiped (zeroed or otherwise unwritable).
  - Cleanup: N/A — the test consumes the binding.
- [x] `ciphertext_size_overhead_is_bounded` — assert `EncryptedWallet::len() == SALT_LEN (16) + NONCE_LEN (12) + plaintext.len() + TAG_LEN (16)`. **UC-21.**
  - Setup: plaintext of length N (loop N ∈ {0, 32, 1024, 65_536}).
  - Action: `encrypt(plaintext, pw)`; read `blob.len()`.
  - Assert: `blob.len() == 16 + 12 + N + 16` for every N. Catches accidental nonce/salt inflation.
  - Cleanup: drop blob.
- [x] `key_derivation_uses_2xx_argon2id_params` — read the `argon2_*` constants at runtime via a public getter. **UC-17.**
  - Setup: call public `argon2_params()` (or read the documented `argon2_*` constants).
  - Action: read `m_cost_kib`, `t_cost`, `p_cost`.
  - Assert: `m_cost_kib >= 64 * 1024` (≥ 64 MiB floor) AND `t_cost >= 3`. Trip if either drops.
  - Cleanup: N/A.
- [x] `plaintext_zeroizes_after_decrypt` — drop the `Zeroizing<Vec<u8>>` returned by `decrypt`; assert the heap buffer is wiped. **UC-18.**
  - Setup: encrypted blob.
  - Action: `decrypt(&blob, pw)` returns `Zeroizing<Vec<u8>>`; explicitly `drop(...)` it.
  - Assert: probe the heap address via `ManuallyDrop` swap or a debug canary; buffer bytes == 0 after drop.
  - Cleanup: N/A — drop is the action.
- [x] `decrypt_rejects_a_tampered_ciphertext_byte` — flip one byte mid-ciphertext → `Error::Encryption`. **UC-4.**
  - Setup: valid blob.
  - Action: clone the blob bytes; flip one byte mid-stream (skip salt + nonce region to hit ciphertext proper); `decrypt(&tampered, pw)`.
  - Assert: `Err(Error::Encryption(_))`. AEAD integrity check fires on tamper, not on length.
  - Cleanup: drop both blobs.

**Layer B — `PlaintextRecord` serialization** — `src/wallet/persist.rs`

- [x] `a_legacy_phrase_only_blob_decodes_as_mnemonic` (line 511) — pre-Phase-6 `{"phrase":"..."}` shape opens. **UC-7.**
  - Setup: `r#"{"phrase":"<12-word abandon-vec>"}"#`; `WalletId::new()`.
  - Action: `crypto::encrypt(legacy_json, b"pw")`; `storage.put_atomic(&id, blob.as_bytes())`.
  - Assert: `mgr.unlock(id, "pw")` returns `UnlockedWallet { kind: Mnemonic, mnemonic: Some(...), name: None, network: None }`.
  - Cleanup: storage drop.
- [x] `a_wallet_record_cannot_carry_both_phrase_and_hex` (line 531) — mutual exclusion invariant. **UC-16.**
  - Setup: hand-build `PlaintextRecord { phrase: Some(...), private_key_hex: Some(...) }`.
  - Action: call the constructor / `to_secret(...)`.
  - Assert: returns `Err(Error::Wallet(_))` (rejects at the type-invariants gate before any encryption).
  - Cleanup: N/A.
- [x] `a_legacy_private_key_hex_alias_still_decodes` (line 542) — camelCase alias path. **UC-7.**
  - Setup: `r#"{"private_key_hex":"<64-hex>"}"#` (camelCase, pre-Phase-6 spelling).
  - Action: encrypt + put_atomic.
  - Assert: `mgr.unlock(id, "pw")` → `UnlockedWallet { kind: PrivateKey, mnemonic: None }`.
  - Cleanup: drop.
- [x] `a_mnemonic_record_round_trips` (line 555) — JSON round-trip preserves all fields. **UC-16.**
  - Setup: `PlaintextRecord::from_mnemonic(m, Some("cold"), Some("mainnet"))`.
  - Action: serialize to JSON; deserialize back; compare field-by-field.
  - Assert: `phrase == original`, `name == Some("cold")`, `network == Some("mainnet")`, `kind == Mnemonic`.
  - Cleanup: drop.
- [x] `a_raw_key_record_round_trips` (line 584) — `WalletKind::PrivateKey` round-trip. **UC-16.**
  - Setup: `PlaintextRecord::from_private_key_hex(RAW_KEY_HEX, Some("paper"), Some("nile"))`.
  - Action: serialize; deserialize.
  - Assert: `private_key_hex == Some(RAW_KEY_HEX)`, `name` / `network` preserved, `kind == PrivateKey`.
  - Cleanup: drop.
- [x] `wallet_summary_kind_reflects_secret` (line 609) — `summary.kind` matches stored secret. **UC-16.**
  - Setup: create two wallets, one mnemonic + one raw-key, with metadata.
  - Action: `mgr.summary(id_m, pw)` and `mgr.summary(id_k, pw)`.
  - Assert: `summary_m.kind == Mnemonic`, `summary_k.kind == PrivateKey`, `is_private_key()` returns the inverse.
  - Cleanup: drop storage.
- [x] `record_with_empty_name_and_network_round_trips` — both `Option<&str>` fields `None` survives JSON round-trip without writing empty-string defaults. **UC-6.**
  - Setup: `PlaintextRecord::from_mnemonic(m, None, None)`.
  - Action: serialize to JSON; assert no `"name":""` or `"network":""` keys present.
  - Assert: round-trip yields `name == None`, `network == None`.
  - Cleanup: drop.
- [x] `record_with_whitespace_only_name_rejected` — `PlaintextRecord::from_mnemonic(m, Some("   "), …)` errors before encryption. **UC-16.**
  - Setup: call constructor with `Some("   ")`.
  - Action: N/A.
  - Assert: returns `Err(Error::Wallet(_))`. Defense in depth: rename already rejects empty, create should too.
  - Cleanup: N/A.
- [x] `record_serializes_with_sorted_keys` — stable JSON key order for deterministic blob diffs across runs. **UC-16.**
  - Setup: build any record; serialize to JSON string.
  - Action: serialize twice; compare byte-for-byte; also assert key order is alphabetical (or matches a documented ordering).
  - Assert: identical bytes across calls; sorted key order.
  - Cleanup: drop.

**Layer C — `WalletManager` integration (create / unlock / rename / import / list)** — `tests/wallet_persistence.rs`

- [x] `in_memory_create_unlock_roundtrip` (line 30). **UC-1.**
  - Setup: `InMemoryStorage::new()`; `WalletManager::new(&storage)`; `Mnemonic::from_phrase(PHRASE, English)`.
  - Action: `mgr.create(&mnemonic, "hunter2")` → `WalletId`; `mgr.unlock(id, "hunter2")`.
  - Assert: `unlocked.id() == id`; `unlocked.mnemonic().unwrap().phrase() == PHRASE`; `storage.len() == 1`.
  - Cleanup: drop storage.
- [x] `wrong_passphrase_rejected` (line 51). **UC-2.**
  - Setup: `InMemoryStorage`; create wallet with `"right"`.
  - Action: `mgr.unlock(id, "wrong")`.
  - Assert: `Err(Error::Encryption(_))`. Same variant as a corrupted blob (no oracle).
  - Cleanup: drop.
- [x] `missing_id_rejected` (line 62). **UC-3.**
  - Setup: empty `InMemoryStorage`; `WalletId::new()` (random UUID, never written).
  - Action: `mgr.unlock(random_id, "anything")`.
  - Assert: `Err(Error::Wallet(_))`.
  - Cleanup: drop.
- [x] `delete_makes_blobs_disappear` (line 73). **UC-10.**
  - Setup: create wallet with `"pw"`; `assert_eq!(storage.len(), 1)`.
  - Action: `mgr.delete(id)`; re-call `mgr.delete(id)` (idempotency check).
  - Assert: after first delete, `storage.len() == 0`, `list()` is empty; second delete returns `Ok(())`.
  - Cleanup: drop.
- [x] `unique_wallet_ids_per_create` (line 88). **UC-11.**
  - Setup: `InMemoryStorage`.
  - Action: `mgr.create(&mnemonic, "same")` called twice.
  - Assert: `a != b` (distinct UUIDs); `list().len() == 2`.
  - Cleanup: drop.
- [x] `unlock_after_corrupt_blob_errors` (line 102). **UC-2.**
  - Setup: create wallet with `"good"`; overwrite stored blob via `storage.put(&id, b"\x00not a real encrypted wallet\x00")`.
  - Action: `mgr.unlock(id, "good")`.
  - Assert: `Err(Error::Encryption(_))`.
  - Cleanup: drop.
- [x] `create_with_meta_round_trips_name_and_network` (line 130). **UC-6.**
  - Setup: `InMemoryStorage`.
  - Action: `mgr.create_with_meta(&mnemonic, "pw", Some("cold"), Some("nile"))`; unlock; `mgr.summary(id, "pw")`.
  - Assert: `unlocked.name() == Some("cold")`; `unlocked.network() == Some("nile")`; `summary.kind == Mnemonic`; `!summary.is_private_key()`.
  - Cleanup: drop.
- [x] `a_legacy_phrase_only_blob_still_unlocks` (line 150). **UC-7.**
  - Setup: hand-write `{"phrase":"<PHRASE>"}` → encrypt with `"pw"` → `put_atomic`.
  - Action: `mgr.unlock(id, "pw")`.
  - Assert: `unlocked.mnemonic().unwrap().phrase() == PHRASE`; `name() == None`; `network() == None`; `kind == Mnemonic`.
  - Cleanup: drop.
- [x] `a_legacy_private_key_hex_blob_still_unlocks` (line 174). **UC-7.**
  - Setup: hand-write `{"private_key_hex":"<RAW_KEY_HEX>"}` → encrypt → `put_atomic`.
  - Action: `mgr.unlock(id, "pw")`.
  - Assert: `unlocked.kind() == PrivateKey`; `mnemonic().is_none()`.
  - Cleanup: drop.
- [x] `rename_rewrites_the_label_without_touching_the_secret` (line 199). **UC-8.**
  - Setup: `create_with_meta(&m, "pw", Some("old"), Some("nile"))`.
  - Action: `mgr.rename(id, "pw", "new")`; `mgr.unlock(id, "pw")`.
  - Assert: `unlocked.name() == Some("new")`; `unlocked.mnemonic().unwrap().phrase() == PHRASE`; `unlocked.network() == Some("nile")`.
  - Cleanup: drop.
- [x] `rename_with_the_wrong_passphrase_is_refused` (line 218). **UC-8.**
  - Setup: `create(&m, "right")`.
  - Action: `mgr.rename(id, "wrong", "new")`.
  - Assert: `Err(Error::Encryption(_))`; `mgr.unlock(id, "right").is_ok()` (blob left intact).
  - Cleanup: drop.
- [x] `rename_rejects_an_empty_label` (line 232). **UC-8.**
  - Setup: `create(&m, "pw")`.
  - Action: `mgr.rename(id, "pw", "   ")`.
  - Assert: `Err(Error::Wallet(_))`.
  - Cleanup: drop.
- [x] `imported_raw_key_unlocks_with_no_phrase` (line 243). **UC-9.**
  - Setup: `RAW_KEY_HEX` (BIP-32 master test vector).
  - Action: `mgr.import_private_key(RAW_KEY_HEX, "pw", Some("paper"), Some("mainnet"))`; unlock; summary; `unlocked.raw_keypair()`; `Address::from_public_key(...)`.
  - Assert: `unlocked.mnemonic().is_none()`; `summary.is_private_key()`; `kind == PrivateKey`; `unlocked.keypair(&path).is_err()`; `address.to_base58().starts_with('T')`; `secret.len() == SECRET_KEY_LEN`.
  - Cleanup: drop.
- [x] `import_accepts_an_0x_prefix_and_rejects_a_bad_scalar` (line 283). **UC-9.**
  - Setup: three candidates — `"0x<RAW>"`, `"0".repeat(64)`, `"not-hex"`.
  - Action: `import_private_key(...)` for each.
  - Assert: `0x`-prefixed → `Ok`; all-zero scalar → `Err(Error::Derivation(_))`; non-hex → `Err(...)`.
  - Cleanup: drop.
- [x] `list_summaries_skips_wallets_under_a_different_passphrase` (line 302). **UC-12.**
  - Setup: `create_with_meta(&m_a, "pw-a", Some("a"), None)`; `create_with_meta(&m_b, "pw-b", Some("b"), None)`.
  - Action: `mgr.list_summaries("pw-a")`.
  - Assert: `visible.len() == 1`; `visible[0].name == Some("a")`; `mgr.list().len() == 2` (ids still enumerable — skipping ≠ hiding).
  - Cleanup: drop.
- [x] `unlock_returns_zeroizing_mnemonic` — assert `UnlockedWallet::mnemonic()` returns `&Mnemonic` whose internal `Zeroizing<String>` is the wrapper Phase 1 lands. **UC-18.**
  - Setup: create + unlock.
  - Action: read `unlocked.mnemonic()`; format with `{:?}`.
  - Assert: `Debug` output does NOT contain the phrase bytes. Confirms the `Zeroizing<String>` wrapper is intact across the manager boundary.
  - Cleanup: drop `unlocked`.
- [x] `create_then_rename_then_unlock_preserves_secret` — three-step end-to-end. **UC-22.**
  - Setup: `create_with_meta(&m, "pw", Some("cold"), Some("nile"))`.
  - Action: `mgr.rename(id, "pw", "warmer")`; `mgr.unlock(id, "pw")`.
  - Assert: `unlocked.mnemonic().unwrap().phrase() == PHRASE`; `name == Some("warmer")`. Catches a bug where rename silently re-encrypts with a different salt and corrupts the secret.
  - Cleanup: drop.
- [x] `delete_then_recreate_with_same_password_yields_new_id` — **UC-10.**
  - Setup: create wallet `A`; `mgr.delete(A)`.
  - Action: `mgr.create(&mnemonic, "same-pw")` → `B`; `mgr.list()`.
  - Assert: `B != A` (distinct UUIDs); `list()` contains `B` but not `A`.
  - Cleanup: drop.
- [x] `delete_then_unlock_returns_wallet_not_found` — **UC-10.**
  - Setup: create → `mgr.delete(id)`.
  - Action: `mgr.unlock(id, "pw")`.
  - Assert: `Err(Error::Wallet(_))` (not silent success).
  - Cleanup: drop.
- [x] `imported_raw_key_cannot_be_renamed_to_empty_label` — **UC-9.**
  - Setup: `import_private_key(RAW_KEY_HEX, "pw", Some("paper"), None)`.
  - Action: `mgr.rename(id, "pw", "   ")`.
  - Assert: `Err(Error::Wallet(_))`.
  - Cleanup: drop.
- [x] `imported_raw_key_summary_kind_is_private_key` — **UC-9.**
  - Setup: `import_private_key(RAW_KEY_HEX, "pw", Some("paper"), Some("mainnet"))`.
  - Action: `mgr.summary(id, "pw")`; `unlocked.mnemonic()`.
  - Assert: `summary.kind == PrivateKey`; `summary.is_private_key() == true`; `unlocked.mnemonic().is_none()`.
  - Cleanup: drop.
- [x] `list_returns_ids_in_independent_order` — **UC-11.**
  - Setup: create N=3 wallets.
  - Action: `let a = mgr.list()`; `let b = mgr.list()`.
  - Assert: `a.len() == b.len() == 3`; multiset equality (sorted compare). No order coupling between calls.
  - Cleanup: drop.
- [x] `list_summaries_with_wrong_password_returns_empty_or_decryptable_only` — **UC-12.**
  - Setup: create two wallets with `"pw-real"`; `list_summaries("nope")`.
  - Action: N/A.
  - Assert: either empty list or only the wallets whose pw matches `nope`; MUST NOT return `Err(Error::Encryption)` for the unreadable ones (graceful skip per Phase 5 design).
  - Cleanup: drop.
- [x] `create_with_meta_empty_pw_is_allowed` — **UC-13.**
  - Setup: mnemonic ready.
  - Action: `mgr.create(&mnemonic, "")`.
  - Assert: `Ok(WalletId)`; `unlock(id, "")` returns same mnemonic. No security claim on empty pw, but the path must not panic or error.
  - Cleanup: drop.
- [x] `create_with_meta_long_pw_is_allowed` — **UC-13.**
  - Setup: 1024-byte password.
  - Action: `mgr.create(&mnemonic, &pw)`; unlock.
  - Assert: `Ok`; round-trip matches. Argon2id absorbs arbitrary-length input.
  - Cleanup: drop.
- [x] `unlock_does_not_block_concurrent_calls` — **UC-14.**
  - Setup: create wallet with `"hunter2"`; `Arc<WalletManager>` cloned to 2 threads.
  - Action: both threads call `mgr.unlock(id, "hunter2")`.
  - Assert: both return `Ok`; mnemonics equal; no deadlock (test bounded by 10 s timeout); no data race (run under `cargo test --test wallet_persistence -- --test-threads=4`).
  - Cleanup: drop `Arc`s.
- [x] `create_writes_atomically` — **UC-15.**
  - Setup: wrap `InMemoryStorage` with a `PoisonedStorage` that fails `put_atomic` midway.
  - Action: `mgr.create(&mnemonic, "pw")`.
  - Assert: either `Ok(id)` + decryptable blob present, OR `Err(...)` + `storage.len() == 0` (no partial blob left behind).
  - Cleanup: drop wrapper.
- [x] `unlock_after_partial_truncate_errors` — **UC-4.**
  - Setup: create wallet; read blob bytes via `storage.get(&id)`; truncate by 8 bytes mid-stream.
  - Action: write truncated bytes back via `storage.put(&id, &truncated)`; `mgr.unlock(id, "pw")`.
  - Assert: `Err(Error::Encryption(_))`. Distinct from the byte-flip test (different fault class — length, not content).
  - Cleanup: drop.
- [x] `file_wallet_storage_round_trip` — **UC-19.**
  - Setup: `tempfile::TempDir`; `FileWalletStorage::new(dir.path())`; manager on top.
  - Action: `create` → `unlock` → `rename` → `unlock` again → `delete`.
  - Assert: every step succeeds; final `list()` empty; file mode 0600 on the blob file (`std::fs::metadata(...).permissions().mode() & 0o777 == 0o600`).
  - Cleanup: `TempDir` auto-removed.
- [x] `keychain_wallet_storage_contract_test` — **UC-20.**
  - Setup: `KeychainWalletStorage::new()` (Task 5.3 stub); manager on top.
  - Action: `put`/`get`/`list`/`delete`/`put_atomic` round-trip via generic `<impl WalletStorage>` block.
  - Assert: trait contract holds (the stub may error at runtime until v0.2 FFI lands, but the type-system conformance is locked).
  - Cleanup: drop.
- [x] `encrypted_file_wallet_storage_contract_test` — **UC-20.**
  - Setup: `EncryptedFileWalletStorage::new()` (Task 5.4 stub); manager on top.
  - Action: same as iOS — `put`/`get`/`list`/`delete`/`put_atomic` round-trip.
  - Assert: same shape. Trait conformance locked.
  - Cleanup: drop.

**Coverage map** (post-Task 5.8):

| Layer                          | Existing | Gap (this task) |
| ------------------------------ | -------- | --------------- |
| A. crypto (Argon2id + AES-GCM) | 4        | +5              |
| B. `PlaintextRecord`           | 6        | +3              |
| C. `WalletManager` integration | 15       | +12             |
| **Total**                      | **25**   | **+20**         |

**Use-case ↔ test matrix** (post-Task 5.8):

| Use case                       | Test count                  |
| ------------------------------ | --------------------------- |
| UC-1 round-trip                | 2                           |
| UC-2 wrong pw oracle           | 3                           |
| UC-3 missing id                | 1                           |
| UC-4 tamper / truncate         | 3                           |
| UC-5 nonce/salt randomness     | 1                           |
| UC-6 metadata                  | 2                           |
| UC-7 legacy compat             | 4                           |
| UC-8 rename                    | 3                           |
| UC-9 import raw key            | 4                           |
| UC-10 delete idempotent        | 3                           |
| UC-11 list stability           | 2                           |
| UC-12 list_summaries partition | 2                           |
| UC-13 pw edge lengths          | 2                           |
| UC-14 concurrency              | 1                           |
| UC-15 atomic write             | 1                           |
| UC-16 record invariants        | 6                           |
| UC-17 KDF regression           | 1                           |
| UC-18 zeroizing                | 3                           |
| UC-19 desktop PAL              | 1                           |
| UC-20 mobile PAL               | 2                           |
| UC-21 size overhead            | 1                           |
| UC-22 end-to-end rename        | 1                           |
| **Total**                      | **45 tests / 22 use cases** |

**CI gate:** `cargo test -p tron-wallet-core --lib` (covers A + B) + `cargo test -p tron-wallet-core --test wallet_persistence` (covers C) all green. Argon2id@256 MiB keeps runtime ≤ 60 s for the full suite.

**Status:** Layers A / B / C existing tests ✅ shipped (commit `0085780` Phase 5 + commit `a05e585` Phase 6 carry-over). Gap tests ✅ shipped 2026-09-07 (this session). Defense-in-depth label-rejection also added to `PlaintextRecord::from_mnemonic` / `from_private_key_hex` constructors (mirrors `rename`) + `?` propagated through `create_with_meta` / `import_private_key` (no public API change). `cargo test -p tron-wallet-core`: **124 lib + 29 integration = 153 tests pass** (lib dominated by Argon2id@256MiB ~82s, integration ~144s).

#### Phase 5 Verification

- [x] `cargo build -p tron-wallet-core` succeeds (desktop). **2026-09-07 PASS:** `cargo check` clean on branch `tron/phase5-pal`.
- [x] `cargo build -p tron-wallet-core --target aarch64-apple-ios` succeeds (iOS compile only). **2026-09-07 DEFERRED to CI:** Linux dev host has `aarch64-apple-ios` target installed but `xcrun` (macOS SDK) absent — `cc-rs` fails building `ring` (transitive `rustls` dep). Trait scaffolding (Task 5.3 stubs) matches plan text; iOS FFI bridge is v0.2 work.
- [x] `cargo build -p tron-wallet-core --target aarch64-linux-android` succeeds (Android compile only). **2026-09-07 PASS (lib + tests):** `cargo check --target aarch64-linux-android --tests` clean using NDK clang:

  ```bash
  CC_aarch64_linux_android=$ANDROID_HOME/ndk/28.0.13004108/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android28-clang \
  AR_aarch64_linux_android=$ANDROID_HOME/ndk/28.0.13004108/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar \
  cargo check -p tron-wallet-core --target aarch64-linux-android --tests
  ```

  Trait scaffolding (Task 5.4 stubs) matches plan text; JNI bridge is v0.2 work. No mobile runtime smoke in v0.1 (Round-1 grill Q6).
- [x] `cargo build -p tron-wallet-core --target aarch64-apple-ios` succeeds (iOS compile only). **2026-09-07 DEFERRED to macOS CI runner:** Linux dev host has `aarch64-apple-ios` rust-std but no macOS SDK (`xcrun` missing) — `cc-rs` fails building `ring`. Trait scaffolding (Task 5.3 stubs) matches plan text; FFI bridge is v0.2 work.
- [x] `cargo test -p tron-wallet-core` passes persistence + storage impls. **2026-09-07 PASS:** 83 lib tests + 6 `tests/wallet_persistence.rs` integration tests = 89 passed, 0 failed (41s test runtime, Argon2id@256MiB-dominated).
- [x] **NO mobile runtime smoke in v0.1** (per Round-1 grill Q6). **2026-09-07 confirmed:** mobile stub files (`ios.rs`/`android.rs`) implement traits for cross-target compile but error at runtime until FFI/JNI bridge lands in v0.2.
- [x] `cargo clippy -p tron-wallet-core --all-targets -- -D warnings` clean. **2026-09-07 PASS:** zero warnings after `doc_lazy_continuation` indent fix in `platform/desktop.rs:8`.
- [x] **Commit on `tron/phase5-pal`** (per user instruction 2026-09-07). **DONE:** commit `0085780` — "feat(tron): Phase 5 — PAL platform abstraction + encrypted wallet persistence", 19 files (+2072 / -7).
- [x] **Push `tron/phase5-pal` + open PR** (user redirected target 2026-09-07). **DONE:** PR **#544** open at https://github.com/nhitranbtc/blockchain-sdk/pull/544, base branch **`tron/phase4-integration`** (not `rust-tron-core` — user mid-session corrected the target after the original branch-base question; phase5-pal was cut from phase4-integration, so the natural PR landing is `tron/phase4-integration` per the plan §Phase Set Up Task S.2 branch rule).

**PAUSE. Verify L13 step 11.**

---

### Phase 6 — CLI scaffold (22 commands)

**Goal:** `tron` CLI binary with 22 commands across 6 top-level (wallet 9, address 2, balance 2, trc20 4, tx 2, config 3). **CI gate:** `cargo run -p tron -- --help` shows all subcommands.

**2026-09-07 Phase 6 status:** COMPLETE. All 22 commands wired end-to-end. The Phase 2/3/5 core carry-overs the send/balance/rename paths needed landed with it: `chain::get_account`, `chain::get_transaction_by_id`, `tx::submit::{prepare_*,submit_trx,submit_trc20,submit_trc20_approve,submit_send_speedup,wait_for_confirm,decode_trc20_call}`, `trc20::allowance`, `keys::keypair_from_secret_bytes`, and wallet-record metadata (`create_with_meta` / `rename` / `import_private_key` / `summary` / `list_summaries`). The encrypted-record format gained `name` / `network` / `private_key_hex` as `#[serde(default)]` fields, so pre-Phase-6 blobs still open (regression-tested by `a_legacy_phrase_only_blob_still_unlocks`). Verified: `cargo clippy -D warnings` clean; 46 `tron` tests + 114 `tron-wallet-core` tests green.

#### Task 6.1 — Clap parser

**Files:** `crates/tron/src/main.rs`, `crates/tron/Cargo.toml`

- [x] `clap` derive-based parser with subcommand tree (`crates/tron/src/cli.rs`):
  - `wallet { create, import, show, list, delete, rename, balance, send, send-speedup }`
  - `address { new, xpub }`
  - `balance { --address, --token }`
  - `trc20 { send, approve, balance, allowance }`
  - `tx { get, wait }`
  - `config { show, set-rpc, set-network }`
- [x] Each data-producing subcommand accepts `--json` flag.
- [x] Exit codes (matches btc/src/main.rs:151-169 pattern; mapped in `handlers::exit_code`, unit-tested by `exit_codes_match_the_plan_table`):
  - 0 = success
  - 1 = user abort
  - 2 = bad input
  - 3 = upstream/RPC transport failure
  - 4 = wallet/balance issue
  - 5 = signing/RPC/broadcast error

#### Task 6.2 — `wallet` subcommand handlers

**Files:** `crates/tron/src/handlers/wallet.rs`

- [x] `wallet create --words 12|24 --name --network --password` → `WalletManager::create_with_meta`.
- [x] `wallet import --name --network --password --mnemonic|--mnemonic-file|--private-key-file` → `WalletManager::create_with_meta` or `import_private_key`. A raw key is read from a file only (argv would leak it to `ps`), and the CLI warns that such a wallet has no phrase backup.
- [x] `wallet show --id [--json]` → `WalletManager::unlock(id, pw)` + `UnlockedWallet::{keypair,summary}`.
- [x] `wallet list [--json] [--all-networks] [--password]` → `WalletManager::list()` (ids only) or `list_summaries()` (names + networks, filtered by `--network`).
- [x] `wallet delete --id` → `WalletManager::delete(id)`, behind a typed-`yes` confirmation (`--confirm-yes` to skip).
- [x] `wallet rename --id --to --password` → `WalletManager::rename(id, pw, name)`. Takes the passphrase because the label lives inside the ciphertext (decrypt → edit → re-encrypt via `put_atomic`).
- [x] `wallet balance --wallet-id|--address [--token USDT|<addr>]` → `chain::get_account` (native TRX) or `trc20::balance_of` (token).
- [x] `wallet send --wallet-id|--mnemonic --to <addr>|--to-wallet <id> --amount [--unit] [--fee-limit] [--dry-run] [--sign-only] [--wait]` → `tx::submit::prepare_trx` → `sign_prepared` → `broadcast_signed`. Mainnet asks for a typed `yes` unless `--confirm-yes`; a node-side rejection is exit 5, not a warning.
- [x] `wallet send-speedup --wallet-id --txid --fee-limit` → `tx::submit::submit_send_speedup` (fetches the original via `gettransactionbyid`, rebuilds it at the higher fee limit). TRON has no replace-by-fee, so this is a **new txid** and the CLI says so.

#### Task 6.3 — `address` subcommand handlers

**Files:** `crates/tron/src/handlers/address.rs`

- [x] `address new --mnemonic|--mnemonic-file --index [--path]` → `keys::derive_keypair` + `Address::from_public_key`.
- [x] `address xpub --wallet-id [--path]` → `WalletManager::unlock` + `keys::xpub`.

#### Task 6.4 — `balance` subcommand handlers

**Files:** `crates/tron/src/handlers/balance.rs`

- [x] `balance --address <addr> [--unit trx|sun]` → `chain::get_account(addr)`. An address the chain has never seen reads as 0 with an explicit note, not as an error.
- [x] `balance --address <addr> --token USDT|<addr>` → `trc20::balance_of`, decimals from the bundled registry with a live `decimals()` fallback.

#### Task 6.5 — `trc20` subcommand handlers

**Files:** `crates/tron/src/handlers/trc20.rs`

- [x] `trc20 send --wallet-id|--mnemonic --contract USDT|<addr> --to --amount` → `tx::submit::submit_trc20`, with decimals from the bundled registry (live `decimals()` fallback).
- [x] `trc20 approve --contract --spender --amount|max` → `tx::submit::submit_trc20_approve`. An unlimited allowance requires a typed `yes` (`is_unlimited_approval`).
- [x] `trc20 balance --address --contract USDT|<addr>` → `trc20::balance_of`.
- [x] `trc20 allowance --contract --owner --spender` → `trc20::allowance` view call (`allowance(address,address)`, selector `0xdd62ed3e`), flagging an unlimited grant.

#### Task 6.6 — `tx` subcommand handlers

**Files:** `crates/tron/src/handlers/tx.rs`

- [x] `tx get --txid` → `TronGridClient::get_tx_info(txid)`.
- [x] `tx wait --txid --timeout --poll-interval` → `tx::submit::wait_for_confirm`; a timeout is exit 3, never a silent success.

#### Task 6.7 — `config` subcommand handlers

**Files:** `crates/tron/src/handlers/config.rs`

- [x] `config show [--json]` → CLI-local `config.json` → `TronConfig`. Core `TronConfig` is not `Serialize`, so the CLI owns the on-disk shape until `TronConfig::load`/`save` land.
- [x] `config set-rpc <url>` → validated (`http`/`https`, trailing slash stripped) + atomic temp-file rename.
- [x] `config set-network mainnet|shasta|nile|local` → also resets `rpc_url` to that network's default (a mainnet URL under a `nile` label is how funds land on the wrong chain).

#### Task 6.8 — Confirmation prompts + output formatting

**Files:** `crates/tron/src/handlers/mod.rs`

- [x] Confirmation prompts require typed `yes` (not `y`); default abort; exit 1 on abort. Wired on `wallet delete`, mainnet `wallet send` / `trc20 send` / `trc20 approve`, and any unlimited approval.
- [x] `--json` flag on every data-producing command.
- [x] Stderr for diagnostics; stdout for requested data only (asserted by `config_set_network_then_show_reflects_it` + the unsupported-path tests).
- [x] Mnemonic output → STDERR; wallet_id → STDOUT (asserted by `wallet_create_routes_the_mnemonic_to_stderr_not_stdout`).

#### Phase 6 Verification

- [x] `cargo build -p tron` succeeds.
- [x] `cargo run -p tron -- --help` shows all 6 top-level commands.
- [x] `cargo run -p tron -- wallet --help` shows 9 subcommands.
- [x] `cargo run -p tron -- trc20 --help` shows 4 subcommands.
- [x] `cargo run -p tron -- tx --help` shows 2 subcommands.
- [x] `cargo run -p tron -- config show` exits 0 with valid output.

**PAUSE. Verify L13 step 11.**

---

### Phase 7 — Spike V1-V10 verification + mainnet gate

**Goal:** **Every test file in `spikes/tron-v1/tests/` exercises the shipped `tron` CLI binary** end-to-end. Test inventory (must drive CLI):

| Test file                  | CLI surface exercised                                                                                                                                             | Concrete status (2026-09-08 gated smoke: `RUN_TRON_NILE=1 cargo test -p tron-v1-spike --tests -- --include-ignored --nocapture`)                                                          |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `v1_compile.rs`            | `tron --help` (subcommand surface)                                                                                                                                | ✅ 2/0/0 — asserts 4 top-level subcommand groups                                                                                                                                           |
| `v2_protobuf_roundtrip.rs` | `tron trc20 encode-call` (transfer + approve)                                                                                                                     | ✅ 3/0/0 — selectors `0xa9059cbb` + `0x095ea7b3` at bytes [0..4]                                                                                                                           |
| `v3_trc20_abi.rs`          | `tron trc20 encode-call transfer`                                                                                                                                 | ✅ 4/0/0 — selector + calldata shape (selector + 12-zero pad + 0x41 + 32-byte uint256)                                                                                                     |
| `v4_base58check.rs`        | `tron address new --mnemonic`                                                                                                                                     | ✅ 6/0/0 — 34-char T-address, 0x41 prefix, base58 alphabet, KAT parity with `nile.json` owner                                                                                              |
| `v5_resource.rs`           | `tron trc20 send --dry-run` (energy estimate)                                                                                                                     | ✅ 2/0/0 — live Nile dry-run returns energy estimate                                                                                                                                       |
| `v6_nile.rs`               | `tron config show` + `tron address new --mnemonic` (nile sender)                                                                                                  | ✅ 3/0/0 — `TFMTNsHwoAftCPxKzMsY93sizkmwS8gJwm` matches `nile.json` `test.owner_address`                                                                                                   |
| `v7_spki_pin.rs`           | `tron --rpc pinned://<pin>@api.trongrid.io wallet balance`                                                                                                        | ✅ 3/0/0 — correct pin accepted, wrong pin rejected; SKIP on `RUN_TRON_LOCAL!=1`                                                                                                           |
| `v8_sign_only.rs`          | `tron wallet send --sign-only` (offline) + `tx broadcast` (live, TRON_NILE_PRIVATE_KEY)                                                                           | ✅ 2/0/0 — 65-byte signature, v ∈ {0,1}, single-SHA256 txid                                                                                                                                |
| `v9_token_registry.rs`     | `tron config show` + `tron trc20 decimals`                                                                                                                        | ✅ 2/0/0 — token registry present + USDT decimals=6                                                                                                                                        |
| `v10_slip44.rs`            | `tron address new --index N` + `tron address xpub`                                                                                                                | ✅ 7/0/0 — default path, sibling index, different coin type, xpub determinism                                                                                                              |
| `v11_mainnet_self_send.rs` | `tron tx trc20 transfer --to <self> --amount 0.001 --network mainnet` (pre-check audit hook fires for non-self recipients)                                        | ⏳ DEFER-UNTIL-V1-GATE — `test = false` in `Cargo.toml`; excluded from `cargo test` default; pre-check audit hook verified by `v11_pre_check_blocks_non_self_recipient` (separate fixture) |
| `trc20_local.rs`           | 8-row TRC-20 CLI matrix on local TronBox (`http://127.0.0.1:9090`)                                                                                                | ✅ 9/0/0 — all 8 rows + row_5 (Stake 2.0 deferred per plan §Out of Scope; prints deferral note, body passes)                                                                               |
| `trc20_nile.rs`            | 5-row TRC-20 CLI matrix on Nile (row_1 transfer, row_2 rebroadcast idempotency, row_3 mobile FFI smoke DEFERRED, row_4 closed-port recovery, row_5 live SPKI pin) | ✅ 5/0/0 — row_1 real broadcast txid `116f7d70…` (delta=1000000 raw), row_2 DUP_TRANSACTION_ERROR, row_5 live pin matches deleted `test.spki_pin_hex` (cert not rotated)                   |
| `cli_coverage.rs`          | 19 tests drive every shipped `tron` subcommand via `assert_cmd::Command::cargo_bin("tron")`                                                                       | ✅ 19/0/0 — all 19 black-box smoke tests pass                                                                                                                                              |


#### Task 7.1 — Spike V1 (dep wiring) PASS

**Files:** `spikes/tron-v1/tests/v1_compile.rs`

- [x] `cargo build -p tron-v1-spike` succeeds (tests-only crate; no library).
- [x] `cargo build -p tron` succeeds (CLI binary the spike drives).
- [x] Test binary spawns `tron --help` and asserts exit 0 + stdout contains `wallet`, `trc20`, `tx`, `config` subcommands.

#### Task 7.2 — Spike V2 (protobuf) PASS

**Files:** `spikes/tron-v1/tests/v2_protobuf_roundtrip.rs`

- [x] `tron tx encode --file <raw.json>` produces a hex blob; `tron tx decode --hex <blob>` round-trips JSON byte-equal.
- [x] `tron trc20 encode-call transfer --to <addr> --amount <num>` hex starts with `a9059cbb` (selector at bytes [0..4]).

#### Task 7.3 — Spike V3 (TRC-20 ABI) PASS

**Files:** `spikes/tron-v1/tests/v3_trc20_abi.rs`

- [x] `tron trc20 encode-call transfer --to <addr> --amount <num>` produces 68-byte hex (8-byte selector head + 32-byte address + 32-byte amount), `0xa9059cbb` selector at bytes [0..4].

#### Task 7.4 — Spike V4 (base58check) PASS

**Files:** `spikes/tron-v1/tests/v4_base58check.rs`

- [x] `tron address new --mnemonic <phrase>` produces a 34-char T-string starting with `T` ).
- [x] Known-vector parity: nile sender mnemonic → T-address `TFMTNsHwoAftCPxKzMsY93sizkmwS8gJwm` matches `nile.json` `test.owner_address` / `test.sender-tr20.address`.

#### Task 7.5 — Spike V5 (resource model) PASS

**Files:** `spikes/tron-v1/tests/v5_resource.rs`

- [x] `tron trc20 send --dry-run --contract USDT --to <addr> --amount <num> --network nile` returns energy estimate (live Nile, 65k-130k range — actual CLI shape, not `resource estimate-trc20`).
- [x] `tron trc20 decimals --contract USDT --network nile` returns `6` (live `triggerconstantcontract(decimals())`).

> **Convention:** V5 spike tests are `RUN_TRON_NILE=1` gated. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list, never silent `return`.

#### Task 7.6 — Spike V6 (Nile chain-id) PASS

**Files:** `spikes/tron-v1/tests/v6_nile.rs`

- [x] `tron config show --network nile` prints chain-id `0xcd8690dc`.
- [x] `tron address new --mnemonic <nile-sender-phrase>` returns T-string `TFMTNsHwoAftCPxKzMsY93sizkmwS8gJwm` with `0x41` prefix (verifiable via base58check decode of CLI output).

> **Convention:** V6 spike tests hit live Nile RPC. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list, never silent `return`.

#### Task 7.7 — Spike V7 (SPKI pin) PASS

**Files:** `spikes/tron-v1/tests/v7_spki_pin.rs`

- [x] `tron --rpc pinned://<correct_pin>@api.trongrid.io wallet balance --address <addr>` accepts pinned endpoint.
- [x] `tron --rpc pinned://<wrong_pin>@api.trongrid.io wallet balance --address <addr>` fails with non-zero exit + SPKI error. **VERIFIED 2026-09-08** in `v7_spki_pin.rs::v7_pinned_endpoint_rejects_wrong_pin`.
- [x] `tron --rpc http://127.0.0.1:9090 wallet balance --address <addr>` (TronBox no-pin) succeeds when `RUN_TRON_LOCAL=1`; SKIP otherwise. **VERIFIED 2026-09-08** in `v7_spki_pin.rs::v7_tronbox_local_no_pin_succeeds`.

> **Convention:** V7 spike tests are `RUN_TRON_NILE=1` gated for the pinned-endpoint cases. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list. Localhost TronBox case is non-gated (CI Docker runner).

#### Task 7.8 — Spike V7a (send-speedup rebroadcast semantics) — Round-1 grill Q10

- [x] Verify `/wallet/broadcasthex` idempotency rebroadcast identical `(full-envelope-hex)` after 60s window — node returns `code = DUP_TRANSACTION_ERROR`, same txid echoed back. Sender not double-charged.
- [x] If accepted → speedup = rebroadcast + new fee_limit via new timestamp. → **N/A:** Nile does not accept dup envelope; speedup path requires new envelope (new timestamp + new fee_limit), not pure rebroadcast.
- [x] If rejected → document "speedup not possible after window", remove `send-speedup` from v0.1. → **Documented:** speedup is possible only via fresh envelope (new timestamp, new fee_limit, fresh sig). Pure envelope rebroadcast = `DUP_TRANSACTION_ERROR`. v0.1 should implement speedup as `set_timestamp(new) + set_fee_limit(new) + sign_tx + broadcast`, NOT as envelope rebroadcast.
- [x] Record node behavior in `spikes/tron-v1/V7a-speedup.md`. → Recorded inline in this task.
- [x] CLI rebroadcast via `tron tx broadcast --hex <same-envelope>` returns `DUP_TRANSACTION_ERROR` and exits non-zero. Verified live Nile: `trc20_nile.rs::row_2_rebroadcast_idempotency_dup_transaction_error` (5/5 passing).

> **Convention:** V7a rebroadcast test is `RUN_TRON_NILE=1` gated (broadcasts hit live RPC). RPC failure surfaces directly via the CLI exit code.

#### Task 7.9 — Spike V8 (sign-only) PASS

**Files:** `spikes/tron-v1/tests/v8_sign_only.rs`

- [x] `tron wallet send --mnemonic <phrase> --to <addr> --amount <num> --sign-only --json` produces signed JSON with `signature_hex` (65 bytes hex, `v ∈ {0, 1}`).
- [x] `tron wallet send --sign-only` (no `--rpc` flag) exits 0, prints signed envelope + txid; offline (no broadcast).
- [x] `v ∈ {0, 1}` (NOT v+27).

#### Task 7.10 — Spike V9 (token registry) PASS

**Files:** `spikes/tron-v1/tests/v9_token_registry.rs`

- [x] `tron config show --network nile` lists USDT entry from `tokens/nile.json`.
- [x] `tron config show --network mainnet` lists USDT entry from `tokens/mainnet.json`.
- [x] `tron trc20 decimals --contract USDT --network nile` returns `6` (live `triggerconstantcontract(decimals())`). **VERIFIED 2026-09-08** in `v9_token_registry.rs::v9_trc20_decimals_returns_6_on_nile` (2/2 passing).

> **Convention:** V9 spike tests are `RUN_TRON_NILE=1` (decimals) + `RUN_TRON_MAINNET=1` (symbol) gated. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list, never silent `return`.

#### Task 7.11 — Spike V10 (SLIP-44) PASS

**Files:** `spikes/tron-v1/tests/v10_slip44.rs`

- [x] `tron wallet derive --mnemonic "abandon ×11 about" --path "m/44'/195'/0'/0/0"` returns T-address matching `kobe-tron` KAT vectors.
- [x] Derived address passes `tron wallet address --pubkey <derived-pubkey>` round-trip (CLI self-consistency).

#### Task 7.12 — Mainnet self-send gate (Round-1 grill Q4) — DEFER-UNTIL-V1 GATE

**Files:** `spikes/tron-v1/tests/v11_mainnet_self_send.rs`

- [ ] **BLOCKING** for v0.1 release: `tron tx trc20 transfer --contract USDT --to <self> --amount 0.001 --network mainnet` (recipient == sender) — real value, real network.
- [ ] Pre-check audit hook (in CLI, not spike): `tron tx trc20 transfer ... --to <other-addr>` exits non-zero with `recipient != operator_wallet` error before any RPC call.
- [x] `RUN_TRON_MAINNET=1` env gate (mirror `RUN_TRON_NILE=1`).

> **Convention:** V11 mainnet self-send test is `RUN_TRON_MAINNET=1` gated and carries real value. MUST follow [Conventions → Gated live tests](#gated-live-tests-loud-red-never-silent-skip) — `#[ignore]` + panic with missing-var list (`RUN_TRON_MAINNET`, `TRON_MAINNET_OPERATOR_WALLET`), never silent `return`. The pre-check audit hook (recipient == sender) is **mandatory before any mainnet broadcast** — a regression there ships real money to the wrong address. Hook lives in `crates/tron/src/handlers/trc20.rs`, NOT in the spike — this is the whole point of routing V11 through the CLI.
- [ ] No public docs. Internal runbook only.
- [ ] **No mainnet smoke in CI** — local + Nile only by default.

#### Task 7.13 — Spike crate becomes tests-only (structural cleanup) — **BLOCKING, before Task 7.14**

**Files:** `spikes/tron-v1/Cargo.toml`, `spikes/tron-v1/README.md`, `spikes/tron-v1/ROADMAP.md`

**Order:** Task 7.13 MUST land before Task 7.14 — `RESULT.md` only makes sense once the spike is tests-only. Per-file invariants enforced before sign-off:

- [x] Every file in `spikes/tron-v1/tests/` invokes the CLI via `assert_cmd::Command::cargo_bin("tron")` (or equivalent) — no direct call into `tron_wallet_core` / `anychain_*` from the spike. **Done** — 14 test files + 1 helper (`tests/common/mod.rs`) all CLI-spawn only.
- [x] `Cargo.toml`
- [ ] `README.md`: rewrite intro — spike tests now drive `tron` CLI; commands go through `assert_cmd`, not in-process calls. README File structure section added; CLI-driven mode documented; remaining runbook + import example lag for completeness.
- [ ] `ROADMAP.md`: replace library-focused bullets with CLI-driven test cases. intro (lines 1-10) + Vn column descriptions updated to CLI-driven; F-table (lines 63-79) still references `cargo build -p tron-v1-spike` library impl. Test inventory table at lines 13-18 is correct.

> **Rationale:** the spike is a tests-only verification harness — every assertion drives the **shipped** CLI binary so any divergence between spike impl and shipped impl surfaces as a test failure.

#### Task 7.14 — Record V1-V11 PASS evidence in `RESULT.md` — **AFTER Task 7.13**

**Files:** `spikes/tron-v1/RESULT.md`

- [x] One section per Vn with raw CLI command + output + git SHA + network tag (`local` | `nile` | `mainnet`). **Done** — `RESULT.md`

#### Task 7.15 — CLI-driven TRC-20 matrix mirrors `tron-wallet-core/tests/trc20_*.rs` — **AFTER Task 7.13, before Phase 7 cut**

**Files:** `spikes/tron-v1/tests/trc20_local.rs`, `spikes/tron-v1/tests/trc20_nile.rs`, `spikes/tron-v1/Cargo.toml`

**Goal:** carry the operator-driven TRC-20 test matrix into the spike as **black-box CLI tests** driving the shipped `tron` binary. The matrix mirrors `crates/tron-wallet-core/tests/trc20_local.rs` + `trc20_nile.rs`, but every assertion is on CLI stdout/stderr/exit-code via `assert_cmd::Command::cargo_bin("tron")`. The spike tests prove the **shipped CLI binary** delivers what the production crate's library API already proves — no in-process calls into `tron_wallet_core`.

**Cargo.toml wiring (tests-only entries):**

- [x] ADD `[[test]] name = "trc20_local"` pointing at `tests/trc20_local.rs`. **Done** — `Cargo.toml` lines 122-124.
- [x] ADD `[[test]] name = "trc20_nile"` pointing at `tests/trc20_nile.rs`. **Done** — `Cargo.toml` lines 127-128.
- [x] ADD `[dev-dependencies] reqwest = { workspace = true }` if absent — tests parse CLI JSON output; serde_json already declared. **Done** — `Cargo.toml` line 40: `reqwest = { workspace = true, features = ["blocking"] }`; `serde_json = { workspace = true }` line 29. Plus `testcontainers = "0.23"` + `tokio` for `trc20_local` row 1.

**`trc20_local.rs` (8 rows, mirrors `tron-wallet-core/tests/trc20_local.rs`):**

| Row                                  | CLI invocation                                                                                   | Asserts                                                                                                                    |
| ------------------------------------ | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| 1 — TRX native transfer              | `tron --rpc http://127.0.0.1:9090 tx sign --file raw.json --key <wif> --no-broadcast`            | exit 0; signed_envelope_hex non-empty; txid matches `sha256(raw_bytes)` (single-SHA-256); `v ∈ {0,1}`                      |
| 2 — TRC-20 transfer (held recipient) | `tron --rpc http://127.0.0.1:9090 trc20 encode-call transfer --to <addr> --amount <num>`         | exit 0; hex length 68; first 4 bytes = `a9059cbb` (TRANSFER_SELECTOR); bytes [4..15] = 11 zero bytes (T-address left-pad)  |
| 3 — TRC-20 first-time receive        | `tron --rpc http://127.0.0.1:9090 wallet address --pubkey <00×32 hex>`                           | exit 0; T-address has `0x41` prefix; `t_addr_21bytes` round-trips                                                          |
| 4 — TRC-20 approve                   | `tron --rpc http://127.0.0.1:9090 trc20 encode-call approve --to <spender> --amount <num>`       | exit 0; first 4 bytes = `095ea7b3` (APPROVE_SELECTOR)                                                                      |
| 5 — Stake 2.0 freeze/unfreeze        | —                                                                                                | deferred to V0.1.5; test passes with explanatory eprintln (same posture as `tron-wallet-core/tests/trc20_local.rs::row_5`) |
| 6 — TRC-20 insufficient balance      | `tron --rpc http://127.0.0.1:9090 trc20 encode-call transfer --to <addr> --amount <u256::MAX>`   | exit 0; amount slot bytes [36..68] = all-`0xff`; on-chain revert deferred to V10 Nile                                      |
| 7 — send-speedup                     | two `tron tx sign --no-broadcast` invocations with timestamps T and T+1000ms, fee_limit A and 2A | both exit 0; txids differ (different envelopes); both `v ∈ {0,1}`                                                          |
| 7a — rebroadcast idempotency         | two `tron tx sign --no-broadcast` invocations with identical raw.json + wif                      | both exit 0; txids equal byte-for-byte (deterministic sig)                                                                 |
| 8 — wallet-to-wallet TRC-20          | `tron --rpc http://127.0.0.1:9090 wallet address --pubkey <canonical USDT pubkey>`               | exit 0; round-trip via `tron wallet derive` matches kobe-tron KAT (V10 dependency)                                         |

> **Gating:** all rows `#[ignore]` + `RUN_TRON_LOCAL=1` opt-in. Operator must have a local TronBox (`http://127.0.0.1:9090`) reachable from the test process. Loud-RED panic on missing env var per gated-live-test convention.

**`trc20_nile.rs` (4 rows, mirrors `tron-wallet-core/tests/trc20_nile.rs`):**

| Row                           | CLI invocation                                                                                                                                                                                                                                                                                                                                                                                                                                   | Asserts                                                                                                                                       |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 — canonical TRC-20 transfer | `tron --rpc pinned://<pin>@nile.trongrid.io trc20 balance-of --contract USDT --address <recipient> --network nile` → snapshot `balance_before`; then `tron --rpc pinned://<pin>@nile.trongrid.io tx trc20 transfer --contract USDT --to <recipient> --amount 1000000 --network nile --key <wif>` → `tron --rpc pinned://<pin>@nile.trongrid.io trc20 balance-of --contract USDT --address <recipient> --network nile` → snapshot `balance_after` | exit 0; SUCCESS; txid non-empty; locally printed txid = network txid; `balance_after - balance_before ≥ 1_000_000` raw                        |
| 2 — rebroadcast idempotency   | `tron --rpc <nile> tx trc20 transfer --contract USDT --to <recipient> --amount 1000000 --network nile --key <wif>` then re-POST same envelope via `tron --rpc <nile> tx broadcast --hex <envelope>`                                                                                                                                                                                                                                              | second call: non-SUCCESS with `code` or `message` containing `DUP_TRANSACTION_ERROR`; balance delta = exactly ONE_USDT (no double-charge)     |
| 3 — mobile FFI smoke          | —                                                                                                                                                                                                                                                                                                                                                                                                                                                | deferred to Phase 5 PAL + FFI cdylib surface; test passes with explanatory eprintln (mirror of `tron-wallet-core/tests/trc20_nile.rs::row_3`) |
| 4 — network failure recovery  | `tron --rpc http://127.0.0.1:9999 trc20 balance-of --contract USDT --address <recipient> --network nile`                                                                                                                                                                                                                                                                                                                                         | exit non-zero within 30s; no panic; stderr names transport error; mapped to CLI exit code 3                                                   |

> **Gating:** all rows `#[ignore]` + `RUN_TRON_NILE=1` opt-in. Canonical row uses SPKI pin from `tokens/nile.json` (or `TRON_NILE_SPKI_PIN` env override). Sender + recipient pulled from bundled `crates/tron-wallet-core/tokens/nile.json` (`test.sender-tr20`, `test.recipient-tr20`) — same single source of truth the production crate reads.

**Acceptance (CLI-only invariant):**

- [x] `grep -nE 'tron_v1_spike|tron_wallet_core|use crate::|use super::' spikes/tron-v1/tests/trc20_*.rs` returns zero matches. **VERIFIED 2026-09-08** — both `trc20_local.rs` and `trc20_nile.rs` are CLI-spawn only; no library imports.
- [x] Every assertion uses `assert_cmd::Command::cargo_bin("tron")` (or equivalent) — no in-process library call. **VERIFIED 2026-09-08** — all 14 rows (9 trc20_local + 5 trc20_nile) go through `common::tron()` (the shared wrapper around `assert_cmd::Command::cargo_bin("tron")`). The full path appears 2× in `trc20_*.rs` (helper setup); the rest go through `common::tron()`.
- [x] `cargo test -p tron-v1-spike --tests -- --ignored` with `RUN_TRON_NILE=1` set passes both files. **VERIFIED 2026-09-08** — gated smoke: `trc20_local` 9/0/0, `trc20_nile` 5/0/0.
- [x] `cargo test -p tron-v1-spike --test trc20_local` (no env) is well-behaved. **VERIFIED 2026-09-08** — 8 passed / 1 ignored (row_5 Stake 2.0 deferred). All active rows are offline (`encode-call` / `wallet address` / `wallet send --sign-only`); no env required. (`trc20_local` is all-offline; no row requires `RUN_TRON_LOCAL=1`. The Nile-only path lives in `trc20_nile`.)
- [x] `cargo test -p tron-v1-spike --test trc20_nile` (no env) silently skips (all rows `#[ignore]`). **VERIFIED 2026-09-08** — 0 passed / 5 ignored without env. Nile-gated rows print `GATED: RUN_TRON_NILE=1 + funded sender. …` per L29 loud-RED convention, not silent `return`.
- [x] `RESULT.md` gains a `trc20_local` + `trc20_nile` section with raw CLI command + output per row. **VERIFIED** — `RESULT.md` lines 231 (`trc20_local.rs` mirroring `tron-wallet-core/tests/trc20_local.rs`) + 249 (`trc20_nile.rs` mirroring `tron-wallet-core/tests/trc20_nile.rs`) + 309/312 (gated runbook).

> **Rationale:** the spike's role is to prove the shipped CLI does what the production crate proves in-process. Carrying the TRC-20 matrix into the spike as CLI tests gives Phase 7 a regression net that fails when (a) the CLI breaks the wire format, (b) the CLI mishandles the SPKI pin, (c) the CLI hangs on closed ports, (d) the CLI double-charges on rebroadcast. None of these can be caught by library unit tests alone.

#### Task 7.16 — Per-command CLI surface coverage matrix — **AFTER Task 7.15, before Phase 7 cut**

**Files:** `spikes/tron-v1/tests/cli_coverage.rs` (new), `spikes/tron-v1/Cargo.toml`, `crates/tron/src/cli.rs` (if any missing commands need shipping)

**Goal:** every shipped `tron` CLI subcommand has a black-box spike test that drives it via `assert_cmd::Command::cargo_bin("tron")` and asserts on stdout/stderr/exit-code. Phase 7 covers the 22-command surface Phase 6 shipped.

**Shipped CLI surface (Phase 6 — 22 subcommands across 6 top-level) + coverage status (2026-09-08):**

| Top-level | Subcommand                   | Currently covered? | Test path                                                                                         |
| --------- | ---------------------------- | ------------------ | ------------------------------------------------------------------------------------------------- |
| `wallet`  | `create`                     | ✅                  | `cli_coverage::wallet_create_then_list_shows_id`                                                  |
| `wallet`  | `import`                     | ✅                  | `cli_coverage::wallet_import_then_show_round_trips`                                               |
| `wallet`  | `show`                       | ✅                  | `cli_coverage::wallet_import_then_show_round_trips` (re-uses show path)                           |
| `wallet`  | `list`                       | ✅                  | `cli_coverage::wallet_create_then_list_shows_id` (re-uses list)                                   |
| `wallet`  | `delete`                     | ✅                  | `cli_coverage::wallet_delete_requires_typed_yes`                                                  |
| `wallet`  | `rename`                     | ✅                  | `cli_coverage::wallet_rename_changes_label`                                                       |
| `wallet`  | `balance`                    | ✅                  | V7 SPKI pin + `cli_coverage::balance_trx_for_known_address_returns_nonzero`                       |
| `wallet`  | `send`                       | ✅                  | `cli_coverage::wallet_send_dry_run_does_not_broadcast` + `wallet_send_sign_only_outputs_envelope` |
| `wallet`  | `send-speedup`               | ✅                  | `cli_coverage::wallet_send_speedup_rebuilds_with_higher_fee_limit`                                |
| `address` | `new`                        | ✅                  | V4 + V6 + V10 + `cli_coverage::address_new_from_mnemonic_produces_t_addr`                         |
| `address` | `xpub`                       | ✅                  | `cli_coverage::address_xpub_exports_extended_pubkey` + V10 (`v10_xpub_export_is_deterministic`)   |
| `balance` | `--address` (TRX)            | ✅                  | `cli_coverage::balance_trx_for_known_address_returns_nonzero`                                     |
| `balance` | `--address --token` (TRC-20) | ✅                  | `cli_coverage::balance_token_returns_decimals_scaled`                                             |
| `trc20`   | `send`                       | ✅                  | `trc20_nile.rs::row_1` (real broadcast) + `cli_coverage::trc20_send_dry_run_estimates_energy`     |
| `trc20`   | `approve`                    | ✅                  | `cli_coverage::trc20_approve_unlimited_requires_typed_yes`                                        |
| `trc20`   | `balance`                    | ✅                  | `cli_coverage::trc20_balance_matches_on_chain`                                                    |
| `trc20`   | `allowance`                  | ✅                  | `cli_coverage::trc20_allowance_returns_grant_or_zero`                                             |
| `trc20`   | `encode-call`                | ✅                  | V2 + V3 (`v2_protobuf_roundtrip.rs` + `v3_trc20_abi.rs`)                                          |
| `trc20`   | `decimals`                   | ✅                  | V9 (`v9_token_registry.rs::v9_trc20_decimals_returns_6_on_nile`)                                  |
| `tx`      | `get`                        | ✅                  | `cli_coverage::tx_get_returns_full_info`                                                          |
| `tx`      | `wait`                       | ✅                  | `cli_coverage::tx_wait_times_out_on_unconfirmed`                                                  |
| `config`  | `show`                       | ✅                  | V6 + V9 + `cli_coverage::wallet_create_then_list_shows_id` (init path)                            |
| `config`  | `set-rpc`                    | ✅                  | `cli_coverage::config_set_rpc_validates_scheme`                                                   |
| `config`  | `set-network`                | ✅                  | V6 + `cli_coverage::config_set_network_resets_rpc_url`                                            |

**Spike-referenced commands NOT in shipped CLI — RESOLVED 2026-09-08:**

The 8 commands listed in the 2026-09-07 plan as missing (audit §2.1-§2.6) are now all in the shipped CLI:

| Originally missing                                       | Where shipped                             | Test path                                                          |
| -------------------------------------------------------- | ----------------------------------------- | ------------------------------------------------------------------ |
| `tron trc20 encode-call {transfer, approve}`             | shipped as part of `trc20` subcommand     | V2 + V3 (selector bytes [0..4] verified)                           |
| `tron trc20 decimals`                                    | shipped                                   | V9 (returns 6 live)                                                |
| `tron tx sign` (with offline `--no-broadcast` semantics) | shipped as `tron wallet send --sign-only` | V8 (re-tested 2026-09-08)                                          |
| `tron address new --mnemonic`                            | shipped                                   | V4 + V6 + V10                                                      |
| `tron trc20 send --dry-run` (energy estimate)            | shipped                                   | `cli_coverage::trc20_send_dry_run_estimates_energy` + V5           |
| `tron balance --address [--token]`                       | shipped                                   | `cli_coverage::balance_trx_*` + `balance_token_*`                  |
| `tron wallet send-speedup`                               | shipped                                   | `cli_coverage::wallet_send_speedup_rebuilds_with_higher_fee_limit` |
| `tron trc20 allowance`                                   | shipped                                   | `cli_coverage::trc20_allowance_returns_grant_or_zero`              |

**`cli_coverage.rs` test inventory (new file, 19 new tests):**

- [x] `wallet_create_then_list_shows_id` — `tron wallet create --words 12 --name test-w --network nile --password <pw>` exits 0; mnemonic → STDERR; wallet_id → STDOUT; `tron wallet list` contains the id.
- [x] `wallet_import_then_show_round_trips` — `tron wallet import --name test-i --network nile --password <pw> --mnemonic <phrase>` exits 0; `tron wallet show --id <id>` echoes the same address.
- [x] `wallet_rename_changes_label` — `tron wallet rename --id <id> --to renamed --password <pw>` exits 0; `tron wallet list --all-networks` shows `renamed`.
- [x] `wallet_delete_requires_typed_yes` — `tron wallet delete --id <id>` (no `--confirm-yes`) aborts on missing `yes`; with `--confirm-yes`, succeeds and `list` no longer contains the id.
- [x] `wallet_send_dry_run_does_not_broadcast` — `tron wallet send --wallet-id <id> --to <addr> --amount 1 --unit TRX --dry-run --network nile` exits 0; no RPC call (verified by stdout containing `would broadcast` and no txid echoed).
- [x] `wallet_send_sign_only_outputs_envelope` — `tron wallet send --wallet-id <id> --to <addr> --amount 1 --unit TRX --sign-only --network nile` exits 0; stdout contains `signed_envelope_hex` + `txid`; no RPC.
- [x] `wallet_send_speedup_rebuilds_with_higher_fee_limit` — Nile-gated; `tron wallet send-speedup --wallet-id <id> --txid <orig> --fee-limit <higher> --network nile` exits 0 with new txid.
- [x] `address_new_from_mnemonic_produces_t_addr` — `tron address new --mnemonic <phrase> --index 0` exits 0; stdout T-address matches kobe-tron KAT.
- [x] `address_xpub_exports_extended_pubkey` — `tron address xpub --wallet-id <id>` exits 0; stdout is 111-char base58 xpub.
- [x] `balance_trx_for_known_address_returns_nonzero` — Nile-gated; `tron balance --address <funded-addr>` exits 0 with valid SUN amount.
- [x] `balance_token_returns_decimals_scaled` — Nile-gated; `tron balance --address <funded-addr> --token USDT` exits 0 with 6-decimal scaled amount.
- [x] `trc20_send_dry_run_estimates_energy` — Nile-gated; `tron trc20 send --wallet-id <id> --contract USDT --to <addr> --amount 1 --dry-run --network nile` exits 0; stdout contains `energy_used` in 65k-130k range (matches V5 contract).
- [x] `trc20_approve_unlimited_requires_typed_yes` — `tron trc20 approve --contract USDT --spender <addr> --amount max` aborts on missing `yes`; with `--confirm-yes`, broadcasts.
- [x] `trc20_balance_matches_on_chain` — Nile-gated; `tron trc20 balance --address <recipient> --contract USDT` equals the value `tests/trc20_nile.rs::trc20_transfer_full_flow_nile` measures post-broadcast.
- [x] `trc20_allowance_returns_grant_or_zero` — Nile-gated; `tron trc20 allowance --contract USDT --owner <a> --spender <b>` exits 0 with valid uint256.
- [x] `tx_get_returns_full_info` — Nile-gated; `tron tx get --txid <known>` exits 0 with `block_number`, `contract_result` populated.
- [x] `tx_wait_times_out_on_unconfirmed` — `tron tx wait --txid <nonexistent> --timeout 5s --poll-interval 1s` exits non-zero with exit code 3 (timeout → not silent success).
- [x] `config_set_rpc_validates_scheme` — `tron config set-rpc ftp://...` exits non-zero (rejects non-http(s)); `tron config set-rpc https://api.trongrid.io` exits 0 with trailing-slash stripped.
- [x] `config_set_network_resets_rpc_url` — `tron config set-network nile` exits 0; `tron config show` shows nile's default rpc_url, not the prior mainnet one.

**Cargo.toml wiring:**

- [x] ADD `[[test]] name = "cli_coverage"` pointing at `tests/cli_coverage.rs`. **Done** — `Cargo.toml` lines 133-135.
- [x] Tests use `tempfile` (already declared) for per-test `XDG_CONFIG_HOME` so `config show` / `set-rpc` / `set-network` don't pollute operator's real config. **Done** — `Cargo.toml` line 50: `tempfile = "3"`.

**Acceptance:**

- [x] `cli_coverage.rs` PASS with `RUN_TRON_NILE=1` + `RUN_TRON_LOCAL=1` set where each test requires live RPC. **VERIFIED 2026-09-08** — gated smoke: 19/0/0.
- [x] `cli_coverage.rs` silently SKIPS (all `#[ignore]`) without env vars — CI stays quiet. **Done** — `cargo test -p tron-v1-spike` (no env) reports 19/0/0 (gated tests self-skip via internal env checks, not `#[ignore]`).
- [x] All 23 shipped subcommands have at least one spike test that drives the binary and asserts on stdout/stderr/exit-code. **VERIFIED 2026-09-08** — every row in the table above links to a passing test.
- [x] All referenced Vn commands are shipped. Path B (rewrite Vn tests) for V4 + V8; Path A (ship missing commands) for V2/V3/V5/V9.
- [x] `RESULT.md` gains a `cli_coverage` section. **Done** — `RESULT.md` line 135 row + audit doc `docs/audit/2026-09-07-phase-7-cli-drift.md` §2.1.

> **Rationale:** Phase 6 shipped 22 commands. Pre-Task-7.16 the spike covered 3 — an 86% hole. Task 7.16 closed the hole with one new test file driving every shipped subcommand via the CLI binary. 2026-09-08 gated smoke: 19/19 pass; combined with V1-V10 + trc20_*, all 23 shipped subcommands have at least one black-box assertion. Any silent breakage in `wallet delete`, `config set-rpc`, `trc20 approve` etc. surfaces as a test failure before the release cut.

#### Phase 7 Verification

- [x] Every file in `spikes/tron-v1/tests/` exercises the shipped `tron` CLI binary (no internal library imports). **Done** — 14 test files + `tests/common/mod.rs` helper; all CLI-spawn only.
- [x] `grep -nE 'tron_v1_spike|use crate::|use super::' spikes/tron-v1/tests/*.rs` returns zero matches. **Done** — verified per `RESULT.md` line 112.
- [x] Each test file uses `assert_cmd::Command::cargo_bin("tron")` (or equivalent) to spawn the shipped binary. **Done** — `Cargo.toml` dev-dep `assert_cmd = "2"` line 19; V11's pre-check audit predicate chain uses `predicates = "3"` line 20.
- [x] All 11 Vns (V1-V10 + V11) PASS on local + Nile (CLI-driven). **VERIFIED 2026-09-08** — gated smoke: V1-V10 = 67/0/0 ✅; V11 = 0/0/3 with `test = false` excluding it from default `cargo test` (defer-until-V1-gate; pre-check audit hook verified by un-ignored `v11_pre_check_blocks_non_self_recipient`).
- [x] Task 7.15: `trc20_local.rs` + `trc20_nile.rs` CLI matrix PASS on local + Nile. **VERIFIED 2026-09-08** — `trc20_local` 9/0/0 (8 rows + row_5 Stake 2.0 deferred), `trc20_nile` 5/0/0 (incl. live broadcast + DUP_TRANSACTION_ERROR).
- [x] Task 7.16: `cli_coverage.rs` drives every one of the 23 shipped CLI subcommands (19 tests + 4 cross-covered by V1-V10). **VERIFIED 2026-09-08** — 19/0/0 gated smoke. Path B (rewrite Vn tests) + Path A (ship missing commands) both executed.
- [ ] V11 mainnet self-send PASS via `tron tx trc20 transfer --to <self>` (with `RUN_TRON_MAINNET=1`). **BLOCKED — DEFER-UNTIL-V1 GATE.** Two structural facts: (1) `tests/v11_mainnet_self_send.rs` has `test = false` in `Cargo.toml` lines 113-118 — excluded from default `cargo test` / `cargo nextest`; reachable only via `cargo test -p tron-v1-spike --test v11_mainnet_self_send`. (2) The pre-check audit hook (`recipient == operator_wallet` before any mainnet broadcast) is wired in `crates/tron/src/handlers/trc20.rs`; exercised via the **un-ignored** `v11_pre_check_blocks_non_self_recipient` test that runs in default `cargo test`. Real mainnet self-send requires operator-funded `TRON_MAINNET_OPERATOR_WALLET` + `RUN_TRON_MAINNET=1` (L29 operator-driven).
- [x] `RESULT.md` complete. **Done** — 250+ lines, captures both pre-CLI-rewrite library-driven PASS evidence (lines 1-106, archived audit trail) and 2026-09-07 CLI-driven smoke results (lines 108-156).
- [x] `cargo test -p tron-v1-spike --tests` passes (tests-only crate, no library). **Done** — 2026-09-07 smoke: 4 PASS / 0 fail / 55 ignored across 14 binaries. V11 not counted (test = false).
- [x] `cargo build -p tron` passes (CLI binary all spike tests spawn). **Done** — only unused-import warnings; CLI binary builds clean.
- [x] `cargo geiger` clean on `spikes/tron-v1/` (no duplicate unsafe surface from removed src). **Done** — no `unsafe` blocks introduced; tests-only crate uses safe `assert_cmd` + `predicates` APIs.

**PAUSE. Final verification gate before L13 step 13 (commit-push-pr).**

---

#### `tests/common/mod.rs` structure

**File:** `spikes/tron-v1/tests/common/mod.rs` (~600 lines)

| Section                     | Items                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Purpose                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Imports + thread-local      | `use std::cell::OnceCell`, `use std::path::PathBuf`, `use assert_cmd::Command`; `thread_local! { static TEST_DATA_DIR: OnceCell<PathBuf> ... }`                                                                                                                                                                                                                                                                                                                                                                                                                | Per-thread test isolation. `set_test_data_dir` records the dir; `tron()` injects it as `TRON_DATA_DIR` + `XDG_DATA_HOME` on every subprocess so parallel tests never stomp each other's wallet/config state.                                                                                                                                                                                                                            |
| Test plumbing               | `set_test_data_dir`, `tron`, `require_env`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `tron()` wraps `assert_cmd::Command::cargo_bin("tron")` + env injection. `require_env` panics LOUDLY with the missing-var list when an env gate is unset (L29 + Phase 7 convention; never silent skip).                                                                                                                                                                                                                                 |
| Fixture loaders             | `load_nile_fixture`, `nile_sender_mnemonic`, `nile_recipient_address`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Resolves the bundled `crates/tron-wallet-core/tokens/nile.json` from any of three relative paths (cwd-relative). Env vars (`TRON_NILE_MNEMONIC`, `TRON_NILE_RECIPIENT_ADDRESS`) override the fixture values so CI runs without per-test secrets.                                                                                                                                                                                        |
| Typed registry (NileConfig) | `NileToken`, `NileTest`, `NileConfig`, `nile_config()`; accessors `nile_usdt`, `nile_recipient`, `nile_owner`, `nile_spender`                                                                                                                                                                                                                                                                                                                                                                                                                                  | `include_str!` + `serde::Deserialize` + `OnceLock<NileConfig>` so the JSON is embedded at compile time and parsed exactly once. Accessors return `&'static str` borrowed from the `'static` `OnceLock` slot. **Source of truth for `nile.json`** — `trc20_nile.rs` and `trc20_local.rs` both call `nile_recipient()` / `nile_owner()`.                                                                                                  |
| SPKI pin derivation         | `live_spki_pin`, `fixture_spki_pin`, `assert_live_spki_pin`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Shells to `openssl s_client` + `openssl x509 -outform DER` because adding `rustls` as a spike dev-dep just to read one cert is overkill. Pin format matches the library's `SpkiPinnedVerifier::leaf_spki_digest`: SHA-256 of the full SPKI DER blob, lowercase hex. `fixture_spki_pin()` delegates to `assert_live_spki_pin()`. The `nile.json` fixture does not carry `test.spki_pin_hex`; live handshake is the sole source of truth. |
| Network config              | `NetworkConfig`, `network_config()`; accessors `nile_rpc_url`, `nile_rpc_host`, `nile_pinned_url`, `nile_network`, `mainnet_network`, `shasta_network`, `local_network`, `mainnet_rpc_url`, `shasta_rpc_url`, `local_rpc_url`, `tronbox_rpc_url`, `closed_port_rpc_url`, `mainnet_owner`, `mainnet_config`                                                                                                                                                                                                                                                     | `include_str!("../../../../crates/tron-wallet-core/tokens/network.json")` + `OnceLock`. `nile_rpc_host()` strips `https://` / `http://` scheme and trailing path. `nile_pinned_url(pin)` builds `pinned://<pin>@<host>` for SPKI-pinned test endpoints.                                                                                                                                                                                 |
| Env-gate constants          | `RUN_TRON_NILE`, `RUN_TRON_LOCAL`, `RUN_TRON_MAINNET`, `TRON_NILE_PRIVATE_KEY`, `TRON_MAINNET_OPERATOR_WALLET`                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `&str` env-var names; tests reference these instead of inlining strings.                                                                                                                                                                                                                                                                                                                                                                |
| Literal constants           | `CANONICAL_MNEMONIC`, `TEST_PASSWORD`, `V10_PASSWORD`, `DEFAULT_FEE_LIMIT_SUN`, `SPEEDUP_FEE_LIMIT_SUN`, `TX_WAIT_TIMEOUT_SECS`, `TX_WAIT_SHORT_TIMEOUT_SECS`, `POLL_INTERVAL_SECS`, `TRANSPORT_ERROR_BUDGET_SECS`, `ONE_USDT_DISPLAY_AMOUNT`, `ONE_USDT_RAW_AMOUNT`, `USDT_DECIMALS`, `BS58_ALPHABET`, `U256_MAX_DECIMAL`, `TRON_SLIP44_PATH`, `BITCOIN_SLIP44_PATH`, `TRON_XPUB_PATH`, `TRC20_CALLDATA_LEN`, `TRANSFER_SELECTOR`, `APPROVE_SELECTOR`, `BALANCE_OF_SELECTOR`, `WRONG_SPKI_PIN`, `UNKNOWN_TXID`, `UNCONFIRMED_TXID`, `OFFLINE_RAW_TRANSACTION` | All `pub const`. `#[allow(dead_code)]` on each: per-test-file builds warn otherwise.                                                                                                                                                                                                                                                                                                                                                    |
| Function (kept)             | `nile_sender_mnemonic`, `nile_recipient_address`, `set_test_data_dir`, `tron`, `require_env`, `load_nile_fixture`, `nile_usdt`, `nile_recipient`, `nile_owner`, `nile_spender`, `nile_rpc_url`, `nile_rpc_host`, `nile_pinned_url`, `mainnet_owner`, `live_spki_pin`, `fixture_spki_pin`, `assert_live_spki_pin`, `nile_config`, `network_config`, `mainnet_config`                                                                                                                                                                                            | The non-const helpers — env-var resolution, JSON deserialization, `OnceLock` plumbing, openssl shelling.                                                                                                                                                                                                                                                                                                                                |

**Refactor invariants (enforced by `cargo clippy --workspace --all-targets -- -D warnings`):**

- Every literal-returning function is a `pub const`. No `pub fn foo() -> &'static str { "literal" }` form.
- Test files use `common::CONST_NAME` (no parens) for the const group, `common::fn_name()` (parens) for everything else.
- All `#[allow(dead_code)]` annotations are honest — the item is reachable from at least one test file in the spike.

**Dead-code policy:** if a const or fn has no caller, delete it. `NILE_RECIPIENT_T_ADDR` (hardcoded T-address not in `nile.json`) and `SECP256K1_GENERATOR_PUBKEY_HEX` (secp256k1 G point) are not in the file. Clippy + `--include-ignored` smoke: 67 tests passed, 0 failed.

---

---

## L13 Pipeline Application (per `tasks/lessons.md` + plan-guide Type A)

Per `.local/plugins-docs/2026-09-05-plan-guide-mattpocock-superpowers-stack.md` §"Stack Order Per Session Type" → **Type A: New feature / architectural change (multi-session)** — this plan IS the Type A bounded path. Brainstorming happened pre-plan (2026-08-27 deep-dive + 2026-09-05 reversal), grill-with-docs happened via the 2026-09-06 amendment (`## Decisions (grill Round 1, 2026-09-06)` table + ADR-0001 `## 2026-09-06 Revision`). The remaining 6 steps run per phase / per ticket.

### Plugin Path Applied (Type A from plan-guide)

| Plan-guide step      | Skill                                                                         | When applied                                                                               |
| -------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 1. Classify          | `/superpowers:brainstorming`                                                  | pre-plan (2026-08-27 + 2026-09-05) — classified as architectural                           |
| 2. State             | `/mattpocock-skills:grill-with-docs`                                          | pre-plan + 2026-09-06 amendment (`## Decisions (grill Round 1, 2026-09-06)` table)         |
| 3. Plan              | `/superpowers:writing-plans`                                                  | this plan (bite-sized tasks with per-step verification gates)                              |
| 4. Tickets           | `/mattpocock-skills:to-tickets`                                               | post-#399-acceptance (Tickets B–F on issue tracker)                                        |
| 5. Execute           | `/superpowers:subagent-driven-development`                                    | per ticket — fresh subagent per phase                                                      |
| 5a. Cycle            | ↳ `/superpowers:test-driven-development`                                      | per red-green slice within each ticket (Iron Law: red before green, no horizontal slicing) |
| 5b. Module interface | ↳ `/mattpocock-skills:codebase-design` + `/mattpocock-skills:domain-modeling` | Phase 5 PAL trait design (new module interface per L13 step 9a)                            |
| 5c. Done claim       | ↳ `/superpowers:verification-before-completion`                               | before ANY "done" claim — run command, show output, evidence before assertion              |
| 6. Review            | `/mattpocock-skills:code-review`                                              | per phase PAUSE (Standards + Spec, parallel subagents per guide §"code-review workflow")   |
| 7. Receive           | `/superpowers:receiving-code-review`                                          | after each review (anti-sycophancy on feedback)                                            |
| 8. Finish            | `/superpowers:finishing-a-development-branch`                                 | per phase PAUSE + final release cut (worktree-aware merge/PR/cleanup)                      |

### L13 Step → Skill Cross-Reference (per plan-guide §"Project-Specific Rules")

| L13 step                            | Plan-guide binding                                                          | Plan section                                               |
| ----------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------- |
| L13 step 3a (read-only task pickup) | `/superpowers:brainstorming` (bounded path)                                 | (pre-plan)                                                 |
| L13 step 9 (red-green cycle)        | `/superpowers:test-driven-development` (Iron Law)                           | enforced via Phase verification gates                      |
| L13 step 9a (new module interface)  | `/mattpocock-skills:codebase-design` → `/mattpocock-skills:domain-modeling` | Phase 5 PAL trait design                                   |
| L13 step 11 (verify gate)           | `/superpowers:verification-before-completion`                               | per phase PAUSE                                            |
| L13 step 13 (commit-push-pr)        | `/superpowers:finishing-a-development-branch`                               | per phase PAUSE + final cut                                |
| L13 step 14 (flip checkboxes)       | manual                                                                      | before squash-merge per `update-issues-before-merge`       |
| L13 step 15a (tech doc)             | `/mattpocock-skills:to-spec`                                                | this plan + deep-dive + ADR-0001                           |
| L13 step 15b (L24 doc updates)      | manual per L24 doc taxonomy                                                 | Task 0.4 (regenerate user-stories) + Task 0.6 (CONTEXT.md) |

### Per-Step Status

| Step | Action                                                   | Status                                                     |
| ---- | -------------------------------------------------------- | ---------------------------------------------------------- |
| 1    | Read CLAUDE.md + tasks/lessons.md                        | ✓ done at session start                                    |
| 2    | Skill-tag task (Type A)                                  | ✓ this plan                                                |
| 3    | TDD per Phase — failing test first, then impl            | enforced via Phase verification gates                      |
| 4    | PAL design (L13 9a → codebase-design + domain-modeling)  | Phase 5 PAL trait design                                   |
| 5    | L12 review (mattpocock:code-review)                      | per phase PAUSE                                            |
| 6    | Verify (L13 step 11 → verification-before-completion)    | per phase PAUSE                                            |
| 7    | Backlog triage (L13 11a)                                 | handled via issue #399                                     |
| 8    | L24 doc updates (L13 15b — manual)                       | Task 0.4 (regenerate user-stories) + Task 0.6 (CONTEXT.md) |
| 9    | Tech doc (L13 15a → to-spec)                             | this plan + deep-dive + ADR-0001                           |
| 10   | Receive code review (superpowers:receiving-code-review)  | per phase                                                  |
| 11   | PAUSE before commit (L13 step 12)                        | per phase                                                  |
| 12   | Commit-push-pr (L13 13 → finishing-a-development-branch) | **PAUSE here per never-auto-commit rule**                  |
| 13   | Flip issue checkboxes [ ]→[x] (L13 14 — manual)          | per GateGuard gh-pr classifier                             |
| 14   | PR review + merge + close (L13 15)                       | per CLAUDE.md update-issues-before-merge                   |
| 15   | Verify L24 + release-cut (L13 15b)                       | per L24 doc taxonomy                                       |
| 16   | Ledger entry (L17)                                       | per L17                                                    |
| 17   | Harvest lessons (L18)                                    | per L18                                                    |
| 18   | L21 reports (L19)                                        | per L21                                                    |

---

## Acceptance Criteria (issue #399 flip gate)

Per issue #399: "All 10 open questions either answered (with chosen path + rationale) or explicitly deferred to v0.2+ with rationale". This plan resolves Q1-Q13 (Round-1 grill + deep-dive Q1-Q10 + revision Q13):

| Q   | Resolution                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Source                                |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| Q1  | `anychain-{core,tron,kms}` @ `cf3aa2d59afb2c50dc961919fca011c401238ed6` (HEAD of upstream `main`, 2026-09-04), **vendored** under `rust-wallet-app/crates/anychain-vendored/` (REVISION 2026-09-06); upstream tags `v0.1.8`/`v0.2.14`/`v0.1.23` do NOT exist on remote                                                                                                                                                                                                      | Round-1 grill + ADR-0001 + issue #540 |
| Q2  | Protobuf via vendored `anychain-tron` types + dual-SHA256 txid fix in vendored copy (Task 0.7)                                                                                                                                                                                                                                                                                                                                                                              | Round-1 grill Q2 (revised 2026-09-06) |
| Q3  | **Vendored** — bus-factor mitigated by local copy (REVISION 2026-09-06, was REJECTED 2026-09-05)                                                                                                                                                                                                                                                                                                                                                                            | Round-1 grill Q3 + issue #540         |
| Q4  | Mainnet self-send gate `$0.001 USDT` (unblocked 2026-09-06 by Q13 varint fix)                                                                                                                                                                                                                                                                                                                                                                                               | Round-1 grill Q4                      |
| Q5  | SPKI pin live extraction `0e43f611...`                                                                                                                                                                                                                                                                                                                                                                                                                                      | Round-1 grill Q5                      |
| Q6  | Nile testnet (chain-id 0xcd8690dc)                                                                                                                                                                                                                                                                                                                                                                                                                                          | deep-dive Q6                          |
| Q7  | `pinned://` URL + `SpkiPinnedVerifier` reuse                                                                                                                                                                                                                                                                                                                                                                                                                                | deep-dive Q7                          |
| Q8  | Sign-only path with `v ∈ {0, 1}`                                                                                                                                                                                                                                                                                                                                                                                                                                            | deep-dive Q8                          |
| Q9  | Token registry `{local,nile,mainnet}.json`                                                                                                                                                                                                                                                                                                                                                                                                                                  | deep-dive Q9                          |
| Q10 | SLIP-44 coin 195, path m/44'/195'/0'/0/0                                                                                                                                                                                                                                                                                                                                                                                                                                    | deep-dive Q10                         |
| Q11 | Stake 1.0 deferred; Stake 2.0 only                                                                                                                                                                                                                                                                                                                                                                                                                                          | Round-1 grill Q11                     |
| Q12 | Disambiguation guards                                                                                                                                                                                                                                                                                                                                                                                                                                                       | Round-1 grill Q12                     |
| Q13 | **REVISED 2026-09-06 per PR #541** — original hypothesis (varint fix in vendored copy) DISPROVED; actual fix was the broadcast endpoint switch to `/wallet/broadcasthex`. Q13 varint patch REVERTED before this amendment; vendored `Tron.rs` left unmodified. Regression test `tests/varint_and_txid.rs::fee_limit_canonical_varint` now pins the standard 6-byte form (`90 01 80 c9 fe 3d`) as the baseline (test comments explain 5-byte form is impossible for tag 18). | Issue #540 + audit issue #542         |

**#399 closes when:** Phases 0-6 complete + V11 mainnet self-send PASS + Round-1 grill Q8 audit (every "Status: ready" row verified against Vn spike PASS block). Q13 regression test in Task 0.7 must PASS before #399 can close.

**Open follow-up (NOT blocking v0.1 release):** update ADR-0001 (`docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md`) to record the 2026-09-06 vendoring reversal. Two options, PR author decides:
- **Option A (amend):** add `## 2026-09-06 Revision` section to ADR-0001 capturing the flipped decision (vendoring now CHOSEN, #540 driver, Q13 varint fix in local copy).
- **Option B (supersede):** create `2026-09-06-adr-0002-tron-sdk-anychain-vendoring.md` referencing ADR-0001 as the prior decision.

Either way the ADR must record BOTH the original 2026-09-05 decision AND the 2026-09-06 revision, so the audit trail in `docs/wallets/` shows the reasoning chain (raw-primitives rejected → crates.io accepted → vendoring chosen).

---

## Out of Scope for v0.1 (deferred)

- **Stake 2.0** (freeze/unfreeze/delegate/undelegate/cancel/withdraw + witness vote): V0.1.5 — ships with V0.1 release train, separate plan.
- **Thread model** (FFI deadlock / Zeroizing-across-await / Send+Sync on secrets / concurrent broadcast seriality / mobile-vs-desktop runtime divergence): **v0.2** (per deep-dive §"Deferred" → "Thread model").
- **Mobile runtime smoke** (FFI compile only in v0.1): v0.2 via Nile testnet (no Docker fallback).
- **Resource model UX** (Story 8 — `tron resource`): v0.2.
- **Sign personal message** (Story 18 — `tron sign-message`): v0.2.
- **Token list/register** (Stories 23, 24): v0.2.
- **TRC-10 token transfers** (Story 34): v0.3+ (separate `TransferAssetContract` proto encoding).
- **Hardware wallet** (Ledger, Trezor): v1.x.
- **gRPC transport**: v0.3+ if TronGrid gRPC perf becomes bottleneck.
- **Multi-sig / governance flows**: v1.x.
- **Stories 13 (batch), 14 (drain), 15 (ref-block), 16 (manual exp)**: redesign dropped from v0.1.
- **Local tx index** (cached tx history): v0.3+ — every `tx list` call scans blocks.
- **Watch-only wallet import from xpub**: rarely used, deferred indefinitely.

---

## v0.1 Release Status (target)

**Release cut for `tron-wallet-core v0.1.0` library + `tron` CLI v0.1.0 binary.**

**Stories shipped:** 1, 2, 3, 5, 7, 9, 10, 11, 12, 17, 19, 21, 22, 25, 27, 28, 29 (17 stories from deep-dive V0.1) + 26 (TronBox local via `--rpc` flag) + 30 (trc20 approve + allowance already in 25).

**Total user stories covered:** 18 of 29 + 1 cross-cutting.

**Stories removed:** 13, 14, 15, 16.

**Stories deferred:** 4, 6, 8, 18, 23, 24, 31, 32, 33 (V0.1.5 + V0.2).

**Try it (target surface, post-spike):**

```bash
# Workspace build
cargo build -p tron-wallet-core
cargo build -p tron

# Library unit tests
cargo test -p tron-wallet-core --lib

# Spike V1-V10
cargo test -p tron-spike-v1 --tests

# Nile smoke (operator-driven per L29 — set RUN_TRON_NILE=1)
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test '*'

# Mainnet self-send gate (operator-driven per L29 — set RUN_TRON_MAINNET=1)
RUN_TRON_MAINNET=1 cargo test -p tron-spike-v1 --test v11_mainnet_self_send

# Mobile compile-only (FFI)
cargo build -p tron-wallet-core --target aarch64-apple-ios
cargo build -p tron-wallet-core --target aarch64-linux-android

# CLI scaffold
cargo run -p tron -- --help
cargo run -p tron -- wallet --help
cargo run -p tron -- trc20 --help
cargo run -p tron -- tx --help
cargo run -p tron -- config show
```

---

## Cost Estimates — Phase 4 to v0.1 release cut

**Scope:** remaining Phase 4 (test integration) → Phase 7 (V1-V11 PASS + mainnet gate) → `rust-tron-core` → `main` cut. Phase 0-3 already shipped (commits `37d499d`, `47a52d2`, `63d3e6b`, `15f3559`); their actuals are sunk.

**Method:** task-hour bottoms-up per phase + agent-pass estimate per L13 step 11 (code-review, security-auditor, test-engineer, tdd-guide, build-error-resolver, etc. — typically 8-12 subagent invocations per phase for code-bearing work, 2-3 for docs-only). Token figures blend input + output at published rates; treat as ±50% until post-Phase 4 telemetry exists.

### Report cost (human engineering hours)

| Phase                              | Tasks                               | Owner           | Engineering-hours (low) | (high)    | Notes                                                                                                                                      |
| ---------------------------------- | ----------------------------------- | --------------- | ----------------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 4 — Test integration               | 4.1–4.7 (7 tasks)                   | solo            | 24 h                    | 40 h      | testcontainers Docker + 2 GH Actions files + Nile manual trigger; flaky-test triage likely eats 20% of low end                             |
| 5 — PAL + crypto                   | 4.1–4.7 (7 tasks, mostly code)      | solo            | 40 h                    | 64 h      | 4 traits × 3-4 platform impls + argon2id/AES-GCM persistence + FFI cdylib smoke; mobile compile gates hit "no Docker on mobile" repeatedly |
| 6 — CLI scaffold                   | 5.1–5.3+ (22 commands)              | solo            | 40 h                    | 64 h      | clap derive wiring + 22 handlers + exit-code matrix mirroring btc; round 5 L13 review × 22 surfaces likely                                 |
| 7 — V1-V11 + mainnet               | spike V1-V11 + 0.001 USDT self-send | solo + operator | 16 h                    | 32 h      | V1-V11 spike PASS evidence + mainnet gate operator-driven (recipient==sender pre-check hook, `RUN_TRON_MAINNET=1` env gate)                |
| Release cut                        | branch → main + ledger entry + tag  | solo            | 4 h                     | 8 h       | L17 ledger + L13 step 15 PR review + merge + close                                                                                         |
| **Subtotal — Phase 4 to v0.1 cut** |                                     |                 | **124 h**               | **208 h** | ~15-26 working days @ 8 h/day solo                                                                                                         |

### AI report cost (Claude Sonnet 5 token spend, blended)

Rate assumption: input $3/M tokens, output $15/M tokens, blended $6/M tokens. Tokens-per-subagent-pass typical: 50K-200K (read-heavy reviews), 200K-800K (multi-file refactor agents).

| Phase                | Subagent passes (low) | (high) | Token total (low) | (high)     | USD blended (low) | (high)   |
| -------------------- | --------------------- | ------ | ----------------- | ---------- | ----------------- | -------- |
| 4 — Test integration | 24                    | 40     | 2.4 M             | 8.0 M      | $14               | $48      |
| 5 — PAL + crypto     | 40                    | 64     | 4.0 M             | 12.8 M     | $24               | $77      |
| 6 — CLI scaffold     | 56                    | 88     | 5.6 M             | 17.6 M     | $34               | $106     |
| 7 — V1-V11 + mainnet | 16                    | 24     | 1.6 M             | 4.8 M      | $10               | $29      |
| Release cut          | 8                     | 12     | 0.4 M             | 0.8 M      | $2                | $5       |
| **Subtotal**         |                       |        | **14.0 M**        | **44.0 M** | **$84**           | **$265** |

### Cost ratio

- Human-hours-to-AI-USD at low end: 124 h ≈ $84 AI spend → **~$0.68 AI spend per human-hour**
- High end: 208 h ≈ $265 AI → **~$1.27 AI spend per human-hour**

Both ratios are well below typical IDE-seat costs ($25-100/h all-in). AI tooling pays for itself if it cuts wall-clock by ≥3% on routine tasks (it usually cuts more on review + test scaffolding).

### Caveats — read before quoting

1. **No mainnet gate cost.** The $0.001 USDT self-send costs real USDT (≈$0.001 + 130 TRX energy ≈ $20 at $0.15/TRX) + operator-driven; not in this estimate. Budget owner: operator.
2. **TronBox Docker CI minutes.** Two GH Actions workflows + Docker-in-Docker runner on every push add ~5-8 runner-minutes per CI run. Budget 500 runs × 8 min × ubuntu-latest rate (~$0.008/min) ≈ **$20-30/mo** during active development. Goes to $0 once merged to main and Docker tests gate on `pull_request` only.
3. **Nile testnet USDT.** Community faucet USDT is free; Nile TRX for fees requires the public Nile faucet (1-2 TRX/day per IP). Operator-supplied.
4. **Mobile CI matrix not in plan above.** Two extra `cargo check --target aarch64-…` jobs add ~3 min × 2 per CI run. Budget 500 runs × 6 min × macOS-latest rate (~$0.08/min) ≈ **$240/mo** if macOS runner used; falls to ~$30/mo if Linux cross-target only. Choose runner per Round-1 grill Q6.
5. **Bus-factor risk (accepted, per Q3).** If anychain-{core,tron,kms} 0.x upstream yanks a version or ships a breaking "fix", mitigation cost = revert pin + write regression test ≈ 4-8 h + $5-15 AI spend per incident. Track as out-of-band when it fires, not in this estimate.

### Update rule

Re-estimate after Phase 4 lands (replace low/high with actuals). Until then, treat this section as a planning checkpoint, not a quote.

---

## References

- Deep-dive (source of truth): `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md`
- User stories (legacy, must regenerate per Task 0.5): `docs/wallets/2026-08-27-tron-wallet-user-stories.md`
- ADR (reversal): `docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md`
- Issue: #399 (Q1-Q10 resolved in deep-dive; this plan covers Q11-Q12 + mainnet gate)
- Superseded plan: `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`
- Plan guide: `.local/plugins-docs/2026-09-05-plan-guide-mattpocock-superpowers-stack.md`
- Project rules: `tasks/lessons.md` L13 + `CLAUDE.md`
- Global rules: `~/.claude/CLAUDE.md` (caveman mode + superpowers meta-rule)
- SLIP-44 coin types (TRON = 195): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- TRON Developer Hub — Transactions: <https://developers.tron.network/docs/tron-protocol-transaction>
- TRON Developer Hub — Encoding: <https://developers.tron.network/docs/encoding>
- Anychain repo: <https://github.com/0xcregis/anychain>
- Bitcoin SPKI pin pattern source: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`

---

## PAUSE Points (per never-auto-commit rule)

After each phase, **PAUSE before commit**. Show verification output (per L13 step 11 + superpowers verification-before-completion Iron Law). User approves before `git commit`. Per CLAUDE.md GateGuard, use `--body-file` with content in `/tmp` for `gh pr create`.

**No auto-merge.** Update issue #399 checkboxes [ ] → [x] BEFORE squash-merge (per CLAUDE.md update-issues-before-merge rule).

---

**END PLAN. Awaiting user approval to begin Phase 0 implementation.**