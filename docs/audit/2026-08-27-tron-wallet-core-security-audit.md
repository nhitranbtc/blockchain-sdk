---
title: TRON wallet-core v0.1 security audit (ship-gate)
tracker: https://github.com/nhitranbtc/blockchain-sdk/issues/407
plan: docs/superpowers/plans/2026-08-27-tron-wallet-core.md
deep-dive: docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md
date: 2026-08-27
status: open
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening
---

# TRON wallet-core v0.1 — security audit (ship-gate)

Companion to issue **#407**. Phase-by-phase threat catalog + minimum ship-gate checklist. This file is the input the implementer must clear before tagging `tron-wallet-core-v0.1.0`.

## Drift scan (L13 step 4a)

| Cited SHA / path | Status | Action |
|---|---|---|
| `core/Tron.proto` commit `851575d` (2026-07-14) | **DRIFT** — current file SHA = `6982f5177850db3c5048005f8f222827d925052fbb8c93b0d041b20ed774aed3` | 🟠 Re-pin to actual SHA, OR add `build.rs` SHA assertion (see §C1) **before** Phase 2 ships |
| `bitcoin-wallet-core/src/chain/spki.rs` | ✅ exists at `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs` | none |
| `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md` | ✅ exists | none |
| `Discover.proto` (sibling to `Tron.proto`, also vendored) | **DRIFT** — also un-pinned | 🟡 Apply same SHA-pin treatment |

> **Drift finding matches audit cross-cutting #6**: vendored `core/Tron.proto` SHA not enforced. **Fix lives in §C1.**

---

## Cross-cutting controls (apply to every phase)

| ID | Control | Severity | Ships in |
|---|---|---|---|
| **C1** | Vendored `core/Tron.proto` SHA enforced in `build.rs` (`assert_eq!(sha256_of_file, EXPECTED_SHA)`); re-pin if drift detected | 🟠 | Phase 0 Step 4 |
| **C2** | `bip39` feature pinned to `["zeroize", "rand"]`; `cargo-deny` bans `bip39/rng` feature | 🟡 | Phase 0 Step 2 |
| **C3** | `WalletConfig::default()` MUST NOT have a `default_rpc_url`; require `Some(_)` at construction | 🟡 | Phase 1 Task 2 |
| **C4** | Compile-fail test: `k256::ecdsa::Signature::from_sliced_64(...)` output is rejected by an eth-default `v+27` decoder; asserts our signer never produces eth-style sig | 🟠 | Phase 1 Task 3 |
| **C5** | `Network::Mainnet` / configured network must match the RPC `eth_chainId`; refuse to sign on network mismatch (replay protection, see cross-cutting #2). TRON signatures have no EIP-155-style `chain_id`; residual protection is limited to the TAPOS reference block plus the approximately 60-second expiration window, so replay remains possible inside that window. | 🔴 | Phase 1 Task 3 |
| **C6** | Zeroize-on-drop wrapper for `SigningKey`; `to_bytes()` only callable inside `signing.rs` | 🟠 | Phase 1 Task 3 |
| **C7** | SPKI pin table (`pinned_endpoints.json`) ships in repo for `api.trongrid.io`, `nile.trongrid.io`, `api.shasta.trongrid.io`; CLI `--print-pinned-hosts` lists them. Production pin enforcement is fail-closed when `TRON_REQUIRE_PIN=1` (or `true`): unpinned RPC URLs are rejected rather than silently downgraded; test/development-only unpinned paths remain `#[cfg(test)]`. | 🟠 | Phase 2 Task 6 |
| **C8** | `new_http()` (non-pinned) gated `#[cfg(test)]`; production builds can only reach `new_http_pinned()`. In production, `TRON_REQUIRE_PIN=1` must be present and the endpoint must have a valid pin; an absent pin is an error, never an implicit trust fallback. | 🔴 | Phase 2 Task 6 |
| **C9** | `SECURITY.md` + `cargo-cyclonedx` SBOM + `cargo audit` report committed at release | 🟠 | Phase 4 Task 15 |

---

## Phase 0.0 — Network selection + local-dev testnet

| ID | Control | Severity |
|---|---|---|
| **P00-1** | Pin `tronbox/tre` Docker image by `@sha256:...` digest, not `:latest` tag; verify against TronGrid-published checksums | 🟠 |
| **P00-2** | CI job on protected branches runs gated tests with `RUN_TRON_LOCAL=1` | 🟡 |
| **P00-3** | Nile faucet UX: CLI QR-displays address + requires typed confirmation; never accept clipboard-only | 🟡 |

## Phase 0 — Crate scaffold + V10 address derivation

| ID | Control | Severity |
|---|---|---|
| **P0-1** | Negative test: `assert_ne!(keccak256(...), sha3_256(...))` for canonical `bip39 abandon ×11` pubkey (wire-format hazard) | 🔴 |
| **P0-2** | Property test ≥10k rounds on `(addr_bytes) → base58check_encode → decode → bytes` | 🟠 |
| **P0-3** | `derive_address` returns `Zeroizing<[u8;21]>`; `Drop` zeroes | 🟡 |

## Phase 1 — Core wallet ops

| ID | Control | Severity |
|---|---|---|
| **P1-1** | **Mnemonic-at-rest encrypted with Argon2id (m≥64 MiB, t≥3, p=4) + AES-256-GCM.** Block v0.1 ship without it. | 🔴 |
| **P1-2** | `create_wallet(words, password)` — drop the `password` param OR return `Err(Error::PasswordUnsupportedInV01)` (API contract honesty) | 🟠 |
| **P1-3** | BIP-39 passphrase length ∈ {0, 1..8} rejected with `Error::WeakPassphrase` | 🟡 |
| **P1-4** | Error enum adds `ExpiredTransaction`, `NonceReuse`, `InsufficientEnergy`, `InsufficientBandwidth`; reject `String`-payload variants in PR review | 🟡 |
| **P1-5** | `list_wallets()` defaults to address-only; `--metadata` opt-in flag | 🟡 |

## Phase 2 — RPC + protobuf tx

| ID | Control | Severity |
|---|---|---|
| **P2-1** | `pinned://<hex>@host` parser — 12 unit-test cases (odd-length, non-hex, empty, missing `@`, multi `@`, mixed case, whitespace, NUL, etc.) | 🟠 |
| **P2-2** | `eth_chainId` parser strict: require `0x` prefix + lowercase + 8 hex chars; reject anything else | 🟠 |
| **P2-3** | `walletsolidity/getnowblock` + `wallet/getnowblock` fallback with `ref_block_bytes/hash` cross-validate | 🟡 |
| **P2-4** | `broadcast_transaction` defaults `visible: false`; opt-in `visible: true` only for debug | 🟡 |
| **P2-5** | `get_account` uses `serde_json` + `deserialize_with`; property test against canonical TronGrid response | 🟡 |
| **P2-6** | `build_trx_transfer` rejects `to_21 == [0u8; 21]` and `0x41 ++ [0u8; 20]` (burn address) | 🟠 |
| **P2-7** | `build_trx_transfer` asserts `fee_limit == 0` AND `contract == TransferContract`; `TriggerSmartContract` asserts `fee_limit > 0` | 🟡 |
| **P2-8** | `expiration = node_timestamp + 60s`, NOT local `Instant::now()`; ±skew tolerance | 🟠 |
| **P2-9** | End-to-end protobuf fixture: commit hex dump of a real TRC-20 transfer; `cargo test` asserts round-trip | 🟠 |

## Phase 3 — TRC-20 + resource model

| ID | Control | Severity |
|---|---|---|
| **P3-1** | ABI encoder byte-equals `alloy_sol_types::sol!` for 100 random `(addr, value)` pairs (round-trip property test) | 🔴 |
| **P3-2** | Decimals resolution **always** cross-checked against live chain before sign; refuse if `bundled != on-chain` | 🔴 |
| **P3-3** | `$TRON_TOKEN_REGISTRY` env var + `--token-registry <path>` flag to override compile-time bundle | 🟡 |
| **P3-4** | Post-build check: if `size_fee_limit < energy_used * sun_per_energy`, log warning + require `--yes-i-know` | 🟡 |
| **P3-5** | Fetch DEM factor every call; reject hardcoded `3.4` (mainnet-only) | 🟠 |
| **P3-6** | CLI prints `fee_limit = N SUN (X TRX)`; refuse user-supplied `fee_limit < 0.5 × computed` | 🟡 |
| **P3-7** | v0.1 ships read-only stake view via `getaccount`; refuse stake mutation (`freezeBalanceV2` deferred to v0.2) | 🟡 |
| **P3-8** | Each `tokens/*.json` entry has `provenance: {issue, pr, commit}` field at top of file | 🟡 |
| **P3-9** | Idempotency: refuse double-sign in same 60s window (sender nonce + ref_block) | 🟠 |
| **P3-10** | `BANDWIDTH_INSUFFICIENT` / `OUT_OF_ENERGY` returned as structured error; **no auto-retry** | 🟠 |

## Phase 4 — CLI + smoke + release

| ID | Control | Severity |
|---|---|---|
| **P4-1** | `--trongrid-api-key-file <path>` or env `TRON_PRO_API_KEY` only; **never** as CLI flag | 🔴 |
| **P4-2** | `tron wallet list --redact` flag (default off in v0.1, default on in v0.2) | 🟡 |
| **P4-3** | `tron send --dry-run` does **not** log signed tx bytes; only shows txID + ready-to-broadcast prompt | 🟠 |
| **P4-4** | Mainnet smoke only via `<env flag>` opt-in; default CI smoke on `nile.trongrid.io` | 🟡 |
| **P4-5** | Release workflow generates SBOM (`cargo-cyclonedx`) + commits `cargo audit` JSON to `SECURITY.md` | 🟠 |
| **P4-6** | `SECURITY.md` with `security@…` contact + 90-day disclosure window | 🟡 |
| **P4-7** | `--debug` output goes to stderr-only; refuse `--debug --broadcast` combination | 🟡 |

---

## Minimum ship-gate checklist (v0.1 release acceptance)

All of the following MUST pass before tagging `tron-wallet-core-v0.1.0` / `tron-cli-v0.1.0`:

- [ ] **C1** proto SHA enforcement
- [ ] **C5** chain-id replay guard
- [ ] **C8** `new_http()` test-only
- [ ] **P1-1** Argon2id + AES-256-GCM mnemonic-at-rest
- [ ] **P2-2** strict `eth_chainId` parser
- [ ] **P3-1** ABI ↔ `alloy_sol_types::sol!` property test
- [ ] **P3-2** decimals cross-check at sign time
- [ ] **P4-1** API key never as CLI flag
- [ ] **P4-5** SBOM + cargo audit committed
- [ ] `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` green
- [ ] `cargo-deny` clean (bans `bip39/rng`, license, advisory)
- [ ] Spike V1–V10 PASS evidence recorded in `rust-wallet-app/spikes/tron-v1/RESULT.md` (per #399 acceptance)

---

## Out of scope (deferred per #399)

TRC-10, contract deployment, stake/unstake, multisig, DEX, hardware wallet, EVM sidechains. Each needs its own audit when added.

## References

- Issue tracker: [#407](https://github.com/nhitranbtc/blockchain-sdk/issues/407)
- Plan: [docs/superpowers/plans/2026-08-27-tron-wallet-core.md](../superpowers/plans/2026-08-27-tron-wallet-core.md)
- Deep-dive: [docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md](../wallets/2026-08-27-tron-rust-sdks-deep-dive.md)
- PR #402 (deep-dive), #403 (spike), #405 (spike merged)
- Bitcoin SPKI pin source: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`
- eth Q2/Q4 reference: `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`