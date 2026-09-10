# Solana-Specific Rust SDK Deep-Dive

**Date:** 2026-09-08 (revised 2026-09-09 to apply plan-guide stack)
**Scope:** Focused re-research on Rust crates for a Solana (SOL + SPL stablecoin) wallet built inside `rust-wallet-app/`, covering native SOL transfer, SPL token transfer (USDC primary, USDT/PYUSD secondary), Ed25519 keypair + SLIP-0010 HD derivation, base58 address encoding, versioned transactions (v0 legacy + lookup tables), Compute Budget + priority fee handling, and Associated Token Account (ATA) lifecycle. Verifies the chosen crate surface against current 2026 state, considers alternatives, and digs into program-derived addresses (PDA) + Token-2022 extension awareness.
**Companion docs:**
- `.local/plugins-docs/2026-09-05-plan-guide-mattpocock-superpowers-stack.md` — workflow framework applied here
- `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md` — implementation plan derived from this deep-dive (2026-09-09 session)
- `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md` — security audit companion
- `.local/solana-sdk/` + `.local/crates/solana-sdk/*.md` per-crate notes (deeper dives on specific subcrates)

**Pre-empts:** v0.3+ deliverable sketched in `rust-wallet-app/crates/chain-traits/src/lib.rs:21` (`ChainId::Solana(String)` placeholder for SOL coin — pubkey base58 string).
**Status:** Research report feeding an implementation plan. **`solana-sdk`-first design** — every crypto primitive delegates to Anza; custom wallet-local code only for persistence (Argon2id + AES-GCM), PAL traits, FFI surface, Token-2022 footgun guard, retry/backoff. No HD wrapper, no custom signer, no custom RPC client. **Phantom UX parity** — wallet surface mirrors Phantom's user-facing API (numeric `--account` + `--address-index` flags, no path strings exposed).

## Plan-guide stack applied (2026-09-09)

This deep-dive feeds `Type A: New crate / architectural change` per the plan-guide (`.local/plugins-docs/2026-09-05-plan-guide-mattpocock-superpowers-stack.md`). Stack order for V0.1 sol-wallet-core:

```text
1. /superpowers:brainstorming                          (classification: architectural)
2. /mattpocock-skills:grill-with-docs                  (CONTEXT.md + ADR — captured in `## Decisions (grill Round 1)` at end)
3. /superpowers:writing-plans                          (this doc + the implementation plan 2026-09-09-sol-wallet-core-v0.1.md)
4. /mattpocock-skills:to-tickets                       (Phase 0-9 → issue tracker tickets, blocking edges per L13 step 3)
5. /superpowers:subagent-driven-development            (one subagent per Phase ticket)
   ↳ /superpowers:test-driven-development             (red → green → refactor; library + CLI test scenarios per deep-dive Test scenario — sol-wallet-core V0.1 + Test scenario — sol CLI)
     ↳ /superpowers:verification-before-completion    (per "done" claim — show evidence, never assert)
6. /mattpocock-skills:code-review                      (Standards + Spec, parallel subagents)
7. /superpowers:receiving-code-review                  (anti-sycophancy on review feedback)
8. /superpowers:finishing-a-development-branch         (worktree-aware merge/PR/cleanup)
```

**Plugin selection rationale:**

- **superpowers** wins on: TDD Iron Law, verification-before-completion (run commands show output), SessionStart hook auto-fires every session, subagent-driven-development for the 9-phase plan.
- **mattpocock-skills** wins on: ADR (documents the `solana-sdk`-first vs `ed25519-bip32`-only-HD reversal — same shape as Tron ADR-0001 reversal), domain-modeling for cross-chain glossary (BTC + ETH + Polygon + TRON + SOL), `to-tickets` for blocking edges between Phase tickets.
- **Both together** = state continuity (mattpocock) + process rigour (superpowers).

**Project-specific plugin binding (from `plan-guide` table):**

| Constraint | Plugin invocation | Where in this doc / plan |
|---|---|---|
| L13 step 3: read-only task pickup | `/superpowers:brainstorming` (bounded path) | Phase 0 ticket pickup |
| L13 step 9: red-green cycle | `/superpowers:test-driven-development` | Phase 2-9 deliverable tests |
| L13 step 9a: new module interface | `/mattpocock-skills:codebase-design` → `/mattpocock-skills:domain-modeling` | `### F. Wallet keypair (Phantom-equivalent surface)` — mirrors Phantom |
| L13 step 11: verify gate | `/superpowers:verification-before-completion` | Phase 9 mainnet smoke gate |
| L13 step 13: commit-push-pr | `/superpowers:finishing-a-development-branch` | Phase boundaries |
| L13 step 15a: tech doc | `/mattpocock-skills:to-spec` | synthesize to spec on tracker |
| Never-auto-commit | PAUSE before `git commit` | every Phase close |
| Update-issues-before-merge | flip `[ ]` → `[x]` BEFORE squash-merge | per L13 step 14 |
| GateGuard `gh-pr classifier` | `--body-file` with content in `/tmp` | PR creation |

**Stacking reminder:** superpowers SessionStart hook fires first → mattpocock `ask-matt` for routing → brainstorming (gate) → mattpocock `grill-with-docs` / `to-spec` for state → superpowers `writing-plans` → mattpocock `to-tickets` → superpowers `subagent-driven-development`.

## TL;DR

Use the **Anza stack** (`solana-sdk` 4.1.0 + `solana-client` 4.2.2 + `solana-program` 4.1.0) as the primary SDK. **`solana-sdk` provides the wire-format layer** — Ed25519 `Keypair`, `Pubkey` (base58 address), `Message`/`VersionedTransaction` (legacy + v0), `Transaction::sign` (Ed25519 over SHA-512 truncated). **`solana-client` provides the RPC layer** — HTTP JSON-RPC + WebSocket subscriptions (`RpcClient`, `PubsubClient`). **SPL programs live in the `solana-program` org crates** — `spl-token` 9.0.0 (classic), `spl-token-2022` 11.0.0 (Token Extensions), `spl-associated-token-account` 8.0.0, `spl-memo` 7.0.0. **HD derivation uses `ed25519-bip32` 0.4.3** (typed-io, MIT/Apache-2.0) — SLIP-0010 + SLIP-44 coin 501 (SOL). **MSRV 1.89.0 mandatory** — every Anza crate pins it. **1 known license blocker** — Metaplex's `mpl-token-metadata` / `mpl-core` use the non-OSI `Metaplex NFT Open Source License v1.0`; defer NFT support, do not pull into v0.1. **No third-party RPC client** — `solana-client` covers HTTP + WS; caller adds ~150 lines of `reqwest` fallback only for `getLatestBlockhash` racing patterns. **Token programs split**: classic SPL `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` vs Token-2022 `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` — ATA derivation uses `token_program_id` as a seed, so mixing produces different addresses. **p-token rewrite deployed 2026** dropped SPL `transfer_checked` from ~150 CU to ~3-5 CU; safe CU budget per transfer = 5,000. **Blockhash expiry** ~60-90 sec (~150 slots at ~400ms) — must `getLatestBlockhash` immediately before sign, retry with fresh hash on `BlockhashNotFound`.

**Third-party crate survey:** [`qntx/kobe`](https://github.com/jito-foundation/kobe) — NOT a wallet lib (Jito MEV research repo, 5 stars; name collision). [`otter-sec/anchor`](https://github.com/otter-sec/anchor) 1.2.0 — backup, but adds ~6 MB + IDL codegen step; not needed for direct SOL + SPL transfers in v0.1. [`metaplex-foundation/mpl-token-metadata`](https://github.com/metaplex-foundation/mpl-token-metadata) 5.1.1 — **REJECTED** for v0.1 (non-OSI license with commercial restrictions). [`jito-labs/jito-rust-rpc`](https://github.com/jito-labs/jito-rust-rpc) 0.3.2 — stale (last release 2025-06-21), gate behind `jito` Cargo feature. [`CRossel87a/solana-light-client`](https://github.com/CRossel87a/solana-light-client) 0.1.0 — too immature. **Anza (Solana Labs successor)** **CHOSEN** — official SDK + agave validator + `solana-program` org (SPL); single-vendor bus factor for 90%+ of stack, accepted.

## Chosen crates & SDKs (v0.1, Anza official)

**Decision (locked 2026-09-08):** adopt the Anza stack — `solana-sdk` 4.1.0 (wire format + Ed25519 + Keypair) + `solana-client` 4.2.2 (RPC client + WS subscriptions) + `spl-token` 9.0.0 / `spl-token-2022` 11.0.0 / `spl-associated-token-account` 8.0.0 (SPL programs) + `ed25519-bip32` 0.4.3 (SLIP-0010 HD). Trades ~50 lines of `reqwest` glue (blockhash refresh + confirmation polling) for ~3000 lines of hand-rolled message encoding + sign + send. MSRV 1.89.0 required by every Anza crate.

| Crate | Version | Role | License | MSRV |
| --- | --- | --- | --- | --- |
| `solana-sdk` | 4.1.0 | `Keypair`, `Pubkey` (base58), `Signature`, `Message`/`VersionedTransaction` (legacy + v0), `Transaction::sign` (Ed25519) | Apache-2.0 | **1.89.0** |
| `solana-program` | 4.1.0 | program-side primitives (PDA, hash, sysvar) for on-chain-style code; re-exported by `solana-sdk` | Apache-2.0 | 1.89.0 |
| `solana-client` | 4.2.2 | `RpcClient` (HTTP JSON-RPC) + `PubsubClient` (WS subscriptions); `send_transaction`, `simulate_transaction`, `get_latest_blockhash`, `get_account_info`, `get_signature_statuses` | Apache-2.0 | 1.89.0 |
| `solana-keypair` | 3.1.2 | concrete `Keypair` signer (Ed25519 keypair generation + sign) | Apache-2.0 | 1.89.0 |
| `solana-signer` | 3.0.1 | `Signer` trait (abstract) — supports `Keypair`, `Presigner`, remote signers | Apache-2.0 | 1.89.0 |
| `solana-message` | 4.6.0 | `Message` (legacy) + `VersionedMessage` (v0 with lookup tables) compilation | Apache-2.0 | 1.89.0 |
| `solana-transaction` | 4.3.0 | `Transaction` + `VersionedTransaction` envelopes; `sign` (single + multi) | Apache-2.0 | 1.89.0 |
| `solana-instruction` | 3.5.0 | `Instruction`, `AccountMeta`, `Account` | Apache-2.0 | 1.89.0 |
| `solana-compute-budget-program` | 4.2.2 | `ComputeBudgetInstruction::set_compute_unit_limit` + `set_compute_unit_price` | Apache-2.0 | 1.89.0 |
| `solana-rpc` | 4.2.2 | RPC request/response types | Apache-2.0 | 1.89.0 |
| `solana-rpc-client` | 4.2.2 | typed RPC client (alternative to `solana-client::RpcClient`) | Apache-2.0 | 1.89.0 |
| `spl-token` | 9.0.0 | classic SPL Token program: `transfer_checked`, `mint_to`, `burn`, `approve`, `close_account`, `set_authority` + `unpack_mint` | Apache-2.0 | matches workspace |
| `spl-token-2022` | 11.0.0 | Token Extensions: same as `spl-token` + `transfer_hook`, `confidential_transfer`, `interest_bearing`, `permanent_delegate` | Apache-2.0 | matches workspace |
| `spl-associated-token-account` | 8.0.0 | `get_associated_token_address` + `create_associated_token_account` | Apache-2.0 | matches workspace |
| `spl-memo` | 7.0.0 | `memo` instruction builder | Apache-2.0 | matches workspace |
| `ed25519-dalek` | 3.0.0 | Ed25519 signing (transitive via `solana-keypair`; pin to avoid double-version) | BSD-3-Clause | matches workspace |
| `ed25519-bip32` | 0.4.3 | SLIP-0010 HD derivation for Ed25519 (SLIP-44 coin 501 = SOL) | MIT OR Apache-2.0 | matches workspace |
| `reqwest` | 0.12 (`rustls-tls`) | fallback HTTP client for SPKI-pinned RPC providers + retry-after | Apache-2.0 / MIT | matches workspace |
| `rustls` | 0.23 | TLS for reqwest + SPKI pin verifier (reuse `bitcoin-wallet-core::chain::spki`) | Apache-2.0 / MIT | matches workspace |
| `serde` + `serde_json` | latest | JSON-RPC envelope parse | Apache-2.0 / MIT | matches workspace |
| `clap` | 4 | CLI subcommand parser | Apache-2.0 / MIT | matches workspace |
| `argon2` + `aes-gcm` | workspace | Wallet file encryption (PBKDF2 → seed, AES-256-GCM blob) | MIT OR Apache-2.0 | matches workspace |
| `surfpool` (binary, not crate) | n/a | local Solana validator for tests — see `### Local validator — surfpool` | n/a | install via `cargo install surfpool --locked` |
| `zeroize` | 1.x | Wrap seed bytes + keypair secret before sign | Apache-2.0 / MIT | matches workspace |
| `bip39` | workspace | BIP-39 mnemonic (English only for SOL to avoid bloat) | MIT OR Apache-2.0 | matches workspace |

## Crates used in `sol-wallet-core` (V0.1, projected)

**~25 crates total:** 22 mobile-safe (88%) + 2 desktop-only (8%) + 1 build-time (4%).

### Direct dependencies (organized by purpose)

#### Anza stack (wire format + RPC + signing)

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `solana-sdk` | 4.1.0 | facade re-exporting `solana-keypair`/`solana-message`/`solana-transaction` | ✓ |
| `solana-program` | 4.1.0 | PDA + sysvar + hash (re-exported via solana-sdk) | ✓ |
| `solana-keypair` | 3.1.2 | concrete `Keypair` (Ed25519) | ✓ |
| `solana-signer` | 3.0.1 | `Signer` trait (abstract) | ✓ |
| `solana-message` | 4.6.0 | `Message` (legacy) + `VersionedMessage` (v0) | ✓ |
| `solana-transaction` | 4.3.0 | `Transaction` + `VersionedTransaction` envelopes | ✓ |
| `solana-instruction` | 3.5.0 | `Instruction`, `AccountMeta` | ✓ |
| `solana-client` | 4.2.2 | `RpcClient` + `PubsubClient` | ✓ |
| `solana-rpc-client` | 4.2.2 | typed RPC alt | ✓ |
| `solana-compute-budget-program` | 4.2.2 | CU-limit + CU-price instructions | ✓ |

#### SPL programs (token + token-2022 + ATA + memo)

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `spl-token` | 9.0.0 | classic SPL Token: `transfer_checked`, `mint_to`, `burn`, `close_account` | ✓ |
| `spl-token-2022` | 11.0.0 | Token Extensions: `transfer_checked` (ext-aware) | ✓ |
| `spl-associated-token-account` | 8.0.0 | `get_associated_token_address` + `create_associated_token_account` | ✓ |
| `spl-memo` | 7.0.0 | `memo` instruction | ✓ |

#### Crypto (RustCrypto + Ed25519)

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `ed25519-dalek` | 3.0.0 | Ed25519 sign (transitive via solana-keypair; pin direct) | ✓ pure Rust |
| `argon2` | 0.5 | Argon2id KDF for wallet file encryption | ✓ pure Rust |
| `aes-gcm` | 0.10 | AES-256-GCM for `EncryptedWallet` blob | ✓ pure Rust |
| `sha2` | workspace | SHA-256 (memo hash, msg-id derivation) | ✓ pure Rust |
| `sha3` | workspace | Keccak-256 (only if cross-chain check needed) | ✓ pure Rust |
| `bs58` | 0.5 | base58 encode/decode (SOL addresses, tx signatures) | ✓ pure Rust |
| `hex` | workspace | hex encode/decode | ✓ pure Rust |
| `zeroize` | 1.x | `Zeroizing<Vec<u8>>` for seed + secret key | ✓ pure Rust |
| `subtle` | 2 | ConstantTimeEq for xprv compare + ATA owner verify | ✓ pure Rust |

#### HD derivation (SLIP-0010 + BIP-39)

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `bip39` | workspace | BIP-39 mnemonic (English wordlist only for SOL HD — 8-lang bloat not needed) | ✓ |
| `ed25519-bip32` | 0.4.3 | SLIP-0010 Ed25519 HD (SLIP-44 coin 501 = SOL) | ✓ |
| `hmac` | workspace | HMAC-SHA512 (used by SLIP-0010 chain key derivation) | ✓ |

#### Encoding / serialization

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `serde` | 1.x | derive Serialize/Deserialize | ✓ |
| `serde_json` | 1.x | JSON-RPC envelope + RPC response parse | ✓ |
| `chrono` | workspace | timestamp for tx log; NOT in signature path | ✓ |
| `uuid` | 1.x | Wallet id (UUID v4) | ✓ |
| `directories` | workspace | Desktop data dir — V0.1.5 removal, replaced by `WalletStorage` trait | ❌ desktop-only |

#### Async + HTTP

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `tokio` | 1.x | Async runtime (current_thread for FFI; multi-thread for CLI) | ✓ |
| `reqwest` | 0.12 | HTTP client for RPC providers with SPKI pin | ✓ with `rustls-tls` |
| `rustls` | 0.23 | TLS for reqwest + custom SPKI pin verifier | ✓ mobile uses `aws-lc-rs` provider |
| `rustls-native-certs` | 0.7 | Desktop OS root cert loading | ❌ desktop-only (mobile uses `tls_built_in_root_certs(true)`) |
| `webpki` | 0.22 | Custom `ServerCertVerifier` for SPKI pinning | ✓ pure Rust |
| `x509-parser` | 0.16 | SPKI DER extraction from cert chain | ✓ pure Rust |
| `tungstenite` | 0.24 | WS client (used by `solana-client::PubsubClient` internally) | ✓ |

#### Errors + tracing

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `thiserror` | 1.x | `Error` enum derive | ✓ |
| `tracing` | workspace | Structured logging (STDERR, secret-scrubbing filter) | ✓ |
| `tracing-subscriber` | workspace | Subscriber with EnvFilter | ✓ (CLI only) |

#### FFI + safety

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `once_cell` | 1.x | Lazy-init for FFI runtime + compiled regex patterns | ✓ |
| `regex` | 1.x | Panic-message scrubber (redact mnemonic + seed + secret key) | ✓ |

#### Build-time

| Crate | Version | Purpose | Mobile? |
| --- | --- | --- | --- |
| `cbindgen` | workspace | Generates C header for FFI consumers (Dart/Swift/Kotlin) | ✓ build-time only, doesn't ship |

#### Test-only (dev-dependencies)

| Crate | Purpose | Mobile? |
| --- | --- | --- |
| `proptest` | Property-based tests for amount parsing, ATA derivation, address validation | ✓ |
| `tempfile` | Atomic-write test fixtures | ✓ |
| `surfpool_guard` (test helper, not crate) | `SurfpoolGuard::spawn()` RAII wrapper for `surfpool` binary subprocess | ✓ (test-only, no runtime dep) |
| `solana_test_validator_guard` (test helper, not crate) | `SolanaTestValidatorGuard::spawn()` RAII wrapper for V0.1.5 opt-in (BPF + epoch tests) | ✓ (test-only, no runtime dep) |
| `reqwest` (dev) | integration test HTTP client | ✓ |

### Dependency tree summary

```text
sol-wallet-core
├── Anza stack (10 crates)
│   ├── solana-sdk (facade) + solana-program
│   ├── solana-keypair + solana-signer
│   ├── solana-message + solana-transaction + solana-instruction
│   ├── solana-client + solana-rpc-client
│   └── solana-compute-budget-program
│
├── SPL programs (4 crates)
│   ├── spl-token (classic) + spl-token-2022 (extensions)
│   ├── spl-associated-token-account
│   └── spl-memo
│
├── crypto
│   ├── ed25519-dalek (Ed25519)
│   ├── argon2 (KDF) + aes-gcm (symmetric)
│   ├── sha2, sha3, bs58, hex, zeroize, subtle
│   └── bip39 + ed25519-bip32 + hmac (HD)
│
├── async + HTTP
│   ├── tokio (runtime)
│   ├── reqwest (RPC fallback)
│   ├── rustls + rustls-native-certs + webpki + x509-parser (TLS + SPKI pin)
│   └── tungstenite (WS, via solana-client)
│
├── misc
│   ├── serde + serde_json + chrono + uuid + directories (desktop only — V0.1.5)
│   ├── thiserror, tracing
│   ├── once_cell, regex (FFI safety)
│   └── cbindgen (build-time)
│
└── dev
    ├── proptest, tempfile
    └── (no testcontainers — surfpool spawned via tokio::process::Command)
```

### Mobile-unsafe dependencies (must remove for V0.1.5)

| Crate | Reason | Replacement |
| --- | --- | --- |
| `directories` | No iOS/Android backend | `WalletStorage` trait (File/Keychain/EncryptedFile) |
| `rustls-native-certs` | Desktop-only OS cert loader | Use `tls_built_in_root_certs(true)` on mobile (reqwest) |
| (none — surfpool spawns as subprocess, no Docker daemon required) | n/a | n/a |

### Mobile-unsafe transitive deps (verify at gate)

| Crate | Risk | Mitigation |
| --- | --- | --- |
| `solana-client` + `solana-rpc-client` | ~12 MB stripped, pulls `tokio` + `solana-net-utils` | Acceptable; profile = `release-mobile` with `opt-level = "z"` strips to ~6-7 MB |
| `ed25519-dalek` 3.0.0 | ~1 MB stripped | Acceptable; pure Rust via curve25519-dalek |
| `chrono` | ~150 KB stripped | Acceptable; or drop + use `std::time::SystemTime` |
| `reqwest` + `rustls` | ~500 KB stripped | Acceptable; profile strips to ~300 KB |

### Cargo.toml ordering recommendation

```toml
[dependencies]
# Anza stack (pin each subcrate independently — version desync)
solana-sdk                    = "=4.1.0"
solana-program                = "=4.1.0"
solana-keypair                = "=3.1.2"
solana-signer                 = "=3.0.1"
solana-message                = "=4.6.0"
solana-transaction            = "=4.3.0"
solana-instruction            = "=3.5.0"
solana-client                 = "=4.2.2"
solana-rpc-client             = "=4.2.2"
solana-compute-budget-program = "=4.2.2"

# SPL programs
spl-token                    = { workspace = true }
spl-token-2022               = { workspace = true }
spl-associated-token-account = { workspace = true }
spl-memo                     = { workspace = true }

# Crypto (RustCrypto)
ed25519-dalek = "=3.0.0"
argon2        = { workspace = true }
aes-gcm       = { workspace = true }
sha2          = { workspace = true }
sha3          = { workspace = true }
bs58          = { workspace = true }
hex           = { workspace = true }
zeroize       = { workspace = true }
subtle        = { workspace = true }

# HD derivation
bip39         = { workspace = true, default-features = false, features = ["english"] }
ed25519-bip32 = "=0.4.3"
hmac          = { workspace = true }

# Encoding
serde      = { workspace = true }
serde_json = { workspace = true }
chrono     = { workspace = true }
uuid       = { workspace = true, features = ["v4", "serde"] }

# Async + HTTP
tokio             = { workspace = true }
reqwest           = { workspace = true, default-features = false, features = ["json", "rustls-tls"] }
rustls            = { workspace = true }
webpki            = { workspace = true }
x509-parser       = { workspace = true }

# Errors + tracing
thiserror = { workspace = true }
tracing   = { workspace = true }

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
# (no testcontainers — surfpool spawned via tokio::process::Command in test helpers)
```

### License summary

| License | Crates |
| --- | --- |
| Apache-2.0 | solana-*, spl-*, ed25519-bip32, ed25519-dalek (with curve25519-dalek), argon2, aes-gcm, sha2, sha3, bs58, hex, zeroize, subtle, serde, tokio, reqwest, etc. |
| MIT | bip39, ed25519-bip32, regex, tungstenite |
| MIT OR Apache-2.0 | ed25519-bip32 |
| BSD-3-Clause | ed25519-dalek, curve25519-dalek |
| **EXCLUDED** | `mpl-token-metadata` (Metaplex NFT Open Source License v1.0 — non-OSI) |

All compatible with `rust-wallet-app` MIT workspace license.

### Binary size impact (mobile, stripped + LTO)

| Crate | Stripped contribution |
| --- | --- |
| `solana-client` + `solana-rpc-client` | ~6-7 MB |
| `solana-sdk` + `solana-program` + `solana-message` + `solana-transaction` | ~3-4 MB |
| `ed25519-dalek` + `curve25519-dalek` | ~1 MB |
| `spl-token*` + `spl-associated-token-account` | ~3 MB combined |
| `rustls` + `reqwest` | ~400-500 KB |
| `argon2` | ~50 KB |
| `aes-gcm` | ~30 KB |

**Total V0.1 `libsol_wallet_core.so` size estimate: ~12-15 MB** (largest crate is solana-client; acceptable for mobile, optimize in V0.1.5 with `opt-level = "z"` and dead-code elimination of unused Anza submodules).

### Summary

| Category | Count | Mobile-safe | Desktop-only |
| --- | --- | --- | --- |
| Anza stack | 10 | 10 | 0 |
| SPL programs | 4 | 4 | 0 |
| Crypto + HD | 12 | 12 | 0 |
| Async + HTTP | 6 | 5 | 1 (`rustls-native-certs`) |
| Misc | 5 | 5 | 0 |
| Errors + tracing | 3 | 3 | 0 |
| FFI + safety | 2 | 2 | 0 |
| Build-time | 1 | 1 | 0 |
| Dev (test) | 3 (surfpool_guard + solana_test_validator_guard helpers in `tests/common/`, no external dev-dep for spawn) | 3 | 0 |
| **Total** | **46** | **44 (96%)** | **2 (4%)** |

**V0.1.5 work:** remove `directories` (1 crate), gate `rustls-native-certs` behind `#[cfg(not(mobile))]`. ~15 LOC of Cargo.toml changes. (`testcontainers` removed 2026-09-08 in favor of surfpool subprocess spawn.)

## Solana Networks

V0.1 wallet targets three Solana clusters: **Mainnet-Beta** (production, gated), **Devnet** (default test cluster), **Localnet** (`surfpool` for unit/integration tests). All traffic served via `solana-client::RpcClient` (HTTP) + `PubsubClient` (WS subscriptions). SPKI pin (Q7) skipped by default per Solana Labs no-pin policy; cert transparency + standard rustls verification used as primary defense. **Testnet is DEPRECATED 2022-23** — Solana Foundation discontinued it in favor of devnet; V0.1 deliberately omits testnet support; `Cluster` enum has `MainnetBeta / Devnet / Localnet` only.

### Cluster overview (endpoints + RPC methods)

| Cluster | HTTP RPC | WebSocket | Notes |
| --- | --- | --- | --- |
| **Mainnet-Beta** | `https://api.mainnet-beta.solana.com` | `wss://api.mainnet-beta.solana.com` | Production. Solana Labs public RPC, heavy rate-limit (~10 RPS). Recommended: Alchemy (30M CU/mo free) or Helius (1M credits/mo free). |
| **Devnet** | `https://api.devnet.solana.com` | `wss://api.devnet.solana.com` | Default test cluster. Free airdrop via `solana airdrop 5 <RECIPIENT> --url devnet` (rate-limited per IP). Scheduled wipe ~quarterly. |
| **Localnet** | `http://127.0.0.1:8899` | `ws://127.0.0.1:8900` | `surfpool` subprocess (V0.1 default). Faucet: built-in (`--faucet 1000000000000`) or `surfpool airdrop`. Per-test ephemeral port, RAII teardown. |
| **Testnet** | `https://api.testnet.solana.com` | `wss://api.testnet.solana.com` | **DEPRECATED 2022-23.** Do NOT use. No `Cluster::Testnet` variant. |

**Paid providers** (mainnet-beta only unless noted): Alchemy `https://solana-mainnet.g.alchemy.com/v2/<KEY>` (30M CU/mo free, **production primary**), Helius `https://mainnet.helius-rpc.com/?api-key=<KEY>` (1M credits/mo @ 10 RPS, **fallback**), QuickNode trial (10M credits @ 15 RPS).

**Endpoint selection rules:**

- Default CLI: Devnet (env `RUN_SOL_DEVNET=1`) — cheapest dev iteration.
- Mainnet: explicit `RUN_SOL_MAINNET=1` gate (deferred to Phase 4 rollout with operator funding).
- Localnet: default for `cargo test` runs; `SurfpoolGuard` subprocess spawn per test.
- Testnet: **do not use** (deprecated, see Testnet section below).
- Chain-id: Solana does NOT use EVM-style chain IDs; cluster identified by URL.

**CLI precedence (highest to lowest):** CLI `--cluster <c>` flag → `SOL_CLUSTER` env var → `sol config set-cluster` persisted in `~/.local/share/sol/config.json` → default `mainnet-beta` (production-first; user must explicitly switch to devnet).

**V0.1 RPC methods used:**

| Method | Use |
| --- | --- |
| `getLatestBlockhash` | fetch fresh blockhash before `sign` (expiry ~60-90 sec) |
| `sendTransaction` | submit signed tx; returns signature (base58); preflight = `true` default |
| `simulateTransaction` | dry-run before broadcast (catches insufficient funds, invalid ix, CU overflow) |
| `getSignatureStatuses` | poll for confirmation (commitment: `confirmed` / `finalized`) |
| `getAccountInfo` | fetch mint decimals, owner verification, ATA state |
| `getMultipleAccountsInfo` | batch mint + ATA fetch |
| `getMinimumBalanceForRentExemption` | pre-fund ATA creation (~0.00204 SOL) |
| `getBalance` | SOL balance (lamports) |
| `getTokenAccountBalance` | SPL token balance (UI units) |
| `getTokenSupply` | mint total supply |
| `getTokenAccountsByOwner` | ATA discovery (V0.1.5 `wallet list-tokens`) |
| `request_airdrop` | devnet/localnet only; mainnet returns `airdrop request failed` |
| `getHealth` | surfpool spawn wait strategy (LOCALNET, not devnet) |
| `accountSubscribe` (WS) | subscribe to ATA state changes (optional) |
| `signatureSubscribe` (WS) | real-time confirmation notification (optional) |
| `programSubscribe` (WS) | watch token-2022 mints for transfer-hook notifications (optional) |

### Testnet (DEPRECATED 2022-23 — do NOT use)

| Field | Status |
| --- | --- |
| URL | `https://api.testnet.solana.com` (still resolves in 2026 but not maintained) |
| Maintenance status | **DEPRECATED by Solana Foundation 2022-23** in favor of devnet |
| Faucet | throttled or discontinued; not reliable for V0.1 development |
| Documentation | Removed from Solana Foundation docs; only mentioned in migration guides |
| Wallet V0.1 status | **EXCLUDED** — `Cluster` enum has `MainnetBeta / Devnet / Localnet` only. No `Testnet` variant. |
| Code paths | Zero. No `SolanaCluster::Testnet`, no `RUN_SOL_TESTNET=1` env var, no testnet URL config. |

**Why deprecated:** Solana Foundation unified testing on devnet because (a) maintaining two test networks added ceremony, (b) testnet's validator uptime was lower than devnet's, (c) most ecosystem tools (Metaplex, Jupiter, Raydium) dropped testnet support in 2022-23 and consolidated on devnet. Devnet has stronger SLAs, more reliable faucet, and is the de-facto standard.

**Migration path for projects still referencing testnet:**

| If you see | Replace with |
| --- | --- |
| `solana airdrop 5 <addr> --url testnet` | `solana airdrop 5 <addr> --url devnet` |
| `https://api.testnet.solana.com` | `https://api.devnet.solana.com` |
| `SolanaCluster::Testnet` (community code) | `SolanaCluster::Devnet` |
| `run_sol_testnet=1` env var | `run_sol_devnet=1` |
| Testnet keypair file | Devnet keypair file (different chain state — must regenerate) |

**Testnet documented for archaeology only.** Do not write new code targeting testnet. V0.1 deliberately omits a testnet variant (legacy testnet endpoints archived; devnet is the de-facto replacement per Solana Foundation 2022-23).

### Mainnet-Beta (V0.1 production, gated)

#### Canonical mainnet mint addresses (verified 2026-09-08)

| Asset | Issuer | Mint address | Token program | Decimals | Token-2022? |
| --- | --- | --- | --- | --- | --- |
| **USDC** | Circle | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | Classic SPL | **6** | No |
| **USDT** | Tether | `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB` | Classic SPL | **6** | No |
| **PYUSD** | Paxos (PayPal) | `2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo` | **Token-2022** | **6** | Yes (May 2024) |
| USDS | Sky Protocol | `USDSwr9ApdHk5bvJKMjzff41FfuX8bSxdKcR81vTwcA` | Classic SPL | 6 | No |
| BONK | Bonk community | `DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263` | Classic SPL | 5 | No |
| JUP | Jupiter | `JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN` | Classic SPL | 6 | No |
| JitoSOL | Jito Foundation | `J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn` | Classic SPL | 9 | No |
| Native SOL | Protocol | `11111111111111111111111111111111` (System Program) | System | **9** | n/a |

**V0.1 must NOT hardcode 6-decimals** — fetch decimals dynamically from `getAccountInfo(mint).data` parsed via `spl-token`/`spl-token-2022` `unpack_mint`.

#### Program IDs (verified)

| Program | Address |
| --- | --- |
| System Program (SOL) | `11111111111111111111111111111111` |
| Classic SPL Token | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` |
| SPL Token-2022 (Extensions) | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` |
| Associated Token Account | `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL` |
| Memo | `MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr` |
| Compute Budget | `ComputeBudget111111111111111111111111111111` |

**Critical ATA footgun:** ATA address = `find_program_address(&[owner, token_program_id, mint], ATA_PROGRAM_ID)`. The `token_program_id` IS a seed — classic SPL ATAs derive with `TokenkegQ...`, Token-2022 ATAs derive with `TokenzQdB...`. Mixing them produces a DIFFERENT address. The wallet must derive the correct ATA based on the mint's owning program (`getAccountInfo(mint).owner`).

#### Rent + transaction fees (verified, stable since 2024)

| Account | Size | Rent-exempt cost | Source |
| --- | --- | --- | --- |
| SPL Token account (classic + Token-2022, mint + token-account layout) | 165 bytes | **2,039,280 lamports ≈ 0.00203928 SOL** | [Solana Cookbook rent](https://solanacookbook.com/docs/core/fees/rent) |
| Mint account (classic) | 82 bytes | ~1.4M lamports | [Solana Rent Calculator](https://rent.solana.com/) |
| Mint account (Token-2022 with extensions) | variable | ~1.4M-5M lamports | same |

Stable across 2024-2026 (rent parameter set by protocol governance). V0.1 funds newly-created ATAs from sender's account — sender pays ~0.00204 SOL per new ATA (one-time, fully recoverable via `close_account`).

| Fee type | Amount | Notes |
| --- | --- | --- |
| Base fee per signature | **5,000 lamports** (0.000005 SOL) | Charged on every `tx.signatures[]` entry. Paid regardless of tx success (lost on failure). |
| Priority fee | **micro-lamports per CU** (0 to 1,000,000 µLamports/CU) | Set via `ComputeBudgetInstruction::set_compute_unit_price`. Multiplied by CU consumed. Local fee market per writable account (50th percentile recommended). |
| Rent (ATA creation) | ~2,039,280 lamports | One-time per ATA |

**Example V0.1 transfer cost (USDC, no priority fee):**

- 1 base sig × 5,000 lamports = **5,000 lamports (~0.000005 SOL)**
- ~5k CU × 0 micro-lamports = 0
- = **~5,000 lamports / tx** (≈ $0.001 at SOL=$200)

With priority fee (10,000 micro-lamports/CU, 5k CU): 10,000 × 5,000 = 50,000,000 micro-lamports = 50,000 lamports + 5,000 base = **55,000 lamports total**.

#### Compute units per instruction (2026, p-token rewrite deployed)

| Operation | Legacy SPL (pre-2026) | Post-p-token (2026) | Token-2022 (with extensions) |
| --- | --- | --- | --- |
| Native SOL transfer (`system_program::transfer`) | ~150 CU | ~150 CU | n/a |
| SPL `transfer` (no extension) | ~150 CU | **~3-5 CU** | n/a |
| SPL `transfer_checked` | ~150 CU | **~3-5 CU** | n/a |
| Token-2022 `transfer_checked` (no extensions) | ~150 CU | n/a | ~150 CU base |
| Token-2022 with Transfer Hook | +5k-20k CU | same | **5k-50k CU** |
| Token-2022 with Confidential Transfers | +20k-100k CU | same | **20k-150k CU** |
| Create + fund new ATA (per recipient) | ~15k CU + rent | same | depends on mint extensions |
| Memo program instruction | ~1k CU | n/a | n/a |

**CU budget caps (raised 2025):**

- Default per instruction: **200,000 CU**
- Max per transaction: **1,400,000 CU** (1.4M, raised from 200k in 2025)

**p-token rewrite (2025-26):** Solana swapped the SPL Token program implementation in-place (same program ID, same `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`) to a Rust rewrite that dropped per-transfer CU from ~150 to ~3-5. For V0.1: assume **~5,000 CU** per simple SPL transfer as safe default; **50,000 CU** when interacting with Token-2022 mints that may have hooks.

#### Token-2022 extensions (V0.1 awareness surface)

- **Transfer Hook** — arbitrary CPI on every transfer (most-used 2025-26)
- **Confidential Transfers** — encrypted balances via Twisted ElGamal
- **Interest-Bearing Tokens** — continuous interest accrual
- **Permanent Delegate** — burn/transfer from any holder (recovery, compliance)
- **Default Account State** — new ATAs initialized frozen/unfrozen by mint policy
- **Transfer Fee** — protocol-level fee on every transfer (basis points)
- **Mint Close Authority** — allow closing the mint itself
- **Non-Transferable** (soulbound) — blocks transfer out
- **Required Memo on Transfer** — force Memo ix in same tx (compliance)
- **Metadata Pointer + Token Metadata** — on-chain metadata
- **Group / Member Pointer** — ERC-1155-style grouping
- **CPI Guard** — block CPIs from this token account (anti-hack)
- **Immutable Owner** — owner cannot be changed

Sources: [USDC — Circle docs](https://developers.circle.com/stablecoins/usdc-on-main-net), [USDT — Tether](https://tether.to), [PYUSD — Solana Foundation](https://solana.com/news/pyusd-paypal-solana-developer), [Token-2022 extensions — Helius](https://www.helius.dev/blog/token-2022), [SPL Token Program — Solana Program Library](https://github.com/solana-program/token), [p-token — Helius Sep 2025](https://www.helius.dev/blog/solana-p-token).

#### Mainnet smoke gate (Q4 analog)

V0.1 release GATED on real-value mainnet self-send.

| Property | Value |
| --- | --- |
| Asset | $0.001 USDC to self (recipient == sender) |
| Network | mainnet-beta |
| Gate | `RUN_SOL_MAINNET=1` env var + pre-check audit hook (refuse if recipient != operator_wallet) |
| Frequency | once per V0.1 release cut, on each candidate RC build |
| Alchemy RPC | `https://solana-mainnet.g.alchemy.com/v2/$ALCHEMY_KEY` recommended for prod stability |
| Success criteria | txid returns via `getSignatureStatuses` with `confirmed` then `finalized` commitment; sender + recipient balances update by 0.001 USDC; no `BlockhashNotFound` or `InsufficientFunds` |
| Failure mode | revert to devnet — investigate locally before retrying mainnet |

**Why only $0.001 USDC:** minimum meaningful value transaction that exercises full pipeline (build + sign + broadcast + confirm + balance query). Below $0.001 USDC the test is "looks like real network"; $0.001+ is "real network" without meaningful operator cost.

**Operator checklist before V0.1 mainnet smoke:**

- [ ] Alchemy API key loaded via `ALCHEMY_API_KEY` env
- [ ] Operator wallet has ≥ 0.01 USDC + 0.01 SOL for fees + rent buffer
- [ ] Recipient == operator wallet (loop-back self-send)
- [ ] `--confirm-mainnet` prompt typed `yes`
- [ ] `RUN_SOL_MAINNET=1` env var set
- [ ] `sol wallet send --to <self> --amount 0.001 --token USDC --wait --wait-finalized --priority-fee 1000`
- [ ] Verify via `sol explorer` or `sol balance --address <self> --token USDC` — balance changed by 0.001 USDC
- [ ] Cleanup: `sol config set-cluster devnet` (return to dev for ongoing work)

### Devnet (V0.1 default test cluster)

#### Devnet cluster details

| Property | Value | Notes |
| --- | --- | --- |
| HTTP RPC | `https://api.devnet.solana.com` | Solana Labs public (heavy rate-limit ~10 RPS) |
| WS RPC | `wss://api.devnet.solana.com` | JSON-RPC over WS |
| Token value | **Test SOL, no real value** | safe for development |
| Token SPLs | test mints (often nil/decimals=9 for memecoin experiments) | NOT the same as mainnet mints |
| Faucet rate | ~5 SOL/airdrop, rate-limited per IP | `solana airdrop 5 <pubkey> --url devnet` |
| Faucet RPC method | `RpcClient::request_airdrop(&pubkey, lamports)` | returns `airdrop_sig`, may take 5-30 sec to confirm |
| Block time | 400ms (matches mainnet) | realistic test bed for blockhash expiry |
| Slot leader rotation | every epoch (~2.5 days on devnet) | not relevant for V0.1 |
| Epoch length | 8192 slots (~54 min) | shorter than mainnet for faster staking tests |
| Validator count | ~150 active (down from mainnet's ~1500) | smaller consensus |
| Reset schedule | **periodic scheduled wipe** (announced via @solaborate on X) | ~quarterly; check before important test runs |
| SPKI pinning | none (rotates certs freely, like mainnet) | skip pinning per Solana Labs policy |
| Cost | **free** | no API key required for ~10 RPS |

#### Devnet RPC endpoints (verified 2026-09-09)

| Endpoint | URL | Use |
| --- | --- | --- |
| **HTTP** | `https://api.devnet.solana.com` | Solana Labs public RPC; ~10 RPS limit; default for V0.1 dev work |
| **WebSocket** | `wss://api.devnet.solana.com` | JSON-RPC over WS (subscriptions for `accountSubscribe`, `signatureSubscribe`, `programSubscribe`) |
| **Health** | `https://api.devnet.solana.com/health` | Returns `{"result":"ok"}` when cluster alive |

For higher RPS (paid/free tier):

- Helius: `https://devnet.helius-rpc.com/?api-key=<KEY>` (paid for higher rate limits)
- Alchemy: `https://solana-devnet.g.alchemy.com/v2/<KEY>` (30M CU/mo free)

#### Canonical devnet stablecoin mints (verified 2026-09-09)

| Asset | Mint address | Token program | Decimals | Faucet source |
| --- | --- | --- | --- | --- |
| **USDC (devnet)** | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` | Classic SPL (`TokenkegQ...`) | **6** | [Circle devnet faucet](https://faucet.circle.com/) |
| **PYUSD (devnet)** | `2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo` (mainnet mint — same address on devnet, no separate deployment) | **Token-2022** (`TokenzQdB...`) | 6 | not directly faucet-funded; deploy own + mint |
| **SOL (devnet)** | `11111111111111111111111111111111` (native) | System | **9** | [Solana Foundation faucet](https://faucet.solana.com/) OR `solana airdrop 5 <addr> --url devnet` OR `RpcClient::request_airdrop` |

**Critical footgun reminder:** the devnet USDC mint `4zMMC...DncDU` is **Circle's test faucet deployment ONLY** — NOT a production token. Wallets querying this mint on mainnet-beta return `{ata: <derived>, exists: false}`. Always verify cluster by URL (`api.devnet.solana.com` vs `api.mainnet-beta.solana.com`) before signing.

#### DevNet faucet procedures

**SOL faucet (test SOL, no value):**

```bash
# CLI method (works without API key, rate-limited per IP)
solana airdrop 5 2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH --url devnet

# CLI method (browser faucet, requires captcha)
# Open: https://faucet.solana.com/
# Click "Devnet" tab → paste address → submit

# Rust method (via solana-client)
let sig = rpc.request_airdrop(&pubkey, 5_000_000_000).await?;
```

Limits: ~5 SOL per airdrop, rate-limited per IP. Airdrop txs confirm in ~5-30 sec on devnet.

**USDC faucet (Circle devnet, test USDC, no value):**

```bash
# Browser-only — Circle requires captcha
# Open: https://faucet.circle.com/
# Click "Solana Devnet" tab → paste address → request 20 USDC
# Token: 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
# Decimals: 6 (recipient receives raw amount 20_000_000)
```

Limits: 20 USDC per request, rate-limited per wallet/IP. The Circle devnet USDC is a separate deployment from mainnet USDC — wallet must verify mint address before expecting the transfer to match mainnet expectations.

**Why Circle devnet faucet, not Solana Foundation:**

- Solana Foundation airdrops only SOL (native)
- Circle maintains a test USDC faucet for testing SPL Token Transfer flows + Token-2022 footer hooks
- PYUSD has no devnet faucet (test by deploying your own Token-2022 mint + minting)
- USDS, USDT: no devnet faucets — create test mints yourself (`sol spl create-mint --cluster devnet`)

#### Devnet RPC method usage pattern (V0.1 wallet)

```rust
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

let rpc = RpcClient::new_with_commitment(
    "https://api.devnet.solana.com",
    CommitmentConfig::confirmed(),
);

// Native SOL balance
let sol_lamports = rpc.get_balance(&wallet_pubkey).await?;
// == 5_000_000_000 (5.0 SOL)

// USDC ATA balance (devnet mint 4zMMC...)
let ata = spl_associated_token_account::get_associated_token_address_with_program_id(
    &wallet_pubkey,
    &usdc_devnet_mint,  // 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
    &spl_token::id(),   // TokenkegQ... (classic SPL)
);
let token_balance = rpc.get_token_account_balance(&ata).await?;
// token_balance.ui_amount == Some(20.0)
```

#### Token-2022 mints on devnet vs mainnet

| Mint | mainnet-beta | devnet |
| --- | --- | --- |
| USDC (`EPjFWdd5...`) | exists, 6 decimals, classic SPL | typically NOT deployed on devnet |
| USDT (`Es9vMFrz...`) | exists, 6 decimals, classic SPL | typically NOT deployed on devnet |
| PYUSD (`2b1kV6Dk...`) | exists, 6 decimals, **Token-2022** | typically NOT deployed on devnet |
| Devnet USDC (test) | n/a | deployed by Solana Foundation for faucet drips; decimals varies |
| Custom test mints | n/a | freely creatable via `sol spl create-mint --cluster devnet` (V0.2+) |

**V0.1 strategy:** for SPL testing, use **devnet test mints** (create your own mint via devnet RPC, or use existing faucet mints). Do NOT assume mainnet USDC/USDT/PYUSD mint addresses exist on devnet. The wallet's `spl balance` will return `{ balance: "0", ata: <derived>, exists: false }` for any mainnet mint queried on devnet — expected behavior.

#### Devnet state lifecycle (scheduled wipes)

Unlike mainnet (which never wipes historical state), devnet undergoes **periodic scheduled wipes** every few quarters (~quarterly cadence, announced via [@solaborate](https://x.com/solaborate) on X). Each wipe:

| What persists | What gets wiped |
| --- | --- |
| Custom program code (deployed BPF) | All account states (lamports, data) |
| Token mints YOU created (if `freeze_authority` = you) | All ATAs (regardless of mint authority) |
| Validator config (rpc-bindings) | All transaction history (slot history) |
| NFT metadata (on-chain) | SOL balances (all wallets go to 0) |

**Operational rules for V0.1 testing on devnet:**

1. Treat devnet as ephemeral — never persist wallets across long time windows
2. After scheduled wipe: re-airdrop SOL + re-airdrop USDC (both faucets work post-wipe)
3. Custom token mints must be re-created (unless you own the freeze authority)
4. Wallet address `2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH` may have different balances between devnet wipes — record balances only at test-time, not as test-fixture
5. V0.1 wallet's `wallet list --cluster devnet` queries are point-in-time snapshots

**V0.1 strategy:** treat devnet as **ephemeral** — never persist wallets across long time windows. Re-airdrop + re-test on wipe. Use localnet (surfpool) for daily test loops; devnet only for cross-cluster conformance + integration smoke.

#### Wallet-verified balance example (2026-09-09)

Test wallet queried via `RpcClient::getBalance` + `getTokenAccountsByOwner`:

| Field | Value |
| --- | --- |
| Wallet (Pubkey) | `2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH` |
| Cluster | **devnet** |
| Slot (at query) | `495,538,052` (SOL RPC) / `495,538,489` (USDC ATA) |
| **SOL balance** | `5,000,000,000` lamports = **5.0 SOL** (test SOL, no value) |
| **USDC ATA address** | `Hhz82iMVQwSACARFinEzn6QR4St1n35z9ZyvRruQzXRv` |
| USDC mint | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` |
| **USDC balance** | `20,000,000` micro-units = **20.0 USDC** |
| USDC ATA rent | `1,488,440` lamports ≈ 0.00149 SOL (variable; less than 2,039,280-lamport default estimate) |

**Explorer link for this wallet:**

- Solana Explorer (devnet): `https://explorer.solana.com/address/2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH?cluster=devnet`
- Solscan (devnet): `https://solscan.io/account/2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH?cluster=devnet`
- SolanaFM (devnet): `https://solana.fm/address/2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH?cluster=devnet`

#### Devnet vs localnet (surfpool) — when to use which

| Use case | Cluster |
| --- | --- |
| Single-test integration (no setup overhead) | **Localnet (surfpool)** — auto-spawn per test |
| Multi-test batch (parallel CI) | **Localnet (surfpool)** — ephemeral port per test |
| Verify against real cluster (wiring check) | **Devnet** |
| Test against Circle USDC or other provided mints | **Devnet** |
| Test mainnet-shaped token interactions before prod | **Devnet** |
| CI smoke gate | **Devnet** — once per build |
| Q4 mainnet smoke | **Mainnet-beta** — `$0.001 USDC self-send` (`RUN_SOL_MAINNET=1`) |

#### Devnet raw URLs cheat-sheet

| Service | URL |
| --- | --- |
| **Solana RPC (HTTP)** | `https://api.devnet.solana.com` |
| **Solana RPC (WS)** | `wss://api.devnet.solana.com` |
| **SOL faucet** | `https://faucet.solana.com/` (Devnet tab) |
| **USDC devnet faucet (Circle)** | `https://faucet.circle.com/` |
| **Solana Explorer (devnet)** | `https://explorer.solana.com/address/<PUBKEY>?cluster=devnet` |
| **Solscan (devnet)** | `https://solscan.io/account/<PUBKEY>?cluster=devnet` |
| **SolanaFM (devnet)** | `https://solana.fm/address/<PUBKEY>?cluster=devnet` |
| **Solana Beach (devnet)** | `https://solanabeach.io/address/<PUBKEY>?cluster=devnet` |
| **Helius devnet dashboard** | `https://dashboard.helius.dev/` (cluster = Devnet) |
| **Alchemy devnet dashboard** | `https://dashboard.alchemy.com/` (Solana → Devnet) |

#### Quick-start: end-to-end devnet test in Rust

```rust
// Cargo.toml dependencies
// solana-sdk = "4.1.0"
// solana-client = "4.2.2"
// solana-program = "4.1.0"
// spl-token = "9.0.0"
// spl-associated-token-account = "8.0.0"

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::LAMPORTS_PER_SOL,
    signature::{Keypair, Signature},
    signer::Signer,
    system_instruction,
    transaction::Transaction,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Generate wallet
    let wallet = Keypair::new();
    println!("Wallet: {}", wallet.pubkey());

    // 2. Connect to devnet
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com",
        CommitmentConfig::confirmed(),
    );

    // 3. Airdrop 5 SOL via CLI or browser (cannot use request_airdrop without faucet PK)
    // $ solana airdrop 5 <wallet> --url devnet
    let balance = rpc.get_balance(&wallet.pubkey()).await?;
    println!("SOL balance: {} ({})", balance, balance as f64 / LAMPORTS_PER_SOL as f64);

    // 4. Build + sign + send native SOL transfer
    let recipient = Keypair::new();
    let ix = system_instruction::transfer(&wallet.pubkey(), &recipient.pubkey(), LAMPORTS_PER_SOL);
    let blockhash = rpc.get_latest_blockhash().await?;
    let tx = Transaction::new_signed_with_payer(
        &[ix], Some(&wallet.pubkey()), &[&wallet], blockhash,
    );
    let sig: Signature = rpc.send_transaction(&tx).await?;
    println!("Transfer sig: {}", sig);

    // 5. Wait for confirmation
    rpc.confirm_transaction(&sig).await?;
    println!("Confirmed.");

    // 6. Get USDC devnet balance (after Circle devnet faucet 20 USDC)
    let usdc_mint: solana_sdk::pubkey::Pubkey = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU".parse()?;
    let ata = spl_associated_token_account::get_associated_token_address_with_program_id(
        &wallet.pubkey(), &usdc_mint, &spl_token::id(),
    );
    let token_balance = rpc.get_token_account_balance(&ata).await?;
    println!("USDC balance: {} (decimals={})", token_balance.ui_amount_string, token_balance.decimals);

    Ok(())
}
```

### Localnet (V0.1 default — surfpool)

#### Local cluster comparison (V0.1 vs V0.1.5+)

| Property | Localnet via `surfpool` (V0.1) | Localnet via `solana-test-validator` (V0.1.5 opt-in) |
| --- | --- | --- |
| Boot | <2 sec | ~10 sec |
| Disk footprint | 0 (in-memory) | ~500 MB (Agave ledger per test) |
| Install | `cargo install surfpool` | `agave-install init 2.1.x` |
| RPC parity | 100% (auto-advance blockhash every 400ms) | 100% (real validator) |
| Custom BPF programs | not supported | `--bpf-program <KEY> <SO>` (future Token-2022 mint tests) |
| Reset | `surfpool reset` (runtime) | kill + restart process |
| Warp slot | `surfpool warp --slot N` | startup only (`--warp-slot N`) |
| Faucet | built-in (`--faucet 1000000000000`) | `--faucet` flag + `solana airdrop` CLI |
| Cluster identity | implicit "local" | implicit "local" |

**Recommendation:** Use `surfpool` for all V0.1 tests. Opt-in `solana-test-validator` only when BPF program loading or epoch boundary manipulation needed (V0.1.5 stake / Token-2022 hook CPI tests). `solana-test-validator` and Docker-based validators are explicitly out-of-scope for V0.1 — they were superseded 2026-09-08.

#### `surfpool` deep-dive (CHOSEN for V0.1)

| Feature | `surfpool` behavior |
| --- | --- |
| Boot time | <2 sec |
| In-memory | yes |
| JSON-RPC HTTP | `:8899` |
| JSON-RPC WS | `:8900` |
| `getLatestBlockhash` auto-advance | yes (every 400ms — matches Solana 400ms slot time) |
| Fork from mainnet/devnet | `surfpool start --clone <ACCOUNT> --url <URL>` |
| Reset state | `surfpool reset` (runtime, mid-test) |
| Impersonate account | `surfpool impersonate <addr>` |
| Set block time | `--block-time <ms>` (millisecond precision) |
| Time warp | `--warp-slot <SLOT>` (skip to historical slot) |
| Faucet / airdrop | `surfpool airdrop <lamports> <addr>` (CLI subcommand, built-in) |
| Wallet file at startup | `--account <KEY>=<file>` (pre-populate from JSON) |
| Slots per epoch | auto (4000 default; configurable) |
| Stake pool / BPF loading | not supported (use `solana-test-validator` for these — V0.1.5 opt-in) |
| CLI flags stability | stable (2025+) |
| Source | Rust (`git clone` + `cargo install surfpool --locked`) |
| Maintenance | active (txtx, 2025+) |
| GitHub | <https://github.com/txtx/surfpool> |

**Verdict (locked 2026-09-08):** `surfpool` is the **chosen** V0.1 local validator. `solana-test-validator` is kept as V0.1.5 opt-in for tests requiring BPF program loading or epoch boundary control. Reasons surfpool wins over solana-test-validator for V0.1:

- **5x faster boot** (<2s vs ~10s) — CI loops complete in seconds
- **Smaller install** (~50 MB single binary vs ~500 MB full Agave toolchain)
- **Simpler CI** — `cargo install surfpool --locked` vs `agave-install init 2.1.16` + multiple binaries on PATH
- **Runtime reset** — `surfpool reset` mid-test vs solana-test-validator requires restart
- **Runtime warp slot + impersonate** — surfpool only (solana-test-validator = startup only)
- **No Docker dependency** — runs in any sandbox/CI without daemon

Surfpool is a single binary spawned as a subprocess (no Docker, no daemon). Sub-second spawn + ephemeral port per test = parallel-safe CI out of the box.

V0.1 uses `surfpool` exclusively. V0.1.5 adds `solana-test-validator` as opt-in via `#[ignore]` gated tests for BPF/epoch edge cases.

#### Local validator — `surfpool` setup (V0.1 default)

`surfpool` ([txtx/surfpool](https://github.com/txtx/surfpool), 2025+) is the in-memory single-binary local validator for Solana — spawned as subprocess, no Docker. Default local validator for V0.1.

**Install (one-time per developer workstation + CI):**

```bash
# Dev
cargo install surfpool --locked

# Verify
surfpool --version

# CI alternative — download prebuilt binary
curl -fsSL https://github.com/txtx/surfpool/releases/latest/download/surfpool-linux-x86_64.tar.gz | tar xz
sudo mv surfpool /usr/local/bin/
```

No `surfpool = "..."` in `Cargo.toml` — surfpool ships as binary. Rust code uses `tokio::process::Command` + `reqwest::blocking::Client` for health poll.

**`SurfpoolGuard` RAII wrapper** (in `rust-wallet-app/crates/sol-wallet-core/tests/common/surfpool_guard.rs`):

```rust
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct SurfpoolGuard {
    child: Child,
    pub rpc_url: String,
    pub ws_url: String,
    pub ws_pubsub_url: String,
}

impl SurfpoolGuard {
    pub fn spawn() -> Self {
        let port = ephemeral_port();
        let ws_port = port + 1;
        let child = Command::new("surfpool")
            .args(["start",
                "--port", &port.to_string(),
                "--ws-port", &ws_port.to_string(),
                "--bind-address", "127.0.0.1",
                "--reset",
                "--faucet", "1000000000000",  // 1000 SOL built-in
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("surfpool binary not on PATH. Run: cargo install surfpool --locked");

        let rpc_url = format!("http://127.0.0.1:{}", port);
        let ws_url = format!("ws://127.0.0.1:{}", ws_port);
        let ws_pubsub_url = ws_url.clone();

        // Wait for /health (max 10s — surfpool boots <2s normally)
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2)).build().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(resp) = client.get(format!("{}/health", rpc_url)).send() {
                if resp.text().unwrap_or_default().contains("\"ok\"") { break; }
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Self { child, rpc_url, ws_url, ws_pubsub_url }
    }

    /// CLI helper — fund test wallet from surfpool's built-in faucet.
    pub fn airdrop(&self, lamports: u64, recipient: &str) {
        Command::new("surfpool")
            .args(["airdrop", &lamports.to_string(), recipient])
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().expect("surfpool airdrop failed");
    }
}

impl Drop for SurfpoolGuard {
    fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); }
}

fn ephemeral_port() -> u16 {
    use std::net::{TcpListener, SocketAddr};
    let l = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}
```

**Test pattern:**

```rust
#[tokio::test]
async fn v6_blockhash_refresh_succeeds_via_surfpool() {
    let guard = SurfpoolGuard::spawn();     // <2s boot
    let rpc = RpcClient::new(guard.rpc_url.clone());

    let sender = Keypair::new();
    guard.airdrop(10_000_000_000, &sender.pubkey().to_string());

    let recipient = Keypair::new().pubkey();
    let bh = rpc.get_latest_blockhash().await.unwrap();
    let ix = system_instruction::transfer(&sender.pubkey(), &recipient, 1_000_000_000);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&sender.pubkey()), &[&sender], bh);
    let sig = rpc.send_transaction(&tx).await.unwrap();

    assert_eq!(rpc.get_balance(&recipient).await.unwrap(), 1_000_000_000);
}
```

**`surfpool` admin commands used:**

| Action | Command |
|---|---|
| Start node | `surfpool start --port N --ws-port N --reset --faucet 1000000000000` |
| Airdrop SOL | `surfpool airdrop <lamports> <addr>` |
| Reset state | `surfpool reset` (runtime) |
| Impersonate account | `surfpool impersonate <addr>` |
| Warp slot | `surfpool warp --slot N` |

**Why surfpool over testcontainers + Docker (DEPRECATED 2026-09-08):**

- **No Docker dependency** — runs in any sandbox/CI without daemon, no `dockerd` permission requirements
- **5x faster CI loops** — <2s spawn vs 15-30s container pull + start
- **Smaller install** — `cargo install surfpool` (~50 MB single binary) vs Agave Docker image (~500 MB)
- **Parallel-safe** — ephemeral port per test, no container collisions
- **Runtime reset** — `surfpool reset` between tests vs container recreate
- **Single-binary subprocess** — same shape as the other rust-wallet-app wallet crates' local-node pattern

### RPC provider matrix (V0.1)

| Provider | Mainnet-Beta URL | Devnet URL | Free tier | Recommended for V0.1 |
| --- | --- | --- | --- | --- |
| **Solana Labs (default)** | `https://api.mainnet-beta.solana.com` | `https://api.devnet.solana.com` | yes (~10 RPS) | dev work, fallback prod |
| **Alchemy** | `https://solana-mainnet.g.alchemy.com/v2/<KEY>` | `https://solana-devnet.g.alchemy.com/v2/<KEY>` | **30M CU/mo** (most generous) | **production primary** |
| **Helius** | `https://mainnet.helius-rpc.com/?api-key=<KEY>` | `https://devnet.helius-rpc.com/?api-key=<KEY>` | 1M credits/mo @ 10 RPS | production alternative |
| **QuickNode** | `https://solana-mainnet.g.quicknode.com/<HASH>` | `https://solana-devnet.g.quicknode.com/<HASH>` | 1-month trial: 10M credits | dev/staging |
| **Triton** | `https://solanamainnet.triton.one/key/<KEY>` | n/a | none (paid only) | production paid tier |
| **Ankr** | `https://rpc.ankr.com/solana` | n/a | public free tier | dev/fallback only |

**V0.1 default:** Solana Labs public RPC for dev + CI. Alchemy for production. Helius as fallback.

**Cluster-specific URL configuration:**

```bash
# Set mainnet RPC to Alchemy
sol config set-rpc https://solana-mainnet.g.alchemy.com/v2/$ALCHEMY_KEY --for-cluster mainnet-beta

# Set devnet RPC to Solana Labs (default)
sol config set-rpc https://api.devnet.solana.com --for-cluster devnet

# Switch to devnet cluster
sol config set-cluster devnet

# Verify
sol config show
# → { cluster: "devnet", rpc_url: "https://api.devnet.solana.com", spki_pin: null, ... }
```

Per-cluster RPC URL persists in `~/.local/share/sol/config.json` (PAL-bound platform data dir).

### Sources

- Solana Labs public RPC: <https://api.mainnet-beta.solana.com>
- Solana Devnet RPC: <https://api.devnet.solana.com>
- Solana Testnet RPC (DEPRECATED): <https://api.testnet.solana.com>
- Solana clusters doc: <https://solana.com/docs/references/clusters>
- Solana Testnet deprecation: <https://docs.anza.xyz/cluster/testnet> (migrated to devnet)
- Solana Cookbook — Airdrops & Faucets: <https://solanacookbook.com/docs/development/airdrops-and-faucets>
- Helius free tier: <https://www.helius.dev/docs/api-reference/endpoints>
- Alchemy Solana free tier: <https://www.alchemy.com/overviews/solana-rpc>
- QuickNode Solana trial: <https://www.quicknode.com/blog/best-solana-rpc-providers-2026>
- Solana Foundation devnet announcement: <https://solana.com/docs/references/clusters>
- Circle Solana devnet USDC details: <https://developers.circle.com/stablecoins/usdc-on-main-net>
- Solana Explorer devnet: <https://explorer.solana.com/?cluster=devnet>
- Solscan devnet: <https://solscan.io/?cluster=devnet>
- Verified wallet: `2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH` on devnet (queried 2026-09-09 via public RPC, slot 495M, balance: 5 SOL + 20 USDC)

## Rejected crates, SDKs

| Crate / Repo | Version | Reason for rejection |
| --- | --- | --- |
| `metaplex-foundation/mpl-token-metadata` | 5.1.1 | **LICENSE BLOCKER** — `Metaplex NFT Open Source License v1.0` is non-OSI with commercial restrictions. Defer NFT support until legal review. |
| `metaplex-foundation/mpl-core` | 0.12.1 | Same Metaplex license blocker. Defer. |
| `jito-labs/jito-rust-rpc` | 0.3.2 | Stale (last release 2025-06-21). Gate behind `jito` Cargo feature if tip routing needed; default OFF. |
| `otter-sec/anchor` | 1.2.0 | Backup, but adds ~6 MB + IDL codegen step. Not needed for direct SOL + SPL transfers in v0.1. Re-evaluate if on-chain program IDL ergonomics required. |
| `CRossel87a/solana-light-client` | 0.1.0 | Too immature (~30 all-time downloads). Ignore. |
| `iqlusioninc/crates::bip32` | 0.5.3 | **secp256k1 ONLY** — not usable for SOL (Ed25519). Use `ed25519-bip32` 0.4.3 (SLIP-0010). |
| `qntx/kobe` (Jito MEV research) | n/a | Not a wallet lib; name collision. N/A. |
| `SergioBenitez/Figment` | 0.10.19 | Config-only, not crypto. Optional dep, not in v0.1. |
| `mockall` / `wiremock` (in-process RPC trait mocks) | n/a | **REJECTED 2026-09-08** — prefer real local node (`surfpool` subprocess) over trait mocks. Catches RPC contract regressions mocks miss. |
| `figment` | 0.10.19 | Config-only, not crypto. Optional dep, not in v0.1. |

## Mnemonic-to-broadcast data flow (end-to-end)

```text
sol-wallet-core::Wallet::fromMnemonicAt(phrase, account, address_index)
    → bip39::Mnemonic::from_phrase(phrase, English)
    → bip39::Seed::new(&m, "") -> [u8; 64]
    → ed25519-bip32::XPrv::from_seed(seed)         // SLIP-0010 master
    → XPrv::derive("m/44'/501'/{account}'/0'/{address_index}")  // Phantom convention, numeric
    → XPrv::public_key() -> [u8; 32]                // Ed25519 verification key
    → Pubkey::new_from_array(pk_bytes)              // wraps in solana_pubkey
    → pk.to_string() -> base58 address              // display

sol-wallet-core::tx::sign::sign_sol_transfer(keypair, recipient, lamports)
    → RpcClient::get_latest_blockhash()             // fresh blockhash (~60-90s lifetime)
    → system_instruction::transfer(&from, &to, lamports)
    → ComputeBudgetInstruction::set_compute_unit_limit(150_000)
    → Message::new_with_blockhash(&[ix, cu_ix], Some(&from), &blockhash)
    → Transaction::new_unsigned(message)
    → tx.sign(keypair, blockhash)                   // Ed25519 over msg
    → tx.verify()                                   // sanity check
    → tx.to_base64()                                // wire format
    → RpcClient::send_transaction(&tx)              // HTTP RPC
    → sig: Signature (base58)                       // returned by RPC
    → RpcClient::get_signature_statuses(&[sig])     // poll for confirmation
    → commitment: confirmed (1 slot) / finalized (~12 slots)
```

**SLIP-0010 derivation path:** `m/44'/501'/0'/0'/0'` (purpose 44', coin 501', account 0', change 0', address_index 0'). Per [Solana Cookbook](https://solanacookbook.com/docs/wallets/hd-wallet) and Phantom/Solflare wallet convention. Hardened only at `44'`, `501'`, `0'` — non-hardened at `0'` and final index.

**Two crates cooperate:** BIP-39 owns mnemonic→seed (PBKDF2-HMAC-SHA512, 2048 rounds); `ed25519-bip32` owns seed→Ed25519 keypair (SLIP-0010 chain key derivation, hardened-only via `'` prefix).

## Network + TLS pinning research (mirrors eth design)

**Solana Labs public RPC pinning policy:** **NO** — they rotate certs freely. Do not hard-pin; re-validate on cert rotation.

**Helius / QuickNode / Triton / Alchemy:** do NOT publish formal SPKI pin sets. All use standard ACM (AWS) / Cloudflare / Google Cloud issued certs with default CA rotation.

**SPKI pin recommendation (Q7, Scenario A):**

- For Solana, **skip pinning by default**. Use cert transparency logs + hostname verification (default rustls behavior) as primary defense.
- Reserve SPKI pinning for hardened-wallet flows where user explicitly imports the pin via QR code (BlueWallet/BitBox pattern).
- Reuse the `SpkiPinnedVerifier` from `bitcoin-wallet-core/src/chain/spki.rs` (generic) — for Solana, parameterize the pin set per endpoint family (Helius vs QuickNode vs Triton vs Cloudflare-edge vs AWS-edge). Each gets its own pin set; failed TLS handshake on pin mismatch should fall back to a captured-pin update path (rare but happens during CA rolls).

**Agave validator RPC (local):** `http://127.0.0.1:8899` (plain HTTP, no TLS). SPKI pin not applicable.

**WebSocket security:** `solana-client::PubsubClient` uses `tungstenite` (pure Rust WS) with the same `rustls` verifier as HTTP. Same SPKI pin reuse pattern.

Sources: [Helius docs](https://www.helius.dev/docs/api-reference/endpoints), [QuickNode docs](https://www.quicknode.com/docs/solana), [rustls webpki verifier](https://github.com/rustls/webpki), Bitcoin SPKI pin pattern source: `bitcoin-wallet-core/src/chain/spki.rs`.

## Zeroizing<_>

Wrap raw secret material in `Zeroizing<Vec<u8>>` before any sign call.

**Known gaps in Anza stack:**

| Gap | Mitigation in sol-wallet-core |
| --- | --- |
| `Keypair::from_seed(seed: &[u8])` takes raw slice — no Zeroize on input | wrap seed in `Zeroizing<Vec<u8>>` before call; copy out 32-byte secret, drop Zeroizing immediately |
| `ed25519-bip32::XPrv::to_string()` returns `String` — no Zeroize | wrap result in `Zeroizing<String>`; never log XPrv string |
| `bip39::Seed::as_bytes()` returns `&[u8]` — no Zeroize on the parent | Zeroize-wrap the master seed at construction; keep the original alive in a scoped `Zeroizing<Vec<u8>>` |
| `Keypair::to_bytes()` returns `[u8; 64]` — no Zeroize on output | wrap result in `Zeroizing<[u8; 64]>`; `copy_from_slice` to disk, drop immediately |
| `SystemTime::now()` in tx `Message` does NOT touch secret material | n/a — public |

**2 of 5 secrets require wallet-local Zeroize wrapper; Anza core libs (Keypair, XPrv) provide Zeroize-on-drop but not on input params.** Caller's responsibility.

Sources: [solana-keypair Rust API](https://docs.rs/solana-keypair/3.1.2), [ed25519-bip32 Rust API](https://docs.rs/ed25519-bip32/0.4.3), [bip39 Rust API](https://docs.rs/bip39/).

## Solana program feature map

Per-crate feature surface for the 4 SPL-supporting crates + the 10 Anza subcrates.

### `solana-sdk` 4.1.0 (PRIMARY — wire format + signer)

| # | Feature | API |
| --- | --- | --- |
| 1 | Ed25519 keypair generate (random) | `Keypair::new()` |
| 2 | Ed25519 keypair from seed | `Keypair::from_seed(seed: &[u8; 32])` |
| 3 | Ed25519 keypair from base58 secret | `Keypair::from_base58_string(s: &str)` |
| 4 | Sign 64-byte message | `keypair.sign_message(msg: &[u8]) -> Signature` |
| 5 | Verify Ed25519 signature | `signature.verify(pubkey, msg) -> bool` |
| 6 | Pubkey from bytes | `Pubkey::new_from_array(bytes: [u8; 32])` |
| 7 | Pubkey from base58 | `Pubkey::from_str(s: &str)` |
| 8 | Pubkey display (base58) | `pubkey.to_string()` |
| 9 | Pubkey short display | `pubkey.short()` (first 4 + last 4 chars) |
| 10 | Pubkey to bytes | `pubkey.to_bytes()` |
| 11 | `is_on_curve` check | `Pubkey::is_on_curve(&bytes)` (Ed25519 valid only) |
| 12 | `find_program_address` (PDA) | `Pubkey::find_program_address(seeds, program_id) -> (Pubkey, u8)` |
| 13 | `create_program_address` (non-PDA) | `Pubkey::create_program_address(seeds, program_id) -> Option<Pubkey>` |
| 14 | System transfer (native SOL) | `system_instruction::transfer(from, to, lamports)` |
| 15 | System create_account | `system_instruction::create_account(from, to, lamports, space, owner)` |
| 16 | Message (legacy) | `Message::new(&ixs, Some(&payer))` |
| 17 | Message with blockhash | `Message::new_with_blockhash(&ixs, Some(&payer), &blockhash)` |
| 18 | Versioned message (v0) | `VersionedMessage::V0(v0::Message::new(...))` |
| 19 | Transaction (legacy) | `Transaction::new(&[&keypair], message, blockhash)` |
| 20 | Versioned transaction | `VersionedTransaction::new(versioned_message, &[&keypair])` |
| 21 | Address lookup table (ALTs) | `address_lookup_table::state::AddressLookupTable` + `MessageAddressTableLookup` |
| 22 | Transaction to base64 wire | `tx.to_base64()` |
| 23 | Transaction from base64 wire | `VersionedTransaction::try_from(base64)` |
| 24 | Signature display | `signature.to_string()` (base58) |

### `solana-client` 4.2.2 (PRIMARY — RPC client)

| # | Feature | API |
| --- | --- | --- |
| 1 | HTTP RPC client | `RpcClient::new(url)` + `RpcClient::new_with_commitment(url, commitment)` |
| 2 | `getLatestBlockhash` | `rpc.get_latest_blockhash()` |
| 3 | `sendTransaction` | `rpc.send_transaction(&tx)` + `send_transaction_with_config(&tx, config)` |
| 4 | `simulateTransaction` | `rpc.simulate_transaction(&tx)` |
| 5 | `getSignatureStatuses` | `rpc.get_signature_statuses(&[sig])` |
| 6 | `getAccountInfo` | `rpc.get_account(&pubkey)` + `get_account_with_config` |
| 7 | `getMultipleAccountsInfo` | `rpc.get_multiple_accounts(&[pk1, pk2])` |
| 8 | `getMinimumBalanceForRentExemption` | `rpc.get_minimum_balance_for_rent_exemption(size)` |
| 9 | `getBalance` | `rpc.get_balance(&pubkey)` |
| 10 | `getTokenAccountBalance` | `rpc.get_token_account_balance(&ata)` |
| 11 | `getTokenSupply` | `rpc.get_token_supply(&mint)` |
| 12 | `requestAirDrop` (devnet only) | `rpc.request_airdrop(&pubkey, lamports)` |
| 13 | WebSocket pubsub client | `PubsubClient::new(url).await` |
| 14 | `accountSubscribe` | `pubsub.account_subscribe(pubkey, config)` |
| 15 | `signatureSubscribe` | `pubsub.signature_subscribe(sig, config)` |
| 16 | `programSubscribe` | `pubsub.program_subscribe(program_id, config)` |
| 17 | `logsSubscribe` | `pubsub.logs_subscribe(filter)` |
| 18 | `slotSubscribe` | `pubsub.slot_subscribe()` |
| 19 | `blockSubscribe` | `pubsub.block_subscribe(filter)` |
| 20 | `voteSubscribe` / `rootSubscribe` | staking dashboards |

### `solana-compute-budget-program` 4.2.2 (PRIMARY — CU + priority fee)

| # | Feature | API |
| --- | --- | --- |
| 1 | Set CU limit | `ComputeBudgetInstruction::set_compute_unit_limit(units: u32)` |
| 2 | Set CU price (priority fee) | `ComputeBudgetInstruction::set_compute_unit_price(micro_lamports: u64)` |
| 3 | Request heap frame | `ComputeBudgetInstruction::request_heap_frame(bytes: u32)` |
| 4 | Loaded accounts data size limit | `ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(bytes: u32)` |

### `spl-token` 9.0.0 (PRIMARY — classic SPL)

| # | Feature | API |
| --- | --- | --- |
| 1 | Mint derive | `spl_token::state::Mint::unpack(&account.data)` |
| 2 | Token account unpack | `spl_token::state::Account::unpack(&account.data)` |
| 3 | `transfer_checked` ix | `spl_token::instruction::transfer_checked(token_program, source, mint, dest, authority, signer, amount, decimals)` |
| 4 | `transfer` (unchecked) ix | `spl_token::instruction::transfer(token_program, source, dest, authority, signer, amount)` |
| 5 | `mint_to` ix | `spl_token::instruction::mint_to(token_program, mint, dest, authority, signer, amount)` |
| 6 | `burn` ix | `spl_token::instruction::burn(token_program, account, mint, authority, signer, amount)` |
| 7 | `approve` ix | `spl_token::instruction::approve(token_program, source, delegate, owner, signer, amount)` |
| 8 | `close_account` ix | `spl_token::instruction::close_account(token_program, account, dest, owner, signer)` |
| 9 | `set_authority` ix | `spl_token::instruction::set_authority(token_program, account, authority, new_authority, authority_type, signer)` |
| 10 | `create_mint` helper | `spl_token::instruction::create_mint(token_program, mint, authority, freeze_authority, signer, decimals)` |
| 11 | `sync_native` ix | wrap native SOL in ATAs (for native SOL wrapping) |
| 12 | Token account state enum | `spl_token::state::AccountState` (Uninitialized / Initialized / Frozen) |

### `spl-token-2022` 11.0.0 (SUPPORTING — Token Extensions)

| # | Feature | API |
| --- | --- | --- |
| 1 | All classic `spl-token` features (1-12) | re-exported + ext-aware |
| 2 | Extension-aware `transfer_checked` | same signature, dispatches to extension logic |
| 3 | Extension state | `spl_token_2022::extension::StateWithExtensions` |
| 4 | Transfer Hook ix | `transfer_hook::instruction::execute` (called by transfer ix) |
| 5 | Confidential Transfer ix | `confidential_transfer::instruction::configure_account` etc. |
| 6 | Interest-Bearing config | `interest_bearing_mint::instruction::set_rate` |
| 7 | Permanent Delegate config | `permanent_delegate::instruction::set` |
| 8 | Default Account State | `default_account_state::instruction::set` |
| 9 | Transfer Fee config | `transfer_fee::instruction::set_fee` |
| 10 | Memo required | `memo_transfer::instruction::enable` |
| 11 | CPI Guard | `cpi_guard::instruction::enable` |
| 12 | Immutable Owner | `immutable_owner::instruction::set` |

### `spl-associated-token-account` 8.0.0 (PRIMARY — ATA)

| # | Feature | API |
| --- | --- | --- |
| 1 | Derive ATA | `get_associated_token_address(wallet, mint)` |
| 2 | Derive ATA (ext-aware) | `get_associated_token_address_with_program_id(wallet, mint, token_program_id)` |
| 3 | Create ATA ix | `create_associated_token_account(payer, wallet, mint)` |
| 4 | Create ATA idempotent | `create_associated_token_account_idempotent(payer, wallet, mint)` |

### `spl-memo` 7.0.0 (SUPPORTING — memo)

| # | Feature | API |
| --- | --- | --- |
| 1 | Memo ix | `spl_memo::build_memo(memo: &str)` |
| 2 | Required memo verify | verifies all accounts listed in ix are signers |

### `ed25519-bip32` 0.4.3 (PRIMARY — HD)

| # | Feature | API |
| --- | --- | --- |
| 1 | Master xprv from seed | `XPrv::from_seed(seed: &[u8; 32])` |
| 2 | Derive child (hardened) | `xprv.derive_child(CKDPriv::Hardened(idx))` |
| 3 | Derive child (non-hardened) | `xprv.derive_child(CKDPriv::Normal(idx))` |
| 4 | Public key | `xprv.public_key() -> [u8; 32]` |
| 5 | Sign | `xprv.sign(msg: &[u8]) -> [u8; 64]` |
| 6 | SLIP-0010 chain key | internal `ChainCode` (HMAC-SHA512) |
| 7 | Derive from path string | `XPrv::derive(path: &str)` → supports `m/44'/501'/0'/0'/0'` |

### `bip39` (English only) (PRIMARY — mnemonic)

| # | Feature | API |
| --- | --- | --- |
| 1 | Generate 12-word mnemonic | `Mnemonic::generate_in(Language::English, 12)` |
| 2 | Generate 24-word mnemonic | `Mnemonic::generate_in(Language::English, 24)` |
| 3 | Validate + import phrase | `Mnemonic::from_phrase(phrase, Language::English)` |
| 4 | Master seed (PBKDF2-HMAC-SHA512, 2048 rounds, NFKD-normalized) | `Seed::new(&m, "")` |
| 5 | Wordlist (English only, ~4 KB) | `Language::English.word_list()` |

### NOT covered (caller must provide)

| Need | Why missing from Anza |
| --- | --- |
| Wallet file encryption (Argon2id + AES-GCM) | Anza has zero persistence layer |
| Token registry JSON | no SDK, application config |
| SPKI pin verifier | reuse `bitcoin-wallet-core::chain::spki` (cross-chain helper) |
| Tx history query | RPC, caller queries `getSignaturesForAddress` + `getTransaction` |
| Token account discovery (own ATAs) | RPC, caller queries `getTokenAccountsByOwner` |
| Blockhash refresh retry logic | caller wraps `get_latest_blockhash` + `send_transaction` with backoff |
| Durable nonce flow | caller assembles `advance_nonce_account` ix + state verify |
| Jito tip routing | external (`jito-sdk-rust` 0.3.2 — gate behind `jito` feature) |
| NFT program (Metaplex) | **EXCLUDED** — Metaplex license blocker |

### Total coverage

- **solana-sdk: 24 wire-format + signer features** (all Ed25519 + Pubkey + Message + Transaction)
- **solana-client: 20 RPC + WS features** (all HTTP + subscription methods)
- **solana-compute-budget-program: 4 CU/priority features**
- **spl-token: 12 classic SPL features** (all wire-format ix)
- **spl-token-2022: 12 extension features** (re-exports classic + ext-aware)
- **spl-associated-token-account: 4 ATA features**
- **spl-memo: 2 memo features**
- **ed25519-bip32: 7 HD features**
- **bip39: 5 mnemonic features**

**Net: 90 features across 9 crates cover ~80% of SOL + SPL wallet functionality.** Caller adds ~300 lines of RPC glue (blockhash refresh, confirmation polling, ATA discovery) + wallet encryption (Argon2id + AES-GCM).

## Solana Wallet V0.1

User-facing features for `sol-wallet-core v0.1.0` + `sol` CLI v0.1.0 release cut. **Must compile on desktop (Linux/macOS/Windows) + mobile (iOS arm64 + Android arm64) with no source changes** — pure Rust + 4-trait Platform Abstraction Layer. 22 commands across 6 top-level commands (`wallet` 9, `address` 2, `balance` 2, `spl` 4, `tx` 2, `config` 3). Each row maps a feature to the story id (TBD in `wallets/2026-09-08-sol-wallet-user-stories.md` per L13 step 0.4) + the `sol-wallet-core` call + CLI command + status.

**Cluster coverage (Solana):**

| Solana variant | RPC URL | Notes |
| --- | --- | --- |
| `SolanaCluster::MainnetBeta` | `https://api.mainnet-beta.solana.com` | production |
| `SolanaCluster::Devnet` | `https://api.devnet.solana.com` | primary test cluster (testnet DEPRECATED) |
| `SolanaCluster::Localnet` | `http://127.0.0.1:8899` (auto-allocated port) | spawned by `surfpool` per test (ephemeral port for parallel-safety) |
| `RUN_SOL_DEVNET=1` env gate | n/a | enable loud-RED live tests against real devnet |
| `RUN_SOL_MAINNET=1` env gate | n/a | Q4 mainnet self-send gate ($0.001 USDC self-send) |

### `sol wallet` (9 commands)

| CLI command | Story | sol-wallet-core call | Status |
| --- | --- | --- | --- |
| `wallet create --words 12\|24 --name --cluster --password [--account <N>] [--address-index <N>]` | 1 | `WalletManager::create_with_mnemonic` (defaults to account=0, address-index=0 per Phantom convention) | ready |
| `wallet import --name --cluster --password --mnemonic\|--mnemonic-file\|--private-key-file [--account <N>] [--address-index <N>]` | 2 | `WalletManager::import_from_phrase` or `import_from_pk` (defaults to account=0, address-index=0) | ready |
| `wallet show --id [--json]` | 11 | `WalletManager::unlock(id, pw).summary()` | ready |
| `wallet list [--json] [--all-clusters]` | 9 | `WalletManager::list()` | ready |
| `wallet delete --id` | 9 | `WalletManager::delete(id)` | ready |
| `wallet rename --id --to` | 9 | `WalletManager::rename(id, name)` | ready |
| `wallet balance --wallet-id [--token USDC\|<addr>\|USDC-SPL] \| --address [--token <addr>]` | 3, 22 | `chain::get_balance(addr)` or `WalletManager::unlock(id).balance()` | ready |
| `wallet send --wallet-id\|--mnemonic --to <addr>\|--to-wallet <name\|id> --amount [--unit] [--priority-fee] [--dry-run] [--sign-only] [--wait]` | 5 | `tx::submit_sol(sk, to, lamports, priority_fee, &cfg)` | ready |
| `wallet send-speedup --wallet-id --sig --priority-fee` | 17 | `tx::submit_send_speedup(...)` — emits *new* sig (no RBF on Solana) | ready |

**Mnemonic handling (L28 / F49 / L12 H-1):**

- `wallet create` → mnemonic → STDERR (red highlight); wallet_id → STDOUT
- `wallet import --mnemonic` → mnemonic → STDERR; wallet_id → STDOUT
- `wallet import --mnemonic-file` → reads mode-0600 file (closes argv-exposure L12 H-1)
- `wallet show` → decrypted mnemonic NEVER displayed; only address + balance

**Wallet-to-wallet transfer pattern (SOL equivalent):**

```bash
# Transfer 0.1 SOL from "trading" wallet to "savings" wallet (both stored locally)
sol wallet send --wallet-id <trading-uuid> --to-wallet savings --amount 0.1 --unit sol

# Transfer 100 USDC from "hot" to "cold" wallet
sol wallet send --wallet-id <hot-uuid> --to-wallet cold --amount 100 --token USDC
```

- `--to <addr>` accepts a base58 Ed25519 pubkey (32-44 chars)
- `--to-wallet <name|id>` accepts a stored wallet name or UUID; resolves via `WalletManager::lookup(name_or_id).address`
- `--to` and `--to-wallet` are mutually exclusive (clap `conflicts_with`)
- For SPL wallet-to-wallet, add `--token USDC|<mint>` flag (re-uses spl builder + ATA create)

**Solana-specific wallet send shape:** See `## Solana fee model + sol-wallet-core integration` section for full fee + Compute Budget + retry semantics. Solana sends differ from Tron: pre-declared Compute Budget (no gas metering), fresh blockhash required per retry (Ed25519 signs blockhash), send-speedup emits a new tx (no RBF).

### `sol address` (2 commands)

| CLI command | Story | sol-wallet-core call | Status |
| --- | --- | --- | --- |
| `address new --mnemonic [--mnemonic-file] --account <N> --address-index <N>` | 3 | `Wallet::fromMnemonicAt(phrase, account, address_index)` (Phantom convention: `m/44'/501'/{account}'/0'/{address_index}`) | ready |
| `address pubkey --wallet-id` | 19 | `WalletManager::pubkey(id)` (Ed25519 verification key, base58) | ready |

**NOTE — `xpub` rename for Ed25519:** Solana does NOT expose `address xpub` because Ed25519 HD (SLIP-0010) does NOT support parent public key derivation (no `xpub` parent chain code in Ed25519 scheme). The Solana equivalent is `address pubkey` — returns the leaf verification key (32 bytes) as base58. Watch-only import via pubkey alone still works (verifier can derive all child addresses from leaf pubkey + non-hardened path components, but cannot sign). Documented gap, not a defect.

### `sol balance` (2 commands — standalone, address-driven)

| CLI command | Story | sol-wallet-core call | Status |
| --- | --- | --- | --- |
| `balance --address <addr> [--unit sol\|lamport]` | 3 | `chain::get_balance(addr)` (lamports via `getBalance`) | ready |
| `balance --address <addr> --token USDC\|<addr>` | 22 | `chain::spl_balance(addr, mint)` (auto-derives ATA, returns 0 if not yet created) | ready |

**Output formats:**

- Native SOL: `JSON: { sol, lamport }` (no energy/bandwidth — Solana fee model differs)
- SPL: `JSON: { mint, symbol, balance, decimals, ata }` (includes ATA address + decimals fetched dynamically via `spl_token::state::Mint::unpack`)

**Use `balance` (not `wallet balance`)** for non-wallet queries (cold wallets, watch-only addresses, exchange hot wallets). Use `wallet balance --wallet-id` for unlocked-wallet queries (auto-decrypts).

**Solana-specific balance shape:**

- No energy/bandwidth fields — Solana has a single-resource fee model (CU). See `## Solana fee model + sol-wallet-core integration` for full fee breakdown.
- SPL balance includes `ata` field (the Associated Token Account address) — caller needs it for `--to` in `sol spl send` and for explorers.
- If ATA does not exist, balance returns `{ balance: "0", ata: <derived>, exists: false }` (no RPC error).

### `sol spl` (4 commands)

| CLI command | Story | sol-wallet-core call | Status |
| --- | --- | --- | --- |
| `spl send --mnemonic --token USDC\|<addr> --to --amount` | 21 | `tx::submit_spl_transfer(sk, mint, to, amount)` (auto-derives ATA for `to` + sender) | ready |
| `spl approve --mnemonic --token --delegate --amount` | 25, 30 | `tx::submit_spl_approve(sk, mint, delegate, amount)` | ready |
| `spl balance --address --token USDC\|<addr>` | 22 | `chain::spl_balance(addr, mint)` (auto-derives ATA) | ready |
| `spl allowance --token --owner --delegate` | 30 | `chain::spl_allowance(mint, owner, delegate)` (view via `getTokenAccountData`) | ready |

**Auto-ATA-create (Q-12 footgun guard):** Before any SPL transfer, the wallet MUST:

1. Fetch mint account info to detect token program (classic vs Token-2022).
2. Derive correct ATA using `get_associated_token_address_with_program_id(owner, mint, token_program_id)`.
3. If sender ATA does not exist, prepend `create_associated_token_account_idempotent` ix to the tx.
4. If recipient ATA does not exist, prepend same ix for recipient (sender pays rent).
5. Fetch mint decimals via `unpack_mint` — NEVER hardcode 6.
6. Use `transfer_checked` (with decimals param) NOT `transfer` (catches decimals mismatch early).

**SPL token model (Solana-specific):** See `## Solana Wallet v0.1 — Complete Feature Inventory` → `### B. SPL token operations` for the full program/discriminator/calldata/account/decimals/allowance/versioning table. Key differences vs EVM (Tron): classic SPL + Token-2022 are separate programs with different ATA addresses; `transfer_checked` (with decimals param) is mandatory to catch decimals mismatch early.

### `sol tx` (2 commands)

| CLI command | Story | sol-wallet-core call | Status |
| --- | --- | --- | --- |
| `tx get --sig` | 7 | `chain::get_signature_statuses(sig, search_tx_history=true)` | ready |
| `tx wait --sig --timeout --poll-interval` | 7 | `tx::wait_for_confirm(sig, timeout)` (polls `getSignatureStatuses`) | ready |

**Solana tx shape:** signature is base58 Ed25519 signature (87-88 chars), NOT hex txid. `commitment: confirmed` (1 slot) for fast UI, `finalized` (~12 slots) for receipt-grade proof. Full polling details in `## Complete Feature Inventory` → `### E. Confirmation polling`.

### `sol config` (3 commands)

| CLI command | Story | sol-wallet-core call | Status |
| --- | --- | --- | --- |
| `config show [--json]` | 10, 11 | `config::SolanaConfig::load().display()` | ready |
| `config set-rpc <url>` | 10, 26, 27 | `config::set_rpc(url)` + save | ready |
| `config set-cluster mainnet-beta\|devnet\|localnet` | 10, 27 | `config::set_cluster(cluster)` + save | ready |

**Solana config:** `config set-cluster mainnet-beta|devnet|localnet` (Solana testnet deprecated; only 3 clusters supported).

### Security (cross-cutting)

| Feature | Story | Implementation | Status |
| --- | --- | --- | --- |
| Argon2id + AES-GCM wallet file encryption | 12 | `sol-wallet-core::wallet::persist` (Anza has none — wallet-local) | ready |
| `Zeroizing<Vec<u8>>` wrap on raw seed before `Keypair::from_seed` | 5, 17, 21, 25 | caller wrap (GAP — Anza's `Keypair::from_seed(s: &[u8])` does NOT Zeroize input param) | ready |
| SPKI pin RPC endpoint (optional) | 28 | `sol-wallet-core::chain::pki::SpkiPinnedVerifier` (reuses `bitcoin-wallet-core` pattern) | ready |
| No SPKI pin (system CAs + localhost/LAN) | 29 | `chain::SolanaClient::new(url, None)` + `webpki-roots` | ready |
| Blockhash freshness (60-90s) on `send_transaction` | 5, 17 | `sol-wallet-core::tx::sign` — re-fetch + re-sign on `BlockhashNotFound` (full retry logic in `## Solana fee model + sol-wallet-core integration` → Layer 3) | ready |
| Token-2022 vs classic SPL footgun guard | 21, 25, 30 | `sol-wallet-core::disambig::reject_wrong_token_program` — caller passes correct `token_program_id` based on `getAccountInfo(mint).owner` | ready |
| Compute Budget explicit (no implicit defaults) | 5, 17, 21 | `tx::builder` prepends `set_compute_unit_limit(150_000)` + `set_compute_unit_price(0)` ix (full detail in `## Solana fee model + sol-wallet-core integration` → Layer 1) | ready |
| ATA rent pre-flight (~0.00204 SOL) | 21, 25 | `tx::builder::spl` pre-checks sender SOL balance ≥ amount + rent + fee (full pre-flight logic in `## Solana fee model + sol-wallet-core integration` → Layer 2) | ready |

**Solana security specifics (see canonical sections):**

- **No dual-SHA256 txid** + **No `recid` v+27 analog** + **No energy/bandwidth resource model** — full Solana-specific cryptographic + fee differences covered in `## Solana fee model + sol-wallet-core integration` (EVM comparison table) and `## Complete Feature Inventory` section G (Address encoding). Solana signature = 64 bytes Ed25519 r‖s, displayed base58 (87-88 chars). Replay defense = `recent_blockhash` + `signatures[]`, not chain-id.

### Cross-cutting (apply to all)

| Feature | Implementation | Status |
| --- | --- | --- |
| `--json` mode on list/show/sync/tx-list/config-show | `serde_json` + clap value-conditional | ready |
| Stable exit codes (0/1/2/3/4/5) | `handlers::error::classify` (matches `btc/src/main.rs:151-169` exit-2 pattern) | ready |
| Base58 address display (32-44 chars, no prefix) | `solana_sdk::Pubkey::to_string` | ready |
| SOL/lamport amount unit handling (`1_000_000_000` lamport = 1 SOL) | `--unit sol\|lamport` flag → `amount.as_lamport(unit)` | ready |
| Mnemonic at rest never plaintext (Zeroizing + Argon2id + AES-GCM) | `sol-wallet-core::wallet::persist` | ready |
| Confirmation prompts (mainnet / drain / unlimited SPL approval) | stdin `yes` confirmation | ready |
| SPKI pin env-only (no CLI setter in V0.1) | `SOL_SPKI_PIN` env + `--spki-pin` flag | ready |
| All crypto delegated to `sol-wallet-core` (CLI never touches secret material directly) | `SecretKeypair` + `Zeroizing<Vec<u8>>` | ready |
| Blockhash refresh on `BlockhashNotFound` (no user intervention) | `tx::broadcast::send_with_retry` (3 attempts, exponential backoff 100ms→200ms→400ms) | ready |
| Compute Budget auto-attach (default 150_000 CU + 0 priority fee) | `tx::builder` prepends CU-limit + CU-price ix | ready |
| Decimals fetched dynamically (never hardcoded 6) | `spl_token::state::Mint::unpack(mint_account.data)` | ready |

### Deferred (not shipped in V0.1)

#### To V0.1.5 (Token-2022 features + Jito MEV, ships between V0.1 and V0.2)

| Feature | Story | Notes |
| --- | --- | --- |
| Token-2022 transfer hook support (CPI handler) | TBD | `spl_token_2022::extension::transfer_hook` — wallet must append hook program ix |
| Token-2022 confidential transfer (zk proofs) | TBD | `solana-zk-sdk` (ElGamal); ~8 MB all-time downloads, defer unless v0.1 ships confidential |
| Token-2022 interest-bearing mint display | TBD | render UI with continuous rate accrual |
| Jito tip routing (MEV bundles) | TBD | gate behind `jito` Cargo feature; `jito-sdk-rust` 0.3.2 stale |
| Address Lookup Tables (ALTs) | TBD | reduce tx size for accounts-heavy txs; v0.1.5 |
| Versioned transactions v0 with ALTs | TBD | `VersionedTransaction::V0` + lookup table resolution |
| Durable nonces (offline signing for >90s) | TBD | `nonce_account` + `advance_nonce_account`; ~0.0015 SOL rent per nonce |

**Round-1 grill Q-Token-2022 finding:** classic SPL + Token-2022 awareness shipped in V0.1 via `disambig.rs` footgun guard, but **Token-2022 extension-specific transactions** (transfer hook CPI, confidential proofs) deferred to V0.1.5. V0.1 supports `transfer_checked` for both programs but does NOT auto-append hook program instructions (caller must do that manually today). Documented gap.

#### To V0.2 (advanced + operator)

| Feature | Notes |
| --- | --- |
| List registered SPL stablecoins | `sol tokens list` |
| Add custom SPL token by mint | `sol tokens register` |
| Bulk SPL balance scan | `sol tokens balances` |
| Compute unit estimation (dry-run) | `sol compute estimate` |
| Recent fee paid history | `sol fee history` |
| Sign personal message (off-chain) | `sol sign message` (Ed25519 over arbitrary msg) |
| Verify personal message | `sol sign verify` |
| Devnet airdrop URL print | `sol faucet show` |
| Auto-drip devnet airdrop | `sol faucet drip` |
| Token-2022 mint creation (deploy new token) | `sol token create-mint` |
| Token-2022 mint + metadata | `sol token create-mint --with-metadata` |
| Network governance proposal | `sol governance propose` (Solana has no formal on-chain gov — likely deferred) |
| Stake account creation (native SOL staking) | `sol stake create` |
| Stake delegate / deactivate / withdraw | `sol stake delegate` / `sol stake deactivate` / `sol stake withdraw` |
| Wallet history sync | `sol wallet sync` |
| Wallet export (cold storage) | `sol wallet export` |
| Wallet show secret (gated mnemonic display) | `sol wallet show --secret` (requires password + confirmation) |
| Address show private key (per-address) | `sol address show --private-key` |
| Wallet import from base58 secret key | `sol wallet import --keypair-file` (Solflare/Phantom format) |
| `tx list` (history scan via getSignaturesForAddress) | `sol tx list` |
| `config set-spki-pin` (move from env-only) | `sol config set-spki-pin` |
| Shell completion script | `sol shell completion` |
| **Thread model** (FFI deadlock / Zeroizing-across-await / Send+Sync on keypair / concurrent broadcast seriality / mobile-vs-desktop runtime divergence) | v0.2 (deferred — see also `#### Thread model` section, deferred from v0.1 to v0.2) |

#### To V0.3 (advanced + requires upstream changes)

| Feature | Notes | Requires |
| --- | --- | --- |
| Confidential transfer (Token-2022 zk proofs) | `sol shield transfer` | `solana-zk-sdk` 7.0.1 (ElGamal) |
| Kora fee abstraction (relayer pays gas) | `sol relayer send` | kora SDK |
| Stake pool integration (LST routing) | `sol stake pool deposit` | spl-stake-pool + marinade/jito pool |
| Compressed NFT (Bubblegum) | `sol cNFT mint` | mpl-bubblegum (Metaplex license applies) |
| Multi-sig wallet (Squads-style) | `sol multisig create` | Squads Protocol SDK (separate crate) |

#### Never (out of scope)

| Feature | Reason |
| --- | --- |
| Hardware wallet (Ledger/Trezor) | future, v1.x |
| EIP-712 typed data | Solana uses different off-chain signing (personal message, no typed-data spec) |
| Plausible-deniability multi-bucket wallet | far future |
| gRPC transport | JSON-RPC + WS sufficient; Anza has no gRPC Rust SDK |
| Watch-only import from xpub | Ed25519 HD has no parent public key (xpub) — use `pubkey` only for limited watch-only |
| Metaplex NFT program support | **license blocker** — `Metaplex NFT Open Source License v1.0` is non-OSI with commercial restrictions |

### Total user stories covered

| Story count | Source | Status |
| --- | --- | --- |
| TBD (per `wallets/2026-09-08-sol-wallet-user-stories.md` to be written) | V0.1 | shipped |
| TBD | V0.1.5 (Token-2022 + Jito + ALTs) | ships with V0.1 release train |
| TBD | V0.2 (operator + advanced) | next release |
| TBD | V0.3 (advanced + upstream deps) | future |

**Round-1 grill finding — BUS-FACTOR ACCEPTED (2026-09-08):** Anza (Solana Labs successor) holds 90%+ of the stack (SDK + validator + SPL org). Single-vendor trust accepted. No vendoring required (Apache-2.0 license, actively maintained, ~3-month release cadence, ~30 contributors, 2-week release cadence). **Action:** monitor `solana-sdk` deprecation warnings + Anza release notes; subscribe to `anza-xyz/agave` security advisories. No local fork needed for V0.1.

**Round-1 grill finding — METAPLEX LICENSE BLOCKER (2026-09-08):** `mpl-token-metadata` 5.1.1 + `mpl-core` 0.12.1 use `Metaplex NFT Open Source License v1.0` — non-OSI, commercial restrictions, forbids use for "creation, minting, auctioning, or sale of NFTs that contain or reference [restricted categories]". **Action:** exclude from V0.1 primary stack; defer NFT support to V1.x pending legal review; do not document NFT commands in CLI.

**Round-1 grill finding — ANZA SUBCRATE VERSION DESYNC (2026-09-08):** `solana-sdk = 4.1.0`, `solana-message = 4.6.0`, `solana-transaction = 4.3.0`, `solana-client = 4.2.2` — pin each independently via `=4.x.y` (NOT caret). Cargo's facade re-export does NOT unify subcrate versions. **Action:** every Anza subcrate in `[dependencies]` of `sol-wallet-core/Cargo.toml` uses exact-pinned version, no caret. Verified via `cargo tree -p sol-wallet-core | grep solana-` in Task 0.1 verification.

**Round-1 grill finding — SOLANA LABS NO SPKI PIN (2026-09-08):** `api.mainnet-beta.solana.com` and `api.devnet.solana.com` rotate certs freely. **Action:** skip SPKI pin for Solana Labs public RPCs; use cert transparency + standard rustls verification as primary defense. SPKI pin optional for Helius/QuickNode/Alchemy paid tiers; reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` if user opts in via `SOL_SPKI_PIN` env or `pki://<pin>@host` URL.

**Round-1 grill finding — TESTNET DEPRECATED (2026-09-08):** `api.testnet.solana.com` deprecated by Solana Foundation 2022-23. **Action:** V0.1 uses devnet only; no testnet support; `RUN_SOL_TESTNET=1` env var NOT defined. Document gap in `sol config` help text.

**Round-1 grill audit:** every "Status: ready" row above is by inspection, NOT by Test scenario PASS. Before v0.1 ships, audit each ready row against the V0.1 test matrix (32 test files × 34 rows + 10 CLI tests × 22 commands). If a feature isn't covered by at least one test row, demote to "ready (untested)" or remove from v0.1. Ship what tests prove, not what inspection suggests.

### Feature map by CLI top-level

| Top-level | Commands | V0.1 features |
| --- | --- | --- |
| `wallet` | 9 | create / import / show / list / delete / rename / balance / send / send-speedup |
| `address` | 2 | new / pubkey |
| `balance` | 2 | `--address`, `--address --token` |
| `spl` | 4 | send / approve / balance / allowance |
| `tx` | 2 | get / wait |
| `config` | 3 | show / set-rpc / set-cluster |
| **Total** | **22** | All user stories TBD per `wallets/2026-09-08-sol-wallet-user-stories.md` |

## Solana Wallet v0.1 — Complete Feature Inventory

Exhaustive feature surface for `sol-wallet-core v0.1.0` + `sol` CLI v0.1.0. Each feature maps to: (a) the `sol-wallet-core` Rust API call, (b) the Anza / SPL API underneath, (c) the story id (TBD), (d) the CLI command/flag. Mapped by category: native SOL, SPL token, compute budget, blockhash lifecycle, confirmation, HD derivation, address encoding, wallet file encryption, RPC client, errors, FFI.

### A. Native SOL operations

| Feature | `sol-wallet-core` API | Anza API | CLI | Status |
| --- | --- | --- | --- | --- |
| Transfer SOL (lamports) | `tx::build_sol_transfer(from, to, lamports)` | `solana_program::system_instruction::transfer(from, to, lamports)` | `wallet send --amount <lamports>` or `--amount 0.1 --unit sol` | ready |
| Request airdrop (devnet only) | `client.request_airdrop(&pubkey, lamports)` | `RpcClient::request_airdrop` | `wallet send --airdrop` flag, OR `faucet drip <amount>` V0.2 | ready (devnet gate) |
| Get SOL balance (lamports) | `client.get_balance(&pubkey)` | `RpcClient::get_balance` | `balance --address <addr> [--unit sol\|lamport]` | ready |
| Get SOL balance (wallet-derived) | `WalletManager::unlock(id).balance_sol()` | internal | `wallet balance --wallet-id` | ready |
| Get account info (owner, lamports, data, executable) | `client.get_account(&pubkey)` | `RpcClient::get_account` | `account info <addr>` (V0.2; V0.1 uses `getMultipleAccountsInfo` under the hood) | internal only |

**Lamport math:** `1 SOL = 1_000_000_000 lamports` (9 decimals). All CLI input parses `--amount` as either SOL (decimal) or lamport (integer) via `--unit`. Internal storage always lamport (u64).

### B. SPL token operations (classic + Token-2022)

| Feature | `sol-wallet-core` API | SPL API | CLI | Status |
| --- | --- | --- | --- | --- |
| Transfer SPL tokens (checked) | `tx::build_spl_transfer_checked(from, to, mint, amount)` | `spl_token::instruction::transfer_checked(token_program, source, mint, dest, authority, signer, amount, decimals)` | `spl send --token USDC --amount 100` | ready |
| Transfer SPL tokens (unchecked — NOT RECOMMENDED) | n/a | `spl_token::instruction::transfer` (no decimals param) | n/a | **rejected** (decimals mismatch undetected) |
| Approve delegate | `tx::build_spl_approve(owner, delegate, mint, amount)` | `spl_token::instruction::approve(token_program, source, delegate, owner, signer, amount)` | `spl approve --delegate <addr> --amount 100` | ready |
| Revoke delegate | `tx::build_spl_revoke(owner, mint)` | `spl_token::instruction::revoke` | (via `spl approve --amount 0`) | ready |
| Close ATA (reclaim rent) | `tx::build_spl_close_account(owner, ata, recipient_rent)` | `spl_token::instruction::close_account` | `spl close --token USDC` | ready |
| Burn tokens (own balance) | `tx::build_spl_burn(owner, mint, amount)` | `spl_token::instruction::burn` | n/a (V0.1 wallet owns no mint authority) | not exposed |
| Mint-to (mint authority only) | `tx::build_spl_mint_to(mint_authority, mint, dest, amount)` | `spl_token::instruction::mint_to` | n/a | not exposed (need mint authority, not wallet) |
| Set authority | `tx::build_spl_set_authority(account, current_authority, new_authority, authority_type)` | `spl_token::instruction::set_authority` | n/a | not exposed |
| Get token account balance | `client.get_token_account_balance(&ata)` | `RpcClient::get_token_account_balance` | `spl balance --address --token USDC` | ready |
| Get allowance (owner → delegate) | `client.get_token_account_data(&ata, \|d\| d.delegate == Some(...) ...)` | `spl_token::state::Account::unpack` | `spl allowance --token --owner --delegate` | ready |
| Get token supply (mint) | `client.get_token_supply(&mint)` | `RpcClient::get_token_supply` | n/a | internal only (display in `tokens list` V0.2) |
| Fetch decimals dynamically | `chain::spl_decimals(&mint)` | `spl_token::state::Mint::unpack(&account.data)` → `.decimals` | n/a | internal (never hardcoded 6) |
| Detect Token-2022 vs classic | `chain::mint_token_program(&mint)` → `TokenkegQ` or `TokenzQdB` | `getAccountInfo(mint).owner` | n/a | internal (footgun guard) |
| Derive ATA (with token program id) | `spl::get_associated_token_address_with_program_id(owner, mint, token_program_id)` | `spl_associated_token_account::instruction::get_associated_token_address_with_program_id` | n/a | internal |
| Create ATA (idempotent) | `tx::prepend_create_ata(payer, owner, mint)` | `spl_associated_token_account::instruction::create_associated_token_account_idempotent` | n/a | internal (auto on first transfer) |
| Close ATA (send rent back to owner) | `tx::prepend_close_ata(owner, ata)` | `spl_token::instruction::close_account` | `spl close` | ready |
| Wrap native SOL (sync_native) | `tx::build_sync_native(ata)` | `spl_token::instruction::sync_native` | n/a (V0.1: native SOL transfers go via `system_program::transfer`, not wrapped SOL) | internal only |
| **Token-2022 extension awareness** | `mint_owner == TokenzQdB` flag; transfer_checked still works without hook CPI | `spl_token_2022::instruction::transfer_checked` | flag set, behavior identical | **footgun guard only** |
| Transfer Hook CPI dispatch | manual ix append (caller) | `spl_token_2022::extension::transfer_hook` | n/a | **deferred V0.1.5** (custom Token-2022 mints) |

**Auto-ATA-create flow (V0.1 SPEND tx shape):**

```text
[0] ComputeBudget::set_compute_unit_limit(150_000)
[1] ComputeBudget::set_compute_unit_price(0)         # or configurable priority fee
[2] spl_associated_token_account::create_associated_token_account_idempotent(payer=sender, owner=sender, mint=mint)   # if sender ATA missing
[3] spl_associated_token_account::create_associated_token_account_idempotent(payer=sender, owner=recipient, mint=mint) # if recipient ATA missing
[4] spl_token::transfer_checked(spl_token_program, source=sender_ata, mint, dest=recipient_ata, authority=sender, amount, decimals)
[5] (optional) spl_memo::build_memo("invoice-12345")   # if --memo flag set
```

Payer = sender. Sender pays ~0.00204 SOL rent per new ATA created. Recipient ATA ownership transfers automatically on creation.

### C. Compute Budget (every transaction prepends)

| Feature | Anza API | Default in V0.1 | Configurable via |
| --- | --- | --- | --- |
| CU limit per tx | `ComputeBudgetInstruction::set_compute_unit_limit(units: u32)` | **150,000 CU** | `--cu-limit <N>` CLI flag (V0.2) or `cfg.compute_unit_limit` |
| CU price (priority fee) | `ComputeBudgetInstruction::set_compute_unit_price(micro_lamports_per_cu: u64)` | **0 µLamports/CU** | `--priority-fee <micro_lamports>` CLI flag |
| Heap frame request | `ComputeBudgetInstruction::request_heap_frame(bytes: u32)` | not set | V0.2 (program deployment only) |
| Loaded accounts data size limit | `ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(bytes: u32)` | not set | V0.2 (large tx) |
| Recent prioritization fees (oracle) | `RpcClient::get_recent_prioritization_fees(&[account])` | queried on demand | `--priority-fee auto` (V0.1.5) — 50th percentile across writable accounts |
| Simulate transaction (dry-run) | `RpcClient::simulate_transaction(&tx)` | called only via `wallet send --dry-run` | `--dry-run` flag |

**CU limit defaults by tx type (V0.1 calibration):**

| Tx type | Estimated CU | V0.1 limit | Notes |
| --- | --- | --- | --- |
| Native SOL transfer | ~150 CU | **150,000 CU** | 1000x safety margin |
| SPL `transfer_checked` (classic) | ~5 CU | **150,000 CU** | p-token rewrite 2026 |
| SPL `transfer_checked` (Token-2022, no hooks) | ~150 CU | **150,000 CU** | same; tx plan shape unchanged |
| SPL + create 2 ATAs (sender + recipient) | ~15,000 CU | **200,000 CU** | auto-set when ATA prepended |
| Memo + SPL transfer | ~6,000 CU | **150,000 CU** | memo instruction ~1k CU |
| Composite (memo + SPL + 2 ATA creates) | ~22,000 CU | **200,000 CU** | worst case V0.1 |

**Priority fee defaults by cluster (V0.1):**

| Cluster | Default priority fee | Notes |
| --- | --- | --- |
| Mainnet-Beta | 0 µLamports/CU (unset — surcharges on demand) | user can override `--priority-fee` |
| Devnet | 0 µLamports/CU | free tier, no surge |
| Local validator (surfpool) | 0 µLamports/CU | off by default |

V0.1.5: auto-prioritization via `get_recent_prioritization_fees` oracle (50th percentile).

### D. Blockhash lifecycle (deterministic freshness proof)

| Feature | Anza API | V0.1 behavior | Failure mode |
| --- | --- | --- | --- |
| Fetch latest blockhash | `RpcClient::get_latest_blockhash()` | **Once per `send_transaction` invocation** (right before `sign`) | RPC error → retry with backoff |
| Blockhash validity window | ~60-90 sec (~150 slots at 400ms slot time on mainnet-beta) | n/a (cluster-enforced) | expired before confirmation |
| Stale-blockhash detection | `RpcError::BlockhashNotFound` on `send_transaction` or `BlockCleanedUp` on `getSignatureStatuses` | detected via `solana_client::client_error::ClientErrorKind` match | tx not confirmed |
| Retry policy | custom (not in Anza) | **3 attempts max, fresh blockhash each time, re-sign** | n/a |
| Backoff schedule | custom | attempt 1: 0ms; attempt 2: 100ms; attempt 3: 400ms | n/a |
| Never re-sign identical bytes | n/a | **always re-sign with same keypair after fresh blockhash** | signatures are nonces over full message |
| `recent_blockhash` field on tx | `Message::set_recent_blockhash(hash)` | updated before each `sign` | none |

**Retry pseudocode (in `sol-wallet-core/src/tx/broadcast.rs::send_with_retry`):**

```rust
async fn send_with_retry(
    rpc: &RpcClient,
    keypair: &Keypair,
    message: Message,
    max_attempts: u32 = 3,
) -> Result<Signature, Error> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        let fresh_blockhash = rpc.get_latest_blockhash().await?;
        let mut msg = message.clone();
        msg.set_recent_blockhash(fresh_blockhash);
        let tx = Transaction::new_signed_with_payer(
            &msg.instructions(), Some(&keypair.pubkey()), &[keypair], fresh_blockhash,
        );
        match rpc.send_transaction(&tx).await {
            Ok(sig) => return Ok(sig),
            Err(e) if is_blockhash_not_found(&e) && attempt < max_attempts => {
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt - 1))).await;
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

### E. Confirmation polling

| Feature | Anza API | V0.1 default | V0.1.5 enhancement |
| --- | --- | --- | --- |
| Poll signature status | `RpcClient::get_signature_statuses(&[sig])` | yes | yes |
| WS subscription for realtime | `PubsubClient::signature_subscribe(sig, config)` | not used (HTTP poll sufficient) | opt-in via `--watch` flag |
| Commitment level | `CommitmentConfig::confirmed()` (1 slot) for UI; `finalized` (~12 slots) for receipts | `confirmed` for `tx wait` default; `finalized` for `--wait-finalized` flag | unchanged |
| Poll interval | custom | **2 seconds** | configurable `--poll-interval <secs>` |
| Timeout | custom | **60 seconds** | configurable `--timeout <secs>` |
| Returned fields | `TransactionStatus { slot, confirmations, err, status }` | serialized to JSON via `TxSummary` struct | same |
| `BlockCleanedUp` (blockhash expired before confirmation) | n/a (cluster-side) | surface as `Error::BroadcastFailed { reason: "blockhash expired" }`; caller may retry with fresh blockhash (covered by §D retry) | same |
| Slot finality (root) | `RpcClient::get_slot(commitment: finalized)` | not used in V0.1 (cost) | V0.1.5 (release-train gate) |

### F. Wallet keypair (Phantom-equivalent surface)

Wallet keypair surface matches Phantom's user-facing API. Internals delegate to `bip39` + `ed25519-bip32` + `solana_sdk::signer::Keypair` — NO custom HD wrapper module. Path strings NOT exposed; numeric `--account` + `--address-index` flags only.

| Feature | Crate used | V0.1 surface | Configurable |
| --- | --- | --- | --- |
| Import mnemonic (12/15/18/21/24 words) | `bip39::Mnemonic::from_phrase` | `Wallet::fromMnemonic(phrase)` | `--words 12\|15\|18\|21\|24` |
| Master seed (PBKDF2-HMAC-SHA512, 2048 rounds) | `bip39::Seed::new(&m, "")` | internal | `--passphrase <str>` V0.2 |
| SLIP-0010 derivation at `m/44'/501'/account'/0'/address` | `ed25519_bip32::XPrv::from_seed` + `XPrv::derive` | internal (path hidden from user) | `--account <N>` + `--address-index <N>` (numeric) |
| Import base58 private key (64 bytes) | `solana_sdk::Keypair::from_base58_string` | `Wallet::fromBase58(secret)` | n/a |
| Import read-only (public key only) | `solana_sdk::Pubkey::from_str` | `Wallet::fromPublicKey(pubkey)` | n/a |
| Sign arbitrary bytes | `solana_sdk::signer::Signer::sign_message` | `Wallet::signMessage(msg)` | n/a |
| Sign VersionedTransaction | `solana_sdk::signer::Signer::sign_transaction` | `Wallet::signTransaction(tx)` | n/a |
| Public key export (base58, leaf vk) | `solana_sdk::signer::Signer::pubkey` | `Wallet::publicKey() -> Pubkey` | address exported as base58 |
| **`xpub` export (NOT POSSIBLE)** | n/a (Ed25519 SLIP-0010 spec lacks parent pubkey) | n/a | documented gap |

**Phantom-equivalent API surface (the only public Wallet API):**

```rust
// crates/sol-wallet-core/src/wallet.rs (REPLACES keys.rs)
pub struct Wallet(solana_sdk::signature::Keypair);

impl Wallet {
    /// Phantom: "Import secret phrase" → defaults to m/44'/501'/0'/0'/0
    pub fn fromMnemonic(phrase: &str) -> Result<Self>;
    /// Phantom: "Add account" → m/44'/501'/{account}'/0'/{address_index}
    pub fn fromMnemonicAt(phrase: &str, account: u32, address_index: u32) -> Result<Self>;
    /// Phantom: "Import private key" (base58 64-byte secret)
    pub fn fromBase58(secret: &str) -> Result<Self>;
    /// Phantom: "Watch-only" (read-only, no sign methods)
    pub fn fromPublicKey(pubkey: Pubkey) -> ReadOnlyWallet;
    /// Phantom: base58 Ed25519 pubkey (32 bytes)
    pub fn publicKey(&self) -> Pubkey;
    /// Phantom: sign arbitrary VersionedTransaction
    pub fn signTransaction(&self, tx: VersionedTransaction) -> Result<VersionedTransaction>;
    /// Phantom: sign arbitrary bytes (Ed25519 over SHA-512 truncated)
    pub fn signMessage(&self, msg: &[u8]) -> Signature;
}

pub struct ReadOnlyWallet(Pubkey);  // NO sign methods
```

**Why no path strings exposed:**

- Phantom users never see `m/44'/501'/0'/0'/N` — they see "Account 0, Address 0" (numeric).
- `sol-wallet-core` mirrors Phantom UX: `--account <N>` (default 0) + `--address-index <N>` (default 0).
- Derivation path `m/44'/501'/account'/0'/address` fixed at 5 components, hardcoded internally.
- Advanced users (Ledger HW path `m/44'/501'`, ZIP-32 paths) → use `sol keygen` raw CLI (V0.2 deferred).
- Crust behind the API = `bip39::Mnemonic::from_phrase` → `bip39::Seed::new` → `ed25519_bip32::XPrv::from_seed` → `XPrv::derive("m/44'/501'/.../0'/0")` → `Keypair::try_from(seed_bytes)`.

**xpub limitation:** Ed25519 SLIP-0010 does NOT expose parent public key (`xpub`). Watch-only = leaf `Pubkey` only, cannot derive sibling addresses without seed. Phantom uses the same limitation.

### G. Address encoding

| Feature | Anza API | V0.1 behavior |
| --- | --- | --- |
| Pubkey from bytes | `Pubkey::new_from_array([u8; 32])` | yes (internal) |
| Pubkey from base58 string | `Pubkey::from_str(s)` | yes (parse user input) |
| Pubkey display (base58) | `pubkey.to_string()` | yes (32-44 chars, no prefix) |
| Pubkey short display | `pubkey.short()` | not exposed (V0.1) |
| `is_on_curve` check | `Pubkey::is_on_curve(&bytes)` | yes (rejects PDA-from-bytes footgun — PDA may NOT be on Ed25519 curve) |
| PDA derivation | `Pubkey::find_program_address(&[seeds], &program_id)` | yes (internal; exposed V0.1.5 for staking) |
| Non-PDA derivation | `Pubkey::create_program_address(&[seeds], &program_id) -> Option<Pubkey>` | not exposed |
| Address validation (user input) | `parse + is_on_curve check` | yes (reject invalid base58, reject off-curve PDA bytes) |

**Address comparison table:**

| Chain | Encoding | Length | Prefix | Checksum |
| --- | --- | --- | --- | --- |
| **Solana** | base58 | 32-44 chars | none | none |
| Bitcoin (bech32) | bech32 | 42-62 chars | `bc1` | BCH |
| Bitcoin (legacy) | base58check | 34 chars | `1`/`3` | base58check |
| Ethereum | EIP-55 hex | 40 chars + `0x` | `0x` | mixed-case |
| Polygon | EIP-55 hex | 40 chars + `0x` | `0x` | mixed-case |

### H. Wallet file encryption (Argon2id + AES-256-GCM)

| Feature | Implementation | V0.1 default | Configurable |
| --- | --- | --- | --- |
| Argon2id KDF | `argon2` crate, Argon2id variant | memory: 64 MB, iterations: 3, parallelism: 1 | n/a (V0.1 fixed; V0.1.5 allows `--kdf-params`) |
| Salt generation | `OsRng` 16 bytes | per-wallet | n/a |
| AES key derivation | Argon2 output → 256-bit symmetric key | yes | n/a |
| AES-256-GCM encryption | `aes-gcm` crate, 96-bit nonce + 256-bit key | yes | n/a |
| Nonce generation | `OsRng` 12 bytes per encrypt | per-blob | n/a |
| Encrypted blob layout | `nonce ‖ ciphertext ‖ tag` | yes | n/a |
| Wallet file format | JSON metadata + encrypted blob | yes | n/a |
| Atomic write (rename) | `write to .tmp + fsync + rename` | yes (no corruption on panic) | n/a |
| Password verification | derive key + try decrypt; success = correct password | yes (no error message leaks KDF params) | n/a |

**Encrypted wallet file schema (V0.1):**

```json
{
  "version": "1.0.0",
  "wallet_id": "uuid-v4",
  "name": "trading",
  "cluster": "mainnet-beta",
  "kind": "mnemonic",
  "kdf": {
    "algorithm": "argon2id",
    "memory_kb": 65536,
    "iterations": 3,
    "parallelism": 1,
    "salt": "<base64-16-bytes>"
  },
  "cipher": {
    "algorithm": "aes-256-gcm",
    "nonce": "<base64-12-bytes>"
  },
  "encrypted_payload": "<base64-ciphertext-tag>"
}
```

### I. RPC client (full method coverage in V0.1)

`sol-wallet-core` uses these `solana_client::RpcClient` methods across V0.1:

| RPC method | V0.1 caller | Round-trip needed? |
| --- | --- | --- |
| `get_latest_blockhash` | `tx::broadcast::send_with_retry` | yes (every send) |
| `send_transaction` | `tx::broadcast::send_with_retry` | yes (every send) |
| `simulate_transaction` | `tx::broadcast::simulate` (--dry-run) | optional |
| `get_signature_statuses` | `tx::wait::poll` (`tx wait`) | yes (every wait) |
| `get_account_info` | `chain::account::info`, `spl::decimals` (mint), `spl::owner_check` (ATA) | yes (SPL + ATA ops) |
| `get_multiple_accounts_info` | `chain::account::multi` (batch mint + ATA fetch) | optional (perf) |
| `get_minimum_balance_for_rent_exemption` | `tx::builder::spl::preflight_rent` | yes (auto-ATA create) |
| `get_balance` | `chain::balance::sol` | yes (`wallet balance`, `balance`) |
| `get_token_account_balance` | `chain::spl::balance` | yes (SPL balance) |
| `get_token_supply` | `tokens::supply` (V0.2 display) | optional |
| `get_token_accounts_by_owner` | `chain::account::discover_atas` | yes (V0.1.5 `wallet list-tokens`) |
| `request_airdrop` | `tx::airdrop::request` (devnet only, gated) | yes (faucet) |
| `get_health` | `chain::boot::wait_for_ready` (surfpool spawn helper) | yes (CI helper) |
| `get_recent_prioritization_fees` | `tx::builder::priority_fee::auto` (V0.1.5) | optional |
| `get_version` | `chain::cluster::detect` (cache) | one-time at startup |
| `get_epoch_info` | `tx::builder::stake` (V0.1.5+) | optional |

WS subscription methods (`PubsubClient`, **NOT shipped in V0.1 CLI** — read-only internal use):

| WS method | V0.1 use | Future |
| --- | --- | --- |
| `account_subscribe` | internal helper for `chain::account::watch` (V0.1.5 watch mode) | V0.1.5 `wallet watch` |
| `signature_subscribe` | internal helper for `tx::wait::realtime` (V0.1.5) | V0.1.5 |
| `program_subscribe` | not used | V0.2 (Token-2022 hook monitoring) |
| `logs_subscribe` | not used | V0.2 (debug tracing) |
| `slot_subscribe` | not used | V0.2 (chain sync monitor) |

### J. Error classification

`sol-wallet-core::error::Error` (thiserror enum, exhaustive):

| Variant | Cause | CLI exit code |
| --- | --- | --- |
| `InvalidMnemonic { phrase: String, reason: &'static str }` | bip39 validation failure | 2 (input) |
| `InvalidAddress { addr: String, reason: &'static str }` | base58 parse fail OR `is_on_curve` false | 2 (input) |
| `InvalidDerivationPath { path: String, reason: &'static str }` | path string parse fail OR depth > 5 | 2 (input) |
| `InvalidCluster { cluster: String }` | cluster string not in enum | 2 (input) |
| `InvalidTokenMint { mint: String, reason: &'static str }` | mint parse fail OR not a valid mint | 2 (input) |
| `InvalidTokenProgram { mint: String, expected: Pubkey, actual: Pubkey }` | Token-2022 vs classic footgun triggered | 2 (input) |
| `SignFailed { source: Box<dyn Error> }` | Ed25519 sign failure (extremely rare) | 4 (sign) |
| `BroadcastFailed { kind: BroadcastErrorKind, context: String }` | `send_transaction` failure | 3 (network) |
| `BroadcastErrorKind::RpcError { code: i32, message: String }` | cluster-side RPC error | 3 |
| `BroadcastErrorKind::BlockhashNotFound` | stale blockhash after 3 retry attempts | 3 |
| `BroadcastErrorKind::BlockCleanedUp` | confirmed in expired blockhash window | 3 |
| `BroadcastErrorKind::InsufficientFunds { required_lamports: u64, available: u64 }` | balance check before send | 3 |
| `BroadcastErrorKind::AccountNotFound { addr: Pubkey }` | ATA doesn't exist (must call `create_associated_token_account_idempotent`) | 3 |
| `BroadcastErrorKind::NodeUnhealthy { message: String }` | cluster health check failed | 3 |
| `BroadcastErrorKind::Timeout { seconds: u64 }` | network/timeout | 3 |
| `ConfirmTimeout { sig: Signature, waited_secs: u64 }` | confirmation polling timeout | 3 |
| `WalletNotFound { id: WalletId }` | UUID lookup miss | 5 (internal) |
| `WalletDecryptFailed { id: WalletId }` | wrong password OR corrupt blob | 5 (internal) |
| `FileIo { path: PathBuf, source: io::Error }` | persist.rs atomic_write failure | 5 |
| `ConfigInvalid { key: String, value: String, reason: &'static str }` | config load fail | 5 |
| `PalError { source: Box<dyn Error + Send + Sync> }` | platform abstraction layer failed | 5 |

**Exit code mapping (matches `btc/src/main.rs:151-169` pattern):**

| Exit code | Meaning | Error variant |
| --- | --- | --- |
| 0 | success | n/a |
| 1 | generic catch-all | n/a |
| 2 | input validation | all `Invalid*` variants |
| 3 | network/RPC | all `Broadcast*`, `ConfirmTimeout` |
| 4 | signing | `SignFailed` |
| 5 | persistence/config/internal | `Wallet*`, `FileIo`, `ConfigInvalid`, `PalError` |

### K. FFI surface (cdylib)

C ABI exported from `sol_wallet_core` cdylib for Dart/Swift/Kotlin FFI consumers:

| C function | Signature | Purpose |
| --- | --- | --- |
| `sol_wallet_create_mnemonic` | `int (*out_id)(char* out_id_buf, size_t buf_len); int (*out_mnemonic)(char* out_mnemonic_buf, size_t mnemonic_buf_len); int password(char*, size_t); int cluster(char*, size_t); int error_msg(char*, size_t);` | create new wallet; returns mnemonic via out param (stderr on Dart side, stdio capture in CLI) |
| `sol_wallet_import_mnemonic` | same shape, `in_mnemonic` instead of `out_mnemonic` | import existing wallet |
| `sol_wallet_unlock` | `int (*out_secret_bytes)(uint8_t*, size_t)` | decrypt wallet, return 32-byte secret for signing (caller must Zeroize) |
| `sol_wallet_lock` | n/a (just drops in-memory secret) | clear secret buffer |
| `sol_wallet_get_address` | `int (*out_pubkey)(char*, size_t)` | get base58 Ed25519 pubkey for wallet |
| `sol_wallet_sign_transaction` | `int (*out_signature)(uint8_t*, size_t); int (*in_message)(const uint8_t*, size_t);` | sign arbitrary 32-byte hash |
| `sol_wallet_send_sol` | high-level: signs + sends in one FFI call | mobile convenience |
| `sol_wallet_send_spl` | high-level: signs + sends SPL in one FFI call | mobile convenience |
| `sol_wallet_get_balance_sol` | `uint64_t* out_lamports` | mobile UI |
| `sol_wallet_get_balance_spl` | `int (*out_balance)(char*); int (*out_decimals)(uint8_t*);` | mobile UI |
| `sol_wallet_last_error_message` | `int (*out_msg)(char*, size_t);` | error reporting |
| `sol_wallet_panic_message_clear` | n/a (no in/out) | clear panic message buffer after recovery |

**FFI safety contract:**

- All `out_*` params caller-allocated; FFI writes at most `buf_len` bytes, null-terminates
- All `in_*` params caller-allocated; FFI reads at most `buf_len` bytes
- FFI NEVER holds raw pointers across calls (no async FFI in V0.1)
- FFI NEVER owns the secret key — caller must Zeroize after use
- FFI panic → cleanup buffer + return exit code 99 (matches existing convention)
- FFI surfaces raw secret for `sol_wallet_unlock` (32 bytes) — caller must store in `Zeroizing<Vec<u8>>` (Rust side) or `Uint8List` + manual `fill(0)` (Dart side)

### L. PAL (Platform Abstraction Layer) — 4 traits

Mirrors the `bitcoin-wallet-core` PAL design (4 traits, ~5% of crate code).

| Trait | Method | Desktop impl | iOS impl | Android impl | Test impl |
| --- | --- | --- | --- | --- | --- |
| `WalletStorage` | `put_atomic(id, blob) -> Result<()>` | `FileWalletStorage` (mode 0600) | `KeychainWalletStorage` | `EncryptedFileWalletStorage` | `InMemoryStorage` |
| `WalletStorage` | `get(id) -> Result<Vec<u8>>` | yes | yes | yes | yes |
| `WalletStorage` | `delete(id) -> Result<()>` | yes | yes | yes | yes |
| `WalletStorage` | `list_ids() -> Result<Vec<WalletId>>` | yes | yes | yes | yes |
| `PlatformInfo` | `data_dir() -> PathBuf` | `~/.local/share/sol-wallet` | `Library/Application Support` | `/data/data/<pkg>/files` | `/tmp` |
| `PlatformInfo` | `app_name() -> &'static str` | `"sol-wallet"` | same | same | same |
| `PlatformInfo` | `app_version() -> &'static str` | `env!("CARGO_PKG_VERSION")` | same | same | `0.1.0-test` |
| `PlatformInfo` | `is_mobile() -> bool` | false | true | true | true |
| `NetworkClient` | `post_json(url, body) -> Result<Value>` | `ReqwestClient` (rustls-native-certs) | `OSRootsClient` (Mobile Apple Trust) | `OSRootsClient` (Android Trust) | `MockClient` |
| `NetworkClient` | `get_json(url) -> Result<Value>` | yes | yes | yes | yes |
| `Clock` | `now_epoch_ms() -> u64` | `SystemClock` | `IosClock` (NSDate) | `AndroidClock` (System.currentTimeMillis) | `MockClock` |

All traits `Send + Sync + 'static`. Production apps construct one impl per platform; tests use `InMemoryStorage + StaticInfo + MockClient + MockClock`.

### M. Complete CLI command reference (22 commands)

Full flag reference for every command:

#### wallet (9 subcommands)

```text
sol wallet create
    --name <name>                     # wallet display name (required)
    --words <12|24>                   # mnemonic length (default: 12)
    --cluster <mainnet-beta|devnet|localnet>  # target cluster (required)
    --password <password>             # encryption password (or via stdin prompt)
    [--password-stdin]                # read password from stdin (recommended)
    [--json]                          # JSON output

sol wallet import
    --name <name>                     # wallet display name (required)
    --cluster <mainnet-beta|devnet|localnet>  # target cluster (required)
    --password <password>             # encryption password
    [--mnemonic <phrase> | --mnemonic-file <path>]
    [--private-key-file <path>]       # import from base58-encoded 32-byte secret
    [--json]

sol wallet show --id <uuid>          # show address + balance + meta (no secret)
    [--json]

sol wallet list
    [--all-clusters]                  # include all clusters (default: current)
    [--json]

sol wallet delete --id <uuid>        # delete wallet + encrypted blob

sol wallet rename --id <uuid> --to <new_name>

sol wallet balance
    [--wallet-id <uuid>]              # unlocked wallet (auto-decrypt + fetch)
    [--address <base58>]              # raw address query (no decrypt)
    [--token <USDC|USDT|PYUSD|USDS|<mint>]  # SPL token (default: native SOL)

sol wallet send
    [--wallet-id <uuid> | --mnemonic <phrase> | --mnemonic-file <path>]
    --to <base58>                     # recipient base58 pubkey
    [--to-wallet <name|id>]           # recipient = stored wallet
    --amount <number> [--unit <sol|lamport>]  # native SOL
    [--token <USDC|...<mint>>] [--amount <number>]  # SPL transfer
    [--cu-limit <number>]             # override default 150_000
    [--priority-fee <micro-lamports-per-cu>]  # default 0
    [--memo <string>]                 # attach memo ix
    [--skip-memo-required]            # opt out of Token-2022 Required Memo enforcer
    [--dry-run]                       # simulate only
    [--sign-only]                     # output tx base64, no broadcast
    [--wait]                          # wait for confirmation after send
    [--wait-finalized]                # wait for finalized commitment (~12 slots)
    [--confirm-mainnet]               # extra confirm prompt for mainnet (default: true)
    [--json]

sol wallet send-speedup
    --wallet-id <uuid>
    --sig <base58-signature>          # original tx to rebroadcast with higher priority
    [--priority-fee <micro-lamports-per-cu>]
    [--json]
```

#### address (2 subcommands)

```text
sol address new
    --mnemonic <phrase> [--mnemonic-file <path>]
    [--account <N>]                   # Phantom convention: m/44'/501'/account'/0'/0
    [--address-index <N>]             # Phantom convention: m/44'/501'/account'/0'/address-index
    [--cluster <...>]
    [--json]

sol address pubkey --wallet-id <uuid>  # Ed25519 verification key (base58)
    [--json]
```

#### balance (2 subcommands)

```text
sol balance --address <base58>
    [--unit <sol|lamport>]            # native SOL unit (default: sol)
    [--token <USDC|USDT|PYUSD|USDS|<mint>>]  # SPL token
    [--json]

sol balance --address <base58> --token <mint>  # SPL balance (delegates to wallet balance --address --token)
```

#### spl (4 subcommands)

```text
sol spl send
    [--wallet-id <uuid> | --mnemonic <phrase>]
    --token <USDC|USDT|PYUSD|USDS|<mint>>
    --to <base58> [--to-wallet <name|id>]
    --amount <number>
    [--cu-limit <N>] [--priority-fee <X>]
    [--memo <string>]
    [--skip-ata-create]               # skip auto-ATA-create (assume ATAs exist)
    [--sign-only] [--wait] [--wait-finalized]
    [--json]

sol spl approve
    [--wallet-id <uuid> | --mnemonic <phrase>]
    --token <mint>
    --delegate <base58>
    --amount <number>                 # use 0 to revoke
    [--json]

sol spl balance --address <base58> --token <mint>
    [--json]                          # returns { mint, symbol, balance, decimals, ata }

sol spl allowance
    --token <mint>
    --owner <base58>
    --delegate <base58>
    [--json]
```

#### tx (2 subcommands)

```text
sol tx get --sig <base58-signature>
    [--commitment <confirmed|finalized>]
    [--json]                          # returns TxSummary { sig, slot, from, to, amount, mint, timestamp, status }

sol tx wait --sig <base58-signature>
    [--timeout <secs>]                # default 60
    [--poll-interval <secs>]          # default 2
    [--commitment <confirmed|finalized>]  # default confirmed
    [--json]
```

#### config (3 subcommands)

```text
sol config show [--json]

sol config set-rpc <url>             # e.g., https://api.mainnet-beta.solana.com or https://mainnet.helius-rpc.com/?api-key=...
    [--for-cluster <mainnet-beta|devnet|localnet>]  # which cluster's RPC URL

sol config set-cluster <mainnet-beta|devnet|localnet>
```

### N. Output formats (JSON mode for all commands)

Stable JSON shape for `--json` mode (consumed by scripts, dashboards, FFI):

```json
{
  "command": "wallet send",
  "version": "0.1.0",
  "cluster": "mainnet-beta",
  "status": "confirmed",
  "result": {
    "sig": "5Xg...base58-87-88-chars...",
    "slot": 312345678,
    "block_time": 1725868800,
    "fee_lamports": 5000,
    "priority_fee_lamports": 0,
    "compute_units_consumed": 4500,
    "from": "7xK9...sender-pubkey...",
    "to": "9aB2...recipient-pubkey...",
    "amount_lamports": 1000000000,
    "amount_sol": "1.0",
    "mint": null
  }
}
```

For SPL transfer, `mint` field populated with mint address; `amount` field uses token-decimals-adjusted amount + `decimals` field.

### O. Memory hygiene (Zeroizing discipline)

| Secret | Type | Zeroize strategy |
| --- | --- | --- |
| Mnemonic phrase (string) | `Zeroizing<String>` | drop after wallet encrypt |
| BIP-39 seed (64 bytes) | `Zeroizing<[u8; 64]>` | drop after derive complete |
| Ed25519 secret key (32 bytes) | `Zeroizing<[u8; 32]>` | drop after sign complete |
| Ed25519 HD xprv (variable) | `Zeroizing<String>` (auto from ed25519-bip32) | drop on Lock |
| Wallet encryption KDF output (32 bytes) | `Zeroizing<[u8; 32]>` | drop after AES key derived |
| AES-GCM nonce (12 bytes) | `Zeroizing<[u8; 12]>` (transient) | drop after encrypt |
| Blockhash (32 bytes, public) | `[u8; 32]` (no Zeroize needed) | n/a |
| RPC response cache | NOT zeroized (public data) | n/a |

**FFI boundary:** `sol_wallet_unlock` returns secret into a caller-allocated buffer. Caller MUST call `sol_wallet_lock` (which calls the underlying `Zeroizing::drop`) BEFORE returning to FFI consumer's main loop. Never keep secret across FFI boundary.

### R. Cross-cutting concerns (apply to all commands)

| Feature | Implementation | Status |
| --- | --- | --- |
| `Zeroizing<Vec<u8>>` wrap on all secret material (seed, xprv, KDF output, AES-GCM nonce) | `sol-wallet-core::wallet` + `crate::tx::sign` | ready |
| Mnemonic handling policy (L28 / F49 / L12 H-1): mnemonic → STDERR (red highlight); wallet_id → STDOUT; `--mnemonic-file` reads mode-0600 file (closes argv-exposure) | clap args + colored `eprintln` + `std::fs::metadata(mode 0o600)` | ready |
| `wallet show` never prints decrypted mnemonic — only address + balance + meta | `WalletManager::summary()` returns only public fields | ready |
| Confirmation prompts on mainnet / drain / unlimited SPL approval (revoke-all) | `handlers::confirm::prompt_yes` (stdin `yes` confirmation) | ready |
| SPKI pin env-only (no CLI setter in V0.1) | `SOL_SPKI_PIN` env + `--spki-pin` flag | ready |
| No SPKI pin (system CAs + localhost/LAN) | `chain::SolanaClient::new(url, None)` + `webpki-roots` | ready |
| All crypto delegated to `sol-wallet-core` (CLI never touches secret material directly) | `SecretKeypair` + `Zeroizing<Vec<u8>>` + `WalletManager` API surface only | ready |
| Stable exit codes (0/1/2/3/4/5) — matches `btc/src/main.rs:151-169` exit-2 pattern | `handlers::error::classify` | ready |
| `--json` mode on `list` / `show` / `tx get` / `tx wait` / `config show` | `serde_json` + clap value-conditional | ready |
| Compute Budget auto-attach on every tx (default 150_000 CU + 0 priority fee) | `tx::builder` prepends `set_compute_unit_limit` + `set_compute_unit_price` ix | ready |
| Decimals fetched dynamically (never hardcoded 6) — Token-2022 footgun guard | `spl_token::state::Mint::unpack(mint_account.data)` + `reject_wrong_token_program` | ready |
| Blockhash refresh on `BlockhashNotFound` (no user intervention) | `tx::broadcast::send_with_retry` (3 attempts, exp backoff 100→200→400 ms) | ready |

**Solana-specific cross-cutting deltas vs Tron V0.1:**

- **No dual-SHA256 txid workaround** — `Signature` is 64 bytes Ed25519 r‖s, not a hash. Display = base58 of 64 bytes (87-88 chars).
- **No `recid` v+27 analog** — Ed25519 has no recoverable signatures; replay defense = `recent_blockhash` + `signatures[]`, not chain-id.
- **No energy/bandwidth resource model** — fee = base 5000 lamports + priority fee + one-time rent. Simpler attack surface; no Stake 2.0 in V0.1.
- **No `--unit` for SPL amounts** — amounts are integer strings; CLI parses via `mint.decimals` from `unpack_mint`. `--unit` only applies to native SOL (`sol` / `lamport`).
- **Token-2022 vs classic SPL footgun MUST be guarded** — `tx::disambig::reject_wrong_token_program` rejects mismatched `token_program_id` BEFORE signing (caller passes wrong program = tx fails at runtime + lost fee).

### P. What's NOT in V0.1 (explicit non-features)

| Feature | Why excluded |
| --- | --- |
| Stake 2.0 freeze / delegate / vote (energy/bandwidth-style) | Solana stakes native SOL via different model (`stake::StakeState` program); deferred to V0.2+ |
| Token-2022 Transfer Hook CPI dispatch (custom hook programs) | V0.1.5 (requires BPF program loading, opt-in via solana-test-validator) |
| Confidential Transfers (zk proofs) | V0.3 (`solana-zk-sdk` 7.0.1, ~8 MB all-time downloads, niche) |
| Jito tip routing (MEV bundles) | V0.1.5 (gated behind `jito` Cargo feature) |
| Address Lookup Tables (ALTs) | V0.1.5 (reduce tx size for accounts-heavy txs) |
| Durable nonces (offline signing >90s) | V0.1.5 (~0.0015 SOL rent per nonce) |
| Hardware wallet (Ledger/Trezor) | V1.x (separate SDK integration) |
| NFT program (Metaplex) | **EXCLUDED — license blocker** (Metaplex NFT Open Source License v1.0) |
| EIP-712 typed data | Solana has no equivalent spec (off-chain signing = `sign_message` arbitrary bytes) |
| Multi-sig wallet (Squads) | V0.3 (separate crate dep) |
| Compressed NFT (Bubblegum) | V0.3 (Metaplex license) |
| Watch-only from xpub | **NOT POSSIBLE for Solana** (Ed25519 HD has no xpub) |
| Stake pool (LST) integration | V0.3 (jito-pool, marinade deps) |

### Q. Total feature count (V0.1)

| Category | Features |
| --- | --- |
| Native SOL ops | 5 |
| SPL token ops | 17 (incl ATA + Token-2022 awareness) |
| Compute Budget | 6 |
| Blockhash lifecycle | 7 |
| Confirmation polling | 7 |
| HD derivation | 8 (replaced by `Wallet keypair` (4 features) in Phantom-equivalent surface — see §F) |
| Address encoding | 7 |
| Wallet encryption | 9 |
| RPC client methods | 16 HTTP + 5 WS (read-only) |
| Error variants | 21 |
| FFI C functions | 12 |
| PAL traits | 4 traits + 14 methods |
| Cross-cutting concerns (R) | 12 |
| CLI commands | 22 (across 6 top-level groups) |
| CLI flags (leaf total) | ~85 flags |
| **Total V0.1 features** | **~148** (Phantom-equivalent surface: HD derivation 8 → Wallet keypair 4, -4 features) |

All features exposed via 22 CLI commands + 12 FFI functions + ~152 internal Rust APIs.

### S. Feature map by CLI top-level

| Top-level | Commands | V0.1 features |
| --- | --- | --- |
| `wallet` | 9 | create / import / show / list / delete / rename / balance / send / send-speedup |
| `address` | 2 | new / pubkey |
| `balance` | 2 | `--address` (native), `--address --token` (SPL) |
| `spl` | 4 | send / approve / balance / allowance |
| `tx` | 2 | get / wait |
| `config` | 3 | show / set-rpc / set-cluster |
| **Total** | **22** | All shipped in V0.1 |

**Solana top-level deltas vs Tron V0.1:**

- `address` exposes `pubkey` (Ed25519 verification key, base58) NOT `xpub` — Ed25519 SLIP-0010 has no parent public key. Use `pubkey` for limited watch-only (verifier can derive all child addresses from leaf pubkey + non-hardened path components, cannot sign).
- `address new` accepts `--account <N>` + `--address-index <N>` (numeric) ONLY — Phantom-equivalent UX. NO `--path <m/...>` flag exposed. Path string `m/44'/501'/{account}'/0'/{address_index}` hardcoded internally (Phantom/Solflare convention).
- `spl` ships 4 subcommands; Tron's `trc20` ships 4 subcommands. Same surface shape, different program ID handling (SPL auto-derives ATAs vs TRC-20 contract lookup).
- `balance` is standalone + address-driven (no decrypt); both chains offer `wallet balance --wallet-id` as the unlocked variant.
- `tx` returns base58 Ed25519 signature (87-88 chars); Tron returns hex SHA-256 txid (64 chars). Both use `confirmed` commitment / 1-block wait semantics.

## Solana Wallet Core v0.1

`sol-wallet-core` library (rlib + cdylib for FFI). Pattern follows `bitcoin-wallet-core` (`cdylib` + 4-trait PAL) and `polygon-wallet-core` (251 LOC thin wrapper) — Solana has no upstream EVM-style `evm-wallet-core` analog so `sol-wallet-core` is fat standalone (~4500 LOC V0.1). All crypto delegated to `solana-sdk` + `spl-token` / `spl-token-2022` / `spl-associated-token-account` / `ed25519-bip32` + `bip39`. RPC + persistence + encryption + FFI wallet-local. **Must compile on desktop (Linux/macOS/Windows) + mobile (iOS arm64 + Android arm64) with no source changes** — pure Rust + 4-trait Platform Abstraction Layer (WalletStorage, PlatformInfo, NetworkClient, Clock) for platform-specific concerns.

### Four-layer PAL design

```text
Layer 4: FFI (cdylib)
   - C ABI surface (extern "C" fn sol_wallet_create, sol_wallet_import, ...)
   - Panic-message scrubber (Zeroize + Drop on sensitive types)
   - tokio runtime pinned (single-threaded current_thread on FFI side)

Layer 3: Pure Rust Core (portable, 90%)
   - address/, keys/, tx/builder, tx/sign
   - crypto (argon2id + AES-GCM logic)
   - error, disambig, config (types), util
   - tx_summary, tokens (bundled JSON via include_str!)
   - chain/ (RPC client + retry + WS subscription helpers)

Layer 2: PAL — 4 traits
   - WalletStorage (encrypted blob persistence: File/Keychain/EncryptedFile)
   - PlatformInfo (data dir, app name, version, is_mobile)
   - NetworkClient (HTTP + TLS root certs + custom SPKI verifier)
   - Clock (monotonic time for tx expiration + commitment checks)

Layer 1: Platform impls (10%)
   Desktop:    FileWalletStorage, SystemDirsInfo,   ReqwestClient,        SystemClock
   iOS:        KeychainWalletStorage, BundleInfo,   OSRootsClient,        IosClock
   Android:    EncryptedFileWalletStorage, ContextInfo, OSRootsClient,    AndroidClock
   Tests:      InMemoryStorage, StaticInfo,        MockClient,           MockClock
```

### V0.1 compile targets + tooling

| Target | Cargo invocation | Purpose | V0.1 status |
| --- | --- | --- | --- |
| Linux x86_64 | `cargo build --target x86_64-unknown-linux-gnu` | CI desktop | verified (Phase 4) |
| Linux aarch64 | `cargo build --target aarch64-unknown-linux-gnu` | ARM Linux desktop | verified (Phase 4) |
| macOS x86_64 | `cargo build --target x86_64-apple-darwin` | Intel macOS dev | verified (Phase 4) |
| macOS aarch64 | `cargo build --target aarch64-apple-darwin` | Apple Silicon | verified (Phase 4) |
| Windows x86_64 | `cargo build --target x86_64-pc-windows-msvc` | Windows desktop | verified (Phase 4) |
| **iOS arm64** | `cargo check --target aarch64-apple-ios` | mobile compile gate | Phase 5+ (V0.1.5) |
| **Android arm64** | `cargo check --target aarch64-linux-android` | mobile compile gate | Phase 5+ (V0.1.5) |

**Mobile compile-only gate:** `cargo check` (no run) for `aarch64-apple-ios` + `aarch64-linux-android`. Tests skip mobile runtime smoke in V0.1 (Anza crates support target = mobile iOS/Android arm64, but no device-level integration test available without physical device or expensive emulator). Add device smoke in V0.1.5.

### MSRV + toolchain

| Property | Value | Notes |
| --- | --- | --- |
| **MSRV** | **Rust 1.89.0** | mandatory — every Anza crate (solana-sdk 4.1.0, solana-client 4.2.2, solana-message 4.6.0, etc.) pins `rust-version = "1.89.0"` |
| Workspace toolchain | 1.89.0 stable | matches MSRV |
| Toolchain file | `rust-toolchain.toml` = `"1.89.0"` (per-crate) | pins compiler version across desktop + CI |
| Nightly usage | none | no nightly features needed for V0.1 |

**MSRV split pattern:** advertise MSRV 1.85 in workspace `Cargo.toml` (loose) + toolchain file pins 1.89.0 (tight). Workspace-level pins stay flexible while the toolchain file enforces the exact minimum.

### Crate inventory (full table — 46 crates)

**Anza stack (10 crates):**

| Crate | Version | Role | MSRV |
| --- | --- | --- | --- |
| `solana-sdk` | `=4.1.0` | facade (Keypair, Pubkey, Message, Transaction) | 1.89.0 |
| `solana-program` | `=4.1.0` | program-side (PDA, hash, sysvar) | 1.89.0 |
| `solana-keypair` | `=3.1.2` | concrete Ed25519 Keypair | 1.89.0 |
| `solana-signer` | `=3.0.1` | Signer trait (abstract) | 1.89.0 |
| `solana-message` | `=4.6.0` | Message (legacy) + VersionedMessage (v0) | 1.89.0 |
| `solana-transaction` | `=4.3.0` | Transaction + VersionedTransaction envelopes | 1.89.0 |
| `solana-instruction` | `=3.5.0` | Instruction, AccountMeta | 1.89.0 |
| `solana-client` | `=4.2.2` | RpcClient + PubsubClient (HTTP + WS) | 1.89.0 |
| `solana-rpc-client` | `=4.2.2` | typed RPC client (alt to solana-client) | 1.89.0 |
| `solana-compute-budget-program` | `=4.2.2` | CU-limit + CU-price instructions | 1.89.0 |

**SPL programs (4 crates):**

| Crate | Version | Role |
| --- | --- | --- |
| `spl-token` | `9.0.0` | classic SPL (transfer_checked, mint_to, burn, close_account) |
| `spl-token-2022` | `11.0.0` | Token Extensions (extension-aware transfer_checked) |
| `spl-associated-token-account` | `8.0.0` | ATA derive + create (idempotent) |
| `spl-memo` | `7.0.0` | memo instruction builder |

**Crypto + HD (12 crates):**

| Crate | Version | Role |
| --- | --- | --- |
| `ed25519-dalek` | `=3.0.0` | Ed25519 sign (transitive via solana-keypair; pin direct) |
| `bip39` | latest | BIP-39 mnemonic (English only for V0.1) |
| `ed25519-bip32` | `=0.4.3` | SLIP-0010 Ed25519 HD (SLIP-44 coin 501 = SOL) |
| `hmac` | latest | HMAC-SHA512 (SLIP-0010 chain key) |
| `argon2` | `0.5` | Argon2id KDF for wallet file encryption |
| `aes-gcm` | `0.10` | AES-256-GCM symmetric cipher |
| `sha2` | `0.10` | SHA-256 (memo hash, msg-id derivation) |
| `sha3` | latest | Keccak-256 (cross-chain check, future) |
| `bs58` | `0.5` | base58 encode/decode (SOL addresses, tx signatures) |
| `hex` | latest | hex encode/decode |
| `zeroize` | `1.x` | Zeroizing<Vec<u8>> for seed + secret key |
| `subtle` | `2` | ConstantTimeEq for xprv compare + ATA owner verify |

**Async + HTTP (6 crates):**

| Crate | Version | Mobile? | Role |
| --- | --- | --- | --- |
| `tokio` | `1.x` | ✓ | async runtime (current_thread for FFI, multi-thread for CLI) |
| `reqwest` | `0.12` | ✓ (`rustls-tls`) | HTTP client for SPKI-pinned RPC fallback |
| `rustls` | `0.23` | ✓ (`aws-lc-rs` on mobile) | TLS + SPKI pin verifier |
| `rustls-native-certs` | `0.7` | ❌ desktop-only | OS root cert loader |
| `webpki` | `0.22` | ✓ pure Rust | custom ServerCertVerifier for SPKI pinning |
| `x509-parser` | `0.16` | ✓ pure Rust | SPKI DER extraction from cert chain |
| `tungstenite` | `0.24` | ✓ | WS client (via solana-client PubsubClient) |

**Misc (5 crates):**

| Crate | Version | Role |
| --- | --- | --- |
| `serde` + `serde_json` | `1.x` | JSON-RPC envelope parse, --json output |
| `chrono` | latest | timestamp for tx log; NOT in signature path |
| `uuid` | `1.x` | Wallet id (UUID v4) |
| `directories` | latest (desktop-only, V0.1.5 removal) | desktop data dir resolution (replaced by PAL PlatformInfo) |
| `clap` | `4.x` | CLI subcommand parser (CLI binary only, NOT in lib) |

**Errors + tracing (3 crates):**

| Crate | Version | Mobile? | Role |
| --- | --- | --- | --- |
| `thiserror` | `1.x` | ✓ pure Rust | Error enum derive (no_std-compatible) |
| `tracing` | latest | ✓ | structured logging (STDERR, secret-scrubbing filter) |
| `tracing-subscriber` | latest | ✓ (CLI only) | subscriber with EnvFilter |

**FFI + safety (2 crates):**

| Crate | Version | Mobile? | Role |
| --- | --- | --- | --- |
| `once_cell` | `1.x` | ✓ pure Rust | lazy-init for FFI runtime + compiled regex |
| `regex` | `1.x` | ✓ | panic-message scrubber (mnemonic + secret key + xprv redaction) |

**Build-time (1 crate):**

| Crate | Version | Mobile? | Role |
| --- | --- | --- | --- |
| `cbindgen` | latest | ✓ build-time only, doesn't ship | generates C header for FFI consumers (Dart/Swift/Kotlin) |

**Test-only (3 helpers + 0 external crates — surfpool spawned via tokio::process::Command):**

| Item | Role |
| --- | --- |
| `tests/common/surfpool_guard.rs` | SurfpoolGuard RAII wrapper (V0.1 default) |
| `tests/common/solana_test_validator_guard.rs` | SolanaTestValidatorGuard (V0.1.5 opt-in) |
| `tests/common/mod.rs` | pub mod exports + JSON fixture path helpers |

**Profile / cross-cutting build settings:**

```toml
# Cargo.toml — sol-wallet-core
[profile.dev]
opt-level = 1    # faster local builds

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true     # strip symbols for mobile builds (saves ~5 MB)

[profile.release-mobile]  # V0.1.5: mobile-specific profile
inherits = "release"
opt-level = "z"  # optimize for size (mobile binary)
strip = true
panic = "abort"  # smaller panic handler (~30 KB savings)
```

### Anza subcrate version pinning (desync audit)

**Critical:** Anza split subcrates have desynced versions. **MUST pin each individually** with exact `=x.y.z`. Caret `^x.y` on `solana-sdk` does NOT propagate to subcrates.

| Subcrate | Version | Desync source |
| --- | --- | --- |
| `solana-sdk` | 4.1.0 | facade version |
| `solana-program` | 4.1.0 | same workspace |
| `solana-keypair` | 3.1.2 | lower — split in 4.0 refactor (keypair extracted) |
| `solana-signer` | 3.0.1 | lower — generic trait abstraction |
| `solana-message` | 4.6.0 | higher — independent release cycle |
| `solana-transaction` | 4.3.0 | higher — independent release cycle |
| `solana-instruction` | 3.5.0 | lower — independent release cycle |
| `solana-client` | 4.2.2 | same workspace |
| `solana-rpc-client` | 4.2.2 | alt to solana-client |
| `solana-compute-budget-program` | 4.2.2 | separate crate (split from main) |
| `ed25519-dalek` | 3.0.0 | curve25519-dalek family (druk hardening 2025-26) |
| `ed25519-bip32` | 0.4.3 | typed-io, lower version (0.x) |

**Cargo.toml ordering requirement:** every Anza subcrate pin uses `=x.y.z` (NOT `^x.y.z`). Lockfile (`Cargo.lock`) must reflect these exact pins. Re-vendoring concern: `cargo update -p solana-sdk` will roll all subcrates; pin each in `[dependencies]` to prevent.

### Cross-crate reuse from workspace (NOT new deps)

Reuse existing workspace crates where possible:

| Used from | What | How |
| --- | --- | --- |
| `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` | Reusable SPKI pin verifier (no fork) | depends via Cargo path; `sol-wallet-core` imports the type |
| `polygon-wallet-core::chain::spki` | Same shape, different cluster | reference for shape parity |
| `bitcoin-wallet-core::Error` shape | thiserror enum naming | L13 reference, not import |
| Workspace workspace deps | `bip39`, `argon2`, `aes-gcm`, `sha2`, etc. | `[workspace.dependencies]` table |

### Wallet Core — mobile-unsafe dependencies (must remove for V0.1.5)

| Crate | Reason | Replacement |
| --- | --- | --- |
| `directories` | No iOS/Android backend | `WalletStorage` trait (File/Keychain/EncryptedFile impls) |
| `rustls-native-certs` | Desktop-only OS cert loader | Use `tls_built_in_root_certs(true)` on mobile (reqwest) |

### Wallet Core — mobile-unsafe transitive deps (verify at gate)

| Crate | Risk | Mitigation |
| --- | --- | --- |
| `solana-client` + `solana-rpc-client` | ~12 MB stripped, pulls `tokio` + `solana-net-utils` | Profile `release-mobile` with `opt-level = "z"` strips to ~6-7 MB |
| `ed25519-dalek` 3.0.0 | ~1 MB stripped | Acceptable; pure Rust via curve25519-dalek |
| `chrono` | ~150 KB stripped | Acceptable; or drop + use `std::time::SystemTime` |
| `reqwest` + `rustls` | ~500 KB stripped | Acceptable; profile strips to ~300 KB |

### Wallet Core — binary size impact (mobile, stripped + LTO)

| Crate | Stripped contribution |
| --- | --- |
| `solana-client` + `solana-rpc-client` | ~6-7 MB |
| `solana-sdk` + `solana-program` + `solana-message` + `solana-transaction` | ~3-4 MB |
| `ed25519-dalek` + `curve25519-dalek` | ~1 MB |
| `spl-token*` + `spl-associated-token-account` | ~3 MB combined |
| `rustls` + `reqwest` | ~400-500 KB |
| `argon2` | ~50 KB |
| `aes-gcm` | ~30 KB |

**Total V0.1 `libsol_wallet_core.so` size estimate: ~12-15 MB.** Largest crate is `solana-client` (dominates size budget). Acceptable for mobile. Optimize in V0.1.5 with `opt-level = "z"` + dead-code elimination of unused Anza submodules (can shave ~3-4 MB by removing `solana-program` features not used by wallet).

### Public APIs shipped in V0.1

`sol-wallet-core` library exposes ~140 Rust APIs across these modules (full enumeration lives at `docs/api/2026-09-08-sol-wallet-core-v0.1-api.md`, untracked per L24):

| Module | API count | Examples |
| --- | --- | --- |
| `lib.rs` | 12 | per-item `pub use` re-exports of `solana_sdk::*` types |
| `address` | 8 | `pubkey_from_bytes`, `pubkey_to_base58`, `is_on_curve`, `find_pda` |
| `wallet` | 4 | `Wallet::fromMnemonic`, `Wallet::fromBase58`, `Wallet::fromPublicKey`, `Wallet::signTransaction` (Phantom-equivalent surface) |
| `crypto` | 6 | `encrypt_wallet`, `decrypt_wallet`, `derive_kdf_key`, `random_salt` |
| `config` | 10 | `SolanaCluster`, `SolanaConfig::load`, `set_rpc`, `set_cluster`, `set_priority_fee` |
| `tx::builder` | 18 | `build_sol_transfer`, `build_spl_transfer_checked`, `build_spl_approve`, `build_spl_close_account`, `prepend_create_ata`, `prepend_compute_budget` |
| `tx::sign` | 5 | `sign_sol`, `sign_spl`, `sign_only_sol`, `sign_only_spl`, `txid` (Ed25519 r‖s display helper) |
| `tx::broadcast` | 7 | `submit_sol`, `submit_spl`, `submit_sol_speedup`, `submit_spl_approve`, `simulate`, `send_with_retry`, `wait_for_confirm` |
| `chain::client` | 14 | `RpcClient::new`, `RpcClient::from_config`, `request_airdrop`, `get_latest_blockhash`, `get_balance`, `get_account_info`, `get_token_account_balance`, `get_minimum_balance_for_rent_exemption`, `get_token_accounts_by_owner`, `get_recent_prioritization_fees` |
| `chain::pki` | 4 | `SpkiPinnedVerifier::new`, `verify_pinned`, `extract_spki_digest`, `matches_pin_set` |
| `chain::account` | 8 | `discover_atas`, `fetch_token_supply`, `fetch_decimals`, `mint_token_program`, `derive_ata_with_program_id` |
| `wallet_manager` | 11 | `WalletManager::create_with_mnemonic`, `import_from_phrase`, `import_from_pk_file`, `unlock`, `lock`, `summary`, `list`, `delete`, `rename`, `pubkey`, `keypair` (multi-wallet CRUD, CLI-only scaffolding) |
| `tokens` | 6 | `TokenRegistry::load_mainnet`, `load_devnet`, `by_symbol`, `by_mint`, `decimals_for_mint`, `mints_for` |
| `disambig` | 5 | `cluster_for_mint`, `ensure_cluster_matches`, `reject_wrong_token_program`, `split_token_mint`, `merge_atas` |
| `error` | 21 | `Error` enum variants (see §J above) |
| `ffi` | 12 | C functions (see §K above) |
| `util` | 4 | `atomic_write`, `human_lamports_to_sol`, `human_token_amount`, `zeroize_secret` |
| `platform` (4 traits × 14 methods) | 14 | see `### Four-layer PAL design` above |

**Total: ~190 Rust public APIs** in V0.1 (re-exports + implementor methods).

### Pending in V0.1 (deferred per plan)

| Surface | Status | Source |
| --- | --- | --- |
| Stake account creation + delegate + deactivate + withdraw + merge | **pending V0.2** | Uses `stake::StakeState` program (`stake::instruction::*`) — different model from legacy 2-resource fee chains |
| Token-2022 Transfer Hook CPI dispatch (custom hook programs) | **pending V0.1.5** | requires BPF program loading (`solana-test-validator` opt-in) |
| Token-2022 Confidential Transfers (zk proofs) | **pending V0.3** | `solana-zk-sdk` 7.0.1 (ElGamal), ~8 MB all-time downloads |
| Token-2022 Interest-Bearing mint display | **pending V0.1.5** | UI rendering of continuous accrual rate |
| Token-2022 Permanent Delegate / Transfer Fee / Memo Required enforcement | **pending V0.1.5** (V0.1 has detection + skip flags, NOT auto-dispatch) | footgun guard via `disambig.rs::reject_wrong_token_program` |
| Jito tip routing (MEV bundles) | **pending V0.1.5** (gated behind `jito` Cargo feature) | `jito-sdk-rust` 0.3.2 — stale, opt-in only |
| Address Lookup Tables (ALTs) | **pending V0.1.5** | reduce tx size for accounts-heavy txs (e.g., Jupiter swaps) |
| Versioned transactions v0 with ALTs | **pending V0.1.5** | `VersionedTransaction::V0` with lookup table resolution |
| Durable nonces (offline signing for >90s) | **pending V0.1.5** | `nonce_account` + `advance_nonce_account`; ~0.0015 SOL rent per nonce |
| `sign_message` (Ed25519 over arbitrary bytes) | **pending V0.1.5** | off-chain signing; CLI flag |
| `verify_message` | **pending V0.1.5** | Ed25519 sig verification + recovered pubkey |
| Hardware wallet (Ledger) | **pending V1.x** | separate SDK + transport protocol |
| NFT program (Metaplex) | **EXCLUDED** | `Metaplex NFT Open Source License v1.0` non-OSI blocker |
| Bubblegum compressed NFT | **EXCLUDED** | same Metaplex license |
| Watch-only from `xpub` | **NOT POSSIBLE** | Ed25519 HD has no `xpub` (parent public key) — only `pubkey` per leaf |

### Notes

- **`xpub` rename for Ed25519:** unlike BIP-32 secp256k1 HD chains, Ed25519 HD does NOT expose parent public key. Solana equivalent = `address pubkey` = leaf 32-byte verification key (base58). Watch-only import works for leaf pubkey + non-hardened path only.
- **Decimals never hardcoded:** all SPL transfers use `transfer_checked` with dynamically-fetched decimals via `spl_token::state::Mint::unpack(mint_account.data)`. Hardcoding 6 = footgun, fails for non-6-decimal mints (BONK=5, USDS=6, etc.).
- **Token-2022 vs classic footgun guard:** `disambig::reject_wrong_token_program` checks `mint.owner == TokenkegQ...` (classic) vs `TokenzQdB...` (Token-2022). Mismatched `token_program_id` seed produces DIFFERENT ATA address; never auto-detect without explicit verification.
- **ATA rent pre-flight:** before any transfer, wallet checks sender SOL balance covers `amount + rent + fee`. Insufficient = `BroadcastFailed::InsufficientFunds` (exit 3).
- **Blockhash refresh retry:** never re-sign identical bytes — signatures are nonces over the full message. Always fetch fresh blockhash + re-sign per attempt (up to 3 attempts, exponential backoff 100ms→200ms→400ms).
- **No 2-resource fee in V0.1:** Solana uses `stake::StakeState` program for native SOL staking — no energy/bandwidth model. Defer native stake ops to V0.2 (different crate + UI work).
- **No NFT in V0.1:** Metaplex license blocker. Foundation never ships for non-OSI.

### Gated live tests — loud RED, never silent skip

Every test that touches a live network or operator-held secret MUST follow the loud-RED contract:

1. Mark `#[ignore]` so excluded from default `cargo test` run.
2. Gate body on required env vars (`RUN_SOL_DEVNET`, `RUN_SOL_MAINNET`, `SOL_OPERATOR_WALLET`).
3. If missing, **panic!** with actionable message naming every missing variable.
4. Harness reports `FAILED` when env vars absent, `ignored` when opted in without env.

**Tests with loud-RED gate** (gated live tests that require explicit env vars; see Test scenario section for full list):

- Devnet send (loud-RED `RUN_SOL_DEVNET=1`)
- SPKI pin live extraction (loud-RED optional, Helius/QuickNode)
- Token registry live verification on mainnet (loud-RED `RUN_SOL_MAINNET=1`)
- Mainnet self-send `$0.001 USDC` (`RUN_SOL_MAINNET=1`) — **Q4 gate**
- CLI coverage against `--cluster devnet` (loud-RED `RUN_SOL_DEVNET=1`)

See `## Test scenario — sol-wallet-core V0.1` for the complete test matrix (34 rows × 32 files) and `## Test scenario — sol CLI` for the 22-command end-to-end scenarios.

---

### V0.1.5 work
~15 LOC of Cargo.toml changes:

- Remove `directories` dev-dep (1 crate) — replaced by PAL trait
- Gate `rustls-native-certs` behind `#[cfg(not(mobile))]`
- Add `release-mobile` profile (opt-level=z, panic=abort)
- Drop `tour-de-force` for Anza `solana-program` modules unused by wallet (~3-4 MB binary savings)

### Wallet Core — license summary

| License | Crates |
| --- | --- |
| Apache-2.0 | solana-*, spl-*, ed25519-dalek (with curve25519-dalek), argon2, aes-gcm, sha2, sha3, bs58, hex, zeroize, subtle, serde, tokio, reqwest, rustls, webpki, x509-parser, tungstenite, thiserror, tracing, tracing-subscriber, cbindgen, once_cell |
| MIT | bip39, ed25519-bip32, regex |
| MIT OR Apache-2.0 | ed25519-bip32 |
| BSD-3-Clause | ed25519-dalek, curve25519-dalek |
| **EXCLUDED** | `mpl-token-metadata` (Metaplex NFT Open Source License v1.0 — non-OSI) |

All compatible with `rust-wallet-app` MIT workspace license.

### Cross-crate feature map (sol-wallet-core × solana-sdk + SPL)

| sol-wallet-core module | Anza / SPL API | Direct? |
| --- | --- | --- |
| `address` | `solana_sdk::Pubkey::new_from_array`, `::from_str`, `::is_on_curve`, `Pubkey::find_program_address` | yes |
| `keys` | `bip39::Mnemonic::generate_in`, `bip39::Mnemonic::from_phrase`, `bip39::Seed::new`, `ed25519_bip32::XPrv::from_seed`, `XPrv::derive`, `XPrv::public_key` | yes (3 crates) |
| `tx::builder` (native SOL) | `solana_program::system_instruction::transfer`, `::create_account` | yes |
| `tx::builder` (SPL) | `spl_token::instruction::transfer_checked`, `::approve`, `::close_account`, `::burn`, `::set_authority`, `::mint_to` | yes |
| `tx::builder` (Token-2022) | `spl_token_2022::instruction::transfer_checked` (extension-aware) | yes (extension dispatch = V0.1.5) |
| `tx::builder` (ATA) | `spl_associated_token_account::instruction::create_associated_token_account_idempotent` | yes |
| `tx::builder` (CU) | `solana_compute_budget_program::ComputeBudgetInstruction::set_compute_unit_limit`, `::set_compute_unit_price` | yes |
| `tx::builder` (memo) | `spl_memo::build_memo` | yes |
| `tx::sign` | `Keypair::from_seed`, `Keypair::sign_message`, `Transaction::new_signed_with_payer`, `VersionedTransaction::new` | yes |
| `tx::broadcast` | `RpcClient::send_transaction`, `simulate_transaction`, `get_signature_statuses`, `get_latest_blockhash` | yes |
| `chain::client` | `RpcClient::new`, `RpcClient::new_with_commitment`, ALL RPC methods (16 HTTP, 5 WS) | yes |
| `chain::pki` | custom SPKI pin verifier (Bitcoin-compat shape) | wraps rustls + x509-parser |
| `chain::account` | `RpcClient::get_account_info`, `get_token_accounts_by_owner`, `get_token_account_balance` | yes |
| `wallet` (persistence + encryption) | Argon2id + AES-256-GCM (RustCrypto) — NOT in Anza | no (wallet-local) |
| `tokens` | bundled JSON via `include_str!` (no Anza analog) | no |
| `disambig` | compile-time guard constants + cluster footgun check | no |
| `ffi` | C ABI + panic-message scrubber (custom) | no |
| `platform` (PAL traits) | 4 traits + per-platform impls | no |
| `util` (atomic write + format helpers) | `std::fs::rename` for atomic; `format!` for human-readable | no |

**Coverage summary:** 14 of 17 `sol-wallet-core` modules wrap Anza/SPL APIs directly. 3 wallet-local (wallet persistence, tokens registry, disambig) use no Anza code (intentional — separation of concerns).

### Risk register (V0.1)

| # | Risk | Severity | Mitigation |
| --- | --- | --- | --- |
| 1 | Anza bus-factor = single-vendor (90%+ of stack) | ACCEPTED | Apache-2.0 license, ~3-month release cadence, ~30 contributors, actively maintained. No vendoring required. Monitor `anza-xyz/agave` security advisories. |
| 2 | Anza subcrate version drift (solana-sdk 4.1.0 vs solana-message 4.6.0) | LOW | Pin each subcrate with `=x.y.z` exact; `cargo update -p solana-sdk` does NOT roll subcrates (separate deps). |
| 3 | Blockhash lifetime ~60-90 sec (no ChainId replay protection) | MITIGATED | `send_with_retry` re-fetches + re-signs on `BlockhashNotFound` (3 attempts, exponential backoff). |
| 4 | Blockchain never auto-rolls Ed25519 keys (no derivation chain code via SLIP-0010 parent pubkey) | DOCUMENTED | `address pubkey` CLI command instead of `address xpub`; watch-only via per-leaf pubkey only. |
| 5 | SPL token ATA derivation seed includes `token_program_id` → different addresses for classic vs Token-2022 | MITIGATED | `disambig::reject_wrong_token_program` checks `mint.owner`. |
| 6 | `Keypair::from_seed(s: &[u8])` does NOT Zeroize input param | MITIGATED | caller wraps seed in `Zeroizing<Vec<u8>>` (Risk #3 analog). |
| 7 | `bip39::Seed::as_bytes()` returns `&[u8]` — no Zeroize on parent | MITIGATED | Zeroize-wrap master seed at construction; scope-bounded. |
| 8 | `ed25519-bip32::XPrv::to_string()` returns `String` (not Zeroize) | MITIGATED | caller wraps in `Zeroizing<String>`; never log XPrv. |
| 9 | Solana Labs public RPC no SPKI pinning | DOCUMENTED | skip pinning per Solana Labs policy; rely on cert transparency + standard rustls verification. Pinning optional for Helius/QuickNode prod via `pinned://<pin>@host` URL. |
| 10 | `solana-test-validator` flag API unstable across Agave versions | LOW | Wrap flags behind `SolanaTestValidatorGuard` (V0.1.5 opt-in); tests don't break on version bumps. |
| 11 | p-token rewrite could change SPL CU economics mid-release | LOW | Assume ~5,000 CU per simple SPL transfer as safe default (1000x safety margin). Re-calibrate in V0.1.5 if real economy differs. |
| 12 | Testnet cluster confusion (dev across teams) | MITIGATED | Cluster enum has NO `Testnet` variant. CLI does not accept `testnet`. `sol config set-cluster testnet` → `Error::InvalidCluster`. Lint-block in `Cluster` enum. |
| 13 | Devnet scheduled wipes (quarterly) | DOCUMENTED | Treat devnet as ephemeral; never persist wallets across long time windows. Re-airdrop on wipe. |
| 14 | Metaplex NFT license blocker | ACCEPTED | Defer NFT support indefinitely (V1.x at earliest). No `mpl-token-metadata` in V0.1 dep tree. |
| 15 | `spl_token_2022` Token-2022 extensions (transfer hook, confidential) NOT auto-dispatched in V0.1 | MITIGATED | V0.1 detects program; auto-creates ATA; `transfer_checked` works (extension-aware). Hook CPI dispatch = V0.1.5 manual append. |
| 16 | MSRV bump 1.85 → 1.89.0 may break workspace contract for OTHER crates | LOW | Anza crates pin 1.89.0; sibling workspace crates may have MSRV 1.74-1.81. Bumping workspace MSRV to 1.89.0 may force rebuilds but not break code. Verify with `cargo check --workspace` after pin. |
| 17 | Mobile CI matrix has no device-level smoke gate | MEDIUM | V0.1 ships compile-only mobile gate (`cargo check --target aarch64-apple-ios` + `cargo check --target aarch64-linux-android`). Real device smoke deferred V0.1.5. |
| 18 | FFI panic-message scrubber leak (mnemonic) | MITIGATED | `regex` crate scrubber filters all STDERR panic msgs (mnemonic + secret + xprv patterns); `Zeroizing` wrap on all secrets; FFI returns exit code 99 + scrubbed msg. |

### Solana Wallet Core — design notes (Solana-only)

### Confidence summary for V0.1

| Aspect | Confidence | Reason |
| --- | --- | --- |
| Wire format (Pubkey, Message, Transaction) | HIGH | Anza = official; 90%+ of stack. Stable for 12+ months |
| Sign + broadcast | HIGH | `RpcClient::send_transaction` + `Transaction::sign` — battle-tested in ecosystem |
| HD derivation (SLIP-0010) | MEDIUM | `ed25519-bip32` 0.4.3 community crate; sole Rust option; low bus factor |
| Blockhash refresh retry | MEDIUM | lifetime ~60-90 sec, well-known; retry pattern standard |
| SPL transfer with ATA auto-create | HIGH | `transfer_checked` + `create_associated_token_account_idempotent` — standard |
| Token-2022 extension dispatch | LOW | V0.1 supports `transfer_checked`; hook CPI dispatch = V0.1.5 |
| Compile targets (desktop + mobile) | HIGH | Anza MSRV 1.89.0; PAL pattern proven by sibling chains |
| Mainnet smoke gate | MEDIUM | requires Alchemy/Helius API key; $0.001 USDC self-send |
| Local validator (surfpool) | HIGH | matches Anvil pattern from `alloy-node-bindings`; <2s boot |
| FFI surface | HIGH | cdylib + 12 C functions, panic-message scrubber |
| Overall V0.1 readiness | MEDIUM-HIGH | standard Solana wallet surface; Anza ecosystem mature |

V0.1 achievable in 2-3 sessions by an experienced Rust developer following this spec + V0.1.5 work for Token-2022 hooks.

## Solana CLI

Single binary `sol`, clap subcommand dispatch. All crypto delegated to `sol-wallet-core`. **~85 subcommand variants across 6 top-level commands** for V0.1 (22), V0.1.5 (~25 Token-2022 hooks + Jito + ALTs + durable nonces), V0.2 (~30 operator + stake + sign-message), V0.3 (~8 advanced).

### Architecture

```text
rust-wallet-app/crates/sol/
├── Cargo.toml
└── src/
    ├── main.rs       # entry, tokio runtime, tracing init, dispatch
    ├── cli.rs        # clap Cli struct + Commands enum + per-subcommand args
    └── handlers/     # one fn per subcommand, async, returns Result<(), SolError>
        ├── mod.rs
        ├── wallet.rs     # create, import, show, list, delete, rename, balance, send, send-speedup (V0.1)
        ├── address.rs    # new, pubkey (renamed from xpub — Ed25519 HD has no xpub)
        ├── spl.rs        # send, approve, allowance (V0.1); + burn, close, + memo-required-check (V0.1.5)
        ├── stake.rs      # V0.1.5/V0.2: create, delegate, deactivate, withdraw, merge
        ├── tokens.rs     # V0.2: list, register, balances
        ├── compute.rs    # V0.2: estimate, history (priority fees per writable account)
        ├── faucet.rs     # V0.2: show, drip (devnet/localnet only)
        ├── sign.rs       # V0.1.5: message, verify (Ed25519 over arbitrary bytes)
        ├── jito.rs       # V0.1.5: tip submit (gated behind `jito` Cargo feature)
        ├── alts.rs       # V0.1.5: create, extend, close (Address Lookup Tables)
        ├── conf.rs       # V0.1.5: confidential transfer (Token-2022 zk proofs)
        ├── tx.rs         # V0.1: get, wait; V0.2: + list
        ├── config.rs     # V0.1: show, set-rpc, set-cluster; V0.2: + set-spki-pin
        └── error.rs      # classify (exit-code mapping 0/1/2/3/4/5)
```

**V0.1 ships 6 handlers** (wallet, address, spl, stake skipped→V0.2 placeholder, tx, config). Stake + tokens + compute + faucet + sign + jito + alts + conf + error = V0.1.5+ additions.

**CLI parser** (stable across versions):

```rust
use clap::{Parser, Subcommand, ValueEnum};
use solana_sdk::commitment_config::CommitmentConfig;

#[derive(Parser)]
#[command(name = "sol", version, about = "Solana wallet CLI")]
pub struct Cli {
    #[arg(long, env = "SOL_DATA_DIR", global = true)]
    pub data_dir: Option<PathBuf>,

    #[arg(long, env = "SOL_RPC", global = true)]
    pub rpc: Option<String>,

    /// SPKI pin (hex SHA-256 of SPKI DER) — env-only per L12 H-1.
    #[arg(long, env = "SOL_SPKI_PIN", global = true)]
    pub spki_pin: Option<String>,

    #[arg(long, env = "SOL_CLUSTER", value_enum, default_value_t = Cluster::MainnetBeta, global = true)]
    pub cluster: Cluster,

    /// Commitment level for confirmation polling (confirmed | finalized).
    #[arg(long, env = "SOL_COMMITMENT", value_enum, default_value_t = Commitment::Confirmed, global = true)]
    pub commitment: Commitment,

    /// Priority fee in micro-lamports per CU (0 = no priority, surcharged on demand).
    #[arg(long, env = "SOL_PRIORITY_FEE", global = true)]
    pub priority_fee: Option<u64>,

    /// Compute unit limit per tx (default 150,000 CU; auto-set higher for ATA creates).
    #[arg(long, env = "SOL_CU_LIMIT", global = true)]
    pub cu_limit: Option<u32>,

    #[arg(long, global = true)]
    pub allow_insecure_tls: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum Cluster {
    MainnetBeta,
    Devnet,
    Localnet,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum Commitment {
    Processed,
    Confirmed,
    Finalized,
}
```

`Commands` enum grows with each version (V0.1: 6, V0.1.5: +4 (jito, alts, sign, conf), V0.2: +6 (stake, tokens, compute, faucet, tx-list, config-spki-pin), V0.3: +2 (multi-sig)). Manual `Debug` impl on password-bearing structs (redact) + `SecretSeed` wrapper per L12 CRITICAL #2 panic-message scrubber pattern.

**Handler dispatch** (V0.1 shape, V0.1.5+ add arms):

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
    let cfg = SolanaConfig::load(&data_dir)?.with_overrides(&cli.into());

    let dispatch_result: Result<()> = async {
        match cli.command {
            Commands::Wallet(action)   => handlers::wallet::dispatch(action, &cfg).await,
            Commands::Address(action)  => handlers::address::dispatch(action, &cfg).await,
            Commands::Balance(action)  => handlers::balance::dispatch(action, &cfg).await,
            Commands::Spl(action)      => handlers::spl::dispatch(action, &cfg).await,
            Commands::Tx(action)       => handlers::tx::dispatch(action, &cfg).await,
            Commands::Config(action)   => handlers::config::dispatch(action, &cfg).await,
            // V0.1.5: Commands::Sign(action)    => handlers::sign::dispatch(action, &cfg).await,
            // V0.1.5: Commands::Jito(action)    => handlers::jito::dispatch(action, &cfg).await,
            // V0.1.5: Commands::Alts(action)    => handlers::alts::dispatch(action, &cfg).await,
            // V0.1.5: Commands::Conf(action)    => handlers::conf::dispatch(action, &cfg).await,
            // V0.2:   Commands::Stake(action)   => handlers::stake::dispatch(action, &cfg).await,
            // V0.2:   Commands::Tokens(action)  => handlers::tokens::dispatch(action, &cfg).await,
            // V0.2:   Commands::Compute(action) => handlers::compute::dispatch(action, &cfg).await,
            // V0.2:   Commands::Faucet(action)  => handlers::faucet::dispatch(action, &cfg).await,
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

**Error classifier** (`handlers/error.rs`, stable across versions, Solana-specific variants):

```rust
pub fn classify(e: &anyhow::Error) -> i32 {
    for c in e.chain() {
        if let Some(lib_err) = c.downcast_ref::<sol_wallet_core::Error>() {
            return match lib_err {
                // exit 2: invalid input
                Error::InvalidMnemonic { .. }
                | Error::InvalidAddress { .. }
                | Error::InvalidDerivationPath { .. }
                | Error::InvalidCluster { .. }
                | Error::InvalidTokenMint { .. }
                | Error::InvalidTokenProgram { .. } => 2,
                // exit 3: transport / RPC / broadcast
                Error::BroadcastFailed { .. }
                | Error::ConfirmTimeout { .. } => 3,
                // exit 4: wallet/balance error
                Error::WalletNotFound { .. }
                | Error::WalletDecryptFailed { .. } => 4,
                // exit 5: signing / persistence / config / internal
                Error::SignFailed { .. }
                | Error::FileIo { .. }
                | Error::ConfigInvalid { .. }
                | Error::PalError { .. } => 5,
            };
        }
    }
    1
}
```

**Why 6 `Invalid*` variants:** covers Mnemonic, Address, DerivationPath, Cluster, TokenMint, TokenProgram — the last is the Token-2022 vs classic footgun guard, which needs its own variant for actionable error messages.

### Cross-cutting patterns (all versions)

**Global flags:**

| Flag | Env | Default | Notes |
| --- | --- | --- | --- |
| `--data-dir <path>` | `SOL_DATA_DIR` | OS data dir (Linux: `~/.local/share/sol/`) | per-platform from PAL |
| `--rpc <url>` | `SOL_RPC` | per-cluster default (`https://api.mainnet-beta.solana.com`, etc.) | override per-cluster |
| `--cluster <mainnet-beta\|devnet\|localnet>` | `SOL_CLUSTER` | `mainnet-beta` | 3-variant enum (testnet deprecated) |
| `--commitment <processed\|confirmed\|finalized>` | `SOL_COMMITMENT` | `confirmed` | default for `tx wait`; `finalized` for `--wait-finalized` |
| `--priority-fee <micro-lamports>` | `SOL_PRIORITY_FEE` | unset (0 µLamports/CU) | forwarded to `set_compute_unit_price` ix |
| `--cu-limit <number>` | `SOL_CU_LIMIT` | `150_000` per tx | auto-set higher when ATA prepend needed |
| `--spki-pin <hex>` | `SOL_SPKI_PIN` | none (env-only per L12 H-1) | optional, off by default |
| `--allow-insecure-tls` | — | false | always SPKI-pin if pin set |

**New flag patterns (L12 H-1 + cold-sign + Solana-specific):**

```rust
/// Mode-0600 file path containing the 12/24-word BIP-39 mnemonic.
/// Closes the L12 H-1 argv-exposure hole.
#[arg(long, conflicts_with = "mnemonic")]
pub mnemonic_file: Option<PathBuf>,

/// Sign + return signed-tx-base64 without broadcasting. Cold-sign pipeline
/// for future hardware-wallet migration (V0.1.5+ Ledger).
#[arg(long)]
pub sign_only: bool,

/// Mode-0600 file path containing the base58-encoded 32-byte Ed25519 secret key.
/// (Solflare/Phantom format.) Closes L12 H-1 for private-key import path.
#[arg(long, conflicts_with = "mnemonic", conflicts_with = "mnemonic_file")]
pub private_key_file: Option<PathBuf>,

/// Recipient pubkey as base58 (32-44 chars). Required for native SOL send.
#[arg(long, conflicts_with = "to_wallet")]
pub to: Option<String>,

/// Recipient = stored wallet name or UUID. Resolved via WalletManager::lookup().
#[arg(long, conflicts_with = "to", value_parser = parse_name_or_uuid)]
pub to_wallet: Option<String>,

/// Amount in native SOL or lamport (depending on --unit). For SPL transfers,
/// this flag is reused under --token for the token amount.
#[arg(long)]
pub amount: Option<String>,

/// --unit sol | lamport (default: sol)
#[arg(long, default_value = "sol")]
pub unit: UnitEnum,

/// --token USDC | USDT | PYUSD | USDS | <mint>
/// Triggers SPL transfer mode. Combined with --amount for the token quantity.
#[arg(long)]
pub token: Option<String>,

/// Skip auto-ATA-create ix (assume sender + recipient ATAs both exist).
/// Rare flag — auto-ATA-create is the normal path for first-time recipients.
#[arg(long)]
pub skip_ata_create: bool,

/// Skip Token-2022 "Required Memo on Transfer" enforcement (opt-out).
/// Use only if you know the mint does not enforce memo.
#[arg(long)]
pub skip_memo_required: bool,

/// Wait for confirmation after broadcast (default: false — immediate return with sig).
#[arg(long)]
pub wait: bool,

/// Wait for `finalized` commitment (~12 slots) instead of `confirmed` (1 slot).
#[arg(long, conflicts_with = "wait", conflicts_with_all = [wait])]
pub wait_finalized: bool,

/// Confirm mainnet send with extra prompt (default: true for mainnet-beta).
#[arg(long)]
pub confirm_mainnet: bool,
```

**Output conventions (mirrors `btc` parity + Solana-specific fields):**

- **STDOUT**: machine-readable. wallet_id, base58 pubkey, signature, JSON, or empty.
- **STDERR**: human logs + secrets + diagnostics. `tracing` crate, levels via `SOL_LOG` env.
- **JSON mode**: `--json` flag on `wallet list`/`show`/`tx get`/`tx wait`/`config show` switches table → JSON.
- **Mnemonic secrecy**: NEVER on STDOUT. STDERR only when `wallet create`/`import` triggered.
- **Signature display**: base58 of 64-byte Ed25519 r‖s (87-88 chars).
- **Address display**: base58 Ed25519 pubkey (32-44 chars, no prefix).
- **Exit codes** (stable across versions):
  - `0` success
  - `1` anyhow default
  - `2` invalid input (bad mnemonic, bad address, bad derivation path, bad cluster, bad mint, Token-2022/classic footgun)
  - `3` transport / RPC / broadcast (RPC error, BlockhashNotFound after 3 retry, ConfirmTimeout, NodeUnhealthy)
  - `4` wallet/balance error (WalletNotFound, WalletDecryptFailed)
  - `5` signing / persistence / config / internal (SignFailed, FileIo, ConfigInvalid, PalError)

**Mnemonic handling (L28 / F49 / L12 H-1):**

- `wallet create` → mnemonic → STDERR (red highlight); wallet_id → STDOUT
- `wallet import --mnemonic` → mnemonic → STDERR; wallet_id → STDOUT
- `wallet import --mnemonic-file` → reads mode-0600 file (closes argv-exposure L12 H-1)
- `wallet show` → decrypted mnemonic NEVER displayed; only address + balance + cluster

**Handler module signature (per command) — Solana-specific shapes:**

```rust
// wallet send (native SOL or SPL via --token)
pub async fn handle_send(
    wallet_id: Option<Uuid>,
    mnemonic: Option<SecretSeed>,
    mnemonic_file: Option<PathBuf>,
    private_key_file: Option<PathBuf>,
    to: Option<String>,
    to_wallet: Option<String>,
    amount: Amount,
    unit: UnitEnum,
    token: Option<String>,             // None = native SOL; Some = SPL
    cu_limit: Option<u32>,
    priority_fee: Option<u64>,
    memo: Option<String>,
    skip_ata_create: bool,
    skip_memo_required: bool,
    dry_run: bool,
    sign_only: bool,
    wait: bool,
    wait_finalized: bool,
    confirm_mainnet: bool,
    cfg: &SolanaConfig,
) -> Result<SendReceipt, anyhow::Error> {
    // Resolve secret key from wallet_id / mnemonic / mnemonic_file / private_key_file
    let sk = resolve_secret_key(wallet_id, mnemonic, mnemonic_file, private_key_file, &cfg)?;

    // Resolve recipient: --to base58 OR --to-wallet name|uuid
    let recipient_pubkey = match (to, to_wallet) {
        (Some(addr), None) => Pubkey::from_str(&addr)?,
        (None, Some(name_or_id)) => WalletManager::lookup(&name_or_id)?.address()?,
        _ => anyhow::bail!("--to or --to-wallet required"),
    };

    // Determine tx kind: native SOL or SPL
    let mint = token.as_deref().map(parse_mint_with_decimals).transpose()?;

    let receipt = if let Some((mint_pubkey, decimals)) = mint {
        // SPL transfer_checked — auto-prepend ATA create + compute budget + memo
        if sign_only {
            let (tx_b64, sig) = sol_wallet_core::tx::sign_only_spl(
                &sk, &recipient_pubkey, &mint_pubkey, decimals, amount.as_base_units(decimals)?,
                cu_limit, priority_fee, memo.as_deref(), skip_ata_create, skip_memo_required, &cfg,
            ).await?;
            SendReceipt { sig, result: "SIGN_ONLY".into(), tx_b64: Some(tx_b64), kind: SendKind::Spl { mint: mint_pubkey, decimals } }
        } else {
            sol_wallet_core::tx::submit_spl(
                &sk, &recipient_pubkey, &mint_pubkey, decimals, amount.as_base_units(decimals)?,
                cu_limit, priority_fee, memo.as_deref(), skip_ata_create, skip_memo_required, wait, wait_finalized, &cfg,
            ).await?
        }
    } else {
        // Native SOL via system_instruction::transfer
        let lamports = amount.as_lamport(unit)?;
        if sign_only {
            let (tx_b64, sig) = sol_wallet_core::tx::sign_only_sol(
                &sk, &recipient_pubkey, lamports, cu_limit, priority_fee, &cfg,
            ).await?;
            SendReceipt { sig, result: "SIGN_ONLY".into(), tx_b64: Some(tx_b64), kind: SendKind::Sol { lamports } }
        } else {
            sol_wallet_core::tx::submit_sol(
                &sk, &recipient_pubkey, lamports, cu_limit, priority_fee, wait, wait_finalized, &cfg,
            ).await?
        }
    };

    Ok(receipt)
}
```

### Consolidated flat table (all versions)

| Ver | Subcommand | Story | STDOUT |
| --- | --- | --- | --- |
| **v0.1** | `wallet create` | 1 | wallet_id |
| **v0.1** | `wallet import` | 2 | wallet_id |
| **v0.1** | `wallet show` | 11 | JSON `{id, name, address, cluster, created_at}` |
| **v0.1** | `wallet list` | 9 | table or JSON array |
| **v0.1** | `wallet delete` | 9 | empty |
| **v0.1** | `wallet rename` | 9 | empty |
| **v0.1** | `wallet balance` | 3, 22 | JSON balance |
| **v0.1** | `wallet send` | 5 | JSON `{sig, slot?, result, tx_b64?, kind}` (supports `--to-wallet`) |
| **v0.1** | `wallet send-speedup` | 17 | JSON `{new_sig, result}` |
| **v0.1** | `address new` | 3 | base58 Ed25519 pubkey |
| **v0.1** | `address pubkey` | 19 | base58 string (Ed25519 pubkey; Ed25519 HD has no xpub) |
| **v0.1** | `spl send` | 21 | JSON `{sig, result, ata_send, ata_recv?}` |
| **v0.1** | `spl approve` | 25, 30 | JSON `{sig, result}` |
| **v0.1** | `spl allowance` | 30 | `{allowance, decimals, ata}` |
| **v0.1** | `spl balance` | 22 | `{mint, symbol, balance, decimals, ata, exists}` |
| **v0.1** | `balance --address` | 3 | `{sol, lamport}` |
| **v0.1** | `balance --address --token` | 22 | SPL balance (delegates to `spl balance --address`) |
| **v0.1** | `tx get` | 7 | `{sig, slot, block_time, from, to, amount, mint, status}` |
| **v0.1** | `tx wait` | 7 | `{sig, slot, confirmations, status, err?}` |
| **v0.1** | `config show` | 10, 11 | JSON `{data_dir, rpc_url, spki_pin, cluster, version}` |
| **v0.1** | `config set-rpc` | 10, 26 | empty |
| **v0.1** | `config set-cluster` | 10, 27 | empty |
| v0.1.5 | `sign message` | V0.1.5 message | `{sig, message, pubkey}` (Ed25519 over arbitrary bytes) |
| v0.1.5 | `sign verify` | V0.1.5 message | `{valid: bool, recovered_pubkey?}` |
| v0.1.5 | `jito tip submit` | V0.1.5 MEV | `{bundle_id, landed_slot?}` (gated `jito` feature) |
| v0.1.5 | `alts create` | V0.1.5 lookup table | `{alt_address, sig, authority}` |
| v0.1.5 | `alts extend` | V0.1.5 lookup table | `{sig, addresses_added}` |
| v0.1.5 | `alts close` | V0.1.5 lookup table | `{sig, reclaimed_lamports}` |
| v0.1.5 | `conf transfer` | V0.1.5 zk proofs | `{sig, proof_size_kb, range?}` (Token-2022 confidential) |
| v0.2 | `wallet sync` | V0.2 history | JSON `Vec<TxSummary>` (last 100 sigs) |
| v0.2 | `wallet export` | V0.2 cold storage | base58 `SecretKey` (Ed25519) + JSON `Vec<Pubkey>` |
| v0.2 | `wallet show --secret` | V0.2 recovery (password + confirm) | `{mnemonic, expires:30s}` |
| v0.2 | `address show --private-key` | V0.2 recovery | base58 Ed25519 secret |
| v0.2 | `wallet import --keypair-file` | V0.2 Solflare/Phantom | wallet_id |
| v0.2 | `stake create` | V0.2 stake | `{stake_account, sig}` |
| v0.2 | `stake delegate` | V0.2 stake | `{sig, validator_vote_pubkey}` |
| v0.2 | `stake deactivate` | V0.2 stake | `{sig, deactivation_slot}` |
| v0.2 | `stake withdraw` | V0.2 stake | `{sig, lamports_withdrawn}` |
| v0.2 | `stake merge` | V0.2 stake | `{sig, source_stake, dest_stake}` |
| v0.2 | `tokens list` | V0.2 registry | table or JSON of registered SPL |
| v0.2 | `tokens register` | V0.2 registry | JSON `{mint, symbol, decimals, name}` |
| v0.2 | `tokens balances` | V0.2 bulk | JSON `{tokens: [{mint, balance, decimals}]}` |
| v0.2 | `compute estimate` | V0.2 fee | `{cu_consumed, priority_fee_micro_lamports, p50_fee_per_cu}` |
| v0.2 | `compute history` | V0.2 fee | JSON array of last N tx CUs + fees |
| v0.2 | `faucet show` | V0.2 devnet faucet | `{cluster, faucet_url, drip_command}` |
| v0.2 | `faucet drip` | V0.2 devnet faucet | `{sig, lamports, status}` |
| v0.2 | `tx list` | V0.2 history | JSON `Vec<TxSummary>` (via `getSignaturesForAddress`) |
| v0.2 | `config set-spki-pin` | V0.2 (DX) | empty (move from env-only) |
| v0.2 | `shell completion` | V0.2 (DX) | prints completion script |
| v0.3 | `multisig create` | V0.3 multisig | `{multisig_address, sig, threshold, owners}` |
| v0.3 | `multisig send` | V0.3 multisig | `{sig, kind}` |

**Total: ~85 subcommand variants.** V0.1=22, V0.1.5=15 (sign + jito + alts + conf), V0.2=20 (operator + stake + tokens + compute + faucet + tx-list), V0.3=2 (multisig only; no stake-pool/governance/shield variants — those are validator/operator concerns, not user wallet).

### V0.1 — 22 commands, 6 top-level (SHIP)

Minimal viable Solana wallet: create + import + balance + send (SOL + SPL) + tx verify + config.

#### `sol wallet` (9 subcommands)

| Subcommand | Story | `sol-wallet-core` call | STDOUT |
| --- | --- | --- | --- |
| `sol wallet create --words 12\|24 --name <n> --cluster <c> --password <pw> [--account <N>] [--address-index <N>]` | 1 | `WalletManager::create_with_mnemonic` (defaults to account=0, address-index=0 per Phantom convention) | wallet_id |
| `sol wallet import --name --cluster --password --mnemonic\|--mnemonic-file\|--private-key-file [--account <N>] [--address-index <N>]` | 2 | `WalletManager::import_from_phrase` or `import_from_pk` (defaults to account=0, address-index=0) | wallet_id |
| `sol wallet show --id [--json]` | 11 | `WalletManager::unlock(id, pw).summary()` | JSON `{id, name, address, cluster, created_at}` |
| `sol wallet list [--json] [--all-clusters]` | 9 | `WalletManager::list()` | table or JSON array |
| `sol wallet delete --id` | 9 | `WalletManager::delete(id)` | empty |
| `sol wallet rename --id --to` | 9 | `WalletManager::rename(id, name)` | empty |
| `sol wallet balance --wallet-id\|--address [--token USDC\|<addr>]` | 3, 22 | `chain::get_balance(addr)` or `WalletManager::unlock(id).balance()` | JSON balance |
| `sol wallet send --wallet-id\|--mnemonic --to <addr>\|--to-wallet <name\|id> --amount [--unit] [--token] [--cu-limit] [--priority-fee] [--memo] [--dry-run] [--sign-only] [--wait] [--wait-finalized]` | 5 | `tx::submit_sol` or `tx::submit_spl` | JSON `{sig, result, tx_b64?, kind}` |
| `sol wallet send-speedup --wallet-id --sig --priority-fee` | 17 | `tx::submit_send_speedup` | JSON `{new_sig, result}` (Solana has no RBF — emits new sig with higher priority fee) |

#### `sol address` (2 subcommands)

| Subcommand | Story | `sol-wallet-core` call | STDOUT |
| --- | --- | --- | --- |
| `sol address new --mnemonic [--mnemonic-file] --account <N> --address-index <N>` | 3 | `Wallet::fromMnemonicAt(phrase, account, address_index)` (Phantom convention: `m/44'/501'/{account}'/0'/{address_index}`) | base58 Ed25519 pubkey |
| `sol address pubkey --wallet-id` | 19 | `WalletManager::pubkey(id)` (Ed25519 verification key) | base58 string |

**Note on `xpub`:** renamed `xpub` → `pubkey` (Ed25519 HD has no xpub). No `--index 0` default — Solana uses `m/44'/501'/0'/0/0` (Phantom/Solflare convention, hardened only at first 3 components).

#### `sol balance` (2 subcommands — standalone, address-driven)

| Subcommand | Story | `sol-wallet-core` call | STDOUT |
| --- | --- | --- | --- |
| `sol balance --address <addr> [--unit sol\|lamport]` | 3 | `chain::get_balance(addr)` | JSON `{sol, lamport}` |
| `sol balance --address <addr> --token <USDC\|<addr>>` | 22 | `chain::spl_balance(addr, mint)` | JSON `{mint, symbol, balance, decimals, ata, exists}` |

**No `energy`/`bandwidth` fields:** Solana single-resource fee model. `spl balance` includes `ata` field (the Associated Token Account address) + `exists` flag (auto-derive vs on-chain lookup miss).

#### `sol spl` (4 subcommands)

| Subcommand | Story | `sol-wallet-core` call | STDOUT |
| --- | --- | --- | --- |
| `sol spl send --wallet-id\|--mnemonic --token USDC\|<addr> --to <addr>\|--to-wallet --amount [--cu-limit] [--priority-fee] [--memo] [--skip-ata-create] [--skip-memo-required] [--dry-run] [--sign-only] [--wait]` | 21 | `tx::submit_spl` (auto-prepends ATA create ix; uses `transfer_checked` with fetched decimals) | JSON `{sig, result, ata_send, ata_recv?, decimals}` |
| `sol spl approve --wallet-id --token --delegate --amount` | 25, 30 | `tx::submit_spl_approve` | JSON `{sig, result}` |
| `sol spl allowance --token --owner --delegate` | 30 | `chain::spl_allowance(mint, owner, delegate)` (deserializes `spl_token::state::Account.delegate`) | JSON `{allowance, decimals, ata}` |
| `sol spl balance --address --token` | 22 | `chain::spl_balance(addr, mint)` (same as `balance --address --token`) | JSON `{mint, symbol, balance, decimals, ata, exists}` |

**Token-2022 extensions:** uses `spl_token::instruction::transfer_checked` (auto-fetches decimals — never hardcoded 6). `--skip-ata-create` lets caller skip ATA creation ix when they know ATAs exist (perf). `--skip-memo-required` bypasses Token-2022's `Required Memo` extension opt-out (footgun guard).

#### `sol tx` (2 subcommands)

| Subcommand | Story | `sol-wallet-core` call | STDOUT |
| --- | --- | --- | --- |
| `sol tx get --sig [--commitment]` | 7 | `chain::get_signature_statuses(sig, search_tx_history=true)` + `getTransaction` | JSON `{sig, slot, block_time, from, to, amount, mint, fee_lamports, cu_consumed, status}` |
| `sol tx wait --sig [--timeout] [--poll-interval] [--commitment]` | 7 | `tx::wait_for_confirm(sig, timeout, poll_interval, commitment)` (polls `getSignatureStatuses`) | JSON `{sig, slot, confirmations, status, err?}` |

**Signature type:** base58 Ed25519 r‖s (87-88 chars), NOT hex txid. `--commitment` flag (processed/confirmed/finalized) reflects Solana consensus model (Anza RPC).

#### `sol config` (3 subcommands)

| Subcommand | Story | `sol-wallet-core` call | STDOUT |
| --- | --- | --- | --- |
| `sol config show [--json]` | 10, 11 | `config::SolanaConfig::load().display()` | JSON `{data_dir, rpc_url, spki_pin, cluster, version}` |
| `sol config set-rpc <url> [--for-cluster <c>]` | 10, 26 | `config::set_rpc(url, cluster)` + save | empty |
| `sol config set-cluster <mainnet-beta\|devnet\|localnet>` | 10, 27 | `config::set_cluster(cluster)` + save | empty |

**Cluster naming:** `set-cluster` not `set-network` (Solana uses `Cluster` enum). `set-rpc` supports `--for-cluster` per-cluster RPC URL (devnet + mainnet can have different URLs).

### Total CLI surface (V0.1)

- **6 top-level commands**
- **22 subcommands**
- **~85 leaf flags** (--json, --password, --cluster, etc. across all commands)
- **22 distinct outputs** (mix of stdout text, JSON, hex, base58, table)
- **5 exit codes** (0-5)
- **~140 internal Rust APIs** behind the CLI
- **12 FFI C functions** behind the mobile UI

Clap parser surfaces all flags; FFI surfaces direct function calls (no CLI parsing on mobile). Both paths share the same `sol-wallet-core` Rust code — single source of truth.

## Solana Wallet v0.1 — solana-sdk/SPL feature coverage audit

Every V0.1 feature audited for whether the underlying Anza/SPL/Ed25519-HD crate has a primitive, or whether `sol-wallet-core` must write it from scratch.

**Status legend:**
- **ready** = `solana-sdk` / `spl-*` / `ed25519-bip32` / `bip39` / `aes-gcm` / `argon2` (etc.) provides the API. Wallet wraps with thin glue + Zeroizing.
- **not** = no Anza/SPL/standard crypto crate primitive exists. Wallet writes entirely (policy + orchestration + persistence + CLI + FFI).

**Total V0.1 features audited: 173. Ready: 71 (Anza provides primitive). Not: 102 (wallet writes entirely — most are policy + persistence + CLI + FFI + ZERO Anza persistence layer exists).**

### Coverage A. Native SOL operations

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| A.1 | Transfer SOL (lamports) | `solana_program::system_instruction::transfer(from, to, lamports)` | **ready** | thin wrap |
| A.2 | Request airdrop (devnet) | `RpcClient::request_airdrop(&pubkey, lamports)` | **ready** | thin wrap |
| A.3 | Get SOL balance | `RpcClient::get_balance(&pubkey)` | **ready** | thin wrap |
| A.4 | Get account info (owner, lamports, data) | `RpcClient::get_account(&pubkey)` | **ready** | thin wrap |
| A.5 | Subsidy-free transfer (a sender paying own tx) | `system_instruction::transfer` | **ready** | thin wrap |

### Coverage B. SPL token operations

| # | Feature | Anza/SPL primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| B.1 | SPL `transfer_checked` (classic) | `spl_token::instruction::transfer_checked(token_program, source, mint, dest, authority, signer, amount, decimals)` | **ready** | thin wrap |
| B.2 | SPL `transfer` (unchecked) | `spl_token::instruction::transfer(...)` | **ready** (existed but **not** used — wallet rejects; footgun) | wallet uses `transfer_checked` only |
| B.3 | SPL `approve` (delegate) | `spl_token::instruction::approve(...)` | **ready** | thin wrap |
| B.4 | SPL `revoke` (approve 0) | `spl_token::instruction::approve(amount=0)` | **ready** | thin wrap (convention) |
| B.5 | SPL `close_account` (rent reclaim) | `spl_token::instruction::close_account(...)` | **ready** | thin wrap |
| B.6 | SPL `burn` | `spl_token::instruction::burn(...)` | **ready** | thin wrap |
| B.7 | SPL `mint_to` (mint authority) | `spl_token::instruction::mint_to(...)` | **ready** | thin wrap |
| B.8 | SPL `set_authority` | `spl_token::instruction::set_authority(...)` | **ready** | thin wrap |
| B.9 | Get SPL token balance | `RpcClient::get_token_account_balance(&ata)` | **ready** | thin wrap |
| B.10 | Get allowance (deserialized state) | `spl_token::state::Account::unpack(data)` | **ready** | thin wrap |
| B.11 | Get token supply | `RpcClient::get_token_supply(&mint)` | **ready** | thin wrap |
| B.12 | Fetch decimals dynamically | `spl_token::state::Mint::unpack(data)` | **ready** | thin wrap |
| B.13 | Detect Token-2022 vs classic (mint.owner check) | `getAccountInfo(mint).owner` | **ready** | thin wrap |
| B.14 | Derive ATA (with token program) | `spl_associated_token_account::instruction::get_associated_token_address_with_program_id(owner, mint, token_program)` | **ready** | thin wrap |
| B.15 | Create ATA idempotent | `spl_associated_token_account::instruction::create_associated_token_account_idempotent(payer, owner, mint)` | **ready** | thin wrap |
| B.16 | Auto-prepend ATA create (orchestration) | B.15 instruction | **ready** | orchestration (wallet prepends ix conditionally) |
| B.17 | Token-2022 `transfer_checked` (extension-aware) | `spl_token_2022::instruction::transfer_checked(...)` | **ready** | thin wrap (no extension dispatch in V0.1) |
| B.18 | **Token-2022 Transfer Hook CPI dispatch** (custom hook programs) | n/a (caller writes hook ix manually) | **not** | deferred V0.1.5 |
| B.19 | **Confidential Transfer** (zk proofs) | n/a (`solana-zk-sdk` separate, ~8 MB downloads) | **not** | deferred V0.3 |
| B.20 | **Interest-Bearing mint** (continuous accrual UI) | n/a | **not** | deferred V0.1.5 |
| B.21 | **Permanent Delegate / Transfer Fee** dispatch (Token-2022 ext) | n/a (caller must detect + comply) | **not** | deferred V0.1.5 |
| B.22 | **Memo Required on Transfer** enforcement (Token-2022 ext) | n/a (relies on ix-data shape detection) | **not** | partial — `disambig.rs` detects flag; auto-append deferred |

### Coverage C. Compute Budget

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| C.1 | Set CU limit | `solana_compute_budget_program::ComputeBudgetInstruction::set_compute_unit_limit(units: u32)` | **ready** | thin wrap |
| C.2 | Set CU price (priority fee, micro-lamports/CU) | `ComputeBudgetInstruction::set_compute_unit_price(micro_lamports: u64)` | **ready** | thin wrap |
| C.3 | Request heap frame | `ComputeBudgetInstruction::request_heap_frame(bytes: u32)` | **ready** | thin wrap |
| C.4 | Set loaded accounts data size limit | `ComputeBudgetInstruction::set_loaded_accounts_data_size_limit(bytes: u32)` | **ready** | thin wrap |
| C.5 | Recent prioritization fees oracle | `RpcClient::get_recent_prioritization_fees(&[account])` | **ready** | thin wrap (CLI exposure V0.1.5) |
| C.6 | Simulate transaction (dry-run) | `RpcClient::simulate_transaction(&tx)` | **ready** | thin wrap |
| C.7 | Auto-prepend CU ix before send (orchestration) | C.1 | **ready** | wallet orchestration |
| C.8 | Auto-size CU based on tx type (150k vs 200k) | n/a (logical default) | **not** (wallet policy) | wallet logic |

### Coverage D. Blockhash lifecycle

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| D.1 | Fetch latest blockhash | `RpcClient::get_latest_blockhash()` | **ready** | thin wrap |
| D.2 | Blockhash validity window (60-90s) | documented, not enforced by Anza | **ready** | thin wrap |
| D.3 | Stale blockhash detection (`BlockhashNotFound`) | `RpcError` returned by `send_transaction` | **ready** | thin wrap + classify |
| D.4 | Stale blockhash mid-confirm (`BlockCleanedUp`) | `RpcResponseError` | **ready** | thin wrap + classify |
| D.5 | `send_with_retry` retry loop (3 attempts, fresh blockhash + re-sign) | n/a | **not** (wallet orchestration) | retry policy |
| D.6 | Exponential backoff (100ms→200ms→400ms) | n/a | **not** (wallet orchestration) | retry policy |
| D.7 | Never re-sign identical bytes | documented, not enforced by Anza | **not** (wallet policy) | sign discipline |
| D.8 | **Durable nonce** (offline signing >90s) | `nonce_account::advance_nonce_account` | **ready** (existed but **not** used in V0.1) | deferred V0.1.5 |

### Coverage E. Confirmation polling

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| E.1 | Poll signature status (HTTP) | `RpcClient::get_signature_statuses(&[sig])` | **ready** | thin wrap |
| E.2 | Wait for confirmation (loop until confirmed) | n/a | **not** (wallet orchestration) | polling loop |
| E.3 | Poll interval (configurable, default 2s) | n/a | **not** (wallet policy) | orchestration |
| E.4 | Timeout (configurable, default 60s) | n/a | **not** (wallet policy) | orchestration |
| E.5 | Commitment level config (processed/confirmed/finalized) | `CommitmentConfig` | **ready** | thin wrap (CLI flag mapping) |
| E.6 | BlockCleanedUp detection mid-confirm | `RpcResponseError` | **ready** | thin wrap |
| E.7 | WS signature_subscribe (realtime) | `PubsubClient::signature_subscribe` | **ready** (existed but **not** used in V0.1) | V0.1.5 `--watch` flag |
| E.8 | Root slot finality check | `RpcClient::get_slot(CommitmentConfig::finalized())` | **ready** (existed but **not** used in V0.1) | V0.1.5 release-train gate |

### Coverage F. Wallet keypair (Phantom-equivalent surface)

| # | Feature | Crate primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| F.1 | Import mnemonic (12/24 words) → derive `m/44'/501'/0'/0'/0` | `bip39::Mnemonic::from_phrase` + `ed25519_bip32::XPrv::derive` | **ready** | `Wallet::fromMnemonic` (Phantom API) |
| F.2 | Import base58 64-byte private key | `solana_sdk::Keypair::from_base58_string` | **ready** | `Wallet::fromBase58` (Phantom API) |
| F.3 | Read-only wallet from public key | `solana_sdk::Pubkey::from_str` | **ready** | `Wallet::fromPublicKey` (Phantom API) |
| F.4 | Sign VersionedTransaction | `solana_sdk::signer::Signer::sign_transaction` | **ready** | `Wallet::signTransaction` (Phantom API) |
| F.5 | Sign arbitrary bytes | `solana_sdk::signer::Signer::sign_message` | **ready** | `Wallet::signMessage` (Phantom API) |
| F.6 | Verify Ed25519 signature | `solana_sdk::signature::Signature::verify` | **ready** | thin wrap (V0.2 `verify_message` API) |
| F.7 | Numeric `--account` + `--address-index` flags | `Wallet::fromMnemonicAt(phrase, account, address_index)` | **ready** | Phantom UX (no path strings) |
| F.8 | **xpub export** (parent public key) | n/a (Ed25519 SLIP-0010 spec lacks xpub) | **not possible** | replaced by `Wallet::publicKey()` per leaf |
| F.9 | Zeroize-wrap `Keypair::from_seed(s: &[u8])` input | n/a (Anza does NOT Zeroize param) | **not** (wallet gap-fixing) | `Zeroizing<[u8; 32]>` before `Keypair::from_seed` |
| F.10 | Zeroize-wrap master seed | `Zeroizing<[u8; 64]>` (zeroize crate, not Anza) | **ready** | wallet memory hygiene |

### Coverage G. Address encoding

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| G.1 | Pubkey from 32 bytes | `solana_sdk::Pubkey::new_from_array(bytes)` | **ready** | thin wrap |
| G.2 | Pubkey from base58 string | `solana_sdk::Pubkey::from_str(s)` | **ready** | thin wrap |
| G.3 | Pubkey display (base58) | `solana_sdk::pubkey.to_string()` | **ready** | thin wrap |
| G.4 | Pubkey short display (first 4 + last 4 chars) | `solana_sdk::pubkey.short()` | **ready** | thin wrap (CLI exposure V0.2) |
| G.5 | `is_on_curve` check (reject off-curve PDA bytes as input) | `solana_sdk::Pubkey::is_on_curve(&bytes)` | **ready** | thin wrap |
| G.6 | PDA derivation (max 2^32 bumps + nonce) | `solana_sdk::Pubkey::find_program_address(&seeds, &program_id)` | **ready** | thin wrap |
| G.7 | Non-PDA derivation | `solana_sdk::Pubkey::create_program_address(&seeds, &program_id) -> Option<Pubkey>` | **ready** | thin wrap |
| G.8 | Cross-cluster address discrimination (reject BTC/Eth/Trx-style base58) | n/a | **not** (wallet disambig) | wallet guards |
| G.9 | Cluster mismatch detection (mainnet USDC address queried on devnet) | n/a | **not** (wallet policy) | wallet disambig |

### Coverage H. Wallet file encryption + persistence

| # | Feature | Anza/standard crypto primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| H.1 | Argon2id KDF (memory-hard, m=64MB, t=3, p=1) | `argon2` crate | **ready** (primitive crate, NOT Anza) | wrap + config params |
| H.2 | Salt generation (16 bytes OsRng) | `argon2` + `rand::OsRng` | **ready** (primitive) | thin wrap |
| H.3 | AES-256-GCM encryption | `aes_gcm` crate (`Aes256Gcm`) | **ready** (primitive) | thin wrap |
| H.4 | Nonce generation (12 bytes OsRng) | `aes_gcm` + `OsRng` | **ready** (primitive) | thin wrap |
| H.5 | Encrypted blob layout (nonce ‖ ciphertext ‖ tag) | n/a | **not** (wallet format design) | wallet |
| H.6 | Wallet file JSON metadata format (version, id, kdf params) | `serde_json` | **ready** | thin wrap |
| H.7 | Atomic write (write .tmp + fsync + rename) | `std::fs::rename` | **ready** (stdlib) | thin wrap |
| H.8 | UUID v4 wallet id | `uuid` crate | **ready** | thin wrap |
| H.9 | Password verification via decrypt-test | n/a (policy) | **not** (wallet) | wallet |
| H.10 | Mode-0600 file permissions (POSIX) | `std::fs::Permissions` | **ready** (Unix-only) | thin wrap |
| H.11 | EncryptedWallet wrapper struct + methods | n/a | **not** (wallet) | wallet type |

### Coverage I. RPC client methods used by V0.1 (16 HTTP + 5 WS = 21)

| # | RPC method | solana_client primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| I.1 | `getLatestBlockhash` | `RpcClient::get_latest_blockhash` | **ready** | thin wrap |
| I.2 | `sendTransaction` | `RpcClient::send_transaction` + `send_transaction_with_config` | **ready** | thin wrap + retry |
| I.3 | `simulateTransaction` | `RpcClient::simulate_transaction` | **ready** | thin wrap |
| I.4 | `getSignatureStatuses` | `RpcClient::get_signature_statuses` | **ready** | thin wrap + polling |
| I.5 | `getAccountInfo` | `RpcClient::get_account` + `get_account_with_config` | **ready** | thin wrap |
| I.6 | `getMultipleAccountsInfo` | `RpcClient::get_multiple_accounts` | **ready** | thin wrap |
| I.7 | `getMinimumBalanceForRentExemption` | `RpcClient::get_minimum_balance_for_rent_exemption(size)` | **ready** | thin wrap |
| I.8 | `getBalance` | `RpcClient::get_balance` | **ready** | thin wrap |
| I.9 | `getTokenAccountBalance` | `RpcClient::get_token_account_balance` | **ready** | thin wrap |
| I.10 | `getTokenSupply` | `RpcClient::get_token_supply` | **ready** | thin wrap |
| I.11 | `getTokenAccountsByOwner` | `RpcClient::get_token_accounts_by_owner` | **ready** | thin wrap |
| I.12 | `requestAirDrop` (devnet) | `RpcClient::request_airdrop` | **ready** | thin wrap |
| I.13 | `getHealth` | `RpcClient::get_health` | **ready** | thin wrap |
| I.14 | `getRecentPrioritizationFees` | `RpcClient::get_recent_prioritization_fees` | **ready** | thin wrap |
| I.15 | `getVersion` | `RpcClient::get_version` | **ready** | thin wrap |
| I.16 | `getEpochInfo` | `RpcClient::get_epoch_info` | **ready** | thin wrap |
| I.17 | `accountSubscribe` (WS) | `PubsubClient::account_subscribe` | **ready** | V0.1.5 `wallet watch` |
| I.18 | `signatureSubscribe` (WS) | `PubsubClient::signature_subscribe` | **ready** | V0.1.5 realtime |
| I.19 | `programSubscribe` (WS) | `PubsubClient::program_subscribe` | **ready** | V0.2 |
| I.20 | `logsSubscribe` (WS) | `PubsubClient::logs_subscribe` | **ready** | V0.2 |
| I.21 | `slotSubscribe` (WS) | `PubsubClient::slot_subscribe` | **ready** | V0.2 |
| I.22 | **SPKI pin custom verifier** | n/a (rustls + x509-parser + webpki) | **ready** (3rd-party crates, NOT Anza) | wrap + pin policy |

### Coverage J. Errors

| # | Error variant | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| J.1 | `InvalidMnemonic` | n/a | **not** | thiserror variant + display |
| J.2 | `InvalidAddress` | n/a | **not** | thiserror variant + display |
| J.3 | `InvalidDerivationPath` | n/a | **not** | thiserror variant + display |
| J.4 | `InvalidCluster` | n/a | **not** | thiserror variant + display |
| J.5 | `InvalidTokenMint` | n/a | **not** | thiserror variant + display |
| J.6 | `InvalidTokenProgram` (Token-2022 footgun) | n/a | **not** | thiserror variant + display |
| J.7 | `SignFailed` | n/a | **not** | wrap Anza sign errors |
| J.8 | `BroadcastFailed { kind: BroadcastErrorKind }` | n/a | **not** | classify + retry recommendation |
| J.9 | `BroadcastErrorKind::RpcError { code, message }` | `solana_client::client_error::ClientErrorKind` | **ready** | wrap |
| J.10 | `BroadcastErrorKind::BlockhashNotFound` | `RpcError::BlockhashNotFound` | **ready** | detect |
| J.11 | `BroadcastErrorKind::BlockCleanedUp` | `RpcResponseError` | **ready** | detect |
| J.12 | `BroadcastErrorKind::InsufficientFunds` | `RpcResponseError` (parsed from result) | **ready** | detect |
| J.13 | `BroadcastErrorKind::AccountNotFound` | `RpcError::AccountNotFound` | **ready** | detect |
| J.14 | `BroadcastErrorKind::NodeUnhealthy` | `RpcError::NodeUnhealthy` | **ready** | detect |
| J.15 | `BroadcastErrorKind::Timeout` | `ClientErrorKind::Io` (timeout variant) | **ready** | detect |
| J.16 | `ConfirmTimeout` | n/a | **not** | wallet |
| J.17 | `WalletNotFound` | n/a | **not** | wallet |
| J.18 | `WalletDecryptFailed` | n/a | **not** | wallet |
| J.19 | `FileIo` | n/a | **not** | wrap `std::io::Error` |
| J.20 | `ConfigInvalid` | n/a | **not** | wallet |
| J.21 | `PalError` | n/a | **not** | wrap PAL impl errors |
| J.22 | `thiserror::Error` enum derive | n/a | **ready** (primitive crate, NOT Anza) | wrap + define variants |
| J.23 | Exit code mapping (0-5) | n/a | **not** | wallet CLI policy |

### Coverage K. FFI surface (cdylib)

| # | C function | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| K.1 | `sol_wallet_create_mnemonic` | n/a | **not** | ABI design |
| K.2 | `sol_wallet_import_mnemonic` | n/a | **not** | ABI design |
| K.3 | `sol_wallet_unlock` | n/a | **not** | ABI design + 32-byte secret return |
| K.4 | `sol_wallet_lock` | n/a | **not** | ABI design + Zeroize clear |
| K.5 | `sol_wallet_get_address` | n/a | **not** | ABI design + base58 encode |
| K.6 | `sol_wallet_sign_transaction` | n/a | **not** | ABI design + 64-byte sig return |
| K.7 | `sol_wallet_send_sol` | n/a | **not** | ABI design + orchestration |
| K.8 | `sol_wallet_send_spl` | n/a | **not** | ABI design + orchestration |
| K.9 | `sol_wallet_get_balance_sol` | n/a | **not** | ABI design + u64 lamports |
| K.10 | `sol_wallet_get_balance_spl` | n/a | **not** | ABI design + u64 base units + decimals |
| K.11 | `sol_wallet_last_error_message` | n/a | **not** | ABI design + write to out buffer |
| K.12 | `sol_wallet_panic_message_clear` | n/a | **not** | ABI design |
| K.13 | Out-param buffer management (caller-allocated, null-terminate) | n/a | **not** | wallet ABI contract |
| K.14 | Panic → exit code 99 recovery | n/a | **not** | wallet panic handler |
| K.15 | Panic-message scrubber (regex redaction of mnemonic + secret) | n/a | **not** | wallet filter |
| K.16 | `cbindgen` C header generation | n/a (build-time tooling) | **ready** (3rd-party crate) | config |

### Coverage L. PAL (Platform Abstraction Layer)

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| L.1 | `WalletStorage` trait (4 methods: put_atomic, get, delete, list_ids) | n/a | **not** | trait def |
| L.2 | `FileWalletStorage` (desktop) | `std::fs` | **ready** | impl trait |
| L.3 | `KeychainWalletStorage` (iOS) | n/a (security-framework FFI) | **not** | wallet impl (FFI to iOS keychain) |
| L.4 | `EncryptedFileWalletStorage` (Android) | n/a | **not** | wallet impl |
| L.5 | `InMemoryStorage` (test) | `std::collections::HashMap` | **ready** | impl trait |
| L.6 | `PlatformInfo` trait (data_dir, app_name, app_version, is_mobile) | n/a | **not** | trait def |
| L.7 | `SystemDirsInfo` (Linux/macOS) | `directories` crate (V0.1.5 REMOVE) | **ready** | impl trait |
| L.8 | `BundleInfo` (iOS — main bundle) | n/a | **not** | wallet impl |
| L.9 | `ContextInfo` (Android — Context.getFilesDir) | n/a | **not** | wallet impl |
| L.10 | `StaticInfo` (test) | n/a | **not** | wallet impl |
| L.11 | `NetworkClient` trait (post_json, get_json) | n/a | **not** | trait def |
| L.12 | `ReqwestClient` (desktop + rustls-native-certs) | `reqwest` + `rustls` | **ready** | impl trait |
| L.13 | `OSRootsClient` (mobile + AWS-lc-rs provider) | `reqwest` + `rustls` | **ready** | impl trait |
| L.14 | `MockClient` (test) | n/a | **not** | wallet impl |
| L.15 | `Clock` trait (now_epoch_ms) | n/a | **not** | trait def |
| L.16 | `SystemClock` (Linux/macOS/Windows) | `std::time::SystemTime` | **ready** | impl trait |
| L.17 | `IosClock` (NSDate) | n/a | **not** | wallet impl (FFI) |
| L.18 | `AndroidClock` (System.currentTimeMillis) | n/a | **not** | wallet impl (FFI) |
| L.19 | `MockClock` (test) | n/a | **not** | wallet impl |
| L.20 | PAL + per-platform data dir resolution | n/a | **not** | wallet orchestration |

### Coverage M. CLI binary (`sol` crate)

| # | CLI capability | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| M.1 | clap `Cli` struct (8 global flags) | `clap` crate | **ready** (primitive, NOT Anza) | wrap |
| M.2 | `Cluster` enum (3 variants: MainnetBeta, Devnet, Localnet) | `clap::ValueEnum` | **ready** | wrap |
| M.3 | `Commitment` enum (3 variants: Processed, Confirmed, Finalized) | `clap::ValueEnum` | **ready** | wrap |
| M.4 | `Commands` enum dispatch | `clap::Subcommand` | **ready** | wrap |
| M.5 | `wallet` subcommand (9 args: create/import/show/list/delete/rename/balance/send/send-speedup) | `clap` | **ready** | wrap |
| M.6 | `address` subcommand (2 args: new, pubkey) | `clap` | **ready** | wrap |
| M.7 | `balance` subcommand (2 args: --address, --token) | `clap` | **ready** | wrap |
| M.8 | `spl` subcommand (4 args: send, approve, allowance, balance) | `clap` | **ready** | wrap |
| M.9 | `tx` subcommand (2 args: get, wait) | `clap` | **ready** | wrap |
| M.10 | `config` subcommand (3 args: show, set-rpc, set-cluster) | `clap` | **ready** | wrap |
| M.11 | Tokio runtime + tracing init | `tokio` + `tracing` | **ready** (3rd-party crates) | wrap |
| M.12 | `handlers/{wallet,address,balance,spl,tx,config}.rs` (6 files, ~250 LOC each) | n/a | **not** | wallet logic |
| M.13 | Argument validation (--cluster, --commitment, --unit, --token sym vs mint) | n/a | **not** | wallet validation |
| M.14 | `--json` flag conditional output formatting | `serde_json` | **ready** | wrap + shape |
| M.15 | STDOUT/STDERR separation (mnemonic never STDOUT) | n/a | **not** | wallet output discipline |
| M.16 | Stable exit codes (0/1/2/3/4/5) | `std::process::exit` | **ready** (stdlib) | wallet policy |
| M.17 | `--password-stdin` (L12 H-1 argon2-input capture) | `std::io::stdin` | **ready** (stdlib) | wrap |
| M.18 | `--mnemonic-file <path>` (mode-0600 file read) | `std::fs::read_to_string` | **ready** (stdlib) | wrap |
| M.19 | `--dry-run`, `--sign-only`, `--wait`, `--wait-finalized`, `--confirm-mainnet` flags | `clap` | **ready** | wrap |
| M.20 | `wallet-to-wallet` `--to-wallet <name|id>` resolution | n/a | **not** | wallet glue |
| M.21 | ``--token USDC.USDT.PYUSD.USDS.<mint>`` registry lookup | bundled JSON via `include_str!` | **not** (wallet JSON, NOT Anza) | wallet |
| M.22 | `--skip-ata-create`, `--skip-memo-required` flags (Token-2022 footgun opt-outs) | n/a | **not** | wallet policy |

### N. Output formats

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| N.1 | STDOUT/STDERR separation | n/a | **not** | wallet design |
| N.2 | `--json` flag (15 commands) | `serde_json` | **ready** (3rd-party) | wrap |
| N.3 | Stable JSON schema (cross-cmd shape) | `serde` derive | **ready** (3rd-party) | shape |
| N.4 | `TxSummary { sig, slot, from, to, amount, mint }` struct | n/a | **not** | wallet type |
| N.5 | `SendReceipt { sig, result, tx_b64?, kind }` struct | n/a | **not** | wallet type |

### O. Zeroizing discipline (memory hygiene)

| # | Secret / operation | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| O.1 | Mnemonic phrase (`String`) | n/a | **not** | `Zeroizing<String>` wrap |
| O.2 | BIP-39 seed (`[u8; 64]`) | `bip39::Seed::as_bytes()` returns non-Zeroize slice | **not** (gap-fixing layer) | wrap in `Zeroizing<Vec<u8>>` |
| O.3 | Ed25519 secret key (`[u8; 32]`) | `Keypair::from_seed(s: &[u8])` does NOT Zeroize input | **not** (gap-fixing layer) | wrap input in `Zeroizing<Vec<u8>>` before call |
| O.4 | ed25519-bip32 `XPrv::to_string()` returns `String` | not Zeroizing | **not** (gap-fixing) | wrap output `Zeroizing<String>`; never log |
| O.5 | KDF output (`[u8; 32]`) | n/a | **not** | `Zeroizing` |
| O.6 | AES-GCM nonce (`[u8; 12]`) | n/a | **not** | `Zeroizing` (transient) |
| O.7 | Blockhash (`[u8; 32]`, public) | n/a | **ready** (no Zeroizing needed, public) | n/a |
| O.8 | FFI secret return across boundary | n/a | **not** | contract doc: caller Zeroizes Dart/Swift Uint8List |

### T. Test framework

| # | Feature | Anza primitive | Status | Wallet role |
| --- | --- | --- | --- | --- |
| T.1 | `SurfpoolGuard::spawn` (V0.1 default local validator) | n/a (`surfpool` binary external) | **not** (wallet spawn helper) | subprocess + health poll + RAII |
| T.2 | `SolanaTestValidatorGuard::spawn` (V0.1.5 opt-in) | n/a (`solana-test-validator` external) | **not** | wallet spawn helper |
| T.3 | Devnet integration test setup | n/a | **not** | wallet env-var gate + airdrop |
| T.4 | Mainnet smoke gate (`$0.001 USDC self-send`) | n/a | **not** | wallet env-var gate + Alchemy URL |
| T.5 | Wallet local test harness | `tokio` + `proptest` + `reqwest` (test) | **ready** (3rd-party crates) | wrap + fixture loaders |

### Summary — Anza-supported primitives (ready)

| Anza / SPL primitive | Used in V0.1 for | Count |
| --- | --- | --- |
| `solana_sdk::*` (Keypair, Pubkey, Signature, Message, VersionedTransaction) | All wire format + signing | 24 APIs |
| `solana_program::system_instruction` | Native SOL transfer | 1 |
| `solana_client::RpcClient` (HTTP) | All RPC reads + writes | 16 methods |
| `solana_client::PubsubClient` (WS) | Subscriptions (V0.1.5+) | 5 methods |
| `solana_compute_budget_program` | CU-limit + CU-price ix | 4 instructions |
| `spl_token::instruction` (classic) | SPL transfer_checked/approve/burn/close/mint_to/set_authority | 7 instructions |
| `spl_token::state::Mint::unpack`, `Account::unpack` | Decimals + delegate deser | 2 |
| `spl_token_2022::instruction::transfer_checked` | Extension-aware transfer | 1 |
| `spl_associated_token_account::instruction` | ATA derive + create idempotent | 3 instructions |
| `spl_memo::build_memo` | Memo ix | 1 |
| `bip39::Mnemonic::generate_in`, `from_phrase`, `Seed::new` | Mnemonic + PBKDF2 | 3 |
| `ed25519_bip32::XPrv::from_seed`, `derive`, `public_key` | SLIP-0010 HD | 3 |
| `argon2` (3rd-party) | Argon2id KDF | 1 |
| `aes_gcm` (3rd-party) | AES-256-GCM encrypt/decrypt | 1 |
| `zeroize` (3rd-party) | Zeroizing wraps | 1 (used everywhere) |
| `bs58`, `hex`, `clap`, `serde`, `tokio`, `reqwest`, `rustls`, `tracing`, `uuid` | Various | ~15 |
| **Total primitives ready** | | **~85 primitives** |

### Summary — Wallet writes entirely (not, no Anza analog)

| Wallet feature | LOC estimate | Reason no Anza analog |
| --- | --- | --- |
| `wallet/id.rs` (UUID + atomic persist) | ~150 | Anza has zero persistence layer |
| `crypto/` (Argon2 + AES-GCM orchestration) | ~300 | Anza = signing only |
| `config.rs` (SolanaCluster + config file) | ~150 | wallet file format = wallet design |
| `error.rs` (21-variant Error + exit codes) | ~150 | semantic mapping = wallet policy |
| `disambig.rs` (Token-2022 + cluster footgun guards) | ~100 | wallet guard logic |
| `tx/broadcast.rs` (send_with_retry retry loop) | ~200 | policy, not crypto |
| `tx/submit.rs` (offline sign + broadcast convenience) | ~100 | wallet convenience |
| `tx/summary.rs` (TxSummary + SendReceipt) | ~50 | wallet output struct |
| `chain/ retry policy` (ClientErrorKind classification) | ~80 | wallet policy |
| `chain/pki/` (SPKI pin verifier reuse) | ~80 | wallet reuses bitcoin-wallet-core helper |
| `wallet/` (UUID + encrypted store + atomic_write) | ~200 | persistence |
| `tokens/` (bundled JSON + registry loader) | ~80 | app config |
| `platform/` (4 PAL traits + 14+ impls) | ~600 | portability abstraction |
| `ffi/` (C ABI + panic scrubber + Zeroize discipline) | ~400 | mobile binary interface |
| `cli/` (separate `sol` binary, clap + 22 handlers) | ~600 | operator CLI |
| `util/` (format helpers + human_lamports_to_sol) | ~80 | UX |
| `tests/common/surfpool_guard.rs` | ~150 | local validator spawn |
| `tests/common/solana_test_validator_guard.rs` | ~150 | V0.1.5 opt-in |
| Zeroizing wrappers (8 secrets) | ~50 | memory hygiene |
| Orchestration around Anza primitives (auto-ATA-create, CU prepend, decimals fetch) | ~400 | policy |
| **Total wallet-local LOC** | **~3,820 LOC** | |

### Final audit verdict

| Metric | Count |
| --- | --- |
| V0.1 features audited | 173 |
| Features where Anza/SPL provides the primitive | 71 (41%) |
| Features where wallet writes entirely | 102 (59%) |
| Anza primitives used | ~85 distinct API calls |
| Wallet-local modules | 20 |
| Wallet-local LOC | ~3,820 |
| Anza-glue LOC | ~1,500 |

**V0.1 conclusion:** Anza + SPL give us a strong primitive base (~41% of features). The remaining ~59% is wallet code — split roughly evenly between (a) infrastructure Anza simply doesn't ship (persistence, encryption, FFI, PAL, CLI) and (b) orchestration + policy around Anza primitives (retry, polling, footgun guards, auto-prepends). **No feature requires writing cryptographic primitives from scratch** — all crypto (Ed25519, Argon2id, AES-256-GCM, BIP-39, SLIP-0010) is available via 3rd-party crates (`solana-sdk`, `ed25519-bip32`, `bip39`, `argon2`, `aes-gcm`).

## Solana fee model + sol-wallet-core integration

Solana's fee model differs from EVM's gas-metering model — fees are pre-declared (compute budget) rather than burned on execution. Three independent fee components, each handled by `sol-wallet-core` at a different code path.

### Three fee components

| Component | Unit | Default | Source | When charged |
| --- | --- | --- | --- | --- |
| **Base fee** | lamports per signature | 5000 lamports | cluster-enforced (`solana_fee_structure::FeeStructure`) | per tx, per signature |
| **Priority fee** | micro-lamports per CU | 0 µLamports/CU | `ComputeBudgetInstruction::set_compute_unit_price` (prepend ix) | per CU consumed × priority |
| **Compute limit** | CU units (capped) | 150,000 CU | `ComputeBudgetInstruction::set_compute_unit_limit` (prepend ix) | declares max CUs tx may consume |
| **Rent** | lamports | variable (~0.00204 SOL for ATA) | `solana_sdk::system_instruction::create_account` + rent sysvar | one-time per new account (ATA creation, mint deploy, nonce account) |

**Total cost per tx:** `5000 lamports + (CU consumed × priority_fee_µLamports / 1_000_000) + rent` (only when new accounts created).

**No EVM-style gas metering.** Solana tx declares CU limit upfront; cluster aborts tx if exceeded (returns `InstructionError::ComputationalBudgetExceeded`). Failed txs still pay base fee — lost fee. Wallet must pre-flight CU estimate.

### sol-wallet-core integration layers

#### Layer 1: Compute Budget auto-attach (every tx)

`tx::builder` prepends 2 instructions as the FIRST 2 ix in every tx message:

```rust
// From tx_serde.rs / compute_budget.rs (test row 15)
use solana_compute_budget_program::ComputeBudgetInstruction;

let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(150_000);   // default
let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(0);         // default 0 priority

let mut message = Message::new(
    &[cu_limit_ix, cu_price_ix, /* ... user ix ... */],
    Some(&fee_payer.pubkey()),
);
```

**Behavior:**

- Defaults applied silently on every send (CLI and library)
- Override via `cfg.compute_unit_limit` + `cfg.compute_unit_price` config keys
- CLI flag `--cu-limit <N>` (V0.2 only — V0.1 has fixed 150_000)
- CLI flag `--priority-fee <micro_lamports>` overrides default 0
- Sent in fixed position (ix[0] = limit, ix[1] = price) — cluster parses in order

**Crank-up for known tx types** (computed in `compute_budget.rs::cu_limit_for_tx_type`):

| Tx type | Estimated CU | V0.1 limit |
| --- | --- | --- |
| Native SOL transfer | ~150 CU | 150,000 (1000x safety margin) |
| SPL `transfer_checked` (classic) | ~5 CU | 150,000 |
| SPL `transfer_checked` (Token-2022, no hooks) | ~150 CU | 150,000 |
| SPL + 2 ATA creates (sender + recipient) | ~15,000 CU | 200,000 (auto-upgraded) |
| Memo + SPL transfer | ~6,000 CU | 150,000 |
| Composite (memo + SPL + 2 ATA creates) | ~22,000 CU | 200,000 |

V0.1 over-allocates CU (150k default) to avoid `ComputationalBudgetExceeded` failures. V0.2 will use simulation (`simulate_transaction`) + actual consumption (`TransactionStatus.compute_units_consumed`) to right-size.

#### Layer 2: Pre-flight balance check (before broadcast)

`tx::preflight.rs` (test row 19) runs before `send_transaction`:

```rust
// From preflight_balance.rs
let required = lamports_for(amount)
    + rent_for_new_accounts        // ~0.00204 SOL per new ATA
    + base_fee_for_signatures(1)    // 5000 lamports per signer
    + priority_fee_for(estimated_cu, cu_price_µ_lamports);

if balance < required {
    return Err(Error::BroadcastErrorKind::InsufficientFunds { required, available });
}
```

Returns BEFORE any RPC `send_transaction` call → no fee lost on balance check failure.

#### Layer 3: Retry on stale blockhash (no fee waste)

`tx::broadcast::send_with_retry` (test row 30) handles `BlockhashNotFound` gracefully:

- Attempt 1: fresh blockhash → sign → send
- On `BlockhashNotFound`: retry with FRESH blockhash + FRESH signature (Ed25519 sign is non-deterministic over blockhash)
- 3 attempts max, exponential backoff (100ms → 200ms → 400ms)
- Each attempt burns 5000 lamports if it lands on stale blockhash before cluster rejects

**Critical:** retry must re-fetch blockhash each time, NOT reuse the original. Re-signing with same keypair + fresh blockhash = new signature (Ed25519 includes blockhash in signed bytes).

#### Layer 4: send-speedup (priority fee bump, V0.1)

`tx::submit_send_speedup` (test row 35) emits a NEW tx with higher priority fee — Solana has NO RBF (replace-by-fee) like Bitcoin. Old tx stays on chain; speedup is a separate tx.

```rust
let new_cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(bumped_priority_fee);
let new_tx = sign_new_tx_with_priority_fee(new_cu_price_ix);
```

Original sig stays valid until confirmed or expired (~60-90s blockhash lifetime).

### Crate surface (V0.1 dependencies)

| Crate | Version | Used for | V0.1 touched? |
| --- | --- | --- | --- |
| `solana_compute_budget_program` | 4.2.2 | `ComputeBudgetInstruction::{set_compute_unit_limit, set_compute_unit_price}` builders | YES (every tx) |
| `solana_fee_structure` | 0.1.0 | `FeeStructure::calculate_fee(message, FeeBudgetLimits)` | NO (deferred V0.2) |
| `solana_native_token` | 0.1.0 | `sol_str_to_lamports` + `lamports_to_sol` | NO (deferred V0.2; V0.1 uses wallet-local helpers) |
| `solana_system_interface` | latest | `SystemInstruction::CreateAccount` + rent sysvar | YES (ATA creation, account creation) |

**V0.1 simplification:** rather than vendor `solana_fee_structure` for full fee math, V0.1 uses wallet-local helpers (`amount_lamport.rs` + `preflight_balance.rs`) that compute the simple case: `5000 lamports + rent + priority_fee`. Faster to compile, no transitive deps. Full `FeeStructure::calculate_fee` lands V0.2.

### CLI surface

```bash
# Default — 150k CU + 0 priority fee, base 5000 lamports
sol wallet send --mnemonic --to <addr> --amount 1

# Override priority fee (V0.1 already supported)
sol wallet send --mnemonic --to <addr> --amount 1 --priority-fee 50000
# → sets set_compute_unit_price(50000) µLamports/CU

# Override CU limit (V0.2 only — V0.1 ignores --cu-limit)
sol wallet send --mnemonic --to <addr> --amount 1 --cu-limit 200000
# → sets set_compute_unit_limit(200000)

# Confirm prompt on drain / mainnet
sol wallet send --mnemonic --to <addr> --amount <near_balance> --confirm-mainnet
# → prompt: "Send 9.99 SOL to <addr>? [y/N]"
```

### Test coverage (wallet-core matrix)

| Row | Test case | File |
| --- | --- | --- |
| 15 | builder round-trip × 7 (incl. Compute Budget ix) | `tx_serde.rs` + `compute_budget.rs` |
| 19 | preflight balance check before broadcast | `preflight_balance.rs` |
| 30 | `send_with_retry` on stale blockhash (3 attempts) | `send_with_retry.rs` |
| 35 | `submit_send_speedup` (new sig + higher priority) | `submit_send_speedup_local.rs` |

CLI rows covering Compute Budget integration:

| CLI row | Scenario |
| --- | --- |
| 6 | Compute Budget auto-attach (first 2 ix = set_compute_unit_limit + set_compute_unit_price) |
| 10 | Send-speedup (new sig + higher priority fee) |
| 11 | Blockhash retry (fresh blockhash + fresh sig each attempt) |
| 13 | Dry-run (`simulate_transaction` reports CU consumed before broadcast) |

### V0.x roadmap

| Version | Fee enhancement | Source crate | Reference |
| --- | --- | --- | --- |
| V0.1 | Base fee + CU limit/price prepended | `solana_compute_budget_program` 4.2.2 | this section |
| V0.1 | Preflight balance check | `sol-wallet-core::preflight_balance` | test row 19 |
| V0.1 | Retry on stale blockhash | `sol-wallet-core::send_with_retry` | test row 30 |
| V0.1.5 | Prioritization fee oracle (`get_recent_prioritization_fees` → 50th percentile) | `solana_client::rpc_client::RpcClient` | "auto-prioritization" cross-cutting R |
| V0.1.5 | `--priority-fee auto` flag | CLI | deferred |
| V0.2 | `--cu-limit <N>` CLI flag (V0.1 has fixed 150_000) | CLI | deferred (Complete Feature Inventory C3) |
| V0.2 | Full fee calc via `FeeStructure::calculate_fee` | `solana_fee_structure` 0.1.0 | Group 1 "Compute Budget: full CU cost model" |
| V0.2 | Micro-lamports fee math via `solana_native_token` | `solana_native_token` 0.1.0 | Group 1 |
| V0.2 | `simulate_transaction` returns CU consumed → auto-right-size next send | `solana_client` | "RPC: auto right-size" deferred |

### EVM comparison (Tron reference)

Tron uses Energy + Bandwidth (2-resource model): every tx burns Energy + Bandwidth proportionally to opcodes; Energy is stake-recoverable, Bandwidth is free (recoverable) up to 5 per day.

Solana uses Compute Units (1-resource model): every tx declares max CU upfront; CU consumed is reported after execution. No recovery mechanism.

| Aspect | Tron | Solana |
| --- | --- | --- |
| Fee resources | Energy + Bandwidth (2) | Compute Units (1) |
| Declaration | implicit (sum of opcodes) | explicit (`set_compute_unit_limit` ix) |
| Priority mechanism | free Energy/Bandwidth OR burn TRX | `set_compute_unit_price` µLamports/CU |
| Rent | none (account creation free) | yes (~0.00204 SOL per ATA) |
| Fee recovery | yes (stake Energy recover) | no |
| Failed-tx fee | only Energy burned for opcodes run; revert is partial | full base fee (5000 lamports) even if tx aborts on CU exceeded |

`sol-wallet-core` does NOT carry over Tron's dual-resource model — it implements Solana's single-resource model.

### Cross-reference

- Section C (Compute Budget) — Complete Feature Inventory: line ~1957
- Cross-cutting R row 10 (Compute Budget auto-attach): line ~2480
(Q4 cleanup: V8 spike row removed; Compute Budget auto-attach lives in deep-dive §C Compute Budget table)
- `solana_compute_budget_program` crate feature map: line 1137
- Section "Compute Budget (every transaction prepends)" — V0.1 SPIKE calibration: line ~1960

## Test scenario — sol-wallet-core V0.1

Library-level counterpart to the network-level `## Test Scenario` (CLI + endpoint driven, surfpool + devnet). Maps each V0.1 module + each public entry point to a concrete test, its layer, its fixture, and an observable pass criterion. Every row runs with `cargo test -p sol-wallet-core`. Rows marked **local** additionally require `surfpool` spawned via `tokio::process::Command`; rows marked **devnet** require `RUN_SOL_DEVNET=1` + funded test keypair.

### Layer split

| Layer        | Command                                                                  | Network                          | Runs on                  |
| ------------ | ------------------------------------------------------------------------ | -------------------------------- | ------------------------ |
| Unit         | `cargo test -p sol-wallet-core --lib`                                    | none (pure + mock)               | every commit, all targets |
| Local (CI)   | `RUN_SOL_LOCAL=1 cargo test -p sol-wallet-core --test '*'`                | surfpool (auto-spawn, ephemeral port) | every PR (desktop CI) |
| Devnet       | `RUN_SOL_DEVNET=1 cargo test -p sol-wallet-core --test 'devnet_*'`        | `https://api.devnet.solana.com`  | `workflow_dispatch` only  |
| Mainnet gate | `RUN_SOL_MAINNET=1 cargo test -p sol-wallet-core --test 'mainnet_*'`      | `https://api.mainnet-beta.solana.com` | manual pre-release |

Unit tests must pass on `aarch64-apple-ios` and `aarch64-linux-android` builds; local tests are desktop-only (gated behind `#[cfg(not(any(target_os = "ios", target_os = "android")))]`). Devnet + mainnet gates are loud-RED per gated-live-test convention; never silent skip.

### Module × test scenario matrix (20 modules)

| #  | Module                              | Scenario                                                                  | Layer        | Fixture                                                | Pass criterion                                                                                                                  |
| -- | ----------------------------------- | ------------------------------------------------------------------------- | ------------ | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| 1  | `keys.rs`                           | BIP-39 mnemonic → seed → SLIP-0010 derive `m/44'/501'/0'/0/0`            | unit         | BIP-39 English test vectors + SLIP-0010 path           | derived XPrv matches `ed25519_bip32` vector byte-for-byte                                                                      |
| 2  | `keys.rs`                           | xprv → Ed25519 `Pubkey` (32-byte verification key) → base58               | unit         | known keypair from KAT                                 | base58 pubkey equals expected; `Pubkey::is_on_curve` true                                                                     |
| 3  | `amount.rs`                         | `Amount::from_str_in("1.5", Unit::Sol)` → `as_lamport()`                  | unit         | table-driven: `0`, `1`, `1.5`, `0.000000001`, overflow | `1.5` → `1_500_000_000` lamport; overflow returns `Error::AmountOverflow`, no panic                                           |
| 4  | `amount.rs`                         | round-trip `as_lamport` → `display_in(Unit::Sol)`                        | unit         | property test (proptest, 10k cases)                    | `display_in(as_lamport(x)) == x` for all 9-decimal inputs                                                                      |
| 5  | `crypto/argon2.rs`                  | Argon2id KDF determinism + parameter pinning                              | unit         | fixed salt + passphrase                                | same input → same 32-byte key; params (m=64MB, t=3, p=1) match pinned constants                                                |
| 6  | `crypto/aes_gcm.rs`                 | AES-GCM encrypt → decrypt round-trip; tamper detection                    | unit         | random 32-byte key + nonce                             | plaintext recovered; single-bit ciphertext flip → `Error::DecryptFailed`                                                       |
| 7  | `crypto/mnemonic_cipher.rs`         | mnemonic encrypt-at-rest → decrypt, correct + wrong passphrase           | unit         | 12-word + 24-word mnemonics                            | correct passphrase recovers mnemonic; wrong one errors without leaking plaintext                                               |
| 8  | `wallet/persist.rs`                 | create → save → load → sign round-trip                                   | unit         | `tempfile::tempdir()` + `FileWalletStorage`            | loaded wallet signs identically to the in-memory original                                                                      |
| 9  | `wallet/persist.rs`                 | file permissions + atomic write                                          | unit         | tempdir                                                | wallet file mode is `0600`; no `.tmp` residue after write; interrupted write leaves prior file intact                          |
| 10 | `wallet/id.rs`                      | UUID v4 uniqueness + name → address lookup                               | unit         | 1000 generated wallets                                 | no id collision; `WalletManager::lookup("cold")` resolves to stored base58 address                                              |
| 11 | `config.rs`                         | `SolanaConfig` TOML save → load; per-cluster defaults                     | unit         | tempdir + all 3 clusters                               | round-trip equality; devnet URL + cluster id match `## Solana Networks` table                                                   |
| 12 | `config.rs`                         | recent blockhash cache TTL (60s)                                          | unit         | mock `Clock` trait (PAL)                               | second call within 60s hits cache (0 RPC); call at 61s refetches                                                               |
| 13 | `spl/disambig.rs`                   | Token-2022 vs classic SPL detection + reject wrong program                | unit         | captured mainnet mint owners                            | `TokenkegQ...` returns classic; `TokenzQdB...` returns Token-2022; caller passing wrong program → `Error::InvalidTokenProgram`   |
| 14 | `spl/decimals.rs`                   | dynamic decimals via `Mint::unpack` (never hardcoded)                     | unit         | captured mint accounts (USDC, USDT, PYUSD)             | decimals match per-mainnet registry at [line 1032-1128](docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md#L1032-L1128); USDC=6, PYUSD=6 |
| 15 | `tx/builder.rs` (SOL + SPL)         | all 7 builders: build → bincode encode → decode → compare                | unit         | one fixture per builder                                | decoded message equals input params for every builder (100% builder coverage)                                                   |
| 16 | `tx/sign.rs`                        | sign flow + recent_blockhash attach + fee-payer derivation                | unit         | known keypair + fixed blockhash                         | signed tx verifies via `solana_sdk::transaction::verify`; non-default fee-payer reflected in signature count                    |
| 17 | `tx/sign.rs`                        | `sign_only_tx` cold-sign path                                             | unit         | fixed key + fixed message                              | returns `(tx_base64, sig)`; performs no RPC; re-signing same input deterministic                                               |
| 18 | `tx/ata.rs`                         | auto-derive ATA + auto-create-ATA-when-missing pre-pend                   | unit         | mock `RpcClient` returning missing-then-present ATA    | first call returns `None` → tx gets `create_associated_token_account_idempotent` ix prepended                                  |
| 19 | `tx/preflight.rs`                   | balance ≥ amount + fee + rent check                                       | unit         | mock `RpcClient` returning fixed balance               | under-funded send returns `Error::InsufficientFunds` before broadcast                                                           |
| 20 | `tx/receipt.rs`                     | `TransactionStatus` JSON → struct; TxSummary serde                       | unit         | captured devnet tx success + RPC error JSON             | success parses slot + confirmations; error variant maps per `J. Error classification`                                           |
| 21 | `chain/spki.rs`                     | SPKI pin match / mismatch                                                 | unit         | real leaf cert fixture + a deliberately wrong pin      | matching pin verifies; wrong pin returns `Error::SpkiPinMismatch { expected, actual }`                                           |
| 22 | `chain/solana_client.rs`            | 12 RPC methods against mock HTTP server                                   | unit         | `wiremock` stubs                                        | each method parses response; HTTP 5xx maps to `Error::Transport`, never panic                                                    |
| 23 | `error.rs`                          | all 21 `From` impls + `Debug` redaction                                   | unit         | one constructed error per source                        | each source maps to intended variant; `Debug` output contains no mnemonic or secret bytes                                        |
| 24 | `ffi/panic.rs`                      | panic-message scrubber                                                    | unit         | fuzz inputs containing mnemonics + hex keys             | scrubbed output never contains any input secret (regex fuzz, 10k cases)                                                        |
| 25 | `ffi/`                              | C ABI smoke: create wallet → sign → status code                           | unit         | in-process `cdylib` call                               | returns expected status code; no panic crosses FFI boundary                                                                     |
| 26 | `tx/` end-to-end SOL transfer        | `submit_sol` full flow                                                    | **local**    | surfpool + funded sender                               | receipt slot ≥ 1; sender lamport delta = amount; recipient lamport delta = amount                                              |
| 27 | `tx/` end-to-end SPL transfer held   | `submit_spl_transfer` to a held ATA                                       | **local**    | surfpool + deployed USDC-style mint                    | receipt SUCCESS; recipient token balance = sent amount; CU consumed ≈ 5_000 (per `C` table)                                   |
| 28 | `tx/` end-to-end SPL first-time recv | `submit_spl_transfer` to fresh ATA                                        | **local**    | surfpool + deployed mint + fresh recipient              | receipt SUCCESS; ~0.00204 SOL rent charged to sender; 2 ATAs created (sender + recipient)                                      |
| 29 | `tx/` end-to-end SPL approve         | `submit_spl_approve` + `spl_allowance` view                               | **local**    | surfpool + deployed mint + delegate                     | approve accepted; allowance view returns approved amount                                                                       |
| 30 | `tx/broadcast.rs`                   | `send_with_retry` on stale blockhash                                      | **local**    | surfpool with stale-block injection                    | first attempt returns `BlockhashNotFound`; retry succeeds with fresh blockhash; sig emitted                                      |
| 31 | `tx/wait.rs`                        | `wait_for_confirm` success + timeout                                      | **local**    | real sig, plus bogus sig for timeout path               | success returns receipt; bogus sig returns `Error::ConfirmTimeout` within budget                                                 |
| 32 | `chain/solana_client.rs` end-to-end  | `get_health` boot probe + cluster detect                                  | **local**    | surfpool                                                 | `get_health` returns `Ok`; cluster enum resolves to `Localnet`                                                                  |
| 33 | SPL USDC mainnet smoke              | `submit_spl_transfer` $0.001 USDC self-send                               | **devnet** + **mainnet** | funded test keypair (RUN_SOL_MAINNET=1) + 20 USDC devnet airdrop | mainnet USDC transfer confirmed on explorer; amount = 100_000 µUSDC; CU consumed reported in receipt               |
| 34 | full stack                          | transport failure: RPC pointed at closed port                             | unit         | `http://127.0.0.1:9999`                                | `Error::Transport` within 30s timeout; no panic, no hang                                                                       |

### Entry-point coverage (16 functions)

| Entry point                       | Covered by rows | Note                                                                                              |
| --------------------------------- | --------------- | ------------------------------------------------------------------------------------------------- |
| `submit_sol`                      | 16, 26, 34      | preflight, happy path, transport failure                                                          |
| `submit_spl_transfer`             | 27, 28          | held recipient and fresh recipient — the rent delta is the assertion                               |
| `submit_spl_approve`              | 29              | approve plus allowance read-back                                                                   |
| `submit_send_speedup`             | 30              | new sig with higher priority fee (no RBF on Solana)                                                |
| `sign_only_tx`                    | 17              | offline path — asserted to perform no RPC                                                          |
| `wait_for_confirm`                | 31              | success and timeout                                                                               |
| `derive_keypair` (SLIP-0010)      | 1               | BIP-39 + derivation path                                                                          |
| `WalletManager::create_with_mnemonic` | 8, 9         | persist round-trip + atomic write                                                                 |
| `WalletManager::import_from_phrase` | 8             | same path as create, persists imported wallet                                                     |
| `WalletManager::import_from_pk`   | 8               | raw 32-byte secret import                                                                         |
| `WalletManager::unlock`           | 8               | password → decrypted secret                                                                       |
| `WalletManager::summary`          | 8               | returns only public fields (no mnemonic leak per Cross-cutting R)                                  |
| `WalletManager::list` / `delete` / `rename` | 10    | name/UUID lookup + lifecycle                                                                      |
| `WalletManager::pubkey`           | 2               | Ed25519 verification key export (NOT `xpub` — documented gap)                                      |
| `chain::get_balance`              | 26, 34          | native lamport fetch, mock + transport failure                                                    |
| `chain::spl_balance` / `spl_allowance` | 27, 28, 29 | ATA balance + delegate allowance view-call                                                        |
| `chain::mint_token_program` (disambig) | 13, 27, 28 | Token-2022 vs classic SPL detection before every SPL transfer                                      |
| FFI `sol_wallet_*` (12 fns)       | 25              | one C ABI smoke covers all 12 exports via in-process `cdylib` call                                 |

The 6 stake entry points (`stake::create`, `stake::delegate`, `stake::deactivate`, `stake::withdraw`, `stake::split`, `stake::merge`) ship in V0.2 (per [P. non-features table](docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md)) — out of V0.1 test scope.

### Fixture inventory

| Fixture                      | Source                                          | Used by rows |
| ---------------------------- | ----------------------------------------------- | ------------ |
| BIP-39 / SLIP-0010 vectors   | `bip39` + `ed25519_bip32` crate tests           | 1, 2         |
| Known keypair + base58       | captured from mainnet via `RpcClient::getAccount` | 2, 8, 16  |
| Token mint accounts (mainnet) | captured JSON via `getAccountInfo`             | 14           |
| Solana RPC responses         | captured success + error JSON, checked in       | 20, 22       |
| Leaf certificate + SPKI hash | captured from `api.mainnet-beta.solana.com`     | 21           |
| `tokens/mainnet.json`        | bundled in the crate                            | 14           |
| MockUSDC-style SPL mint      | deployed by surfpool helper in test fixture     | 27, 28, 29   |

Secrets never checked in. Devnet + mainnet runs read `SOLANA_TEST_MNEMONIC` from environment (CI secret); local runs derive throwaway keypairs from fixed byte arrays that hold no real funds.

### Coverage gates

- 100% line coverage on `crypto/`, `amount.rs`, `tx/builder.rs`, `spl/disambig.rs` — pure and deterministic, no excuse for gaps.
- Every builder in the 7-builder table has a round-trip test (row 15). Adding a builder without a test fails review.
- Every `Error` variant constructed at least once across the suite (row 23), so no variant ships unreachable or unrendered.
- No test prints a mnemonic, seed, or private key — enforced by redaction assertions in rows 23 and 24.
- Every FFI export exercised at least once (row 25) — silent drop on `cargo build` is a defect.
- Gated live tests (devnet + mainnet) fail loud-RED per L13 step 11 — never `#[ignore]`-away.

### File structure × test cases

Maps each row of the matrix above to the physical test file. Two test layers:

1. **`crates/sol-wallet-core/tests/`** — library tests (32 files, ~3290 LOC). Semantic file names matching test case concerns. `mock_spl_usdc.rs` lives here (sol-wallet-core test scope; reusable by CLI tests).
2. **`crates/sol/tests/`** — CLI binary tests (10 files, ~1400 LOC). Future home for end-to-end CLI command tests (22 commands × 4-layer coverage). Imports `mock_spl_usdc.rs` from sol-wallet-core via `path = "../../sol-wallet-core/tests/mock_spl_usdc.rs"` (test-only dep) OR re-exports via `sol-wallet-core::test_helpers::mock_spl_usdc` (preferred — keeps CLI test crate surface clean).

Solana swap vs Tron: wire-format coverage = `v2_protobuf_roundtrip` → `tx_serde` (bincode round-trip). One new file added (`mock_spl_usdc.rs`) — no Tron analog because TRC-20 deploys via TronBox Solidity compile while SPL mints deploy via 2-tx Rust helper.

**Layout — library tests (`crates/sol-wallet-core/tests/`):**

```text
rust-wallet-app/crates/sol-wallet-core/tests/   # File structure defined; implementation lands Phase 0 (stubs) → Phase 9 per Test scenario — sol-wallet-core V0.1
├── address_derivation.rs             # row 1, 2: SLIP-0010 + base58 pubkey
├── bip39_mnemonic.rs                 # row 1 part: English wordlist (12/15/18/21/24 words)
├── amount_lamport.rs                 # row 3, 4: Amount + as_lamport + proptest round-trip
├── argon2_kdf.rs                     # row 5: Argon2id determinism + parameter pinning
├── aes_gcm_cipher.rs                 # row 6: AES-GCM round-trip + tamper detection
├── mnemonic_encrypt.rs               # row 7: mnemonic encrypt-at-rest round-trip
├── wallet_persist.rs                 # row 8, 9, 10: atomic write + mode 0600 + name lookup
├── solana_config.rs                  # row 11: SolanaConfig TOML + per-cluster defaults
├── blockhash_cache.rs                # row 12: recent blockhash TTL cache
├── token2022_disambig.rs             # row 13, 14 part: Token-2022 vs classic SPL guard + decimals via unpack
├── stablecoin_registry.rs            # row 14 part: USDC/USDT/PYUSD/USDS mainnet mint registry
├── spl_instruction.rs               # row 15, 18: SPL transfer_checked + auto-ATA-create
├── tx_serde.rs                       # row 15 SOL part: bincode tx message round-trip
├── compute_budget.rs                 # row 15: Compute Budget builder + auto-attach
├── sign_tx.rs                        # row 16: full sign + send with recent_blockhash
├── sign_only.rs                      # row 17: sign_only_tx cold path
├── preflight_balance.rs              # row 19: balance check before send (insufficient funds)
├── tx_status_parse.rs                # row 20: TransactionStatus JSON parse + TxSummary serde
├── spki_pin.rs                       # row 21: SPKI pin match/mismatch
├── rpc_methods_mock.rs               # row 22: 12 RPC methods against wiremock
├── error_mapping.rs                  # row 23: Error From + Debug redaction
├── placeholder.rs                    # row 24, 25: FFI panic scrubber + C ABI smoke (when FFI lands)
├── submit_sol_local.rs               # row 26: submit_sol E2E on surfpool
├── submit_spl_local_held.rs          # row 27: submit_spl E2E on held ATA (mock USDC)
├── submit_spl_local_fresh.rs         # row 28: submit_spl E2E on fresh ATA (rent delta)
├── submit_spl_local_approve.rs       # row 29: submit_spl_approve E2E + allowance view
├── send_with_retry.rs                # row 30, 31: send_with_retry on stale blockhash + wait_for_confirm
├── boot_probe_local.rs               # row 32: get_health boot probe
├── mainnet_smoke.rs                  # row 33: mainnet $0.001 USDC self-send (gated RUN_SOL_MAINNET=1)
├── transport_failure.rs              # row 34: transport failure (closed port)
├── submit_send_speedup_local.rs      # row 35: submit_send_speedup (new sig + higher priority fee)
├── wallet_lifecycle.rs               # row 36, 37, 38: import_from_pk + summary + list/delete/rename lifecycle
├── common/
│   ├── mod.rs                        # re-export shim: `pub mod mock_spl_usdc; pub mod surfpool_spawn; pub mod faucet; pub mod keypair_fixture;`
│   ├── mock_spl_usdc.rs              # SHARED with crates/sol/tests/ (CLI tests) via test-helpers feature
│   ├── surfpool_spawn.rs             # `spawn_surfpool(port) -> Child` (split from common/mod.rs)
│   ├── faucet.rs                     # `airdrop_surfpool(rpc, pubkey, lamports)` (split from common/mod.rs)
│   └── keypair_fixture.rs            # `throwaway_keypair() -> Keypair` (split from common/mod.rs)
└── fixtures/
    └── spki_pin_test_cert.der        # leaf cert captured from api.mainnet-beta.solana.com
```

**Shared test helpers (cross-crate reuse — see `### Shared helpers reused from sol-wallet-core` in Test scenario — sol CLI for canonical version):**

CLI tests live in `crates/sol/tests/` and reuse the entire `common/` + `fixtures/` trees from `crates/sol-wallet-core/tests/` via `#[path]` attribute or `sol_wallet_core::test_helpers` re-export. Full reuse table + 3 reuse mechanisms + 5 drift-prevention guarantees documented in the CLI section. **Canonical location: `### Shared helpers reused from sol-wallet-core` in Test scenario — sol CLI.**

**File × test case matrix (matrix row → physical file, semantic names):**

| Matrix row | Test file | Cluster | Gate env | LOC est. |
| --- | --- | --- | --- | --- |
| 1 (BIP-39 + SLIP-0010) | `address_derivation.rs` + `bip39_mnemonic.rs` | unit | none | ~120 + ~40 |
| 2 (xprv → base58 pubkey) | `address_derivation.rs` | unit | none | (shared with row 1) |
| 3 (Amount → as_lamport) | `amount_lamport.rs` | unit | none | ~80 |
| 4 (round-trip proptest) | `amount_lamport.rs` | unit | none | (shared with row 3) |
| 5 (Argon2id KDF) | `argon2_kdf.rs` | unit | none | ~60 |
| 6 (AES-GCM round-trip) | `aes_gcm_cipher.rs` | unit | none | ~80 |
| 7 (mnemonic cipher) | `mnemonic_encrypt.rs` | unit | none | ~80 |
| 8 (persist round-trip) | `wallet_persist.rs` | unit | none | ~150 |
| 9 (file perms + atomic) | `wallet_persist.rs` | unit | none | (shared with row 8) |
| 10 (WalletManager lookup) | `wallet_persist.rs` | unit | none | (shared with row 8) |
| 11 (SolanaConfig TOML) | `solana_config.rs` | unit | none | ~80 |
| 12 (blockhash cache TTL) | `blockhash_cache.rs` | unit | none | ~50 |
| 13 (Token-2022 disambig) | `token2022_disambig.rs` | unit | none | ~80 |
| 14 (decimals via unpack) | `token2022_disambig.rs` + `stablecoin_registry.rs` | unit | none | ~40 + ~60 |
| 15 (builder round-trip × 7) | `tx_serde.rs` + `spl_instruction.rs` + `compute_budget.rs` | unit | none | ~80 + ~80 + ~60 |
| 16 (sign + recent_blockhash) | `sign_tx.rs` | unit | none | ~100 |
| 17 (sign_only_tx cold) | `sign_only.rs` | unit | none | ~80 |
| 18 (auto-ATA-create) | `spl_instruction.rs` (ATA pre-pend section) | unit | none | (shared with row 15) |
| 19 (preflight balance) | `preflight_balance.rs` | unit | none | ~60 |
| 20 (TransactionStatus JSON) | `tx_status_parse.rs` | unit | none | ~80 |
| 21 (SPKI pin) | `spki_pin.rs` | unit | none | ~80 |
| 22 (12 RPC methods mock) | `rpc_methods_mock.rs` | unit | none | ~150 |
| 23 (Error From + Debug) | `error_mapping.rs` | unit | none | ~70 |
| 24 (FFI panic scrubber) | `placeholder.rs` (FFI section) | unit | none | ~50 (when FFI lands) |
| 25 (FFI C ABI smoke) | `placeholder.rs` (FFI section) | unit | none | ~100 (shared with row 24) |
| 26 (submit_sol E2E) | `submit_sol_local.rs` | **local** | RUN_SOL_LOCAL=1 | ~150 |
| 27 (submit_spl held E2E) | `submit_spl_local_held.rs` | **local** | RUN_SOL_LOCAL=1 | ~120 |
| 28 (submit_spl fresh E2E) | `submit_spl_local_fresh.rs` | **local** | RUN_SOL_LOCAL=1 | ~120 |
| 29 (submit_spl_approve E2E) | `submit_spl_local_approve.rs` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 30 (send_with_retry E2E) | `send_with_retry.rs` (surfpool warp) | **local** | RUN_SOL_LOCAL=1 | ~60 |
| 31 (wait_for_confirm E2E) | `send_with_retry.rs` (surfpool wait) | **local** | RUN_SOL_LOCAL=1 | ~60 (shared with row 30) |
| 32 (get_health boot probe) | `boot_probe_local.rs` | **local** | RUN_SOL_LOCAL=1 | ~50 |
| 33 (mainnet $0.001 USDC) | `mainnet_smoke.rs` | **mainnet** | RUN_SOL_MAINNET=1 | ~150 |
| 34 (transport failure) | `transport_failure.rs` | unit | none | ~50 |
| 35 (`submit_send_speedup`) | `submit_send_speedup_local.rs` | **local** | RUN_SOL_LOCAL=1 | ~120 |
| 36 (`import_from_pk` raw 32-byte secret) | `wallet_lifecycle.rs` | unit | none | ~80 |
| 37 (`summary` no-mnemonic leak) | `wallet_lifecycle.rs` | unit | none | (shared with row 36) |
| 38 (`list` / `delete` / `rename`) | `wallet_lifecycle.rs` | unit | none | (shared with row 36) |

**File → test cases covered (reverse mapping — semantic names):**

| Test file | Rows covered | LOC est. |
| --- | --- | --- |
| `address_derivation.rs` | 1, 2 | ~120 |
| `bip39_mnemonic.rs` | 1 (English wordlist) | ~40 |
| `amount_lamport.rs` | 3, 4 | ~80 |
| `argon2_kdf.rs` | 5 | ~60 |
| `aes_gcm_cipher.rs` | 6 | ~80 |
| `mnemonic_encrypt.rs` | 7 | ~80 |
| `wallet_persist.rs` | 8, 9, 10 | ~150 |
| `solana_config.rs` | 11 | ~80 |
| `blockhash_cache.rs` | 12 | ~50 |
| `token2022_disambig.rs` | 13, 14 (Token-2022 guard + decimals) | ~80 |
| `stablecoin_registry.rs` | 14 (USDC/USDT/PYUSD/USDS mints) | ~60 |
| `spl_instruction.rs` | 15, 18 (SPL transfer_checked + auto-ATA-create) | ~80 |
| `tx_serde.rs` | 15 (SOL builder bincode round-trip) | ~80 |
| `compute_budget.rs` | 15 (Compute Budget builder + auto-attach) | ~60 |
| `sign_tx.rs` | 16 | ~100 |
| `sign_only.rs` | 17 | ~80 |
| `preflight_balance.rs` | 19 | ~60 |
| `tx_status_parse.rs` | 20 | ~80 |
| `spki_pin.rs` | 21 | ~80 |
| `rpc_methods_mock.rs` | 22 | ~150 |
| `error_mapping.rs` | 23 | ~70 |
| `placeholder.rs` | 24, 25 (when FFI lands) | ~150 |
| `submit_sol_local.rs` | 26 | ~150 |
| `submit_spl_local_held.rs` | 27 | ~120 |
| `submit_spl_local_fresh.rs` | 28 | ~120 |
| `submit_spl_local_approve.rs` | 29 | ~100 |
| `send_with_retry.rs` | 30, 31 | ~120 |
| `boot_probe_local.rs` | 32 | ~50 |
| `mainnet_smoke.rs` | 33 | ~150 |
| `transport_failure.rs` | 34 | ~50 |
| `submit_send_speedup_local.rs` | 35 | ~120 |
| `wallet_lifecycle.rs` | 36, 37, 38 | ~150 |
| `mock_spl_usdc.rs` | helper for rows 27, 28 | ~80 |
| `common/mod.rs` | shared helpers | ~120 |
| `fixtures/spki_pin_test_cert.der` | row 21 binary fixture | ~2 KB |

**Total LOC (library tests):** ~3290 across `crates/sol-wallet-core/tests/` (32 files + 2 fixtures). Maturation target: V0.1 ships ≤ 3500 LOC. ~210 LOC headroom for unforeseen edge cases.

**Total LOC (CLI binary tests):** ~3380 across `crates/sol/tests/` (~32 files: 22 CLI commands + 4 gap-fill rows 23-26 + devnet harness + setup helper + ~32 per-row test files). Imports `mock_spl_usdc.rs` + `common/` helpers from sol-wallet-core via `#[path] = "..."` attribute OR `sol_wallet_core::test_helpers::common::*` re-export.

**Combined V0.1+ test footprint:** ~4690 LOC across both crates. ~40% reduction vs duplicated helpers (CLI tests would otherwise duplicate MockUSDC deploy logic = ~120 LOC + drift risk).

### Coverage audit — wallet-core API ↔ test cases

Verifies every public `sol-wallet-core` API maps to ≥1 test case in the matrix above (rows 1-38). Each shipped API has at least one row; each row maps to ≥1 API.

| Wallet-core API | Section | Test row(s) | Status |
| --- | --- | --- | --- |
| `tx::build_sol_transfer` | A1 | 15 (builder), 26 (E2E sol) | covered |
| `client.request_airdrop` | A2 | 33 (mainnet smoke) | covered |
| `client.get_balance` / `chain::get_balance` | A3 | 26, 34 (transport failure path) | covered |
| `WalletManager::unlock(id).balance_sol()` | A4 | 26 (via unlock during E2E) | covered |
| `client.get_account` | A5 | 22 (wiremock embedded) | covered |
| `tx::build_spl_transfer_checked` | B1 | 15 (builder), 27, 28 (E2E held + fresh ATA) | covered |
| `tx::build_spl_approve` | B2 | 15, 29 (E2E approve) | covered |
| `tx::build_spl_revoke` | B3 | 15 (builder only — no E2E test; revoke = approve(amount=0)) | covered (builder) |
| `tx::build_spl_close_account` | B4 | 15 (builder only) | covered (builder) |
| `tx::build_spl_burn` | B5 | n/a — not exposed V0.1 (no mint authority) | gap (documented, expected) |
| `tx::build_spl_mint_to` | B6 | n/a — not exposed V0.1 | gap (documented) |
| `tx::build_spl_set_authority` | B7 | n/a — not exposed V0.1 | gap (documented) |
| `client.get_token_account_balance` | B8 | 27, 28 (SPL E2E balance check) | covered |
| `chain::spl_allowance` | B9 | 29 (E2E approve + allowance view) | covered |
| `client.get_token_supply` | B10 | n/a — internal only V0.1, display V0.2 | deferred V0.2 (documented) |
| `chain::spl_decimals` | B11 | 14 (decimals via unpack) | covered |
| `chain::mint_token_program` (disambig) | B12 | 13 (Token-2022 disambig) | covered |
| `spl::get_associated_token_address_with_program_id` | B13 | 13, 27, 28 (ATA derivation) | covered |
| `tx::prepend_create_ata` (idempotent) | B14 | 18 (auto-ATA-create), 28 (fresh ATA E2E) | covered |
| `tx::prepend_close_ata` | B15 | 15 (builder only — close surface V0.1 minimal) | covered (builder) |
| `tx::build_sync_native` | B16 | 15 (builder only — wrapped SOL deferred) | covered (builder) |
| Token-2022 extension awareness | B17 | 13 (disambig + extension flag) | covered |
| Transfer Hook CPI dispatch | B18 | n/a — V0.1.5 (requires BPF loader) | deferred V0.1.5 |
| Compute Budget auto-attach (CU limit + CU price) | C1, C2 | 15 (builder), 26-29 (E2E tx includes ix), R (cross-cutting R10) | covered |
| Heap frame request | C3 | n/a — V0.2 (program deployment only) | deferred V0.2 |
| Loaded accounts data size limit | C4 | n/a — V0.2 (large tx) | deferred V0.2 |
| Recent prioritization fees oracle | C5 | n/a — V0.1.5 (auto-prioritization) | deferred V0.1.5 |
| Simulate transaction (--dry-run) | C6 | CLI matrix row 13 (--dry-run flag) | covered (CLI, not wallet-core) |
| `tx::broadcast::send_with_retry` | D (blockhash) | 30 (E2E stale blockhash retry) | covered |
| Recent blockhash fetch + attach | D1, D7 | 16 (sign + recent_blockhash), 12 (blockhash cache TTL) | covered |
| BlockhashNotFound detection + backoff | D3, D4, D5 | 30 (retry E2E) | covered |
| Never re-sign identical bytes | D6 | 16 (sign_tx uses fresh blockhash each retry) | covered |
| `tx::wait_for_confirm` (poll) | E1, E3, E4, E5 | 31 (E2E wait_for_confirm) | covered |
| WS subscription for realtime | E2 | n/a — V0.1 HTTP-poll only (WS V0.1.5+) | deferred V0.1.5 |
| `BlockCleanedUp` handling | E7 | 30 (surfpool warp injects expiry) | covered (via retry) |
| Slot finality (root) | E8 | 31 (E2E wait_for_confirm + finalize flag) | covered |
| BIP-39 generate + validate | F1, F2 | 1 (BIP-39 + SLIP-0010) | covered |
| PBKDF2-HMAC-SHA512 seed | F3 | 1 (BIP-39 seed) | covered |
| SLIP-0010 master xprv + derive | F4, F5 | 1, 2 (address_derivation + bip39_mnemonic) | covered |
| `XPrv::sign` (Ed25519) | F7 | 16 (sign_tx), 17 (sign_only), 2 (xprv → pubkey via sign-verify round-trip) | covered |
| `xpub` export | F8 | n/a — NOT POSSIBLE for Ed25519 SLIP-0010 | gap (documented, chain-design) |
| `Pubkey::is_on_curve` | G5 | 2 (base58 pubkey + on-curve check) | covered |
| PDA derivation (`find_program_address`) | G6 | n/a — V0.1.5 (PDA for staking) | deferred V0.1.5 |
| `Pubkey::short` display | G4 | n/a — not exposed V0.1 | gap (documented) |
| Address validation (parse + is_on_curve) | G7 | 2 (base58 parse + curve check) | covered |
| Argon2id KDF | H1, H2, H3 | 5 (argon2_kdf) | covered |
| AES-256-GCM encryption + nonce | H4, H5, H6 | 6 (aes_gcm_cipher) | covered |
| Mnemonic encrypt-at-rest | H7 | 7 (mnemonic_encrypt) | covered |
| Wallet file format (JSON) | H7 | 8 (wallet_persist round-trip) | covered |
| Atomic write (rename) + mode 0600 | H8 | 9 (file perms + atomic) | covered |
| Password verification | H9 | 8 (persist round-trip includes password check) | covered |
| `get_latest_blockhash` (RPC) | I1 | 16 (sign_tx), 12 (cache TTL) | covered |
| `send_transaction` (RPC) | I2 | 26, 27, 28, 29, 33 (E2E sends) | covered |
| `simulate_transaction` (RPC) | I3 | CLI matrix row 13 (--dry-run) | covered (CLI) |
| `get_signature_statuses` (RPC) | I4 | 31 (wait_for_confirm) | covered |
| `get_account_info` (RPC) | I5 | 13 (mint_token_program), 14 (mint decimals), 22 (wiremock) | covered |
| `get_multiple_accounts_info` (RPC) | I6 | 22 (wiremock) | covered |
| `get_minimum_balance_for_rent_exemption` (RPC) | I7 | 28 (fresh ATA rent pre-flight) | covered |
| `get_balance` (RPC) | I8 | 26, 34 | covered |
| `get_token_account_balance` (RPC) | I9 | 27, 28 | covered |
| `get_token_supply` (RPC) | I10 | n/a — internal only V0.1 | deferred V0.2 |
| `get_token_accounts_by_owner` (RPC) | I11 | n/a — V0.1.5 (wallet list-tokens) | deferred V0.1.5 |
| `request_airdrop` (RPC) | I12 | 33 (mainnet smoke) | covered |
| `get_health` (RPC) | I13 | 32 (boot_probe_local) | covered |
| `get_recent_prioritization_fees` (RPC) | I14 | n/a — V0.1.5 (auto-prioritization) | deferred V0.1.5 |
| `get_version` (RPC) | I15 | 22 (wiremock) | covered |
| `get_epoch_info` (RPC) | I16 | n/a — V0.1.5 (stake) | deferred V0.1.5 |
| Error variants (21) + Debug redaction | J (21 variants) | 23 (error_mapping covers all `From` impls + Debug) | covered |
| Exit code mapping (0-5) | J (5 codes) | 34 (transport failure → exit 3), R8 (cross-cutting stable exit codes) | covered |
| FFI panic scrubber (regex fuzz) | K (safety) | 24 (placeholder.rs when FFI lands) | covered (pending FFI surface) |
| FFI C ABI smoke (12 exports) | K (12 functions) | 25 (placeholder.rs when FFI lands) | covered (pending FFI surface) |
| PAL `WalletStorage` (4 methods) | L | 8 (persist round-trip uses FileWalletStorage), 9 (atomic), 10 (list_ids via WalletManager lookup) | covered |
| PAL `PlatformInfo` (4 methods) | L | 8, 9 (uses PlatformInfo via persist) | covered |
| PAL `NetworkClient` (2 methods) | L | 26-29, 32-34 (all use NetworkClient against surfpool) | covered |
| PAL `Clock` (1 method) | L | 12 (blockhash cache TTL uses mock Clock) | covered |
| `--json` output helper | N | CLI matrix row (all 22 commands) | covered (CLI) |
| Stable exit codes | R8 | 34 (Error::Transport → exit 3), J (cross-cutting) | covered |
| Confirmation prompts (mainnet/drain/unlimited SPL approval) | R4 | CLI matrix rows 1, 5, 8 (--confirm-mainnet flag) | covered (CLI) |
| SPKI pin env-only | R5 | 21 (spki_pin match/mismatch), R6 (no-pin default) | covered |
| All crypto delegated to sol-wallet-core | R7 | R7 invariant tested via cross-cutting: no test shadow-crypto exists (no direct sk in CLI); coverage = `secret_key_*` never appears in `crates/sol/tests/` | covered |
| Mnemonic handling policy (L28/F49/L12 H-1) | R2 | CLI integration (--password-stdin, mode-0600 file reads); wallet-core row 8 (persist uses Zeroizing) | covered |
| `wallet show` no-mnemonic leak | R3 | 37 (summary returns only public fields) | covered |
| Blockhash refresh on `BlockhashNotFound` | R12 | 30 (send_with_retry E2E) | covered |
| `submit_send_speedup` | CLI row 10 / R | **35 (submit_send_speedup_local)** | covered (added 2026-09-09 audit) |
| `WalletManager::import_from_pk` | API surface | **36 (wallet_lifecycle)** | covered (added 2026-09-09 audit) |
| `WalletManager::summary` | API surface | **37 (wallet_lifecycle)** | covered (added 2026-09-09 audit) |
| `WalletManager::list` / `delete` / `rename` | API surface | **38 (wallet_lifecycle)** | covered (added 2026-09-09 audit) |

**Coverage verdict:** Every shipped V0.1 wallet-core API has ≥1 corresponding test row. Four new rows (35-38) added in this audit pass to close gaps found by reverse-engineering the API list against the matrix. Total: 38 rows × 32 test files = ~3290 LOC.

**Gaps (all documented, all deferred or out-of-scope):**

| Gap | Reason | V0.x target |
| --- | --- | --- |
| `tx::build_spl_burn` / `mint_to` / `set_authority` | wallet owns no mint authority | never (out of scope) |
| `xpub` export | NOT POSSIBLE — Ed25519 SLIP-0010 has no parent public key | never (chain-design) |
| `Pubkey::short` display | not exposed V0.1 | V0.2 |
| PDA derivation | V0.1.5 (stake use case) | V0.1.5 |
| WS subscription | V0.1 HTTP-poll sufficient | V0.1.5 |
| Heap frame / loaded accounts size | V0.2 (program deployment) | V0.2 |
| Prioritization fees oracle | V0.1.5 (auto-prioritization) | V0.1.5 |
| `get_token_supply` | V0.2 (tokens list display) | V0.2 |
| `get_token_accounts_by_owner` | V0.1.5 (wallet list-tokens) | V0.1.5 |
| `get_recent_prioritization_fees` | V0.1.5 | V0.1.5 |
| `get_epoch_info` | V0.1.5 (stake) | V0.1.5 |
| Transfer Hook CPI dispatch | V0.1.5 (BPF loader required) | V0.1.5 |
| Stake account ops (create/delegate/withdraw) | V0.2 | V0.2 |
| Multi-sig (Squads) | V0.3 | V0.3 |
| Compressed NFT (Bubblegum) | V0.3 (Metaplex license) | V0.3 |
| Confidential Transfer (Token-2022 zk) | V0.3 (solana-zk-sdk) | V0.3 |

No unaccounted gap. **Audit passes.**

**Total estimated LOC: ~3290** (final post-refactor count — see `### Total LOC` paragraphs above for split: library 3290 + CLI 3380, but CLI tests live in `crates/sol/tests/` not `crates/sol/tests/`; the latter not yet landed).

### Devnet (V0.1 default test cluster)

Cluster shared between `sol-wallet-core/tests/` (rows 33 + devnet variations) and `crates/sol/tests/cli_sol_devnet.rs` (CLI binary tests) — same RPC URL, same faucet procedure, same gate env. Full cluster documentation (RPC endpoints, mints, faucet procedures, state lifecycle, raw URLs, RPC method usage pattern, end-to-end Rust quick-start) lives in `## Solana Networks` → `### Devnet (V0.1 default test cluster)` and its 11 `####` subsections. **This subsection only documents the 6 test scenarios + cluster-specific mint addresses unique to test execution.**

**Test scenarios (Devnet variations — shared CLI + library):**

Same 6 scenarios run via both layers (single source of truth on devnet cluster). CLI commands shown for readability; the underlying wallet-core API called by each CLI command is in parentheses. **To switch cluster + airdrop + verify balances:** `sol config set-cluster devnet` (see Solana Networks section for full faucet procedure).

| #   | Scenario                       | CLI command (calls wallet-core API)                                            | Difference from Local                                                          | Pass criteria                                                                                            |
| --- | ------------------------------ | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| 1   | SOL transfer                   | `sol wallet send --mnemonic` (→ `tx::submit_sol`)                               | Same                                                                            | tx accepted on real network; sig visible on `https://explorer.solana.com/?cluster=devnet`                |
| 2   | SPL transfer (classic)         | `sol spl send --token USDC` (→ `tx::submit_spl_transfer`)                       | Use devnet USDC mint `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`            | tx accepted; balance visible on explorer devnet tab                                                       |
| 3   | SPL transfer (Token-2022)      | `sol spl create-mint` + `sol spl send` (→ `chain::mint_token_program` + `tx::submit_spl_transfer`) | Use devnet PYUSD test mint (deploy own)               | tx accepted; Token-2022 footer hook (if any) fires; balance visible                                       |
| 4   | Token-2022 vs classic footgun  | `sol spl send --token <classic-mint>` (→ `tx::disambig::reject_wrong_token_program`) | Same                                                                            | caller passes wrong program → `Error::InvalidTokenProgram` BEFORE signing (no fee lost)                   |
| 5   | Network failure recovery       | `sol config set-rpc http://127.0.0.1:9999` + `sol balance` (→ `chain::get_balance`) | Point RPC at `http://127.0.0.1:9999` (closed port)                             | CLI returns exit code 3 (`Error::Transport`) within 30s timeout; no panic                                  |
| 6   | Confirmation polling — `finalized` | `sol tx wait --wait-finalized` (→ `tx::wait_for_confirm`)                    | Wait ~12 slots for root commitment                                              | `sol tx wait --sig <sig> --wait-finalized` returns `commitment: finalized`                                |

**Cluster-specific mint addresses (apply to BOTH wallet-core + CLI tests):**

- **Devnet USDC:** `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` (6 decimals, classic SPL, deployed by Circle for devnet faucet)
- **Devnet PYUSD:** NOT pre-deployed — wallet owner deploys own mint via `sol spl create-mint` (V0.2 CLI), then mints to own ATA
- **Devnet USDT/USDS:** NOT deployed — same as PYUSD, deploy own
- **Localnet USDC:** MOCK mint deployed by `crates/sol-wallet-core/tests/common/mock_spl_usdc.rs::deploy_mock_usdc` (1M initial supply, 6 decimals, classic SPL)
- **Mainnet USDC:** `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` (6 decimals, classic SPL, Circle-issued)

Mock vs real mint resolution happens via `SOL_MOCK_USDC_MINT` env var (set by `mock_spl_usdc::register_mock_alias`); CLI alias lookup chain in `crates/sol-wallet-core/src/spl/alias.rs` reads env first, then per-cluster registry. Same chain used by library and CLI tests.

### CI workflow — `rust-sol-core-ci.yml` (planned, not yet landed)

Mirror of `.github/workflows/rust-tron-core-ci.yml` adapted for Solana. Three jobs: operator smoke (matrix: surfpool-local + wallet-core), devnet integration, mobile compile (iOS + Android arm64). Surfpool replaces TronBox — no Docker daemon needed, pure binary spawn via `tokio::process::Command`.

```yaml
name: rust-sol-core

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  workflow_dispatch: {}

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  # Live RPC + surfpool smoke gate. Runs full `sol-wallet-core` test suite
  # with `--include-ignored` against surfpool (auto-spawn per test) +
  # devnet + mainnet. Matches Tron's `rust-test-operator-smoke` shape.
  #
  # Matrix fan-out: two child legs run in parallel:
  #   - `submit-local`: surfpool-local E2E (submit_sol/spl + send-speedup
  #     + send_with_retry + boot_probe + transport_failure)
  #   - `wallet-core`: every OTHER integration test binary (unit + wiremock
  #     RPC + crypto + SPL instruction encode + Token-2022 disambig + etc.)
  #
  # RUN_SOL_LOCAL=1 / RUN_SOL_DEVNET=1 / RUN_SOL_MAINNET=1 flip gated
  # tests from skip-and-return to actual round-trip. --test-threads=1
  # serializes RPC round-trips so a flaky RPC cannot interleave state.
  # SOL_MOCK_USDC_MINT env surfaces the deployed mock mint from local
  # tests so CLI alias resolution finds it.
  rust-test-operator-smoke:
    name: Rust test (operator smoke — ${{ matrix.name }} #[ignore])
    runs-on: ubuntu-latest
    timeout-minutes: 30
    env:
      RUSTFLAGS: "-D warnings"
    strategy:
      fail-fast: false
      matrix:
        include:
          - name: submit-local
            env:
              RUN_SOL_LOCAL: "1"
              RUN_SOL_DEVNET: "1"
              RUN_SOL_MAINNET: "1"
              SOL_MOCK_USDC_MINT: "auto-deploy"  # see crates/sol-wallet-core/tests/common/mock_spl_usdc.rs::register_mock_alias
            command: >-
              cargo test -p sol-wallet-core
              --test submit_sol_local
              --test submit_spl_local_held
              --test submit_spl_local_fresh
              --test submit_spl_local_approve
              --test submit_send_speedup_local
              --test send_with_retry
              --test boot_probe_local
              --test transport_failure --
              --include-ignored --nocapture --test-threads=1
          - name: wallet-core
            env:
              RUN_SOL_NILE: "1"  # cluster alias — handled by aliases/alias.rs
              RUN_SOL_MAINNET: "1"
            command: >-
              cargo test -p sol-wallet-core
              --test address_derivation --test bip39_mnemonic
              --test amount_lamport --test argon2_kdf
              --test aes_gcm_cipher --test mnemonic_encrypt
              --test wallet_persist --test solana_config
              --test blockhash_cache --test token2022_disambig
              --test stablecoin_registry --test spl_instruction
              --test tx_serde --test compute_budget
              --test sign_tx --test sign_only
              --test preflight_balance --test tx_status_parse
              --test spki_pin --test rpc_methods_mock
              --test error_mapping
              --test wallet_lifecycle
              --test placeholder --
              --ignored --nocapture --test-threads=1
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.98.1"
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rust-wallet-app -> target
      - name: cargo build (workspace)
        working-directory: rust-wallet-app
        run: cargo build --workspace
      - name: cargo test (operator smoke — ${{ matrix.name }} #[ignore])
        working-directory: rust-wallet-app
        run: ${{ matrix.command }}

  # Solana devnet smoke (analog of Tron's `rust-test` harness). Exercises the
  # CLI binary end-to-end against `api.devnet.solana.com` — no Docker,
  # no testcontainers (surfpool replaces both). The CLI test suite lives
  # in `crates/sol/tests/cli_sol_devnet.rs` (separate harness from the
  # wallet-core crate tests).
  #
  # RUN_SOL_DEVNET=1 flips gated devnet tests from skip-and-return to
  # actual RPC round-trip. SOLANA_TEST_MNEMONIC from CI secret funds
  # the test wallet. RUN_SOL_LOCAL=1 also runs the local surfpool CLI
  # suite in the same leg.
  rust-test-devnet-smoke:
    name: Rust test (CLI devnet smoke — surfpool + api.devnet.solana.com)
    runs-on: ubuntu-latest
    timeout-minutes: 30
    env:
      RUSTFLAGS: "-D warnings"
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.98.1"
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rust-wallet-app -> target
      - name: cargo build (workspace — builds `sol` CLI binary)
        working-directory: rust-wallet-app
        run: cargo build --workspace
      - name: cargo test (CLI devnet smoke — RUN_SOL_LOCAL + RUN_SOL_DEVNET)
        # RUN_SOL_LOCAL=1 runs surfpool-per-test local suite.
        # RUN_SOL_DEVNET=1 runs real-cluster devnet suite (gated on
        # SOLANA_TEST_MNEMONIC CI secret). --include-ignored catches
        # any #[ignore]-marked cases. --nocapture surfaces operator
        # runbook output. --test-threads=1 serializes RPC round-trips.
        working-directory: rust-wallet-app
        env:
          RUN_SOL_LOCAL: "1"
          RUN_SOL_DEVNET: "1"
          SOLANA_TEST_MNEMONIC: ${{ secrets.SOLANA_DEVNET_TEST_MNEMONIC }}
        run: |
          cargo test -p sol-wallet-core --tests -- \
            --include-ignored --nocapture --test-threads=1

  # Mobile compile-only gate. `sol-wallet-core` MUST compile for iOS +
  # Android arm64 with no source changes. Catches platform-incompatible
  # deps (e.g. ring/rustls needing xcrun) the moment they enter the tree.
  #
  # Runtime smoke on real device deferred to V0.2 (per mobile CI plan in
  # the mobile build gate doc). Skips cleanly until sol-wallet-core crate
  # exists in `crates/sol-wallet-core/`.
  mobile-check:
    name: Mobile compile-only (iOS + Android arm64)
    runs-on: macos-latest
    timeout-minutes: 25
    env:
      RUSTFLAGS: "-D warnings"
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.98.1"
          targets: aarch64-apple-ios,aarch64-linux-android
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rust-wallet-app -> target
      - name: cargo check (aarch64-apple-ios)
        if: runner.os == 'macOS'
        working-directory: rust-wallet-app
        run: |
          if [ ! -d crates/sol-wallet-core ]; then
            echo "sol-wallet-core not created yet (plan Phase 0) - skipping"
            exit 0
          fi
          cargo check -p sol-wallet-core --target aarch64-apple-ios
      - name: cargo check (aarch64-linux-android)
        if: runner.os == 'Linux'
        working-directory: rust-wallet-app
        run: |
          if [ ! -d crates/sol-wallet-core ]; then
            echo "sol-wallet-core not created yet (plan Phase 0) - skipping"
            exit 0
          fi
          cargo check -p sol-wallet-core --target aarch64-linux-android
```

**Key deltas vs Tron's CI:**

| Aspect | Tron | Solana |
| --- | --- | --- |
| Local harness | `trc20_local.rs` (testcontainers + TronBox Docker) | `submit_*_local*.rs` (surfpool binary, no Docker) |
| Service container | `docker:dind` (TronBox needs Docker) | none (surfpool is pure binary, no daemon) |
| Gate envs | `RUN_TRON_LOCAL`, `RUN_TRON_NILE`, `RUN_TRON_MAINNET` | `RUN_SOL_LOCAL`, `RUN_SOL_DEVNET`, `RUN_SOL_MAINNET` |
| Mock fixture | `MockTRC20.bin` (compiled Solidity) | `mock_spl_usdc.rs::deploy_mock_usdc` (2-tx Rust helper) |
| RPC client | `reqwest::Client` (vendored anychain-tron) | `solana_client::nonblocking::RpcClient` (surfpool binary on ephemeral port) |
| MSRV | 1.98.1 | 1.98.1 (Anza SDK requires same) |
| Mobile compile | iOS + Android arm64 (Tron rustls backend) | same (Anza SDK uses ring; same skip pattern) |
| Branch rule | PRs target `rust-tron-core`, not `main` | PRs target `main` directly (no integration branch yet) |

**Tron reference:** `.github/workflows/rust-tron-core-ci.yml` lines 1-283 (3 jobs, 30-min timeout, matrix fan-out, mobile compile skip-guard). Solana CI mirrors this structure 1:1 with cluster + helper swaps.

### See also (Test scenario — sol-wallet-core)

- Network-level scenarios (CLI + endpoints): `## Test Scenario`
- CLI-level scenarios (22 commands): `## Test scenario — sol CLI`
(Q4 cleanup: V1-V12 spike ladder removed; canonical test file structure lives in `## Test scenario — sol-wallet-core V0.1` File structure × test cases + `## Test scenario — sol CLI` File structure)
- Build order these tests follow: `### TDD sequencing (per L13 step 2-4)`
- Public API inventory under test: `### Public APIs shipped in V0.1`

---

## Test scenario — sol CLI

Network-level CLI counterpart to `## Test scenario — sol-wallet-core`. Each of the 22 V0.1 commands exercised end-to-end against surfpool (default), devnet (gated), and mainnet (Q4 smoke). Surfpool = ephemeral validator per-test (parallel-safe). Devnet = real cluster conformance. Mainnet = $0.001 USDC self-send per Q4 analog.

### Local (surfpool, CI default)

`tokio::process::Command` spawns `surfpool` per test on an ephemeral port. Container-less — pure binary. Boots in <2 sec, 0 MB disk footprint, full RPC parity, auto-advance blockhash every 400ms.

**Setup (one-time):**

```bash
# Toolchain check (CI runs this automatically)
cargo --version                 # 1.98.1 (MSRV)
surfpool --version              # any 0.6.x+

# First test run spawns surfpool per-test (auto, ~2s)
RUN_SOL_LOCAL=1 cargo test --test cli_local -- --nocapture
```

**Stablecoin mint in local tests:** `--token USDC` in rows 2-4, 9 refers to the **mock 6-decimal classic SPL mint** deployed by the surfpool test helper (`mock_spl_usdc::deploy_mock_usdc` in `crates/sol-wallet-core/tests/common/mock_spl_usdc.rs`, shared with CLI tests). Mainnet USDC mint `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` does NOT exist on localnet. Same decimals (6) + same program ID (`TokenkegQ...`) + same `transfer_checked` ix shape, different mint address. Real mainnet USDC = V0.1 row 33 (gated `RUN_SOL_MAINNET=1`, $0.001 self-send). Same logic for any `--token <stable>` alias — mock mint per cluster, never cross-cluster mint reuse.

**Test scenarios (22 commands × surfpool):**

| #   | Scenario                                    | Command                                                                                                          | Pass criteria                                                                                                                       |
| --- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Native SOL transfer                         | `sol wallet send --mnemonic --to <addr> --amount 1000000000`                                                    | tx accepted; sender balance -1 SOL; recipient balance +1 SOL; slot ≥ 1                                                              |
| 2   | SPL transfer (held ATA, classic)            | `sol spl send --mnemonic --token USDC --to <held-ata> --amount 50`                                              | tx accepted; recipient USDC balance = 50; CU consumed ≈ 5_000                                                                       |
| 3   | SPL transfer (fresh ATA — first-time recv)  | `sol spl send --mnemonic --token USDC --to <fresh-addr> --amount 50`                                            | tx accepted; ~0.00204 SOL rent charged to sender; 2 ATAs created                                                                     |
| 4   | SPL approve + allowance                     | `sol spl approve --mnemonic --token USDC --delegate <addr> --amount 1000` + `sol spl allowance --token USDC`   | approval tx accepted; allowance view returns 1000                                                                                   |
| 5   | Token-2022 vs classic SPL footgun guard     | `sol spl send --mnemonic --token <classic-mint-as-Token2022>` (caller forces wrong program)                       | command errors with `Error::InvalidTokenProgram` BEFORE signing; no sig emitted; no fee lost                                         |
| 6   | Compute Budget auto-attach                  | `sol wallet send --mnemonic --to <addr> --amount 1`                                                              | tx includes `set_compute_unit_limit(150_000)` + `set_compute_unit_price(0)` ix as first 2 instructions                             |
| 7   | Memo attach                                 | `sol wallet send --mnemonic --to <addr> --amount 1 --memo "invoice-12345"`                                       | tx includes `spl_memo::build_memo("invoice-12345")` ix; memo string visible on explorer                                              |
| 8   | Wallet-to-wallet SOL                        | `sol wallet send --wallet-id <trading-uuid> --to-wallet savings --amount 100000000`                              | resolves "savings" → stored address; transfer accepted                                                                              |
| 9   | Wallet-to-wallet SPL                        | `sol wallet send --wallet-id <hot-uuid> --to-wallet cold --token USDC --amount 100`                              | resolves "cold" → stored address; transfer accepted                                                                                 |
| 10  | Send-speedup (new sig, higher priority)     | `sol wallet send-speedup --wallet-id <uuid> --sig <stuck> --priority-fee 50000`                                  | new sig emitted; new tx has CU price = 50000 µLamports/CU; old sig remains on chain (no RBF)                                        |
| 11  | Blockhash retry on stale                    | inject stale blockhash via surfpool warp; `sol wallet send ...`                                                  | first attempt: `BlockhashNotFound`; retry succeeds; 3 attempts max; fresh blockhash each time                                        |
| 12  | Insufficient balance                        | `sol wallet send --mnemonic --to <addr> --amount 99999999999999`                                                 | tx REJECTED with `Error::BroadcastErrorKind::InsufficientFunds` before broadcast; exit code 3                                      |
| 13  | Dry-run (simulate)                          | `sol wallet send --mnemonic --to <addr> --amount 1 --dry-run`                                                    | tx simulated; CU consumed reported; no sig; no balance change                                                                       |
| 14  | Sign-only (no broadcast)                    | `sol wallet send --mnemonic --to <addr> --amount 1 --sign-only`                                                 | tx base64 emitted to stdout; no RPC `send_transaction` call                                                                         |
| 15  | Confirmation polling — success              | `sol tx wait --sig <sig> --timeout 60 --poll-interval 2`                                                        | returns `TransactionStatus { slot, confirmations >= 1, err: None, status: confirmed }`                                              |
| 16  | Confirmation polling — timeout              | `sol tx wait --sig <bogus-sig> --timeout 5 --poll-interval 1`                                                   | returns `Error::ConfirmTimeout { sig, waited_secs: 5 }` within budget; exit code 3                                                |
| 17  | Finalized commitment                        | `sol tx wait --sig <sig> --wait-finalized`                                                                       | returns when slot finalized (~12 slots after confirmed); `commitment: finalized` in JSON                                            |
| 18  | Wallet list across clusters                 | `sol wallet list --all-clusters`                                                                                 | JSON output lists wallets across mainnet-beta + devnet + localnet; per-cluster address resolution                                  |
| 19  | Wallet delete + rename                      | `sol wallet delete --id <uuid>` + `sol wallet rename --id <other-uuid> --to "savings-v2"`                         | delete removes encrypted blob (no recovery); rename persists new name; both verified via `wallet list`                              |
| 20  | Config switch cluster                       | `sol config set-cluster devnet`                                                                                  | writes `~/.config/sol/config.toml` with cluster=devnet, RPC=api.devnet.solana.com; `sol config show` reflects change                |
| 21  | Network failure recovery                    | `sol config set-rpc http://127.0.0.1:9999` (closed port) + `sol balance --address <addr>`                       | returns `Error::Transport` within 30s timeout; no panic, no hang                                                                    |
| 22  | Mainnet smoke (Q4 analog)                    | `sol wallet send --mnemonic "$MAINNET_MNEMONIC" --to "$OWN_ADDR" --token USDC --amount 100000 --confirm-mainnet`  | $0.10 USDC self-send on mainnet; receipt visible on explorer; gate env `RUN_SOL_MAINNET=1` required; manual trigger only              |

### File structure × test cases

Maps each row of the CLI test matrix above (rows 1-22 + devnet variants) to the physical test file under `rust-wallet-app/crates/sol/tests/` (CLI binary tests, parallel to `crates/tron/tests/` for the Tron CLI). Per-CLI-command files for traceability — each scenario row has its own greppable file.

**Layout (one file per scenario row):**

```text
rust-wallet-app/crates/sol/tests/   # CLI integration tests (Tier-2 harness, gated RUN_SOL_LOCAL=1 / RUN_SOL_DEVNET=1)
├── cli_sol_local.rs                 # main local harness — spawns surfpool + sets cluster config + airdrops
├── cli_sol_devnet.rs                # main devnet harness — gated RUN_SOL_DEVNET=1 + SOLANA_TEST_MNEMONIC
├── cli_mainnet_smoke.rs             # row 22 (Q4 analog): mainnet $0.001 USDC self-send — gated RUN_SOL_MAINNET=1
├── cli_wallet_create.rs             # row 1: `sol wallet create` — mnemonic → STDERR (red), wallet_id → STDOUT
├── cli_wallet_import.rs             # row 2: `sol wallet import` — mnemonic-file (mode 0600) + private-key-file paths
├── cli_wallet_show.rs               # row 11: `sol wallet show` — address + balance, NO mnemonic leak (row 37)
├── cli_wallet_list.rs               # row 9 part: `sol wallet list --all-clusters` — per-cluster JSON
├── cli_wallet_delete.rs             # row 9 part: `sol wallet delete --id <uuid>` — encrypted blob removed
├── cli_wallet_rename.rs             # row 9 part: `sol wallet rename --id --to <new_name>`
├── cli_wallet_balance.rs            # row 3: `sol wallet balance --wallet-id` — auto-decrypts
├── cli_wallet_send.rs               # rows 5, 8, 9: `sol wallet send` — SOL + SPL, --to + --to-wallet, --memo
├── cli_wallet_send_speedup.rs       # rows 10, 17: `sol wallet send-speedup` — new sig + higher priority fee
├── cli_address_new.rs               # row 3 part: `sol address new --mnemonic --index` — SLIP-0010 derivation
├── cli_address_pubkey.rs            # row 19: `sol address pubkey --wallet-id` — base58 Ed25519 verification key
├── cli_balance.rs                   # rows 3, 22: `sol balance --address` (SOL + SPL token)
├── cli_spl_send.rs                  # row 21: `sol spl send --token --to --amount` — auto-ATA-create
├── cli_spl_approve.rs               # rows 25, 30: `sol spl approve --delegate --amount` (0 = revoke)
├── cli_spl_balance.rs               # row 22: `sol spl balance --address --token`
├── cli_spl_allowance.rs             # row 30: `sol spl allowance --token --owner --delegate`
├── cli_tx_get.rs                    # row 7 part: `sol tx get --sig` — TxSummary JSON
├── cli_tx_wait.rs                   # rows 7, 15, 16, 17: `sol tx wait --sig --timeout --poll-interval --commitment`
├── cli_config_show.rs               # row 10: `sol config show [--json]`
├── cli_config_set_rpc.rs            # row 10 part: `sol config set-rpc <url>`
├── cli_config_set_cluster.rs        # row 10 part: `sol config set-cluster mainnet-beta|devnet|localnet`
├── cli_blockhash_retry.rs           # row 11: stale blockhash → BlockhashNotFound → send_with_retry (3 attempts)
├── cli_insufficient_balance.rs      # row 12: insufficient SOL → Error::BroadcastErrorKind::InsufficientFunds before broadcast
├── cli_dry_run.rs                   # row 13: --dry-run → simulate only, no broadcast
├── cli_sign_only.rs                 # row 14: --sign-only → output tx base64, no RPC call
├── cli_compute_budget_attach.rs     # row 6: first 2 ix = set_compute_unit_limit + set_compute_unit_price
├── cli_memo_attach.rs               # row 7 part: --memo string → spl_memo ix included
├── cli_token2022_footgun.rs         # row 5: caller passes wrong token_program_id → Error::InvalidTokenProgram BEFORE sign
├── cli_finalized_commitment.rs      # row 17: --wait-finalized → slot finalized (~12 slots after confirmed)
├── cli_network_failure.rs           # row 21: RPC closed port → Error::Transport within 30s timeout
├── common/
│   ├── mod.rs                       # CLI test helpers (CLI spawn, JSON parse, --json assertion)
│   ├── surfpool_spawn.rs            # (re-export from sol-wallet-core/tests/common/surfpool_spawn.rs)
│   ├── mock_spl_usdc.rs             # (re-export from sol-wallet-core/tests/common/mock_spl_usdc.rs)
│   ├── faucet.rs                    # (re-export from sol-wallet-core/tests/common/faucet.rs)
│   └── keypair_fixture.rs           # (re-export from sol-wallet-core/tests/common/keypair_fixture.rs)
└── fixtures/
    └── (none — CLI tests use sol-wallet-core fixtures via `#[path] = "../../../../crates/sol-wallet-core/tests/fixtures/"`)
```

**File × test case matrix (CLI scenario row → physical file):**

| Scenario row | Test file | Cluster | Gate env | LOC est. |
| --- | --- | --- | --- | --- |
| 1 (SOL transfer) | `cli_wallet_send.rs::sol_wallet_send_sol_e2e` | **local** | RUN_SOL_LOCAL=1 | ~150 |
| 2 (SPL transfer held) | `cli_spl_send.rs::sol_spl_send_held_ata_e2e` | **local** | RUN_SOL_LOCAL=1 | ~150 |
| 3 (SPL transfer fresh ATA) | `cli_spl_send.rs::sol_spl_send_fresh_ata_e2e` | **local** | RUN_SOL_LOCAL=1 | ~150 |
| 4 (SPL approve + allowance) | `cli_spl_approve.rs` + `cli_spl_allowance.rs` | **local** | RUN_SOL_LOCAL=1 | ~120 + ~100 |
| 5 (Token-2022 footgun) | `cli_token2022_footgun.rs` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 6 (Compute Budget auto-attach) | `cli_compute_budget_attach.rs` | **local** | RUN_SOL_LOCAL=1 | ~120 |
| 7 (Memo attach) | `cli_memo_attach.rs` | **local** | RUN_SOL_LOCAL=1 | ~80 |
| 8 (Wallet-to-wallet SOL) | `cli_wallet_send.rs::sol_wallet_to_wallet_sol` | **local** | RUN_SOL_LOCAL=1 | (in row 1 file) |
| 9 (Wallet-to-wallet SPL) | `cli_wallet_send.rs::sol_wallet_to_wallet_spl` | **local** | RUN_SOL_LOCAL=1 | (in row 1 file) |
| 10 (Send-speedup) | `cli_wallet_send_speedup.rs` | **local** | RUN_SOL_LOCAL=1 | ~120 |
| 11 (Blockhash retry) | `cli_blockhash_retry.rs` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 12 (Insufficient balance) | `cli_insufficient_balance.rs` | **local** | RUN_SOL_LOCAL=1 | ~80 |
| 13 (Dry-run) | `cli_dry_run.rs` | **local** | RUN_SOL_LOCAL=1 | ~80 |
| 14 (Sign-only) | `cli_sign_only.rs` | **local** | RUN_SOL_LOCAL=1 | ~80 |
| 15 (Confirmation polling — success) | `cli_tx_wait.rs::sol_tx_wait_success` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 16 (Confirmation polling — timeout) | `cli_tx_wait.rs::sol_tx_wait_timeout` | **local** | RUN_SOL_LOCAL=1 | (in row 15 file) |
| 17 (Finalized commitment) | `cli_finalized_commitment.rs` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 18 (Wallet list across clusters) | `cli_wallet_list.rs` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 19 (Wallet delete + rename) | `cli_wallet_delete.rs` + `cli_wallet_rename.rs` | **local** | RUN_SOL_LOCAL=1 | ~100 + ~80 |
| 20 (Config switch cluster) | `cli_config_set_cluster.rs` | **local** | RUN_SOL_LOCAL=1 | ~80 |
| 21 (Network failure recovery) | `cli_network_failure.rs` | unit | none | ~80 |
| 22 (Mainnet $0.001 USDC smoke) | `cli_mainnet_smoke.rs` | **mainnet** | RUN_SOL_MAINNET=1 | ~150 |
| Devnet row 1-6 | `cli_sol_devnet.rs` | **devnet** | RUN_SOL_DEVNET=1 | ~600 |
| 23 (wallet show no-mnemonic leak) | `cli_wallet_show.rs::sol_wallet_show_no_mnemonic_leak` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 24 (config show --json) | `cli_config_show.rs::sol_config_show_json` | **local** | RUN_SOL_LOCAL=1 | ~80 |
| 25 (config set-rpc persists) | `cli_config_set_rpc.rs::sol_config_set_rpc_persists` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| 26 (spl balance returns decimal + ata) | `cli_spl_balance.rs::sol_spl_balance_returns_decimal_ata` | **local** | RUN_SOL_LOCAL=1 | ~100 |
| Setup helpers (create wallet, set config) | `cli_sol_local.rs` (master setup) | **local** | RUN_SOL_LOCAL=1 | ~200 |
| Cross-cutting (all commands: --json mode) | `cli_sol_local.rs::assert_json_mode` | all | layered | ~50 |

**Coverage verdict:** All 22 CLI commands have a dedicated test file (or file co-located with related scenario). Total ~3380 LOC across ~32 files in `crates/sol/tests/` (26 matrix rows × 22 commands + 4 gap-fill rows 23-26 + devnet harness + setup helper).

**Shared helpers reused from sol-wallet-core:**

**Directory-level reuse:** CLI test suite reuses entire `common/` AND `fixtures/` directories from `crates/sol-wallet-core/tests/` — no duplication. CLI `tests/common/` is a re-export shim (4-line `mod.rs` plus re-exports); CLI `tests/fixtures/` is either a symlink, copy, or `#[path]` attribute pointing at the sol-wallet-core fixture dir.

| Helper / Fixture | Source | Used by CLI tests |
| --- | --- | --- |
| `common/mock_spl_usdc.rs` | `crates/sol-wallet-core/tests/common/mock_spl_usdc.rs` | `cli_spl_send.rs`, `cli_spl_approve.rs`, `cli_spl_balance.rs` (mock mint deploy + alias) |
| `common/surfpool_spawn.rs` | `crates/sol-wallet-core/tests/common/surfpool_spawn.rs` | `cli_sol_local.rs`, all `cli_*_local*.rs` files |
| `common/faucet.rs` | `crates/sol-wallet-core/tests/common/faucet.rs` | `cli_sol_local.rs` (airdrop_surfpool before tests) |
| `common/keypair_fixture.rs` | `crates/sol-wallet-core/tests/common/keypair_fixture.rs` | `cli_sol_local.rs`, `cli_mainnet_smoke.rs` (test wallet) |
| `fixtures/spki_pin_test_cert.der` | `crates/sol-wallet-core/tests/fixtures/spki_pin_test_cert.der` | future `cli_spl.rs` SPKI pin CLI tests (when SPKI pin CLI flag ships) |

**Reuse mechanisms (in preference order):**

1. **`sol_wallet_core::test_helpers::*` re-export** (preferred) — single `Cargo.toml` dev-dep `sol-wallet-core = { path = "../sol-wallet-core", features = ["test-helpers"] }`. CLI imports via `use sol_wallet_core::test_helpers::mock_spl_usdc::deploy_mock_usdc;`. Works across crate boundaries; survives file moves; single update point.

2. **`#[path] = "..."` attribute** (fallback when feature flag rejected) — `#[path = "../../../../crates/sol-wallet-core/tests/common/mock_spl_usdc.rs"] mod mock_spl_usdc;`. Works within same workspace; brittle to crate relocation but no API surface change needed.

3. **Symlink / copy** (last resort, deprecated) — `ln -s ../../../../crates/sol-wallet-core/tests/common ./common` at test build time. Works but breaks on Windows; visible to git as untracked symlink.

**Single source of truth guarantees:**

- `MOCK_USDC_INITIAL_SUPPLY_RAW = 1_000_000_000_000` (6 decimals, 1M USDC) defined once in sol-wallet-core/tests/common/mock_spl_usdc.rs. Hardcoding in CLI = drift risk.
- `register_mock_alias()` sets `SOL_MOCK_USDC_MINT` env var so CLI `--token USDC` resolves to deployed mint. Same function called by both library + CLI tests.
- Surfpool spawn helper spawns `tokio::process::Command("surfpool", "--port", &port.to_string(), "--faucet", "1000000000000")` — same args = same cluster state. Drift = test flake.
- Splitting `common/mod.rs` into per-helper sub-files (`mock_spl_usdc`, `surfpool_spawn`, `faucet`, `keypair_fixture`) gives each helper a single file. Each grows independently without merge conflicts on shared `mod.rs`.
- `fixtures/spki_pin_test_cert.der` captured once from `api.mainnet-beta.solana.com`; cert rotation auto-propagates to CLI tests.

### Decision matrix

| Stage                 | Network                          | Why                                                                                |
| --------------------- | -------------------------------- | ---------------------------------------------------------------------------------- |
| Unit (per-commit)     | none (mock + InMemoryStorage)    | fast, no I/O, deterministic                                                        |
| Integration CI        | surfpool (per-test ephemeral)    | deterministic, fast (~2s boot), parallel-safe via ephemeral port                   |
| Pre-release manual QA | devnet                           | real cluster conformance + faucet-funded edge cases                                |
| Mobile CI             | surfpool (desktop compile + run via host bridge) | no Docker fallback for mobile; **Round-1 grill Q-6: mobile CI matrix is `cargo build --target aarch64-apple-ios` + `cargo build --target aarch64-linux-android` (FFI compile only). Runtime mobile smoke via devnet deferred to V0.2.** |
| Production            | mainnet-beta                     | gated behind `RUN_SOL_MAINNET=1` + Q4 analog ($0.001 USDC self-send)               |

**Harness files (Phase 4 plan-aligned):**

- **Local CLI integration (CI + desktop dev):** `rust-wallet-app/crates/sol/tests/cli_sol_local.rs` + `.github/workflows/sol-cli.yml`. Spawns `surfpool` per-test via `tokio::process::Command` (no testcontainers). Surfpool spawn + readiness probe (`getHealth`) + cluster detect (`getVersion`) + commitment config (`confirmed`) probes run unconditionally. Deeper scenario rows (22 commands × 4-layer coverage) ship as `#[ignore]` stubs awaiting the CLI binary build step. Gated behind `RUN_SOL_LOCAL=1` + `surfpool` binary on PATH (loud-RED panic per gated-live-test convention).
- **Devnet CLI integration (manual QA only):** `rust-wallet-app/crates/sol/tests/cli_sol_devnet.rs` + `.github/workflows/sol-devnet.yml` (`workflow_dispatch` only — no automated CI per plan). Gated on `RUN_SOL_DEVNET=1` + `SOLANA_TEST_MNEMONIC`.

### CI workflow — `rust-sol-cli-ci.yml` (planned, not yet landed)

CLI binary integration tests. Single matrix job fanning out to 3 legs (local surfpool + devnet RPC + mainnet smoke). Lives in `crates/sol/tests/` (separate harness from wallet-core crate). Mirrors Tron's `.github/workflows/tron-nile.yml` shape but adapted for surfpool (no Docker).

```yaml
name: rust-sol-cli

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  workflow_dispatch: {}

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  # CLI binary E2E. Three legs (parallel):
  #   - `local`: surfpool auto-spawn per test (RUN_SOL_LOCAL=1). 22 CLI
  #     commands + 4 gap-fill rows + edge cases. Fast (~2s boot per test).
  #   - `devnet`: api.devnet.solana.com (RUN_SOL_DEVNET=1 + CI secret
  #     SOLANA_TEST_MNEMONIC). 6 devnet-variation rows from `## Test
  #     scenarios (Devnet variations)`. Slow (real RPC round-trips).
  #   - `mainnet`: mainnet-beta $0.001 USDC self-send (RUN_SOL_MAINNET=1).
  #     Q4 analog smoke. Manual trigger only via workflow_dispatch.
  #
  # fail-fast: false keeps passing legs' signal even when one reds.
  # --test-threads=1 serializes RPC round-trips.
  rust-cli-smoke:
    name: Rust CLI smoke (${{ matrix.name }})
    runs-on: ubuntu-latest
    timeout-minutes: 30
    env:
      RUSTFLAGS: "-D warnings"
    strategy:
      fail-fast: false
      matrix:
        include:
          - name: local
            env:
              RUN_SOL_LOCAL: "1"
              SOL_MOCK_USDC_MINT: "auto-deploy"
            command: >-
              cargo test -p sol-wallet-core --test submit_sol_local
              --test submit_spl_local_held --test submit_spl_local_fresh
              --test submit_spl_local_approve --test submit_send_speedup_local
              --test send_with_retry --test boot_probe_local --test transport_failure
              -- --include-ignored --nocapture --test-threads=1
          - name: devnet
            env:
              RUN_SOL_DEVNET: "1"
              SOLANA_TEST_MNEMONIC: ${{ secrets.SOLANA_DEVNET_TEST_MNEMONIC }}
            command: >-
              cargo test -p sol-wallet-core --test mainnet_smoke --
              --include-ignored --nocapture --test-threads=1
          - name: mainnet
            env:
              RUN_SOL_MAINNET: "1"
              SOLANA_TEST_MNEMONIC: ${{ secrets.SOLANA_MAINNET_TEST_MNEMONIC }}
            command: >-
              cargo test -p sol-wallet-core --test mainnet_smoke --
              --include-ignored --nocapture --test-threads=1
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.98.1"
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rust-wallet-app -> target
      - name: cargo build (workspace — builds `sol` CLI binary)
        # Pre-build the CLI binary so `assert_cmd::cargo_bin!("sol")` in
        # CLI integration tests resolves CARGO_BIN_EXE_sol correctly.
        working-directory: rust-wallet-app
        run: cargo build --workspace
      - name: cargo test (CLI smoke — ${{ matrix.name }})
        working-directory: rust-wallet-app
        run: ${{ matrix.command }}
```

**Key deltas vs Tron's CI:**

| Aspect | Tron | Solana |
| --- | --- | --- |
| Harness file location | `rust-wallet-app/spikes/tron-v1/tests/` | `rust-wallet-app/crates/sol-wallet-core/tests/` |
| CLI binary name | `tron` | `sol` |
| Local validator | TronBox Docker (testcontainers) | surfpool binary (no Docker) |
| Service container | `docker:dind` (TronBox) | none (surfpool pure binary) |
| Gate envs | `RUN_TRON_LOCAL`, `RUN_TRON_NILE`, `RUN_TRON_MAINNET` | `RUN_SOL_LOCAL`, `RUN_SOL_DEVNET`, `RUN_SOL_MAINNET` |
| CI secret | `TRON_NILE_TEST_MNEMONIC` | `SOLANA_DEVNET_TEST_MNEMONIC` + `SOLANA_MAINNET_TEST_MNEMONIC` |
| Test runner | `cargo test -p tron-v1 --tests` | `cargo test -p sol-wallet-core --test <test-file-name>` (one test per scenario) |
| Mobile compile | separate job in tron-core-ci | not yet — deferred until sol-wallet-core crate lands |

**Why no separate devnet CI workflow:** Tron's `.github/workflows/tron-nile.yml` exists as a separate `workflow_dispatch` job because the nile test suite needs manual faucet funding. Solana devnet has free airdrop (faucet.solana.com + Circle faucet) — no manual step needed. Therefore all 3 legs run on push + PR to `main` with matrix fan-out. `workflow_dispatch` retained for operator override.

**Why mock USDC mint auto-deploys:** `SOL_MOCK_USDC_MINT=auto-deploy` triggers `mock_spl_usdc::register_mock_alias` in `crates/sol-wallet-core/tests/common/mock_spl_usdc.rs` which:
1. Spawns fresh keypair for mock mint
2. Deploys via 2-tx (create_account + initialize_mint2)
3. Mints 1M USDC to payer ATA
4. Sets `SOL_MOCK_USDC_MINT` env var to deployed mint address

CLI alias resolution in `crates/sol-wallet-core/src/spl/alias.rs` reads `SOL_MOCK_USDC_MINT` before per-cluster registry, so `--token USDC` resolves to the deployed mint. Single test setup covers all SPL scenarios.

**Reference:** `.github/workflows/tron-nile.yml` (analog workflow, not yet inspected here — uses `RUN_TRON_NILE=1` + `TRON_TEST_MNEMONIC` secret + manual `workflow_dispatch`). Solana CI is simpler (3 legs vs Tron's 1 leg) because surfpool replaces Docker + free devnet airdrops replace manual faucet funding.

### See also (Test scenario — sol CLI)

- Library-level scenarios (modules + entry points): `## Test scenario — sol-wallet-core`
(Q4 cleanup: V1-V12 spike ladder removed; canonical test file structure lives in `## Test scenario — sol-wallet-core V0.1` File structure × test cases + `## Test scenario — sol CLI` File structure)
- Surfpool tool choice rationale: `## Local validator — surfpool (CHOSEN for V0.1)`
- Layer-level coverage targets: `### Test strategy (stable across versions)`
- Build order these tests follow: `### TDD sequencing (per L13 step 2-4)`

---



## Solana Wallet v0.1 — solana-sdk features NOT in V0.1 (monorepo coverage gap analysis)

`.local/solana-sdk/` (= `anza-xyz/solana-sdk` workspace at HEAD `main` 2026-09-08) ships **117 workspace members**. Of those, our V0.1 uses **11 directly** (solana-sdk 4.1.0, solana-program 4.1.0, solana-keypair 3.1.2, solana-signer 3.0.1, solana-message 4.6.0, solana-transaction 4.3.0, solana-instruction 3.5.0, solana-client 4.2.2, solana-rpc-client 4.2.2, solana-compute-budget-program 4.2.2, solana-rpc 4.2.2). That leaves **~106 subcrates unused**.

This section classifies each unused subcrate by relevance to a user-facing wallet — broken into 3 groups:
1. **Wallet-relevant BUT deferred** — subcrates that are usable by a wallet but intentionally pushed to V0.1.5/V0.2/V0.3 (or `not possible` per Solana HD design).
2. **Wallet-relevant BUT excluded** — subcrates blocked by license / chain-design / governance reasons.
3. **NOT wallet-relevant** — primitives for on-chain program authors, validator implementations, zk proofs, WASM bindings — informative only.

### Group 1: Wallet-relevant but deferred (~25 subcrates)

These ARE listed in `## Solana Wallet v0.1 — Complete Feature Inventory` already (sections B/C/D/F/P/Q), but documented here so wallet authors know the upstream source per capability.

| Capability | Subcrate to use in deferred version | Why deferred | V0.x target |
| --- | --- | --- | --- |
| **Token-2022 Transfer Hook CPI dispatch** | `spl_token_2022::extension::transfer_hook` (loader instructions for hook program) + `solana_pubkey::Pubkey::find_program_address` for hook PDAs | requires BPF program loading + custom hook ix append | **V0.1.5** |
| **Token-2022 Confidential Transfer** | `solana_zk_sdk` (ElGamal proof materialization) + `solana_curve25519` (Ristretto ops) + `sol_poseidon` syscall binding | zk proof performance + ~8 MB transitive dep footprint | **V0.3** |
| **Token-2022 Interest-Bearing mint UI** | `solana_program` ix dispatch (already available) | UI rendering of continuous accrual — not wallet primitive work | **V0.1.5** |
| **Token-2022 Permanent Delegate / Transfer Fee / CPI Guard / Memo Required** | existing `spl_token_2022::instruction` surface | detection already in V0.1 `disambig`; full enforcement dispatch = V0.1.5+ for hook CPI | **V0.1.5** |
| **Address Lookup Tables (ALTs)** | `solana_address_lookup_table_interface` (instruction builders: CreateLookupTable, ExtendLookupTable, DeactivateLookupTable, CloseLookupTable; state struct `AddressLookupTable`) + `solana_message::v0::MessageAddressTableLookup` | reduces tx size for accounts-heavy txs (Jupiter swaps, etc.) | **V0.1.5** |
| **Versioned transactions v0 with ALTs** | `solana_message::versions::VersionedMessage::V0` + `solana_address` for ALT resolution | deferred alongside ALTs | **V0.1.5** |
| **Durable nonces (offline signing for >90s)** | `solana_nonce_account::create_account` + `solana_nonce_account::initialize_account` + `solana_nonce_account::advance_nonce_account` + `solana_nonce_account::withdraw_nonce_account` + `solana_nonce_account::authorize_nonce_account` | ~0.0015 SOL rent per nonce; lifetime semantics; deferred until use case emerges | **V0.1.5** |
| **Stake account creation + delegate + deactivate + withdraw + merge** | `solana_program::stake::instruction` (already in `solana_program` re-export; just not used) + `solana_program::stake::state::StakeState` | not in V0.1 (no native SOL staking) | **V0.2** |
| **Vote account ops** | `solana_vote_interface::instruction::VoteInstruction` + `solana_vote_interface::state::VoteState` | only for stake/vote flows; deferred | **V0.2** |
| **Sign message** (off-chain arbitrary bytes) | `solana_offchain_message::OffchainMessage::v0` + verify via ed25519 | useful for "Sign-In with Solana" / dapp auth | **V0.1.5** |
| **Compute Budget: prioritization fee oracle** | `solana_fee_structure::FeeStructure` + RPC `getRecentPrioritizationFees` | already accessible via RPC; GUI exposure in CLI = V0.1.5 | **V0.1.5** |
| **Compute Budget: full CU cost model** | `solana_fee_structure::calculate_fee(...)` free function (uses `FeeBudgetLimits`) | pre-flight fee calculation; V0.1 just pays flat 5000 base | **V0.2** |
| **AccountInfo / Account state reads** (for tx receipt inspection) | `solana_account::Account` + `solana_account_info::AccountInfo` + `solana_account_view::AccountView` (low-overhead zero-copy) | wallet needs Account struct for parsing transaction receipt states (balance, owner, data); not in V0.1 minimum | **V0.2** |
| **Pubkey re-export via Address** | `solana_address::Address` (`Pubkey` is now re-export alias from solana-pubkey) | already used implicitly via solana-sdk facade | **V0.1** (already in) |
| **DerivationPath typed wrapper** | `solana_derivation_path::DerivationPath::from_str("m/44'/501'/0'/0'")` (handles 501 constant + path validation) | we use raw string; typed wrapper is cleaner; V0.1 already uses raw path string | **V0.1** (already in raw form; V0.1.5 typed) |
| **Seed-derivable Keypair** | `solana_keypair::Keypair::from_seed_phrase_and_passphrase` (behind `seed_derivable` feature) + `solana_seed_phrase` (PBKDF2-HMAC-SHA512) + `solana_seed_derivable` (SeedDerivable trait) | we use `bip39` + `ed25519-bip32` directly; thin wrap possible in V0.1.5 for less code | **V0.1.5** (switch to native) |
| **Presigner (externally-built signature)** | `solana_presigner::Presigner::new(pubkey, signature)` | for offline multi-sig / hardware wallet (V0.3) | **V0.3** |
| **Transaction sanitization** | `solana_sanitize::Sanitize + SanitizeError` + per-tx sanitize check | wallet builds valid txs; sanitization is validator-side concern | **never** (validator-only) |
| **SanitizedMessage** | `solana_message::sanitized::SanitizedMessage` | validator-side post-sanitize form | **never** (validator-only) |
| **SVM-flavored message/transaction** | `solana_svm_transaction::SVMMessage` trait + validator-side types | validator-side batch processing; not for client wallet | **never** (validator-only) |
| **System interface re-export (replace direct solana-program::system_instruction)** | `solana_system_interface::SystemInstruction` enum (deprecation-resistant; solana_program::system_instruction is the legacy path) | use the interface crate directly for forward compat | **V0.2** (refactor) |
| **Loader V3 (BPF upgradeable) interface** | `solana_loader_v3_interface::instruction::UpgradeableLoaderInstruction` | only for wallet-deploying-programs scenario (user-as-developer) | **never** (use Solana CLI) |
| **Compile errors via compile-fail test** | n/a (test discipline) | surface "wrong xpub v+27" pattern analog for offchain-message v-byte | **V0.2** |
| **Micro-lamports fee math** | `solana_native_token::sol_str_to_lamports` (parse user input) + `solana_native_token::lamports_to_sol` (display) | we duplicate this in wallet `util`; switch to native helpers to avoid divergence | **V0.1.5** (refactor) |
| **Hash type re-export for blockhash** | `solana_hash::Hash` (already in via solana-sdk facade) | used; only listed for completeness | **V0.1** (already in) |

**Count: 25 wallet-relevant deferred features spanning 11 subcrate aliases.**

### Group 2: Wallet-relevant but excluded (~5 categories)

| Capability | Subcrate | Why excluded |
| --- | --- | --- |
| **NFT (Metaplex)** | `mpl_token_metadata` 5.1.1 + `mpl_core` 0.12.1 | **License blocker** — `Metaplex NFT Open Source License v1.0` is non-OSI with commercial restrictions. Defer NFT support indefinitely (V1.x at earliest). No mpl in V0.1 dep tree. |
| **Hardware wallet (Ledger/Trezor)** | separate SDK integration (Ledger SDK + Solana transport app) | separate vendor ecosystem; v1.x scope |
| **Compressed NFT (Bubblegum)** | `mpl_bubblegum` (Metaplex) | same Metaplex license blocker |
| **Multi-sig (Squads)** | Squads Protocol SDK (separate crate) | independent SDK maintenance; V0.3 |
| **Web3.js / Solana Wallet Adapter** | `@solana/wallet-adapter` (browser) | out of scope — Rust CLI only |
| **Anchor IDL** | `anchor_lang`, `anchor_client`, `anchor_spl` | Anchor is for program authors; wallet doesn't need IDL parsing. Backed up +6 MB + proc-macro chain |
| **Stake pool integration (LST routing)** | `spl_stake_pool` | V0.3 (depends on V0.2 stake module) |
| **ZK light client (Lighter / Solana ZK ElGamal)** | `solana_zk_sdk` v7.0.1 | V0.3+ (confidential transfers dependency) |
| **EIP-712 typed data signing** | n/a (Solana has no equivalent spec) | Solana uses `sign_message` arbitrary bytes; no typed-data scheme |
| **Watch-only from extended public key (xpub)** | n/a | Ed25519 HD spec has no parent pubkey (`xpub` semantic absent for SLIP-0010). Use `pubkey` per-leaf only. Documented as **NOT POSSIBLE** for Solana |

**Count: 10 explicit non-features.**

### Group 3: NOT wallet-relevant (~70 subcrates — on-chain / validator / zk / wasm)

These primitives are for program authors, validator binaries, or runtime/sidecar tools. A user-facing wallet never imports them. Listed for completeness so wallet authors don't waste time researching "is this subcrate useful for me?"

#### A. On-chain program runtime (~25 crates)

| Subcrate | Role | Wallet relevance |
| --- | --- | --- |
| `solana_account` | Account state struct (lamports, data, owner, executable) | validator/runtime only — wallet reads via RPC |
| `solana_account_info` | AccountInfo passed to on-chain programs (CPI args) | on-chain-side only |
| `solana_account_view` | lightweight zero-copy AccountView | on-chain-side hot CPI path |
| `solana_address` (raw, no Pubkey alias) | 32-byte address primitive (re-exported as Pubkey) | already covered by solana-sdk facade |
| `solana_big_mod_exp` | on-chain sol_big_mod_exp syscall (RSA/DH/zk) | on-chain precompile |
| `solana_blake3-hasher` | Blake3 hash (SBF backend) | on-chain precompile + off-chain fallback |
| `solana_bls12_381` (NOT yet released as separate crate — see bls-signatures) | BLS12-381 syscall wrappers (SIMD-0388) | on-chain zk |
| `solana_bn254` | alt_bn128 (BN254) syscall wrappers — Ethereum zk compat | on-chain zk |
| `solana_borsh` | Borsh v1 re-export + helpers | serialization util; used transitively by spl-token |
| `solana_cpi` | invoke / invoke_signed (CPI from a program) | on-chain only |
| `solana_define-syscall` | define_syscall! macro + syscall hash table | proc-macro for syscall authors |
| `solana_ed25519-program` | on-chain ed25519 verify syscall (Ed25519SignatureOffsets struct) | on-chain verification; wallet signs OFF-chain |
| `solana_instructions-sysvar` | Instructions sysvar (introspection) | on-chain only |
| `solana_instruction-view` | zero-copy instruction view (for CPI hot path) | on-chain only |
| `solana_program` (full re-export hub) | on-chain "std" crate | we use `solana-program` selectively; full crate not pulled |
| `solana_program-entrypoint` | entrypoint!(..) proc macro | program authors only |
| `solana_program-error` | ProgramError + constants | error type; wallet consumes via `RpcResponseError` |
| `solana_program-log` + `solana_program-log-macro` | on-chain log buffering | on-chain only |
| `solana_program-memory` | sol_memcpy/memset/memcmp | on-chain only |
| `solana_program-option` | COption<T> (C-ABI Option) | C-ABI compatibility; on-chain only |
| `solana_program-pack` | Pack trait (binary stable pack/unpack) | on-chain state serialization |
| `solana_secp256k1-program` | on-chain secp256k1 verify syscall | on-chain precompile |
| `solana_secp256k1-recover` | sol_secp256k1_recover syscall | on-chain precompile |
| `solana_secp256r1-program` | on-chain secp256r1 (P-256) verify | on-chain precompile (WebAuthn / passkeys) |
| `solana_get-sysvar` | sol_get_sysvar syscall wrapper | on-chain-only |
| `solana_sysvar` + `solana_sysvar-id` | Sysvar trait + ID | on-chain-only access |
| `solana_curve25519` | curve25519 syscalls (edwards + ristretto) | on-chain only |
| `solana_poseidon` | Poseidon hash (zk) syscall | on-chain zk circuit dep |
| `solana_offchain-message` | OffchainMessage format (sign-in-with-Solana) | wallet V0.1.5 (sign-message CLI) |
| `solana_address-lookup-table-interface` | ALT program instructions + state | wallet V0.1.5; listed in Group 1 |
| `solana_loader-v2-interface` | BPF loader v2 (deprecated) | historical compat only |

#### B. Validator infrastructure (~20 crates)

| Subcrate | Role | Wallet relevance |
| --- | --- | --- |
| `solana_clock` (full Sysvar + time constants) | Clock/Slot/Epoch types + `DEFAULT_MS_PER_SLOT = 400` | validator-only; we use just `commitment_config`, not Clock directly |
| `solana_epoch-info` | EpochInfo RPC response | wallet reads via `RpcClient::get_epoch_info`; struct not pulled directly |
| `solana_epoch-rewards` | EpochRewards sysvar | validator-only |
| `solana_epoch-rewards-hasher` | SipHasher13 partitioner | validator-only |
| `solana_epoch-schedule` | EpochSchedule sysvar + slot math | validator-only |
| `solana_epoch-stake` | sol_get_epoch_stake syscall | validator/UI only |
| `solana_slot-hashes` | SlotHashes sysvar | validator-only |
| `solana_slot-history` | SlotHistory sysvar (BitVec) | validator-only |
| `solana_stake-history` | StakeHistory sysvar | validator-only |
| `solana_last-restart-slot` | LastRestartSlot sysvar | validator-only |
| `solana_instructions-sysvar` | Instructions sysvar | validator/UI introspection |
| `solana_rent` (the sysvar crate) | Rent + RentDue helpers | wallet reads via RPC, not directly |
| `solana_rent` (vs fee-calculator) | modern fee calc types | wallet uses `FeeStructure::calculate_fee` (V0.2) |
| `solana_fee-calculator` | legacy FeeCalculator | deprecated; use fee-structure |
| `solana_fee-structure` | FeeStructure + FeeBin + FeeBudgetLimits | wallet V0.2 (used in preflight tx cost) |
| `solana_hard-forks` | HardForks schedule (slot history) | validator bootstrap; wallet doesn't need |
| `solana_inflation` | Inflation config struct (legacy post-2022) | validator genesis only |
| `solana_poh-config` | PohConfig | validator leader scheduling |
| `solana_shred-version` | block shred version calc | validator side |
| `solana_genesis-config` | GenesisConfig + file load/save | validator bootstrap |
| `solana_packet` | network Packet + Meta (UDP TPU) | validator TPU pipeline |
| `solana_validator-exit` | validator exit hooks | validator-side |
| `solana-reward-info` | RewardType + RewardInfo structs | validator UI; wallet reads inflation rewards via RPC |
| `solana_vote-interface` | vote program instructions + state | wallet V0.2 stake |
| `solana-feature-gate-interface` | feature activation program | validator governance only |

#### C. ZK / crypto syscall internals (~5 crates)

| Subcrate | Role | Wallet relevance |
| --- | --- | --- |
| `solana_big-mod-exp` | sol_big_mod_exp syscall | on-chain zk |
| `solana_bls12_381` | BLS12-381 syscall (SIMD-0388, v0.1.0) | on-chain zk (BLS aggregation) |
| `solana_bn254` | alt_bn128 syscall (zk-SNARK) | on-chain zk (Ethereum compat) |
| `solana_curve25519` | curve25519 syscalls (edwards + ristretto) | on-chain |
| `solana_poseidon` | Poseidon hash syscall | on-chain zk |
| `solana_zk-sdk` (separate crate, not in monorepo) | ElGamal proof materialization | V0.3 confidential |

#### D. Encoding infrastructure (~15 crates)

| Subcrate | Role | Wallet relevance |
| --- | --- | --- |
| `solana_bincode` | `limited_deserialize` helper (cap-safe bincode) | on-chain deserialization, no wallet use |
| `solana_borsh` | Borsh v1 macros + helpers | used transitively by `spl-token` (ATA state); wallet doesn't pull |
| `solana_serde` | `default_on_eof` serializer helper | off-chain serialization util |
| `solana_serde-varint` | serde-compatible VarInt | encoding |
| `solana_wincode-varint` | wincode LEB128 schemas | encoding |
| `solana_serialize-utils` | append_u16, append_pubkey helpers | on-chain serialization |
| `solana_short-vec` | compact-u16 length encoding | used transitively by Solana wire format; wallet sees decoded form only |
| `solana_sha256-hasher` | SHA-256 Hasher | on-chain hash |
| `solana_sha512-hasher` | SHA-512 Hasher | on-chain hash |
| `solana_keccak-hasher` | Keccak-256 (Ethereum-address compat) | on-chain hash |
| `solana_hash` (Pubkey re-export not relevant — bare type) | Hash type | re-exported via solana-sdk; we use solana-sdk |
| `solana_hash-512` | Hash512 (64-byte) | light client / randomness beacon |
| `solana_msg` | `msg!` macro (on-chain logging) | on-chain logging; wallet doesn't call |
| `solana_frozen-abi` + `solana_frozen-abi-macro` | StableAbi digest verification | validator ABI checks |
| `solana_stable-layout` | stable_rc/ref_cell/slice/vec (stable memory layout) | on-chain stability |
| `solana_zero-copy` | unaligned primitive wrappers | on-chain serialization |
| `solana_nullable` | MaybeNull + Nullable traits | null-sentinel on-chain values |

#### E. WASM / mobile bindings (~3 crates — NOT applicable)

| Subcrate | Role | Wallet relevance |
| --- | --- | --- |
| `solana_sdk-wasm-js` | `#[wasm_bindgen]` exports for browser JS | we ship native cdylib (FFI), not WASM |
| `solana_sdk-wasm-js-tests` | wasm tests | testing |
| `solana_system-wasm-js` | wasm-bindgen for system program ix | testing |

#### F. Tests / examples / macros (~10 crates)

| Subcrate | Role | Wallet relevance |
| --- | --- | --- |
| `solana_example-mocks` | doc-only mock modules (cargo doc) | rustdoc only |
| `solana-package-metadata` + `solana_package-metadata-macro` | compile-time metadata extraction | binaries only; not libraries |
| `solana_sdk-macro` | declare_id! proc-macro | program authors only |
| `solana_program-log-macro` | log parameter parser | proc-macro for program authors |
| `solana_sdk-ids` | native program ID constants | transitively via solana_sdk::* |
| `solana_atomic-u64` | AtomicU64 (32-bit fallback) | SBF only |
| `solana-fee-calculator` (legacy) | legacy FeeCalculator | deprecated |
| `solana_file-download` | progress-bar download helper | CLI tools; not used in wallet core |
| `solana_seed_derivable` | SeedDerivable trait (BIP-44) | analog of our `ed25519-bip32` use |
| `solana_seed_phrase` | PBKDF2-HMAC-SHA512 seed from BIP-39 phrase | analog of `bip39` crate we use directly |
| `solana_derivation-path` | BIP-44 path types + parser + Solana coin type 501 | typed wrapper; we use raw string |
| `solana_time-utils` | timestamp + years_as_slots | off-chain util |
| `solana_program-entrypoint` | `entrypoint!(..)` proc-macro | program authors only |
| `solana_precompile-error` | PrecompileError enum (ed25519/secp256k1/secp256r1 verify errors) | on-chain error type; wallet sees precompile result codes |
| `solana_instruction-error` | InstructionError enum | on-chain error type; wallet reads these from tx log |
| `solana_transaction-error` | TransactionError enum | on-chain error type; wallet sees via `getSignatureStatuses` result |
| `solana_sanitize` | Sanitize trait (validator-side validation) | validator only |

### Summary — Solana SDK monorepo coverage (V0.1)

| Metric | Count | % |
| --- | --- | --- |
| Total subcrates in `solana-sdk` workspace | 117 | 100% |
| Subcrates used by `sol-wallet-core` directly (v0.1) | **11** | **9%** |
| Subcrates unused but relevant + deferred | 25 | 21% |
| Subcrates unused + excluded (license/chain-shape) | 10 | 9% |
| Subcrates unused + not wallet-relevant | ~71 | 61% |
| Subcrates unused (total) | 106 | 91% |

**Why so few solana-sdk subcrates are needed:** A user-facing wallet sits on the **client side** of the cluster. The `solana-sdk` monorepo is overwhelmingly targeted at **(a) on-chain program authors** (~50 subcrates), **(b) validator implementations** (~30 subcrates), and **(c) serde/encoding/wasm/zk edge cases** (~25 subcrates). The wallet only needs ~10 client-API primitives, all re-exported through `solana-sdk` + `spl-*` umbrella crates.

**Wallet-relevant deferred features** (Group 1, ~25) map cleanly to deferred V0.x phases:
- V0.1.5 (high-priority): Token-2022 hook CPI dispatch, ALTs, durable nonces, sign-message, prioritization fee oracle, native typed DerivationPath, native seed-phrase
- V0.2 (operator): stake account ops, vote account ops, AccountInfo receipt parsing, full fee-structure model, SystemTransaction legacy builders, micro-lamports fee math via `solana_native_token`
- V0.3 (advanced): Confidential Transfer, Presigner multi-sig, compressed NFT (if license cleared), ZK light client
- Never: FrozenAbi, Sanitize (validator-only), SVM-flavored Message, hard-fork schedule, granular precompile error codes — these are runtime/validator primitives

**Wallet-relevant excluded features** (Group 2, ~10) map to:
- License blocked: NFT (Metaplex), Compressed NFT (Bubblegum)
- Chain-design absent: EIP-712 typed data, Ed25519 HD `xpub`
- Vendor SDK separate: Ledger/Trezor, Squads multi-sig
- Out of scope: Web3.js/wallet-adapter (browser), Anchor IDL

**Sources:**
- `.local/solana-sdk/` (workspace HEAD `main` 2026-09-08, 117 members) via `cargo metadata --no-deps` + per-crate `src/lib.rs` heads
- `.local/crates/solana-sdk/*.md` per-crate technical deep-dives (110 markdown files, ~7k lines)
- Cross-referenced against `.local/solana-sdk/Cargo.toml` `[workspace.members]`

## Rust wallet design — `sol-wallet-core` + `sol` CLI

Mirror pattern of `bitcoin-wallet-core` + `btc` in `rust-wallet-app/crates/`. Workspace will add `sol-wallet-core` + `sol` CLI alongside existing chains.

### Workspace layout

```text
rust-wallet-app/crates/
├── sol-wallet-core/   # NEW — library (rlib + cdylib for FFI)
└── sol/               # NEW — bin
```

### `sol-wallet-core` (library)

Crate-type `["rlib", "cdylib"]`. 14 modules. Wire-format via `solana-sdk`, HD via `bip39` + `ed25519-bip32`, RPC via `solana-client`. Crypto deps shared with `bitcoin-wallet-core`. Design lessons from `polygon-wallet-core` (thin wrapper, 251 LOC) — apply chain-named loaders, per-item re-exports, `disambig` helpers, and `TxSummary` in lib.

| Module | LOC budget | Feature |
| --- | --- | --- |
| `lib.rs` | ~80 | per-item `pub use` re-exports of `solana_sdk` types (NOT glob) — same pattern as `polygon-wallet-core/src/lib.rs` |
| `address` | ~200 | base58 encode/decode, pubkey validation (`is_on_curve`), PDA derivation |
| `wallet` | ~150 | Phantom-equivalent keypair surface: `Wallet::fromMnemonic` + `fromMnemonicAt` + `fromBase58` + `fromPublicKey` + `signTransaction` + `signMessage` + `publicKey`. Internals delegate to `bip39` + `ed25519-bip32` + `solana_sdk::signer::Keypair` — NO custom HD wrapper module |
| `crypto` | ~400 | mnemonic cipher (argon2id + AES-GCM) |
| `config` | ~250 | `SolanaCluster` (Mainnet/Devnet/Localhost), data dir layout, RPC endpoint URL |
| `tx/builder` | ~700 | `system_instruction::transfer` (native SOL) + `spl_token::instruction::transfer_checked` (classic SPL) + `spl_token_2022` (Token-2022 ext-aware) + `spl_associated_token_account::create_associated_token_account_idempotent` + `spl_memo::build_memo` + `ComputeBudgetInstruction` |
| `tx/sign` | ~300 | `Keypair::from_seed` + Zeroize wrapper, `Transaction::sign`, `VersionedTransaction::sign` |
| `tx/broadcast` | ~200 | `RpcClient::send_transaction` with blockhash refresh retry, `get_signature_statuses` confirmation polling, `simulate_transaction` dry-run |
| `tx_summary.rs` | ~50 | `TxSummary { signature, slot, from, to, amount, mint }` for SPL transfer logs (LIVES IN LIB — mirrors `polygon-wallet-core::TxSummary`) |
| `chain` | ~500 | `RpcClient` wrapper + retry policy + WS subscription helper (account/SPL), `get_latest_blockhash` cache, ATA discovery (`getTokenAccountsByOwner`) |
| `wallet_manager` | ~500 | multi-wallet CRUD scaffolding (CLI-only; Phantom has no analog): UUID v4 wallet id, encrypted store (argon2id + AES-GCM, same shape as `bitcoin-wallet-core/wallet/persist.rs`), atomic write |
| `tokens/` | ~40 | `load_mainnet()` / `load_devnet()` — bundled JSON via `include_str!` (chain-named loaders, NO `Network` discriminator) |
| `disambig` | ~150 | `reject_wrong_chain_spl(mint, expected_cluster)` + `cluster_rpc_url(cluster)` + `derive_ata_with_program(owner, mint, token_program_id)` (footgun guards) |
| `error` | ~100 | `thiserror` enum: `InvalidMnemonic`, `InvalidAddress`, `SignFailed`, `BroadcastFailed { code, msg }`, `BlockhashExpired` |
| `ffi` | ~400 | C ABI (cdylib), Dart FFI hooks, panic-message scrubber for mnemonic/secret-key leaks |
| `util` | ~100 | atomic write, permissions |

### `sol-wallet-core` × `solana-sdk 4.1.0` — feature map

Focused map of which `sol-wallet-core` features consume `solana-sdk`. Source ref: Anza monorepo at `github.com/anza-xyz/solana-sdk`, 4.1.0 release tag.

**Direct usage:**

| `sol-wallet-core` feature | `solana-sdk` API | File ref |
| --- | --- | --- |
| Generate random keypair | `Keypair::new()` | `signer/keypair.rs` |
| Derive keypair from seed | `Keypair::from_seed(&[u8; 32])` | `signer/keypair.rs` |
| Pubkey from base58 | `Pubkey::from_str("...")` | `pubkey.rs` |
| Pubkey display | `pubkey.to_string()` | `pubkey.rs` |
| PDA derivation | `Pubkey::find_program_address(&seeds, &program_id)` | `pubkey.rs` |
| Pubkey is_on_curve check | `Pubkey::is_on_curve(&bytes)` | `pubkey.rs` |
| Sign message | `keypair.sign_message(msg: &[u8])` | `signer.rs` |
| Verify signature | `signature.verify(&pubkey, msg)` | `signature.rs` |
| System transfer (native SOL) | `system_instruction::transfer(&from, &to, lamports)` | `system_instruction.rs` |
| Message (legacy) | `Message::new(&[ix], Some(&payer))` | `message.rs` |
| Message with blockhash | `Message::new_with_blockhash(&[ix], Some(&payer), &blockhash)` | `message.rs` |
| Versioned message (v0) | `VersionedMessage::V0(v0::Message::new(...))` | `message/versions/v0.rs` |
| Transaction sign | `tx.sign(&[&keypair], blockhash)` | `transaction.rs` |
| Versioned transaction sign | `vtx.sign(&[&keypair])` | `transaction.rs` |
| Transaction verify | `tx.verify()` + `tx.verify_with_results()` | `transaction.rs` |
| Transaction to base64 wire | `tx.to_base64()` | `transaction.rs` |
| Transaction from base64 wire | `VersionedTransaction::try_from(base64)` | `transaction.rs` |
| Compute Budget ix | `ComputeBudgetInstruction::set_compute_unit_limit(N)` | (in `solana-compute-budget-program` crate) |

**Features NOT using solana-sdk (wallet-local):**

| Feature | Implementation | Why not solana-sdk |
| --- | --- | --- |
| Mnemonic encryption (Argon2id + AES-GCM) | `sol-wallet-core/src/crypto/` | solana-sdk has zero persistence |
| Wallet id (UUID v4) | `sol-wallet-core/src/wallet/id.rs` | wallet metadata |
| Encrypted wallet store + atomic write | `sol-wallet-core/src/wallet/persist.rs` | persistence layer |
| Address validation | `sol-wallet-core/src/address/` | chains `is_on_curve` over solana-sdk |
| tx/broadcast | `sol-wallet-core/src/tx/broadcast.rs` | HTTP RPC + retry |
| RPC client wrapper | `sol-wallet-core/src/chain/` | retry + WS subscription mgmt |
| Token registry | `sol-wallet-core/src/tokens/` | bundled JSON |
| SPL disambiguation (Token-2022 vs classic) | `sol-wallet-core/src/disambig.rs` | compile-time data |

**Address derivation flow (cross-crate):**

```text
sol-wallet-core::Wallet::fromMnemonicAt(phrase, account, address_index)
    → bip39::Mnemonic::from_phrase(phrase, English)
    → bip39::Seed::new(&m, "") -> [u8; 64]
    → ed25519_bip32::XPrv::from_seed(seed)         // SLIP-0010 master
    → XPrv::derive("m/44'/501'/{account}'/0'/{address_index}")  // Phantom convention, numeric
    → xprv.public_key() -> [u8; 32]                // Ed25519 verification key
    → Pubkey::new_from_array(pk_bytes)              // wraps in solana_pubkey
    → pk.to_string() -> base58 address              // display
```

**Three crates cooperate** — bip39 owns mnemonic→seed, ed25519-bip32 owns seed→Ed25519, solana-sdk owns keypair↔pubkey.

**Coverage summary for `sol-wallet-core::keys` module:**

| Function | Source |
| --- | --- |
| `generate_mnemonic(words)` | bip39 |
| `import_mnemonic(phrase)` | bip39 |
| `derive_keypair(mnemonic, path)` | bip39 + ed25519-bip32 |
| `pubkey_from_xprv(xprv)` | ed25519-bip32 + solana-sdk |
| `sign_tx(sk, msg)` | solana-sdk (keypair.sign) |
| `encrypt_mnemonic(phrase, password)` | wallet-local (argon2id + AES-GCM) |
| `decrypt_mnemonic(blob, password)` | wallet-local |

**7 of 9 key functions use external crates (bip39, ed25519-bip32, solana-sdk).** The 2 wallet-local functions are persistence-layer (encryption for wallet file).

**MSRV implication:** solana-sdk 4.1.0 requires Rust 1.89.0. Affects `sol-wallet-core` directly. Apply the split pattern (toolchain channel 1.89.0, advertised MSRV 1.85) drafted in `### Toolchain — split channel + MSRV (recommended)`.

### `sol-wallet-core` × `solana-client 4.2.2` — feature map

Map every `sol-wallet-core` RPC call to its `solana-client` API. Source ref: `github.com/anza-xyz/agave` (rpc + rpc-client + client crates), 4.2.2 release tag.

**Direct usage by module:**

| `sol-wallet-core` module | `solana-client` API | File in solana-client |
| --- | --- | --- |
| `tx/broadcast` (send) | `RpcClient::send_transaction(&tx)` | `rpc_client.rs` |
| `tx/broadcast` (simulate) | `RpcClient::simulate_transaction(&tx)` | `rpc_client.rs` |
| `tx/broadcast` (confirm) | `RpcClient::get_signature_statuses(&[sig])` | `rpc_client.rs` |
| `tx/builder` (fresh blockhash) | `RpcClient::get_latest_blockhash()` | `rpc_client.rs` |
| `chain` (account state) | `RpcClient::get_account(&pubkey)` + `get_account_with_commitment` | `rpc_client.rs` |
| `chain` (multi-account) | `RpcClient::get_multiple_accounts(&[pks])` | `rpc_client.rs` |
| `chain` (ATA discovery) | `RpcClient::get_token_accounts_by_owner(&owner, token_program_id)` | `rpc_client.rs` |
| `chain` (rent) | `RpcClient::get_minimum_balance_for_rent_exemption(size)` | `rpc_client.rs` |
| `chain` (balance) | `RpcClient::get_balance(&pubkey)` | `rpc_client.rs` |
| `chain` (SPL balance) | `RpcClient::get_token_account_balance(&ata)` | `rpc_client.rs` |
| `chain` (supply) | `RpcClient::get_token_supply(&mint)` | `rpc_client.rs` |
| `chain` (WS — ATA changes) | `PubsubClient::account_subscribe(&ata, config)` | `nonblocking/pubsub_client.rs` |
| `chain` (WS — tx confirmations) | `PubsubClient::signature_subscribe(&sig, config)` | `nonblocking/pubsub_client.rs` |
| `chain` (WS — slot updates) | `PubsubClient::slot_subscribe()` | `nonblocking/pubsub_client.rs` |
| `tx/builder` (priority fee) | `RpcClient::get_recent_prioritization_fees(&accounts)` | `rpc_client.rs` |
| `config` (cluster health) | `RpcClient::get_health()` | `rpc_client.rs` |
| `config` (version) | `RpcClient::get_version()` | `rpc_client.rs` |
| `config` (epoch info) | `RpcClient::get_epoch_info()` | `rpc_client.rs` |
| `tx/broadcast` (airdrop) | `RpcClient::request_airdrop(&pubkey, lamports)` (devnet only) | `rpc_client.rs` |
| `error` (retry classification) | `ClientErrorKind` (RpcError, Io, etc.) | `client_error.rs` |

**Errors requiring workaround:**

| Error | Fix in sol-wallet-core |
| --- | --- |
| `RpcError::BlockhashNotFound` on `send_transaction` | retry with fresh blockhash (up to 3 times); never re-sign identical bytes (sigs are nonces over the full message) |
| `RpcError::NodeUnhealthy` / `RpcError::Timeout` | exponential backoff with jitter (100ms → 200ms → 400ms → 800ms) |
| `ClientErrorKind::Io` (network down) | same backoff; surface after 3 attempts |
| `ClientErrorKind::RpcError(RpcResponseError::BlockCleanedUp)` | blockhash expired before confirmation; retry with fresh blockhash |
| `RpcError::AccountNotFound` | caller treats as "account not yet created" — for ATA, trigger `create_associated_token_account_idempotent` |
| `ClientErrorKind::SerdeJsonError` | log + surface; do not retry (caller bug) |

**Retry policy design (chain module):**

```rust
async fn send_with_retry(
    rpc: &RpcClient,
    tx: &VersionedTransaction,
    max_attempts: u32,
) -> Result<Signature, Error> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        let fresh_blockhash = rpc.get_latest_blockhash().await?;
        let mut msg = tx.message.clone();
        msg.set_recent_blockhash(fresh_blockhash);
        let mut fresh_tx = VersionedTransaction::new(msg, tx.signatures.clone());
        // Re-sign with current keypair (signatures are nonces over the full message)
        // Note: caller must re-sign with the original keypair, NOT reuse old sigs
        match rpc.send_transaction(&fresh_tx).await {
            Ok(sig) => return Ok(sig),
            Err(e) if e.is_blockhash_not_found() && attempt < max_attempts => continue,
            Err(e) => return Err(e.into()),
        }
    }
}
```

**MSRV implication:** solana-client 4.2.2 requires Rust 1.89.0. Affects `sol-wallet-core` directly. Same split pattern as solana-sdk.

## Sources

### Solana protocol + transaction format

- Solana Developer Hub — Transactions: <https://solana.com/docs/core/transactions>
- Solana Developer Hub — Compute Budget: <https://solana.com/docs/core/fees/compute-budget>
- Solana Developer Hub — Fees: <https://solana.com/docs/core/fees>
- Solana Developer Hub — Durables Nonces: <https://solana.com/docs/core/transactions/durable-nonces>
- Solana Cookbook — HD wallet derivation: <https://solanacookbook.com/docs/wallets/hd-wallet>
- Solana Cookbook — Rent: <https://solanacookbook.com/docs/core/fees/rent>
- Solana Cookbook — Airdrops & Faucets: <https://solanacookbook.com/docs/development/airdrops-and-faucets>
- Solana Cookbook — ATA: <https://solanacookbook.com/docs/core/tokens/associated-token-account>
- Solana JSON-RPC spec: <https://solana.com/docs/rpc>
- Solana WebSocket spec: <https://solana.com/docs/rpc/websocket>
- Solana clusters: <https://solana.com/docs/references/clusters>
- Andrew Stanger — p-token rewrite analysis: <https://www.helius.dev/blog/solana-p-token>
- Asymmetric Research — p-token bug catch: <https://blog.asymmetric.re/solana-p-token-catching-a-bug-before-mainnet/>
- Andrew Stanger — durable nonces in Solana: <https://www.helius.dev/blog/solana-transactions>

### Rust SDK candidates (Anza + community)

- `solana-sdk` (Anza monorepo): <https://github.com/anza-xyz/solana-sdk>
- `agave` (Anza validator + client + rpc): <https://github.com/anza-xyz/agave>
- `solana-program` org (SPL repos): <https://github.com/solana-program>
- `spl-token` (classic): <https://github.com/solana-program/token>
- `spl-token-2022` (Token Extensions): <https://github.com/solana-program/token-2022>
- `spl-associated-token-account`: <https://github.com/solana-program/associated-token-account>
- `spl-memo`: <https://github.com/solana-program/memo>
- `solana-zk-sdk` (ElGamal): <https://github.com/solana-program/zk-elgamal-proof>
- `anchor-lang` (Otter Sec, backup): <https://github.com/otter-sec/anchor>
- `metaplex-foundation/mpl-token-metadata` (REJECTED — license): <https://github.com/metaplex-foundation/mpl-token-metadata>
- `jito-labs/jito-rust-rpc` (stale, gate): <https://github.com/jito-labs/jito-rust-rpc>
- `jito-foundation/jito-solana` (active fork): <https://github.com/jito-foundation/jito-solana>
- `ed25519-dalek` (Dalek Cryptography): <https://github.com/dalek-cryptography/curve25519-dalek>
- `ed25519-bip32` (typed-io, SLIP-0010): <https://github.com/typed-io/rust-ed25519-bip32>
- `bip32` (iqlusion, secp256k1 only — NOT for SOL): <https://github.com/iqlusioninc/crates>
- Solana Labs Ed25519 BIP32 standardization issue: <https://github.com/solana-labs/solana/issues/6301>
- `surfpool` (in-memory validator, 2025+): <https://github.com/txtx/surfpool>
- `solana-light-client` (immature): <https://github.com/CRossel87a/solana-light-client>

### Testnet + RPC

- Solana public RPC: <https://api.mainnet-beta.solana.com>
- Solana devnet RPC: <https://api.devnet.solana.com>
- Solana testnet RPC (DEPRECATED): <https://api.testnet.solana.com>
- Helius free tier: <https://www.helius.dev/docs/api-reference/endpoints>
- Alchemy Solana free tier: <https://www.alchemy.com/overviews/solana-rpc>
- QuickNode Solana trial: <https://www.quicknode.com/blog/best-solana-rpc-providers-2026>
- Triton Solana paid: <https://triton.one/solana>
- `solana-test-validator` docs: <https://docs.solana.com/cluster/benchmarks>
- Agave CLI: <https://github.com/anza-xyz/agave>
- `solanalabs/solana` Docker Hub: <https://hub.docker.com/r/solanalabs/solana>
- `testcontainers` Rust crate: <https://github.com/testcontainers/testcontainers-rs>

### SPL tokens + stablecoins

- Solana Explorer: <https://explorer.solana.com>
- Solscan: <https://solscan.io>
- USDC (Circle, classic): <https://developers.circle.com/stablecoins/usdc-on-main-net>
- USDT (Tether, classic): <https://tether.to>
- PYUSD (Paxos/PayPal, **Token-2022**): <https://solana.com/news/pyusd-paypal-solana-developer> + <https://newsroom.paypal-corp.com/2024-05-29-PayPal-USD-Stablecoin-Now-Available-on-Solana-Blockchain>
- USDS (Sky, classic): <https://developers.skyeco.com/protocol/tokens/usds/>
- Bonk (community): <https://bonkcoin.com>
- Jupiter (JUP): <https://jup.ag/tokens>
- JitoSOL: <https://www.jito.network/docs/jitosol/>
- Token-2022 extensions: <https://www.helius.dev/blog/token-2022>
- Solana Rent Calculator: <https://rent.solana.com/>

### Standards + cross-references

- SLIP-0010 (Ed25519 HD): <https://github.com/satoshilabs/slips/blob/master/slip-0010.md>
- SLIP-0044 coin types (SOL = 501): <https://github.com/satoshilabs/slips/blob/master/slip-0044.md>
- BIP-32 HD derivation: <https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki>
- BIP-39 mnemonic wordlist: <https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki>
- SPL Token Program: <https://spl.solana.com/token>
- SPL Token-2022 Program: <https://spl.solana.com/token-2022>
