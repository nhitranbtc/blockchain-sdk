# tron-wallet-core (v0.1) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-KILLS: `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver `rust-wallet-app/crates/tron-wallet-core/` — a TRON (TRX + TRC-20 stablecoin) wallet library built on raw `reqwest` + `prost` 0.14.4, plus a `tron` CLI in the umbrella. Mirrors `bitcoin-wallet-core/` (v0.1) and `eth-wallet-core/` (v0.2) structure. Resolves the 10 open questions from `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md` and PR #402.

**Architecture:** Four phases.
- **Phase 0** = scaffold crate + canonical test (mnemonic → T-base58check address derivation).
- **Phase 1** = core wallet ops (create / import / list / delete / show / sign).
- **Phase 2** = RPC integration (raw `reqwest` JSON-RPC + protobuf tx construction + SPKI pin verifier).
- **Phase 3** = TRC-20 stablecoin transfer (USDT-TRC20 + USDC-TRC20) + bundled token registry + energy/bandwidth fee display.
- **Phase 4** = `tron` CLI + Nile/mainnet smoke + release cut.

**Tech Stack:** Rust 1.94 stable (workspace pin), `prost = "0.14.4"` + `prost-types = "0.14.4"` (Q2, NEW workspace deps), `bs58 = "0.5"` (Q4, NEW), `tiny-keccak = "2.0.2"` (Keccak-256, NEW), `k256` + `sha2` + `bip32 ^0.5` + `bip39 2.2` + `reqwest 0.12` + `rustls 0.23` (workspace, reused from Bitcoin/eth sides), `serde` + `serde_json` (workspace), `clap 4` (CLI). Build dep: `protoc ≥3.12` for `prost-build`. **Zero FFI — all pure Rust.**

## Rust SDKs, tools, and crates — full inventory

The TRON wallet builds on **15 Rust crates + 2 build tools + 1 cross-crate reuse** — all pure-Rust, zero FFI. Divided into 4 categories below. Every choice ties back to a Q resolution in §"Global Constraints" below; full citations in `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md`.

### A. Workspace-reused crates (already in `rust-wallet-app/Cargo.toml`, no dep-tree growth)

| Crate | Version | Role in TRON wallet | Why chosen |
|---|---|---|---|
| `k256` | latest | secp256k1 signing primitive — `SigningKey::sign_prehash(tx_hash)` + `VerifyingKey::recover_from_prehash` for `v` byte | Pure-Rust (no FFI); same signing primitive Bitcoin side uses; `k256` already in workspace via Bitcoin-side `bitcoin-wallet-core`. Adding again would double signing impls + risk version drift (mirror eth `alloy-signer-local` choice per Q1). |
| `sha2` | workspace | SHA-256 for two purposes — (1) `txID = SHA-256(protobuf-serialize(raw_data))` (Q2); (2) base58check 4-byte checksum = `SHA-256(SHA-256(payload))[0..4]` (Q4) | Canonical Rust SHA-256; already workspace dep. Avoid pulling `sha3` crate for tx hashing — SHA-3-256 ≠ Keccak-256 (different padding bytes 0x06 vs 0x01). |
| `bip32` | ^0.5 | HD derivation `m/44'/195'/0'/0/{idx}` (Q10) | Same `XPrv` + `DerivationPath` API Bitcoin uses; only coin type differs (195 = TRX vs 0 = BTC vs 60 = ETH). Reusing Bitcoin-side pattern keeps derivation path logic consistent across all 3 chains. |
| `bip39` | 2.2 | BIP-39 mnemonic generate + parse + to_seed (Q10) | Same wordlist as Bitcoin/eth. One BIP-39 implementation serves all 3 chains (seed is identical for same mnemonic; only the derivation path differs). Already workspace dep with `zeroize` + `rand` features. |
| `reqwest` | 0.12 | JSON-RPC HTTP transport to `https://api.trongrid.io/wallet/*`, `https://nile.trongrid.io/wallet/*`, `https://api.shasta.trongrid.io/wallet/*` (Q6) | Standard async HTTP client with `rustls-tls` feature. No TRON-specific client SDK available (all rejected per Q1). Raw reqwest + custom SPKI verifier mirrors Bitcoin F20 / eth Q2 pattern. |
| `rustls` | 0.23 | TLS termination + custom `ServerCertVerifier` for SPKI pinning (Q7) | Pinned to Bitcoin-side version (0.23) so `SpkiPinnedVerifier` from `bitcoin-wallet-core::chain::spki` plugs in directly with zero version drift. `webpki-roots` for system trust store. |
| `serde` | 1.x | Derive `Serialize`/`Deserialize` for `WalletConfig`, `Token`, `SignedTx`, `Network` | Standard Rust serialization. Already workspace dep via Bitcoin/eth sides. |
| `serde_json` | 1.x | Parse JSON-RPC envelopes (`{"jsonrpc":"2.0","method":"...","params":{...},"id":1}`) + TronGrid `/wallet/*` responses | Standard. Used in eth-side `alloy-rpc-types` flow, same parsing pattern. |
| `clap` | 4 | CLI subcommand parser for `tron` binary — `wallet create`, `wallet import`, `wallet list`, `send`, etc. (Phase 4) | Already workspace dep via `btc` and `eth` CLIs. Reuses same `clap` derive macros pattern. |

### B. NEW workspace crates (added in Phase 0 Step 2 — adds 4 direct deps, ~6 transitive via `prost`)

| Crate | Version | Role in TRON wallet | Why chosen |
|---|---|---|---|
| `prost` | **0.14.4** (released 2026-06-07) | Protobuf serialization for `Transaction`, `TransferContract`, `TriggerSmartContract`, `BlockHeader`, `Block` — `prost::Message::encode_to_vec(&raw_data)` for signing | De-facto Rust protobuf implementation, maintained by tokio-rs. MSRV Rust 1.85 (matches workspace). **Version corrected 2026-08-27** (prior doc said 0.13; 0.14.4 is the current line, what `39george/tronic` already pins). 0.13 line had minor API churn on the `Message` trait — staying on 0.14 keeps ecosystem alignment. |
| `prost-types` | **0.14.4** | Protobuf well-known types (e.g. `google.protobuf.Timestamp` if used in Tron proto extensions) | Required by `prost` for well-known type support. Likely transitive, declared explicit for clarity. |
| `bs58` | **0.5** | base58 + base58check encoding for T-base58check address display (Q4) | Canonical Rust base58 impl — used by `rust-bitcoin`, `solana-sdk`, `near-primitives`. 0.5 is current line; 0.4 is deprecated by upstream. MIT licensed, no CVEs in either line per GitHub Advisory DB. **Note:** TRON uses base58 + 4-byte SHA-256d checksum (== base58check); we hand-roll the checksum verify (4 lines) — `bs58::encode` is plain base58, no built-in check. |
| `tiny-keccak` | **2.0.2** | Keccak-256 hash for address derivation — `Keccak256::new()` → `keccak(pubkey_uncompressed[1..65])` → take last 20 bytes (Q4) | Pure-Rust, CC0/Apache-2.0, no_std + unsafe-free. Used by `alloy-primitives`, `revm`, `near-primitives`. **Wire-format hazard flagged:** tiny-keccak's `Keccak` uses original Keccak padding (0x01) — TRON/Ethereum convention. `sha3::Sha3_256` uses NIST FIPS-202 padding (0x06) and produces DIFFERENT digests. Never use `sha3` crate for TRON address derivation. |

### C. Build tools + system dependencies (CI + dev environment)

| Tool | Version | Role | Why chosen |
|---|---|---|---|
| `prost-build` | **0.14.4** | Build-time codegen — compiles vendored `core/Tron.proto` → Rust types at compile time via `build.rs` | Companion to `prost`. Required to call `prost_build::Config::new().compile_protos(...)`. |
| `protoc` | **≥3.12** | System protobuf compiler invoked by `prost-build` during `cargo build` | The standard `protoc` binary is a CI install dep. CI must have `protobuf-compiler` package. Alternative (vendored pre-generated `.rs` via `include!`) couples source tree to schema version — rejected; we re-pin the vendored `.proto` to a specific SHA each spike. |
| **TronBox** | **4.10.0+** (Node ≥20) | Local dev regtest + `MockTRC20` deploy (Phase 3 testing) | Now at `tronprotocol/tronbox` (org moved from `trufflesuite/tronbox` — verified 2026-08-27). Active monthly release cadence (v4.10.0 on 2026-08-13). **v4.5.0 was BREAKING** — dropped `web3` v4 for `ethers` v6. Solidity 0.8.x compatible. **NOT a Rust dep** — runs in Node for spike regtest only. Mirrors eth `alloy-node-bindings::AnvilInstance` pattern. |
| `cargo` | 1.94 stable | Workspace MSRV pin via `rust-toolchain.toml` | Matches Bitcoin + eth plan toolchain. `prost 0.14.4` MSRV is 1.85; `k256` MSRV is 1.65; we exceed both comfortably. |

### D. Cross-crate reuse (NOT a new dep — single import)

| Source | Path | Role in TRON wallet | Why chosen |
|---|---|---|---|
| `SpkiPinnedVerifier` | `bitcoin_wallet_core::chain::spki` | Custom `rustls::ServerCertVerifier` for SPKI-pinned JSON-RPC transport (Q7) | Bitcoin-side F20 finding produced this verifier — single import, zero new code. Re-uses the same `rustls = "0.23"` version as TRON-side `reqwest`. Path verified 2026-08-27 at `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`. **Reuse over rewrite** — same threat model, same pin format, same Cloudflare rotation pattern. |

**Total Rust dep footprint:** 13 crates (9 reused + 4 new) + 2 build tools + 1 cross-crate import. No FFI. No C compiler required at runtime.

## Global Constraints (verbatim from #399 + deep-dive Q resolutions)

- **Q1 — SDK choice.** **Raw `reqwest` + `prost` 0.14.4.** No TRON-specific SDK. Reject `rust-tron` (andelf — last push 2025-01-09, LGPL-3.0 copyleft risk), `tronic` v0.6.1 (39george — gRPC-only, 7 stars, single maintainer), `tronz` 0.6.0 (throgxyz — 55 stars but 1-follower account), `0xcregis/anychain` 253 stars but multi-chain scope, `tron-rs` 0.1.0 (proto-only). Decision rationale: maintenance risk floor + Bitcoin/eth precedent + cross-chain type unification (Keccak-256 shared with Bitcoin future needs).
- **Q2 — Transaction format.** **Protobuf via `prost` 0.14.4 + `prost-build`**, schema from `core/Tron.proto` (`tronprotocol/java-tron`, recommended pinned SHA `851575d` 2026-07-14). Build script compiles proto to Rust types via `build.rs` + system `protoc ≥3.12`. Signing: SHA-256 of protobuf-serialized `raw_data` (== `txID`) → 65-byte ECDSA signature = `r(32 BE) ‖ s(32 BE) ‖ v(1)` with **`v ∈ {0, 1}`** — NOT Ethereum's `v+27 ∈ {27, 28}`. Use `k256::ecdsa::Signature::from_sliced_64(...)` + `k256::ecdsa::VerifyingKey::recover_from_prehash(...)` for recovery-byte computation.
- **Q3 — TRC-20 ABI.** **Hand-roll for v0.1** (`transfer(address,uint256)` → `0xa9059cbb`, `balanceOf(address)` → `0x70a08231`, `decimals()` → `0x313ce567`). ~30 lines, no new deps. **Spike V3 round-trips against `alloy-sol-types` standalone** (4 deps only, no provider/transport — confirmed 2026-08-27) as reference impl; re-evaluate `alloy_sol_types::sol!` reuse at v0.3. **Wire-format contract (corrected 2026-08-27 via #410):** the server (`/wallet/triggerconstantcontract` + `/wallet/triggersmartcontract`) **prepends the 4-byte selector** to the `parameter` field; the client sends **encoded args only** (32 bytes per Solidity uint256/address). Do NOT include the selector in `data` — sending full 68-byte calldata (selector + arg) causes the server to prepend another selector, the call hits the wrong method signature, and the broadcast fails with `OUT_OF_ENERGY` or returns empty `constant_result`. Verified live in #409 spike (`rust-wallet-app/spikes/tron-v1/src/tx.rs:354` strips selector for `transfer`; `tx.rs:114` for `balanceOf`). Production crate's `encode_transfer`/`encode_balance_of` already emit arg-only bytes.
- **Q4 — Address encoding.** **T-base58check for display + storage, 21-byte raw form for internal API calls.** User-facing = T-base58check (34 chars, starts with `T`). Inter-contract call args (`TransferContract.owner_address`, `TriggerSmartContract.contract_address`) = `[0x41, last 20 bytes of keccak256(pubkey)]`. **Prefix byte `0x41` universal across all networks** (mainnet, Shasta, Nile — corrected 2026-08-27; prior doc said `0xa0` for Nile which is a legacy `net.type=testnet` flag never adopted). Encoding helper: `bs58` 0.5 + 4-byte double-SHA-256 checksum (hand-roll the checksum verify — `bs58::encode` is plain base58, no built-in check).
- **Q5 — Resource model UX.** Stake 2.0 (April 2023 via proposal #84 / TIP-467): 1 TRX = 1 TP, each stake picks **either Energy OR Bandwidth** (not both), 14-day unstake pending. USDT-TRC20 `transfer`: ~65,000 Energy if recipient holds USDT, ~130,300 if empty. Bandwidth: free 600/day (chain parameter #61) + 1,000 sun/byte TRX burn fallback. Energy: 100 sun/Energy default (`getEnergyFee` param, re-query before sizing `fee_limit`). DEM penalty scales per-contract energy by `max_factor = 3.4` per 6-hour cycle. **`fee_limit` denominated in SUN (not TRX)** — `fee_limit = 100` is 0.0001 TRX, tx fails with `OUT_OF_ENERGY`. **Wallet UX pattern:** per-resource breakdown (bandwidth / energy / TRX-equivalent) per MetaMask-TRON style; reference impl: `tron protocol java-tron` Resource Model docs. Estimation: `wallet/triggerconstantcontract` (primary, returns `energy_used` + optional `energy_penalty`) → fallback `wallet/estimateenergy` (edge cases, requires `vm.estimateEnergy` enabled).
- **Q6 — Testnet.** **Nile** for v0.1. Chain-id `0xcd8690dc` / 3448148188 (corrected 2026-08-27 — prior doc had `0x94a9059e` which is actually **Shasta's** chain-id). **Use `eth_chainId` JSON-RPC method via TronGrid `/jsonrpc`** — `wallet/getchainid` returns HTTP 405 on TronGrid's HTTP front. Address generation uses prefix `0x41` (same code path for mainnet/Shasta/Nile). Shasta kept as v0.2+ fallback. Faucet: TronFAQBot `!nile ADDR` → 5,000 nile TRX.
- **Q7 — RPC pinning.** **Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` verbatim** (file path verified 2026-08-27 at `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`). CLI URL scheme: `pinned://<spki-sha256-hex>@host[:port]`. Pin against `api.trongrid.io` requires Cloudflare rotation handling (~30 day cadence). For TAPOS reference: **use `walletsolidity/getnowblock`** (not `wallet/getnowblock`) for finality. **Enforcement is ship-gate follow-up (logged 2026-08-27 via #410):** the `pinned://<hex>@host` URL parser + `SpkiPin`/`SpkiPinSet` types parse and record the pin on `JsonRpcClient`, but the `post_*` HTTP helpers in v0.1 still use Rustls default verification (`rustls 0.23` + `webpki-roots`). Wiring `EsploraVerifier` (the `rustls::ServerCertVerifier` impl — currently private inside `bitcoin-wallet-core`) into a custom reqwest `ClientBuilder` is `#408` ship-gate follow-up. Without the enforcement bridge, the live outbound request trusts the system trust store plus the URL-pinned endpoint identity (Nile = one well-known host, low risk; broader multi-host pinning needs the bridge). Do NOT ship v1.0+ without enforcement in place.
- **Q8 — Sign-only path.** Same as eth Task 3. Local-sign TRX or TRC-20 transfer, return `SignedTx { tx_id, raw_data_hex, signature_hex }` without broadcasting. **MUST verify signature is `r‖s‖v` with `v ∈ {0, 1}` (NOT Ethereum convention).** **Do not reuse ETH/BSC signers (clarified 2026-08-27 via #410):** off-the-shelf Ethereum-/BSC-/Polygon-style signers emit `v ∈ {27, 28}` (`v + 27`). Reusing such a signer on TRON produces a 65-byte signature with `signature[64] == 27 || 28` instead of `0 || 1` — `txID = SHA256(raw_data_hex)` still hashes correctly and `recover_from_prehash` may accept the wrong key, but the broadcast node rejects the signature as malformed. Audit control **C4** (compile-fail test rejecting `k256::ecdsa::Signature::from_sliced_64(...)` output from eth-default `v+27` decoders — see Phase 1 Task 3 Step 5) catches this regression at build time. Production crate's `signing::sign_raw_data` is the only authorized TRON signing path — never share `k256::SigningKey` across TRON and ETH/BSC contexts.
- **Q9 — Token registry.** Bundled JSON in repo, mirrors eth Task 8: `rust-wallet-app/crates/tron-wallet-core/tokens/mainnet.json` (5 entries: USDT `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` (6 decimals), USDC `TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8` (6 decimals — Circle stopped issuing new TRC-20 USDC post-2023, existing token still active), TUSD `TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9` (18 decimals), USDD `TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz` (18 decimals), stUSDT `TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3` (6 decimals)) + `tokens/nile.json` (1 entry: community test USDT `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`). Spike V9 verifies USDT decimals = 6 via `triggerconstantcontract` call to `decimals()` selector.
- **Q10 — Mnemonic reuse + SLIP-44 vector.** **SLIP-44 coin type 195 = TRX** (confirmed `bip_utils::slip44::Coin::Tron` const exists per Agent B). Canonical derivation path: `m/44'/195'/0'/0/0`. Spike V10 verifies against canonical test vector: `bip39::Mnemonic::parse_in(English, "abandon ×11 about")` → seed → `m/44'/195'/0'/0/0` → address must match TronWeb/`andelf/rust-tron` reference. Address must start with `T` using prefix `0x41` (universal — same code path for all 3 networks).

## File Structure (decomposition)

```text
rust-wallet-app/crates/tron-wallet-core/
├── Cargo.toml
├── build.rs                              # prost-build compiles vendored core/Tron.proto
├── proto/
│   └── core/Tron.proto                   # vendored, pinned to SHA 851575d (2026-07-14)
├── src/
│   ├── lib.rs                            # pub mod + Error enum + re-exports
│   ├── error.rs                          # thiserror Error enum (mirrors eth Error)
│   ├── config.rs                         # WalletConfig { network: Network, rpc_url, derivation_path, fee_limit_sun }
│   ├── network.rs                        # Network enum (Mainnet/Shasta/Nile) + chain_id + prefix_byte (0x41 universal)
│   ├── mnemonic.rs                       # bip39 generate + Zeroizing wrap
│   ├── derivation.rs                     # bip32 m/44'/195'/0'/0/{idx}
│   ├── address.rs                        # base58check encode/decode (Q4)
│   ├── signing.rs                        # k256 sign_prehash + recovery-byte computation (Q2)
│   ├── proto/                            # generated by prost-build (TRANSFER_CONTRACT, TRIGGER_SMART_CONTRACT, etc.)
│   ├── transaction.rs                    # raw_data builder + TransferContract + TriggerSmartContract (Q2)
│   ├── trc20.rs                          # hand-rolled ABI encoder (Q3)
│   ├── rpc.rs                            # raw reqwest JSON-RPC client + SPKI pin verifier (Q7)
│   ├── resource.rs                       # energy/bandwidth estimation + fee_limit sizing + DEM awareness (Q5)
│   ├── provider.rs                       # tron-specific (no fillers, explicit nonce/fee)
│   ├── tokens.rs                         # bundled token registry loader (Q9)
│   └── wallet/
│       ├── mod.rs                        # WalletManager facade
│       ├── manager.rs                    # create/import/list/delete/show
│       └── sign.rs                       # sign-only path (Q8)
├── tokens/
│   ├── mainnet.json                      # 5 TRC-20 entries (Q9)
│   └── nile.json                         # 1 community test USDT
└── tests/
    ├── mnemonic_address.rs               # V10 — SLIP-44 vector → T-address
    ├── address_prefix.rs                 # V4 — 0x41 universal + base58check round-trip
    ├── transaction_roundtrip.rs          # V2 — prost encode/decode round-trip + TriggerSmartContract data-at-field-4
    ├── trc20_calldata.rs                 # V3 — hand-rolled transfer → 0xa9059cbb
    ├── rpc_nile.rs                       # V6 — eth_chainId via /jsonrpc + triggerconstantcontract
    ├── resource_model.rs                 # V5 — DEM awareness + fee_limit in SUN + Stake 2.0 path
    ├── spki_pin.rs                       # V7 — pinned:// URL + SpkiPinnedVerifier reuse
    ├── sign_only.rs                      # V8 — r‖s‖v with v ∈ {0, 1} (NOT v+27)
    └── token_registry.rs                 # V9 — mainnet.json + nile.json + decimals()

rust-wallet-app/crates/tron/              # CLI binary
├── Cargo.toml
└── src/main.rs                            # clap subcommands: create, import, list, show, sign, send, balance

rust-wallet-app/spikes/tron-v1/           # verification harness (V1–V10, one per Q)
├── Cargo.toml                            # workspace member; deps = chosen surface (prost 0.14.4, prost-types, prost-build, bs58, tiny-keccak, reqwest, rustls, k256, sha2, bip32, bip39, serde, serde_json)
├── README.md                             # V1-V10 acceptance criteria + PASS evidence template + run instructions
├── RESULT.md                             # V1-V10 PASS evidence log (filled after running each test)
├── proto/
│   └── core/Tron.proto                   # vendored copy (pinned SHA 851575d 2026-07-14); shared with crates/tron-wallet-core/proto/
├── tokens/
│   ├── mainnet.json                      # 5 entries (USDT/USDC/TUSD/USDD/stUSDT); shared with crates/tron-wallet-core/tokens/
│   └── nile.json                         # 1 entry (community test USDT); shared with crates/tron-wallet-core/tokens/
├── src/
│   ├── lib.rs                            # re-export module tree; spike helpers
│   ├── address.rs                        # Keccak-256 + base58check address derivation (used by V4 + V10)
│   ├── base58check.rs                    # bs58 + 4-byte double-SHA-256 checksum (used by V4)
│   ├── keccak.rs                         # tiny-keccak wrapper (used by V4 + V10)
│   ├── proto.rs                          # re-export generated proto types from crates/tron-wallet-core/src/proto/
│   ├── protobuf.rs                       # Transaction encode + sign helper (used by V2 + V8)
│   ├── abi.rs                            # hand-rolled TRC-20 ABI encoder/decoder (3 functions: transfer/balanceOf/decimals) (used by V3)
│   ├── rpc.rs                            # reqwest JSON-RPC client + eth_chainId via /jsonrpc (used by V6 + V9)
│   └── spki.rs                           # wrapper around bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier (used by V7)
└── tests/
    ├── v1_compile.rs                     # cargo build succeeds with workspace deps (V1)
    ├── v2_protobuf_roundtrip.rs          # TransferContract encode/decode byte-equal + TriggerSmartContract.data at field 4 (V2)
    ├── v3_trc20_abi.rs                   # encode_transfer produces 68-byte calldata with 0xa9059cbb selector at bytes [0..4] (V3)
    ├── v4_base58check.rs                 # base58check encode/decode round-trip + canonical-vector regression (V4)
    ├── v5_resource.rs                    # live wallet/triggerconstantcontract → energy_used 65k-130k for USDT-TRC20 transfer + getcontractinfo.energy_factor round-trip (V5; GATED)
    ├── v6_nile.rs                        # live POST /jsonrpc eth_chainId → 0xcd8690dc + address generation matches nile.tronscan.org + walletsolidity/getnowblock TAPOS (V6; GATED)
    ├── v7_spki_pin.rs                    # SpkiPinnedVerifier accepts pinned://<correct_pin>@api.trongrid.io, rejects wrong pin (V7; GATED for live cert)
    ├── v8_sign_only.rs                   # local-sign TRX transfer (no broadcast) + txID = SHA256(raw_data_hex) matches what network reports (V8; GATED)
    ├── v9_token_registry.rs              # tokens/{mainnet,nile}.json load + USDT decimals=6 verified via live triggerconstantcontract(decimals()) + energy_penalty field present (V9; GATED)
    └── v10_slip44.rs                     # bip39 "abandon x11" → seed → m/44'/195'/0'/0/0 → T-address matches TronWeb reference (V10)

**Spike build dependency:** `protoc ≥ 3.12` must be in PATH for `prost-build` codegen at `cargo build` time. CI image must install `protobuf-compiler` package (Debian/Ubuntu) or `brew install protobuf` (macOS). Document in spike README.

**Spike live-testnet gating:** V5/V6/V7/V8/V9 require network access to Nile testnet (`https://nile.trongrid.io`) + an optional TRON-PRO-API-KEY for higher rate limits. These tests are gated behind the `RUN_TRON_NILE=1` env var (mirrors eth L29 pattern); without it they print `[SKIP — RUN_TRON_NILE=1 required]` and exit 0. V1/V2/V3/V4/V10 are sync + offline and always run.
```

## Spike mapping (issue #399 deliverable 4)

The spike (`rust-wallet-app/spikes/tron-v1/`) is the verification harness that proves each Q's chosen path before the corresponding phase ships. V1–V10 each tie to a Q and produce PASS evidence (command output + SHA).

| V# | Q | What it verifies | Maps to Phase |
|---|---|---|---|
| V1 | Q1 | `cargo add prost@0.14 prost-types@0.14 bs58@0.5 tiny-keccak@2.0.2` compiles against workspace | Phase 0 dep wiring |
| V2 | Q2 | `prost-build` compiles pinned `core/Tron.proto` (SHA `851575d`) + TriggerSmartContract `data` at field 4 round-trip | Phase 2 transaction.rs |
| V3 | Q3 | Hand-rolled `encode_transfer(to_20_bytes, value_u128)` produces 68-byte calldata with `0xa9059cbb` at bytes 0..4; round-trips against `alloy-sol-types` standalone | Phase 3 trc20.rs |
| V4 | Q4 | `base58check_encode([0x41; 1] ++ last_20_bytes_of_keccak256(pubkey))` produces 34-char `T...` string; decode round-trips | Phase 1 address.rs |
| V5 | Q5 | `triggerconstantcontract` returns `energy_used` 65k–130k for USDT-TRC20 transfer; `getcontractinfo.energy_factor` round-trip; `fee_limit` sizing in SUN | Phase 3 resource.rs |
| V6 | Q6 | `POST /jsonrpc eth_chainId` → `0xcd8690dc` on Nile; address generation against `0x41` prefix matches `nile.tronscan.org`; `walletsolidity/getnowblock` for TAPOS | Phase 2 network.rs + rpc.rs |
| V7 | Q7 | `SpkiPinnedVerifier` (imported from `bitcoin-wallet-core::chain::spki`) accepts `pinned://<correct_pin>@api.trongrid.io`, rejects wrong pin | Phase 2 rpc.rs |
| V8 | Q8 | Local-sign TRX transfer; verify `txID = SHA256(raw_data_hex)` matches network; **signature is `r‖s‖v` with `v ∈ {0, 1}`** | Phase 1 signing.rs + wallet/sign.rs |
| V9 | Q9 | `tokens/mainnet.json` 5 entries load + USDT decimals = 6 verified via `triggerconstantcontract(decimals())`; **`energy_penalty` field present** for fee UX | Phase 3 tokens.rs |
| V10 | Q10 | `bip_utils::slip44::Coin::Tron` (195) → mnemonic → seed → `m/44'/195'/0'/0/0` → T-address matches TronWeb reference | Phase 1 derivation.rs |

## Phase 0.0 — Network selection + local-dev testnet (NEW, added 2026-08-27 from #403 + use case)

> **Why this section**: The original Phase 0 jumped straight to crate scaffolding without
> first answering "which networks does the wallet target?" and "how do we run the wallet
> against a deterministic local chain in tests?". #403 + the `use_case_alpha_sends_beta_100_usdt`
> spike surfaced both as missing prerequisite decisions. This section locks them in
> before any production code lands.

### 0.0.a — Research network (which TRON networks)

The wallet targets **three TRON networks** — one production, two test:

| Network | Chain ID | Use | Faucet | Endpoint |
|---------|----------|-----|--------|----------|
| Mainnet | `0x2b6653dc` (728126428) | Production TRX + TRC-20 | none | `https://api.trongrid.io/wallet/*` |
| Nile (testnet) | `0xcd8690dc` (3448148188) | **Primary testnet** for v0.1 | TronFAQBot `!nile <ADDR>` → 5,000 nile TRX | `https://nile.trongrid.io/wallet/*` |
| Shasta (testnet, deprecated) | `0x94a9059e` (2494104990) | Kept as v0.2+ fallback; **NOT v0.1 primary** | tronbox / TRON faucet | `https://api.shasta.trongrid.io/wallet/*` |

**Drift correction (2026-08-27)**: prior doc listed Shasta as primary testnet; corrected
to Nile (correct chain-id `0xcd8690dc`, confirmed by V6 spike via `POST /jsonrpc eth_chainId`).
Nile's chain-id was verified live against `nile.trongrid.io`. Address generation uses the
universal `0x41` prefix across all 3 networks (correction to prior doc's `0xa0` for Nile,
a legacy `net.type=testnet` flag never adopted).

### 0.0.b — Choose local testnet (the "in-process chain" for offline CI)

Per L29 + per chain family:

| Chain family | In-process local chain | Rust spawn crate | Operator setup |
|--------------|------------------------|------------------|----------------|
| Ethereum (already in repo) | **Anvil** (Foundry) | `alloy-node-bindings::Anvil::new().spawn()` | `cargo install --git https://github.com/foundry-rs/foundry --bin anvil --locked` |
| TRON (this plan) | **TronBox** (`tronbox/tre` Docker image) | **`testcontainers = "0.23"`** (Rust, wraps Docker) | `docker pull tronbox/tre:latest` |

**Why no pure-Rust TRON emulator exists**: TRON's reference implementation is Java
(`java-tron`); there is no Foundry/Anvil-equivalent for TRON in Rust today. The
closest all-Rust option — [Tronic](https://www.reddit.com/r/rust/comments/1marc3n/announcing_tronic_a_rust_toolkit_for_tron/)
(July 2025) — is a **client** SDK, not a chain simulator. So the spike falls back to
**testcontainers + tronbox/tre Docker image**: in-process spawn from Rust, real local
TRON node, `drop(container)` cleans up. This is the pattern `alloy-v1/tests/v6_erc20_anvil.rs`
uses for Ethereum, with the swap `Anvil::new()` → `testcontainers` to accommodate TRON's
Java dependency.

**Decision**: Adopt `testcontainers = "0.23"` as the local-dev chain spawn crate for TRON.
Workspace dep version aligned with the existing `btc` crate's `^0.23` constraint
(workspace resolver = "2" unifies `bollard-stubs` across both). Spike V11 use-case test
already validated the pattern locally (commit `439c2e0`, test PASS in 4.36s).

### 0.0.c — Choose testnet (live operator-driven verification)

**Decision**: **Nile** (`https://nile.trongrid.io/wallet/*`) is the v0.1 primary testnet.
Selection criteria + citations:

| Criterion | Nile | Shasta | Decision |
|-----------|------|--------|----------|
| TronGrid operator support | Active (2024-2026) | Maintained but lower traffic | **Nile** |
| Community USDT faucet | `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (100 USDT minted by TronGrid) | None | **Nile** |
| Chain-id verifiable via `eth_chainId` | `0xcd8690dc` (verified V6 spike) | `0x94a9059e` | Either works |
| Stake 2.0 (proposal #84 / TIP-467, April 2023) | Active | Active | Either works |
| Faucet UX | TronFAQBot `!nile ADDR` (Telegram) | tronbox | **Nile** (1-step Telegram) |

Shasta remains as v0.2+ fallback (different chain-id, different faucet). The spike V5/V6
tests both gate on `RUN_TRON_NILE=1` (operator-driven per L29). Production code in
`crates/tron-wallet-core/` will default to `Network::Nile` for testnet builds, `Network::Mainnet`
for release.

### 0.0.d — Use case validation (cross-reference ROADMAP.md)

The end-to-end "alpha → beta 100 USDT-TRC20 on local testnet" use case is documented at
`rust-wallet-app/spikes/tron-v1/ROADMAP.md` (committed 2026-08-27 in spike PR #405).
Status as of merge commit `bfb14eb`:

- **Offline companion** (`use_case_alpha_sends_beta_100_usdt_offline`, always runs in CI): PASS — verifies
  alpha + beta wallet derivation + 68-byte TRC-20 calldata + 65-byte k256 signature with `v ∈ {0, 1}`.
- **Live variant** (`use_case_alpha_sends_beta_100_usdt_live_local_node`, gated `RUN_TRON_LOCAL=1`):
  **PASS locally** via testcontainers (4.36s wall-clock, observed blockID
  `0000000000000000c93baa76a4a508f798a96f59156d9eb17ecede8ec845df2f`).

**Gap to full end-to-end (backlog)**: full TRC-20 broadcast + balance-verify requires shipping
a `MockTRC20.sol` fixture + running `tronbox migrate --network development` inside the
spawned container. Tracked as a follow-up issue after #403 closes. Not a Phase 0/1/2 blocker —
the wire format is already proven by V2 (protobuf roundtrip), V3 (TRC-20 ABI calldata),
V8 (sign-only).

### 0.0.e — Research crates/SDKs that support spawning a local testnet (decision matrix)

| Crate / tool | Language | Chain | Local-node spawn API | Verdict |
|--------------|----------|-------|----------------------|---------|
| `alloy-node-bindings` (Foundry) | Rust | Ethereum | `Anvil::new().spawn()` — pure Rust, returns `AnvilInstance` | **Use** (already adopted; `alloy-v1/tests/v6_erc20_anvil.rs`) |
| `testcontainers` (Docker) | Rust | any | `GenericImage::new(name, tag).start().await` — wraps Docker daemon | **Use for TRON** (no Rust emulator) |
| `tronbox/tre` Docker image | Node + Java | TRON | external Docker container; `tronbox migrate` to deploy contracts | **Adopt via testcontainers** |
| [Tronic](https://www.reddit.com/r/rust/comments/1marc3n/announcing_tronic_a_rust_toolkit_for_tron/) | Rust | TRON | client only — no node simulator | **Skip** (not a spawn library) |
| Hardhat | Node + JS | Ethereum | external Node process; `npx hardhat node` | **Skip** (Node dep; Anvil already used in repo) |
| Ganache | Node + JS | Ethereum | external Node process | **Skip** (deprecated; Anvil supersedes) |
| Foundry (`cast`/`anvil`/`forge` CLI) | Rust binary | Ethereum | external CLI | **Skip** (Anvil via `alloy-node-bindings` already covers) |
| `revm` | Rust | Ethereum | in-process EVM (no chain state, deterministic) | **Skip** (no p2p / state layer; Anvil handles) |

**Final stack**:

- Ethereum local: `alloy-node-bindings::Anvil::new().spawn()` (already in `alloy-v1`)
- TRON local: `testcontainers = "0.23"` + `tronbox/tre:latest` Docker image (already in `spikes/tron-v1`)

---

## Phase 0 — Scaffold + canonical address test (1 task)

### Task 1 (#4XX): Crate scaffold + V10 mnemonic → T-address test

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/Cargo.toml`
- Create: `rust-wallet-app/crates/tron-wallet-core/build.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/proto/core/Tron.proto` (vendored from `tronprotocol/java-tron` SHA `851575d`)
- Create: `rust-wallet-app/crates/tron-wallet-core/src/lib.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/error.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/network.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/address.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/mnemonic.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/derivation.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/tests/mnemonic_address.rs`

**Interfaces:**
- `tron_wallet_core::mnemonic::generate_12_word() -> Zeroizing<Mnemonic>`
- `tron_wallet_core::derivation::derive_address(phrase: &Mnemonic, index: u32, network: Network) -> [u8; 21]` (Q4 — returns 21-byte raw form, prefix `0x41` universal per Q4 correction)
- `tron_wallet_core::address::to_base58check(raw_21_bytes: &[u8; 21]) -> String` (Q4)
- `tron_wallet_core::network::Network` enum: `Mainnet { chain_id: 0x2b6653dc, prefix: 0x41 }`, `Shasta { chain_id: 0x94a9059e, prefix: 0x41 }`, `Nile { chain_id: 0xcd8690dc, prefix: 0x41 }` (all use `0x41` prefix — corrected from prior doc's `0xa0` for Nile)

**Steps:**
- [ ] Step 1: Add `tron-wallet-core` to umbrella `members` in `rust-wallet-app/Cargo.toml`
- [ ] Step 2: Add `prost = "0.14.4"` + `prost-types = "0.14.4"` + `bs58 = "0.5"` + `tiny-keccak = "2.0.2"` to workspace deps
- [ ] Step 3: Vendor `core/Tron.proto` from `tronprotocol/java-tron` at SHA `851575d` (2026-07-14) — `curl -L https://raw.githubusercontent.com/tronprotocol/java-tron/851575d/protocol/src/main/protos/core/Tron.proto -o proto/core/Tron.proto` (or `git clone` + checkout)
- [ ] Step 4: Write `build.rs` that calls `prost_build::Config::new().compile_protos(&["proto/core/Tron.proto"], &["proto/"])?` — generates Rust types at compile time
- [ ] Step 5: Implement `address::to_base58check()` using `bs58::encode(...)` + 4-byte double-SHA-256 checksum (hand-roll checksum verify — `bs58::encode` is plain base58, no built-in check)
- [ ] Step 6: Implement `derivation::derive_address()` using `bip32::XPrv::derive_from_path(seed, "m/44'/195'/0'/0/0")` + `k256::SigningKey::from_bytes` + Keccak-256 of pubkey → last 20 bytes + `0x41` prefix
- [ ] Step 7: Implement `network::Network` enum with Mainnet/Shasta/Nile variants (all `prefix: 0x41`, distinct chain-id hex)
- [ ] Step 8: Write `tests/mnemonic_address.rs` — all-`abandon` mnemonic → seed → `m/44'/195'/0'/0/0` → T-address must match TronWeb reference (round-trip via `nile.tronscan.org` lookup is acceptable for Nile testnet)
- [ ] Step 9: Verify gate (cargo fmt + clippy --all-targets -- -D warnings + test)
- [ ] Step 10: **Audit ship-gate** — verify controls **C1** (proto SHA assert in `build.rs`), **C2** (`bip39` features `["zeroize","rand"]`), **P0-1** (Keccak-256 vs SHA3-256 negative test), **P0-2** (base58check property test ≥10k rounds), **P0-3** (`derive_address` returns `Zeroizing<[u8;21]>`). See [audit doc §Phase 0](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 11: Commit `feat(tron): scaffold tron-wallet-core crate — address + derivation (Task 1)`

## Phase 1 — Core wallet ops (3 tasks)

### Task 2 (#4XX): WalletManager + create/import/list/delete

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/src/wallet/{mod,manager}.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/config.rs`
- Modify: `rust-wallet-app/crates/tron-wallet-core/src/lib.rs`

**Interfaces:**
- `tron_wallet_core::wallet::WalletManager` holding `RwLock<HashMap<WalletId, Zeroizing<Mnemonic>>>` (parallel to eth Task 2)
- `tron_wallet_core::config::WalletConfig { network: Network, rpc_url: String, derivation_path: DerivationPath, fee_limit_sun: u64 }`

**Steps:**
- [ ] Step 1: Implement `WalletManager` + persistence (SQLite or sled; mirror Bitcoin `WalletStore`)
- [ ] Step 2: Implement `create_wallet(words, password) -> WalletCreated` (Q7 — wraps in Zeroizing; v0.1 stores plaintext like eth, v0.2+ adds Argon2id + AES-256-GCM)
- [ ] Step 3: Implement `import_wallet(phrase, password) -> WalletId`
- [ ] Step 4: Implement `list_wallets() -> Vec<WalletInfo>` + `delete_wallet(id)` + `show_wallet(id)`
- [ ] Step 5: Tests for each op + persistence round-trip
- [ ] Step 6: **Audit ship-gate** — verify controls **C3** (`WalletConfig::default()` has no `default_rpc_url`), **P1-1** (mnemonic-at-rest encrypted with Argon2id m≥64MiB t≥3 p=4 + AES-256-GCM — **block v0.1 ship without this**), **P1-2** (`create_wallet(words, password)` either drops param or returns `Error::PasswordUnsupportedInV01`), **P1-3** (passphrase length ∈ {0, 1..8} → `Error::WeakPassphrase`), **P1-5** (`list_wallets()` defaults to address-only; `--metadata` opt-in). See [audit doc §Phase 1](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 7: Commit

### Task 3 (#4XX): Sign-only path (Q8 + signature convention hazard)

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/src/signing.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/wallet/sign.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/tests/sign_only.rs`

**Interfaces:**
- `tron_wallet_core::signing::sign_raw_data(sk: &SigningKey, raw_data: &RawData) -> [u8; 65]` — returns `r‖s‖v` with `v ∈ {0, 1}` (Q2 + Q8 hazard)
- `tron_wallet_core::signing::recover_pubkey(tx_hash: &[u8; 32], sig: &[u8; 64], v: u8) -> Option<VerifyingKey>` — recovery-byte computation via `k256::ecdsa::VerifyingKey::recover_from_prehash`
- `tron_wallet_core::wallet::sign::sign_trx_transfer(wallet: &WalletManager, wallet_id: WalletId, to: TAddress, amount_sun: u64) -> SignedTx`
- `tron_wallet_core::wallet::sign::sign_trc20_transfer(wallet: &WalletManager, wallet_id: WalletId, token: TAddress, to: TAddress, amount_base_units: u128) -> SignedTx`

**Steps:**
- [ ] Step 1: Implement `signing::sign_raw_data()` — `k256::SigningKey::sign_prehash(tx_hash).to_bytes()` (64 bytes r‖s) + compute `v` via `recover_pubkey` retry at `v=0` and `v=1` against expected sender pubkey
- [ ] Step 2: Implement `sign_trx_transfer()` — build `TransferContract` via `Transaction.raw_data` protobuf, get TAPOS reference from `walletsolidity/getnowblock`, set `expiration = head_block_ts + 60_000`, `fee_limit = 0` (TRX transfers use bandwidth, not energy), sign, return `SignedTx`
- [ ] Step 3: Implement `sign_trc20_transfer()` — build `TriggerSmartContract` with `data` at **field 4** (off-by-one hazard flagged in deep-dive), `fee_limit` sized via Phase 3 resource model, sign, return `SignedTx`
- [ ] Step 4: Test `sign_only.rs` verifies **signature byte order `r‖s‖v` with `v ∈ {0, 1}`** (assert `signature[64] == 0 || signature[64] == 1`, NOT `27 || 28`). Ethereum-default signers produce invalid TRON signatures — this test catches the regression at build time.
- [ ] Step 5: **Audit ship-gate** — verify controls **C4** (compile-fail test that `k256::ecdsa::Signature::from_sliced_64(...)` output is rejected by eth-default `v+27` decoder), **C5** (`Network::Mainnet` ↔ embedded `ref_block` chain_id mismatch → refuse to sign), **C6** (zeroize-on-drop `SigningKey`; `to_bytes()` only callable inside `signing.rs`), **P1-4** (Error enum adds `ExpiredTransaction`, `NonceReuse`, `InsufficientEnergy`, `InsufficientBandwidth`). See [audit doc §Phase 1](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 6: Commit

### Task 4 (#4XX): Error enum + serde + zeroize wrap

**Files:**
- Modify: `rust-wallet-app/crates/tron-wallet-core/src/error.rs`
- Modify: `rust-wallet-app/crates/tron-wallet-core/src/mnemonic.rs`

**Steps:**
- [ ] Step 1: ~20-variant Error enum mirroring eth Error schema + TRON-specific variants (`OutOfEnergy`, `BandwidthExhausted`, `SignatureRecoveryFailed`, `ProtoDecodeError`, `Base58ChecksumMismatch`, `AddressPrefixMismatch` etc.)
- [ ] Step 2: Wrap `Mnemonic` in `Zeroizing<Mnemonic>` (Q7 zeroize treatment, mirrors eth Task 4 + Bitcoin Task 30)
- [ ] Step 3: `Zeroizing` wrap for `SigningKey`'s internal secret bytes
- [ ] Step 4: **Audit ship-gate** — verify control **P1-4** Error enum surfaces `ExpiredTransaction`, `NonceReuse`, `InsufficientEnergy`, `InsufficientBandwidth` (rejects `String`-payload variants in PR review). See [audit doc §P1-4](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 5: Commit

## Phase 2 — RPC integration + protobuf tx (4 tasks)

### Task 5 (#4XX): Vendored protobuf schema + generated types

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/proto/core/Tron.proto` (already vendored in Task 1 Step 3)
- Modify: `rust-wallet-app/crates/tron-wallet-core/build.rs`
- Modify: `rust-wallet-app/crates/tron-wallet-core/src/lib.rs` (add `pub mod proto`)

**Steps:**
- [ ] Step 1: Verify SHA `851575d` is committed in `proto/core/Tron.proto` (CI check: SHA must match the pinned commit or build fails)
- [ ] Step 2: `build.rs` calls `prost_build::Config::new().compile_protos(&["proto/core/Tron.proto"], &["proto/"])?` — generates `Transaction`, `TransferContract`, `TriggerSmartContract`, `BlockHeader`, `Block` types in `pub mod proto`
- [ ] Step 3: Verify `cargo build` produces the generated types (commit `cargo:rerun-if-changed=proto/core/Tron.proto` so proto changes trigger rebuild)
- [ ] Step 4: Document `protoc ≥3.12` requirement in `Cargo.toml` `[package.build-dependencies]` + README
- [ ] Step 5: **Audit ship-gate** — verify control **C1** re-affirmed: vendored `core/Tron.proto` SHA `851575d` matches in `build.rs` (assert via `sha256_of_file`); CI fails on drift. See [audit doc §C1](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 6: Commit

### Task 6 (#4XX): Raw reqwest JSON-RPC client + SPKI pin verifier (Q7)

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/src/rpc.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/tests/spki_pin.rs`

**Interfaces:**
- `tron_wallet_core::rpc::new_http(url: &str) -> Result<reqwest::Client, Error>` (Scenario B — system trust + localhost)
- `tron_wallet_core::rpc::new_http_pinned(url: &str, spki: &[u8; 32]) -> Result<reqwest::Client, Error>` (Scenario A — reuse `bitcoin_wallet_core::chain::spki::SpkiPinnedVerifier` directly)
- `tron_wallet_core::rpc::parse_rpc_url(url: &str) -> Result<(String, Option<[u8; 32]>), Error>` — handles `pinned://<hex>@host[:port]` scheme

**Steps:**
- [ ] Step 1: Implement `new_http()` — plain `reqwest::Client::new()` with `rustls-tls` + `webpki-roots`
- [ ] Step 2: Implement `new_http_pinned()` — reuse `bitcoin_wallet_core::chain::spki::SpkiPinnedVerifier` (single import, zero new code — verified path `bitcoin-wallet-core/src/chain/spki.rs`)
- [ ] Step 3: Implement `parse_rpc_url()` for `pinned://<hex>@host` extension
- [ ] Step 4: Test `spki_pin.rs` — correct pin against `pinned://<hex>@api.trongrid.io` succeeds; wrong pin returns `Error::SpkiPinMismatch { expected, actual }`
- [ ] Step 5: Add `TRON-PRO-API-KEY` header support via `--trongrid-api-key` CLI flag (raises rate limit from 3 QPS unauth to 15 QPS auth — corrected 2026-08-27)
- [ ] Step 6: **Audit ship-gate** — verify controls **C7** (`pinned_endpoints.json` ships in repo for `api.trongrid.io` / `nile.trongrid.io` / `api.shasta.trongrid.io`; CLI `--print-pinned-hosts` lists them), **C8** (`new_http()` non-pinned gated `#[cfg(test)]`; production builds reach only `new_http_pinned()`), **P2-1** (`pinned://<hex>@host` parser — 12 unit-test cases: odd-length, non-hex, empty, missing `@`, multi `@`, mixed case, whitespace, NUL bytes). See [audit doc §Phase 2](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 7: Commit

### Task 7 (#4XX): RPC methods — getnowblock, getchainid, createaccount, getaccount, getblockbynum

**Files:**
- Modify: `rust-wallet-app/crates/tron-wallet-core/src/rpc.rs`

**Interfaces:**
- `tron_wallet_core::rpc::get_solidity_now_block(client: &Client) -> Result<Block, Error>` (Q6 — use `walletsolidity/getnowblock`, NOT `wallet/getnowblock`)
- `tron_wallet_core::rpc::eth_chain_id(client: &Client) -> Result<u64, Error>` (Q6 — use `eth_chainId` JSON-RPC via `/jsonrpc`, NOT `wallet/getchainid` which returns HTTP 405)
- `tron_wallet_core::rpc::get_account(client: &Client, addr: &TAddress) -> Result<Account, Error>` — balance + resource snapshot
- `tron_wallet_core::rpc::broadcast_transaction(client: &Client, signed_tx: &SignedTx) -> Result<TxId, Error>`

**Steps:**
- [ ] Step 1: Implement `get_solidity_now_block()` — POST to `/walletsolidity/getnowblock`, parse JSON
- [ ] Step 2: Implement `eth_chain_id()` — POST to `/jsonrpc` with `{"method":"eth_chainId"}`, parse hex string response (chain-id for current network; verified `0xcd8690dc` Nile, `0x94a9059e` Shasta, `0x2b6653dc` Mainnet live 2026-08-27)
- [ ] Step 3: Implement `get_account()` — POST to `/wallet/getaccount`, parse JSON response with balance + frozen bandwidth/energy
- [ ] Step 4: Implement `broadcast_transaction()` — POST to `/wallet/broadcasttransaction` with the FULL signed-tx JSON body `{ "transaction": { "raw_data": <RawData JSON object>, "raw_data_hex": "<hex>", "txID": "<hex>", "signature": ["<hex sig>"], "visible": true } }` (NOT just `raw_data_hex` + `txID` + `signature` — see shipped-bug note below). Parse `result: true, txid: "..."` response.
- [ ] Step 4a: Implement `get_transaction_info_by_id(client: &Client, tx_id: &TxId) -> Result<TxReceipt, Error>` — POST to `/wallet/gettransactioninfobyid`, parse `id` (echoes submitted txID — this is the request-to-response binding) + `receipt.result` (must equal `"SUCCESS"`). **Poll loop MUST use this endpoint for receipt-based confirmation** — `/wallet/gettransactionbyid` returns only the bare transaction record (`ret[0].contractRet`) with NO `txID` field, so request-to-response binding cannot rely on it. Verified live 2026-08-27 in #409 spike (`rust-wallet-app/spikes/tron-v1/src/tx.rs` switched from by-id to receipt endpoint; `tx_confirmed_in_receipt` parser + `id == tx_id` binding).
  - **Shipped-bug notes (logged 2026-08-27 via #410):**
    - **#409 broadcast NPE:** dropping the structured `raw_data` JSON object from the broadcast body caused `NULLPointerException` server-side — the node re-serializes `raw_data` to protobuf for signature verification. Per [BroadcastServlet spec](https://github.com/tronprotocol/documentation-en/blob/master/docs/api/http/tx-build-and-broadcast/broadcasttransaction.md) the node expects the full transaction tree. Fix: parse `transaction.raw_data` in `TriggerSmartResponse` and pass through into `broadcast_body`.
    - **#409 poll timeout:** initial `tx_visible_in_response` looked for a `txID` field in the by-id response (which doesn't exist) and polled indefinitely. Fix: switched to the receipt endpoint + required `id == tx_id` + `receipt.result == "SUCCESS"` (defense-in-depth, also flagged by automated security review).
- [ ] Step 5: All RPC tests use `#[tokio::test]` per `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` §"Appendix: Async test function priority"
- [ ] Step 6: **Audit ship-gate** — verify controls **P2-2** (`eth_chainId` parser strict: `0x` prefix + lowercase + 8 hex chars; rejects non-canonical), **P2-3** (`walletsolidity/getnowblock` + `wallet/getnowblock` fallback with `ref_block_bytes/hash` cross-validate), **P2-4** (`broadcast_transaction` defaults `visible: false`; opt-in `visible: true` only for debug), **P2-5** (`get_account` uses `serde_json` + `deserialize_with`; property test against canonical TronGrid response). See [audit doc §Phase 2](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 7: Commit

### Task 8 (#4XX): Transaction builder — TransferContract + TriggerSmartContract (Q2)

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/src/transaction.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/tests/transaction_roundtrip.rs`

**Interfaces:**
- `tron_wallet_core::transaction::build_trx_transfer(sender_21: &[u8; 21], to_21: &[u8; 21], amount_sun: u64, ref_block: &Block) -> RawData` (Q2 — protobuf-serialize `Transaction.raw_data`)
- `tron_wallet_core::transaction::build_trc20_call(sender_21: &[u8; 21], token_21: &[u8; 21], calldata: &[u8], fee_limit_sun: u64, ref_block: &Block) -> RawData`
- `tron_wallet_core::transaction::tapos_ref(ref_block: &Block) -> (Vec<u8>, Vec<u8>)` — `(ref_block_bytes, ref_block_hash)` slices

**Steps:**
- [ ] Step 1: Implement `tapos_ref()` — `ref_block_bytes = block_number_be[6..8]`, `ref_block_hash = block_id[8..16]` (Q2 + verified against live mainnet block 85707935 on 2026-08-27)
- [ ] Step 2: Implement `build_trx_transfer()` — protobuf `Transaction.raw_data { contract: [TransferContract { owner_address, to_address, amount }], ref_block_bytes, ref_block_hash, expiration: head_block_ts + 60_000, timestamp: now_ms, fee_limit: 0, data: b"" }`
- [ ] Step 3: Implement `build_trc20_call()` — protobuf `Transaction.raw_data { contract: [TriggerSmartContract { owner_address, contract_address, call_value: 0, data: calldata, call_token_value: 0, token_id: 0 }], ref_block_bytes, ref_block_hash, expiration, timestamp, fee_limit: <sized in Task 10>, data: b"" }` — **`data` is field 4 (NOT 3)** — off-by-one hazard flagged
- [ ] Step 4: Test `transaction_roundtrip.rs` — `RawData::encode_to_vec()` + `RawData::decode()` round-trip for hand-crafted TransferContract + TriggerSmartContract
- [ ] Step 5: **Audit ship-gate** — verify controls **P2-6** (`build_trx_transfer` rejects `to_21 == [0u8; 21]` and `0x41 ++ [0u8; 20]` burn address), **P2-7** (`build_trx_transfer` asserts `fee_limit == 0` AND `contract == TransferContract`; `TriggerSmartContract` asserts `fee_limit > 0`), **P2-8** (`expiration = node_timestamp + 60s` via `walletsolidity/getnowblock`; NOT local `Instant::now()`), **P2-9** (end-to-end protobuf fixture: commit hex dump of real TRC-20 transfer; `cargo test` asserts round-trip). See [audit doc §Phase 2](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 6: Commit

## Phase 3 — TRC-20 stablecoin transfer + resource model UX (4 tasks)

### Task 9 (#4XX): TRC-20 ABI encoder (Q3 hand-rolled)

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/src/trc20.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/tests/trc20_calldata.rs`

**Interfaces:**
- `tron_wallet_core::trc20::encode_transfer(to_20: &[u8; 20], value: u128) -> [u8; 68]` — selector `0xa9059cbb` + padded address + padded uint256
- `tron_wallet_core::trc20::encode_balance_of(holder_20: &[u8; 20]) -> [u8; 36]` — selector `0x70a08231` + padded address
- `tron_wallet_core::trc20::encode_decimals() -> [u8; 4]` — selector `0x313ce567`

**Steps:**
- [ ] Step 1: Implement `encode_transfer()` — `[0xa9, 0x05, 0x9c, 0xbb] ++ pad_left_32(to_20) ++ pad_left_32(&value.to_be_bytes())`
- [ ] Step 2: Implement `encode_balance_of()` + `encode_decimals()`
- [ ] Step 3: Test `trc20_calldata.rs` — selector at bytes 0..4 == `0xa9059cbb`, total length 68 bytes; round-trips against `alloy-sol-types` standalone reference (`sol! { function transfer(address to, uint256 value) external returns (bool); }` + `transferCall.abi_encode()` produces identical bytes per Agent B finding)
- [ ] Step 4: **Audit ship-gate** — verify controls **P3-1** (ABI encoder byte-equals `alloy_sol_types::sol!` for 100 random `(addr, value)` pairs — property test), **P3-3** (`$TRON_TOKEN_REGISTRY` env var + `--token-registry <path>` flag override compile-time bundle). See [audit doc §Phase 3](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 5: Commit

### Task 10 (#4XX): Resource model — energy estimation + fee_limit sizing + DEM awareness (Q5)

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/src/resource.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/tests/resource_model.rs`

**Interfaces:**
- `tron_wallet_core::resource::estimate_energy(client: &Client, sender_21: &[u8; 21], token_21: &[u8; 21], calldata: &[u8]) -> Result<EnergyEstimate, Error>` — calls `triggerconstantcontract`, returns `{ energy_used, energy_penalty? }`
- `tron_wallet_core::resource::get_energy_price_sun(client: &Client) -> Result<u64, Error>` — calls `getchainparameters`, returns `getEnergyFee` value
- `tron_wallet_core::resource::get_dem_factor(client: &Client, contract_21: &[u8; 21]) -> Result<f64, Error>` — calls `getcontractinfo`, returns `energy_factor / 10000`
- `tron_wallet_core::resource::size_fee_limit(energy_estimate: u64, sun_per_energy: u64, dem_factor: f64) -> u64` — `energy_estimate * sun_per_energy * dem_factor * 11/10` (DEM buffer)
- `tron_wallet_core::resource::ResourceSnapshot { bandwidth_free: u64, bandwidth_staked: u64, bandwidth_used: u64, energy_free: u64, energy_staked: u64, energy_used: u64, trx_power: u64 }`

**Steps:**
- [ ] Step 1: Implement `estimate_energy()` — POST to `/wallet/triggerconstantcontract` with `function_selector: "transfer(address,uint256)"` + `parameter: <ABI-encoded to+value>` + `visible: true`. Parse response `result.energy_used` (required) + `result.energy_penalty` (optional — present in many node versions per Agent A, undocumented in OpenAPI schema). **Response-shape note (corrected 2026-08-27 via #410):** TronGrid returns `{ "constant_result": ["<hex>"], "result": { "result": true } }` for `balanceOf(address)` — **parse `constant_result[0]` for the balance; the nested `result.result` is a boolean success flag, NOT a hex balance**. Fall back to `result.result` only when `constant_result` is missing AND it parses as a hex string. Verified live in #409 spike (`rust-wallet-app/spikes/tron-v1/src/tx.rs:135`).
- [ ] Step 2: Implement `get_energy_price_sun()` — POST to `/wallet/getchainparameters`, parse `getEnergyFee` field. Default 100 sun/Energy — re-query each call (governance can change)
- [ ] Step 3: Implement `get_dem_factor()` — POST to `/wallet/getcontractinfo`, parse `energy_factor` (scaled ×10,000; divide by 10,000 for multiplier). Mainnet `max_factor = 3.4` per Agent A finding.
- [ ] Step 4: Implement `size_fee_limit()` — buffer the estimate: `ceil(energy_used * sun_per_energy * max_factor * 1.1)` (DEM + 10% safety). **Hard cap: 15,000,000,000 sun = 15,000 TRX** (`getMaxFeeLimit` chain parameter #47). **UNITS: sun, not TRX** — footgun flagged (Q5)
- [ ] Step 5: Implement `get_resource_snapshot()` — POST to `/wallet/getaccount`, parse frozen + free bandwidth/energy + TRON Power
- [ ] Step 6: Test `resource_model.rs` — verify `estimate_energy` returns 65k–130k range for USDT-TRC20 transfer (lower bound when recipient holds USDT, upper bound for empty recipient); verify DEM factor round-trip; verify `size_fee_limit` produces sun units in correct range
- [ ] Step 7: **Audit ship-gate** — verify controls **P3-4** (post-build check: if `size_fee_limit < energy_used * sun_per_energy`, log warning + require `--yes-i-know`), **P3-5** (DEM factor fetched every call; reject hardcoded `3.4` — mainnet-only), **P3-6** (CLI prints `fee_limit = N SUN (X TRX)`; refuse user-supplied `fee_limit < 0.5 × computed`), **P3-7** (v0.1 ships read-only stake view via `getaccount`; refuse `freezeBalanceV2` mutation — deferred to v0.2). See [audit doc §Phase 3](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 8: Commit

### Task 11 (#4XX): Token registry loader + USDT/USDC decimals (Q9)

**Files:**
- Create: `rust-wallet-app/crates/tron-wallet-core/src/tokens.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/tokens/mainnet.json` (5 entries)
- Create: `rust-wallet-app/crates/tron-wallet-core/tokens/nile.json` (1 entry)
- Create: `rust-wallet-app/crates/tron-wallet-core/tests/token_registry.rs`

**Steps:**
- [ ] Step 1: Author `tokens/mainnet.json` with 5 entries per Q9 spec (USDT `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` 6 decimals, USDC `TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8` 6 decimals, TUSD `TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9` 18 decimals, USDD `TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz` 18 decimals, stUSDT `TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3` 6 decimals)
- [ ] Step 2: Author `tokens/nile.json` with 1 entry (community test USDT `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`)
- [ ] Step 3: Implement `tokens::load(network: Network) -> Vec<Token>` — reads bundled JSON, returns `Vec<Token { symbol, contract_t_address, decimals }>`
- [ ] Step 4: Implement `tokens::resolve_decimals(client: &Client, token: &Token) -> Result<u8, Error>` — calls `triggerconstantcontract` with `encode_decimals()` selector (cache result; Q5 decimals caching pattern)
- [ ] Step 5: Test `token_registry.rs` — 5 mainnet + 1 nile entries load; USDT decimals = 6 verified via `decimals()` call
- [ ] Step 6: **Audit ship-gate** — verify controls **P3-2** (decimals resolution **always** cross-checked against live chain before sign; refuse if `bundled != on-chain`), **P3-8** (each `tokens/*.json` entry has `provenance: {issue, pr, commit}` field at top of file). See [audit doc §Phase 3](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 7: Commit

### Task 12 (#4XX): Sign + broadcast TRC-20 transfer (end-to-end)

**Files:**
- Modify: `rust-wallet-app/crates/tron-wallet-core/src/wallet/sign.rs`
- Create: `rust-wallet-app/crates/tron-wallet-core/src/provider.rs`

**Interfaces:**
- `tron_wallet_core::wallet::sign::send_trc20_transfer(wallet: &WalletManager, wallet_id: WalletId, token: TAddress, to: TAddress, amount_human: Decimal, decimals: u8) -> Result<TxId, Error>`

**Steps:**
- [ ] Step 1: Implement `send_trc20_transfer()` — fetch TAPOS via `walletsolidity/getnowblock`, fetch sender nonce via `getaccount`, estimate energy via `triggerconstantcontract`, size `fee_limit` via DEM buffer, build `TriggerSmartContract` (data at field 4!), sign, broadcast. **Poll loop uses `/wallet/gettransactioninfobyid`** (NOT `/wallet/gettransactionbyid`) — see Task 7 Step 4a. The e2e flow exercised by #409's ship-gate demo `use_case_alpha_sends_beta_usdt` (logged 2026-08-27 via #410): (1) `getaccount` → sender balance + resource snapshot; (2) `walletsolidity/getnowblock` → TAPOS ref_block; (3) `triggerconstantcontract(balanceOf)` → confirm recipient holds USDT (sets lower-bound estimate); (4) `triggerconstantcontract(transfer)` → `energy_used` estimate; (5) `build_trc20_call` + `sign_raw_data` → SignedTx; (6) `broadcasttransaction` with FULL `{transaction:{raw_data, ...}}` body; (7) poll `gettransactioninfobyid` until `id == txID && receipt.result == "SUCCESS"` (≤120s); (8) final `triggerconstantcontract(balanceOf)` confirms recipient balance reflects the transfer.
- [ ] Step 2: Implement `provider::tron_provider(rpc_url: &str) -> TronProvider` — explicit nonce + explicit fee (NO fillers — parallel to eth Q4 decision)
- [ ] Step 3: Implement error retry on `BANDWIDTH_INSUFFICIENT` + `OUT_OF_ENERGY` (return structured error so CLI can suggest stake)
- [ ] Step 4: Tests on Nile testnet + MockTRC20 (TronBox regtest) — `send_trc20_transfer` succeeds; recipient `balanceOf` reflects change
- [ ] Step 5: **Audit ship-gate** — verify controls **P3-9** (idempotency: refuse double-sign in same 60s window — sender nonce + ref_block), **P3-10** (`BANDWIDTH_INSUFFICIENT` / `OUT_OF_ENERGY` returned as structured error; **no auto-retry** — surface to user + suggest stake). See [audit doc §Phase 3](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 6: Commit

## Phase 4 — `tron` CLI + smoke + release cut (3 tasks)

### Task 13 (#4XX): `tron` CLI scaffold + wallet commands

**Files:**
- Create: `rust-wallet-app/crates/tron/Cargo.toml`
- Create: `rust-wallet-app/crates/tron/src/main.rs`

**Steps:**
- [ ] Step 1: Add `tron` binary to umbrella `members`
- [ ] Step 2: Implement clap subcommands: `tron wallet create --name w --network mainnet|nile`, `tron wallet import`, `tron wallet list`, `tron wallet show --name w`, `tron wallet delete --name w`
- [ ] Step 3: Add `--trongrid-api-key` global flag (Q6 — raises rate limit 3 QPS → 15 QPS)
- [ ] Step 4: Tests: CLI integration tests cover create + import + list + show + delete (parallel to btc-import-demo pattern)
- [ ] Step 5: **Audit ship-gate** — verify control **P4-2** (`tron wallet list --redact` flag; default off in v0.1, default on in v0.2). See [audit doc §Phase 4](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 6: Commit

### Task 14 (#4XX): `tron send` subcommand + Nile smoke

**Files:**
- Modify: `rust-wallet-app/crates/tron/src/main.rs`

**Steps:**
- [ ] Step 1: Add `tron send` subcommand — `tron send --wallet w --to T... --amount 1.5 --token USDT --network nile` (or `--token native` for TRX)
- [ ] Step 2: Implement human amount → base units conversion via token registry (USDT 6 decimals: `1.5 * 10^6 = 1_500_000` base units)
- [ ] Step 3: Print per-resource breakdown before sign: `Energy: 65k (~$0.01) | Bandwidth: 345 (free) | Total: ~0.011 TRX` (MetaMask-TRON UX pattern per Q5)
- [ ] Step 4: Add `--dry-run` flag (build + sign only, no broadcast — Q8 sign-only path)
- [ ] Step 5: Add `--broadcast` flag (default true) for explicit opt-out
- [ ] Step 6: Smoke test on Nile testnet: `tron send --wallet w --to T... --amount 1 --token USDT --network nile --dry-run` produces valid signed tx; without `--dry-run` broadcasts and `txID` returned
- [ ] Step 7: **Audit ship-gate** — verify controls **P4-3** (`tron send --dry-run` does **not** log signed tx bytes; only shows txID + ready-to-broadcast prompt), **P4-4** (mainnet smoke only via `<env flag>` opt-in; default CI smoke on `nile.trongrid.io`). See [audit doc §Phase 4](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 8: Commit

### Task 15 (#4XX): Mainnet smoke + release cut

**Files:**
- Modify: `rust-wallet-app/crates/tron-wallet-core/Cargo.toml`
- Modify: `rust-wallet-app/crates/tron/Cargo.toml`
- Create: `rust-wallet-app/crates/tron-wallet-core/CHANGELOG.md`

**Steps:**
- [ ] Step 1: Smoke test on TronGrid mainnet with SPKI pin (Scenario A — `pinned://<hex>@api.trongrid.io`); verify `eth_chainId` returns `0x2b6653dc`; send 0.001 TRX to self — succeeds, balance reflects change
- [ ] Step 2: Bump `tron-wallet-core` to `0.1.0` + `tron` to `0.1.0` in respective `Cargo.toml`
- [ ] Step 3: Author `CHANGELOG.md` entry: "v0.1.0 — Initial release. Mnemonic HD wallet (BIP-39 + BIP-32 m/44'/195'/0'/0/0), raw reqwest JSON-RPC, protobuf tx construction via prost 0.14.4, TRC-20 transfer (USDT, USDC, TUSD, USDD, stUSDT), Stake 2.0 fee display, SPKI pin support via bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier."
- [ ] Step 4: Update `rust-wallet-app/README.md` to mention `tron` CLI alongside `btc` and `eth`
- [ ] Step 5: **Audit ship-gate** — verify controls **C9** (`SECURITY.md` + `cargo-cyclonedx` SBOM + `cargo audit` report committed at release), **P4-1** (`--trongrid-api-key-file <path>` or env `TRON_PRO_API_KEY` only; **never** as CLI flag), **P4-5** (release workflow generates SBOM `cargo-cyclonedx` + commits `cargo audit` JSON to `SECURITY.md`), **P4-6** (`SECURITY.md` with `security@…` contact + 90-day disclosure window), **P4-7** (`--debug` output goes to stderr-only; refuse `--debug --broadcast` combination). See [audit doc §Phase 4](../audit/2026-08-27-tron-wallet-core-security-audit.md).
- [ ] Step 6: Final commit + tag `tron-wallet-core-v0.1.0` + `tron-cli-v0.1.0`

## Spike closure (V1–V10 acceptance)

After Phase 0–4 ship, the `rust-wallet-app/spikes/tron-v1/` spike produces PASS evidence for V1–V10 (one per Q, mapping in §"Spike mapping" above). The spike tree is per-Vn granular (see §"File Structure" — `src/` library surface + `tests/v{1..10}_*.rs` per-Vn integration tests + `RESULT.md` evidence log), not a single `main.rs` runner — each Vn is independently runnable for fast iteration + surgical debugging.

### Per-Vn run protocol

```bash
# Offline Vns (always run — no network needed)
cargo test -p tron-spike-v1 --test v1_compile            # V1: workspace compile check
cargo test -p tron-spike-v1 --test v2_protobuf_roundtrip # V2: prost encode/decode round-trip
cargo test -p tron-spike-v1 --test v3_trc20_abi         # V3: hand-rolled ABI encoder output
cargo test -p tron-spike-v1 --test v4_base58check       # V4: base58check round-trip + canonical vector
cargo test -p tron-spike-v1 --test v10_slip44          # V10: SLIP-44 canonical mnemonic → T-address

# Gated Vns (require RUN_TRON_NILE=1 — live testnet access)
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test v5_resource          # V5: triggerconstantcontract → energy_used
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test v6_nile             # V6: eth_chainId via /jsonrpc + TAPOS
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test v7_spki_pin         # V7: SpkiPinnedVerifier accept/reject
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test v8_sign_only        # V8: local-sign TRX transfer + txID verify
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test v9_token_registry   # V9: tokens/*.json + USDT decimals=6 verify

# All Vns at once
cargo test -p tron-spike-v1 --test '*'                                    # offline only
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test '*'                    # offline + gated
```

### PASS evidence requirements (issue #399 acceptance)

Each Vn must produce:
- **Command output:** the `cargo test` stdout/stderr showing the test pass.
- **SHA:** the git SHA of the commit that added/ran the test (per L13 review trail).
- **Recorded in:** `rust-wallet-app/spikes/tron-v1/RESULT.md` — one section per Vn with the raw command + output + SHA.

When all 10 Vns pass, issue #399 acceptance criterion "All 10 open questions either answered (with chosen path + rationale) or explicitly deferred to v0.2+ with rationale" can flip `[x]` — the deep-dive resolves Q1-Q5 with citations + Q6-Q10 resolved by the spike's PASS evidence (NOT deferred to v0.2+).

## Out of scope (per issue #399 body, deferred unless explicitly added)

- TRC-10 token transfers (separate `TransferAssetContract` proto encoding)
- Smart-contract deployment via wallet (sign-only + broadcast external path is enough)
- Stake/unstake/freeze resource delegation (`FreezeBalanceV2Contract` proto encoding separate — v0.2+)
- Multi-sig / governance flows
- TRON-specific DEX integration (SunSwap, etc.)
- Hardware wallet support (Ledger/Trezor) — same deferral as eth #293
- L2s / EVM-compatible sidechains (Ethereum side already deferred per eth #293)

## Dependencies

- **Issue body:** #399 deliverables 1 (deep-dive, [x] in PR #402), 2 (user-stories), 3 (this plan), 4 (spike).
- **Prior plans:**
  - `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` (v0.1 — SPKI pin pattern source, `bitcoin-wallet-core/src/chain/spki.rs`)
  - `docs/superpowers/plans/2026-08-23-eth-wallet-core.md` (v0.2 — async test policy, alloy-sol-types reference, CLI structure)
- **Deep-dive:** `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md` (every Q resolution cited)
- **User-stories (Ticket B):** `docs/wallets/2026-08-27-tron-wallet-user-stories.md` (use-case matrix per eth template)
- **Workspace deps to add (Phase 0 Step 2):** `prost = "0.14.4"`, `prost-types = "0.14.4"`, `bs58 = "0.5"`, `tiny-keccak = "2.0.2"`. Build dep: `prost-build = "0.14.4"`. CI requires `protoc ≥3.12` in PATH.
- **Vendored proto:** `core/Tron.proto` at SHA `851575d` (2026-07-14) — re-pin if upstream `develop` branch ships a breaking schema change before Phase 2 ships.

## References

- TRON Rust SDK deep-dive: `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md` (PR #402)
- Bitcoin SPKI pin pattern: `bitcoin-wallet-core/src/chain/spki.rs` (F20 / Task 7 of Bitcoin plan)
- eth-wallet-core deep-dive (companion): `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md`
- eth-wallet-core plan (template source): `docs/superpowers/plans/2026-08-23-eth-wallet-core.md`
- Bitcoin plan (pattern source): `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`
- SLIP-0044 (TRON coin type 195): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- Issue #399: <https://github.com/nhitranbtc/blockchain-sdk/issues/399>
- PR #402: <https://github.com/nhitranbtc/blockchain-sdk/pull/402>
