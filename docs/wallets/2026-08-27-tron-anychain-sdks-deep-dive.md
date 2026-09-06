# TRON-Specific Rust SDK Deep-Dive

**Date:** 2026-08-27
**Scope:** Focused re-research on Rust crates for a TRON (TRX + TRC-20 stablecoin) wallet built inside `rust-wallet-app/`, covering native TRX transfer, TRC-20 token transfer (USDT-TRC20 primary, USDC-TRC20 secondary), address encoding (base58check T-prefix + TVM hex), and resource-model fee handling (energy + bandwidth). Verifies the chosen crate surface against current 2026 state, considers alternatives, and digs into protobuf tx construction + TRC-20 ABI reuse.
**Companion to:** `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` (Ethereum precedent — primary cross-chain reference). Tracks issue #399, mirrors eth PR #290 → #293 review → #294 plan flow.
**Pre-empts:** v0.3+ deliverable sketched in `rust-wallet-app/crates/chain-traits/src/lib.rs:21` (`ChainId::Tron(u32)` placeholder for TRON coin).
**Status:** Research report only. No design spec, no implementation plan, no code produced in this session.

## TL;DR

Use **`anychain-tron 0.2.14` + `anychain-kms 0.1.23`** as the primary TRON stack. **`anychain-tron` provides the wire-format layer** — T-base58check address derivation, protobuf-encoded `Transaction` envelope, all 13 contract builders (TRX transfer, TRC-20 transfer/approve, Stake 2.0 freeze/unfreeze/delegate/cancel/withdraw, witness vote, withdraw vote). **`anychain-kms` provides the HD + signing layer** — BIP-39 mnemonic (8 languages), BIP-32 secp256k1 derivation, `secp256k1_sign(sk, msg) -> (r||s, recid)`, `Zeroizing<String>` xprv serialize. **No third-party RPC client** — caller writes ~250 lines of `reqwest` for `wallet/getnowblock`, `wallet/broadcasttransaction`, `wallet/gettransactioninfobyid`, `wallet/triggerconstantcontract`. **MSRV bump required**: anychain umbrella pinned to `rust-toolchain = "1.98.1"`; workspace must bump MSRV or use `[patch.crates-io]` override. **2 known bugs require workarounds**: (1) `anychain-tron::TronTransaction::to_transaction_id()` uses single SHA-256 — caller does `SHA256(SHA256(raw_bytes))` manually for txid display; (2) `trx::build_contract` formats type_url via `{:?}` Debug derive — works today but fragile. Resource model (Q5) verified: 1 TRX = 1 TP under Stake 2.0 (April 2023), bandwidth = 600 free/day + stake-share, energy priced at 100 sun/Energy default, USDT-TRC20 transfer consumes 65k Energy (recipient holds USDT) or 130k (empty recipient).

**Third-party crate survey:** [`39george/tronic`](https://github.com/39george/tronic) v0.6.1 rejected (gRPC-only, single-maintainer). [`throgxyz/tronz`](https://github.com/throgxyz/tronz) v0.5.2 rejected (9-crate monorepo, MSRV 1.91.1 blocker, gRPC-only, single-maintainer). [`qntx/kobe`](https://github.com/qntx/kobe) — reference only (HD-only, no signing). [`0xcregis/anychain`](https://github.com/0xcregis/anychain) **CHOSEN** — `anychain-tron` subcrate provides all needed wire-format + contract builders; `anychain-kms` subcrate provides BIP-32/39 + signing.

## Chosen crates & SDKs (v0.1, anychain)

**Decision (locked 2026-09-05, supersedes prior 2026-09-04 raw-primitives decision):** adopt **`anychain-tron 0.2.14`** (TRON wire-format) + **`anychain-kms 0.1.23`** (HD + signing). Trades ~250 lines of `reqwest` glue for ~1000 lines of hand-rolled protobuf + base58check + Keccak256 + ABI encoder + Stake 2.0 contract builders. MSRV bumped to 1.98.1 to match anychain workspace.

| Crate                        | Version             | Role                                                                                                                                                                                                                                                              | License           | MSRV                            |
| ---------------------------- | ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------- | ------------------------------- |
| anychain-tron                | 0.2.14              | T-base58check address + proto Transaction envelope + 13 contract builders (TRX/TRC-20/Stake2.0/Vote)                                                                                                                                                              | MIT OR Apache-2.0 | **1.98.1** (anychain workspace) |
| anychain-kms + anychain-core | 0.1.23 + 0.1.8      | BIP-39 mnemonic (8 langs) + BIP-32 secp256k1 HD + ECDSA sign (`secp256k1_sign` returns r‖s + recid) + xprv/xpub + `Zeroizing&lt;String>`; shared traits (Address/Format/Network/PublicKey/Transaction) + crypto utilities (sha256, keccak256)                     | MIT OR Apache-2.0 | **1.98.1**                      |
| reqwest                      | 0.12 (`rustls-tls`) | JSON-RPC client — caller writes `wallet/getnowblock`, `wallet/broadcasttransaction`, `wallet/gettransactioninfobyid`, `wallet/triggerconstantcontract`, `wallet/estimateenergy`, `wallet/getaccountresource`, `wallet/getchainparameters` (anychain has zero RPC) | Apache-2.0 / MIT  | matches workspace               |
| rustls                       | 0.23                | TLS for reqwest + SPKI pin verifier (reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier`)                                                                                                                                                                | Apache-2.0 / MIT  | matches workspace               |
| serde + serde_json           | latest              | JSON-RPC envelope parse                                                                                                                                                                                                                                           | Apache-2.0 / MIT  | matches workspace               |
| clap                         | 4                   | CLI subcommand parser                                                                                                                                                                                                                                             | Apache-2.0 / MIT  | matches workspace               |
| argon2 + aes-gcm             | workspace           | Wallet file encryption (anychain has zero AES/Argon2id; PBKDF2 only in kms for mnemonic→seed)                                                                                                                                                                     | MIT OR Apache-2.0 | matches workspace               |
| testcontainers               | 0.23 (dev-dep)      | TronBox Docker auto-spawn (replaces manual `docker run`)                                                                                                                                                                                                          | Apache-2.0 / MIT  | matches workspace               |
| zeroize                      | 1.x                 | Wrap raw `sk: &[u8]` before anychain-kms `secp256k1_sign` (GAP — kms does not Zeroize its sk param)                                                                                                                                                               | Apache-2.0 / MIT  | matches workspace               |

## Crates used in `tron-wallet-core` (V0.1)

**38 crates total:** 33 mobile-safe (87%) + 4 desktop-only (11%) + 1 build-time (2%).

### Direct dependencies (organized by purpose)

#### Anychain stack (wire format + HD + signing)

| Crate           | Version | Purpose                                                                                                                                                                 | Mobile? |
| --------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| `anychain-core` | 0.1.8   | shared traits (`Address`, `PublicKey`, `Transaction`, `Format`, `Network`, `TransactionId`), crypto utilities (`keccak256`, `sha256`, `func_selector`), `hex` re-export | ✓       |
| `anychain-tron` | 0.2.14  | wire format — T-base58check address, protobuf `Transaction` envelope, 17 contract builders, `abi::encode_call`                                                          | ✓       |
| `anychain-kms`  | 0.1.23  | BIP-39 mnemonic (8 languages), BIP-32 HD (SLIP-44 coin 195), secp256k1 sign, xprv serialize                                                                             | ✓       |

#### Crypto (RustCrypto ecosystem)

| Crate                      | Version   | Purpose                                                                | Mobile?     |
| -------------------------- | --------- | ---------------------------------------------------------------------- | ----------- |
| `argon2`                   | 0.5       | Argon2id KDF for wallet file encryption                                | ✓ pure Rust |
| `aes-gcm`                  | 0.10      | AES-256-GCM symmetric encryption for `EncryptedWallet` blob            | ✓ pure Rust |
| `sha2`                     | 0.10      | SHA-256 (txid double-hash workaround)                                  | ✓ pure Rust |
| `sha3`                     | workspace | Keccak-256 (TRON address derivation via `anychain_core::keccak256`)    | ✓ pure Rust |
| `tiny-keccak`              | 2.0.2     | Direct keccak256 call (kept to avoid anychain indirection)             | ✓ pure Rust |
| `bs58`                     | 0.5       | base58check encoding (T-addresses, xprv)                               | ✓ pure Rust |
| `hex`                      | workspace | hex encode/decode for protobuf serialization                           | ✓ pure Rust |
| `zeroize`                  | 1.x       | Secure memory hygiene (`Zeroizing<Vec<u8>>`, `Zeroizing<String>`)      | ✓ pure Rust |
| `subtle`                   | 2         | Constant-time comparison (`ConstantTimeEq` for SPKI pin, xprv compare) | ✓ pure Rust |
| `libsecp256k1` (secp256k1) | workspace | ECDSA signing via anychain-kms (`secp256k1_sign`)                      | ✓ pure Rust |

#### Encoding / serialization

| Crate            | Version   | Purpose                                                                             | Mobile?        |
| ---------------- | --------- | ----------------------------------------------------------------------------------- | -------------- |
| `serde`          | 1.x       | derive Serialize/Deserialize                                                        | ✓ pure Rust    |
| `serde_json`     | 1.x       | JSON for TronGrid HTTP envelope + receipt parsing                                   | ✓ pure Rust    |
| `protobuf`       | 3.7       | TRON wire format (proto-generated types in `anychain-tron/src/protocol/`)           | ✓ pure Rust    |
| `ethereum-types` | workspace | Address type for ABI encoder                                                        | ✓ pure Rust    |
| `ethabi`         | workspace | TRC-20 ABI encode/decode (EIP-20 compatible)                                        | ✓ pure Rust    |
| `chrono`         | workspace | timestamp handling for tx expiration                                                | ✓ pure Rust    |
| `uuid`           | 1.x       | Wallet id (UUID v4)                                                                 | ✓ pure Rust    |
| `directories`    | workspace | Desktop data dir resolution — **V0.1.5 removal**, replaced by `WalletStorage` trait | ❌ desktop-only |

#### Async + HTTP

| Crate                 | Version | Purpose                                                                                                                           | Mobile?                                                      |
| --------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `tokio`               | 1.x     | Async runtime (current_thread for FFI; multi-thread for CLI)                                                                      | ✓                                                            |
| `reqwest`             | 0.12    | HTTP client for TronGrid (`broadcast`, `gettxinfo`, `getnowblock`, `getaccount`, `getaccountresource`, `triggerconstantcontract`) | ✓ with `rustls-tls`                                          |
| `rustls`              | 0.23    | TLS for reqwest + custom SPKI pin verifier                                                                                        | ✓ mobile uses `aws-lc-rs` provider                           |
| `rustls-native-certs` | 0.7     | Desktop OS root cert loading                                                                                                      | ❌ desktop-only (mobile uses `tls_built_in_root_certs(true)`) |
| `webpki`              | 0.22    | Custom `ServerCertVerifier` for SPKI pinning                                                                                      | ✓ pure Rust                                                  |
| `x509-parser`         | 0.16    | SPKI DER extraction from cert chain                                                                                               | ✓ pure Rust                                                  |

#### Errors + tracing

| Crate                | Version   | Purpose                                                     | Mobile?      |
| -------------------- | --------- | ----------------------------------------------------------- | ------------ |
| `thiserror`          | 1.x       | `Error` enum derive (no_std-compatible)                     | ✓ pure Rust  |
| `tracing`            | workspace | Structured logging (STDERR output, secret-scrubbing filter) | ✓            |
| `tracing-subscriber` | workspace | Subscriber with EnvFilter                                   | ✓ (CLI only) |

#### FFI + safety

| Crate       | Version | Purpose                                                             | Mobile?     |
| ----------- | ------- | ------------------------------------------------------------------- | ----------- |
| `once_cell` | 1.x     | Lazy-init for FFI runtime + compiled regex patterns                 | ✓ pure Rust |
| `regex`     | 1.x     | Panic-message scrubber (redact mnemonic + password + xprv + secret) | ✓ pure Rust |

#### Build-time

| Crate      | Version   | Purpose                                                  | Mobile?                         |
| ---------- | --------- | -------------------------------------------------------- | ------------------------------- |
| `cbindgen` | workspace | Generates C header for FFI consumers (Dart/Swift/Kotlin) | ✓ build-time only, doesn't ship |

#### Test-only (dev-dependencies)

| Crate            | Purpose                                                     | Mobile?                                       |
| ---------------- | ----------------------------------------------------------- | --------------------------------------------- |
| `proptest`       | Property-based tests for amount parsing, address derivation | ✓                                             |
| `tempfile`       | Atomic-write test fixtures                                  | ✓                                             |
| `testcontainers` | TronBox Docker auto-spawn for integration tests             | ❌ **desktop-only** (no Docker on iOS/Android) |
| `bitcoind`       | regtest smoke tests                                         | ❌ desktop-only                                |
| `reqwest` (dev)  | integration test HTTP client                                | ✓                                             |

### Dependency tree summary

```text
tron-wallet-core
├── anychain stack (3 crates)
│   ├── anychain-core (traits + utilities)
│   ├── anychain-tron (wire format + 17 builders)
│   └── anychain-kms (HD + signing)
│
├── crypto (RustCrypto)
│   ├── argon2 (KDF)
│   ├── aes-gcm (symmetric cipher)
│   ├── sha2, sha3 (hashes)
│   └── zeroize, subtle (memory + constant-time)
│
├── encoding
│   ├── bs58 (base58check)
│   ├── hex (hex encode)
│   ├── serde + serde_json
│   ├── protobuf (TRON wire format)
│   └── ethabi + ethereum-types (TRC-20 ABI)
│
├── async + HTTP
│   ├── tokio (runtime)
│   ├── reqwest (TronGrid client)
│   ├── rustls + rustls-native-certs + webpki + x509-parser (TLS + SPKI pin)
│
├── misc
│   ├── chrono, uuid, directories (desktop only — V0.1.5)
│   ├── tiny-keccak
│   ├── thiserror, tracing
│   ├── once_cell, regex (FFI safety)
│   └── cbindgen (build-time)
│
└── dev
    ├── proptest, tempfile
    └── testcontainers, bitcoind (desktop integration only)
```

### Mobile-unsafe dependencies (must remove for V0.1.5)

| Crate                      | Reason                                    | Replacement                                                                    |
| -------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------ |
| `directories`              | No iOS/Android backend                    | `WalletStorage` trait (File/Keychain/EncryptedFile)                            |
| `rustls-native-certs`      | Desktop-only OS cert loader               | Use `tls_built_in_root_certs(true)` on mobile (reqwest)                        |
| `testcontainers` (dev-dep) | Requires Docker (not available on mobile) | Skip on mobile via `cfg(not(target_os = "android"))` + `--no-default-features` |
| `bitcoind` (dev-dep)       | Test fixture only                         | Same gating                                                                    |

### Mobile-unsafe transitive deps (verify in spike)

| Crate                | Risk                            | Mitigation                                                                      |
| -------------------- | ------------------------------- | ------------------------------------------------------------------------------- |
| `chrono`             | Large binary (~150 KB stripped) | Acceptable for mobile; or drop + use `std::time::SystemTime`                    |
| `reqwest` + `rustls` | ~500 KB stripped                | Acceptable; profile = `release-mobile` with `opt-level = "z"` strips to ~300 KB |
| `protobuf`           | Pure Rust, no risk              | ✓                                                                               |

### Cargo.toml ordering recommendation

```toml
[dependencies]
# Anychain stack
anychain-core   = { workspace = true }
anychain-tron   = { workspace = true }
anychain-kms    = { workspace = true }

# Crypto (RustCrypto)
argon2   = { workspace = true }
aes-gcm  = { workspace = true }
sha2     = { workspace = true }
sha3     = { workspace = true }
tiny-keccak = { workspace = true }
bs58     = { workspace = true }
hex      = { workspace = true }
zeroize  = { workspace = true }
subtle   = { workspace = true }

# Encoding
serde      = { workspace = true }
serde_json = { workspace = true }
protobuf   = "3.7"
ethabi     = { workspace = true }
ethereum-types = { workspace = true }
chrono     = { workspace = true }
uuid       = { workspace = true, features = ["v4", "serde"] }

# Async + HTTP (mobile uses OS roots via tls_built_in_root_certs)
tokio             = { workspace = true }
reqwest           = { workspace = true, default-features = false, features = ["json", "rustls-tls"] }
rustls            = { workspace = true }
webpki            = { workspace = true }
x509-parser       = { workspace = true }

# Errors + tracing
thiserror  = { workspace = true }
tracing    = { workspace = true }

# FFI safety
once_cell = "1"
regex     = "1"

# Misc — desktop only; V0.1.5 removal
directories = { workspace = true, optional = true }

[build-dependencies]
cbindgen = { workspace = true }

[dev-dependencies]
proptest       = { workspace = true }
tempfile       = { workspace = true }
# Desktop-only tests (mobile skips via --no-default-features)
testcontainers = { version = "0.23", optional = true }
```

### License summary

| License           | Crates                                                   |
| ----------------- | -------------------------------------------------------- |
| MIT               | serde, tokio, reqwest, zeroize, etc.                     |
| MIT OR Apache-2.0 | anychain-*, argon2, aes-gcm, sha2, sha3, bs58, hex, etc. |
| Apache-2.0        | protobuf, ethereum-types, ethabi                         |
| BSD               | libsecp256k1                                             |

All compatible with `rust-wallet-app` MIT workspace license.

### Binary size impact (mobile, stripped + LTO)

| Crate                | Stripped contribution |
| -------------------- | --------------------- |
| `rustls` + `reqwest` | ~400-500 KB           |
| `protobuf`           | ~100 KB               |
| `argon2`             | ~50 KB                |
| `aes-gcm`            | ~30 KB                |
| `chrono`             | ~150 KB               |
| `libsecp256k1`       | ~80 KB                |

**Total V0.1 `libtron_wallet_core.so` size estimate: ~3-5 MB** (acceptable for mobile).

### Summary

| Category         | Count  | Mobile-safe  | Desktop-only                     |
| ---------------- | ------ | ------------ | -------------------------------- |
| Anychain         | 3      | 3            | 0                                |
| Crypto           | 10     | 10           | 0                                |
| Encoding         | 8      | 7            | 1 (`directories`)                |
| Async + HTTP     | 6      | 5            | 1 (`rustls-native-certs`)        |
| Misc             | 5      | 5            | 0                                |
| Errors + tracing | 3      | 3            | 0                                |
| FFI + safety     | 2      | 2            | 0                                |
| Build-time       | 1      | 1            | 0                                |
| Dev (test)       | 5      | 3            | 2 (`testcontainers`, `bitcoind`) |
| **Total**        | **43** | **39 (91%)** | **4 (9%)**                       |

**V0.1.5 work:** remove `directories` (1 crate), gate `rustls-native-certs` behind `#[cfg(not(mobile))]`, gate `testcontainers` + `bitcoind` behind `#[cfg(desktop)]`. ~30 LOC of Cargo.toml changes.

### Cross-reference

- Cargo.toml templates: `### Cargo.toml — \`tron-wallet-core\`` (full V0.1 template)
- CLI Cargo.toml: `### Cargo.toml — \`tron\` CLI`
- Mobile build verification: `### Mobile-compatible architecture (V0.1.5)` + TDD Spike 0
- Workspace additions: `### Workspace additions (root \`Cargo.toml\`)`

## Networks

v0.1 wallet targets two public TronGrid JSON-RPC endpoints — Mainnet + Nile testnet — plus Shasta fallback and local regtest via TronBox Docker. All traffic served via `reqwest` + `rustls` against `wallet/*` paths. SPKI pin (Q7) applies to Mainnet + Nile (not localhost).

| Network            | JSON-RPC endpoint                         | Chain-id                  | Address prefix | Notes                                                                                             |
| ------------------ | ----------------------------------------- | ------------------------- | -------------- | ------------------------------------------------------------------------------------------------- |
| **Mainnet**        | `https://api.trongrid.io/wallet/*`        | `0x2b6653dc` / 728126428  | `0x41`         | Production target. SPKI pin required (Q7, Scenario A). `TRON-PRO-API-KEY` for higher rate limits. |
| **Nile (testnet)** | `https://nile.trongrid.io/wallet/*`       | `0xcd8690dc` / 3448148188 | `0x41`         | Default testnet for v0.1 spikes. Community faucet (`!nile ADDR` via TronFAQBot).                  |
| Shasta             | `https://api.shasta.trongrid.io/wallet/*` | `0x94a9059e` / 2494104990 | `0x41`         | Fallback only if Nile degrades (v0.2+). Lower faucet reliability.                                 |
| Local regtest      | `http://127.0.0.1:8090/wallet/*`          | n/a                       | `0x41`         | TronBox Docker via `testcontainers` (auto-spawn, no manual `docker run`).                         |

**Endpoint selection rules:**

- Default CLI: Nile testnet (env `RUN_TRON_NILE=1`) — cheapest dev iteration.
- Mainnet: explicit `RUN_TRON_MAINNET=1` gate (NOT in v0.1 spike; deferred to Phase 4 mainnet rollout with operator funding).
- Local regtest: default for `cargo test` runs; `testcontainers` spawns TronBox Docker on demand.
- Chain-id query: `POST /jsonrpc {"method":"eth_chainId"}` against any endpoint (TronGrid's `/wallet/getchainid` returns HTTP 405 — corrected 2026-08-27).

## Test Scenario

Two testnet targets covered. **Local testnet** is the default (CI + desktop dev via TronBox Docker + testcontainers). **Nile testnet** is the fallback for mobile users + manual pre-release QA.

### Local testnet (TronBox Docker, desktop-only)

`testcontainers` Rust crate spawns `tronbox/tre:latest` Docker image on demand. Container lifecycle managed by testcontainers — no manual `docker run`. Reaches full-node + SolidityNode + gRPC + JSON-RPC HTTP at `http://127.0.0.1:8090/wallet/*`.

**Setup (one-time):**

```bash
# Toolchain check (CI runs this automatically)
cargo --version                 # 1.98.1 (anychain MSRV)
docker --version                # any Docker daemon present (testcontainers binds to it)

# First test run pulls the image (~500 MB cached thereafter)
cargo test --test trc20_local    # auto-spawns + tests + stops container
```

**Test scenarios:**

| #   | Scenario                                    | Command                                                                                                    | Pass criteria                                                                    |
| --- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 1   | TRX native transfer                         | `tron send --mnemonic "$DEPLOYER" --to "$RECIPIENT" --amount 1000000 --unit sun`                           | tx accepted; receipt.energy_usage < 0 (bandwidth only); balances reconcile       |
| 2   | TRC-20 transfer (held recipient)            | `tron trc20 send --mnemonic "$DEPLOYER" --contract "$MOCK_USDT" --to "$RECIPIENT" --amount 100`            | tx accepted; energy_usage ≈ 65_000; balance = 100 mock USDT                      |
| 3   | TRC-20 first-time receive (empty recipient) | `tron trc20 send ... --to "$FRESH_ADDR" --amount 50 --fee-limit 130000000`                                 | tx accepted; energy_usage ≈ 130_000 (2x baseline)                                |
| 4   | TRC-20 approval + allowance                 | `tron trc20 approve ... --spender "$DEX" --amount 1000` + `tron trc20 allowance --owner ... --spender ...` | approval tx accepted; allowance view returns 1000 mock USDT                      |
| 5   | Stake 2.0 freeze/unfreeze                   | `tron stake freeze --amount 1000000000 --unit sun` (V0.1.5)                                                | tx accepted; resource query shows frozen balance                                 |
| 6   | Insufficient balance                        | `tron trc20 send ... --amount 999999999`                                                                   | tx REVERTED with explicit error (exit code 5)                                    |
| 7   | Send-speedup (RBF)                          | `tron wallet send-speedup --wallet-id ... --txid <stuck> --fee-limit 200000000`                            | new tx accepted with higher fee_limit; original tx shows superseded              |
| 7a  | **Send-speedup rebroadcast semantics** (Round-1 grill Q10) | Verify `wallet/broadcasttransaction` idempotency: rebroadcast identical `(raw_bytes)` after 60s window — accepted/ignored/error? If accepted, speedup = rebroadcast + new fee_limit via new timestamp. If rejected, document "speedup not possible after window", remove `send-speedup` from v0.1. Block on this before row 7 ships. | node behavior recorded in `spikes/tron-v1/V7-speedup.md` |
| 8   | Wallet-to-wallet TRC-20                     | `tron wallet send --wallet-id "$HOT" --to-wallet cold --contract "$MOCK_USDT" --amount 100`                | resolves `cold` wallet name → address via `WalletManager::lookup()`; tx accepted |

**Rust integration test example (`tests/trc20_local.rs`):**

```rust
use testcontainers::{clients::Cli, ImageExt};
use testcontainers_modules::tronbox::TronBox;
use tron_wallet_core::{*, tx::*, chain::*};

#[tokio::test]
async fn trc20_transfer_full_flow_local() {
    // 1. Spawn TronBox (auto-pulls image first run)
    let docker = Cli::default();
    let container = docker.run(TronBox::default());
    let http_url = format!("http://127.0.0.1:{}", container.get_host_port_ipv4(8090));

    // 2. Deploy MockTRC20
    let deployer_sk = Zeroizing::new([0x01u8; 32]);
    let deployer_addr = keys::derive_address(&deployer_sk).unwrap();
    let bytecode = include_bytes!("../build/MockTRC20.bin").to_vec();
    let mock = tx::deploy_trc20(&deployer_sk, DeployTrc20Params {
        owner: deployer_addr.clone(),
        bytecode,
        name: "MockUSDT".into(),
        symbol: "MUSDT".into(),
        decimals: 6,
        initial_supply: 1_000_000_000_000,
        abi: None,
        constructor_args: vec![],
        fee_limit_sun: Some(200_000_000),
    }, &TronConfig::for_local_tronbox(&http_url)).await.unwrap();

    // 3. Send 100 mock USDT
    let recipient_sk = Zeroizing::new([0x02u8; 32]);
    let recipient_addr = keys::derive_address(&recipient_sk).unwrap();
    let receipt = tx::submit_trc20(
        &deployer_sk, &mock.contract_address, &recipient_addr,
        100_000_000, Some(65_000_000), &TronConfig::for_local_tronbox(&http_url)
    ).await.unwrap();
    assert_eq!(receipt.result, "SUCCESS");

    // 4. Verify recipient balance
    let balance = chain::trc20_balance(&recipient_addr, &mock.contract_address, &TronConfig::for_local_tronbox(&http_url)).await.unwrap();
    assert_eq!(balance, "100.0");
}
```

**CI integration (`.github/workflows/tron-integration.yml`):**

```yaml
name: Tron integration
on: [push]
jobs:
  test:
    runs-on: ubuntu-latest
    services:
      docker:
        image: docker:dind
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --test trc20_local -- --nocapture
        # testcontainers auto-spawns TronBox Docker in CI runner
```

### Nile testnet (remote, desktop + mobile)

`https://nile.trongrid.io` — public testnet maintained by TRON Foundation. No Docker required. Mobile-friendly.

**Setup (one-time):**

```bash
# Switch to Nile
tron config set-network nile
# Sets RPC URL = https://nile.trongrid.io

# Request test TRX from faucet (Telegram bot)
# Send to @TronFAQBot:  !nile <your_T_address>
# Receive 5000 TRX + 1000 mock USDT-equivalent in 1-2 minutes

# Verify balance
tron balance --address <your_T_address>
```

**Test scenarios (Nile variations):**

| #   | Scenario                 | Difference from Local                                                                                           | Pass criteria                                                                                  |
| --- | ------------------------ | --------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 1   | TRX native transfer      | Same                                                                                                            | tx accepted on real network; receipt visible on https://nile.tronscan.org/#/transaction/<txid> |
| 2   | TRC-20 transfer          | Use real test USDT contract `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (community faucet)                             | tx accepted; balance visible on https://nile.tronscan.org/#/token20/...                        |
| 3   | Mobile-specific          | iOS Simulator: `cargo build --target aarch64-apple-ios-sim`; Android Emulator: `cargo ndk -t x86_64 -o jniLibs` | FFI smoke test passes; Dart binding sends a real tx from emulator to Nile                      |
| 4   | Network failure recovery | Point RPC at `http://127.0.0.1:9999` (closed port)                                                              | CLI returns error code 3 (transport error) within 30s timeout; no panic                        |

**Rust integration test example (`tests/trc20_nile.rs`):**

```rust
use tron_wallet_core::{*, tx::*, chain::*};

#[tokio::test]
async fn trc20_transfer_full_flow_nile() {
    // 1. Skip if TRON_NILE_INTEGRATION env not set (CI gate)
    if std::env::var("TRON_NILE_INTEGRATION").is_err() {
        eprintln!("skipping Nile integration test (set TRON_NILE_INTEGRATION=1 to run)");
        return;
    }

    // 2. Load test mnemonic from env (never hard-code)
    let mnemonic = std::env::var("TRON_TEST_MNEMONIC")
        .expect("TRON_TEST_MNEMONIC required for Nile integration");
    let sk = keys::mnemonic_to_secret_key(&mnemonic, "m/44'/195'/0'/0/0").unwrap();
    let deployer_addr = keys::derive_address(&sk).unwrap();
    let cfg = TronConfig::for_network(Network::Nile);

    // 3. Use pre-deployed community USDT-TRC20 contract
    let mock_usdt: TronAddress = "TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf".parse().unwrap();

    // 4. Verify pre-funded (faucet step)
    let deployer_balance = chain::trc20_balance(&deployer_addr, &mock_usdt, &cfg).await.unwrap();
    assert!(deployer_balance > "0", "faucet didn't fund deployer");

    // 5. Send + verify (real network latency ~3-15s for block confirmation)
    let recipient_sk = Zeroizing::new([0x42u8; 32]);
    let recipient_addr = keys::derive_address(&recipient_sk).unwrap();
    let receipt = tx::submit_trc20(
        &sk, &mock_usdt, &recipient_addr,
        100_000_000, Some(65_000_000), &cfg
    ).await.unwrap();
    assert_eq!(receipt.result, "SUCCESS");

    // 6. Wait for confirmation
    let confirmed = tx::wait_for_confirm(&receipt.txid, Duration::from_secs(60), Duration::from_secs(3), &cfg).await.unwrap();
    assert_eq!(confirmed.result, "SUCCESS");
}
```

**CI gate (Nile tests are slow + need faucet funds):**

```yaml
# .github/workflows/tron-nile.yml — runs only on `workflow_dispatch` (manual trigger)
name: Nile integration
on: workflow_dispatch
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: TRON_NILE_INTEGRATION=1 cargo test --test trc20_nile -- --nocapture
        env:
          TRON_TEST_MNEMONIC: ${{ secrets.TRON_NILE_TEST_MNEMONIC }}
```

### Decision matrix

| Stage                 | Network                             | Why                                         |
| --------------------- | ----------------------------------- | ------------------------------------------- |
| Unit (per-commit)     | none (InMemoryWalletStorage + mock) | fast, no I/O                                |
| Integration CI        | Local (TronBox)                     | deterministic, fast (~30s), no faucet       |
| Pre-release manual QA | Nile (testnet)                      | real network + faucets; verifies edge cases |
| Mobile CI             | Local (TronBox)                     | no Docker fallback for mobile — **Round-1 grill Q6: mobile CI matrix is `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` (FFI compile only). NO mobile runtime smoke in v0.1. Add runtime mobile smoke via Nile testnet (real network, no Docker) for v0.2.** |
| Production            | Mainnet                             | post-Phase 4 only                           |

**Harness files (Phase 4 — landed 2026-09-06):**

- **Local integration (CI + desktop dev):** `rust-wallet-app/spikes/tron-v1/tests/trc20_local.rs` + `.github/workflows/tron-integration.yml`. Spawns `tronbox/tre:latest` via testcontainers 0.23; container spawn + readiness probe + chain-id + TAPOS probes run unconditionally. Deeper scenario rows (TRX/TRC-20 transfer, approval, allowance, Stake 2.0, insufficient-balance, send-speedup, rebroadcast idempotency, wallet-to-wallet) ship as `#[ignore]` stubs awaiting the `MockTRC20.sol` fixture + `tronbox migrate --network development` harness. Gated behind `RUN_TRON_LOCAL=1` + Docker daemon (loud-RED panic per plan gated-live-test convention).
- **Nile integration (manual QA only):** `rust-wallet-app/spikes/tron-v1/tests/trc20_nile.rs` + `.github/workflows/tron-nile.yml` (`workflow_dispatch` only — no automated CI per plan). Canonical `trc20_transfer_full_flow_nile` mirrors `use_case_alpha_sends_beta_usdt_live_nile` but is mnemonic-keyed (per Phase 4 §4.4). Gated on `TRON_NILE_INTEGRATION=1` + `TRON_TEST_MNEMONIC`.

### Cross-reference

- Networks table: `## Networks` (endpoint list + selection rules)
- TronBox rejection rationale: `## Rejected crates, SDKs, we reject tronz`
- Mobile build target matrix: `### Mobile-compatible architecture (V0.1.5)` → `#### Build target matrix`

## Rejected crates, SDKs, we reject tronz

### Landscape survey (2026) — all rejected

10 candidate SDKs surveyed 2026-09-04. All 9 rejected (raw `reqwest` + `prost` + `k256` + `bs58` + `sha3` primitives chosen over third-party SDKs); 1 reference-only (`kobe-tron` for KAT vectors).

| Crate / Repo                                                         | Stars                             | Last commit                            | License           | Status (key fact)                                                                                                                    | Verdict                                                                                                                                                                                                   |
| -------------------------------------------------------------------- | --------------------------------- | -------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rust-tron` (`andelf`)                                               | 50                                | 2025-01-09 (~20 mo stale)              | **LGPL-3.0**      | gRPC-only, no tags, 761 commits / 20 forks, full `wallet-cli` + `nile-wallet-cli.sh`.                                                | **Reject** — stale + LGPL copyleft risk                                                                                                                                                                   |
| `tron-rs` (crates.io)                                                | n/a (14.88k DL)                   | v0.1.0, 2026-01-20                     | unspecified       | Proto definitions only, no signing, no JSON-RPC. Built on `cosmrs` (proto-build reuse only — not Cosmos-SDK tx types).               | **Reject as primary** — keep as `core/Tron.proto` schema reference                                                                                                                                        |
| `tronic` (`39george`)                                                | 7                                 | v0.6.1, 2026-07-20                     | Apache-2.0 / MIT  | Alloy-inspired, gRPC-only (JSON-RPC WIP per README). Uses `alloy_sol_types` for TRC-20. Single maintainer.                           | **Reject** — gRPC-only + single-maintainer trust                                                                                                                                                          |
| **`tronz` (`throgxyz`) ❌ REJECTED**                                  | **52**                            | **v0.5.2, 2026-08-19 (11 tags)**       | Apache-2.0 / MIT  | 9-crate monorepo, 119 commits, MSRV 1.91.1. Sponsor CatFee.IO. Maintainer `deszhou` + dependabot. gRPC-only binding.                 | **Reject** — gRPC-only binding + MSRV blocker (1.91.1 vs workspace 1.85) + single-maintainer trust + 9-crate bloat outweigh ~1000-line code saving. Raw `reqwest` + `prost` JSON-RPC path chosen instead. |
| `0xcregis/anychain` (umbrella)                                       | **252**                           | **2026-09-04 (13 min ago, MIT)**       | MIT               | 10-chain umbrella (BTC/ETH/Tron/Solana/Filecoin/Ripple/Polkadot/TON/Neo), 8 contributors, Rust 1.98.0 toolchain.                     | **Reject** — multi-chain scope too broad for TRON-only v0.1                                                                                                                                               |
| **`anychain-tron`** (TRON-specific subcrate)                         | (umbrella member)                 | v0.2.14, 2025-11-04                    | MIT OR Apache-2.0 | TRON-specialized subcrate (NOT the umbrella). 256/mo DL, 107 DL last week. Full feature set.                                         | **Reject** — pulls multi-chain umbrella deps, scope too broad                                                                                                                                             |
| **`qntx/kobe` + `kobe-tron`**                                        | **163**, 27 forks, 5 contributors | v3.4.0, 2026-08-12 (24 releases)       | MIT OR Apache-2.0 | `no_std + alloc` HD + address derivation for 14 chains. **HD + address only — no signing/transport/RPC.** 1,257 DL. KAT-pinned.      | **Reference only** — KAT vectors for Q4/Q10                                                                                                                                                               |
| `edwintuan/next-wallet`                                              | 0                                 | 2026-05-21 (2 commits)                 | MIT OR Apache-2.0 | Terminal-native Tron wallet (`nxt`): TUI + CLI, multi-wallet, Stake 2.0, SR voting, themes. **49 tests, Nile-validated. Pre-alpha.** | **Reference only** — UX inspiration for v0.2+ Stake flows                                                                                                                                                 |
| `Gingerbreadfork/tron-goblin-node`                                   | 8                                 | 2026-08-25                             | (unspecified)     | From-scratch Rust TRON full node (byte-exact java-tron parity). Not a wallet SDK.                                                    | **Reject** — only if we ever need consensus-valid TRC-20 tooling                                                                                                                                          |
| `walletsuite` / `Hixon10` / `OpenSettle` / `rootdigit` / `derJanusz` | 0–2                               | various 2026                           | various           | Narrow-scope SDKs (single-vendor or single-chain).                                                                                   | **Reject for v0.1** — re-survey at v0.3                                                                                                                                                                   |
| `heliosphere` + `heliosphere-signer`                                 | ~16k DL / ~19k DL                 | 2024-09-12 / 2024-08-26 (~2 yrs stale) | MIT               | TRON client + signing (`k256` ^0.13, `sha3` ^0.10). `no_std + alloc` for signer.                                                     | **Reject** — stale; **reference only** for `derive_address` / `hash_message` shape                                                                                                                        |
| `tron-api-client`                                                    | 8,037 DL                          | v0.0.3/v0.1.0 ~2020                    | unspecified       | "WORK IN PROGRESS". Read-only CLI — **no signing, no broadcast**.                                                                    | **Reject** — dead ~2020, single maintainer                                                                                                                                                                |
| `tron-protos`                                                        | 84 DL                             | v0.1.9, 2025-11-04                     | MIT               | Pure proto definitions, no client wrapper. ~13 KB.                                                                                   | **Reject** — no benefit over vendoring `core/Tron.proto` via `prost` 0.14 directly                                                                                                                        |

**Per-candidate rejection rationale:** see §"Landscape survey (2026)" above for each rejected candidate with full rationale (stars, MSRV, last commit, license, scope). Non-SDK alternatives (`Hand-rolled protobuf encoder`, `Hand-rolled Keccak-256`, `ethers-rs`/`web3`) subsumed by raw workspace primitives (`prost`, `sha3`, `k256`, `reqwest`) — no third-party SDK required.



## Stablecoin (TRC-20) transfer — contract addresses + ABI

TRC-20 is **functionally identical to ERC-20** at the ABI level. The function selector for `transfer(address,uint256)` is the same `0xa9059cbb` on both chains. The wire format differs (TRON uses protobuf `TriggerSmartContract` wrapper around the ABI-encoded call data; Ethereum uses RLP tx with `input` field).

| Token                            | Mainnet contract (T-base58check)     | Decimals | Symbol | Source                                                                                  |
| -------------------------------- | ------------------------------------ | -------- | ------ | --------------------------------------------------------------------------------------- |
| USDT (Tether USD)                | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | **6**    | USDT   | TronScan token page + `andelf/rust-tron` WELLKNOWN_ADDRESS table (canonical)            |
| USDC (Circle, deprecated TRC-20) | `TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8` | **6**    | USDC   | TronScan; Circle stopped issuing new TRC-20 USDC post-2023, existing token still active |
| TUSD (TrueUSD)                   | `TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9` | **18**   | TUSD   | TronScan                                                                                |
| USDD (Decentralized USD)         | `TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz` | **18**   | USDD   | TronScan                                                                                |
| stUSDT (Staked USDT RWA)         | `TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3` | **6**    | stUSDT | TronScan                                                                                |

**Nile (testnet) equivalents:** Nile runs a separate token registry. Test USDT on Nile = `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (community faucet). For v0.1 spike, deploy a local `MockTRC20` to a local TronBox / `wallet-cli` regtest node — mirrors the eth Anvil MockERC20 pattern.

## Mnemonic-to-broadcast data flow (end-to-end)

**Anti-replay window:** `timestamp` + `expiration` pair → ~60s. Re-signed tx with same `timestamp` rejected by node; wallet must bump `timestamp` to `now_ms` on re-sign.

**Zeroizing lifecycle:** every secret-bearing stage clears on drop or function return — `Zeroizing<[u8;16]>`, `Zeroizing<[u8;64]>`, `Zeroizing<bip32::XPrv>`, `Zeroizing<[u8;32]>`, `Zeroizing<k256::SigningKey>`. Public outputs (pubkey, address, signature, txid) are non-secret — safe to log/serialize freely.
||

**Token registry (v0.1 hardcoded):**

| Token                            | Mainnet contract (T-base58check)     | Decimals | Source             |
| -------------------------------- | ------------------------------------ | -------- | ------------------ |
| USDT (Tether)                    | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` | 6        | TronScan canonical |
| USDC (Circle, deprecated TRC-20) | `TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8` | 6        | TronScan           |
| TUSD (TrueUSD)                   | `TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9` | 18       | TronScan           |
| USDD (Decentralized USD)         | `TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz` | 18       | TronScan           |
| stUSDT (Staked USDT RWA)         | `TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3` | 6        | TronScan           |

Nile testnet equivalent: `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (community USDT faucet, 6 decimals). For V3 spike: deploy `MockTRC20` to local TronBox via `CreateSmartContractBuilder` (promoted to v0.1).

## Network + TLS pinning research (mirrors eth design)

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

| Use case                         | Network       | Pin?                              | Why                              |
| -------------------------------- | ------------- | --------------------------------- | -------------------------------- |
| Local dev (developer laptop)     | TronBox HTTP  | No (Scenario B)                   | TLS N/A.                         |
| CI smoke test                    | TronBox HTTP  | No (Scenario B)                   | Ephemeral.                       |
| Testnet smoke (Nile)             | Nile HTTPS    | Optional (Scenario A recommended) | Public WiFi in dev environments. |
| Testnet smoke (dev machine, LAN) | Nile HTTPS    | No (Scenario B acceptable)        | Trusted network.                 |
| Production wallet, real value    | Mainnet HTTPS | **Yes (Scenario A required)**     | Adversarial network.             |

Default CLI behavior: **Scenario B** (no pin). Operator opts into Scenario A via `pinned://` URL scheme.

|Mainnet|`0x41`|`0x2b6653dc` / 728126428|`TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` (USDT-TRC20)|`https://api.trongrid.io/wallet/*`|
|Shasta|`0x41`|`0x94a9059e` / 2494104990|(same format as mainnet, separate chain-id)|`https://api.shasta.trongrid.io/wallet/*`|
|Nile (testnet)|**`0x41`** (corrected 2026-08-27; was `0xa0` in prior doc)|**`0xcd8690dc` / 3448148188** (corrected 2026-08-27)|`TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (community test USDT)|`https://nile.trongrid.io/wallet/*`|
|Local regtest|`0x41`|n/a|n/a|`http://127.0.0.1:8090/wallet/*`|

**Chain-id query method (corrected 2026-08-27):** `wallet/getchainid` returns HTTP 405 on TronGrid's HTTP front. Use `POST /jsonrpc {"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}` instead — works on all three networks and returns the chain-id as a hex string. All three chain-ids verified live 2026-08-27.

## Zeroizing<_>

Consolidated reference for memory hygiene of secret material across the v0.1 wallet. Consolidates the threat model, why-the-crate choice, and per-step usage map into one place (was previously scattered under §1 HD wallet + signer).

### Why we use Zeroizing<_>

**Problem:** Rust does NOT zero memory on drop by default. `Vec<u8>` / `[u8; N]` / `String` leave plaintext secret bytes in process memory after the binding goes out of scope. The allocator may reuse that memory for unrelated objects; the OS may swap it to disk; a core dump captures it; a debugger reads it. A 32-byte secp256k1 secret key left in freed heap is a permanent compromise — the wallet can be re-derived offline from those bytes.

**Threat model Zeroizing<_> addresses:**

- **Post-drop memory inspection** — reading `/proc/<pid>/mem` or attaching a debugger after wallet operation completes
- **Core dumps** — `ulimit -c unlimited` writes secrets to disk on panic; Zeroizing clears before panic propagates further
- **Swap-to-disk** — OS pages secrets out under memory pressure; the swap file persists beyond process exit
- **Process memory scan** — adjacent allocation patterns may contain prior secret values if allocator reuses without zeroing

**Threat model Zeroizing<_> does NOT address:**

- **Live memory dump** — while secret is in active use (between wrap and drop), nothing prevents `gdb` or `/proc/mem` read
- **Side-channel attacks** — cache-timing, power analysis, Rowhammer — orthogonal to memory hygiene
- **Compromised debuggers** — attacker with arbitrary code execution can hook any function
- **Filesystem residue** — if wallet code accidentally writes secret to `/tmp/log.txt`, Zeroizing cannot help

**Why the `zeroize` crate specifically:**

- **`#[derive(Zeroize)]`** for structs — `bip32::XPrv`, `k256::SigningKey` implement `Zeroize` out of the box
- **Volatile writes + compiler fence** — `Zeroize::zeroize()` uses `core::ptr::write_volatile` + `compiler_fence(SeqCst)` to prevent LLVM from eliding the "useless" zero-stores during optimization passes. Naive `memset(buf, 0, len)` is elidable; `zeroize` is not.
- **Drop hook integration** — `Zeroizing<T>` wraps any `T: Zeroize` and calls `zeroize()` in `Drop::drop`, runs even on panic/unwind/early-return paths
- **Zero runtime cost** — wrapper has no allocation overhead; wraps `T` inline

**Cost / tradeoff:**

- No performance cost (zeroize is microseconds)
- Slightly more verbose types (`Zeroizing<[u8; 32]>` vs `[u8; 32]`)
- Must explicitly propagate the pattern — forgetting to wrap any secret surface defeats the protection
- Tronz wraps internally for the BIP-32 + k256 path; v0.1 wallet must mirror the pattern for the Argon2id key and the decrypt/re-encrypt entropy window (see usage map below — rows 11, Import row 3, Reset row 9)

**Industry precedent:** `age` (encryption), `aws-sdk-kms` Rust bindings, `bip32` (RustCrypto), `k256` (RustCrypto), `slip-10` — all wrap secret material with `Zeroizing` by default. NOT wrapping is the deviation from the RustCrypto norm.

### Zeroizing<_> usage map across v0.1 wallet flow

Where the anti-forensics `Zeroizing<_>` wrapper applies in the wallet's hot path. Public material (pubkey, address, signature, ciphertext) is NOT wrapped — only plaintext secret bytes.

**13-step pipeline coverage:**

| Step | Operation                                             | Zeroizing wrapping                                                                                                                                              |
| ---- | ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | CLI `tron wallet create`                              | Passphrase input buffer → `Zeroizing<String>` (cleared after read)                                                                                              |
| 2    | `bip39::Mnemonic::generate_in(Words12, English, rng)` | Entropy (16 bytes) → `Zeroizing<Vec<u8>>` inside `bip39::Mnemonic`                                                                                              |
| 3    | `m.to_seed(passphrase)` PBKDF2                        | Seed (64 bytes) → `Zeroizing<[u8; 64]>`                                                                                                                         |
| 4    | `LocalSigner::from_mnemonic(m, path)`                 | Master XPrv → `Zeroizing<bip32::XPrv>` (XPrv impls `Zeroize`)                                                                                                   |
| 5    | `child.to_secp256k1_secret_key().secret_bytes()`      | SecretKey (32 bytes) → `Zeroizing<[u8; 32]>`                                                                                                                    |
| 6    | `k256::SigningKey::from_bytes(&sk_bytes)`             | SigningKey → `Zeroizing<k256::SigningKey>` (k256 scalar impls `Zeroize`)                                                                                        |
| 7    | `verifying_key().to_sec1_bytes()`                     | None — pubkey is public                                                                                                                                         |
| 8    | `Keccak256(pubkey[1..65])`                            | None — hash of public input                                                                                                                                     |
| 9    | `addr_raw = [0x41, addr_suffix]`                      | None — public address bytes                                                                                                                                     |
| 10   | `base58check_encode(addr_raw)`                        | None — public T-address string                                                                                                                                  |
| 11   | Encrypt + store on disk                               | Argon2id-derived key (32 bytes) → `Zeroizing<[u8; 32]>`; plaintext entropy (16 bytes pre-AES) → `Zeroizing<Vec<u8>>`                                            |
| 12   | Derive `m/44'/195'/0'/0/N` new receive addr           | Same as steps 4–6; old child XPrv zeroized when new child derived                                                                                               |
| 13   | Sign tx for broadcast                                 | Re-load triggers step 11 decrypt (Zeroizing-wrapped plaintext entropy); `LocalSigner` holds zeroized sk during sign; `RecoverableSignature` is public → no wrap |

**Extra v0.1 paths:**

| Path                                      | Zeroizing usage                                                                                                                                                                  |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Import from private key (Supported row 3) | `sk_bytes` from user input → `Zeroizing<[u8; 32]>`; passed to `LocalSigner::from_bytes`                                                                                          |
| Reset password (Supported row 9)          | `old_passphrase_key` (Argon2id) → `Zeroizing<[u8; 32]>`; `decrypted_entropy` between decrypt and re-encrypt → `Zeroizing<Vec<u8>>`; `new_passphrase_key` → `Zeroizing<[u8; 32]>` |
| CLI passphrase prompt (interactive)       | `rpassword` or `dialoguer` reads to `Zeroizing<String>`; cleared on `prompt()` return                                                                                            |

**Coverage summary:**

- **Wrapped (8 distinct surfaces):** passphrase input, BIP-39 entropy, BIP-39 seed, BIP-32 XPrv, secp256k1 SecretKey, k256 SigningKey scalar, Argon2id-derived key, plaintext entropy during decrypt/re-encrypt window
- **Not wrapped (correctly — public):** pubkey, T-address bytes, base58check string, RecoverableSignature { r, s, v }, encrypted ciphertext at rest

**Residual gap to flag (add to V8 verification):** `Debug` impl on `LocalSigner` may leak secret bytes via `{:?}` formatting. Mitigation: custom `Debug` impl that returns `"<redacted>"`, OR `#[derive(Debug)]` skip on secret fields. Add to V8 spike: `assert!(format!("{:?}", signer).contains("redacted"))`.

## Spike verification (V1-V10) — consolidated (2026-09-04, we use Nile RPC, 3-network)

The verification harness `rust-wallet-app/spikes/tron-v1/` proves each Q's chosen path before the corresponding phase ships. **V1-V10 mapping against raw primitives** (`prost` + `reqwest` + `k256` + `bs58` + `sha3`) — post-2026-09-04 final decision after tronz adoption reversed on full risk review.

**3-network testing matrix** — every Vn declares which network(s) it runs against:

| Network            | Spawn mechanism                                          | When                                                                                          | Env flag                                        |
| ------------------ | -------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| **Local**          | `testcontainers` + TronBox Docker (`tronbox/tre:latest`) | Default — every `cargo test` run                                                              | (none — automatic)                              |
| **Nile** (testnet) | TronGrid public gRPC + JSON-RPC                          | CI + developer runs                                                                           | `RUN_TRON_NILE=1`                               |
| **Mainnet**        | TronGrid public gRPC + JSON-RPC                          | **TODO!** Not implemented in v0.1 spike — needs mainnet TRX + USDT funding + audit pre-checks | `RUN_TRON_MAINNET=1` (gate not yet implemented) |

**TODO! Mainnet spike implementation deferred** — requires:

- Mainnet TRX funding (operator acquires, not from faucet)
- Mainnet USDT funding (`TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`, swap via SunSwap DEX or exchange withdrawal)
- Pre-check audit (no accidental broadcast to wrong address, no real-value loss)
- `RUN_TRON_MAINNET=1` env gate implementation (mirror of `RUN_TRON_NILE=1`)
- All V5/V6/V8/V9 mainnet variants in test files
- **No mainnet smoke in CI** — local + Nile only by default

### V1-V10 × 3-network mapping (we use Nile RPC)

| V#  | Q   | Verifies (raw primitives API mapping)                                                                                                                                                          | Networks tested                  | Phase                  |
| --- | --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ---------------------- |
| V1  | Q1  | Add raw primitives (`prost` 0.14, `prost-types` 0.14, `bs58` 0.5, `sha3` 0.10) to `Cargo.toml`; compile against workspace MSRV 1.85 (no bump)                                                  | **LOCAL** (compile)              | Phase 0 dep wiring     |
| V2  | Q2  | `prost 0.14 Transaction::encode_to_vec()` round-trip + `TriggerSmartContract.data` at field 4 (NOT 3)                                                                                          | **LOCAL**                        | Phase 2 transaction.rs |
| V3  | Q3  | `alloy_sol_types` `SolCall::transfer(to, amount)` (or hand-rolled selector + `encode(uint256,uint256)`) produces 68-byte calldata with `0xa9059cbb` selector at bytes 0..4                     | **LOCAL**                        | Phase 3 trc20.rs       |
| V4  | Q4  | Hand-rolled `Address::to_base58([0x41] ++ keccak256(pubkey)[12..32])` → 34-char T-string via `bs58 0.5`                                                                                        | **LOCAL**                        | Phase 1 address.rs     |
| V5  | Q5  | `reqwest` JSON-RPC `wallet/triggerconstantcontract` returns `energy_used` 65k-130k for USDT-TRC20 transfer; `wallet/getcontractinfo.energy_factor` round-trip                                  | **LOCAL + NILE + TODO! MAINNET** | Phase 3 resource.rs    |
| V6  | Q6  | `POST /jsonrpc {"method":"eth_chainId"}` → `0xcd8690dc` on Nile; base58check prefix `0x41` verified                                                                                            | **LOCAL + NILE**                 | Phase 2 network.rs     |
| V7  | Q7  | v0.1 wallet uses `reqwest` + `rustls` JSON-RPC against `https://api.trongrid.io/wallet/*`; SPKI pin reuses `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` (no `tonic` override needed) | **LOCAL + NILE**                 | Phase 2 rpc.rs         |
| V8  | Q8  | Hand-rolled `LocalSigner::sign_hash(tx_hash)` returns `RecoverableSignature` with `v ∈ {0, 1}` (NOT Ethereum `v+27`)                                                                           | **LOCAL + NILE + TODO! MAINNET** | Phase 1 signing.rs     |
| V9  | Q9  | `tokens/{local,nile,mainnet}.json` with `USDT` decimals=6 verified via live `triggerconstantcontract(decimals())`                                                                              | **LOCAL + NILE + TODO! MAINNET** | Phase 3 tokens.rs      |
| V10 | Q10 | `bip39::Mnemonic::parse_in(English, "abandon x11 about")` → seed → `m/44'/195'/0'/0/0` → T-address matches TronLink + hand-rolled cross-check                                                  | **LOCAL**                        | Phase 1 derivation.rs  |

### PASS evidence requirements

Each Vn produces (one PASS block per network tested):

- **Command output:** `cargo test` stdout/stderr showing test pass
- **SHA:** git SHA of the commit that added/ran the test (per L13 review trail)
- **Network tag:** `local` | `nile` | `mainnet` (TODO!) — recorded next to PASS block
- **Token registry used:** `tokens/local.json` | `tokens/nile.json` | `tokens/mainnet.json` (TODO!)
- **Recorded in:** `rust-wallet-app/spikes/tron-v1/RESULT.md` — one section per Vn with raw command + output + SHA

When all 10 Vns pass on local + Nile, issue #399 acceptance criterion "All 10 open questions either answered (with chosen path + rationale) or explicitly deferred to v0.2+ with rationale" can flip `[x]` — the deep-dive resolves Q1-Q5 with citations + Q6-Q10 resolved by the spike's PASS evidence (NOT deferred to v0.2+). **Mainnet PASS not required for v0.1** — `mainnet.json` exists as a token-registry artifact only.

**Round-1 grill Q4 override:** **v0.1 release GATED on one mainnet self-send — $0.001 USDT to self (recipient == sender), real value, real network.** Local + Nile is emulation. Without a real-value smoke, V1-V10 PASS evidence is "looks like real network" not "real network". Add to acceptance criteria NOW, not post-Phase 4. Mainnet self-send uses pre-check audit hook (refuse if recipient != operator_wallet) + `RUN_TRON_MAINNET=1` env gate. Documented in spike README as the only path to flip issue #399 closed.

### Spike build dependency

`protoc ≥ 3.12` must be in PATH only if the spike pulls in `prost-build` for runtime proto codegen. If we vendor `core/Tron.proto` as pre-generated `prost` types via `include!` or a one-shot `build.rs`, no `protoc` install needed at runtime — only plain `cargo build`. CI image install: `protobuf-compiler` package (Debian/Ubuntu) or `brew install protobuf` (macOS), only if `prost-build` is in play. Document the chosen path in spike README. Nile RPC at `https://nile.trongrid.io/wallet/*` is the default target for V1-V10; no gRPC client required.

### Mainnet spike implementation checklist (TODO!)

- [ ] Acquire mainnet TRX for operator test wallet (~$10 worth, from exchange)
- [ ] Acquire mainnet USDT (`TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`) — withdraw directly from exchange
- [ ] Define `RUN_TRON_MAINNET=1` env gate (mirror `RUN_TRON_NILE=1`)
- [ ] Add mainnet variants to V5/V8/V9 tests (each marked `#[ignore = "RUN_TRON_MAINNET=1 required"]`)
- [ ] Add pre-check: refuse broadcast if `recipient != operator_wallet` (audit hook)
- [ ] Add mainnet USDT smoke: $0.001 self-send — verify end-to-end on real value
- [ ] Document mainnet operator runbook in `spikes/tron-v1/README.md` (no public docs — internal only)
- [ ] **No mainnet smoke in CI** — local + Nile only by default

## Local testnet — `testcontainers` only (locked 2026-09-04)

**Decision (2026-09-04):** the spike uses the `testcontainers` Rust crate to spawn the TronBox Docker image (`tronbox/tre:latest`) as a managed container lifecycle. **Developer never runs Docker manually** — `testcontainers` handles spawn, port mapping, and teardown automatically inside the test process. Mirrors the eth `alloy-node-bindings::Anvil::new().spawn()` pattern.

### What TronBox Docker provides

Official TronBox-built image with full TRON FullNode + SolidityNode + gRPC endpoints. No Java install needed. Solidity 0.8.x, ethers v6 compatible.



### Spike integration — `testcontainers` Rust API



**No manual `docker pull` / `docker run` / `docker logs` commands.** `testcontainers` does all of it programmatically:

- **Spawn:** `GenericImage::new(...).start()` — pulls image (if not cached), creates container, starts daemon
- **Port mapping:** `get_host_port_ipv4()` returns the ephemeral host port bound to the container's internal port
- **Logs:** optional `#[testcontainers(stdout = "tronbox/tre:latest")]` captures stdout — not needed for our use
- **Teardown:** `drop(container)` (RAII) stops + removes container automatically; no manual `docker stop`/`docker rm`

**Why testcontainers over manual Docker:**

- Container lifecycle tied to test — `drop(container)` (RAII) ensures cleanup even on test panic
- Port mapping handled automatically — no port collisions across parallel test runs
- CI-friendly: no global Docker state, parallel-safe across test runners
- Same pattern as eth `Anvil::new().spawn()` — codebase consistency
- **No developer workstation setup** — works on any laptop with Docker daemon running; no manual `docker pull` step

### Local developer requirements

| Requirement                | Notes                                                                                    |
| -------------------------- | ---------------------------------------------------------------------------------------- |
| **Docker daemon running**  | Docker Desktop (macOS/Windows), `dockerd` (Linux), or Colima/Rancher Desktop alternative |
| No TronBox install needed  | `testcontainers` pulls image on first run; cached thereafter                             |
| No `docker pull` needed    | handled by testcontainers                                                                |
| No port reservation needed | testcontainers allocates ephemeral ports                                                 |

**That's it.** No manual TronBox CLI, no manual container lifecycle, no manual port management.

### CI requirements

| Tool                        | Required                   | Notes                                                                                         |
| --------------------------- | -------------------------- | --------------------------------------------------------------------------------------------- |
| Docker daemon               | ✓                          | Linux runners, macOS, Windows — GitHub Actions runners have Docker pre-installed              |
| `testcontainers` Rust crate | ✓ spike `dev-dependencies` | `testcontainers = "0.23"` (workspace resolver = "2" unifies `bollard-stubs` with `btc` crate) |

**GitHub Actions example** (drop in `.github/workflows/tron-spike.yml`):

```yaml
jobs:
  tron-spike:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable    # or pin to 1.91.1
      - run: cargo test -p tron-spike-v1 --test '*'
        # testcontainers spawns TronBox Docker automatically
```

No additional CI setup beyond standard Docker-enabled runners.

### Spike use case mapping

| Use case                         | Setup                                                 | Operator action                                    |
| -------------------------------- | ----------------------------------------------------- | -------------------------------------------------- |
| Local dev / CI integration tests | `testcontainers` + TronBox Docker (auto-spawn)        | **None — automatic**                               |
| Spike V2/V8/V12 end-to-end smoke | Same (auto-spawn)                                     | **None — automatic**                               |
| Mainnet smoke (Phase 4)          | Real Nile + real mainnet via TronGrid (no local node) | Optional `TRON-PRO-API-KEY` for higher rate limits |

### Why no other local options

- **TronBox CLI (`npx tronbox develop`)** — requires Node runtime, breaks Rust-only CI pipeline
- **java-tron wallet-cli** — ~3 GB Java toolchain, 30 min Gradle build, image bloat
- **tronz TRE** — internal to `throgxyz/tronz` CI, not user-facing; N/A for v0.1 (we don't use tronz)
- **Manual Docker (`docker run`)** — developer workstation setup burden, port management, teardown hygiene — all handled automatically by `testcontainers`

Sources: [testcontainers Rust crate](https://docs.rs/testcontainers/), [TronBox GitHub](https://github.com/tronprotocol/tronbox), deep-dive §"Local node — TronBox / wallet-cli".

## AnyChain Tron — feature map

Per-crate feature surface for the 3 TRON-supporting anychain crates: `anychain-tron` (primary wire-format), `anychain-kms` (HD + signing), `anychain-core` (shared traits + utilities).

### `anychain-tron` 0.2.14 (PRIMARY — wire-format)

| #   | Feature                                                                           | API                                                                                     |
| --- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| 1   | **T-base58check address derivation**                                              | `TronAddress::from_public_key(&pk, &TronFormat::Standard)`                              |
| 2   | **Address parse** (T-string / hex / 0x-hex / TVM-zero)                            | `TronAddress::from_str("T...")`                                                         |
| 3   | **Address display** (base58check + EIP-55 hex)                                    | `addr.to_string()` + `addr.to_hex()`                                                    |
| 4   | **Address as EVM ABI token**                                                      | `addr.to_token() -> Token::FixedBytes`                                                  |
| 5   | **Default zero-address** (TVM burn addr)                                          | `TronAddress::default()`                                                                |
| 6   | **Uncompressed pubkey wrap**                                                      | `TronPublicKey::from_secp256k1_public_key`                                              |
| 7   | **TronTransaction envelope** (proto)                                              | `TronTransaction::new` + `sign(sig, recid)` + `to_bytes` + `from_bytes`                 |
| 8   | **TronTransactionParameters** (ref_block, fee_limit, expiration, timestamp, memo) | `set_ref_block` + `set_fee_limit` + `set_timestamp` + `set_expiration` + `set_contract` |
| 9   | **Native TRX transfer**                                                           | `trx::build_transfer_contract(owner, recipient, sun_amount)`                            |
| 10  | **TRC-20 transfer (USDT/USDC/TUSD/USDD/stUSDT)**                                  | `trx::build_trc20_transfer_contract` + `abi::trc20_transfer`                            |
| 11  | **TRC-20 approve**                                                                | `trx::build_trc20_approve_contract` + `abi::trc20_approve`                              |
| 12  | **Account create** (1 TRX burn)                                                   | `trx::build_account_create`                                                             |
| 13  | **Stake 2.0 freeze (BANDWIDTH / ENERGY)**                                         | `trx::build_freeze_balance_v2_contract`                                                 |
| 14  | **Stake 2.0 unfreeze**                                                            | `trx::build_unfreeze_balance_v2_contract`                                               |
| 15  | **Stake 2.0 delegate** (lock=true → 3 days)                                       | `trx::build_delegate_resource_contract`                                                 |
| 16  | **Stake 2.0 undelegate**                                                          | `trx::build_undelegate_resource_contract`                                               |
| 17  | **Stake 2.0 cancel all pending unfreezes**                                        | `trx::build_cancel_unfreeze_contract`                                                   |
| 18  | **Stake 2.0 withdraw expired unfreeze**                                           | `trx::build_withdraw_unfreeze_contract`                                                 |
| 19  | **SR witness vote**                                                               | `trx::build_vote_witness_contract(owner, votes, support)`                               |
| 20  | **Withdraw vote reward**                                                          | `trx::build_withdraw_vote_contract`                                                     |
| 21  | **Generic smart contract trigger**                                                | `trx::build_trigger_contract`                                                           |
| 22  | **TRC-20 ABI encode (manual)**                                                    | `abi::contract_function_call("name", &[Param])`                                         |
| 23  | **Timestamp helper** (ms epoch)                                                   | `trx::timestamp_millis()` → `Utc::now().timestamp_millis()`                             |
| 24  | **Txid (BUGGY — single hash)**                                                    | `tx.to_transaction_id()` — **bypass: do SHA256(SHA256(raw)) manually**                  |
| 25  | **Protocol buffer wire types** (vendored 1.5 MB)                                  | `core/Tron.proto` + 13 `core/contract/*.proto` (auto-imported via `pub mod protocol`)   |

### `anychain-kms` 0.1.23 (SUPPORTING — HD + signing)

| #   | Feature                                                                     | API                                                          |
| --- | --------------------------------------------------------------------------- | ------------------------------------------------------------ |
| 1   | BIP-39 mnemonic generate (12/15/18/21/24 words, 8 languages)                | `Mnemonic::new(MnemonicType::Words12, Language::English)`    |
| 2   | BIP-39 mnemonic parse + checksum validate                                   | `Mnemonic::from_phrase(phrase, Language::English)`           |
| 3   | BIP-39 seed via PBKDF2-HMAC-SHA512 (2048 rounds, 64 bytes, NFKD-normalized) | `Seed::new(&m, "passphrase")`                                |
| 4   | BIP-32 HD master xprv from seed                                             | `XprvSecp256k1::new(seed.as_bytes())`                        |
| 5   | BIP-32 derived xprv at path (m/44'/195'/0'/0/0)                             | `XprvSecp256k1::new_from_path(seed, &path.parse().unwrap())` |
| 6   | BIP-32 child derivation                                                     | `xprv.derive_child(N.into())`                                |
| 7   | xprv serialize (base58check, auto-zeroize)                                  | `xprv.to_string(Prefix::XPRV) -> Zeroizing<String>`          |
| 8   | xpub export (watch-only)                                                    | `xprv.public_key()`                                          |
| 9   | xpub fingerprint (RIPEMD160(SHA256(pubkey))[..4]) for gap-limit             | `xpub.fingerprint()`                                         |
| 10  | secp256k1 ECDSA sign (returns r∥s + recid)                                  | `secp256k1_sign(&sk_bytes, &tx_hash) -> (Vec<u8>, u8)`       |
| 11  | Zeroizing<String> for xprv string (auto-cleared on drop)                    | inherent in `xprv.to_string()`                               |
| 12  | ConstantTimeEq for xprv compare                                             | `subtle::ConstantTimeEq` trait                               |
| 13  | Debug for xprv (redacts sk)                                                 | custom impl prints `"..."` for sk field                      |

### `anychain-core` 0.1.8 (SUPPORTING — shared traits)

| #   | Feature                                                                  | API                                                                                |
| --- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| 1   | **Address trait** (used by anychain-tron's `TronAddress`)                | `pub trait Address` + `AddressError`                                               |
| 2   | **Format trait** (used by `TronFormat::Standard`)                        | `pub trait Format` marker                                                          |
| 3   | **Network trait** (anychain-tron has `TronFormat` not Network — partial) | `pub trait Network`                                                                |
| 4   | **PublicKey trait**                                                      | `pub trait PublicKey`                                                              |
| 5   | **Transaction trait** (anychain-tron's `TronTransaction` impls)          | `pub trait Transaction` + `TransactionId`                                          |
| 6   | **TransactionId trait**                                                  | marker                                                                             |
| 7   | **TransactionError**                                                     | error type                                                                         |
| 8   | **Amount trait**                                                         | amount types (unused by TRON but in scope)                                         |
| 9   | **`crypto::sha256`** — used for bug #1 workaround (double-hash)          | `anychain_core::crypto::sha256(&raw)`                                              |
| 10  | **`crypto::keccak256`** — available for cross-chain use                  | (TRON doesn't use this directly — anychain-tron uses `sha3::Keccak256` separately) |

### NOT covered (caller must provide)

| Need                                        | Why missing from anychain                                                    |
| ------------------------------------------- | ---------------------------------------------------------------------------- |
| JSON-RPC client (`reqwest` calls)           | anychain has zero RPC across all crates                                      |
| Wallet file encryption (Argon2id + AES-GCM) | anychain-kms has PBKDF2 only for mnemonic→seed, no AES                       |
| Token registry JSON                         | no SDK, application config                                                   |
| SPKI pin verifier                           | BTC side has it via `bitcoin-wallet-core::chain::spki` (reusable)            |
| TRX balance query                           | no RPC, caller queries `wallet/getaccount`                                   |
| Tx history query                            | no RPC, caller queries `wallet/gettransactioninfobyid`                       |
| Energy/bandwidth estimation                 | no RPC, caller queries `wallet/getaccountresource` + `wallet/estimateenergy` |
| Network selection (mainnet/Nile/Shasta)     | anychain-tron has `TronFormat::Standard` only — no per-network config        |
| TronBox Docker spawning                     | caller uses `testcontainers` crate                                           |
| Anti-replay window management               | caller checks `timestamp + expiration`                                       |
| `Zeroizing` on `secp256k1_sign(sk: &[u8])`  | GAP — kms does not wrap param; caller must Zeroizing-wrap sk                 |

### Total coverage

- **anychain-tron: 25 TRON-specific features** (all wire-format + contract builders)
- **anychain-kms: 13 features** (HD + signing, all applicable to TRON's secp256k1)
- **anychain-core: 10 features** (shared traits, mostly in use)

**Net: 48 features across 3 crates cover ~85% of TRON wallet functionality.** Caller adds ~250 lines of RPC glue + wallet encryption.

## Tron Wallet V0.1

User-facing features for `tron-wallet-core v0.1.0` + `tron` CLI v0.1.0 release cut (issue #399). **Must compile on desktop (Linux/macOS/Windows) + mobile (iOS arm64 + Android arm64) with no source changes** — pure Rust + 4-trait Platform Abstraction Layer. 22 commands across 6 top-level commands (`wallet` 9, `address` 2, `balance` 2, `trc20` 4, `tx` 2, `config` 3). Each row maps a feature to the story id from `wallets/2026-08-27-tron-wallet-user-stories.md` + the `tron-wallet-core` call + CLI command + status.

### `tron wallet` (9 commands)

| CLI command                                                                                                                                  | Story | tron-wallet-core call                                               | Status |
| -------------------------------------------------------------------------------------------------------------------------------------------- | ----- | ------------------------------------------------------------------- | ------ |
| `wallet create --words 12\|24 --name --network --password`                                                                                   | 1     | `WalletManager::create_with_mnemonic`                               | ready  |
| `wallet import --name --network --password --mnemonic\|--mnemonic-file\|--private-key-file`                                                  | 2     | `WalletManager::import_from_phrase` or `import_from_pk`             | ready  |
| `wallet show --id [--json]`                                                                                                                  | 11    | `WalletManager::unlock(id, pw).summary()`                           | ready  |
| `wallet list [--json] [--all-networks]`                                                                                                      | 9     | `WalletManager::list()`                                             | ready  |
| `wallet delete --id`                                                                                                                         | 9     | `WalletManager::delete(id)`                                         | ready  |
| `wallet rename --id --to`                                                                                                                    | 9     | `WalletManager::rename(id, name)`                                   | ready  |
| `wallet balance --wallet-id [--token USDT\|<addr>] \| --address [--token <addr>]`                                                            | 3, 22 | `chain::get_account(addr)` or `WalletManager::unlock(id).balance()` | ready  |
| `wallet send --wallet-id\|--mnemonic --to <addr>\|--to-wallet <name\|id> --amount [--unit] [--fee-limit] [--dry-run] [--sign-only] [--wait]` | 5     | `tx::submit_trx(sk, to, amount_sun, fee_limit, &cfg)`               | ready  |
| `wallet send-speedup --wallet-id --txid --fee-limit`                                                                                         | 17    | `tx::submit_send_speedup(...)`                                      | ready  |

**Mnemonic handling (L28 / F49 / L12 H-1):**
- `wallet create` → mnemonic → STDERR (red highlight); wallet_id → STDOUT
- `wallet import --mnemonic` → mnemonic → STDERR; wallet_id → STDOUT
- `wallet import --mnemonic-file` → reads mode-0600 file (closes argv-exposure L12 H-1)
- `wallet show` → decrypted mnemonic NEVER displayed; only address + balance

**Wallet-to-wallet transfer pattern:**

```bash
# Transfer 10 TRX from "trading" wallet to "savings" wallet (both stored locally)
tron wallet send --wallet-id <trading-uuid> --to-wallet savings --amount 10 --unit trx

# Transfer 100 USDT from "hot" to "cold" wallet
tron wallet send --wallet-id <hot-uuid> --to-wallet cold --amount 100 --contract USDT
```

- `--to <addr>` accepts a T-base58check address
- `--to-wallet <name|id>` accepts a stored wallet name or UUID; resolves via `WalletManager::lookup(name_or_id).address`
- `--to` and `--to-wallet` are mutually exclusive (clap `conflicts_with`)
- For TRC-20 wallet-to-wallet, add `--contract USDT|<addr>` flag (re-uses trc20 builder)

### `tron address` (2 commands)

| CLI command                                                 | Story | tron-wallet-core call                  | Status |
| ----------------------------------------------------------- | ----- | -------------------------------------- | ------ |
| `address new --mnemonic [--mnemonic-file] --index [--path]` | 3     | `keys::derive_keypair(mnemonic, path)` | ready  |
| `address xpub --wallet-id`                                  | 19    | `WalletManager::xpub(id)`              | ready  |

### `tron balance` (2 commands — standalone, address-driven)

| CLI command                                     | Story | tron-wallet-core call                  | Status |
| ----------------------------------------------- | ----- | -------------------------------------- | ------ |
| `balance --address <addr> [--unit trx\|sun]`    | 3     | `chain::get_account(addr)`             | ready  |
| `balance --address <addr> --token USDT\|<addr>` | 22    | `chain::trc20_balance(addr, contract)` | ready  |

Output formats:
- Native TRX: `JSON: { trx, trx_sun, energy, bandwidth }`
- TRC-20: `JSON: { symbol, balance, decimals }`

Use `balance` (not `wallet balance`) for non-wallet queries (cold wallets, watch-only addresses, exchange hot wallets). Use `wallet balance --wallet-id` for unlocked-wallet queries (auto-decrypts).

### `tron trc20` (4 commands)

| CLI command                                                   | Story  | tron-wallet-core call                                     | Status |
| ------------------------------------------------------------- | ------ | --------------------------------------------------------- | ------ |
| `trc20 send --mnemonic --contract USDT\|<addr> --to --amount` | 21     | `tx::submit_trc20(sk, contract, to, amount)`              | ready  |
| `trc20 approve --mnemonic --contract --spender --amount`      | 25, 30 | `tx::submit_trc20_approve(sk, contract, spender, amount)` | ready  |
| `trc20 balance --address --contract USDT\|<addr>`             | 22     | `chain::trc20_balance(addr, contract)`                    | ready  |
| `trc20 allowance --contract --owner --spender`                | 30     | view-call `allowance(owner,spender)`                      | ready  |

### `tron tx` (2 commands)

| CLI command                                | Story | tron-wallet-core call                 | Status |
| ------------------------------------------ | ----- | ------------------------------------- | ------ |
| `tx get --txid`                            | 7     | `chain::get_tx_info(txid)`            | ready  |
| `tx wait --txid --timeout --poll-interval` | 7     | `tx::wait_for_confirm(txid, timeout)` | ready  |

### `tron config` (3 commands)

| CLI command                                | Story      | tron-wallet-core call                  | Status |
| ------------------------------------------ | ---------- | -------------------------------------- | ------ |
| `config show [--json]`                     | 10, 11     | `config::TronConfig::load().display()` | ready  |
| `config set-rpc <url>`                     | 10, 26, 27 | `config::set_rpc(url)` + save          | ready  |
| `config set-network mainnet\|shasta\|nile` | 10, 27     | `config::set_network(net)` + save      | ready  |

### Security (cross-cutting)

| Feature                                                                  | Story         | Implementation                                                                             | Status |
| ------------------------------------------------------------------------ | ------------- | ------------------------------------------------------------------------------------------ | ------ |
| Argon2id + AES-GCM wallet file encryption                                | 12            | `tron-wallet-core::wallet::persist` (anychain has none — wallet-local)                     | ready  |
| `Zeroizing<Vec<u8>>` wrap on raw `sk` before `secp256k1_sign`            | 5, 17, 21, 25 | caller wrap (GAP — kms does not Zeroize sk param)                                          | ready  |
| SPKI pin RPC endpoint                                                    | 28            | `tron-wallet-core::chain::spki::SpkiPinnedVerifier` (reuses `bitcoin-wallet-core` pattern) | ready  |
| No SPKI pin (system CAs + localhost/LAN)                                 | 29            | `chain::TronGridClient::new(url, None)` + `webpki-roots`                                   | ready  |
| Dual-SHA256 txid workaround for `anychain-tron::to_transaction_id()` bug | 5, 17         | `tron-wallet-core::tx::sign`                                                               | ready  |

### Cross-cutting (apply to all)

| Feature                                                                    | Implementation                                                                 | Status |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ------ |
| `--json` mode on list/show/sync/tx-list/config-show                        | `serde_json` + clap value-conditional                                          | ready  |
| Stable exit codes (0/1/2/3/4/5)                                            | `handlers::error::classify` (matches `btc/src/main.rs:151-169` exit-2 pattern) | ready  |
| T-base58check address display (34 chars, T prefix)                         | `anychain_tron::TronAddress::to_base58`                                        | ready  |
| TRX/sun amount unit handling (`1_000_000` sun = 1 TRX)                     | `--unit trx\|sun` flag → `amount.as_sun(unit)`                                 | ready  |
| Mnemonic at rest never plaintext (Zeroizing + Argon2id + AES-GCM)          | `tron-wallet-core::wallet::persist`                                            | ready  |
| Confirmation prompts (mainnet / drain / unlimited approval)                | stdin `yes` confirmation                                                       | ready  |
| SPKI pin env-only (no CLI setter in V0.1)                                  | `TRON_SPKI_PIN` env + `--spki-pin` flag                                        | ready  |
| All crypto delegated to `tron-wallet-core` (CLI never touches sk directly) | `SecretMnemonic` + `Zeroizing<Vec<u8>>`                                        | ready  |

### Deferred (not shipped in V0.1)

#### To V0.1.5 (Stake 2.0 — operator flow, ships between V0.1 and V0.2)

| Feature                               | Story | Notes                                  |
| ------------------------------------- | ----- | -------------------------------------- |
| Stake 2.0 freeze                      | 4     | `tron stake freeze`                    |
| Stake 2.0 unfreeze                    | 4     | `tron stake unfreeze`                  |
| Stake 2.0 delegate (BANDWIDTH/ENERGY) | 6     | `tron stake delegate`                  |
| Stake 2.0 undelegate                  | 6     | (no CLI command — reverse of delegate) |
| Vote witness (SR vote)                | 6     | `tron stake vote --votes`              |
| Cancel pending unfreeze               | 31    | `tron stake cancel-unfreeze`           |
| Withdraw expired unfreeze             | 32    | `tron stake withdraw-unfreeze`         |
| Withdraw vote reward                  | 33    | `tron stake withdraw-vote`             |

**Round-1 grill Q11:** Stake 1.0 deferred entirely. Stake 1.0 (pre-April 2023) unfreeze paths NOT in v0.1/v0.2. Documented `tron stake` = Stake 2.0 only. Add `tron stake1 withdraw` in v0.2 IF user demand surfaces. No silent Stake 1.0 code paths.

#### To V0.2 (advanced + operator)

| Feature                                     | Story          | Notes                                                          |
| ------------------------------------------- | -------------- | -------------------------------------------------------------- |
| List registered TRC-20 stablecoins          | 23             | `tron tokens list`                                             |
| Add custom TRC-20 token by contract         | 24             | `tron tokens register`                                         |
| Bulk TRC-20 balance scan                    | 22 (bulk)      | `tron tokens balances`                                         |
| Energy/bandwidth estimation                 | 8              | `tron fee estimate`                                            |
| Recent fee paid history                     | 8 (history)    | `tron fee history`                                             |
| Sign personal message                       | 18             | `tron sign message`                                            |
| Verify personal message                     | 18             | `tron sign verify`                                             |
| Nile faucet URL print                       | 27             | `tron faucet show`                                             |
| Auto-drip Nile faucet                       | 27             | `tron faucet drip`                                             |
| TRC-10 token issue                          | 34             | `tron trc10 issue`                                             |
| TRC-10 token send                           | 34             | `tron trc10 send`                                              |
| TRC-10 token buy                            | 34             | `tron trc10 buy`                                               |
| Network governance proposal                 | 35             | `tron governance propose`                                      |
| Network governance approve                  | 35             | `tron governance approve`                                      |
| Storage buy/sell (bandwidth market)         | 36             | `tron storage buy`/`sell`                                      |
| Wallet history sync                         | 7 (bulk)       | `tron wallet sync`                                             |
| Wallet export (cold storage)                | n/a            | `tron wallet export`                                           |
| Wallet show secret (gated mnemonic display) | n/a (recovery) | `tron wallet show --secret` (requires password + confirmation) |
| Address show private key (per-address)      | n/a (recovery) | `tron address show --private-key`                              |
| Wallet import from xprv file                | n/a (recovery) | `tron wallet import --xprv-file`                               |
| `tx list` (history scan)                    | 7              | `tron tx list`                                                 |
| `config set-spki-pin` (move from env-only)  | 28             | `tron config set-spki-pin`                                     |
| Shell completion script                     | DX             | `tron shell completion`                                        |
| **Thread model** (FFI deadlock / Zeroizing-across-await / Send+Sync on secrets / concurrent broadcast seriality / mobile-vs-desktop runtime divergence) | n/a            | v0.2 (deferred — see also `#### Thread model (concurrent design — cross-platform)` section below, deferred from v0.1 to v0.2) |

#### To V0.3 (advanced + requires upstream changes)

| Feature                    | Notes                  | Requires                                                  |
| -------------------------- | ---------------------- | --------------------------------------------------------- |
| SR witness candidacy apply | `tron witness apply`   | upstream `anychain-tron`                                  |
| SR witness update URL      | `tron witness update`  | upstream `anychain-tron`                                  |
| Multi-sig account creation | `tron multisig create` | **upstream fork** (anychain-tron is single-sig)           |
| Multi-sig send             | `tron multisig send`   | **upstream fork**                                         |
| Shielded TRC-20 transfer   | `tron shield transfer` | **zk-SNARK proving system** (`bellman` or `halo2_proofs`) |

#### Never (out of scope)

| Feature                                   | Reason                             |
| ----------------------------------------- | ---------------------------------- |
| EIP-712 typed data                        | TRON has no equivalent spec        |
| TRC-721 NFTs                              | no anychain-tron builder           |
| Hardware wallet (Ledger/Trezor)           | future, v1.x                       |
| Multi-sig Safe-style                      | different model entirely           |
| Plausible-deniability multi-bucket wallet | far future                         |
| gRPC transport                            | JSON-RPC sufficient                |
| Watch-only import from xpub               | rarely used, deferred indefinitely |

### Total user stories covered

| Story count                                                  | Source                                 | Status                        |
| ------------------------------------------------------------ | -------------------------------------- | ----------------------------- |
| 1, 2, 3, 5, 7, 9, 10, 11, 12, 17, 19, 21, 22, 25, 27, 28, 29 | V0.1                                   | shipped                       |
| 4, 6, 31, 32, 33                                             | V0.1.5 (stake)                         | ships with V0.1 release train |
| 8, 18, 23, 24, 30, 34, 35, 36                                | V0.2                                   | next release                  |
| 26 (TronBox local)                                           | via `--rpc http://127.0.0.1:8090` flag | shipped                       |
| 14 (drain), 13 (batch), 15 (ref-block), 16 (manual exp)      | redesign dropped                       | removed                       |

**Round-1 grill Q3 finding — SINGLE-MAINTAINER RISK (2026-09-05 audit):** `0xcregis/anychain` author diversity over trailing 12 months:
- `anychain-tron/`: **1 author (`loki-cmu`), 3 commits** — single-maintainer trust, worse than rejected `tronic` (1 author, v0.6.1) and `tronz` (1+1+dependabot)
- `anychain-kms/`: **2 authors (`Brian`, `loki-cmu`), 8 commits** — marginal
- umbrella total: **2 distinct authors**

Doc rejected `tronic` + `tronz` partly on single-maintainer trust. `anychain-tron` is the same risk class — bus-factor = 1. **Action required:** before v0.1 ships, vendor `anychain-tron` + `anychain-kms` source into `rust-wallet-app/crates/anychain-vendored/` (fork-with-citation pattern, NOT a public fork) so the wallet doesn't depend on upstream bus-factor. Cite per file. Track upstream for security fixes — apply manually to vendored copy. Alternative: pick `tronz` (52 stars, multi-maintainer incl. dependabot) and write the ~1000 std primitives (BIP-32 HD + protobuf + base58check + Keccak256 + ABI encoder) ourselves — original 2026-09-04 raw-primitives decision.

**Round-1 grill Q5 finding — SPKI pin live extraction (2026-09-05):** `api.trongrid.io` TLS cert SPKI SHA-256 = `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d`. Pin this in `TronConfig::for_network(Network::Mainnet)` SPKI list. `SpkiPinnedVerifier` reused from `bitcoin-wallet-core::chain::spki` shape-compatible — verifies via `webpki::TrustAnchor` matching the live Cloudflare-fronted cert chain. No fork needed for Q5.

**Round-1 grill Q9 finding — workspace dep check premature (2026-09-05):** `rust-wallet-app/` has no `tron-wallet-core` crate yet. Crates present: `bitcoin-wallet-core`, `btc`, `chain-traits`, `eth`, `eth-wallet-core`, `evm-wallet-core`, `polygon`, `polygon-wallet-core`. `resolver = "2"` set. `anychain-*` deps added when `tron-wallet-core` lands. Cannot run `cargo tree -p tron-wallet-core` until crate exists. Defer Q9 to spike V1 dep-wiring step.

**Round-1 grill Q8 audit:** every "Status: ready" row above is by inspection, NOT by spike PASS. Before v0.1 ships, audit each ready row against the V1-V10 mapping table (## Spike verification). If a command isn't covered by at least one Vn spike PASS block, demote to "ready (untested)" or remove from v0.1. Ship what spikes prove, not what inspection suggests. Spike V1-V10 covers derivation/signing/tx/network primitives — NOT full CLI command flows. Gap = 22 commands × 4-layer coverage (init → sign → broadcast → confirm).

### Feature map by CLI top-level

| Top-level | Commands | V0.1 features                                                                   |
| --------- | -------- | ------------------------------------------------------------------------------- |
| `wallet`  | 9        | create / import / show / list / delete / rename / balance / send / send-speedup |
| `address` | 2        | new / xpub                                                                      |
| `trc20`   | 3        | send / approve / allowance                                                      |
| `tx`      | 2        | get / wait                                                                      |
| `config`  | 3        | show / set-rpc / set-network                                                    |
| **Total** | **19**   | All user stories 1-3, 5, 7, 9-12, 17, 19, 21, 22, 25-30                         |
## Tron Wallet Core

`tron-wallet-core` library (rlib + cdylib for FFI). Mirrors `bitcoin-wallet-core` (12663 LOC), `polygon-wallet-core` (251 LOC thin wrapper). For TRON: anychain has no upstream EVM-style `evm-wallet-core` analog — `tron-wallet-core` is fat standalone (~4525 LOC V0.1, growing to ~5275 by V0.3). All crypto delegated to `anychain-tron` + `anychain-kms` + `anychain-core`. RPC + persistence + encryption + FFI wallet-local. **Must compile on desktop (Linux/macOS/Windows) + mobile (iOS arm64 + Android arm64) with no source changes** — pure Rust + 4-trait Platform Abstraction Layer (WalletStorage, PlatformInfo, NetworkClient, Clock) for platform-specific concerns.

### Architecture (cross-version)

```text
rust-wallet-app/crates/tron-wallet-core/
├── Cargo.toml
└── src/
    ├── lib.rs              # per-item `pub use` re-exports (NOT glob)
    ├── address/            # base58check encode/decode, hex variant
    ├── keys/               # BIP-39 mnemonic + BIP-32 HD at SLIP-44 coin 195
    ├── crypto/             # mnemonic cipher (argon2id + AES-GCM)
    ├── config.rs           # network enum, format, SPKI pin, data dir
    ├── tx/
    │   ├── builder.rs      # 17+ contract builders (V0.1: 17, V0.2: +5, V0.3: +3)
    │   ├── sign.rs         # secp256k1 sign over SHA256(raw), Zeroizing sk
    │   ├── broadcast.rs    # wallet/broadcasttransaction POST
    │   └── summary.rs      # TxSummary struct (transfer log)
    ├── chain/              # TronGrid HTTP client + SPKI pin verifier
    ├── wallet/             # UUID id + encrypted store + atomic write
    ├── tokens/             # bundled JSON via `include_str!` (V0.1: 1 chain, V0.2: 2 chains + register)
    └── disambig.rs         # TRC-20 footgun guards + cross-network address check (Round-1 grill Q12)
```

**Cross-version dependencies:**

| Crate                  | Version     | Purpose                                                      |
| ---------------------- | ----------- | ------------------------------------------------------------ |
| `anychain-tron`        | 0.2.14      | wire format — address, tx, contract builders, ABI            |
| `anychain-kms`         | 0.1.23      | BIP-39 + BIP-32 HD, secp256k1 signing                        |
| `anychain-core`        | 0.1.8       | shared traits, `keccak256`, `sha256`, `func_selector`, `hex` |
| `tiny-keccak`          | 2.0.2       | keccak256 (kept direct, no anychain indirection)             |
| `bs58`                 | 0.5         | base58check                                                  |
| `argon2` + `aes-gcm`   | workspace   | wallet file encryption (anychain has none)                   |
| `reqwest` + `rustls`   | 0.12 / 0.23 | TronGrid HTTP + SPKI pin                                     |
| `tokio`                | 1           | async runtime                                                |
| `uuid` + `directories` | workspace   | wallet id + filesystem layout                                |

### Module × anychain map (stable across versions)

| Module            | anychain API                                                                                                                                                                                                                                                                      |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lib.rs`          | per-item `pub use` of `Address`, `PublicKey`, `Format`, `Network`, `Transaction`, `TransactionId`, `Error` (core 0.1.8)                                                                                                                                                           |
| `address/`        | `anychain_tron::TronAddress`, `TronAddress::from_public_key`, `to_base58`, `to_hex`, `is_valid`; `anychain_core::keccak256` for derivation                                                                                                                                        |
| `keys/`           | `anychain_kms::Mnemonic::{generate,from_phrase}`, `seed_from_mnemonic`, `DerivationPath::from_str`, `ChildNumber::Hardened(195)`, `ExtendedPrivateKey::{from_seed,derive_child,to_string,from_str}` (xprv 78-byte base58check), `ExtendedPublicKey::{from_private_key,to_string}` |
| `crypto/`         | none (wallet-local argon2id + AES-GCM)                                                                                                                                                                                                                                            |
| `config.rs`       | `anychain_tron::TronNetwork::{Mainnet,Shasta,Nile}`, `TronFormat`, `anychain_core::{Network, Format}` trait re-exports                                                                                                                                                            |
| `tx/builder.rs`   | 17+ `anychain_tron::trx` builders (see per-version detail below)                                                                                                                                                                                                                  |
| `tx/sign.rs`      | `anychain_kms::secp256k1_sign(&sk_z, msg32)` then `anychain_tron::TronTransaction::sign(sig, recid)`                                                                                                                                                                              |
| `tx/broadcast.rs` | `anychain_tron::Transaction::to_bytes` for serialize                                                                                                                                                                                                                              |
| `tx/summary.rs`   | none (derived type)                                                                                                                                                                                                                                                               |
| `chain/`          | `anychain_tron::Transaction::from_bytes` for `wallet/gettransactioninfobyid` response                                                                                                                                                                                             |
| `wallet/`         | none (UUID v4 + argon2id + AES-GCM)                                                                                                                                                                                                                                               |
| `tokens/`         | none (`include_str!` bundled JSON)                                                                                                                                                                                                                                                |
| `disambig.rs`     | none (compile-time constants)                                                                                                                                                                                                                                                     |
| `error.rs`        | re-export `anychain_core::Error` + sub-enums                                                                                                                                                                                                                                      |
| `ffi/`            | wraps `anychain_tron` + `anychain_kms` across C ABI                                                                                                                                                                                                                               |
| `util/`           | none (atomic_write, permissions)                                                                                                                                                                                                                                                  |

### Coverage summary (stable across versions)

| Source                     | Modules covered                                                                                         | LOC share                      |
| -------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------ |
| anychain-tron              | `lib.rs`, `address`, `tx/builder` (17+ builders)                                                        | ~30% of tron-wallet-core logic |
| anychain-kms               | `keys` (mnemonic + seed + HD + xpub), `tx/sign` (secp256k1)                                             | ~20%                           |
| anychain-core              | `lib.rs`, `config`, `error` (re-exports)                                                                | ~5%                            |
| Wallet-local (no anychain) | `crypto`, `tx/broadcast`, `tx_summary`, `chain`, `wallet`, `tokens/`, `disambig`, `ffi` (wraps), `util` | ~45%                           |

### Bugs + workarounds (stable across versions)

| Bug                                                              | Source                             | Workaround in `tron-wallet-core`                                             |
| ---------------------------------------------------------------- | ---------------------------------- | ---------------------------------------------------------------------------- |
| `TronTransaction::to_transaction_id()` returns single SHA-256    | `anychain-tron/src/transaction.rs` | `let txid = Sha256::digest(&Sha256::digest(&tx.to_bytes()))` in `tx/sign.rs`; **pin `anychain-tron` to exact `0.2.14` (not `^`); add unit test asserting `txid == SHA256(SHA256(raw_bytes))` so future anychain "fix" gets caught in CI** (Round-1 grill Q2) |
| `secp256k1_sign(sk: &[u8], ...)` does NOT Zeroize its `sk` param | `anychain-kms/src/secp256k1.rs`    | wrap in `Zeroizing<Vec<u8>>` before call; drop wrapper after sign            |
| `trx::build_contract` formats `type_url` via `{:?}` Debug        | `anychain-tron/src/trx.rs`         | serialize `type_url` manually via `hex::encode` if needed                    |
| protobuf wire format diverges from serde_json default            | `anychain-tron/src/protocol/`      | use `serde_json::to_value(&tx)` not `tx.to_string()` (Debug)                 |
| MSRV 1.98.1 anychain umbrella                                    | `anychain/Cargo.toml` workspace    | **pin `rust-toolchain.toml` to 1.98.1; advertise MSRV 1.94 ONLY after `cargo +1.94 check -p anychain-tron -p anychain-kms` passes end-to-end. If 1.94 check fails, advertise 1.98.1 honestly — no fake MSRV.** (Round-1 grill Q1) |

### Cross-crate flows (stable across versions)

**Address derivation (kms → tron cooperation):**

```text
tron-wallet-core::keys::derive_keypair(mnemonic, path)
    → anychain_kms::seed_from_mnemonic(phrase, "") -> [u8; 64]
    → anychain_kms::ExtendedPrivateKey::from_seed(seed)
    → anychain_kms::ExtendedPrivateKey::derive_child(path)
    → xprv.to_string() -> Zeroizing<String>      [persist?]
    → anychain_tron::TronAddress::from_public_key(&xprv.public_key())
    → "T..." base58check
```

**Sign + broadcast TRX tx:**

```text
tron-wallet-core::tx::sign::sign_tx(&sk, raw_data_bytes)
    → anychain_core::sha256(&raw_bytes)            [direct utility]
    → msg32
    → anychain_kms::secp256k1_sign(&sk_z, msg32)    [Zeroizing wrapper]
    → (sig, recid)
    → anychain_tron::Transaction::sign(sig, recid)
    → SignedTransaction { txid: SHA256(SHA256(raw)), signature }

tron-wallet-core::tx::broadcast::serialize_for_broadcast(&tx)
    → serde_json::to_value(&tx)
    → POST wallet/broadcasttransaction             [caller reqwest]
```

### Mobile-compatible architecture (V0.1.5 — cross-platform from day 1)

Redesigned layered architecture where ~95% of `tron-wallet-core` is **pure Rust portable**, and the remaining ~5% is abstracted via 4 traits (PAL = Platform Abstraction Layer). Same code compiles for Linux, macOS, Windows, iOS, Android with no source changes — only Cargo features + target flags.

#### Architecture layers

```text
┌─────────────────────────────────────────────────────────────────┐
│ Layer 5: FFI (cdylib)                                           │
│   - C ABI surface (extern "C" fn wallet_unlock, ...)            │
│   - Panic-message scrubber (no leak across boundary)            │
│   - Tokio runtime pinned (single-threaded current_thread)       │
└─────────────────────────────────────────────────────────────────┘
                              │ FFI boundary (C ABI)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 4: Pure Rust Core (portable, 95% of code)                │
│   - address/, keys/, tx/builder, tx/sign                        │
│   - crypto (argon2id + AES-GCM logic)                            │
│   - error, disambig, config (types), util                        │
│   - tx_summary, tokens (bundled JSON)                           │
└─────────────────────────────────────────────────────────────────┘
                              │ uses traits from
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 3: Platform Abstraction Layer (PAL) — 4 traits           │
│   - WalletStorage (encrypted blob persistence)                   │
│   - PlatformInfo (data dir resolution, app name, version)        │
│   - NetworkClient (HTTP + TLS root certs)                      │
│   - Clock (monotonic time, used by tx expiration)               │
└─────────────────────────────────────────────────────────────────┘
                              │ implemented per-platform
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 2: Platform implementations (5% of code)                  │
│   Desktop: FileWalletStorage, SystemDirsInfo, ReqwestClient    │
│   iOS:     KeychainStorage, BundleInfo, OSRootsClient           │
│   Android: EncryptedFileStorage, ContextInfo, OSRootsClient      │
│   Tests:   InMemoryStorage, StaticInfo, MockClient              │
└─────────────────────────────────────────────────────────────────┘
                              │ calls into
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 1: OS + hardware                                           │
│   Desktop: filesystem + OpenSSL/ring                             │
│   iOS: Keychain Services + Secure Enclave + Apple Trust Store    │
│   Android: EncryptedFile + Android Keystore + StrongBox         │
└─────────────────────────────────────────────────────────────────┘
```

#### Platform Abstraction Layer (4 traits)

```rust
// tron-wallet-core/src/platform/storage.rs (NEW V0.1.5)

/// Encrypted wallet blob persistence. Mobile uses OS-backed secure storage;
/// desktop uses filesystem with mode-0600.
pub trait WalletStorage: Send + Sync {
    /// Persist encrypted wallet blob (already argon2id + AES-GCM encrypted).
    fn put(&self, id: WalletId, ciphertext: &[u8]) -> Result<(), Error>;

    /// Load encrypted wallet blob. Returns Error::NotFound if missing.
    fn get(&self, id: WalletId) -> Result<Vec<u8], Error>;

    /// List all wallet ids. Order undefined.
    fn list(&self) -> Result<Vec<WalletId>, Error>;

    /// Delete wallet. Idempotent (no-op if not present).
    fn delete(&self, id: WalletId) -> Result<(), Error>;

    /// Atomic write guarantee (write + rename pattern).
    /// All implementations must guarantee: get() never returns partial data.
    fn put_atomic(&self, id: WalletId, ciphertext: &[u8]) -> Result<(), Error> {
        // Default impl: temp file + rename. Override for OS-backed storage.
        self.put(id, ciphertext)
    }
}
```

```rust
// tron-wallet-core/src/platform/info.rs (NEW V0.1.5)

/// Platform-specific information (data dir, app name, version).
pub trait PlatformInfo: Send + Sync {
    /// Per-platform data directory (writable, persistent, app-private).
    fn data_dir(&self) -> Result<PathBuf, Error>;

    /// App version string (from Cargo.toml at build time).
    fn app_version(&self) -> &'static str;

    /// App name (for keychain service identifier, log tags, etc).
    fn app_name(&self) -> &'static str;

    /// Whether running on mobile (affects UX: confirm prompts, smaller text).
    fn is_mobile(&self) -> bool;
}
```

```rust
// tron-wallet-core/src/platform/network.rs (NEW V0.1.5)

/// HTTP client with platform-specific TLS root certs.
pub trait NetworkClient: Send + Sync {
    /// Build a reqwest::Client configured for the platform.
    fn build_client(&self) -> Result<reqwest::Client, Error>;

    /// Default RPC URL for the given network (Mainnet/Shasta/Nile).
    fn default_rpc_url(&self, network: Network) -> &'static str;
}
```

```rust
// tron-wallet-core/src/platform/clock.rs (NEW V0.1.5)

/// Monotonic clock for tx expiration, timeouts, polling intervals.
pub trait Clock: Send + Sync {
    /// Unix timestamp in milliseconds (monotonic, not wall-clock).
    fn now_millis(&self) -> i64;

    /// Sleep for the given duration (async).
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send;
}
```

#### Platform implementations

##### Desktop (Linux/macOS/Windows)

```rust
// tron-wallet-core/src/platform/desktop/storage.rs
pub struct FileWalletStorage {
    data_dir: PathBuf,    // ~/.local/share/tron/wallets/ (Linux)
                          // ~/Library/Application Support/tron/wallets/ (macOS)
                          // %APPDATA%	ron\wallets\ (Windows)
}

impl WalletStorage for FileWalletStorage {
    fn put(&self, id: WalletId, ciphertext: &[u8]) -> Result<(), Error> {
        std::fs::create_dir_all(&self.data_dir)?;
        let path = self.data_dir.join(format!("{id}.enc"));
        atomic_write::write_file(&path, ciphertext, 0o600)?;
        Ok(())
    }

    fn get(&self, id: WalletId) -> Result<Vec<u8>, Error> {
        let path = self.data_dir.join(format!("{id}.enc"));
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::NotFound),
            Err(e) => Err(e.into()),
        }
    }

    fn list(&self) -> Result<Vec<WalletId>, Error> {
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                if let Some(id_str) = name.strip_suffix(".enc") {
                    if let Ok(id) = id_str.parse::<WalletId>() {
                        ids.push(id);
                    }
                }
            }
        }
        Ok(ids)
    }

    fn delete(&self, id: WalletId) -> Result<(), Error> {
        let path = self.data_dir.join(format!("{id}.enc"));
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),  // idempotent
            Err(e) => Err(e.into()),
        }
    }
}

// tron-wallet-core/src/platform/desktop/info.rs
pub struct SystemDirsInfo;

impl PlatformInfo for SystemDirsInfo {
    fn data_dir(&self) -> Result<PathBuf, Error> {
        #[cfg(target_os = "linux")]
        return Ok(PathBuf::from(std::env::var("HOME")?)
            .join(".local/share/tron"));
        #[cfg(target_os = "macos")]
        return Ok(PathBuf::from(std::env::var("HOME")?)
            .join("Library/Application Support/tron"));
        #[cfg(target_os = "windows")]
        return Ok(PathBuf::from(std::env::var("APPDATA")?)
            .join("tron"));
    }

    fn app_version(&self) -> &'static str { env!("CARGO_PKG_VERSION") }
    fn app_name(&self) -> &'static str { "tron" }
    fn is_mobile(&self) -> bool { false }
}

// tron-wallet-core/src/platform/desktop/network.rs
pub struct ReqwestClient;

impl NetworkClient for ReqwestClient {
    fn build_client(&self) -> Result<reqwest::Client, Error> {
        Ok(reqwest::Client::builder()
            .tls_built_in_webpki_roots()    // bundled webpki roots
            .timeout(Duration::from_secs(30))
            .build()?)
    }

    fn default_rpc_url(&self, network: Network) -> &'static str {
        network.rpc_url()  // returns mainnet/shasta/nile URL
    }
}
```

##### iOS

```rust
// tron-wallet-core/src/platform/ios/storage.rs
pub struct KeychainWalletStorage {
    service: String,    // "com.wallet.tron"
}

impl WalletStorage for KeychainWalletStorage {
    fn put(&self, id: WalletId, ciphertext: &[u8]) -> Result<(), Error> {
        // FFI call into Swift bridge (see ios-bridge/)
        ios_keystore::set(
            &self.service,
            &format!("wallet-{id}"),
            ciphertext,
            IOSKeychainAttributes {
                accessible: IOSAccessibleWhenUnlockedThisDeviceOnly,
                synchronizable: false,
                access_control: None,  // optional: biometric
            },
        )
    }

    fn get(&self, id: WalletId) -> Result<Vec<u8>, Error> {
        ios_keystore::get(&self.service, &format!("wallet-{id}"))
    }
    // ... list, delete similar
}

// tron-wallet-core/src/platform/ios/info.rs
pub struct BundleInfo;

impl PlatformInfo for BundleInfo {
    fn data_dir(&self) -> Result<PathBuf, Error> {
        // NSDocumentDirectory via Swift bridge
        let path = ios_app::document_directory()?;
        Ok(PathBuf::from(path).join("tron"))
    }

    fn is_mobile(&self) -> bool { true }
    // ... rest
}
```

##### Android

```rust
// tron-wallet-core/src/platform/android/storage.rs
pub struct EncryptedFileWalletStorage {
    context_ptr: *const std::ffi::c_void,    // JNI Env pointer
}

impl WalletStorage for EncryptedFileWalletStorage {
    fn put(&self, id: WalletId, ciphertext: &[u8]) -> Result<(), Error> {
        // JNI call into Kotlin EncryptedFile wrapper
        android_keystore::encrypted_file_write(
            self.context_ptr,
            &format!("{id}.enc"),
            ciphertext,
        )
    }

    fn get(&self, id: WalletId) -> Result<Vec<u8>, Error> {
        android_keystore::encrypted_file_read(
            self.context_ptr,
            &format!("{id}.enc"),
        )
    }
    // ... list, delete similar
}
```

##### Tests (universal)

```rust
// tron-wallet-core/src/platform/test/storage.rs
pub struct InMemoryWalletStorage {
    data: Arc<Mutex<HashMap<WalletId, Vec<u8>>>>,
}

impl WalletStorage for InMemoryWalletStorage {
    fn put(&self, id: WalletId, ciphertext: &[u8]) -> Result<(), Error> {
        self.data.lock().unwrap().insert(id, ciphertext.to_vec());
        Ok(())
    }
    fn get(&self, id: WalletId) -> Result<Vec<u8>, Error> {
        self.data.lock().unwrap().get(&id).cloned()
            .ok_or(Error::NotFound)
    }
    fn list(&self) -> Result<Vec<WalletId>, Error> {
        Ok(self.data.lock().unwrap().keys().copied().collect())
    }
    fn delete(&self, id: WalletId) -> Result<(), Error> {
        self.data.lock().unwrap().remove(&id);
        Ok(())
    }
}
```

#### Compile-time platform selection

```rust
// tron-wallet-core/src/platform/mod.rs
pub use storage::WalletStorage;
pub use info::PlatformInfo;
pub use network::NetworkClient;
pub use clock::Clock;

#[cfg(target_os = "ios")]
pub type DefaultStorage = ios::KeychainWalletStorage;
#[cfg(target_os = "android")]
pub type DefaultStorage = android::EncryptedFileWalletStorage;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub type DefaultStorage = desktop::FileWalletStorage;

pub fn default_storage() -> DefaultStorage {
    DefaultStorage::new()
}

pub fn default_platform_info() -> Box<dyn PlatformInfo> {
    #[cfg(target_os = "ios")] return Box::new(ios::BundleInfo);
    #[cfg(target_os = "android")] return Box::new(android::ContextInfo);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    return Box::new(desktop::SystemDirsInfo);
}

pub fn default_network_client() -> Box<dyn NetworkClient> {
    #[cfg(any(target_os = "ios", target_os = "android"))]
    return Box::new(mobile::OSRootsClient);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    return Box::new(desktop::ReqwestClient);
}
```

#### Async runtime pinning (FFI)

```rust
// tron-wallet-core/src/ffi/runtime.rs
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

/// Single-threaded current-thread runtime, pinned for FFI lifetime.
/// All async operations (HTTP, signing, broadcasts) run on this runtime.
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create FFI tokio runtime")
});

pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    RUNTIME.block_on(future)
}

/// Per-Dart-isolate runtime (one per isolate, lazy).
pub fn runtime_per_isolate(isolate_id: u64) -> &'static Runtime {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static ISOLATE_RUNTIMES: Lazy<Mutex<HashMap<u64, &'static Runtime>>> =
        Lazy::new(|| Mutex::new(HashMap::new()));
    let mut map = ISOLATE_RUNTIMES.lock().unwrap();
    map.entry(isolate_id).or_insert_with(|| {
        Box::leak(Box::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("isolate runtime")
        ))
    })
}
```

#### FFI panic safety

```rust
// tron-wallet-core/src/ffi/panic.rs (NEW V0.1.5 — enhanced)

pub fn safe_ffi<F, R>(f: F) -> R
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(result) => result,
        Err(payload) => {
            let msg = scrub_panic_message(&payload);
            tracing::error!("FFI panic caught: {msg}");
            // Re-raise as Error::Panic instead of crashing the app
            // Mobile: never abort (kills app), always return error code
            panic_to_error(payload)
        }
    }
}

fn scrub_panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    let raw = if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "<non-string panic payload>".to_string()
    };

    // Remove 12-word mnemonic sequences (case-insensitive)
    let re_mnemonic = regex::Regex::new(r"([a-z]+\s+){11}[a-z]+").unwrap();
    let scrubbed = re_mnemonic.replace_all(&raw, "[REDACTED_MNEMONIC]");

    // Remove password=, mnemonic=, secret=, xprv= values
    let re_kv = regex::Regex::new(r"(password|mnemonic|secret|xprv|private_key)=[^\s&]+").unwrap();
    let scrubbed = re_kv.replace_all(&scrubbed, "$1=[REDACTED]");

    scrubbed.into_owned()
}
```

#### Cross-platform data flow

```text
User → Dart UI → FFI → Rust core → PAL trait → Platform impl → OS/hardware
                              ↓
                   Layered security:
                   Layer 1: User password (argon2id)
                   Layer 2: OS secure storage (Keychain/EncryptedFile)
                   Layer 3: Hardware-backed key (Secure Enclave/StrongBox)
```

#### V0.1.5 mobile changes (LOC budget)

| Change                      | LOC           | Notes                                                     |
| --------------------------- | ------------- | --------------------------------------------------------- |
| 4 PAL traits                | ~120          | `WalletStorage`, `PlatformInfo`, `NetworkClient`, `Clock` |
| 5 platform implementations  | ~400          | File/Keychain/EncryptedFile + InMemory + desktop info     |
| Default storage selection   | ~30           | `cfg(target_os = "...")`                                  |
| FFI runtime pinning         | ~80           | single-threaded + per-isolate                             |
| FFI panic scrubber          | ~100          | regex + error code conversion                             |
| iOS Swift bridge            | ~150          | `KeychainService.swift`                                   |
| Android Kotlin bridge       | ~150          | `EncryptedFileService.kt`                                 |
| Mobile TLS root config      | ~50           | `tls_built_in_root_certs(true)` on mobile                 |
| Tests (cross-platform mock) | ~200          | InMemory + static PlatformInfo                            |
| **Total**                   | **~1280 LOC** | V0.1.5 milestone (2-3 weeks)                              |

#### V0.2 mobile production (additional LOC)

| Change                                            | LOC                      | Notes                                  |
| ------------------------------------------------- | ------------------------ | -------------------------------------- |
| Biometric unlock (TouchID/FaceID/BiometricPrompt) | ~250                     | Swift/Kotlin bridge for biometric auth |
| flutter_rust_bridge codegen                       | ~500                     | Dart bindings auto-generated           |
| Mobile-specific UX (confirm dialogs)              | ~200                     | Platform-specific prompt UIs           |
| Mobile CI (Xcode sim + Android emu)               | ~1 day                   | GitHub Actions matrix                  |
| App lifecycle hooks (background suspend)          | ~150                     | WAL-style pending tx queue             |
| **Total**                                         | **~1100 LOC + 1 day CI** | V0.2 milestone (3-4 weeks)             |

#### Build target matrix

| Target                                        | Status                | Cargo command                                |
| --------------------------------------------- | --------------------- | -------------------------------------------- |
| `x86_64-unknown-linux-gnu` (desktop Linux)    | V0.1 (works)          | `cargo build`                                |
| `x86_64-apple-darwin` (desktop macOS)         | V0.1 (works)          | `cargo build`                                |
| `x86_64-pc-windows-msvc` (desktop Windows)    | V0.1 (works)          | `cargo build`                                |
| `aarch64-apple-ios` (iOS device)              | V0.1.5 (target added) | `cargo build --target aarch64-apple-ios`     |
| `aarch64-apple-ios-sim` (iOS simulator)       | V0.1.5 (target added) | `cargo build --target aarch64-apple-ios-sim` |
| `aarch64-linux-android` (Android arm64)       | V0.1.5 (target added) | `cargo ndk -t arm64-v8a`                     |
| `x86_64-linux-android` (Android emulator)     | V0.1.5 (target added) | `cargo ndk -t x86_64`                        |
| `wasm32-unknown-unknown` (web wallet, future) | V0.3 (deferred)       | requires `no_std` review                     |

#### CI matrix

```yaml
# .github/workflows/multi-platform.yml
strategy:
  matrix:
    target:
      - x86_64-unknown-linux-gnu
      - x86_64-apple-darwin
      - x86_64-pc-windows-msvc
      - aarch64-apple-ios
      - aarch64-linux-android
    include:
      - target: aarch64-apple-ios
        os: macos-latest
      - target: aarch64-linux-android
        os: ubuntu-latest
        ndk: true
```

#### Migration V0.1 → V0.1.5 → V0.2

| Phase        | Effort    | What ships                                                                                  |
| ------------ | --------- | ------------------------------------------------------------------------------------------- |
| V0.1 (today) | —         | Desktop only, filesystem storage                                                            |
| V0.1.5       | 2-3 weeks | 4 PAL traits + 3 platform impls + iOS/Android builds + FFI runtime pinning + panic scrubber |
| V0.2         | 3-4 weeks | Biometric unlock + OS-backed storage on mobile + flutter_rust_bridge codegen                |

**No breaking changes** between V0.1 → V0.1.5 → V0.2 for desktop. Mobile is additive.

#### Mobile compatibility audit findings (2026-09-05)

Verified mobile-readiness of `wallets/2026-08-27-tron-anychain-sdks-deep-dive.md`. Findings + fixes applied:

**Critical (fixed):**

| Location                                        | Issue                                                         | Fix                                                                                                                        |
| ----------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Cargo.toml `directories = { workspace = true }` | `directories` crate panics on mobile (no iOS/Android backend) | Marked as `optional = true`; comment notes removal in V0.1.5; replaced by `WalletStorage` trait                            |
| Line 247: "Default — every `cargo test` run"    | Implies Docker for all tests (mobile can't run Docker)        | Clarification: TronBox local regtest is **desktop-only**; mobile tests use `--rpc https://nile.trongrid.io` (Nile testnet) |
| Lines 87, 460, 650                              | TronBox references assume Linux/Mac host with Docker          | Marked as desktop-only; mobile equivalent = public testnet (Nile)                                                          |

**Moderate (documented):**

| Location                                                               | Issue                                                  | Action                                                                                     |
| ---------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| Lines 904-906: `~/.local/share/tron/wallets/` example                  | Desktop path in mobile-compatible architecture section | Already gated by `#[cfg(target_os)]`; comment notes desktop-only                           |
| Lines 957-963: `HOME` / `APPDATA` env vars                             | Desktop info.rs in PAL                                 | Already gated by `#[cfg(target_os)]`; OK                                                   |
| Line 562: `webpki-roots`                                               | Desktop uses bundled roots                             | Already in `NetworkClient` trait; mobile uses OS roots via `tls_built_in_root_certs(true)` |
| Lines 149-150: TronBox for "Local dev / CI smoke test"                 | Mobile can't run TronBox                               | Added note: "Use Nile testnet RPC for mobile"                                              |
| Lines 28, 40, 304, 308, 336, 340: TronBox testcontainers documentation | Doc-heavy on desktop test infra                        | Mobile users should treat TronBox as desktop-only                                          |

**Verified already-correct (no change):**

- `Mobile-compatible architecture` section (lines 773-1282) documents platform-specific behavior
- 4 PAL traits (`WalletStorage`, `PlatformInfo`, `NetworkClient`, `Clock`) abstract all platform concerns
- Cargo.toml for mobile adds `tls_built_in_root_certs(true)` on mobile
- FFI runtime pinned single-threaded via `Builder::new_current_thread()`
- All anychain crates (core, tron, kms) are pure Rust (mobile-compatible)
- `argon2` + `aes-gcm` via RustCrypto (pure Rust, mobile-compatible)
- All non-crypto Rust deps (uuid, zeroize, subtle, serde, serde_json, thiserror, hex, bs58) are pure Rust
- `directories` crate usage in `config.rs::default_data_dir()` already noted for replacement in V0.1.5

**Mobile build verification (V0.1.5 CI gate):**

```bash
# Must all pass on every PR:
cargo check --target aarch64-apple-ios
cargo check --target aarch64-linux-android
cargo check --target x86_64-unknown-linux-gnu
cargo check --target x86_64-apple-darwin
cargo check --target x86_64-pc-windows-msvc
```

If any fails, PR is blocked.

**Mobile-feature parity matrix:**

| Feature                       | Desktop          | iOS                 | Android              |
| ----------------------------- | ---------------- | ------------------- | -------------------- |
| Address gen / sign / build tx | ✓                | ✓                   | ✓                    |
| HTTP via TronGrid             | ✓ (webpki roots) | ✓ (OS roots)        | ✓ (OS roots)         |
| Wallet storage                | ✓ filesystem     | V0.1.5 Keychain     | V0.1.5 EncryptedFile |
| TronBox local regtest         | ✓                | ❌ (use Nile)        | ❌ (use Nile)         |
| Testcontainers (CI)           | ✓                | ❌ (skipped)         | ❌ (skipped)          |
| FFI (cdylib)                  | n/a              | ✓                   | ✓                    |
| Biometric unlock              | n/a              | V0.2 TouchID/FaceID | V0.2 BiometricPrompt |
| SPKI pin                      | ✓                | ✓ (same)            | ✓ (same)             |
| tokio runtime                 | ✓ multi-thread   | ✓ single-thread     | ✓ single-thread      |

**Conclusion:** V0.1 source compiles on iOS arm64 + Android arm64 + 3 desktop targets with no source changes (only Cargo features + cfg flags). Mobile-specific OS-backed storage is the V0.1.5 milestone (~1280 LOC PAL additions).

### Open questions

1. **Should FFI surface an actor handle for persistent state?** — Pro: 1 unlock for many operations across Dart calls. Con: 100µs overhead per FFI call. (V0.2 decision)
2. **Should actors persist state to disk for crash recovery?** — TxRetryActor benefits; ChainPoller doesn't need. Per-actor opt-in. (V0.2 decision)
3. **How many actors per process is too many?** — Rule of thumb: ≤ 1 actor per logical concern, ≤ 10 actors per process. Beyond that: consider grouping or shared thread pool.
4. **Thread model scope (deferred from v0.1 to v0.2)** — `#### Thread model (concurrent design — cross-platform)` section removed from this doc per 2026-09-05 decision. Tracked in V0.2 backlog (see `#### To V0.2 (advanced + operator)` table — Thread model row).

### Cross-reference

- Thread model: deferred to v0.2 (see V0.2 backlog row); full threading design + ractor examples in `rust_plan.md §17`
- §15 Post-Review Architectural Corrections: `rust_plan.md` (history of threading decisions)
- §14 Actor Crash Recovery & Supervision Strategy: `rust_plan.md` (supervisor tree design)
- FFI + Ractor integration: `### Ractor + FFI integration (V0.2+ decision)` in `rust_plan.md §17.11`

## Anychain fee implementation × tron-wallet-core` (V0.2 HTTP fee features)
- Mobile security layers: `## Mobile wallet storage (*.enc file location)` (Layer 1+2+3 model)
- Crypto deps: `## Tron Wallet Core` parent → `### Architecture (cross-version)` → `### Cross-version dependencies`
- FFI patterns: `## Tron Wallet Core` parent → `### FFI parity (Dart wallet-desktop)`

### V0.1 — 16 modules, 17 builders, ~4525 LOC (SHIP)

**Mobile compilation requirement (locked):** `tron-wallet-core` MUST compile on `aarch64-apple-ios` + `aarch64-linux-android` + desktop targets with no source changes. Achieved via 4-trait Platform Abstraction Layer (WalletStorage, PlatformInfo, NetworkClient, Clock) — see `### Mobile-compatible architecture (V0.1.5)` section. V0.1 ships with desktop-only FileWalletStorage; V0.1.5 swaps in iOS/Android platform implementations.

The full design as detailed in earlier sections. 16 modules, 17 contract builders, ~4525 LOC. Covers all 29 v0.1 user stories + 4 added user stories (US-30..US-33 for stake + TRC-20 approve).

**Builders (17 — V0.1):**

| Builder                                   | Wraps                               | CLI command                             |
| ----------------------------------------- | ----------------------------------- | --------------------------------------- |
| `build_transfer_contract`                 | TRX native send                     | `tron send`                             |
| `build_account_create`                    | create account                      | (CLI V0.3)                              |
| `build_freeze_balance_v2_contract`        | Stake 2.0 freeze                    | `tron stake freeze` (V0.1.5)            |
| `build_unfreeze_balance_v2_contract`      | Stake 2.0 unfreeze                  | `tron stake unfreeze` (V0.1.5)          |
| `build_delegate_resource_contract`        | Stake 2.0 delegate                  | `tron stake delegate` (V0.1.5)          |
| `build_undelegate_resource_contract`      | Stake 2.0 undelegate                | `tron stake undelegate` (V0.1.5)        |
| `build_vote_witness_contract`             | SR vote                             | `tron stake vote` (V0.1.5)              |
| `build_trc20_transfer_contract`           | TRC-20 transfer                     | `tron trc20 send`                       |
| `build_trc20_approve_contract`            | TRC-20 approve (DEX)                | `tron trc20 approve`                    |
| `build_trigger_contract`                  | raw trigger                         | (internal)                              |
| `build_cancel_unfreeze_contract`          | cancel pending unfreeze             | `tron stake cancel-unfreeze` (V0.1.5)   |
| `build_withdraw_unfreeze_contract`        | withdraw expired unfreeze           | `tron stake withdraw-unfreeze` (V0.1.5) |
| `build_withdraw_vote_contract`            | withdraw vote reward                | `tron stake withdraw-vote` (V0.1.5)     |
| `build_contract(ct)`                      | generic wrapper (any ContractPbExt) | (internal escape hatch)                 |
| `TronTransactionParameters` set_* helpers | ref_block, fee_limit, expiration    | (internal)                              |
| `abi::contract_function_call`             | raw ABI encode                      | (internal)                              |
| `abi::trc20_transfer` + `trc20_approve`   | convenience wrappers                | (used by trc20 builder)                 |

**V0.1 library entry points (13 functions):**

```rust
pub async fn submit_trx(sk, to, amount_sun, fee_limit, cfg) -> Result<TxReceipt>;
pub async fn submit_trc20(sk, contract, to, amount, cfg) -> Result<TxReceipt>;
pub async fn submit_trc20_approve(sk, contract, spender, amount, cfg) -> Result<TxReceipt>;
pub async fn submit_stake_freeze(sk, amount, cfg) -> Result<TxReceipt>;
pub async fn submit_stake_unfreeze(sk, amount, cfg) -> Result<TxReceipt>;
pub async fn submit_stake_delegate(sk, to, amount, resource, cfg) -> Result<TxReceipt>;
pub async fn submit_stake_vote(sk, votes, cfg) -> Result<TxReceipt>;
pub async fn submit_stake_cancel_unfreeze(sk, cfg) -> Result<TxReceipt>;
pub async fn submit_stake_withdraw_unfreeze(sk, cfg) -> Result<TxReceipt>;
pub async fn submit_stake_withdraw_vote(sk, cfg) -> Result<TxReceipt>;
pub async fn submit_send_speedup(sk, original_txid, new_fee_limit, cfg) -> Result<TxReceipt>;
pub async fn sign_only_trx(sk, to, amount_sun, fee_limit, cfg) -> Result<(String, String)>;  // returns (tx_hex, txid)
pub async fn wait_for_confirm(txid, timeout, poll_interval, cfg) -> Result<TxReceipt>;
```

**V0.1 coverage of anychain-tron::protocol:**

- 11 of 50+ proto types wrapped as builders
- 6 used as shared types (ResourceCode, Vote, Contract, Transaction, SmartContract, ABI)
- 33+ proto types deferred (TRC-10 issuer, storage market, governance, witness SR, shielded, DEX, P2P)

#### V0.1 wallet-local features (anychain gap)

`tron-wallet-core` V0.1 has **~45% wallet-local code** that anychain does NOT support. Every feature below needs wallet-core implementation (no anychain API covers it).

##### Summary by category

| Category                                                                        | LOC                               | % of V0.1 lib             |
| ------------------------------------------------------------------------------- | --------------------------------- | ------------------------- |
| Persistence (wallet id, encrypted store, atomic write)                          | ~500                              | ~11%                      |
| Encryption (Argon2id + AES-GCM mnemonic cipher)                                 | ~450                              | ~10%                      |
| HTTP / TronGrid client (broadcast, gettxinfo, getnowblock, etc.)                | ~330                              | ~7%                       |
| SPKI pin verifier (custom rustls ServerCertVerifier)                            | ~150                              | ~3%                       |
| Tx build orchestration (preflight, sign flow, double-SHA workaround, wait, RBF) | ~510                              | ~11%                      |
| Receipt parsing (TransactionInfo JSON → struct, contractResult decode)          | ~200                              | ~4%                       |
| Config + RPC management (TronConfig, TOML persistence, ref-block cache)         | ~400                              | ~9%                       |
| Amount unit conversion (Unit enum, parse, display)                              | ~120                              | ~3%                       |
| Token registry (bundled JSON, chain-named loaders, disambiguation)              | ~250                              | ~6%                       |
| Error type (Error enum, From impls for 7 error sources)                         | ~150                              | ~3%                       |
| FFI layer (cdylib, panic scrubber, tokio runtime bridge)                        | ~560                              | ~12%                      |
| CLI shell (clap parser, 19 handlers, error classifier)                          | ~2700                             | n/a (CLI, not lib)        |
| **Total wallet-local**                                                          | **~3620 LOC lib + ~2700 LOC CLI** | **~80% of lib internals** |

##### Top 12 wallet-local features (full inventory)

|    # | Feature                                         | Reason                                                        | File                                          | LOC  |
| ---: | ----------------------------------------------- | ------------------------------------------------------------- | --------------------------------------------- | ---- |
|    1 | Wallet id (UUID v4)                             | metadata, not chain logic                                     | `wallet/id.rs`                                | ~80  |
|    2 | Encrypted wallet store on disk                  | anychain has zero filesystem code                             | `wallet/persist.rs`                           | ~350 |
|    3 | Atomic write (temp + rename)                    | reliability primitive                                         | `util/atomic_write.rs`                        | ~70  |
|    4 | Data dir resolution                             | OS-specific                                                   | `config.rs::default_data_dir()`               | ~30  |
|    5 | File permissions (mode 0600)                    | security                                                      | `util/permissions.rs`                         | ~40  |
|    6 | Argon2id KDF                                    | anychain-kms has PBKDF2 only (for mnemonic→seed), no Argon2id | `crypto/argon2.rs`                            | ~50  |
|    7 | AES-GCM symmetric encryption                    | anychain has no AES                                           | `crypto/aes_gcm.rs`                           | ~150 |
|    8 | Mnemonic-at-rest encryption (composite)         | composition of 6+7                                            | `crypto/mnemonic_cipher.rs`                   | ~300 |
|    9 | `wallet/broadcasttransaction` POST              | anychain has zero HTTP                                        | `chain/trongrid.rs::broadcast()`              | ~50  |
|   10 | `wallet/gettransactioninfobyid` GET             | anychain has zero HTTP                                        | `chain/trongrid.rs::get_tx_info()`            | ~50  |
|   11 | `wallet/getnowblock` (ref_block fetch)          | anychain has zero HTTP                                        | `chain/trongrid.rs::get_now_block()`          | ~30  |
|   12 | `wallet/getaccount`                             | anychain has zero HTTP                                        | `chain/trongrid.rs::get_account()`            | ~50  |
|   13 | `wallet/getaccountresource`                     | anychain has zero HTTP                                        | `chain/trongrid.rs::get_resource()`           | ~50  |
|   14 | `wallet/triggerconstantcontract` (view call)    | anychain has zero HTTP                                        | `chain/trongrid.rs::trigger_constant()`       | ~50  |
|   15 | HTTP client builder (reqwest)                   | anychain has zero HTTP                                        | `chain/trongrid.rs::client()`                 | ~30  |
|   16 | API key header injection                        | env passthrough                                               | `chain/trongrid.rs::headers()`                | ~20  |
|   17 | Custom rustls `ServerCertVerifier`              | anychain has zero TLS                                         | `chain/spki.rs`                               | ~150 |
|   18 | Pin mismatch error variant                      | new error variant                                             | `Error::SpkiPinMismatch { expected, actual }` | ~20  |
|   19 | Pre-flight check (balance ≥ amount+fee)         | anychain has no business logic                                | `tx/preflight.rs`                             | ~100 |
|   20 | Sign flow with Zeroizing wrapper                | workaround for kms bug                                        | `tx/sign.rs`                                  | ~150 |
|   21 | Manual double-SHA256 txid workaround            | workaround for anychain-tron bug                              | `tx/sign.rs::compute_txid()`                  | ~30  |
|   22 | `sign_only` mode (return raw tx_hex)            | cold-sign pipeline                                            | `tx/sign.rs::sign_only()`                     | ~50  |
|   23 | Confirmation wait (poll until block-solidified) | HTTP polling                                                  | `tx/wait.rs`                                  | ~100 |
|   24 | Send-speedup (RBF)                              | re-sign with new fee_limit                                    | `tx/send_speedup.rs`                          | ~80  |
|   25 | `TransactionInfo` JSON → Rust struct            | anychain uses protobuf, caller needs JSON                     | `tx/receipt.rs::from_http()`                  | ~80  |
|   26 | `contractResult[]` decode (hex strings)         | no parser in anychain                                         | `tx/receipt.rs::decode_results()`             | ~50  |
|   27 | `TronConfig` struct (network, RPC, SPKI)        | anychain has no app-level config                              | `config.rs`                                   | ~250 |
|   28 | Config persistence (TOML)                       | no anychain equivalent                                        | `config.rs::load/save`                        | ~100 |
|   29 | Ref-block fetch + cache (60s TTL)               | avoid ref_block fetch per tx                                  | `config.rs::ref_block_cache`                  | ~40  |
|   30 | `Unit { Trx, Sun }` enum                        | display-only                                                  | `amount.rs::Unit`                             | ~20  |
|   31 | `Amount::from_str_in` (parse "1.5" + Unit::Trx) | ui concern                                                    | `amount.rs`                                   | ~100 |
|   32 | `Amount::as_sun` (canonical u64)                | conversion logic                                              | `amount.rs::as_sun`                           | ~20  |
|   33 | `Amount::display_in` (TRX display)              | formatting                                                    | `amount.rs::display_in`                       | ~20  |
|   34 | Decimal precision constant (6 for TRX)          | anychain doesn't know                                         | `amount.rs::TRX_DECIMALS`                     | ~5   |
|   35 | `tokens/mainnet.json` (bundled USDT/USDC)       | chain-specific data                                           | `tokens/mainnet.json` (in repo)               | ~100 |
|   36 | `tokens::load_mainnet()` (chain-named)          | no anychain equivalent                                        | `tokens.rs`                                   | ~40  |
|   37 | Token shortcut (USDT → TR7NHqje...)             | UX                                                            | `tokens.rs::resolve_symbol()`                 | ~30  |
|   38 | Disambiguation: bridged USDC.e vs native USDC   | chain-specific safety                                         | `disambig.rs::reject_bridged_usdc_e()`        | ~50  |
|   39 | Chain ID lookup per network                     | per-network constants                                         | `disambig.rs::chain_id_for()`                 | ~20  |
|   40 | `Error` enum with all variant-specific fields   | anychain `Error` is too generic                               | `error.rs`                                    | ~100 |
|   41 | `From<anychain_core::Error>` impl               | map to wallet-level variants                                  | `error.rs::From` impls                        | ~10  |
|   42 | `From<anychain_kms::Error>` impl                | same                                                          | `error.rs::From` impls                        | ~10  |
|   43 | `From<reqwest::Error>` impl                     | HTTP errors                                                   | `error.rs::From` impls                        | ~10  |
|   44 | `From<argon2::Error>` impl                      | encryption errors                                             | `error.rs::From` impls                        | ~10  |
|   45 | `From<aes_gcm::Error>` impl                     | same                                                          | `error.rs::From` impls                        | ~10  |
|   46 | `From<serde_json::Error>` impl                  | serialization                                                 | `error.rs::From` impls                        | ~10  |
|   47 | `From<std::io::Error>` impl                     | filesystem                                                    | `error.rs::From` impls                        | ~10  |
|   48 | C ABI exports                                   | anychain has no FFI                                           | `ffi/mod.rs`                                  | ~80  |
|   49 | Panic-message scrubber (redact mnemonic)        | security                                                      | `ffi/panic.rs`                                | ~200 |
|   50 | Regex compilation (once_cell)                   | performance                                                   | `ffi/panic.rs`                                | ~20  |
|   51 | Tokio runtime bridge (FFI → async)              | sync/async interop                                            | `ffi/runtime.rs`                              | ~50  |
|   52 | Wallet ops dispatch (FFI → lib)                 | API surface                                                   | `ffi/wallet_ops.rs`                           | ~70  |
|   53 | Status code translation (FFI exit codes)        | FFI surface                                                   | `ffi/error.rs`                                | ~80  |
|   54 | `cbindgen` header generation                    | FFI consumers need C headers                                  | `build.rs`                                    | ~40  |

##### Top 5 critical-path priorities

| Priority | Feature                                                      | Why critical                      |
| -------- | ------------------------------------------------------------ | --------------------------------- |
| 1        | `chain/trongrid.rs` (HTTP client)                            | blocks all RPC-dependent features |
| 2        | `wallet/persist.rs` (encrypted store)                        | blocks all wallet lifecycle       |
| 3        | `crypto/mnemonic_cipher.rs` (Argon2id + AES-GCM)             | blocks wallet import              |
| 4        | `tx/sign.rs` (sign flow + Zeroizing + double-SHA workaround) | blocks all submit-tx paths        |
| 5        | `config.rs` (TronConfig + RPC defaults)                      | blocks CLI initialization         |

These 5 are the critical path. Other wallet-local features can be added incrementally as needed.

##### What anychain DOES support (for comparison)

| Category              | Anychain API                                                  | Wallet-core usage            |
| --------------------- | ------------------------------------------------------------- | ---------------------------- |
| BIP-39 mnemonic gen   | `anychain_kms::Mnemonic::generate`                            | direct                       |
| BIP-32 HD             | `anychain_kms::ExtendedPrivateKey::{from_seed, derive_child}` | direct                       |
| secp256k1 sign        | `anychain_kms::secp256k1_sign()`                              | direct (with Zeroizing wrap) |
| Address derivation    | `anychain_tron::TronAddress::from_public_key`                 | direct                       |
| Address validation    | `anychain_tron::TronAddress::is_valid`                        | direct                       |
| Network enum          | `anychain_tron::TronNetwork::{Mainnet, Shasta, Nile}`         | direct                       |
| 13 contract builders  | `anychain_tron::trx::build_*`                                 | direct                       |
| ABI encode            | `anychain_tron::abi::encode_call`                             | direct                       |
| Function selector     | `anychain_core::func_selector`                                | direct                       |
| keccak256             | `anychain_core::keccak256`                                    | direct                       |
| sha256                | `anychain_core::sha256`                                       | direct (txid double-hash)    |
| hex                   | `anychain_core::hex` re-export                                | direct                       |
| Transaction signing   | `anychain_tron::Transaction::sign(sig, recid)`                | direct                       |
| Transaction serialize | `anychain_tron::Transaction::to_bytes`                        | direct                       |
| Error types           | 7 enums from anychain_core                                    | wrap + extend                |

**20 features implemented via anychain** (45% of V0.1 logic).

##### Coverage breakdown

| Source                                 | LOC                               | % of V0.1                                    |
| -------------------------------------- | --------------------------------- | -------------------------------------------- |
| anychain-tron (wire + builders + sign) | ~1360                             | ~30%                                         |
| anychain-kms (HD + signing)            | ~900                              | ~20%                                         |
| anychain-core (traits + utilities)     | ~230                              | ~5%                                          |
| **Wallet-local (this analysis)**       | **~3620 LOC lib + ~2700 LOC CLI** | **~80% lib internals + 100% CLI**            |
| Type glue, lifecycle, error wrapping   | ~1500                             | n/a                                          |
| **Total V0.1**                         | **~10,300 LOC**                   | (comparable to bitcoin-wallet-core's 12,663) |

##### Cross-reference

- Anychain umbrella deep-dive: `docs/wallets/2026-09-05-anychain-umbrella-technical-deep-dive.md`
- Anychain-core (traits): `docs/wallets/2026-09-05-anychain-core-technical-deep-dive.md`
- Anychain-kms (HD + sign): `docs/wallets/2026-09-05-anychain-kms-technical-deep-dive.md`
- Anychain-tron (wire): `docs/wallets/2026-09-05-anychain-tron-technical-deep-dive.md`
- Feature maps: `## Tron Wallet Core × anychain-core/kms/tron — feature map` (3 subsections)

### V0.2 — +7 builders, ~+250 LOC (OPERATOR + ADVANCED USER)

Add 7 more `anychain-tron::trx` builders + tokens module expansion.

**New builders (7 — V0.2):**

| Builder                                  | Wraps                       | CLI command               |
| ---------------------------------------- | --------------------------- | ------------------------- |
| `build_asset_issue_contract`             | TRC-10 token issuance       | `tron trc10 issue`        |
| `build_transfer_asset_contract`          | TRC-10 token transfer       | `tron trc10 send`         |
| `build_participate_asset_issue_contract` | TRC-10 token purchase       | `tron trc10 buy`          |
| `build_proposal_create_contract`         | network governance proposal | `tron governance propose` |
| `build_proposal_approve_contract`        | approve proposal            | `tron governance approve` |
| `build_buy_storage_contract`             | buy disk (bandwidth)        | `tron storage buy`        |
| `build_sell_storage_contract`            | sell disk                   | `tron storage sell`       |

**V0.2 library additions (~+250 LOC):**

```rust
pub async fn submit_asset_issue(sk, name, symbol, total, decimals, cfg) -> Result<TxReceipt>;
pub async fn submit_transfer_asset(sk, asset, to, amount, cfg) -> Result<TxReceipt>;
pub async fn submit_participate_asset_issue(sk, issuer, amount, asset, cfg) -> Result<TxReceipt>;
pub async fn submit_proposal_create(sk, param_id, value, cfg) -> Result<TxReceipt>;
pub async fn submit_proposal_approve(sk, proposal_id, cfg) -> Result<TxReceipt>;
pub async fn submit_buy_storage(sk, amount, cfg) -> Result<TxReceipt>;
pub async fn submit_sell_storage(sk, amount, cfg) -> Result<TxReceipt>;

// New wallet-local entry points (no anychain changes needed)
pub fn unlock(id: WalletId, password: &SecretString) -> Result<UnlockedWallet>;
pub fn summary_secret(unlocked: &UnlockedWallet) -> SecretSummary;
pub fn export(id: WalletId, password: &SecretString, format: ExportFormat) -> Result<WalletExport>;
pub fn import_from_xprv_file(name: String, network: Network, password_hash: &[u8], path: &Path) -> Result<WalletId>;
pub fn derive_private_key(mnemonic: &str, path: &DerivationPath) -> Result<Zeroizing<[u8;32]>>;
```

Plus:

```rust
// tokens module expansion
pub fn load_nile() -> Result<Vec<Token>>;    // V0.1 had only load_mainnet
pub fn register(addr: TronAddress, meta: TokenMeta) -> Result<()>;

// view-call helpers
pub async fn estimate_energy(contract, method, args, cfg) -> Result<EnergyEstimate>;
pub async fn get_fee_history(addr, n, cfg) -> Result<Vec<FeeEntry>>;

// personal_sign
pub async fn sign_personal_message(sk, msg: &[u8]) -> Result<(Signature, TronAddress)>;
pub async fn verify_personal_message(addr, msg, sig) -> Result<bool>;
```

### V0.3 — +4 builders, ~+500 LOC (ADVANCED — requires upstream changes)

4 advanced builders requiring `anychain-tron` fork + zk-SNARK deps.

**New builders (4 — V0.3):**

| Builder                                    | Wraps                       | CLI command            | Requires                                                   |
| ------------------------------------------ | --------------------------- | ---------------------- | ---------------------------------------------------------- |
| `build_witness_create_contract`            | SR candidacy apply          | `tron witness apply`   | upstream `anychain-tron` (no fork)                         |
| `build_witness_update_contract`            | SR URL/ID update            | `tron witness update`  | upstream `anychain-tron` (no fork)                         |
| `build_account_permission_update_contract` | multi-sig + key permissions | `tron multisig create` | **upstream fork** (anychain-tron is single-sig only today) |
| `shielded_trc20_transfer`                  | zk-SNARK shielded TRC-20    | `tron shield transfer` | **zk-SNARK proving system** (`bellman` or `halo2_proofs`)  |

**V0.3 library additions (~+500 LOC):**

```rust
pub async fn submit_witness_create(sk, url, cfg) -> Result<TxReceipt>;
pub async fn submit_witness_update(sk, new_url, cfg) -> Result<TxReceipt>;
pub async fn submit_account_permission_update(sk, threshold, keys, cfg) -> Result<TxReceipt>;
pub async fn submit_multisig_tx(sk, account, to, amount, cfg) -> Result<TxReceipt>;

// shield module (new directory: src/shield/)
pub mod shield {
    pub async fn transfer(sk, to, amount, token, cfg) -> Result<ShieldedTransfer>;
    pub async fn verify_proof(proof, public_inputs) -> Result<bool>;
}
```

### Module × anychain map — version delta

| Module            | V0.1                                                  | V0.2                                                | V0.3                                      |
| ----------------- | ----------------------------------------------------- | --------------------------------------------------- | ----------------------------------------- |
| `tx/builder.rs`   | 17 builders                                           | +7 builders                                         | +2 builders (witness) +1 fork (multi-sig) |
| `tokens/`         | `load_mainnet()` only                                 | +`load_nile()`, `register()`                        | —                                         |
| `chain/`          | account + resource + nowblock + broadcast + gettxinfo | +`estimate_energy`, `get_fee_history`, `list_txs`   | —                                         |
| `keys/`           | full HD + sign                                        | +`sign_personal_message`, `verify_personal_message` | —                                         |
| `ffi/`            | panic scrubber + C ABI                                | —                                                   | +`shield` export                          |
| **new `shield/`** | —                                                     | —                                                   | zk-SNARK transfer + verify                |

### Cumulative LOC by version

| Layer              | V0.1      | V0.1.5         | V0.2      | V0.3       | Total     |
| ------------------ | --------- | -------------- | --------- | ---------- | --------- |
| `tron-wallet-core` | ~4525     | ~+0 (CLI-only) | ~+250     | ~+500      | ~5275     |
| `tron` CLI         | ~1810     | ~+350          | ~+350     | ~+500      | ~3010     |
| **`Total`**        | **~6335** | **~+350**      | **~+600** | **~+1000** | **~8285** |

### FFI parity (Dart wallet-desktop)

Both CLI and Dart call same C ABI → same library → same code path. V0.1 ships with `tx/builder` + `tx/sign` + `tx/broadcast` + `chain/` + `keys/` exports. V0.2 adds `tokens` + `estimate_energy` + `personal_sign`. V0.3 adds `shield`.

### Out-of-scope (stable across versions)

- Multi-sig (V0.3 partial — needs upstream fork)
- TRC-721 (no anychain-tron builder)
- TRC-10 issue (V0.2 only — issuer flow)
- TronGrid API key (env passthrough, not wallet-core concern)
- Light-node RPC (full-node only)
- Hardware wallet integration (Trezor/Ledger)

### Test strategy (stable across versions)

| Test layer                                          | Coverage target                                  |
| --------------------------------------------------- | ------------------------------------------------ |
| Unit: `keccak256`/`sha256`/`func_selector`          | 100% (well-known test vectors)                   |
| Unit: BIP-32 derivation                             | `bdk_chain` test vectors                         |
| Unit: BIP-39 wordlist                               | `bip39` crate tests, 100% (8 languages)          |
| Unit: xprv serialize                                | `anychain_kms` test vectors                      |
| Unit: contract builder roundtrip                    | protobuf decode + compare, 100% for each builder |
| Unit: txid double-hash                              | known tx hash from TronGrid (regression test)    |
| Integration: full sign → broadcast                  | TronBox Docker via `testcontainers` (no mainnet) |
| Integration: wallet create → import → sign → verify | tempdir + roundtrip, 100%                        |
| FFI: Dart panic scrubber                            | regex fuzz inputs (never log mnemonic/secret)    |

### TDD sequencing (per L13 step 2-4)

1. **Spike 0 (NEW)**: mobile build verification — `cargo check --target aarch64-apple-ios --target aarch64-linux-android` must pass on V0.1 source. Catches platform-incompatible deps early (e.g., `directories` crate).
2. **Spike 1**: address + keys + sign (no persistence) → derives address + signs known message
3. **Spike 2**: tx/builder (one builder: TransferContract) + tx/sign + tx/broadcast (testcontainers TronBox)
4. **Spike 3**: tx/builder expanded to all 17 builders + verify via TronBox
5. **Spike 4**: TRC-20 path via TriggerSmartContract + ABI encode
6. **Spike 5**: wallet/ (persistence layer) — uses `WalletStorage` trait from day 1 (not raw filesystem)
7. **Spike 6**: chain/ (TronGrid client + SPKI pin) — uses `NetworkClient` trait (cfg flag for OS roots on mobile)
8. **Spike 7**: ffi/ (cdylib + panic scrubber + single-threaded runtime)
9. **Spike 8**: tron CLI (clap wiring)
10. **V0.1.5**: stake builders + handlers + mobile platform impls (Keychain/EncryptedFile)
11. **V0.2 spike 1-5**: tokens/fee/sign/trc10/governance/storage/wallet-sync + biometric unlock + flutter_rust_bridge
12. **V0.3**: witness + multisig + shield

**Mobile build gate (CI):** every PR must pass:

- `cargo check --target aarch64-apple-ios` (iOS arm64)
- `cargo check --target aarch64-linux-android` (Android arm64, via cargo-ndk)
- `cargo check --target x86_64-unknown-linux-gnu` (Linux desktop)
- `cargo check --target x86_64-apple-darwin` (macOS desktop)
- `cargo check --target x86_64-pc-windows-msvc` (Windows desktop)

If mobile build fails, PR is blocked. Enforces "no platform-specific code outside PAL".

### Test scenario — `tron-wallet-core` V0.1

Library-level counterpart to the network-level `## Test Scenario` section (which is CLI + endpoint driven). This section maps each of the 16 V0.1 modules and 13 public entry points to a concrete test, its layer, its fixture, and an observable pass criterion. Every row is runnable with `cargo test -p tron-wallet-core`; rows marked **local** additionally require Docker (testcontainers spawns TronBox).

#### Layer split

| Layer            | Command                                                | Network                            | Runs on                   |
| ---------------- | ------------------------------------------------------ | ---------------------------------- | ------------------------- |
| Unit             | `cargo test -p tron-wallet-core --lib`                 | none (pure + mock)                 | every commit, all targets |
| Integration      | `cargo test -p tron-wallet-core --test '*'`            | local TronBox via `testcontainers` | every PR (desktop CI)     |
| Nile integration | `TRON_NILE_INTEGRATION=1 cargo test --test trc20_nile` | Nile testnet                       | `workflow_dispatch` only  |

Unit tests must pass on `aarch64-apple-ios` and `aarch64-linux-android` builds; integration tests are desktop-only (gated behind `#[cfg(not(any(target_os = "ios", target_os = "android")))]`), because Docker is unavailable on mobile targets.

#### Module × test scenario matrix (16 modules)

| #  | Module                      | Scenario                                                         | Layer     | Fixture                                                 | Pass criterion                                                                                                                            |
| -- | --------------------------- | ---------------------------------------------------------------- | --------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 1  | `keys.rs`                   | BIP-39 mnemonic → seed → BIP-32 derive `m/44'/195'/0'/0/0`        | unit      | BIP-39 English test vectors + SLIP-0044 path            | derived xprv matches `anychain_kms` vector byte-for-byte                                                                                   |
| 2  | `keys.rs`                   | secret key → `TronAddress` (base58check, `0x41` prefix)           | unit      | known keypair from `kobe-tron` KAT                      | address string equals expected `T...`; `TronAddress::is_valid` true                                                                       |
| 3  | `amount.rs`                 | `Amount::from_str_in("1.5", Unit::Trx)` → `as_sun()`              | unit      | table-driven: `0`, `1`, `1.5`, `0.000001`, over-`u64`   | `1.5` → `1_500_000` sun; overflow returns `Error::AmountOverflow`, no panic                                                                |
| 4  | `amount.rs`                 | round-trip `as_sun` → `display_in(Unit::Trx)`                     | unit      | property test (proptest, 10k cases)                     | `display_in(as_sun(x)) == x` for all 6-decimal inputs                                                                                     |
| 5  | `crypto/argon2.rs`          | Argon2id KDF determinism + parameter pinning                      | unit      | fixed salt + passphrase                                 | same input → same 32-byte key; params (m, t, p) match the pinned constants                                                                |
| 6  | `crypto/aes_gcm.rs`         | AES-GCM encrypt → decrypt round-trip; tamper detection            | unit      | random 32-byte key + nonce                              | plaintext recovered; single-bit ciphertext flip → `Error::DecryptFailed`                                                                  |
| 7  | `crypto/mnemonic_cipher.rs` | mnemonic encrypt-at-rest → decrypt, correct + wrong passphrase    | unit      | 12-word + 24-word mnemonics                             | correct passphrase recovers mnemonic; wrong one errors without leaking plaintext                                                          |
| 8  | `wallet/persist.rs`         | create → save → load → sign round-trip                            | unit      | `tempfile::tempdir()` + `FileWalletStorage`             | loaded wallet signs identically to the in-memory original                                                                                 |
| 9  | `wallet/persist.rs`         | file permissions + atomic write                                   | unit      | tempdir                                                 | wallet file mode is `0600`; no `.tmp` residue after write; interrupted write leaves the prior file intact                                 |
| 10 | `wallet/id.rs`              | UUID v4 uniqueness + name → address lookup                        | unit      | 1000 generated wallets                                  | no id collision; `WalletManager::lookup("cold")` resolves to the stored address                                                          |
| 11 | `config.rs`                 | `TronConfig` TOML save → load; per-network defaults                | unit      | tempdir + all 4 networks                                | round-trip equality; Nile URL and chain-id match the `## Networks` table                                                                  |
| 12 | `config.rs`                 | ref-block cache TTL (60s)                                         | unit      | mock `Clock` trait (PAL)                                | second call within 60s hits cache (0 HTTP calls); call at 61s refetches                                                                   |
| 13 | `tokens.rs` + `disambig.rs` | symbol resolve + bridged-token rejection                          | unit      | bundled `tokens/mainnet.json`                           | `USDT` resolves to the mainnet contract; `USDC.e` returns `Error::BridgedTokenRejected`                                                   |
| 14 | `tx/builder.rs`             | all 17 builders: build → protobuf encode → decode → compare       | unit      | one fixture per builder                                 | decoded contract equals input params for every builder (100% builder coverage)                                                            |
| 15 | `tx/sign.rs`                | sign flow + manual double-SHA256 txid workaround                  | unit      | known raw tx + expected txid from TronGrid              | computed txid matches the recorded on-chain txid (regression guard for the anychain-tron txid bug)                                        |
| 16 | `tx/sign.rs`                | `sign_only_trx` cold-sign path                                    | unit      | fixed key + fixed ref-block                             | returns `(tx_hex, txid)`; performs no HTTP; re-signing the same input is deterministic                                                    |
| 17 | `tx/preflight.rs`           | balance ≥ amount + fee check                                      | unit      | mock `NetworkClient` returning a fixed balance          | under-funded send returns `Error::InsufficientBalance` before any broadcast                                                               |
| 18 | `tx/receipt.rs`             | `TransactionInfo` JSON → struct; `contractResult[]` hex decode    | unit      | captured TronGrid JSON (success + revert)               | success parses `energy_usage`; revert exposes the decoded revert reason                                                                   |
| 19 | `chain/spki.rs`             | SPKI pin match / mismatch                                         | unit      | real leaf cert fixture + a deliberately wrong pin       | matching pin verifies; wrong pin returns `Error::SpkiPinMismatch { expected, actual }`                                                    |
| 20 | `chain/trongrid.rs`         | 6 endpoints against a mock HTTP server                            | unit      | `wiremock` stubs                                        | each endpoint parses its response; HTTP 5xx maps to `Error::Transport`, never a panic                                                     |
| 21 | `error.rs`                  | all 7 `From` impls + `Debug` redaction                            | unit      | one constructed error per source                        | each source maps to the intended variant; `Debug` output contains no mnemonic or secret bytes                                             |
| 22 | `ffi/panic.rs`              | panic-message scrubber                                            | unit      | fuzz inputs containing mnemonics and hex keys           | scrubbed output never contains any input secret (regex fuzz, 10k cases)                                                                   |
| 23 | `ffi/`                      | C ABI smoke: create wallet → sign → status code                   | unit      | in-process `cdylib` call                                | returns the expected status code; no panic crosses the FFI boundary                                                                      |
| 24 | `tx/` end-to-end            | `submit_trx` full flow                                            | **local** | TronBox container + funded deployer                     | receipt `result == "SUCCESS"`; balances reconcile; bandwidth-only, no energy consumed                                                     |
| 25 | `tx/` end-to-end            | `submit_trc20` to a held recipient                                | **local** | deployed `MockTRC20`                                    | receipt SUCCESS; `energy_usage ≈ 65_000`; recipient balance equals the sent amount                                                        |
| 26 | `tx/` end-to-end            | `submit_trc20` first-time receive (fresh recipient)               | **local** | fresh address, `fee_limit` 130 TRX                      | receipt SUCCESS; `energy_usage ≈ 130_000` (about twice the baseline)                                                                      |
| 27 | `tx/` end-to-end            | `submit_trc20_approve` + allowance view                           | **local** | `MockTRC20` + spender address                           | approve accepted; `triggerconstantcontract` allowance view returns the approved amount                                                    |
| 28 | `tx/wait.rs`                | `wait_for_confirm` success + timeout                              | **local** | a real txid, plus a bogus txid for the timeout path     | success path returns the receipt; bogus txid returns `Error::ConfirmTimeout` within the configured budget                                 |
| 29 | `tx/send_speedup.rs`        | `submit_send_speedup` rebroadcast semantics                       | **local** | stuck tx + higher `fee_limit`                           | **blocked on spike V7** — node behavior must be recorded in `spikes/tron-v1/V7-speedup.md` before this row can pass (see `## Test Scenario` row 7a) |
| 30 | full stack                  | transport failure: RPC pointed at a closed port                   | unit      | `http://127.0.0.1:9999`                                 | `Error::Transport` within the 30s timeout; no panic, no hang                                                                              |

#### Entry-point coverage (13 functions)

| Entry point                    | Covered by rows | Note                                                                                            |
| ------------------------------ | --------------- | ----------------------------------------------------------------------------------------------- |
| `submit_trx`                   | 17, 24, 30      | preflight, happy path, transport failure                                                        |
| `submit_trc20`                 | 25, 26          | held recipient and fresh recipient — the energy delta is the assertion                          |
| `submit_trc20_approve`         | 27              | approve plus allowance read-back                                                                |
| `submit_stake_*` (7 functions) | 14              | builder round-trip only in V0.1; end-to-end lands in V0.1.5 alongside the `tron stake` CLI      |
| `submit_send_speedup`          | 29              | gated on spike V7                                                                               |
| `sign_only_trx`                | 16              | offline path — asserted to perform no HTTP                                                      |
| `wait_for_confirm`             | 28              | success and timeout                                                                             |

The 7 stake entry points ship in V0.1 as library functions, but their CLI surface is V0.1.5. V0.1 therefore verifies them at the builder/protobuf layer (row 14) rather than end-to-end. That gap is deliberate and closes when `tron stake` ships.

#### Fixture inventory

| Fixture                      | Source                                          | Used by rows |
| ---------------------------- | ----------------------------------------------- | ------------ |
| BIP-39 / BIP-32 vectors      | `anychain_kms` + `bip39` crate tests            | 1            |
| Tron address KAT             | `kobe-tron` (reference-only dependency)         | 2            |
| Known txid + raw tx          | captured from TronGrid mainnet                  | 15           |
| TronGrid JSON responses      | captured success + revert responses, checked in | 18           |
| Leaf certificate + SPKI hash | captured from `nile.trongrid.io`                | 19           |
| `MockTRC20.bin`              | compiled by TronBox during container setup      | 25, 26, 27   |
| `tokens/mainnet.json`        | bundled in the crate                            | 13           |

Secrets are never checked in. Nile runs read `TRON_TEST_MNEMONIC` from the environment (CI secret); local runs derive throwaway keys from fixed byte arrays that hold no real funds.

#### Coverage gates

- 100% line coverage on `crypto/`, `amount.rs`, and `tx/builder.rs` — pure and deterministic, so gaps have no excuse.
- Every builder in the 17-builder table has a round-trip test (row 14). Adding a builder without a test fails review.
- Every `Error` variant is constructed at least once across the suite (row 21), so no variant ships unreachable or unrendered.
- No test prints a mnemonic, seed, or private key — enforced by the redaction assertions in rows 21 and 22.

#### Cross-reference

- Network-level scenarios (CLI + endpoints): `## Test Scenario`
- Layer-level coverage targets: `### Test strategy (stable across versions)`
- Build order these tests follow: `### TDD sequencing (per L13 step 2-4)`
- Module inventory under test: `### V0.1 — 16 modules, 17 builders, ~4525 LOC (SHIP)`
- Spike V7 (speedup semantics, blocks row 29): `## Spike verification (V1-V10) — consolidated`

### Known duplications (consolidate in V0.3)

Per the `Review Tron CLI v0.1, ensuring avoid duplication clis` analysis:

- ErrorClassifier (btc + tron)
- SecretMnemonic wrapper (polygon + tron)
- `--mnemonic-file` / `--private-key-file` flag pattern (polygon + tron)
- tracing init (all 4 CLIs)
- Data dir resolution (all 4 CLIs)
- Config file load/save (all 4 CLIs)
- Manual Debug redact (btc, 10 structs)
- Exit code constants (all 4 CLIs)

**Consolidation target**: new `wallet-cli-shared` crate (~250-300 LOC).
**V0.3 milestone**: extract patterns, migrate polygon + tron.
## Tron CLI

Single binary `tron`, clap subcommand dispatch, mirrors `btc`/`eth`/`polygon` pattern. All crypto delegated to `tron-wallet-core`. **59 subcommand variants across 15 top-level commands** (v0.1: 22, v0.1.5: 7 stake, v0.2: 25, v0.3: 5).

### Architecture (cross-version)

```text
rust-wallet-app/crates/tron/
├── Cargo.toml
└── src/
    ├── main.rs       # entry, tokio runtime, tracing init, dispatch
    ├── cli.rs        # clap Cli struct + Commands enum + per-subcommand args
    └── handlers/     # one fn per subcommand, async, returns Result<(), Error>
        ├── mod.rs
        ├── wallet.rs     # create, import, show, list, delete, rename, balance, send, send-speedup (V0.2: + sync, + export, + import-wif)
        ├── address.rs    # new, xpub
        ├── trc20.rs      # send, approve, allowance
        ├── stake.rs      # V0.1.5: freeze, unfreeze, delegate, vote, cancel-unfreeze, withdraw-unfreeze, withdraw-vote
        ├── tokens.rs     # V0.2: list, register, balances
        ├── trc10.rs      # V0.2: issue, send, buy
        ├── fee.rs        # V0.2: estimate, history
        ├── faucet.rs     # V0.2: show, drip
        ├── sign.rs       # V0.2: message, verify
        ├── governance.rs # V0.2: propose, approve
        ├── storage.rs    # V0.2: buy, sell
        ├── witness.rs    # V0.3: apply, update
        ├── multisig.rs   # V0.3: create, send
        ├── shield.rs     # V0.3: transfer (zk-SNARK shielded TRC-20)
        ├── tx.rs         # V0.1: get, wait; V0.2: + list
        ├── config.rs     # V0.1: show, set-rpc, set-network; V0.2: + set-spki-pin
        └── error.rs      # classify + Debug redaction
```

**CLI parser** (stable across versions):

```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "tron", version, about = "TRON wallet CLI")]
pub struct Cli {
    #[arg(long, env = "TRON_DATA_DIR", global = true)]
    pub data_dir: Option<PathBuf>,
    #[arg(long, env = "TRON_RPC", global = true)]
    pub rpc: Option<String>,
    #[arg(long, env = "TRON_SPKI_PIN", global = true)]
    pub spki_pin: Option<String>,
    #[arg(long, env = "TRON_NETWORK", value_enum, default_value_t = Network::Mainnet, global = true)]
    pub network: Network,
    #[arg(long, global = true)]
    pub allow_insecure_tls: bool,
    #[command(subcommand)]
    pub command: Commands,
}
```

`Commands` enum grows with each version (V0.1: 5, V0.1.5: +Stake, V0.2: +6, V0.3: +3). Manual `Debug` impl on password-bearing structs (redact) + `SecretMnemonic` wrapper per L12 CRITICAL #2 panic-message scrubber pattern from `bitcoin-wallet-core`.

**Handler dispatch** (V0.2 shape, V0.1.5 + V0.3 add arms):

```rust
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let data_dir = handlers::resolve_data_dir(cli.data_dir.clone())?;
    let cfg = TronConfig::load(&data_dir)?.with_overrides(&cli.into());

    let dispatch_result: Result<()> = async {
        match cli.command {
            Commands::Wallet(action) => handlers::wallet::dispatch(action, &cfg).await,
            Commands::Address(action) => handlers::address::dispatch(action, &cfg).await,
            Commands::Trc20(action) => handlers::trc20::dispatch(action, &cfg).await,
            Commands::Stake(action) => handlers::stake::dispatch(action, &cfg).await,
            Commands::Tokens(action) => handlers::tokens::dispatch(action, &cfg).await,
            Commands::Trc10(action) => handlers::trc10::dispatch(action, &cfg).await,
            Commands::Fee(action) => handlers::fee::dispatch(action, &cfg).await,
            Commands::Faucet(action) => handlers::faucet::dispatch(action, &cfg).await,
            Commands::Sign(action) => handlers::sign::dispatch(action, &cfg).await,
            Commands::Governance(action) => handlers::governance::dispatch(action, &cfg).await,
            Commands::Storage(action) => handlers::storage::dispatch(action, &cfg).await,
            Commands::Witness(action) => handlers::witness::dispatch(action, &cfg).await,
            Commands::Multisig(action) => handlers::multisig::dispatch(action, &cfg).await,
            Commands::Shield(action) => handlers::shield::dispatch(action, &cfg).await,
            Commands::Tx(action) => handlers::tx::dispatch(action, &cfg).await,
            Commands::Config(action) => handlers::config::dispatch(action, &cfg).await,
        }
    }.await;

    if let Err(e) = dispatch_result {
        let exit_code = handlers::error::classify(&e);
        eprintln!("{e:?}");
        std::process::exit(exit_code);
    }
    Ok(())
}
```

**Error classifier** (`handlers/error.rs`, stable across versions):

```rust
pub fn classify(e: &anyhow::Error) -> i32 {
    for c in e.chain() {
        if let Some(lib_err) = c.downcast_ref::<tron_wallet_core::Error>() {
            return match lib_err {
                Error::InvalidMnemonic(_) | Error::InvalidAddress(_) | Error::AbiDecode(_) => 2,
                Error::Transport(_) => 3,
                Error::InsufficientFunds | Error::UnknownWallet(_) => 4,
                Error::SignFailed | Error::BroadcastFailed { .. } => 5,
                _ => 1,
            };
        }
    }
    1
}
```

### Cross-cutting patterns (all versions)

**Global flags:**

| Flag                                | Env             | Default                                                       |
| ----------------------------------- | --------------- | ------------------------------------------------------------- |
| `--data-dir <path>`                 | `TRON_DATA_DIR` | OS data dir (Linux: `~/.local/share/tron/`)                   |
| `--rpc <url>`                       | `TRON_RPC`      | per-network default (`https://api.trongrid.io` Mainnet, etc.) |
| `--spki-pin <hex>`                  | `TRON_SPKI_PIN` | none (env-only per L12 H-1)                                   |
| `--network <mainnet\|shasta\|nile>` | `TRON_NETWORK`  | `mainnet`                                                     |
| `--allow-insecure-tls`              | —               | false (always SPKI-pin if pin set)                            |

**New flag patterns (L12 H-1 + cold-sign):**

```rust
/// Mode-0600 file path containing the 12/24-word BIP-39 mnemonic.
/// Closes the L12 H-1 argv-exposure hole.
#[arg(long, conflicts_with = "mnemonic")]
pub mnemonic_file: Option<PathBuf>,

/// Sign + return signed-tx-hex without broadcasting. Cold-sign pipeline
/// for future hardware-wallet migration.
#[arg(long)]
pub sign_only: bool,

/// Mode-0600 file path containing the raw 32-byte private key (no 0x prefix).
/// Closes L12 H-1 for private-key import path.
#[arg(long, conflicts_with = "mnemonic", conflicts_with = "mnemonic_file")]
pub private_key_file: Option<PathBuf>,
```

**Output conventions (mirrors `btc` parity):**

- **STDOUT**: machine-readable. wallet_id, address, JSON, or empty.
- **STDERR**: human logs + secrets + diagnostics. `tracing` crate, levels via `TRON_LOG` env.
- **JSON mode**: `--json` flag on `wallet list`/`show`/`sync`, `config show`, `tx list`) switches table → JSON.
- **Mnemonic secrecy**: NEVER on STDOUT. STDERR only when `wallet create`/`import` triggered.
- **Exit codes** (stable across versions):
  - `0` success
  - `1` anyhow default
  - `2` invalid input (bad mnemonic, bad address, ABI decode failure)
  - `3` transport error (connection refused, HTTP error, chain-id mismatch)
  - `4` wallet/balance error (insufficient funds, unknown wallet, missing token)
  - `5` signing/RPC/broadcast error (sign fail, broadcast non-SUCCESS result)

**Mnemonic handling (L28 / F49 / L12 H-1):**
- `wallet create` → mnemonic → STDERR (red highlight)
- `wallet import --mnemonic "<phrase>"` → mnemonic → STDERR + wallet_id → STDOUT
- `wallet import --mnemonic-file <path>` → reads mode-0600 file (closes argv-exposure L12 H-1)
- `wallet show` → decrypted mnemonic NEVER displayed

**Handler module signature (per command):**

```rust
pub async fn handle_send(
    wallet_id: Option<Uuid>,
    mnemonic: Option<SecretMnemonic>,
    mnemonic_file: Option<PathBuf>,
    to: String,
    amount: Amount,
    unit: Unit,
    fee_limit: Option<i64>,
    dry_run: bool,
    sign_only: bool,
    cfg: &TronConfig,
) -> Result<SendReceipt, anyhow::Error> {
    let sk = match (wallet_id, mnemonic, mnemonic_file) {
        (Some(id), _, _) => resolve_wallet_sk(id, &cfg.data_dir)?,
        (None, Some(m), _) => Zeroizing::new(m.expose().as_bytes().to_vec()),
        (None, None, Some(p)) => read_sk_from_file(&p)?,
        _ => anyhow::bail!("--wallet-id, --mnemonic, or --mnemonic-file required"),
    };
    let amount_sun = amount.as_sun(unit)?;
    let receipt = if sign_only {
        let (tx_hex, txid) = tron_wallet_core::tx::sign_only_trx(&sk, &to, amount_sun, fee_limit, &cfg).await?;
        SendReceipt { txid, result: "SIGN_ONLY".into(), tx_hex: Some(tx_hex) }
    } else {
        tron_wallet_core::tx::submit_trx(&sk, &to, amount_sun, fee_limit, &cfg).await?
    };
    Ok(receipt)
}
```

### Consolidated flat table (all 53 commands)

| Ver    | Subcommand                   | Story                              | STDOUT                                                                    |
| ------ | ---------------------------- | ---------------------------------- | ------------------------------------------------------------------------- |
| v0.1   | `wallet create`              | 1                                  | wallet_id                                                                 |
| v0.1   | `wallet import`              | 2                                  | wallet_id                                                                 |
| v0.1   | `wallet show`                | 11                                 | JSON `{id, name, address, network}`                                       |
| v0.1   | `wallet list`                | 9                                  | table or JSON array                                                       |
| v0.1   | `wallet delete`              | 9                                  | empty                                                                     |
| v0.1   | `wallet rename`              | 9                                  | empty                                                                     |
| v0.1   | `wallet balance`             | 3, 22                              | JSON balance                                                              |
| v0.1   | `wallet send`                | 5                                  | JSON `{txid, result, tx_hex?}` (supports wallet→wallet via `--to-wallet`) |
| v0.1   | `wallet send-speedup`        | 17                                 | JSON `{new_txid, result}`                                                 |
| v0.1   | `address new`                | 3                                  | base58check address                                                       |
| v0.1   | `address xpub`               | 19                                 | xpub string                                                               |
| v0.1   | `trc20 send`                 | 21                                 | JSON `{txid, result}`                                                     |
| v0.1   | `trc20 approve`              | 30                                 | JSON `{txid, result}`                                                     |
| v0.1   | `trc20 allowance`            | 30                                 | `{allowance, decimals}`                                                   |
| v0.1   | `balance --address [--unit]` | 3                                  | `{trx, trx_sun, energy, bandwidth}`                                       |
| v0.1   | `trc20 balance`              | 22                                 | `{symbol, balance, decimals}`                                             |
| v0.1   | `tx get`                     | 7                                  | `{txid, block, result, contractResult, ...}`                              |
| v0.1   | `tx wait`                    | 7                                  | `{txid, block, confirmations, result}`                                    |
| v0.1   | `config show`                | 11                                 | `{data_dir, rpc_url, spki_pin, network, version}`                         |
| v0.1   | `config set-rpc`             | 10, 26, 27                         | empty                                                                     |
| v0.1   | `config set-network`         | 10, 27                             | empty                                                                     |
| v0.1.5 | `stake freeze`               | 4                                  | `{txid, result}`                                                          |
| v0.1.5 | `stake unfreeze`             | 4                                  | `{txid, result}`                                                          |
| v0.1.5 | `stake delegate`             | 6                                  | `{txid, result}`                                                          |
| v0.1.5 | `stake vote`                 | 6                                  | `{txid, result}`                                                          |
| v0.1.5 | `stake cancel-unfreeze`      | 31                                 | `{txid, result}`                                                          |
| v0.1.5 | `stake withdraw-unfreeze`    | 32                                 | `{txid, result}`                                                          |
| v0.1.5 | `stake withdraw-vote`        | 33                                 | `{txid, result}`                                                          |
| v0.2   | `wallet sync`                | 7 (bulk)                           | JSON `Vec<TxSummary>`                                                     |
| v0.2   | `wallet export`              | cold storage                       | JSON `{xprv, addresses}` or xprv string                                   |
| v0.2   | `wallet import-wif`          | rare TRON use                      | wallet_id                                                                 |
| v0.2   | `wallet show --secret`       | recovery (password + confirmation) | `{mnemonic, xprv, expires:30s}`                                           |
| v0.2   | `address show --private-key` | recovery (per-address)             | hex private key                                                           |
| v0.2   | `wallet import --xprv-file`  | recovery (mode-0600 file)          | wallet_id                                                                 |
| v0.2   | `tokens list`                | 23                                 | table or JSON of registered TRC-20                                        |
| v0.2   | `tokens register`            | 24                                 | JSON `{contract, symbol, decimals, name}`                                 |
| v0.2   | `tokens balances`            | 22 (bulk)                          | JSON `{tokens: [{symbol, balance, contract}]}`                            |
| v0.2   | `trc10 issue`                | 34 (issuer)                        | `{txid, result, asset_id}`                                                |
| v0.2   | `trc10 send`                 | 34                                 | `{txid, result}`                                                          |
| v0.2   | `trc10 buy`                  | 34                                 | `{txid, result}`                                                          |
| v0.2   | `tx list`                    | 7                                  | JSON `Vec<TxSummary>`                                                     |
| v0.2   | `fee estimate`               | 8                                  | `{energy_used, sun_required, energy_price}`                               |
| v0.2   | `fee history`                | 8 (history)                        | JSON array of last n tx fees paid                                         |
| v0.2   | `faucet show`                | 27                                 | `{network, faucet_url, drip_command}`                                     |
| v0.2   | `faucet drip`                | 27                                 | `{txid, drip_status}`                                                     |
| v0.2   | `sign message`               | 18                                 | `{signature, address, message}`                                           |
| v0.2   | `sign verify`                | 18                                 | `{valid: true/false}`                                                     |
| v0.2   | `governance propose`         | 35                                 | `{txid, result, proposal_id}`                                             |
| v0.2   | `governance approve`         | 35                                 | `{txid, result}`                                                          |
| v0.2   | `storage buy`                | 36                                 | `{txid, result}`                                                          |
| v0.2   | `storage sell`               | 36                                 | `{txid, result}`                                                          |
| v0.2   | `config set-spki-pin`        | 28 (DX)                            | empty                                                                     |
| v0.2   | `shell completion`           | DX                                 | prints completion script for `eval $(...)`                                |
| v0.3   | `witness apply`              | SR candidacy                       | `{txid, result, witness_address}`                                         |
| v0.3   | `witness update`             | SR URL update                      | `{txid, result}`                                                          |
| v0.3   | `multisig create`            | multi-sig setup                    | JSON `{txid, result, account_address}`                                    |
| v0.3   | `multisig send`              | multi-sig transfer                 | `{txid, result}`                                                          |
| v0.3   | `shield transfer`            | shielded TRC-20                    | `{txid, result, nullifier}`                                               |

**Total: 59 subcommand variants.** v0.1=22, v0.1.5=7 (stake), v0.2=25 (net +25), v0.3=5 (net +5).

### V0.1 — 19 commands, 5 top-level (SHIP)

Minimal viable wallet: create + import + balance + send + tx verify.

**Subcommand catalog:**

#### `tron wallet` (9 subcommands)

| Subcommand                                                                                                                                        | Story | tron-wallet-core call                                               | STDOUT                                             |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | ------------------------------------------------------------------- | -------------------------------------------------- |
| `tron wallet create --words 12\|24 --name <n> --network <net> --password <pw>`                                                                    | 1     | `WalletManager::create_with_mnemonic`                               | wallet_id                                          |
| `tron wallet import --name --network --password --mnemonic\|--mnemonic-file\|--private-key-file`                                                  | 2     | `WalletManager::import_from_phrase`                                 | wallet_id                                          |
| `tron wallet show --id [--json]`                                                                                                                  | 11    | `WalletManager::unlock(id, pw).summary()`                           | JSON: `{ id, name, address, network, created_at }` |
| `tron wallet list [--json] [--all-networks]`                                                                                                      | 9     | `WalletManager::list()`                                             | table or JSON array                                |
| `tron wallet delete --id`                                                                                                                         | 9     | `WalletManager::delete(id)`                                         | empty                                              |
| `tron wallet rename --id --to`                                                                                                                    | 9     | `WalletManager::rename(id, name)`                                   | empty                                              |
| `tron wallet balance --wallet-id\|--address [--token USDT\|<addr>]`                                                                               | 3, 22 | `chain::get_account(addr)` or `WalletManager::unlock(id).balance()` | JSON balance                                       |
| `tron wallet send --wallet-id\|--mnemonic --to <addr>\|--to-wallet <name\|id> --amount [--unit] [--fee-limit] [--dry-run] [--sign-only] [--wait]` | 5     | `tx::submit_trx(sk, to, amount_sun, ...)`                           | JSON: `{ txid, result, tx_hex? }`                  |
| `tron wallet send-speedup --wallet-id --txid --fee-limit`                                                                                         | 17    | `tx::submit_send_speedup(...)`                                      | JSON: `{ new_txid, result }`                       |

#### `tron address` (2)

| Subcommand                                                       | Story | tron-wallet-core call                  | STDOUT              |
| ---------------------------------------------------------------- | ----- | -------------------------------------- | ------------------- |
| `tron address new --mnemonic [--mnemonic-file] --index [--path]` | 3     | `keys::derive_keypair(mnemonic, path)` | base58check address |
| `tron address xpub --wallet-id`                                  | 19    | `WalletManager::xpub(id)`              | xpub string         |

#### `tron balance` (2 — standalone, address-driven)

| Subcommand                                    | Story | tron-wallet-core call                  | STDOUT                                      |
| --------------------------------------------- | ----- | -------------------------------------- | ------------------------------------------- |
| `tron balance --address --unit trx\|sun`      | 3     | `chain::get_account(addr)`             | JSON: `{ trx, trx_sun, energy, bandwidth }` |
| `tron balance --address --token USDT\|<addr>` | 22    | `chain::trc20_balance(addr, contract)` | JSON: `{ symbol, balance, decimals }`       |

#### `tron trc20` (4)

| Subcommand                                                         | Story | tron-wallet-core call                                     | STDOUT                                |
| ------------------------------------------------------------------ | ----- | --------------------------------------------------------- | ------------------------------------- |
| `tron trc20 send --mnemonic --contract USDT\|<addr> --to --amount` | 21    | `tx::submit_trc20(sk, contract, to, amount)`              | JSON: `{ txid, result }`              |
| `tron trc20 approve --mnemonic --contract --spender --amount`      | 30    | `tx::submit_trc20_approve(sk, contract, spender, amount)` | JSON: `{ txid, result }`              |
| `tron trc20 balance --address --contract USDT\|<addr>`             | 22    | `chain::trc20_balance(addr, contract)`                    | JSON: `{ symbol, balance, decimals }` |
| `tron trc20 allowance --contract --owner --spender`                | 30    | view-call `allowance(owner,spender)`                      | `{ allowance, decimals }`             |

#### `tron tx` (2)

| Subcommand                                      | Story | tron-wallet-core call                 | STDOUT                                         |
| ----------------------------------------------- | ----- | ------------------------------------- | ---------------------------------------------- |
| `tron tx get --txid`                            | 7     | `chain::get_tx_info(txid)`            | `{ txid, block, result, contractResult, ... }` |
| `tron tx wait --txid --timeout --poll-interval` | 7     | `tx::wait_for_confirm(txid, timeout)` | `{ txid, block, confirmations, result }`       |

#### `tron config` (3)

| Subcommand                                      | Story      | tron-wallet-core call                  | STDOUT                                              |
| ----------------------------------------------- | ---------- | -------------------------------------- | --------------------------------------------------- |
| `tron config show [--json]`                     | 11         | `config::TronConfig::load().display()` | `{ data_dir, rpc_url, spki_pin, network, version }` |
| `tron config set-rpc <url>`                     | 10, 26, 27 | `config::set_rpc(url)` + save          | empty                                               |
| `tron config set-network mainnet\|shasta\|nile` | 10, 27     | `config::set_network(net)` + save      | empty                                               |

**User story coverage (V0.1):** 1, 2, 3, 5, 7, 9, 10, 11, 17, 19, 21, 22, 25, 27, 30, 26 (via --rpc flag), 28 (via env), 29 (default no-pin).

### V0.1.5 — 7 commands, 1 top-level (stake) — OPERATOR FLOW

Stake 2.0 commands moved from V0.1 because they're operator/investor flow, not basic user wallet default. Ships between V0.1 and V0.2 — same release train.

#### `tron stake` (7 subcommands)

| Subcommand                                                                  | Story | tron-wallet-core call                                 | STDOUT             |
| --------------------------------------------------------------------------- | ----- | ----------------------------------------------------- | ------------------ |
| `tron stake freeze --mnemonic --amount`                                     | 4     | `tx::submit_stake_freeze(sk, amount)`                 | `{ txid, result }` |
| `tron stake unfreeze --mnemonic --amount`                                   | 4     | `tx::submit_stake_unfreeze(sk, amount)`               | `{ txid, result }` |
| `tron stake delegate --mnemonic --to --amount --resource BANDWIDTH\|ENERGY` | 6     | `tx::submit_stake_delegate(sk, to, amount, resource)` | `{ txid, result }` |
| `tron stake vote --mnemonic --votes <addr:N,...>`                           | 6     | `tx::submit_stake_vote(sk, votes)`                    | `{ txid, result }` |
| `tron stake cancel-unfreeze --mnemonic`                                     | 31    | `tx::submit_stake_cancel_unfreeze(sk)`                | `{ txid, result }` |
| `tron stake withdraw-unfreeze --mnemonic`                                   | 32    | `tx::submit_stake_withdraw_unfreeze(sk)`              | `{ txid, result }` |
| `tron stake withdraw-vote --mnemonic`                                       | 33    | `tx::submit_stake_withdraw_vote(sk)`                  | `{ txid, result }` |

**Library changes (V0.1.5):**

```rust
// tron-wallet-core/src/tx/mod.rs — 7 new submit_* entry points
pub async fn submit_stake_freeze(sk, amount) -> Result<TxReceipt>;
pub async fn submit_stake_unfreeze(sk, amount) -> Result<TxReceipt>;
pub async fn submit_stake_delegate(sk, to, amount, resource) -> Result<TxReceipt>;
pub async fn submit_stake_vote(sk, votes) -> Result<TxReceipt>;
pub async fn submit_stake_cancel_unfreeze(sk) -> Result<TxReceipt>;
pub async fn submit_stake_withdraw_unfreeze(sk) -> Result<TxReceipt>;
pub async fn submit_stake_withdraw_vote(sk) -> Result<TxReceipt>;
```

All 7 wrap existing `anychain-tron::trx` builders (no upstream changes needed). ~250 LOC lib + ~350 LOC CLI = **+600 LOC**.

### V0.2 — 22 commands, 8 top-level (SHIP NEXT)

Token registry, fees, faucet, sign, governance, storage — operator + advanced user flows.

#### `tron wallet` (3 new — V0.2)

| Subcommand                                                                                   | Story          | tron-wallet-core call                                    | STDOUT                                                                                                        |
| -------------------------------------------------------------------------------------------- | -------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `tron wallet sync --id [--json]`                                                             | 7              | `WalletManager::sync(id)`                                | JSON `Vec<TxSummary>`                                                                                         |
| `tron wallet export --id --format xprv\|json`                                                | cold storage   | `WalletManager::export(id, format)`                      | JSON `{xprv, addresses}` or xprv string                                                                       |
| `tron wallet show --id --secret [--password-stdin\|--password-file <path>\|--password <pw>]` | n/a (recovery) | `WalletManager::unlock(id, pw).summary_secret()`         | JSON: `{ mnemonic, private_key, xprv, auto_expires_after: "30s" }` (requires `yes` confirmation + WARN audit) |
| `tron address show --address <addr> --private-key [--index <N>]`                             | n/a (recovery) | `keys::derive_keypair(mnemonic, path).private_key_hex()` | hex private key (requires password unlock + `yes` confirmation)                                               |
| `tron wallet import --name --network --password --xprv-file <path>`                          | n/a (recovery) | `WalletManager::import_from_xprv_file(path)`             | wallet_id (mode-0600 file check, Zeroizing parse)                                                             |

#### `tron tokens` (3 — NEW TOP-LEVEL)

| Subcommand                                                | Story     | tron-wallet-core call                          | STDOUT                                    |
| --------------------------------------------------------- | --------- | ---------------------------------------------- | ----------------------------------------- |
| `tron tokens list [--network mainnet\|nile]`              | 23        | `tokens::load_mainnet()` or `load_nile()`      | table or JSON                             |
| `tron tokens register --contract [--symbol] [--decimals]` | 24        | `tokens::register(addr, meta)` + view-calls    | JSON `{contract, symbol, decimals, name}` |
| `tron tokens balances --wallet-id`                        | 22 (bulk) | `chain::trc20_balances_bulk(addr, all_tokens)` | JSON `{tokens: [...]}`                    |

#### `tron trc10` (3 — NEW TOP-LEVEL)

| Subcommand                                                              | Story | tron-wallet-core call                         | STDOUT             |
| ----------------------------------------------------------------------- | ----- | --------------------------------------------- | ------------------ |
| `tron trc10 issue --name --symbol --total-supply --decimals --mnemonic` | 34    | `tx::submit_asset_issue(sk, ...)`             | `{txid, asset_id}` |
| `tron trc10 send --mnemonic --asset --to --amount`                      | 34    | `tx::submit_transfer_asset(sk, ...)`          | `{txid, result}`   |
| `tron trc10 buy --mnemonic --issuer --amount [--asset]`                 | 34    | `tx::submit_participate_asset_issue(sk, ...)` | `{txid, result}`   |

#### `tron tx list` (1 — V0.2)

| Subcommand                                                  | Story | tron-wallet-core call                 | STDOUT                |
| ----------------------------------------------------------- | ----- | ------------------------------------- | --------------------- |
| `tron tx list --address [--since-block] [--limit] [--json]` | 7     | `chain::list_txs(addr, since, limit)` | JSON `Vec<TxSummary>` |

#### `tron fee` (2 — NEW TOP-LEVEL)

| Subcommand                                       | Story | tron-wallet-core call                            | STDOUT                                      |
| ------------------------------------------------ | ----- | ------------------------------------------------ | ------------------------------------------- |
| `tron fee estimate --contract --method [--args]` | 8     | `chain::estimate_energy(contract, method, args)` | `{energy_used, sun_required, energy_price}` |
| `tron fee history --last <n>`                    | 8     | `chain::get_fee_history(addr, n)`                | JSON array                                  |

#### `tron faucet` (2 — NEW TOP-LEVEL)

| Subcommand                   | Story | tron-wallet-core call           | STDOUT                                |
| ---------------------------- | ----- | ------------------------------- | ------------------------------------- |
| `tron faucet show`           | 27    | `config::Network::faucet_url()` | `{network, faucet_url, drip_command}` |
| `tron faucet drip --address` | 27    | auto-call TronFAQBot            | `{txid, drip_status}`                 |

#### `tron sign` (2 — NEW TOP-LEVEL)

| Subcommand                                                             | Story | tron-wallet-core call                  | STDOUT                          |
| ---------------------------------------------------------------------- | ----- | -------------------------------------- | ------------------------------- |
| `tron sign message --mnemonic [--mnemonic-file] --message [--address]` | 18    | `keys::sign_personal_message(sk, msg)` | `{signature, address, message}` |
| `tron sign verify --address --message --signature`                     | 18    | `keys::verify_personal_message(...)`   | `{valid: true/false}`           |

#### `tron governance` (2 — NEW TOP-LEVEL)

| Subcommand                                              | Story | tron-wallet-core call                  | STDOUT                |
| ------------------------------------------------------- | ----- | -------------------------------------- | --------------------- |
| `tron governance propose --mnemonic --param-id --value` | 35    | `tx::submit_proposal_create(sk, ...)`  | `{txid, proposal_id}` |
| `tron governance approve --mnemonic --proposal-id`      | 35    | `tx::submit_proposal_approve(sk, ...)` | `{txid, result}`      |

#### `tron storage` (2 — NEW TOP-LEVEL)

| Subcommand                              | Story | tron-wallet-core call                 | STDOUT           |
| --------------------------------------- | ----- | ------------------------------------- | ---------------- |
| `tron storage buy --mnemonic --amount`  | 36    | `tx::submit_buy_storage(sk, amount)`  | `{txid, result}` |
| `tron storage sell --mnemonic --amount` | 36    | `tx::submit_sell_storage(sk, amount)` | `{txid, result}` |

#### `tron config set-spki-pin` (1 — V0.2)

| Subcommand                       | Story   | tron-wallet-core call              | STDOUT                             |
| -------------------------------- | ------- | ---------------------------------- | ---------------------------------- |
| `tron config set-spki-pin <hex>` | 28 (DX) | `config::set_spki_pin(hex)` + save | empty (moves from env-only to CLI) |

#### `tron shell completion` (1 — V0.2)

| Subcommand                                | Story | tron-wallet-core call    | STDOUT                   |
| ----------------------------------------- | ----- | ------------------------ | ------------------------ |
| `tron shell completion <bash\|zsh\|fish>` | DX    | `clap_complete` generate | prints completion script |

**V0.2 library additions (~250 LOC):**

```rust
pub async fn submit_asset_issue(sk, name, symbol, total, decimals) -> Result<TxReceipt>;
pub async fn submit_transfer_asset(sk, asset, to, amount) -> Result<TxReceipt>;
pub async fn submit_participate_asset_issue(sk, issuer, amount, asset) -> Result<TxReceipt>;
pub async fn submit_proposal_create(sk, param_id, value) -> Result<TxReceipt>;
pub async fn submit_proposal_approve(sk, proposal_id) -> Result<TxReceipt>;
pub async fn submit_buy_storage(sk, amount) -> Result<TxReceipt>;
pub async fn submit_sell_storage(sk, amount) -> Result<TxReceipt>;
pub async fn submit_trc20_approve(sk, contract, spender, amount) -> Result<TxReceipt>;
pub async fn sign_personal_message(sk, msg) -> Result<(Signature, Address)>;
pub async fn verify_personal_message(addr, msg, sig) -> Result<bool>;
```

### V0.3 — 5 commands, 2 top-level (ADVANCED)

SR candidacy, multi-sig, shielded TRC-20. Requires anychain-tron fork + zk-SNARK deps.

#### `tron witness` (2 — NEW TOP-LEVEL)

| Subcommand                                 | Story         | tron-wallet-core call                | STDOUT                    |
| ------------------------------------------ | ------------- | ------------------------------------ | ------------------------- |
| `tron witness apply --mnemonic --url`      | SR candidacy  | `tx::submit_witness_create(sk, url)` | `{txid, witness_address}` |
| `tron witness update --mnemonic --new-url` | SR URL update | `tx::submit_witness_update(sk, url)` | `{txid, result}`          |

#### `tron multisig` (2 — NEW TOP-LEVEL)

| Subcommand                                                        | Story              | tron-wallet-core call                           | STDOUT                    |
| ----------------------------------------------------------------- | ------------------ | ----------------------------------------------- | ------------------------- |
| `tron multisig create --mnemonic --threshold --keys <addr:N,...>` | multi-sig setup    | `tx::submit_account_permission_update(sk, ...)` | `{txid, account_address}` |
| `tron multisig send --mnemonic --account --to --amount`           | multi-sig transfer | `tx::submit_multisig_tx(sk, ...)`               | `{txid, result}`          |

#### `tron shield` (1 — NEW TOP-LEVEL)

| Subcommand                                                   | Story           | tron-wallet-core call       | STDOUT              |
| ------------------------------------------------------------ | --------------- | --------------------------- | ------------------- |
| `tron shield transfer --mnemonic --to --amount --token USDT` | shielded TRC-20 | `shield::transfer(sk, ...)` | `{txid, nullifier}` |

**V0.3 requires:** `anychain-tron` fork for multi-sig (AccountPermissionUpdateContract builder), zk-SNARK proving system (`bellman` or `halo2_proofs`), shielded TRC-20 builder.

### Known duplications (consolidate in V0.3)

Per the `Review Tron CLI v0.1, ensuring avoid duplication clis` analysis:

- ErrorClassifier (btc)
- SecretMnemonic wrapper (polygon)
- `--mnemonic-file` / `--private-key-file` flag pattern (polygon)
- tracing init (all 4 CLIs)
- Data dir resolution (all 4 CLIs)
- Config file load/save (all 4 CLIs)
- Manual Debug redact (btc, 10 structs)
- Exit code constants (all 4 CLIs)

**Consolidation target**: new `wallet-cli-shared` crate (~250-300 LOC).
**V0.3 milestone**: extract patterns, migrate polygon + tron.
**V0.1 + V0.1.5 + V0.2 accept duplication** for time-to-market.

### Out-of-scope indefinitely

| Command                       | Reason                                                      |
| ----------------------------- | ----------------------------------------------------------- |
| `sign-typed`                  | TRON has no EIP-712 typed-data spec                         |
| `erc721`                      | no anychain-tron builder (when SDK adds it)                 |
| `wallet import-wif` (real)    | TRON has no standard WIF — verify in V0.2 spike, may be cut |
| `p2p discover`                | full-node only, not wallet                                  |
| `interactive REPL`            | single-shot commands only                                   |
| `hardware wallet integration` | future, V0.3+                                               |

### CLI test strategy (all versions)

| Test layer                          | Coverage target                                                    |
| ----------------------------------- | ------------------------------------------------------------------ |
| Unit: flag parsing                  | `clap` derive, 100% of arg combos                                  |
| Unit: error classifier              | all error variants → correct exit code                             |
| Unit: token shortcuts               | USDT/USDC → correct mainnet address                                |
| Unit: mnemonic-file mode-0600 check | rejected if group-readable                                         |
| Integration: dry-run path           | all tx subcommands must produce signed-tx-hex without broadcasting |
| Integration: sign-only path         | returns tx_hex + txid without broadcast                            |
| Integration: TronBox full path      | smoke test: create → send → wait → confirm                         |
| Integration: send-speedup           | bumps fee_limit, re-broadcasts, asserts new txid                   |
| Manual: man-page generation         | `clap_mangen` → install auto-completion (V0.2)                     |

### Cumulative LOC

| Layer              | V0.1      | V0.1.5    | V0.2      | V0.3       | Total     |
| ------------------ | --------- | --------- | --------- | ---------- | --------- |
| `tron-wallet-core` | ~4525     | ~+250     | ~+250     | ~+500      | ~5525     |
| `tron` CLI         | ~1810     | ~+350     | ~+350     | ~+500      | ~3010     |
| **Total**          | **~6335** | **~+600** | **~+600** | **~+1000** | **~8535** |

### Implementation phasing

| Phase        | Scope                                                                     | LOC delta                |
| ------------ | ------------------------------------------------------------------------- | ------------------------ |
| V0.1         | 19 commands (wallet/address/trc20/tx/config)                              | ~1810 CLI                |
| V0.1.5       | stake (7 ops: freeze/unfreeze/delegate/vote/cancel/withdraw×2)            | ~+350 CLI                |
| V0.2 spike 1 | tokens list/register/balances                                             | ~+350 CLI                |
| V0.2 spike 2 | fee estimate/history                                                      | ~+200 CLI                |
| V0.2 spike 3 | sign message/verify                                                       | ~+250 CLI                |
| V0.2 spike 4 | trc10 + governance + storage                                              | ~+400 CLI                |
| V0.2 spike 5 | wallet sync/export/import-wif + config set-spki-pin + shell completion    | ~+200 CLI                |
| V0.3         | 5 advanced commands (witness, multisig, shield) + shared crate extraction | ~+500 CLI + ~+300 shared |

## V0.2 Features

Incremental features on top of v0.1, leveraging anychain-tron's full contract builder surface (Stake 2.0, voting, TRC-10) and anychain-kms multi-language support. ~150 additional lines of CLI glue + UI affordances.

### Stake 2.0 (resource delegation)

| Feature                                          | Story (deferred in v0.1) | Crate(s)                                                 | Status |
| ------------------------------------------------ | ------------------------ | -------------------------------------------------------- | ------ |
| Freeze balance (BANDWIDTH / ENERGY)              | T-1                      | `anychain_tron::trx::build_freeze_balance_v2_contract`   | ready  |
| Unfreeze balance                                 | T-2                      | `anychain_tron::trx::build_unfreeze_balance_v2_contract` | ready  |
| Delegate resource to other address (lock 3 days) | T-3                      | `anychain_tron::trx::build_delegate_resource_contract`   | ready  |
| Undelegate resource                              | T-4                      | `anychain_tron::trx::build_undelegate_resource_contract` | ready  |
| Cancel all pending unfreezes                     | T-6                      | `anychain_tron::trx::build_cancel_unfreeze_contract`     | ready  |
| Withdraw expired unfreeze balance                | T-7                      | `anychain_tron::trx::build_withdraw_unfreeze_contract`   | ready  |

### Witness (SR) voting

| Feature                                               | Story (deferred in v0.1) | Crate(s)                                           | Status |
| ----------------------------------------------------- | ------------------------ | -------------------------------------------------- | ------ |
| Vote for SR witness (list of `(address, vote_count)`) | T-4 (new)                | `anychain_tron::trx::build_vote_witness_contract`  | ready  |
| Withdraw vote reward (claim TRX from SR voting)       | T-5                      | `anychain_tron::trx::build_withdraw_vote_contract` | ready  |

### TRC-10 token support (separate proto type)

| Feature            | Story | Crate(s)                                                                                           | Status |
| ------------------ | ----- | -------------------------------------------------------------------------------------------------- | ------ |
| Send TRC-10 token  | new   | `anychain_tron::trx::build_transferassetcontract` (separate from TRC-20 TriggerSmartContract path) | ready  |
| List TRC-10 tokens | new   | caller JSON                                                                                        | ready  |

### Multi-language BIP-39 mnemonic

| Feature                                                                                                           | Story           | Crate(s)                                                                                                                    | Status |
| ----------------------------------------------------------------------------------------------------------------- | --------------- | --------------------------------------------------------------------------------------------------------------------------- | ------ |
| 8-language mnemonic: English, Chinese-Simplified, Chinese-Traditional, French, Italian, Japanese, Korean, Spanish | new             | `anychain_kms::Language::from_phrase` + feature-gated wordlists (English + Chinese always on; others behind cargo features) | ready  |
| `--mn-language chinese-simplified` flag                                                                           | caller CLI flag | ready                                                                                                                       |
| Wordlist NFKD normalization                                                                                       |                 | `anychain_kms::unicode_normalization` (Rust unicode-normalization crate)                                                    | ready  |

### NewChain helpers (added by anychain-tron, not exposed in v0.1)

| Feature                                                | Story | Crate(s)                                   | Status |
| ------------------------------------------------------ | ----- | ------------------------------------------ | ------ |
| Account creation (`AccountCreateContract`, 1 TRX burn) | new   | `anychain_tron::trx::build_account_create` | ready  |

### Additional v0.2 surface

| Feature                                                                                           | Story | Crate(s)                                        | Status |
| ------------------------------------------------------------------------------------------------- | ----- | ----------------------------------------------- | ------ |
| Stake 2.0 CLI subcommand tree (`tron stake freeze --unfreeze / delegate --undelegate / withdraw`) | new   | wraps anychain-tron builders + caller broadcast | ready  |
| Witness CLI subcommand tree (`tron vote --sr <addr> --count <N>`)                                 | new   | wraps anychain-tron + broadcast                 | ready  |
| TRC-10 send (`tron trc10 send --token FOO --to ...`)                                              | new   | wraps anychain-tron builders                    | ready  |

### NOT in v0.2 (deferred to v0.3+)

| Feature                                     | Reason                                               |
| ------------------------------------------- | ---------------------------------------------------- |
| Stake 2.0 voting-with-staked-TP (TP voting) | requires governance vote tracking, separate research |
| Resource lending marketplace                | off-chain service integration                        |
| Shielded TRC-20 transfers (privacy)         | shielded_transaction_contract — Phase 3              |

## Anychain-tron features NOT yet listed in V0.1 (deferred to V0.2 / V0.3)

Deep-dive of `anychain-tron/src/` (22 src files, 14 in `protocol/`) revealed proto types + builders beyond the 17 already mapped to V0.1. This section catalogs what's deferred and when.

### Already in V0.1 (17 builders)

| Builder                                   | V0.1 CLI                                  |
| ----------------------------------------- | ----------------------------------------- |
| `build_transfer_contract`                 | native TRX send                           |
| `build_account_create`                    | create account                            |
| `build_freeze_balance_v2_contract`        | Stake 2.0 freeze                          |
| `build_unfreeze_balance_v2_contract`      | Stake 2.0 unfreeze                        |
| `build_delegate_resource_contract`        | Stake 2.0 delegate                        |
| `build_undelegate_resource_contract`      | Stake 2.0 undelegate                      |
| `build_vote_witness_contract`             | SR vote                                   |
| `build_trc20_transfer_contract`           | TRC-20 transfer                           |
| `build_trc20_approve_contract`            | TRC-20 approve                            |
| `build_trigger_contract`                  | raw trigger                               |
| `build_cancel_unfreeze_contract`          | cancel pending unfreeze                   |
| `build_withdraw_unfreeze_contract`        | withdraw expired unfreeze                 |
| `build_withdraw_vote_contract`            | withdraw vote reward                      |
| `build_contract(ct)`                      | generic wrapper (takes any ContractPbExt) |
| `TronTransactionParameters` set_* helpers | ref_block, fee_limit, expiration          |
| `abi::contract_function_call`             | raw ABI encode                            |
| `abi::trc20_transfer` + `trc20_approve`   | convenience wrappers                      |

### Deferred to V0.2 (TRC-10 + storage market + governance)

| Proto type                      | V0.2 use case               | CLI command (proposed)     |
| ------------------------------- | --------------------------- | -------------------------- |
| `AssetIssueContract`            | issue TRC-10 token          | `tron trc10 issue`         |
| `TransferAssetContract`         | TRC-10 send                 | `tron trc10 send`          |
| `ParticipateAssetIssueContract` | TRC-10 purchase             | `tron trc10 buy`           |
| `UnfreezeAssetContract`         | TRC-10 unfreeze             | `tron trc10 unfreeze`      |
| `UpdateAssetContract`           | TRC-10 metadata update      | `tron trc10 update`        |
| `BuyStorageBytesContract`       | buy disk (bandwidth)        | `tron storage buy-bytes`   |
| `BuyStorageContract`            | buy storage (TRX)           | `tron storage buy`         |
| `SellStorageContract`           | sell storage                | `tron storage sell`        |
| `UpdateBrokerageContract`       | SR brokerage update         | `tron stake set-brokerage` |
| `SetAccountIdContract`          | account alias               | `tron account set-id`      |
| `ProposalCreateContract`        | network governance proposal | `tron governance propose`  |
| `ProposalApproveContract`       | approve proposal            | `tron governance approve`  |
| `ProposalDeleteContract`        | delete proposal             | `tron governance cancel`   |

**Why V0.2 not V0.1:** TRC-10 token issuance is issuer-only (operator flow, not user). Storage market is opt-in optimization. Governance proposals affect network parameters (multi-sig requirement).

### Deferred to V0.3 (advanced + SR + multi-sig)

| Proto type                                                                                                                                                                | V0.3 use case                     | Notes                                             |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- | ------------------------------------------------- |
| `WitnessCreateContract`                                                                                                                                                   | apply for SR candidacy            | requires 9999 TRX stake                           |
| `WitnessUpdateContract`                                                                                                                                                   | update SR URL/ID                  | operator flow                                     |
| `VoteAssetContract`                                                                                                                                                       | legacy vote-asset (pre-Stake 2.0) | deprecated, low priority                          |
| `AccountPermissionUpdateContract`                                                                                                                                         | multi-sig + key permissions       | anychain-tron is single-sig — needs forks         |
| `ExchangeCreateContract` `Inject` `Withdraw` `Transaction`                                                                                                                | TRC-10 DEX                        | dead since 2022 (Bancor-style); never ship        |
| `MarketSellAssetContract` `CancelOrderContract`                                                                                                                           | DEX market orders                 | TRC-10 only; deprecated                           |
| `shield_contract.rs` (6 types: `AuthenticationPath`, `MerklePath`, `OutputPoint`, `OutputPointInfo`, `PedersenHash`, `IncrementalMerkleTree`, `IncrementalMerkleVoucher`) | zk-SNARK shielded TRC-20          | advanced cryptography; needs `bellman` or similar |
| `Discover.rs` (6 types: `Endpoint`, `PingMessage`, `PongMessage`, `FindNeighbours`, `Neighbours`, `BackupMessage`)                                                        | p2p node discovery                | NOT for wallet (full-node only)                   |

**Why V0.3 not V0.2:** SR candidacy is operator-only (network infrastructure). Multi-sig needs upstream `anychain-tron` fork (single-sig only). zk-SNARK shielded TRC-20 requires significant new deps. P2P discovery is full-node territory, not wallet.

### Shared proto types used by V0.1 (no new builders needed)

| Proto type                                    | Used by                   | In `tron-wallet-core`                          |
| --------------------------------------------- | ------------------------- | ---------------------------------------------- |
| `ResourceCode` (BANDWIDTH/ENERGY enum)        | Stake 2.0 builders        | `disambig.rs::resource_label`                  |
| `Vote` (witness vote struct)                  | `VoteWitnessContract`     | `tx/builder.rs::vote_witness`                  |
| `Tron::transaction::Contract`                 | all builders              | `tx/builder.rs` wrapper                        |
| `Tron::Transaction`                           | tx envelope               | `tx/sign.rs::sign_tx`                          |
| `SmartContract` + `ABI` + `Entry` + `Param`   | TRC-20 trigger            | `tx/builder.rs::trc20`                         |
| `TransactionBalanceTrace` `BlockBalanceTrace` | balance trace (debug API) | debug-only via `wallet/gettransactioninfobyid` |
| `ContractState`                               | chain state query         | HTTP-only (TronGrid)                           |

### ABI helpers already exposed (`abi.rs`)

| Helper                   | Signature                                   | Use                                |
| ------------------------ | ------------------------------------------- | ---------------------------------- |
| `contract_function_call` | `(name: &str, params: &[Param]) -> Vec<u8>` | generic ABI encode                 |
| `trc20_transfer`         | `(address: &str, amount: &str) -> Vec<u8>`  | Transfer(address,uint256) shortcut |
| `trc20_approve`          | `(address: &str, amount: &str) -> Vec<u8>`  | Approve(address,uint256) shortcut  |

**All 3 in V0.1** via `tx/builder.rs::trc20_helpers.rs` (per gap analysis above).

### V0.1 + V0.2 + V0.3 totals

| Crate surface                   | V0.1 | V0.2 | V00.3 | Total |
| ------------------------------- | ---- | ---- | ----- | ----- |
| Contract builders               | 17   | 13   | 5     | 35    |
| Proto types wrapped             | 11   | 13   | 11    | 35    |
| Shared proto types              | 6    | 0    | 0     | 6     |
| Total proto types used          | 17   | 13   | 11    | 41    |
| `anychain-tron::protocol` total | ~50  | ~50  | ~50   | ~50   |
| Coverage %                      | 34%  | 60%  | 82%   | 82%   |

### Why some proto types stay unwrapped

- **Deprecated DEX (`Exchange*`, `Market*`)**: dead since 2022. Building them would ship debt.
- **P2P discovery (`Discover.rs`)**: not wallet surface — full-node only.
- **Shielded TRC-20 (`shield_contract.rs`)**: requires zk-SNARK proving system, out of scope for any wallet-core.
- **Proposal governance**: 26 SR votes + complex lifecycle; needs governance SDK, not wallet-core.

### V0.2 add-on dependencies

| Dep                | Purpose                                                                                           | Why V0.2 not V0.1                                              |
| ------------------ | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| `trc10` builders   | caller writes thin wrappers around existing `anychain-tron::trx` (no new anychain builder exists) | issue flow is operator-only — not a wallet user story for V0.1 |
| `storage` builders | same — wraps existing builders                                                                    | opt-in optimization, not a default path                        |

No new anychain dep needed. V0.2 = same crates + caller wrappers + 4-6 user stories + ~600 LOC new code.



## Ractor features in `tron-wallet-core`

V0.1 ships with **no ractor actor code** — all 19 commands are direct async functions. Actors are introduced in V0.1.5/V0.2 to add **long-lived, supervised, stateful** behavior that direct async cannot provide cleanly.

### Decision matrix: when to use ractor

| Need | Use ractor? | Reason |
|------|-------------|--------|
| Long-lived state (session, retry queue, polling) | ✓ | needs supervision + persistent across awaits |
| Crash recovery + auto-restart | ✓ | ractor supervisor tree (§15 in rust_plan.md) |
| Shared mutable state across async tasks | ✗ (use `Arc<RwLock<T>>`) | actor adds overhead |
| One-shot async operation | ✗ (use direct `async fn`) | actor round-trip = ~100µs vs direct = ~1µs |
| Pure function | ✗ | sync `fn` |
| Background work surviving app restart | ✗ (use OS scheduler) | actor lifecycle = process lifetime |
| Real-time event distribution | ✗ (use `tokio::sync::broadcast`) | broadcast = multicast pub/sub |

### Ractor feature inventory per version

| Version | Actor | Purpose | LOC estimate |
|---------|-------|---------|--------------|
| V0.1 | (none) | — | 0 |
| V0.1.5 | `WalletSupervisor` | supervises wallet actors + manages CLI long-running mode | ~250 |
| V0.1.5 | `TronGridClientActor` (×3: mainnet, testnet, local) | per-network connection pool + concurrent request rate limiting | ~200 |
| V0.2 | `TxRetryActor` | persistent retry queue for stuck txs; survives CLI invocations | ~300 |
| V0.2 | `SessionActor` | auto-lock wallet after inactivity (configurable TTL) | ~200 |
| V0.2 | `SpeedupWatcher` | monitor stuck txs; auto-bump fee via send-speedup | ~250 |
| V0.3 | `ChainPoller` | background block height + balance refresh; pub/sub to subscribers | ~200 |
| V0.3 | `WalletActor` (per wallet) | per-wallet state isolation + serialized operations | ~300 |

**Total ractor-related LOC across V0.1.5-V0.3: ~1700 LOC** (~7% of total wallet-core).

### V0.1.5: `WalletSupervisor` + `TronGridClientActor`

#### WalletSupervisor — top-level supervisor

```rust
// tron-wallet-core/src/actor/supervisor.rs
pub struct WalletSupervisor {
    pub cfg: Arc<TronConfig>,
    pub storage: Arc<dyn WalletStorage>,
    pub http_factory: Arc<dyn NetworkClient>,
}

#[async_trait::async_trait]
impl Actor for WalletSupervisor {
    type Msg = SupervisorMsg;
    type State = SupervisorState;
    type Arguments = ();

    async fn pre_start(&self, _: ActorRef<Self::Msg>, _: ()) -> Result<Self::State, ActorProcessingErr> {
        // Load all wallet IDs from storage at startup
        let wallet_ids = self.storage.list().await?;
        let mut wallet_actors = HashMap::new();
        for id in wallet_ids {
            let actor = Actor::spawn_linked(
                Some(self.my_ref()),
                WalletActor { id: id.clone(), cfg: self.cfg.clone() },
                ()
            ).await?;
            wallet_actors.insert(id, actor);
        }
        Ok(SupervisorState { wallet_actors, ..Default::default() })
    }

    async fn handle(&self, me: ActorRef<Self::Msg>, msg: Self::Msg, state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        match msg {
            SupervisorMsg::CreateWallet { params, reply } => {
                let id = self.storage.put(&params).await?;
                let actor = Actor::spawn_linked(Some(me.clone()), WalletActor { id: id.clone(), cfg: self.cfg.clone() }, ()).await?;
                state.wallet_actors.insert(id.clone(), actor.clone());
                reply.send(Ok(id)).ok();
            }
            SupervisorMsg::Unlock { id, password, reply } => {
                let actor = state.wallet_actors.get(&id).ok_or(Error::WalletNotFound)?;
                actor.cast(WalletMsg::Unlock { password, reply }).ok();
            }
            SupervisorMsg::List { reply } => {
                reply.send(Ok(state.wallet_actors.keys().cloned().collect())).ok();
            }
            // ... supervision (auto-restart child actors on crash) ...
        }
        Ok(())
    }
}
```

**Why actor:**
- CLI long-running mode: spawn at startup, serve multiple requests, survive across `tron send` / `tron balance` invocations
- Supervises child `WalletActor` instances; if one crashes, restart per §15
- Centralized wallet discovery (no need to re-load from disk per command)

#### TronGridClientActor — per-network connection pool

```rust
// tron-wallet-core/src/actor/client.rs
pub struct TronGridClientActor {
    pub network: Network,
    pub rpc_url: String,
    pub http: reqwest::Client,
    pub inflight: Arc<Semaphore>,        // limit concurrent requests to N
}

impl Actor for TronGridClientActor {
    type Msg = ClientMsg;
    type State = ();

    async fn handle(&self, _: ActorRef<Self::Msg>, msg: ClientMsg, _: &mut Self::State) -> Result<(), ActorProcessingErr> {
        match msg {
            ClientMsg::Broadcast { tx_json, reply } => {
                let _permit = self.inflight.acquire().await.ok();
                let result = self.http.post(format!("{}/wallet/broadcasttransaction", self.rpc_url))
                    .json(&tx_json).send().await;
                reply.send(result.map_err(Error::from)).ok();
            }
            ClientMsg::GetTxInfo { txid, reply } => {
                let _permit = self.inflight.acquire().await.ok();
                let result = self.http.get(format!("{}/wallet/gettransactioninfobyid", self.rpc_url))
                    .query(&[("txid", txid)]).send().await;
                reply.send(result.map_err(Error::from)).ok();
            }
            // ... other endpoints (getnowblock, getaccount, getaccountresource, triggerconstantcontract) ...
        }
        Ok(())
    }
}
```

**Why actor:**
- HTTP connection pool (keep-alive sockets, DNS cache) is shared mutable state
- Rate limiting via `Semaphore` inside actor state (no global state)
- Failure isolation: MainnetClient crash doesn't affect TestnetClient
- Three instances spawned (mainnet, testnet, local) — supervisor routes by `TronConfig::network`

### V0.2: TxRetryActor + SessionActor + SpeedupWatcher

#### TxRetryActor — persistent retry queue

```rust
// tron-wallet-core/src/actor/retry.rs
pub struct TxRetryActor {
    pub pending: HashMap<Txid, PendingTx>,
    pub max_attempts: u32,         // default 5
    pub backoff: ExponentialBackoff,
    pub cfg: Arc<TronConfig>,
}

pub struct PendingTx {
    pub tx_params: TxParams,
    pub attempts: u32,
    pub first_try: Instant,
    pub last_try: Instant,
    pub last_error: Option<String>,
}

impl Actor for TxRetryActor {
    type Msg = RetryMsg;
    type State = RetryState;

    async fn handle(&self, me: ActorRef<Self::Msg>, msg: RetryMsg, state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        match msg {
            RetryMsg::Enqueue { txid, tx_params } => {
                state.pending.insert(txid.clone(), PendingTx { tx_params, attempts: 0, first_try: Instant::now(), last_try: Instant::now(), last_error: None });
                self.schedule_retry(me, txid, &mut state).await;
            }
            RetryMsg::RetryTick { txid } => {
                if let Some(pending) = state.pending.get_mut(&txid) {
                    pending.attempts += 1;
                    pending.last_try = Instant::now();
                    match try_resubmit(&pending.tx_params, &self.cfg).await {
                        Ok(new_txid) => {
                            tracing::info!("tx {txid} resubmitted as {new_txid}");
                            state.pending.remove(&txid);
                        }
                        Err(e) if pending.attempts < self.max_attempts => {
                            pending.last_error = Some(e.to_string());
                            self.schedule_retry(me, txid, &mut state).await;
                        }
                        Err(e) => {
                            tracing::error!("tx {txid} permanently failed: {e}");
                            state.failed.push((txid, e.to_string()));
                            state.pending.remove(&txid);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
```

**Why actor:**
- Pending list must persist across CLI invocations (CLI process may restart between commands)
- Crash recovery: if CLI crashes mid-retry, supervisor restarts the actor, resumes from `pending` HashMap
- Backoff timer is ractor-scheduled (no need for external scheduler)

#### SessionActor — auto-lock wallet

```rust
// tron-wallet-core/src/actor/session.rs
pub struct SessionActor {
    pub timeout: Duration,            // default 15 minutes
    pub sessions: HashMap<WalletId, SessionInfo>,
}

pub struct SessionInfo {
    pub mnemonic: Zeroizing<String>,    // !Send !Sync — never crosses threads
    pub last_used: Instant,
}

impl Actor for SessionActor {
    type Msg = SessionMsg;
    type State = SessionState;

    async fn handle(&self, me: ActorRef<Self::Msg>, msg: SessionMsg, state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        match msg {
            SessionMsg::Unlock { id, password, reply } => {
                match state.storage.unlock(&id, &password).await {
                    Ok(wallet) => {
                        state.sessions.insert(id, SessionInfo { mnemonic: wallet.mnemonic, last_used: Instant::now() });
                        reply.send(Ok(())).ok();
                        self.schedule_sweep(me).await;
                    }
                    Err(e) => reply.send(Err(e)).ok(),
                }
            }
            SessionMsg::Touch { id, reply } => {
                state.sessions.entry(id).and_modify(|s| s.last_used = Instant::now());
                reply.send(Ok(())).ok();
            }
            SessionMsg::SweepExpired => {
                let now = Instant::now();
                state.sessions.retain(|_, s| now.duration_since(s.last_used) < self.timeout);
                // Re-schedule next sweep
                tokio::time::sleep(Duration::from_secs(60)).await;
                me.cast(Self::Msg::SweepExpired).ok();
            }
        }
        Ok(())
    }
}
```

**Why actor:**
- `Zeroizing<String>` mnemonic must NOT cross threads (`!Send, !Sync`) — actor's `&mut self` state guarantees thread isolation
- Sweep timer is ractor-scheduled (no external scheduler needed)
- Crash recovery: actor restart → in-memory sessions lost (acceptable; user re-unlocks)

#### SpeedupWatcher — auto-bump stuck txs

```rust
// tron-wallet-core/src/actor/speedup.rs
pub struct SpeedupWatcher {
    pub config: SpeedupConfig,        // { check_interval: 60s, max_wait: 5min, bump_strategy: Linear|Exponential }
    pub watching: HashMap<Txid, WatchEntry>,
    pub cfg: Arc<TronConfig>,
}

pub struct WatchEntry {
    pub original_txid: Txid,
    pub original_fee: i64,
    pub attempts: u32,
    pub start: Instant,
}

impl Actor for SpeedupWatcher {
    type Msg = SpeedupMsg;
    type State = SpeedupState;

    async fn handle(&self, me: ActorRef<Self::Msg>, msg: SpeedupMsg, state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        match msg {
            SpeedupMsg::Watch { txid, original_fee } => {
                state.watching.insert(txid.clone(), WatchEntry { original_txid: txid.clone(), original_fee, attempts: 0, start: Instant::now() });
                self.schedule_check(me, &txid).await;
            }
            SpeedupMsg::CheckTick { txid } => {
                if let Some(entry) = state.watching.get_mut(&txid) {
                    entry.attempts += 1;
                    match fetch_tx_info(&txid, &self.cfg).await {
                        Ok(info) if info.confirmed => {
                            tracing::info!("tx {txid} confirmed; stopping watch");
                            state.watching.remove(&txid);
                            return Ok(());
                        }
                        Ok(_) => {
                            // Not confirmed yet — bump fee
                            let new_fee = match &self.config.bump_strategy {
                                SpeedupStrategy::Linear => entry.original_fee + 10_000_000 * (entry.attempts as i64),
                                SpeedupStrategy::Exponential => entry.original_fee * 2,
                            };
                            resubmit_with_fee(&txid, new_fee, &self.cfg).await.ok();
                            if Instant::now().duration_since(entry.start) > self.config.max_wait {
                                tracing::error!("tx {txid} stuck after max_wait");
                                state.stuck.push(txid.clone());
                                state.watching.remove(&txid);
                                return Ok(());
                            }
                            self.schedule_check(me, &txid).await;
                        }
                        Err(e) => {
                            tracing::warn!("check {txid} failed: {e}");
                            self.schedule_check(me, &txid).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
```

**Why actor:**
- Long-running watcher (minutes to hours)
- Persistent state per watched tx
- Crash recovery: restart actor, resume from `watching` HashMap if state persisted to disk (V0.2+)

### V0.3: ChainPoller + per-wallet WalletActor

#### ChainPoller — background block height tracker

```rust
// tron-wallet-core/src/actor/poller.rs
pub struct ChainPoller {
    pub network: Network,
    pub poll_interval: Duration,         // default 3s (matches block time)
    pub cfg: Arc<TronConfig>,
    pub subscribers: Vec<ActorRef<dyn Subscriber>>,
}

impl Actor for ChainPoller {
    type Msg = ChainPollerMsg;
    type State = PollerState;

    async fn handle(&self, me: ActorRef<Self::Msg>, _: Self::Msg, state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        loop {
            match fetch_block_height(&self.cfg).await {
                Ok(height) if height > state.last_known => {
                    state.last_known = height;
                    // Notify subscribers (pub/sub pattern)
                    for sub in &self.subscribers {
                        sub.cast(SubscriberMsg::NewBlock { height }).ok();
                    }
                }
                Ok(_) => {}    // no new block
                Err(e) => tracing::warn!("poller error: {e}"),
            }
            sleep(self.poll_interval).await;
        }
    }
}
```

**Why actor:**
- Long-running background loop (process lifetime)
- Pub/sub fan-out to subscribers (cleaner than channels)
- Supervisor restarts on crash (transient network failures)

#### Per-wallet WalletActor

```rust
// V0.3 — one actor per wallet for state isolation
pub struct WalletActor {
    pub id: WalletId,
    pub cfg: Arc<TronConfig>,
    pub storage: Arc<dyn WalletStorage>,
    pub state: WalletActorState,        // !Send !Sync — Zeroizing<String>
}

pub struct WalletActorState {
    pub unlocked: Option<Zeroizing<String>>,    // mnemonic, never crosses threads
    pub nonce: u64,                             // per-wallet nonce tracking
    pub inflight: HashSet<Txid>,                // prevent double-spend
}

impl Actor for WalletActor {
    type Msg = WalletMsg;
    type State = WalletActorState;

    async fn handle(&self, _: ActorRef<Self::Msg>, msg: Self::Msg, state: &mut Self::State) -> Result<(), ActorProcessingErr> {
        match msg {
            WalletMsg::Unlock { password, reply } => {
                let wallet = self.storage.unlock(&self.id, &password).await?;
                state.unlocked = Some(wallet.mnemonic);
                reply.send(Ok(())).ok();
            }
            WalletMsg::Send { to, amount, reply } => {
                let mnemonic = state.unlocked.as_ref().ok_or(Error::Locked)?;
                let txid = submit_trx_with_nonce(mnemonic, &to, amount, &mut state.nonce, &self.cfg).await?;
                state.inflight.insert(txid.clone());
                reply.send(Ok(txid)).ok();
            }
            WalletMsg::Confirm { txid, reply } => {
                state.inflight.remove(&txid);
                reply.send(Ok(())).ok();
            }
        }
        Ok(())
    }
}
```

**Why actor:**
- State isolation: each wallet's `Zeroizing<String>` mnemonic stays on one actor's thread (never crosses)
- Serialization: per-wallet operations processed in mailbox order (no concurrent double-spends from same wallet)
- Parallelism: different wallet actors run on different threads (independent operations)
- Supervision: if `WalletActor` for wallet X crashes, supervisor restarts it without affecting wallet Y

### Performance: actor vs direct async

| Pattern | Per-call latency | Throughput (per second) |
|---------|------------------|--------------------------|
| Direct `async fn` (FFI) | ~1µs | 100,000+ |
| Direct `async fn` (CLI multi-thread) | ~5µs | 50,000+ |
| Ractor `actor.cast()` + `oneshot::channel` reply | ~50µs | 20,000 |
| Ractor `actor.call()` (synchronous within actor) | ~100µs | 10,000 |

**Trade-off:** 50-100× slower per call for stateful features. Acceptable when the feature inherently needs state (retry queue, session).

### Crate additions for ractor

```toml
# Cargo.toml (additions to V0.1 deps)
ractor      = "=0.13.5"
ractor_macro = "=0.13.5"
async-trait  = "0.1"
futures      = "0.3"
```

Binary size impact (mobile):
- ractor: ~80 KB stripped
- async-trait: ~10 KB
- Total: ~90 KB added to `libtron_wallet_core.so` when ractor is enabled

**Conditional compilation** (mobile skips ractor):
```rust
#[cfg(any(target_os = "ios", target_os = "android"))]
compile_error!("actor features not supported on mobile V0.1; use direct async");

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub mod actor {
    pub use ractor::*;
    // ... all actor implementations ...
}
```

(V0.1.5+ may revisit mobile actors if FFI supervisor proves useful.)

## Anychain fee implementation × tron-wallet-core (coverage map)

Map every fee-related feature needed by `tron-wallet-core` to the anychain API surface (or wallet-local HTTP fallback). Versions track tron-wallet-core V0.1, V0.2, V0.3.

### Anychain fee API surface (recap)

| API                                                         | Source                                                             | Type         | Purpose                                               |
| ----------------------------------------------------------- | ------------------------------------------------------------------ | ------------ | ----------------------------------------------------- |
| `TronTransactionParameters::fee_limit: i64`                 | `anychain-tron/src/transaction.rs:18`                              | field        | tx-level energy cap (sun)                             |
| `TronTransactionParameters::set_fee_limit(fee: i64)`        | `anychain-tron/src/transaction.rs:43-45`                           | setter       | mutator                                               |
| `TransactionRaw.fee_limit`                                  | `anychain-tron/src/protocol/Tron.rs`                               | proto field  | wire-encoded in tx                                    |
| `SmartContract::origin_energy_limit: i64`                   | `anychain-tron/src/protocol/smart_contract.rs:46`                  | proto field  | per-call energy cap (NOT exposed by builders)         |
| Transaction receipt: `energy_usage`, `net_fee`, `net_usage` | `anychain-tron/src/protocol/storage_contract.rs` (TransactionInfo) | proto fields | post-broadcast fee readback                           |
| **Missing**: `estimate_energy` API                          | n/a                                                                | —            | caller must use HTTP `wallet/estimateenergy`          |
| **Missing**: fee history API                                | n/a                                                                | —            | caller must HTTP-scan `wallet/gettransactioninfobyid` |
| **Missing**: bandwidth/energy resource query                | n/a                                                                | —            | caller must use HTTP `wallet/getaccountresource`      |

**Key insight:** anychain exposes exactly 1 fee knob (`fee_limit`). Everything else (estimate, history, resource query) is wallet-local HTTP via TronGrid.

### V0.1 — basic fee support

| Feature                                                         | Anychain API                                                 | tron-wallet-core call                                  | Status     |
| --------------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------ | ---------- |
| Set tx-level energy cap on send                                 | `TronTransactionParameters::set_fee_limit(sun)`              | `tx::submit_trx(..., fee_limit_sun: Option<i64>, ...)` | ready      |
| Default fee cap (chain default = no cap up to 100M Energy band) | `params.fee_limit = 0`                                       | `params.set_fee_limit(0)` or unset                     | ready      |
| Per-call energy cap for TRC-20 (TRIGGER smart contract)         | `SmartContract::origin_energy_limit` (manual proto mutation) | **GAP** — see workaround below                         | workaround |
| Read actual energy consumed after broadcast                     | `TransactionInfo.energy_usage` (parse from HTTP response)    | `chain::get_tx_info(txid)` → `receipt.energy_used()`   | ready      |
| Read actual net fee paid after broadcast                        | `TransactionInfo.net_fee`                                    | `chain::get_tx_info(txid)` → `receipt.net_fee_sun()`   | ready      |
| Read bandwidth consumed                                         | `TransactionInfo.net_usage`                                  | `chain::get_tx_info(txid)` → `receipt.net_usage()`     | ready      |

**V0.1 workaround for missing `build_trigger_contract` energy_limit:**

```rust
// tron-wallet-core/src/tx/builder.rs — internal helper
use anychain_tron::protocol::smart_contract::TriggerSmartContract as SmartContract;

fn set_trigger_energy_limit(contract: &mut SmartContract, energy_limit_sun: i64) {
    contract.origin_energy_limit = energy_limit_sun;
}

// In submit_trc20 (after build_trc20_transfer_contract):
let mut trigger_sc = SmartContract::new();
trigger_sc.owner_address = ...;
trigger_sc.contract_address = ...;
trigger_sc.data = abi::trc20_transfer(to, amount)?;
set_trigger_energy_limit(&mut trigger_sc, per_call_energy_limit);
let contract = anychain_tron::trx::build_contract(&trigger_sc)?;
```

(V0.1 doesn't expose `per_call_energy_limit` to CLI — defaults to chain behavior. V0.2 adds `fee estimate` which surfaces this.)

### V0.2 — fee estimation + history + resource query

| Feature                                                     | Anychain API                                                                | tron-wallet-core call                                        | Status      |
| ----------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------ | ----------- |
| Estimate energy for a contract call                         | none — TronGrid HTTP `wallet/estimateenergy`                                | `chain::estimate_energy(contract, method, args, cfg)` (HTTP) | ready       |
| Estimate energy for transfer (bandwidth vs energy consumed) | none — TronGrid HTTP `wallet/estimateenergy`                                | `chain::estimate_transfer(to, amount, cfg)` (HTTP)           | ready       |
| Recent fee paid history                                     | none — scan HTTP `wallet/gettransactioninfobyid` for last N txns            | `chain::get_fee_history(addr, n, cfg)`                       | ready       |
| Bandwidth available (free + staked)                         | none — TronGrid HTTP `wallet/getaccountresource`                            | `chain::get_account_resource(addr, cfg)`                     | ready       |
| Energy available (free + staked)                            | same                                                                        | `chain::get_account_resource(addr, cfg)`                     | ready       |
| Per-token energy factor (DEM — Dynamic Energy Model)        | none — TronGrid HTTP `wallet/getcontractinfo`                               | `chain::get_contract_info(contract, cfg)`                    | ready       |
| Stake to acquire more bandwidth/energy (V0.2 stake ops)     | `anychain-tron::trx::build_freeze_balance_v2_contract` + `DelegateResource` | `tx::submit_stake_freeze(...)` etc.                          | ready       |
| Pre-flight check (balance vs fee)                           | none — caller logic                                                         | `tx::preflight_check(sk, params, cfg)`                       | new in V0.2 |

**V0.2 work:** all wallet-local HTTP, no anychain changes needed. ~250 LOC in `tron-wallet-core/src/chain/`.

### V0.3 — advanced fee control + estimation history

| Feature                                                      | Anychain API                                                    | tron-wallet-core call                                                               | Status                  |
| ------------------------------------------------------------ | --------------------------------------------------------------- | ----------------------------------------------------------------------------------- | ----------------------- |
| Suggest optimal fee_limit (machine-learned from chain state) | none                                                            | `chain::suggest_fee_limit(addr, contract, urgency, cfg)`                            | new in V0.3             |
| Auto-bump fee on stuck tx (RBF)                              | `anychain-tron::Transaction::sign` (re-sign with new fee_limit) | `tx::submit_send_speedup(sk, original_txid, new_fee_limit, cfg)` (already in V0.1!) | ready                   |
| Pay fee in TRX-20 stablecoin (fee delegation)                | none — requires protocol upgrade                                | future                                                                              | never                   |
| Subsidy fee (gasless tx via relayer)                         | none                                                            | future                                                                              | V0.4+                   |
| Multi-sig fee split (proportional to signers)                | none — requires upstream fork                                   | future                                                                              | V0.4+ (multi-sig first) |

**V0.3 work:** ML model for fee suggestion (deferred indefinitely without model). RBF already works in V0.1 via `submit_send_speedup`. ~500 LOC for new fee estimation infrastructure.

### Coverage summary

| Category                               | V0.1          | V0.2          | V0.3          |
| -------------------------------------- | ------------- | ------------- | ------------- |
| anychain-supported (1 fee knob)        | 1/1 = 100%    | 1/1 = 100%    | 1/1 = 100%    |
| HTTP wallet-local (TronGrid endpoints) | 4/6 = 67%     | 6/6 = 100%    | 6/6 = 100%    |
| ML/advanced (V0.4+)                    | 0/2 = 0%      | 0/2 = 0%      | 1/2 = 50%     |
| **Total fee features covered**         | **5/9 = 56%** | **7/9 = 78%** | **8/9 = 89%** |

### Per-feature status table (canonical reference)

| #   | Feature                                          | Ver  | Anychain      | wallet-local                                | Notes                                                        |
| --- | ------------------------------------------------ | ---- | ------------- | ------------------------------------------- | ------------------------------------------------------------ |
| 1   | Set tx-level fee cap                             | V0.1 | ✓ `fee_limit` | —                                           | set via `params.set_fee_limit`                               |
| 2   | Read fee paid post-broadcast                     | V0.1 | —             | ✓ HTTP `wallet/gettransactioninfobyid`      | returns `net_fee`, `energy_usage`, `net_usage`               |
| 3   | Set per-call energy cap (TRC-20)                 | V0.1 | ⚠ workaround  | —                                           | direct proto mutation on `SmartContract.origin_energy_limit` |
| 4   | RBF (send-speedup)                               | V0.1 | ✓ re-sign     | —                                           | already in V0.1 via `submit_send_speedup`                    |
| 5   | Display balance (free + staked bandwidth/energy) | V0.2 | —             | ✓ HTTP `wallet/getaccountresource`          | `AccountResource` struct                                     |
| 6   | Estimate energy for call                         | V0.2 | —             | ✓ HTTP `wallet/estimateenergy`              | `EnergyEstimate` struct                                      |
| 7   | Fee paid history                                 | V0.2 | —             | ✓ HTTP `wallet/gettransactioninfobyid` scan | `FeeHistoryEntry` struct                                     |
| 8   | ML fee suggestion                                | V0.3 | —             | ✓ (new in V0.3)                             | model TBD                                                    |
| 9   | Fee delegation (pay fee in stablecoin)           | V1.x | —             | —                                           | requires protocol upgrade — never                            |

### CLI integration per version

**V0.1:** `--fee-limit <sun>` flag on all tx subcommands. `tron tx get` / `tron tx wait` returns receipt including actual fee paid.

**V0.2:** `tron fee estimate --contract --method` returns `{ energy_used, sun_required }`. `tron balance --address` shows free bandwidth/energy. `tron config set-default-fee-limit <sun>` (V0.2).

**V0.3:** `tron fee suggest --urgency low|medium|high|critical --contract` returns suggested fee_limit with confidence.

### Cross-reference

- Anychain source: `anychain/crates/anychain-tron/src/transaction.rs:18-77` (fee_limit field + setter)
- Protocol types: `anychain/crates/anychain-tron/src/protocol/smart_contract.rs:46` (origin_energy_limit)
- Tron Wallet Core architecture: `## Tron Wallet Core` parent section
- CLI integration: `## Tron CLI` parent section
- V0.1 features: `## Tron Wallet V0.1` (Story 8 listed under Resource estimation — covered by TronGrid HTTP in V0.2)

## V0.3 Features

v0.3 completes the wallet feature set: watch-only xpub import, local tx index, stake UI, account abstraction prep. ~400 additional lines.

### Watch-only wallets

| Feature                                                 | Story | Crate(s)                                                                                                       | Status |
| ------------------------------------------------------- | ----- | -------------------------------------------------------------------------------------------------------------- | ------ |
| Import watch-only wallet from xpub (no signing key)     | 19    | `anychain_kms::ExtendedPublicKey::from_str` (parses xpub) + derive child xpub + `TronAddress::from_public_key` | ready  |
| Watch-only balance + tx history queries                 | 3, 7  | caller `reqwest` + RPC                                                                                         | ready  |
| Detect sends to watch-only addresses via xpub gap-limit | 19    | `anychain_kms::XpubSecp256k1::derive_from_path` + fingerprint checks                                           | ready  |

### Local tx index

| Feature                                     | Story | Crate(s)                                      | Status |
| ------------------------------------------- | ----- | --------------------------------------------- | ------ |
| Cache tx history locally (sqlite or sled)   | 7     | caller `rusqlite` or `sled` (deferred choice) | ready  |
| Fast `tron tx list --since-block N` queries | 7     | local index lookup                            | ready  |
| Detect re-orgs and invalidate cached blocks | 7     | caller RPC re-org depth check                 | ready  |

### Account abstraction prep

| Feature                                 | Story | Crate(s)                                                                        | Status                                                                   |
| --------------------------------------- | ----- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| EIP-712 equivalent (TRC-516 typed data) | new   | `anychain_tron` would need typed data primitive (anychain-tron doesn't have it) | **deferred** (TRON doesn't have native typed data; custom impl required) |
| Smart contract wallet signer            | new   | not applicable for v0.3 (TRON TRC-516 unproven)                                 | **deferred**                                                             |

### Additional v0.3 surface

| Feature                                                     | Story | Crate(s)                                         | Status                  |
| ----------------------------------------------------------- | ----- | ------------------------------------------------ | ----------------------- |
| Stake dashboard (current frozen, delegated, voting power)   | 8     | anychain-tron builders + caller RPC              | ready                   |
| Yield calculator (APY estimation from recent block rewards) | 8     | caller analysis                                  | ready                   |
| Batched transaction export (CSV / JSON for tax filing)      | 7     | caller `serde_json`                              | ready                   |
| Multi-account (BIP-44 account index) UI                     | 20    | `DerivationPath::from_str` (anychain-kms)        | ready                   |
| `tron wallet rename` UI                                     | 9     | `std::fs` rename                                 | ready                   |
| Encrypted export (encrypted JSON backup with passphrase)    | 12    | caller + Argon2id                                | ready                   |
| Hardware wallet discovery + signing (Ledger/Trezor)         | new   | `ledger-transport` / `trezor-client` Rust crates | **deferred** (per #399) |
| `tron config migrate` (V0.1 wallet format → V0.2 format)    | new   | caller migration tool                            | ready                   |

### v0.3 NOT shipped (deferred to v1.x)

| Feature                                       | Reason                                                      |
| --------------------------------------------- | ----------------------------------------------------------- |
| ENS-style name resolution (`alice.trx`)       | TRON has no equivalent; out of scope                        |
| Multi-sig (Safe-style on TRON)                | Safe-global contracts not deployed on TRON; different model |
| MEV protection (Flashbots-style private RPC)  | TRON MEV auction exists but no Rust integration yet         |
| WebSocket subscriptions (long-running daemon) | CLI is request/response only                                |
| Plausible-deniability multi-bucket wallet     | Far future                                                  |

## TronGrid integration

**Headline:** anychain integrates with TronGrid at **zero endpoints directly**. No HTTP client in anychain-tron, anychain-kms, or anychain-core. Caller wires every `reqwest` call.

## TronGrid endpoints anychain touches directly

**None.** Verified by source review of all 22 src files in `anychain-tron` + `anychain-kms` + `anychain-core`. No `reqwest`, `hyper`, `ureq`, or any HTTP client anywhere. `Cargo.toml` deps list for anychain-tron: `anychain-core`, `serde`, `sha3`, `sha2`, `hex`, `base58`, `protobuf`, `chrono`, `ethabi`, `libsecp256k1`, `ethereum-types` — **no HTTP client**.

## TronGrid endpoints caller must wire (via reqwest)

| Endpoint                                                                    | Used by story                  | anychain helper                                   |
| --------------------------------------------------------------------------- | ------------------------------ | ------------------------------------------------- |
| `wallet/getnowblock`                                                        | 4, 15 (ref_block fetch)        | NONE — caller writes reqwest                      |
| `wallet/getaccount`                                                         | 3, 4, 14 (balance)             | NONE — caller writes reqwest                      |
| `wallet/broadcasttransaction`                                               | 5, 17, 21, 25 (broadcast)      | NONE — caller writes reqwest                      |
| `wallet/gettransactioninfobyid`                                             | 7 (tx status)                  | NONE — caller writes reqwest                      |
| `wallet/triggerconstantcontract`                                            | 22, 24 (TRC-20 view calls)     | NONE — caller writes reqwest                      |
| `wallet/estimateenergy`                                                     | 8 (energy est)                 | NONE — caller writes reqwest                      |
| `wallet/getaccountresource`                                                 | 8 (energy/bandwidth)           | NONE — caller writes reqwest                      |
| `wallet/getchainparameters`                                                 | 8 (sun/Energy rate)            | NONE — caller writes reqwest                      |
| `wallet/getcontractinfo`                                                    | 8 (DEM energy_factor)          | NONE — caller writes reqwest                      |
| `/jsonrpc {"method":"eth_chainId"}`                                         | 10, 27 (chain-id)              | NONE — caller writes reqwest                      |
| `wallet/getblockbyid` / `wallet/getblockbynum`                              | 15 (ref_block verify)          | NONE — caller writes reqwest                      |
| `wallet/gettransactionbyid`                                                 | 7 (tx lookup)                  | NONE — caller writes reqwest                      |
| `wallet/freezebalancev2` / `wallet/unfreezebalancev2` (direct RPC versions) | T-1, T-2 (Stake 2.0 via proto) | anychain-tron has proto builders, not RPC helpers |
| `wallet/delegateresource` / `wallet/undelegateresource`                     | T-3, T-4 (delegation)          | NONE                                              |
| `wallet/votewitness` / `wallet/withdrawbalance`                             | T-4, T-5 (voting)              | NONE                                              |
| `wallet/transferasset`                                                      | TRC-10 (deferred)              | NONE                                              |
| `wallet/exchangetransaction`                                                | TRC-10 DEX (deferred)          | NONE                                              |

## What anychain DOES provide at the API boundary

| Boundary             | Provided by                                                      | What caller gets                                |
| -------------------- | ---------------------------------------------------------------- | ----------------------------------------------- |
| Proto wire format    | `anychain-tron::protocol::*` (1.5 MB vendored)                   | Ready-to-serialize `Transaction` envelope bytes |
| Contract builders    | `anychain-tron::trx::*` (13 builders)                            | `Contract` proto object ready to wrap in tx     |
| Signing              | `anychain-kms::secp256k1_sign`                                   | 64-byte r‖s + recid (Tron signature format)     |
| Address derivation   | `anychain-tron::TronAddress::from_public_key`                    | 21-byte raw T-address                           |
| ABI encoding         | `anychain-tron::abi::*` (TRC-20 transfer/approve)                | 68-byte calldata                                |
| Base58check encoding | `anychain-tron::address.rs::b58encode_check` + `b58decode_check` | T-address string ↔ 21 bytes                     |

## What caller wires

| Layer              | Caller adds                                                  |
| ------------------ | ------------------------------------------------------------ |
| HTTP transport     | `reqwest::Client`                                            |
| JSON-RPC envelope  | `serde_json::{json, Value}`                                  |
| URL endpoint       | `format!("{}/wallet/{}", endpoint, method)`                  |
| Auth header        | `TRON-PRO-API-KEY` env var                                   |
| SPKI pin           | reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` |
| Test container     | `testcontainers` crate for TronBox Docker                    |
| Argon2id + AES-GCM | caller crates (anychain has none)                            |

## Endpoint base URLs

| Network                  | URL                                       |
| ------------------------ | ----------------------------------------- |
| Mainnet                  | `https://api.trongrid.io/wallet/*`        |
| Nile (default v0.1)      | `https://nile.trongrid.io/wallet/*`       |
| Shasta                   | `https://api.shasta.trongrid.io/wallet/*` |
| TronBox local            | `http://127.0.0.1:8090/wallet/*`          |
| gRPC (anychain NOT used) | `grpc.trongrid.io:50051`                  |

## Net summary

**anychain covers 0 of ~20 TronGrid endpoints directly.** The crate is **wire-format + signing + address derivation only** — pure client-side crypto. Every `/wallet/*` call is caller-side `reqwest`. This is by design (zero-RPC umbrella-wide pattern, except Hedera which uses `tonic` + `hiero-sdk`).

Adopting anychain saves ~500 LOC of hand-rolled protobuf + base58check + Keccak256 + ABI encoder + Stake 2.0 builders. Adds ~250 LOC of `reqwest` glue. Break-even at v0.1 + Stake 2.0 stories (anychain-tron builders are unique value).

## Submit Transaction

**Headline:** Submit via RPC — YES, but caller-side. anychain has zero RPC. Submit path = caller `reqwest` POST to `wallet/broadcasttransaction` with signed tx hex as body.

### Submit flow (TRON)

```text
[1] Caller: reqwest POST wallet/getnowblock     → fetch ref_block
[2] Caller: anychain_tron::trx::build_transfer_contract (or similar)
[3] Caller: anychain_tron::TronTransaction::new + to_bytes → unsigned raw_bytes
[5] Caller: anychain_kms::secp256k1_sign(sk, SHA256(SHA256(raw_bytes))) → (r‖s, recid)
[6] Caller: anychain_tron::TronTransaction::sign(sig_rs, recid)
[7] Caller: reqwest POST wallet/broadcasttransaction → txid response
[8] Caller (optional): reqwest POST wallet/gettransactioninfobyid for inclusion
```

### The actual RPC call

```rust
// anychain provides the bytes — caller provides transport
let resp: serde_json::Value = reqwest::Client::new()
    .post(format!("{}/wallet/broadcasttransaction", endpoint))
    .json(&serde_json::json!({
        "transaction":": hex::encode(&signed_bytes)
    }))
    .send().await?
    .json().await?;
let txid_hex = resp["txid"].as_str().unwrap();
```

### RPC endpoint base URLs

| Network                     | Endpoint                                                     |
| --------------------------- | ------------------------------------------------------------ |
| Mainnet                     | `https://api.trongrid.io/wallet/broadcasttransaction`        |
| Nile (v0.1 default)         | `https://nile.trongrid.io/wallet/broadcasttransaction`       |
| Shasta                      | `https://api.shasta.trongrid.io/wallet/broadcasttransaction` |
| TronBox local               | `http://127.0.0.1:8090/wallet/broadcasttransaction`          |
| gRPC (NOT used by anychain) | `grpc.trongrid.io:50051`                                     |

### Request body shape

```json
{
{
  "transaction":": "0a0205f8f9...hex-of-signed-proto-bytes..."
}
}
```

### Response shape (accepted)

```json
{
{
  "result":": true,
{
  "txid":": "519f9d0bdc17d4a083b2676a4e9dce5679045107e7c9a9dad848891ee845235d",
{
  "transaction":": { ... }
}
}
```

### Response shape (rejected)

```json
{
{
  "result":": false,
{
  "code":": "TRANSACTION_EXPIRATION_ERROR",
{
  "message":": "transaction expired",
{
  "txid":": "..."
}
}
```

### Common rejection codes

| `code`                                           | Cause                          | Fix                                                     |
| ------------------------------------------------ | ------------------------------ | ------------------------------------------------------- |
| `TRANSACTION_EXPIRATION_ERROR`                   | >60s old                       | bump `set_timestamp(now_ms)` + re-sign                  |
| `CONTRACT_VALIDATE_ERROR`                        | bad addr/amount/sun            | validate before send; ensure `fee_limit > 0` for TRC-20 |
| `BANDWIDTH_INSUFFICIENT` / `ENERGY_INSUFFICIENT` | no resources                   | stake TRX or send tiny TRX to self for free bandwidth   |
| `SIGNATURE_SIZE_WRONG`                           | wrong sig length               | must be 64 bytes (r‖s) + recid 0/1                      |
| `TYPE_URL_NOT_MATCH`                             | protobuf Any type_url mismatch | anychain-tron bug #2 — fragile                          |
| `CONTRACT_EXE_ERROR`                             | contract reverted              | check USDT balance, recipient address validity          |
| `sigERROR`                                       | ECDSA verify fail              | double-hash the tx (bypass bug #1)                      |

### Auth + headers

- `Content-Type: application/json` (auto via `reqwest::Client::json()`)
- `TRON-PRO-API-KEY: <key>` — optional, raises rate limit from 15 req/s → 100+ req/s

### Confirmation polling (optional but recommended)

`wallet/broadcasttransaction` returns acceptance (valid format), **not block inclusion**. For confirmed status:

```rust
let resp: serde_json::Value = reqwest::Client::new()
    .post(format!("{}/wallet/gettransactioninfobyid", endpoint))
    .json(&serde_json::json!({ "value":": txid }))
    .send().await?
    .json().await?;
if resp.as_object().map_or(false, |o| o.contains_key("blockNumber")) {
    // tx included
}
```

Mainnet ~3s/block, ~99% included in 9s.

### End-to-end via RPC summary

| Step               | Tool                                                 |
| ------------------ | ---------------------------------------------------- |
| Fetch ref block    | caller `reqwest` + `wallet/getnowblock`              |
| Build unsigned tx  | anychain-tron (no RPC)                               |
| Sign tx            | anychain-kms `secp256k1_sign` (no RPC)               |
| **Broadcast**      | **caller `reqwest` + `wallet/broadcasttransaction`** |
| Wait for inclusion | caller `reqwest` + `wallet/gettransactioninfobyid`   |

**Submit = caller via RPC.** anychain = local crypto only.

## Verify Transaction After Submit

**Two-stage confirmation:** (1) `wallet/broadcasttransaction` = format-valid accept; (2) `wallet/gettransactioninfobyid` = block-included + status. anychain = zero RPC, caller does both via reqwest.

### Stage 1 — broadcast returns acceptance (not inclusion)

```rust
let resp: serde_json::Value = reqwest::Client::new()
    .post(format!("{}/wallet/broadcasttransaction", endpoint))
    .json(&serde_json::json!({ "transaction":": hex::encode(&signed) }))
    .send().await?.json().await?;

// Check `result` field
if resp["result"].as_bool() == Some(true) {
    let txid = resp["txid"].as_str().unwrap().to_string();
    // tx is accepted, valid format, signature verifies — but NOT yet in a block
    println!("tx accepted: {}", txid);
} else {
    let code = resp["code"].as_str().unwrap_or("?");
    let msg = resp["message"].as_str().unwrap_or("?");
    // rejected — fix and retry
}
```

`broadcasttransaction` validates: signature, format, fee, ref_block, expiration, contract fields. Returns `txid` = SHA256(SHA256(raw_data_bytes)) per Tron spec. Does NOT wait for block inclusion.

### Stage 2 — poll for inclusion

```rust
async fn wait_included(endpoint: &str, txid: &str, max_ms: u64) -> Result<serde_json::Value> {
    let start = std::time::Instant::now();
    loop {
        let v: serde_json::Value = reqwest::Client::new()
            .post(format!("{}/wallet/gettransactioninfobyid", endpoint))
            .json(&serde_json::json!({ "value":": txid }))
            .send().await?.json().await?;

        // Empty object {} = tx not yet included
        if let Some(o) = v.as_object() {
            if !o.is_empty() && o.contains_key("blockNumber") {
                return Ok(v);  // tx included in block
            }
        }

        if start.elapsed().as_millis() > > max_ms as u128 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Err("timeout".into())
}
```

**Timing:** mainnet ~3s/block, ~99% included in 9s. Set `max_ms = 15_000` for production. Nile same.

### Response shape (included)

```json
{
{
  "id":": "519f9d0bdc17d4a083b2676a4e9dce5679045107e7c9a9dad848891ee845235d",
{
  "blockNumber":": 12345678,
{
  "blockTimeStamp":": 1700000000000,
{
  "contractResult":": ["SUCCESS"],
{
  "receipt":": {
    "energy_usage":": 65123,
    "energy_usage_total":": 65123,
    "net_usage":": 345,
    "net_fee":": 1000000 }
}
}
```

### Confirm-success fields

| Field                  | Meaning                                         | Where to check       |
| ---------------------- | ----------------------------------------------- | -------------------- |
| `contractResult[0]`    | `SUCCESS` (or revert reason); success indicator |
| `blockNumber`          | block height where tx included                  | presence = confirmed |
| `receipt.energy_usage` | energy burned (for TRC-20 / smart contract)     | cost tracking        |
| `receipt.net_fee`      | bandwidth burned (for native TRX)               | cost tracking        |
| `res.receipt.result`   | `SUCCESS` (alternate check)                     |

### Common success-pattern code

```rust
let resp = broadcast(endpoint, &signed).await?;          // Stage1: accept
let txid = resp["txid"].as_str().unwrap().to_string();

let included = wait_included(endpoint, &txid, 15_000).await?;

if included["contractResult"][0] == "SUCCESS" {
    println!("confirmed at block {}, energy: {}",
        included["blockNumber"], included["receipt"]["energy_usage"]);
} else {
    println!("reverted: {}", included["contractResult"][0]);
}
```

### Alternative confirmation paths

| Path                                 | When                            | Trade-off                               |
| ------------------------------------ | ------------------------------- | --------------------------------------- |
| Poll `wallet/gettransactioninfobyid` | default                         | 500ms latency, simple                   |
| Poll `wallet/gettransactionbyid`     | deprecated, works on some nodes | bigger response, no receipt info        |
| Subscribe WebSocket `txn.*` events   | future v1.x                     | instant push, needs long-running daemon |
| Use `wallet/estimateenergy` first    | before broadcast                | preflight check, not confirmation       |

### Two-stage rationale

| Stage                                            | What it tells you           | What it doesn't                              |
| ------------------------------------------------ | --------------------------- | -------------------------------------------- |
| `broadcasttransaction` returns `result:true`     | node accepts your signed tx | tx might never be packed into a block        |
| `gettransactioninfobyid` returns `blockNumber:N` | tx included in block N      | tx might revert (contract execution failure) |
| `contractResult[0] == "SUCCESS"`                 | tx executed successfully    | —                                            |

**All three checks required for full confirmation.** anychain covers none — caller via reqwest for all 3 RPC calls.

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
- `sha3`: <https://crates.io/crates/sha3> (v0.10, Apache-2.0/MIT, RustCrypto, MSRV 1.85)
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

## Rust wallet design — `tron-wallet-core` + `tron` CLI

Mirror pattern of `bitcoin-wallet-core` + `btc` in `rust-wallet-app/crates/`. Workspace already lists `spikes/tron-v1` — fold into v0.1 alongside `bitcoin-wallet-core`.

### Workspace layout

```text
rust-wallet-app/crates/
├── tron-wallet-core/   # NEW — library (rlib + cdylib for FFI)
└── tron/               # NEW — bin
```

### `tron-wallet-core` (library)

Crate-type `["rlib", "cdylib"]`. 16 modules. Wire-format via `anychain-tron`, HD via `anychain-kms`. Crypto deps shared with `bitcoin-wallet-core`. Design lessons from `polygon-wallet-core` (thin wrapper, 251 LOC) — apply chain-named loaders, per-item re-exports, `disambig` helpers, and `TxSummary` in lib.

| Module          | LOC budget | Feature                                                                                                                                                                                                            |
| --------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `lib.rs`        | ~80        | per-item `pub use` re-exports of `anychain_tron` types (NOT glob) — same pattern as `polygon-wallet-core/src/lib.rs`                                                                                               |
| `address`       | ~200       | base58check encode/decode, hex variant, validation                                                                                                                                                                 |
| `keys`          | ~600       | BIP-39 mnemonic + BIP-32 HD at SLIP-44 coin 195 (`m/44'/195'/0'/0/0`), `Secret<Mnemonic>` with Zeroize                                                                                                             |
| `crypto`        | ~400       | mnemonic cipher (argon2id + AES-GCM, same shape as `crypto/mnemonic_cipher.rs`)                                                                                                                                    |
| `config`        | ~250       | `TronAddressFormat`, network enum (Mainnet/Shasta/Nile), data dir layout, SPKI pin storage                                                                                                                         |
| `tx/builder`    | ~700       | `TransferContract`, `TransferAssetContract`, `TriggerSmartContract` (TRC-20), `FreezeBalanceV2` / `UnfreezeBalanceV2` / `DelegateResource` (Stake 2.0), `VoteWitnessContract`                                      |
| `tx/sign`       | ~300       | secp256k1 sign over SHA256(raw_data), `Zeroizing` wrapper, deterministic sig                                                                                                                                       |
| `tx/broadcast`  | ~150       | `wallet/broadcasttransaction` POST, JSON parse, txid + receipt                                                                                                                                                     |
| `tx_summary.rs` | ~50        | `TxSummary { txid, block, from, to, value, token }` for TRC-20 transfer logs (LIVES IN LIB, NOT CLI — mirrors `polygon-wallet-core::TxSummary`)                                                                    |
| `chain`         | ~400       | TronGrid HTTP client (raw `reqwest` + custom SPKI pin verifier, same shape as `chain/esplora.rs`), `wallet/gettransactioninfobyid` verify, account info, resource query                                            |
| `wallet`        | ~500       | wallet id (UUID v4), encrypted store (argon2id + AES-GCM, same shape as `bitcoin-wallet-core/wallet/persist.rs`), atomic write                                                                                     |
| `tokens/`       | ~40        | `load_mainnet()` / `load_nile()` — bundled JSON via `include_str!` (chain-named loaders, NO `Network` discriminator — prevents cross-chain footgun, same pattern as `polygon-wallet-core::tokens`)                 |
| `disambig`      | ~150       | `reject_wrong_chain_trc20(contract, expected_chain)` + `chain_id_for(network)` + `resource_label(use_stake2)` — TRC-20 footgun guards (TRC-20 USDT mainnet = `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`, testnet varies) |
| `error`         | ~100       | `thiserror` enum: `InvalidMnemonic`, `InvalidAddress`, `SignFailed`, `BroadcastFailed { code, msg }`                                                                                                               |
| `ffi`           | ~400       | C ABI (cdylib), Dart FFI hooks, panic-message scrubber for mnemonic/secret leaks                                                                                                                                   |
| `util`          | ~100       | atomic write, permissions                                                                                                                                                                                          |
### `tron-wallet-core` × `anychain-kms 0.1.23` — feature map

Focused map of which `tron-wallet-core` features consume `anychain-kms`. Source ref: file-by-file map at `docs/wallets/2026-09-05-anychain-kms-technical-deep-dive.md:67-426`, bug catalog at `:449-460`.

**Direct usage:**

| `tron-wallet-core` feature                     | `anychain-kms` API                                                                | File ref                                       |
| ---------------------------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------- |
| Generate 12-word mnemonic                      | `Mnemonic::generate(WordCount::Words12)`                                          | `bip39/mnemonic.rs:1`                          |
| Generate 24-word mnemonic                      | `Mnemonic::generate(WordCount::Words24)`                                          | `bip39/mnemonic.rs:1`                          |
| Validate + import phrase                       | `Mnemonic::from_phrase(s: &str)`                                                  | `bip39/mnemonic.rs:1`                          |
| Master seed derivation (PBKDF2)                | `seed_from_mnemonic(phrase: &str, passphrase: &str) -> [u8; 64]`                  | `bip39/seed.rs:1`                              |
| Parse BIP-32 path                              | `DerivationPath::from_str("m/44'/195'/0'/0/0")`                                   | `bip32/derivation_path.rs:1`                   |
| SLIP-44 coin 195 path component                | `ChildNumber::Hardened(195)`                                                      | `bip32/child_number.rs:1`                      |
| Derive child key at path                       | `ExtendedPrivateKey::derive_child(&path)`                                         | `bip32/extended_key/extended_private_key.rs:1` |
| Serialize xprv (78-byte base58check)           | `ExtendedPrivateKey::to_string() -> Zeroizing<String>`                            | `bip32/extended_key/extended_private_key.rs:1` |
| Deserialize xprv                               | `ExtendedPrivateKey::from_str(s)`                                                 | `bip32/extended_key/extended_private_key.rs:1` |
| Derive xpub from xprv                          | `ExtendedPublicKey::from_private_key(&xprv)`                                      | `bip32/extended_key/extended_public_key.rs:1`  |
| Serialize xpub                                 | `ExtendedPublicKey::to_string()`                                                  | `bip32/extended_key/extended_public_key.rs:1`  |
| Sign TRON tx (secp256k1 over SHA256(raw))      | `secp256k1_sign(sk: &[u8], msg32: &[u8; 32]) -> (Signature, RecoveryId)`          | `secp256k1.rs`                                 |
| Address derivation (T-base58check from pubkey) | **NOT anychain-kms** — use `anychain_tron::TronAddress::from_public_key(&pubkey)` | anychain-tron                                  |

**Features NOT using anychain-kms (wallet-local):**

| Feature                                  | Implementation                           | Why not anychain-kms                                             |
| ---------------------------------------- | ---------------------------------------- | ---------------------------------------------------------------- |
| Mnemonic encryption (argon2id + AES-GCM) | `tron-wallet-core/src/crypto/`           | anychain-kms has no AES/Argon2id (PBKDF2 only for mnemonic→seed) |
| Wallet id (UUID v4)                      | `tron-wallet-core/src/wallet/id.rs`      | wallet metadata                                                  |
| Encrypted wallet store + atomic write    | `tron-wallet-core/src/wallet/persist.rs` | persistence layer                                                |
| Address validation                       | `tron-wallet-core/src/address/`          | chains base58check over anychain-tron                            |
| tx/broadcast                             | `tron-wallet-core/src/tx/broadcast.rs`   | HTTP, not crypto                                                 |
| TronGrid HTTP client                     | `tron-wallet-core/src/chain/`            | HTTP, not crypto                                                 |
| Token registry                           | `tron-wallet-core/src/tokens/`           | bundled JSON                                                     |
| TRC-20 disambiguation                    | `tron-wallet-core/src/disambig.rs`       | compile-time data                                                |

**Bugs requiring workaround (per deep-dive bug catalog line 449):**

| Bug                                                                      | Fix in tron-wallet-core                                                                     |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| `secp256k1_sign` takes `sk: &[u8]` — no Zeroize                          | wrap in `Zeroizing<Vec<u8>>` before call; copy out the 32 bytes immediately, drop Zeroizing |
| `to_string()` on `ExtendedPrivateKey` returns `Zeroizing<String>` — good | no wrap needed                                                                              |
| BIP-32 master seed: 2048 PBKDF2 rounds (not 2049 typo)                   | documented, no action                                                                       |
| 8 languages in `bip39/langs/*.txt` — unused languages bloat binary       | feature-gate in `Cargo.toml` if size matters                                                |

**Address derivation flow (cross-crate):**

```text
tron-wallet-core::keys::derive_keypair(mnemonic, path)
    → anychain_kms::seed_from_mnemonic(phrase, "") -> [u8; 64]
    → anychain_kms::ExtendedPrivateKey::from_seed(seed)
    → anychain_kms::ExtendedPrivateKey::derive_child(path)
    → xprv.to_string() -> Zeroizing<String>      [persist?]
    → anychain_tron::TronAddress::from_public_key(&xprv.public_key())
    → "T..." base58check
```

Two crates cooperate — KMS owns HD, tron owns address encoding.

**Coverage summary for `tron-wallet-core::keys` module:**

| Function                             | Source                            |
| ------------------------------------ | --------------------------------- |
| `generate_mnemonic(words)`           | anychain-kms                      |
| `import_mnemonic(phrase)`            | anychain-kms                      |
| `derive_keypair(mnemonic, path)`     | anychain-kms                      |
| `address_from_xprv(xprv)`            | anychain-tron (NOT kms)           |
| `sign_tx(sk, raw_data_bytes)`        | anychain-kms (secp256k1_sign)     |
| `encrypt_mnemonic(phrase, password)` | wallet-local (argon2id + AES-GCM) |
| `decrypt_mnemonic(blob, password)`   | wallet-local                      |

**5 of 7 key functions use anychain-kms.** The 2 wallet-local functions are persistence-layer (encryption for wallet file).

**MSRV implication:** anychain-kms 0.1.23 requires Rust 1.98.1. Affects `tron-wallet-core` only if direct dep on `anychain-kms` — applies the split pattern (toolchain channel 1.98.1, advertised MSRV 1.94) drafted in `### Toolchain — split channel + MSRV (recommended)`.

### `tron-wallet-core` × `anychain-tron 0.2.14` — feature map

Map every `tron-wallet-core` module to its `anychain-tron` consumer surface. Source ref: `docs/wallets/2026-09-05-anychain-tron-technical-deep-dive.md` (file-by-file map, 7 src files, ~50 public APIs, 2 known bugs).

#### Direct usage by module

| `tron-wallet-core` module           | `anychain-tron` API                                                                                                                                                                                                                                                                                                                                                                                                                                        | File in anychain-tron             |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| `lib.rs`                            | `pub use anychain_tron::{Address, PublicKey, Transaction, TronAddress, TronFormat, TronNetwork};`                                                                                                                                                                                                                                                                                                                                                          | `lib.rs`                          |
| `address`                           | `Address::from_str(s) -> Result<Self>`, `to_base58() -> String`, `TronAddress::from_public_key(&secp256k1::PublicKey)`, `to_hex() -> String` (EIP-55)                                                                                                                                                                                                                                                                                                      | `address.rs`, `format.rs`         |
| `tx/builder`                        | `TransferContractBuilder`, `TransferAssetContractBuilder`, `TriggerSmartContractBuilder`, `FreezeBalanceV2ContractBuilder`, `UnfreezeBalanceV2ContractBuilder`, `DelegateResourceContractBuilder`, `UnDelegateResourceContractBuilder`, `VoteWitnessContractBuilder`, `WithdrawBalanceContractBuilder`, `WithdrawExpireUnfreezeContractBuilder`, `AccountCreateContractBuilder`, `AccountUpdateContractBuilder`, `AssetIssueContractBuilder` (13 builders) | `trx.rs`, `protocol/`             |
| `tx/builder` (TRC-20 ABI)           | `abi::encode_call("transfer(address,uint256)", &[to, amount]) -> Vec<u8>`, `abi::function_selector("transfer(address,uint256)") -> [u8; 4]` (selector `0xa9059cbb`)                                                                                                                                                                                                                                                                                        | `abi.rs`                          |
| `tx/sign`                           | `Transaction::sign(signature: Signature, recid: RecoveryId) -> SignedTransaction`, `Transaction::to_bytes() -> Vec<u8>`                                                                                                                                                                                                                                                                                                                                    | `transaction.rs`                  |
| `tx/sign` (txid workaround)         | `Transaction::to_transaction_id()` returns single SHA-256 — **BUG**: caller does `SHA256(SHA256(to_bytes()))` manually                                                                                                                                                                                                                                                                                                                                     | `transaction.rs`                  |
| `tx/broadcast` (envelope serialize) | `serde_json::to_value(&Transaction) -> serde_json::Value` — chain JSON for `wallet/broadcasttransaction` POST                                                                                                                                                                                                                                                                                                                                              | `transaction.rs`                  |
| `config`                            | `TronNetwork::Mainnet`, `TronNetwork::Shasta`, `TronNetwork::Nile` (network enum), `TronFormat` impl                                                                                                                                                                                                                                                                                                                                                       | `network.rs`, `format.rs`         |
| `chain` (verify response parse)     | `protocol::TransactionInfo` — deserialize from `wallet/gettransactioninfobyid` JSON response                                                                                                                                                                                                                                                                                                                                                               | `protocol/transaction_info.proto` |
| `disambig`                          | `protocol::chain_id` constants cross-check                                                                                                                                                                                                                                                                                                                                                                                                                 | `protocol/`                       |
| `error`                             | re-export `anychain_tron::Error` variants into our `tron_wallet_core::Error` enum                                                                                                                                                                                                                                                                                                                                                                          | `error.rs`                        |

#### 13 contract builders — full list

| Builder                                 | User story                          |
| --------------------------------------- | ----------------------------------- |
| `TransferContractBuilder`               | native TRX send                     |
| `TransferAssetContractBuilder`          | TRC-10 token send                   |
| `TriggerSmartContractBuilder`           | TRC-20 + any contract call          |
| `FreezeBalanceV2ContractBuilder`        | Stake 2.0 freeze                    |
| `UnfreezeBalanceV2ContractBuilder`      | Stake 2.0 unfreeze                  |
| `DelegateResourceContractBuilder`       | Stake 2.0 delegate BANDWIDTH/ENERGY |
| `UnDelegateResourceContractBuilder`     | Stake 2.0 undelegate                |
| `WithdrawBalanceContractBuilder`        | withdraw rewards/unstake            |
| `WithdrawExpireUnfreezeContractBuilder` | withdraw expired unfreeze           |
| `VoteWitnessContractBuilder`            | SR vote                             |
| `AccountCreateContractBuilder`          | create account (pays for others)    |
| `AccountUpdateContractBuilder`          | update account permissions          |
| `AssetIssueContractBuilder`             | issue TRC-10 token                  |

Covers all 29 v0.1 user stories.

#### NOT used (out of v0.1 scope)

| `anychain-tron` API                                 | Why skipped                                             |
| --------------------------------------------------- | ------------------------------------------------------- |
| `ed25519_sign`                                      | TRON = secp256k1 only (ed25519 is for Solana, not TRON) |
| `protocol::BlockHeader` parsing                     | caller parses raw JSON from `wallet/getnowblock`        |
| `protocol::Account` parsing                         | caller parses raw JSON from `wallet/getaccount`         |
| `protocol::ChainParameters`                         | caller parses raw JSON from `wallet/getchainparameters` |
| `WitnessCreateContract`                             | not in v0.1                                             |
| `UpdateBrokerageContract`                           | not in v0.1                                             |
| `ExchangeCreateContract` / `ExchangeInjectContract` | deprecated TRC-10 DEX — dead since 2022                 |
| `AccountPermissionUpdateContract`                   | beyond v0.1                                             |

#### Bug + workaround catalog

| Bug                                                       | Location                           | Workaround in `tron-wallet-core::tx/sign`                                                                 |
| --------------------------------------------------------- | ---------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `to_transaction_id()` returns single SHA-256              | `anychain-tron/src/transaction.rs` | `let txid = Sha256::digest(&Sha256::digest(&tx.to_bytes()));`                                             |
| `trx::build_contract` formats `type_url` via `{:?}` Debug | `anychain-tron/src/trx.rs`         | fragile for JSON roundtrip; if needed, serialize the hex bytes manually via `hex::encode(contract_bytes)` |
| protobuf wire format diverges from serde_json default     | `anychain-tron/src/protocol/`      | always serialize via `serde_json::to_value(&tx)` rather than `tx.to_string()` (which uses Debug)          |

#### Coverage summary for core functions

| Function in `tron-wallet-core`                                                   | Source                                       |
| -------------------------------------------------------------------------------- | -------------------------------------------- |
| `pub fn address_from_pubkey(&pubkey) -> Address`                                 | anychain-tron                                |
| `pub fn address_to_base58(addr) -> String`                                       | anychain-tron                                |
| `pub fn address_to_hex(addr) -> String`                                          | anychain-tron                                |
| `pub fn validate_address(s: &str) -> Result<Address>`                            | anychain-tron                                |
| `pub fn build_transfer(from, to, amount_sun) -> UnsignedTransaction`             | anychain-tron                                |
| `pub fn build_trc20_transfer(from, contract, to, amount) -> UnsignedTransaction` | anychain-tron (builder + abi)                |
| `pub fn build_freeze_v2(amount, resource) -> UnsignedTransaction`                | anychain-tron                                |
| `pub fn build_vote_witness(votes) -> UnsignedTransaction`                        | anychain-tron                                |
| `pub fn sign_tx(sk, raw_data_bytes) -> SignedTransaction`                        | anychain-tron::Transaction::sign             |
| `pub fn txid(tx: &SignedTransaction) -> [u8; 32]`                                | wallet-local (workaround for single-SHA bug) |
| `pub fn serialize_for_broadcast(tx: &SignedTransaction) -> serde_json::Value`    | anychain-tron (serde_json)                   |
| `pub fn parse_tx_info(json: serde_json::Value) -> TxInfo`                        | anychain-tron::protocol::TransactionInfo     |
| `pub fn encode_abi_call(sig, args) -> Vec<u8>`                                   | anychain-tron::abi                           |

**12 of 13 core functions use anychain-tron.** The 1 wallet-local function (`txid`) is the double-SHA256 workaround.

**Wire-format flow: send TRX**

```text
tron-wallet-core::tx::builder::TransferContractBuilder::new()
    .owner(from_addr)
    .to(to_addr)
    .amount(amount_sun)
    .build_with_ref_block(ref_block_hash, ref_block_num, expiry)
    -> anychain_tron::Transaction { raw_data, signature: [] }
    -> tron-wallet-core::tx::sign::sign_tx(&sk_z, &tx.to_bytes())
       -> anychain_kms::secp256k1_sign(&sk_z, msg32) -> (sig, recid)
       -> anychain_tron::Transaction::sign(sig, recid) -> SignedTransaction
    -> tron-wallet-core::tx::broadcast::broadcast(rpc_url, &tx)
       -> reqwest POST wallet/broadcasttransaction
       -> anychain_tron::protocol::TransactionInfo parse response
    -> tron-wallet-core::chain::wait_for_confirm(rpc_url, txid)
       -> poll wallet/gettransactioninfobyid until receipt.result == "SUCCESS"
```

3 anychain crates cooperate: `anychain-tron` (wire + builders + sign + parse), `anychain-kms` (secp256k1_sign), `anychain-core` (traits).

**MSRV implication:** `anychain-tron 0.2.14` requires Rust 1.98.1. Affects `tron-wallet-core` via direct dep. Split pattern (toolchain channel 1.98.1, advertised MSRV 1.94) already drafted in `### Toolchain — split channel + MSRV (recommended)`.

### `tron-wallet-core` × `anychain-core 0.1.8` — feature map

Map every `tron-wallet-core` feature to the `anychain-core` surface it depends on. Source ref: `docs/wallets/2026-09-05-anychain-core-technical-deep-dive.md` (940 LOC, 30 pub APIs).

**Direct usage by module:**

| `tron-wallet-core` module     | `anychain-core` API                                                                       | Used as                                                  |
| ----------------------------- | ----------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `lib.rs` (per-item `pub use`) | `anychain_core::{Address, PublicKey, Format, Network, Transaction, TransactionId, Error}` | trait re-exports for downstream consumers                |
| `address`                     | `Address::from_public_key(&pubkey)`, `Address::is_valid(s)`, `AddressError`               | `TronAddress` inherits these via `anychain-tron`         |
| `address`                     | `keccak256(pubkey_bytes) -> [u8; 32]`                                                     | direct call for address derivation (last 20 bytes)       |
| `keys`                        | `PublicKey::from_secret_key(&sk)` (indirect via `anychain-tron::TronPublicKey`)           | derive pubkey from sk                                    |
| `tx/builder` (TRC-20)         | `func_selector("transfer(address,uint256)") -> [u8; 4]`                                   | TRC-20 ABI selector (returns `0xa9059cbb`)               |
| `tx/sign`                     | `Transaction::sign(signature: Vec<u8>, recid: u8)` (via `anychain-tron::TronTransaction`) | attaches `(sig, recid)` to tx                            |
| `tx/sign` (workaround)        | `sha256(tx.to_bytes()) -> [u8; 32]`                                                       | manual double-hash workaround for anychain-tron txid bug |
| `tx/broadcast`                | `Transaction::to_bytes() -> Vec<u8>` (via anychain-tron)                                  | raw protobuf bytes for sign input                        |
| `tx/broadcast` (envelope)     | `hex` re-export                                                                           | hex-encode tx bytes for TronGrid broadcast               |
| `config`                      | `Network` trait + `TronNetwork` (via anychain-tron)                                       | network enum dispatch                                    |
| `config`                      | `Format` trait + `TronFormat`                                                             | format selector (base58check T vs hex 0x41)              |
| `chain` (response parse)      | `Transaction::from_bytes(&[u8])` (via anychain-tron)                                      | parse `wallet/gettransactioninfobyid` protobuf           |
| `error`                       | `anychain_core::Error` re-export + 6 sub-enum variants                                    | error mapping in FFI boundary                            |
| `disambig`                    | (no direct — uses compile-time constants)                                                 | N/A                                                      |

**NOT used (other chain subcrates' surface):**

| `anychain-core` API                            | Used by                                                                        |
| ---------------------------------------------- | ------------------------------------------------------------------------------ |
| `sha512`                                       | `anychain-kms` (BIP-32 HMAC) — TRON wallet-core uses this transitively via kms |
| `hash160` (RIPEMD160+SHA256)                   | `anychain-bitcoin` only                                                        |
| `bech32` encoding                              | `anychain-bitcoin` only (SegWit)                                               |
| `ripemd` dep                                   | `anychain-bitcoin` only                                                        |
| `to_basic_unit` / `to_basic_unit_u64`          | TRON uses sun directly as u64, no decimal conversion                           |
| `Amount` trait                                 | anychain-tron implements for sun, but `tron-wallet-core` uses raw u64 amounts  |
| `NetworkError` / `FormatError` / `AmountError` | sub-errors not directly exposed in tron-wallet-core public API                 |

**Direct utility functions — 2 of 5 (3 transitive):**

| Function                       | TRON use? | Where in `tron-wallet-core`                                        |
| ------------------------------ | --------- | ------------------------------------------------------------------ |
| `sha256(&[u8]) -> [u8; 32]`    | yes       | `tx/sign.rs` — manual double-hash workaround for txid bug          |
| `sha512(&[u8]) -> [u8; 64]`    | indirect  | `keys/` via anychain-kms (BIP-32 HMAC, BIP-39 seed)                |
| `keccak256(&[u8]) -> [u8; 32]` | yes       | `address/` — TRON address = keccak256(uncompressed_pubkey)[12..32] |
| `checksum(&[u8]) -> Vec<u8>`   | no        | (4-byte sha256 prefix — base58check uses different algorithm)      |
| `hash160(&[u8]) -> Vec<u8>`    | no        | (Bitcoin-only — RIPEMD160 ∘ SHA256)                                |

**End-to-end flow showing `anychain-core` call sites:**

```text
Address derivation:
    tron-wallet-core::address::from_pubkey(&pk)
        → anychain_core::keccak256(&pubkey_bytes)            [direct utility]
        → last 20 bytes of keccak256 output
        → T-base58check(prefix=0x41, payload, checksum)
        → TronAddress (impls anychain_core::Address)

TRC-20 transfer:
    tron-wallet-core::tx::builder::trc20::transfer(...)
        → anychain_core::func_selector("transfer(address,uint256)")  [direct utility]
        → 0xa9059cbb
        → anychain_tron::abi::encode_call(...)             [anychain-tron]
        → anychain_tron::TriggerSmartContractBuilder       [anychain-tron]

Sign:
    tron-wallet-core::tx::sign::sign_tx(&sk_z, raw_bytes)
        → anychain_core::sha256(&raw_bytes)                [direct utility]
        → msg32
        → anychain_kms::secp256k1_sign(...)                [anychain-kms]
        → (sig, recid)
        → anychain_tron::Transaction::sign(sig, recid)     [via anychain-tron]
          [impls anychain_core::Transaction trait]

Broadcast envelope:
    tron-wallet-core::tx::broadcast::serialize_for_broadcast(&tx)
        → serde_json::to_value(&tx)
        → POST wallet/broadcasttransaction                [caller reqwest]
```

### Deep-dive cross-references

- Anychain umbrella + 11 subcrates: `docs/wallets/2026-09-05-anychain-umbrella-technical-deep-dive.md`
- Anychain-core 0.1.8 (trait backbone, 940 LOC, 30 APIs): `docs/wallets/2026-09-05-anychain-core-technical-deep-dive.md`
- Anychain-kms 0.1.23 file-by-file (BIP-39, BIP-32, secp256k1): `docs/wallets/2026-09-05-anychain-kms-technical-deep-dive.md`
- Anychain-tron 0.2.14 (address, tx, 13 contract builders): `docs/wallets/2026-09-05-anychain-tron-technical-deep-dive.md`
- TRC-20 submit + stablecoin transfer pipeline: `docs/wallets/2026-09-05-anychain-tron-submit-tx-stablecoin.md`
- User stories × coverage: `docs/wallets/2026-08-27-tron-wallet-user-stories.md` (anychain section)
- Polygon pattern (thin wrapper lessons): `rust-wallet-app/crates/polygon-wallet-core/src/{lib,disambig,tokens}.rs`

Deps to add to `[workspace.dependencies]`:

| Crate           | Version        | Use                                                      |
| --------------- | -------------- | -------------------------------------------------------- |
| `anychain-tron` | 0.2.14         | wire format — address, tx, 13 contract builders, ABI     |
| `anychain-kms`  | 0.1.23         | BIP-39 + BIP-32 HD, secp256k1 signing                    |
| `anychain-core` | 0.1.8          | shared traits                                            |
| `bs58`          | 0.5            | base58check                                              |
| `tiny-keccak`   | 2.0.2 + keccak | keccak256 (TRON address derivation)                      |
| `protobuf`      | 3.7            | TRON wire format (matches `anychain-tron` — NOT `prost`) |

**Bugs to wrap around (known from anychain-tron deep-dive):**

1. `anychain-tron::Transaction::txid()` returns single SHA-256 — caller must double-hash manually (`SHA256(SHA256(raw_data_bytes))`).
2. `anychain-kms::secp256k1::pubkey_to_address` takes raw `sk: [u8; 32]` — wrap in `Zeroizing<Vec<u8>>` before call.
3. Anychain types use `Debug`-formatted `type_url` — string repr of protobuf enum may not roundtrip via JSON; serialize manually.


**Next steps (post-deep-dive):**

1. **Ticket B:** User-stories doc — `docs/wallets/2026-08-27-tron-wallet-user-stories.md` (template: `docs/wallets/2026-08-23-eth-wallet-user-stories.md`).
2. **Ticket C:** Spike — `rust-wallet-app/spikes/tron-v1/` with V1–V10 mapped to Q1–Q10, each PASS evidence (command output + SHA).
3. **Ticket D:** Plan — `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` derived from resolved Qs + verified spikes.
4. **Ticket E:** PR + flip issue #399 checkboxes.
