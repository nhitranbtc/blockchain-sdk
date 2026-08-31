# Polygon-Specific Rust SDK Deep-Dive

**Date:** 2026-08-27
**Scope:** Focused research on Rust crates for a Polygon PoS (EVM-compatible L2) wallet built on top of `evm-wallet-core/` (refactor target under Option A — see issue #416). Covers native POL transfer + ERC-20 stablecoin transfer (USDT, USDC, DAI), EIP-1559 gas model on Polygon's 2-second blocks, POL (post-MATIC) tokenomics, and the Amoy testnet (replaces Mumbai as of 2024). Reuses the eth-wallet-core alloy-based surface for primitives; documents only the Polygon-specific deltas.
**Companion to:** `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` (EVM primitive reference), `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md` (sibling non-EVM chain template).
**Tracks:** issue #416 (plan(pol), rust-sdks deep-dive + polygon-wallet-core).
**Status:** Research report only. No design spec, no implementation plan, no code produced in this session.

## TL;DR

Use **`alloy` 1.x** (same as eth-wallet-core) as the primary Polygon stack — Polygon is EVM-compatible at the consensus layer (solidity runs unchanged, RLP tx envelopes, keccak256 addresses), so the EVM-reuse decision (Option A in issue #416) makes Polygon a config-only addition to `evm-wallet-core` rather than a new SDK surface. **Five primitives reused from eth-wallet-core** (`alloy` + `alloy-signer-local` + `alloy-provider` + `bip32` + `bip39`), plus the existing workspace `reqwest`+`rustls` stack. **Polygon-specific deltas**: native token POL (replaced MATIC on 2024-09-04 mainnet / 2024-09-25 Ahmedabad hardfork); EIP-1559 active since London fork 2022-01-18 (block 23,850,000) with **Polygon-specific baseFee dynamics** (2-second blocks → ~12s to 2× baseFee, vs Ethereum's ~60s); Amoy testnet (chain-id 80002) replaces Mumbai (Goerli-rooted) as of 2024-01; gas-token display = "POL" (post-rebrand) with MATIC alias for legacy wallet UX.

## The 5 chosen crates — current 2026 state

| Crate | Version | Stars (GitHub) | Recent DL/wk (crates.io) | Role | Reused from workspace? | License | Maintained? | Mobile-friendly? | Notes |
|---|---|---|---|---|---|---|---|---|---|
| `alloy` | 1.8.3 (stable 1.0 May 2025; 2.x tracks latest) | **1,321** ([alloy-rs/alloy](https://github.com/alloy-rs/alloy)) | **3.17M** | EVM provider + signer + types | Workspace dep once `evm-wallet-core` ships | Apache-2.0 / MIT | Yes | Yes — `no_std` core, pure-Rust signer default | MSRV 1.85 (1.x) / 1.91 (2.x). **Workspace `rust-toolchain.toml` pins 1.94** → 2.x OK. Pin to **1.8.x for v0.1** (matches ETH precedent — same MSRV parity logic). 140 published versions, last update 2026-08-13. |
| `alloy-signer-local` | same as alloy | (same monorepo) | n/a | Local signer + BIP-39 mnemonic | Yes via `evm-wallet-core` | Apache-2.0 | Yes | Yes | `PrivateKeySigner::from_phrase(...)` → EOA. Same `m/44'/60'/0'/0/0` derivation as Ethereum. **Polygon does NOT define its own SLIP-44 coin type — reuses ETH coin type 60** (verified via SLIP-0044 master; no Polygon-specific entry). |
| `alloy-provider` + `alloy-transport-http` | same as alloy | (same monorepo) | n/a | JSON-RPC client | Yes via `evm-wallet-core` | Apache-2.0 | Yes | Yes | `ProviderBuilder::new().connect_http(url)` against `https://polygon-rpc.com` (mainnet) or `https://polygon-amoy.drpc.org` (Amoy). Same filler pattern as ETH — pass signer explicitly, do not use auto-wallet filler. |
| `bip32` | ^0.5 (already workspace from BTC + ETH) | n/a (in [iqlusioninc/crates monorepo](https://github.com/iqlusioninc/crates/tree/main/bip32)) | **3.55M** | HD derivation `m/44'/60'/0'/0/0` | Yes | MIT | Yes | Yes | Identical mechanics to ETH — only the RPC URL differs. |
| `bip39` | 2.2 (already workspace) | **111** ([rust-bitcoin/rust-bip39](https://github.com/rust-bitcoin/rust-bip39)) | **3.70M** | Mnemonic generate/parse/to_seed | Yes | MIT | Yes — last push 2026-08-20 | Yes | Same wordlist as BTC/ETH — one mnemonic, multiple chains, only derivation path differs. |

**Supporting crates (used transitively or for SPKI pinning — not direct Polygon deps):**

| Crate | Repo | Stars | License | Notes |
|---|---|---|---|---|
| `reqwest` | [seanmonstar/reqwest](https://github.com/seanmonstar/reqwest) | **11,801** | Apache-2.0 | Workspace dep. Last push 2026-08-10. |
| `rustls` | [rustls/rustls](https://github.com/rustls/rustls) | **7,587** | Apache-2.0 / ISC / MIT (repo shows NOASSERTION — verified dual-license from source headers) | Workspace dep for SPKI pin verifier. Last push 2026-08-26. |
| `k256` (secp256k1) | [RustCrypto/elliptic-curves monorepo](https://github.com/RustCrypto/elliptic-curves) (k256 submodule) | **873** (monorepo total) | Apache-2.0 / MIT | Transitively via `alloy-signer-local` `mnemonic` feature. **16.97M recent DL/wk** — high production usage. Last push 2026-08-10. |
| `tiny-keccak` | [debris/tiny-keccak](https://github.com/debris/tiny-keccak) | **203** | CC0-1.0 | Transitively via `alloy`. **46.20M recent DL/wk** — last crate publish 2020-04-01 (stale on crates.io) but repo last push 2024-06-10 (still maintained). Stable API, no recent changes needed. |

**Rejected crates (for reference — not adopted):**

| Crate | Repo | Stars | Status | Why rejected |
|---|---|---|---|---|
| `ethers-rs` | [gakonst/ethers-rs](https://github.com/gakonst/ethers-rs) | **2,509** | **Deprecated 2024-09-23** (last repo push). 698K recent DL/wk — still installed but not maintained. | Officially deprecated (issue #2667); maintainers redirect to alloy. Don't adopt for v0.1. |
| Polygon Bor client | [0xPolygon/polygon-sdk](https://github.com/0xPolygon/polygon-sdk) | **1,052** | Active (Apache-2.0, last push 2024-08-27) | Consensus client (Bor node), not a wallet SDK. Reference for block-encoding details if v0.3+ ships a full-node integration. |

**Crate health summary (2026-08-27 live data via `gh api`):** every direct Polygon wrapper dep has ≥1,000 GitHub stars (alloy = 1,321) AND ≥3M recent crates.io downloads/week, except `bip39` (111 stars, but 3.70M DL — niche crate, high production usage signal). Maintenance signals uniformly green: alloy last push 2026-08-26, bip39 last push 2026-08-20, reqwest 2026-08-10, rustls 2026-08-26. No deprecation warnings, no abandoned maintainers, no security advisories active on these versions.

**No new direct deps needed** for Polygon. Total new workspace deps = **0** (everything reuses `evm-wallet-core`). This is the Option A payoff: Polygon = thin wrapper crate (`polygon-wallet-core` or `polygon` CLI) that just configures the existing EVM stack with chain-id + RPC URL + POL gas-token display.

## Why EVM-reuse (Option A) — refactor `eth-wallet-core` → `evm-wallet-core` + thin wrappers

Three reasons, in priority order:

1. **Polygon is EVM-compatible at the consensus layer.** Per `docs.polygon.technology/pos/get-started/building-on-polygon` (verified 2026-08-27): *"Polygon Chain is EVM-compatible. Foundry, Remix, Hardhat, ethers.js, and web3.js all work on Polygon without modification. Point your tooling at the Polygon RPC and deploy."* Same Solidity bytecode, same RLP tx envelope, same keccak256 address derivation, same EIP-1559 fee semantics. The **only chain-specific differences** are: chain-id (137 vs 1), RPC endpoint, native gas token (POL vs ETH), block time (2s vs 12s — affects baseFee dynamics only), and testnet identity (Amoy vs Sepolia). All four are runtime configuration; none require SDK changes.

2. **`alloy` already covers both chains.** The 5 chosen crates for eth-wallet-core (above) work unchanged on Polygon. `alloy_chains::Chain::Polygon` is a built-in enum value; `provider.get_chain_id()` returns `137` for mainnet and `80002` for Amoy. No new dep, no new crate surface, no new signing path.

3. **Future EVM chains (Base, Arbitrum, Optimism) = same wrapper pattern.** Once `evm-wallet-core` exists, adding a new EVM L2 is `git checkout -b feat/base-wallet-core` + a thin wrapper crate. This is the Option A payoff: ship N EVM chains for the cost of one SDK.

**Rejected alternatives** (per issue #416 architecture table):

| Option | Why rejected |
|---|---|
| **B — Extend `eth-wallet-core` with chain-config enum** | Backward-compatible but the name drifts (`eth-wallet-core` carries non-Ethereum chains). Enums grow ugly; new chains require touching the same crate. |
| **C — Standalone `polygon-wallet-core` (TRON-shape)** | Duplicates signing + RPC + ABI code (~80% with eth-wallet-core). Maintenance cost doubles per chain. |
| **`ethers-rs` for Polygon** | ethers-rs officially deprecated 2024-06 (issue #2667). alloy is the successor. |
| **Raw `reqwest` + hand-rolled RLP** | Loses `alloy`'s type-safe `Address` / `U256` / `TransactionRequest`. Adds hundreds of lines. |

## Polygon vs Ethereum — crate-by-crate comparison

The Polygon wallet shares **100% of its crate surface** with `eth-wallet-core` (Option A payoff). The differences are *configuration*, not *dependencies*. This table makes that concrete.

| Concern | Ethereum (`eth-wallet-core`) | Polygon (`polygon-wallet-core`) | Delta |
|---|---|---|---|
| **`alloy` (meta)** | 1.8.x direct dep | Indirect via `evm-wallet-core` re-export | **None** — same version, same crate |
| **`alloy-signer-local`** | Direct dep (`mnemonic` feature) | Indirect via `evm-wallet-core` | **None** — same crate, same feature flags |
| **`alloy-provider` + `alloy-transport-http`** | Direct dep | Indirect via `evm-wallet-core` | **None** |
| **`alloy-sol-types`** | Direct dep (ERC-20 ABI) | Indirect via `evm-wallet-core` | **None** — ERC-20 ABI identical across EVM |
| **`alloy-chains`** | n/a | Direct dep (`Chain::Polygon`, `Chain::PolygonAmoy`) | **NEW in Polygon wrapper** — enum variant for Polygon mainnet + Amoy |
| **`k256`** (secp256k1) | Indirect via `alloy-signer-local` | Indirect via `evm-wallet-core` → `alloy-signer-local` | **None** |
| **`bip32` ^0.5** | Direct dep (`m/44'/60'/0'/0/0`) | Direct dep (same path) | **None** — same workspace crate, same path, same address |
| **`bip39` 2.2** | Direct dep (`zeroize` + `rand`) | Direct dep (same features) | **None** |
| **`reqwest` 0.12 + `rustls` 0.23** | Direct dep | Direct dep | **None** — same workspace stack |
| **`tiny-keccak` 2.0.2** | Indirect (via `alloy`) | Indirect (via `alloy`) | **None** |
| **Chain-id constant** | `1` (mainnet) / `11155111` (Sepolia) | `137` (mainnet) / `80002` (Amoy) | **DIFFERENT** — runtime config only |
| **RPC default URL** | Configurable (per-call) | `https://polygon-rpc.com` (mainnet) / `https://polygon-amoy.drpc.org` (Amoy) | **DIFFERENT default** — overridable via `--rpc-url` |
| **Network enum variant** | `Network::Ethereum` | `Network::Polygon` | **NEW variant** on shared enum in `evm-wallet-core` |
| **Native gas token** | ETH | POL (post-MATIC, with MATIC alias for legacy UX) | **DIFFERENT** — display-only, no SDK impact |
| **Token registry** | `tokens/mainnet.json` (USDT, USDC) | `tokens/mainnet.json` (USDT, USDC, DAI) + `tokens/amoy.json` (USDC Amoy) | **MORE entries + new Amoy file** |
| **SPKI pin verifier** | Reused from `bitcoin-wallet-core` | Reused from `bitcoin-wallet-core` | **None** — same path, same `pinned://` URL scheme |
| **Gas estimation cadence** | 12-second blocks → cache 30s OK | 2-second blocks → re-estimate immediately before broadcast | **BEHAVIORAL** — wallet must call `estimate_eip1559_fees()` per-broadcast, not cache |
| **Tx envelope** | EIP-1559 Type 2 (London active since 2021-08-05) | EIP-1559 Type 2 (London active since 2022-01-18) | **None** — same envelope |
| **EIP-712 typed data** | `chain_id: 1` in domain separator | `chain_id: 137` in domain separator | **DIFFERENT constant** — alloy's built-in replay protection |
| **Async test pattern** | `#[tokio::test] async fn` (per ETH #333) | `#[tokio::test] async fn` (same rule) | **None** |
| **Total new direct deps for Polygon** | n/a | **+1** (`alloy-chains`) | Minimal |

### What the wrapper crate (`polygon-wallet-core`) actually adds

```text
evm-wallet-core/          # shared EVM core (signing, RPC, ABI, gas estimation)
├── src/
│   ├── lib.rs            # Network enum: Ethereum | Polygon | ...
│   ├── chain.rs          # alloy-chains::Chain re-export
│   └── wallet/           # signing + RPC plumbing
└── Cargo.toml            # alloy + bip32 + bip39 + reqwest + rustls (workspace deps)

polygon-wallet-core/      # thin wrapper — config only
├── src/
│   ├── lib.rs            # re-exports from evm-wallet-core
│   ├── network.rs        # Network::Polygon config (chain_id, RPC URL, gas token display)
│   └── tokens/
│       ├── mainnet.json  # USDT, USDC, DAI addresses (Polygon mainnet)
│       └── amoy.json     # USDC Amoy address
└── Cargo.toml            # alloy-chains (NEW), evm-wallet-core (path dep)

eth-wallet-core/          # unchanged — pure wrapper around evm-wallet-core for ETH
├── src/lib.rs            # re-exports + Network::Ethereum config
└── Cargo.toml            # evm-wallet-core (path dep), no new direct deps
```

### Why this matters

- **No `polygon-wallet-core/src/{signing,rpc,abi}.rs` duplication.** The wrapper is ~200 lines of config + re-exports, not a parallel implementation. v0.1 surface area stays small.
- **Future EVM L2 (Base, Arbitrum, Optimism) = same pattern.** Copy `polygon-wallet-core` shape, swap `Network::Polygon` → `Network::Base`, change RPC URL + chain-id + token registry. ~1 day of work per L2.
- **Refactor risk = contained to `evm-wallet-core`.** The `eth-wallet-core` rename + split is a one-shot PR. Polygon lives entirely in the new wrapper.
- **Bug fix in signing → propagates to all EVM chains.** Single canonical impl = single place to fix. (vs Option C where each chain has its own signing code that can drift.)

## Polygon-specific deltas (what changes vs Ethereum)

### Network identity + RPC endpoints

| Network | Chain ID | Native token | Gas station | Public RPC | Block explorer |
|---|---|---|---|---|---|
| **Polygon PoS mainnet** | **137** | **POL** | `https://gasstation.polygon.technology/v2` | `https://polygon-rpc.com` | `https://polygonscan.com` |
| **Polygon Amoy testnet** | **80002** | **POL** | `https://gasstation.polygon.technology/amoy` | `https://polygon-amoy.drpc.org` (or `https://rpc-amoy.polygon.technology/`) | `https://amoy.polygonscan.com` |

Sources:
- Chain ID 137 (mainnet) + RPC URL: `docs.polygon.technology/pos/get-started/building-on-polygon` (`Mainnet: https://polygon-rpc.com (chain ID 137)`)
- Chain ID 80002 (Amoy) + RPC + gas station: `docs.polygon.technology/pos/reference/rpc-endpoints` (table row: `Network name: Amoy`, `Chain ID: 80002`, `RPC endpoint: https://polygon-amoy.drpc.org`)
- Amoy launch announcement: `polygon.technology/blog/introducing-the-amoy-testnet-for-polygon-pos` (2024-01-12)
- Polygon docs consolidated RPC list: `docs.polygon.technology/pos/reference/rpc-endpoints`

**Amoy replaces Mumbai** (Goerli-rooted) as the Polygon PoS testnet. Both operated concurrently during the transition (Mumbai deprecated in 2024-Q2 after Goerli's deprecation). For v0.1, use Amoy exclusively.

**RPC provider priority** (per Q4 resolution):
1. `https://polygon-rpc.com` (official Polygon Labs endpoint, no API key required for low-volume dev)
2. `https://polygon-amoy.drpc.org` (Amoy fallback)
3. Alchemy / Infura as operator-supplied fallback (not bundled — pass `--rpc-url` flag)

### POL native token (post-MATIC rebrand)

**MATIC → POL migration completed 2024-09-04** on Polygon PoS mainnet. Ahmedabad hardfork (Bor v1.4.0, PIP-37) finalized the symbol change at block **62,278,656** on 2024-09-25/26 (Amoy: block 11,865,856, 2024-09-12).

Key facts (verified 2026-08-27):
- **Initial supply**: 10 billion POL (1:1 with MATIC at migration per PIP-25)
- **Bridge contract**: MATIC held in migration contract (NOT burned); "unmigration" feature available via governance
- **Wallet display**: "POL" for new transactions; MATIC alias acceptable for legacy wallet UX (per Q8 resolution)
- **Migration mechanism**: ERC-20-compatible token (built on OpenZeppelin ERC-20 per `docs.polygon.technology/pos/concepts/tokens/pol`); supports EIP-2612 permits

Sources:
- Migration announcement: `polygon.technology/blog/matic-to-pol-migration-is-now-live-everything-you-need-to-know` (2024-09-04)
- PIPs: PIP-17 (POL token), PIP-19 (Polygon PoS native token), PIP-25 (total supply), PIP-26 (validator rewards)
- Ahmedabad hardfork: `polygon.technology/blog/polygon-pos-the-ahmedabad-upgrade-is-live-on-mainnet` (2024-09-26)
- Bor v1.4.0 release: `github.com/0xPolygon/bor/releases/tag/v1.4.0` (2024-09-13)
- Token spec: `docs.polygon.technology/pos/concepts/tokens/pol`

### EIP-1559 on Polygon — 2-second-block gas dynamics

EIP-1559 activated on Polygon PoS mainnet via the **London hardfork on 2022-01-18 at block 23,850,000**. Polygon uses the same Type 2 transaction format as Ethereum: `maxFeePerGas` ceiling, `maxPriorityFeePerGas` tip, network-determined `baseFee` that's burned.

**Polygon-specific divergence from Ethereum** (per `forum.polygon.technology/t/impact-of-eip1559-and-future-possibilities/1749`, verified 2026-08-27):

| Property | Ethereum | Polygon PoS |
|---|---|---|
| Block time | 12 seconds | **2 seconds** |
| Average block gas limit | 30M | 10M (post-London cap) |
| `baseFee` change rate per block | 12.5% (1/8) | 12.5% (same formula, but **6× more frequent** changes) |
| Time to 2× `baseFee` under sustained full blocks | ~60 seconds | **~12 seconds** |
| Time to `baseFee` halve under empty blocks | ~60 seconds | ~12 seconds |
| Burned baseFee + priority tip | Yes | Yes |

**Practical implication for the wallet:** Polygon's 2-second blocks make `baseFee` estimation significantly more volatile than Ethereum's. **The wallet must re-estimate `maxFeePerGas` immediately before broadcast** (within seconds), not rely on cached values. `provider.estimate_gas()` + `provider.estimate_eip1559_fees()` (alloy) cover this — both call `eth_feeHistory` and return `(max_fee_per_gas, max_priority_fee_per_gas)` recommendations.

**Type 0 (legacy) transactions remain compatible** per `docs.polygon.technology/pos/concepts/transactions/eip-1559` but **Type 2 (1559) is recommended**. Per Q5 resolution, v0.1 ships **Type 2 only** (no legacy fallback) — matches the Bitcoin-side precedent of "ship one tx envelope, do it well".

**Risk:** Polygon has discussed raising `baseFee` change rate from 1/8 to 1/48 to match Ethereum's 60-second volatility (forum discussion, not adopted). Re-check at v0.3 if wallet UX shows stuck-tx complaints.

### Top ERC-20 stablecoins on Polygon (verified 2026-08-27)

| Token | Mainnet contract | Amoy testnet contract | Decimals | Symbol | Source |
|---|---|---|---|---|---|
| **USDC (native, Circle)** | `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` | `0x41E94Eb019C0762f9Bfcf9Fb1e58725BfB0e7582` | **6** | USDC | Circle official developer docs (`developers.circle.com/stablecoins/usdc-contract-addresses`) + Polygon docs |
| **USDT (Tether)** | `0xc2132D05D31c914a87C6611C10748AEb04B58e8F` | n/a (Tether does not publish Amoy testnet contract) | **6** | USDT | `stableregistry.com/contracts/usdt-on-polygon/` (last verified May 2026) + PolygonScan |
| **DAI (MakerDAO)** | `0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063` | n/a | **18** | DAI | MakerDAO docs + `stableregistry.com/contracts/dai-on-polygon/` (verified May 2026) |

**Bridge vs native USDC footgun** (flagged): The address `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` is **native USDC issued by Circle** (post-2023). The **legacy bridged USDC.e** uses a different address and is NOT Circle-issued. For v0.1, the wallet should hard-code the **native USDC address** and label it `USDC` (NOT `USDC.e`). Per `docs.polygon.technology/pos/payments/transfers/transfer-usdc`:

```js
// Native USDC on Polygon Chain (NOT the old bridged USDC.e)
// Source: Circle "USDC Contract Addresses"
const USDC = "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359";
```

**Token registry format** (per Q3 resolution): static bundled JSON at `polygon-wallet-core/tokens/mainnet.json` + `polygon-wallet-core/tokens/amoy.json` (mirror of `eth-wallet-core/tokens/mainnet.json` shape, only contracts + chain-id differ).

### Mnemonic-to-broadcast data flow (end-to-end)

```text
1. polygon wallet create --name w --network mainnet
   ↓
2. bip39::Mnemonic::generate_in(Words12, English, rng)  -- 12-word phrase (identical to BTC/ETH)
   ↓
3. m.to_seed(passphrase)  -- 64-byte PBKDF2 output
   ↓
4. bip32::XPrv::derive_from_path(&seed, "m")  -- master xprv
   ↓
5. master.derive_path("m/44'/60'/0'/0/0")  -- SLIP-44 coin type 60 (reuses ETH, no Polygon-specific entry)
   ↓
6. sk_bytes = child.to_secp256k1_secret_key(&secp).secret_bytes()  -- 32 bytes
   ↓
7. signer = PrivateKeySigner::from_slice(&sk_bytes)  -- via evm-wallet-core
   ↓
8. addr = signer.address()  -- 20-byte EVM address (keccak256(pubkey)[12..32])
   ↓
9. provider = ProviderBuilder::new().connect_http("https://polygon-rpc.com")
   ↓
10. chain_id = provider.get_chain_id().await?  -- 137 (mainnet) or 80002 (Amoy)
   ↓
11. Store m as plaintext (v0.1) or encrypt with Argon2id → AES-256-GCM (v0.2+) on disk
   ↓
12. At send time (native POL transfer):
    - nonce = provider.get_transaction_count(signer.address()).await?
    - (max_fee_per_gas, max_priority_fee_per_gas) = provider.estimate_eip1559_fees().await?  -- re-estimate immediately, NOT cached (Polygon's 2s blocks)
    - tx = TransactionRequest::default()
              .with_to(recipient)
              .with_value(value_wei)
              .with_nonce(nonce)
              .with_gas_limit(estimate_gas + buffer)
              .with_max_fee_per_gas(max_fee_per_gas)
              .with_max_priority_fee_per_gas(max_priority_fee_per_gas)
              .with_chain_id(137)  // 80002 for Amoy
    - signature = signer.sign_transaction_sync(&mut tx)?
    - pending = provider.send_transaction(tx).await?
    - receipt = pending.get_receipt().await?
   ↓
13. At send time (ERC-20 stablecoin transfer):
    - call = transferCall { to: recipient, value: U256::from(human_amount * 10^decimals) }
    - calldata = call.abi_encode().into()
    - tx.with_to(token_contract).with_value(U256::ZERO).with_input(calldata)
    - sign + send as in step 12
```

**Key differences vs the ETH flow** (in `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`):
1. `chain_id = 137` (or `80002`) instead of `1` (or `11155111`)
2. RPC URL = `polygon-rpc.com` instead of mainnet Ethereum RPC
3. **Re-estimate `max_fee_per_gas` immediately before broadcast** (not cached) — Polygon 2-second block time makes cached values stale within seconds
4. Gas-token display = "POL" (or "MATIC" for legacy wallet UX)
5. Everything else (BIP-39, BIP-32, keccak256 address, ERC-20 ABI selectors, signing format) = identical

## Crate-by-crate notes (Polygon deltas vs eth-wallet-core)

### `alloy` 1.x (reused from `evm-wallet-core`)

**Polygon-specific surface:**
- `alloy_chains::Chain::Polygon` (mainnet, chain-id 137) — built-in
- `alloy_chains::NamedChain::Polygon` — for log/UI display
- `alloy_provider::Provider::estimate_eip1559_fees()` — **must be called immediately before broadcast** on Polygon due to 2-second block volatility
- `alloy_provider::Provider::get_chain_id()` — returns `137` or `80002`

No `alloy` config changes needed. The `Network` enum (or chain-config struct) on `evm-wallet-core` adds a `Polygon` variant alongside `Ethereum`.

**Risks (reused from ETH doc):**
- **MSRV drift:** Pin alloy 1.8.x for v0.1 (same MSRV parity as ETH). Re-evaluate 2.x at v0.3.
- **Heavy default features:** Use sub-crates individually (same pattern as ETH).
- **Default fillers:** Pass signer explicitly at send-time (not auto-wallet filler).

### `alloy-signer-local` (reused)

**Polygon-specific:**
- **Derivation path: `m/44'/60'/0'/0/0`** — reuses ETH SLIP-44 coin type 60. Polygon does NOT have its own SLIP-44 entry (verified via `github.com/satoshilabs/slips/blob/master/slip-0044.md`). Same mnemonic, same address derivation as ETH.
- **Signing format: identical to ETH** (k256 ECDSA, keccak256 tx-hash, EIP-155 / EIP-1559 replay protection via `chain_id` field)
- **EIP-712 typed data: chain_id always included** (per Q7 resolution) — `sign_typed_data_sync(&TypedData)` with domain separator carrying `chain_id: 137` prevents cross-chain replay

### `alloy-provider` + `alloy-transport-http` (reused)

**Polygon-specific:**
- `ProviderBuilder::new().connect_http("https://polygon-rpc.com".parse()?)` for mainnet
- `ProviderBuilder::new().connect_http("https://polygon-amoy.drpc.org".parse()?)` for Amoy
- **SPKI pinning** reuses `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` (F20 finding) — same `rustls` version, same pin format. **Same `pinned://<pin>@polygon-rpc.com` CLI URL scheme** as BTC/ETH.
- **Rate limits** (per `docs.polygon.technology/pos/reference/rpc-endpoints`): `https://polygon-rpc.com` public RPC = ~100 requests/10s (no API key, may rate-limit). Alchemy/Infura recommended for production (operator supplies key).

### `bip32` + `bip39` (reused)

Identical to ETH. No Polygon-specific changes.

### Local dev testnet (Anvil Polygon-fork)

For v0.1 unit + integration tests, use **Anvil (Foundry) in Polygon-fork mode**.
Pattern: `AnvilInstance::new().spawn()` via `alloy-node-bindings` (already in
`evm-wallet-core`'s `[dev-dependencies]`) returns a running node + 10 prefunded
accounts and preserves Polygon mainnet state at the forked block. Existing
wiring reference: `rust-wallet-app/crates/evm-wallet-core/tests/erc20_anvil.rs`.

Trade-offs, alternative comparison (Anvil vs `polygon-cli`/`bor` vs
testcontainers), and the spike V9 use-case validation all live in the plan:

[`docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 0.0](../superpowers/plans/2026-08-27-polygon-wallet-core.md)

## Alternatives considered (and why rejected)

Already covered in §"Why EVM-reuse (Option A)". Consolidated for the at-a-glance view:

| Alternative | Why rejected |
|---|---|
| **B — `eth-wallet-core` with chain-config enum** | Name drifts (`eth-wallet-core` carries non-ETH chains). Enums grow ugly. |
| **C — Standalone `polygon-wallet-core` (TRON-shape)** | ~80% code duplication with eth-wallet-core. Maintenance cost doubles per chain. |
| `ethers-rs` | Officially deprecated 2024-06. Use alloy. |
| Raw `reqwest` + hand-rolled RLP | Loses alloy type-safety. Hundreds of LOC. |
| `web3` (Parity) | Last release 2023. Not maintained. |
| Hand-rolled Keccak-256 | Don't. `tiny-keccak` (already workspace) is the standard. |
| Polygon-specific SLIP-44 coin type | None exists. Polygon reuses ETH coin type 60. |
| Mumbai testnet | Replaced by Amoy 2024-01. Goerli-rooted, deprecated. |
| Polygon zkEVM | Out of scope per Q2 (PoS only for v0.1). |
| Bridged `USDC.e` | Footgun — use **native** Circle USDC `0x3c499c...3359`. |
| Legacy Type 0 transactions | Per Q5: EIP-1559 only for v0.1. |
| Hardware wallet (Ledger, Trezor) | Out of scope per Q6. Defer to v0.2. |
| Smart contract deployment via wallet | Out of scope. Sign-only + broadcast external path is enough. |
| L2 DEX integration (Uniswap, QuickSwap) | Out of scope. |

## Open questions — all resolved

| Q | Resolution | Source |
|---|---|---|
| **Q1: EVM-reuse strategy** | Option A — refactor `eth-wallet-core` → `evm-wallet-core` + thin `eth` + `polygon` wrappers | issue #416 architecture table |
| **Q2: PoS only or include zkEVM?** | **PoS only** for v0.1. zkEVM deferred to v0.2 (different chain-id 1101, different RPC, separate token registry) | issue #416 |
| **Q3: Token registry source** | Static bundled JSON (`polygon-wallet-core/tokens/mainnet.json` + `amoy.json`) — matches TRON/ETH shape | issue #416 |
| **Q4: RPC provider default** | `polygon-rpc.com` primary (mainnet), `polygon-amoy.drpc.org` fallback (Amoy), no Alchemy key bundled | `docs.polygon.technology/pos/reference/rpc-endpoints` |
| **Q5: Gas pricing — EIP-1559 only or legacy fallback?** | **EIP-1559 only** (Type 2 transactions). London hardfork active since 2022-01-18 | `forum.polygon.technology/t/impact-of-eip1559-and-future-possibilities/1749` |
| **Q6: Hardware wallet integration scope** | Defer to v0.2 (`alloy-signer-ledger` / `alloy-signer-trezor` available when needed) | issue #416 |
| **Q7: Signature replay protection** | Always include `chain_id` in EIP-712 typed data + use `with_chain_id(137)` in `TransactionRequest` (replay protection for tx + typed data) | alloy built-in |
| **Q8: POL (post-MATIC rebrand) display** | Display "POL". Keep "MATIC" alias available for legacy wallet UX | `polygon.technology/blog/matic-to-pol-migration-is-now-live-everything-you-need-to-know` |

## Network + TLS pinning (mirrors ETH design)

Same two-scenario model as `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` §"Network + TLS pinning research":

- **Scenario A — Pin the RPC endpoint** (`pinned://<spki-sha256-hex>@polygon-rpc.com`): reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` (verified path at `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`). Same `rustls` version, same pin format.
- **Scenario B — No pin (system trust store + localhost)**: `RootProvider::new_http(rpc_url)` with system CAs. Acceptable for localhost / LAN / trusted-network deployments.

**Decision matrix:**

| Use case | Network | Pin? |
|---|---|---|
| Local dev | Anvil HTTP (or Polygon local fork) | No (Scenario B) |
| CI smoke test | Anvil HTTP | No (Scenario B) |
| Testnet smoke (Amoy) | Amoy HTTPS | Optional (Scenario A recommended on public WiFi) |
| Testnet smoke (LAN) | Amoy HTTPS | No (Scenario B acceptable) |
| Production wallet, real value | Polygon mainnet HTTPS | **Yes (Scenario A required)** |

Default CLI behavior: Scenario B. Operator opts into Scenario A via `pinned://` URL scheme.

## Verification (analogous to TRON V1–V10)

No implementation work in this session. The next-session spike (`rust-wallet-app/spikes/polygon-v1/`) must validate:

1. **V1 (Q1 EVM-reuse):** Confirm `evm-wallet-core` refactor builds + `polygon` wrapper compiles with zero new deps beyond ETH. Verify `cargo build -p evm-wallet-core -p eth-wallet-core -p polygon-wallet-core` clean (per L55 scope rule, run per-crate).
2. **V2 (chain-id switching):** `provider.get_chain_id()` returns `137` against `https://polygon-rpc.com` and `80002` against `https://polygon-amoy.drpc.org`.
3. **V3 (POL derivation):** `bip39::Mnemonic::parse_in(English, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")` → seed → `m/44'/60'/0'/0/0` → address matches canonical reference (MetaMask or ethers.js test vector). Verify address is identical to ETH derivation (same coin type 60, same path → same address on both chains).
4. **V4 (Q5 EIP-1559 estimation):** `provider.estimate_eip1559_fees()` against `polygon-rpc.com` returns `(max_fee_per_gas, max_priority_fee_per_gas)` in the expected range (30–500 gwei depending on congestion). Re-estimate twice 3 seconds apart, confirm values diverge (proves the 2-second block volatility).
5. **V5 (Q4 RPC connectivity):** `provider.get_block_number()` against `polygon-rpc.com` returns a sane value (~60M+ range as of 2026).
6. **V6 (Q3 token registry):** `tokens/mainnet.json` with 3 entries (USDC, USDT, DAI) + `tokens/amoy.json` with 1 entry (USDC Amoy). USDC decimals = 6 verified via `decimals()` selector.
7. **V7 (Q4 Amoy faucet):** Request Amoy POL from Polygon faucet (`https://faucet.polygon.technology/`), confirm receipt via `provider.get_balance(addr)`.
8. **V8 (Q5 native POL transfer):** Send 0.01 test POL on Amoy from one wallet to another, confirm recipient `get_balance()` reflects the change (minus gas). Track `gas_used`, `effective_gas_price`, `cumulative_gas_used` from receipt.
9. **V9 (Q3 ERC-20 stablecoin transfer):** Deploy `MockERC20` to Anvil (Polygon-equivalent via `alloy-node-bindings` AnvilInstance — chain-id 31337), transfer 100 mock tokens, confirm `balanceOf` on recipient.
10. **V10 (Q7 signature replay protection):** Sign an EIP-712 typed message on chain-id 137, verify replay attempt on chain-id 1 (Ethereum) fails with `InvalidSignature` (proves chain_id in domain separator prevents cross-chain replay).

If all 10 pass, the chosen crate surface + Option A refactor is confirmed for v0.1.

## Sources

### Polygon protocol + network

- Polygon developer docs: <https://docs.polygon.technology/>
- RPC endpoints (chain-id, RPC URLs, gas station): <https://docs.polygon.technology/pos/reference/rpc-endpoints>
- Building on Polygon (EVM-compat statement): <https://docs.polygon.technology/pos/get-started/building-on-polygon>
- EIP-1559 on Polygon: <https://docs.polygon.technology/pos/concepts/transactions/eip-1559>
- London hardfork activation (2022-01-18): <https://forum.polygon.technology/t/impact-of-eip1559-and-future-possibilities/1749>
- POL token spec: <https://docs.polygon.technology/pos/concepts/tokens/pol>
- MATIC → POL migration: <https://polygon.technology/blog/matic-to-pol-migration-is-now-live-everything-you-need-to-know>
- Save-the-date migration: <https://polygon.technology/blog/save-the-date-matic-pol-migration-coming-september-4th-everything-you-need-to-know>
- Ahmedabad hardfork: <https://polygon.technology/blog/polygon-pos-the-ahmedabad-upgrade-is-live-on-mainnet>
- Bor v1.4.0 release: <https://github.com/0xPolygon/bor/releases/tag/v1.4.0>
- Amoy testnet launch: <https://polygon.technology/blog/introducing-the-amoy-testnet-for-polygon-pos>
- Amoy + Cardona zkEVM testnets: <https://polygon.technology/blog/polygon-pos-and-polygon-zkevm-new-testnets-for-polygon-protocols>
- New testnet RPC + support: <https://support.polygon.technology/support/solutions/articles/82000907114-how-to-add-the-polygon-amoy-testnet-to-your-wallet>

### Stablecoins (ERC-20 on Polygon)

- Circle USDC addresses (canonical source): <https://developers.circle.com/stablecoins/usdc-contract-addresses>
- Polygon docs USDC transfer: <https://docs.polygon.technology/pos/payments/transfers/transfer-usdc/>
- USDT on Polygon (StableRegistry): <https://stableregistry.com/contracts/usdt-on-polygon/>
- DAI on Polygon (StableRegistry): <https://stableregistry.com/contracts/dai-on-polygon/>
- PolygonScan USDC native contract: <https://polygonscan.com/address/0x3c499c542cef5e3811e1192ce70d8cc03d5c3359>
- PolygonScan Amoy USDC: <https://amoy.polygonscan.com/address/0x41e94eb019c0762f9bfcf9fb1e58725bfb0e7582>

### Polygon Improvement Proposals (PIPs)

- PIP-17 (POL token): <https://forum.polygon.technology/t/pip-17-polygon-ecosystem-token-pol/12912>
- PIP-19 (Polygon PoS native token → POL): <https://forum.polygon.technology/t/pip-19-update-polygon-pos-native-token-to-pol/12914>
- PIP-25 (POL total supply): <https://forum.polygon.technology/t/pip-25-adjust-pol-total-supply/13008>
- PIP-26 (validator rewards): <https://forum.polygon.technology/t/pip-26-transition-from-matic-to-pol-validator-rewards/13046>
- PIP-37 (Ahmedabad hardfork): <https://github.com/maticnetwork/Polygon-Improvement-Proposals/blob/main/PIPs/PIP-37.md>

### Rust crates + EVM ecosystem

- `alloy` crates.io: <https://crates.io/crates/alloy>
- `alloy` GitHub: <https://github.com/alloy-rs/alloy>
- `alloy` docs: <https://alloy.rs>
- `alloy` v1.0 announcement: <https://www.paradigm.xyz/2025/05/introducing-alloy-v1-0>
- `alloy` 0.1 release (predecessor of v1.0): <https://www.paradigm.xyz/2024/06/alloy-release>
- `ethers-rs` deprecation: <https://github.com/gakonst/ethers-rs/issues/2667>
- EVM-reuse strategy (this issue): issue #416 architecture table

### Standards + cross-references

- SLIP-0044 coin types (ETH = 60, **no Polygon-specific entry**): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- EIP-1559: <https://eips.ethereum.org/EIPS/eip-1559>
- EIP-20 (ERC-20 ABI): <https://eips.ethereum.org/EIPS/eip-20>
- EIP-712 typed structured data signing: <https://eips.ethereum.org/EIPS/eip-712>
- BIP-39 wordlist: <https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki>

### EVM deep-dive (cross-references)

- eth-wallet-core (companion, alloy reference): `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`
- bitcoin-wallet-core (SPKI pin pattern source): `docs/wallets/2026-08-05-bitcoin-rust-sdks-deep-dive.md`
- bitcoin `SpkiPinnedVerifier` source: `bitcoin-wallet-core/src/chain/spki.rs`
- eth-wallet-core SPKI pin localnet test: `rust-wallet-app/crates/eth-wallet-core/tests/spki_pin_localnet.rs`
- tron-wallet-core (sibling non-EVM chain template): `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md`

---

**Next steps (post-deep-dive):**

1. **Ticket B:** User-stories doc — `docs/wallets/2026-08-27-polygon-wallet-user-stories.md` (template: `docs/wallets/2026-08-23-eth-wallet-user-stories.md`).
2. **Ticket C:** Spike — `rust-wallet-app/spikes/polygon-v1/` with V1–V10 mapped to Q1–Q10, each PASS evidence (command output + SHA).
3. **Ticket D:** Plan — `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` derived from resolved Qs + verified spikes.
4. **Ticket E:** Refactor — `eth-wallet-core` → `evm-wallet-core` + thin `eth` wrapper + `polygon` wrapper crates.
5. **Ticket F:** PR + flip issue #416 checkboxes + L24 CHANGELOG entry.