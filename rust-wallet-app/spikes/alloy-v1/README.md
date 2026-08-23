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
| V2 | `MnemonicBuilder::new().phrase(mnemonic).build()` → expected ETH address at `m/44'/60'/0'/0/0` | runs always | `cargo test --test v2_mnemonic` |
| V3 | `provider.get_block_number()` against `https://ethereum.reth.rs/rpc` | ignored | `RUN_V3_LIVE_RPC=1 cargo test --test v3_live_rpc -- --ignored` (env var AND `--ignored` flag both required) |
| V4 | `provider.send_transaction(...)` against local Anvil + `get_receipt()` | ignored | `RUN_V4_ANVIL=1 cargo test --test v4_anvil_send -- --ignored` (env var AND `--ignored` flag both required; needs foundry) |
| V7 | SPKI-pinned `rustls::ServerCertVerifier` accepts matching SPKI + rejects non-matching | runs always | `cargo test --test v7_spki_pin` |

V5 (ERC-20 calldata `0xa9059cbb`) and V6 (Anvil MockERC20 + balanceOf) are
**deferred to the eth/ crate implementation** (per issue #293 resolution).

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
    ├── v2_mnemonic.rs   # deterministic BIP-39 → address
    ├── v3_live_rpc.rs   # live RPC against reth.rs (#[ignore])
    ├── v4_anvil_send.rs # Anvil spawn + signed tx (#[ignore])
    └── v7_spki_pin.rs   # rustls ServerCertVerifier SPKI pin
```