# TRON-Specific Rust SDK Deep-Dive

**Date:** 2026-08-27
**Scope:** Focused re-research on Rust crates for a TRON (TRX + TRC-20 stablecoin) wallet built inside `rust-wallet-app/`, covering native TRX transfer, TRC-20 token transfer (USDT-TRC20 primary, USDC-TRC20 secondary), address encoding (base58check T-prefix + TVM hex), and resource-model fee handling (energy + bandwidth). Verifies the chosen crate surface against current 2026 state, considers alternatives, and digs into protobuf tx construction + TRC-20 ABI reuse.
**Companion to:** `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` (Ethereum precedent — primary cross-chain reference). Tracks issue #399, mirrors eth PR #290 → #293 review → #294 plan flow.
**Pre-empts:** v0.3+ deliverable sketched in `rust-wallet-app/crates/chain-traits/src/lib.rs:21` (`ChainId::Tron(u32)` placeholder for TRON coin).
**Status:** Research report only. No design spec, no implementation plan, no code produced in this session.

## TL;DR

Use **raw `reqwest` + `prost`/`prost-types`** as the primary TRON stack, with **`k256` for secp256k1 signing**, **`bs58` for base58check encoding**, **`bip32`/`bip39` for HD wallet** (already workspace deps from Bitcoin side), **`sha2` + `tiny-keccak` for tx-hash + address derivation**, and the same **`rustls`-backed SPKI pin verifier** from `bitcoin-wallet-core/src/chain/spki.rs` (reused for Q7 RPC pinning, path verified 2026-08-27). Four new direct deps needed: `prost` 0.14, `prost-types` 0.14, `bs58` 0.5, `tiny-keccak` 2.0.2 (keccak256, distinct from sha3 crate — wire-format hazard, see §"tiny-keccak"). Existing workspace `reqwest`, `rustls`, `bip32`, `bip39`, `sha2`, `k256` cover the rest. The `tronic` crate (`39george/tronic`, v0.6.1, gRPC-only — JSON-RPC still WIP) is noted as an Alloy-inspired experimental alternative but **rejected for v0.1 due to 7-star single-maintainer risk**. Same risk profile applies to the new `throgxyz/tronz` (55 stars but 1 follower, created 2026-06-14) and the multi-chain `0xcregis/anychain` (253 stars but broader scope). Build our own `TronTransaction` proto via `prost 0.14.4` against `core/Tron.proto` (Buf mirror at `buf.build/streamingfast/tron-protocol`). No dependency on TronGrid-proprietary APIs — standard `wallet/*` JSON-RPC endpoints work for both mainnet and Nile testnet. Resource model (Q5) is now fully resolved per §"Resource model — verified 2026 numbers": 1 TRX = 1 TP under Stake 2.0 (April 2023), bandwidth = 600 free/day + stake-share, energy priced at 100 sun/Energy default, USDT-TRC20 transfer consumes 65k Energy (recipient holds USDT) or 130k (empty recipient).

## The chosen surface — current 2026 state

| Crate | Version | Role | Reused from workspace? | License | Maintained? | Mobile-friendly? | Notes |
|---|---|---|---|---|---|---|---|
| `reqwest` | 0.12 (workspace) | JSON-RPC HTTP transport | Yes — already workspace dep (Bitcoin F20, eth Q2) | Apache-2.0 / MIT | Yes | Yes — `rustls-tls` feature | JSON-RPC client to `https://api.trongrid.io/wallet/*` (mainnet) and `https://nile.trongrid.io/wallet/*` (Nile). Same `pinned://` SPKI scheme from Bitcoin. |
| `rustls` | 0.23 (workspace) | TLS + custom `ServerCertVerifier` for SPKI pin | Yes — workspace dep | Apache-2.0 / MIT | Yes | Yes | SPKI pin verifier reuses `bitcoin-wallet-core/src/chain/spki.rs` (F20 finding). Verify pattern matches before reuse. |
| `bip32` | ^0.5 (workspace) | HD derivation `m/44'/195'/0'/0/0` | Yes — workspace dep from Bitcoin | MIT | Yes | Yes | Same `XPrv` + `DerivationPath` API. Only coin type changes (195 = TRX). |
| `bip39` | 2.2 (workspace) | BIP-39 mnemonic generate/parse/to_seed | Yes — workspace dep from Bitcoin | MIT | Yes | Yes | BIP-39 wordlist identical to Bitcoin. Seed identical for same mnemonic. |
| `k256` | latest (workspace) | secp256k1 signing primitive | Yes — workspace dep via Bitcoin | Apache-2.0 / MIT | Yes | Yes | Pure-Rust, no FFI. Signs 32-byte SHA-256 digest per TRON spec. |
| `sha2` | (workspace) | SHA-256 for tx-hash + base58check checksum | Yes — workspace dep | Apache-2.0 / MIT | Yes | Yes | `Sha256::digest(...)` for both tx-hash and base58check double-hash. |
| `prost` | **0.14.4** (2026-06-07) | Protobuf serialization for `Transaction` | **NEW** — add to workspace | Apache-2.0 | Yes | Yes | MSRV Rust 1.85 (matches workspace). Build `core/Tron.proto` via `prost-build` in `build.rs`. Version corrected 2026-08-27 (prior doc said 0.13 — wrong; 0.14 is the current line and is what `tronic` already pins). |
| `prost-types` | **0.14.4** (2026-06-07) | Protobuf well-known types (Timestamp) | **NEW** — pulled by `prost` | Apache-2.0 | Yes | Yes | Required for `google.protobuf.Timestamp` if used in Tron proto extensions. Likely transitive. |
| `bs58` | 0.5 | base58 + base58check encoding | **NEW** — add to workspace | MIT | Yes | Yes | Canonical impl used by `rust-bitcoin`, `solana-sdk`, `near-primitives`. Well-audited. |
| `tiny-keccak` | 2.0.2 | Keccak-256 hash (≠ SHA3-256) | **NEW** — add to workspace | CC0 / Apache-2.0 | Yes | Yes | TRON address derivation uses Keccak-256 (Ethereum-compatible), NOT SHA3-256. tiny-keccak's `Keccak256` is the right primitive. `sha3` crate is also valid but tiny-keccak is smaller and already used by alloy transitively. |
| `serde` + `serde_json` | (workspace) | JSON-RPC request/response parsing | Yes — workspace dep | Apache-2.0 / MIT | Yes | Yes | `{"jsonrpc":"2.0","method":"...","params":{...},"id":1}` envelope. |

Total new direct deps: **4** (`prost`, `prost-types`, `bs58`, `tiny-keccak`). Workspace dep count grows by ~6 transitive crates via `prost-build` + `prost-types`. No FFI footprint. All Rust-native.

## Why raw reqwest + prost and not a TRON SDK

### Landscape survey (2026)

Eight candidate SDKs/projects exist as of 2026-08-27. Two are stale, three are promising but immature (single-maintainer risk), three are tangential or multi-chain.

| Crate / Repo | Stars | Last commit | License | Status | Verdict |
|---|---|---|---|---|---|
| `rust-tron` (`andelf`) | 50 | **2025-01-09** (~20 mo ago, effectively stale) | **LGPL-3.0** (copyleft concern) | No git tags. README still claims "active development". gRPC-only. | **Reject.** Stale + LGPL copyleft is a closed-source wallet integration risk. Reference impl for `Address::from_public` base58check pattern only (see §"Address encoding"). Prior doc said "2021-03-06" — corrected 2026-08-27 via GitHub API. |
| `tron-rs` (crates.io) | n/a (crates.io: 14.88k total DL) | **v0.1.0 published 2026-01-20** | (unspecified in crate) | Proto definitions only, no signing, no JSON-RPC. Built on `cosmrs`. | **Reject as primary.** Useful as a `core/Tron.proto` reference for `prost-build` schema. |
| `tronic` (`39george`) | 7 | **v0.6.1 on 2026-07-20** (active) | Apache-2.0 / MIT | Alloy-inspired, async-first, **gRPC only** — JSON-RPC still WIP per README TODO. Uses `alloy-sol-types = "1.4"` for TRC-20 calldata. Single primary maintainer. | **Watch — not adopt for v0.1.** Compelling API design but 7 stars + single maintainer. Re-evaluate at v0.3 once it crosses ~50 stars and ≥2 maintainers OR ships a working JSON-RPC client. |
| `tronz` (`throgxyz`) | **55** (newer than tronic) | **2026-08-25** (active) | Apache-2.0 | Created 2026-06-14 (~2.5 mo old). "Modern Rust SDK for the TRON network with async-first RPC APIs, inspired by Alloy". No releases tagged yet. Maintainer `throgxyz`: 1 follower, 9 public repos — new/small account. | **Watch alongside tronic.** More stars than tronic but same single-maintainer risk profile. Re-evaluate at v0.3 when either ships stable JSON-RPC + ≥2 maintainers. Prior doc said "0-1 stars, no engagement" — corrected 2026-08-27 via GitHub API. |
| `0xcregis/anychain` | **253** | **2026-08-26** (active) | (multi-chain) | Multi-chain Rust wallet SDK (BTC/ETH/Tron/Solana). Has published gitbook at `cregisoffical.gitbook.io/anychain/`. Not TRON-specialized. | **Watch — multi-chain abstraction may be relevant** if v0.3+ unifies the chain-traits surface. Lower priority than tronic/tronz for TRON-only work. |
| `edwintuan/next-wallet` | n/a (not star-counted here) | 2026-05-21 | (check repo) | Terminal-native crypto wallet for Tron: TUI + CLI, multi-wallet, **Stake 2.0, SR voting**. Useful as **UX reference** for the Stake 2.0 / vote flows if we ship those in v0.2+. | **Reference only.** Not a library dep — read for UX inspiration. |
| `Gingerbreadfork/tron-goblin-node` | 8 | 2026-08-25 | (check repo) | From-scratch Rust TRON **full node** targeting byte-exact consensus parity with java-tron. Not a wallet SDK. | **Reject for wallet purposes.** Relevant only if we ever need consensus-valid TRC-20 tooling. |
| `walletsuite/walletsuite-tx-compiler`, `Hixon10/usdt-wallet-rs`, `OpenSettle/opensettle-sdk-rust`, `rootdigit/sagapay-rust`, `derJanusz/rust-tron` | 0–2 | various 2026 | various | Narrow-scope SDKs (single-vendor or single-chain). | **Reject for v0.1.** Re-survey at v0.3 if any cross 10 stars + multi-maintainer. |

### Decision: build on raw reqwest + prost

Three reasons, in priority order:

1. **Maintenance risk floor.** The two production-grade attempts (`rust-tron`, `ronic`) have either gone stale (2021) or are pre-1.0 with one maintainer. For a wallet handling real value, the build surface we own must be small and well-understood. `reqwest` + `rustls` + `prost` + `bs58` + `k256` + `sha2` + `tiny-keccak` is **the same stack Bitcoin and Ethereum use** — every crate here has ≥1M downloads/week and a multi-maintainer governance model.

2. **Bitcoin-side precedent.** `bitcoin-wallet-core` already works this way for SPKI-pinned HTTP transport (F20 finding, Task 7 of the Bitcoin plan). Reusing the pattern — not the SDK — is the proven path.

3. **Cross-chain type unification.** Ethereum and TRON share Keccak-256 for address derivation. The Bitcoin-side pattern of "build thin Rust wrappers around well-audited primitives" composes cleanly with the eth-wallet-core pattern (where alloy-signer-local wraps k256 the same way). Adding `tiny-keccak` to the workspace pays dividends for both chains — we may eventually want Keccak-256 on Bitcoin side too (EIP-7503 silent payments, BIP-coinjoin tx hashing).

**Rejected alternatives:**

| Alternative | Why rejected |
|---|---|
| `rust-tron` (andelf) | Effectively stale (last push 2025-01-09, no git tags, README claim of "active development" stale). **LGPL-3.0** is a copyleft concern for closed-source wallet integrations. Reference impl cited in docs only for `Address::from_public` base58check pattern. |
| `tronic` (39george) | v0.6.1 gRPC-only (JSON-RPC still WIP), 7 stars, single maintainer. Re-evaluate at v0.3 once JSON-RPC ships and it crosses ~50 stars + ≥2 maintainers. Already validates `alloy-sol-types` reuse for TRC-20 calldata — useful proof-by-example. |
| `tronz` (throgxyz) | 55 stars, single new maintainer (1 follower, 9 public repos), created 2026-06-14. No tagged releases yet. Watch alongside tronic; reject for v0.1. |
| `0xcregis/anychain` | 253 stars but multi-chain scope (BTC/ETH/Tron/Solana). Not TRON-specialized. Watch if chain-traits unification work lands in v0.3. |
| `tron-rs` | Proto defs only — no signing, no JSON-RPC. v0.1.0 on crates.io (2026-01-20). Use as `prost-build` schema source. |
| Hand-rolled protobuf encoder | `prost` 0.14.4 is the standard. Hand-rolling invites subtle wire-format bugs (see §"TriggerSmartContract field numbers" for off-by-one hazards). |
| Hand-rolled Keccak-256 | `tiny-keccak` 2.0.2 is the standard. Hand-rolling is a security liability. The Keccak-256 vs NIST SHA3-256 padding distinction (0x01 vs 0x06) is the exact bug a hand-rolled impl gets wrong. |
| `ethers-rs` against TronGrid via a TRON-compatible EVM endpoint | TRON is **not** EVM-compatible at the consensus layer. Its TVM (TRON Virtual Machine) is Solidity-compatible but the **transaction format is protobuf**, not RLP. ethers-rs cannot encode TRON transactions. Rejected per Q1. |
| `web3` (Parity) against TRON | Same — transaction format mismatch. |

## Crate-by-crate deep-dive (2026)

### `prost` 0.14.4 + `prost-build` 0.14.4 (NEW workspace deps)

**Why this one:** The de-facto Rust protobuf implementation. Used by `tonic` (gRPC), `substrate`, `nearcore`, `solana-sdk`, and most Rust protobuf consumers. Apache-2.0, maintained by tokio-rs (Casper Meijn). **Version corrected 2026-08-27** — prior doc said 0.13; the current line is 0.14.4 (released 2026-06-07, MSRV Rust 1.85). `39george/tronic` already pins `prost = "0.14"` — staying on the 0.14 line keeps ecosystem alignment.

**API surface used by the TRON wallet:**

- `prost::Message` trait — `Transaction::encode(&mut buf)` (serialize) and `Transaction::decode(&buf[..])` (parse).
- `prost-build::Config::new().compile_protos(&["core/Tron.proto"], &["proto/"])?` in `build.rs` — generates Rust types at compile time from the canonical `core/Tron.proto`.
- Generated types land in a module the wallet crate declares via `prost-build`'s output file.

**Protobuf schema source:**

The canonical schema is `core/Tron.proto` in the `tronprotocol` repo (`github.com/tronprotocol/java-tron`, `protocol/src/main/protos/core/Tron.proto` on `develop` branch). Buf mirror at `buf.build/streamingfast/tron-protocol/file/84fc05905d3a49318eaafd7e63e2e5e4:core/Tron.proto` (mirrors git, can break if upstream renames). **Latest commit on file (develop, 2026-07-14): `851575d` "revert go_package to github.com/tronprotocol/grpc-gateway (#6874)"** — schema changes have been grpc-gateway housekeeping only; `TransferContract` / `TriggerSmartContract` field shapes unchanged. The schema is also documented in `java-tron/Tron protobuf protocol document.md` (developer-friendly prose form).

**Transaction structure** (from `developers.tron.network/docs/tron-protocol-transaction`, verified against live mainnet block 85707935 on 2026-08-27):

```text
Transaction {
  raw_data: Transaction.raw_data,
  signature: [bytes]   // 65 bytes each: r(32) ‖ s(32) ‖ v(1, recovery 0 or 1)
}

Transaction.raw_data {
  contract: [Contract],            // exactly one in current practice
  ref_block_bytes: bytes,          // TAPOS — bytes [6,8) of ref block number (2 bytes)
  ref_block_hash: bytes,           // TAPOS — bytes [8,16) of blockID (8 bytes)
  expiration: int64,               // ms epoch; head_block_ts + 60_000 default; max head_block_ts + 86_400_000
  data: bytes,                     // optional memo
  timestamp: int64,                // ms epoch; informational, not authoritative
  fee_limit: int64                 // max SUN (1 TRX = 1_000_000 sun) for energy/bandwidth burn
}

Contract {
  ContractType type,               // TransferContract, TriggerSmartContract, ...
  google.protobuf.Any parameter    // oneof: serialized contract body
}
```

**`TransferContract` field numbers** (from `core/Tron.proto`, verified 2026-08-27):

| # | Field | Type | Notes |
|---|---|---|---|
| 1 | `owner_address` | bytes | 21-byte raw address (`0x41` + last 20 bytes of `keccak256(pubkey)`) |
| 2 | `to_address` | bytes | same form |
| 3 | `amount` | int64 | sun (1 TRX = 1_000_000 sun) |

**`TriggerSmartContract` field numbers** (from `core/Tron.proto`, verified 2026-08-27 — **off-by-one hazard flagged**):

| # | Field | Type | Notes |
|---|---|---|---|
| 1 | `owner_address` | bytes | 21-byte raw sender address |
| 2 | `contract_address` | bytes | TRC-20 token contract (21-byte raw) |
| 3 | `call_value` | int64 | TRX sent with call (0 for TRC-20 transfer) |
| 4 | **`data`** | bytes | **TRC-20 calldata: 4-byte selector + ABI-encoded args** |
| 5 | `call_token_value` | int64 | TRC-10 token value (0 for TRC-20) |
| 6 | `token_id` | int64 | TRC-10 token id (0 for TRC-20) |

**Encoding hazard:** field #4 is `data`, NOT #3. `call_value` is #3. Hand-written encoders that assign calldata to field 3 (the natural-looking "third field" position for ABI input) will produce invalid transactions that silently fail decode at the receiving node. Spike V2 must round-trip a hand-crafted `TriggerSmartContract` with `data` at field 4 and verify the network accepts it.

**Signing algorithm** (verified 2026-08-27 against `developers.tron.network/docs/transaction-signature-validation`):

```text
tx_hash = SHA-256(protobuf-serialize(raw_data))    # txID = same hash
signature = secp256k1_sign(k256_signing_key, tx_hash)   # 65 bytes: r(32 BE) ‖ s(32 BE) ‖ v(1, in {0, 1})
signed_tx = Transaction { raw_data, signature: [signature] }
```

**Critical convention difference from Ethereum:** TRON uses `r ‖ s ‖ v` with `v ∈ {0, 1}`. Ethereum uses `v ‖ r ‖ s` with `v + 27 ∈ {27, 28}`. Several SDKs and tools default to Ethereum format and silently produce invalid signatures. The official docs explicitly warn: *"Reversing the order produces an invalid signature that recovers the wrong address."* Spike V2 must round-trip sign → verify → recover on a known message to catch this class of bug at build time.

`secp256k1_sign` returns 64 bytes (r, s). The recovery byte `v` is computed by trying both `0` and `1` against `tx_hash` and `signature`, recovering the public key, and comparing to the expected sender's public key. `k256::ecdsa::Signature::from_sliced_64(...)` + `k256::ecdsa::VerifyingKey::recover_from_prehash(...)` does this in pure Rust. **`GreatVoyage-v4.8.2+` accepts 65–68 bytes for legacy compat but truncates to first 65 before validation** — newly built transactions should still be exactly 65 bytes.

**TAPOS source block — prefer SolidityNode for finality:** `wallet/getnowblock` returns the latest *unsolidified* block; `walletsolidity/getnowblock` returns the latest *solidified* block. **Wallet should call `walletsolidity/getnowblock`** so the reference block survives microforks. Then extract `block_header.raw_data.number` (8-byte big-endian) and `blockID` (32 bytes = `number_be ‖ SHA-256(raw_data)[8..32]`); the `ref_block_bytes`/`ref_block_hash` are slices `[6,8)` and `[8,16)` respectively (verified live against mainnet block 85707935 on 2026-08-27).

**Risks:**

- **`prost-build` adds `protoc` as a build dependency.** Build script will invoke the system `protoc` compiler (≥3.12). CI must have `protobuf-compiler` installed. Alternative: ship pre-generated `.rs` files via `include!` and skip `build.rs` — possible but couples the source tree to a specific schema version. **Recommendation: use `build.rs` with system `protoc`, document in README.**
- **Protobuf wire format vs JSON-RPC for tx broadcast.** Most public RPC nodes (TronGrid, Nileex) accept both JSON-encoded `Transaction` and raw-protobuf `Transaction`. We use JSON for the request envelope (matches `wallet/broadcasttransaction` API) but the inner `raw_data` field is hex-encoded protobuf bytes. **This is the JSON convention, not a Rust choice.**
- **Schema drift.** `core/Tron.proto` in `tronprotocol` repo's `develop` branch can change. Pin to a specific commit SHA via `prost-build`'s `compile_protos` call (read the schema from a vendored copy). Spike V2 verifies the chosen SHA produces valid transactions on Nile testnet. Recommended SHA: **`851575d` (2026-07-14)** at session time — re-pin before each spike run.
- **`wallet/getchainid` is HTTP 405 on TronGrid's HTTP front** (verified 2026-08-27 against `https://api.trongrid.io/wallet/getchainid`). Use the `eth_chainId` JSON-RPC method via `/jsonrpc` instead — works on all three networks (Mainnet `0x2b6653dc`, Shasta `0x94a9059e`, Nile `0xcd8690dc`).

### `bs58` 0.5 (NEW workspace dep)

**Why this one:** The canonical Rust base58 + base58check implementation. Used by `rust-bitcoin`, `solana-sdk`, `near-primitives`, `bitcoinjs-lib` (Rust port). MIT, well-audited, 8+ years of stable API.

**API surface:**

- `bs58::encode(bytes).into_string()` → `String` — plain base58.
- `bs58::decode(s).into_vec()?` → `Vec<u8>` — plain base58 decode.
- For base58check (TRON): prepend 4-byte double-SHA-256 checksum, then `bs58::encode(payload_with_checksum).into_string()`. Decode: `bs58::decode(s).into_vec()?` then split off last 4 bytes and verify `SHA256(SHA256(payload)) == checksum`.

**Encoding detail (canonical reference impl):** `andelf/rust-tron/keys/src/address.rs` shows the exact byte sequence — `ADDRESS_TYPE_PREFIX = 0x41` for mainnet/Shasta/Nile prefix `0x41` (`developers.tron.network/docs/encoding`), 21-byte raw (prefix + last 20 bytes of keccak256(pubkey)), 4-byte double-SHA-256 checksum, then base58-encode the 25-byte payload. **Note: Nile testnet uses prefix `0x41`, NOT `0xa0`** — verified 2026-08-27 against the official Developer Hub encoding page. The `0xa0` prefix is a *legacy* `net.type = testnet` config flag in `Constant.java` that was never adopted for production networks. **The wallet does not need a configurable prefix byte** — `0x41` is universal across mainnet, Shasta, and Nile.

**Risks:**

- **Plain base58 vs base58check confusion.** `bs58` crate has `bs58::encode` (plain) but no built-in check. Two safe patterns: (a) hand-roll the checksum verify (4 lines, mirrors `andelf/rust-tron/keys/src/address.rs:b58decode_check`), or (b) use `bitcoin_hashes` from the `rust-bitcoin` workspace for `Hash160` + checksum composition. **Recommendation (a)** — keep the Bitcoin dep surface minimal.
- **Case sensitivity.** Base58 alphabet includes both upper and lower case. `bs58` crate handles case insensitively on decode; encode always produces the canonical mixed case. No wallet-side risk.

### `tiny-keccak` 2.0.2 (NEW workspace dep)

**Why this one:** Pure-Rust Keccak-256. CC0 / Apache-2.0 dual-licensed. Used by `alloy-primitives` transitively, `near-primitives`, `revm`, and `parity-common`. Tiny (no_std + unsafe-free), well-audited.

**API surface:**

- `Keccak256::new()` → `Hasher`.
- `hasher.update(bytes)`.
- `hasher.finalize()` → 32-byte `[u8; 32]`.

**Why Keccak-256 and not SHA3-256:** TRON address derivation uses the **Ethereum-flavored Keccak-256** (padding byte `0x01`, not `0x06`). SHA3-256 (NIST) and Keccak-256 produce different digests for the same input. `tiny-keccak::Keccak` is the right primitive; `sha3::Sha3_256` is NOT.

**Risks:**

- **Same as Bitcoin side for any future Keccak-256 use** — none. tiny-keccak is the canonical impl.

### `k256` (workspace dep, reused)

**Why reuse:** Already in workspace (Bitcoin signing primitive). TRON signing uses the same secp256k1 curve and SHA-256 prehash as Bitcoin — the only difference is the data being signed (the protobuf-serialized `raw_data`, not the Bitcoin double-SHA-256 of the tx bytes).

**API surface:**

- `k256::SigningKey::from_bytes(&secret_bytes)` — load 32-byte secret.
- `signing_key.sign_prehash(tx_hash)?` → `k256::ecdsa::Signature`.
- `Signature::to_bytes()` → 64 bytes (r ‖ s).
- `Signature::recover_from_prehash(tx_hash, &signature)?` → `VerifyingKey` (for computing the `v` recovery byte).
- `verifying_key.to_sec1_bytes()` → 65-byte uncompressed public key (for address derivation).

**Address derivation (reused):**

```text
pubkey = verifying_key.to_sec1_bytes()[1..65]   // 64-byte X‖Y, drop 0x04 prefix
addr_suffix = Keccak256(pubkey)[12..32]         // last 20 bytes
addr_raw = [0x41, addr_suffix]                   // 21-byte mainnet form
addr_base58check = base58check_encode(addr_raw)  // 34 chars, starts with T
```

**Risks:** None specific to TRON. Same `k256` handling as Bitcoin side — wrap secret in `Zeroizing<[u8; 32]>` for v0.1 (mirrors Bitcoin F47 zeroize treatment).

### `bip32` ^0.5 (workspace dep, reused)

**Why reuse:** Already a workspace dep from Bitcoin side. TRON BIP-44 derivation `m/44'/195'/0'/0/0` is identical in mechanics to Bitcoin BIP-44 derivation — only the coin type differs (195 vs 0). Same `XPrv` + `DerivationPath` API, same `to_secp256k1_secret_key(&secp)` extraction.

**SLIP-44 verification (Q10):** Coin type 195 = TRX confirmed via `github.com/satoshilabs/slips/blob/master/slip-0044.md` master branch. `bip_utils::slip44::Coin::Tron` const available (cross-checked). Canonical derivation path: `m/44'/195'/0'/0/0` (matches TronLink, Ledger, and `andelf/rust-tron` examples).

**Spike V10 must verify against a canonical test vector.** Candidate: BIP-39 test mnemonic `"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"` → seed → `m/44'/195'/0'/0/0` → expected address. The expected address must come from a reference implementation (TronWeb, TronLink, or `andelf/rust-tron` test vectors). Spike ties out.

**API surface:** identical to Bitcoin side. See `bitcoin-wallet-core/src/wallet/manager.rs` for the established pattern.

### `bip39` 2.2 (workspace dep, reused)

**Why reuse:** Already in workspace with `zeroize` + `rand` features. BIP-39 wordlist identical to Bitcoin. Same mnemonic generates the same seed.

**Risks:** None specific to TRON. Same zeroize handling as Bitcoin.

### `reqwest` + `rustls` (workspace deps, reused)

**Why reuse:** Standard HTTP transport. `rustls-tls` feature enabled in workspace `Cargo.toml`. Custom `ServerCertVerifier` reuses `bitcoin-wallet-core/src/chain/spki.rs` for SPKI pinning (Q7).

**TRON-specific JSON-RPC endpoints:**

| Network | HTTP base URL | Chain-id (hex / decimal) | Notes |
|---|---|---|---|
| Mainnet | `https://api.trongrid.io/wallet/*` | `0x2b6653dc` / 728126428 | Official. `TRON-PRO-API-KEY` header for higher rate limits. Chain-id verified live 2026-08-27 via `eth_chainId` JSON-RPC. |
| Shasta (testnet) | `https://api.shasta.trongrid.io/wallet/*` | `0x94a9059e` / 2494104990 | Active 2026 (GreatVoyage-v4.8.1, 2026-03-18). Chain-id verified live 2026-08-27. |
| Nile (testnet) | `https://nile.trongrid.io/wallet/*` | `0xcd8690dc` / 3448148188 | Community-maintained since 2021. Chain-id verified live 2026-08-27. **`0x94a9059e` is Shasta's chain-id, NOT Nile's** — the prior doc swapped them; corrected here. |
| Local regtest | `http://127.0.0.1:8090/wallet/*` (HTTP) or `http://127.0.0.0:50051/wallet/*` (gRPC) | n/a | `wallet-cli` from `java-tron` ships both. |

**JSON-RPC envelope:**

```text
→ POST https://api.trongrid.io/wallet/createtransaction
  Content-Type: application/json
  TRON-PRO-API-KEY: <key>
  {"owner_address": "T...", "to_address": "T...", "amount": 1000000}

← {"visible": true, "txID": "...", "raw_data": {...}, "raw_data_hex": "0a02..."}
```

The `visible: true` flag makes the response use T-base58check strings instead of hex — friendlier for wallet UIs. For programmatic consumption, set `visible: false` (default).

**Risks:**

- **Rate limits (corrected 2026-08-27).** TronGrid free tier: **15 QPS authenticated, 3 QPS unauthenticated, 100,000 req/day cap** (per `trongrid.io/changeLog/v1-10-0` reduction + Chainstack 2026 comparison). Prior doc said "~10/~90 QPS" — both numbers were stale. **Wallet should accept `--trongrid-api-key` CLI flag**, default to unauthenticated 3 QPS for dev, use key for production. Do NOT hard-code fixed QPS into business logic — TronGrid explicitly warns this can change with plan/network/endpoint.
- **SPKI pin endpoint.** `api.trongrid.io` serves a cert via Cloudflare; the SPKI pin must be regenerated when Cloudflare rotates (every ~30 days). Operator pain — see Bitcoin precedent for `pinned://` rotation strategy.

## Stablecoin (TRC-20) transfer — contract addresses + ABI

TRC-20 is **functionally identical to ERC-20** at the ABI level. The function selector for `transfer(address,uint256)` is the same `0xa9059cbb` on both chains. The wire format differs (TRON uses protobuf `TriggerSmartContract` wrapper around the ABI-encoded call data; Ethereum uses RLP tx with `input` field).

| Token | Mainnet contract (T-base58check) | Decimals | Symbol | Source |
|---|---|---|---|---|
| USDT (Tether USD) | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | **6** | USDT | TronScan token page + `andelf/rust-tron` WELLKNOWN_ADDRESS table (canonical) |
| USDC (Circle, deprecated TRC-20) | `TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8` | **6** | USDC | TronScan; Circle stopped issuing new TRC-20 USDC post-2023, existing token still active |
| TUSD (TrueUSD) | `TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9` | **18** | TUSD | TronScan |
| USDD (Decentralized USD) | `TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz` | **18** | USDD | TronScan |
| stUSDT (Staked USDT RWA) | `TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3` | **6** | stUSDT | TronScan |

**Nile (testnet) equivalents:** Nile runs a separate token registry. Test USDT on Nile = `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` (community faucet). For v0.1 spike, deploy a local `MockTRC20` to a local TronBox / `wallet-cli` regtest node — mirrors the eth Anvil MockERC20 pattern.

**TRC-20 ABI surface we need:**

```solidity
// TRC-20 standard subset (identical to ERC-20)
function name() external view returns (string)
function symbol() external view returns (string)
function decimals() external view returns (uint8)
function totalSupply() external view returns (uint256)
function balanceOf(address account) external view returns (uint256)
function transfer(address to, uint256 value) external returns (bool)
function approve(address spender, uint256 value) external returns (bool)
function allowance(address owner, address spencer) external view returns (uint256)
function transferFrom(address from, address to, uint256 value) external returns (bool)

event Transfer(address indexed from, address indexed to, uint256 value)
event Approval(address indexed owner, address indexed spender, uint256 value)
```

Function selectors (identical to ERC-20):

- `transfer(address,uint256)` → `0xa9059cbb`
- `balanceOf(address)` → `0x70a08231`
- `decimals()` → `0x313ce567`

**Encoding for send-stablecoin (USD-TRC20 = 1.50 USDT = 1_500_000 base units):**

```text
data = 0xa9059cbb                                              // transfer selector
      ‖ 000000000000000000000000<TVM_address: 20 bytes>        // to (Ethereum-style 32-byte word)
      ‖ 000000000000000000000000000000000000000000000000000000000016e360  // value = 1_500_000

TriggerSmartContract {
  owner_address:   <21-byte sender addr>,
  contract_address: <21-byte USDT-TRC20 contract>,
  data:            <ABI-encoded calldata above>,
  call_value:      0,
  call_token_value: 0,
  token_id:        0
}
```

**ABI encoding question (Q3 resolution):** TRC-20 ABI = ERC-20 ABI. The wallet has two options:

1. **Reuse `alloy-sol-types` from the eth workspace dep.** Once `eth-wallet-core` lands, `alloy-sol-types` becomes a workspace dep. The TRON wallet can declare `sol! { function transfer(address to, uint256 value) external returns (bool); }` and call `transferCall { to, value }.abi_encode()` to produce identical calldata. **Risk:** ties TRON wallet to alloy dep — heavyweight for a chain that doesn't otherwise need alloy.
2. **Hand-roll the ABI encoder for the three functions we need.** ~30 lines of code. Pure Rust, no dep. Bytes: selector (4) + padded address (32) + padded uint256 (32) = 68 bytes for `transfer`.

**Recommendation:** Hand-roll for v0.1 (smaller dep tree, clearer code, matches "build on raw primitives" theme). Re-evaluate `alloy-sol-types` reuse at v0.3 once both chains share a stable workspace.

**Encoding example (hand-rolled):**

```rust
fn encode_transfer(to_tvm_bytes: &[u8; 20], value: u128) -> [u8; 68] {
    let mut out = [0u8; 68];
    out[0..4].copy_from_slice(&[0xa9, 0x05, 0x9c, 0xbb]);  // transfer selector
    out[4..36].copy_from_slice(&pad_left_32(to_tvm_bytes));
    out[36..68].copy_from_slice(&pad_left_32(&value.to_be_bytes()));
    out
}

fn pad_left_32(b: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[32 - b.len()..].copy_from_slice(b);
    out
}
```

## Mnemonic-to-broadcast data flow (end-to-end)

```text
1. tron wallet create --name w --network mainnet
   ↓
2. bip39::Mnemonic::generate_in(Words12, English, rng)  -- 12-word phrase (identical to BTC/ETH)
   ↓
3. m.to_seed(passphrase)  -- 64-byte PBKDF2 output (identical to BTC/ETH)
   ↓
4. bip32::XPrv::derive_from_path(&seed, "m")  -- master xprv
   ↓
5. master.derive_path("m/44'/195'/0'/0/0")  -- first TRX receive xprv (SLIP-44 coin type 195)
   ↓
6. sk_bytes = child.to_secp256k1_secret_key(&secp).secret_bytes()  -- 32 bytes
   ↓
7. signing_key = k256::SigningKey::from_bytes(&sk_bytes)
   ↓
8. pubkey_uncompressed = signing_key.verifying_key().to_sec1_bytes()  -- 65 bytes: 0x04 ‖ X ‖ Y
   ↓
9. addr_suffix = Keccak256(pubkey_uncompressed[1..65])[12..32]  -- last 20 bytes
   ↓
10. addr_raw = [0x41, addr_suffix]  -- 21-byte mainnet/Shasta form; Nile uses 0xa0 prefix
    ↓
11. addr_base58check = base58check_encode(addr_raw)  -- 34 chars, starts with T
    ↓
12. Store m as plaintext (v0.1) or encrypt with Argon2id → AES-256-GCM (v0.2+) on disk
    ↓
13. At send time (TRX transfer):
    - ref_block = wallet/getnowblock → ref_block_bytes, ref_block_hash
    - raw_data = Transaction.raw_data {
        contract: [Contract { type: TransferContract, parameter: { owner_address: sender_21, to_address: recipient_21, amount: sun } }],
        ref_block_bytes, ref_block_hash,
        expiration: ref_block_ts + 60_000,
        timestamp: now_ms,
        fee_limit: 0,           // TRX transfer uses bandwidth, not energy
        data: b""               // optional memo
      }
    - raw_bytes = prost::Message::encode_to_vec(&raw_data)
    - tx_hash = SHA-256(raw_bytes)
    - sig_64 = signing_key.sign_prehash(tx_hash)?.to_bytes()
    - (v, sig_65) = recover_v(tx_hash, sig_64, sender_pubkey)  // try v=0 and v=1
    - signed_tx = Transaction { raw_data, signature: [sig_65] }
    - broadcast → POST /wallet/broadcasttransaction { ...signed_tx, visible: true }
    - txid = response.txID
   ↓
14. At send time (TRC-20 transfer):
    - calldata = encode_transfer(to_tvm_bytes, amount_in_base_units)
    - raw_data.contract = [Contract { type: TriggerSmartContract, parameter: {
        owner_address: sender_21,
        contract_address: token_contract_21,
        data: calldata,
        call_value: 0,
        call_token_value: 0,
        token_id: 0
      }}]
    - raw_data.fee_limit = estimated_energy × energy_price_in_sun  // V5 spike verifies
    - sign + broadcast as in step 13
```

**Parallel to Bitcoin and Ethereum flows:** one BIP-39 mnemonic, one BIP-32 derivation, one secp256k1 keypair. Only the derivation path coin type (195), address encoding (base58check vs bech32 vs EIP-55 hex), transaction envelope (protobuf vs PSBT vs RLP), and signature hash (SHA-256 of protobuf raw_data vs SHA-256d of tx vs keccak256 of RLP) differ.

## Resource model — verified 2026 numbers (Q5 resolved)

> Added 2026-08-27. Prior doc deferred Q5 to the spike. Numbers below are cited against developer hub, java-tron docs, and four wallet vendors; live `wallet/triggerconstantcontract` output for the chosen Spike V5 should still be re-pulled before publishing a hard-coded fee estimate.

### Resource economics (mainnet, 2026-08-27)

| Resource | Acquisition | Cost per unit | Default daily free | Notes |
|---|---|---|---|---|
| **Bandwidth** | Stake 2.0 OR TRX burn | `tx_size_bytes × bandwidth_rate (1)`; TRX burn at **1,000 sun/byte** | **600 / day** (chain parameter #61) | OKX FAQ cites 5,000/day — that figure is **stale**, use 600. |
| **Energy** | Stake 2.0 OR TRX burn | **100 sun / Energy** (chain parameter `getEnergyFee`, default — re-query `wallet/getchainparameters` before sizing `fee_limit`) | none | DEM penalty can scale actual cost up to `max_factor = 3.4` per 6-hour cycle. |
| **TRON Power (TP)** | Stake 2.0 freeze | 1 TP per 1 TRX staked | n/a | TP itself is non-transferable and doesn't burn — only used for voting / governance. |
| **Stake 2.0 launch** | 2023-04-07 via proposal #84 / TIP-467 | n/a | n/a | Stake 1.0 legacy still reclaimable via separate unstaking API. Each stake picks **either** Energy **or** Bandwidth (not both). Unstake pending = 14 days. Concurrent unstake-op cap = 32. |

### USDT-TRC20 `transfer` cost (verified against multiple sources, 2026-08-27)

| Recipient state | Energy used | Bandwidth used | Notes |
|---|---|---|---|
| Recipient already holds USDT | **~65,000** | ~345 | Baseline — `tr.energy`, `finassets.io`, `eco.com`, `tronxenergy.com` all converge on this range. |
| Recipient has no USDT (empty) | **~130,300** | ~345 | "First-touch" cost — contract init logic runs. |

Both figures can be **scaled up by DEM** (Dynamic Energy Model) per-contract factor `energy_factor ∈ [1.0, max_factor=3.4]`. Mainnet params: `threshold = 5,000,000,000`, `increase_factor = 0.2`. The `getcontractinfo` API returns `energy_factor` (scaled ×10,000) for any contract. Same transfer can cost 65k off-peak vs 90k+ hot on Mainnet. **Wallet should size `fee_limit` with `max_factor` buffer or re-estimate just-in-time before broadcast.**

### Estimation API call pattern (verified)

```text
# 1. Discover current energy price (always re-query — governance can change it)
POST wallet/getchainparameters     -> find "getEnergyFee" param  (sun per Energy)
# or:
POST wallet/getenergyprices        -> historical "prices" string (timestamp:sun,…)

# 2. Estimate resource consumption (primary path)
POST wallet/triggerconstantcontract {
    owner_address:    <T-base58check sender>,
    contract_address: <T-base58check token contract>,
    function_selector:"transfer(address,uint256)",
    parameter:        <ABI-encoded to (32) + value (32)>,
    visible:          true
}
  -> response.energy_used     (required)
  -> response.energy_penalty  (optional, present in many node versions, undocumented in OpenAPI schema)

# 3. Fallback for edge-case contracts (introduced in java-tron 4.7.0.1)
POST wallet/estimateenergy    { …same body… }
  -> response.energy_required

# 4. Build + sign + broadcast
POST wallet/triggersmartcontract {
    …same params + fee_limit (in SUN, see below)…
}

# 5. (optional) DEM-adjusted actual cost on-chain
POST wallet/getcontractinfo   { value: <contract_address>, visible: true }
  -> response.energy_factor (scaled ×10,000; divide by 10,000 for the multiplier)
```

**Use `triggerconstantcontract` as primary** (works on every node, requires `vm.supportConstant`). Use `estimateenergy` only for the small set of edge-case contracts where it returns closer estimates; it requires both `vm.estimateEnergy` AND `vm.supportConstant` enabled and may be off on public RPCs. `estimateenergy` returns `energy_required` (not `energy_used`); conversion `fee_limit_sun = energy_required × sunPerEnergy`.

### `fee_limit` semantics — SUN, not TRX (footgun flagged)

`fee_limit` is denominated in **sun** (10⁻⁶ TRX), not Energy. Hard upper bound on Mainnet: **15,000 TRX = 15,000,000,000 sun** (`getMaxFeeLimit` chain parameter #47, can change).

```rust
// CORRECT — sun units
let fee_limit_sun: i64 = (energy_estimate as i64) * (sun_per_energy as i64) * 11 / 10;  // +10% DEM buffer
raw_data.fee_limit = fee_limit_sun;

// WRONG — looks like 100 TRX but is actually 0.0001 TRX, transaction fails immediately with OUT_OF_ENERGY
raw_data.fee_limit = 100;
```

### Wallet UX patterns (verified against 4 wallet vendors)

| Wallet | Fee display pattern | Citation |
|---|---|---|
| **TronLink** | "Energy Required" + estimated TRX fallback | `tronagg.ai/blog/check-tron-energy-balance`, `tronnrg.com` |
| **Trust Wallet** | "Discount Percentage" + optional "Tronify" auto-rent savings | `trustwallet.com/blog/company/stake-trx-earn-energy-or-bandwidth-points` |
| **OKX Wallet** | Single merged "estimated fee" in TRX (also quotes "280 sun per unit when burned" for energy) | `okx.com/en-gb/help/gas-fees-faq` |
| **MetaMask (TRON)** | Per-resource breakdown (bandwidth / energy / TRX) + zero-fee display if resources sufficient | `support.metamask.io/configure/networks/tron/` |

**Recommended pattern for our wallet (synthesis):** display the **per-resource breakdown** (bandwidth / energy / TRX-equivalent) so users see *why* a USDT transfer is cheaper than a TRX transfer. Add a "discount %" line that shows how much is covered by staked resources vs TRX burn. Reference: MetaMask TRON pattern.

### Safety nets to surface in UX

- **Failed transactions do NOT refund** consumed Energy or burned TRX. A revert deducts only the energy for instructions executed up to that point; a crash/timeout deducts ALL available Energy for the tx as a penalty.
- **USDT-TRC20 deployer** sets `consume_user_resource_percent = 100` → users always pay 100% of Energy cost. Other tokens may subsidise (deployer can set 0–100).
- **Minimum Stake 2.0 amount:** 1 TRX (1,000,000 sun) per `FreezeBalanceV2Contract`. Smaller amounts rejected with `ContractValidateException`.

### Q5 — RESOLVED here (was deferred to spike in prior doc)

Prior doc deferred Q5 to "Spike V5 verifies fee-display format." Agent A research on 2026-08-27 produced all the numbers, API shapes, and UX references above. **Spike V5 now only needs to (a) re-pull live `triggerconstantcontract` output for our exact MockTRC20 contract, (b) confirm DEM `energy_factor` round-trip, (c) implement the chosen UX pattern.** The architectural decisions (display in resource units vs TRX, fee_limit buffer strategy, DEM re-estimation cadence) are settled here.

## Network + TLS pinning research (mirrors eth design)

### Local node — TronBox / wallet-cli

For CI + local development, TronBox is the closest analogue to Anvil. **Repo has moved** from `trufflesuite/tronbox` to **`tronprotocol/tronbox`** (220 stars, 130 forks) — corrected 2026-08-27. Active monthly release cadence through 2026 (v4.10.0 on 2026-08-13, adds Solidity 0.8.27–0.8.29 with default 0.8.29, upgrades `tronweb` 6.4.0). **v4.5.0 was a BREAKING release** — dropped `web3` v4 for `ethers` v6, requires Node ≥20. Any docs that pin an older TronBox version need updating.

Decision matrix:

| Option | Pros | Cons | Decision |
|---|---|---|---|
| **TronBox** + `wallet-cli` regtest | Solidity-native testing. Compatible with existing Tron contracts. | Requires Node ≥20 + npm install. v4.5.0+ uses ethers v6 (not web3). | **Pick for v0.1 spike** — TRC-20 MockTRC20 deploy. |
| `java-tron` `wallet-cli` regtest | Official, stable. | Java dep (~3 GB). Heavy for CI. | Defer to v0.3+ if TronBox inadequate. |
| `tronprotocol/tron` Docker image | Official. | ~1.5 GB Docker image, slow startup. | Reject for spike (CI cost). |
| `geth`-style Ethereum dev tooling | n/a — TRON not EVM-at-consensus. | — | Reject. |
| OpenZeppelin `tron-upgrades` Hardhat plugin | Standard upgrade patterns. | Requires Hardhat setup. | Reference only if v0.2+ ships contract deployment. |
| `walletsuite/walletsuite-tx-compiler` | Deterministic unsigned-tx compilation across EVM + Tron. | 2 stars, niche. | Reference only — not a runtime dep. |

**v0.1 spike uses TronBox 4.10.0+** for `MockTRC20` deployment + RPC testing. Mirrors eth `alloy-node-bindings` Anvil precedent.

### Testnet — Nile (Q6 resolution)

Nile (`nileex.io`) is the only testnet pick for v0.1. Justification:

- **Ecosystem share:** default testnet for TronLink, TronScan Nile, TronGrid Nile (`https://nile.trongrid.io`). Community-maintained, stable since 2021.
- **Tooling:** Nile faucet (TronFAQBot: `!nile ADDR` → 5,000 nile TRX), Nile TronScan explorer (`https://nile.tronscan.org`), TronGrid Nile API. All live and documented at `nileex.io` (status page confirms `GreatVoyage-Nile-v4.8.2`).
- **Address prefix:** **`0x41` (NOT `0xa0`)** — corrected 2026-08-27 against `developers.tron.network/docs/encoding`. `0x41` is universal across mainnet, Shasta, and Nile. The wallet does NOT need a configurable prefix byte — same code path for all three networks.
- **Chain-id:** `0xcd8690dc` (3448148188 decimal). The prior doc quoted `0x94a9059e` which is actually **Shasta's** chain-id — corrected here. Use the `eth_chainId` JSON-RPC method (TronGrid's `/jsonrpc` endpoint) to query; `wallet/getchainid` returns HTTP 405 on TronGrid's HTTP front.

**Rejected:** Shasta (`https://api.shasta.trongrid.io`) — still actively maintained (GreatVoyage-v4.8.1 on 2026-03-18, full release notes at `shasta.tronex.io`) but less documentation, lower faucet reliability, and mainnet-shape prefix `0x41` complicates local dev (same address collision surface as mainnet). Keep Shasta as a v0.2+ fallback if Nile degrades.

### SPKI pinning (Q7) — reuse `SpkiPinnedVerifier`

**Two scenarios, two paths.** Mirrors eth design exactly.

#### Scenario A: Pin the RPC endpoint (defense against MITM)

Wallet runs `tron` from a hostile network. Wants assurance the JSON-RPC responses come from the real TronGrid server, not a TLS-terminating proxy. Solution: SPKI pin.

**Design:**

1. CLI URL scheme extension: `pinned://<spki-sha256-hex>@host[:port]`. Same scheme as Bitcoin (`bitcoin-wallet-core/src/chain/spki.rs`) and eth.
2. Library function: `pub fn new_http_pinned(url: &str, spki: &[u8; 32]) -> Result<reqwest::Client, Error>`. Implementation: build raw `reqwest::Client` with a custom `rustls::ServerCertVerifier` from `bitcoin-wallet-core`.
3. **Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier`** directly — same `rustls` version, same pin format. Single import, zero new code.

**Test vectors:**

- Wrong pin against `api.trongrid.io` → `Error::SpkiPinMismatch`.
- Correct pin against `api.trongrid.io` → request succeeds.
- Pin against self-signed TronBox HTTP server (no TLS in dev) → pin ignored, plain HTTP allowed (localhost chain-id guard catches mis-config).

**Tradeoff:** pin rotation = operator pain. Mitigation: support comma-separated pin list (`pinned://<pin1>,<pin2>@host`) for rotation windows. Out of scope for v0.3.x.

#### Scenario B: No pin (system trust store + localhost)

Same as eth Scenario B. Plain `reqwest::Client::new()` + system CAs. Acceptable for localhost dev / trusted-network deployments.

### Decision matrix — which scenario when

| Use case | Network | Pin? | Why |
|---|---|---|---|
| Local dev (developer laptop) | TronBox HTTP | No (Scenario B) | TLS N/A. |
| CI smoke test | TronBox HTTP | No (Scenario B) | Ephemeral. |
| Testnet smoke (Nile) | Nile HTTPS | Optional (Scenario A recommended) | Public WiFi in dev environments. |
| Testnet smoke (dev machine, LAN) | Nile HTTPS | No (Scenario B acceptable) | Trusted network. |
| Production wallet, real value | Mainnet HTTPS | **Yes (Scenario A required)** | Adversarial network. |

Default CLI behavior: **Scenario B** (no pin). Operator opts into Scenario A via `pinned://` URL scheme.

## Alternatives considered (and why rejected)

Already covered in §"Why raw reqwest + prost and not a TRON SDK" and §"Stablecoin (TRC-20) transfer — contract addresses + ABI". Consolidated for the at-a-glance view:

| Alternative | Why rejected |
|---|---|
| `rust-tron` (andelf) | 5-year-stale. Reference impl cited in docs only. |
| `tronic` (39george) | 7 stars, single maintainer. Re-evaluate at v0.3. |
| `tronz` (throgxyz) | No releases, no engagement. |
| `tron-rs` | Proto defs only — no signing, no JSON-RPC. Use as `prost-build` schema source. |
| `ethers-rs` against TRON | TRON is not EVM-at-consensus; transaction format mismatch. |
| `web3` (Parity) against TRON | Same — transaction format mismatch. |
| `alloy-sol-types` for TRC-20 ABI | Hand-rolled encoder is ~30 lines, avoids alloy dep weight. Re-evaluate at v0.3. |
| Hand-rolled protobuf encoder | `prost` is the standard. Hand-rolling invites subtle wire-format bugs. |
| Hand-rolled Keccak-256 | `tiny-keccak` is the standard. Hand-rolling is a security liability. |
| L2 / sidechains (BitTorrent Chain, etc.) | Out of scope per session scoping. |
| TRC-10 token transfers | Out of scope per issue body (deferred). TRC-10 has separate `TransferAssetContract` proto encoding. |
| Stake / unstake / freeze resource delegation | Out of scope per issue body. `FreezeBalanceV2Contract` proto encoding separate. |
| Hardware wallet (Ledger, Trezor) | Out of scope per issue body. Same deferral as eth #293. |
| TronBox V8.x (Solidity 0.8) for spike | **Pick** for spike (current line). |
| Smart contract deployment via wallet | Out of scope. Sign-only + broadcast external path is enough. |
| TRON DEX integration (SunSwap) | Out of scope. |

## Open questions — resolved in this doc vs deferred to spike

### Resolved here

- **Q1 — SDK choice.** **Raw reqwest + prost.** See §"Why raw reqwest + prost". Reject `rust-tron`, `tronic`, `tronz`, `tron-rs`, `anychain` for v0.1. Rationale: maintenance risk floor + Bitcoin/eth precedent. Updated 2026-08-27: `tronic` v0.6.1 confirmed gRPC-only (JSON-RPC WIP), `tronz` now at 55 stars active, `anychain` at 253 stars but multi-chain scope. **Decision unchanged** — still reject for v0.1; re-evaluate at v0.3 once any single-maintainer project ships stable JSON-RPC + ≥2 maintainers.
- **Q2 — Transaction format.** **Protobuf via `prost` 0.14.4 + `prost-build` 0.14.4** (corrected from 0.13 — verified 2026-08-27 against `crates.io/crates/prost`), schema from `core/Tron.proto` (`tronprotocol/java-tron` repo, recommended pinned SHA `851575d` 2026-07-14). Build script compiles proto to Rust types. Signing: SHA-256 of protobuf-serialized `raw_data` → 65-byte ECDSA signature (r‖s‖v, recovery byte `v ∈ {0, 1}` — **NOT Ethereum's `v+27 ∈ {27, 28}`**). See §"`prost` 0.14.4 + `prost-build` 0.14.4" and §"Mnemonic-to-broadcast data flow".
- **Q3 — TRC-20 transfer.** **Hand-rolled ABI encoder for `transfer(address,uint256)`, `balanceOf(address)`, `decimals()`.** ~30 lines, no new deps. TRC-20 ABI = ERC-20 ABI at the wire level; selectors are identical (`0xa9059cbb` for `transfer`). See §"Stablecoin (TRC-20) transfer — contract addresses + ABI". **Spike V3 should round-trip against `alloy-sol-types` standalone** (4 deps only — confirmed 2026-08-27) as a reference impl; `tronic` already uses this pattern in production.
- **Q4 — Address encoding.** **T-base58check for display + storage, hex/TVM form (`0x... + 20 bytes` or bare 20-byte) for internal API calls only.** Boundary: user-facing output uses T-base58check; inter-contract call args (`TransferContract.owner_address`, `TriggerSmartContract.contract_address`) use the 21-byte raw form (prefix + last 20 bytes of `keccak256(pubkey)`). Encoding helper: `bs58` 0.5 + 4-byte double-SHA-256 checksum. Prefix byte is **`0x41` for all three networks** (mainnet, Shasta, Nile) — corrected 2026-08-27 (prior doc said `0xa0` for Nile, which is a legacy `net.type=testnet` flag never adopted). See §"`bs58` 0.5".

### Deferred to spike (V1–V10)

- **Q5 — Resource model UX.** **RESOLVED here** — see §"Resource model — verified 2026 numbers (Q5 resolved)". Spike V5 only needs to (a) re-pull live `triggerconstantcontract` output for our exact MockTRC20, (b) confirm DEM `energy_factor` round-trip, (c) implement the chosen per-resource-breakdown UX pattern (MetaMask-TRON style). All architectural decisions settled.
- **Q6 — Testnet choice.** **Nile for v0.1.** See §"Testnet — Nile". Updated 2026-08-27: chain-id `0xcd8690dc` (NOT `0x94a9059e` — that was Shasta), address prefix `0x41` (NOT `0xa0`). Use `eth_chainId` JSON-RPC method (TronGrid `/jsonrpc`); `wallet/getchainid` returns HTTP 405.
- **Q7 — RPC pinning.** **Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier`** verbatim. File path verified 2026-08-27 at `bitcoin-wallet-core/src/chain/spki.rs`. See §"SPKI pinning (Q7)".
- **Q8 — Sign-only path.** Spike V8 verifies local-signing-only path (no broadcast), matches eth Task 3. **MUST verify `r‖s‖v` byte order with `v ∈ {0, 1}` (NOT Ethereum `v+27` convention).**
- **Q9 — Token registry.** Spike V9 produces a `tokens/mainnet.json` + `tokens/nile.json` mirroring eth Task 8. **MUST verify `energy_penalty` from `triggerconstantcontract`** (real but undocumented in OpenAPI schema; needed for TRX-equivalent fee display).
- **Q10 — Mnemonic reuse + SLIP-44 vector.** Spike V10 verifies `m/44'/195'/0'/0/0` against a canonical test vector. Address must start with `T` using prefix `0x41` (same for all three networks).

## Network + address prefix summary

| Network | Address prefix byte | Chain-id (hex / decimal) | Example (T-base58check) | RPC endpoint |
|---|---|---|---|---|
| Mainnet | `0x41` | `0x2b6653dc` / 728126428 | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` (USDT-TRC20) | `https://api.trongrid.io/wallet/*` |
| Shasta | `0x41` | `0x94a9059e` / 2494104990 | (same format as mainnet, separate chain-id) | `https://api.shasta.trongrid.io/wallet/*` |
| Nile (testnet) | **`0x41`** (corrected 2026-08-27; was `0xa0` in prior doc) | **`0xcd8690dc` / 3448148188** (corrected 2026-08-27) | `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` (community test USDT) | `https://nile.trongrid.io/wallet/*` |
| Local regtest | `0x41` | n/a | n/a | `http://127.0.0.1:8090/wallet/*` |

**Chain-id query method (corrected 2026-08-27):** `wallet/getchainid` returns HTTP 405 on TronGrid's HTTP front. Use `POST /jsonrpc {"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}` instead — works on all three networks and returns the chain-id as a hex string. All three chain-ids verified live 2026-08-27.

## Verification (spike V1–V10, one per Q)

No implementation work in this session. The next-session spike (`rust-wallet-app/spikes/tron-v1/`) must validate:

1. **V1 (Q1 SDK choice):** `cargo add prost@0.14 prost-types@0.14 bs58@0.5 tiny-keccak@2.0.2` compiles against the existing workspace. Confirms zero dependency conflict. (Corrected 2026-08-27 — prior doc said `prost@0.13`; current line is `0.14.4`.)
2. **V2 (Q2 protobuf tx):** `prost-build` compiles `core/Tron.proto` (recommended pinned SHA: `851575d` from `develop` branch, 2026-07-14) into Rust types. Generated `Transaction::encode_to_vec()` round-trips with `Transaction::decode()` for a hand-crafted `TransferContract`. **MUST also include a hand-crafted `TriggerSmartContract` with `data` at field 4 (NOT 3)** — the off-by-one hazard that silently produces invalid transactions.
3. **V3 (Q3 TRC-20 ABI):** Hand-rolled `encode_transfer(to_20_bytes, value_u256)` produces 68-byte calldata with selector `0xa9059cbb` at bytes 0..4. Round-trips against `alloy-sol-types` reference impl (the `sol!` macro + `transferCall { to, value }.abi_encode()` produces identical bytes; confirmed standalone dep surface per Agent B finding).
4. **V4 (Q4 base58check):** `base58check_encode(0x41 ++ last_20_bytes_of_keccak256(pubkey))` produces a 34-char string starting with `T`. `base58check_decode(s).is_ok()` and returns original 21 bytes. (Markdown lint: avoid `[12..32]` notation outside code blocks — use `last 20 bytes` prose form.)
5. **V5 (Q5 resource model):** Against TronBox regtest + MockTRC20, `wallet/triggerconstantcontract` returns `energy_used` in the **65,000–130,000** range for a USDT-TRC20 `transfer` (lower bound when recipient holds USDT, upper bound for empty recipient). Verify `wallet/getbandwidth` returns ≥0 free per account (600/day per chain parameter #61). **Use `triggerconstantcontract` as the primary simulation endpoint, fall back to `estimateenergy` for edge cases** (per Agent A finding — `estimateenergy` introduced in java-tron 4.7.0.1, may be disabled on public RPCs).
6. **V6 (Q6 testnet):** `POST https://nile.trongrid.io/jsonrpc {"method":"eth_chainId"}` returns `"result": "0xcd8690dc"`. **MUST NOT use `wallet/getchainid`** (HTTP 405 on TronGrid). Address generation against Nile prefix **`0x41`** (NOT `0xa0`) produces a valid T-base58check string matching `nile.tronscan.org` lookup. Verify TAPOS reference via `walletsolidity/getnowblock` (NOT `wallet/getnowblock`) for finality.
7. **V7 (Q7 SPKI pin):** `SpkiPinnedVerifier` (imported from `bitcoin-wallet-core::chain::spki`, file path verified 2026-08-27) accepts a request to `pinned://<correct_pin>@api.trongrid.io` and rejects a wrong pin.
8. **V8 (Q8 sign-only):** Local-sign a TRX transfer (no `wallet/broadcasttransaction` call). Verify `txID = SHA256(raw_data_hex)` matches what the network computes. **MUST verify signature format is `r‖s‖v` with `v ∈ {0, 1}` (NOT Ethereum's `v+27 ∈ {27, 28}`)** — Ethereum-default signers produce invalid TRON signatures.
9. **V9 (Q9 token registry):** `tokens/mainnet.json` with 5 entries (USDT, USDC, TUSD, USDD, stUSDT) + `tokens/nile.json` with 1 entry (community test USDT). USDT decimals = 6 verified via `wallet/triggersmartcontract` call to `decimals()` selector. **Also verify `triggerconstantcontract` returns `energy_penalty`** field for fee-display UX (real but undocumented in OpenAPI schema; useful for "TRX equivalent" cost display).
10. **V10 (Q10 SLIP-44 vector):** `bip39::Mnemonic::parse_in(English, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")` → seed → `m/44'/195'/0'/0/0` → address matches canonical reference (TronWeb or `andelf/rust-tron` test vector). Address must start with `T` and use prefix `0x41` regardless of network (mainnet/Shasta/Nile).

If all 10 pass, the chosen crate surface is confirmed for v0.1.

## Sources

### TRON protocol + transaction format

- TRON Developer Hub — Transactions: <https://developers.tron.network/docs/tron-protocol-transaction>
- TRON Developer Hub — Encoding addresses and data: <https://developers.tron.network/docs/encoding>
- TRON Developer Hub — Contract types: <https://developers.tron.network/docs/tron-contracttype>
- `tronprotocol` repo — `core/Tron.proto` (Buf mirror): <https://buf.build/streamingfast/tron-protocol/file/84fc05905d3a49318eaafd7e63e2e5e4:core/Tron.proto>
- `tronprotocol/java-tron` — Tron protobuf protocol document: <https://github.com/tronprotocol/java-tron/blob/develop/Tron%20protobuf%20protocol%20document.md>
- `tronprotocol/java-tron` — HTTP API docs: <https://tronprotocol.github.io/documentation-en/api/http/>
- Andrew Koidan — TRON transaction prices: <https://blog.akoidan.com/posts/tron-transaction-prices/> (wire-format walkthrough)
- arXiv — Decoding TRON (large-scale extraction): <https://arxiv.org/html/2509.16292v1>

### Rust SDK candidates

- `39george/tronic` (Alloy-inspired): <https://github.com/39george/tronic> (7 stars, last commit 2026-07-20, Apache-2.0/MIT)
- `andelf/rust-tron` (gRPC + CLI): <https://github.com/andelf/rust-tron> (50 stars, last commit 2021-03-06)
- `andelf/rust-tron/keys/src/address.rs` (canonical base58check impl): <https://github.com/andelf/rust-tron/blob/master/keys/src/address.rs>
- `tron-rs` (crates.io): <https://crates.io/crates/tron-rs> (proto + gRPC defs, cosmrs-based)
- `throgxyz/tronz`: <https://github.com/throgxyz/tronz>
- r/rust announcement of `tronic`: <https://www.reddit.com/r/rust/comments/1marc3n/announcing_tronic_a_rust_toolkit_for_tron/>
- GitHub topics — tron: <https://github.com/topics/tron?l>

### Testnet

- Nile testnet portal: <https://nileex.io/> (community, stable since 2021)
- Nile status page: <https://nileex.io/status/getStatusPage>
- Nile TronScan: <https://nile.tronscan.org/>

### Stablecoins (TRC-20)

- TronScan — TRC-20 token tracker: <https://tronscan.io/tokens/list>
- USDT-TRC20 contract: <https://tronscan.org/contract/TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t/code>
- USDC (deprecated TRC-20): <https://tronscan.io/token20/TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8>
- TrueUSD TRC-20: <https://tronscan.io/token20/TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9>
- USDD TRC-20: <https://tronscan.io/token20/TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz>
- Crypto APIs — TRC-20 explained: <https://cryptoapis.io/layers/trc-20>

### TronGrid (RPC endpoint)

- TronGrid home: <https://www.trongrid.io/>
- `TRON-PRO-API-KEY` header documentation: TronGrid dashboard

### Rust crates

- `prost`: <https://crates.io/crates/prost> (v0.13, Apache-2.0, tokio-rs maintainership)
- `prost-types`: <https://crates.io/crates/prost-types> (v0.13, Apache-2.0)
- `prost-build`: <https://crates.io/crates/prost-build> (build-time codegen)
- `bs58`: <https://crates.io/crates/bs58> (v0.5, MIT, used by rust-bitcoin + solana-sdk)
- `tiny-keccak`: <https://crates.io/crates/tiny-keccak> (v2.0.2, CC0/Apache-2.0)
- `k256`: <https://crates.io/crates/k256> (already workspace dep from Bitcoin)
- `sha2`: <https://crates.io/crates/sha2> (already workspace dep)
- `bip32`: <https://crates.io/crates/bip32> (already workspace dep from Bitcoin)
- `bip39`: <https://crates.io/crates/bip39> (already workspace dep from Bitcoin)
- `reqwest`: <https://crates.io/crates/reqwest> (already workspace dep)
- `rustls`: <https://crates.io/crates/rustls> (already workspace dep)

### Standards + cross-references

- SLIP-0044 coin types (TRON = 195): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- `bip_utils::slip44::Coin::Tron`: <https://docs.rs/slip44/latest/slip44/enum.Coin.html>
- BIP-32 HD derivation: <https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki>
- BIP-39 mnemonic wordlist: <https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki>
- EIP-20 (ERC-20 ABI, identical to TRC-20): <https://eips.ethereum.org/EIPS/eip-20>
- TRON = TVM (TRON Virtual Machine) docs: <https://developers.tron.network/docs/tvm-overview>

### Bitcoin + Ethereum deep-dive (cross-references)

- Bitcoin deep-dive (SPKI pin pattern source): `docs/wallets/2026-08-05-bitcoin-rust-sdks-deep-dive.md`
- Ethereum deep-dive (companion): `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`
- Bitcoin `SpkiPinnedVerifier` source: `bitcoin-wallet-core/src/chain/spki.rs`

## Appendix: Async test functions (cross-reference)

Per eth-wallet-core async test policy (issue #333), every test in any future `tron-wallet-core/` crate that touches async code MUST be declared `async fn` + `#[tokio::test]`. See `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` §"Appendix: Async test function priority" for the canonical pattern and anti-patterns.

The TRON wallet will follow the same policy:

- `reqwest` async HTTP → `async fn` + `#[tokio::test]`.
- `prost::Message::encode`/`decode` → sync (no `async fn` needed).
- `k256::SigningKey::sign_prehash` → sync.
- `bip32`/`bip39` derivation → sync.
- `bs58` encode/decode → sync.

Only RPC client tests + signing-broadcast integration tests need async. Unit tests for proto/encoding/base58 stay sync.

---

**Next steps (post-deep-dive):**

1. **Ticket B:** User-stories doc — `docs/wallets/2026-08-27-tron-wallet-user-stories.md` (template: `docs/wallets/2026-08-23-eth-wallet-user-stories.md`).
2. **Ticket C:** Spike — `rust-wallet-app/spikes/tron-v1/` with V1–V10 mapped to Q1–Q10, each PASS evidence (command output + SHA).
3. **Ticket D:** Plan — `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` derived from resolved Qs + verified spikes.
4. **Ticket E:** PR + flip issue #399 checkboxes.

## Sources added 2026-08-27 (verification round)

### Q5 — Resource model

- Developer Hub — FeeLimit & Energy cost: <https://developers.tron.network/docs/set-feelimit>
- Developer Hub — Resource model: <https://developers.tron.network/docs/resource-model>
- Developer Hub — Stake 2.0: <https://developers.tron.network/docs/staking-on-tron-network>
- Developer Hub — `estimateenergy` reference: <https://developers.tron.network/reference/estimateenergy>
- java-tron Resource Model: <https://tronprotocol.github.io/documentation-en/mechanism-algorithm/resource/>
- java-tron Dynamic Energy Model section: <https://tronprotocol.github.io/documentation-en/mechanism-algorithm/resource/> (Dynamic Energy Model)
- tr.energy Energy Calculator: <https://tr.energy/en/tron-energy-calculator/>
- finassets.io TRON energy calculator: <https://www.finassets.io/en/tron-energy-calculator/>
- eco.com USDT-TRC20 fees 2026: <https://eco.com/support/en/articles/15197974-usdt-trc-20-fees-2026-per-transfer-cost-on-every-exchange>
- tronxenergy.com TRC-20 fee explainer: <https://tronxenergy.com/blog/understanding-trc20-fees-how-much-does-it-cost-to-send-usdt-on-the-tron-network>
- TronPoolEnergy dictionary: <https://tronpoolenergy.com/tron-crypto-dictionary/>
- ChainScore Labs Dynamic Energy Model: <https://chainscorelabs.com/protocol/tron/governance-and-parameter-changes/dynamic-energy-model-and-parameter-adjustments>
- Chainstack `getenergyprices`: <https://docs.chainstack.com/reference/tron-getenergyprices>
- QuickNode `getenergyprices`: <https://www.quicknode.com/docs/tron/wallet-getenergyprices>
- TronWeb `getEnergyPrices` reference: <https://tronweb.network/docu/docs/API%20List/trx/getEnergyPrices>
- HuaweiCloud TRON API table: <https://support.huaweicloud.com/intl/en-us/devg-nes/nes_devg_0129.html>
- TIP-511 (Decrease energy unit price proposal): <https://github.com/tronprotocol/tips/issues/511>
- TronLink (TRON Energy Required UX): <https://tronagg.ai/blog/check-tron-energy-balance> · <https://tronnrg.com/en/blog/trust-wallet-tron-energy/>
- Trust Wallet Stake + Discount % UX: <https://trustwallet.com/blog/company/stake-trx-earn-energy-or-bandwidth-points> · <https://trustwallet.com/blog/company/smarter-tron-transfers>
- OKX Wallet Gas fees FAQ: <https://www.okx.com/en-gb/help/gas-fees-faq>
- MetaMask TRON support: <https://support.metamask.io/configure/networks/tron/>
- TRON DAO Stake 2.0 announcement: <https://forum.trondao.org/t/tron-stake-2-0-launches-today/17101>
- CoolWallet Stake 2.0 guide: <https://www.coolwallet.io/blogs/blog/tron-trx-stake-2-0-guide-coolwallet-adds-hardware-wallet-support>

### Q1 — Rust SDK landscape (2026 re-survey)

- `39george/tronic` GitHub: <https://github.com/39george/tronic> (v0.6.1, 7 stars, last push 2026-07-20, Apache-2.0/MIT, gRPC-only — JSON-RPC WIP)
- `tronic` crates.io: <https://crates.io/crates/tronic>
- `tronic` docs.rs: <https://docs.rs/tronic/>
- r/rust announcement of `tronic`: <https://www.reddit.com/r/rust/comments/1marc3n/announcing_tronic_a_rust_toolkit_for_tron/>
- `andelf/rust-tron` GitHub: <https://github.com/andelf/rust-tron> (50 stars, last push **2025-01-09**, LGPL-3.0, no tags)
- `throgxyz/tronz` GitHub: <https://github.com/throgxyz/tronz> (55 stars, created 2026-06-14, last push 2026-08-25, Apache-2.0, no tags)
- `0xcregis/anychain` GitHub: <https://github.com/0xcregis/anychain> (253 stars, multi-chain BTC/ETH/Tron/Solana)
- `anychain` gitbook: <https://cregisoffical.gitbook.io/anychain/>
- `edwintuan/next-wallet` GitHub (TUI + Stake 2.0 reference): <https://github.com/edwintuan/next-wallet>
- `Gingerbreadfork/tron-goblin-node` GitHub: <https://github.com/Gingerbreadfork/tron-goblin-node>
- `walletsuite/walletsuite-tx-compiler` GitHub: <https://github.com/walletsuite/walletsuite-tx-compiler>
- `Hixon10/usdt-wallet-rs` GitHub: <https://github.com/Hixon10/usdt-wallet-rs>
- `OpenSettle/opensettle-sdk-rust` GitHub: <https://github.com/OpenSettle/opensettle-sdk-rust>
- `rootdigit/sagapay-rust` GitHub: <https://github.com/rootdigit/sagapay-rust>
- `tron-rs` crates.io: <https://crates.io/crates/tron-rs> (v0.1.0, 2026-01-20, proto-only)
- `tron-rs` docs.rs: <https://docs.rs/tron-rs>
- `bip_utils::slip44::Coin::Tron` docs.rs: <https://docs.rs/bip_utils/latest/bip_utils/slip44/>
- SLIP-0044 (TRON coin type 195): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- Keccak team original spec: <https://keccak.team/keccak.html>
- NIST FIPS-202 SHA-3 standard: <https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.202.pdf>
- `prost` crates.io: <https://crates.io/crates/prost> (v0.14.4, 2026-06-07, MSRV 1.85)
- `prost-build` crates.io: <https://crates.io/crates/prost-build>
- `prost` GitHub: <https://github.com/tokio-rs/prost>
- `alloy-sol-types` docs.rs: <https://docs.rs/alloy-sol-types/> (standalone — only `alloy-primitives`, `alloy-sol-macro`, optional `alloy-json-abi` and `serde`)
- `alloy-sol-types` v1.4 docs: <https://docs.rs/alloy-sol-types/1.4.0/alloy_sol_types/>
- `bs58` crates.io: <https://crates.io/crates/bs58> (v0.5.1, no CVEs, MIT)
- `bs58-rs` CHANGELOG: <https://github.com/Nullus157/bs58-rs/blob/master/CHANGELOG.md>
- GitHub Advisory DB `bs58` query: <https://github.com/advisories?query=bs58>

### Q2/Q3 — Signing + ABI verification

- Developer Hub — Transaction signature validation: <https://developers.tron.network/docs/transaction-signature-validation>
- Developer Hub — API signature and broadcast flow: <https://developers.tron.network/docs/api-signature-and-broadcast-flow>
- Developer Hub — TRON contract type: <https://developers.tron.network/docs/tron-contracttype>
- Developer Hub — FAQ (raw_data_hex example): <https://developers.tron.network/docs/faq>
- Developer Hub — TRC-20 contract interaction: <https://developers.tron.network/docs/trc20-contract-interaction>
- Developer Hub — Block: <https://developers.tron.network/docs/block>
- Developer Hub — Account (address encoding): <https://developers.tron.network/docs/account>
- Developer Hub — Account (legacy): <https://tronprotocol.github.io/documentation-en/mechanism-algorithm/account/>
- Developer Hub — Eth chainId reference: <https://developers.tron.network/reference/eth_chainid>
- Developer Hub — `eth_chainId` (MCP): <https://developers.tron.network/docs/mcp>
- `tronprotocol/java-tron` — `Tron protobuf protocol document.md`: <https://github.com/tronprotocol/java-tron/blob/develop/Tron%20protobuf%20protocol%20document.md>
- `tronprotocol/java-tron` — `core/Tron.proto` develop branch: <https://github.com/tronprotocol/java-tron/blob/develop/protocol/src/main/protos/core/Tron.proto>
- `tronprotocol/java-tron` — file commit history: <https://github.com/tronprotocol/java-tron/commits/develop/protocol/src/main/protos/core/Tron.proto>
- 4byte.directory `transfer(address,uint256)` selector: <https://www.4byte.directory/signatures/?bytes4_signature=0xa9059cbb>
- Chainstack `estimateenergy`: <https://docs.chainstack.com/reference/tron-estimateenergy>
- Dwellir `getnowblock`: <https://www.dwellir.com/docs/tron/wallet-getnowblock>
- dynamic-docs (mintlify) — TRON reference: <https://dynamic-docs.mintlify.app/javascript/reference/tier-2-chains/tron>
- Stack Overflow on `0xa9059cbb`: <https://stackoverflow.com/questions/55258332/find-the-function-name-and-parameter-from-input-data>
- Andrew Koidan blog — TRON tx prices: <https://blog.akoidan.com/posts/tron-transaction-prices/>
- TronWeb issue 487 (energy_penalty field observed): <https://github.com/tronprotocol/tronweb/issues/487>

### Q6 — Network state (2026 verification)

- Nile status page: <https://nileex.io/status/getStatusPage>
- Nile portal: <https://nileex.io/>
- Shasta status page: <https://shasta.tronex.io/>
- TronFAQBot (Telegram) — `!nile ADDR` / `!shasta ADDR` faucet commands
- Developer Hub — Networks: <https://developers.tron.network/docs/networks>
- Developer Hub — Connect to TRON: <https://developers.tron.network/docs/connect-to-the-tron-network>
- Developer Hub — Getting testnet tokens: <https://developers.tron.network/docs/getting-testnet-tokens-on-tron>
- Developer Hub — Rate limits: <https://developers.tron.network/reference/rate-limits>
- Developer Hub — Networks (encoding, legacy `0xa0` context): <https://github.com/tronprotocol/wallet-cli/issues/459>
- TronGrid changelog v1.10.0 (QPS reduction): <https://www.trongrid.io/changeLog/v1-10-0>
- Chainstack TRON RPC providers 2026: <https://chainstack.com/best-tron-rpc-providers-2026/>
- TRON DAO forum — API key usage clarifications: <https://forum.trondao.org/t/clarifications-on-tron-api-key-usage/5868>
- rpc.info TRON Nile: <https://rpc.info/tron-nile>
- chainlist.org 3448148188 (Nile): <https://chainlist.org/chain/3448148188>
- chainid.network 728126428 (mainnet): <https://chainid.network/chain/728126428/>
- chainlist.org 728126428 (mainnet): <https://chainlist.org/chain/728126428>
- TronLink docs — networks reference: <https://docs.tronlink.org/reference/networks/>

### Local dev tooling (TronBox org move)

- TronBox GitHub (new home): <https://github.com/tronprotocol/tronbox>
- TronBox releases: <https://github.com/tronprotocol/tronbox/releases>
- TronBox site: <https://tronbox.io/>
- TronBox docs — vs Hardhat: <https://tronbox.io/docs/migration/tronbox-vs-hardhat>
- TronBox npm: <https://www.npmjs.com/package/tronbox>
- OpenZeppelin TRON upgrades: <https://github.com/OpenZeppelin/tron-upgrades>
- Developer Hub — TronBox: <https://developers.tron.network/docs/tronbox-1>

### SPKI pin reuse (verified path)

- `bitcoin-wallet-core/src/chain/spki.rs` — file path verified 2026-08-27 (exists at `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`).
- `bitcoin-wallet-core` Rust doc: `rust-wallet-app/target/doc/src/bitcoin_wallet_core/chain/spki.rs.html`
- eth-wallet-core SPKI pin localnet test: `rust-wallet-app/crates/eth-wallet-core/tests/spki_pin_localnet.rs`
