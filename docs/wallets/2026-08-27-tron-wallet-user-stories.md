# `tron` TRON Wallet — User Stories

**Date:** 2026-08-27
**Companion to:** [TRON Rust SDK deep-dive](2026-08-27-tron-rust-sdks-deep-dive.md) + [Ethereum user stories precedent](2026-08-23-eth-wallet-user-stories.md) + [Bitcoin user stories precedent](2026-08-05-btc-wallet-user-stories.md)
**Surface:** `tron` CLI binary inside `rust-wallet-app/crates/tron/` next to `btc/` and `eth/`. **Default network = Nile testnet (chain id `0xcd8690dc` / 3448148188 decimal, address prefix `0x41`).** Mainnet opt-in via `--network mainnet` (chain id `0x2b6653dc` / 728126428). Shasta opt-in via `--network shasta` (chain id `0x94a9059e`). TronBox regtest opt-in via `--rpc-url http://localhost:8090`. **All production networks (Mainnet, Shasta, Nile) share address prefix `0x41`** — TRON does not use per-network prefix bytes (verified 2026-08-27 against `developers.tron.network/docs/encoding`). Network discrimination is by chain-id only, not by address prefix.
**Tracks issue:** #399 (Q1–Q10 resolved in deep-dive; this doc maps user stories to the chosen crate surface).

**Wallet identity (mirrors eth #297 B1 + BTC v0.1):** user-facing identifier = `--name` (string); internal `wallet_id` (UUID) generated at create time and used for cross-wallet uniqueness on disk.

Personas:

- **Alice** — TRON power user. Manages multiple wallets across chains. Wants CLI control + scriptable commands + TRX + TRC-20 stablecoin (USDT-TRC20 primary) transfers.
- **Bob** — Developer integrating TRON. Wants stable exit codes + JSON output + raw message signing for automation.
- **Carol** — First-time TRON user. Wants clear prompts, warnings about mnemonic safety, no surprises, simple send/receive.

---

## Story → crate map

Each story is implemented via one or more crates from the chosen surface (deep-dive §"The chosen surface — current 2026 state"). This table is the traceability link between the user-facing feature and the underlying wallet engine call. The 4th column shows which workspace primitive backs the call. The 5th column reuses `bip32` + `bip39` where they apply (mnemonic + HD derivation are identical to Bitcoin/Ethereum; only the coin type differs at 195 vs 0/60).

| # | Story | Crate(s) used | Primitive(s) | `bip32` / `bip39` reuse |
|---|---|---|---|---|
| 1 | Create a new wallet | `bip39` (gen); `bip32` (derive); `k256` (sign); `tiny-keccak` (addr hash); `bs58` (encode) | `Mnemonic`, `XPrv`, `SigningKey`, `Keccak256`, base58check | `bip39::Mnemonic::generate_in(Words12, English, rng)` + `bip32::XPrv::derive_path("m/44'/195'/0'/0/0")` |
| 2 | Import an existing wallet | `bip39`; `bip32`; `k256`; `tiny-keccak`; `bs58` | same as Story 1 | `bip39::Mnemonic::parse_in(English, s)` + same `bip32` path |
| 3 | Check TRX balance | `reqwest` + `rustls` + `serde_json` | sun (1 TRX = 1_000_000 sun); 21-byte raw address | n/a |
| 4 | Sync chain state | `reqwest` + `serde_json` | `u64` block_number; `u64` chain_id; `u64` nonce (via `wallet/getaccount`) | n/a |
| 5 | Send native TRX | `reqwest` (RPC); `prost` (protobuf tx); `k256` (sign); `sha2` (tx-hash) | `Transaction { raw_data, signature }`, `TransferContract { owner_address, to_address, amount }` | extract `k256::SigningKey` from derived `SecretKey` (same as Story 1) |
| 6 | Send with custom `fee_limit` | `prost`; `k256`; `sha2` | `Transaction.raw_data.fee_limit: int64` (sun) | n/a |
| 7 | Inspect transaction history | `reqwest` + `serde_json` | `TransactionInfo { id, blockNumber, contractResult, fee }` | n/a |
| 8 | Get current energy/bandwidth estimates | `reqwest` + `serde_json` | `AccountResourceMessage { EnergyCurrent, BandwidthCurrent }` (V5 spike verifies exact field names) | n/a |
| 9 | List / show / delete / rename wallets | `std::fs`; `chain-traits` registry | n/a | n/a |
| 10 | Use mainnet explicitly | `reqwest` + `serde_json` | `chainid: 0x2b6653dc` (mainnet = 42 decimal); address prefix `0x41` | n/a |
| 11 | Show config + debug info | `std::env`, `version()` | n/a | n/a |
| 12 | Persist wallet across CLI invocations | filesystem + UUID-based wallet dir; encrypted mnemonic (F5/F6) | n/a | n/a (mnemonic on disk, never re-derived) |
| 13 | Send to multiple recipients (sequential txs) | N `send_transaction` calls; no native multi-output (TRON `Contract` array typically holds one contract per tx) | N × `Transaction` (each its own `ref_block`/`nonce`) | n/a |
| 14 | Sweep / drain wallet to one address | `getaccount` + `TransferContract.amount = balance - fee` | sun balance arithmetic | n/a |
| 15 | Choose ref_block strategy (auto vs manual) | `walletsolidity/getnowblock` (auto) vs caller-supplied `ref_block_bytes` + `ref_block_hash` (manual) | `bytes` + `bytes` (TAPOS) | n/a |
| 16 | Manual override (expiration, fee_limit) | `Transaction.raw_data.expiration: int64`, `fee_limit: int64` | `int64` (ms epoch for expiration; sun for fee_limit) | n/a |
| 17 | Replace / speed-up tx (same nonce + higher fee_limit) | new `Transaction` with same `ref_block` + higher `fee_limit` | `Signature` (different from original) | n/a |
| 18 | Sign personal message (raw) | `k256::SigningKey::sign_prehash(msg_hash)` | `Signature`, recovered 21-byte address | n/a |
| 19 | Export the wallet xpub + first addresses | `bip32::XPub::to_string` | T-base58check addresses (first 5 receive addresses) | `bip32::XPub` + `bip32::DerivationPath` |
| 20 | Pick derivation path (Ledger vs custom) | `bip32::XPrv::derive_path` override | n/a | override path in config |
| 21 | Send TRC-20 stablecoin (USDT-TRC20) | `reqwest`; `prost`; hand-rolled ABI encoder; `k256`; `sha2` | `TriggerSmartContract { owner_address, contract_address, data, call_value=0 }`; calldata = `0xa9059cbb` + padded `address` + padded `uint256` | n/a |
| 22 | Check TRC-20 token balance | `reqwest`; hand-rolled ABI encoder; `provider.call` equivalent via `wallet/triggerconstantcontract` with `visible: true` + ABI-decode result | `balanceOfCall` selector `0x70a08231`; result `uint256` | n/a |
| 23 | List registered TRC-20 stablecoins / tokens | token registry JSON in repo (`rust-wallet-app/crates/tron-wallet-core/tokens/{mainnet,nile}.json`) | 21-byte raw contract address, `u8` decimals, `String` symbol | n/a |
| 24 | Add custom TRC-20 token by contract address | hand-rolled ABI encoder; `wallet/triggerconstantcontract` view call to `decimals()` + `symbol()` | `uint8` decimals, `String` symbol | n/a |
| 25 | Approve TRC-20 spending (for DEX) | hand-rolled ABI encoder; `TriggerSmartContract` with `approve(address,uint256)` selector `0x095ea7b3` | `approveCall`, calldata | n/a |
| 26 | Use TronBox local node for testing | `--rpc-url http://localhost:8090`; `eth_chainId` via `/jsonrpc` asserts the configured chain-id for `--network tronbox` | `u64` chain id | n/a |
| 27 | Use Nile testnet | `--network nile`; TronGrid Nile `https://nile.trongrid.io` | `chainid: 0xcd8690dc (3448148188 decimal)` | n/a |
| 28 | Connect to RPC endpoint with SPKI pin (Q7) | `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` (reused verbatim) | 32-byte SPKI SHA-256 pin | n/a |
| 29 | Connect to RPC endpoint without SPKI pin (system CAs only) | `reqwest::Client::new()` + `webpki-roots` (no pin) | n/a | n/a |
| Cross-cutting | `--json` everywhere; stable exit codes; no daemons | `serde_json`; `std::process::ExitCode` | n/a | n/a |
| Cross-cutting | `Secret<Mnemonic>` zeroize (v0.2+ hygiene) | `zeroize::Zeroizing<Mnemonic>` (mirror Bitcoin Task 30) | n/a | `bip39::Mnemonic` (wrap) |
| Cross-cutting | T-base58check address display (Q4) | `bs58` + 4-byte double-SHA-256 checksum | 21-byte raw + base58check | n/a |

**Layer separation summary:**

- **`bip32` + `bip39` (reuse from Bitcoin + eth)** = HD derivation + mnemonic. Identical primitives; only coin type differs (195 = TRX).
- **`k256` (workspace dep from Bitcoin)** = secp256k1 signing. Signs SHA-256 prehash of protobuf `raw_data`.
- **`sha2` (workspace dep)** = SHA-256 for both tx-hash and base58check double-chash.
- **`tiny-keccak` (NEW)** = Keccak-256 for address derivation (last 20 bytes of `Keccak256(pubkey_uncompressed)`).
- **`bs58` (NEW)** = base58check encoding for T-prefix addresses.
- **`prost` + `prost-build` (NEW)** = protobuf serialization for `Transaction` (compiled from `core/Tron.proto`).
- **`reqwest` + `rustls` (workspace deps, reused)** = JSON-RPC transport to `wallet/*` endpoints + SPKI-pinning verifier from `bitcoin-wallet-core`.
- **`serde_json` (workspace dep)** = JSON-RPC request/response envelope parsing.

**Stories NOT using the chosen crates (custom code only):**

- Story 11 version string — `env!("CARGO_PKG_VERSION")`, no new dep.
- Story 23 token registry — filesystem JSON, no crate.
- Story 9 list/show/delete — `std::fs` only, no crate.

## Crate use case coverage

Cross-check: every crate use case from the deep-dive §"Crate-by-crate deep-dive" mapped to the user story that exercises it. Any "not covered" row is either an internal-only operation (no CLI surface needed) or a gap to add.

| # | Crate use case | User story that exercises it | Covered? |
|---|---|---|---|
| 1 | Generate BIP-39 mnemonic (`bip39::Mnemonic::generate_in`) | Story 1 (create) | ✅ |
| 2 | Parse BIP-39 mnemonic (`bip39::Mnemonic::parse_in`) | Story 2 (import) | ✅ |
| 3 | Derive `XPrv` via `bip32::XPrv::derive_from_path` | Story 1 + 2 | ✅ |
| 4 | Override derivation path | Story 20 | ✅ |
| 5 | Export `XPub` (`bip32::XPub::to_string`) | Story 19 | ✅ |
| 6 | `k256::SigningKey::sign_prehash(tx_hash)` (returns 64 bytes r‖s) | Story 5 + 17 + 18 + 21 | ✅ |
| 7 | Compute recovery byte `v` via `k256::ecdsa::VerifyingKey::recover_from_prehash` | Story 5 + 17 + 21 (sign-and-broadcast path) | ✅ |
| 8 | `sha2::Sha256::digest(raw_data_bytes)` for tx-hash | Story 5 + 17 + 21 | ✅ |
| 9 | `sha2::Sha256::double-hash` for base58check checksum | Story 4 — Q4 base58check display | ✅ |
| 10 | `tiny-keccak::Keccak256` for address derivation | Story 1 + 2 (always); Story 19 (export) | ✅ |
| 11 | `bs58::encode + decode + 4-byte checksum append/verify` | Story 1 + 2 + 9 (show) + 19 (export) | ✅ |
| 12 | `prost::Message::encode_to_vec` for `Transaction` protobuf | Story 5 + 6 + 17 + 21 | ✅ |
| 13 | `prost::Message::decode` for response parsing (e.g. `walletsolidity/getnowblock`) | Story 4 + 5 (chain state sync) | ✅ |
| 14 | `prost-build::compile_protos` in `build.rs` for `core/Tron.proto` codegen | internal (compile-time) | ⚠️ internal |
| 15 | `reqwest::Client` JSON-RPC POST to `wallet/*` | all RPC stories (3, 4, 5, 7, 8, 21, 22, 24, 25) | ✅ |
| 16 | Custom `rustls::ServerCertVerifier` (SPKI pin) — `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` | Story 28 | ✅ |
| 17 | `reqwest::Client` plain (system CAs, no pin) | Story 29 (default) | ✅ |
| 18 | `serde_json::Value` for JSON-RPC envelope + response parse | all RPC stories | ✅ |
| 19 | Configurable chain-id (universal `0x41` prefix across Mainnet/Shasta/Nile) — network discrimination by chain-id only | Story 10 + 27 + 9 (show) | ✅ |
| 20 | Configurable RPC URL (`--rpc-url` flag + `pinned://` scheme) | Story 28 + 29 | ✅ |
| 21 | TRON-PRO-API-KEY header injection (rate-limit increase) | cross-cutting (every RPC call) | ✅ |

**Coverage summary:**

- **18 of 21 use cases directly covered by user stories** (rows 1-13, 15-21)
- **1 use case is internal-only** (`prost-build` codegen — compile-time, no CLI surface)
- **0 deferred to v1.x**
- **0 explicitly rejected**

(Total: 18 + 1 = 19; rows 14 is internal; rows 1-13 + 15-21 = 18 covered = all user-facing surface covered. No gaps.)

---

## Story 1 — Create a new wallet (Alice)

> As Alice, I want to generate a new TRON testnet wallet from a single command, so I can start receiving TRX + TRC-20 in under 10 seconds.

**Acceptance criteria:**

- `tron wallet create --name test-wallet --network nile` runs in <1s on a developer laptop.
- Output shows: 12-word BIP-39 mnemonic, first receive address (T-base58check, 34 chars starting with `T`), chain id + network name, wallet name. (Address prefix byte is universal `0x41` — no per-network discrimination.)
- Mnemonic written encrypted to `~/.local/share/tron/test-wallet/mnemonic.enc` (Argon2id + AES-256-GCM per F5/F6, mirrors BTC v0.1). On every CLI call that touches key material, `tron` prompts `wallet unlock:` for the passphrase (or reads `TRON_WALLET_PASSPHRASE` env var). Decrypted mnemonic lives only in zeroized memory; nothing plaintext touches disk.
- Command exits 0 on success, non-zero on filesystem error.
- A prominent `WARNING` line reminds the user to back up the mnemonic before continuing.
- Running the command twice with the same name fails with exit code 2 and message `wallet 'test-wallet' already exists`.

**Options:**

- `--network mainnet|nile|tronbox` (default `nile`)
- `--rpc-url <URL>` (default `https://nile.trongrid.io` for nile, `https://api.trongrid.io` for mainnet, `http://localhost:8090` for tronbox)
- `--derivation-path <PATH>` (default `m/44'/195'/0'/0/0` — SLIP-44 coin type 195)
- `--account-index <N>` (default 0; advanced — picks `m/44'/195'/<N>'/0/0` instead)
- `--address-index <N>` (default 0; advanced — picks `m/44'/195'/0'/0/<N>` instead)

---

## Story 2 — Import an existing wallet (Alice)

> As Alice, I want to import a wallet from an existing 12/24-word mnemonic, so I can recover access to a wallet I created elsewhere (TronLink, Ledger, or a different `tron` install).

**Acceptance criteria:**

- `tron wallet import --name recovered --mnemonic "word1 word2 ... word12" --network nile` accepts a valid mnemonic.
- `tron wallet import --name dev --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` imports a raw secp256k1 key (mirrors eth #297 G4). Validates the scalar is `< secp256k1::ORDER`; rejects out-of-range or zero-key with exit 2 (`invalid private key: out of range`).
- Invalid checksum returns exit code 2 + clear error `invalid mnemonic: checksum mismatch`.
- Imported wallet produces the same first address as the source (verified by deterministic derivation).
- BIP39 passphrase (`--passphrase "..."`) supported; empty passphrase is the default.
- Output does **not** echo the mnemonic back to the terminal.
- Supports 12, 15, 18, 21, 24-word mnemonics.
- Importing from a Ledger-derived mnemonic requires `--derivation-path m/44'/195'/0'/0/0` to match the original address.

---

## Story 3 — Check TRX balance (Alice, Carol)

> As Carol, I want to see my confirmed TRX balance in a single line, so I know how much I can spend right now.

**Acceptance criteria:**

- `tron wallet balance --name test-wallet --network nile --rpc-url <URL>` prints **two lines** (scriptable): `address=TXYZ... balance_sun=1234567000000`.
- First run after wallet creation shows `balance_sun=0` (no funds yet) and exits 0.
- If RPC fails, the command retries once, then prints `rpc failed: <reason>` and exits 3.
- Reads balance from `wallet/getaccount` (returns TRX balance + frozen balances + resource info).
- Output balance in sun by default; `--unit trx|sun` (default `sun`); `--human` prints `1234.567 TRX`.
- **Note:** per #399 B3 (mirrors eth #297 B3), v0.1 uses named-wallet model with persisted handles (mirrors eth v0.3 Story 12 design).

---

## Story 4 — Sync chain state (Alice)

> As Alice, I want to force a full chain sync, so I can see incoming transactions that arrived since the last sync.

**Acceptance criteria:**

- `tron wallet sync --name test-wallet --network nile --rpc-url <URL>` connects to the JSON-RPC endpoint and pulls fresh chain state.
- Output: `block_number=<N> chain_id=<ID> nonce=<N> (address=TXYZ...)`.
- Exit 0 on success. Exit 3 on RPC failure.
- A subsequent `tron wallet balance` reflects the synced state without a second sync (cached nonce + block number).
- **Note:** mirrors eth Story 4 / Story 12 design.

---

## Story 5 — Send native TRX (Alice)

> As Alice, I want to send 1.5 TRX to a Nile address, so I can complete a payment in one command.

**Acceptance criteria:**

- `tron wallet send --name test-wallet --network nile --to TXYZ... --amount 1.5 --rpc-url <URL>` builds, signs (secp256k1 + SHA-256 prehash of protobuf `raw_data`), and broadcasts a `TransferContract`.
- `--amount 1.5` is interpreted as `1.5 * 1_000_000 = 1_500_000` sun. CLI accepts TRX (float) or sun (integer with `--amount-sun`).
- Default `fee_limit = 0` (TRX transfer burns bandwidth, not energy; `fee_limit` field present but value 0).
- Default `expiration = head_block_ts + 60_000` (60-second window per TRON protocol default).
- Output on success: `sent. tx_id: <64-hex-chars> (nonce: <N>, amount: 1500000 sun, block: <N>, fee: <N> sun)`.
- Exit 0 on broadcast. Exit 4 on insufficient funds. Exit 5 on signing/RPC error.
- `--dry-run` builds + signs but does not broadcast; prints the signed-tx hex + JSON-decoded `raw_data` fields.
- `--wait` polls `wallet/gettransactioninfobyid` until confirmed (≥19 SR solidified per TRON protocol §"Transaction lifecycle"); prints the receipt (`status: success`, `block: <N>`, `fee: <N> sun`).
- **Note:** TRON has no native multi-recipient tx (`TransferContract` array typically holds one contract). The `--batch` flag (Story 13) fires N sequential transactions with separate `ref_block` per tx.

---

## Story 6 — Send with custom `fee_limit` (Bob)

> As Bob, I want to specify `fee_limit` in sun, so I can match my back-end's fee policy exactly for TRC-20 calls that burn energy.

**Acceptance criteria:**

- `tron wallet send --name test-wallet --network nile --to TXYZ... --amount 1.5 --fee-limit 50000000 --rpc-url <URL>` builds a `Transaction` with `raw_data.fee_limit = 50_000_000` (50 TRX worth of energy allowance).
- Validation: `--fee-limit` must be `>= 0` (exit 2 with `fee_limit must be >= 0` if negative).
- Validation: `--fee-limit` must be `<= 1_000_000_000_000` (1B TRX = 10^15 sun; exit 2 with `fee_limit too large` if exceeded — sanity guard against typos).
- Output includes the effective `fee_limit` used alongside the `TransferContract.amount`.
- **Note:** for plain TRX transfer, `fee_limit` is effectively 0 (bandwidth covers the cost). Setting a positive `fee_limit` reserves energy budget for the tx even if not consumed.

---

## Story 7 — Inspect transaction history (Alice)

> As Alice, I want to list my past transactions with confirmations, so I can reconcile my records.

**Acceptance criteria:**

- `tron tx list --name test-wallet --network nile --rpc-url <URL> --since-block 1000000` queries historical blocks and prints a table: `tx_id | direction | amount_sun | fee_sun | confirmations | block_number | timestamp`.
- Default 25 most recent txs. `--limit N` overrides.
- `--json` outputs a JSON array (for piping to `jq`).
- Unconfirmed txs show `confirmations: 0` with `pending` tag (requires `--pending` flag to include).
- `tron tx get --tx-id <64-hex> --rpc-url <URL>` returns full details of one tx (decoded `raw_data` + `TransactionInfo` receipt status).
- Exit 0 even if no transactions yet.

---

## Story 8 — Get current energy/bandwidth estimates (Bob, Carol)

> As Carol, I want to see the current resource state (energy + bandwidth) before sending a TRC-20 transfer, so I can pick the right `fee_limit`.

**Acceptance criteria:**

- `tron resource --name test-wallet --network nile --rpc-url <URL>` prints a table:
  ```text
  bandwidth:     600 / day  (free, chain parameter #61; replenishes 24h after last consume)
  energy:        varies (no free allocation; TRX-burn default = 100 sun/Energy)
  energy_used:   <estimated>  (live fetch via wallet/triggerconstantcontract for the target contract)
  fee_estimate:  <energy_used> × <sunPerEnergy> = <sun>  (TRX-equivalent)
  ```
- Numbers are empirical (verified 2026-08-27 against `developers.tron.network/docs/resource-model` + 4 wallet vendors): Stake 2.0 launched 2023-04-07 via TIP-467/proposal #84. 1 TRX staked = 1 TRON Power (TP). TP itself doesn't burn — only used for voting/governance. Stake 2.0 pays Bandwidth (1,000 sun/byte) OR Energy (100 sun/Energy) per stake, not both. Unstake pending = 14 days, concurrent unstake-op cap = 32. Minimum Stake 2.0 amount = 1 TRX (1,000,000 sun).
- For USDT-TRC20 `transfer`: 65,000 Energy (recipient already holds USDT) up to 130,000 Energy (empty recipient). **DEM (Dynamic Energy Model)** can scale both figures up to `max_factor = 3.4×` per 6-hour cycle — `getcontractinfo` returns `energy_factor` for any contract. Wallet should size `fee_limit` with the `max_factor` buffer OR re-estimate just-in-time before broadcast.
- Output refreshed on every call (live fetch, not cached).
- Exit 3 on RPC failure (chain-id mismatch, transport failure).
- `--json` outputs the same as a JSON object with schema `{bandwidth: {available_per_day, used, refilled_at}, energy: {energy_used_for_target_contract, sun_per_energy, fee_limit_sun_recommended, dem_factor}}`.

**Note:** Q5 (resource model UX) is RESOLVED in the deep-dive (`§"Resource model — verified 2026 numbers"`). Spike V5 only needs to (a) re-pull live `wallet/triggerconstantcontract` output for our exact MockTRC20, (b) confirm DEM `energy_factor` round-trip, (c) implement the per-resource-breakdown UX pattern. Architectural decisions (display in resource units vs TRX, fee_limit buffer strategy, DEM re-estimation cadence) all settled.

---

## Story 9 — List / show / delete / rename wallets (Alice, Bob)

> As Alice, I want to list all my wallets in the data directory and manage them, so I can pick one quickly.

**Acceptance criteria:**

- `tron wallet list` prints one wallet name per line.
- `tron wallet list --json` outputs a JSON array of `{name, network, address, derivation_path, created_at}`.
- `tron wallet show --name w` prints full info: network, chain id, address (T-base58check), derivation path, address prefix byte, wallet name, file path, created_at.
- `tron wallet show --name w --addresses` exports the first 5 receive addresses (m/44'/195'/0'/0/0 through m/44'/195'/0'/0/4).
- `tron wallet delete --name w` removes the wallet (mnemonic.enc + cached state). Prints `wallet 'w' deleted.` Exits 4 if the wallet doesn't exist.
- `tron wallet rename --name w --to w2` renames the wallet in place. Exits 4 if `w2` already exists.
- Empty directory: `tron wallet list` prints `(no wallets)` and exits 0.
- Corrupt wallets (no `mnemonic.enc`): listed but marked `(corrupt — missing mnemonic.enc)`.

---

## Story 10 — Use mainnet explicitly (Alice)

> As Alice, I want to create and use a mainnet wallet, so I can use real TRX.

**Acceptance criteria:**

- `tron wallet create --name main --network mainnet` produces a T-base58check address starting with `T` (e.g. `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`).
- Address prefix byte is universal `0x41` — same as Shasta + Nile. Network discrimination by chain-id only.
- `eth_chainId` JSON-RPC via `/jsonrpc` is called at startup and must return `0x2b6653dc` (728126428 decimal) — fails fast with exit 3 if the RPC endpoint disagrees.
- Default RPC URL for mainnet is `https://api.trongrid.io`.
- A confirmation prompt requires typing `yes` to proceed; default is abort.
- Output shows `WARNING: this wallet uses real TRX on TRON mainnet. Funds are at risk.` before the mnemonic.
- Exit 1 if user does not type `yes`.

---

## Story 11 — Show config + debug info (Bob)

> As Bob, I want to see what the CLI thinks the current network and RPC URL are, so I can debug "why is this connecting to the wrong place".

**Acceptance criteria:**

- `tron config show` prints: data dir, RPC URL, network, chain id (after `--rpc-url` lookup), address prefix byte, list of loaded wallets, derivation path default, default fee_limit.
- `tron config show --json` outputs the same as JSON.
- Exit 0 always.
- Diagnostic output includes the version string `tron 0.1.0` (via `--version`) and the chosen crate versions (`prost 0.14.4`, `bs58 0.5.x`, `tiny-keccak 2.0.x`).

---

## Story 12 — Persist wallet across CLI invocations (Alice) **[v0.2 — per #399 B3]**

> As Alice, I want each CLI invocation to find my wallets without re-deriving from the mnemonic, so it's fast.

**Acceptance criteria:**

- First `tron wallet create` writes an encrypted mnemonic to `$XDG_DATA_HOME/tron/wallets/<network>/<wallet_id>.enc` per ADR 0001 (UUID wallet id, like BTC v0.1). Argon2id + AES-256-GCM (F5/F6) — no plaintext on disk.
- Subsequent `tron wallet show --id <wallet_id> --network <NET>` prompts unlock (or reads `TRON_WALLET_PASSPHRASE`), reads the encrypted mnemonic, derives keys, syncs via RPC, prints the wallet state.
- If the data dir is on slow disk (HDD, network FS), `tron wallet show` still completes in <500ms after sync.
- If the file is missing or unreadable, the command exits 2 with `wallet '<id>' is missing or corrupt`.
- **Note (per #399 B1 + B2):** v0.1 uses UUID-based wallet IDs (internal `wallet_id`) with user-facing `--name` flag for cross-wallet operations. v0.2 ships encryption from day 1; no plaintext fallback.

---

## Story 13 — Send to multiple recipients (sequential txs) (Alice)

> As Alice, I want to send to several addresses in quick succession, so I can pay multiple recipients without re-typing the command.

**Acceptance criteria:**

- `tron wallet send --name test-wallet --batch <file>` reads a CSV file (format = `address,amount_trx` per line; no header row; trim whitespace; ignore blank lines and `# comment` lines) and sends each as a separate transaction.
- Up to 100 recipients per `--batch` (CLI-enforced; exit 2 with `max 100 recipients per batch`).
- Default `fee_limit = 0`. Override via `--fee-limit`.
- Each tx gets its own `ref_block` from `walletsolidity/getnowblock` at the moment of signing (TRON protocol requirement — `ref_block` ages out of `RecentBlockStore` after ~65,536 blocks).
- Output: `batch sent. count: 3, tx_ids: [<64-hex>, <64-hex>, <64-hex>], total: 0.05 TRX, total_fees: <N> sun`.
- Exit 0 if all broadcast; non-zero if any failed (prints which recipient + which tx_id failed).
- `--stop-on-error` (default) aborts the batch on the first failure; `--continue` sends the rest.

---

## Story 14 — Sweep / drain wallet to one address (Alice)

> As Alice, I want to sweep my entire TRX balance to one address, so I can consolidate funds before re-organizing my wallets.

**Acceptance criteria:**

- `tron wallet send --name test-wallet --drain --to TXYZ... --rpc-url <URL>` builds a `TransferContract` that sends `balance_sun` to `to`, leaving 0 sun behind.
- Balance read via `wallet/getaccount`.
- Default `fee_limit = 0`. Override via `--fee-limit`.
- Output: `drained. tx_id: <64-hex> (sent: 1234.567 TRX, fee: <N> sun, leftover: 0 sun)`.
- Exit 4 if balance < `fee_limit` (i.e., nothing to sweep).
- Confirmation prompt: `Drain wallet 'w' to TXYZ...? Type 'yes' to confirm.` (default abort).

---

## Story 15 — Choose `ref_block` strategy (auto vs manual) (Bob)

> As Bob, I want to pick between auto-ref_block (read from RPC) and manual ref_block (supply my own), so I can match my back-end's tx-batching policy.

**Acceptance criteria:**

- `tron wallet send --name test-wallet --to TXYZ... --amount 1.5` (default) uses auto-ref_block: `walletsolidity/getnowblock` → `ref_block_bytes` (bytes [6,8) of block number) + `ref_block_hash` (bytes [8,16) of block id).
- `tron wallet send --name test-wallet --to TXYZ... --amount 1.5 --ref-block-bytes 0xc145 --ref-block-hash 0xc56bd8a3b3341d9d --ref-block-num 12345678` uses the supplied ref_block. The CLI verifies the supplied values match a real block via `wallet/getblockbyid` (or `wallet/getblockbynum`) — exits 2 with `ref_block not found` if mismatch.
- Validation: `--ref-block-bytes` must be 2 bytes; `--ref-block-hash` must be 8 bytes; `--ref-block-num` must be ≤ current block number (exit 2 if ahead).
- Output always prints the actual `ref_block_bytes` + `ref_block_hash` used (whether auto or manual).

---

## Story 16 — Manual `expiration` + `fee_limit` override (Bob)

> As Bob, I want to supply an exact `expiration` timestamp and `fee_limit`, so I can replace or compose transactions deterministically.

**Acceptance criteria:**

- `tron wallet send --name test-wallet --to TXYZ... --amount 1.5 --expiration 1700000000000 --fee-limit 50000000` uses exactly these values.
- Validation: `--expiration` must be `> now_ms` (exit 2 with `expiration must be in the future`).
- Validation: `--expiration` must be `<= now_ms + 86_400_000` (24 hours, per TRON protocol max; exit 2 with `expiration too far in future`).
- Validation: `--fee-limit` must be `>= 0` (Story 6).
- Output includes the supplied `expiration` + `fee_limit` alongside the auto-detected values.
- Useful as the building block for Story 17 (speed-up) and Story 13 (batch with deterministic ordering).

---

## Story 17 — Replace / speed-up tx (same nonce + higher `fee_limit`) (Alice)

> As Alice, I want to speed up a stuck transaction by sending a new one with the same nonce but a higher `fee_limit`, so I don't have to wait for the original to drop.

**Acceptance criteria:**

- `tron wallet send speed-up --name test-wallet --tx-id <64-hex> --rpc-url <URL> --fee-limit 100000000` reads the original tx (via `wallet/gettransactionbyid`), extracts its `raw_data` (minus `signature`) + `owner_address` + `to_address` + `amount`, builds a new tx with the same nonce (implied by re-signing with same sender — TRON uses sender address for nonce lookup, not a nonce field per se) + higher `fee_limit`, signs + broadcasts.
- **TRON-specific note:** TRON does not have an explicit `nonce` field in `TransferContract`. Nonce is tracked by the network as the count of confirmed txs from the sender address. "Replace by fee" works because the new tx with the same `owner_address` and higher `fee_limit` outbids the old one in SR mempool ordering. The CLI must confirm the original tx is unconfirmed (`wallet/gettransactioninfobyid` returns `blockNumber: <absent>`) before allowing the replace.
- `--fee-limit` must be `>` the original tx's `fee_limit`. Exit 2 with `new fee_limit must exceed original` if not.
- Output on success: `sped up. new_tx_id: <64-hex> (old_tx_id: <64-hex>, fee_limit_increase: 50 TRX)`.
- Exit 4 if original tx not found in last N blocks (`--lookback-blocks`, default 200).
- Exit 5 if the original tx is already mined and confirmed (no point speeding up).

---

## Story 18 — Sign personal message (raw, no EIP-191 prefix) (Alice)

> As Alice, I want to sign an arbitrary message with my wallet's private key, so I can prove ownership of an address (e.g., for off-chain auth, TRC-516 message signing, or signed-message board).

**Acceptance criteria:**

- `tron sign-message --name test-wallet --message "I own this address"` signs with `k256::SigningKey::sign_prehash(SHA256(message))`. **No EIP-191 prefix** — TRON has no equivalent standard; raw SHA-256 of the message bytes is the convention (mirrors Bitcoin message signing, not Ethereum EIP-191).
- Default output: hex of the 65-byte signature (`r || s || v`, recovery byte `v` computed via `VerifyingKey::recover_from_prehash`).
- `--address <T-base58>` signs with the key for that address (must be wallet-owned); default is the first receive address.
- Output format: `address: TXYZ...\nsignature: <130-hex> (v=<0|1>)`.
- `--verify <T-base58>` is a sanity check: recovered address from `(tx_hash, signature)` == `<T-base58>`.
- Exit 0 on success. Exit 4 if `--address` is not wallet-owned.

---

## Story 19 — Export the wallet xpub + first addresses (Bob)

> As Bob, I want to export my wallet's xpub and the first few receive addresses, so I can import the watch-only descriptor into a hardware-wallet companion or a block explorer.

**Acceptance criteria:**

- `tron wallet show --name w --export` prints the BIP-32 extended public key (`xpub...` per SLIP-0132, same format as Bitcoin) + the first 5 receive addresses.
- Output format: `xpub: xpub6CU...\naddress_0: TXYZ...\naddress_1: TXYZ...\naddress_2: TXYZ...\naddress_3: TXYZ...\naddress_4: TXYZ...`.
- One line per item, easy to pipe to `pbcopy`.
- No private-key material in this output (the xpub is public-by-design).
- Exit 0 always (xpub export is not a security-sensitive operation).

**Note:** unlike Bitcoin, TRON has no descriptor concept (no script template — every address is the same `keccak256(pubkey)[12..]` with the address prefix byte). The xpub is the only thing to share for watch-only.

---

## Story 20 — Pick derivation path (Ledger vs custom) (Alice)

> As Alice, I want to choose between Ledger-style and custom derivation paths, so I can match the wallet that originally generated my addresses.

**Acceptance criteria:**

- `tron wallet create --name w --derivation-path m/44'/195'/0'/0/0` (Ledger-style, default — SLIP-44 coin type 195, account slot = 0, address slot = 0).
- `--address-index N` shorthand: expands to `m/44'/195'/0'/0/<N>` (BIP-44 address index, 5th position).
- `--account-index M` shorthand: expands to `m/44'/195'/<M>'/0/0` (BIP-44 account index, 3rd position).
- `--derivation-path`, `--address-index`, and `--account-index` are mutually exclusive. Exit 2 with `pick one of --derivation-path, --address-index, or --account-index`.
- Validation: path must start with `m/44'/195'/`. Exit 2 with `derivation path must start with m/44'/195'/`.
- Output always shows the path used, so the user can verify.

---

## Story 21 — Send TRC-20 stablecoin (USDT-TRC20) (Alice)

> As Alice, I want to send 1.50 USDT-TRC20 to a recipient with a single command, so I can pay in stablecoin without worrying about TRX price volatility.

**Acceptance criteria:**

- `tron trc20 send --name test-wallet --token USDT --to TXYZ... --amount 1.5 --network nile --rpc-url <URL>` builds + signs + broadcasts a `TriggerSmartContract` calling `transfer(address,uint256)` on the USDT-TRC20 contract.
- `--token USDT` resolves to `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` (mainnet) or the Nile equivalent (`TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` per nileex.io community faucet — spike V9 verifies).
- `--amount 1.5` is interpreted as `1.5 * 10^decimals` base units. USDT-TRC20 uses 6 decimals, so `1.5 USDT = 1_500_000` base units. The CLI fetches `decimals()` once per token (cached, or read from `tokens/<network>.json`).
- Tx shape: `TriggerSmartContract { owner_address: sender_21, contract_address: token_contract_21, data: <0xa9059cbb + padded_to_32(recipient_20) + padded_to_32(amount_256)>, call_value: 0, call_token_value: 0, token_id: 0 }`.
- Default `fee_limit = 130_000_000` (130 TRX worth of energy allowance — covers USDT-TRC20 `transfer` comfortably per V5 spike estimate). Override via `--fee-limit`.
- Output on success: `sent. tx_id: <64-hex> (token: USDT, amount: 1.5, decimals: 6, fee_limit: 130 TRX, block: <N>)`.
- Exit 0 on broadcast. Exit 4 if the wallet's USDT-TRC20 balance < `1.5 * 10^6` (insufficient token balance).
- Validation: `--amount` must be `> 0` (exit 2 with `amount must be > 0`).
- Validation: token must be in the registry (Story 23); unknown `--token SYMBOL` fails fast (exit 2 with `unknown token: USDT_FOO. Use --token-address to register a custom one`).
- `--token-address <T-base58>` overrides `--token SYMBOL` and uses the supplied contract directly (skips registry).

---

## Story 22 — Check TRC-20 token balance (Alice, Carol)

> As Carol, I want to see my USDT-TRC20 and USDC-TRC20 balances next to my TRX balance, so I know my total stablecoin holdings.

**Acceptance criteria:**

- `tron wallet balance --name test-wallet --token USDT --network nile --rpc-url <URL>` prints `<scaled> USDT (token: TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t)` (e.g. `15.000000 USDT (token: TR7NHqje...)`), with decimals auto-detected via `decimals()` `wallet/triggerconstantcontract` call (selector `0x313ce567`) or `--decimals <N>` override.
- Drift from spec: spec body proposed `tron trc20 balance --token USDT` as a separate subcommand; impl extends `tron wallet balance` with `--token <SYMBOL>` (single subcommand covers TRX + token). `<name>` required (balance is read against a named wallet's first address).
- Exit 0 even if balance is 0. Exit 2 on invalid `--token` symbol (UnknownToken). Exit 3 on RPC failure (transport-layer). Exit 5 on ABI decode failure (per #399 M-ABI — contract claims to be TRC-20 but isn't).
- **Note:** requires hand-rolled ABI decoder for `balanceOf(address)` → selector `0x70a08231`, decode 32-byte response as `uint256`. No contract binding crate needed.

---

## Story 23 — List registered TRC-20 stablecoins / tokens (Bob)

> As Bob, I want to see the list of supported TRC-20 tokens, so I know which ones I can `--token USDT` against without `--token-address`.

**Acceptance criteria:**

- `tron trc20 list` prints a table:
  ```text
  SYMBOL  ADDRESS                                          DECIMALS  NETWORK
  USDT    TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t                  6      mainnet
  USDC    TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8                  6      mainnet
  TUSD    TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9                 18      mainnet
  USDT    TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z                  6      nile
  ```
- `--json` outputs the same as a JSON array.
- Tokens are loaded from two sources (per eth #297 G3 + #399 Q9):
  1. **Bundled** (compile-time via `include_str!`): `rust-wallet-app/crates/tron-wallet-core/tokens/mainnet.json` + `nile.json`.
  2. **User** (runtime from `$XDG_CONFIG_HOME/tron/tokens/<network>.json`): operator-added tokens from Story 24.
- **Resolution rule:** user registry wins over bundled on symbol collision. `--list --include-bundled` prints both with a source tag (`bundled` / `user`).
- Empty registry: prints `(no tokens registered)` and exits 0.

---

## Story 24 — Add custom TRC-20 token by contract address (Alice)

> As Alice, I want to add a custom TRC-20 token by supplying its contract address, so I can send/receive tokens that aren't in the bundled registry.

**Acceptance criteria:**

- `tron trc20 register --address TRXXX... --rpc-url <URL>` queries `decimals()` + `symbol()` + `name()` via `wallet/triggerconstantcontract` view calls, then writes the token entry to `$XDG_CONFIG_HOME/tron/tokens/<network>.json` (operator-editable, separate from bundled).
- The new token can immediately be used as `--token SYMBOL` in `tron trc20 send` (resolution checks both bundled + user registries).
- `tron trc20 register --list` shows user-added tokens.
- `tron trc20 register --remove --symbol FOOBAR` removes a user-added token.
- Validation: `decimals()` must return 0-36 (exit 2 with `invalid decimals: <N>` if out of range).
- Validation: `symbol()` must return 1-11 printable ASCII chars (exit 2 with `invalid symbol: <s>`).
- Validation: `name()` must return 1-256 chars of UTF-8 with no control characters (exit 2 with `invalid name: <s>`, per #399 M-NAME).

---

## Story 25 — Approve TRC-20 spending (for DEX) (Bob)

> As Bob, I want to approve a SunSwap router contract to spend my USDT-TRC20, so I can swap tokens without manually signing every transfer.

**Acceptance criteria:**

- `tron trc20 approve --name test-wallet --token USDT --spender TRRouter... --amount 100 --network nile --rpc-url <URL>` builds + signs + broadcasts a `TriggerSmartContract` calling `approve(address,uint256)` on the USDT-TRC20 contract.
- Selector: `0x095ea7b3`. Calldata: `0x095ea7b3 + padded_to_32(spender_20) + padded_to_32(value_256)`.
- Tx shape: `TriggerSmartContract { owner_address: sender_21, contract_address: token_contract_21, data: <approve calldata>, call_value: 0, call_token_value: 0, token_id: 0 }`.
- Default `fee_limit = 130_000_000` (130 TRX energy allowance). Override via `--fee-limit`.
- Output on success: `approved. tx_id: <64-hex> (token: USDT, spender: TRRouter..., allowance: 100)`.
- `--amount unlimited` (or `--amount max`) sets `value = U256::MAX` (the "infinite approval" pattern). Confirmation prompt required: `Setting unlimited allowance to TRRouter... Type 'yes' to confirm.`
- Exit 0 on broadcast. Exit 4 if wallet TRX balance < fee_limit (not enough TRX to pay energy for the approve tx).

---

## Story 26 — Use TronBox local node for testing (Alice)

> As Alice, I want to point the CLI at a local TronBox instance, so I can test end-to-end without spending Nile TRX.

**Acceptance criteria:**

- `tron --network tronbox --rpc-url http://localhost:8090 wallet create --name dev` works against TronBox.
- `eth_chainId` via `/jsonrpc` is asserted at startup; mismatch with `--network tronbox` fails fast (exit 3 with `expected chain_id <X>, got <N>`).
- TronBox's prefunded accounts (deterministic, TronBox-specific private keys) can be imported via `tron wallet import --private-key 0x...`.
- `tron trc20 deploy --token-name Foo --token-symbol FOO --decimals 6` deploys a `MockTRC20` contract to TronBox (per eth #297 M8 pattern). Returns the deployed contract address as a T-base58check string.
- TronBox integration via `trufflesuite/tronbox` as `[dev-dependencies]` only (not in production builds).

---

## Story 27 — Use Nile testnet (Bob, Carol) **[Q6 resolution]**

> As Bob, I want to use the Nile testnet by default, so I can test without spending real TRX.

**Acceptance criteria:**

- `--network nile` is the default; no flag needed for the common case.
- `eth_chainId` via `/jsonrpc` against `https://nile.trongrid.io` returns `"0xcd8690dc"` (3448148188 decimal). **MUST NOT use `wallet/getchainid`** — returns HTTP 405 on TronGrid.
- Address prefix byte is universal `0x41` (same as Mainnet + Shasta). Network discrimination by chain-id only. Cross-network send (mainnet address to a Nile-configured CLI) fails fast with exit 5 `chain_id mismatch: 0x2b6653dc expected, tx derived from 0xcd8690dc`.
- Default RPC URL = `https://nile.trongrid.io`. Override via `--rpc-url`.
- Nile faucet: `https://nileex.io/faucet` — operator must request test TRX + test USDT-TRC20 before running transfers.

**Rejected:** Shasta (`https://shasta.trongrid.io`) — official but less documentation, lower faucet reliability, address prefix `0x41` matches mainnet (complicates local dev).

---

## Story 28 — Connect to RPC endpoint with SPKI pin (Alice, Bob) **[Q7 resolution]**

> As Alice, I want to pin the SPKI hash of my RPC endpoint so I detect MITM certificate swaps, so I can transact against `https://api.trongrid.io` or other public RPCs from a hostile network without trusting the system CA store.

**Acceptance criteria:**

- `tron --rpc-url pinned://<spki-hex>@api.trongrid.io config show` parses the `pinned://` scheme + extracts the SPKI pin (32-byte SHA-256 hex) from the URL.
- `reqwest::Client` builder uses `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` (reused verbatim — same `rustls` 0.23 version, same pin format).
- **Library test:** `tests/spki_pin_localnet.rs::spki_pinned_endpoint_rejects_wrong_pin` connects to a known HTTPS endpoint with a wrong pin (all-zero bytes) and asserts the error class is `Error::SpkiPinMismatch { expected, actual }` (or equivalent).
- **Library test:** same file `spki_pinned_endpoint_accepts_correct_pin` connects to a real public RPC with the cert's actual SPKI pinned and asserts the JSON-RPC call returns successfully (or returns the expected transport-level error, not pin-mismatch).
- `--allow-insecure-tls` (debug only) bypasses the pin verifier with a `tracing::warn!`.

---

## Story 29 — Connect to RPC endpoint without SPKI pin (system CAs only) (Alice)

> As Alice, I want to opt out of SPKI pinning for local development or trusted-network use, so I can point `tron` at a local TronBox or LAN RPC node without pinning ceremony.

**Acceptance criteria:**

- `tron --rpc-url http://127.0.0.1:8090 ...` uses plain `reqwest::Client::new()` + system CAs + `webpki-roots`. **No pinning. Validates the trust-on-first-use contract for localhost + LAN nodes.**
- `tron --rpc-url https://api.trongrid.io ...` (without `pinned://`) uses the same plain path — system CAs validate Cloudflare's cert. Convenient but accepts any cert the OS trusts (no MITM detection).
- **Library test:** `tests/spki_pin_localnet.rs::no_pin_localhost_tronbox_succeeds` spawns TronBox via Node, connects via `Client::new()`, asserts `eth_chainId` via `/jsonrpc` returns the expected TronBox chain-id.
- Test pattern matches the existing `bitcoin-wallet-core/src/chain/spki.rs` test suite.

---

## Cross-cutting acceptance criteria (apply to all stories)

- **Help text:** every command accepts `--help` and prints a clear, multi-line description with examples.
- **Exit codes (mirrors eth #297 M11):** documented and stable (0 = success, 1 = user abort, 2 = bad input — operator passed a value that doesn't fit the function surface, *or* an ABI decode failure on a `wallet/triggerconstantcontract` (view call) or `wallet/triggersmartcontract` (state-changing broadcast) response that was well-formed JSON but wrong shape — `Error::AbiDecodeFailed` per #399 M-ABI distinguishes decode fail from transport fail), 3 = upstream/RPC transport failure (connection refused, HTTP error, chain-id mismatch, malformed response), 4 = wallet/balance issue (insufficient funds, unknown wallet, insufficient token balance, missing pre-image), 5 = signing/RPC/broadcast error.
- **Address display (per #399 Q4):** all addresses display in T-base58check format (34 chars starting with `T`). Override via `--address-format base58check|hex-21-byte|hex-20-byte`.
- **Amount unit (TRON-specific):** all amount flags accept TRX (`--amount 1.5`) or sun (`--amount-sun 1500000`) — both are valid; reject ambiguity in one command (exit 2 if both given). For TRC-20, `--amount` uses the token's `decimals()`.
- **Resource unit:** `--fee-limit` always in sun (1 TRX = 1_000_000 sun). Resource estimates from `wallet/triggerconstantcontract` (per-contract `energy_used`) + `wallet/getbandwidth` (free bandwidth remaining) + `wallet/getchainparameters` (`getEnergyFee` for sun/Energy rate) displayed in `energy` / `bandwidth` / `sun_per_energy` units (see Story 8).
- **Confirmation prompts** (`mainnet`, `drain`, `unlimited approval`, `--no-private` export): require typing `yes` (not `y`); default is abort; exit code 1 on abort.
- **Mnemonic at rest (per #399 B2):** encrypted with Argon2id + AES-256-GCM (F5/F6) — never plaintext. Operator must run `tron wallet unlock` (or set `TRON_WALLET_PASSPHRASE`) on every CLI call that touches key material.
- **Output:** human-readable by default; `--json` flag on every command that produces data.
- **Stderr for diagnostics:** logs and errors go to stderr; stdout contains only the requested data (safe to pipe).
- **No background processes:** every `tron` invocation is a single foreground command. No daemons.
- **No telemetry:** the CLI makes no network calls except to the configured RPC URL.
- **Bounded inputs:** batch limited to 100 recipients (Story 13); `--fee-limit` bounded `[0, 10^15]` sun; derivation path must start with `m/44'/195'/`.
- **TLS pinning (per #399 Q7):** when `--rpc-url` points to a pinned endpoint (e.g., `pinned://<hex>@api.trongrid.io`), the custom SPKI verifier applies. Reuses `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` verbatim. `--allow-insecure-tls` flag disables pinning (debug only).
- **Network discrimination:** the CLI refuses to send on the wrong network. Since all production networks (Mainnet, Shasta, Nile) share address prefix `0x41`, discrimination is by chain-id only — the sender's address does not encode the network. The CLI asserts the target recipient's chain-id matches the active `--network` flag before signing; mismatch fails fast with exit 5 `chain_id mismatch: <expected> expected, tx derived from <actual>`.

---

## Out of scope for v0.1 (deferred per issue #399)

- **TRC-10 token transfers.** TRC-10 has separate `TransferAssetContract` proto encoding. Bundle into v0.3+ if needed.
- **Stake/unstake/freeze resource delegation.** `FreezeBalanceV2Contract` + `UnfreezeBalanceV2Contract` + `WithdrawExpireUnfreezeContract` proto encodings all separate. v0.3+.
- **Hardware wallet** (Ledger, Trezor, Keystone). Same deferral as eth #293 — out of scope until v1.x.
- **TRON DEX integration** (SunSwap, Sun.io). Smart contract interaction beyond `transfer` / `approve` / `balanceOf` / `decimals` / `symbol` / `name` out of scope.
- **Smart contract deployment via wallet.** Sign-only + broadcast external path is enough for v0.1.
- **Multi-sig / governance flows.** Different signer model entirely.
- **L2 / sidechains** (BitTorrent Chain BTTC, etc.). The `ChainId::Tron(u32)` placeholder in `chain-traits/src/lib.rs:21` already supports a discriminator — drop in another chain-id constant + another RPC URL. UX: `--network bttc` flag.
- **Stake 2.0** (delegated resource model). v0.3+ after the mainnet Stake 2.0 stabilizes.
- **gRPC transport.** v0.1 is JSON-RPC over HTTP only. gRPC can come later if TronGrid gRPC perf becomes a bottleneck (`grpc.trongrid.io:50051`).
- **Local tx index** (cached tx history). Per eth #297 D2 / #399 out-of-scope, every `tx list` call scans blocks (slow). v0.3+.
- **Watch-only wallet import** from an xpub (no signing key). Mirrors Bitcoin Plan Task 32 / eth Story 19 (export-only, not import).
- **Plausible-deniability multi-bucket wallet.** v1.0+.
- **REST/HTTP interface** (Breez-style server). Separate spec. Owner: TBD (per eth #297 M12 follow-up needed).
- **Mobile (iOS/Android) integration.** Phase 2 via UniFFI (mirrors Bitcoin + eth Phase 2).

---

## v0.1 release status (issue #399, in-progress)

Status snapshot for the `tron-wallet-core v0.1.0` library + `tron` CLI scaffold release cut. 29 stories; all deferred until spike V1-V10 lands + plan doc resolves Q5-Q10.

### Stories blocked on spike (29)

- [ ] **Story 1** — Create a new wallet. Blocked on spike V10 (SLIP-44 vector) + V4 (base58check). Library surface: `bip39::Mnemonic::generate_in` + `bip32::XPrv::derive_path("m/44'/195'/0'/0/0")` + `k256::SigningKey` + `tiny-keccak::Keccak256(pubkey)` + `bs58` + configurable prefix byte.
- [ ] **Story 2** — Import an existing wallet. Same primitives as Story 1.
- [ ] **Story 3** — Check TRX balance. Blocked on V1 (reqwest + serde_json compile-check).
- [ ] **Story 4** — Sync chain state. Blocked on V6 (Nile `eth_chainId` via `/jsonrpc` returns `0xcd8690dc`).
- [ ] **Story 5** — Send native TRX. Blocked on V2 (prost-build protobuf round-trip) + V8 (sign-only path) + V6 (Nile broadcast).
- [ ] **Story 6** — Send with custom `fee_limit`. Blocked on V2 + V5 (energy estimate).
- [ ] **Story 7** — Inspect transaction history. Blocked on V1 (RPC compile-check) + V6 (Nile tx query).
- [ ] **Story 8** — Get current energy/bandwidth estimates. Blocked on V5 (resource model + exact field names).
- [ ] **Story 9** — List / show / delete / rename wallets. Independent of spike. CLI scaffold can ship without Vn.
- [ ] **Story 10** — Use mainnet explicitly. Blocked on V6 (Nile chain-id assertion) + prefix-byte configuration.
- [ ] **Story 11** — Show config + debug info. Independent of spike.
- [ ] **Story 12** — Persist wallet across CLI invocations. v0.2+ per #399 B3 (mirrors eth v0.3 Story 12).
- [ ] **Story 13** — Send to multiple recipients (sequential txs). Blocked on V2 + V8.
- [ ] **Story 14** — Sweep / drain wallet. Blocked on V1 + V2.
- [ ] **Story 15** — Choose `ref_block` strategy (auto vs manual). Blocked on V2 + V6.
- [ ] **Story 16** — Manual `expiration` + `fee_limit` override. Blocked on V2.
- [ ] **Story 17** — Replace / speed-up tx. Blocked on V2 + V6 + V8.
- [ ] **Story 18** — Sign personal message (raw). Blocked on V10 (key derivation) + V4 (address encoding).
- [ ] **Story 19** — Export xpub + first addresses. Blocked on V10 + V4.
- [ ] **Story 20** — Pick derivation path. Blocked on V10.
- [ ] **Story 21** — Send TRC-20 stablecoin. Blocked on V2 + V3 (TRC-20 ABI encoder) + V9 (token registry).
- [ ] **Story 22** — Check TRC-20 token balance. Blocked on V3 + V9.
- [ ] **Story 23** — List registered TRC-20 stablecoins / tokens. Blocked on V9.
- [ ] **Story 24** — Add custom TRC-20 token by contract address. Blocked on V3 + V9.
- [ ] **Story 25** — Approve TRC-20 spending. Blocked on V2 + V3.
- [ ] **Story 26** — Use TronBox local node. Blocked on V1 (RPC compile-check) + TronBox availability.
- [ ] **Story 27** — Use Nile testnet. Blocked on V6.
- [ ] **Story 28** — SPKI pin RPC endpoint. Blocked on V7 (SpkiPinnedVerifier reuse).
- [ ] **Story 29** — No-pin RPC endpoint. Independent of spike.

### Stories that can ship in CLI scaffold without spike (3)

- Story 9 (list/show/delete — `std::fs` only, no chosen-crate dep)
- Story 11 (config show — `env!` only, no chosen-crate dep)
- Story 29 (no-pin RPC — `reqwest` + system CAs, V1 only)

### Cross-cutting (no user-facing flip)

- `--json` everywhere — `serde_json` in CLI scaffold args
- Stable exit codes per #399 M-EXIT (mirror eth #297 M11)
- `Secret<Mnemonic>` zeroize — `zeroize::Zeroizing<Mnemonic>` (F47 mirror BTC Task 30)
- T-base58check address display — `bs58` + 4-byte double-SHA-256 checksum

### Try it (target surface, post-spike)

```bash
# Library surface (target after V1-V10 PASS)
cargo test -p tron-wallet-core --lib                            # unit tests pass
cargo test -p tron-wallet-core --test mnemonic                  # Story 1, 2, 20 (V10)
cargo test -p tron-wallet-core --test address                   # Story 1, 2, 19 (V4)
cargo test -p tron-wallet-core --test protobuf                 # Story 5, 17, 21 (V2)
cargo test -p tron-wallet-core --test trc20                    # Story 21, 22, 25 (V3, V9)
cargo test -p tron-wallet-core --test spki_pin                 # Story 28 (V7)

# Nile smoke (operator-driven per L29 — set RUN_TRON_NILE=1)
RUN_TRON_NILE=1 cargo test -p tron-wallet-core --test '*'        # Stories 5, 6, 21, 22 against Nile

# CLI scaffold
cargo run -p tron -- --help                                    # show clap subcommand LAYOUT
cargo run -p tron -- wallet --help                             # wallet subcommand tree
cargo run -p tron -- tx --help                                 # tx subcommand tree
cargo run -p tron -- trc20 --help                              # trc20 subcommand tree
cargo run -p tron -- config show                               # Story 11
```

### References

- Issue #399 (this release cut spec, Q1-Q10 open questions)
- Plan (forthcoming): `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`
- Deep-dive (Ticket A, PR #402): `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md`
- Ethereum user stories (template): `docs/wallets/2026-08-23-eth-wallet-user-stories.md`
- Bitcoin deep-dive (SPKI pin pattern source): `docs/wallets/2026-08-05-bitcoin-rust-sdks-deep-dive.md`
- Bitcoin `SpkiPinnedVerifier` source: `bitcoin-wallet-core/src/chain/spki.rs`
- SLIP-0044 coin types (TRON = 195): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- TRON Developer Hub — Transactions: <https://developers.tron.network/docs/tron-protocol-transaction>
- TRON Developer Hub — Encoding: <https://developers.tron.network/docs/encoding>
