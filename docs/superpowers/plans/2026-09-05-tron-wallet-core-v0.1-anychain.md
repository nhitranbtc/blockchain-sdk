# tron-wallet-core (v0.1) — Implementation Plan (anychain stack)

> **For agentic workers:** REQUIRED SUB-SKILLS: `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver `rust-wallet-app/crates/tron-wallet-core/` — a TRON (TRX + TRC-20 stablecoin) wallet library built on **`anychain-tron 0.2.14` + `anychain-kms 0.1.23` + `anychain-core 0.1.8`** (decision locked 2026-09-05, supersedes 2026-08-27 raw-primitives plan), plus a `tron` CLI in the umbrella workspace. Pulls anychain direct from crates.io (no vendoring — bus-factor accepted risk, mitigated via exact-version pin + regression tests). **Bumps MSRV to 1.98.1** (anychain workspace toolchain). Compiles for desktop (Linux/macOS/Windows) + mobile (iOS arm64 + Android arm64) via 4-trait PAL.

**Companion docs:**
- Research: `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md` (this plan's source of truth)
- User stories (legacy, outdated — must regenerate per mismatch report): `docs/wallets/2026-08-27-tron-wallet-user-stories.md`
- ADR capturing reversal: `docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md`
- Supersedes: `docs/superpowers/plans/2026-08-27-tron-wallet-core.md` (raw-primitives plan — kept for archaeology)

**Tracks:** issue #399 (Q1–Q10 closed by deep-dive Round-1 grill Q1–Q12). PR #402 (Ticket A — research) closed; this plan covers Ticket B–F (implementation).

**Pre-empts:** v0.3+ placeholder in `rust-wallet-app/crates/chain-traits/src/lib.rs:21` (`ChainId::Tron(u32)`).

**Status:** Plan. No code produced yet. **PAUSE before commit** per never-auto-commit rule.

---

## Global Constraints (verbatim from deep-dive Round-1 grill Q1–Q12)

- **Q1 — SDK choice.** **anychain-tron 0.2.14 + anychain-kms 0.1.23 + anychain-core 0.1.8.** Trades ~250 lines of `reqwest` glue for ~1000 lines of hand-rolled protobuf + base58check + Keccak-256 + ABI encoder + Stake 2.0 contract builders. Raw `reqwest` + `prost` 0.14.4 plan (2026-08-27) **REJECTED** by Round-1 audit — see ADR-0001.
- **Q2 — Transaction format.** Protobuf via anychain-tron's vendored `core/Tron.proto`. Signing: `SHA256(raw_bytes)` then `secp256k1_sign` → `TronTransaction::sign(sig, recid)`. **txid BUG workaround:** `TronTransaction::to_transaction_id()` returns single-SHA256; caller computes `SHA256(SHA256(raw_bytes))` manually in `tx/sign.rs`. Pin `anychain-tron` to exact `0.2.14` (NOT `^`) + add regression test asserting `txid == SHA256(SHA256(raw_bytes))` so any future anychain "fix" gets caught in CI.
- **Q3 — Bus-factor risk (accepted).** `0xcregis/anychain` author diversity trailing 12 months: `anychain-tron` = **1 author (`loki-cmu`), 3 commits**; `anychain-kms` = 2 authors, 8 commits. Bus-factor = 1. Risk accepted (not vendored). Mitigation: (a) pin exact versions in `Cargo.toml` via `=0.2.14` syntax — no `^` auto-bump; (b) regression tests for known bugs (dual-SHA256 txid in `v8_sign_only.rs`, Zeroizing gap in `tx/sign.rs`) catch silent behavior changes if upstream "fixes" them. Alternative considered: vendor into `rust-wallet-app/crates/anychain-vendored/` with per-file citation — **rejected by user 2026-09-05** (operational overhead exceeds benefit for v0.1 scope).
- **Q4 — Mainnet smoke gate.** **v0.1 release GATED on one mainnet self-send — $0.001 USDT to self (recipient == sender), real value, real network.** Local + Nile is emulation. Without a real-value smoke, V1-V10 PASS evidence = "looks like real network" not "real network". Add to acceptance criteria NOW, not post-Phase 4. Pre-check audit hook (refuse if recipient != operator_wallet) + `RUN_TRON_MAINNET=1` env gate.
- **Q5 — SPKI pin live extraction.** `api.trongrid.io` TLS cert SPKI SHA-256 = `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d` (verified 2026-09-05). Pin in `TronConfig::for_network(Network::Mainnet)` SPKI list. Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` shape-compatible.
- **Q6 — Testnet.** **Nile** for v0.1. Chain-id `0xcd8690dc` / 3448148188. Use `POST /jsonrpc {"method":"eth_chainId"}` (TronGrid's `/wallet/getchainid` returns HTTP 405). Address prefix byte `0x41` universal across Mainnet/Shasta/Nile — network discrimination by chain-id only.
- **Q7 — RPC pinning.** `pinned://<spki-hex>@host[:port]` URL scheme. Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` verbatim. Default CLI: Scenario B (no pin); operator opts into Scenario A via `pinned://`.
- **Q8 — Sign-only path.** `v ∈ {0, 1}` (NOT Ethereum's `v+27 ∈ {27, 28}`). anychain-kms returns recid ∈ {0, 1} natively. Audit control: compile-fail test rejecting eth-default `v+27` decoders in `tx/sign.rs`.
- **Q9 — Token registry.** Bundled JSON. **Nile USDT = `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`** (canonical per TronScan verified 2026-09-05). **CAUTION:** user-stories.md Story 21 quotes `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` — this is **WRONG**; deep-dive value is canonical. Tokens live at `tokens/{local,nile,mainnet}.json` (3 files including local — local added per Round-1 grill Q6 for TronBox Docker integration test).
- **Q10 — Mnemonic + SLIP-44.** Coin type 195 = TRX. Path `m/44'/195'/0'/0/0`. Spike V10 verifies canonical test vector ("abandon ×11 about") matches `bip_utils::slip44::Coin::Tron`.
- **Q11 — Stake 1.0.** Deferred entirely. `tron stake` = Stake 2.0 only. Add `tron stake1 withdraw` in v0.2 IF user demand. No silent Stake 1.0 code paths.
- **Q12 — Disambiguation guards.** Compile-time constants in `disambig.rs`: cross-network address check (refuse mainnet→nile send), TRC-20 footgun guards (refuse `transfer` to non-Tron address).
- **Round-1 grill Q8 audit:** every "Status: ready" row is by inspection, NOT by spike PASS. Before v0.1 ships, audit each against V1-V10. If a command isn't covered by Vn spike PASS block, demote to "ready (untested)" or remove from v0.1.

---

## Architecture (locked 2026-09-04)

### Five-layer PAL design

```text
Layer 5: FFI (cdylib)
   - C ABI surface (extern "C" fn wallet_unlock, ...)
   - Panic-message scrubber
   - tokio runtime pinned (single-threaded current_thread)

Layer 4: Pure Rust Core (portable, 95%)
   - address/, keys/, tx/builder, tx/sign
   - crypto (argon2id + AES-GCM logic)
   - error, disambig, config (types), util
   - tx_summary, tokens (bundled JSON via include_str!)

Layer 3: PAL — 4 traits
   - WalletStorage (encrypted blob persistence)
   - PlatformInfo (data dir, app name, version, is_mobile)
   - NetworkClient (HTTP + TLS root certs)
   - Clock (monotonic time for tx expiration)

Layer 2: Platform impls (5%)
   Desktop: FileWalletStorage, SystemDirsInfo, ReqwestClient
   iOS:     KeychainWalletStorage, BundleInfo, OSRootsClient
   Android: EncryptedFileWalletStorage, ContextInfo, OSRootsClient
   Tests:   InMemoryStorage, StaticInfo, MockClient

Layer 1: OS + hardware
   Desktop: filesystem + OpenSSL/ring
   iOS: Keychain Services + Secure Enclave + Apple Trust Store
   Android: EncryptedFile + Android Keystore + StrongBox
```

### V0.1 CLI surface (22 commands, 6 top-level)

| Top-level | Commands                                                                                          | Stories              |
|-----------|---------------------------------------------------------------------------------------------------|----------------------|
| `wallet`  | create, import, show, list, delete, rename, balance, send, send-speedup (9)                      | 1, 2, 3, 5, 9, 10, 11, 12, 17, 22 |
| `address` | new, xpub (2)                                                                                     | 3, 19                |
| `balance` | --address, --address --token (2)                                                                  | 3, 22                |
| `trc20`   | send, approve, balance, allowance (4)                                                             | 21, 22, 25, 30       |
| `tx`      | get, wait (2)                                                                                     | 7                    |
| `config`  | show, set-rpc, set-network (3)                                                                    | 10, 11, 26, 27       |

**Story coverage matrix** (from deep-dive V0.1 feature map):

| Stories                                  | Status     |
|------------------------------------------|------------|
| 1, 2, 3, 5, 7, 9, 10, 11, 12, 17, 19, 21, 22, 25, 27, 28, 29 | shipped V0.1 |
| 4, 6, 31, 32, 33                         | V0.1.5 (Stake 2.0, ships with V0.1 release train) |
| 8, 18, 23, 24, 30, 34, 35, 36            | V0.2 (deferred — `tron resource`, `tron sign-message`, `tron tokens list/register`, `tron tokens balances`, `tron trc10 issue/send/buy`, `tron governance propose/approve`, `tron storage buy/sell`) |
| 26 (TronBox local)                       | shipped via `--rpc http://127.0.0.1:8090` flag |
| **13 (batch), 14 (drain), 15 (ref-block), 16 (manual exp)** | **DROPPED from V0.1** — redesign deferred (user-stories.md must update to mark these as removed) |

### Cross-crate flows (stable)

**Address derivation (kms → tron cooperation):**
```text
keys::derive_keypair(mnemonic, path)
    → anychain_kms::seed_from_mnemonic(phrase, "") -> [u8; 64]
    → anychain_kms::ExtendedPrivateKey::from_seed(seed)
    → anychain_kms::ExtendedPrivateKey::derive_child(path)
    → xprv.to_string() -> Zeroizing<String>
    → anychain_tron::TronAddress::from_public_key(&xprv.public_key())
    → "T..." base58check
```

**Sign + broadcast TRX tx:**
```text
tx::sign::sign_tx(&sk, raw_data_bytes)
    → anychain_core::sha256(&raw_bytes)            [direct utility]
    → msg32
    → anychain_kms::secp256k1_sign(&sk_z, msg32)    [Zeroizing wrapper — GAP]
    → (sig, recid)
    → anychain_tron::Transaction::sign(sig, recid)
    → SignedTransaction { txid: SHA256(SHA256(raw)), signature }

tx::broadcast::serialize_for_broadcast(&tx)
    → serde_json::to_value(&tx)
    → POST wallet/broadcasttransaction             [caller reqwest]
```

---

## Rust SDKs, tools, and crates — full inventory

**Total: 38 crates** (33 mobile-safe 87% + 4 desktop-only 11% + 1 build-time 2%) per deep-dive §"Crates used in `tron-wallet-core` (V0.1)".

### A. anychain stack (PRIMARY — wire-format + HD + signing) [workspace deps]

| Crate           | Version | Purpose                                                                                |
|-----------------|---------|----------------------------------------------------------------------------------------|
| `anychain-core` | 0.1.8   | Shared traits (`Address`, `PublicKey`, `Transaction`, `Format`, `Network`), crypto utilities (`keccak256`, `sha256`, `func_selector`), `hex` re-export |
| `anychain-tron` | 0.2.14  | Wire format — T-base58check address, protobuf `Transaction` envelope, **17 contract builders** (TRX transfer, TRC-20 transfer/approve, Stake 2.0 freeze/unfreeze/delegate/cancel/withdraw, witness vote, withdraw vote, account create, generic trigger), `abi::encode_call` |
| `anychain-kms`  | 0.1.23  | BIP-39 mnemonic (8 languages), BIP-32 HD (SLIP-44 coin 195), secp256k1 sign, xprv serialize with `Zeroizing<String>` |

**Pin to exact version (NOT `^`).** Phase 0 Task 0.2 enforces this in `[workspace.dependencies]`.

### B. Vendoring — considered and REJECTED (Round-1 grill Q3, revised 2026-09-05)

| Source repo            | Considered path                                            | Outcome                                   |
|------------------------|----------------------------------------------------------|------------------------------------------|
| `0xcregis/anychain`    | `rust-wallet-app/crates/anychain-vendored/{core,tron,kms}/` | **Rejected 2026-09-05** — operational overhead exceeds the benefit at v0.1 scope. Bus-factor = 1 for `anychain-tron` (1 author, 3 commits trailing 12 months) is an **accepted risk**, mitigated by (a) exact `=X.Y.Z` pins and (b) regression tests that assert the known dual-SHA256 txid and `Zeroizing` behaviours, so a silent upstream "fix" fails CI. Crates are pulled direct from crates.io. |

### C. Crypto (RustCrypto ecosystem) [workspace deps]

| Crate                      | Version   | Purpose                                                                                          |
|----------------------------|-----------|--------------------------------------------------------------------------------------------------|
| `argon2`                   | 0.5       | Argon2id KDF for wallet file encryption                                                          |
| `aes-gcm`                  | 0.10      | AES-256-GCM symmetric encryption for `EncryptedWallet` blob                                      |
| `sha2`                     | 0.10      | SHA-256 (txid double-hash workaround per Q2)                                                     |
| `sha3`                     | workspace | Keccak-256 (TRON address derivation via `anychain_core::keccak256`)                              |
| `tiny-keccak`              | 2.0.2     | Direct keccak256 call (kept to avoid anychain indirection)                                       |
| `bs58`                     | 0.5       | base58check encoding (T-addresses, xprv)                                                         |
| `hex`                      | workspace | hex encode/decode for protobuf serialization                                                     |
| `zeroize`                  | 1.x       | Secure memory hygiene (`Zeroizing<Vec<u8>>` wrap on raw `sk` before `secp256k1_sign`)            |
| `subtle`                   | 2         | Constant-time comparison (`ConstantTimeEq` for SPKI pin, xprv compare)                            |
| `libsecp256k1` (secp256k1) | workspace | ECDSA signing via anychain-kms (`secp256k1_sign`)                                                |

### D. Encoding / serialization [workspace deps]

| Crate            | Version   | Purpose                                                                 |
|------------------|-----------|-------------------------------------------------------------------------|
| `serde`          | 1.x       | derive Serialize/Deserialize                                            |
| `serde_json`     | 1.x       | JSON for TronGrid HTTP envelope + receipt parsing                       |
| `protobuf`       | 3.7       | TRON wire format (proto-generated types in `anychain-tron/src/protocol/`) |
| `ethereum-types` | workspace | Address type for ABI encoder                                            |
| `ethabi`         | workspace | TRC-20 ABI encode/decode (EIP-20 compatible)                            |
| `chrono`         | workspace | timestamp handling for tx expiration                                    |
| `uuid`           | 1.x       | Wallet id (UUID v4)                                                     |
| `directories`    | workspace | Desktop data dir resolution — **V0.1.5 removal**, replaced by `WalletStorage` trait |

### F. Async + HTTP [workspace deps]

| Crate                 | Version | Purpose                                                                                  |
|-----------------------|---------|------------------------------------------------------------------------------------------|
| `tokio`               | 1.x     | Async runtime (current_thread for FFI; multi-thread for CLI)                             |
| `reqwest`             | 0.12    | HTTP client for TronGrid (`broadcast`, `gettxinfo`, `getnowblock`, `getaccount`, `getaccountresource`, `triggerconstantcontract`) |
| `rustls`              | 0.23    | TLS for reqwest + custom SPKI pin verifier                                                |
| `rustls-native-certs` | 0.7     | Desktop OS root cert loading                                                              |
| `webpki`              | 0.22    | Custom `ServerCertVerifier` for SPKI pinning                                             |
| `x509-parser`         | 0.16    | SPKI DER extraction from cert chain                                                       |

### G. Errors + tracing + FFI safety [workspace deps]

| Crate                | Version   | Purpose                                                                  |
|----------------------|-----------|--------------------------------------------------------------------------|
| `thiserror`          | 1.x       | `Error` enum derive                                                      |
| `tracing`            | workspace | Structured logging (STDERR, secret-scrubbing filter)                     |
| `tracing-subscriber` | workspace | Subscriber with EnvFilter                                                |
| `once_cell`          | 1.x       | Lazy-init for FFI runtime + compiled regex patterns                      |
| `regex`              | 1.x       | Panic-message scrubber (redact mnemonic + password + xprv + secret)      |
| `cbindgen`           | workspace | Generates C header for FFI consumers (build-time only, doesn't ship)     |

### H. Test-only (dev-dependencies)

| Crate            | Purpose                                                                  |
|------------------|--------------------------------------------------------------------------|
| `proptest`       | Property-based tests for amount parsing, address derivation              |
| `tempfile`       | Atomic-write test fixtures                                               |
| `testcontainers` | TronBox Docker auto-spawn (`0.23`, desktop-only, mobile skips via `--no-default-features`) |
| `bitcoind`       | regtest smoke tests (desktop-only)                                       |

### I. Build tools + system dependencies (CI + dev environment)

| Tool        | Version       | Role                                                                  |
|-------------|---------------|-----------------------------------------------------------------------|
| `protoc`    | **≥3.12**     | System protobuf compiler invoked by anychain-tron build (transitive). Required only if anychain-tron uses `prost-build` at runtime. Spike README documents install path. |
| `cargo`     | 1.98.1 stable | **MSRV bump from 1.85 to 1.98.1** (anychain workspace toolchain pin). Pin via `rust-toolchain.toml`. |
| **TronBox** | **4.10.0+**   | Local dev regtest + `MockTRC20` deploy. Node ≥20. NOT a Rust dep — runs in Node for spike regtest only via `testcontainers`. |

### J. Cross-crate reuse (NOT a new dep — single import)

| Source                  | Path                                                | Role                                                          |
|-------------------------|-----------------------------------------------------|--------------------------------------------------------------|
| `SpkiPinnedVerifier`    | `bitcoin_wallet_core::chain::spki`                  | Custom `rustls::ServerCertVerifier` for SPKI-pinned JSON-RPC transport. Same `rustls = "0.23"` version as TRON-side `reqwest`. Single import, zero new code. |

### License summary (all compatible with workspace MIT)

MIT OR Apache-2.0 (anychain-*, argon2, aes-gcm, sha2, sha3, bs58, hex), Apache-2.0 (reqwest, serde_json, protobuf, ethereum-types, ethabi, libsecp256k1 variant), MIT (serde, tokio, zeroize, etc.), BSD (libsecp256k1).

### Mobile-unsafe deps (must remove for V0.1.5)

| Crate                      | Reason                            | Replacement                                       |
|----------------------------|-----------------------------------|---------------------------------------------------|
| `directories`              | No iOS/Android backend           | `WalletStorage` trait (File/Keychain/EncryptedFile) |
| `rustls-native-certs`      | Desktop-only OS cert loader       | `tls_built_in_root_certs(true)` on mobile (reqwest) |
| `testcontainers` (dev-dep) | Requires Docker                  | Skip on mobile via `cfg(not(target_os = "android"))` + `--no-default-features` |
| `bitcoind` (dev-dep)       | Test fixture only                | Same gating                                       |

**V0.1.5 work:** remove `directories` (1 crate), gate `rustls-native-certs` behind `#[cfg(not(mobile))]`, gate `testcontainers` + `bitcoind` behind `#[cfg(desktop)]`. ~30 LOC of Cargo.toml changes.

---

## File Structure

```text
rust-wallet-app/crates/tron-wallet-core/        # V0.1 — fat standalone (~4525 LOC)
├── Cargo.toml
├── src/
│   ├── lib.rs                                  # per-item `pub use` re-exports (NOT glob)
│   ├── error.rs                                # thiserror Error + sub-enums (anychain_core::Error re-export)
│   ├── config.rs                               # TronConfig { network, rpc_url, spki_pin, fee_limit_sun, data_dir }
│   ├── disambig.rs                             # cross-network address check + TRC-20 footgun guards (Round-1 grill Q12)
│   ├── address/                                # wraps anychain_tron::TronAddress + to_base58 + to_hex + is_valid
│   ├── keys/                                   # BIP-39 + BIP-32 via anychain_kms
│   ├── crypto/                                 # argon2id + AES-GCM (wallet-local; anychain has none)
│   ├── tx/
│   │   ├── builder.rs                          # 17+ contract builders via anychain_tron::trx
│   │   ├── sign.rs                             # secp256k1_sign + Zeroizing wrapper + dual-SHA256 txid workaround
│   │   ├── broadcast.rs                        # POST wallet/broadcasttransaction via reqwest
│   │   └── summary.rs                          # TxSummary struct (transfer log)
│   ├── chain/                                  # TronGridClient HTTP client + SPKI pin verifier
│   ├── wallet/                                 # UUID id + encrypted store + atomic write (PAL-bound)
│   ├── tokens/
│   │   ├── mod.rs                              # bundled JSON via include_str!
│   │   ├── local.json                          # TronBox Docker mock USDT (Round-1 grill Q6)
│   │   ├── nile.json                           # community test USDT
│   │   └── mainnet.json                        # USDT/USDC/TUSD/USDD/stUSDT
│   ├── platform/                               # PAL — 4 traits + per-platform impls
│   │   ├── mod.rs                              # trait re-exports + default_storage/info/network_client
│   │   ├── storage.rs                          # WalletStorage trait
│   │   ├── info.rs                             # PlatformInfo trait
│   │   ├── network.rs                          # NetworkClient trait
│   │   ├── clock.rs                            # Clock trait
│   │   ├── desktop/                            # Linux/macOS/Windows impls
│   │   ├── ios/                                # iOS impls (KeychainWalletStorage, BundleInfo)
│   │   ├── android/                            # Android impls (EncryptedFile, ContextInfo)
│   │   └── test/                               # InMemoryStorage, StaticInfo, MockClient
│   ├── ffi/                                    # cdylib C ABI (Layer 5)
│   │   ├── lib.rs                              # extern "C" exports
│   │   ├── runtime.rs                          # tokio pinned single-thread runtime
│   │   └── panic_scrubber.rs                   # regex redaction filter
│   └── util/                                   # atomic_write, permissions
├── tests/
│   ├── derivation.rs                           # V10 — SLIP-44 vector → T-address via anychain_kms
│   ├── address.rs                              # V4 — 0x41 universal + base58check round-trip
│   ├── protobuf.rs                             # V2 — TronTransaction encode round-trip + TriggerSmartContract.data
│   ├── trc20.rs                                # V3 — abi::encode_call round-trip + transfer selector
│   ├── rpc_nile.rs                             # V6 — eth_chainId via /jsonrpc + triggerconstantcontract
│   ├── resource.rs                             # V5 — DEM awareness + fee_limit in SUN + Stake 2.0 path
│   ├── spki_pin.rs                             # V7 — pinned:// URL + SpkiPinnedVerifier reuse
│   ├── sign_only.rs                            # V8 — r‖s‖v with v ∈ {0, 1} (NOT v+27) + dual-SHA256 txid regression
│   ├── tokens.rs                               # V9 — local+nile+mainnet.json + USDT decimals=6 verified
│   └── wallet_persistence.rs                   # argon2id + AES-GCM round-trip

rust-wallet-app/crates/tron/                    # CLI binary
├── Cargo.toml
└── src/
    ├── main.rs                                 # clap parser + subcommand dispatch
    ├── handlers/
    │   ├── wallet.rs                           # create / import / list / show / delete / rename / balance / send / send-speedup
    │   ├── address.rs                          # new / xpub
    │   ├── balance.rs                          # --address / --token
    │   ├── trc20.rs                            # send / approve / balance / allowance
    │   ├── tx.rs                               # get / wait
    │   ├── config.rs                           # show / set-rpc / set-network
    │   └── error.rs                            # classify exit code (mirrors btc/src/main.rs:151-169)
    └── cmd/                                    # one file per subcommand tree

rust-wallet-app/spikes/tron-v1/                 # verification harness (V1-V10)
├── Cargo.toml                                  # workspace member; deps = chosen surface
├── README.md                                   # V1-V10 acceptance criteria + PASS evidence template
├── RESULT.md                                   # V1-V10 PASS evidence log (filled after each test)
├── tokens/
│   ├── local.json                              # shared with crates/tron-wallet-core/tokens/
│   ├── nile.json
│   └── mainnet.json
└── tests/
    ├── v1_dep_wiring.rs                        # cargo build succeeds with pinned anychain-* + MSRV 1.98.1
    ├── v2_protobuf_roundtrip.rs                # TronTransaction encode/decode + TriggerSmartContract.data
    ├── v3_trc20_abi.rs                         # abi::encode_call → 68-byte calldata with 0xa9059cbb
    ├── v4_base58check.rs                       # 0x41 universal + canonical vector regression
    ├── v5_resource.rs                          # live wallet/triggerconstantcontract → 65k-130k Energy (GATED)
    ├── v6_nile.rs                              # POST /jsonrpc eth_chainId → 0xcd8690dc (GATED)
    ├── v7_spki_pin.rs                          # pinned://<correct_pin>@api.trongrid.io + SpkiPinnedVerifier
    ├── v7a_send_speedup.rs                     # rebroadcast idempotency per Round-1 grill Q10
    ├── v8_sign_only.rs                         # local-sign + dual-SHA256 txid regression + v ∈ {0,1}
    ├── v9_token_registry.rs                    # tokens/{local,nile,mainnet}.json + decimals() (GATED)
    ├── v10_slip44.rs                           # bip39 "abandon x11 about" → T-address matches TronWeb
    └── v11_mainnet_self_send.rs                # $0.001 USDT self-send (GATED, RUN_TRON_MAINNET=1)
```

---

## Risk Register

| # | Risk                                                                                  | Severity | Mitigation                                                                                  |
|---|---------------------------------------------------------------------------------------|----------|---------------------------------------------------------------------------------------------|
| 1 | anychain-tron bus-factor = 1 (loki-cmu)                                              | ACCEPTED | Exact-version pin `=0.2.14` + regression tests for known bugs (Phase 0 + Phase 1)            |
| 2 | anychain `TronTransaction::to_transaction_id()` returns single-SHA256                  | MEDIUM   | Caller-side `SHA256(SHA256(raw_bytes))` workaround in `tx/sign.rs`; pin exact version + add regression test |
| 3 | `secp256k1_sign(sk: &[u8])` does NOT Zeroize its sk param                              | MEDIUM   | Caller wraps `sk_bytes` in `Zeroizing<Vec<u8>>` before call; drop wrapper after sign        |
| 4 | MSRV bump 1.85 → 1.98.1 breaks workspace MSRV contract                                 | MEDIUM   | Pin `rust-toolchain.toml` to 1.98.1; if 1.94 check passes, advertise 1.94 — no fake MSRV    |
| 5 | `trx::build_contract` formats `type_url` via `{:?}` Debug derive                      | LOW      | Serialize `type_url` via `hex::encode` if needed; works today but fragile                   |
| 6 | `protobuf` wire format diverges from `serde_json` default                              | LOW      | Use `serde_json::to_value(&tx)` not `tx.to_string()` (Debug)                                |
| 7 | Mobile CI matrix has no Docker fallback                                                | MEDIUM   | Round-1 grill Q6: `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` FFI compile only; NO mobile runtime smoke in v0.1; add Nile-based runtime mobile smoke in v0.2 |
| 8 | user-stories.md diverges from deep-dive on 11 stories + Nile USDT address              | MEDIUM   | Phase 0 Task 0.5 — regenerate user-stories.md from this plan                                |
| 9 | send-speedup rebroadcast semantics not verified (Round-1 grill Q10)                    | MEDIUM   | Spike V7a: verify `wallet/broadcasttransaction` idempotency before row 7 ships              |
| 10 | Mainnet smoke not implemented                                                          | HIGH     | Q4 gate: $0.001 USDT self-send with pre-check audit hook + `RUN_TRON_MAINNET=1` env gate     |

---

## Phases

**Nine phases** — Phase Set Up, then Phase 0 through Phase 7. Each phase has a goal, tasks (bite-sized with checkboxes), files, verification gate, and PAUSE point.

### Phase Set Up — Branch, labels, milestone, CI

**Goal:** the `rust-tron-core` integration branch, its tracker vocabulary, and its CI gate all exist before any Rust code is written. Mirrors the `rust-eth-core` precedent (see `.github/workflows/rust-eth-core-ci.yml`) per L25. **Gate:** a no-op PR into `rust-tron-core` triggers `.github/workflows/rust-tron-core-ci.yml` and passes.

This phase is repo plumbing only — no crate code, no `cargo` changes. It exists because branch and tracker mistakes are expensive to unwind after work has landed: a task branched off `main` inherits none of the integration branch's history, and a PR opened against `main` bypasses the whole v0.1 review train.

#### Task S.1 — Cut the `rust-tron-core` integration branch from `main`

**Files:** none (git refs only)

- [ ] Confirm `main` is clean and up to date: `git status --short` empty, `git fetch origin && git rev-parse main origin/main` match.
- [ ] Create the branch from `main`: `git checkout main && git pull --ff-only && git checkout -b rust-tron-core`.
- [ ] Push and set upstream: `git push -u origin rust-tron-core`.
- [ ] Record the base commit SHA in the ledger entry (L17) so the eventual cut PR back to `main` has a known fork point.

**Verification:** `git rev-parse --abbrev-ref HEAD` returns `rust-tron-core`; `gh api repos/:owner/:repo/branches/rust-tron-core --jq .name` returns `rust-tron-core`.

**Note:** `origin/docs/tron-wallet-core-399` already exists and holds the planning docs. It is a docs branch, not the integration branch — do not reuse it, and do not branch `rust-tron-core` from it.

#### Task S.2 — Branch rule: every task branches from `rust-tron-core`, never `main`

**Files:** this plan (the rule below is the reference every later phase points at)

The rule, stated once so every later phase can cite it:

- **Branch from:** `rust-tron-core`. Never `main`, never another task branch.
- **PR into:** `rust-tron-core`. Never `main`.
- **Only exception:** the final v0.1 cut PR, `rust-tron-core` → `main`, opened once at the end of Phase 7 after the acceptance criteria pass.
- **Naming:** `tron/<phase>-<slug>`, e.g. `tron/phase1-address-keys`, `tron/phase3-trc20-abi`.

Per-task ritual:

```bash
git checkout rust-tron-core
git pull --ff-only origin rust-tron-core
git checkout -b tron/phase1-address-keys
# ... work, commit (PAUSE per never-auto-commit) ...
git push -u origin tron/phase1-address-keys
gh pr create --base rust-tron-core --body-file /tmp/pr-body.md   # --base is mandatory
```

- [ ] `gh pr create` always passes `--base rust-tron-core` explicitly — the repo default base is `main`, so omitting the flag silently targets the wrong branch.
- [ ] Before opening any PR, confirm the base: `gh pr view --json baseRefName --jq .baseRefName` must return `rust-tron-core`.
- [ ] If a PR is opened against `main` by mistake, retarget it rather than reopening: `gh pr edit <n> --base rust-tron-core`.

**Verification:** a scratch branch cut from `rust-tron-core` shows the integration branch in its history — `git merge-base --is-ancestor rust-tron-core HEAD` exits 0.

#### Task S.3 — Confirm and extend tracker labels

**Files:** none (tracker state)

Two labels already exist and are reused as-is — do not recreate them:

| Label            | Colour    | Existing meaning       | Use in v0.1                        |
| ---------------- | --------- | ---------------------- | ---------------------------------- |
| `rust-tron-core` | `#c41e3a` | TRON core crate work   | every library task (Phases 0-5, 7) |
| `rust-tron-cli`  | `#1f6feb` | tron CLI feature tasks | every CLI task (Phase 6)           |

- [ ] Verify both exist before filing issues: `gh label list --search rust-tron`.
- [ ] Reuse the existing priority scale — `priority/p0` … `priority/p3` are already defined repo-wide. Do NOT create a parallel `P0`/`P1` set (the stray `P2` label already in the tracker is a duplicate; leave it alone and do not extend the pattern).
- [ ] Reuse the existing `task`, `backlog`, `security`, and `documentation` labels.
- [ ] Create a phase label only if issues need grouping beyond the milestone: `gh label create tron/phase-setup --color c41e3a --description "TRON v0.1 Phase Set Up"` (optional; skip if the milestone alone is sufficient).

Priority assignment for v0.1 issues:

| Priority      | Applies to                                                                                       |
| ------------- | ------------------------------------------------------------------------------------------------ |
| `priority/p0` | Phase Set Up, plus the 5 critical-path modules (trongrid, persist, mnemonic_cipher, sign, config) |
| `priority/p1` | Phases 1-4 — address/keys/sign, tx, TRC-20, test scenario integration                            |
| `priority/p2` | Phases 5-6 — PAL traits, CLI scaffold                                                            |
| `priority/p3` | Phase 7 polish, plus anything deferred but still tracked                                          |

**Verification:** `gh label list --search rust-tron` shows both labels; `gh label list --search priority/` shows p0-p3.

#### Task S.4 — Create the `tron-v0.1` milestone

**Files:** none (tracker state)

The repo currently has no milestones, so this is the first one.

- [ ] Create it:

```bash
gh api repos/:owner/:repo/milestones -f title='tron-v0.1' \
  -f state='open' \
  -f description='tron-wallet-core v0.1 + tron CLI v0.1 — integration branch rust-tron-core. Closes with the cut PR to main.'
```

- [ ] Attach every v0.1 issue to it as issues are filed: `gh issue edit <n> --milestone tron-v0.1`.
- [ ] Attach the umbrella issue #399 to it.
- [ ] Do NOT set a due date — the v0.1 gate is the acceptance criteria, not a calendar date.

**Verification:** `gh api repos/:owner/:repo/milestones --jq '.[].title'` includes `tron-v0.1`.

#### Task S.5 — Add `.github/workflows/rust-tron-core-ci.yml`

**Files (new):** `.github/workflows/rust-tron-core-ci.yml`

Copy the structure of `.github/workflows/rust-eth-core-ci.yml` and retarget it. Same jobs, same action pins, same least-privilege token.

- [ ] `on.push.branches: [rust-tron-core]` and `on.pull_request.branches: [rust-tron-core]`, plus `workflow_dispatch: {}`. **Do not** add `main` to either list — the umbrella `ci.yml` covers main.
- [ ] `permissions: contents: read` only.
- [ ] `concurrency` group keyed on workflow + ref with `cancel-in-progress: true`.
- [ ] Jobs: `rust-fmt` (`cargo fmt --all -- --check`), `rust-clippy` (`cargo clippy -- -D warnings`), `rust-test` (`cargo test -p tron-wallet-core`), all with `working-directory: rust-wallet-app`.
- [ ] Add the mobile compile-only gate as its own job (per deep-dive "Mobile build gate (CI)"): `cargo check --target aarch64-apple-ios` and `cargo check --target aarch64-linux-android`.
- [ ] Pin the MSRV toolchain to `1.98.1` to match Task 0.1 rather than floating on `stable`.
- [ ] Action pins follow L37: tag-based to mirror the umbrella `ci.yml`, with resolved SHAs captured in a follow-up commit after the first green run.
- [ ] Header comment states the scope explicitly: "any PR that targets `rust-tron-core` (NOT main)".

**Verification:** open a trivial no-op PR into `rust-tron-core`; the `rust-tron-core` workflow appears in checks and every job passes. `gh run list --workflow rust-tron-core-ci.yml --limit 1` shows a `success` conclusion.

#### Task S.6 — Optional branch protection on `rust-tron-core`

**Files:** none (repo settings)

- [ ] If the repo plan allows branch protection, require the `rust-tron-core` CI checks to pass before merge, and require at least one review.
- [ ] If protection is unavailable, record that here and rely on the L13 PAUSE points instead — this is a soft gate, not a blocker for Phase 0.

**Verification:** either protection is configured, or the fallback is written into the ledger entry.

#### Phase Set Up — Verification

- [ ] `git rev-parse --abbrev-ref HEAD` = `rust-tron-core`, and the branch exists on `origin`.
- [ ] `gh label list --search rust-tron` shows `rust-tron-core` + `rust-tron-cli`.
- [ ] `gh api repos/:owner/:repo/milestones --jq '.[].title'` includes `tron-v0.1`.
- [ ] `.github/workflows/rust-tron-core-ci.yml` exists and its first run concluded `success`.
- [ ] Issue #399 carries the `tron-v0.1` milestone and a priority label.
- [ ] The branch rule from Task S.2 is restated in the body of every v0.1 task issue, so an agent picking up a task cannot miss it.

**PAUSE here.** Branch creation, label edits, milestone creation, and the workflow commit are all state-modifying — per the workflow-approval-required rule, discuss before executing, and per never-auto-commit, the workflow file is committed only after approval.

---

### Phase 0 — Workspace setup + dependency pinning + MSRV bump

**Goal:** Cargo workspace admits the anychain-* deps at exact crates.io pins + MSRV 1.98.1. **CI gate:** `cargo build -p tron-wallet-core` succeeds in clean checkout.

**Tasks:**

#### Task 0.1 — Update `rust-wallet-app/rust-toolchain.toml`

**Files:** `rust-wallet-app/rust-toolchain.toml`

- [x] Set `[toolchain] channel = "1.98.1"`.
- [x] Document the bump in commit message body: "MSRV bump 1.85 → 1.98.1 for anychain workspace compatibility".

**Verification:** `rustc --version` returns `1.98.1` in `rust-wallet-app/`.

**Subagent prompt:** `mattpocock-skills:codebase-design` for the rust-toolchain.toml change rationale.

#### Task 0.2 — Add anychain-* to workspace (crates.io, exact pin)

**Files:** `rust-wallet-app/Cargo.toml` (workspace root)

- [x] Add `anychain-core = "=0.1.8"` to `[workspace.dependencies]` (exact pin, NOT `^`).
- [x] Add `anychain-tron = "=0.2.14"` to `[workspace.dependencies]` (exact pin, NOT `^`).
- [x] Add `anychain-kms = "=0.1.23"` to `[workspace.dependencies]` (exact pin, NOT `^`).
- [x] Document `=X.Y.Z` syntax rationale in commit message: "exact pin mandatory per Q3 — auto-bump would silently change txid / signing / Zeroizing behavior".

**Verification:** `cargo build` succeeds at workspace root. `cargo tree -p anychain-tron` shows resolved version = `0.2.14` exact (no caret).

#### Task 0.3 — Create `tron-wallet-core` crate skeleton

**Files (new):**
- `rust-wallet-app/crates/tron-wallet-core/Cargo.toml`
- `rust-wallet-app/crates/tron-wallet-core/src/lib.rs` (empty re-export)

- [x] Add `crates/tron-wallet-core` to workspace `members`.
- [x] Add crate skeleton with `package.edition = "2021"`, `package.version = "0.1.0"`, `package.license = "MIT"` (matches the workspace and every sibling crate; the dual `MIT OR Apache-2.0` originally written here is *anychain's* license, which does not propagate to ours), `publish = false`.
- [x] Add crate-type `["rlib", "cdylib"]` for FFI support.
- [x] Add dependencies matching deep-dive §"Crates used in `tron-wallet-core` (V0.1)".
- [x] `src/lib.rs` is `// placeholder — Phase 1 wires modules`.
- [x] Canonical test: `tests/placeholder.rs` with `#[test] fn it_compiles() { assert!(true); }`.

**Verification:** `cargo build -p tron-wallet-core` succeeds. `cargo test -p tron-wallet-core` passes placeholder.

#### Task 0.4 — Regenerate `user-stories.md` from this plan

**Files:** `docs/wallets/2026-08-27-tron-wallet-user-stories.md`

**This task is mandatory** — the legacy user-stories diverges from deep-dive on 7 critical mismatches (crate choice, MSRV, Nile USDT address, ABI encoder origin, protobuf layer, story scope, CLI command layout, broken "Companion to" link). See verification report in session log 2026-09-05.

- [x] Update "Companion to" link: `2026-08-27-tron-rust-sdks-deep-dive.md` → `2026-08-27-tron-anychain-sdks-deep-dive.md`.
- [x] Update Story → crate map: replace raw primitives with anychain-* (preserves story IDs).
- [x] Mark **Stories 13 (batch), 14 (drain), 15 (ref-block), 16 (manual exp)** as **REMOVED from V0.1** (per deep-dive "redesign dropped").
- [x] Mark **Stories 8, 18, 23, 24** as **deferred to V0.2**.
- [x] Fix **Nile USDT address** in Story 21: `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` → `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (canonical per TronScan).
- [x] Update Story 23 token registry path: `tokens/{mainnet,nile}.json` → `tokens/{local,nile,mainnet}.json`.
- [x] Update CLI command layout (wallet/address/balance/trc20/tx/config top-level) per V0.1 feature map.
- [x] Update Story 12 cross-cutting note: remove "v0.2+ per #399 B3" — encryption ships V0.1.

**Verification:** file matches deep-dive V0.1 feature map exactly.

**Subagent prompt:** `mattpocock-skills:domain-modeling` for user-stories regeneration.

#### Task 0.5 — Add `dev-dependencies` for spike + test

**Files:** `rust-wallet-app/crates/tron-wallet-core/Cargo.toml`

- [x] Add `[dev-dependencies] proptest = { workspace = true }`.
- [x] Add `tempfile = { workspace = true }`.
- [x] Add `testcontainers = { version = "0.23", optional = true }` under `[dependencies]`, **not** `[dev-dependencies]` — Cargo rejects the latter with `dev-dependencies are not allowed to be optional: testcontainers` (verified 2026-09-05). Optional + feature-gated keeps it uncompiled by default, which preserves the lean-CI intent.
- [x] Add feature flag `desktop-tests = ["dep:testcontainers"]` for gating.

**Verification:** `cargo test -p tron-wallet-core` passes placeholder.

#### Task 0.6 — Update `CONTEXT.md` with anychain terminology

**Files:** `docs/wallets/CONTEXT.md`

- [x] Add "anychain" entry to vocabulary: umbrella crate family, MIT OR Apache-2.0, pulled direct from crates.io at exact pins (vendoring rejected 2026-09-05).
- [x] Add "anychain-tron" entry: wire-format + 17 contract builders (TRX/TRC-20/Stake 2.0/Vote).
- [x] Add "anychain-kms" entry: BIP-39 + BIP-32 HD + secp256k1 sign + Zeroizing xprv.
- [x] Add "anychain-core" entry: shared traits + crypto utilities.

**Verification:** `CONTEXT.md` covers anychain vocabulary without contradicting deep-dive.

**Subagent prompt:** `mattpocock-skills:domain-modeling`.

#### Task 0 — Verification

- [x] `cargo build` succeeds at workspace root.
- [x] `cargo build -p tron-wallet-core` succeeds.
- [x] `cargo test -p tron-wallet-core` passes placeholder.
- [x] `cargo tree -p tron-wallet-core | grep anychain` shows `anychain-core v0.1.8`, `anychain-kms v0.1.23`, `anychain-tron v0.2.14` — exact versions, no caret drift.
- [x] `rustc --version` = 1.98.1.

**PAUSE here. Verify no other workspace crate broke under MSRV 1.98.1 bump before proceeding.** Per L13 step 11.

---

### Phase 1 — Foundation: address + keys + signing

**Goal:** `tron-wallet-core` derives T-base58check addresses from BIP-39 mnemonics + signs arbitrary 32-byte prehashes with `v ∈ {0, 1}`. **CI gate:** Spike V10 (SLIP-44 vector) + V4 (base58check) PASS.

#### Task 1.1 — Wrap `anychain_kms::Mnemonic`

**Files:** `rust-wallet-app/crates/tron-wallet-core/src/keys/mod.rs`, `src/keys/mnemonic.rs`

- [x] `keys::Mnemonic::generate(word_count: MnemonicType, language: Language) -> Self` wraps `anychain_kms::bip39::Mnemonic::new`. **Deviation:** the plan sketched `new(words: u8, ...) -> Result<Self>`. `MnemonicType` is an enum, so there is no invalid word count to reject — returning `Result` would be a failure case that never fires. Infallible, and named `generate` so it does not read as a constructor over an existing phrase.
- [x] `keys::Mnemonic::from_phrase(phrase: &str, language: Language) -> Result<Self>` wraps `anychain_kms::bip39::Mnemonic::from_phrase`.
- [x] `keys::Mnemonic::to_seed(&self, passphrase: &str) -> Zeroizing<[u8; 64]>` wraps `bip39::Seed::new` + copies into `Zeroizing<[u8; 64]>`. **Correction to the module path:** the plan cited `anychain_kms::seed_from_mnemonic`, which does not exist; the real API is `bip39::Seed::new(&mnemonic, password)`.
- [x] Test: `Mnemonic::from_phrase("abandon ".repeat(11) + "about", English).is_ok()`, plus checksum rejection, out-of-wordlist rejection, and the BIP-39 reference seed vector for passphrase `"TREZOR"`.
- [x] **Finding that narrows Risk Register #3:** `anychain_kms::bip39::Mnemonic` already stores `phrase: Zeroizing<String>` and `entropy: Zeroizing<Vec<u8>>`, and `bip39::Seed` has an explicit zeroizing `Drop`. The wrapper does not need to re-add mnemonic hygiene. The real gap is narrower than the plan assumed: `ExtendedPrivateKey` holds a `libsecp256k1::SecretKey` with no zeroizing `Drop`, and `secp256k1_sign` takes a `&[u8]` it never clears.
- [x] `Debug` is implemented by hand to redact the phrase.

#### Task 1.2 — Wrap `anychain_kms::ExtendedPrivateKey`

**Files:** `src/keys/derivation.rs`

- [x] `keys::derive_keypair(mnemonic: &Mnemonic, passphrase: &str, path: &DerivationPath) -> Result<KeyPair>` chains `to_seed → XprvSecp256k1::new_from_path → private_key`. **Deviation:** `passphrase` is not in the plan's signature. It is part of the BIP-39 seed, so leaving it out would force a second derivation entry point once passphrase wallets are supported — two key-derivation paths through one crate is how key-handling bugs start. **Correction to the API name:** `ExtendedPrivateKey::from_seed` does not exist; the real constructors are `XprvSecp256k1::new` / `new_from_path`.
- [x] `keys::KeyPair { secret: Zeroizing<[u8; 32]>, public: TronPublicKey }` — fields private, read through `secret_bytes()` and `public_key()`.
- [x] Wrap `sk_bytes` in `Zeroizing<[u8; 32]>` before any `secp256k1_sign` call (closes anychain GAP).
- [x] **Zeroize hazard found via clippy:** `needless_borrows_for_generic_args` fires on `new_from_path(&*seed, …)` and suggests `*seed`. Taking that suggestion would deref the `Zeroizing<[u8; 64]>` to a `Copy` array and pass it **by value**, leaving an unwiped copy of the seed behind. Resolved with `seed.as_slice()`, which satisfies the lint without duplicating secret material. Same fix in `xpub.rs`. Commented in both files so the reasoning survives.
- [x] Test: `derive_keypair(m, "", "m/44'/195'/0'/0/0".parse().unwrap()).is_ok()`, plus determinism, sibling-index divergence, passphrase sensitivity, and a `Debug`-redaction check.

#### Task 1.3 — Wrap `anychain_tron::TronAddress`

**Files:** `src/address/mod.rs`

- [x] `address::Address::from_public_key(pk: &TronPublicKey) -> Result<Self>` wraps `TronAddress::from_public_key`. **Deviation:** returns `Result`, not `Self` — the upstream call is fallible and takes a `&TronFormat` second argument, which this wrapper supplies as `TronFormat::Standard`.
- [x] `address::Address::to_base58(&self) -> String`.
- [x] `address::Address::to_hex(&self) -> String`.
- [x] `address::Address::from_str(s: &str) -> Result<Self>` via `FromStr`; accepts base58check, bare hex, and `0x`-prefixed hex.
- [x] `address::Address::is_valid(candidate: &str) -> bool`. **Deviation:** associated function, not the planned `&self` method. On a constructed `Address` the answer is always `true`, so a method would be a check that can never fail; callers need to screen untrusted input before constructing one.
- [x] Test: round-trip `to_base58 → from_str`, plus rejection of truncated, over-length, non-alphabet, mutated-checksum, and wrong-version-byte input.

#### Task 1.4 — SLIP-44 canonical vector test (Spike V10)

**Files:** `tests/v10_slip44.rs`

- [x] **Correction to the plan's premise:** neither this plan nor the deep-dive records a published TRON address for the canonical mnemonic, so "match a SLIP-44 reference" had no reference to match. Pinning a value this crate produced would have made the test self-confirming. Two outside anchors are used instead.
- [x] **Anchor 1 (independent implementation):** `spikes/tron-v1` derives the same mnemonic and path with a separate stack — `bip39` + `bip32` + `k256` + hand-rolled base58check, sharing no code with anychain — and produces `TUEZSdKsoDHQMeZwihtdoBiN46zxhGWYdH`. The test asserts anychain lands on the same address. This is the deep-dive's "hand-rolled cross-check".
- [x] **Anchor 2 (repo-wide vector):** the same mnemonic at `m/44'/60'/0'/0/0` must hash to `0x9858EfFD232B4033E47d90003D41EC34EcaEda94`, which `evm-wallet-core`, `polygon-wallet-core`, `spikes/alloy-v1`, and `spikes/polygon-v1` all already assert. TRON and Ethereum hash accounts identically, so this exercises the whole BIP-39 → BIP-32 → pubkey → keccak chain.
- [x] Address must start with `T` using prefix `0x41`; 34 characters; base58 round-trip.
- [x] Coin 195 and coin 60 must derive distinct accounts (guards a derivation that ignores the coin index).
- [x] Passphrase must change the derived account.
- [x] Test fails if anychain-kms derivation drifts.

**Verification:** `cargo test -p tron-wallet-core --test v10_slip44` passes — 6 tests.

**Test Scenario mapping:** supports deep-dive §"Test Scenario" infrastructure (foundational — SLIP-44 vector required by every row 1-8 wallet derivation). Used by Phase 7 V10 PASS gate.

#### Task 1.5 — Sign-only path (Spike V8)

**Files:** `src/tx/sign.rs`, `tests/v8_sign_only.rs`

- [x] `tx::sign::sign_hash(secret: &Zeroizing<[u8; 32]>, msg32: &[u8; 32]) -> Result<Signed>` wraps `anychain_kms::secp256k1_sign`.
- [x] **ZEROIZE wrap the secret param** (closes anychain GAP — `secp256k1_sign` does NOT Zeroize its sk param).
- [x] Reject any recovery id outside `0..=1`. **Deviation:** the plan asked for a compile-fail test for `v ∈ {27, 28}`. `v` is a runtime `u8` returned by libsecp256k1, so the constraint is not expressible as a compile failure; it is enforced by a validating `RecoveryId` newtype whose only constructor rejects anything outside `0..=1`, and covered by a test that signs six different digests. Note libsecp256k1 can also return 2 or 3 on r-overflow, which TRON likewise rejects — the check is `v > 1`, not just `v != 27 && v != 28`.
- [x] **Dual-SHA256 txid workaround:** `tx::sign::txid(raw_bytes: &[u8]) -> [u8; 32]` computes `Sha256::digest(&Sha256::digest(raw_bytes))`.
- [x] **Regression test:** `txid(b"")` must equal `5df6e0e2…4c9456`, confirmed independently via `printf '' | sha256sum | cut -d' ' -f1 | xxd -r -p | sha256sum`. A future anychain "fix" to single-SHA256 breaks this.
- [x] Test: `sign_hash` returns `v ∈ {0, 1}`, is deterministic (RFC 6979), diverges per message, and rejects an out-of-range secret.

**Verification:** `cargo test -p tron-wallet-core --test v8_sign_only` passes.

**Test Scenario mapping:** supports **Local row 1 (TRX native transfer)** + **row 7 (send-speedup/RBF)** + **row 8 (wallet-to-wallet TRC-20)** — all require `sign_hash` + dual-SHA256 txid. Used by Phase 7 V8 PASS gate.

#### Task 1.6 — Xpub export (Story 19)

**Files:** `src/keys/xpub.rs`, `tests/address.rs`

- [x] `keys::xpub(mnemonic: &Mnemonic, passphrase: &str, path: &DerivationPath) -> Result<String>` returns `xprv.public_key().to_string(Prefix::XPUB)`. **Deviation:** returns `Result` (derivation is fallible) and takes `passphrase`, matching `derive_keypair`.
- [x] Test: export starts with `xpub` (SLIP-0132, same as Bitcoin), is deterministic, diverges per account and per passphrase, and never emits an `xprv`.

**Test Scenario mapping:** supports **Local row 8 (wallet-to-wallet TRC-20)** + **Nile row 1 + 2** — `--to-wallet <name|id>` resolution requires xpub export path. Also supports **Nile row 3 (Mobile-specific)** — FFI smoke derives xpub for hardware-wallet companion.

#### Phase 1 Verification

- [x] `cargo test -p tron-wallet-core --tests` passes (V10 + V8 + address + lib units).
- [x] `cargo clippy -p tron-wallet-core --all-targets -- -D warnings` passes.
- [x] `cargo fmt --all -- --check` passes.

**Naming note:** the plan's §File Structure lists these as `tests/derivation.rs` and `tests/sign_only.rs`, while Task 1.4/1.5 above name them `tests/v10_slip44.rs` and `tests/v8_sign_only.rs`. The task-level names win, since the stated verification commands (`--test v10_slip44`, `--test v8_sign_only`) cite them. §File Structure is stale on this point.

**PAUSE. Verify L13 step 11 (claims gate). Proceed to Phase 2 only after sign-off.**

---

### Phase 2 — Transaction: builder + sign + broadcast

**Goal:** `tron-wallet-core` builds `TronTransaction` for TRX native transfer + broadcasts via `wallet/broadcasttransaction`. **CI gate:** Spike V2 (protobuf round-trip) + V7 (SPKI pin) PASS.

#### Task 2.1 — Wrap `anychain_tron::trx::build_transfer_contract`

**Files:** `src/tx/builder.rs`

- [ ] `tx::builder::trx_transfer(owner: Address, recipient: Address, amount_sun: u64) -> TronTransactionParameters` wraps `anychain_tron::trx::build_transfer_contract`.
- [ ] `tx::builder::set_ref_block(params: &mut TronTransactionParameters, block: BlockHeader)`.
- [ ] `tx::builder::set_fee_limit(params: &mut TronTransactionParameters, fee_limit_sun: i64)`.
- [ ] `tx::builder::set_timestamp(params: &mut TronTransactionParameters, ts_ms: i64)`.
- [ ] `tx::builder::set_expiration(params: &mut TronTransactionParameters, exp_ms: i64)`.

#### Task 2.2 — Wrap `anychain_tron::TronTransaction::sign`

**Files:** `src/tx/sign.rs` (extend from Task 1.5)

- [ ] `tx::sign::sign_tx(sk_z: &Zeroizing<[u8; 32]>, params: TronTransactionParameters) -> SignedTransaction`.
- [ ] Internally: `params.to_bytes()` → `anychain_core::sha256(&raw_bytes)` → `secp256k1_sign(&sk_z, msg32)` → `(sig, recid)` → `TronTransaction::sign(sig, recid)`.
- [ ] Return `SignedTransaction { txid, raw_bytes, signature }` where `txid = SHA256(SHA256(raw_bytes))`.

#### Task 2.3 — `wallet/broadcasttransaction` RPC call

**Files:** `src/tx/broadcast.rs`, `src/chain/mod.rs`

- [ ] `chain::TronGridClient::new(rpc_url: &str, spki_pin: Option<&[u8; 32]>) -> Result<Self>`.
- [ ] `chain::TronGridClient::broadcast(&self, tx: &SignedTransaction) -> Result<BroadcastReceipt>`.
- [ ] Internally: `serde_json::to_value(&tx)` → POST `{rpc_url}/wallet/broadcasttransaction` with `{"raw_data_hex": ..., "signature_hex": ...}` body.
- [ ] Reuse `SpkiPinnedVerifier` from `bitcoin-wallet-core::chain::spki` when `spki_pin` is `Some`.

#### Task 2.4 — `walletsolidity/getnowblock` for TAPOS

**Files:** `src/chain/mod.rs`

- [ ] `chain::TronGridClient::get_now_block(&self) -> Result<BlockHeader>` queries `walletsolidity/getnowblock` (fullnode, NOT `wallet/getnowblock` which uses SolidityNode).
- [ ] Returns `BlockHeader { ref_block_bytes: [u8; 2], ref_block_hash: [u8; 8], block_number: u64, block_id: [u8; 32] }`.
- [ ] **Note:** TAPOS reference per deep-dive Q7 uses `walletsolidity/getnowblock` (not `wallet/getnowblock`) for finality.

#### Task 2.5 — `wallet/gettransactioninfobyid` for receipt

**Files:** `src/chain/mod.rs`

- [ ] `chain::TronGridClient::get_tx_info(&self, txid: &str) -> Result<TransactionInfo>`.
- [ ] Returns `TransactionInfo { id, blockNumber, contractResult, fee }`.

#### Task 2.6 — Protobuf round-trip test (Spike V2)

**Files:** `tests/v2_protobuf_roundtrip.rs`

- [ ] `TronTransaction::encode_to_vec(&raw_data)` round-trips byte-equal via decode.
- [ ] `TriggerSmartContract.data` field at proto field **4** (NOT 3) — confirmed via anychain-tron's vendored proto.

**Verification:** `cargo test -p tron-wallet-core --test v2_protobuf_roundtrip` passes.

**Test Scenario mapping:** supports **Local rows 1-8** + **Nile rows 1-2** — every scenario row builds `TronTransaction` envelope. `TriggerSmartContract.data` at field 4 = required for **Local row 2 (TRC-20 transfer)**, **row 3 (first-time receive)**, **row 4 (TRC-20 approval)**.

#### Task 2.7 — SPKI pin live extraction (Round-1 grill Q5)

**Files:** `src/config.rs`

- [ ] Add `TronConfig::mainnet_default_spki_pin() -> [u8; 32]` returning hex-decoded `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d`.
- [ ] `TronConfig::for_network(Network::Mainnet)` returns `TronConfig { spki_pin: Some(mainnet_default_spki_pin()), .. }`.
- [ ] `TronConfig::for_network(Network::Nile)` returns `TronConfig { spki_pin: Some(nile_default_spki_pin()), .. }` (extract from `nile.trongrid.io` cert during Phase 2 spike V7).

**Test Scenario mapping:** SPKI pin config supports **Local rows 1-8** + **Nile rows 1-2** — every RPC call (TronBox local + TronGrid remote) requires either pinned endpoint (Scenario A) or system CAs (Scenario B). Live cert extraction `0e43f611...` per Round-1 grill Q5.

#### Task 2.8 — SPKI pin integration test (Spike V7)

**Files:** `tests/v7_spki_pin.rs`

- [ ] `spki_pinned_endpoint_accepts_correct_pin`: connect to `api.trongrid.io` with correct pin, JSON-RPC call succeeds.
- [ ] `spki_pinned_endpoint_rejects_wrong_pin`: connect to `api.trongrid.io` with wrong pin, returns `Error::SpkiPinMismatch`.
- [ ] `no_pin_localhost_tronbox_succeeds`: connect to `http://127.0.0.1:8090` (TronBox) with no pin, JSON-RPC call succeeds.

**Verification:** `cargo test -p tron-wallet-core --test v7_spki_pin` passes.

**Test Scenario mapping:** SPKI pin integration supports **Local rows 1-8** (TronBox localhost = Scenario B no-pin path) + **Nile rows 1-2** (TronGrid Nile HTTPS = Scenario A pinned-path). **Nile row 4 (network failure recovery)** validates `no-pin + closed port → exit code 3 within 30s timeout` — covered by this spike.

#### Phase 2 Verification

- [ ] `cargo test -p tron-wallet-core --tests` passes (V2 + V7 + broadcast + get_tx_info).
- [ ] `cargo clippy -p tron-wallet-core -- -D warnings` passes.
- [ ] Send 1 TRX from test wallet to recipient via `TronGridClient::broadcast` (Nile, `RUN_TRON_NILE=1`).

**PAUSE. Verify L13 step 11.**

---

### Phase 3 — TRC-20 + ABI + token registry

**Goal:** `tron-wallet-core` sends USDT-TRC20 via `TriggerSmartContract` + reads balances via `wallet/triggerconstantcontract` + bundled token registry. **CI gate:** Spike V3 (TRC-20 ABI) + V9 (token registry) + V5 (resource model) PASS.

#### Task 3.1 — Wrap `anychain_tron::trx::build_trc20_transfer_contract`

**Files:** `src/tx/builder.rs` (extend)

- [ ] `tx::builder::trc20_transfer(owner: Address, contract: Address, recipient: Address, amount: U256) -> TronTransactionParameters` wraps `anychain_tron::trx::build_trc20_transfer_contract + abi::trc20_transfer`.
- [ ] Default `fee_limit = 130_000_000` sun (130 TRX energy allowance per Spike V5).

#### Task 3.2 — Wrap `anychain_tron::trx::build_trc20_approve_contract`

**Files:** `src/tx/builder.rs` (extend)

- [ ] `tx::builder::trc20_approve(owner: Address, contract: Address, spender: Address, value: U256) -> TronTransactionParameters` wraps `anychain_tron::trx::build_trc20_approve_contract + abi::trc20_approve`.

#### Task 3.3 — `wallet/triggerconstantcontract` for view calls

**Files:** `src/chain/mod.rs` (extend)

- [ ] `chain::TronGridClient::trigger_constant_contract(&self, contract: Address, selector: [u8; 4], args: &[u8]) -> Result<Vec<u8>>`.
- [ ] POST `{rpc_url}/wallet/triggerconstantcontract` with `{"contract_address", "function_selector", "parameter": hex(args), "visible": true}` body.
- [ ] **Wire-format contract (corrected 2026-08-27 via #410):** the server **prepends the 4-byte selector** to `parameter`; client sends **encoded args only** (32 bytes per Solidity uint256/address).

#### Task 3.4 — `balanceOf` + `decimals` + `symbol` ABI decoding

**Files:** `src/trc20.rs`

- [ ] `trc20::balance_of(rpc: &TronGridClient, contract: Address, owner: Address) -> Result<U256>`.
  - Selector `0x70a08231` + arg `padded_to_32(owner_20)`.
  - Decode 32-byte response as `uint256`.
- [ ] `trc20::decimals(rpc: &TronGridClient, contract: Address) -> Result<u8>`.
  - Selector `0x313ce567`. Decode response as `uint8`.
- [ ] `trc20::symbol(rpc: &TronGridClient, contract: Address) -> Result<String>`.
  - Selector `0x95d89b41`. Decode response as ABI string (offset + length + bytes).

#### Task 3.5 — Bundled token registry (Spike V9)

**Files:** `src/tokens/mod.rs`, `tokens/local.json`, `tokens/nile.json`, `tokens/mainnet.json`

- [ ] `tokens::load(network: Network) -> &[Token]` reads bundled JSON via `include_str!`.
- [ ] **local.json:** TronBox Docker mock USDT (mock contract address from `testcontainers`).
- [ ] **nile.json:** 1 entry — community test USDT `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (6 decimals).
  - **CAUTION:** user-stories.md Story 21 quotes `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z` — WRONG. Use `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` per deep-dive canonical.
- [ ] **mainnet.json:** 5 entries — USDT `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` (6), USDC `TEkxiTehnzSmSe2XqrBj4w32RUN966rdz8` (6), TUSD `TUpMhErZL2fhh4sVNULAbNKLokS4GjC1F9` (18), USDD `TXDk8mbtRbXeYuMNS83CfKPaYYT8Xvi9Hz` (18), stUSDT `TThzxNRLrW2Brp9DcTQU8i4Wd9udCWEdZ3` (6).

#### Task 3.6 — TRC-20 ABI round-trip test (Spike V3)

**Files:** `tests/v3_trc20_abi.rs`

- [ ] `anychain_tron::abi::contract_function_call("transfer", &[Param])` produces 68-byte calldata with `0xa9059cbb` selector at bytes [0..4].
- [ ] `balanceOf` produces 36-byte calldata with `0x70a08231` selector at bytes [0..4].
- [ ] `approve` produces 68-byte calldata with `0x095ea7b3` selector at bytes [0..4].
- [ ] `decimals` produces 4-byte calldata with `0x313ce567`.

**Verification:** `cargo test -p tron-wallet-core --test v3_trc20_abi` passes.

**Test Scenario mapping:** supports **Local row 2 (TRC-20 transfer held recipient)** + **row 3 (first-time receive empty recipient)** + **row 4 (TRC-20 approval)** + **row 6 (insufficient balance)** + **row 8 (wallet-to-wallet TRC-20)** + **Nile row 2 (real test USDT)** — every TRC-20 scenario row requires `transfer(0xa9059cbb)`, `approve(0x095ea7b3)`, `balanceOf(0x70a08231)`, `decimals(0x313ce567)` ABI encoding round-trips.

#### Task 3.7 — Token registry live verification (Spike V9)

**Files:** `tests/v9_token_registry.rs`

- [ ] `tokens::load(Network::Nile)` returns 1 entry.
- [ ] `tokens::load(Network::Mainnet)` returns 5 entries.
- [ ] Live `trc20::decimals(rpc, USDT)` against Nile → `6` (GATED, `RUN_TRON_NILE=1`).
- [ ] Live `trc20::symbol(rpc, USDT)` against Mainnet → `"USDT"` (GATED, `RUN_TRON_MAINNET=1`).

**Test Scenario mapping:** supports **Local row 2 (TRC-20 transfer)** + **row 3 (first-time receive)** + **row 4 (TRC-20 approval)** + **row 6 (insufficient balance)** + **Nile row 2 (real test USDT)** — every TRC-20 scenario row requires `tokens::{local,nile,mainnet}.json` lookup → `decimals()` verification. **Mainnet gate**: `RUN_TRON_MAINNET=1` validates `USDT` symbol on real mainnet contract `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`.

#### Task 3.8 — Resource model UX (Spike V5)

**Files:** `src/resource.rs`, `tests/v5_resource.rs`

- [ ] `resource::estimate_energy(rpc, contract, selector, args) -> Result<EnergyEstimate>` queries `wallet/triggerconstantcontract`, returns `energy_used` + optional `energy_penalty`.
- [ ] Fallback: `wallet/estimateenergy` (requires `vm.estimateEnergy` enabled).
- [ ] Apply DEM `max_factor = 3.4×` per 6-hour cycle — `getcontractinfo` returns `energy_factor` for any contract.
- [ ] USDT-TRC20 baseline: 65,000 Energy (recipient holds USDT) up to 130,000 Energy (empty recipient).
- [ ] Default `fee_limit = 100_000_000` sun (100 TRX) sized with `max_factor` buffer.
- [ ] Test (GATED, `RUN_TRON_NILE=1`): `estimate_energy` for MockTRC20 transfer returns 65k-130k.

**Test Scenario mapping:** supports **Local row 2 (TRC-20 held recipient 65k Energy)** + **row 3 (first-time receive empty recipient 130k Energy)** + **row 4 (TRC-20 approval energy estimate)** — every TRC-20 scenario row requires `fee_limit` sizing from `wallet/triggerconstantcontract` energy_used + DEM `max_factor=3.4×` buffer. **Nile row 1 + 2**: live `getcontractinfo.energy_factor` round-trip validates DEM scaling on real network.

#### Phase 3 Verification

- [ ] `cargo test -p tron-wallet-core --tests` passes (V3 + V5 + V9).
- [ ] `cargo clippy -p tron-wallet-core -- -D warnings` passes.
- [ ] Send 1 USDT-TRC20 from test wallet to recipient via `TronGridClient::broadcast` (Nile, `RUN_TRON_NILE=1`).

**PAUSE. Verify L13 step 11.**

---

### Phase 4 — Test Scenario integration (TronBox Docker + Nile)

**Goal:** Provide integration test infrastructure covering local (TronBox Docker via testcontainers, desktop-only) + Nile testnet (real network, fallback for mobile + manual QA). Both surface in CI + spike V11 mainnet gate uses same harness. **CI gate:** `cargo test --test trc20_local` PASS in CI (Docker runner); `TRON_NILE_INTEGRATION=1 cargo test --test trc20_nile` PASS on manual trigger.

**Two testnet targets covered.** **Local testnet (TronBox Docker, desktop-only)** is the default (CI + desktop dev). **Nile testnet (remote)** is fallback for mobile users + manual pre-release QA.

#### Task 4.1 — Local testnet: testcontainers TronBox spawn

**Files:** `spikes/tron-v1/tests/trc20_local.rs`

- [ ] Add `testcontainers = { version = "0.23" }` to `[dev-dependencies]` of `tron-wallet-core/Cargo.toml`.
- [ ] Add `testcontainers-modules = { version = "0.x", features = ["tronbox"] }` for TronBox preset.
- [ ] Write integration test `trc20_transfer_full_flow_local`:
  1. Spawn TronBox Docker via `Cli::default().run(TronBox::default())`.
  2. Get host port via `container.get_host_port_ipv4(8090)`.
  3. Deploy `MockTRC20` via `tx::deploy_trc20(&deployer_sk, DeployTrc20Params { ... }, &TronConfig::for_local_tronbox(&http_url))`.
  4. Submit TRC-20 transfer 100 mock USDT to recipient.
  5. Verify recipient balance via `chain::trc20_balance(recipient_addr, mock.contract_address, &cfg)`.

#### Task 4.2 — Local testnet test scenarios (rows 1-7a)

**Files:** `spikes/tron-v1/tests/trc20_local.rs` (extend)

| # | Scenario | Command | Pass criteria |
|---|----------|---------|---------------|
| 1 | TRX native transfer | `tron send --mnemonic "$DEPLOYER" --to "$RECIPIENT" --amount 1000000 --unit sun` | tx accepted; `receipt.energy_usage < 0` (bandwidth only); balances reconcile |
| 2 | TRC-20 transfer (held recipient) | `tron trc20 send --mnemonic "$DEPLOYER" --contract "$MOCK_USDT" --to "$RECIPIENT" --amount 100` | tx accepted; `energy_usage ≈ 65_000`; balance = 100 mock USDT |
| 3 | TRC-20 first-time receive (empty recipient) | `tron trc20 send ... --to "$FRESH_ADDR" --amount 50 --fee-limit 130000000` | tx accepted; `energy_usage ≈ 130_000` (2x baseline) |
| 4 | TRC-20 approval + allowance | `tron trc20 approve ... --spender "$DEX" --amount 1000` + `tron trc20 allowance --owner ... --spender ...` | approval tx accepted; allowance view returns 1000 mock USDT |
| 5 | Stake 2.0 freeze/unfreeze | `tron stake freeze --amount 1000000000 --unit sun` (V0.1.5) | tx accepted; resource query shows frozen balance |
| 6 | Insufficient balance | `tron trc20 send ... --amount 999999999` | tx REVERTED with explicit error (exit code 5) |
| 7 | Send-speedup (RBF) | `tron wallet send-speedup --wallet-id ... --txid <stuck> --fee-limit 200000000` | new tx accepted with higher `fee_limit`; original tx shows superseded |
| 7a | **Send-speedup rebroadcast semantics** (Round-1 grill Q10) | Verify `wallet/broadcasttransaction` idempotency: rebroadcast identical `(raw_bytes)` after 60s window — accepted/ignored/error? If accepted, speedup = rebroadcast + new fee_limit via new timestamp. If rejected, document "speedup not possible after window", remove `send-speedup` from v0.1. Block on this before row 7 ships. | node behavior recorded in `spikes/tron-v1/V7-speedup.md` |
| 8 | Wallet-to-wallet TRC-20 | `tron wallet send --wallet-id "$HOT" --to-wallet cold --contract "$MOCK_USDT" --amount 100` | resolves `cold` wallet name → address via `WalletManager::lookup()`; tx accepted |

#### Task 4.3 — CI integration: `tron-integration.yml`

**Files:** `.github/workflows/tron-integration.yml` (new)

- [ ] Add GitHub Actions workflow file:
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

**Verification:** CI runs on every push; testcontainers spawns TronBox in `docker:dind` runner.

#### Task 4.4 — Nile testnet integration test

**Files:** `spikes/tron-v1/tests/trc20_nile.rs`

- [ ] Write integration test `trc20_transfer_full_flow_nile`:
  1. Skip if `TRON_NILE_INTEGRATION` env not set (CI gate).
  2. Load test mnemonic from `TRON_TEST_MNEMONIC` env (never hard-code).
  3. Derive deployer address via `keys::mnemonic_to_secret_key(&mnemonic, "m/44'/195'/0'/0/0")`.
  4. Use pre-deployed community USDT-TRC20 contract **`TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`** (canonical per TronScan).
  5. Verify deployer pre-funded via `chain::trc20_balance(...)`.
  6. Submit transfer 100 mock USDT to recipient.
  7. Wait for confirmation via `tx::wait_for_confirm(&receipt.txid, Duration::from_secs(60), Duration::from_secs(3), &cfg)`.

#### Task 4.5 — Nile testnet test scenarios (rows 1-4)

**Files:** `spikes/tron-v1/tests/trc20_nile.rs` (extend)

| # | Scenario | Difference from Local | Pass criteria |
|---|----------|----------------------|---------------|
| 1 | TRX native transfer | Same | tx accepted on real network; receipt visible on `https://nile.tronscan.org/#/transaction/<txid>` |
| 2 | TRC-20 transfer | Use real test USDT contract `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` (community faucet) | tx accepted; balance visible on `https://nile.tronscan.org/#/token20/...` |
| 3 | Mobile-specific | iOS Simulator: `cargo build --target aarch64-apple-ios-sim`; Android Emulator: `cargo ndk -t x86_64 -o jniLibs` | FFI smoke test passes; Dart binding sends a real tx from emulator to Nile |
| 4 | Network failure recovery | Point RPC at `http://127.0.0.1:9999` (closed port) | CLI returns error code 3 (transport error) within 30s timeout; no panic |

#### Task 4.6 — CI gate: `tron-nile.yml` (manual trigger only)

**Files:** `.github/workflows/tron-nile.yml` (new)

- [ ] Add GitHub Actions workflow file with `on: workflow_dispatch` (manual trigger only — Nile tests are slow + need faucet funds):
  ```yaml
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
            TRON_TEST_MNEMONIC: ${{ secrets.TON_TEST_MNEMONIC }}
  ```
- [ ] **No automated CI** — Nile tests only on manual trigger.

#### Task 4.7 — Decision matrix (test stage → network)

**Files:** `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md` §"Test Scenario" → §"Decision matrix"

| Stage                 | Network                             | Why                                         |
|----------------------|-------------------------------------|---------------------------------------------|
| Unit (per-commit)     | none (InMemoryWalletStorage + mock) | fast, no I/O                                |
| Integration CI        | Local (TronBox)                     | deterministic, fast (~30s), no faucet       |
| Pre-release manual QA | Nile (testnet)                      | real network + faucets; verifies edge cases |
| Mobile CI             | Local (TronBox)                     | no Docker fallback for mobile — **Round-1 grill Q6: mobile CI matrix = `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` (FFI compile only). NO mobile runtime smoke in v0.1. Add runtime mobile smoke via Nile testnet (real network, no Docker) for v0.2.** |
| Production            | Mainnet                             | post-Phase 4 only                           |

#### Phase 4 Verification

- [ ] `cargo test --test trc20_local` PASS (local CI gate).
- [ ] `TRON_NILE_INTEGRATION=1 TRON_TEST_MNEMONIC=... cargo test --test trc20_nile` PASS (operator runbook).
- [ ] `.github/workflows/tron-integration.yml` triggers on push.
- [ ] `.github/workflows/tron-nile.yml` triggers on workflow_dispatch only.
- [ ] Round-1 grill Q6 mobile matrix: `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` both succeed.

**PAUSE. Verify L13 step 11.**

---

### Phase 5 — PAL platform abstraction (4 traits)

**Goal:** Pure Rust core compiles for Linux/macOS/Windows + iOS arm64 + Android arm64 with no source changes. **CI gate:** `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` succeeds.

#### Task 4.1 — Define 4 traits

**Files:** `src/platform/storage.rs`, `src/platform/info.rs`, `src/platform/network.rs`, `src/platform/clock.rs`

- [ ] `pub trait WalletStorage: Send + Sync { fn put/get/list/delete/put_atomic }`.
- [ ] `pub trait PlatformInfo: Send + Sync { fn data_dir/app_version/app_name/is_mobile }`.
- [ ] `pub trait NetworkClient: Send + Sync { fn build_client/default_rpc_url }`.
- [ ] `pub trait Clock: Send + Sync { fn now_millis/sleep }`.

#### Task 4.2 — Desktop impls

**Files:** `src/platform/desktop/storage.rs`, `info.rs`, `network.rs`

- [ ] `FileWalletStorage` uses `~/.local/share/tron/wallets/` (Linux) / `~/Library/Application Support/tron/wallets/` (macOS) / `%APPDATA%\tron\wallets\` (Windows). Mode 0600. Atomic write via temp + rename.
- [ ] `SystemDirsInfo` returns per-OS data dir.
- [ ] `ReqwestClient` builds `reqwest::Client::builder().tls_built_in_webpki_roots().timeout(30s)`.

#### Task 4.3 — iOS impls

**Files:** `src/platform/ios/storage.rs`, `info.rs`

- [ ] `KeychainWalletStorage` calls `ios_keystore::set/get/list/delete` via FFI bridge (Swift wrapper).
- [ ] `BundleInfo` returns NSDocumentDirectory via Swift bridge.
- [ ] `OSRootsClient` uses `tls_built_in_root_certs(true)` on reqwest (mobile uses OS roots).

#### Task 4.4 — Android impls

**Files:** `src/platform/android/storage.rs`, `info.rs`

- [ ] `EncryptedFileWalletStorage` calls `android_keystore::encrypted_file_write/read` via JNI bridge (Kotlin wrapper).
- [ ] `ContextInfo` returns `context.getFilesDir()` via JNI.

#### Task 4.5 — Test impls

**Files:** `src/platform/test/storage.rs`, `info.rs`, `network.rs`, `clock.rs`

- [ ] `InMemoryStorage` uses `Arc<Mutex<HashMap<WalletId, Vec<u8>>>>`.
- [ ] `StaticInfo` returns compile-time constants.
- [ ] `MockClient` returns stub responses.
- [ ] `MockClock` returns deterministic time.

#### Task 4.6 — Compile-time platform selection

**Files:** `src/platform/mod.rs`

- [ ] `#[cfg(target_os = "ios")] pub type DefaultStorage = ios::KeychainWalletStorage;`
- [ ] `#[cfg(target_os = "android")] pub type DefaultStorage = android::EncryptedFileWalletStorage;`
- [ ] `#[cfg(not(any(target_os = "ios", target_os = "android")))] pub type DefaultStorage = desktop::FileWalletStorage;`
- [ ] `default_storage()`, `default_platform_info()`, `default_network_client()` factory functions with cfg gating.

#### Task 4.7 — Wallet persistence (Argon2id + AES-GCM)

**Files:** `src/crypto/mod.rs`, `src/wallet/persist.rs`

- [ ] `crypto::encrypt(plaintext: &[u8], passphrase: &str) -> Result<EncryptedWallet>` uses Argon2id KDF + AES-256-GCM.
- [ ] `crypto::decrypt(ciphertext: &EncryptedWallet, passphrase: &str) -> Result<Vec<u8>>`.
- [ ] `wallet::WalletManager::create(mnemonic: Mnemonic, passphrase: &str) -> Result<WalletId>` → encrypt → `WalletStorage::put`.
- [ ] `wallet::WalletManager::unlock(id: WalletId, passphrase: &str) -> Result<UnlockedWallet>` → `WalletStorage::get` → decrypt → Zeroizing-wrapped mnemonic.
- [ ] **Zeroizing wraps:** Argon2id-derived key (32 bytes) → `Zeroizing<[u8; 32]>`; plaintext entropy during decrypt/re-encrypt window → `Zeroizing<Vec<u8>>`.
- [ ] Test: round-trip create → unlock returns same mnemonic.

**Test Scenario mapping:** supports **Local row 8 (wallet-to-wallet TRC-20)** — `WalletManager::lookup(name_or_id)` requires wallet-id resolution across CLI invocations. Persists encrypted blob via `WalletStorage` (PAL = `FileWalletStorage` desktop / `KeychainWalletStorage` iOS / `EncryptedFileWalletStorage` Android). Also enables **Nile row 3 (Mobile-specific)** — Keychain storage validates FFI boundary for Dart binding on emulator.

#### Phase 4 Verification

- [ ] `cargo build -p tron-wallet-core` succeeds (desktop).
- [ ] `cargo build -p tron-wallet-core --target aarch64-apple-ios` succeeds (iOS compile only).
- [ ] `cargo build -p tron-wallet-core --target aarch64-linux-android` succeeds (Android compile only).
- [ ] `cargo test -p tron-wallet-core` passes persistence + storage impls.
- [ ] **NO mobile runtime smoke in v0.1** (per Round-1 grill Q6).

**PAUSE. Verify L13 step 11.**

---

### Phase 6 — CLI scaffold (22 commands)

**Goal:** `tron` CLI binary with 22 commands across 6 top-level (wallet 9, address 2, balance 2, trc20 4, tx 2, config 3). **CI gate:** `cargo run -p tron -- --help` shows all subcommands.

#### Task 5.1 — Clap parser

**Files:** `crates/tron/src/main.rs`, `crates/tron/Cargo.toml`

- [ ] `clap` derive-based parser with subcommand tree:
  - `wallet { create, import, show, list, delete, rename, balance, send, send-speedup }`
  - `address { new, xpub }`
  - `balance { --address, --token }`
  - `trc20 { send, approve, balance, allowance }`
  - `tx { get, wait }`
  - `config { show, set-rpc, set-network }`
- [ ] Each subcommand accepts `--json` flag.
- [ ] Exit codes (matches btc/src/main.rs:151-169 pattern):
  - 0 = success
  - 1 = user abort
  - 2 = bad input
  - 3 = upstream/RPC transport failure
  - 4 = wallet/balance issue
  - 5 = signing/RPC/broadcast error

#### Task 5.2 — `wallet` subcommand handlers

**Files:** `crates/tron/src/handlers/wallet.rs`

- [ ] `wallet create --words 12|24 --name --network --password` → `WalletManager::create_with_mnemonic`.
- [ ] `wallet import --name --network --password --mnemonic|--mnemonic-file|--private-key-file` → `WalletManager::import_from_phrase` or `import_from_pk`.
- [ ] `wallet show --id [--json]` → `WalletManager::unlock(id, pw).summary()`.
- [ ] `wallet list [--json] [--all-networks]` → `WalletManager::list()`.
- [ ] `wallet delete --id` → `WalletManager::delete(id)`.
- [ ] `wallet rename --id --to` → `WalletManager::rename(id, name)`.
- [ ] `wallet balance --wallet-id [--token USDT|<addr>] | --address [--token <addr>]` → `chain::get_account` or `WalletManager::unlock(id).balance()`.
- [ ] `wallet send --wallet-id|--mnemonic --to <addr>|--to-wallet <name|id> --amount [--unit] [--fee-limit] [--dry-run] [--sign-only] [--wait]` → `tx::submit_trx`.
- [ ] `wallet send-speedup --wallet-id --txid --fee-limit` → `tx::submit_send_speedup`.

#### Task 5.3 — `address` subcommand handlers

**Files:** `crates/tron/src/handlers/address.rs`

- [ ] `address new --mnemonic [--mnemonic-file] --index [--path]` → `keys::derive_keypair`.
- [ ] `address xpub --wallet-id` → `WalletManager::xpub(id)`.

#### Task 5.4 — `balance` subcommand handlers

**Files:** `crates/tron/src/handlers/balance.rs`

- [ ] `balance --address <addr> [--unit trx|sun]` → `chain::get_account(addr)`.
- [ ] `balance --address <addr> --token USDT|<addr>` → `chain::trc20_balance(addr, contract)`.

#### Task 5.5 — `trc20` subcommand handlers

**Files:** `crates/tron/src/handlers/trc20.rs`

- [ ] `trc20 send --mnemonic --contract USDT|<addr> --to --amount` → `tx::submit_trc20`.
- [ ] `trc20 approve --mnemonic --contract --spender --amount` → `tx::submit_trc20_approve`.
- [ ] `trc20 balance --address --contract USDT|<addr>` → `chain::trc20_balance`.
- [ ] `trc20 allowance --contract --owner --spender` → view-call `allowance(owner, spender)`.

#### Task 5.6 — `tx` subcommand handlers

**Files:** `crates/tron/src/handlers/tx.rs`

- [ ] `tx get --txid` → `chain::get_tx_info(txid)`.
- [ ] `tx wait --txid --timeout --poll-interval` → `tx::wait_for_confirm(txid, timeout)`.

#### Task 5.7 — `config` subcommand handlers

**Files:** `crates/tron/src/handlers/config.rs`

- [ ] `config show [--json]` → `config::TronConfig::load().display()`.
- [ ] `config set-rpc <url>` → `config::set_rpc(url)` + save.
- [ ] `config set-network mainnet|shasta|nile` → `config::set_network(net)` + save.

#### Task 5.8 — Confirmation prompts + output formatting

**Files:** `crates/tron/src/handlers/mod.rs`

- [ ] Confirmation prompts for `mainnet`, `drain`, `unlimited approval`: require `yes` (not `y`); default abort; exit 1 on abort.
- [ ] `--json` flag on every data-producing command.
- [ ] Stderr for diagnostics; stdout for requested data only.
- [ ] Mnemonic output → STDERR with red highlight; wallet_id → STDOUT.

#### Phase 6 Verification

- [ ] `cargo build -p tron` succeeds.
- [ ] `cargo run -p tron -- --help` shows all 6 top-level commands.
- [ ] `cargo run -p tron -- wallet --help` shows 9 subcommands.
- [ ] `cargo run -p tron -- trc20 --help` shows 4 subcommands.
- [ ] `cargo run -p tron -- tx --help` shows 2 subcommands.
- [ ] `cargo run -p tron -- config show` exits 0 with valid output.

**PAUSE. Verify L13 step 11.**

---

### Phase 7 — Spike V1-V10 verification + mainnet gate

**Goal:** All 10 spikes pass on local + Nile; **mainnet self-send gate** ($0.001 USDT) succeeds. **CI gate:** Issue #399 acceptance criterion "All 10 open questions either answered or explicitly deferred" flips `[x]`.

#### Task 6.1 — Spike V1 (dep wiring) PASS

**Files:** `spikes/tron-v1/tests/v1_dep_wiring.rs`

- [ ] `cargo build -p tron-spike-v1` succeeds with pinned anychain-* + MSRV 1.98.1.

#### Task 6.2 — Spike V2 (protobuf) PASS

**Files:** `spikes/tron-v1/tests/v2_protobuf_roundtrip.rs`

- [ ] `TronTransaction::encode_to_vec(&raw_data)` round-trips byte-equal.
- [ ] `TriggerSmartContract.data` at proto field 4 (NOT 3).

#### Task 6.3 — Spike V3 (TRC-20 ABI) PASS

**Files:** `spikes/tron-v1/tests/v3_trc20_abi.rs`

- [ ] `abi::encode_call("transfer", to, amount)` produces 68-byte calldata with `0xa9059cbb` selector at bytes [0..4].

#### Task 6.4 — Spike V4 (base58check) PASS

**Files:** `spikes/tron-v1/tests/v4_base58check.rs`

- [ ] Hand-rolled `Address::to_base58([0x41] ++ keccak256(pubkey)[12..32])` → 34-char T-string via `anychain_tron::TronAddress`.

#### Task 6.5 — Spike V5 (resource model) PASS

**Files:** `spikes/tron-v1/tests/v5_resource.rs`

- [ ] Live `wallet/triggerconstantcontract` returns `energy_used` 65k-130k for USDT-TRC20 transfer.
- [ ] `wallet/getcontractinfo.energy_factor` round-trip.

#### Task 6.6 — Spike V6 (Nile chain-id) PASS

**Files:** `spikes/tron-v1/tests/v6_nile.rs`

- [ ] `POST /jsonrpc {"method":"eth_chainId"}` → `0xcd8690dc` on Nile.
- [ ] base58check prefix `0x41` verified.

#### Task 6.7 — Spike V7 (SPKI pin) PASS

**Files:** `spikes/tron-v1/tests/v7_spki_pin.rs`

- [ ] `SpkiPinnedVerifier` accepts `pinned://<correct_pin>@api.trongrid.io`.
- [ ] Rejects wrong pin.
- [ ] No-pin localhost TronBox succeeds.

#### Task 6.8 — Spike V7a (send-speedup rebroadcast semantics) — Round-1 grill Q10

**Files:** `spikes/tron-v1/tests/v7a_send_speedup.rs`

- [ ] Verify `wallet/broadcasttransaction` idempotency: rebroadcast identical `(raw_bytes)` after 60s window — accepted/ignored/error?
- [ ] If accepted → speedup = rebroadcast + new fee_limit via new timestamp.
- [ ] If rejected → document "speedup not possible after window", remove `send-speedup` from v0.1.
- [ ] Record node behavior in `spikes/tron-v1/V7a-speedup.md`.

#### Task 6.9 — Spike V8 (sign-only) PASS

**Files:** `spikes/tron-v1/tests/v8_sign_only.rs`

- [ ] Local-sign TRX transfer (no broadcast).
- [ ] txID = SHA256(SHA256(raw_data_hex)) matches what network reports.
- [ ] `v ∈ {0, 1}` (NOT v+27).

#### Task 6.10 — Spike V9 (token registry) PASS

**Files:** `spikes/tron-v1/tests/v9_token_registry.rs`

- [ ] `tokens/{local,nile,mainnet}.json` loads with expected entries.
- [ ] USDT decimals=6 verified via live `triggerconstantcontract(decimals())`.

#### Task 6.11 — Spike V10 (SLIP-44) PASS

**Files:** `spikes/tron-v1/tests/v10_slip44.rs`

- [ ] `bip39::Mnemonic::parse_in(English, "abandon ×11 about")` → seed → `m/44'/195'/0'/0/0` → T-address matches `kobe-tron` KAT vectors.

#### Task 6.12 — Mainnet self-send gate (Round-1 grill Q4) — DEFER-UNTIL-V1 GATE

**Files:** `spikes/tron-v1/tests/v11_mainnet_self_send.rs`

- [ ] **BLOCKING** for v0.1 release: `$0.001 USDT-TRC20 to self` (recipient == sender) on Mainnet, real value, real network.
- [ ] Pre-check audit hook: refuse if `recipient != operator_wallet`.
- [ ] `RUN_TRON_MAINNET=1` env gate (mirror `RUN_TRON_NILE=1`).
- [ ] No public docs. Internal runbook only.
- [ ] **No mainnet smoke in CI** — local + Nile only by default.

#### Task 6.13 — Record V1-V11 PASS evidence in `RESULT.md`

**Files:** `spikes/tron-v1/RESULT.md`

- [ ] One section per Vn with raw command + output + git SHA + network tag (`local` | `nile` | `mainnet`).
- [ ] When all 10 Vns pass on local + Nile, issue #399 acceptance criterion flips `[x]`.

#### Phase 7 Verification

- [ ] All 10 Vns PASS on local + Nile.
- [ ] V11 mainnet self-send PASS (with `RUN_TRON_MAINNET=1`).
- [ ] `RESULT.md` complete.
- [ ] `cargo test -p tron-spike-v1 --tests` passes.

**PAUSE. Final verification gate before L13 step 13 (commit-push-pr).**

---

## L13 Pipeline Application (per `tasks/lessons.md`)

Per L13 step 3a → this plan IS the bounded path (Type A from plan-guide). Subagent-driven-development applied per phase.

| Step | Action | Status |
|------|--------|--------|
| 1 | Read CLAUDE.md + tasks/lessons.md | ✓ done at session start |
| 2 | Skill-tag task (Type A → superpowers:mattpocock + superpowers:superpowers) | ✓ this plan |
| 3 | TDD per Phase — failing test first, then impl | enforced via Phase verification gates |
| 4 | L12 review (mattpocock:code-review) | per phase PAUSE |
| 5 | Verify (L13 step 11) | per phase PAUSE |
| 6 | Backlog triage (11a) | handled via issue #399 |
| 7 | L24 doc updates (15b) | Task 0.5 (regenerate user-stories) + Task 0.7 (CONTEXT.md) |
| 8 | PAUSE before commit (12) | per phase |
| 9 | Commit-push-pr (13) | **PAUSE here per never-auto-commit rule** |
| 10 | Flip issue checkboxes [ ]→[x] (14) | per GateGuard gh-pr classifier |
| 11 | PR review + merge + close (15) | per CLAUDE.md update-issues-before-merge |
| 12 | Tech doc (15a) | this plan + deep-dive + ADR-0001 |
| 13 | Verify L24 + release-cut (15b) | per L24 doc taxonomy |
| 14 | Ledger entry (17) | per L17 |
| 15 | Harvest lessons (18) | per L18 |
| 16 | L21 reports (19) | per L21 |

---

## Acceptance Criteria (issue #399 flip gate)

Per issue #399: "All 10 open questions either answered (with chosen path + rationale) or explicitly deferred to v0.2+ with rationale". This plan resolves Q1-Q12 (Round-1 grill + deep-dive Q1-Q10):

| Q | Resolution | Source |
|---|------------|--------|
| Q1 | anychain-tron 0.2.14 + anychain-kms 0.1.23 (crates.io, exact pin) | Round-1 grill + ADR-0001 |
| Q2 | Protobuf via anychain-tron vendored types + dual-SHA256 txid workaround | Round-1 grill Q2 |
| Q3 | **No vendoring** — bus-factor accepted risk; exact-version pin + regression tests | Round-1 grill Q3 (revised 2026-09-05) |
| Q4 | Mainnet self-send gate `$0.001 USDT` | Round-1 grill Q4 |
| Q5 | SPKI pin live extraction `0e43f611...` | Round-1 grill Q5 |
| Q6 | Nile testnet (chain-id 0xcd8690dc) | deep-dive Q6 |
| Q7 | `pinned://` URL + `SpkiPinnedVerifier` reuse | deep-dive Q7 |
| Q8 | Sign-only path with `v ∈ {0, 1}` | deep-dive Q8 |
| Q9 | Token registry `{local,nile,mainnet}.json` | deep-dive Q9 |
| Q10 | SLIP-44 coin 195, path m/44'/195'/0'/0/0 | deep-dive Q10 |
| Q11 | Stake 1.0 deferred; Stake 2.0 only | Round-1 grill Q11 |
| Q12 | Disambiguation guards | Round-1 grill Q12 |

**#399 closes when:** Phases 0-6 complete + V11 mainnet self-send PASS + Round-1 grill Q8 audit (every "Status: ready" row verified against Vn spike PASS block).

---

## Out of Scope for v0.1 (deferred)

- **Stake 2.0** (freeze/unfreeze/delegate/undelegate/cancel/withdraw + witness vote): V0.1.5 — ships with V0.1 release train, separate plan.
- **Thread model** (FFI deadlock / Zeroizing-across-await / Send+Sync on secrets / concurrent broadcast seriality / mobile-vs-desktop runtime divergence): **v0.2** (per deep-dive §"Deferred" → "Thread model").
- **Mobile runtime smoke** (FFI compile only in v0.1): v0.2 via Nile testnet (no Docker fallback).
- **Resource model UX** (Story 8 — `tron resource`): v0.2.
- **Sign personal message** (Story 18 — `tron sign-message`): v0.2.
- **Token list/register** (Stories 23, 24): v0.2.
- **TRC-10 token transfers** (Story 34): v0.3+ (separate `TransferAssetContract` proto encoding).
- **Hardware wallet** (Ledger, Trezor): v1.x.
- **gRPC transport**: v0.3+ if TronGrid gRPC perf becomes bottleneck.
- **Multi-sig / governance flows**: v1.x.
- **Stories 13 (batch), 14 (drain), 15 (ref-block), 16 (manual exp)**: redesign dropped from v0.1.
- **Local tx index** (cached tx history): v0.3+ — every `tx list` call scans blocks.
- **Watch-only wallet import from xpub**: rarely used, deferred indefinitely.

---

## v0.1 Release Status (target)

**Release cut for `tron-wallet-core v0.1.0` library + `tron` CLI v0.1.0 binary.**

**Stories shipped:** 1, 2, 3, 5, 7, 9, 10, 11, 12, 17, 19, 21, 22, 25, 27, 28, 29 (17 stories from deep-dive V0.1) + 26 (TronBox local via `--rpc` flag) + 30 (trc20 approve + allowance already in 25).

**Total user stories covered:** 18 of 29 + 1 cross-cutting.

**Stories removed:** 13, 14, 15, 16.

**Stories deferred:** 4, 6, 8, 18, 23, 24, 31, 32, 33 (V0.1.5 + V0.2).

**Try it (target surface, post-spike):**

```bash
# Workspace build
cargo build -p tron-wallet-core
cargo build -p tron

# Library unit tests
cargo test -p tron-wallet-core --lib

# Spike V1-V10
cargo test -p tron-spike-v1 --tests

# Nile smoke (operator-driven per L29 — set RUN_TRON_NILE=1)
RUN_TRON_NILE=1 cargo test -p tron-spike-v1 --test '*'

# Mainnet self-send gate (operator-driven per L29 — set RUN_TRON_MAINNET=1)
RUN_TRON_MAINNET=1 cargo test -p tron-spike-v1 --test v11_mainnet_self_send

# Mobile compile-only (FFI)
cargo build -p tron-wallet-core --target aarch64-apple-ios
cargo build -p tron-wallet-core --target aarch64-linux-android

# CLI scaffold
cargo run -p tron -- --help
cargo run -p tron -- wallet --help
cargo run -p tron -- trc20 --help
cargo run -p tron -- tx --help
cargo run -p tron -- config show
```

---

## References

- Deep-dive (source of truth): `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md`
- User stories (legacy, must regenerate per Task 0.5): `docs/wallets/2026-08-27-tron-wallet-user-stories.md`
- ADR (reversal): `docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md`
- Issue: #399 (Q1-Q10 resolved in deep-dive; this plan covers Q11-Q12 + mainnet gate)
- Superseded plan: `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`
- Plan guide: `.local/plugins-docs/2026-09-05-plan-guide-mattpocock-superpowers-stack.md`
- Project rules: `tasks/lessons.md` L13 + `CLAUDE.md`
- Global rules: `~/.claude/CLAUDE.md` (caveman mode + superpowers meta-rule)
- SLIP-44 coin types (TRON = 195): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- TRON Developer Hub — Transactions: <https://developers.tron.network/docs/tron-protocol-transaction>
- TRON Developer Hub — Encoding: <https://developers.tron.network/docs/encoding>
- Anychain repo: <https://github.com/0xcregis/anychain>
- Bitcoin SPKI pin pattern source: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs`

---

## PAUSE Points (per never-auto-commit rule)

After each phase, **PAUSE before commit**. Show verification output (per L13 step 11 + superpowers verification-before-completion Iron Law). User approves before `git commit`. Per CLAUDE.md GateGuard, use `--body-file` with content in `/tmp` for `gh pr create`.

**No auto-merge.** Update issue #399 checkboxes [ ] → [x] BEFORE squash-merge (per CLAUDE.md update-issues-before-merge rule).

---

**END PLAN. Awaiting user approval to begin Phase 0 implementation.**