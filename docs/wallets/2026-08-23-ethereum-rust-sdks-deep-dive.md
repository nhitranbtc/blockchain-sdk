# Ethereum-Specific Rust SDK Deep-Dive

**Date:** 2026-08-23
**Scope:** Focused re-research on Rust crates for an Ethereum (EVM) wallet built inside `rust-wallet-app/`, covering send/receive native ETH plus ERC-20 stablecoin transfer (USDT, USDC). Verifies the 5 chosen crates against current 2026 state, considers alternatives, and digs into signing + stablecoin-ABI details.
**Companion to:** `docs/wallets/2026-08-05-bitcoin-rust-sdks-deep-dive.md` (Bitcoin precedent). Pre-empts the v0.2 deliverable sketched in `rust-wallet-app/crates/chain-traits/src/lib.rs:21` (`ChainId::Ethereum(u32)` with comment "ethereum-wallet-core for v0.2+").
**Status:** Research report only. No design spec, no implementation plan, no code produced in this session.

## TL;DR

Use **alloy v1.x** (stable, 1.0 released 2025-05-15) as the primary Ethereum stack. It subsumes the role ethers-rs used to play and is now officially recommended by the ethers-rs maintainers. Five crates cover the whole surface: `alloy` (meta), `alloy-signer-local` (BIP-39 + secp256k1), `alloy-provider` + `alloy-transport-http` (JSON-RPC), `bip32` (HD derivation, reused from Bitcoin side), and the existing workspace `reqwest`+`rustls` stack for any raw HTTP needs. `ethers-rs` is rejected (deprecated 2024-06, see issue #2667). `k256` is used transitively through alloy's `alloy-signer-local` `mnemonic` feature — no standalone dep needed.

## The 5 chosen crates — current 2026 state

| Crate | Current version | Stars | Last release | License | Maintained? | Mobile-friendly? | Notes |
|---|---|---|---|---|---|---|---|
| `alloy` | 1.8.3 (stable 1.0 May 2025; 2.x line tracks latest) | 1,286 on `alloy-rs/alloy` | 2026-03-27 | Apache-2.0 / MIT | Yes — backbone of Reth, Foundry, Revm, SP1 zkVM | Yes — `no_std` core; pure-Rust signer default | Modern Ethereum SDK. Replaces ethers-rs (officially deprecated 2024-06). MSRV 1.91 on 1.x; 1.94 on 2.x. **Workspace `rust-toolchain.toml` pins 1.94** → 2.x OK. |
| `alloy-signer-local` | same as alloy | (same monorepo) | tracks alloy | Apache-2.0 | Yes | Yes | Local signer with `PrivateKeySigner`, K256 (pure Rust) by default, feature-gated `Secp256k1` (libsecp256k1 FFI), and `mnemonic` feature for BIP-39 → `PrivateKeySigner::from_phrase()`. |
| `alloy-provider` + `alloy-transport-http` | same as alloy | (same monorepo) | tracks alloy | Apache-2.0 | Yes | Yes | JSON-RPC provider. `ProviderBuilder::new().connect_http(url)` for mainnet/Sepolia. Layer/filler pattern for nonce + gas estimation. |
| `bip32` | ^0.5 (already in workspace, F46 fallback pinned) | ~50 | active | MIT | Yes | Yes | BIP-32 HD derivation. Already a workspace dep from Bitcoin side. No new dep. |
| `bip39` | 2.2 (already in workspace, `zeroize` + `rand` features) | ~150 | active | MIT | Yes | Yes | BIP-39 mnemonic generate/parse/to_seed. Already a workspace dep. `alloy-signer-local` re-export not yet — we keep the standalone direct dep (matches Bitcoin precedent). |

All 5 are correct, mature, and ready. No 2026 alternatives have surpassed them.

## Why alloy and not ethers-rs

**ethers-rs is deprecated.** `gakonst/ethers-rs` README carries a top-of-page banner: "ethers-rs has been deprecated for alloy. Learn how to use Alloy by visiting the book." Issue #2667 (closed by DaniPopes 2023-11-07) tracks the official deprecation, and the Paradigm "Releasing Alloy" post (2024-06-18) and "Introducing Alloy v1.0" post (2025-05-15) formally announce end of maintenance. Mapping (from the deprecation issue):

| ethers-rs | replacement |
|---|---|
| `ethers::abi`, `ethers::contract`, `ethers::core`, `ethers::types` | `alloy-core` (`alloy-primitives`, `alloy-sol-types`, `alloy-dyn-abi`, `alloy-json-abi`) |
| `ethers::middleware`, `ethers::providers`, `ethers::signers` | `alloy` (`alloy-provider`, `alloy-signer`, `alloy-transport-http`) |
| `ethers_core::types::Chain` | `alloy-chains` |
| `ethers::etherscan` | `foundry-block-explorers` |

**Decision:** Use alloy. No ethers-rs even for v0.1. ethers-rs is not even a fallback — it's a maintenance liability.

## Crate-by-crate deep-dive (2026)

### `alloy` 1.x (stable) / 2.x (cutting edge)

**Why this one:** The current canonical Ethereum Rust SDK. Replaces ethers-rs. Backbone of the Reth execution client, the Foundry dev toolkit, Revm (Rust EVM), and SP1 zkVM — when this much of the Ethereum Rust ecosystem sits on one crate, the crate is the right choice. Apache-2.0 + MIT dual-licensed (permissive, compatible with everything).

**API surface used by an ETH wallet:**

- `alloy_primitives::{Address, U256, Bytes, address!}` — core value types. `address!(0x...)` is a const-eval macro for compile-time address literals.
- `alloy_consensus::TxLegacy`, `alloy_consensus::TxEip1559`, `alloy_consensus::TxEnvelope` — transaction envelopes. EIP-1559 (type 0x02) is the default for post-London mainnet.
- `alloy_signer::Signer`, `alloy_signer::SignerSync`, `alloy_network::TxSignerSync` — signer traits (sync + async).
- `alloy_signer_local::{PrivateKeySigner, MnemonicBuilder}` — concrete local signer. `MnemonicBuilder::new().phrase("...").index(0)?.build()` returns a `PrivateKeySigner`.
- `alloy_provider::{Provider, ProviderBuilder, RootProvider}` — RPC client. `ProviderBuilder::new().connect_http(url)` for HTTP.
- `alloy_provider::Provider::get_balance(addr)`, `get_transaction_count(addr)`, `estimate_gas(tx)`, `send_transaction(tx)`, `get_block_number()`, `get_chain_id()` — RPC methods.
- `alloy_rpc_types::TransactionRequest` — fluent builder for tx request (`with_to`, `with_value`, `with_gas_limit`, `with_max_fee_per_gas`, `with_max_priority_fee_per_gas`, `with_input`, `with_chain_id`, `with_nonce`).
- `alloy_sol_types::sol!` — proc macro for declaring Solidity types inline (`sol! { function transfer(address to, uint256 value) external returns (bool); }`).
- `alloy_contract::ContractInstance` — typed contract binding.

**Signing model** (from `alloy-signer-local` README): the `mnemonic` feature enables BIP-39 mnemonic → `PrivateKeySigner` directly. Internally it uses **k256** (pure-Rust `secp256k1`) by default; `Secp256k1` (libsecp256k1 FFI) is feature-gated. For our wallet this is good news — k256 is the same signing primitive the Bitcoin side will use once we standardise (or already uses via `secp256k1` crate). No new FFI footprint.

**Risks:**

- **MSRV drift.** alloy 1.x = MSRV 1.85 → compatible with workspace `rust-version = "1.85"`. alloy 2.x = MSRV 1.91–1.94 → still inside our pinned `rust-toolchain.toml` 1.94, but a transitive bump in a minor release could break us. **Pin to alloy 1.8.x for v0.2 to keep MSRV parity; revisit 2.x once the rest of the Bitcoin-side deps settle.**
- **Heavy default features.** The meta `alloy` crate pulls ~25 sub-crates. For a minimal wallet we should depend on the sub-crates individually (`alloy-primitives`, `alloy-signer-local`, `alloy-provider`, `alloy-transport-http`, `alloy-rpc-types`, `alloy-sol-types`) and skip `alloy-contract`, `alloy-node-bindings`, `alloy-signer-aws/gcp/ledger/trezor/turnkey`. Smaller dep tree = faster compile.
- **`sol!` macro = heavy codegen.** Acceptable for typed contract bindings; we only need one or two `sol!` blocks (ERC-20 + maybe ERC-20 decimals/balanceOf).
- **Default provider has fillers (nonce, gas, chain-id).** `ProviderBuilder::new()` includes `NonceFiller` + `GasFiller` + `ChainIdFiller` + `WalletFiller` — handy but wallet filler requires a signer, and our wallet manages nonce itself (parallel to Bitcoin `WalletManager` pattern). **Pass the signer explicitly at send-time; do not use the auto-wallet filler.**

### `alloy-signer-local`

**Why this one:** The local-signer sub-crate. Supports three signing paths we care about: BIP-39 mnemonic → keypair, raw secp256k1 secret bytes, and YubiHSM2. The `mnemonic` feature flag is the one we enable.

**API surface:**

- `PrivateKeySigner::random()` — random key for tests.
- `PrivateKeySigner::from_slice(&bytes)` — from raw 32-byte secret.
- `MnemonicBuilder::new().phrase("word1 word2 ...").index(0)?.build()` — from BIP-39 phrase with optional account index (default 0 → `m/44'/60'/0'/0/0`).
- `signer.address()` → `alloy_primitives::Address`.
- `signer.sign_transaction_sync(&mut tx)` → fills signature fields in-place, returns `Signature`.
- `signer.sign_message_sync(&[u8])` → personal-sign (EIP-191 prefix), used for off-chain auth/ownership proofs.
- `signer.sign_typed_data_sync(&TypedData)` → EIP-712 typed structured data signing.

**Risks:**

- The `mnemonic` feature depends on `bip39` (re-exported, but feature-gated). Enabling it pulls `bip39` into our tree regardless. **Harmless — we already have `bip39` direct-dep from the Bitcoin side.**
- The builder's default derivation path is `m/44'/60'/0'/0/0`. This matches Ledger/Trezor (the "Ledger path"). MetaMask uses `m/44'/60'/0'/0/{idx}` (incremented at the address index slot, not account slot). **Document the chosen variant in the config; allow override via env or CLI flag (F11-style — same pattern as Bitcoin's `WalletConfig`).**
- `PrivateKeySigner` does NOT zeroize on drop. **Wrap in `Zeroizing<PrivateKeySigner>` or extract the secret into a zeroize-owned buffer for v0.2 (mirrors F47 mnemonic zeroize treatment on the Bitcoin side).**

### `alloy-provider` + `alloy-transport-http`

**Why this one:** JSON-RPC client. Layer-based architecture (`Tower`-style). Supports fillers for nonce + gas estimation. Built-in support for batching, retries, and rate limiting.

**API surface:**

- `ProviderBuilder::new().connect_http("https://...".parse()?)` → `RootProvider<Ethereum>`.
- `provider.get_block_number().await?` → `u64`.
- `provider.get_chain_id().await?` → `u64` (1 for mainnet, 11155111 for Sepolia).
- `provider.get_balance(addr).await?` → `U256` (wei).
- `provider.get_transaction_count(addr).await?` → `u64` (nonce).
- `provider.estimate_gas(&tx).await?` → `u64`.
- `provider.send_transaction(tx).await?` → `PendingTransactionBuilder` (yields `TransactionReceipt` once mined).

**Risks:**

- **TLS pinning.** The Bitcoin side deliberately avoids `bdk_esplora` and writes a raw `reqwest` + custom SPKI pinning verifier (Task 7 of the Bitcoin plan, F20 in threat model). **Apply the same pattern here:** alloy's `alloy-transport-http` uses `hyper` + `reqwest` under the hood, and the SPKI pin must be applied at the transport level. Verify in the implementation spike whether `alloy-transport-http` exposes a transport hook or if we need to subclass. **Worst case:** bypass `alloy-provider` and hand-roll a thin `reqwest` JSON-RPC client for the wallet — matches the Bitcoin pattern exactly.
- **Etherscan / block explorer:** alloy has no first-party Etherscan API. Use `foundry-block-explorers` if needed (separate crate, optional dep).
- **WebSocket subscriptions.** `alloy-provider` supports WS via `connect_ws`, useful for `pendingTransactions` / `logs` subscriptions. Not needed for v0.2 (no mempool-tracking requirement).

### `bip32` ^0.5 (workspace dep, reused)

**Why reuse:** Already a workspace dep from the Bitcoin side. ETH derivation `m/44'/60'/0'/0/0` is identical in mechanics to Bitcoin BIP-44 derivation — only the coin type differs (60 vs 0). Same `XPrv` + `DerivationPath` API, same `to_secp256k1_secret_key(&secp)` extraction.

**API surface (reused):**

- `bip32::XPrv::derive_from_path(&seed, &DerivationPath::from_str("m")?)` — master from seed.
- `master.derive_path(&DerivationPath::from_str("m/44'/60'/0'/0/0")?)` — first receive-address xprv.
- `child.to_secp256k1_secret_key(secp)` — convert to `secp256k1::SecretKey` for signing.

**Why not just lean on `alloy-signer-local::MnemonicBuilder` end-to-end?** Because:
- The Bitcoin side already has a `bip32` direct dep for the same reason (we want explicit control over the derivation path, not hidden behind a builder).
- `MnemonicBuilder` works fine, but the CLI needs to **display the 12-word mnemonic** as part of `wallet create` — that's a `bip39::Mnemonic::generate_in(Words12, English, rng)` operation, not a builder operation. We use `bip39` for generation + display, `bip32` for derivation, then hand the resulting `SecretKey` to `PrivateKeySigner::from_slice(&sk.secret_bytes())` for signing. Same shape as the Bitcoin data flow.
- **Path consistency:** the Bitcoin plan documents path as part of `WalletConfig`. The ETH plan should do the same (single field, configurable, default `m/44'/60'/0'/0/0`).

### `bip39` 2.2 (workspace dep, reused)

**Why reuse:** Already a workspace dep with `zeroize` + `rand` features. ETH mnemonic is identical to Bitcoin BIP-39 (same wordlist, same PBKDF2 params). **One BIP-39 implementation serves both chains** — the master seed is identical, only the derivation path differs.

**API surface (reused):**

- `bip39::Mnemonic::generate_in(MnemonicType::Words12, Language::English, &mut rng)` — generate.
- `bip39::Mnemonic::parse_in(Language::English, "word1 word2 ...")` — parse + checksum.
- `m.to_seed(passphrase: &str)` → `[u8; 64]` for BIP-32 master key derivation.

**Risks:**

- Same as Bitcoin side — `Mnemonic` does NOT zeroize its internal entropy on drop. **Wrap in `Zeroizing<Mnemonic>`** (mirrors Task 30 of the Bitcoin plan).

## Stablecoin transfer — contract addresses + ABI

Both USDT and USDC are ERC-20 tokens on Ethereum mainnet. They differ from most ERC-20s in one important way: **6 decimals, not 18**. A naive wallet that treats all ERC-20 balances as `value / 10^18` will mis-display USDC/USDT by 12 orders of magnitude.

| Token | Mainnet contract | Decimals | Symbol | Source |
|---|---|---|---|---|
| USDT (Tether USD) | `0xdAC17F958D2ee523a2206206994597C13D831ec7` | **6** | USDT | etherscan.io token page; eco.com support article confirms "USDT uses 6 decimals on Ethereum, unlike most ERC-20s that use 18" |
| USDC (USD Coin) | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | **6** | USDC | Circle official developer docs (`developers.circle.com/stablecoins/usdc-contract-addresses`); Etherscan token page |

**Sepolia (testnet) equivalents** (for v0.2 smoke testing):

- USDC Sepolia: `0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238` (Circle-issued, per `developers.circle.com/wallets/tokens`).
- USDT Sepolia: Tether does not always publish a Sepolia contract. **For testnet smoke, prefer USDC Sepolia + a local Anvil deployment of a mock ERC-20** (Anvil ships with a `MockERC20` you can deploy in 5 lines via the `alloy_sol_types::sol!` + `Contract::deploy` API).

**ERC-20 ABI surface we need:**

```solidity
// EIP-20 standard subset
function name() external view returns (string)
function symbol() external view returns (string)
function decimals() external view returns (uint8)
function totalSupply() external view returns (uint256)
function balanceOf(address account) external view returns (uint256)
function transfer(address to, uint256 value) external returns (bool)
function approve(address spender, uint256 value) external returns (bool)
function allowance(address owner, address spender) external view returns (uint256)
function transferFrom(address from, address to, uint256 value) external returns (bool)

event Transfer(address indexed from, address indexed to, uint256 value)
event Approval(address indexed owner, address indexed spender, uint256 value)
```

For send-stablecoin we only need `transfer(address,uint256)` → function selector **`0xa9059cbb`** (keccak256("transfer(address,uint256)")[0..4]). For read-balance we need `balanceOf(address)` → selector `0x70a08231`. For display we need `decimals()` → selector `0x313ce567`.

**Encoding example (send 1.50 USDC = 1_500_000 base units):**

```rust
use alloy_sol_types::sol;

sol! {
    function transfer(address to, uint256 value) external returns (bool);
}

// Build calldata for transfer(0xRecipient..., 1_500_000)
let call = transferCall {
    to: address!("0xRecipient..."),
    value: U256::from(1_500_000_u64),
};
let calldata: Bytes = call.abi_encode().into();
```

Then build a `TransactionRequest` with `to = Address(token_contract)`, `value = U256::ZERO` (we're not sending ETH), `input = calldata`. Sign with `PrivateKeySigner`, send via `provider.send_transaction(tx)`.

## Mnemonic-to-broadcast data flow (end-to-end)

```text
1. eth wallet create --name w
   ↓
2. bip39::Mnemonic::generate_in(Words12, English, rng)  -- 12-word phrase
   ↓
3. m.to_seed(passphrase)  -- 64-byte PBKDF2 output
   ↓
4. bip32::XPrv::derive_from_path(&seed, "m")  -- master xprv
   ↓
5. master.derive_path("m/44'/60'/0'/0/0")  -- first ETH receive xprv (SLIP-44 coin type 60)
   ↓
6. sk_bytes = child.to_secp256k1_secret_key(&secp).secret_bytes()  -- 32 bytes
   ↓
7. signer = PrivateKeySigner::from_slice(&sk_bytes)  -- wraps bytes for signing
   ↓
8. addr = signer.address()  -- 20-byte Ethereum address (keccak256(pubkey)[12..])
   ↓
9. Store m as plaintext (v0.2) or encrypt with Argon2id → AES-256-GCM (v0.3) on disk
   ↓
10. At send time:
    - nonce = provider.get_transaction_count(signer.address()).await?
    - gas_price = provider.estimate_gas(&tx).await? + EIP-1559 fee estimation
    - tx = TransactionRequest::default()
              .with_to(recipient)
              .with_value(value_wei)
              .with_nonce(nonce)
              .with_gas_limit(gas_limit)
              .with_max_fee_per_gas(max_fee)
              .with_max_priority_fee_per_gas(priority_fee)
              .with_chain_id(1)
              .with_input(calldata);  // for ERC-20 transfer
    - signature = signer.sign_transaction_sync(&mut tx)?
    - pending = provider.send_transaction(tx).await?
    - receipt = pending.get_receipt().await?  // wait for inclusion
   ↓
11. For ERC-20 send:
    - call = transferCall { to: recipient, value: U256::from(human_amount * 10^decimals) }
    - calldata = call.abi_encode().into()
    - tx.with_to(token_contract).with_value(U256::ZERO).with_input(calldata)
```

**Parallel to the Bitcoin flow** (bitcoin deep-dive §"Mnemonic-to-signing-path"): one BIP-39, one BIP-32, one `SecretKey`. Only the derivation path and the transaction envelope differ.

## Alternatives considered (and why rejected)

| Alternative | Why rejected |
|---|---|
| `ethers-rs` (any version) | Officially deprecated 2024-06 (issue #2667). Maintainers redirect to Alloy + Foundry. No bug-fix SLA beyond "crits only". |
| `web3` (Parity) | Last release 2023; not maintained. ethers-rs already superseded it; alloy supersedes ethers. |
| `revm` for transaction simulation | Overkill. `revm` is a full EVM executor (used by Foundry/Reth). Wallets don't need to simulate arbitrary bytecode; `provider.estimate_gas()` covers our needs. |
| `k256` as standalone direct dep | Already transitively present via `alloy-signer-local`'s `mnemonic` feature. Adding it directly doubles the signing impls and risks version drift. **Use alloy's bundled k256; do not re-declare.** |
| `secp256k1` (libsecp256k1 FFI) as standalone direct dep | Same — alloy-signer-local already exposes it behind the `secp256k1` feature flag if we ever need the FFI signer. **Bitcoin side already uses `secp256k1` 0.30** — keep the existing dep; do not re-declare in ETH crate. |
| `primitive-types` (Parity) | Subsumed by `alloy_primitives`. `alloy_primitives::U256` is backed by `ruint` (faster, const-generic, used by Revm/Foundry). Direct `primitive-types` dep adds redundant types. |
| `ruint` as standalone direct dep | Use `alloy_primitives::U256` which re-exports ruint types. Direct ruint dep only if we need `Uint<256, 4>` outside alloy's surface — unlikely for a wallet. |
| Hand-rolled RLP encoder | `alloy_rlp` is in the workspace via alloy. Don't write it ourselves. |
| Hand-rolled Keccak-256 | Don't. `alloy_primitives` + `tiny-keccak` (if needed for non-alloy code paths) are the standards. |
| `web3` JSON-RPC client (Parity) | `alloy-provider` is the successor. |
| `jsonrpsee` as direct dep | alloy's transport already wraps JSON-RPC. Adding jsonrpsee would mean two JSON-RPC stacks. **Don't add** — use alloy's. (jsonrpsee is relevant only if we ever run a local Ethereum node that needs JSON-RPC server-side, which is not a wallet concern.) |
| Solidity-bindings via `ethers-rs` ABI derive macro | `alloy_sol_types::sol!` is the successor. |
| `ethers-flashbots` MEV bundle | Out of scope for v0.2 (no MEV protection requirement). Optional dep for a future "protect tx" flag. |
| L2 chains (Optimism, Arbitrum, Base, Polygon) | Out of scope per session scoping. The `ChainId::Ethereum(u32)` placeholder in `chain-traits` already supports a `chain_id` discriminator, so L2s are an additive change — drop in another chain-id constant + another RPC URL. Not a v0.2 blocker. |
| ENS name resolution | Out of scope. `alloy-ens` exists if needed later. |
| Hardware wallet (Ledger, Trezor) integration | Out of scope for v0.2. `alloy-signer-ledger` and `alloy-signer-trezor` are first-party sub-crates if added later. |
| EIP-4337 account abstraction | Out of scope. Different wallet model (smart contract wallets). v1.x concern at earliest. |
| EIP-712 typed-data signing | **In scope** — needed for some ERC-20 approvals and DEX interactions. alloy supports it via `sign_typed_data_sync`. Cover in v0.3. |

## Open questions

1. **MSRV drift between alloy 1.x and 2.x.** 1.x = 1.85 (matches workspace `rust-version`). 2.x = 1.91–1.94 (still inside `rust-toolchain.toml` pin but outside declared `rust-version`). **Decision: pin to alloy 1.8.x for v0.2 to keep MSRV parity; re-evaluate when the Bitcoin-side ecosystem settles.**
2. **TLS pinning transport.** Does `alloy-transport-http` expose a hook for a custom `ServerCertVerifier`? If not, fall back to raw `reqwest` for the pinned endpoints (mirrors Bitcoin Task 7). Resolve at implementation spike, not before.
3. **MetaMask vs Ledger derivation path default.** Ledger = `m/44'/60'/0'/0/0` (index at account slot). MetaMask = `m/44'/60'/0'/0/{idx}` (index at address slot). **Default to Ledger path (canonical, SLIP-44-compliant). Make configurable via `WalletConfig` (matches Bitcoin precedent).**
4. **`ProviderBuilder` auto-fillers vs explicit nonce/gas.** alloy's default fillers are convenient but pull in a signer (`WalletFiller`). We want explicit nonce + explicit gas estimation in our `WalletManager`, parallel to the Bitcoin side. **Use `Provider::new_http(url)` (no fillers) instead of `ProviderBuilder::new()`.**
5. **Decimals handling.** Hard-code (USDT=6, USDC=6) for the v0.2 stablecoin list, OR query `decimals()` once and cache? **Recommend cache (one `eth_call` per token at startup, persist in token registry) — matches how MetaMask/Rabby behave.**
6. **Stablecoin registry.** Where does the token list live? (a) hard-coded in `WalletConfig` (2 entries, low maintenance), (b) JSON file in `~/.config/<app>/tokens.json` (operator-editable), (c) bundled in the repo (`rust-wallet-app/crates/eth-wallet-core/tokens/mainnet.json`). **Recommendation: (c) for v0.2, (b) for v1.x.**
7. **Zeroize coverage.** `Mnemonic`, `XPrv`, `PrivateKeySigner`'s internal key all need zeroize treatment. Mirror Bitcoin Task 30.
8. **Anvil for smoke tests.** `alloy-node-bindings` ships Anvil + Geth + Reth node spawners. Add as `[dev-dependencies]` to run regtest-style smoke (mirrors Bitcoin's Docker regtest setup).
9. **Stablecoin source of truth.** Circle publishes `developers.circle.com/stablecoins/usdc-contract-addresses` as the canonical source. Tether does not maintain an equivalent page. **Risk:** Tether could deploy a new USDT contract and we'd ship stale data. **Mitigation:** version the registry + a `--update-tokens` CLI command that fetches the Circle list at runtime.

## Verification

No implementation work in this session. The next-session spike (if implementation is approved) should validate:

1. `cargo add alloy@1 --features="signers,signer-local,provider,transport-http"` compiles against the existing workspace.
2. `MnemonicBuilder::new().phrase(test_mnemonic).build()` returns a `PrivateKeySigner` whose `.address()` matches the expected Ethereum address for that mnemonic.
3. `provider.get_block_number()` against `https://ethereum.reth.rs/rpc` returns a sane value.
4. `provider.send_transaction(signed_native_eth_tx).await` against Anvil returns a `TransactionReceipt`.
5. `transferCall { to, value }.abi_encode()` produces calldata whose first 4 bytes are `0xa9059cbb`.
6. ERC-20 transfer against an Anvil-deployed MockERC20 succeeds and the recipient's `balanceOf` reflects the change.
7. SPKI-pinned HTTP transport returns a request to the pinned RPC endpoint and rejects an unpinned one.

If all 7 pass, the 5 chosen crates are confirmed correct for v0.2.

## Sources

- alloy crates.io: <https://crates.io/crates/alloy> (v2.4.1 latest, v1.8.3 stable line)
- alloy GitHub: <https://github.com/alloy-rs/alloy> (1,286 stars, Apache-2.0)
- alloy docs: <https://alloy.rs>
- alloy v1.0 announcement: <https://www.paradigm.xyz/2025/05/introducing-alloy-v1-0>
- alloy 0.1 release (predecessor of v1.0): <https://www.paradigm.xyz/2024/06/alloy-release>
- ethers-rs deprecation issue: <https://github.com/gakonst/ethers-rs/issues/2667>
- ethers-rs README deprecation banner: <https://github.com/gakonst/ethers-rs>
- ethers-rs → alloy migration reference: <https://alloy.rs/migrating-from-ethers/reference/>
- USDC mainnet contract (Circle official): <https://developers.circle.com/stablecoins/usdc-contract-addresses>
- USDC Etherscan token page: <https://etherscan.io/token/0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48>
- USDT ERC-20 reference (eco.com): <https://eco.com/support/en/articles/15082529-usdt-erc-20-fees-speed-how-to-send>
- ERC-20 standard (EIP-20): <https://eips.ethereum.org/EIPS/eip-20>
- SLIP-0044 coin types (ETH = 60): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- EIP-601 "Ethereum hierarchy for deterministic wallets": <https://eips.ethereum.org/EIPS/eip-601>
- ethers.js HDWallet default path (industry reference, Ledger-style `m/44'/60'/0'/0/0`): <https://github.com/ethers-io/ethers.js/blob/main/src.ts/wallet/hdwallet.ts>
- tiny-keccak (Keccak-256 hash): <https://crates.io/crates/tiny-keccak> (v2.0.2, CC0)
- ruint (U256 backing alloy): <https://crates.io/crates/ruint> (v1.20.0, alloy-rs maintainership)
- BIP-39 wordlist (English): implicit via `bip39` crate (already in workspace)
