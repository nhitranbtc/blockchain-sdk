# alloy v1.x spike (Issue #293)

Standalone Rust workspace that verifies the `alloy` v1.x API surface for the
upcoming `eth/` crate (companion to `bitcoin-wallet-core/`).

**Why a separate workspace**: not added to the umbrella `rust-wallet-app/`
`members` list. The spike is a one-shot verification harness; production
builds do not include it.

## What this proves

| ID | Test                                                  | Status      | Operator opt-in              |
| -- | ----------------------------------------------------- | ----------- | ---------------------------- |
| V1 | `alloy 1.8.3` + transitive deps compile               | runs always | `cargo check`                |
| V2 | `MnemonicBuilder::english().phrase(mnemonic).index(0).build()` → expected ETH address at `m/44'/60'/0'/0/0` | runs always | `cargo test --test v2_mnemonic` |
| V3 | `provider.get_block_number()` against `https://ethereum.reth.rs/rpc` | ignored | `RUN_V3_LIVE_RPC=1 cargo test --test v3_live_rpc -- --ignored` (env var AND `--ignored` flag both required) |
| V4 | `provider.send_transaction(...)` against local Anvil + `get_receipt()` | ignored | `RUN_V4_ANVIL=1 cargo test --test v4_anvil_send -- --ignored` (env var AND `--ignored` flag both required; needs foundry) |
| V5 | ERC-20 `transferCall` calldata → first 4 bytes `0xa9059cbb` | runs always | `cargo test --test v5_erc20_calldata` |
| V6 | Anvil `MockERC20` `sol!` definition + constructor calldata round-trip | ignored | `RUN_V6_ANVIL=1 cargo test --test v6_erc20_anvil -- --ignored` (env var AND `--ignored` flag both required; needs foundry) |
| V7 | SPKI-pinned `rustls::ServerCertVerifier` accepts matching SPKI + rejects non-matching | runs always | `cargo test --test v7_spki_pin` |

V5/V6 deferred to eth/ crate implementation per accepted recommendation; spike ships
the type-level verification (calldata selector for V5; `MockERC20` `sol!` + constructor
calldata for V6) so the eth/ crate Phase 3 implementation has a known-good API surface.

## Sepolia sample tests (Issue #299, template for #298)

The three `e2e_sepolia_*.rs` tests are reference implementations for the broader
per-user-story e2e suite tracked in [#298](https://github.com/nhitranbtc/blockchain-sdk/issues/298).
One sample per story-class:

| Test                     | Story | What it proves                                              |
| ------------------------ | ----- | ----------------------------------------------------------- |
| `e2e_sepolia_balance`    | 3     | Read-only `provider.get_balance(addr)` against Sepolia      |
| `e2e_sepolia_send_native`| 5     | Signed EIP-1559 tx + `pending.get_receipt()` (status=true)   |
| `e2e_sepolia_erc20_balance` | 22 | `sol!`-typed `balanceOf` via `provider.call` + ABI decode    |

All three are `#[ignore]` (L29 operator-driven — never run in CI).

### Required env vars

| Var                     | Used by                | Default                                 |
| ----------------------- | ---------------------- | --------------------------------------- |
| `RUN_ETH_E2E`           | all 3                  | must be `1` (other values → SKIP)       |
| `ETH_E2E_RPC_URL`       | all 3                  | none — required                         |
| `ETH_E2E_MNEMONIC`      | all 3 (or `_FILE`)     | mutually exclusive w/ `ETH_E2E_MNEMONIC_FILE` |
| `ETH_E2E_RECIPIENT`     | send_native            | derived `m/44'/60'/0'/0/1`              |
| `ETH_E2E_TOKEN_ADDRESS` | erc20_balance          | none — required when erc20 target runs  |

### Sepolia ETH (operator fund)

Sample mnemonic must be funded first — visit one of:

- https://sepoliafaucet.com/
- https://www.alchemy.com/faucets/ethereum-sepolia
- https://www.infura.io/faucet/sepolia

Funds m/44'/60'/0'/0/0 of the phrase (Story 5 sender).

### Operator run

```bash
# From repo root.
ETH_E2E_TESTNET=1 \
  ETH_E2E_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com \
  ETH_E2E_MNEMONIC_FILE=$HOME/.sepolia-test-mnem.txt \
  ETH_E2E_TOKEN_ADDRESS=0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238 \
  bash rust-wallet-app/scripts/eth-send-sepolia-e2e.sh
```

Each target logs to `/tmp/eth-e2e-<target>.log` (one of `balance`,
`send_native`, `erc20_balance`). Override with `ETH_E2E_TEST_TARGETS="balance erc20_balance"`
to skip the state-changing send. Exit code is non-zero if any selected target fails.

### Promotion path

When `eth-wallet-core` crate ships (Plan Task 1), these samples migrate to
`rust-wallet-app/crates/eth-wallet-core/tests/e2e_sepolia/`. Issue #299 keeps
the spike paths until the crate exists.

## Run it

```bash
# From the spike root (NOT from rust-wallet-app/)
cd rust-wallet-app/spikes/alloy-v1

# V1 — compile check
cargo check --all-targets

# V2 — deterministic mnemonic test
cargo test --test v2_mnemonic -- --nocapture

# V7 — SPKI pin verifier
cargo test --test v7_spki_pin -- --nocapture

# V3 — LIVE RPC against reth.rs (operator-only per L29).
# ⚠️  DOUBLE-GATE: env var AND --ignored flag are BOTH required.
# Missing either → cargo silently skips the test.
RUN_V3_LIVE_RPC=1 cargo test --test v3_live_rpc -- --ignored --nocapture

# V4 — Anvil spawn + signed tx (operator-only per L29).
# Requires `foundry` install first: curl -L https://foundry.paradigm.xyz | bash
# ⚠️  DOUBLE-GATE: env var AND --ignored flag are BOTH required.
RUN_V4_ANVIL=1 cargo test --test v4_anvil_send -- --ignored --nocapture
```

## Q1 resolution evidence (already captured)

- `cargo info alloy` shows `alloy 1.8.3 (latest 2.4.1)`; `alloy@1.10` does not
  exist → 1.8.3 is the latest 1.x line.
- `rust-version: 1.91` for alloy 1.8.3 < workspace toolchain `1.94` → MSRV parity.
- Workspace `rust-toolchain.toml` pins `1.94` channel → `cargo` will use 1.94.

## Q2 resolution evidence (deferred)

`alloy-transport-http` does **not** expose a public hook for a custom
`ServerCertVerifier`. The eth/ crate will use raw `reqwest` + `rustls` with a
custom verifier for pinned endpoints (mirrors Bitcoin F20 / Task 7).
V7 spike validates the verifier wiring.

## Q3–Q9 resolution

See [Issue #293 body](https://github.com/nhitranbtc/blockchain-sdk/issues/293)
for resolved paths + rationale + verification evidence. Q1 + Q2 captured above;
Q3–Q9 are documented in the issue body and the
[`docs/superpowers/plans/2026-08-23-eth-wallet-core.md`](../../../docs/superpowers/plans/2026-08-23-eth-wallet-core.md)
implementation plan.

## Files

```text
rust-wallet-app/spikes/alloy-v1/
├── Cargo.toml          # standalone, NOT in umbrella members
├── README.md           # this file
└── tests/
    ├── v2_mnemonic.rs          # deterministic BIP-39 → address
    ├── v3_live_rpc.rs          # live RPC against reth.rs (#[ignore])
    ├── v4_anvil_send.rs        # Anvil spawn + signed tx (#[ignore])
    ├── v5_erc20_calldata.rs    # ERC-20 calldata selector check
    ├── v6_erc20_anvil.rs       # Anvil MockERC20 sol! (#[ignore])
    ├── v7_spki_pin.rs          # rustls ServerCertVerifier SPKI pin
    ├── e2e_sepolia_balance.rs        # Story 3 sample (#[ignore], Issue #299)
    ├── e2e_sepolia_send_native.rs    # Story 5 sample (#[ignore], Issue #299)
    └── e2e_sepolia_erc20_balance.rs  # Story 22 sample (#[ignore], Issue #299)
```