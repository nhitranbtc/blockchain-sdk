# TRON Wallet Core — Roadmap Report

**Project:** `tron-wallet-core v0.1.0` library + `tron` CLI scaffold
**Issue:** [#399](https://github.com/nhitranbtc/blockchain-sdk/issues/399)
**Plan:** [`docs/superpowers/plans/2026-08-27-tron-wallet-core.md`](../superpowers/plans/2026-08-27-tron-wallet-core.md) (commit `7736233`)
**Deep-dive:** [`docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md`](../wallets/2026-08-27-tron-rust-sdks-deep-dive.md) (PR [#402](https://github.com/nhitranbtc/blockchain-sdk/pull/402), commit `7e303c0`)
**User-stories:** [`docs/wallets/2026-08-27-tron-wallet-user-stories.md`](../wallets/2026-08-27-tron-wallet-user-stories.md) (29 stories)
**Last updated:** 2026-08-27

## Status legend

- ✅ **done** — shipped, verified, in branch
- � **in-progress** — work started this session / currently active
- ⏳ **pending** — planned, not started
- 🔒 **blocked** — depends on another task (typically a spike Vn)
- ❌ **deferred** — explicitly out of scope for v0.1 (issue #399 body)

## Top-level summary

| Phase | Tasks done / total | Stories done / total | Status |
|---|---|---|---|
| **Phase 0** — Scaffold + canonical test | 0 / 1 | 0 / 1 | ⏳ pending |
| **Phase 1** — Wallet ops + sign + error | 0 / 3 | 0 / 8 | � pending |
| **Phase 2** — RPC + protobuf tx | 0 / 4 | 0 / 14 | ⏳ pending |
| **Phase 3** — TRC-20 + resource + tokens + e2e | 0 / 4 | 0 / 6 | ⏳ pending |
| **Phase 4** — CLI + smoke + release | 0 / 3 | 0 / 29 (all surfaced) | � pending |
| **Spike V1–V10** | 0 / 10 | n/a | ⏳ pending |

**Issue #399 deliverables:**

- [x] Deep-dive doc committed (PR #402, commit `7e303c0`)
- [x] Plan committed (`7736233`)
- [ ] User-stories doc committed (pre-existing draft untracked on `docs/tron-wallet-core-399`)
- [ ] Spike (`rust-wallet-app/spikes/tron-v1/`) — V1–V10 PASS evidence
- [x] All 10 Qs answered or deferred — **10/10 answered** in deep-dive (`7736233` extended the verification; Q5 fully resolved)
- [ ] Issue body checkboxes flipped to `[x]`

## Phase 0 — Scaffold + canonical mnemonic → T-address test

**Goal:** Wire up workspace deps + vendor `core/Tron.proto` + produce canonical `m/44'/195'/0'/0/0` → T-base58check round-trip test.

| Task | Title | Status | Story | Spike dep |
|---|---|---|---|---|
| Task 1 | Crate scaffold + V10 mnemonic → T-address test | ⏳ pending | Story 1 (Create wallet) — partial | V1, V4, V10 |

**Phase 0 stories (1):**

- Story 1 — Create a new wallet (Alice) ⏳ blocked on V10 + V4

**Phase 0 cross-refs:**

- Workspace dep add: `prost = "0.14.4"`, `prost-types = "0.14.4"`, `bs58 = "0.5"`, `tiny-keccak = "2.0.2"`
- Build dep: `prost-build = "0.14.4"`, system `protoc ≥3.12`
- Vendored: `proto/core/Tron.proto` from `tronprotocol/java-tron` SHA `851575d` (2026-07-14)

---

## Phase 1 — Core wallet ops + sign-only + error

**Goal:** Wallet CRUD (create / import / list / delete / show) + sign-only path with TRON signature convention (`r‖s‖v`, `v ∈ {0, 1}`) + error enum + zeroize wrap.

| Task | Title | Status | Stories | Spike dep |
|---|---|---|---|---|
| Task 2 | WalletManager + create/import/list/delete | ⏳ pending | 1, 2, 9, 12 | V10, V4 |
| Task 3 | Sign-only path + signature convention test | ⏳ pending | 5, 18 | V2, V8, V10 |
| Task 4 | Error enum + zeroize wrap (F47 mirror) | ⏳ pending | cross-cutting (zeroize hygiene) | V1 |

**Phase 1 stories (8):**

- Story 1 — Create a new wallet ⏳ blocked on V10 + V4
- Story 2 — Import an existing wallet ⏳ blocked on V10 + V4
- Story 9 — List / show / delete / rename wallets ⏳ **can ship in CLI scaffold without spike** (uses `std::fs` only, per user-stories doc §"Stories that can ship in CLI scaffold without spike")
- Story 12 — Persist wallet across CLI invocations ❌ deferred to v0.2 (per #399 B3, mirrors eth v0.3 Story 12)
- Story 5 — Send native TRX ⏳ blocked on V2 + V8 + V6 (tx construction + sign + broadcast)
- Story 18 — Sign personal message (raw) ⏳ blocked on V10 + V4
- Story 19 — Export xpub + first addresses ⏳ blocked on V10 + V4
- Story 20 — Pick derivation path ⏳ blocked on V10

**Phase 1 cross-refs:**

- Signature convention hazard (Q2 + Q8): `r‖s‖v` with `v ∈ {0, 1}` — NOT Ethereum `v+27 ∈ {27, 28}`. Test `sign_only.rs` asserts `signature[64] == 0 || signature[64] == 1` at build time.
- Zeroize (F47): `Zeroizing<Mnemonic>` + `Zeroizing<SigningKey>` — mirrors Bitcoin Task 30 + eth Task 4.

---

## Phase 2 — RPC integration + protobuf tx

**Goal:** Raw `reqwest` JSON-RPC + protobuf tx construction via `prost 0.14.4` + SPKI pin verifier reuse + RPC method coverage.

| Task | Title | Status | Stories | Spike dep |
|---|---|---|---|---|
| Task 5 | Vendored protobuf schema + generated types | ⏳ pending | 5, 6, 13, 14, 15, 16, 17, 21, 25 (tx construction) | V2 |
| Task 6 | Raw reqwest JSON-RPC client + SPKI pin verifier (Q7) | ⏳ pending | 3, 4, 7, 26, 27, 28, 29 | V6, V7 |
| Task 7 | RPC methods — `getnowblock`, `getchainid`, `createaccount`, `getaccount`, `getblockbynum` | ⏳ pending | 3, 4, 7, 15 | V6 |
| Task 8 | Transaction builder — TransferContract + TriggerSmartContract (Q2) | ⏳ pending | 5, 6, 13, 14, 16, 17, 21, 25 | V2 |

**Phase 2 stories (14):**

- Story 3 — Check TRX balance ⏳ blocked on V1 + V6
- Story 4 — Sync chain state ⏳ blocked on V6
- Story 7 — Inspect transaction history � blocked on V1 + V6
- Story 26 — Use TronBox local node for testing ⏳ blocked on V1 + TronBox availability
- Story 27 — Use Nile testnet ⏳ blocked on V6 (Nile chain-id `0xcd8690dc`)
- Story 28 — Connect to RPC endpoint with SPKI pin ⏳ blocked on V7
- Story 29 — Connect to RPC endpoint without SPKI pin ⏳ **can ship in CLI scaffold without spike** (V1 only)
- Story 5 — Send native TRX ⏳ blocked on V2 + V8 + V6
- Story 6 — Send with custom `fee_limit` ⏳ blocked on V2 + V5
- Story 13 — Send to multiple recipients (sequential txs) ⏳ blocked on V2 + V8
- Story 14 — Sweep / drain wallet ⏳ blocked on V1 + V2
- Story 15 — Choose `ref_block` strategy (auto vs manual) ⏳ blocked on V2 + V6
- Story 16 — Manual `expiration` + `fee_limit` override ⏳ blocked on V2
- Story 17 — Replace / speed-up tx ⏳ blocked on V2 + V6 + V8

**Phase 2 cross-refs:**

- RPC: `walletsolidity/getnowblock` (finality) for TAPOS — NOT `wallet/getnowblock`
- Chain-id: `eth_chainId` JSON-RPC via `/jsonrpc` — NOT `wallet/getchainid` (HTTP 405 on TronGrid)
- TriggerSmartContract `data` is field **4** (NOT 3) — off-by-one hazard flagged in deep-dive
- SPKI pin: reuse `bitcoin_wallet_core::chain::spki::SpkiPinnedVerifier` directly (path verified 2026-08-27)

---

## Phase 3 — TRC-20 stablecoin + resource model + token registry + e2e send

**Goal:** Hand-rolled TRC-20 ABI encoder + energy/bandwidth estimation + DEM awareness + bundled token registry + end-to-end `send_trc20_transfer` with Nile smoke.

| Task | Title | Status | Stories | Spike dep |
|---|---|---|---|---|
| Task 9 | TRC-20 ABI encoder (Q3 hand-rolled) | ⏳ pending | 21, 22, 24, 25 | V3 |
| Task 10 | Resource model — energy + fee_limit + DEM (Q5) | ⏳ pending | 6, 8, 21 | V5 |
| Task 11 | Token registry loader + USDT/USDC decimals (Q9) | ⏳ pending | 23, 24 | V9 |
| Task 12 | Sign + broadcast TRC-20 transfer (end-to-end) | ⏳ pending | 21, 22 | V2, V3, V5, V9 |

**Phase 3 stories (6):**

- Story 8 — Get current energy/bandwidth estimates � blocked on V5
- Story 21 — Send TRC-20 stablecoin (USDT-TRC20) ⏳ blocked on V2 + V3 + V9
- Story 22 — Check TRC-20 token balance ⏳ blocked on V3 + V9
- Story 23 — List registered TRC-20 stablecoins ⏳ blocked on V9
- Story 24 — Add custom TRC-20 token by contract address ⏳ blocked on V3 + V9
- Story 25 — Approve TRC-20 spending (for DEX) ⏳ blocked on V2 + V3

**Phase 3 cross-refs:**

- TRC-20 ABI = ERC-20 ABI at wire level — selectors `0xa9059cbb` (transfer), `0x70a08231` (balanceOf), `0x313ce567` (decimals)
- Stake 2.0 (April 2023, proposal #84 / TIP-467): 1 TRX = 1 TP, each stake picks **either Energy OR Bandwidth** (not both), 14-day unstake pending
- `fee_limit` in **SUN, not TRX** — footgun flagged; max 15,000,000,000 sun (`getMaxFeeLimit` #47)
- DEM penalty: `max_factor = 3.4` per 6-hour cycle — buffer `fee_limit` by `max_factor * 1.1`
- Token registry: `tokens/mainnet.json` (5 entries: USDT, USDC, TUSD, USDD, stUSDT) + `tokens/nile.json` (1 entry)
- USDT-TRC20 `transfer`: ~65k Energy if recipient holds USDT, ~130k if empty (per deep-dive §"Resource model — verified 2026 numbers")

---

## Phase 4 — `tron` CLI + smoke + release cut

**Goal:** `tron` binary with clap subcommands + Nile smoke + mainnet smoke + v0.1.0 release cut + CHANGELOG.

| Task | Title | Status | Stories surfaced | Spike dep |
|---|---|---|---|---|
| Task 13 | `tron` CLI scaffold + wallet commands | ⏳ pending | 9, 11, 29 (no-spike shippable subset) | V1 |
| Task 14 | `tron send` subcommand + Nile smoke | ⏳ pending | 5, 6, 10, 13, 14, 17, 21, 22 | V2, V3, V5, V6, V8, V9 |
| Task 15 | Mainnet smoke + release cut | ⏳ pending | 10, 26, 27, 28 (mainnet + TronBox + SPKI) | All Vn |

**Phase 4 stories (all 29 surfaced via CLI):**

- Stories 1, 2 — `tron wallet create` / `tron wallet import`
- Story 3 — `tron balance --wallet w`
- Story 4 — `tron sync`
- Stories 5, 6 — `tron send --token native` / `tron send --fee-limit X`
- Story 7 — `tron tx-list`
- Story 8 — `tron resources --wallet w`
- Story 9 — `tron wallet list` / `show` / `delete` / `rename`
- Story 10 — `--network mainnet|nile|shasta` flag
- Story 11 — `tron config show`
- Story 12 — ❌ deferred to v0.2 (persistence)
- Story 13 — `--to T1,T2,T3,...` (sequential txs)
- Story 14 — `tron send --sweep`
- Story 15 — `tron send --ref-block-strategy auto|manual`
- Story 16 — `tron send --expiration-ms X --fee-limit-sun Y`
- Story 17 — `tron send --replace --nonce N --fee-limit-sun Y2`
- Story 18 — `tron sign-message --wallet w --msg ...`
- Story 19 — `tron export-xpub --wallet w`
- Story 20 — `--derivation-path m/44'/195'/0'/N` flag
- Story 21 — `tron send --token USDT --amount 1.5`
- Story 22 — `tron balance --wallet w --token USDT`
- Story 23 — `tron tokens list`
- Story 24 — `tron tokens add --contract T... --symbol USDX --decimals 6`
- Story 25 — `tron approve --token USDT --spender T... --amount X`
- Story 26 — `--network tronbox --rpc-url http://localhost:8090`
- Story 27 — `--network nile`
- Story 28 — `pinned://<hex>@api.trongrid.io`
- Story 29 — default (no pin)

**Phase 4 cross-refs:**

- CLI parity with `btc` (Bitcoin v0.1) + `eth` (eth v0.2): same `clap` derive macros, same exit code policy (#399 M-EXIT, mirrors eth #297 M11)
- `trongrid-api-key` flag raises rate limit 3 QPS → 15 QPS (corrected 2026-08-27)
- TronGrid free tier: 15 QPS authenticated, 100K req/day cap
- Release cut: bump `tron-wallet-core` + `tron` to `0.1.0`, tag `tron-wallet-core-v0.1.0` + `tron-cli-v0.1.0`, author `CHANGELOG.md`

---

## Spike V1–V10 tracking

The verification harness at `rust-wallet-app/spikes/tron-v1/` produces PASS evidence for each Vn before the corresponding phase ships.

| V# | Q | What it verifies | Status | Phases unblocked |
|---|---|---|---|---|
| V1 | Q1 | `cargo add prost@0.14 prost-types@0.14 bs58@0.5 tiny-keccak@2.0.2` compiles | ⏳ pending | Phase 0 (Task 1), Phase 2 (Tasks 6/7/8) |
| V2 | Q2 | `prost-build` compiles `core/Tron.proto` SHA `851575d` + TriggerSmartContract `data` field 4 round-trip | ⏳ pending | Phase 1 (Task 3), Phase 2 (Task 8), Phase 3 (Task 12) |
| V3 | Q3 | Hand-rolled `encode_transfer` 68-byte calldata with `0xa9059cbb` at bytes 0..4; round-trips against `alloy-sol-types` standalone | ⏳ pending | Phase 3 (Tasks 9/12) |
| V4 | Q4 | `base58check_encode([0x41] ++ last_20_bytes_of_keccak256(pubkey))` → 34-char `T...` string; decode round-trips | ⏳ pending | Phase 0 (Task 1), Phase 1 (Tasks 2/3) |
| V5 | Q5 | `triggerconstantcontract` returns `energy_used` 65k–130k for USDT-TRC20; DEM factor round-trip; `fee_limit` in SUN | ⏳ pending | Phase 3 (Tasks 10/12) |
| V6 | Q6 | `POST /jsonrpc eth_chainId` → `0xcd8690dc` on Nile; `0x41` prefix universal; `walletsolidity/getnowblock` for TAPOS | ⏳ pending | Phase 2 (Tasks 6/7), Phase 4 (Task 14) |
| V7 | Q7 | `SpkiPinnedVerifier` (from `bitcoin-wallet-core::chain::spki`) accepts `pinned://<correct_pin>@api.trongrid.io`, rejects wrong pin | ⏳ pending | Phase 2 (Task 6) |
| V8 | Q8 | Local-sign TRX; `txID = SHA256(raw_data_hex)` matches network; signature is `r‖s‖v` with `v ∈ {0, 1}` | ⏳ pending | Phase 1 (Task 3), Phase 4 (Task 14) |
| V9 | Q9 | `tokens/mainnet.json` 5 entries load; USDT decimals = 6 via `triggerconstantcontract`; `energy_penalty` field present | ⏳ pending | Phase 3 (Task 11) |
| V10 | Q10 | `bip_utils::slip44::Coin::Tron` (195) → mnemonic → seed → `m/44'/195'/0'/0/0` → T-address matches TronWeb | ⏳ pending | Phase 0 (Task 1), Phase 1 (Tasks 2/3) |

**V1–V10 cumulative: 0/10 PASS.**

---

## Cross-cutting acceptance (apply to all phases)

- `--json` everywhere — `serde_json` in CLI scaffold args
- Stable exit codes per issue #399 M-EXIT (mirror eth #297 M11)
- `Secret<Mnemonic>` zeroize — `zeroize::Zeroizing<Mnemonic>` (F47 mirror, Bitcoin Task 30 + eth Task 4)
- T-base58check address display — `bs58` + 4-byte double-SHA-256 checksum (hand-roll checksum verify)
- `#[tokio::test]` for all async test functions (per `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` §"Appendix: Async test function priority", issue #333)
- `cargo fmt` + `cargo clippy --all-targets -- -D warnings` + `cargo test` on every commit
- `cargo geiger` enforces zero `unsafe` in user code (mirrors Bitcoin plan)
- No `unwrap` / `panic!` in library code (F43 mirror)
- All public functions return `Result<T, Error>`

---

## Deferred (per issue #399 body)

❌ TRC-10 token transfers (separate `TransferAssetContract` proto encoding) — covered by `chain-traits` umbrella v0.2+
❌ Smart-contract deployment via wallet (sign-only + broadcast external path is enough for v0.1)
❌ Stake/unstake/freeze resource delegation (`FreezeBalanceV2Contract` proto encoding) — v0.2+
❌ Multi-sig / governance flows — v0.2+
❌ TRON-specific DEX integration (SunSwap, etc.) — out of scope
❌ Hardware wallet support (Ledger/Trezor) — same deferral as eth #293
❌ L2s / EVM-compatible sidechains — same deferral as eth #293

---

## Update log

| Date | Change | Commit |
|---|---|---|
| 2026-08-27 | Deep-dive verified + extended (10 wrong claims corrected, Q5 resolved, 60+ sources added) | `7e303c0` |
| 2026-08-27 | Plan committed (Phase 0–4, 15 tasks, spike V1–V10 mapping, Rust SDKs/tools/crates inventory) | `7736233` |
| 2026-08-27 | Roadmap report created (this file) | (pending) |
