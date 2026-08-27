---
title: evm-wallet-core + polygon-wallet-core (v0.1) security audit (ship-gate)
tracker: https://github.com/nhitranbtc/blockchain-sdk/issues/418
plan: ../superpowers/plans/2026-08-27-polygon-wallet-core.md
deep-dive: ../wallets/2026-08-27-polygon-rust-sdks-deep-dive.md
date: 2026-08-27
status: open
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening
---

# evm-wallet-core + polygon-wallet-core (v0.1) security audit (ship-gate)

End-to-end audit for issue **#416** (plan) and **#417** (spike implementation). Mirrors the TRON audit
schema (`docs/audit/2026-08-27-tron-wallet-core-security-audit.md`). All findings
derive from the Q1-Q8 resolutions in the plan doc + the spike V1-V10 acceptance
tests in #417. Drift scan: clean (zero findings, see §"Drift scan").

## Drift scan (L13 step 4a)

| Citation | Verified | Method |
|---|---|---|
| `docs/wallets/2026-08-27-polygon-rust-sdks-deep-dive.md` | ✓ EXISTS | `ls` 2026-08-27 |
| `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` (companion) | ✓ EXISTS | `ls` 2026-08-27 |
| `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md` (sibling) | ✓ EXISTS | `ls` 2026-08-27 |
| `docs/wallets/2026-08-27-polygon-wallet-user-stories.md` | ✓ EXISTS | `ls` 2026-08-27 |
| `docs/superpowers/plans/2026-08-23-eth-wallet-core.md` (refactor target) | ✓ EXISTS | `ls` 2026-08-27 |
| `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` (sibling template) | ✓ EXISTS | `ls` 2026-08-27 |
| `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs` (SPKI pin source — Q7) | ✓ EXISTS | `find` 2026-08-27 |
| `rust-wallet-app/spikes/alloy-v1/tests/v6_erc20_anvil.rs` (Anvil Polygon-fork precedent) | ✓ EXISTS | `ls` 2026-08-27 |
| `rust-wallet-app/spikes/tron-v1/RESULT.md` (PASS evidence schema precedent) | ✓ EXISTS | `ls` 2026-08-27 |
| `alloy = "=1.8.3"` (workspace dep) | ✓ Pinned | `grep` `rust-wallet-app/Cargo.toml` 2026-08-27 |
| USDC mainnet `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` | ✓ Verified | Circle + PolygonScan 2026-08-27 |
| USDT mainnet `0xc2132D05D31c914a87C6611C10748AEb04B58e8F` | ✓ Verified | StableRegistry 2026-08-27 |
| DAI mainnet `0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063` | ✓ Verified | StableRegistry 2026-08-27 |
| USDC Amoy `0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582` | ✓ Verified | Circle + PolygonScan 2026-08-27 |
| Chain-id mainnet 137 (`0x89`) | ✓ Verified | `eth_chainId` via `polygon-rpc.com` 2026-08-27 |
| Chain-id Amoy 80002 (`0x13882`) | ✓ Verified | `eth_chainId` via `polygon-amoy.drpc.org` 2026-08-27 |

## Cross-cutting controls

| ID | Control | Severity | Ships in | Ref |
|---|---|---|---|---|
| **C1** | `chain_id` always bound in `TransactionRequest::with_chain_id(137\|80002)` + EIP-712 domain separator `chainId` field | 🔴 critical | Phase 2 Task 3 + Phase 3 Task 5 | Q7, deep-dive §"Crate-by-crate notes → alloy-signer-local" |
| **C2** | SPKI pin reuse from `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` (verified path `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`) — never reimplement | 🔴 critical | Phase 2 Task 3 | Q7, deep-dive §"SPKI pinning (Q7)" |
| **C3** | Mnemonic-at-rest encrypted with Argon2id (m≥64MiB, t≥3, p=4) + AES-256-GCM, **inherited from `eth-wallet-core::WalletManager`** — `polygon-wallet-core` reuses same `WalletManager` (no new crypto code) | 🔴 critical | Phase 1 Task 2 (re-export from evm-wallet-core) | mirror eth #311 + plan §"Phase 1" |
| **C4** | Native USDC vs bridged USDC.e disambiguation — `polygon_wallet_core::disambig::assert_native_usdc()` rejects bridged USDC.e address (Polygon mainnet historical `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`) | 🟠 high | Phase 3 Task 5 | Q3 implicit, deep-dive §"Bridge vs native USDC footgun" |
| **C5** | Gas-estimation cadence per-broadcast (NOT cached) — `provider.estimate_eip1559_fees()` called immediately before broadcast because Polygon's 2-second block time makes cached `baseFee` stale within ~12s | 🟡 medium | Phase 2 Task 3 (per-broadcast helper) | Q5, deep-dive §"EIP-1559 on Polygon — 2-second-block gas dynamics" |
| **C6** | ZERO new direct deps beyond `alloy-chains` (per Option A refactor payoff) — `cargo tree` audit at release verifies only `alloy-chains` added since eth-wallet-core v0.2 | 🟠 high | Phase 1 Task 2 Step 1 | Q1, plan §"Global Constraints" |
| **C7** | Derivation path validation — path must start with `m/44'/60'/`. Reject all other prefixes. Polygon reuses ETH coin type 60, NOT a separate SLIP-44 entry | 🟡 medium | Phase 1 Task 2 | deep-dive §"Derivation path" |
| **C8** | SLIP-44 coin type 60 shared between ETH + Polygon — same mnemonic + same path produces same address on both chains. Wallet warns user at create-time: "Importing this mnemonic into `eth wallet import` produces the same address" | 🔵 low | Phase 1 Task 2 (warning text in `wallet create`) | deep-dive §"Crate-by-crate notes → alloy-signer-local" |
| **C9** | No hardware wallet in v0.1 — defer to v0.2 per Q6 (out of scope) | 🔵 low | n/a (deferred) | Q6 |

## Phase controls

### Phase 0 — Refactor `eth-wallet-core` → `evm-wallet-core` (Task 1)

| ID | Control | Severity | Ships in |
|---|---|---|---|
| **P0-1** | Refactor preserves eth-wallet-core mnemonic → `0x9858EfFD232B4033E47d90003D41EC34EcaEda94` regression test (all-`abandon` mnemonic → EIP-55 address) | 🔴 critical | Task 1 Step 8 |
| **P0-2** | `Network` enum exhaustive — `Ethereum` (chain_id 1) + `Polygon { mainnet: 137, amoy: 80002 }` variants, both with `chain_id() -> u64` accessor | 🟡 medium | Task 1 Step 4 |

### Phase 1 — `polygon-wallet-core` thin wrapper (Task 2)

| ID | Control | Severity | Ships in |
|---|---|---|---|
| **P1-1** | `derivation_cross_check` test (V3) confirms `m/44'/60'/0'/0/0` produces same address on ETH + Polygon (cross-chain identity per C8) | 🔴 critical | Task 2 Step 5 |
| **P1-2** | `polygon-wallet-core/tokens/mainnet.json` + `amoy.json` bundled via `include_str!` (no runtime fetch — Q3) | 🟡 medium | Task 2 Steps 2-3 |
| **P1-3** | USDC contract `0x3c499c...3359` verified at scaffold time (not silently swapped with bridged USDC.e `0x2791Bca1...4174`) | 🟠 high | Task 2 Step 2 + Phase 3 Task 4 (V6 cross-check) |

### Phase 2 — RPC integration (Task 3)

| ID | Control | Severity | Ships in |
|---|---|---|---|
| **P2-1** | `new_http_polygon_mainnet()` + `new_http_polygon_amoy()` constructors wired to correct URLs (no typo swap) | 🟡 medium | Task 3 Steps 1-2 |
| **P2-2** | SPKI pin scheme `pinned://<spki-hex>@polygon-rpc.com` parsed correctly (verify against `#408` URL parser test cases) | 🟡 medium | Task 3 Step 4 + V7 spike |
| **P2-3** | `provider.estimate_eip1559_fees()` returns valid `(max_fee_per_gas, max_priority_fee_per_gas)` tuple (V4 spike) | 🟠 high | Task 3 Step 4 + V4 |

### Phase 3 — Polygon-specific config (Tasks 4-5)

| ID | Control | Severity | Ships in |
|---|---|---|---|
| **P3-1** | `decimals()` on-chain query cross-checks bundled registry value (V6 spike) — refuse if `bundled != on-chain` | 🟠 high | Task 4 Step 4 + V6 |
| **P3-2** | `disambig::assert_native_usdc()` rejects bridged USDC.e (C4 enforcement) | 🟠 high | Task 5 Step 1 |
| **P3-3** | EIP-712 typed-data signing validates `chain_id ∈ {137, 80002}` — reject `--chain-id 1` (ETH) with exit 2 (cross-chain replay protection per C1) | 🔴 critical | Task 5 Step 4 + V10 |

### Phase 4 — `polygon` CLI + smoke + release cut (Tasks 6-9)

| ID | Control | Severity | Ships in |
|---|---|---|---|
| **P4-1** | `polygon wallet send --network mainnet` requires typing `yes` to confirm real-value txn (default abort, exit 1) | 🔴 critical | Task 6 |
| **P4-2** | Amoy smoke (V7/V8) gated behind `RUN_POLYGON_AMOY=1` per L29 (operator-driven, not CI) | 🟡 medium | Task 7 + V7/V8 |
| **P4-3** | Mainnet smoke (V5) gated behind `RUN_POLYGON_MAINNET=1` per L29 (operator-driven only, never auto in CI) | 🟠 high | Task 8 + V5 |
| **P4-4** | `--legacy-token-symbol` flag (off by default) renames "POL" → "MATIC" in CLI output — preserves pre-September-2024 mental model for legacy wallet UX (Q8) | 🔵 low | Task 6 |
| **P4-5** | Release-cut audit: `cargo tree --depth 1` shows only `alloy-chains` as new direct dep since eth-wallet-core v0.2 (C6 enforcement) | 🟠 high | Task 9 |

## Minimum ship-gate checklist (v0.1)

Reverse-engineered from 🔴 critical + 🟠 high findings. Each must be green before squash-merge.

- [ ] **G1 — Refactor regression** (P0-1): `cargo test -p evm-wallet-core --test mnemonic` returns ETH derivation `0x9858...e94` for all-`abandon` mnemonic (eth-wallet-core v0.2 surface preserved)
- [ ] **G2 — Cross-chain identity** (P1-1 + C8): `cargo test -p polygon-v1-spike --test v3_derivation` PASS (V3 spike from #417)
- [ ] **G3 — chain_id replay protection** (C1 + P3-3): `cargo test -p polygon-v1-spike --test v10_eip712_replay` PASS (V10 spike)
- [ ] **G4 — SPKI pin reuse** (C2): `polygon --rpc-url pinned://<spki>@polygon-rpc.com` succeeds; wrong pin returns `Error::SpkiPinMismatch` (V7 spike)
- [ ] **G5 — Native USDC vs USDC.e** (C4 + P3-2): `disambig::assert_native_usdc(0x3c499c...3359)` succeeds; rejects bridged USDC.e address
- [ ] **G6 — Gas-estimation cadence** (C5): `cargo test -p polygon-v1-spike --test v4_eip1559_estimates` PASS (V4 spike, `RUN_POLYGON_AMOY=1` gated)
- [ ] **G7 — Mnemonic-at-rest** (C3): Wallet `WalletManager` inherits Argon2id + AES-256-GCM from eth-wallet-core v0.2 — no plaintext mnemonic on disk
- [ ] **G8 — Mainnet smoke** (P4-3): `cargo test -p polygon-v1-spike --test v5_rpc_connectivity` PASS (V5, `RUN_POLYGON_MAINNET=1` operator-driven per L29)
- [ ] **G9 — Amoy smoke** (P4-2): V2/V4/V6/V7/V8 PASS under `RUN_POLYGON_AMOY=1`
- [ ] **G10 — Anvil Polygon-fork ERC-20 transfer** (P3-1 + V9): `cargo test -p polygon-v1-spike --test v9_erc20_transfer` PASS against Anvil Polygon-fork
- [ ] **G11 — Zero new deps** (C6): `cargo tree --depth 1 -p polygon-wallet-core` shows ONLY `alloy-chains` as net-new direct dep vs eth-wallet-core v0.2
- [ ] **G12 — Mainnet confirmation** (P4-1): `polygon wallet send --network mainnet` requires literal `yes` input (default abort, exit 1)

## Out of scope (deferred per plan)

- Polygon zkEVM (chain-id 1101) — v0.2
- Mumbai testnet — deprecated
- Bridged `USDC.e` — footgun, only native Circle USDC supported
- Hardware wallet (Ledger/Trezor) via `alloy-signer-ledger` / `alloy-signer-trezor` — v0.2 (Q6)
- Smart-contract deployment via wallet — sign-only + broadcast external path
- L2 DEX integration (QuickSwap swaps, Uniswap-v3-Polygon swaps)
- Polygon staking delegation
- Flashbots / MEV protection on Polygon private RPCs
- ENS resolution (`alice.eth`)
- EIP-4337 account abstraction
- Full EIP-712 domain separation + complex nested types — v0.3

## References

- Issue tracker: #416 (this audit's parent plan) — https://github.com/nhitranbtc/blockchain-sdk/issues/416
- Spike implementation: #417 — https://github.com/nhitranbtc/blockchain-sdk/issues/417
- Plan doc: `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` (committed at `d246432`, enriched at `663a81e`)
- Deep-dive: `docs/wallets/2026-08-27-polygon-rust-sdks-deep-dive.md` (committed at `a54dbb3`)
- User-stories: `docs/wallets/2026-08-27-polygon-wallet-user-stories.md` (committed at `8bf7837`)
- TRON audit (precedent): `docs/audit/2026-08-27-tron-wallet-core-security-audit.md`
- ETH plan (refactor target): `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`
- SPKI pin source: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`
- Alloys chain metadata: `alloy-chains::Chain::Polygon`, `Chain::PolygonAmoy`
- TRON spike PASS schema: `rust-wallet-app/spikes/tron-v1/RESULT.md`
- ETH deep-dive (async test policy): `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` §"Appendix: Async test function priority"