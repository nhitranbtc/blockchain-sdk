# `polygon` Polygon PoS Wallet — User Stories

**Date:** 2026-08-27
**Companion to:** [Polygon Rust SDK deep-dive](2026-08-27-polygon-rust-sdks-deep-dive.md) + [ETH user-stories precedent](2026-08-23-eth-wallet-user-stories.md)
**Surface:** `polygon` CLI binary inside `rust-wallet-app/crates/polygon/` next to `btc/` + `eth/`. **Default network = Amoy testnet (chain id 80002).** Mainnet opt-in via `--network mainnet` (chain id 137). Anvil regtest opt-in via `--rpc-url http://localhost:8545` (chain id 31337).
**Architecture:** thin wrapper over `evm-wallet-core` (Option A refactor target under issue #416). All EVM primitives (signing, RPC, ABI, gas estimation) inherit from `eth-wallet-core` — only chain-id + RPC URL + native-token display differ.

**Wallet identity (mirrors eth-wallet-core v0.3 Story 12 + BTC v0.1 PR #81):** user-facing identifier = `--name` (string); internal `wallet_id` (UUID) generated at create time. Argon2id + AES-256-GCM encrypted mnemonic at rest.

**Polygon-specific deltas vs ETH:**
- **Native gas token = POL** (replaced MATIC on 2024-09-04; Ahmedabad hardfork finalized symbol change 2024-09-25). Display "POL", keep MATIC alias for legacy wallet UX (Q8 resolution).
- **SLIP-44 coin type 60 (reuses ETH)** — no Polygon-specific entry in SLIP-0044 master. Same `m/44'/60'/0'/0/0` derivation as ETH.
- **Block time 2s** (vs ETH 12s) — `max_fee_per_gas` must be re-estimated immediately before broadcast, NOT cached (Q5 resolution).
- **Chain-id 137 mainnet / 80002 Amoy** (vs ETH 1 / 11155111).
- **Default RPC**: `https://polygon-rpc.com` (mainnet), `https://polygon-amoy.drpc.org` (Amoy) — per Q4 resolution.
- **Token registry**: USDT + USDC + DAI on Polygon mainnet + USDC on Amoy (vs ETH USDT + USDC). All 6 decimals except DAI (18).
- **Bridge vs native USDC footgun**: native USDC `0x3c499c...3359` (Circle-issued), NOT bridged `USDC.e`.

Personas (mirrors ETH user-stories, adapted for Polygon context):

- **Alice** — Polygon power user. Manages wallets across EVM chains. Wants CLI control + scriptable commands + ERC-20 stablecoin transfers on Polygon.
- **Bob** — Developer integrating Polygon. Wants stable exit codes + JSON output + EIP-712 typed-data signing for automation (e.g., Permit signatures, DEX interactions on QuickSwap/Uniswap-v3-Polygon).
- **Carol** — First-time user. Wants clear prompts, warnings about mnemonic safety, no surprises, simple POL/USDC send/receive.

---

## Story → alloy sub-crate map

The Polygon wallet shares **100% of its alloy surface** with `eth-wallet-core`. The table below documents both directions: stories inherited verbatim from ETH (column "Inherited from ETH?"), and Polygon-specific deltas (Amoy faucet, POL display, gas-estimation cadence). Traceability between user-facing feature and the `evm-wallet-core` engine call lives in column "alloy sub-crate(s) used".

| # | Story | alloy sub-crate(s) used | `alloy_primitives` / `alloy_sol_types` primitive(s) | Inherited from ETH? |
|---|---|---|---|---|
| 1 | Create a new wallet | `alloy-signer-local::MnemonicBuilder`; `alloy-primitives::Address` | `Address::from_slice(&keccak256(pubkey)[12..])` | yes — same `m/44'/60'/0'/0/0` derivation |
| 2 | Import an existing wallet | `alloy-signer-local::MnemonicBuilder::phrase`; `alloy-primitives::Address` | `Address` derivation | yes |
| 3 | Check POL balance | `alloy-provider::Provider::get_balance` | `U256` (wei), `Address` | yes — only chain-id differs |
| 4 | Sync chain state | `alloy-provider::Provider::get_block_number`, `get_chain_id`, `get_transaction_count` | `U256`, `u64` (nonce), `u64` (chain id = 137 / 80002) | yes |
| 5 | Send native POL | `alloy-provider::Provider::send_transaction`; `alloy-signer-local::PrivateKeySigner::sign_transaction_sync` | `alloy_rpc_types::TransactionRequest` (EIP-1559 type 2), `alloy_consensus::TxEip1559`, `Signature` | yes |
| 6 | Send with custom EIP-1559 fee | `alloy_rpc_types::TransactionRequest::with_max_fee_per_gas`, `with_max_priority_fee_per_gas` | `U256` (wei/gas) | yes — but **re-estimate immediately per-broadcast** (2-second blocks) |
| 7 | Inspect transaction history | `alloy-provider::Provider::get_transaction_by_hash`, `get_transaction_receipt` | `alloy_rpc_types::Transaction`, `TransactionReceipt` (`block_number`, `gas_used`, `status`) | yes |
| 8 | Get current gas estimates | `alloy-provider::Provider::estimate_gas`, `estimate_eip1559_fees` (alloy-specific) | `U256` (gas units), `alloy_rpc_types::FeeHistory` | yes — **must call estimate_eip1559_fees() per-broadcast**, NOT cached |
| 9 | List / show / delete / rename wallets | `std::fs`; `chain-traits` registry | n/a | yes |
| 10 | Use mainnet explicitly | `alloy-provider::Provider::get_chain_id` (assert `0x89` = 137) | `u64` chain id | yes |
| 11 | Show config + debug info | `std::env`, `version()` | n/a | yes |
| 12 | Persist wallet across CLI invocations | filesystem + UUID-based wallet dir; encrypted mnemonic (Argon2id + AES-256-GCM) | n/a | yes |
| 13 | Send to multiple recipients (sequential txs) | N `send_transaction` calls | N × `TransactionRequest` | yes |
| 14 | Sweep / drain wallet to one address | `provider.get_balance` + `TransactionRequest::with_value(balance - gas_estimate)` | `U256` balance arithmetic | yes |
| 15 | Choose nonce strategy (auto vs manual) | `provider.get_transaction_count` (auto) vs caller-supplied `with_nonce` (manual) | `u64` nonce | yes |
| 16 | Manual nonce + gas limit override | `TransactionRequest::with_nonce`, `with_gas_limit` | `u64` nonce, `u64` gas limit | yes |
| 17 | Replace / speed-up tx (same nonce, higher fee) | new `TransactionRequest` with same `nonce` + higher `max_fee_per_gas` | `Signature` (different from original) | yes |
| 18 | Sign EIP-191 personal message | `alloy-signer-local::PrivateKeySigner::sign_message_sync` | `Signature`, recovered `Address` | yes |
| 19 | Export the wallet xpub + first addresses | `bip32::XPub::to_string` | `alloy_primitives::Address` (first 5 receive addresses) | yes |
| 20 | Pick derivation path (Ledger vs MetaMask) | `alloy-signer-local::MnemonicBuilder::derivation_path` | n/a | yes — same path `m/44'/60'/0'/0/0` (reuses ETH coin type 60) |
| 21 | Send ERC-20 stablecoin (USDT/USDC/DAI) | `alloy_sol_types::sol!`; `provider.send_transaction` with calldata | `transferCall { to: Address, value: U256 }`, `Bytes` (calldata), `Address` (token contract) | yes — contracts differ (USDC `0x3c499c...3359`, USDT `0xc2132D...e8F`, DAI `0x8f3Cf7...63`) |
| 22 | Check ERC-20 token balance | `sol! { function balanceOf(address) external view returns (uint256); }` + `provider.call(&req)` | `balanceOfCall`, `balanceOfReturn`, `U256` | yes |
| 23 | List registered stablecoins / tokens | token registry JSON in repo (`polygon-wallet-core/tokens/mainnet.json` + `amoy.json`) | `Address`, `U256` decimals, `String` symbol | yes — different token lists vs ETH |
| 24 | Add custom ERC-20 token by contract address | `sol! { function decimals() ...; function symbol() ...; }` + `provider.call` | `U256`, `String` | yes |
| 25 | Approve ERC-20 spending (for QuickSwap etc.) | `sol! { function approve(address spender, uint256 value) external returns (bool); }` + `provider.send_transaction` | `approveCall`, `Bytes` calldata | yes |
| 26 | Use Anvil local node for testing | `--rpc-url http://localhost:8545`; `provider.get_chain_id()` asserts `0x7a69` (31337) | `u64` chain id | yes |
| 27 | Sign EIP-712 typed data | `alloy-signer-local::PrivateKeySigner::sign_typed_data_sync` | `alloy_primitives::eip712::TypedData`, `Signature` | yes — **chain_id must be 137 (or 80002) in domain separator** to prevent replay vs ETH (Q7 resolution) |
| 28 | Connect to RPC with SPKI pin | reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` | n/a | yes (same `pinned://<spki>@polygon-rpc.com` scheme) |
| 29 | Connect to RPC without SPKI pin (system CAs) | `RootProvider::new_http(rpc_url)` with system trust store | n/a | yes — Scenario B (localhost / LAN) |
| 30 | Request Amoy testnet POL from faucet | HTTP GET/POST to `https://faucet.polygon.technology/` | n/a | NEW — Polygon-specific (no equivalent on ETH) |
| 31 | Display POL gas-token balance with MATIC alias | native UI affordance, no alloy surface | `String` display | NEW — Polygon-specific (Q8 resolution) |
| Cross-cutting | `--json` everywhere; stable exit codes; no daemons | `serde_json`; `std::process::ExitCode` | n/a | yes |
| Cross-cutting | `Secret<Mnemonic>` zeroize | `zeroize::Zeroizing<Mnemonic>` (mirror Bitcoin Task 30) | n/a | yes |
| Cross-cutting | EIP-55 checksum address display | `alloy_primitives::Address::to_checksum_buffer(None)` | `Address` | yes |

**Layer separation summary** (inherited from ETH doc §"Layer separation summary" — repeated here for completeness):

- **`alloy_primitives`** = value types (Address, U256, Bytes, FixedBytes). The EVM analogue of `rust-bitcoin`'s script + address + amount types.
- **`alloy_sol_types` + `sol!` macro** = Solidity ABI encode/decode (typed `sol! { function transfer(...) }` blocks + auto-generated `*Call`/`*Return` types).
- **`alloy-provider` + `alloy-transport-http`** = JSON-RPC client.
- **`alloy-signer-local`** = local signer (BIP-39 → `PrivateKeySigner`, k256 by default).
- **`alloy_consensus` + `alloy_rpc_types`** = transaction envelopes (EIP-1559 type 2) + RPC request/response types.
- **`alloy-chains`** = `Chain::Polygon`, `Chain::PolygonAmoy` enums (NEW in Polygon wrapper).
- **`bip32` + `bip39` (reuse from Bitcoin + ETH side)** = HD derivation + mnemonic. **Identical primitives, identical path** — only the RPC URL and chain-id constant differ.

## alloy use case coverage

Cross-check: every use case that alloy provides natively (per the deep-dive §Crate-by-crate deep-dive) mapped to the user story that exercises it. Inherited from ETH doc (the alloy surface is identical); added 2 Polygon-specific rows for new Stories 30 + 31.

| # | alloy use case | User story that exercises it | Covered? |
|---|---|---|---|
| 1 | Build a value type (`Address`, `U256`, `Bytes`) | Story 1 (address derivation), Story 21 (calldata), Story 24 (decimals) | ✅ |
| 2 | `Address::to_checksum_buffer` (EIP-55) | Cross-cutting (every address displayed) | ✅ |
| 3 | Build an EIP-1559 tx (`TxEip1559`, `TransactionRequest::with_*`) | Story 5 + 6 + 14 + 16 + 17 | ✅ |
| 4 | Sign a tx with `PrivateKeySigner` | Story 5 + 17 + 21 + 25 | ✅ |
| 5 | Provider RPC: `get_balance` | Story 3 + 14 + 22 | ✅ |
| 6 | Provider RPC: `get_transaction_count` (nonce) | Story 4 + 15 | ✅ |
| 7 | Provider RPC: `estimate_gas` | Story 8 + 14 | ✅ |
| 8 | Provider RPC: `estimate_eip1559_fees` (`max_fee_per_gas` + `max_priority_fee_per_gas`) | Story 6 + 8 — **must be called per-broadcast on Polygon** (2-second blocks) | ✅ |
| 9 | Provider RPC: `get_block_number`, `get_chain_id` | Story 4 + 10 + 26 | ✅ |
| 10 | Provider RPC: `send_transaction` | Story 5 + 14 + 17 + 21 + 25 | ✅ |
| 11 | Provider RPC: `get_transaction_by_hash`, `get_transaction_receipt` | Story 7 | ✅ |
| 12 | `sol!` macro for inline ABI typing | Story 21 + 22 + 24 + 25 | ✅ |
| 13 | Raw `provider.call(&req)` for view calls | Story 22 + 24 | ✅ |
| 14 | Sign EIP-191 personal message | Story 18 | ✅ |
| 15 | Sign EIP-712 typed data | Story 27 — **must include `chain_id: 137` in domain separator** (Q7 cross-chain replay protection) | ✅ |
| 16 | `Signature::recover_address_from_msg` | Story 18 | ✅ |
| 17 | `alloy_chains` chain metadata (`Chain::Polygon`, `Chain::PolygonAmoy`) | Story 10 + 26 | ✅ |
| 18 | `MnemonicBuilder::derivation_path` override | Story 20 | ✅ |
| 19 | `PendingTransactionBuilder::get_receipt` | Story 5 + 17 | ✅ |
| 20 | `alloy_node_bindings::AnvilInstance` | Story 26 (as `[dev-dependencies]`) | ✅ |
| 21 | RLP encoding/decoding | internal to all tx + ABI paths | ⚠️ internal |
| 22 | Keccak-256 hashing | internal to address derivation + ABI selectors | ⚠️ internal |
| 23 | EIP-2930 (type 1) + EIP-4844 (type 3) | not in v0.1 scope (EIP-1559 = type 2 is sufficient for Polygon + ERC-20) | ⚠️ defer |
| 24 | ENS resolution | out of scope | ⚠️ defer (v1.x) |
| 25 | Hardware signer | out of scope | ⚠️ defer (v1.x) |
| 26 | MEV protection | out of scope (Polygon has Flashbots-style MEV auction via private RPCs but not Alloy-supported yet) | ⚠️ defer (v1.x) |
| 27 | WebSocket subscriptions | not in v0.1 scope (CLI is request/response) | ⚠️ defer (post-v1.x) |
| 28 | Gas-filler + nonce-filler auto-management | Story 15 (rejected — we use explicit nonce) | ✅ (as "we don't use this") |
| **29** | **Polygon mainnet RPC** (`polygon-rpc.com` default) | Story 10 | ✅ |
| **30** | **Amoy testnet RPC** (`polygon-amoy.drpc.org` default) | Story 30 + Story 4 | ✅ |

**Coverage summary (Polygon-specific delta vs ETH):**
- **30 of 30 use cases directly covered** (28 inherited + 2 new Polygon-specific rows for Amoy RPC + mainnet RPC)
- **3 use cases internal-only** (RLP, Keccak-256, alloy-node-bindings AnvilInstance as dev-dep)
- **4 deferred to v1.x** (ENS, hardware, MEV, WebSocket subs)
- **0 explicitly rejected**

**No alloy use cases required by v0.1 Polygon are missing from the user stories.**

---

## Story 1 — Create a new wallet (Alice)

> As Alice, I want to generate a new Polygon (Amoy) wallet from a single command, so I can start receiving POL + ERC-20 in under 10 seconds.

**Acceptance criteria (Polygon-specific deltas in bold):**

- `polygon wallet create --name test-wallet` runs in <1s on a developer laptop.
- Output shows: 12-word BIP-39 mnemonic, first receive address (EIP-55 checksum), **chain id (137 or 80002) + network name (Polygon mainnet / Polygon Amoy)**, wallet name, **native gas token display = "POL" (with MATIC alias hidden behind `--legacy-token-symbol` flag for v0.2+)**.
- **Derivation path = `m/44'/60'/0'/0/0`** — **same as ETH** (Polygon reuses SLIP-44 coin type 60). Documented prominently so users don't accidentally diverge from their ETH address space.
- Mnemonic written encrypted to `~/.local/share/polygon/test-wallet/mnemonic.enc` (Argon2id + AES-256-GCM, mirror BTC v0.1 + ETH v0.3).
- **WARNING line** before mnemonic: `Backup this 12-word phrase. Same mnemonic generates the same address on Ethereum mainnet + Polygon mainnet + Polygon Amoy (EVM-reuse). Funds at risk if lost.`
- Command exits 0 on success, non-zero on filesystem error.
- Running the command twice with the same name fails with exit code 2.

**Options:**

- `--network mainnet|amoy|anvil` (default `amoy`)
- `--rpc-url <URL>` (default `https://polygon-amoy.drpc.org` for amoy, `https://polygon-rpc.com` for mainnet, `http://localhost:8545` for anvil)
- `--derivation-path <PATH>` (default `m/44'/60'/0'/0/0`)
- `--account-index <N>` (default 0)

---

## Story 2 — Import an existing wallet (Alice)

> As Alice, I want to import a Polygon-compatible wallet from an existing 12/24-word mnemonic (e.g., from MetaMask or Ledger), so I can recover access to a wallet I created elsewhere.

**Acceptance criteria (Polygon-specific delta in bold):**

- `polygon wallet import --name recovered --mnemonic "word1 word2 ... word12" --network amoy` accepts a valid mnemonic.
- `polygon wallet import --name dev --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` imports a raw secp256k1 key.
- **Imported wallet produces the same first address as the source** (verified by deterministic derivation; same mnemonic → same ETH + Polygon address because SLIP-44 coin type 60 is shared).
- BIP39 passphrase supported.
- Output does **not** echo the mnemonic.
- Supports 12, 15, 18, 21, 24-word mnemonics.

---

## Story 3 — Check POL balance (Alice, Carol)

> As Carol, I want to see my confirmed POL balance on Polygon mainnet in a single line, so I know how much I can spend right now.

**Acceptance criteria:**

- `polygon wallet balance --address 0xAbC...123 --network mainnet` prints `address=0xAbC...123 balance_wei=1234567890000000000` (or `balance_pol=1.2345` with `--unit pol|wei`).
- **Note (Q8 resolution):** `--unit pol` displays `1.2345 POL`; deprecated alias `--unit matic` still accepted with deprecation warning: `Warning: 'matic' is deprecated; use 'pol' (MATIC was upgraded to POL on 2024-09-04)`.
- First run after wallet creation shows `balance_pol=0`.
- If RPC fails, command retries once, then prints `rpc failed: <reason>` and exits 3.
- Reads balance from a single `eth_getBalance` RPC call.
- Output balance in wei by default; `--unit pol|wei` (default `wei`); `--human` prints `1.2345 POL`.

---

## Story 4 — Sync chain state (Alice)

> As Alice, I want to force a full chain sync, so I can see incoming POL + ERC-20 transactions on Polygon that arrived since the last sync.

**Acceptance criteria:**

- `polygon wallet sync --address 0xAbC...123 --network amoy` connects to the JSON-RPC endpoint and pulls fresh chain state.
- Output: `block_number=<N> chain_id=<ID: 80002> nonce=<N>`.
- Exit 0 on success. Exit 3 on RPC failure.
- **Note:** Polygon's 2-second blocks mean `block_number` advances every 2s. Sync is effectively continuous.

---

## Story 5 — Send native POL (Alice)

> As Alice, I want to send 0.01 POL to an Amoy address with a known fee tier, so I can complete a payment in one command.

**Acceptance criteria (Polygon-specific deltas in bold):**

- `polygon wallet send --name w --password p --network amoy --to 0xAbC...123 --amount 0.01 --rpc-url <URL>` builds, signs (EIP-1559 type 2), and broadcasts.
- **Default fee tier = `half_hour`** (target ~3 blocks = ~6s on Polygon). Override via `--fee fastest|half_hour|hour|economy` or `--max-fee-gwei 60 --priority-fee-gwei 30`.
- **Output on success:** `sent. tx_hash: 0xDeF...456 (nonce: 7, gas_used: 21000, max_fee: 60 gwei, priority_fee: 30 gwei, network: amoy)`.
- Exit 0 on broadcast. Exit 4 on insufficient POL balance. Exit 5 on signing/RPC error.
- `--dry-run` builds + signs but does not broadcast.
- `--wait` waits for the tx to be mined and prints the receipt.
- EIP-1559 default: `max_fee_per_gas = base_fee + priority_fee`, `max_priority_fee_per_gas = priority_fee`.
- **`provider.estimate_eip1559_fees()` called immediately before broadcast** — not cached, because Polygon's 2-second block time makes cached `baseFee` stale within seconds.

---

## Story 6 — Send with custom EIP-1559 fee (Bob)

> As Bob, I want to specify `max_fee_per_gas` and `max_priority_fee_per_gas` in gwei, so I can match my back-end's fee policy exactly.

**Acceptance criteria:**

- `polygon send --name w --password p --to 0xAbC... --amount 0.01 --max-fee-gwei 100 --priority-fee-gwei 50` builds an EIP-1559 tx with exact values.
- Validation: `--priority-fee-gwei` must be `<= --max-fee-gwei` (exit 2).
- **Polygon-specific:** Typical fee tier on Polygon = 30–500 gwei (depending on congestion). Hard cap `--max-fee-gwei 5000` (exit 2 if exceeded). This is ~100x ETH mainnet (which caps at ~100 gwei) — reflects Polygon's higher gas demand.
- Output includes effective values used.

---

## Story 7 — Inspect transaction history (Alice)

> As Alice, I want to list my past POL transactions on Polygon mainnet with confirmations, so I can reconcile my records.

**Acceptance criteria:**

- `polygon tx list --address 0xAbC... --network mainnet --since-block 60000000` queries historical blocks and prints a table: `tx_hash | direction | amount | gas_used | confirmations | block_number | timestamp`.
- Default 25 most recent txs. `--limit N` overrides.
- `--json` outputs JSON array.
- Unconfirmed txs show `confirmations: 0` with `pending` tag (requires `--pending` flag).
- `polygon tx get --tx-hash 0xDeF... --network mainnet` returns full details.

---

## Story 8 — Get current gas estimates (Bob, Carol)

> As Carol, I want to see the current gas tiers on Polygon before sending, so I can pick the right speed/cost trade-off.

**Acceptance criteria (Polygon-specific deltas in bold):**

- `polygon fee --network mainnet --rpc-url <URL>` prints a table:

  ```text
  fastest:     500 gwei  (base: 250, priority: 250)   # ~2s confirmation
  half_hour:   320 gwei  (base: 250, priority: 70)    # ~6s
  hour:        280 gwei  (base: 250, priority: 30)    # ~10s
  economy:     260 gwei  (base: 250, priority: 10)    # ~30s
  ```

- Tier derivation (per ETH Story 8, adapted for Polygon's 2-second blocks): `fastest` = 95th percentile, `half_hour` = 80th, `hour` = 70th, `economy` = 50th — over last 20 blocks via `eth_feeHistory`.
- **`base_fee` update rate = 12.5% per 2-second block** (same formula as ETH but **6× more frequent** — `baseFee` doubles in ~12s vs ~60s on ETH).
- Output refreshed on every call (live fetch, NOT cached).
- Exit 3 on RPC failure.

---

## Story 9 — List / show / delete / rename wallets (Alice, Bob) **[v0.3 — per eth-wallet-core Story 12]**

> As Alice, I want to list all my Polygon wallets and manage them, so I can pick one quickly.

**Acceptance criteria:**

- `polygon wallet list --network mainnet` prints one wallet name per line.
- `polygon wallet list --json` outputs JSON array of `{name, network, address, derivation_path, created_at}`.
- `polygon wallet show --name w --network mainnet` prints full info: network, chain id (137), address (EIP-55), derivation path, wallet name, file path, created_at.

---

## Story 10 — Use mainnet explicitly (Alice)

> As Alice, I want to create and use a Polygon mainnet wallet, so I can transact with real POL.

**Acceptance criteria (Polygon-specific deltas in bold):**

- `polygon wallet create --name main --network mainnet` produces an EIP-55 checksummed address (`0xAbCd...123`).
- `provider.get_chain_id()` is called at startup and must return `0x89` (137) — fails fast with exit 3 if the RPC endpoint disagrees.
- Default RPC URL for mainnet = `https://polygon-rpc.com`.
- Confirmation prompt: `This wallet uses real POL on Polygon mainnet. Type 'yes' to confirm.`
- Output shows `WARNING: this wallet uses real POL on Polygon mainnet (chain id 137). Funds are at risk.` before the mnemonic.
- **Note:** **The same mnemonic + derivation path produces the same address on ETH mainnet + Polygon mainnet + Polygon Amoy** (because all three use SLIP-44 coin type 60). User warning: `Importing this mnemonic into `eth wallet import` produces the same address. If you funded an ETH address with this mnemonic, the Polygon wallet shares the funds' control key (NOT the funds — those stay on their respective chains).`

---

## Story 11 — Show config + debug info (Bob)

> As Bob, I want to see what the CLI thinks the current network and RPC URL are, so I can debug "why is this connecting to the wrong place".

**Acceptance criteria:**

- `polygon config show` prints: data dir, RPC URL, network, chain id (after `--rpc-url` lookup), list of loaded wallets, derivation path default (must show `m/44'/60'/0'/0/0`), default fee tier.
- `polygon config show --json` outputs the same as JSON.
- Exit 0 always.
- Diagnostic output includes the version string + `alloy 1.8.x` + `evm-wallet-core 0.1.x`.

---

## Story 12 — Persist wallet across CLI invocations (Alice) **[v0.3 — per eth-wallet-core Story 12]**

> As Alice, I want each CLI invocation to find my wallets without re-deriving from the mnemonic, so it's fast.

**Acceptance criteria (Polygon-specific delta in bold):**

- First `polygon wallet create` writes encrypted mnemonic to `$XDG_DATA_HOME/polygon/wallets/<network>/<wallet_id>.enc` per ADR 0001 (UUID wallet id, like BTC v0.1 + ETH v0.3).
- Argon2id + AES-256-GCM — no plaintext on disk.
- Subsequent `polygon wallet show --id <wallet_id> --network mainnet` prompts unlock, reads encrypted mnemonic, derives keys, syncs via RPC, prints wallet state.
- **Note:** `evm-wallet-core` reuses ETH's `WalletManager` (same Argon2id params + AES-256-GCM nonce format). No new crypto code — wrapper just routes to the shared `WalletManager` with `Network::Polygon` config.

---

## Story 13 — Send to multiple recipients (sequential txs) (Alice)

> As Alice, I want to send POL to several addresses in quick succession, so I can pay multiple recipients without re-typing the command.

**Acceptance criteria:**

- `polygon send --name w --password p --batch <file>` reads a CSV file (format = `address,amount_pol` per line) and sends each as a separate transaction.
- Up to 100 recipients per `--batch`.
- Output: `batch sent. count: 3, txs: [0xAbC..., 0xDeF..., 0x123...], total: 0.05 POL, total_fees: 0.00021 POL`.
- Default fee tier `half_hour`. Override via `--fee` or `--max-fee-gwei`.
- `--stop-on-error` (default) or `--continue`.

---

## Story 14 — Sweep / drain wallet to one address (Alice)

> As Alice, I want to sweep my entire POL balance to one address, so I can consolidate funds before re-organizing my wallets.

**Acceptance criteria:**

- `polygon send --name w --password p --drain --to 0xAbC...` builds a transaction that sends `balance - gas_estimate` POL.
- Output: `drained. tx_hash: 0xDeF... (sent: 1.2345 POL, fee: 0.00021 POL, leftover: < 1 wei)`.
- **Polygon-specific:** `gas_estimate` uses **`provider.estimate_gas` + `provider.estimate_eip1559_fees()` called immediately before broadcast** (2-second block volatility). Same pattern as Story 5.

---

## Story 15 — Choose nonce strategy (auto vs manual) (Bob)

> As Bob, I want to pick between auto-nonce and manual nonce, so I can match my back-end's tx-batching policy.

**Acceptance criteria:** Inherited from ETH Story 15 (same auto/manual nonce semantics). **Polygon-specific note:** the nonce increments per chain (mainnet nonce ≠ Amoy nonce ≠ ETH nonce), even with the same address — confirm at sign-time via `provider.get_transaction_count(signer.address(), BlockTag::Pending)`.

---

## Story 16 — Manual nonce + gas limit override (Bob)

> As Bob, I want to supply an exact nonce and gas limit, so I can replace or compose transactions deterministically.

**Acceptance criteria:** Inherited from ETH Story 16. **`--gas-limit` floor on Polygon = 21,000** (same as ETH, since native POL transfer has identical intrinsic gas).

---

## Story 17 — Replace / speed-up tx (same nonce, higher fee) (Alice)

> As Alice, I want to speed up a stuck transaction by sending a new one with the same nonce but a higher fee, so I don't have to wait for the original to drop.

**Acceptance criteria (Polygon-specific delta in bold):**

- `polygon send speed-up --tx-hash 0xAbC... --max-fee-gwei 100 --priority-fee-gwei 60` reads the original tx, extracts its nonce + to + value + input, builds a new tx with same nonce + higher fee.
- **`--max-fee-gwei` must be `>` the original tx's `max_fee_per_gas`**, AND the new `base_fee + priority_fee` must exceed the original effective fee. Polygon validators accept same-nonce + higher-fee replacement within a few blocks (~30s typical = 15 blocks).
- Output: `sped up. new_tx_hash: 0xDeF... (old_tx_hash: 0xAbC..., nonce: 7, fee_increase: 40 gwei, network: mainnet)`.
- Exit 5 if the original tx is already mined and confirmed.

---

## Story 18 — Sign EIP-191 personal message (Alice)

> As Alice, I want to sign a message with my wallet's private key (EIP-191 `personal_sign`), so I can prove ownership of a Polygon address.

**Acceptance criteria:** Inherited from ETH Story 18. **Polygon-specific delta:** verify that the recovered address matches **either** the ETH address **or** the Polygon address — they are the same address, but the verifier might display context differently. Wallet displays recovered address without chain prefix (it's just an address).

---

## Story 19 — Export the wallet xpub + first addresses (Bob)

**Acceptance criteria:** Inherited from ETH Story 19. Note that the xpub for the same mnemonic is **identical** across ETH + Polygon (because SLIP-44 coin type 60 is shared); wallet displays the same xpub whether created via `eth` or `polygon` CLI.

---

## Story 20 — Pick derivation path (Ledger vs MetaMask) (Alice)

**Acceptance criteria (Polygon-specific delta in bold):**

- `polygon wallet create --name w --derivation-path m/44'/60'/0'/0/0` (Ledger-style, default).
- **`--derivation-path`, `--address-index`, `--account-index` accept the same path as ETH** — `m/44'/60'/...` (NOT `m/44'/9660'/...` — no Polygon-specific SLIP-44 coin type exists).
- Validation: path must start with `m/44'/60'/`. Exit 2 with `derivation path must start with m/44'/60/' (Polygon reuses ETH coin type 60, not a separate SLIP-44 entry)`.

---

## Story 21 — Send ERC-20 stablecoin on Polygon (Alice)

> As Alice, I want to send 1.50 USDC to a recipient on Polygon mainnet with a single command, so I can pay in stablecoin without worrying about POL price volatility.

**Acceptance criteria (Polygon-specific deltas in bold):**

- `polygon erc20 send --name w --password p --token USDC --to 0xAbC... --amount 1.5 --network mainnet` builds + signs + broadcasts a `transfer(address,uint256)` call to the USDC contract.
- **`--token USDC` resolves to `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` (native Circle-issued USDC on Polygon mainnet)** — NOT the bridged `USDC.e` address. Wallet label = `USDC` (NOT `USDC.e`). **Bridge footgun warning** printed on first USDC use: `Using native Circle USDC. Bridged USDC.e is NOT supported in v0.1.`
- **`--token USDT` resolves to `0xc2132D05D31c914a87C6611C10748AEb04B58e8F`** (6 decimals).
- **`--token DAI` resolves to `0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063`** (18 decimals).
- `--amount 1.5` interpreted as `1.5 * 10^decimals` base units. USDC/USDT use 6 decimals, DAI uses 18 decimals. CLI fetches `decimals()` once per token (cached).
- Tx shape: `to = token_contract`, `value = 0`, `input = transferCall { to: recipient, value: amount_base_units }.abi_encode()`.
- Gas limit auto-estimated via `provider.estimate_gas`; override via `--gas-limit 100000`.
- Output: `sent. tx_hash: 0xDeF... (token: USDC, amount: 1.5, decimals: 6, gas_used: 65000, network: mainnet)`.

---

## Story 22 — Check ERC-20 token balance on Polygon (Alice, Carol)

> As Carol, I want to see my USDC, USDT, and DAI balances next to my POL balance on Polygon, so I know my total stablecoin holdings.

**Acceptance criteria (Polygon-specific delta in bold):**

- `polygon wallet balance --address 0xAbC... --token <USDC/USDT/DAI> --network mainnet` prints `<scaled> <token-symbol>`.
- Symbol resolution: `USDC` → `0x3c499c...3359`, `USDT` → `0xc2132D...e8F`, `DAI` → `0x8f3Cf7...63` (mainnet).
- Amoy: `USDC` → `0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582`. USDT/DAI not on Amoy in v0.1 (use `--token-address` for custom Amoy tokens).
- `--all` iterates bundled registry (USDC, USDT, DAI on mainnet; USDC on Amoy).

---

## Story 23 — List registered stablecoins / tokens on Polygon (Bob)

> As Bob, I want to see the list of supported ERC-20 tokens on Polygon, so I know which ones I can `--token SYMBOL` against.

**Acceptance criteria (Polygon-specific deltas in bold):**

- `polygon token list --network mainnet` prints a table:

  ```text
  SYMBOL  ADDRESS                                     DECIMALS  NETWORK
  USDC    0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359    6      mainnet
  USDT    0xc2132D05D31c914a87C6611C10748AEb04B58e8F    6      mainnet
  DAI     0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063    18     mainnet
  USDC    0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582    6      amoy
  ```

- Tokens loaded from `polygon-wallet-core/tokens/mainnet.json` + `amoy.json` (bundled via `include_str!`).

---

## Story 24 — Add custom ERC-20 token by contract address (Alice)

**Acceptance criteria:** Inherited from ETH Story 24. **Polygon-specific delta:** `polygon erc20 register --address 0xDeF... --network mainnet` queries `decimals()` + `symbol()` + `name()` via raw `provider.call`, writes to `$XDG_CONFIG_HOME/polygon/tokens/<network>.json`.

---

## Story 25 — Approve ERC-20 spending (for QuickSwap etc.) (Bob)

**Acceptance criteria:** Inherited from ETH Story 25. **Polygon-specific note:** QuickSwap v3 router on Polygon mainnet = `0xf5b509bB0909a55B11b3Cdb41B0d322bD74bBf72` (for reference; not bundled — user supplies `--spender`).

---

## Story 26 — Use Anvil local node for testing (Alice)

**Acceptance criteria:** Inherited from ETH Story 26. `--rpc-url http://localhost:8545` works against Anvil (chain-id 31337). **Polygon-specific note:** for Polygon-fork testing, use `--fork-url https://polygon-rpc.com` against anvil — preserves Polygon state at the forked block.

---

## Story 27 — Sign EIP-712 typed data (Bob) **[v0.1 — per Q7 resolution]**

> As Bob, I want to sign EIP-712 typed structured data (e.g., a `Permit` message for gasless approvals on Polygon, or a QuickSwap `Order`), so I can interact with dApps that require typed-data signatures.

**Acceptance criteria (Polygon-specific delta in bold):**

- `polygon sign-typed --name w --password p --typed-data '<JSON>' --chain-id 137` parses JSON `TypedData` and signs with `PrivateKeySigner::sign_typed_data_sync`.
- **Domain separator MUST include `chainId: 137` (or `chainId: 80002` for Amoy)** to prevent cross-chain replay (e.g., an EIP-712 signature intended for Polygon must not be replayable on Ethereum mainnet, and vice versa).
- **Validation: reject `--chain-id 1` or any non-Polygon chain-id** with exit 2 (`chain_id must be 137 (mainnet) or 80002 (amoy)`). Cross-chain replay protection.
- Output: `address: 0xAbC...\ndigest: 0x...32bytes\nsignature: 0x...65bytes`.

---

## Story 28 — Connect to RPC endpoint with SPKI pin (Alice, Bob) **[v0.3.x — issue #393]**

**Acceptance criteria:** Inherited from ETH Story 28. **Polygon-specific delta:** `polygon --rpc-url pinned://<spki-hex>@polygon-rpc.com config show` parses the pin. Same SPKI pattern as ETH + BTC (file path: `bitcoin-wallet-core/src/chain/spki.rs`).

---

## Story 29 — Connect to RPC endpoint without SPKI pin (Alice) **[v0.1 — current default]**

**Acceptance criteria:** Inherited from ETH Story 29. **Polygon-specific delta:** `polygon --rpc-url http://127.0.0.1:8545` uses `RootProvider::new_http(rpc_url)` with system CAs. Default for localhost / LAN / Amoy dev.

---

## Story 30 — Request Amoy testnet POL from faucet (Alice, Bob) **[v0.1 NEW — Polygon-specific]**

> As Alice, I want to request free testnet POL for Amoy, so I can run end-to-end tests without spending real money.

**Acceptance criteria (Polygon-specific — NEW):**

- `polygon faucet --address 0xAbC... --network amoy` opens the browser to `https://faucet.polygon.technology/` with the address pre-filled, OR calls the faucet API if credentials are configured (`--faucet-token <TOKEN>` env var).
- Default: prints the URL + address (no automation) — operator completes the captcha + claim manually. Per L29 "live testnet smoke is operator-driven, not CI".
- `--auto` (with `--faucet-token`) POSTs to faucet API and waits for receipt.
- Exit 0 on URL display. Exit 3 on API failure.
- Output: `Visit https://faucet.polygon.technology/?address=0xAbC... to claim Amoy POL (5,000 POL per request, max 1 request per 24h per address)`.

---

## Story 31 — Display POL gas-token balance with MATIC alias (Alice, Carol) **[v0.1 NEW — Polygon-specific]**

> As Carol, I want to see my gas balance in "POL" (post-rebrand) but understand that legacy tools still call it "MATIC", so I'm not confused when I see both labels.

**Acceptance criteria (Polygon-specific — NEW):**

- All `polygon` CLI output that displays the native gas token uses the label **"POL"** by default.
- `--legacy-token-symbol` flag (off by default, on for `--network amoy` in v0.1 to ease migration) renames "POL" → "MATIC" in CLI output, preserving the pre-September-2024 mental model.
- Token registry JSON stores **both** keys for display flexibility: `{ "symbol": "POL", "legacy_symbol": "MATIC" }`.
- No on-chain implications — token contract address is unchanged; only the display string differs.
- **Note:** The token contract (`0x0000000000000000000000000000000000001010` on Polygon mainnet — precompile address, not an ERC-20) is identical across the MATIC→POL migration. Wallet does NOT need to track two tokens.

---

## Network research — local + testnet + mainnet picks

**Local node:** **Anvil** (`alloy_node_bindings::AnvilInstance`, foundry-rs/foundry). Reasons (inherited from ETH user-stories §"Network research"):

- Already in `[dev-dependencies]` of `eth-wallet-core` (mirror to `polygon-wallet-core`).
- Fastest startup (~50 ms), zero-config chain-id 31337, prefunded deterministic accounts.
- **Polygon-fork mode:** `anvil --fork-url https://polygon-rpc.com --fork-block-number 60000000` — preserves Polygon mainnet state at the forked block for local testing against real USDC/USDT/DAI contracts.

**Testnet:** **Polygon Amoy** (chain-id 80002). Reasons:

- **Only current Polygon PoS testnet** (replaces Mumbai as of 2024-01).
- Sepolia-rooted (parent chain) — Ethereum Sepolia faucets can also fund validator operations.
- Polygon docs (`docs.polygon.technology/pos/reference/rpc-endpoints`) confirm Amoy is the canonical testnet for v0.1+.
- **Faucet:** `https://faucet.polygon.technology/` — 5,000 POL per request, 1 request per 24h per address.
- **Block explorer:** `https://amoy.polygonscan.com/`.
- **Test USDC contract (Circle-issued):** `0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582`.

**Mainnet:** **Polygon PoS mainnet** (chain-id 137). Reasons:

- Dominant EVM-compatible L2 by TVL.
- ~$1B daily on-chain volume (per issue #416 Why section).
- Native POL token (post-MATIC rebrand 2024-09-04).
- EIP-1559 active since London hardfork (2022-01-18, block 23,850,000).

**Rejected alternatives:**

| Alternative | Why rejected |
|---|---|
| Mumbai testnet | Deprecated 2024-Q2 (Goerli-rooted). Use Amoy exclusively. |
| Polygon zkEVM | Out of scope per Q2 (PoS only for v0.1). Different chain-id 1101. |
| Polygon PoS testnet as alternative to Amoy | None exists. Amoy is the only testnet. |
| Bridged `USDC.e` on Polygon | Footgun — use **native** Circle USDC `0x3c499c...3359`. |
| Publicnode.com vs polygon-rpc.com | `polygon-rpc.com` (official Polygon Labs) is primary per Q4; publicnode.com as fallback. |

**Test matrix — Stories 28 + 29:**

| Scenario | Network | SPKI pin | Test name | Status |
|---|---|---|---|---|
| Local dev, no pin | Anvil (31337) | None | `no_pin_localhost_anvil_succeeds` | ready to implement |
| Local dev, pin match | Polygon HTTPS (mainnet) | Correct pin | `spki_pinned_polygon_mainnet_succeeds` | blocks on issue #393 |
| Testnet, no pin | Amoy (80002) | None | `no_pin_amoy_get_chain_id` | ready |
| Testnet, pin match | Amoy (80002) | Correct pin | `spki_pinned_amoy_succeeds` | blocks on issue #393 |
| Mainnet, pin match | Polygon mainnet (137) | Correct pin | `spki_pinned_polygon_mainnet_succeeds` | blocks on issue #393 |

---

## Cross-cutting acceptance criteria (apply to all stories)

- **Help text:** every command accepts `--help` and prints a clear, multi-line description with examples.
- **Exit codes (per ETH user-stories §"Cross-cutting"):** 0 = success, 1 = user abort, 2 = bad input, 3 = upstream/RPC transport failure, 4 = wallet/balance issue, 5 = signing/RPC/broadcast error.
- **Tx types (per Q5 resolution):** Reads accept legacy (type 0) + EIP-2930 (type 1) + EIP-1559 (type 2). Writes = **EIP-1559 only** in v0.1.
- **Mnemonic at rest:** encrypted with Argon2id + AES-256-GCM (reuses `eth-wallet-core::WalletManager`). Operator must run `polygon wallet unlock` (or set `POLYGON_WALLET_PASSPHRASE`) on every CLI call that touches key material.
- **Output:** human-readable by default; `--json` flag on every command.
- **Stderr for diagnostics:** logs/errors to stderr; stdout contains only the requested data.
- **No background processes:** every `polygon` invocation is a single foreground command. No daemons.
- **No telemetry:** the CLI makes no network calls except to the configured RPC URL (and optionally the faucet for Story 30).
- **Address unit:** all addresses display in EIP-55 mixed-case checksum. Override via `--address-format checksum|lowercase`.
- **Amount unit:** all amount flags accept POL (`--amount 0.5`) or wei (`--amount-wei 500000000000000000`); deprecated alias `--amount-matic` accepted with warning.
- **Gas unit:** all gas flags accept gwei (`--max-fee-gwei 60`) or wei (`--max-fee-wei 60000000000`). Polygon typical range: 30–500 gwei.
- **Confirmation prompts** (`mainnet`, `drain`, `unlimited approval`, faucet auto-claim): require typing `yes`. Default abort.
- **Bounded inputs:** batch limited to 100 recipients; `--gas-limit` bounded `>= 21000`; derivation path must start with `m/44'/60'/`.
- **TLS pinning:** `pinned://<spki-hex>@polygon-rpc.com` triggers SPKI verifier. Same path as ETH + BTC (`bitcoin-wallet-core/src/chain/spki.rs`).
- **Chain-id assertion:** `provider.get_chain_id()` called at startup; mismatch with `--network` flag fails fast (exit 3).
- **`evm-wallet-core` reuse:** all EVM primitives (signing, RPC, ABI, gas estimation) come from `evm-wallet-core`. Polygon wrapper adds ONLY: `Network::Polygon` enum variant + RPC URL default + token registry + POL display.

---

## Out of scope for v1

- **EIP-712 typed-data signing (Story 27)** — covered in v0.1 per Q7.
- **zkEVM chain support** (chain-id 1101). Different chain-id + RPC + token registry. Add via `Network::PolygonZkEvm` enum variant + another RPC URL. UX: `--network zkevm`.
- **ENS resolution** (`alice.eth`). `alloy-ens` sub-crate exists if added later.
- **Hardware wallet** (Ledger, Trezor, Keystone) via `alloy-signer-ledger` / `alloy-signer-trezor`. Defer per Q6.
- **EIP-4337 account abstraction** (smart contract wallets). Different model.
- **Contract deployment** (only contract calls in v0.1). `sol!` + `Contract::deploy` covers it when added.
- **Flashbots / MEV protection** — Polygon MEV auction exists via private RPCs (`https://polygon-bor-erigon.flashbots.io` etc.) but no Alloy integration yet. Defer.
- **WebSocket subscriptions** — CLI is request/response; subscriptions belong in a long-running daemon.
- **Multi-sig wallets** (Safe). Different signer model.
- **Watch-only wallet import** from xpub. v0.3.
- **Local tx index** (cached tx history; Story 7 "every `tx list` call scans blocks" workaround). v0.3.
- **Plausible-deniability multi-bucket wallet**. v1.0+.
- **REST/HTTP interface**. Separate spec.
- **Mobile (iOS/Android) integration**. Phase 2 via UniFFI.
- **Polygon staking delegation** (POL staking via Stake 2.0 equivalent). Out of scope — wallet is for transfer only, not staking.
- **L2 DEX integration beyond ERC-20 transfer** (QuickSwap swaps, Uniswap-v3-Polygon swaps). Out of scope — wallet is sign-only + broadcast, swap UI is external.

---

## References

- Issue #416 (this work): plan(polygon) rust-sdks deep-dive + polygon-wallet-core
- Deep-dive: `docs/wallets/2026-08-27-polygon-rust-sdks-deep-dive.md`
- ETH user-stories template: `docs/wallets/2026-08-23-eth-wallet-user-stories.md`
- TRON user-stories sibling: `docs/wallets/2026-08-27-tron-wallet-user-stories.md`
- ETH deep-dive (EVM primitive reference): `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`
- TRON deep-dive (sibling non-EVM chain template): `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md`
- eth-wallet-core (EVM core source): `rust-wallet-app/crates/eth-wallet-core/`
- evm-wallet-core (refactor target — to be created per Option A): `rust-wallet-app/crates/evm-wallet-core/`
- bitcoin `SpkiPinnedVerifier` (SPKI pin reuse): `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`
- L13 pipeline spec: `tasks/lessons.md` L13 (apply literally)
- L24 (CHANGELOG + User Stories cascade)
- Related: L29 (live testnet smoke is operator-driven)