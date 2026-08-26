# `eth` Ethereum Wallet — User Stories

**Date:** 2026-08-23
**Companion to:** [Ethereum Rust SDK deep-dive](2026-08-23-ethereum-rust-sdks-deep-dive.md) + [Bitcoin user stories precedent](2026-08-05-btc-wallet-user-stories.md)
**Surface:** `eth` CLI binary inside `rust-wallet-app/crates/eth/` next to `btc/`. **Default network = Sepolia testnet (chain id 11155111).** Mainnet opt-in via `--network mainnet` (chain id 1). Anvil regtest opt-in via `--rpc-url http://localhost:8545` (chain id 31337).

**Wallet identity (per #297 B1):** user-facing identifier = `--name` (string); internal `wallet_id` (UUID) generated at create time and used for cross-wallet uniqueness on disk. Mirrors BTC v0.1 PR #81 user model.

Personas:

- **Alice** — Ethereum power user. Manages multiple wallets across chains. Wants CLI control + scriptable commands + ERC-20 stablecoin transfers.
- **Bob** — Developer integrating Ethereum. Wants stable exit codes + JSON output + EIP-712 typed-data signing for automation.
- **Carol** — First-time user. Wants clear prompts, warnings about mnemonic safety, no surprises, simple send/receive.

---

## Story → alloy sub-crate map

Each story is implemented via one or more `alloy` sub-crates. This table is the traceability link between the user-facing feature and the underlying wallet engine call. The 4th column shows which `alloy_primitives` + `alloy_sol_types` primitive backs the alloy call — making the two-layer architecture (`alloy_primitives` value types + `alloy-provider` RPC + `alloy_signer_local` signing) explicit per story. The 5th column reuses `bip32` + `bip39` where they apply (mnemonic + HD derivation are identical to Bitcoin; only the coin type differs).

| # | Story | alloy sub-crate(s) used | `alloy_primitives` / `alloy_sol_types` primitive(s) | `bip32` / `bip39` reuse |
|---|---|---|---|---|
| 1 | Create a new wallet | `alloy-signer-local::MnemonicBuilder`; `alloy-primitives::Address` | `Address::from_slice(&keccak256(pubkey)[12..])` | `bip39::Mnemonic::generate_in(Words12, English, rng)` + `bip32::XPrv::derive_path("m/44'/60'/0'/0/0")` |
| 2 | Import an existing wallet | `alloy-signer-local::MnemonicBuilder::phrase`; `alloy-primitives::Address` | `Address` derivation | `bip39::Mnemonic::parse_in(English, s)` + same `bip32` path |
| 3 | Check ETH balance | `alloy-provider::Provider::get_balance` | `U256` (wei), `Address` | n/a |
| 4 | Sync chain state | `alloy-provider::Provider::get_block_number`, `get_chain_id`, `get_transaction_count` | `U256`, `u64` (nonce), `u64` (chain id) | n/a |
| 5 | Send native ETH | `alloy-provider::Provider::send_transaction`; `alloy-signer-local::PrivateKeySigner::sign_transaction_sync` | `alloy_rpc_types::TransactionRequest` (EIP-1559 type 2), `alloy_consensus::TxEip1559`, `Signature` | extract `PrivateKeySigner` from derived `SecretKey` (same as Story 1) |
| 6 | Send with custom EIP-1559 fee | `alloy_rpc_types::TransactionRequest::with_max_fee_per_gas`, `with_max_priority_fee_per_gas` | `U256` (wei/gas) | n/a |
| 7 | Inspect transaction history | `alloy-provider::Provider::get_transaction_by_hash`, `get_transaction_receipt` | `alloy_rpc_types::Transaction`, `TransactionReceipt` (`block_number`, `gas_used`, `status`) | n/a |
| 8 | Get current gas estimates | `alloy-provider::Provider::estimate_gas`, `get_fee_history` (EIP-1559 base fee + priority) | `U256` (gas units), `alloy_rpc_types::FeeHistory` | n/a |
| 9 | List / show / delete / rename wallets | `std::fs`; `chain-traits` registry | n/a | n/a |
| 10 | Use mainnet explicitly | `alloy-provider::Provider::get_chain_id` (assert `0x1`) | `u64` chain id | n/a |
| 11 | Show config + debug info | `std::env`, `version()` | n/a | n/a |
| 12 | Persist wallet across CLI invocations | filesystem + UUID-based wallet dir; encrypted mnemonic (F5/F6) | n/a | n/a (mnemonic on disk, never re-derived) |
| 13 | Send to multiple recipients (sequential txs) | N `send_transaction` calls; no native multi-output (EVM is single-recipient per tx) | N × `TransactionRequest` | n/a |
| 14 | Sweep / drain wallet to one address | `provider.get_balance` + `TransactionRequest::with_value(balance - gas_estimate)` | `U256` balance arithmetic | n/a |
| 15 | Choose nonce strategy (auto vs manual) | `provider.get_transaction_count` (auto) vs caller-supplied `with_nonce` (manual) | `u64` nonce | n/a |
| 16 | Manual nonce + gas limit override | `TransactionRequest::with_nonce`, `with_gas_limit` | `u64` nonce, `u64` gas limit | n/a |
| 17 | Replace / speed-up tx (same nonce, higher fee) | new `TransactionRequest` with same `nonce` + higher `max_fee_per_gas` | `Signature` (different from original) | n/a |
| 18 | Sign EIP-191 personal message | `alloy-signer-local::PrivateKeySigner::sign_message_sync` | `Signature`, recovered `Address` (via `signature.recover_address_from_msg`) | n/a |
| 19 | Export the wallet xpub + first addresses | `bip32::XPub::to_string` (no descriptor in ETH — no script template) | `alloy_primitives::Address` (first 5 receive addresses) | `bip32::XPub` + `bip32::DerivationPath` |
| 20 | Pick derivation path (Ledger vs MetaMask) | `alloy-signer-local::MnemonicBuilder::derivation_path` | n/a | override path in config |
| 21 | Send ERC-20 stablecoin (USDT/USDC) | `alloy_sol_types::sol!`; `provider.send_transaction` with calldata | `transferCall { to: Address, value: U256 }`, `Bytes` (calldata), `Address` (token contract) | n/a |
| 22 | Check ERC-20 token balance | `sol! { function balanceOf(address) external view returns (uint256); }` + `provider.call(&req)` | `balanceOfCall`, `balanceOfReturn`, `U256` | n/a |
| 23 | List registered stablecoins / tokens | token registry JSON in repo (`rust-wallet-app/crates/eth/tokens/mainnet.json`) | `Address`, `U256` decimals, `String` symbol | n/a |
| 24 | Add custom ERC-20 token by contract address | `sol! { function decimals() ...; function symbol() ...; }` + `provider.call` | `U256`, `String` | n/a |
| 25 | Approve ERC-20 spending (for DEX) | `sol! { function approve(address spender, uint256 value) external returns (bool); }` + `provider.send_transaction` | `approveCall`, `Bytes` calldata | n/a |
| 26 | Use Anvil local node for testing | `--rpc-url http://localhost:8545`; `provider.get_chain_id()` asserts `0x7a69` (31337) | `u64` chain id | n/a |
| 27 | Sign EIP-712 typed data (v0.3 deferred) | `alloy-signer-local::PrivateKeySigner::sign_typed_data_sync` | `alloy_primitives::eip712::TypedData`, `Signature` | n/a |
| Cross-cutting | `--json` everywhere; stable exit codes; no daemons | `serde_json`; `std::process::ExitCode` | n/a | n/a |
| Cross-cutting | `Secret<Mnemonic>` zeroize (v0.2 hygiene) | `zeroize::Zeroizing<Mnemonic>` (mirror Bitcoin Task 30) | n/a | `bip39::Mnemonic` (wrap) |
| Cross-cutting | EIP-55 checksum address display | `alloy_primitives::Address::to_checksum_buffer(None)` | `Address` | n/a |

**Layer separation summary:**

- **`alloy_primitives`** = value types (Address, U256, Bytes, FixedBytes). The Ethereum analogue of `rust-bitcoin`'s script + address + amount types.
- **`alloy_sol_types` + `sol!` macro** = Solidity ABI encode/decode (typed `sol! { function transfer(...) }` blocks + auto-generated `*Call`/`*Return` types). Replaces hand-rolled RLP + ABI packing.
- **`alloy-provider` + `alloy-transport-http`** = JSON-RPC client. Replaces ethers-rs `Provider`. Layer-based (Tower-style).
- **`alloy-signer-local`** = local signer (BIP-39 → `PrivateKeySigner`, k256 by default). Replaces ethers-rs `Signer`.
- **`alloy_consensus` + `alloy_rpc_types`** = transaction envelopes (EIP-1559, EIP-2930, EIP-4844) + RPC request/response types. The Ethereum analogue of `rust-bitcoin`'s `Transaction` + `Psbt`.
- **`bip32` + `bip39` (reuse from Bitcoin side)** = HD derivation + mnemonic. Identical primitives; only the derivation path differs.

**Stories NOT using alloy (custom code only):**

- Story 19 xpub export — uses `bip32::XPub::to_string` directly (no alloy needed).
- Story 24 custom-token discovery — uses raw `provider.call(&req)` to invoke `decimals()` and `symbol()`; no contract binding needed.
- Story 23 token registry — filesystem JSON, no alloy.

## alloy use case coverage

Cross-check: every use case that alloy provides natively (per the deep-dive §Crate-by-crate deep-dive) mapped to the user story that exercises it. Any "not covered" row is either an internal-only operation (no CLI surface needed) or a gap to add.

| # | alloy use case | User story that exercises it | Covered? |
|---|---|---|---|
| 1 | Build a value type (`Address`, `U256`, `Bytes`) | Story 1 (address derivation), Story 21 (calldata), Story 24 (decimals) | ✅ |
| 2 | `Address::to_checksum_buffer` (EIP-55) | Cross-cutting (every address displayed) | ✅ |
| 3 | Build an EIP-1559 tx (`TxEip1559`, `TransactionRequest::with_*`) | Story 5 (send) + Story 6 (custom fee) + Story 14 (drain) + Story 16 (manual gas) + Story 17 (speed-up) | ✅ |
| 4 | Sign a tx with `PrivateKeySigner` | Story 5 (send) + Story 17 (speed-up) + Story 21 (ERC-20 send) + Story 25 (approve) | ✅ |
| 5 | Provider RPC: `get_balance` | Story 3 (balance) + Story 14 (drain balance read) + Story 22 (token balance — via `call`) | ✅ |
| 6 | Provider RPC: `get_transaction_count` (nonce) | Story 4 (sync) + Story 15 (auto nonce) | ✅ |
| 7 | Provider RPC: `estimate_gas` | Story 8 (gas estimate) + Story 14 (drain gas) | ✅ |
| 8 | Provider RPC: `get_fee_history` (EIP-1559) | Story 8 (priority fee tiers) | ✅ |
| 9 | Provider RPC: `get_block_number`, `get_chain_id` | Story 4 (sync) + Story 10 (mainnet verify) + Story 26 (anvil chain id) | ✅ |
| 10 | Provider RPC: `send_transaction` (broadcast) | Story 5 (send) + Story 14 (drain) + Story 17 (speed-up) + Story 21 (ERC-20) + Story 25 (approve) | ✅ |
| 11 | Provider RPC: `get_transaction_by_hash`, `get_transaction_receipt` | Story 7 (history) | ✅ |
| 12 | `sol!` macro for inline ABI typing | Story 21 (transfer) + Story 22 (balanceOf) + Story 24 (decimals/symbol) + Story 25 (approve) | ✅ |
| 13 | Raw `provider.call(&req)` for view calls | Story 22 (balanceOf) + Story 24 (decimals/symbol) | ✅ |
| 14 | Sign EIP-191 personal message | Story 18 | ✅ |
| 15 | Sign EIP-712 typed data | v0.3 deferred (Story 27 captured for traceability) | ⚠️ defer (v0.3) |
| 16 | `Signature::recover_address_from_msg` | Story 18 (verify signer) | ✅ |
| 17 | `alloy_chains` chain metadata (mainnet, sepolia, anvil) | Story 10 (mainnet) + Story 26 (anvil) | ✅ |
| 18 | `MnemonicBuilder::derivation_path` override | Story 20 (path variant) | ✅ |
| 19 | `PendingTransactionBuilder::get_receipt` (wait for inclusion) | Story 5 (broadcast confirmation) + Story 17 (speed-up confirmation) | ✅ |
| 20 | `alloy_node_bindings::AnvilInstance` (local Anvil spawner) | Story 26 (anvil) — as `[dev-dependencies]` | ✅ |
| 21 | RLP encoding/decoding (`alloy_rlp`) | internal to all tx + ABI paths | ⚠️ internal |
| 22 | Keccak-256 hashing (`alloy_primitives::keccak256`) | internal to address derivation (`Address::from_public_key`) + ABI selectors | ⚠️ internal |
| 23 | EIP-2930 (type 1) + EIP-4844 (type 3) tx envelopes | not in v0.2 scope (EIP-1559 = type 2 is sufficient for ETH + ERC-20) | ⚠️ defer (post-London only needs type 2) |
| 24 | ENS resolution (`alloy-ens`) | out of scope | ⚠️ defer (v1.x) |
| 25 | Hardware signer (`alloy-signer-ledger`, `alloy-signer-trezor`) | out of scope | ⚠️ defer (v1.x) |
| 26 | MEV protection (`alloy-signer-flashbots`) | out of scope | ⚠️ defer (v1.x) |
| 27 | WebSocket subscriptions (`provider.subscribe_logs`, `subscribe_pending_transactions`) | not in v0.2 scope (CLI is request/response) | ⚠️ defer (post-v1.x) |
| 28 | Gas-filler + nonce-filler auto-management | Story 15 (rejected — we use explicit nonce) — covered indirectly as a non-goal | ✅ (as "we don't use this") |

**Coverage summary:**

- **21 of 28 use cases directly covered by user stories** (rows 1–20 + row 28 — auto-fillers covered as a non-goal in Story 15)
- **2 use cases are internal-only** (RLP encode/decode, Keccak-256) — used by alloy internals, not user-facing
- **1 deferred as not-needed** (EIP-2930 type 1 + EIP-4844 type 3) — EIP-1559 covers all v0.2 use cases
- **4 deferred to v1.x** (ENS, Ledger/Trezor, Flashbots, WebSocket subs)
- **0 explicitly rejected beyond the 1 already counted as a non-goal in Story 15**

(Total: 21 + 2 + 1 + 4 = 28 = all use cases accounted for. Per #297 M1: row 28 — "Gas-filler + nonce-filler auto-management" — is counted as covered via Story 15 non-goal, not as a primary story match. The "21 covered" subset includes rows 1–20 + row 28.)

**No alloy use cases required by v0.2 are missing from the user stories.** The 27 stories + the 2 internal-only operations cover the full surface of alloy that the `eth` CLI exposes.

---

## Story 1 — Create a new wallet (Alice)

> As Alice, I want to generate a new Ethereum testnet wallet from a single command, so I can start receiving ETH + ERC-20 in under 10 seconds.

**Acceptance criteria:**

- `eth wallet create --name test-wallet` runs in <1s on a developer laptop.
- Output shows: 12-word BIP-39 mnemonic, first receive address (EIP-55 checksum), chain id + network name, wallet name.
- Mnemonic written encrypted to `~/.local/share/eth/test-wallet/mnemonic.enc` (Argon2id + AES-256-GCM per F5/F6, mirrors BTC v0.1). On every CLI call that touches key material, `eth` prompts `wallet unlock:` for the passphrase (or reads `ETH_WALLET_PASSPHRASE` env var). Decrypted mnemonic lives only in zeroized memory; nothing plaintext touches disk.
- Command exits 0 on success, non-zero on filesystem error.
- A prominent `WARNING` line reminds the user to back up the mnemonic before continuing.
- Running the command twice with the same name fails with exit code 2 and message `wallet 'test-wallet' already exists`.

**Options:**

- `--network mainnet|sepolia|anvil` (default `sepolia`)
- `--rpc-url <URL>` (default `https://ethereum.reth.rs/rpc` for sepolia, `https://cloudflare-eth.com` for mainnet, `http://localhost:8545` for anvil)
- `--derivation-path <PATH>` (default `m/44'/60'/0'/0/0` — Ledger-style)
- `--account-index <N>` (default 0; advanced — picks `m/44'/60'/<N>'/0/0` instead)

---

## Story 2 — Import an existing wallet (Alice)

> As Alice, I want to import a wallet from an existing 12/24-word mnemonic, so I can recover access to a wallet I created elsewhere.

**Acceptance criteria:**

- `eth wallet import --name recovered --mnemonic "word1 word2 ... word12" --network sepolia` accepts a valid mnemonic.
- `eth wallet import --name dev --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` imports a raw secp256k1 key (per #297 G4). Validates the scalar is `< secp256k1::ORDER`; rejects out-of-range or zero-key with exit 2 (`invalid private key: out of range`).
- Invalid checksum returns exit code 2 + clear error `invalid mnemonic: checksum mismatch`.
- Imported wallet produces the same first address as the source (verified by deterministic derivation).
- BIP39 passphrase (`--passphrase "..."`) supported; empty passphrase is the default.
- Output does **not** echo the mnemonic back to the terminal.
- Supports 12, 15, 18, 21, 24-word mnemonics.
- Importing from a Ledger-derived mnemonic requires `--derivation-path m/44'/60'/0'/0/0` to match the original address.

---

## Story 3 — Check ETH balance (Alice, Carol) **[DEPRECATED → Story 12 in v0.3]**

> As Carol, I want to see my confirmed ETH balance in a single line, so I know how much I can spend right now.

**Acceptance criteria:**

- `eth wallet balance --mnemonic <words> --network sepolia --rpc-url <URL>` prints **two lines** (scriptable): `address=0xAbC...123 balance_wei=1234567890000000000`.
- First run after wallet creation shows `balance_wei=0` (no funds yet) and exits 0.
- If RPC fails, the command retries once, then prints `rpc failed: <reason>` and exits 3.
- Reads nonce + balance from a single batched `eth_getBalance` + `eth_getTransactionCount` request (where supported).
- Output balance in wei by default; `--unit eth|gwei|wei` (default `wei`); `--human` prints `1.2345 ETH`.
- **Note (per #297 B3):** v0.2 uses stateless mnemonic inline (mirrors BTC v0.1 PR #81). Named-wallet model is Story 12, deferred to v0.3.

---

## Story 4 — Sync chain state (Alice) **[DEPRECATED → Story 12 in v0.3]**

> As Alice, I want to force a full chain sync, so I can see incoming transactions that arrived since the last sync.

**Acceptance criteria:**

- `eth wallet sync --mnemonic <words> --network sepolia --rpc-url <URL>` connects to the JSON-RPC endpoint and pulls fresh chain state.
- Output: `block_number=<N> chain_id=<ID> nonce=<N>` (same format as `eth wallet balance`).
- Exit 0 on success. Exit 3 on RPC failure.
- A subsequent `eth wallet balance` reflects the synced state without a second sync (cached nonce + block number).
- **Note (per #297 B3):** v0.2 uses stateless mnemonic inline; named-wallet model is Story 12, deferred to v0.3.

---

## Story 5 — Send native ETH (Alice)

> As Alice, I want to send 0.01 ETH to a Sepolia address with a known fee tier, so I can complete a payment in one command.

**Acceptance criteria:**

- `eth wallet send --mnemonic <words> --network sepolia --to 0xAbC...123 --amount 0.01 --rpc-url <URL>` builds, signs (EIP-1559 type 2), and broadcasts.
- Default fee tier is `half_hour` (target ~3 blocks). Override via `--fee fastest|half_hour|hour|economy` or `--max-fee-gwei 30 --priority-fee-gwei 2`.
- Output on success: `sent. tx_hash: 0xDeF...456 (nonce: 7, gas_used: 21000, max_fee: 30 gwei, priority_fee: 2 gwei)`.
- Exit 0 on broadcast. Exit 4 on insufficient funds. Exit 5 on signing/RPC error.
- `--dry-run` builds + signs but does not broadcast; prints the signed-tx hex + JSON-decoded fields.
- `--wait` waits for the tx to be mined and prints the receipt (`status: success`, `block: 1234567`, `gas_used: 21000`).
- EIP-1559 default: `max_fee_per_gas = base_fee + priority_fee`, `max_priority_fee_per_gas = priority_fee`.
- **Note:** EVM has no native multi-recipient tx. The `--batch` flag (see Story 13) fires N sequential `send_transaction` calls.

---

## Story 6 — Send with custom EIP-1559 fee (Bob)

> As Bob, I want to specify `max_fee_per_gas` and `max_priority_fee_per_gas` in gwei, so I can match my back-end's fee policy exactly.

**Acceptance criteria:**

- `eth send --mnemonic <words> --to 0xAbC... --amount 0.01 --max-fee-gwei 50 --priority-fee-gwei 3` builds an EIP-1559 tx with exact values.
- Validation: `--priority-fee-gwei` must be `<= --max-fee-gwei` (exit 2 with `priority_fee must be <= max_fee`).
- Validation: `--max-fee-gwei` must be `>= 1 gwei` (exit 2 with `max_fee must be >= 1 gwei`).
- Output includes the effective values used (might differ slightly from requested if `provider.fee_history` reports a higher base fee at signing time).

---

## Story 7 — Inspect transaction history (Alice)

> As Alice, I want to list my past transactions with confirmations, so I can reconcile my records.

**Acceptance criteria:**

- `eth tx list --mnemonic <words> --network sepolia --rpc-url <URL> --since-block 1000000` queries historical blocks and prints a table: `tx_hash | direction | amount | gas_used | confirmations | block_number | timestamp`.
- Default 25 most recent txs. `--limit N` overrides.
- `--json` outputs a JSON array (for piping to `jq`).
- Unconfirmed txs show `confirmations: 0` with `pending` tag (requires `--pending` flag to include).
- `eth tx get --tx-hash 0xDeF... --rpc-url <URL>` returns full details of one tx (decoded fields + receipt status).
- Exit 0 even if no transactions yet.
- **Note (per #297 M2):** v0.2 maintains an in-memory block cache (block_number + nonce per call, from Story 4 `sync`) but NOT a local tx-history index. Every `tx list` call scans blocks (slow). Local tx index is a v0.3 enhancement.

---

## Story 8 — Get current gas estimates (Bob, Carol)

> As Carol, I want to see the current gas tiers before sending, so I can pick the right speed/cost trade-off.

**Acceptance criteria:**

- `eth fee --network sepolia --rpc-url <URL>` prints a table:

  ```text
  fastest:     50 gwei  (base: 30, priority: 20)
  half_hour:   35 gwei  (base: 30, priority: 5)
  hour:        32 gwei  (base: 30, priority: 2)
  economy:     31 gwei  (base: 30, priority: 1)
  ```

- Tier derivation (per #297 G1): `fastest` = 95th percentile, `half_hour` = 80th, `hour` = 70th, `economy` = 50th — over last 20 blocks via `eth_feeHistory`. Max fee = base fee (next block estimate) + priority.
- Output refreshed on every call (live fetch, not cached).
- Exit 3 on RPC failure.
- `--json` outputs the same as a JSON object with schema `{tier: string, max_fee_gwei: number, base_fee_gwei: number, priority_fee_gwei: number}` (per #297 M6).

---

## Story 9 — List / show / delete / rename wallets (Alice, Bob) **[DEPRECATED → Story 12 in v0.3]**

> As Alice, I want to list all my wallets in the data directory and manage them, so I can pick one quickly.

**Acceptance criteria:**

- `eth wallet list` prints one wallet name per line.
- `eth wallet list --json` outputs a JSON array of `{name, network, address, derivation_path, created_at}`.
- `eth wallet show --name w` prints full info: network, chain id, address (EIP-55), derivation path, wallet name, file path, created_at.
- `eth wallet show --name w --addresses` exports the first 5 receive addresses (m/44'/60'/0'/0/0 through m/44'/60'/0'/0/4).
- `eth wallet delete --name w` removes the wallet (mnemonic.txt + cached state). Prints `wallet 'w' deleted.` Exits 4 if the wallet doesn't exist.
- `eth wallet rename --name w --to w2` renames the wallet in place. Exits 4 if `w2` already exists.
- Empty directory: `eth wallet list` prints `(no wallets)` and exits 0.
- Corrupt wallets (no `mnemonic.txt`): listed but marked `(corrupt — missing mnemonic.txt)`.
- **Note (per #297 B3):** v0.2 MVP only; superseded by Story 12 in v0.3.

---

## Story 10 — Use mainnet explicitly (Alice) **[DEPRECATED → Story 12 in v0.3]**

> As Alice, I want to create and use a mainnet wallet, so I can use real ETH.

**Acceptance criteria:**

- `eth wallet create --name main --network mainnet` produces an EIP-55 checksummed address (`0xAbCd...123`).
- `provider.get_chain_id()` is called at startup and must return `0x1` (1) — fails fast with exit 3 if the RPC endpoint disagrees.
- Default RPC URL for mainnet is `https://cloudflare-eth.com`.
- A confirmation prompt requires typing `yes` to proceed; default is abort.
- Output shows `WARNING: this wallet uses real ETH on Ethereum mainnet. Funds are at risk.` before the mnemonic.
- Exit 1 if user does not type `yes`.
- **Note (per #297 B3):** v0.2 MVP only; superseded by Story 12 in v0.3.

---

## Story 11 — Show config + debug info (Bob)

> As Bob, I want to see what the CLI thinks the current network and RPC URL are, so I can debug "why is this connecting to the wrong place".

**Acceptance criteria:**

- `eth config show` prints: data dir, RPC URL, network, chain id (after `--rpc-url` lookup), list of loaded wallets, derivation path default, default fee tier.
- `eth config show --json` outputs the same as JSON.
- Exit 0 always.
- Diagnostic output includes the version string `eth 0.2.0` (via `--version`) and the alloy version (`alloy 1.8.x`).

---

## Story 12 — Persist wallet across CLI invocations (Alice) **[v0.3 — per #297 B3]**

> As Alice, I want each CLI invocation to find my wallets without re-deriving from the mnemonic, so it's fast.

**Acceptance criteria:**

- First `eth wallet create` writes an encrypted mnemonic to `$XDG_DATA_HOME/eth/wallets/<network>/<wallet_id>.enc` per ADR 0001 (UUID wallet id, like BTC v0.1). Argon2id + AES-256-GCM (F5/F6) — no plaintext on disk.
- Subsequent `eth wallet show --id <wallet_id> --network <NET>` prompts unlock (or reads `ETH_WALLET_PASSPHRASE`), reads the encrypted mnemonic, derives keys, syncs via RPC, prints the wallet state.
- If the data dir is on slow disk (HDD, network FS), `eth wallet show` still completes in <500ms after sync.
- If the file is missing or unreadable, the command exits 2 with `wallet '<id>' is missing or corrupt`.
- **Note (per #297 B1 + B2):** v0.2 uses UUID-based wallet IDs (internal `wallet_id`) with user-facing `--name` flag for cross-wallet operations. v0.2 ships encryption from day 1; no plaintext fallback.

---

## Story 13 — Send to multiple recipients (sequential txs) (Alice)

> As Alice, I want to send to several addresses in quick succession, so I can pay multiple recipients without re-typing the command.

**Acceptance criteria:**

- `eth send --mnemonic <words> --batch <file>` reads a CSV file (per #297 M3: format = `address,amount_eth` per line; no header row; trim whitespace; ignore blank lines and `# comment` lines) and sends each as a separate transaction.
- Up to 100 recipients per `--batch` (CLI-enforced; exit 2 with `max 100 recipients per batch`).
- Default fee tier `half_hour`. Override via `--fee fastest|...` or `--max-fee-gwei`.
- Output: `batch sent. count: 3, txs: [0xAbC..., 0xDeF..., 0x123...], total: 0.05 ETH, total_fees: 0.00021 ETH`.
- Exit 0 if all broadcast; non-zero if any failed (prints which recipient + which tx hash failed).
- `--stop-on-error` (default) aborts the batch on the first failure; `--continue` sends the rest.
- **Note:** EVM has no native multi-output tx. Each recipient is a separate tx with a separate nonce + fee.

---

## Story 14 — Sweep / drain wallet to one address (Alice)

> As Alice, I want to sweep my entire ETH balance to one address, so I can consolidate funds before re-organizing my wallets.

**Acceptance criteria:**

- `eth send --mnemonic <words> --drain --to 0xAbC... --rpc-url <URL>` builds a transaction that sends `balance - gas_estimate` to `to`, leaving 0 (or near-0) ETH behind.
- Gas is estimated via `provider.estimate_gas` + the current `max_fee_per_gas` from the fee tier.
- Default fee tier `half_hour`. Override via `--max-fee-gwei`.
- Output: `drained. tx_hash: 0xDeF... (sent: 1.2345 ETH, fee: 0.00021 ETH, leftover: < 1 wei)`.
- Exit 4 if balance < gas_estimate (i.e., nothing to sweep).
- Confirmation prompt: `Drain wallet 'w' to 0xAbC...? Type 'yes' to confirm.` (default abort).

---

## Story 15 — Choose nonce strategy (auto vs manual) (Bob)

> As Bob, I want to pick between auto-nonce (read from RPC) and manual nonce (supply my own), so I can match my back-end's tx-batching policy.

**Acceptance criteria:**

- `eth send --mnemonic <words> --to 0xAbC... --amount 0.01` (default) uses auto-nonce: `provider.get_transaction_count(signer.address())`.
- `eth send --mnemonic <words> --to 0xAbC... --amount 0.01 --nonce 42` uses the supplied nonce.
- Validation: manual nonce must equal `current_nonce + k` for some k >= 0 (exit 2 with `nonce < current_nonce` if too low).
- Output always prints the actual nonce used (whether auto or manual).
- **Note:** auto-nonce uses a single `eth_getTransactionCount` RPC call. No local nonce cache. Safe for single-threaded CLI use; multi-process concurrent sends need manual nonce (Story 16).

---

## Story 16 — Manual nonce + gas limit override (Bob)

> As Bob, I want to supply an exact nonce and gas limit, so I can replace or compose transactions deterministically.

**Acceptance criteria:**

- `eth send --mnemonic <words> --to 0xAbC... --amount 0.01 --nonce 42 --gas-limit 50000` uses exactly these values.
- `--gas-limit` must be `>= 21000` (the intrinsic gas for a value transfer). Exit 2 with `gas_limit must be >= 21000` if lower.
- For ERC-20 transfers, the CLI auto-detects ~65,000 as a reasonable default; manual override allowed via `--gas-limit`.
- Output includes the supplied nonce + gas limit alongside the auto-detected values.
- Useful as the building block for Story 17 (speed-up) and Story 13 (batch with deterministic ordering).

---

## Story 17 — Replace / speed-up tx (same nonce, higher fee) (Alice)

> As Alice, I want to speed up a stuck transaction by sending a new one with the same nonce but a higher fee, so I don't have to wait for the original to drop.

**Acceptance criteria:**

- `eth send speed-up --mnemonic <words> --tx-hash 0xAbC... --rpc-url <URL> --max-fee-gwei 60` reads the original tx (via `provider.get_transaction_by_hash`), extracts its nonce + to + value + input, builds a new tx with the same nonce + higher `max_fee_per_gas`, signs + broadcasts.
- `--max-fee-gwei` must be `>` the original tx's `max_fee_per_gas`. Exit 2 with `new max_fee must exceed original` if not.
- Output on success: `sped up. new_tx_hash: 0xDeF... (old_tx_hash: 0xAbC..., nonce: 7, fee_increase: 30 gwei)`.
- Exit 4 if original tx not found in last N blocks (`--lookback-blocks`, default 200, per #297 G5). No mempool lookup in v0.2 (`subscribe_pending_transactions` deferred to v0.3).
- Exit 5 if the original tx is already mined and confirmed (no point speeding up).
- **Note:** this is the EVM equivalent of BTC's RBF (BIP-125). EVM has no native RBF signaling — same-nonce + higher-fee is the convention.

---

## Story 18 — Sign EIP-191 personal message (Alice)

> As Alice, I want to sign a message with my wallet's private key (EIP-191 `personal_sign`), so I can prove ownership of an address (e.g., for an airdrop claim, SIWE login, or signed-message board).

**Acceptance criteria:**

- `eth sign-message --mnemonic <words> --message "I own this address"` signs with EIP-191 prefix (`\x19Ethereum Signed Message:\n` + varint(len) + message).
- Default output: hex of the 65-byte signature (`r || s || v`).
- `--address <addr>` signs with the key for `<addr>` (must be wallet-owned); default is the first receive address.
- Output format: `address: 0xAbC...\nsignature: 0xDeF...` (so the verifier has both pieces).
- `--verify <recovered_addr>` is a sanity check: `signature.recover_address_from_msg(message) == <recovered_addr>`.
- Exit 0 on success. Exit 4 if `--address` is not wallet-owned.

---

## Story 19 — Export the wallet xpub + first addresses (Bob)

> As Bob, I want to export my wallet's xpub and the first few receive addresses, so I can import the watch-only descriptor into a hardware-wallet companion or a block explorer.

**Acceptance criteria:**

- `eth wallet show --name w --export` prints the BIP-32 extended public key (`xpub...` for mainnet, `tpub...` for testnet — different version bytes per SLIP-0132) + the first 5 receive addresses.
- Output format: `xpub: xpub6CU...\naddress_0: 0xAbC...\naddress_1: 0xDeF...\naddress_2: 0x123...\naddress_3: 0x456...\naddress_4: 0x789...`.
- One line per item, easy to pipe to `pbcopy`.
- No private-key material in this output (the xpub is public-by-design).
- Exit 0 always (xpub export is not a security-sensitive operation).

**Note:** unlike BTC, ETH has no descriptor concept (no script template — every address is the same `keccak256(pubkey)[12..]`). The xpub is the only thing to share for watch-only.

---

## Story 20 — Pick derivation path (Ledger vs MetaMask) (Alice)

> As Alice, I want to choose between Ledger-style and MetaMask-style derivation paths, so I can match the wallet that originally generated my addresses.

**Acceptance criteria:**

- `eth wallet create --name w --derivation-path m/44'/60'/0'/0/0` (Ledger-style, default — account slot = 0, address slot = 0).
- `--address-index N` shorthand (per #297 G2): expands to `m/44'/60'/0'/0/<N>` (BIP-44 address index, 5th position).
- `--account-index M` shorthand: expands to `m/44'/60'/<M>'/0/0` (BIP-44 account index, 3rd position).
- `--derivation-path`, `--address-index`, and `--account-index` are mutually exclusive. Exit 2 with `pick one of --derivation-path, --address-index, or --account-index`.
- Validation: path must start with `m/44'/60'/`. Exit 2 with `derivation path must start with m/44'/60'/`.
- Output always shows the path used, so the user can verify.

---

## Story 21 — Send ERC-20 stablecoin (USDT/USDC) (Alice)

> As Alice, I want to send 1.50 USDC to a recipient with a single command, so I can pay in stablecoin without worrying about ETH price volatility.

**Acceptance criteria:**

- `eth erc20 send --mnemonic <words> --token USDC --to 0xAbC... --amount 1.5 --rpc-url <URL>` builds + signs + broadcasts a `transfer(address,uint256)` call to the USDC contract.
- `--token USDC` resolves to `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` (mainnet) or the Sepolia equivalent.
- `--amount 1.5` is interpreted as `1.5 * 10^decimals` base units. USDC/USDT use 6 decimals, so `1.5 USDC = 1_500_000` base units. The CLI fetches `decimals()` once per token (cached).
- Tx shape: `to = token_contract`, `value = 0` (no ETH sent), `input = transferCall { to: recipient, value: 1_500_000 }.abi_encode()`.
- Gas limit auto-estimated via `provider.estimate_gas`; override via `--gas-limit 100000`.
- Output on success: `sent. tx_hash: 0xDeF... (token: USDC, amount: 1.5, decimals: 6, gas_used: 65000)`.
- Exit 0 on broadcast. Exit 4 if the wallet's USDC balance < `1.5 * 10^6` (insufficient token balance — separate from ETH-for-gas check).
- Validation: `--amount` must be `> 0` (exit 2 with `amount must be > 0`).
- Validation: token must be in the registry (`Story 23`); unknown `--token SYMBOL` fails fast (exit 2 with `unknown token: SYMBOL. Use --token-address to register a custom one`).
- `--token-address 0xDeF...` overrides `--token SYMBOL` and uses the supplied contract directly (skips registry).

---

## Story 22 — Check ERC-20 token balance (Alice, Carol)

> As Carol, I want to see my USDC and USDT balances next to my ETH balance, so I know my total stablecoin holdings.

**Acceptance criteria:**

- `eth wallet balance --address <addr> --token <USDC_ADDR> --rpc-url <URL>` prints `<scaled> <token-addr>` (e.g. `15.000000 0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238`), with decimals auto-detected via `decimals()` `eth_call` (selector `0x313ce567`) or `--decimals <N>` override.
- **Drift from original spec (resolved via #356, commit `9a76f1c` on `rust-eth-core`):** spec body proposed `eth erc20 balance --mnemonic <words> --token USDC` as a separate subcommand; impl extends the existing `eth wallet balance` with `--token <ADDRESS>` (single subcommand covers ETH + token). `<mnemonic>` not required (balance is read-only). `--all` / `--json` deferred (per #356 out-of-scope: multi-token batch queries).
- **Drift from CLI surface:** spec said `--token USDC` (symbol); impl uses `--token <ADDRESS>` (contract address). Symbol resolution deferred.
- Exit 0 even if balance is 0. Exit 2 on invalid `--token` address (InvalidInput) or on `balanceOf`/`decimals()` ABI decode failure (per #357 `Error::AbiDecodeFailed` — contract claims to be ERC-20 but isn't). Exit 3 on RPC failure (transport-layer, not decode).
- **Note:** requires `sol! { function balanceOf(address) external view returns (uint256); }` + raw `provider.call` — does not require deploying a contract binding. Plus `sol! { function decimals() external view returns (uint8); }` for the auto-detect path.

---

## Story 23 — List registered stablecoins / tokens (Bob)

> As Bob, I want to see the list of supported ERC-20 tokens, so I know which ones I can `--token USDC` against without `--token-address`.

**Acceptance criteria:**

- `eth erc20 list` prints a table:

  ```text
  SYMBOL  ADDRESS                                     DECIMALS  NETWORK
  USDC    0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48    6      mainnet
  USDT    0xdAC17F958D2ee523a2206206994597C13D831ec7    6      mainnet
  USDC    0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238    6      sepolia
  ```

- `--json` outputs the same as a JSON array.
- Tokens are loaded from two sources (per #297 G3):
  1. **Bundled** (compile-time via `include_str!`): `rust-wallet-app/crates/eth/tokens/mainnet.json` + `sepolia.json` + `anvil.json`.
  2. **User** (runtime from `$XDG_CONFIG_HOME/eth/tokens/<network>.json`): operator-added tokens from Story 24.
- **Resolution rule:** user registry wins over bundled on symbol collision. `--list --include-bundled` prints both with a source tag (`bundled` / `user`).
- Empty registry: prints `(no tokens registered)` and exits 0.

---

## Story 24 — Add custom ERC-20 token by contract address (Alice)

> As Alice, I want to add a custom ERC-20 token by supplying its contract address, so I can send/receive tokens that aren't in the bundled registry.

**Acceptance criteria:**

- `eth erc20 register --address 0xDeF... --rpc-url <URL>` queries `decimals()` + `symbol()` + `name()` via raw `provider.call`, then writes the token entry to `$XDG_CONFIG_HOME/eth/tokens/<network>.json` (operator-editable, separate from bundled).
- The new token can immediately be used as `--token SYMBOL` in `eth erc20 send` (resolution checks both bundled + user registries).
- `eth erc20 register --list` shows user-added tokens.
- `eth erc20 register --remove --symbol FOOBAR` removes a user-added token.
- Validation: `decimals()` must return 0-36 (exit 2 with `invalid decimals: <N>` if out of range).
- Validation: `symbol()` must return 1-11 printable ASCII chars (exit 2 with `invalid symbol: <s>`).
- Validation: `name()` must return 1-256 chars of UTF-8 with no control characters (exit 2 with `invalid name: <s>`, per #297 M7).

---

## Story 25 — Approve ERC-20 spending (for DEX) (Bob)

> As Bob, I want to approve a DEX router contract to spend my USDC, so I can swap tokens without manually signing every transfer.

**Acceptance criteria:**

- `eth erc20 approve --mnemonic <words> --token USDC --spender 0xRouter... --amount 100 --rpc-url <URL>` builds + signs + broadcasts an `approve(address spender, uint256 value)` call.
- `--amount 100` = `100 * 10^6` base units (USDC decimals applied).
- Tx shape: `to = token_contract`, `value = 0`, `input = approveCall { spender, value }.abi_encode()`.
- Output on success: `approved. tx_hash: 0xDeF... (token: USDC, spender: 0xRouter..., allowance: 100)`.
- `--amount unlimited` (or `--amount max`) sets `value = U256::MAX` (the "infinite approval" pattern). Confirmation prompt required: `Setting unlimited allowance to 0xRouter... Type 'yes' to confirm.`
- Exit 0 on broadcast. Exit 4 if wallet USDC balance < gas estimate (i.e., not enough ETH to pay gas for the approve tx).

---

## Story 26 — Use Anvil local node for testing (Alice)

> As Alice, I want to point the CLI at a local Anvil instance, so I can test end-to-end without spending Sepolia ETH.

**Acceptance criteria:**

- `eth --network anvil --rpc-url http://localhost:8545 wallet create --name dev` works against Anvil (chain id 31337 = `0x7a69`).
- `provider.get_chain_id()` is asserted at startup; mismatch with `--network anvil` fails fast (exit 3 with `expected chain_id 31337, got <N>`).
- Anvil's prefunded accounts (a long list of `0x...` private keys known to Anvil) can be imported via `eth wallet import --private-key 0xac0974...` (the standard Anvil dev key #0).
- `eth erc20 deploy --token-name Foo --token-symbol FOO --decimals 18` deploys a `MockERC20` contract to Anvil (per #297 M8: uses `sol!` macro + compile-time bytecode embedded via `include_bytes!`, matching Plan Task 8 / Q8 resolution). Returns the deployed contract address.
- Anvil integration uses `alloy-node-bindings::AnvilInstance` as a `[dev-dependencies]` only (not in production builds).

---

## Story 27 — Sign EIP-712 typed data (Bob) **[v0.2 — per #297 D1, un-deferred from v0.3]**

> As Bob, I want to sign EIP-712 typed structured data (e.g., a `Permit` message for gasless approvals, or a `MetaMask` `Order` for a DEX), so I can interact with dApps that require typed-data signatures.

**Acceptance criteria:**

- `eth sign-typed --mnemonic <words> --typed-data '<JSON>'` parses a JSON `TypedData` (EIP-712) and signs with `PrivateKeySigner::sign_typed_data_sync`.
- Default input format: JSON file (`eth sign-typed --typed-data-file <path>`). Inline string also accepted.
- Output format: `address: 0xAbC...\ndigest: 0x...32bytes\nsignature: 0x...65bytes`.
- `--verify <recovered_addr>` sanity-check via `signature.recover_typed_data(&typed_data)`.
- Exit 0 on success. Exit 2 if JSON is malformed. Exit 4 if `--address` is not wallet-owned.
- **Note (v0.2 subset, per #297 D1):** v0.2 ships `sign_typed_data_sync` + `signature.recover_typed_data(&typed_data)` only. Full domain-separation + complex nested types land in v0.3.

---

## Story 28 — Connect to RPC endpoint with SPKI pin (Alice, Bob) **[v0.3.x — issue #393]**

> As Alice, I want to pin the SPKI hash of my RPC endpoint so I detect MITM certificate swaps, so I can transact against `https://cloudflare-eth.com` or other public RPCs from a hostile network without trusting the system CA store.

**Acceptance criteria:**

- `eth --rpc-url pinned://<spki-hex>@ethereum-rpc.publicnode.com config show` parses the `pinned://` scheme + extracts the SPKI pin (32-byte SHA-256 hex) from the URL.
- `provider.new_http_pinned(url, &spki_bytes)` ([`provider.rs`](../../rust-wallet-app/crates/eth-wallet-core/src/provider.rs)) builds a `RootProvider` over a raw `reqwest::Client` whose `rustls::ServerCertVerifier` rejects any cert whose SubjectPublicKeyInfo SHA-256 ≠ the pinned hash.
- **Library test:** `tests/spki_pin_localnet.rs::spki_pinned_endpoint_rejects_wrong_pin` connects to a known HTTPS endpoint with a wrong pin (all-zero bytes) and asserts the error class is `Error::SpkiPinMismatch { expected, actual }` (or equivalent).
- **Library test:** same file `spki_pinned_endpoint_accepts_correct_pin` connects to a real public RPC with the cert's actual SPKI pinned and asserts the JSON-RPC call returns successfully (or returns the expected transport-level error, not pin-mismatch).
- **Anvil gateway:** `spki_pinned_localhost_anvil_succeeds` — pin match path works against a locally-spawned HTTPS Anvil with a known self-signed cert (chain-id 31337 still matches `--network anvil` assertion).
- F20 SPKI pin finding closed when these tests land.
- `--allow-insecure-tls` (debug only) bypasses the pin verifier with a `tracing::warn!`.

---

## Story 29 — Connect to RPC endpoint without SPKI pin (system CAs only) (Alice) **[v0.3 — current default]**

> As Alice, I want to opt out of SPKI pinning for local development or trusted-network use, so I can point `eth` at a local Anvil or LAN RPC node without pinning ceremony.

**Acceptance criteria:**

- `eth --rpc-url http://127.0.0.1:8545 ...` uses `provider.new_http(url)` ([`provider.rs`](../../rust-wallet-app/crates/eth-wallet-core/src/provider.rs)) — system trust store + `webpki-roots`. **No pinning. Validates the trust-on-first-use contract for localhost + LAN nodes.**
- `eth --rpc-url https://cloudflare-eth.com ...` (without `pinned://`) uses the same `new_http` path — system CAs validate Cloudflare's cert. Convenient but accepts any cert the OS trusts (no MITM detection).
- **Library test:** `tests/spki_pin_localnet.rs::no_pin_localhost_anvil_succeeds` spawns Anvil via `alloy_node_bindings::AnvilInstance`, connects via `new_http("http://127.0.0.1:<port>")`, asserts `provider.get_chain_id() == 31337`.
- **Library test:** same file `no_pin_localhost_anvil_rejects_wrong_chain_id` asserts the chain-id trust-boundary guard (`handlers.rs:650-659`) fires when Anvil reports a non-31337 chain id — failure surface is `Error::ChainIdMismatch { expected: 31337, actual: <N> }`.
- Test pattern matches the existing `tests/erc20_anvil.rs` suite (Story 26 already shipped this path).

---

## Network research — local + testnet picks (added 2026-08-26, session verification)

**Local node:** **Anvil** (`alloy_node_bindings::AnvilInstance`, foundry-rs/foundry). Reasons:

- Already in `[dev-dependencies]` of both `eth-wallet-core` and `eth` crates (`Cargo.toml:60` + `eth/Cargo.toml:54`).
- Fastest startup (~50 ms), zero-config chain-id 31337, prefunded deterministic accounts (`0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` is account #0, 10000 ETH).
- Matches existing `tests/erc20_anvil.rs` pattern (Story 26 already shipped).
- Alternatives rejected: Hardhat Node (requires npm toolchain + `npx hardhat node`, slower cold start), Ganache (legacy, deprecated, no `alloy-node-bindings` integration).

**Testnet:** **Sepolia** (chain-id 11155111). Reasons:

- Most-used L1 testnet across the ecosystem (all major wallets, Etherscan, Alchemy, Infura default to Sepolia).
- Already integrated in `rust-wallet-app` (`scripts/eth-send-sepolia-e2e.sh`, `scripts/eth_sepolia.sh`, `tests/cli_sepolia.rs`).
- USDC Sepolia contract published by Circle (`0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238`).
- Alternatives rejected: Holesky (deprecated for staking 2025-Q4, faucet reliability degraded), Hoodi (newest replacement for Holesky but tooling support thin).

**Test matrix — Stories 28 + 29:**

| Scenario | Network | SPKI pin | Test name | Status |
|---|---|---|---|---|
| Local dev, no pin | Anvil (31337) | None | `no_pin_localhost_anvil_succeeds` | ready to implement (this PR) |
| Local dev, pin mismatch | HTTPS Anvil (self-signed cert) | Wrong pin | `spki_pinned_endpoint_rejects_wrong_pin` | blocks on issue #393 `new_http_pinned` impl |
| Local dev, pin match | HTTPS Anvil (self-signed cert) | Correct pin (cert's actual SPKI) | `spki_pinned_localhost_anvil_succeeds` | blocks on issue #393 |
| Testnet, no pin | Sepolia (11155111) | None | `no_pin_sepolia_get_chain_id` (L29-gated) | ready (extend existing `cli_sepolia.rs`) |
| Testnet, pin match | Sepolia (11155111) | Public RPC SPKI | `spki_pinned_sepolia_succeeds` | blocks on issue #393 |

Implementation order: Story 29 first (no-SPKI Anvil test, ships in this PR as the canonical "setup + write test on local testnet" deliverable), then Story 28 once issue #393 lands `new_http_pinned`.

---

## Cross-cutting acceptance criteria (apply to all stories)

- **Help text:** every command accepts `--help` and prints a clear, multi-line description with examples.
- **Exit codes (per #297 M11, refined by #357):** documented and stable (0 = success, 1 = user abort, 2 = bad input — operator passed a value that doesn't fit the function surface, *or* an ABI decode failure on a Solidity call whose response was well-formed HTTP but wrong shape — `Error::AbiDecodeFailed` per #357 distinguishes decode fail from transport fail), 3 = upstream/RPC transport failure (connection refused, HTTP error, chain-id mismatch, malformed response), 4 = wallet/balance issue (insufficient funds, unknown wallet, insufficient token balance, missing pre-image), 5 = signing/RPC/broadcast error.
- **Tx types (per #297 G6):** Reads accept legacy (type 0) + EIP-2930 (type 1) + EIP-1559 (type 2). Writes = EIP-1559 only in v0.2. Pre-London `eth send` is rejected with exit 2; pre-London reads in tx-history work transparently.
- **Mnemonic at rest (per #297 B2):** encrypted with Argon2id + AES-256-GCM (F5/F6) — never plaintext. Operator must run `eth wallet unlock` (or set `ETH_WALLET_PASSPHRASE`) on every CLI call that touches key material.
- **Output:** human-readable by default; `--json` flag on every command that produces data.
- **Stderr for diagnostics:** logs and errors go to stderr; stdout contains only the requested data (safe to pipe).
- **No background processes:** every `eth` invocation is a single foreground command. No daemons.
- **No telemetry:** the CLI makes no network calls except to the configured RPC URL.
- **Address unit:** all addresses display in EIP-55 mixed-case checksum (`0xAbCd...123`), not all-lowercase. Override via `--address-format checksum|lowercase`.
- **Amount unit:** all amount flags accept ETH (`--amount 0.5`) or wei (`--amount-wei 500000000000000000`) — both are valid; reject ambiguity in one command (exit 2 if both given). For ERC-20, `--amount` uses the token's `decimals()`.
- **Gas unit:** all gas-related flags accept gwei (`--max-fee-gwei 30`) or wei (`--max-fee-wei 30000000000`). Reject ambiguity.
- **Confirmation prompts** (`mainnet`, `drain`, `unlimited approval`, `--no-private` export): require typing `yes` (not `y`); default is abort; exit code 1 on abort.
- **Bounded inputs:** batch limited to 100 recipients (Story 13); `--gas-limit` bounded `>= 21000`; derivation path must start with `m/44'/60'/`.
- **TLS pinning (per #297 M10):** when `--rpc-url` points to a pinned endpoint (e.g., `https://cloudflare-eth.com`), the custom SPKI verifier applies. Cross-ref: V7 spike `rust-wallet-app/spikes/alloy-v1/tests/v7_spki_pin.rs` + Bitcoin `bitcoin-wallet-core/src/chain/spki.rs` (F20 SPKI pin pattern). `--allow-insecure-tls` flag disables pinning (debug only).

---

## Out of scope for v1 (separate user stories when shipped)

- **EIP-712 typed-data signing** (Story 27). Needed for some ERC-20 approvals + DEX interactions; alloy supports it via `sign_typed_data_sync`. **v0.2 ships `sign_typed_data_sync` + `signature.recover_typed_data` per #297 D1.** Full domain-separation + complex nested types remain v0.3.
- **L2 chains** (Optimism, Arbitrum, Base, Polygon). The `ChainId::Ethereum(u32)` placeholder in `chain-traits/src/lib.rs:21` already supports the discriminator — drop in another chain-id constant + another RPC URL. UX: `--network optimism|arbitrum|base|polygon` flag.
- **ENS resolution** (`alice.eth` → address). `alloy-ens` sub-crate exists if added later.
- **Hardware wallet** (Ledger, Trezor, Keystone) via `alloy-signer-ledger` / `alloy-signer-trezor`.
- **EIP-4337 account abstraction** (smart contract wallets — Safe, ERC-4337 EntryPoint). Different wallet model entirely.
- **Contract deployment** (only contract calls in v0.2). `sol!` + `Contract::deploy` covers it when added.
- **Flashbots / MEV protection** (`alloy-signer-flashbots`). v1.x.
- **WebSocket subscriptions** (`subscribe_logs`, `subscribe_pending_transactions`). CLI is request/response; subscriptions belong in a long-running daemon.
- **Multi-sig wallets** (Safe, Gnosis Safe). Different signer model.
- **Watch-only wallet import** from an xpub (no signing key). v0.3 (mirrors Bitcoin Plan Task 32).
- **Local tx index** (cached tx history; Story 7 "every `tx list` call scans blocks" workaround). v0.3 (per #297 D2).
- **Encrypted mnemonic at rest** (Argon2id + AES-256-GCM per F5/F6). v0.3 (mirrors Bitcoin v0.2).
- **Silent payments / payment codes**. No mature Rust SDK for EVM.
- **Other EVM chains** (BSC, Avalanche C-Chain, Fantom). Add via chain-id constant + RPC URL, no code changes.
- **Plausible-deniability multi-bucket wallet**. v1.0+.
- **REST/HTTP interface** (Breez-style server). Separate spec. Owner: TBD (per #297 M12 — open follow-up needed).
- **Mobile (iOS/Android) integration**. Phase 2 via UniFFI (mirrors Bitcoin Phase 2).

---

## v0.3.0 release status (Issue #311)

Status snapshot for the `eth-wallet-core v0.3.0` + `eth` CLI scaffold release cut (PR #325). 21 stories v0.3.0-active (library surface functional + token registry + Sepolia smoke); 5 stories remain `[ ]` per DEPRECATION → v0.3 Story 12.

### Active v0.3.0 stories (21) — `[x]` shipped

- [x] **Story 1** — Create a new wallet. `eth wallet create` (CLI scaffold) / `mnemonic::generate_12_word()` + `mnemonic::derive_address(&Mnemonic, u32)` (library). Try it: `cargo run -p eth -- wallet create` (scaffold) | `cargo test -p eth-wallet-core --test mnemonic`
- [x] **Story 2** — Import an existing wallet. `eth wallet import --phrase <words>` (CLI scaffold) / `mnemonic::derive_address(&Mnemonic, u32)` from parsed phrase (library). Try it: `cargo run -p eth -- wallet import --phrase "<12 words>"` (scaffold)
- [x] **Story 5** — Send native ETH. `eth tx send --to <addr> --value <wei>` (CLI scaffold) / `sign_native_eth_tx` + `provider.send_raw_transaction` (library). Try it: `cargo run -p eth -- tx send --to 0x... --value 1000000000000000` (scaffold)
- [x] **Story 6** — Send with custom EIP-1559 fee. CLI `--max-fee-per-gas <wei>` + `--max-priority-fee-per-gas <wei>` (u128, both required or both omitted); env vars `ETH_MAX_FEE_PER_GAS` + `ETH_MAX_PRIORITY_FEE_PER_GAS` (PR #341 precedence). Try it: `RUN_ANVIL_E2E=1 ETH_RPC_URL=http://localhost:8545 cargo run -p eth -- send --name alpha --password pwd --network anvil --to 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 --amount 1 --max-fee-per-gas 99000000000 --max-priority-fee-per-gas 2000000000` (L29-gated; Anvil-gated end-to-end test in `cli_localnet.rs:2095+`)
- [x] **Story 7** — Inspect transaction history. `eth chain tx <hash>` (CLI scaffold) / `provider.get_transaction_by_hash` + `get_transaction_receipt` (library). Try it: `cargo test -p eth-wallet-core --test tx_list_get` (L29-gated)
- [x] **Story 8** — Get current gas estimates. `eth chain fee` (CLI scaffold) / `provider.estimate_gas` + `get_fee_history` (library). Try it: `cargo test -p eth-wallet-core --test fee` (L29-gated)
- [x] **Story 11** — Show config + debug info. `eth --version` + `eth chain info` (CLI scaffold) / `version()` (library). Try it: `cargo run -p eth -- --version` (scaffold)
- [x] **Story 13** — Send to multiple recipients (sequential txs). Library: N sequential `provider.send_transaction` calls. Try it: `cargo test -p eth-wallet-core --test wallet_send_native` (batch variant)
- [x] **Story 14** — Sweep / drain wallet to one address. Library: `provider.get_balance` + `TransactionRequest::with_value(balance - gas_estimate)`. Try it: `cargo test -p eth-wallet-core --test wallet_send_native` (drain variant)
- [x] **Story 15** — Choose nonce strategy (auto vs manual). Library: `provider.get_transaction_count` (auto) vs `with_nonce` (manual). Try it: `cargo test -p eth-wallet-core --test wallet_send_native` (nonce variants)
- [x] **Story 16** — Manual nonce + gas limit override. Library: `TransactionRequest::with_nonce` + `with_gas_limit`. Try it: `cargo test -p eth-wallet-core --test wallet_send_native` (manual-gas variant)
- [x] **Story 17** — Replace / speed-up tx (same nonce, higher fee). Library: new `TransactionRequest` with same nonce + higher `max_fee_per_gas`. CLI: `eth wallet speedup --speedup <hash> --max-fee-per-gas <N> --max-priority-fee-per-gas <N>` (Issue #381 PR). Try it: `RUN_ANVIL_E2E=1 cargo test -p eth --test wallet_send_speedup -- --ignored` (L29-gated Anvil-fork suite: 3 acceptance tests covering happy-path replacement, lower-fee rejection → exit 2 + stderr `fee too low`, mismatched-nonce rejection → exit 2 + stderr `nonce mismatch`)
- [x] **Story 18** — Sign EIP-191 personal message. `sign_message` + `signature.recover_address_from_msg`. Try it: `cargo test -p eth-wallet-core --lib signer::tests::sign_message_round_trip`
- [x] **Story 20** — Pick derivation path (Ledger vs MetaMask). Library: `MnemonicBuilder::derivation_path` override. Try it: `cargo test -p eth-wallet-core --test mnemonic`
- [x] **Story 21** — Send ERC-20 stablecoin (USDT/USDC). `eth tx erc20 --token usdc --to <addr> --value <units>` (CLI scaffold) / `sol!` + `provider.send_transaction` with calldata (library). Try it: `cargo run -p eth -- tx erc20 --token usdc --to 0x... --value 1000000` (scaffold) | `cargo test -p eth-wallet-core --test erc20_send` (L29-gated)
- [x] **Story 22** — Check ERC-20 token balance. `eth wallet balance --token <ADDR> --address <addr>` (single token — Issue #356) + `eth wallet balance --all --network mainnet` (iterate bundled registry + `--token` overrides — Issue #358) / `sol! { balanceOf(address) }` + `provider.call` (library). Try it: `cargo run -p eth -- wallet balance --address 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --all --network mainnet` | `cargo test -p eth --test cli_localnet wallet_balance_all`
- [x] **Story 23** — List registered stablecoins / tokens. `eth token list` (CLI scaffold) / token registry JSON at `rust-wallet-app/crates/eth-wallet-core/tokens/{mainnet,sepolia,anvil}.json` (library). Try it: `cargo run -p eth -- token list` (scaffold)
- [x] **Story 24** — Add custom ERC-20 token by contract address. Library: `sol! { decimals() }` + `provider.call` to fetch decimals + symbol. Try it: `cargo test -p eth-wallet-core --lib tokens::tests`
- [x] **Story 25** — Approve ERC-20 spending (for DEX). `eth token approve --token usdc --spender <addr> --value <units>` (CLI scaffold) / `sol! { approve(address, uint256) }` (library). Try it: `cargo run -p eth -- token approve --token usdc --spender 0x... --value 1000000` (scaffold) | `cargo test -p eth-wallet-core --test erc20_approve` (L29-gated)
- [x] **Story 26** — Use Anvil local node for testing. `eth --rpc-url http://localhost:8545 ...` (CLI scaffold) / `provider.get_chain_id()` assert `0x7a69` (library). Try it: `cargo test -p eth-wallet-core --test erc20_anvil` (L29-gated, `RUN_ANVIL_E2E=1`)
- [x] **Story 27** — Sign EIP-712 typed data. Library: `sign_typed_data` deferred follow-up (PR #290); `MnemonicBuilder::derivation_path` library covers the closest analog for v0.3. Try it: deferred to v0.3.x follow-up.

### DEPRECATED — stay `[ ]` (v0.3 Story 12)

- [ ] **Story 3** — Check ETH balance. *DEPRECATED → v0.3 Story 12* (named-wallet model with persisted handles; stateless mnemonic inline for v0.3.0 per #297 B3).
- [ ] **Story 4** — Sync chain state. *DEPRECATED → v0.3 Story 12* (same rationale as Story 3).
- [ ] **Story 9** — List / show / delete / rename wallets. *DEPRECATED → v0.3 Story 12* (covered library-side by WalletManager; CLI surface waits for Story 12).
- [ ] **Story 10** — Use mainnet explicitly. *DEPRECATED → v0.3 Story 12* (chain-id assertion library-side via `get_chain_id`; CLI mainnet flag waits).
- [ ] **Story 12** — Persist wallet across CLI invocations. *DEPRECATED → v0.3 only* (filesystem + UUID-based wallet dir + encrypted mnemonic is the v0.3 Story 12 milestone; v0.3.0 ships library-side via `WalletManager` + per-call `<base_dir>` arg).

### Cross-cutting (no user-facing flip)

- `--json` everywhere — `serde_json` in CLI scaffold args
- Stable exit codes per #297 M11 — `Error::exit_code()` 0..5
- `Secret<Mnemonic>` zeroize — `zeroize::Zeroizing<Mnemonic>` (F47 mirror BTC Task 30)
- EIP-55 checksum address display — `Address::to_checksum_buffer(None)`

### Drift from acceptance (per #311)

- **Story 27** listed in #311 v0.3.0-active list but library coverage is closest-analog only (`derivation_path`); full `sign_typed_data_sync` + `recover_typed_data` integration deferred per PR #290 reconcile.
- **CLI surface** is **scaffold-only** for all 21 active stories. Library primitives + token registry + Sepolia smoke ARE functional. CLI business logic deferred to follow-up issues per author's "shipped conservative" framing.
- **Story 3 + 4 + 10** appear in PR #322 (Task 11) Sepolia smoke as test targets (e.g. `wallet_balance` test, `wallet_sync` test). They DO exercise the library primitives but the user-stories doc keeps them `[ ]` per #311 explicit guidance.

### Try it — full surface

```bash
# Library surface (all functional)
cargo test -p eth-wallet-core --lib                          # 51 unit + dev-dep tests pass
cargo test -p eth-wallet-core --test mnemonic               # Story 1, 2, 20
cargo test -p eth-wallet-core --test wallet_manager          # Story 12 library-side
cargo test -p eth-wallet-core --lib signer::tests           # Story 18

# Sepolia smoke (operator-driven per L29 — set RUN_ETH_E2E=1)
RUN_ETH_E2E=1 cargo test -p eth-wallet-core --test '*'        # 13-test e2e suite

# CLI scaffold (--help is functional; bodies are scaffold-only)
cargo run -p eth -- --help                                   # show clap subcommand LAYOUT
cargo run -p eth -- wallet --help                           # wallet subcommand tree
cargo run -p eth -- tx --help                               # tx subcommand tree
cargo run -p eth -- chain --help                            # chain subcommand tree
cargo run -p eth -- token --help                            # token subcommand tree
```

### References

- Issue #311 (this release cut spec)
- Plan Task 12: `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`
- L24 (CHANGELOG + User Stories cascade)
- L21 (estimate-report + ai-cost-report cascade in `.superpowers/sdd/2026-08-23-eth-wallet-core/`)
- Related: #333 — all new test functions MUST be `async fn` + `#[tokio::test]`

---

## v0.3.x PR-A status (Issue #337, branch `task/eth-wallet-core-v0.2/13-cli-anvil-a-sync-read`, commit `262daac`, PR #338)

Post-v0.3.0 follow-up that flips 5 of the 9 LAYOUT stubs from "scaffold" to Anvil-backed. Per-issue user-confirmed split into 2 sub-PRs (PR-A = sync + read; PR-B = sign + broadcast deferred to follow-up issue).

### Stories flipped to `[x]` by PR-A (5)

- [x] **Story 1** — Create a new wallet. **PR-A wired** `eth wallet create --name --password --network` against `WalletManager::create_wallet_for_network`. Try it: `ETH_DATA_DIR=/tmp/wallet-demo cargo run -p eth -- wallet create --name alpha --password p1 --network anvil`
- [x] **Story 2** — Import an existing wallet. **PR-A wired** `eth wallet import --name --password --network --mnemonic|--private-key`. Try it: same as Story 1, swap `create` → `import --mnemonic "<12 words>"`.
- [x] **Story 9** — List / show / delete / rename wallets. **PR-A wired** `eth wallet list`, `eth wallet show --name|--id --network`, `eth wallet delete --name|--id --network` against `WalletManager::{list_wallets, lookup_by_name, delete_wallet}`. (Rename is deferred — out of scope for PR-A.) Try it: `cargo run -p eth -- wallet list` / `wallet show --name alpha` / `wallet delete --name alpha`.
- [x] **Story 3** — Check ETH balance. **PR-A wired** `eth wallet balance --address --unit [--rpc-url]` against `provider.get_balance(addr)` via `new_http(url)`. Try it: with Anvil running: `cargo run -p eth -- --rpc-url http://127.0.0.1:8545 wallet balance --address 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`. Note: previously DEPRECATED → v0.3 Story 12 per #311; PR-A promotes back to active per #337 scope.
- [x] **Story 26** — Use Anvil local node for testing. **PR-A wired** `--rpc-url` global flag + `provider.get_balance` + `provider.get_transaction_by_hash` paths. Try it: see Story 3 command above. Story 26 was already `[x]` from PR #318; PR-A extends the active surface.

### Stories still `[ ]` (PR-B deferred — 4)

- [ ] **Story 5** — Send native ETH. CLI stub returns `Err(Error::Rpc("wallet send-native: wired in PR-B follow-up (#337 phase 2)"))`. PR-B owns: sign via `unlock_signer` + broadcast via `send_raw_transaction`. Library surface already functional.
- [ ] **Story 6** — Custom EIP-1559 fee. PR-B owns: `--max-fee-gwei --priority-fee-gwei` flag wiring + nonce + gas plumbing through `TransactionRequest`.
- [ ] **Story 13/14/15/16/17** — Batch / drain / nonce strategy / manual gas / replace-by-fee. All deferred to PR-B alongside Story 5 wiring.
- [ ] **Story 21** — Send ERC-20 stablecoin. CLI stub returns PR-B error message. PR-B owns: `sign_erc20_tx_bytes` + calldata via `tokens::lookup_by_symbol` for decimals.
- [ ] **Story 7** — Transaction history (tx list). CLI stub returns PR-B error message. PR-B owns: `provider.get_logs(filter)` scan over recent blocks.
- [ ] **Story 4** — Sync chain state. `eth wallet sync` returns PR-B error message. PR-B owns: chain-state resync (deferred per #309).

### Stories unchanged (no PR-A surface impact)

- [x] **Story 18** — Sign EIP-191 personal message. Library surface only; CLI flag wiring deferred.
- [x] **Story 20** — Pick derivation path. Library surface only.
- [x] **Story 22/23/24/25** — ERC-20 surface. Library + registry surface only; CLI flag wiring deferred.
- [x] **Story 27** — Sign EIP-712 typed data. Already deferred per `sign_typed_data` follow-up (alloy eip712 feature flag decision).

### New CLI surface (PR-A)

| Flag | Surface |
|---|---|
| `--data-dir <PATH>` (global, env `ETH_DATA_DIR`) | Hermetic wallet-store location. Tests inject a `tempfile::TempDir`. |
| `--password <PWD>` (wallet create + import, required) | Wallet encryption password. ⚠️ Currently argv-visible — PR-B replaces with `rpassword` / stdin. |
| `--name <NAME>` (wallet create + import + show + delete) | User-facing wallet handle. |
| `--address <ADDR>` (wallet balance, required) | Target address for balance query. |
| `--network <NET>` (wallet create + import + show + delete + balance) | `mainnet` / `sepolia` / `anvil` / chain-id `1` / `11155111` / `31337` / `dev` / `local`. |
| `--unit <UNIT>` (wallet balance) | `wei` / `gwei` / `eth` (default `eth`). |
| `--tx-hash <HEX>` (tx get) | 32-byte hex transaction hash. |
| `--rpc-url <URL>` (global, env `ETH_RPC_URL`, default `http://127.0.0.1:8545`) | RPC endpoint for read paths. |

### Library gaps closed (PR-A side effects)

- `WalletMeta` JSON now persisted alongside `.enc` as `<wallet_id>.meta.json`. `list_wallets` returns real name/address/network (was returning placeholder `wallet-<uuid8>` / `Address::ZERO`).
- `import_wallet_for_network` honors `--network` flag (was hardcoded Sepolia).
- `unlock_signer(wallet_id, password)` returns `PrivateKeySigner` for both mnemonic + private-key wallets; unlocks the PR-B sign paths without an extra round of library work.
- New `Error::InvalidInput(String)` + `Error::WalletNotFoundByName { name, network }` variants (16 → 18) so CLI parse + lookup-miss errors flow through the same `exit_code()` table.
- `Network::parse_cli` returns `Error::InvalidInput` directly (exit 2) on unknown network — was previously `Error::Rpc` (exit 3).
- `(name, network)` uniqueness enforced on create/import (was previously silent data corruption when two wallets shared a name).

### Security hardening (PR-A side effects)

- `write_atomic` + `open_at` set explicit mode `0o600` / `0o700` on Unix targets. Default umask 0022 would have left encrypted blobs world-readable.
- `map_wallet_err` splits `WalletError::Crypto` (AEAD auth-tag mismatch — wrong password OR tamper) → `Error::DecryptionFailed` (exit 5); was previously mapped to `Error::InvalidMnemonic` (exit 2) — misleading for tamper detection.
- `tracing::warn!` in `wallet_create` flags `--password` on argv as insecure (shell history + process list). PR-B replaces with `rpassword`.

### Try it (PR-A additions)

```bash
# PR-A wired CLI surface (sync + read paths against Anvil)
ETH_DATA_DIR=/tmp/wallet-demo cargo run -p eth -- wallet create --name alpha --password p1 --network anvil
cargo run -p eth -- wallet list
cargo run -p eth -- wallet show --name alpha --network anvil
cargo run -p eth -- wallet delete --name alpha --network anvil
cargo run -p eth -- --rpc-url http://127.0.0.1:8545 wallet balance --address 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
cargo run -p eth -- --rpc-url http://127.0.0.1:8545 tx get --tx-hash 0x000...000

# Binary integration tests (5 always-on + 2 Anvil-gated)
cargo test -p eth --test cli_localnet                 # always-on sync tests pass
RUN_ANVIL_E2E=1 cargo test -p eth --test cli_localnet -- --ignored   # full e2e
```

### Status

PR-A body-flipped #337; issue stays OPEN until PR #338 merges to `rust-eth-core`. PR-B (sign + broadcast) tracked in follow-up issue (pending file) per L13 round budget.
