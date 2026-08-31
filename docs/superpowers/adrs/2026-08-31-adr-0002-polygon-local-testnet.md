# ADR 0002: Polygon local-testnet strategy — Tier 1 Anvil hardfork, Tier 2 Testcontainers bor

> **Status:** Proposed (2026-08-31)
> **Deciders:** Nhi Tran
> **Issue:** [#492](https://github.com/nhitranbtc/blockchain-sdk/issues/492)
> **Blocks:** polygon-v1 spike V9/use_case (already shipped in PR #485 via Tier 1); future Bor-specific opcode coverage (Tier 2)
> **Supersedes:** None
> **Related:** #416 (plan), #419 (V9/use_case), PR #485, #474 (RPC drift), `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §0 + Phase 0.0, `rust-wallet-app/spikes/polygon-v1/` (verification harness V1-V10)

## Context

Issue #492 asks: which local-testnet approach for Polygon — Anvil hardfork, Testcontainers (`polygon-edge` / `geth-bor`), or alternative. The decision affects:

- **Spike use-case coverage** (`polygon-v1` V1-V10) — every verification scenario needs a local Polygon state
- **CI shape** (L29 operator-driven `#[ignore]` + env opt-in vs CI-gated)
- **Bor-specific opcode coverage** for future scenarios (validator-set changes, state-sync precompile, full EIP-1559 edge cases)
- **PR #485** already shipped V9/use_case `balanceOf` round-trip via Anvil Polygon hardfork — empirically proven path for the simplest scenario class

**Open question taxonomy (from #492 body):**
1. Chain ID — 137 (mainnet) vs 80002 (Amoy) for local fork? Both? Matrix?
2. Anvil Polygon hardfork support — works today? Bor opcode gaps?
3. Testcontainers availability — `polygon-edge` image maturity; `geth-bor` Docker image?
4. Funding test wallets — pre-funded (Anvil default 10) vs `anvil_setState` vs faucet mock?
5. RPC drift (#474) — does local fork sidestep `publicnode.com` default?
6. Fork staleness — how far behind mainnet/Amoy head?
7. CI integration — L29 operator-driven vs CI-gated per L29 framing

**Research findings:**

| Concern | Anvil (Foundry) | Testcontainers polygon-edge | Testcontainers bor |
|---|---|---|---|
| Production parity | Low — Foundry, no Bor-specific opcodes | **Very low** — polygon-edge is a separate framework, **not** the production PoS chain | **High** — bor is the production execution layer (`0xpolygon/bor`), paired with `heimdall` |
| Setup cost | Low — binary + `--fork-url` | Medium — Docker | Medium-High — bor + heimdall pair |
| Image maturity | N/A (binary) | Medium but **deprioritized** per Polygon tech blog (Edge ≠ production direction) | Medium — community-maintained `bor` Docker images |
| Verification (`forge --verify`) | Known broken on Polygon: [#7733](https://github.com/foundry-rs/foundry/issues/7733), [#5379](https://github.com/foundry-rs/foundry/issues/5379), [#3861](https://github.com/foundry-rs/foundry/issues/3861) | N/A | N/A |
| Bor opcode coverage | Partial — generic EVM only | None (Edge = IBFT 2.0 consensus, not Bor) | **Full** |
| Already in use in repo | **YES** — PR #485 ships it for V9/use_case | No | No |
| Testcontainers module | N/A | First-party Go: [`modules/polygonedge`](https://github.com/testcontainers/testcontainers-go/tree/master/modules/polygonedge) | No first-party Rust/Go module; custom container or shell script |

**Key insight:** `polygon-edge` (the most discoverable Testcontainers target) is **not** the production Polygon PoS chain. Polygon's production chain uses `bor` (Geth fork) + `heimdall` (consensus). Investing in `polygon-edge` test infrastructure gives framework-flexibility but not production-parity — wrong target for a wallet-core that will sign real Polygon mainnet transactions.

## Decision

**Tier 1 (now — already shipped via PR #485):** Anvil Polygon hardfork via `anvil --fork-url <RPC> --fork-chain-id 137|80002`.

**Tier 2 (later — Bor-specific opcode coverage needed):** Testcontainers `bor` image (NOT `polygon-edge` — different framework, no production parity). Switch triggers:

- Any Bor-specific opcode (validator-set changes, state-sync precompile)
- Bor consensus test
- Full EIP-1559-on-Polygon edge case beyond generic EIP-1559

**Defer:** Testcontainers `polygon-edge` — wrong product (Edge framework ≠ PoS chain), maintenance burden without production parity.

### Tier 1 details

**Spin-up:**

```bash
# Mainnet fork
anvil --fork-url "$POLYGON_MAINNET_RPC_URL" \
      --fork-chain-id 137 \
      --port 8545 \
      --block-time 2  # match Polygon block time
# Amoy fork (same shape, --fork-chain-id 80002)
```

**Default RPC URLs** (per #474 drift note):

- Mainnet: `https://polygon-bor-rpc.publicnode.com`
- Amoy: `https://polygon-amoy-bor-rpc.publicnode.com`
- Override via `POLYGON_RPC_URL` / `POLYGON_AMOY_RPC_URL` env (L29 / L61)

**Funding test wallets** (cheapest first):

1. **Pre-funded accounts** (Anvil default indices 0-9, 10000 ETH each) — covers 90% of scenarios; **use by default**
2. **`anvil_setBalance`** — set arbitrary ETH balance; **for ERC-20 scenarios needing a balance but not specific state**
3. **`anvil_impersonateAccount`** — preserve fork-mode state for a real address; **for scenarios requiring a specific mainnet account's holdings**

**CI integration (per L29):**

- `#[ignore]` on the integration test by default
- `RUN_POLYGON_LOCAL=1` env var to opt in
- Manual run script at `scripts/run-polygon-local-smoke.sh`
- NOT in CI gate — operator-driven, L29-style

**L46 + L45 routing:** `rust-evm-core` integration branch hosts both ETH + Polygon local-testnet fixtures.

### Tier 2 details (future — when Bor-specific scenarios land)

**Switch trigger criteria:**

- Any scenario requiring Bor precompile not in generic EVM
- Validator-set or state-sync testing
- Full Polygon-specific EIP-1559 edge cases beyond generic EIP-1559

**Implementation shape:**

- Testcontainers Rust crate (`testcontainers` ~0.20) wrapping a `bor` Docker image
- `heimdall` consensus partner — minimal mock (single-validator mode for test only)
- Local RPC at fixed port, same `--fork-url` pattern as Tier 1
- Keep Tier 1 path for non-Bor-specific scenarios

**Why not polygon-edge:** confirmed at `0xpolygon/polygon-edge` Docker Hub tags — deprioritized per Polygon tech blog. Production parity = zero. Wallet-core needs to sign real Polygon PoS, not Polygon Edge.

### Files affected (this PR)

- `docs/superpowers/adrs/2026-08-31-adr-0002-polygon-local-testnet.md` — **this file**
- `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §0 drift note — append one-line summary

### Files affected (Tier 1 follow-up — separate PR when implemented)

- **No new crates** for Tier 1 — Anvil is already a dep (`alloy-node-bindings` per plan line 16)
- `rust-wallet-app/spikes/polygon-v1/scripts/run-polygon-local-smoke.sh` — new (Tier 1)
- `rust-wallet-app/spikes/polygon-v1/tests/local_testnet_smoke.rs` — new (Tier 1, `#[ignore]` per L29)
- Tier 2 follow-up issue — to be filed when trigger criteria hit

### Test scenario draft (Tier 1, pasteable into `polygon-v1/tests/local_testnet_smoke.rs`)

```rust
//! Polygon local-testnet smoke (Anvil hardfork, per ADR 0002 Tier 1).
//! Per L29: opt-in via RUN_POLYGON_LOCAL=1. NOT CI-gated.

#![cfg(feature = "local-testnet")] // gated feature; off by default

#[tokio::test]
#[ignore = "L29: opt-in via RUN_POLYGON_LOCAL=1 + manual script"]
async fn polygon_local_testnet_balanceof_roundtrip() {
    // Skip when RUN_POLYGON_LOCAL not set (L29 opt-in discipline)
    if std::env::var("RUN_POLYGON_LOCAL").is_err() {
        eprintln!("RUN_POLYGON_LOCAL not set; skipping per L29");
        return;
    }

    // 1. Spin-up: anvil already running on :8545 via scripts/run-polygon-local-smoke.sh
    let cfg = Config::local_mainnet()?;

    // 2. Fund: pre-funded account index 0 (Anvil default 10000 ETH)
    let sender = cfg.anvil_prefunded_account(0)?;

    // 3. Query: balanceOf via polygon-v1 client (existing PR #485 path)
    let balance = balance_of(&cfg, sender, erc20_address).await?;

    // 4. Assert: non-zero (Anvil fork mirrors mainnet state)
    assert!(balance > U256::ZERO);

    // 5. Teardown: no explicit drop — anvil exits on script shutdown
}
```

### Acceptance criteria (this issue #492)

- [x] **Decision recorded**: Anvil / Testcontainers / other — rationale, trade-offs, evidence links (this ADR)
- [ ] **Test scenario drafted**: Rust sketch above, pasteable into `polygon-v1/tests/local_testnet_smoke.rs` (paste + file follow-up tracked in #492 acceptance)
- [ ] **eth CLI parity check**: how `eth-wallet-core` handles local-testnet today (read `eth/src/handlers.rs` + `crates/eth-wallet-core/`); mirror pattern in `polygon/` — Tier 1 follow-up PR
- [ ] **CI plan**: `#[ignore]` + `RUN_POLYGON_LOCAL=1` per L29; NOT in CI gate — documented in this ADR §Tier 1
- [ ] **Decision propagates to plan** `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §0 — one-line summary appended (this PR)
- [ ] **Cross-crate divergence check** (per L13 step 4a extended): diff the equivalent helper in sibling CLIs at pickup — Tier 1 follow-up PR

### Open questions deferred to Tier 2

- Testcontainers Rust `bor` image availability + startup time on this host
- Heimdall single-validator mode for test (Bor consensus without full PoS)
- `anvil_setStorageAt` for ERC-20 storage overrides — useful for testing specific token balances without mainnet history
- Fork mode vs fresh chain mode — fresh-chain mode skips fork-staleness; useful for tests that don't depend on mainnet state

### Anti-patterns to avoid

- **Defaulting CI-gated without operator opt-in** — violates L29
- **Picking polygon-edge for production parity** — wrong product, framework ≠ PoS chain
- **Skipping Tier 1 because Tier 2 looks "more correct"** — Tier 1 already proven in PR #485; Tier 2 only when trigger criteria hit
- **Bundling Tier 2 setup cost onto Tier 1 tasks** — scope creep, defers the proven path
- **Forgetting the SPKI pin** — per plan line 16 (`reqwest` 0.12 + `rustls` 0.23 + raw SPKI pin per Q7); local Anvil must verify against pinned key, not skip TLS

## Cross-references

- [#492](https://github.com/nhitranbtc/blockchain-sdk/issues/492) — this ADR
- [#416](https://github.com/nhitranbtc/blockchain-sdk/issues/416) — parent plan
- [#419](https://github.com/nhitranbtc/blockchain-sdk/issues/419) — V9/use_case parent
- PR [#485](https://github.com/nhitranbtc/blockchain-sdk/pull/485) — V9/use_case `balanceOf` round-trip via Anvil Tier 1 (empirically validates this decision)
- [#474](https://github.com/nhitranbtc/blockchain-sdk/issues/474) — RPC drift → publicnode.com defaults
- [foundry-rs/foundry #7733](https://github.com/foundry-rs/foundry/issues/7733), [#5379](https://github.com/foundry-rs/foundry/issues/5379), [#3861](https://github.com/foundry-rs/foundry/issues/3861) — Polygon verification known broken
- [0xpolygon/bor](https://github.com/0xpolygon/bor), [0xpolygon/heimdall](https://github.com/0xpolygon/heimdall) — production chain = bor + heimdall, NOT polygon-edge
- [0xpolygon/polygon-edge Docker Hub](https://hub.docker.com/r/0xpolygon/polygon-edge/tags) — deprioritized
- [testcontainers/testcontainers-go modules/polygonedge](https://github.com/testcontainers/testcontainers-go/tree/master/modules/polygonedge) — first-party Go module, not Rust
- L29 — live-testnet smoke operator-driven rule
- L11 — skill → step mapping for research + ADR
- L13 step 11a — this issue created as backlog big-task class
- L13 step 4a extended — sibling-CLI divergence check at pickup
- `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` — parent plan, Phase 0.0 + §0 drift note