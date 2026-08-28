# Polygon PoS spike V1–V10 + use-case (Issue #417 / parent #416)

Verification harness for the open questions from Issue #416 (Polygon + evm-wallet-core plan).
Lives in the `rust-wallet-app` umbrella workspace per plan §File Structure; production code
lives in `crates/evm-wallet-core/` (refactored from `eth-wallet-core`) + `crates/polygon-wallet-core/`
(thin wrapper).

Each Vn ties to one open question (Q1–Q8) from the deep-dive + plan. The spike IS the resolution
mechanism per L13 Q-resolution-before-code rule — Q-pass evidence in `RESULT.md` flips the
parent #416 acceptance checkboxes.

| Vn | Q | What it proves |
|----|---|----------------|
| V1 | Q1 | `cargo build -p polygon-v1-spike` + `cargo build -p evm-wallet-core -p eth-wallet-core -p polygon-wallet-core` clean (EVM-reuse path: shared `alloy` provider + signer primitives) |
| V2 | Q4 | Chain-id 137 (mainnet) + 80002 (Amoy testnet) via `eth_chainId` (GATED on `RUN_POLYGON_AMOY=1`) |
| V3 | Q1 | Cross-chain derivation identity: canonical "abandon ×11 + about" BIP-39 mnemonic → same EVM address on Ethereum + Polygon (SLIP-44 coin type 60 → both chains) |
| V4 | Q5 | EIP-1559 fee estimates: `max_fee_per_gas` + `max_priority_fee_per_gas` + 2-second-block `baseFee` cadence (GATED on `RUN_POLYGON_AMOY=1`) |
| V5 | Q4 | Mainnet RPC connectivity + first-block age + finality (~256 blocks ≈ 8 min) (GATED on `RUN_POLYGON_MAINNET=1`) |
| V6 | Q3 | Token registry load: bundled `tokens/mainnet.json` (USDC + USDT + DAI) + `tokens/amoy.json` (USDC); decimals verified on-chain (GATED on `RUN_POLYGON_AMOY=1`) |
| V7 | Q4 | Amoy faucet reachability + fund-and-poll pattern (GATED on `RUN_POLYGON_AMOY=1`) |
| V8 | Q5 | Native POL transfer: signer → recipient POL value transfer + receipt poll (GATED on `RUN_POLYGON_AMOY=1`) |
| V9 | Q3 | ERC-20 transfer on Anvil Polygon-fork: deploy mock USDC → `transfer(beta, N)` → `balanceOf` verify (offline, Anvil in-process) |
| V10 | Q7 | EIP-712 cross-chain replay protection: signed payload from Polygon-amoy MUST NOT verify on Ethereum-mainnet (chain-id in domain separator per EIP-712) |
| Use-case | Q1-Q8 | End-to-end: alpha → beta 100 USDC (offline on Anvil Polygon-fork; live Amoy gated) |

## Run

### Offline (V1, V3, V9, V10)

```bash
cargo test -p polygon-v1-spike --tests
```

CI-friendly. No network, no API keys, no Docker.

### Live — operator-driven per L29

| Path | Env vars | Purpose |
|---|---|---|
| V2, V4, V6, V7, V8 | `RUN_POLYGON_AMOY=1` | Read-only + send live calls to Amoy testnet RPC (`https://rpc-amoy.polygon.technology`) |
| V5 | `RUN_POLYGON_MAINNET=1` | Read-only mainnet connectivity + finality check |
| Use-case live | `RUN_POLYGON_AMOY=1` + `POLYGON_AMOY_PRIVATE_KEY` + `POLYGON_AMOY_RECIPIENT` | End-to-end live Amoy broadcast |

Run live paths:

```bash
RUN_POLYGON_AMOY=1 cargo test -p polygon-v1-spike --tests
RUN_POLYGON_MAINNET=1 cargo test -p polygon-v1-spike --test v5_rpc_connectivity -- --ignored --nocapture
```

Requires outbound HTTPS to `https://rpc-amoy.polygon.technology` (or `https://polygon-rpc.com` for mainnet).