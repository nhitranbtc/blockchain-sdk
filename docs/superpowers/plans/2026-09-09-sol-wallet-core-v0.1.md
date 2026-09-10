# sol-wallet-core (v0.1) — Implementation Plan (Anza stack, Anza-owned crypto)

> **For agentic workers:** REQUIRED SUB-SKILLS: `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Apply per `.local/plugins-docs/2026-09-05-plan-guide-mattpocock-superpowers-stack.md` — Type A: New crate / architectural change.

**mattpocock-skills source mapping (per `.local/plugins-docs/2026-08-31-mattpocock-skills-deepdive.md` main flow):**
- **Decisions Q1-Q12** = `/mattpocock-skills:grilling` output (Round-1 grill 2026-09-09; design tree + frontier; facts vs decisions split).
- **Phase Set Up (S.1-S.6)** = `/mattpocock-skills:to-spec` output (branch / labels / milestone / CI plumbing before any code).
- **Phases 0-9** = `/mattpocock-skills:to-tickets` output (spec decomposed into tracer-bullet tickets with blocking edges; Phase 6.2 + Phase 7 verification = pre-Phase gates).
- **Per-Phase workflow** = `/mattpocock-skills:implement` (drive `/tdd` slices at seams → typecheck regularly → single test file regularly → full test suite at end → `/code-review` → commit on current branch).
- **Post-Phase review** = `/mattpocock-skills:code-review` (Standards + Spec axes run separately per L13 step 15; aggregate under separate headings; do not merge; do not rerank). 12-smell Fowler baseline applies.
- **Hard deps satisfied:** `/mattpocock-skills:setup-matt-pocock-skills` outputs (`docs/agents/{issue-tracker,triage-labels,domain}.md`) already exist per repo `CLAUDE.md`. Hard skills (`to-tickets`, `to-spec`, `triage`) usable without re-running setup.
- **Soft deps in use:** `/mattpocock-skills:tdd` (red-green per Task) + `/mattpocock-skills:diagnosing-bugs` (per L13 debugging sub-skill).
- **Anti-patterns avoided:** no horizontal TDD slicing (each test file = one concern); no tautological tests (Phantom canonical + SLIP-0010 KAT vectors); no wayfinder→`/implement` skip (no wayfinder used); no merged review axes (Q1-Q12 separate); no author-naming on docs.

**Goal:** Deliver `rust-wallet-app/crates/sol-wallet-core/` — a Solana (SOL + SPL stablecoin) wallet library built on the Anza stack (`solana-sdk` 4.1.0 + `solana-client` 4.2.2 + SPL programs + `ed25519-bip32` 0.4.3 for HD only), plus a `sol` CLI in the umbrella workspace. **`solana-sdk`-first, Phantom UX parity, Anza-owned crypto** — every crypto primitive delegates to Anza; custom wallet-local code only for persistence (Argon2id + AES-GCM), PAL traits, FFI surface, Token-2022 footgun guard, blockhash retry/backoff. HD derivation via `ed25519-bip32` for SLIP-0010 chain key only — `solana-sdk` does NOT cover HD (Anza explicitly excludes BIP-32 / HD derivation from its scope). Mirrors the structure of `bitcoin-wallet-core` (cdylib + 4-trait PAL) and `tron-wallet-core` (reuses `spki::SpkiPinnedVerifier` shape). Compiles for desktop (Linux/macOS/Windows) + mobile (iOS arm64 + Android arm64) via 4-trait PAL.

**Architecture:** Nine phases implementing the deep-dive Test scenario file structure (32 library + 10 CLI test files + 5 common helpers), plus Phase Set Up (branch / labels / CI).

- **Phase 0** = scaffold crate + compile check (no tests in Phase 0 — every test lands with its owning phase).
- **Phase 1** = Phantom-equivalent wallet keypair (Mnemonic → Seed → XPrv → Keypair + `fromMnemonic` / `fromBase58` / `fromPublicKey` / `signTransaction` / `signMessage` API).
- **Phase 2** = Address surface (base58 + `is_on_curve` + PDA derivation).
- **Phase 3** = tx::builder native SOL (`system_instruction::transfer` + Compute Budget prepend).
- **Phase 4** = SPL transfer (classic + Token-2022) + ATA lifecycle + disambig footgun guard + decimals fetched dynamically.
- **Phase 5** = RPC client (reqwest JSON-RPC, 15 HTTP methods in 5.1 + 1 in 5.2 `requestAirdrop` + 1 in 5.3 `getTransaction` + per-`RpcClient` rate limiter in 5.4 + SPKI escape hatch in 5.5 — Anza `solana-rpc-client` DROPPED per issue #555) + `send_and_confirm` (single send + poll, no retry-on-stale-hash). Retry-on-stale-hash + BlockhashCache + full SPKI pin + 5 WS subscribes DEFERRED to V0.1.5. The 17 RPC methods across 5.1/5.2/5.3 + the security hardening in 5.4/5.5 cover all 22 Phase 7 `sol` CLI commands. Final test count: 94 (Phase 1-4) + 70 (5.1) + 3 (5.2) + 4 (5.3) + 3 (5.4) + 5 (5.5) = **179 tests pass** at Phase 5 done.
- **Phase 6** = Wallet persistence (Argon2id + AES-256-GCM) + WalletManager CRUD + encrypted blob atomic write.
- **Phase 7** = `sol` CLI (22 commands: wallet / address / balance / spl / tx / config).
- **Phase 8** = FFI cdylib (12 C functions + panic-message scrubber).
- **Phase 9** = Mainnet smoke gate ($0.001 USDC self-send via `RUN_SOL_MAINNET=1`).

**Tech Stack:** Rust 1.89.0 stable (mandatory MSRV per every Anza crate), `solana-sdk` 4.1.0 (facade) + `solana-client` 4.2.2 (RPC) + `solana-program` 4.1.0 + `solana-keypair` 3.1.2 + `solana-message` 4.6.0 + `solana-transaction` 4.3.0 + `solana-instruction` 3.5.0 + `solana-compute-budget-program` 4.2.2, SPL: `spl-token` 9.0.0 + `spl-token-2022` 11.0.0 + `spl-associated-token-account` 8.0.0 + `spl-memo` 7.0.0, HD: `ed25519-bip32` 0.4.3 + `bip39` 2.2 (English only), crypto: `argon2` 0.5 + `aes-gcm` 0.10 + `zeroize` 1.x + `subtle` 2 + `ed25519-dalek` 3.0.0 (transitive pin), async+HTTP: `tokio` 1.x + `reqwest` 0.12 (`rustls-tls`) + `rustls` 0.23 + `webpki` 0.22 + `x509-parser` 0.16, errors+tracing: `thiserror` 1.x + `tracing` workspace, FFI safety: `once_cell` 1.x + `regex` 1.x, build-time: `cbindgen`, CLI: `clap` 4, encoding: `serde` + `serde_json` + `bs58` 0.5 + `hex` + `chrono` + `uuid` 1.x.

**Companion docs:**
- Research: `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` (this plan's source of truth — SDK selection + cluster + ATA footgun + retry/backoff)
- Audit (companion): `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md` (security audit per `audit-issue` skill)
- Workflow framework: `.local/plugins-docs/2026-09-05-plan-guide-mattpocock-superpowers-stack.md`
- Per-crate notes: `.local/solana-sdk/*.md` + `.local/crates/solana-sdk/*.md`
- Lessons: `tasks/lessons.md` L13 (per-task pipeline), L24 (tech doc updates), L11 (skill-tag)

**Tracks:** TBD GitHub issue (Q1-Q12 closed by deep-dive Round-1 grill). PR Ticket A (research) lands once this plan is committed. This plan covers Ticket B-J (implementation per phase).

**Pre-empts:** v0.3+ placeholder in `rust-wallet-app/crates/chain-traits/src/lib.rs:21` (`ChainId::Solana(String)` placeholder for SOL coin — pubkey base58 string).

**Status:** Plan. No code produced yet. **PAUSE before commit** per never-auto-commit rule.

---

## Decisions (grill Round 1, 2026-09-09)

Locked by user against the grill Round-1 frontier. **Phantom UX parity** = the dominant design choice: wallet surface mirrors Phantom's user-facing API exactly (no custom HD wrapper; numeric `--account` + `--address-index` flags; no path strings exposed).

| Q   | Decision                            | Answer                                                                                                                                                                       |
| --- | ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1  | Primary SDK                         | **(a) Anza official stack** (`solana-sdk` 4.1.0 + `solana-client` 4.2.2 + `solana-program` 4.1.0 + SPL + `ed25519-bip32` 0.4.3 for HD). Rejected: Metaplex (license), Jito-rust (stale), qntx/kobe (name collision), light-client (immature), raw custom crypto. |
| Q2  | HD coverage                         | **(a) `ed25519-bip32` 0.4.3 only for HD chain key derivation** — `solana-sdk` does NOT cover BIP-32 / SLIP-0010. Phantom-equivalent Wallet API delegates signing to `solana_sdk::Keypair`. NO custom HD wrapper module. |
| Q3  | Derivation path                     | **(a) Phantom convention** = `m/44'/501'/account'/0'/address_index` (SLIP-44 coin 501 = SOL). Numeric `--account` (default 0) + `--address-index` (default 0). Path string NEVER exposed. |
| Q4  | Validation smoke gate               | **(a) `$0.001 USDC self-send on mainnet-beta`** with `RUN_SOL_MAINNET=1` env gate + pre-check audit hook (refuse if recipient != operator_wallet). Real value, real network. Once per V0.1 release candidate. |
| Q5  | Local validator                     | **(a) `surfpool` (txtx)** — V0.1 default (sub-second boot, in-memory, no Docker). V0.1.5 opt-in `solana-test-validator` for BPF + epoch boundary tests. |
| Q6  | Token-2022 disambig                 | **(a) `disambig::reject_wrong_token_program`** — auto-detect via `getAccountInfo(mint).owner` (`TokenkegQ...` vs `TokenzQdB...`); never mix programs in ATA derivation seed. |
| Q7  | Blockhash retry                     | **(V0.1: single send + poll, no retry) → (V0.1.5) `send_with_retry`** — 3 attempts max, exponential backoff 100ms→200ms→400ms, fresh blockhash each retry, re-sign each attempt. Ed25519 signs `recent_blockhash` directly, signatures are nonces over full message. |
| Q8  | Compute Budget defaults             | **(a) 150_000 CU default limit + 0 priority fee** — 1000x safety margin for p-token rewrite 2026 (~5 CU per simple transfer). Auto-set 200_000 CU for composite tx (memo + 2 ATA creates). |
| Q9  | SPKI pinning                        | **(V0.1: `RpcClient::new_with_pinned_spki(url, spki_hex)` escape hatch for high-value wallets; default `RpcClient::new` uses rustls system roots) → (V0.1.5) Scenario A opt-in** via `pinned://<pin>@host` URL for paid Helius/QuickNode/Alchemy. Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` verbatim.
| Q10 | Decimals                            | **(a) NEVER hardcoded** — always `spl_token::state::Mint::unpack(mint_account.data).decimals` + `transfer_checked` (NOT `transfer`). Prevents USDC=6 / BONK=5 / USDS=6 mismatch. |
| Q11 | Cluster coverage                    | **(a) `MainnetBeta / Devnet / Localnet` ONLY** — Solana testnet DEPRECATED 2022-23 (Foundation abandoned). `Cluster` enum has NO `Testnet` variant; `sol config set-cluster testnet` → `Error::InvalidCluster`. |
| Q12 | Token program support scope         | **(a) V0.1 = classic SPL + Token-2022 awareness (footgun guard)** — `transfer_checked` works for both programs. NFT/Metaplex EXCLUDED (license blocker). Token-2022 extension-specific transactions (transfer hook CPI, confidential proofs) deferred V0.1.5+. |

### Why Anza-only (Anza vs custom crypto)

**Phantom UX parity argument:** Phantom is the reference Solana wallet; users expect `--account` + `--address-index` flags (not `m/44'/501'/0'/0'/N`). Phantom exposes `Signer` trait (via `solana-signer`) + `Keypair` (via `solana-keypair`) + `Pubkey` (via `solana-sdk::pubkey`) — same surface. Why rewrite what Phantom already gets right via stable Anza crates?

**Bus-factor (accepted):** Anza holds 90%+ of the stack (SDK + agave validator + SPL org). Single-vendor trust accepted per Round-1 grill finding 2026-09-08. Apache-2.0 license, ~3-month release cadence, ~30 contributors, actively maintained. **Mitigation = monitor `anza-xyz/agave` security advisories.** No vendoring needed (unlike TRON 2026-09-06 reversal — that was bus-factor = 1 plus active varint bug; Anza has ~30 contributors so cost/benefit is different).

**Anza subcrate version drift audit:** `solana-sdk` = 4.1.0, `solana-message` = 4.6.0, `solana-transaction` = 4.3.0, `solana-keypair` = 3.1.2, `solana-signer` = 3.0.1, `solana-instruction` = 3.5.0 — desynced versions. **MUST pin each individually** with exact `=x.y.z` (NOT caret `^x.y`). Cargo's facade re-export does NOT unify subcrate versions. Verified via `cargo tree -p sol-wallet-core | grep solana-` in Task 0.1 verification.

---

## Global Constraints (verbatim from deep-dive Round-1 grill Q1-Q12)

- **Q1 — SDK choice.** Anza stack = `solana-sdk` 4.1.0 (facade: Keypair, Pubkey, Message, Transaction) + **`solana-rpc-client` DROPPED per #555 (V0.1 ships thin `reqwest` JSON-RPC client with 15 HTTP methods in 5.1 + 2 follow-on methods across Tasks 5.2 `requestAirdrop` + 5.3 `getTransaction`; 5 WS subscribes deferred to V0.1.5 watch mode; revisit when Anza stable `solana-sdk 4.2.x` ships or when V0.1.5 needs the full 21-method table back)** + `solana-program` 4.1.0 (PDA + sysvar + hash) + SPL programs (`spl-token` 9.0.0 classic + `spl-token-2022` 11.0.0 extensions + `spl-associated-token-account` 8.0.0 + `spl-memo` 7.0.0) + `ed25519-bip32` 0.4.3 (SLIP-0010 HD only). Anza = official; 90%+ of stack; active maintenance; Apache-2.0. Bus-factor accepted. **REJECTED:** `mpl-token-metadata` (Metaplex NFT Open Source License v1.0 — non-OSI blocker, deferred indefinitely), `jito-labs/jito-rust-rpc` (stale 2025-06-21, gate behind `jito` Cargo feature if ever needed), `qntx/kobe` (Jito MEV research repo, name collision — NOT a wallet lib), `CRossel87a/solana-light-client` (immature, ~30 all-time downloads), `SergioBenitez/Figment` (config-only, not crypto).
- **Q2 — HD coverage gap.** `solana-sdk` 4.1.0 does NOT cover BIP-32 / SLIP-0010 (Anza explicitly excludes HD derivation from scope; Phantom wallet solves this with a separate crate). **`ed25519-bip32` 0.4.3** (typed-io, MIT/Apache-2.0) provides SLIP-0010 Ed25519 chain key derivation. Phantom-equivalent Wallet API internally: `bip39::Mnemonic::from_phrase` → `bip39::Seed::new(&m, "")` → `ed25519_bip32::XPrv::from_seed(seed)` → `XPrv::derive("m/44'/501'/.../0'/0")` → `solana_sdk::Keypair::try_from(seed_bytes)`. NO custom HD wrapper module exposed in `sol-wallet-core`.
- **Q3 — Derivation path.** Phantom convention = `m/44'/501'/{account}'/0'/{address_index}` (SLIP-44 coin 501 = SOL). Hardened only at `44'`, `501'`, `account'`; non-hardened at `0'` and final `address_index`. CLI flags numeric: `--account <N>` (default 0) + `--address-index <N>` (default 0). Path string NEVER exposed (Ledger-style ZIP-32 users → `sol keygen raw` V0.2 deferred).
- **Q4 — Mainnet smoke gate.** **V0.1 release GATED on one mainnet self-send — $0.001 USDC to self (recipient == sender), real value, real network.** Local + devnet is emulation. Without a real-value smoke, test scenario PASS evidence = "looks like real network" not "real network". Operator checklist: Alchemy API key loaded, operator wallet has ≥ 0.01 USDC + 0.01 SOL for fees + rent, `--confirm-mainnet` prompt typed `yes`, `RUN_SOL_MAINNET=1` env var set, `sol wallet send --to <self> --amount 0.001 --token USDC --wait --wait-finalized --priority-fee 1000`. Verify via `sol balance --address <self> --token USDC` — balance changed by 0.001 USDC.
- **Q5 — Local validator.** `surfpool` (txtx/surfpool, 2025+) sub-second boot, in-memory, no Docker, runtime reset (`surfpool reset`), warp slot (`surfpool warp --slot N`), built-in faucet (`surfpool airdrop`). Spawned as subprocess via `tokio::process::Command` in `tests/common/surfpool_guard.rs::SurfpoolGuard` RAII wrapper (ephemeral port, 10s health-poll deadline). `solana-test-validator` DEFERRED to V0.1.5 opt-in for BPF + epoch boundary tests.
- **Q6 — Token-2022 disambig.** ATA derivation seed = `[owner, token_program_id, mint]`. `token_program_id` IS a seed → classic SPL (`TokenkegQ...`) and Token-2022 (`TokenzQdB...`) produce DIFFERENT ATA addresses for same `(owner, mint)`. `disambig::reject_wrong_token_program` checks `mint.owner` via `getAccountInfo`. Mix detected → `Error::InvalidTokenProgram`. Wallet derives correct ATA programmatically; caller NEVER passes wrong `token_program_id`. **Same ATAs do NOT equal Token-2022 ATAs** — surface this rule in CLI docs.
- **Q7 — Blockhash retry.** Blockhash lifetime ~60-90 sec (~150 slots at 400ms). Ed25519 signs `recent_blockhash` directly → signature covers blockhash + message. Re-sign identical bytes NEVER works (signature is nonce over full message). **V0.1: `tx::broadcast::send_and_confirm` does single send + poll, no retry** (stale hash rate <1% on devnet; user can re-run). **V0.1.5: `send_with_retry` 3 attempts max, fresh blockhash each retry, exponential backoff 100ms→200ms→400ms.** `BlockhashNotFound` and `BlockCleanedUp` → retry in V0.1.5 (V0.1 returns `Error::BroadcastFailed { kind: "BlockhashNotFound" }` and surfaces to CLI).
- **Q8 — Compute Budget defaults.** Prepend every tx with `ComputeBudgetInstruction::set_compute_unit_limit(150_000)` + `set_compute_unit_price(0)`. 150k CU = 1000x safety margin for p-token rewrite 2026 (~5 CU per simple SPL transfer). Composite tx (memo + 2 ATA creates) auto-set 200k CU. Priority fee defaults to 0; user overrides via `--priority-fee <micro_lamports>` or `SOL_PRIORITY_FEE` env. Heap frame + loaded-accounts-size = V0.2 (program deployment only).
- **Q9 — SPKI pinning.** Solana Labs public RPC rotates certs freely (no published SPKI pin). Scenario B default: no pin, rely on cert transparency + standard rustls verification (hostname + CA chain). Scenario A opt-in: `pinned://<spki-hex>@host[:port]` URL scheme for paid Helius/QuickNode/Alchemy. Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` shape (no fork). `SOL_SPKI_PIN` env-only per L12 H-1 (no CLI setter in V0.1).
- **Q10 — Decimals hardcode = footgun.** USDC=6, USDT=6, USDS=6, PYUSD=6 (Token-2022), BONK=5, JUP=6, JitoSOL=9. NEVER hardcode — `chain::spl_decimals(&mint)` → `spl_token::state::Mint::unpack(&account.data).decimals` (classic) OR `spl_token_2022::state::Mint::unpack(&account.data).decimals` (Token-2022) per detected program. Always `transfer_checked(mint, decimals)` NOT `transfer(mint)` — unchecked `transfer` would silently truncate / overflow on decimals mismatch.
- **Q11 — Cluster enum.** `Cluster::MainnetBeta | Devnet | Localnet` ONLY. Testnet excluded (Solana Foundation deprecated 2022-23). `sol config set-cluster testnet` → `Error::InvalidCluster { cluster: "testnet" }` (exit 2). Lint-block in `Cluster` enum (no variant compiles). V0.1 spec: NEVER reference `testnet` URLs in code, comments, or tests.
- **Q12 — Metaplex license blocker.** `mpl-token-metadata` 5.1.1 + `mpl-core` 0.12.1 use `Metaplex NFT Open Source License v1.0` — non-OSI, commercial restrictions. EXCLUDE indefinitely (V1.x at earliest). No `mpl-token-metadata` in dep tree. No `sol tokens nft` or NFT commands documented. Bubblegum compressed NFT = same blocker. Hard rule: `cargo deny` will refuse any PR adding Metaplex dependency.

---

## Architecture (locked 2026-09-08)

### Four-layer PAL design

```text
Layer 4: FFI (cdylib)
   - C ABI surface (extern "C" fn sol_wallet_create, sol_wallet_import, ...)
   - Panic-message scrubber (regex: redact mnemonic + seed + secret + xprv patterns)
   - tokio runtime pinned (single-threaded current_thread on FFI side)

Layer 3: Pure Rust Core (portable, ~90%)
   - address/, wallet/, tx/builder, tx/sign, tx/broadcast
   - crypto (Argon2id + AES-GCM wallet file encryption)
   - error, disambig, config (types), util
   - tokens (bundled JSON via include_str! for USDC/USDT/PYUSD mainnet)
   - chain/ (RPC client + retry + WS subscription helpers)

Layer 2: PAL — 4 traits
   - WalletStorage (encrypted blob persistence: File/Keychain/EncryptedFile impls)
   - PlatformInfo (data_dir, app_name, app_version, is_mobile)
   - NetworkClient (HTTP + TLS root certs + SPKI pin verifier — Bitcoin shape)
   - Clock (monotonic time for tx expiration + commitment checks)

Layer 1: Platform impls (~10%)
   Desktop:  FileWalletStorage, SystemDirsInfo, ReqwestClient (rustls-native-certs), SystemClock
   iOS:      KeychainWalletStorage, BundleInfo, OSRootsClient (mobile roots), IosClock
   Android:  EncryptedFileWalletStorage, ContextInfo, OSRootsClient (mobile roots), AndroidClock
   Tests:    InMemoryStorage, StaticInfo, MockClient (or surfpool-backed ReqwestClient), MockClock
```

### Wallet surface — Phantom-equivalent (the only public Wallet API)

```rust
// crates/sol-wallet-core/src/wallet.rs (REPLACES any keys.rs)
pub struct Wallet(solana_sdk::signature::Keypair);  // tuple struct — solana-sdk owns the crypto

impl Wallet {
    /// Phantom: "Import secret phrase" → defaults to m/44'/501'/0'/0'/0
    pub fn fromMnemonic(phrase: &str) -> Result<Self>;
    /// Phantom: "Add account" → m/44'/501'/{account}'/0'/{address_index}
    pub fn fromMnemonicAt(phrase: &str, account: u32, address_index: u32) -> Result<Self>;
    /// Phantom: "Import private key" (base58 64-byte secret)
    pub fn fromBase58(secret: &str) -> Result<Self>;
    /// Phantom: "Watch-only" (read-only, no sign methods)
    pub fn fromPublicKey(pubkey: Pubkey) -> ReadOnlyWallet;
    /// Phantom: base58 Ed25519 pubkey (32 bytes → 32-44 chars base58)
    pub fn publicKey(&self) -> Pubkey;
    /// Phantom: sign arbitrary VersionedTransaction (delegates to solana_signer::Signer::sign_transaction)
    pub fn signTransaction(&self, tx: VersionedTransaction) -> Result<VersionedTransaction>;
    /// Phantom: sign arbitrary bytes (Ed25519 over SHA-512 truncated)
    pub fn signMessage(&self, msg: &[u8]) -> Signature;
}

pub struct ReadOnlyWallet(Pubkey);  // NO sign methods (Phantom watch-only)
```

**Crypto delegation (zero Anza-independent code):**

- `fromMnemonic(phrase)` → `bip39::Mnemonic::from_phrase(phrase, English)` → `bip39::Seed::new(&m, "")` → `ed25519_bip32::XPrv::from_seed(seed)` → `XPrv::derive("m/44'/501'/0'/0'/0")` → `XPrv::public_key()` → `solana_sdk::Keypair::try_from(seed_bytes)` — every cryptographic step delegates to Anza or `ed25519-bip32`; no wallet-local HMAC-SHA512 or chain key code.
- `fromBase58(secret)` → `solana_sdk::Keypair::from_base58_string(secret)` directly.
- `fromPublicKey(pubkey)` → `solana_sdk::Pubkey::from_str(s)` directly.
- `signTransaction(tx)` → `tx.sign(&[keypair], tx.message.recent_blockhash())` (uses Anza `Signer::sign`).
- `signMessage(msg)` → `keypair.sign_message(msg)` (uses Anza `Signer::sign_message`).

`xpub` rename for Ed25519: Solana does NOT expose `address xpub` (Ed25519 HD has no parent public key). Equivalent is `address pubkey` — returns leaf 32-byte verification key as base58. Watch-only via `pubkey` only (cannot derive sibling addresses without seed). **Documented gap, not defect.**

---

## F47 zeroize gap (applies to seed + keypair memory)

| Gap | Mitigation in sol-wallet-core |
|---|---|
| `Keypair::from_seed(s: &[u8])` takes raw slice — no Zeroize on input | wrap seed in `Zeroizing<Vec<u8>>`; copy to 32-byte secret, drop Zeroizing immediately |
| `ed25519-bip32::XPrv::to_string()` returns `String` (no Zeroize) | wrap in `Zeroizing<String>`; never log XPrv string |
| `bip39::Seed::as_bytes()` returns `&[u8]` (no Zeroize on parent) | Zeroize-wrap master seed at construction; scope-bounded |
| `Keypair::to_bytes()` returns `[u8; 64]` (no Zeroize on output) | wrap in `Zeroizing<[u8; 64]>`; copy to disk, drop immediately |

**FFI panic-message scrubber:** `regex` crate builds once via `once_cell::Lazy`. Filters all STDERR panic messages (mnemonic word-list + seed hex + xprv `xprv...` prefix + 64-byte base58 secret pattern). FFI returns exit code 99 + scrubbed msg. Anza's Keypair + XPrv provide Zeroize-on-drop but NOT on input params — caller's responsibility.

---

## File Structure (decomposition)

```text
rust-wallet-app/crates/sol-wallet-core/
├── Cargo.toml                              # MSRV 1.89.0, exact-pinned Anza subcrates
├── rust-toolchain.toml                    # "1.89.0"
├── src/
│   ├── lib.rs                              # pub mod + re-export solana_sdk::* facade types
│   ├── error.rs                            # ~21-variant Error enum (thiserror); 5 exit codes 0-5
│   ├── address.rs                          # pubkey_from_bytes, pubkey_to_base58, is_on_curve, find_pda, parse_user_address
│   ├── wallet.rs                           # Wallet(struct Keypair) + fromMnemonic/fromBase58/fromPublicKey/signTransaction/signMessage
│   ├── read_only_wallet.rs                 # ReadOnlyWallet(Pubkey) (separated for type clarity)
│   ├── wallet_manager.rs                   # CRUD: create_with_mnemonic, import_from_phrase, import_from_pk_file, unlock, lock, summary, list, delete, rename, pubkey, keypair
│   ├── persist.rs                          # Argon2id + AES-256-GCM; atomic_write (.tmp + fsync + rename)
│   ├── config.rs                           # SolanaCluster, SolanaConfig::load, set_rpc, set_cluster, set_priority_fee, get_rpc, get_priority_fee
│   ├── crypto.rs                           # encrypt_wallet, decrypt_wallet, derive_kdf_key, random_salt
│   ├── disambig.rs                         # reject_wrong_token_program, cluster_for_mint, ensure_cluster_matches, split_token_mint
│   ├── tokens.rs                           # bundled mainnet USDC/USDT/PYUSD registry via include_str!; load_mainnet, by_symbol, decimals_for_mint, mints_for
│   ├── tx/
│   │   ├── mod.rs
│   │   ├── builder.rs                      # build_sol_transfer, build_spl_transfer_checked, build_spl_approve, build_spl_close_account, build_spl_burn, build_spl_set_authority, prepend_create_ata, prepend_compute_budget
│   │   ├── sign.rs                         # sign_sol, sign_spl, sign_only_sol, sign_only_spl
│   │   └── broadcast.rs                    # send_and_confirm (V0.1: single send + poll); V0.1.5: submit_sol, submit_spl, submit_sol_speedup, send_with_retry, wait_for_confirm
│   ├── chain/
│   │   ├── mod.rs
│   │   ├── client.rs                       # SolanaClient (wraps RpcClient); request_airdrop, get_latest_blockhash, get_balance, get_account_info, get_token_account_balance, get_minimum_balance_for_rent_exemption, get_token_accounts_by_owner, get_recent_prioritization_fees
│   │   ├── account.rs                      # discover_atas, fetch_token_supply, fetch_decimals, mint_token_program, derive_ata_with_program_id
│   │   └── pki.rs                          # (V0.1.5) SpkiPinnedVerifier (re-export bitcoin-wallet-core::chain::spki)
│   ├── platform/
│   │   ├── mod.rs                          # PAL trait definitions (4 traits × ~14 methods total)
│   │   ├── storage.rs                      # WalletStorage + InMemoryStorage (test) + FileWalletStorage (desktop)
│   │   ├── info.rs                         # PlatformInfo + StaticInfo (test) + SystemDirsInfo (desktop)
│   │   ├── network.rs                      # NetworkClient + MockClient (test) + ReqwestClient (prod with rustls)
│   │   └── clock.rs                        # Clock + MockClock (test) + SystemClock (desktop)
│   ├── util.rs                             # atomic_write, human_lamports_to_sol, human_token_amount, zeroize_secret
│   ├── tokens/                             # bundled JSON
│   │   ├── mainnet.json                    # USDC, USDT, PYUSD mainnet (canonical addresses from Circle/Tether/Solana Foundation)
│   │   ├── devnet.json                     # Circle devnet USDC (4zMMC...)
│   │   └── local.json                      # surfpool test mint placeholder
│   └── ffi.rs                              # C ABI; sol_wallet_create, sol_wallet_import, sol_wallet_unlock, sol_wallet_lock, sol_wallet_get_address, sol_wallet_sign_transaction, sol_wallet_send_sol, sol_wallet_send_spl, sol_wallet_get_balance_sol, sol_wallet_get_balance_spl, sol_wallet_last_error_message, sol_wallet_panic_message_clear
├── tests/
│   ├── common/                             # cross-phase helpers; full layout per `## Test File Structure` below
│   │   ├── mod.rs
│   │   └── surfpool_guard.rs               # SurfpoolGuard RAII helper
│   └── tests/                              # 32 library test files per deep-dive Test scenario (see `## Test File Structure` below for full layout)
└── tokens/                                 # symlink or duplicate from src/tokens/ for include_str!

rust-wallet-app/crates/sol/                  # CLI binary
├── Cargo.toml
└── src/
    ├── main.rs                              # entry, tokio runtime, tracing init, dispatch
    ├── cli.rs                               # clap Cli struct + Commands enum + per-subcommand args
    └── handlers/
        ├── mod.rs
        ├── wallet.rs                        # create, import, show, list, delete, rename, balance, send, send-speedup
        ├── address.rs                       # new, pubkey (renamed from xpub — Ed25519 HD has no xpub)
        ├── balance.rs                       # --address (SOL), --address --token (SPL)
        ├── spl.rs                           # send, approve, allowance (V0.1)
        ├── tx.rs                            # get, wait
        ├── config.rs                        # show, set-rpc, set-cluster
        └── error.rs                         # classify (exit-code mapping 0/1/2/3/4/5)
```

---

## Test File Structure (Test scenario → file layout, mapped to all phases)

Per deep-dive [§"Test scenario — sol-wallet-core V0.1" File structure × test cases](docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md) (lines 105-156), every test file is created by the phase that owns it during its `Task <N.M>`. **Phase 0 creates no tests** — Phase 0 only scaffolds the crate and confirms `cargo build -p sol-wallet-core` succeeds; the first test file lands in Phase 1.1.

### Test tree (32 library + 10 CLI = 42 files)

```text
rust-wallet-app/crates/sol-wallet-core/tests/   # created by owning phases (Phases 1-9); Phase 0 creates none
├── address_derivation.rs             # Phase 1.1: rows 1, 2 — SLIP-0010 + base58 pubkey
├── bip39_mnemonic.rs                 # Phase 1.1: row 1 part — English wordlist (12/15/18/21/24 words)
├── amount_lamport.rs                 # Phase 3.1: rows 3, 4 — Amount + as_lamport + proptest round-trip
├── argon2_kdf.rs                     # Phase 6.1: row 5 — Argon2id determinism + parameter pinning
├── aes_gcm_cipher.rs                 # Phase 6.1: row 6 — AES-GCM round-trip + tamper detection
├── mnemonic_encrypt.rs               # Phase 6.1: row 7 — mnemonic encrypt-at-rest round-trip
├── wallet_persist.rs                 # Phase 6.1: rows 8, 9, 10 — atomic write + mode 0600 + name lookup (UUID uniqueness via deep-dive J)
├── solana_config.rs                  # Phase 6.1: row 11 — SolanaConfig TOML + per-cluster defaults
├── blockhash_cache.rs                # Phase 5.1: row 12 — recent blockhash TTL cache
├── token2022_disambig.rs             # Phase 4.1: rows 13, 14 part — Token-2022 vs classic SPL guard + decimals via unpack
├── stablecoin_registry.rs            # Phase 4.1: row 14 part — USDC/USDT/PYUSD/USDS mainnet mint registry
├── spl_instruction.rs                # Phase 4.1: rows 15, 18 — SPL transfer_checked + auto-ATA-create
├── tx_serde.rs                       # Phase 3.1: row 15 SOL part — bincode tx message round-trip (SOL transfer)
├── compute_budget.rs                 # Phase 3.1: row 15 — Compute Budget builder + auto-attach
├── sign_tx.rs                        # Phase 1.2: row 16 — full sign + send with recent_blockhash
├── sign_only.rs                      # Phase 1.2: row 17 — sign_only_tx cold path
├── preflight_balance.rs              # Phase 5.1: row 19 — balance check before send (insufficient funds)
├── tx_status_parse.rs                # Phase 5.1: row 20 — TransactionStatus JSON parse + TxSummary serde
├── spki_pin.rs                       # Phase 5.5 (V0.1.5 opt-in): row 21 — SPKI pin match/mismatch
├── rpc_methods_mock.rs              # Phase 5.1 (V0.1): 15 HTTP RPC methods against wiremock — full Phase 7 critical-path binding-point coverage (4 send/confirm + 5 account/balance + 2 mint/metadata + 4 cluster info/priority fee); 5.2 adds `requestAirdrop` mock, 5.3 adds `getTransaction` mock
├── error_mapping.rs                  # Phase 5/6: row 23 — Error From + Debug redaction
├── placeholder.rs                    # Phase 8.1: rows 24, 25 — FFI panic scrubber + C ABI smoke
├── submit_sol_local.rs               # Phase 7.2: row 26 — submit_sol E2E on surfpool
├── submit_spl_local_held.rs          # Phase 7.2: row 27 — submit_spl E2E on held ATA (mock USDC)
├── submit_spl_local_fresh.rs         # Phase 7.2: row 28 — submit_spl E2E on fresh ATA (rent delta)
├── submit_spl_local_approve.rs      # Phase 7.2: row 29 — submit_spl_approve E2E + allowance view
├── send_native.rs                   # Phase 5.1 (V0.1): prepare_sol_transfer_message + sign + send + confirm; integration gated `RUN_SOL_DEVNET=1`
├── send_token.rs                    # Phase 5.1 (V0.1): prepare_spl_transfer_message + sign + send + confirm; integration gated `RUN_SOL_DEVNET=1`
# (V0.1.5) send_with_retry.rs        # Phase 5.5: rows 30, 31 — stale blockhash retry + wait_for_confirm timeout
├── boot_probe_local.rs               # Phase 7.2: row 32 — get_health boot probe
├── mainnet_smoke.rs                  # Phase 9.1: row 33 — mainnet $0.001 USDC self-send (gated RUN_SOL_MAINNET=1)
├── transport_failure.rs              # Phase 5.1: row 34 — transport failure (closed port)
├── submit_send_speedup_local.rs      # Phase 7.2: row 35 — submit_send_speedup (new sig + higher priority fee)
├── wallet_lifecycle.rs               # Phase 6.1: rows 36, 37, 38 — import_from_pk + summary + list/delete/rename lifecycle
├── common/
│   ├── mod.rs                        # re-export shim: pub mod mock_spl_usdc; surfpool_spawn; faucet; keypair_fixture
│   ├── mock_spl_usdc.rs              # SHARED with crates/sol/tests/ via test-helpers feature
│   ├── surfpool_spawn.rs             # spawn_surfpool(port) -> Child
│   ├── faucet.rs                     # airdrop_surfpool(rpc, pubkey, lamports)
│   └── keypair_fixture.rs            # throwaway_keypair() -> Keypair
└── fixtures/
    └── spki_pin_test_cert.der        # leaf cert captured from api.mainnet-beta.solana.com (Phase 5.5)

rust-wallet-app/crates/sol/tests/  # Phase 7.1 + Phase 7.2 + Phase 9.1 (CLI tests; 10 files)
├── cli_wallet.rs                     # Phase 7.1: wallet create/import/show/list/delete/rename/balance/send
├── cli_address.rs                    # Phase 7.1: address new/pubkey (Phantom UX parity)
├── cli_balance.rs                    # Phase 7.1: balance --address (SOL), --address --token (SPL)
├── cli_spl.rs                        # Phase 7.1: spl send/approve/balance/allowance
├── cli_tx.rs                         # Phase 7.1: tx get/wait (poll for confirm)
├── cli_config.rs                     # Phase 7.1: config show/set-rpc/set-cluster (Testnet rejected per Q11)
├── cli_json_output.rs                # Phase 7.2 GAP stub: row N-shape — JSON output for all 22 commands
├── cli_mainnet_smoke.rs              # Phase 9.1: loud-RED `RUN_SOL_MAINNET=1` operator-run smoke
├── cli_integration_surfpool.rs       # Phase 7.2: row V15 — surfpool spawn + ephemeral port + 22 commands
└── cli_devnet_conformance.rs         # Phase 7.2: loud-RED `RUN_SOL_DEVNET=1` cross-cluster conformance
```

### Phase-to-test-file mapping summary

| Phase | Test files it owns                                     | # files |
| ----- | ------------------------------------------------------- | ------- |
| **Phase 0** (scaffold only — no tests)                   | —                                                                                                              | 0  |
| **Phase 1** (Phantom Wallet)                             | `address_derivation`, `sign_tx`, `sign_only` (3)                                                              | 3  |
| **Phase 2** (Address surface)                             | (covered by `address_derivation` from Phase 1; no new file)                                                | 0  |
| **Phase 3** (tx::builder SOL)                            | `tx_serde`, `compute_budget` (2)                                                                            | 2  |
| **Phase 4** (SPL transfer + ATA + disambig)               | `token2022_disambig`, `stablecoin_registry`, `spl_instruction` (3)                                            | 3  |
| **Phase 5.1** (RPC + send + confirm)                      | `send_and_confirm` (1)                                                                                         | 1  |
| **Phase 5.5** (V0.1.5 opt-in SPKI)                        | `spki_pin`, `fixtures/spki_pin_test_cert.der` (2)                                                              | 2  |
| **Phase 6.1** (Wallet persistence)                        | `argon2_kdf`, `aes_gcm_cipher`, `mnemonic_encrypt`, `wallet_persist`, `wallet_lifecycle` (5)                   | 5  |
| **Phase 6.2** (Library completeness verification)         | (no test creates — verification only; cross-checks 33/34 deep-dive rows GREEN, 32/32 files compile, coverage gates)  | 0  |
| **Phase 7.1** (CLI scaffold + handlers)                   | `cli_wallet`, `cli_address`, `cli_balance`, `cli_spl`, `cli_tx`, `cli_config` (6)                             | 6  |
| **Phase 7.2** (CLI full integration)                      | `submit_sol_local`, `submit_spl_local_held`, `submit_spl_local_fresh`, `submit_spl_local_approve`, `submit_send_speedup_local`, `boot_probe_local`, `cli_integration_surfpool`, `cli_devnet_conformance`, `cli_json_output` (9) | 9  |
| **Phase 7 verification** (CLI completeness)              | (no test creates — verification only; cross-checks 21/22 deep-dive CLI rows GREEN, 10/10 CLI files compile, rows 7+13 extended via Phase 4.1/3.1 Modify) | 0  |
| **Phase 8.1** (FFI cdylib)                                | `placeholder` (1 — covers FFI smoke + panic scrubber)                                                       | 1  |
| **Phase 9.1** (mainnet smoke gate)                        | `mainnet_smoke`, `cli_mainnet_smoke` (2)                                                                       | 2  |
| common + helpers (cross-phase, no stubs)                 | `common/{mod, mock_spl_usdc, surfpool_spawn, faucet, keypair_fixture}` (5)                                  | 5  |
| **Total**                                                |                                                                                                              | **48 file-creates** (32 lib + 6 CLI + 5 common + 5 CLI extra; matches deep-dive scope)       |

**Per-Phase action:** when Phase N starts, open the file list above and create every test file assigned to that phase during its `Task <N.M>`. No file is stubbed ahead of time — the row's owning phase writes its test from scratch (TDD: red → green).

---

## Phase Set Up — Branch, labels, milestone, CI (mirror TRON plan Phase Set Up)

**Goal:** the `rust-sol-core` integration branch, its tracker vocabulary, and its CI gate all exist before any Rust code is written. Mirrors `rust-tron-core` precedent (see `.github/workflows/rust-tron-core-ci.yml`) per L25, and the `rust-eth-core` precedent before it. **Gate:** a no-op PR into `rust-sol-core` triggers `.github/workflows/rust-sol-core-ci.yml` and passes.

This phase is repo plumbing only — no crate code, no `cargo` changes. It exists because branch and tracker mistakes are expensive to unwind after work has landed: a task branched off `main` inherits none of the integration branch's history, and a PR opened against `main` bypasses the whole v0.1 review train.

### Task S.1 — Cut the `rust-sol-core` integration branch from `main`

**Files:** none (git refs only)

- [x] Confirm `main` is clean and up to date: `git status --short` empty, `git fetch origin && git rev-parse main origin/main` match.
- [x] Create the branch from `main`: `git checkout main && git pull --ff-only && git checkout -b rust-sol-core`.
- [x] Push and set upstream: `git push -u origin rust-sol-core`.
- [x] Record the base commit SHA in the ledger entry (L17) so the eventual cut PR back to `main` has a known fork point.

**Verification:** `git rev-parse --abbrev-ref HEAD` returns `rust-sol-core`; `gh api repos/:owner/:repo/branches/rust-sol-core --jq .name` returns `rust-sol-core`.

**Note:** `origin/docs/2026-09-08-solana-rust-sdks-deep-dive` (parent branch for this planning doc) already exists and holds the research + planning docs. It is a docs branch, not the integration branch — do not reuse it, and do not branch `rust-sol-core` from it.

### Task S.2 — Branch rule: every task branches from `rust-sol-core`, never `main`

**Files:** this plan (the rule below is the reference every later phase points at)

The rule, stated once so every later phase can cite it:

- **Branch from:** `rust-sol-core`. Never `main`, never another task branch.
- **PR into:** `rust-sol-core`. Never `main`.
- **Only exception:** the final v0.1 cut PR, `rust-sol-core` → `main`, opened once at the end of Phase 9 after the acceptance criteria pass.
- **Naming:** `sol/<phase>-<slug>`, e.g. `sol/phase1-wallet-keypair`, `sol/phase4-spl-ata-disambig`.

Per-task ritual:

```bash
git checkout rust-sol-core
git pull --ff-only origin rust-sol-core
git checkout -b sol/phase1-wallet-keypair
# ... work, commit (PAUSE per never-auto-commit) ...
git push -u origin sol/phase1-wallet-keypair
gh pr create --base rust-sol-core --body-file /tmp/pr-body.md   # --base is mandatory
```

- [x] `gh pr create` always passes `--base rust-sol-core` explicitly — the repo default base is `main`, so omitting the flag silently targets the wrong branch.
- [x] Before opening any PR, confirm the base: `gh pr view --json baseRefName --jq .baseRefName` must return `rust-sol-core`.
- [x] If a PR is opened against `main` by mistake, retarget it rather than reopening: `gh pr edit <n> --base rust-sol-core`.
- [x] Use `--body-file` with content in `/tmp` (GateGuard `gh-pr classifier` per memory `gate-guard-gh-pr-classifier.md` — inline body with `rm`/`rmdir` prose trips Fact-Force gate).

**Verification:** a scratch branch cut from `rust-sol-core` shows the integration branch in its history — `git merge-base --is-ancestor rust-sol-core HEAD` exits 0.

### Task S.3 — Confirm and extend tracker labels

**Files:** none (tracker state)

Two labels likely exist or must be created — verify before filing issues:

| Label            | Colour     | Meaning                    | Use in v0.1                            |
| ---------------- | ---------- | -------------------------- | -------------------------------------- |
| `rust-sol-core`  | `#c41e3a`  | SOL core crate work        | every library task (Phases 0-6, 8-9)   |
| `rust-sol-cli`   | `#1f6feb`  | `sol` CLI feature tasks    | every CLI task (Phase 7)               |

- [x] Verify both exist before filing issues: `gh label list --search rust-sol`. If absent, create: `gh label create rust-sol-core --color c41e3a --description "SOL wallet core work"` + `gh label create rust-sol-cli --color 1f6feb --description "sol CLI feature tasks"`.
- [x] Reuse the existing priority scale — `priority/p0` … `priority/p3` are already defined repo-wide. Do NOT create a parallel `P0`/`P1` set (per TRON Task S.3 precedent).
- [x] Reuse the existing `task`, `backlog`, `security`, and `documentation` labels.
- [x] Create a phase label only if issues need grouping beyond the milestone: `gh label create sol/phase-setup --color c41e3a --description "SOL v0.1 Phase Set Up"` (optional; skip if the milestone alone is sufficient).

Priority assignment for v0.1 issues:

| Priority      | Applies to                                                                                   |
| ------------- | -------------------------------------------------------------------------------------------- |
| `priority/p0` | Phase Set Up, plus the 5 critical-path modules (wallet, tx::builder, chain::client, persist, ffi) |
| `priority/p1` | Phases 1-4 — Phantom Wallet, Address, tx::builder SOL, SPL/ATA/disambig                      |
| `priority/p2` | Phases 5-7 — RPC+retry, persistence+WalletManager, CLI scaffold                                |
| `priority/p3` | Phases 8-9 polish (FFI + mainnet smoke), plus anything deferred but still tracked           |

**Verification:** `gh label list --search rust-sol` shows both labels; `gh label list --search priority/` shows p0-p3.

### Task S.4 — Create the `sol-wallet-core v0.1` milestone

**Files:** none (tracker state)

The repo may already have `polygon-v0.1` / `tron-v0.1` milestones; create the SOL equivalent.

- [x] Create it:

```bash
gh api repos/:owner/:repo/milestones -f title='sol-wallet-core v0.1' \
  -f state='open' \
  -f description='sol-wallet-core v0.1 + sol CLI v0.1 — integration branch rust-sol-core. Closes with the cut PR to main.'
```

- [ ] Attach every v0.1 issue to it as issues are filed: `gh issue edit <n> --milestone sol-wallet-core v0.1`.
- [x] Attach the umbrella issue for this plan to it.
- [x] Do NOT set a due date — the v0.1 gate is the acceptance criteria, not a calendar date.

**Verification:** `gh api repos/:owner/:repo/milestones --jq '.[].title'` includes `sol-wallet-core v0.1`.

### Task S.5 — Add `.github/workflows/rust-sol-core-ci.yml`

**Files (new):** `.github/workflows/rust-sol-core-ci.yml`

Copy the structure of `.github/workflows/rust-tron-core-ci.yml` and retarget it. Same jobs, same action pins, same least-privilege token.

- [x] `on.push.branches: [rust-sol-core]` and `on.pull_request.branches: [rust-sol-core]`, plus `workflow_dispatch: {}`. **Do not** add `main` to either list — the umbrella `ci.yml` covers main.
- [x] `permissions: contents: read` only.
- [x] `concurrency` group keyed on workflow + ref with `cancel-in-progress: true`.
- [x] Jobs: `rust-fmt` (`cargo fmt --all -- --check`), `rust-clippy` (`cargo clippy -p sol-wallet-core --all-targets -- -D warnings`), `rust-test` (`cargo test -p sol-wallet-core --lib --tests`), all with `working-directory: rust-wallet-app`.
- [x] `cargo-deny` job (`cargo deny check`) — enforces Q12 (no Metaplex dep), no GPL, no `mpl-token-metadata`/`mpl-core` in dep tree. Fails PR if any banned crate added.
- [x] Add the mobile compile-only gate as its own job (per deep-dive "Mobile build gate (CI)" + Q17): `cargo check --target aarch64-apple-ios` and `cargo check --target aarch64-linux-android`.
- [ ] ~~Pin the MSRV toolchain to `1.89.0`~~ **NOT DONE — deliberate (drift D2, verified 2026-09-10).** `rust-wallet-app/rust-toolchain.toml` hard-pins `channel = "1.98.1"` and overrides whatever toolchain the CI action installs for every cargo invocation inside that directory, so a `1.89.0` line in the workflow would be inert — it would assert a constraint the build does not enforce. 1.98.1 satisfies Anza's `rust-version = "1.89.0"` floor. Revisit only if the workspace pin drops below 1.89.0.
- [x] Action pins follow L37: tag-based to mirror the umbrella `ci.yml`, with resolved SHAs captured in a follow-up commit after the first green run.
- [x] Header comment states the scope explicitly: "any PR that targets `rust-sol-core` (NOT main)".

**Verification:** open a trivial no-op PR into `rust-sol-core`; the `rust-sol-core` workflow appears in checks and every job passes. `gh run list --workflow rust-sol-core-ci.yml --limit 1` shows a `success` conclusion.

### Task S.6 — Optional branch protection on `rust-sol-core`

**Files:** none (repo settings)

- [x] If the repo plan allows branch protection, require the `rust-sol-core` checks to pass before merge, and require at least one review.
- [ ] ~~If protection is unavailable, record that here and rely on the L13 PAUSE points instead~~ **N/A — protection WAS available and is applied (verified 2026-09-10).** This fallback branch never triggered. Applied policy: 6 required contexts, `strict: true`, linear history, 1 approving review, stale reviews dismissed, no force-push, no deletion. `enforce_admins: false`, mirroring `main` — on a solo-maintainer repo a required review the author cannot supply would otherwise hard-block every merge.

**Verification:** either protection is configured, or the fallback is written into the ledger entry.

#### Phase Set Up — Verification

- [x] `git rev-parse --abbrev-ref HEAD` = `rust-sol-core`, and the branch exists on `origin`.
- [x] `gh label list --search rust-sol` shows `rust-sol-core` + `rust-sol-cli`.
- [x] `gh api repos/:owner/:repo/milestones --jq '.[].title'` includes `sol-wallet-core v0.1`.
- [x] `.github/workflows/rust-sol-core-ci.yml` exists and its first run concluded `success`.
- [x] Umbrella issue for this plan carries the `sol-wallet-core v0.1` milestone and a priority label.
- [ ] The branch rule from Task S.2 is restated in the body of every v0.1 task issue, so an agent picking up a task cannot miss it.
- [x] `rust-wallet-app/crates/sol-wallet-core/CHANGELOG.md` exists; first entry covers Phase Set Up (plumbing) per L24 doc-update rule.

**PAUSE here.** Branch creation, label edits, milestone creation, and the workflow commit are all state-modifying — per the workflow-approval-required rule, discuss before executing, and per never-auto-commit, the workflow file is committed only after approval.

---

## Phase 0 — Scaffold + compile check (no tests in Phase 0)

### Task 0.1 (TBD): Crate scaffold + compile/`--help` test stub

**Files:**

src/ (Rust API stubs):

- Create: `rust-wallet-app/crates/sol-wallet-core/Cargo.toml`
- Create: `rust-wallet-app/crates/sol-wallet-core/rust-toolchain.toml` (or reuse workspace; Anza MSRV 1.89.0)
- Create: `rust-wallet-app/crates/sol-wallet-core/src/lib.rs` (facade with `pub use solana_sdk::*` re-exports)
- Create: `rust-wallet-app/crates/sol-wallet-core/src/error.rs` (minimal stub: 1 placeholder variant; full 21-variants land in Phase 5/6/7)

**Phase 0 creates no test files.** The 32 library tests, 10 CLI tests, 5 common helpers, and 1 fixture land in their owning phases per the mapping table above.

Workspace `Cargo.toml`:

- Modify: `rust-wallet-app/Cargo.toml` (add `sol-wallet-core` to workspace `members`)
- Modify: `rust-wallet-app/Cargo.toml` workspace `[workspace.dependencies]` table (add Anza + SPL + `ed25519-bip32` + `bip39` + `argon2` + `aes-gcm` versions)

CLI binary skeleton:

- Create: `rust-wallet-app/crates/sol/` skeleton (Cargo.toml + empty `src/main.rs`)

CI:

- Create + Modify: `.github/workflows/rust-sol-core-ci.yml` — created in Phase Set Up Task S.5; Phase 0 modifies it to add the cdylib-gated skip-guards (commits `fd36f2bd`, `757330c3`, `9153eff4` on `sol/phase0-scaffold`; PR #549).

  **Scope.** Six jobs at parity with umbrella `ci.yml`, all gated on PRs into `rust-sol-core` (NOT `main` — umbrella `ci.yml` covers main):
  - `rust-lint` (name: `Rust lint (fmt + clippy)`): `cargo fmt --all -- --check` + `cargo clippy -p sol-wallet-core --all-targets -- -D warnings`. fmt runs first (no compile); clippy second. Both scoped to the new crate (workspace-wide fmt catches drift in sibling crates too).
  - `rust-test` (name: `Rust test (sol-wallet-core)`): `cargo test -p sol-wallet-core --lib --tests`. `RUSTFLAGS="-D warnings"` at job level. Phase 0 ships 1 placeholder test (`facade_compiles`); the 32-test library suite lands in Phase 1+.
  - `rust-deps` (name: `Rust dep checks (dedup + audit + deny)`): `cargo tree --workspace --duplicates` + `cargo audit` (with `--ignore` list mirroring `ci.yml`: `RUSTSEC-2026-0098`, `RUSTSEC-2026-0099`, `RUSTSEC-2026-0104`, `RUSTSEC-2025-0111`) + `cargo deny check`. Workspace-scoped, no crate guard (they read the shared lockfile). Enforces Q12 (no Metaplex via `deny.toml` `[bans]`).
  - `rust-ffi-cdylib` (name: `Rust FFI cdylib (sol-wallet-core)`): `cargo build -p sol-wallet-core` + `test -f target/debug/libsol_wallet_core.so` post-check. Gated on the crate having cdylib declared in `[lib] crate-type`.
  - `rust-geiger` (name: `Rust unsafe-code audit (geiger)`): `cargo geiger` from inside `crates/sol-wallet-core`. Dedicated `CARGO_TARGET_DIR=/tmp/geiger-target` keeps artifacts out of the shared rust-cache. `|| true` keeps the audit advisory.
  - `mobile-check` (name: `Mobile compile-only (iOS + Android arm64)`): runs on `macos-latest` with `targets: aarch64-apple-ios,aarch64-linux-android`. Two legs gated `if: runner.os == 'macOS'` (iOS) and `if: runner.os == 'Linux'` (Android). Gated on the crate having cdylib declared.

  **Triggers / permissions / concurrency.**

  ```yaml
  on:
    push:
      branches: [rust-sol-core]
    pull_request:
      branches: [rust-sol-core]
    workflow_dispatch: {}
  permissions:
    contents: read
  concurrency:
    group: ${{ github.workflow }}-${{ github.ref }}
    cancel-in-progress: true
  ```

  **Skip-guard pattern (Phase 0 — cdylib gated).** Each crate-scoped step runs `awk '/^[[:space:]]*crate-type[[:space:]]*=/{ if ($0 ~ /"cdylib"/) exit 0; else exit 1 }' crates/sol-wallet-core/Cargo.toml` — exits 0 only when a `crate-type = [...]` line itself contains `"cdylib"`. The guard tests the MANIFEST, not a directory (Phase Set Up's own `CHANGELOG.md` creates the directory; only `Cargo.toml` proves a cargo package exists) AND not a comment (the Phase 0 manifest comment line 13 contains the literal `"cdylib"` explaining what Phase 8 will add — original `grep -q 'cdylib'` matched the comment and let the build through, runs `34437404128` + `34438180847` reproduced the failure).

  **Toolchain pin.** All six jobs use `dtolnay/rust-toolchain@stable`; `rust-wallet-app/rust-toolchain.toml` pins channel `1.98.1`, which overrides the action for every cargo invocation inside that directory. Plan asked for `1.89.0` (Anza `rust-version`); `1.89.0` would be inert since the workspace file always wins.

  **Layout mirror with `rust-tron-core-ci.yml`.** Mobile-check layout matches the tron workflow: `Install protoc` step gated `if: runner.os == 'Linux'` (macos runner doesn't need protoc; the iOS leg's `xcrun` is the macOS-only dependency). The Linux-gated Android leg is a documented no-op placeholder on the macOS runner until a Linux leg is added (per the tron workflow comment).

  **Per-job step skeleton (canonical form):**

  ```yaml
  - uses: actions/checkout@v4                  # v4
    with:
      persist-credentials: false
  - uses: dtolnay/rust-toolchain@stable        # stable (1.98.1 via rust-toolchain.toml)
    with:
      components: rustfmt, clippy              # lint job only
      targets: <arch>                          # mobile-check only
  - name: Install protoc (>=3.12)              # most jobs; gated on Linux for mobile-check
    run: sudo apt-get update && sudo apt-get install -y protobuf-compiler
  - uses: Swatinem/rust-cache@v2               # v2
    with:
      workspaces: rust-wallet-app -> target
  - name: <cargo command>
    working-directory: rust-wallet-app
    run: |
      if [ ! -f crates/sol-wallet-core/Cargo.toml ]; then ...; fi
      if ! awk '/^[[:space:]]*crate-type[[:space:]]*=/{ if ($0 ~ /"cdylib"/) exit 0; else exit 1 }' crates/sol-wallet-core/Cargo.toml; then ...; fi
      cargo <command>
  ```

  **Per L37 (action-SHA hygiene).** Actions pinned by tag mirror umbrella `ci.yml`; resolved SHAs land in a follow-up commit after the first green run captures them.

**Interfaces (V0.1 scaffolding — empty body, just compiles):**
- `sol_wallet_core::error::Error` enum with 1 placeholder variant
- `sol_wallet_core::lib.rs` `pub use` re-exports of `solana_sdk::*` facade types

**Critical pinning (each subcrate exact-pin `=x.y.z`):**

```toml
# Cargo.toml — sol-wallet-core
[dependencies]
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

spl-token                    = { workspace = true }    # 9.0.0
spl-token-2022               = { workspace = true }    # 11.0.0
spl-associated-token-account = { workspace = true }    # 8.0.0
spl-memo                     = { workspace = true }    # 7.0.0

ed25519-dalek                = "=3.0.0"
ed25519-bip32                = "=0.4.3"
bip39                        = { workspace = true, default-features = false, features = ["english"] }
argon2                       = { workspace = true }
aes-gcm                      = { workspace = true }
sha2                         = { workspace = true }
sha3                         = { workspace = true }
bs58                         = { workspace = true }
hex                          = { workspace = true }
zeroize                      = { workspace = true }
subtle                       = { workspace = true }
hmac                         = { workspace = true }

serde                        = { workspace = true }
serde_json                   = { workspace = true }
chrono                       = { workspace = true }
uuid                         = { workspace = true, features = ["v4", "serde"] }

tokio                        = { workspace = true }
reqwest                      = { workspace = true, default-features = false, features = ["json", "rustls-tls"] }
rustls                       = { workspace = true }
webpki                       = { workspace = true }
x509-parser                  = { workspace = true }

thiserror                    = { workspace = true }
tracing                      = { workspace = true }
once_cell                    = "1"
regex                        = "1"

clap                         = "4"  # CLI binary only

[build-dependencies]
cbindgen = { workspace = true }

[dev-dependencies]
proptest    = { workspace = true }
tempfile    = { workspace = true }
```

**Steps:**
- [x] Step 1: Add `sol-wallet-core` + `sol` to umbrella `members` in workspace `Cargo.toml`
- [x] Step 2: Add Anza stack + SPL + `ed25519-bip32` + crypto deps to workspace `[workspace.dependencies]`
- [x] Step 3: Create `sol-wallet-core/src/lib.rs` with module placeholders (`pub mod address; pub mod wallet; pub mod error;` + `pub use solana_sdk::*` re-exports) **PARTIAL** — `pub mod` declarations landed (`crates/sol-wallet-core/src/lib.rs:18-20`); `pub use solana_sdk::*` re-export deferred to Phase 1 because the crate doesn't depend on `solana_sdk` yet (Option 3 resolution of crates.io pin drift — see Step 6 annotation + `crates/sol-wallet-core/CHANGELOG.md` Phase 0 section). Three empty doc-only module files (`address.rs`, `error.rs`, `wallet.rs`) also created to satisfy the Rust 2021 module resolver — the plan did not ask for them but the `pub mod` declarations require their files to exist.
- [x] Step 4: Verify `cargo build -p sol-wallet-core` exits 0 (no test needed — this is the compile smoke; first test lands in Phase 1.1)
- [x] Step 5: Verify gate: `cargo fmt --all -- --check && cargo clippy -p sol-wallet-core -- -D warnings && cargo test -p sol-wallet-core`
- [x] Step 6: Verify Anza subcrate pinning — `cargo tree -p sol-wallet-core | grep solana-` shows exact `=x.y.z` pins, NO version unification **DEFERRED to Phase 1** — crates.io drift: `solana-rpc-client = "=4.2.2"` is not published (only `4.4.0-alpha.3` exists, and its manifest pins `solana-instruction >=3.4.0, <3.5.0` — incompatible with the plan's `=3.5.0`). All Anza + SPL + ed25519-bip32 pins are declared in workspace `[workspace.dependencies]` but NOT wired into `sol-wallet-core/Cargo.toml` so the build resolves; Phase 1 uncomments the Anza block at the top of that file, runs `cargo tree -p sol-wallet-core | grep solana-` to discover the actual constraint graph, picks compatible exact pins, then verifies "no version unification" on its own build before claiming done. **PHASE 5 UPDATE (2026-09-10): Anza `solana-rpc-client` DROPPED from V0.1 dep graph per issue #555 — V0.1 ships thin `reqwest` JSON-RPC client (15 HTTP methods in 5.1 + 1 in 5.2 + 1 in 5.3, full Phase 7 binding surface) with no Anza RPC dep. Workspace `[workspace.dependencies]` retains `solana-rpc-client = "=4.2.2"` as a future-V0.1.5 placeholder; V0.1 build does not pull it.** Full drift log in `crates/sol-wallet-core/CHANGELOG.md` Phase 0 section.
- [ ] Step 7: PAUSE — PR review on the scaffold PR; squash-merge only after issue body checkboxes flipped to `[x]` (L13 step 14). CI hardening detail (the awk cdylib guard + Linux-gated protoc install) lives in the "CI:" Files block above.

---

## Phase 1 — Phantom-equivalent Wallet Keypair

### Task 1.1 (TBD): Wallet::fromMnemonic(phrase) + fromMnemonicAt(phrase, account, addr_idx)

**Files:**
- Create: `rust-wallet-app/crates/sol-wallet-core/src/wallet.rs`
- Modify: `rust-wallet-app/crates/sol-wallet-core/src/lib.rs` (`pub mod wallet;`)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/address_derivation.rs` (deep-dive row 1+4 — SLIP-0010 path + is_on_curve; row 2 base58 round-trip added by Phase 2 via Modify; canonical file per Test scenario §F)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/bip39_mnemonic.rs` (Phase 1.1 owns creation + implementation — English wordlist 12/15/18/21/24 words)

**Steps:**
- [x] Step 1: Implement `Wallet(solana_sdk::signature::Keypair)` tuple struct + Phantom-equivalent API per architecture section
- [x] Step 2: Implement `fromMnemonic(phrase: &str) -> Result<Self>` → delegates to `bip39::Mnemonic::from_phrase` + `bip39::Seed::new` + `ed25519_bip32::XPrv::from_seed` + `XPrv::derive("m/44'/501'/0'/0'/0")` + `solana_sdk::Keypair::try_from(seed_bytes)`
- [x] Step 3: Implement `fromMnemonicAt(phrase, account, address_index)` → same chain with `m/44'/501'/{account}'/0'/{address_index}` path string built via `format!`
- [x] Step 4: Wrap seed + xprv in `Zeroizing<Vec<u8>>` / `Zeroizing<String>` during derivation; drop after `Keypair::try_from` consumes bytes
- [x] Step 5: Implement `tests/address_derivation.rs` — Mnemonic("abandon ×11 about") → base58 address matches Phantom canonical (cross-verify via Phantom's documented vector or `solana-keygen pubkey "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" prompt://`); same mnemonic with `--address-index 0` and `--address-index 1` produces DIFFERENT addresses (proves HD chain key works); `is_on_curve` true; accept known devnet address `2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH`; reject invalid base58 (`not-base58!!!`); reject off-curve bytes. Also implement `tests/bip39_mnemonic.rs` — assert 12/15/18/21/24-word English mnemonic validity; reject 11-word / 25-word; reject non-English wordlist; reject checksum-failing phrase.
- [x] Step 6: Verify gate: `cargo fmt + cargo clippy -p sol-wallet-core -- -D warnings + cargo test -p sol-wallet-core --test address_derivation --test bip39_mnemonic` (SLIP-0010 + HD multi-index + English wordlist acceptance)
- [x] Step 7: PAUSE — commit-push-pr; PR body cites Q1+Q2+Q3 from grill Round-1

### Task 1.2 (TBD): Wallet::fromBase58(secret) + fromPublicKey(pubkey) + sign APIs

**Files:**
- Modify: `rust-wallet-app/crates/sol-wallet-core/src/wallet.rs`
- Create: `rust-wallet-app/crates/sol-wallet-core/src/read_only_wallet.rs` (separate file for ReadOnlyWallet struct)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/sign_tx.rs` (deep-dive row 16 — full sign + send with recent_blockhash)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/sign_only.rs` (deep-dive row 17 — `sign_only_tx` cold path)

**Steps:**
- [x] Step 1: Implement `Wallet::fromBase58(secret: &str) -> Result<Self>` → delegates to `solana_sdk::Keypair::from_base58_string`; canonical 64-byte base58 secret (32-byte secret + 32-byte pubkey)
- [x] Step 2: Implement `Wallet::fromPublicKey(pubkey: Pubkey) -> ReadOnlyWallet` — does NOT consume signing material; delegates to `solana_sdk::Pubkey::from_str`
- [x] Step 3: Implement `ReadOnlyWallet(Pubkey)` with `pubkey(&self) -> Pubkey` getter ONLY (no sign methods)
- [x] Step 4: Implement `Wallet::signTransaction(&self, tx: VersionedTransaction)` → delegates to `tx.sign(&[keypair], tx.message.recent_blockhash())` (Anza `Signer::sign_transaction`)
- [x] Step 5: Implement `Wallet::signMessage(&self, msg: &[u8]) -> Signature` → delegates to `keypair.sign_message(msg)` (Anza `Signer::sign_message`)
- [x] Step 6: Implement `Wallet::publicKey(&self) -> Pubkey` → delegates to `Signer::pubkey`
- [x] Step 7: Implement `tests/sign_tx.rs` (row 16) — sign arbitrary `Transaction`; verify via `solana_sdk::transaction::verify`; non-default fee-payer reflected in signature count
- [x] Step 8: Implement `tests/sign_only.rs` (row 17 — `sign_only_tx` cold path) — `sign_only_tx` cold path: returns `(tx_base64, sig)`; performs no RPC; re-signing same input deterministic; sign arbitrary 32-byte message; verify recovered pubkey via `solana_sdk::signature::Signature::verify`
- [x] Step 9: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test sign_tx --test sign_only` (sign-only acceptance)
- [x] Step 10: PAUSE — commit-push-pr; PR body cites Q2 (HD coverage) + Q3 (Phantom UX parity)

### Phase 1 drift recorded at execution time (PR #550, commit `33798ef5`)

7 deltas between the plan text and the live Anza 4.x / ed25519-bip32 0.4.3 / solana-transaction 4.3.0 API surface, resolved as follows:

1. **SLIP-0010 path corrected from 5 to 4 components.** Steps 2 + 3 cite `m/44'/501'/{account}'/0'/{address_index}'` — 5 components with an extra `0'` slot. Standard Solana / Phantom derivation is 4 components: `m/44'/501'/{account}'/{address_index}'`. Implemented the 4-component path (Step 3 text corrected accordingly).
2. **`Keypair::try_from(&[u8])` expects 64 bytes, not 32.** Step 2 cites `Keypair::try_from(seed_bytes)` as the seed-only constructor. The actual Anza `solana-keypair 3.1.2` API has two: `try_from(&[u8])` accepts a 64-byte secret+pubkey blob (delegates to `ed25519_dalek::SigningKey::from_keypair_bytes`); `new_from_array([u8; 32])` accepts the 32-byte seed alone. Used `new_from_array`. Zeroizing still fires before the bytes drop.
3. **`XPrv::from_nonextended_force` does its own master SHA-512 stretch internally.** An initial implementation manually ran `HMAC-SHA512("ed25519 seed", seed)` to derive the master XPrv, but `ed25519-bip32 0.4.3`'s `from_nonextended_force` already does this stretch internally — passing the pre-stretched bytes caused a double-hash producing an invalid Ed25519 seed (`InvalidSeed` from `new_from_array`). Removed the manual HMAC; the function takes the raw BIP-39 seed halves directly.
4. **`derive_from_path` / `DerivationPath::from_str` don't exist in `ed25519-bip32 0.4.3`.** Steps 2 + 3 implied those as the path-walking API. The 0.4.3 API exposes only `XPrv::derive(scheme: DerivationScheme, index: DerivationIndex)` where `DerivationIndex = u32` with the top bit set meaning hardened. Walk the path iteratively via `format!` of components → array of `0x80000000 | n` → loop `derive(V2, idx)`.
5. **`Transaction::try_sign` + `Transaction::sign` are gated behind the `wincode` cargo feature.** Task 1.2 Step 4 cites `tx.sign(&[keypair], tx.message.recent_blockhash())`; both `Transaction::sign` and `try_sign` are `#[cfg(feature = "wincode")]`-gated in `solana-transaction 4.3.0`. Workspace does not enable `wincode`. Implemented manual signing: serialize the `VersionedMessage`, sign via `Signer::sign_message`, place the signature at the wallet's pubkey position in `tx.signatures`.
6. **`system_instruction` not in `solana-sdk` 4.x root.** Step 7 (test) references `solana-sdk::system_instruction::transfer`; in 4.x the system instruction module lives behind `solana-system-interface` (not a direct workspace dep). Tests use a hand-built `Instruction { program_id: Pubkey::new_unique(), accounts: vec![AccountMeta::new(payer, true)], data: vec![] }` — exercises the signing path without depending on `solana-system-interface`.
7. **`Keypair::from_base58_string` is infallible (panicking); fallible sibling is `try_from_base58_string`.** Task 1.2 Step 1 cites the panic-on-error `from_base58_string`; the fallible `try_from_base58_string` returns `Result<Self, SignatureError>` and is the correct one for `Wallet::from_base58`. Used `try_from_base58_string` + mapped to `Error::InvalidBase58Secret(usize)`.

### Phase 1 deliverable summary

- PR #550 squash-merged into `rust-sol-core` as commit `33798ef5` on 2026-09-10.
- 6/6 CI green (`rust-lint` + `rust-test` + `rust-deps` + `rust-ffi-cdylib` + `rust-geiger` + `mobile-check`).
- 21/21 tests pass: 4 `address_derivation` + 10 `bip39_mnemonic` + 3 `sign_tx` + 4 `sign_only`.
- Issue #548 Phase 1 checkbox flipped to `[x]` per `update-issues-before-merge` rule.
- CHANGELOG.md Phase 1.1 + Phase 1.2 entries (Added/Changed/Drift) per L24.

---

## Phase 2 — Address surface (base58, is_on_curve, PDA)

### Task 2.1 (TBD): Address module + base58 round-trip + is_on_curve

**Files:**
- Create: `rust-wallet-app/crates/sol-wallet-core/src/address.rs`
- Modify: `src/lib.rs` (`pub mod address;`)
- Modify: `rust-wallet-app/crates/sol-wallet-core/tests/address_derivation.rs` (Phase 1.1 file; add base58 round-trip + invalid base58 reject + off-curve bytes reject cases; deep-dive row 2 part)

**Steps:**
- [x] Step 1: Implement `pubkey_from_bytes(bytes: [u8; 32]) -> Pubkey` → `solana_sdk::Pubkey::new_from_array`
- [x] Step 2: Implement `pubkey_to_base58(pk: &Pubkey) -> String` → `pk.to_string()` (32-44 chars, no prefix)
- [x] Step 3: Implement `is_on_curve(bytes: &[u8]) -> bool` → `solana_sdk::Pubkey::is_on_curve` (rejects PDA-from-bytes footgun — PDA may NOT be on Ed25519 curve)
- [x] Step 4: Implement `parse_user_address(s: &str) -> Result<Pubkey>` → `Pubkey::from_str(s)` + `is_on_curve` check; reject invalid base58 + reject off-curve bytes
- [x] Step 5: Implement `find_pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8)` → `Pubkey::find_program_address` (V0.1.5 staking use; V0.1 internal)
- [x] Step 6: Extend `tests/address_derivation.rs` — bytes → base58 → bytes round-trip; assert accepts known valid address `2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH`; reject invalid base58 (`not-base58!!!`); reject off-curve bytes (e.g. all-zeros); accept known SHA-2/256-bip44 vector from Phantom canonical
- [x] Step 7: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test address_derivation`
- [x] Step 8: PAUSE — commit-push-pr

### Phase 2 drift recorded at execution time (PR #551, commit `3d5745bb`)

2 deltas between the plan text and the live Ed25519 / Anza `solana-sdk 4.1.0` API surface, resolved as follows:

1. **All-zero 32-byte buffer is ON the Ed25519 curve, not off.** Step 6 specified `reject off-curve bytes (e.g. all-zeros)` as the negative fixture for `parse_user_address`. `Pubkey::new_from_array([0u8; 32]).is_on_curve()` returns `true` because the Ed25519 identity point satisfies the curve equation. The naive `assert!(!zero.is_on_curve())` precondition failed both the inline unit test and the integration test. Replaced with a derived PDA from `find_pda(&[b"off-curve-fixture"], &program_id)` — `find_pda` is contractually guaranteed to return an off-curve address (Solana enforces this to prevent PDA-curve exploits), so the negative-fixture path is robust regardless of how the underlying curve library evolves.
2. **`assert!(bump < 256)` is a useless comparison.** The `bump` field returned by `find_pda` is `u8`, so `< 256` is always true and trips `clippy::unused_comparisons`. Dropped the assertion in both `src/address.rs` inline test and `tests/address_derivation.rs`. The off-curve check + determinism check (same inputs → same PDA + bump byte) still prove the `find_pda` contract.

### Phase 2 deliverable summary

- PR #551 squash-merged into `rust-sol-core` as commit `3d5745bb` on 2026-09-10.
- 6/6 CI green (`rust-lint` + `rust-test` + `rust-deps` + `rust-ffi-cdylib` + `rust-geiger` + `mobile-check`).
- 33/33 tests pass: 5 lib unit (Phase 0 `facade_compiles` + 4 new `address::tests`) + 11 `address_derivation` (4 Phase 1.1 + 7 Phase 2) + 10 `bip39_mnemonic` + 4 `sign_only` + 3 `sign_tx`.
- Issue #548 Phase 2 checkbox flipped to `[x]` per `update-issues-before-merge` rule (commit SHA `d431f111`, PR #551 referenced).
- CHANGELOG.md Phase 2 entry (Added / Changed / Drift) per L24.

---

## Phase 3 — tx::builder native SOL (system_instruction::transfer + Compute Budget)

### Task 3.1 (TBD): tx::builder build_sol_transfer + Compute Budget prepend

**Files:**
- Create: `src/tx/mod.rs`
- Create: `src/tx/builder.rs` (Phase 3 SOL builder; Phase 4 adds SPL builders in same file)
- Modify: `src/lib.rs` (`pub mod tx;`)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/amount_lamport.rs` (deep-dive rows 3+4 — `Amount` newtype + `as_lamport` + proptest round-trip)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/compute_budget.rs` (deep-dive row 15 — Compute Budget builder + auto-attach)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/tx_serde.rs` (deep-dive row 15 SOL part — bincode tx message round-trip)

**Steps:**
- [x] Step 1: Implement `build_sol_transfer(from: &Pubkey, to: &Pubkey, lamports: u64) -> Vec<Instruction>` → `system_instruction::transfer(from, to, lamports)`
- [x] Step 2: Implement `prepend_compute_budget(units: u32, micro_lamports: u64) -> [Instruction; 2]` → `[ComputeBudgetInstruction::set_compute_unit_limit(units), ComputeBudgetInstruction::set_compute_unit_price(micro_lamports)]`
- [x] Step 3: Implement `build_sol_transfer_with_budget(from, to, lamports, cu_limit, priority_fee) -> Vec<Instruction>` → calls both; returns 3-ix vector
- [x] Step 4: Wire defaults: `cu_limit = 150_000`, `priority_fee = 0` (Q8) per `SolanaConfig`; read full per-tx-type 6-row defaults table from deep-dive §C (line 1592) before Phase 5 integration
- [x] Step 5: Implement `tests/tx_serde.rs` — bincode `Message::new_with_blockhash(&[ix, cu_ix], Some(&from), &blockhash)` round-trip; decoded message equals input for every builder (100% builder coverage)
- [x] Step 6: Implement `tests/amount_lamport.rs` — `Amount::from_lamports(u64)` + `as_lamport()` + `Amount::from_sol(f64)`; proptest round-trip via `proptest!`; reject overflow at `u64::MAX`; assert `Amount::ZERO.lamports() == 0`
- [x] Step 7: Implement `tests/compute_budget.rs` — against surfpool; fresh blockhash → sign with keypair → send_tx → assert `get_balance` recipient == lamports transferred - 5000 base fee; verify auto-attach prepends ComputeBudget ix in correct position
- [x] Step 8: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test amount_lamport --test tx_serde --test compute_budget`
- [x] Step 9: PAUSE — commit-push-pr

### Phase 3 drift recorded at execution time (PR #552, commit `1aec6bea`)

1. **`system_instruction::transfer` is feature-gated behind `bincode`/`wincode`** in `solana-system-interface` 3.3.0 (`src/instruction.rs` line 902). A naive `solana-system-interface = "=3.3.0"` workspace entry fails to compile the builder — the function symbol is `cfg`-stripped. Fixed by adding `features = ["bincode"]` to the workspace entry. The `bincode` feature is also what Anza 4.1.0 pulls transitively, so no transitive-pin drift.

2. **`ComputeBudgetInstruction` uses Borsh on the wire, not serde** — `solana-compute-budget-interface` 3.1.0's serde derive uses a different variant-tag layout than the Borsh derive the Solana runtime uses. Initial `tests/compute_budget.rs` decoded the data field via `bincode::deserialize::<ComputeBudgetInstruction>` (after enabling `serde` feature) and failed with "invalid value: integer `51200002`, expected variant index 0 <= i < 5". Switched the assertions to manual Borsh decode: `data[0]` variant tag (`0x02` = `SetComputeUnitLimit`, `0x03` = `SetComputeUnitPrice`) + little-endian payload. Tag values match the `to_instruction!` macro at `solana-compute-budget-interface-3.1.0/src/lib.rs` lines 26-29.

3. **`solana_sdk::system_program::id()` is no longer re-exported by `solana-sdk` 4.1.0** — the modern Anza split hoists the system_program ID to `solana_system_interface::program::ID`. Plan §Task 3.1 Step 1 referenced `system_instruction::transfer` without naming the source crate; the implicit assumption (that the `system_program` module lives under `solana_sdk`) no longer holds. Replaced all four call sites with `solana_system_interface::program::ID` (renamed to `SYSTEM_PROGRAM_ID` locally).

4. **Phase 3 test surface is 18 tests + 9 inline unit tests, not 7 tests** as the plan §Step 5-7 implied. The plan described one test file per step; we landed three integration files (`amount_lamport` 9 tests, `tx_serde` 5 tests, `compute_budget` 4 tests) covering wire-format invariants the plan only sketched. The total `sol-wallet-core` suite moved from 33 tests (post-Phase 2) to 60 tests (post-Phase 3) — 27 tests added, +0 regressions.

### Phase 3 deliverable summary

- **Commit:** `7c290905` (feature branch); `1aec6bea` (squash-merged into `rust-sol-core`)
- **PR:** #552
- **CI:** 6/6 jobs green (lint, test, dep checks, FFI cdylib, geiger, mobile-check)
- **Test count:** 60/60 pass (was 33 after Phase 2; +27 from Phase 3)
- **Public surface added:** `Amount` newtype + `tx::builder::{build_sol_transfer, compute_budget_instructions, build_sol_transfer_with_budget, DEFAULT_COMPUTE_UNIT_LIMIT}` + `Error::InvalidAmount`
- **Workspace deps added:** `solana-system-interface = "=3.3.0"` (bincode feature), `solana-compute-budget-interface = "=3.1.0"` (serde feature)

---

## Phase 4 — SPL transfer + ATA + Token-2022 disambig

### Task 4.1 (TBD): tx::builder SPL transfer_checked + ATA lifecycle + Token-2022 disambig

**Files:**
- Modify: `src/tx/builder.rs` (add SPL builders — `build_spl_transfer_checked`, `build_spl_approve`, `build_spl_close_account`, `prepend_create_ata`)
- Create: `src/disambig.rs` (`disambig::reject_wrong_token_program`, `cluster_for_mint`, `ensure_cluster_matches`)
- Create: `src/tokens.rs` + `src/tokens/mainnet.json` + `src/tokens/devnet.json` (bundled mint registry via `include_str!`)
- Modify: `src/chain/account.rs` (derive ATA + detect token program + fetch_decimals)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/token2022_disambig.rs` (deep-dive row 13+14 — Token-2022 vs classic SPL guard + decimals via `Mint::unpack`)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/stablecoin_registry.rs` (deep-dive row 14 — USDC/USDT/PYUSD/USDS mainnet mint registry)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/spl_instruction.rs` (deep-dive row 15 SPL + row 18 — SPL `transfer_checked` round-trip + auto-ATA-create prepend)

**Steps:**
- [x] Step 1: Implement `detect_token_program(mint: &Pubkey) -> TokenProgram` → `getAccountInfo(mint).owner` returns `TokenkegQ...` (`Classic`) or `TokenzQdB...` (`Token2022`); error otherwise
- [x] Step 2: Implement `derive_ata_with_program_id(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey` → `spl_associated_token_account::get_associated_token_address_with_program_id`
- [x] Step 3: Implement `prepend_create_ata(payer: &Pubkey, owner: &Pubkey, mint: &Pubkey) -> Instruction` → `spl_associated_token_account::instruction::create_associated_token_account_idempotent(payer, owner, mint)`
- [x] Step 4: Implement `build_spl_transfer_checked(from, to, mint, amount, decimals) -> Vec<Instruction>` → `spl_token::instruction::transfer_checked(token_program, source, mint, dest, authority, signer, amount, decimals)` (Q6 — use detected `token_program_id`). Actual API: takes `(source, mint, destination, authority, token_program: TokenProgram, amount, decimals) -> Vec<Instruction>` and dispatches by `token_program` to `spl_token::instruction::transfer_checked` (Classic) or `spl_token_2022::instruction::transfer_checked` (Token-2022) per drift item 9.
- [x] Step 5: Implement `build_spl_approve(owner, delegate, mint, amount)` → `spl_token::instruction::approve`. Actual API: takes `(source, delegate, owner, token_program: TokenProgram, amount)`; same Classic/Token-2022 dispatch.
- [x] Step 6: Implement `build_spl_close_account(owner, ata, recipient_rent)` → `spl_token::instruction::close_account`. Actual API: takes `(account, destination, owner, token_program: TokenProgram)`; same dispatch.
- [x] Step 7: Implement `fetch_decimals(mint, rpc_client) -> u8` → `spl_token::state::Mint::unpack(mint_account.data).decimals` (classic) OR `spl_token_2022::state::Mint::unpack(mint_account.data).decimals` (Token-2022); per Q10 NEVER hardcoded. **DEFERRED to Phase 5.** Phase 4 ships the pure-Rust helper `tokens::decimals_from_state_bytes(data, program) -> Result<u8>` (no RPC). Phase 5 wraps it with `RpcClient::get_account_info(mint)`.
- [x] Step 8: Implement `disambig::reject_wrong_token_program(mint, attempted_program) -> Result<()>` — if `mint.owner ≠ attempted_program`, error with `Error::InvalidTokenProgram`
- [x] Step 9: Implement `tests/token2022_disambig.rs` — classic + Token-2022 mint detection via mock `getAccountInfo`; assert classic ATA ≠ Token-2022 ATA for same `(owner, mint)`; `Mint::unpack(mock_account.data).decimals` returns 6 for USDC-shaped, 5 for BONK-shaped; `disambig::reject_wrong_token_program(USDC mint, Token2022 program)` errors with `Error::InvalidTokenProgram`. Actual: 9 tests covering program-ID distinctness, `TokenProgram::from_program_id` round-trip + unknown reject, `reject_wrong_token_program` pass/mismatch, ATA derivation differences, classic + Token-2022 `Mint::unpack` decimals read, truncated-state rejection.
- [x] Step 10: Implement `tests/stablecoin_registry.rs` — bundle `tokens/mainnet.json` via `include_str!`; `tokens::by_symbol("USDC")` returns mainnet USDC mint pubkey; `decimals_for_mint(USDC)` returns 6; unknown symbol returns `None`; PYUSD + USDT + USDS all parse from registry. Actual: 8 tests; bundled entries are USDC + USDT + USDS (mainnet) + Circle devnet USDC (devnet).
- [x] Step 11: Implement `tests/spl_instruction.rs` — `build_spl_transfer_checked` round-trip via bincode; `prepend_create_ata` + `build_spl_transfer_checked` = 2 instructions in correct order; rejected `transfer_checked` with mismatched decimals returns builder error. Actual: 7 tests.
- [x] Step 12: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test token2022_disambig --test stablecoin_registry --test spl_instruction`
- [x] Step 13: PAUSE — commit-push-pr; PR body cites Q6 (Token-2022 disambig) + Q10 (decimals never hardcoded). PR #553 squash-merged into `rust-sol-core` as commit `894bc861` on 2026-09-10; 6/6 CI green; 94/94 tests pass.

### Phase 4 drift recorded at execution time (PR #553, commit `894bc861`)

Twelve deltas between the plan text and the live Anza 4.1.0 / spl-token 9.0.0 / spl-token-2022 11.0.0 / spl-associated-token-account 8.0.0 API surface, resolved as follows:

1. **`fetch_decimals(mint, rpc_client) -> u8` (Step 7) deferred to Phase 5.** The plan's signature depends on `RpcClient` which lives in `chain::client.rs` (Phase 5.1 Step 1). Phase 4 ships `tokens::decimals_from_state_bytes(data, program) -> Result<u8>` — the pure-Rust unpack helper — and 2 tests (`mint_unpack_reads_decimals_for_classic_state`, `mint_unpack_reads_decimals_for_token2022_state`) exercise it on synthetic 82-byte state. Phase 5.1 wraps the helper with `RpcClient::get_account_info(mint)`. Drift also drops the plan's "Modify: src/chain/account.rs" since the `chain` module is Phase 5 territory.
2. **No `instruction` / `bincode` / `serde` cargo features on `spl-token` 9.0.0 / `spl-token-2022` 11.0.0.** Pod-based crates ship every public module under default features. `spl-token` feature list = `no-entrypoint`, `test-sbf`. Plan §Step 1-7 implicitly relied on feature gates; Phase 4 leaves the workspace dep entries feature-free.
3. **`spl_token::instruction::transfer_checked` rejects Token-2022 program IDs.** Unconditional dispatch produced `IncorrectProgramId` for `TokenProgram::Token2022` (the function validates `program_id == spl_token::ID`). Fixed by `match token_program` routing Classic → `spl_token::instruction::*`, Token-2022 → `spl_token_2022::instruction::*` (the latter is `#[deprecated]` since spl-token-2022 9.1.0 — SPL team points users at `spl-token-2022-interface` — but remains functional + uses `Pubkey`, matching the Anza SDK 4.1.0 ABI).
4. **Duplicate `entrypoint` symbol from `spl-token` + `spl-token-2022`.** Linker fails "duplicate symbol: entrypoint" when both crates link into a single binary (the test harness). Fixed by `features = ["no-entrypoint"]` on both workspace entries. Wallet lib code never calls `entrypoint`; the disable is safe.
5. **`spl_token::state::Mint::SIZE` not accessible.** Trait `SizedTypeProperties` provides `SIZE` but is not re-exported by `spl-token` 9.0.0. Replaced with `const MINT_STATE_SIZE: usize = 82` literal in `tokens.rs`. Same for the test fixtures. The literal matches the on-chain wire layout documented at `solana-program/token/program/src/state.rs`.
6. **`Mint::unpack` rejects `is_initialized = 0`.** `Pack::unpack` requires `data[45] = 1`. Initial fixture (`vec![0u8; 82]`) errored with `UninitializedAccount`. Fixed by patching `data[45] = 1` in both classic + Token-2022 unpack tests.
7. **`pubkey_const!` macro unavailable in Anza SDK 4.1.0.** Dropped re-export from `solana_sdk::pubkey`. Plan example code used `pub const CLASSIC_TOKEN_PROGRAM_ID: Pubkey = pubkey_const!(...)`. Replaced with `pub fn classic_token_program_id() -> Pubkey` returning `Pubkey::from_str(...).expect(...)` — `Pubkey` is `Copy`, so callers use `&classic_token_program_id()` (e.g., `derive_ata_with_program_id(&owner, &mint, &classic_token_program_id())`).
8. **`Mint::unpack` requires `Pack` trait in scope.** The `unpack` function is a trait method on `solana_program_pack::Pack`. spl-token 9.0.0 re-exports the trait as `spl_token::solana_program::program_pack::Pack` (always available, no feature gate). Added `use spl_token::solana_program::program_pack::Pack as _;` in `tokens.rs` to bring `Mint::unpack` and `Mint::SIZE` into scope.
9. **`spl_token_2022::instruction::*` is `#[deprecated]` since 9.1.0.** Phase 4 dispatch uses the deprecated path (still functional + uses `Pubkey`). The interface crate uses Anza `Address` type — would force `&Pubkey → &Address` conversions across the builder API. Phase 7 may migrate to `spl_token_2022_interface::*` with bridging helpers; Phase 4 accepts deprecation warnings.
10. **`build_spl_transfer_checked` signature drift.** Plan cites `(from, to, mint, amount, decimals)` — actual Anza `spl_token::instruction::transfer_checked` signature is `(token_program_id, source_pubkey, mint_pubkey, destination_pubkey, authority_pubkey, signer_pubkeys, amount, decimals)`. Phase 4 builder wraps this as `build_spl_transfer_checked(source, mint, destination, authority, token_program, amount, decimals) -> Vec<Instruction>` for a clean Phase 7 CLI surface.
11. **Test #11 (`prepend_create_ata_plus_transfer`) confused `Instruction` vs `Pubkey`.** Initial draft passed `dest_ata` (the `Instruction` returned by `prepend_create_ata`) as the source/destination pubkey argument to `build_spl_transfer_checked`. Two distinct values conflated. Fixed by calling `derive_ata_with_program_id` separately to get the `Pubkey`, then `prepend_create_ata` for the ix, then `build_spl_transfer_checked(..., &dest_ata_pubkey, ...)` for the transfer.
12. **`Cargo.toml` / `lib.rs` / `error.rs` edits tripped the GateGuard fact-forcing gate** (14+ denials this session). Each Edit/Write required inline presentation of (1) importers, (2) affected public API, (3) data schemas, (4) verbatim user instruction. Cost is roughly +200 tokens per denial — total overhead ~2.8k tokens. Tracked here as drift because future phases will hit the same wall on any `Cargo.toml` / `lib.rs` / `error.rs` change.

### Phase 4 deliverable summary

- PR #553 squash-merged into `rust-sol-core` as commit `894bc861` on 2026-09-10.
- 6/6 CI green (`rust-lint` + `rust-test` + `rust-deps` + `rust-ffi-cdylib` + `rust-geiger` + `mobile-check`).
- 94/94 tests pass: 24 lib unit (Phase 3 12 + Phase 4 12 new) + 9 `token2022_disambig` + 8 `stablecoin_registry` + 7 `spl_instruction` + 11 `address_derivation` + 10 `bip39_mnemonic` + 3 `sign_tx` + 4 `sign_only` + 4 `tx_serde` + 4 `compute_budget` + 9 `amount_lamport`.
- Issue #548 Phase 4 checkbox flipped to `[x]` per `update-issues-before-merge` rule.
- CHANGELOG.md Phase 4 entry (Added / Changed / Drift / Test-coverage / `pubkey_const!` macro migration note) per L24.

---

## Phase 5 — RPC client (15 HTTP methods, 5.1) + `requestAirdrop` (5.2) + `getTransaction` (5.3) + rate limiter (5.4) + SPKI escape hatch (5.5) + send/confirm + Phase 7 binding-point surface

### Security review log (2026-09-10, post-`/ecc:security-review`)

12 findings ranked by severity. Tier 1 lands in 5.1 (blocks merge); Tier 2 lands in 5.1 (cheap hardening); Tier 3 lands in 5.2/5.4/5.5; Tier 4 is doc-comment only.

| # | Tier | Finding | Where addressed |
|---|---|---|---|
| 1 | 🔴 Tier 1 | No URL allowlist / scheme check in `RpcClient::new` — `http://attacker:8080` exfiltrates signed tx | Task 5.1 Step 1 + new Step 2 (URL allowlist) |
| 2 | 🔴 Tier 3 | No SPKI pinning in V0.1 = MITM via compromised CA | Task 5.5 — `RpcClient::new_with_pinned_spki()` escape hatch (V0.1 ships; full `pinned://` URL scheme deferred to V0.1.5) |
| 3 | 🟠 Tier 2 | Plaintext keypair file (raw 64-byte at `~/.config/sol-wallet/wallet.json`) | Task 5.1 Step 24 (warning doc) + Phase 6 (Argon2id encryption) |
| 4 | 🟡 Tier 4 | `simulateTransaction` TOCTOU — preflight result is a hint, not a guarantee | Task 5.1 Step 1 doc comment; re-simulation-after-sign deferred to V0.1.5 |
| 5 | 🟠 Tier 3 | No rate limiter on `RpcClient` outbound — 429-bypass + duplicate-send footgun | Task 5.4 — token bucket (50 req/s, burst 100) |
| 6 | 🟠 Tier 3 | `requestAirdrop` unrestricted — mainnet call is DoS/loss vector | Task 5.2 Step 1 (devnet host allowlist) |
| 7 | 🟡 Tier 2 | JSON-RPC envelope parsing uses `serde_json::Value` first, typed second — silent future-compat breaks | Task 5.1 Step 1 (typed `RpcResponse<T>` + `#[serde(deny_unknown_fields)]`) |
| 8 | 🟡 Tier 2 | `bincode` wire format not pinned + no round-trip test | Task 5.1 Step 1 (`bincode = "=1.3.3"` already pinned; add round-trip test) |
| 9 | 🟡 Tier 3 | `getRecentPrioritizationFees` MITM-bait — hostile RPC returns `u64::MAX` fee | Phase 7 CLI `--max-priority-fee` cap with auto-clamp (out of Phase 5 scope) |
| 10 | 🟡 Tier 3 | `ConfirmTimeout` (30s) vs `Finalized` (~5s lockup) — pending tx misclassified as failed | Task 5.1 acceptance (add `Error::ConfirmPending` variant) |
| 11 | 🟢 Tier 2 | `RpcClient` `Debug` impl prints URL with query string — API key leak via `dbg!()` | Task 5.1 Step 1 (custom `Debug` impl strips query string) |
| 12 | 🟢 Tier 3 | `--wallet-file` permission check missing | Phase 7 CLI (out of Phase 5 scope) |
| 13 | 🟢 Tier 3 | Devnet test keypair could leak via CI if `tests/fixtures/test_keypair.json` committed | Phase 7 CLI (`.gitignore` + `throwaway_keypair()` helper already planned in Phase 6.1) |

### Design decision log (2026-09-10, 2nd Phase 5 attempt)

**Original Phase 5 spec (this file pre-redesign):** 21-method RPC coverage (16 HTTP + 5 WS) via Anza `solana_client::nonblocking::rpc_client::RpcClient` + `PubsubClient`, full BlockhashCache with TTL, retry-on-stale-hash, SPKI pin verifier.

**Why redesigned:** Issue #555 (BLOCKED) + plan §Task 0.1 Step 6 drift = intrinsic Anza SDK sub-dep conflict between `solana-sdk 4.1.0` (only stable) and `solana-rpc-client 4.2.2` (only stable rpc client). Sub-deps (wincode ^0.5 vs ^0.6, solana-address ^2.7 vs <2.7, solana-short-vec ^3.3 vs <3.3) do not unify under any combination of crates.io releases. Recipe 1 (monorepo `[patch.crates-io]` SHA) empirically falsified — `d3f1f55` (= v4.1.0 tag) does align `wincode 0.5.3` + `solana-address 2.6.0` + `solana-short-vec 3.2.1` BUT standalone-repo crates (wincode, solana-address, solana-short-vec, solana-program-error) resolved at crates.io latest versions by transitive deps in other crates (solana-pubkey, spl-type-length-value) re-introduce the wincode 0.5/0.6 dual-resolve. ~$140 cost across two sessions; no path forward via Recipe 1.

**Redesigned surface (this section):** Drop the Anza `solana-rpc-client` dependency. Build a thin `reqwest` JSON-RPC client internally with **15 HTTP methods in 5.1**, plus **2 follow-on tasks** (`requestAirdrop` in 5.2, `getTransaction` in 5.3) — the full set the `sol` CLI (Phase 7) needs to implement all 22 V0.1 commands. Reuse all Phase 1-4 code unchanged. Defer only the 5 WS subscribes + SPKI pin + BlockhashCache + retry-on-stale-hash to V0.1.5 / Phase 5.5. This is Recipe 2 from #555. Phase 5 is a *library API contract for Phase 7*, not a minimal "send native" path.

**Why 17 methods (not 21, not 5), split as 5.1 + 5.2 + 5.3:** Phase 7 binds 22 commands; each command needs at least one RPC method (except `keygen`/`import`/`export` which are pure Phase 1 surface). 15 methods in 5.1 cover the critical path: 4 for send/confirm (`getLatestBlockhash` + `sendTransaction` + `getSignatureStatuses` + `simulateTransaction`), 5 for account/balance (`getBalance` + `getAccountInfo` + `getMultipleAccounts` + `getTokenAccountBalance` + `getTokenAccountsByOwner`), 2 for mint/metadata (`getTokenSupply` + `getMinimumBalanceForRentExemption`), 4 for cluster info + priority fee (`getRecentPrioritizationFees` + `getVersion` + `getEpochInfo` + `getHealth`). Task 5.2 adds `requestAirdrop` (devnet-only helper, 1 method). Task 5.3 adds `getTransaction` (full log for `sol tx`, 1 method + base64 wire decode). Each is a separate task with its own PR + verify gate.

**What we lose vs original 21-method spec:** the 5 WS subscribes (`account_subscribe` / `signature_subscribe` / `program_subscribe` / `logs_subscribe` / `slot_subscribe`) — all deferred to V0.1.5 watch mode.

### Grilled decisions (2026-09-10, post-`/mattpocock-skills:grill-with-docs phase 5`)

14 questions, 4 rounds, all user-approved as recommended. These are the design decisions that disambiguate terms, scope, and semantic boundaries.

**Round 1 — granularity + scope:**

1. **Q1 (RPC method granularity):** `getSignatureStatuses` is ONE method on `RpcClient`; call-site shaping (commitment filter, address pagination, time-budget) lives in `tx::broadcast::wait_for_confirm` + Phase 7 handlers. `RpcClient` = thin Anza replacement, NOT a high-level wallet API.
2. **Q2 (wallet file semantics):** ONE path (`~/.config/sol-wallet/wallet.json`) with versioned format. V0.1 = `{"version": 1, "keypair_b58": "..."}` (raw 64-byte); Phase 6 = `{"version": 2, "kdf": {...}, "cipher": {...}, "encrypted_payload": "..."}` (Argon2id-encrypted BIP-39 mnemonic). `version` field disambiguates. Phase 6 ships a migration script that reads v1, prompts for password, writes v2.
3. **Q3 (`--processed` commitment locus):** V0.1's `sol send` supports only `confirmed` (default) + `finalized` (`--finalized` flag). `--processed` is V0.1.5 watch mode. `sol wait` (already in Phase 7 list) takes explicit `--commitment {processed,confirmed,finalized}` in V0.1 because the user already has the signature and just wants to poll its state.
4. **Q4 (`preflight` shape):** LIBRARY, not a single entry point. `chain::preflight` exposes 5 independent functions; Phase 7 `sol send` handler picks which to run. `sol send <addr> <amount>` calls `check_native_balance` only. `sol send-token <mint> <addr> <amount>` calls `check_ata_exists` + `resolve_mint_decimals` + `check_token_balance`. `sol send-token --create-ata` additionally calls `check_rent_exempt(165)`. Monolithic `preflight::check_all()` would force 4 RPC round-trips on a 0.001 SOL transfer that doesn't need them.

**Round 2 — terms + scope:**

5. **Q5 (`Error` enum shape):** FLAT enum, 9 variants in V0.1. Splitting into `ChainError` + `TxError` + `WalletError` deferred to V0.2 if pattern-match pain surfaces. V0.1 callers (Phase 7 CLI) do simple `if let Err(Transport(_)) = ...` patterns, not nested match — flat enum is sufficient.
6. **Q6 (`RpcClient::new` failure mode):** CONSTRUCTOR returns `Result<Self>`. URL allowlist fail, TLS handshake fail, malformed URL all return `Err(Transport)`. No separate `.validate()` method. Phase 7 CLI handlers propagate with `?`. Builder pattern (`RpcClient::builder().url(...).rate_limit(...).build()`) deferred to V0.1.5 if more config knobs accumulate.
7. **Q7 (rate limiter + URL allowlist ordering):** URL allowlist check FIRST in `RpcClient::new`. Rate limiter constructed only AFTER URL allowlist passes. If allowlist fails, no `Arc<RateLimiter>` is allocated (no resource leak). Order: `parse URL → check scheme/host → return Err(Transport) if bad → construct `reqwest::Client` with timeout → construct `RateLimiter` with defaults → wrap in `Arc` → return `Self`.
8. **Q8 (`requestAirdrop` host check vs `send` host check):** PER-METHOD check, NOT per-`RpcClient` mode flag. `request_airdrop` checks the URL host against the devnet allowlist; `send` / `sendTransaction` works on any host (mainnet, devnet, testnet, local). Rationale: `requestAirdrop` is a CLUSTER-LEVEL restriction (mainnet rejects it); `send` is universal. The `RpcClient` doesn't carry a "Devnet" / "Mainnet" mode — the methods themselves enforce their own host policy.

**Round 3 — semantic boundaries:**

9. **Q9 (`ConfirmPending` vs `ConfirmTimeout` boundary):** ELAPSED TIME, not poll cycles. `wait_for_confirm` returns `Error::ConfirmPending { signature, elapsed_ms: (timeout / 2).as_millis() as u64 }` at `timeout / 2` if some status returned but commitment not reached; returns `Error::ConfirmTimeout { signature, waited_ms: timeout.as_millis() as u64 }` at full `timeout`. For `--finalized` on a stalled cluster, the tx is `Pending` (still valid) at 15s, `Timeout` at 30s. CLI surfaces "tx pending — check <explorer>" for `Pending`, "tx may or may not land — check <explorer>" for `Timeout`.
10. **Q10 (`Error::Transport` payload):** FLAT `String` in V0.1, structured `reqwest::Error` source via `#[source]` in V0.1.5 if CLI logging needs the structured fields. V0.1 `String` is sufficient because the CLI's only consumer is the user-facing message ("Connection refused", "Timeout after 30s", "TLS handshake failed"); the CLI does not need to pattern-match on `is_timeout()` vs `is_connect()`.
11. **Q11 (BlockhashCache location, V0.1.5):** `tx::broadcast` (sits between caller + `RpcClient`), NOT inside `RpcClient`. The cache is a TX-LAYER concern (avoid re-fetching the blockhash for re-sign-and-retry on stale hash), not an RPC-LAYER concern. Putting it in `tx::broadcast` keeps `RpcClient` minimal and lets the cache have its own TTL clock independent of RPC request pacing.

**Round 4 — CLI/UX surface:**

12. **Q12 (`--no-simulate` vs default):** DEFAULT = `simulateTransaction` runs before every send. `--no-simulate` is the opt-out for latency-sensitive sends (e.g. 0.001 SOL transfer where the 200ms simulation overhead is worse than the rare over-CU failure). V0.1 ships simulation on by default. The simulation catches ~all "tx will fail at broadcast" cases (insufficient funds, invalid account, CU overrun) for ~200ms; for V0.1's target user (devnet + early mainnet), the safety > speed tradeoff is right.
13. **Q13 (Error code translation):** `Error::BroadcastFailed { kind }` stores Anza's `ClientError` variant name (PascalCase: `BlockhashNotFound`, `BlockCleanedUp`, `AccountNotFound`). Phase 7 CLI matches on these exact strings. NOT the JSON-RPC error message (lowercase). The source is the Anza variant because the V0.1 transport emits its own error mapping, not raw JSON-RPC.
14. **Q14 (`Error::Rpc` exposure):** RAW `i32` code + `String` message in V0.1. Typed enum (parse error / invalid request / method not found / invalid params / internal error / server error + app-specific codes) deferred to V0.1.5 if Phase 7 CLI needs to surface a specific code. V0.1: CLI renders `Error::Rpc { code, message }` as `"RPC error <code>: <message>"` — sufficient for human consumption, not for programmatic dispatch.

**What this gets us:** unblocks Phase 7's full 22-command CLI. Defeats plan §Step 2 rule "no custom JSON-RPC envelope code" — accepted trade-off, recorded in drift log. Preserves all Phase 1-4 work untouched. Phase 7 binds to a stable library surface, not a moving target.

**Open design questions (decided 2026-09-10, session-end):**

1. Wallet keypair source → raw 64-byte keypair file (mode 0600) at `~/.config/sol-wallet/wallet.json` for V0.1; Phase 6 swaps to Argon2id-encrypted BIP-39 mnemonic file.
2. Confirm commitment default → `confirmed` (1 slot, ~400ms), `--finalized` flag for high-value sends (matches Phantom).
3. Compute Budget defaults → 150_000 CU limit + 0 microlamports priority fee (Phantom-equivalent), both CLI-overridable.
4. Priority-fee auto-estimation → on by default for `sol send --priority-fee auto`; falls back to 0 if RPC unavailable. Off via `--no-priority-fee-auto`.
5. Send dry-run default → `simulateTransaction` runs before every send unless `--no-simulate`; surfaces compute-budget overrun as `Error::ComputeBudgetExceeded` before broadcast.

### Task 5.1 (TBD): `chain::{client, account, preflight}` (15 HTTP methods via reqwest JSON-RPC) + `tx::{broadcast, native, spl}` + Phase 5.1 test suite (36 tests)

**Files:**

- Create: `src/chain/mod.rs` (re-export `RpcClient`, `RpcError`, `BlockhashCache`, `BlockhashCacheEntry`, `RateLimiter`, `DEFAULT_BLOCKHASH_TTL`, `DEFAULT_RATE_LIMIT_RPS`, `DEFAULT_RATE_LIMIT_BURST`, `DEFAULT_REQUEST_TIMEOUT`; 15 RPC method thin wrappers from `account`)
- Create: `src/chain/client.rs` (~430 LoC: `RpcClient { url, host, http, rate_limiter, id_counter }` + URL allowlist (https-only + http://localhost/127.0.0.1) + custom `Debug` impl stripping URL query string + `RateLimiter` (token bucket, 50 req/s, burst 100) + `BlockhashCache` V0.1.5 stub + `post<T>()` JSON-RPC helper with typed `RpcResponse<T>` + `RpcErrorEnvelope`)
- Create: `src/chain/account.rs` (~560 LoC: 15 RPC method wrappers + V0.1 local minimal wire structs `TransactionStatus`, `ConfirmationStatus`, `UiTokenAmount`, `Version`, `RpcPrioritizationFee`, `SolanaSimulateResult`, `SolanaUnitsConsumedDetails`, `RpcKeyedAccount`, `AccountJson` — all `#[serde(rename_all = "camelCase")]` matching Anza wire format; `bincode::serialize` for `sendTransaction` pinned `=1.3.3`; base64 envelope via `base64::engine::general_purpose::STANDARD`)
- Create: `src/chain/preflight.rs` (~135 LoC: `check_native_balance` + `check_token_balance` + `check_ata_exists` + `resolve_mint_decimals` via `spl_token::state::Mint::unpack` + `check_rent_exempt`)
- Create: `src/tx/broadcast.rs` (~165 LoC: `send_and_confirm(rpc, tx, commitment, timeout) -> Result<Signature>` + `wait_for_confirm` with 200ms→2s exponential backoff + `ConfirmPending` at `timeout/2` + `ConfirmTimeout` at full `timeout` + `level_rank()` helper because Anza 4.x `CommitmentLevel` does not impl `PartialOrd`)
- Create: `src/tx/native.rs` (~55 LoC: `prepare_sol_transfer_message(from, to, lamports, cu_limit, cu_price, blockhash) -> Message` — Phase 3 builder orchestration via `Message::new_with_blockhash(&ixs, Some(payer), &blockhash)`)
- Create: `src/tx/spl.rs` (~85 LoC: `prepare_spl_transfer_message(wallet_pubkey, source_ata, dest_ata, mint, program, amount, decimals, cu_limit, cu_price, blockhash, prepend_ata_create) -> Message` — Phase 4 builder orchestration + TokenProgram dispatch + optional `prepend_create_ata`; Q10 `transfer_checked` invariant)
- Modify: `src/tx/mod.rs` (`pub mod broadcast; pub mod builder; pub mod native; pub mod spl;` + re-exports of `prepare_sol_transfer_message`, `prepare_spl_transfer_message`, `send_and_confirm`, `wait_for_confirm`, `DEFAULT_CONFIRM_TIMEOUT`, `DEFAULT_SEND_MAX_ATTEMPTS`)
- Modify: `src/lib.rs` (`pub mod chain;`)
- Modify: `src/error.rs` (add 8 variants: `Transport(String)`, `Rpc { code: i32, message: String }`, `InsufficientFunds { needed: have }`, `BroadcastFailed { kind, context }`, `ConfirmTimeout { signature, waited_ms }`, `ComputeBudgetExceeded { needed_cu, available_cu }`, `ConfirmPending { signature, commitment, elapsed_ms }`, `Unimplemented(&'static str)`)
- Modify: `rust-wallet-app/crates/sol-wallet-core/Cargo.toml` (`reqwest` from workspace + `url = "2"` + `base64 = "0.22"` + `phf = "0.11"` w/ `macros` + `ascii = "1"`; Anza ABI split: `solana-account = "=4.4.0"` + `solana-commitment-config = "=3.1.1"` + `solana-program-pack = "=3.1.0"` + `bincode = "=1.3.3"` + `tokio` from workspace; dev-deps: `wiremock = "0.6"` + `serial_test = "3"` for `#[serial(tokio)]` on parallel `#[tokio::test]` runs; NO Anza `solana-rpc-client` dep — Recipe 2 from #555)
- Create: `tests/chain_rpc.rs` (36 tests, ~1000 LoC: 6 URL allowlist + 1 JSON-RPC envelope + 1 bincode round-trip + 1 Debug + 2 rate limiter + 12 RPC method smokes + 3 preflight + 4 native/SPL/broadcast + 1 `wait_for_confirm` half-timeout; `#[serial(tokio)]` because wiremock 0.6 + parallel tokio runtime = flaky — alternative: set `RUST_TEST_THREADS=1`)
- Devnet integration tests (`tests/send_native.rs` + `tests/send_token.rs` with `RUN_SOL_DEVNET=1`) + per-method split files (`tests/balance.rs` + `tests/list_tokens.rs` + `tests/token_info.rs` + `tests/rent.rs` + `tests/priority_fee.rs` + `tests/info.rs` + `tests/transport_failure.rs`) deferred to V0.1 follow-up PR; Phase 5.1 ships the consolidated `tests/chain_rpc.rs` covering all 15 method smokes + preflight + broadcast in one file

**Wire-format invariants (Phantom-equivalent parity):**

- `sol send` matches `system_instruction::transfer` + Compute Budget 3-ix layout (`set_cu_limit` + `set_cu_price` + `transfer`); 150k CU + 0 priority fee defaults.
- `sol send-token` ALWAYS uses `transfer_checked` (NOT `transfer`) — Q10 invariant; decimals pulled from on-chain Mint state via `spl_token::state::Mint::unpack` (NEVER hardcoded).
- ATA derivation seed = `[owner, token_program_id, mint]` (Q6) — Token-2022 ATA ≠ classic SPL ATA for same `(owner, mint)`.
- `prepend_create_ata` (idempotent variant) lands BEFORE the transfer; respects `--create-ata` flag.
- Ed25519 signs the blockhash (Q7) — every send fetches a fresh blockhash via `getLatestBlockhash`.
- `sendTransaction` body: base64-encoded wire transaction (Anza `bincode::serialize` format); `RpcClient` does the encoding internally.
- `getTransaction` body: base64-encoded wire transaction in result; `RpcClient` decodes back to `Transaction` (or `EncodedTransaction` for V0.1 if bincode decode adds too much surface).

**17 HTTP RPC methods (the full Phase 7 binding surface) — split across 3 tasks:**

**Task 5.1 (15 methods — critical path):**

| # | Method | V0.1 surface | Used by |
|---|---|---|---|
| 1 | `getLatestBlockhash` | `RpcClient::get_latest_blockhash() -> Result<(Hash, u64)>` | send, send-token |
| 2 | `sendTransaction` | `RpcClient::send_transaction(tx: &Transaction) -> Result<Signature>` (base64 wire) | send, send-token |
| 3 | `getSignatureStatuses` | `RpcClient::get_signature_status(sig: &Signature) -> Result<Option<TransactionStatus>>` | wait, send (confirm), history, tx |
| 4 | `simulateTransaction` | `RpcClient::simulate_transaction(tx: &Transaction) -> Result<SimulateResult>` (preflight compute-budget) | send (dry-run), send-token (dry-run) |
| 5 | `getBalance` | `RpcClient::get_balance(pubkey: &Pubkey) -> Result<u64>` | balance |
| 6 | `getAccountInfo` | `RpcClient::get_account_info(pubkey: &Pubkey) -> Result<Option<Account>>` | mint decimals (send-token), token-info |
| 7 | `getMultipleAccounts` | `RpcClient::get_multiple_accounts(pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>>` | list-tokens (batched mint decimals) |
| 8 | `getTokenAccountBalance` | `RpcClient::get_token_account_balance(pubkey: &Pubkey) -> Result<UiTokenAmount>` | balance --token, send-token (preflight) |
| 9 | `getTokenAccountsByOwner` | `RpcClient::get_token_accounts_by_owner(owner: &Pubkey, program: Pubkey) -> Result<Vec<RpcKeyedAccount>>` | list-tokens |
| 10 | `getTokenSupply` | `RpcClient::get_token_supply(mint: &Pubkey) -> Result<UiTokenAmount>` | token-info |
| 11 | `getMinimumBalanceForRentExemption` | `RpcClient::get_minimum_balance_for_rent_exemption(data_len: usize) -> Result<u64>` | rent, send-token (auto-ATA preflight) |
| 12 | `getRecentPrioritizationFees` | `RpcClient::get_recent_prioritization_fees(addresses: &[Pubkey]) -> Result<Vec<RpcPrioritizationFee>>` | send --priority-fee auto |
| 13 | `getVersion` | `RpcClient::get_version() -> Result<Version>` | info |
| 14 | `getEpochInfo` | `RpcClient::get_epoch_info() -> Result<EpochInfo>` | info |
| 15 | `getHealth` | `RpcClient::get_health() -> Result<()>` | info (probe) |

**Task 5.2 (1 method — devnet helper):**

| # | Method | V0.1 surface | Used by |
|---|---|---|---|
| 16 | `requestAirdrop` | `RpcClient::request_airdrop(pubkey: &Pubkey, lamports: u64) -> Result<Signature>` (devnet only) | `sol request-airdrop` (Phase 7.7) + integration test fixtures |

**Task 5.3 (1 method — full tx log):**

| # | Method | V0.1 surface | Used by |
|---|---|---|---|
| 17 | `getTransaction` | `RpcClient::get_transaction(sig: &Signature, encoding) -> Result<Option<EncodedTransaction>>` (full log; base64 wire) | `sol tx <sig>` (Phase 7.6) |

**Phase 7 command → RPC binding table (the contract Phase 5 implements against):**

| Command | RPC methods used |
|---|---|
| `sol balance <addr>` | `getBalance` |
| `sol balance <addr> --token <mint>` | `getTokenAccountBalance` + `getAccountInfo(mint)` (decimals) |
| `sol info` | `getVersion` + `getEpochInfo` + `getHealth` |
| `sol keygen` | none (Phase 1) |
| `sol keygen --mnemonic` | none (Phase 1) |
| `sol import <file>` | none (Phase 1) |
| `sol export <addr>` | none (Phase 1) |
| `sol send <addr> <amount>` | `getLatestBlockhash` + `simulateTransaction` (if not `--no-simulate`) + `sendTransaction` + `getSignatureStatus` (confirm) |
| `sol send --priority-fee auto` | `getRecentPrioritizationFees` + above |
| `sol send-token <mint> <addr> <amount>` | `getAccountInfo(mint)` (decimals + program) + `getTokenAccountBalance` (preflight) + `getAccountInfo(dest_ata)` (existence) + `getLatestBlockhash` + `sendTransaction` + `getSignatureStatus` (confirm) |
| `sol send-token --create-ata` | above + `getMinimumBalanceForRentExemption(165)` (ATA size) |
| `sol history <addr>` | `getSignatureStatuses` (paginated) + `getMultipleAccounts` (resolve keys) |
| `sol tx <signature>` | **Phase 5.3** `getTransaction` (full log) + `getSignatureStatus` (confirmation state) |
| `sol list-tokens <owner>` | `getTokenAccountsByOwner` + `getMultipleAccounts` (batched mint decimals) |
| `sol token-info <mint>` | `getAccountInfo(mint)` + `getTokenSupply` |
| `sol rent <data-len>` | `getMinimumBalanceForRentExemption` |
| `sol watch <addr>` (V0.1.5) | `accountSubscribe` WS (deferred) |
| `sol wait <signature>` (V0.1.5) | `signatureSubscribe` WS (deferred) |
| `sol request-airdrop <addr> <lamports>` (devnet helper) | **Phase 5.2** `requestAirdrop` + `getLatestBlockhash` (confirm) |

**Error mapping (reqwest + JSON-RPC envelopes → `Error`):**

- reqwest connect/DNS/TLS error → `Error::Transport(String)`
- reqwest timeout (30s) → `Error::Transport(String)`
- HTTP 5xx → `Error::Transport(String)`
- JSON-RPC error envelope `{ "error": { "code": -32000, "message": "..." } }` → `Error::Rpc { code, message }`
- Pre-fund check shortfall → `Error::InsufficientFunds { needed, have }` (before broadcast)
- `simulateTransaction` returns units consumed > CU limit → `Error::ComputeBudgetExceeded { needed_cu, available_cu }` (before broadcast)
- `sendTransaction` rejected → `Error::BroadcastFailed { kind }` (from JSON-RPC error code classification)
- `getSignatureStatus` polls return None past timeout → `Error::ConfirmTimeout { signature, waited_ms }`

**V0.1 explicit non-goals (deferred to V0.1.5 / Phase 5.5 / V0.2):**

- BlockhashCache (TTL cache over `getLatestBlockhash`) — V0.1.5 (Q11 still listed in deep-dive §D, not on V0.1 critical path)
- Retry-on-stale-hash in `send_and_confirm` — V0.1.5 (stale hash rate <1% on devnet; user can re-run)
- Full SPKI pinning (full `pinned://<spki-hex>@host[:port]` URL scheme + `SpkiPinnedVerifier` re-export from `bitcoin-wallet-core::chain::spki`) — V0.1.5; **V0.1 ships the escape hatch** `RpcClient::new_with_pinned_spki(url, spki_hex)` for high-value wallets
- WS subscribes (`account_subscribe`, `signature_subscribe`, `program_subscribe`, `logs_subscribe`, `slot_subscribe`) — V0.1.5 watch mode
- Token-2022 transfer fee / transfer hook / confidential transfer extensions — V0.2

**V0.1 critical-path tasks (in order):** Task 5.1 (15 methods + hardening) → Task 5.2 (`requestAirdrop` + devnet allowlist) → Task 5.3 (`getTransaction` + `Error::ConfirmPending`) → Task 5.4 (rate limiter) → Task 5.5 (SPKI escape hatch). Each is a separate PR with its own verify gate.

**Task 5.1 acceptance criteria:**

1. `cargo check -p sol-wallet-core --lib --tests` exits 0 with NO Anza `solana-rpc-client` dep in the dep graph (verify via `cargo tree -p sol-wallet-core | grep solana-rpc-client` → empty).
2. `cargo test -p sol-wallet-core --lib --tests` shows 94 + ~58 new tests passing (20 rpc_methods_mock + 5 preflight + 6 send_native + 8 send_token + 4 balance + 6 list_tokens + 4 token_info + 3 rent + 4 priority_fee + 6 info + 4 transport_failure).
3. `tests/send_native.rs` + `tests/send_token.rs` integration tests pass on devnet with `RUN_SOL_DEVNET=1` + funded test wallet (transfer 0.001 SOL + transfer 1 USDC; assert signature returned + slot observed + `confirmed` commitment reached within 30s).
4. Phase 7 pre-implementation check: every `sol` CLI command listed in the binding table above (except `sol tx` → 5.3 and `sol request-airdrop` → 5.2) can be implemented against the V0.1 RPC surface without adding methods to `RpcClient`. Verified by Phase 6.2 library completeness gate.
5. Plan doc §Phase 5.1 checkboxes flipped to `[x]` in `[skip ci]` follow-up commit (L24).
6. PR opened to `rust-sol-core` from `sol/phase5.1-rpc-client`, squash-merged with admin bypass (L6).

**Steps:**

- [ ] Step 1: Add `chain::RpcClient` to `src/chain/rpc.rs` — signature change from prior design: `new(url: &str) -> Result<Self>` (returns `Error::Transport` on URL allowlist fail per Tier 1 finding #1); + 15 JSON-RPC methods (use `reqwest` POST to `{ "jsonrpc": "2.0", "id": monotonic_counter, "method": "...", "params": [...] }`); parse typed `RpcResponse<T>` / `RpcError { code, message }` structs with `#[derive(Deserialize)]` + `#[serde(deny_unknown_fields)]` (Tier 2 finding #7 — no `serde_json::Value` indexing); 30s timeout via `reqwest::Client::builder().timeout(Duration::from_secs(30))`; `requestAirdrop` and `getTransaction` added in Tasks 5.2 and 5.3 respectively; `sendTransaction` body uses `bincode::serialize` with `bincode::config::legacy()` (wire-compatible with Anza RPC servers) and base64-encodes the result; bincode version pinned to `"=1.3.3"` in workspace (already pinned per Phase 3 plan); `simulate_transaction` doc comment MUST note Tier 4 finding #4 (result is a hint, not a guarantee — cluster state may have changed between simulate and send); custom `Debug` impl on `RpcClient` prints `RpcClient { url: <scheme>://<host>[:port], http: Client }` (Tier 2 finding #11 — strip query string, never leak API keys)
- [ ] Step 2: Add URL allowlist in `RpcClient::new` (Tier 1 finding #1) — parse with `url::Url`; accept `https` for any host; accept `http` ONLY when host == `localhost` OR `127.0.0.1`; reject everything else (`ftp://`, `file://`, `http://attacker.com`, `http://192.168.x.x` from non-localhost address); on reject return `Error::Transport("RPC URL must be https://... or http://localhost[:port] (got scheme=... host=...)")`; do NOT include the full URL in the error message (only scheme + host); add 4 unit tests: `https://api.devnet.solana.com` → `Ok`; `http://localhost:8899` → `Ok`; `http://127.0.0.1:8899` → `Ok`; `http://attacker.com` → `Err(Transport)`; `ftp://...` → `Err(Transport)`
- [ ] Step 3: Add 8 new `Error` variants to `src/error.rs` — existing 7 from prior design (`Transport`, `Rpc`, `InsufficientFunds`, `BroadcastFailed { kind }`, `ConfirmTimeout { signature, waited_ms }`, `ComputeBudgetExceeded { needed_cu, available_cu }`, `Unimplemented`) + new `ConfirmPending { signature: String, elapsed_ms: u64 }` (Tier 3 finding #10 — distinct from `ConfirmTimeout` so the CLI can surface "tx pending — check explorer: <url>" for `--finalized` sends that don't lock up within 30s); preserve all 8 existing Phase 1-4 variants
- [ ] Step 4: Implement `src/chain/preflight.rs` — `check_native_balance(rpc, pubkey, needed_lamports, fee_lamports) -> Result<()>` (calls `getBalance`; if `balance < needed + fee` returns `Error::InsufficientFunds { needed: needed + fee, have: balance }`); `check_token_balance(rpc, ata, expected_mint) -> Result<u64>` (calls `getTokenAccountBalance`; verifies mint matches expected; returns base units); `check_ata_exists(rpc, ata) -> Result<bool>` (calls `getAccountInfo`; returns `Some(()) -> true`, `None -> false`); `resolve_mint_decimals(rpc, mint) -> Result<u8>` (calls `getAccountInfo(mint)`; unpacks `spl_token::state::Mint`; returns `mint.decimals`); `check_rent_exempt(rpc, data_len) -> Result<u64>` (calls `getMinimumBalanceForRentExemption`)
- [ ] Step 5: Implement `src/tx/send_native.rs` — `prepare_sol_transfer_message(from, to, lamports, cu_limit, cu_price, blockhash) -> Message` (calls `tx::builder::build_sol_transfer_with_budget`); pure function, no IO
- [ ] Step 6: Implement `src/tx/send_token.rs` — `prepare_spl_transfer_message(wallet_pubkey, source_ata, dest_ata, mint, program, amount, decimals, cu_limit, cu_price, blockhash, prepend_ata_create) -> Message` (calls `tx::builder::build_spl_transfer_checked` + optional `tx::builder::prepend_create_ata` + `tx::builder::compute_budget_instructions`); pure function, no IO
- [ ] Step 7: Implement `src/tx/broadcast.rs` — `send_and_confirm(rpc, signed_tx, timeout) -> Result<Signature>`: (a) `rpc.send_transaction(signed_tx)` → signature; (b) poll `rpc.get_signature_status(sig)` with 200ms→400ms→...→2s backoff (cap); (c) for `--finalized` commitment, the 30s default may be insufficient (~12 slots = ~5s normally, but cluster can stall); on first poll that returns `Some(status)` WITHOUT reaching the requested commitment after `timeout / 2` elapsed, return `Error::ConfirmPending { signature, elapsed_ms: (timeout / 2).as_millis() }` (NOT `ConfirmTimeout` — tx is still valid, just slow); only return `ConfirmTimeout` if the full `timeout` elapses; this lets Phase 7 CLI surface "tx pending — check <explorer_url>" instead of "failed"
- [ ] Step 8: Wire `pub mod chain;` in `src/lib.rs`; `pub mod broadcast; pub mod send_native; pub mod send_token;` in `src/tx/mod.rs`
- [ ] Step 9: Add `reqwest = { workspace = true }` + `wiremock = "0.6"` to `src/Cargo.toml` (`wiremock` in `[dev-dependencies]`); do NOT add `solana-rpc-client`; verify `cargo tree -p sol-wallet-core | grep solana-rpc-client` returns empty
- [ ] Step 10: Implement `tests/rpc_methods_mock.rs` (~20 tests: 15 method mocks + 4 URL allowlist tests + 1 bincode round-trip test per Tier 2 finding #8) — spin up mock HTTP server on ephemeral port (use `wiremock` crate); mock all 15 methods returning canned responses (blockhash with slot, signature base58, balance u64, account info with optional owner + data, etc.); assert `RpcClient` deserializes correctly; assert JSON-RPC error envelope `{ "error": {"code": -32000, "message": "..."}}` → `Error::Rpc { code: -32000, ... }`; assert connect-refused → `Error::Transport`; assert HTTP 5xx → `Error::Transport`; assert timeout (mock server hangs) → `Error::Transport` after 30s; assert bincode round-trip: serialize a known `Transaction` with `bincode::serialize(&tx, bincode::config::legacy())` → base64 → assert it matches the Anza test vector OR round-trips through `bincode::deserialize` to the original `Transaction`
- [ ] Step 11: Implement `tests/preflight.rs` (~5 tests) — mock `getBalance` returning fixed u64; `check_native_balance` returns `Ok(())` when funded, `Err(InsufficientFunds { needed, have })` when under-funded; `check_token_balance` returns base units from `getTokenAccountBalance`; `check_ata_exists` returns `false` when `getAccountInfo` returns `None`; `resolve_mint_decimals` returns `6` from mocked Mint state (Q10 — verify unpacking 82 bytes returns correct decimals byte)
- [ ] Step 12: Implement `tests/send_native.rs` (~6 tests) — unit: `prepare_sol_transfer_message` produces 3-ix message (cu_limit + cu_price + system_instruction::transfer), correct account_keys ordering, correct recent_blockhash; integration gated `RUN_SOL_DEVNET=1`: load keypair from `tests/fixtures/test_keypair.json`, `RpcClient::new("https://api.devnet.solana.com")`, `prepare_sol_transfer_message`, `tx.sign(...)`, `send_and_confirm(rpc, tx, 30s)`, assert `Ok(sig)` + sig base58 string + presence in `getSignatureStatuses` after 1s poll
- [ ] Step 13: Implement `tests/send_token.rs` (~8 tests) — unit: `prepare_spl_transfer_message` produces 3-ix (or 4-ix with `--create-ata`) message; classic vs Token-2022 dispatch per `TokenProgram` arg (Q6); decimals from `spl_token::state::Mint::unpack` of mocked account data (Q10 — verify changing decimals byte changes ix payload); integration gated `RUN_SOL_DEVNET=1`: resolve USDC mint on devnet (`4wU2tTRJJRx9K7xXYZqDq4WJ7vmnEHr9tM7NMGfX5x1b` or current devnet USDC), `check_ata_exists` for source + dest, `prepare_spl_transfer_message`, sign + send + confirm
- [ ] Step 14: Implement `tests/balance.rs` (~4 tests) — unit: `get_balance` parses JSON-RPC `{ "value": lamports }`; `get_token_account_balance` parses `{ "value": { "amount": "1000000", "decimals": 6, "uiAmount": 1.0 } }`; integration: devnet fetch of a known funded address
- [ ] Step 15: Implement `tests/list_tokens.rs` (~6 tests) — unit: `get_token_accounts_by_owner` parses `{ "value": [{ "pubkey": "...", "account": { "data": "...", "owner": "TokenkegQ..." } }] }`; `get_multiple_accounts` batched fetch; mint decimals unpacked from each account's owner field; integration: devnet fetch of a known wallet's token list
- [ ] Step 16: Implement `tests/token_info.rs` (~4 tests) — unit: `get_account_info` + Mint state unpack returns `decimals`; `get_token_supply` parses `{ "value": { "amount": "...", "decimals": 6, "uiAmount": 1000000.0 } }`; integration: devnet USDC info
- [ ] Step 17: Implement `tests/rent.rs` (~3 tests) — unit: `get_minimum_balance_for_rent_exemption(165)` returns u64 lamports (165 bytes = ATA size); integration: devnet fetch
- [ ] Step 18: Implement `tests/priority_fee.rs` (~4 tests) — unit: `get_recent_prioritization_fees` parses `{ "value": [{ "slot": 123, "prioritizationFee": 5000 }] }`; auto-estimation logic (median of last N observations) returns u64 microlamports; integration: devnet fetch
- [ ] Step 19: Implement `tests/info.rs` (~6 tests) — unit: `get_version` parses `{ "solana-core": "1.18.x", "feature-set": 12345 }`; `get_epoch_info` parses slot + epoch + block height; `get_health` returns `Ok(())` on `Ok`-health, `Err(Rpc { code, ... })` on unhealthy; integration: devnet probe
- [ ] Step 20: Implement `tests/transport_failure.rs` (~4 tests) — RPC pointed at closed port (127.0.0.1:1) → `Error::Transport` within 5s; mock server returns 500 → `Error::Transport`; mock server returns malformed JSON → `Error::Rpc` (or `Transport`); mock server hangs > 30s → `Error::Transport` (timeout)
- [ ] Step 21: Verify gate: `cargo fmt --all && cargo clippy -p sol-wallet-core --lib --tests -- -D warnings && cargo test -p sol-wallet-core --lib --tests` (all green; assert 94 + 70 = **164 tests pass**; 20 rpc_methods_mock + 5 preflight + 6 send_native + 8 send_token + 4 balance + 6 list_tokens + 4 token_info + 3 rent + 4 priority_fee + 6 info + 4 transport_failure)
- [ ] Step 22: Phase 6.2 library completeness gate (run BEFORE Phase 7 starts): confirm every `sol` CLI command listed in the binding table above can be implemented against the V0.1 RPC surface; flag any new RPC method needed (would require Phase 5.5 PR)
- [ ] Step 23: Add `wallet-desktop` integration smoke (out of V0.1 scope but verify the Phase 7 CLI import path works): import `sol_wallet_core::{chain::RpcClient, chain::preflight, tx::{send_native, send_token, broadcast}}` from a small `examples/send_sol_devnet.rs` binary; run against devnet with a funded keypair; print signature
- [ ] Step 24: Add plaintext keypair warning doc comment (Tier 2 finding #3) — on `chain::rpc` module doc + `chain::mod` re-export; text: `// SECURITY: V0.1 reads the wallet keypair from a raw 64-byte file at $HOME/.config/sol-wallet/wallet.json (mode 0600). This file contains unencrypted private key bytes — DO NOT sync to cloud storage (iCloud, Dropbox, Google Drive), DO NOT commit to git, DO NOT share the file with any process you do not trust. Phase 6 replaces this with Argon2id-encrypted BIP-39 mnemonic storage; until then, treat the wallet file like a password.`
- [ ] Step 25: PAUSE — commit-push-pr (L6 same-scope bundle)

### Task 5.2 (TBD): `RpcClient::request_airdrop` (devnet helper)

**Depends on:** Task 5.1 merged (the `RpcClient` + `send_and_confirm` infrastructure + `reqwest` JSON-RPC envelope parser).

**Files:**

- Modify: `src/chain/rpc.rs` (add `request_airdrop(pubkey: &Pubkey, lamports: u64) -> Result<Signature>` method, ~30 LoC; uses existing JSON-RPC envelope + base64 sig parser from 5.1; **devnet host allowlist per Tier 3 finding #6 — parses the `RpcClient`'s `url` host and rejects `api.mainnet-beta.solana.com` + any host not in the devnet allowlist**; allowlist: `api.devnet.solana.com`, `api.testnet.solana.com`, `localhost`, `127.0.0.1`)
- Modify: `tests/rpc_methods_mock.rs` (add 2 tests: `requestAirdrop` returns base58 sig on success; JSON-RPC error envelope `code: -32003` (a known devnet "airdrop limit" code) → `Error::Rpc { code: -32003, message }`)
- Modify: `tests/rpc_methods_mock.rs` (add 1 unit test for the devnet allowlist: `RpcClient::new("https://api.mainnet-beta.solana.com").unwrap().request_airdrop(pubkey, 1_000)` → `Err(Transport("requestAirdrop only valid on devnet / testnet / local validator; current RPC: api.mainnet-beta.solana.com"))`)
- Create: `tests/airdrop.rs` (~3 tests: integration gated `RUN_SOL_DEVNET=1`; `RpcClient::new("https://api.devnet.solana.com")`; `request_airdrop(pubkey, 1_000_000_000)` → `Ok(sig)`; verify with `get_signature_status` after 5s)

**Task 5.2 acceptance criteria:**

1. `cargo test -p sol-wallet-core --test rpc_methods_mock` adds 3 tests (total ~23 in that file: 2 airdrop mock + 1 mainnet-rejection)
2. `cargo test -p sol-wallet-core --test airdrop` shows 3 tests (gated `RUN_SOL_DEVNET=1`); 94 + 70 + 3 = **167 tests pass** when 5.2 lands
3. `sol request-airdrop` (Phase 7.7) binds to `RpcClient::request_airdrop` + `send_and_confirm` without further `RpcClient` additions
4. Plan doc §Phase 5.2 checkboxes flipped to `[x]` in `[skip ci]` follow-up commit (L24)
5. PR opened to `rust-sol-core` from `sol/phase5.2-airdrop`, squash-merged with admin bypass (L6)

**Steps:**

- [ ] Step 1: Add `request_airdrop` to `src/chain/rpc.rs` — POST `{ "jsonrpc": "2.0", "id": 1, "method": "requestAirdrop", "params": [<base58-pubkey>, <lamports>] }`; parse `result` as base58 signature string; **devnet host allowlist check FIRST (before POST)**: reject with `Error::Transport("requestAirdrop only valid on devnet / testnet / local validator; current RPC: <host>")` if `self.url` host is not in the allowlist above
- [ ] Step 2: Add 3 unit tests to `tests/rpc_methods_mock.rs` (success + airdrop-limit error envelope + mainnet-rejection allowlist)
- [ ] Step 3: Create `tests/airdrop.rs` with 3 integration tests (gated `RUN_SOL_DEVNET=1`; assert sig returned; assert confirmed within 30s via `send_and_confirm`; assert `request_airdrop` with `--rpc-url https://api.mainnet-beta.solana.com` fails fast with the allowlist error)
- [ ] Step 4: Verify gate: `RUN_SOL_DEVNET=1 cargo test -p sol-wallet-core --lib --tests --test airdrop`
- [ ] Step 5: PAUSE — commit-push-pr (L6 same-scope bundle)

### Task 5.3 (TBD): `RpcClient::get_transaction` (full tx log for `sol tx`)

**Depends on:** Task 5.1 merged.

**Files:**

- Modify: `src/chain/rpc.rs` (add `get_transaction(sig: &Signature, encoding: TransactionEncoding) -> Result<Option<EncodedTransaction>>` method, ~80 LoC; uses existing JSON-RPC envelope; `encoding` is `TransactionEncoding::Json` or `TransactionEncoding::Binary`; for V0.1 we return `Option<EncodedTransaction>` from the Anza `solana_transaction_status_client_types` crate — Phase 7 decodes to `Transaction` as needed)
- Modify: `tests/rpc_methods_mock.rs` (add 2 tests: `getTransaction` returns encoded tx with base64 wire; `getTransaction` with unknown sig → `Ok(None)` (not an error per JSON-RPC semantics))
- Create: `tests/tx_log.rs` (~4 tests: integration gated `RUN_SOL_DEVNET=1`; send 0.001 SOL via `send_and_confirm`; then `get_transaction(sig, TransactionEncoding::Json)` returns `Some(_)` with `slot` + `transaction.message.recent_blockhash` matching; decode `transaction.message.instructions[0]` = system transfer ix; `get_transaction` with bogus sig → `Ok(None)`)

**Task 5.3 acceptance criteria:**

1. `cargo test -p sol-wallet-core --test rpc_methods_mock` adds 2 tests (total ~25 in that file)
2. `cargo test -p sol-wallet-core --test tx_log` shows 4 tests; 94 + 70 + 3 + 4 = **171 tests pass** when 5.3 lands (Phase 5 critical path done)
3. `sol tx <sig>` (Phase 7.6) binds to `RpcClient::get_transaction` + `get_signature_status` without further `RpcClient` additions
4. Phase 6.2 library completeness gate can now pass for all 22 Phase 7 commands
5. Plan doc §Phase 5.3 checkboxes flipped to `[x]` in `[skip ci]` follow-up commit (L24)
6. PR opened to `rust-sol-core` from `sol/phase5.3-tx-log`, squash-merged with admin bypass (L6)

**Steps:**

- [ ] Step 1: Add `get_transaction` to `src/chain/rpc.rs` — POST `{ "jsonrpc": "2.0", "id": 1, "method": "getTransaction", "params": [<base58-sig>, {"encoding": "json", "maxSupportedTransactionVersion": 0}] }`; parse `result` (an object with `slot: u64`, `blockTime: Option<i64>`, `transaction: EncodedTransaction` with `message: UiMessage` + `signatures: Vec<String>`) or `null` for unknown sig
- [ ] Step 2: Add 2 unit tests to `tests/rpc_methods_mock.rs` (success + unknown sig → `Ok(None)`)
- [ ] Step 3: Create `tests/tx_log.rs` with 4 integration tests (gated `RUN_SOL_DEVNET=1`; send 0.001 SOL; fetch back; decode message.instructions; assert recent_blockhash matches; bogus sig → `Ok(None)`)
- [ ] Step 4: Verify gate: `RUN_SOL_DEVNET=1 cargo test -p sol-wallet-core --lib --tests --test tx_log`
- [ ] Step 5: PAUSE — commit-push-pr (L6 same-scope bundle)

### Task 5.4 (TBD): Per-`RpcClient` rate limiter (token bucket, default 50 req/s burst 100)

**Depends on:** Task 5.1 merged (the `RpcClient` + `send_and_confirm` infrastructure).

**Why this exists (Tier 3 finding #5):** without a rate limiter, a malicious CLI script or buggy Phase 7 handler could fire 1000s of `sendTransaction` / `getSignatureStatuses` per second, bypassing Solana cluster-level rate limits (which count requests, not operations) and enabling duplicate-send footguns (signing a tx, hitting `sendTransaction` twice before the first confirms → potential replay race). `solana-test-validator` and surfpool also rate-limit at lower thresholds; client-side throttling prevents spurious 429s from cascading the test suite.

**Files:**

- Modify: `src/chain/rpc.rs` (add `RateLimiter` struct + `acquire()` non-blocking permit; store in `RpcClient`; default `50 req/s, burst 100`; configurable via `RpcClient::with_rate_limit(req_per_sec: u32, burst: u32)`)
- Modify: `src/chain/rpc.rs` (wrap every JSON-RPC POST in `rate_limiter.acquire().await`; on permit-deny, return `Error::Transport("rate limit exceeded: <url-host>")` after a 1s backoff retry; if still denied, surface `Error::Transport` and log a warning)
- Create: `src/chain/rate_limit.rs` (~80 LoC: hand-rolled token bucket; refill rate `req_per_sec`; max `burst`; no `governor` crate dep to keep workspace lean; uses `tokio::time::Instant` for monotonic clock; thread-safe via `Arc<Mutex<Inner>>`)
- Modify: `tests/rpc_methods_mock.rs` (add 3 tests: `RpcClient::with_rate_limit(10, 5)` then fire 7 requests immediately; first 5 succeed within 100ms; next 2 return `Error::Transport("rate limit exceeded: ...") within 1s; after 1s wait, the bucket refills and 1 more succeeds)
- Create: `tests/rate_limit.rs` (~3 tests: unit-only — fire 200 requests against mock at rate 50 req/s; assert all complete within 4-5s; assert no spurious 429 from mock; assert `with_rate_limit(1, 1)` + immediate second request → `Error::Transport`)

**Task 5.4 acceptance criteria:**

1. `cargo test -p sol-wallet-core --test rpc_methods_mock` adds 3 tests (total ~28)
2. `cargo test -p sol-wallet-core --test rate_limit` shows 3 tests; 94 + 70 + 3 + 4 + 3 = **174 tests pass** when 5.4 lands
3. `sol send` rate-limited to 1 request per ~20ms (matches cluster-level 50 req/s ceiling with headroom); no `sendTransaction` race when user spams Enter
4. `with_rate_limit(0, 0)` is a special "disabled" sentinel — useful for tests that don't want rate limiting
5. Plan doc §Phase 5.4 checkboxes flipped to `[x]` in `[skip ci]` follow-up commit (L24)
6. PR opened to `rust-sol-core` from `sol/phase5.4-rate-limit`, squash-merged with admin bypass (L6)

**Steps:**

- [ ] Step 1: Add `src/chain/rate_limit.rs` — `pub struct RateLimiter { capacity: u32, refill_per_sec: u32, tokens: Arc<Mutex<f64>>, last_refill: Arc<Mutex<Instant>> }`; `pub async fn acquire(&self) -> Result<(), Error>`; refill on each `acquire` call: `tokens = min(capacity, tokens + elapsed * refill_per_sec)`; if `tokens >= 1.0`, deduct and return `Ok(())`; if `tokens < 1.0`, sleep `Duration::from_secs_f64((1.0 - tokens) / refill_per_sec)`, retry once; if still 0, return `Err(Transport("rate limit exceeded"))`
- [ ] Step 2: Add `RateLimiter` field to `RpcClient` (default `RateLimiter::new(50, 100)`); add `with_rate_limit(self, req_per_sec: u32, burst: u32) -> Self` builder method; add `set_rate_limit(&mut self, limiter: RateLimiter)` for tests
- [ ] Step 3: Wrap every JSON-RPC POST helper in `self.rate_limiter.acquire().await?` (one helper function `pub(super) async fn post<T>(&self, method: &str, params: Value) -> Result<T>` that all 17 RPC methods call into)
- [ ] Step 4: Add 3 unit tests to `tests/rpc_methods_mock.rs` (burst exhaust, refill, 0/0 disabled)
- [ ] Step 5: Create `tests/rate_limit.rs` with 3 unit tests (200 req at 50/s completes in 4-5s; `with_rate_limit(1, 1)` + second request fails; `with_rate_limit(0, 0)` passes through)
- [ ] Step 6: Verify gate: `cargo test -p sol-wallet-core --lib --tests --test rate_limit`
- [ ] Step 7: PAUSE — commit-push-pr (L6 same-scope bundle)

### Task 5.5 (TBD): `RpcClient::new_with_pinned_spki` (V0.1 SPKI escape hatch)

**Depends on:** Task 5.1 merged.

**Why this exists (Tier 3 finding #2):** V0.1 ships without SPKI pinning (Anza's `solana-rpc-client` dropped per #555 → custom `reqwest` transport → no built-in SPKI verifier). Without pinning, a compromised CA or hostile LAN DNS can MITM all RPC traffic, including the moment a `sendTransaction` body leaves the wallet — the attacker can rebroadcast with a higher priority fee (front-run) or capture the signature for replay. For high-value wallets, this is unacceptable. The full `pinned://<spki-hex>@host[:port]` URL scheme ships in V0.1.5. For V0.1, we ship a single-URL constructor that takes the SPKI SHA-256 hex explicitly.

**Files:**

- Modify: `src/chain/rpc.rs` (add `pub fn new_with_pinned_spki(url: &str, expected_spki_sha256_hex: &str) -> Result<Self>`; wraps `RpcClient::new`; on every TLS handshake, the custom `rustls::ClientConfig` checks the leaf cert's `SubjectPublicKeyInfo` SHA-256 against the expected value; mismatch → abort connection with `Error::Transport("SPKI pin mismatch: got <sha256>, expected <sha256>")`)
- Modify: `src/chain/rpc.rs` (add `pub fn spki_pin() -> Option<&[u8]>` accessor on `RpcClient` for the CLI to display the active pin in `sol info`)
- Create: `tests/spki_pin.rs` (~5 tests: unit — load 2 self-signed test certs with different SPKIs from `tests/fixtures/spki_a.der` + `spki_b.der`; assert `new_with_pinned_spki` accepts matching SPKI; rejects mismatched SPKI; rejects when expected SPKI is `Vec::new()`; integration `RUN_SOL_DEVNET=1` — `new_with_pinned_spki("https://api.devnet.solana.com", <api-devnet-spki-hex-from-digicert>)` succeeds; on a wrong pin, the request fails with `Error::Transport` within 5s)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/fixtures/spki_a.der` + `spki_b.der` (test fixtures — generate via `openssl req -x509 -newkey ed25519 -nodes -keyout ...` in a setup script; commit to git as test fixtures; 2 KB each)
- Modify: `rust-wallet-app/crates/sol-wallet-core/CHANGELOG.md` (Phase 5.5 entry: "Added `RpcClient::new_with_pinned_spki` constructor for high-value wallets. V0.1.5 will add the `pinned://<hex>@host[:port]` URL scheme. Until V0.1.5, users must extract the SPKI hex from their RPC provider's leaf cert (use `openssl x509 -in cert.pem -pubkey -noout | openssl pkey -pubin -outform DER | sha256sum`) and pass it via `--rpc-spki-pin <hex>` CLI flag")

**Task 5.5 acceptance criteria:**

1. `cargo test -p sol-wallet-core --test spki_pin` shows 5 tests (4 unit + 1 devnet integration); 94 + 70 + 3 + 4 + 3 + 5 = **179 tests pass** when 5.5 lands (full Phase 5 done: 5.1 critical path + 5.2 devnet + 5.3 tx log + 5.4 rate limit + 5.5 SPKI escape hatch)
2. Phase 7 CLI `--rpc-spki-pin <hex>` flag wires to `new_with_pinned_spki` when present
3. `sol info` displays the active pin (or "none — using default rustls system roots" warning) so users know whether their connection is MITM-able
4. Plan doc §Phase 5.5 checkboxes flipped to `[x]` in `[skip ci]` follow-up commit (L24)
5. PR opened to `rust-sol-core` from `sol/phase5.5-spki-pin`, squash-merged with admin bypass (L6)

**Steps:**

- [ ] Step 1: Add `new_with_pinned_spki` to `src/chain/rpc.rs` — builds `rustls::ClientConfig` with a custom `ServerCertVerifier` that calls `x509_parser::parse_x509_certificate(cert_der)`, extracts the SPKI DER, computes `sha256(spki_der)`, compares to expected; on mismatch, returns `rustls::Error::General("SPKI pin mismatch")`; URL allowlist (same as `new`) applied first
- [ ] Step 2: Add `spki_pin()` accessor on `RpcClient` returning `Option<&[u8]>` (the pinned hash, or `None` if default verifier is in use)
- [ ] Step 3: Generate 2 test cert fixtures via `openssl req -x509 -newkey ed25519 -nodes` — commit `tests/fixtures/spki_a.der` + `spki_b.der` to git (test fixtures, not secrets)
- [ ] Step 4: Add 5 tests to `tests/spki_pin.rs` (matching pin ok; mismatched pin rejected; empty pin rejected; integration against devnet; doc-comment test that `new` does NOT pin by default)
- [ ] Step 5: Verify gate: `RUN_SOL_DEVNET=1 cargo test -p sol-wallet-core --lib --tests --test spki_pin`
- [ ] Step 6: PAUSE — commit-push-pr (L6 same-scope bundle)

---

## Phase 6 — Wallet persistence (Argon2id + AES-GCM) + WalletManager

### Task 6.1 (TBD): persist.rs + WalletManager CRUD + encrypted wallet persistence

**Files:**
- Create: `src/persist.rs` (atomic_write + JSON metadata)
- Create: `src/crypto.rs` (Argon2id + AES-GCM wrappers)
- Create: `src/wallet_manager.rs` (CRUD over encrypted blobs)
- Create: `src/platform/mod.rs` + `src/platform/{storage,info,network,clock}.rs` (4 PAL traits × 14 methods per deep-dive §L line 1848)
- Modify: `src/lib.rs`
- Create: `tests/argon2_kdf.rs` (Phase 6.1 owns row 5 — Argon2id determinism + m=64MB t=3 p=1 params)
- Create: `tests/aes_gcm_cipher.rs` (Phase 6.1 owns row 6 — round-trip + single-bit flip → `Error::DecryptFailed`)
- Create: `tests/mnemonic_encrypt.rs` (Phase 6.1 owns row 7 — correct + wrong passphrase; no plaintext leak)
- Create: `tests/wallet_persist.rs` (Phase 6.1 owns rows 8+9+10 — create→save→load→sign round-trip; mode 0600; no `.tmp` residue; 1000 wallets UUID uniqueness)
- Create: `tests/wallet_lifecycle.rs` (deep-dive rows 36+37+38 — `import_from_pk` + `summary` + `list/delete/rename` lifecycle)
- Create: `tests/common/keypair_fixture.rs` (Phase 6.1 owns `throwaway_keypair() -> Keypair`)
- Create: `rust-wallet-app/crates/sol-wallet-core/CHANGELOG.md` (first entry covers Phase 6 per L24)

**Steps:**
- [ ] Step 1: Implement `crypto::encrypt_wallet(plaintext: &[u8], password: &str) -> Result<EncryptedBlob>` — Argon2id (memory 64MB, iterations 3, parallelism 1, salt 16 bytes `OsRng`) + AES-256-GCM (nonce 12 bytes `OsRng`); returns `nonce ‖ ciphertext ‖ tag` + JSON metadata (`kdf {algorithm, memory_kb, iterations, parallelism, salt}` + `cipher {algorithm, nonce}` + `encrypted_payload`) per deep-dive `### H. Wallet file encryption` line 1724
- [ ] Step 2: Implement `crypto::decrypt_wallet(blob: &EncryptedBlob, password: &str) -> Result<Vec<u8>>`; failure = `WalletDecryptFailed { id: WalletId }` (exit 5)
- [ ] Step 3: Implement `persist::atomic_write(path: &Path, bytes: &[u8]) -> Result<()>` — write `.tmp` + `fsync` + `rename` (no corruption on panic); per deep-dive row 9 acceptance
- [ ] Step 4: Implement `WalletManager` (in-memory `RwLock<HashMap<WalletId, EncryptedBlob>>`) + `create_with_mnemonic(words, password)` + `import_from_phrase(phrase, password)` + `import_from_pk_file(path, password)` + `unlock(id, password)` + `lock(id)` + `summary(id)` + `list()` + `delete(id)` + `rename(id, name)`
- [ ] Step 5: Implement `WalletStorage` PAL trait (`put_atomic`, `get`, `delete`, `list_ids`) + `InMemoryStorage` (test) + `FileWalletStorage` (desktop, mode 0600)
- [ ] Step 6: Read deep-dive §H (line 1754) + §L (line 1892) for full encrypted JSON schema + 4 PAL trait method signatures before encoding tests
- [ ] Step 7: Implement `tests/argon2_kdf.rs` (row 5) — Argon2id determinism (same password + salt = same key); fixed params m=64MB t=3 p=1; reject wrong params → distinct key
- [ ] Step 8: Implement `tests/aes_gcm_cipher.rs` (row 6) — AES-GCM round-trip; single-bit flip in ciphertext → `Error::DecryptFailed`; single-bit flip in nonce → `Error::DecryptFailed`; tag tampering → `Error::DecryptFailed`
- [ ] Step 9: Implement `tests/mnemonic_encrypt.rs` (row 7) — encrypt + decrypt with correct passphrase = original mnemonic; wrong passphrase → `Error::WalletDecryptFailed`; assert encrypted blob does NOT contain plaintext mnemonic substring
- [ ] Step 10: Implement `tests/wallet_persist.rs` (rows 8+9+10) — create → save → load → sign round-trip via `tempfile::TempDir` + `FileWalletStorage`; saved file mode == 0o600; no `.tmp` residue after successful write; 1000 wallets UUID uniqueness (no collision); name lookup resolves
- [ ] Step 11: Implement `tests/wallet_lifecycle.rs` (rows 36+37+38) — `import_from_pk_file` + `summary` returns pubkey + name; `list()` includes imported wallet; `delete(id)` removes; `rename(id, new_name)` updates
- [ ] Step 12: Implement `tests/common/keypair_fixture.rs` — `throwaway_keypair() -> Keypair` (OsRng, never logged, dropped after test); used by Phase 6.1 test files + Phase 7.2 e2e
- [ ] Step 13: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test argon2_kdf --test aes_gcm_cipher --test mnemonic_encrypt --test wallet_persist --test wallet_lifecycle`
- [ ] Step 14: PAUSE — commit-push-pr

---

## Phase 6.2 — Library completeness verification (all 33 in-scope deep-dive rows PASS)

**Goal:** confirm every `## Test scenario — sol-wallet-core V0.1` row from `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` (lines 3616-3710) is implemented and GREEN before Phase 7 starts CLI work. CLI depends on library being complete; a missing row blocks the CLI gates downstream.

**Scope:** 33 of 34 in-scope rows. Row 21 (SPKI pin match/mismatch) is **explicitly deferred to V0.1.5** per Q9 Scenario B default + `## V0.1.5 work` section. This phase asserts row 21 is on the V0.1.5 backlog, NOT on the V0.1 release gate.

**Marking discipline (L13 step 14 + `never-auto-commit` rule):**
- Each `- [ ] Step N:` stays unchecked until that step has been RUN locally by the operator.
- The nested `- [ ] Verified:` sub-checkbox flips to `[x]` only after: (a) step ran with exit 0, (b) operator recorded commit SHA + date in the `Verified by:` line, (c) PR merged into `rust-sol-core` per L13 step 15.
- **Do not auto-mark** boxes — operator flips them by hand per the workflow-approval-required rule.
- Loud-RED gate: any parent step box unchecked blocks Phase 7 start.

### Row-to-phase ownership matrix (source of truth — must match `## Test File Structure`)

| Row | Module | Phase | Test file | Status |
| --- | ------ | ----- | --------- | ------ |
| 1  | `keys.rs` BIP-39 + SLIP-0010 | Phase 1.1 | `tests/bip39_mnemonic.rs` + `tests/address_derivation.rs` | ✅ |
| 2  | `keys.rs` xprv → base58 | Phase 1.1 + Phase 2 | `tests/address_derivation.rs` (Phase 2 Modify) | ✅ |
| 3  | `amount.rs` from_str_in | Phase 3.1 | `tests/amount_lamport.rs` | ✅ |
| 4  | `amount.rs` round-trip proptest | Phase 3.1 | `tests/amount_lamport.rs` | ✅ |
| 5  | `crypto/argon2.rs` KDF determinism | Phase 6.1 | `tests/argon2_kdf.rs` | ✅ |
| 6  | `crypto/aes_gcm.rs` round-trip + tamper | Phase 6.1 | `tests/aes_gcm_cipher.rs` | ✅ |
| 7  | `crypto/mnemonic_cipher.rs` encrypt-at-rest | Phase 6.1 | `tests/mnemonic_encrypt.rs` | ✅ |
| 8  | `wallet/persist.rs` create→save→load→sign | Phase 6.1 | `tests/wallet_persist.rs` | ✅ |
| 9  | `wallet/persist.rs` mode 0600 + atomic write | Phase 6.1 | `tests/wallet_persist.rs` | ✅ |
| 10 | `wallet/id.rs` UUID + name lookup | Phase 6.1 | `tests/wallet_persist.rs` + `tests/wallet_lifecycle.rs` | ✅ |
| 11 | `config.rs` SolanaConfig TOML | Phase 6.1 | `tests/solana_config.rs` | ✅ |
| 12 | `config.rs` blockhash cache TTL | Phase 5.1 | `tests/blockhash_cache.rs` | ✅ |
| 13 | `spl/disambig.rs` Token-2022 vs classic | Phase 4.1 | `tests/token2022_disambig.rs` | ✅ |
| 14 | `spl/decimals.rs` Mint::unpack | Phase 4.1 | `tests/stablecoin_registry.rs` + `tests/token2022_disambig.rs` | ✅ |
| 15 | `tx/builder.rs` SOL + SPL round-trip | Phase 3.1 + Phase 4.1 | `tests/tx_serde.rs` + `tests/spl_instruction.rs` | ✅ |
| 16 | `tx/sign.rs` sign + recent_blockhash | Phase 1.2 | `tests/sign_tx.rs` | ✅ |
| 17 | `tx/sign.rs` sign_only_tx cold path | Phase 1.2 | `tests/sign_only.rs` | ✅ |
| 18 | `tx/ata.rs` auto-derive + auto-create | Phase 4.1 | `tests/spl_instruction.rs` | ✅ |
| 19 | `tx/preflight.rs` balance check | Phase 5.1 | `tests/preflight_balance.rs` | ✅ |
| 20 | `tx/receipt.rs` TransactionStatus JSON | Phase 5.1 | `tests/tx_status_parse.rs` | ✅ |
| 21 | `chain/spki.rs` SPKI pin match/mismatch | Phase 5.5 (V0.1.5) | `tests/spki_pin.rs` + `fixtures/spki_pin_test_cert.der` | ⚠️ V0.1.5 DEFERRED |
| 22 | `chain/solana_client.rs` 12 RPC methods | Phase 5.1 | `tests/rpc_methods_mock.rs` | ✅ |
| 23 | `error.rs` 21 From + Debug redaction | Phase 5/6 | `tests/error_mapping.rs` | ✅ |
| 24 | `ffi/panic.rs` panic scrubber | Phase 8.1 | `tests/placeholder.rs` | ✅ |
| 25 | `ffi/` C ABI smoke (12 exports) | Phase 8.1 | `tests/placeholder.rs` | ✅ |
| 26 | `tx/` submit_sol E2E | Phase 7.2 | `tests/submit_sol_local.rs` | ✅ |
| 27 | `tx/` submit_spl_transfer held | Phase 7.2 | `tests/submit_spl_local_held.rs` | ✅ |
| 28 | `tx/` submit_spl_transfer fresh | Phase 7.2 | `tests/submit_spl_local_fresh.rs` | ✅ |
| 29 | `tx/` submit_spl_approve + allowance | Phase 7.2 | `tests/submit_spl_local_approve.rs` | ✅ |
| 30 | `tx/broadcast.rs` send_with_retry stale | Phase 5.5 (V0.1.5) | `tests/send_with_retry.rs` (V0.1.5) | ⏸️ V0.1.5 |
| 31 | `tx/wait.rs` wait_for_confirm + timeout | Phase 5.5 (V0.1.5) | `tests/send_with_retry.rs` (V0.1.5) | ⏸️ V0.1.5 |
| 32 | `chain/solana_client.rs` get_health boot probe | Phase 7.2 | `tests/boot_probe_local.rs` | ✅ |
| 33 | SPL USDC mainnet $0.001 self-send | Phase 9.1 | `tests/mainnet_smoke.rs` + `crates/sol/tests/cli_mainnet_smoke.rs` | ✅ |
| 34 | transport failure: closed port | Phase 5.1 | `tests/transport_failure.rs` | ✅ |

### Entry-point coverage cross-check (16 functions per deep-dive Entry-point coverage table)

`submit_sol` (rows 16, 26, 34) · `submit_spl_transfer` (rows 27, 28) · `submit_spl_approve` (row 29) · `submit_send_speedup` (row 30) · `sign_only_tx` (row 17) · `wait_for_confirm` (row 31) · `derive_keypair` SLIP-0010 (row 1) · `WalletManager::create_with_mnemonic` (rows 8, 9) · `WalletManager::import_from_phrase` (row 8) · `WalletManager::import_from_pk` (row 8) · `WalletManager::unlock` (row 8) · `WalletManager::summary` (row 8) · `WalletManager::list/delete/rename` (row 10) · `WalletManager::pubkey` (row 2) · `chain::get_balance` (rows 26, 34) · `chain::spl_balance/spl_allowance` (rows 27, 28, 29) · `chain::mint_token_program` disambig (rows 13, 27, 28). FFI `sol_wallet_*` 12 fns (row 25).

### Task 6.2.1 (TBD): Library test compile + run gate

**Files:** none (verification only — no source or test creates)

**Steps:**
- [ ] Step 1: Run `cargo build -p sol-wallet-core --tests` — confirm all 32 named library test files compile (per `## Test File Structure` layout). Loud-RED if any compile error.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after step exits 0 + PR merges into `rust-sol-core`)
- [ ] Step 2: Run `cargo test -p sol-wallet-core --lib` — confirm all unit tests pass. Loud-RED if any test fails.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after step exits 0)
- [ ] Step 3: Run `cargo test -p sol-wallet-core --tests` (excludes `--lib` integration; includes `--test <name>` for each of the 32 files). Loud-RED if any test fails.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after step exits 0)
- [ ] Step 4: Cross-check fixture inventory per deep-dive `### Fixture inventory` table — confirm all 7 fixtures (BIP-39 vectors, known keypair, mint accounts, RPC JSON captures, leaf cert (V0.1.5 deferred), `tokens/mainnet.json`, `mock_spl_usdc`) are present in `tests/fixtures/` or referenced from test source. Row 21 leaf cert = V0.1.5 deferred.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after manual fixture inventory check)
- [ ] Step 5: Confirm row 21 SPKI pin is on the V0.1.5 backlog (`## V0.1.5 work` section lists it). Loud-RED if row 21 is silently dropped from V0.1 without being explicitly deferred.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after backlog cross-check)
- [ ] Step 6: Coverage gates per deep-dive `### Coverage gates` — confirm 100% line coverage on `crypto/`, `amount.rs`, `tx/builder.rs`, `spl/disambig.rs` via `cargo tarpaulin -p sol-wallet-core --lib`. Loud-RED if any of these modules drops below 100%.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `tarpaulin` exits 0 with 100% on the 4 named modules)
- [ ] Step 7: Redaction gate per rows 23+24 — confirm `tests/error_mapping.rs` asserts no mnemonic/seed/secret bytes in `Debug` output; `tests/placeholder.rs` (Phase 8.1) asserts panic scrubber regex strips all secret patterns. Pre-Phase 7 cannot fully execute row 24 (Phase 8.1 stub) — defer row 24 to Phase 8.1 verify gate.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after redaction gate check; row 24 partial until Phase 8.1)
- [ ] Step 8: Loud-RED gate audit per deep-dive `### Coverage gates` — confirm NO `#[ignore]`-away on devnet/mainnet tests. All gated-live tests use explicit `RUN_SOL_DEVNET=1` or `RUN_SOL_MAINNET=1` env var + clear STDERR message.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `grep -rn "#\[ignore\]" tests/` audit)
- [ ] Step 9: Update `CHANGELOG.md` per L24 — append Phase 6.2 verification entry: "Library completeness verified: 33/34 deep-dive rows GREEN (row 21 deferred V0.1.5); 32/32 test files compile; 100% coverage on crypto/, amount.rs, tx/builder.rs, spl/disambig.rs."
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after CHANGELOG.md commit lands)
- [ ] Step 10: PAUSE — operator review of the row-coverage table before Phase 7 begins. Any row marked ⚠️ or ❌ blocks Phase 7.
  - [ ] Verified by: operator sign-off in PR review (commit `<pending-sha>` on `<pending-date>`); PR labeled `rust-sol-core` + `priority/p0` + milestone `sol-wallet-core v0.1`

### Task 6.2.2 (TBD): Phase 7 dependency hand-off

**Files:** none (verification only)

**Steps:**

- [ ] Step 1: Confirm `crates/sol-wallet-core/src/lib.rs` re-exports all public surface needed by Phase 7 CLI handlers (`Wallet`, `WalletManager`, `RpcClient`, `SolanaConfig`, `build_sol_transfer`, `build_spl_transfer_checked`, `send_and_confirm`, `Error`; **V0.1.5: `send_with_retry`, `wait_for_confirm` added; `SolanaClient` deprecated in favor of `RpcClient`**).
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `cargo doc -p sol-wallet-core --no-deps` shows all 9 names)
- [ ] Step 2: Confirm `common/mod.rs` exports `mock_spl_usdc`, `surfpool_spawn`, `faucet`, `keypair_fixture` for `crates/sol/tests/` reuse per deep-dive `### Shared helpers reused from sol-wallet-core`.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `grep` audit of `common/mod.rs`)
- [ ] Step 3: Open a no-op scratch PR `sol/phase6.2-library-verified → rust-sol-core` — title "chore(sol): Phase 6.2 library completeness verified — 33/34 rows GREEN". CI must pass.
  - [ ] Verified by: PR number `<pending>` + commit SHA `<pending-sha>` on `<pending-date>` (operator fills after `gh pr create` returns PR URL + CI green)
- [ ] Step 4: PAUSE — merge the no-op PR; Phase 7 can now begin.
  - [ ] Verified by: squash-merge commit SHA `<pending-sha>` on `<pending-date>` (operator fills after `gh pr merge --squash` exits 0 per `update-issues-before-merge` rule)

#### Phase 6.2 — Verification

- [ ] All 33 in-scope rows have a passing test (rows 1-20, 22-34); row 21 explicitly on V0.1.5 backlog.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.1 Step 3 + row-coverage table check)
- [ ] All 32 named test files compile under `cargo build -p sol-wallet-core --tests`.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.1 Step 1 exits 0)
- [ ] `cargo test -p sol-wallet-core --tests` exits 0.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.1 Step 3 exits 0)
- [ ] 100% line coverage on `crypto/`, `amount.rs`, `tx/builder.rs`, `spl/disambig.rs` (per `cargo tarpaulin`).
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.1 Step 6 exits 0 with 100% on the 4 named modules)
- [ ] 7 fixtures present per deep-dive inventory (row 21 cert = V0.1.5 deferred).
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.1 Step 4 manual check)
- [ ] Zero `#[ignore]`-away on gated-live tests; loud-RED env vars present.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.1 Step 8 `grep` audit)
- [ ] CHANGELOG.md entry written (per L24).
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.1 Step 9 CHANGELOG.md commit lands)
- [ ] No-op scratch PR merged into `rust-sol-core`.
  - [ ] Verified by: PR number `<pending>` + squash-merge commit SHA `<pending-sha>` on `<pending-date>` (operator fills after Task 6.2.2 Step 4 merge)

**Loud-RED gate:** any step above fails → STOP Phase 6.2; return to the owning phase to fix the gap before re-running Phase 6.2. Do NOT begin Phase 7 until this verification exits 0.

---

## Phase 7 — `sol` CLI (22 commands)

### Task 7.1 (TBD): CLI scaffold + wallet handlers (create, import, show, list, delete, rename, balance, send, send-speedup)

**Files:**

rust-wallet-app/crates/sol/ CLI binary:

- Modify: `rust-wallet-app/crates/sol/Cargo.toml`
- Create: `rust-wallet-app/crates/sol/src/main.rs`
- Create: `rust-wallet-app/crates/sol/src/cli.rs`
- Create: `rust-wallet-app/crates/sol/src/handlers/mod.rs`
- Create: `rust-wallet-app/crates/sol/src/handlers/wallet.rs`
- Create: `rust-wallet-app/crates/sol/src/handlers/error.rs` (classify → exit code mapping 0/1/2/3/4/5)
- Modify: `rust-wallet-app/crates/sol/Cargo.toml` (depend on `sol-wallet-core`)

CLI tests (crates/sol/tests/) — Phase 7.1 owns 6 of the 10 CLI test files:

- Create: `rust-wallet-app/crates/sol/tests/cli_wallet.rs` (deep-dive CLI Scenario Test scenario — sol CLI — wallet create/import/show/list/delete/rename/balance/send end-to-end)
- Create: `rust-wallet-app/crates/sol/tests/cli_address.rs` (deep-dive CLI Scenario — address new/pubkey + Phantom UX parity)
- Create: `rust-wallet-app/crates/sol/tests/cli_balance.rs` (deep-dive CLI Scenario — balance --address + --address --token)
- Create: `rust-wallet-app/crates/sol/tests/cli_spl.rs` (deep-dive CLI Scenario — spl send/approve/balance/allowance)
- Create: `rust-wallet-app/crates/sol/tests/cli_tx.rs` (deep-dive CLI Scenario — tx get/wait polling)
- Create: `rust-wallet-app/crates/sol/tests/cli_config.rs` (deep-dive CLI Scenario — config show/set-rpc/set-cluster + Testnet reject per Q11)

**Steps:**
- [ ] Step 1: Implement `Cli` struct with global flags (data_dir, rpc, spki_pin, cluster, commitment, priority_fee, cu_limit, allow_insecure_tls) per deep-dive `### Solana CLI` architecture
- [ ] Step 2: Implement `Cluster` enum (`MainnetBeta | Devnet | Localnet`) — Q11: NO Testnet variant
- [ ] Step 3: Implement `Commitment` enum (`Processed | Confirmed | Finalized`)
- [ ] Step 4: Implement `Commands` enum with 22 variants: wallet (9 subcommands), address (2), balance (2), spl (4), tx (2), config (3)
- [ ] Step 5: Implement `wallet create` — wallet_create with_mnemonic + mnemonic → STDERR (red highlight), wallet_id → STDOUT
- [ ] Step 6: Implement `wallet import` — `--mnemonic` / `--mnemonic-file <mode-0600>` / `--private-key-file` (close argv-exposure L12 H-1); mnemonic → STDERR, wallet_id → STDOUT
- [ ] Step 7: Implement `wallet show` / `list` / `delete` / `rename` — invoke WalletManager (decrypted mnemonic NEVER displayed per deep-dive `### Mnemonic handling`)
- [ ] Step 8: Implement `wallet balance` — `--wallet-id` (unlock + `chain::get_balance`) OR `--address` (raw RPC query)
- [ ] Step 9: Implement `wallet send` — `--to <base58>` / `--to-wallet <name|id>` (mutually exclusive); `--amount` + `--unit sol|lamport` OR `--token USDC|<mint>` for SPL; `--priority-fee`, `--cu-limit`, `--memo`, `--dry-run`, `--sign-only`, `--wait`, `--wait-finalized`, `--confirm-mainnet`
- [ ] Step 10: Implement `wallet send-speedup --sig <base58-signature>` — emits NEW tx (no RBF on Solana) with higher priority fee
- [ ] Step 11: Implement `error::classify(err) -> exit_code` per deep-dive `### Exit code mapping` (0/1/2/3/4/5) + full 21-variant error table per deep-dive §J line 1825
- [ ] Step 12: Implement `crates/sol/tests/cli_wallet.rs` — wallet create/import/show/list/delete/rename/balance/send end-to-end via `assert_cmd`; mnemonic to STDERR, wallet_id to STDOUT; never print decrypted mnemonic in STDOUT
- [ ] Step 13: Implement `crates/sol/tests/cli_address.rs` — `address new --mnemonic --account 0 --address-index 0`; `address pubkey --wallet-id`; Phantom UX parity (numeric flags, no path string)
- [ ] Step 14: Implement `crates/sol/tests/cli_balance.rs` — `balance --address <base58>` returns SOL lamports; `balance --address <base58> --token USDC` returns SPL balance via `chain::spl_balance`
- [ ] Step 15: Implement `crates/sol/tests/cli_spl.rs` — `spl send/approve/balance/allowance` end-to-end against surfpool-deployed USDC mint
- [ ] Step 16: Implement `crates/sol/tests/cli_tx.rs` — `tx get --sig <base58>` + `tx wait --sig <base58> --timeout 60 --poll-interval 2` poll for confirmation per `tx::wait_for_confirm`
- [ ] Step 17: Implement `crates/sol/tests/cli_config.rs` — `config show` + `config set-rpc <url>` (per-cluster persist) + `config set-cluster mainnet-beta|devnet|localnet` (rejects `testnet` with `Error::InvalidCluster` per Q11)
- [ ] Step 18: Verify gate: `cargo fmt + cargo clippy -p sol -- -D warnings + cargo test -p sol --tests` (loud-RED `RUN_SOL_DEVNET=1` for `--cluster devnet`)
- [ ] Step 19: PAUSE — commit-push-pr

### Task 7.2 (TBD): address, balance, spl, tx, config handlers + surfpool e2e

**Files:**

- Create: `rust-wallet-app/crates/sol/src/handlers/address.rs`
- Create: `rust-wallet-app/crates/sol/src/handlers/balance.rs`
- Create: `rust-wallet-app/crates/sol/src/handlers/spl.rs`
- Create: `rust-wallet-app/crates/sol/src/handlers/tx.rs`
- Create: `rust-wallet-app/crates/sol/src/handlers/config.rs`

CLI tests (crates/sol/tests/) — Phase 7.2 owns the remaining 4 CLI test files + 5 library e2e tests:

- Create: `rust-wallet-app/crates/sol/tests/cli_integration_surfpool.rs` (deep-dive CLI Scenario — surfpool spawn + ephemeral port + 22 commands)
- Create: `rust-wallet-app/crates/sol/tests/cli_devnet_conformance.rs` (loud-RED `RUN_SOL_DEVNET=1` cross-cluster conformance)
- Create: `rust-wallet-app/crates/sol/tests/cli_json_output.rs` (deep-dive §N — JSON mode for all 22 commands; per-command JSON shape)
- Create: `rust-wallet-app/crates/sol/tests/cli_mainnet_smoke.rs` (Phase 9.1 owns — NOT created here; Phase 9.1 creates it with the loud-RED `RUN_SOL_MAINNET=1` gate and operator runbook)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/submit_sol_local.rs` (deep-dive row 26 — `submit_sol` E2E on surfpool)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/submit_spl_local_held.rs` (deep-dive row 27 — held recipient; ~5k CU; receipt SUCCESS)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/submit_spl_local_fresh.rs` (deep-dive row 28 — fresh recipient; ~0.00204 SOL rent; 2 ATAs created)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/submit_spl_local_approve.rs` (deep-dive row 29 — approve + allowance view)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/submit_send_speedup_local.rs` (deep-dive row 35 — new sig + higher priority fee; no RBF on Solana)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/boot_probe_local.rs` (deep-dive row 32 — `get_health` boot probe + cluster detect → `Localnet`)
- Create: `tests/common/mock_spl_usdc.rs` (Phase 7.2 owns USDC-style mint deploy helper)
- Create: `tests/common/faucet.rs` (Phase 7.2 owns `airdrop_surfpool(rpc, pubkey, lamports)`)

**Steps:**
- [ ] Step 1: Implement `address new --mnemonic [--mnemonic-file] --account <N> --address-index <N>` + `address pubkey --wallet-id` (note Q3: renamed from `xpub` for Ed25519)
- [ ] Step 2: Implement `balance --address <addr> [--unit sol|lamport]` + `balance --address <addr> --token USDC|<addr>` (auto-derives ATA, returns 0 if not yet created, returns `{balance, mint, decimals, ata}` JSON per deep-dive §B + §G)
- [ ] Step 3: Implement `spl send --mnemonic --token USDC|<addr> --to --amount` — auto-derives ATA for both sender and recipient + prepends `create_associated_token_account_idempotent` if ATAs missing + uses `transfer_checked` with dynamic decimals
- [ ] Step 4: Implement `spl approve --token --delegate --amount` + `spl balance --address --token` + `spl allowance --token --owner --delegate`
- [ ] Step 5: Implement `tx get --sig` (`chain::get_signature_statuses`) + `tx wait --sig --timeout --poll-interval` (`tx::wait_for_confirm`)
- [ ] Step 6: Implement `config show [--json]` + `config set-rpc <url>` (per-cluster persist) + `config set-cluster mainnet-beta|devnet|localnet` (rejects `testnet` per Q11)
- [ ] Step 7: Implement `tests/submit_sol_local.rs` (deep-dive row 26) — surfpool-backed; `submit_sol` full flow; receipt slot ≥ 1; sender lamport delta = amount; recipient lamport delta = amount
- [ ] Step 8: Implement `tests/submit_spl_local_held.rs` (row 27) — surfpool + deployed USDC-style mint; held recipient; `submit_spl_transfer` returns SUCCESS; recipient token balance = sent amount; CU ≈ 5_000 per deep-dive §C
- [ ] Step 9: Implement `tests/submit_spl_local_fresh.rs` (row 28) — surfpool + deployed mint + fresh recipient; `submit_spl_transfer` returns SUCCESS; ~0.00204 SOL rent charged to sender; 2 ATAs created (sender + recipient)
- [ ] Step 10: Implement `tests/submit_spl_local_approve.rs` (row 29) — surfpool + delegate; `submit_spl_approve` accepted; `spl_allowance` view returns approved amount
- [ ] Step 11: Implement `tests/submit_send_speedup_local.rs` (row 35) — surfpool; `submit_send_speedup` emits NEW tx (no RBF on Solana) with higher priority fee; new signature distinct from original
- [ ] Step 12: Implement `tests/boot_probe_local.rs` (row 32) — surfpool; `SolanaClient::get_health` returns `Ok`; cluster enum resolves to `Localnet`
- [ ] Step 13: Implement `crates/sol/tests/cli_integration_surfpool.rs` — surfpool spawn + ephemeral port + 22 commands coverage; binary invoked via `assert_cmd`; STDERR/STDOUT captured
- [ ] Step 14: Implement `crates/sol/tests/cli_devnet_conformance.rs` — loud-RED `RUN_SOL_DEVNET=1` cross-cluster conformance; binary pointed at `--cluster devnet`; 22 commands each pass against `https://api.devnet.solana.com`
- [ ] Step 15: Implement `crates/sol/tests/cli_json_output.rs` — JSON mode (`--json`) for all 22 commands; per-command JSON shape asserted
- [ ] Step 16: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test -p sol-wallet-core --test submit_sol_local --test submit_spl_local_held --test submit_spl_local_fresh --test submit_spl_local_approve --test submit_send_speedup_local --test boot_probe_local` + `cargo test -p sol --tests` (loud-RED `RUN_SOL_DEVNET=1` for devnet tests)
- [ ] Step 17: PAUSE — commit-push-pr

---

## Phase 7 verification — CLI completeness (all 22 in-scope deep-dive rows PASS)

**Goal:** confirm every `## Test scenario — sol CLI` row from `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` (lines 4232-4267, 22 rows) has a passing CLI test before Phase 8 begins FFI work. FFI depends on CLI surface being complete; a missing row blocks the FFI gates downstream.

**Scope:** 21 of 22 rows in Phase 7 verification scope. Row 22 (mainnet $0.001 USDC self-send) is **explicitly owned by Phase 9.1** per Q4 Q-gate + `## V0.1 mainnet gate` section; Phase 7 verification asserts row 22 is on the Phase 9.1 backlog, NOT on the V0.1 library gate.

**Marking discipline (L13 step 14 + `never-auto-commit` rule):** same as Phase 6.2 — each `- [ ] Step N:` stays unchecked until that step has been RUN locally by the operator; the nested `- [ ] Verified:` sub-checkbox flips to `[x]` only after the step exits 0 + commit SHA + date filled in + PR merged into `rust-sol-core` per L13 step 15.

### Row-to-CLI-test-file ownership matrix (22 rows)

| Row | Scenario | Owning test file | Phase |
| --- | -------- | ---------------- | ----- |
| 1  | Native SOL transfer | `tests/submit_sol_local.rs` + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 7.2 |
| 2  | SPL transfer held ATA | `tests/submit_spl_local_held.rs` + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 7.2 |
| 3  | SPL transfer fresh ATA | `tests/submit_spl_local_fresh.rs` | Phase 7.2 |
| 4  | SPL approve + allowance | `tests/submit_spl_local_approve.rs` | Phase 7.2 |
| 5  | Token-2022 vs classic SPL footgun guard | `tests/token2022_disambig.rs` (Phase 4.1) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 4.1 + 7.2 |
| 6  | Compute Budget auto-attach | `tests/compute_budget.rs` (Phase 3.1) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 3.1 + 7.2 |
| 7  | Memo attach | `tests/spl_instruction.rs` (extend Phase 4.1 to add memo case) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 4.1 Modify + Phase 7.2 |
| 8  | Wallet-to-wallet SOL | `crates/sol/tests/cli_wallet.rs` | Phase 7.1 |
| 9  | Wallet-to-wallet SPL | `crates/sol/tests/cli_spl.rs` | Phase 7.1 |
| 10 | Send-speedup | `tests/submit_send_speedup_local.rs` + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 7.2 |
| 11 | Blockhash retry on stale | (V0.1.5) `tests/send_with_retry.rs` (Phase 5.5) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 5.5 + 7.2 |
| 12 | Insufficient balance | `tests/preflight_balance.rs` (Phase 5.1) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 5.1 + 7.2 |
| 13 | Dry-run (simulate) | `tests/tx_serde.rs` (extend Phase 3.1 to add `--dry-run` simulate case) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 3.1 Modify + Phase 7.2 |
| 14 | Sign-only (no broadcast) | `tests/sign_only.rs` (Phase 1.2) + `crates/sol/tests/cli_wallet.rs` | Phase 1.2 + 7.1 |
| 15 | Confirmation polling — success | `tests/send_native.rs` + `tests/send_token.rs` (Phase 5.1) + `crates/sol/tests/cli_tx.rs` | Phase 5.1 + 7.1 |
| 16 | Confirmation polling — timeout | `tests/send_native.rs` (V0.1: short blockhash, expect `Error::ConfirmTimeout` after 30s) + `crates/sol/tests/cli_tx.rs` | Phase 5.1 + 7.1 |
| 17 | Finalized commitment | `tests/send_native.rs` (V0.1: `--finalized` flag path) + `crates/sol/tests/cli_tx.rs` | Phase 5.1 + 7.1 |
| 18 | Wallet list across clusters | `crates/sol/tests/cli_wallet.rs` + `crates/sol/tests/cli_config.rs` | Phase 7.1 |
| 19 | Wallet delete + rename | `tests/wallet_lifecycle.rs` (Phase 6.1) + `crates/sol/tests/cli_wallet.rs` | Phase 6.1 + 7.1 |
| 20 | Config switch cluster | `crates/sol/tests/cli_config.rs` | Phase 7.1 |
| 21 | Network failure recovery | `tests/transport_failure.rs` (Phase 5.1) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 5.1 + 7.2 |
| 22 | Mainnet smoke (Q4 analog) | `tests/mainnet_smoke.rs` + `crates/sol/tests/cli_mainnet_smoke.rs` (Phase 9.1 owns) | Phase 9.1 (not Phase 7) |

### Task 7.3 (TBD): CLI test compile + run gate

**Files:** none (verification only — no source or test creates)

**Steps:**
- [ ] Step 1: Run `cargo build -p sol --tests` — confirm all 10 CLI test files compile. Loud-RED if any compile error.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after step exits 0 + PR merges)
- [ ] Step 2: Run `cargo test -p sol --tests` — confirm all CLI tests pass on surfpool (rows 1-21 covered). Loud-RED if any test fails.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after step exits 0)
- [ ] Step 3: Run `cargo test -p sol --test cli_devnet_conformance -- --ignored` (loud-RED `RUN_SOL_DEVNET=1`) — confirm row 14 cross-cluster conformance passes against `https://api.devnet.solana.com`. Loud-RED if any test fails.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after step exits 0 with funded test keypair)
- [ ] Step 4: Cross-check 22 rows against `cli_integration_surfpool.rs` — confirm each row 1-21 has at least one `#[test]` function with the row's pass criterion asserted. Loud-RED if any row is missing.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after manual row cross-check)
- [ ] Step 5: Confirm row 7 (Memo attach) is covered — extend `tests/spl_instruction.rs` (Phase 4.1 Modify) to assert `spl_memo::build_memo` ix present; if not, add a Memo case to `spl_instruction.rs` before Phase 8.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `cargo test -p sol-wallet-core --test spl_instruction` exits 0 with memo case)
- [ ] Step 6: Confirm row 13 (Dry-run/simulate) is covered — extend `tests/tx_serde.rs` (Phase 3.1 Modify) to assert `simulate_transaction` returns CU consumed + no sig emitted + no balance change; if not, add a Dry-run case to `tx_serde.rs` before Phase 8.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `cargo test -p sol-wallet-core --test tx_serde` exits 0 with simulate case)
- [ ] Step 7: Confirm row 22 (Mainnet smoke) is on the Phase 9.1 backlog — verify `tests/mainnet_smoke.rs` + `crates/sol/tests/cli_mainnet_smoke.rs` are scheduled for Phase 9.1 creation per Q4 Q-gate. Loud-RED if row 22 is silently dropped from V0.1.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after backlog cross-check)
- [ ] Step 8: Confirm `sol --help` exits 0 + all 22 commands listed in help output.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `cargo run -p sol -- --help` exits 0)
- [ ] Step 9: Confirm exit code mapping (0/1/2/3/4/5) per deep-dive `### Exit code mapping` — run each error path through CLI; assert correct exit code emitted.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after exit code audit)
- [ ] Step 10: Loud-RED gate audit — confirm NO `#[ignore]`-away on devnet tests; row 14 cli_devnet_conformance uses explicit `RUN_SOL_DEVNET=1` env var + clear STDERR message.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after `grep -rn "#\[ignore\]" crates/sol/tests/` audit)
- [ ] Step 11: Update `CHANGELOG.md` per L24 — append Phase 7 verification entry: "CLI completeness verified: 21/22 deep-dive rows GREEN (row 22 deferred Phase 9.1); 10/10 CLI test files compile; rows 7+13 extended via Phase 4.1/3.1 Modify."
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after CHANGELOG.md commit lands)
- [ ] Step 12: Open a no-op scratch PR `sol/phase7-verified → rust-sol-core` — title "chore(sol): Phase 7 CLI completeness verified — 21/22 rows GREEN". CI must pass.
  - [ ] Verified by: PR number `<pending>` + squash-merge commit SHA `<pending-sha>` on `<pending-date>` (operator fills after `gh pr create` returns URL + CI green + `gh pr merge --squash` exits 0)

#### Phase 7 verification — Verification block

- [ ] All 21 in-scope rows (1-21) have a passing CLI test; row 22 explicitly on Phase 9.1 backlog.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Steps 2+4+7 exit 0)
- [ ] All 10 CLI test files compile under `cargo build -p sol --tests`.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 1 exits 0)
- [ ] `cargo test -p sol --tests` exits 0 (rows 1-21 covered).
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 2 exits 0)
- [ ] Rows 7 + 13 (Memo + Dry-run) extended in Phase 4.1 + 3.1 Modify cases.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Steps 5+6 exit 0)
- [ ] Row 22 on Phase 9.1 backlog per Q4 Q-gate.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 7)
- [ ] `sol --help` exits 0 + 22 commands listed.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 8)
- [ ] Exit code mapping (0/1/2/3/4/5) verified for all error paths.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 9)
- [ ] No `#[ignore]`-away on gated-live tests; loud-RED env vars present.
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 10)
- [ ] CHANGELOG.md entry written (per L24).
  - [ ] Verified by: commit `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 11)
- [ ] No-op scratch PR merged into `rust-sol-core`.
  - [ ] Verified by: PR number `<pending>` + squash-merge commit SHA `<pending-sha>` on `<pending-date>` (operator fills after Task 7.3 Step 12)

**Loud-RED gate:** any step above fails → STOP Phase 7 verification; return to the owning phase (4.1/5.1/7.1/7.2) to fix the gap before re-running. Do NOT begin Phase 8 until this verification exits 0.

---

## Phase 8 — FFI cdylib

### Task 8.1 (TBD): ffi.rs + 12 C functions + panic-message scrubber

**Files:**
- Create: `src/ffi.rs`
- Create: `src/panic_scrubber.rs` (separate module for testability)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/placeholder.rs` (Phase 8.1 owns creation + implementation — covers deep-dive rows 24+25: FFI panic scrubber fuzz + C ABI smoke across all 12 exports)

**Steps:**
- [ ] Step 1: Implement panic-message scrubber (`once_cell::Lazy<regex::Regex>` matching mnemonic word patterns + 64-byte base58 + `xprv...` prefix + 32-byte hex); all STDERR panic msgs filtered; `Zeroizing` wrap on secrets; FFI returns exit code 99 + scrubbed msg (per deep-dive FFI safety contract line 1839 + F47 zeroize gap Plan line 151)
- [ ] Step 2: Implement `sol_wallet_create_mnemonic(*out_id, *out_mnemonic, password, cluster)` → delegates to WalletManager
- [ ] Step 3: Implement `sol_wallet_import_mnemonic(*out_id, *in_mnemonic, password, cluster)`
- [ ] Step 4: Implement `sol_wallet_unlock(*out_secret_bytes, id, password)` → returns 32-byte secret; caller MUST Zeroize
- [ ] Step 5: Implement `sol_wallet_lock(id)`
- [ ] Step 6: Implement `sol_wallet_get_address(*out_pubkey, id)`
- [ ] Step 7: Implement `sol_wallet_sign_transaction(*out_signature, *in_message, id)` → signs 32-byte hash
- [ ] Step 8: Implement `sol_wallet_send_sol(*out_sig, id, *to, amount_lamports, priority_fee)` — high-level mobile convenience
- [ ] Step 9: Implement `sol_wallet_send_spl(*out_sig, id, *mint, *to, amount)` — high-level mobile convenience
- [ ] Step 10: Implement `sol_wallet_get_balance_sol(*out_lamports, *address)` + `sol_wallet_get_balance_spl(*out_balance, *out_decimals, *address, *mint)`
- [ ] Step 11: Implement `sol_wallet_last_error_message(*out_msg, buf_len)` + `sol_wallet_panic_message_clear`
- [ ] Step 12: Implement `tests/placeholder.rs` row 24 part — fuzz panic scrubber with 10k cases containing mnemonics + 64-byte base58 + 32-byte hex + `xprv...` prefix; assert scrubbed output never contains any input secret (regex fuzz via `proptest!` or custom corpus)
- [ ] Step 13: Implement `tests/placeholder.rs` row 25 part — call each of the 12 FFI functions from in-process `cdylib` load via `libloading::Library::new` + symbol resolution; assert each returns expected status code (0 = success, non-zero = documented error); no panic crosses FFI boundary
- [ ] Step 14: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test placeholder`
- [ ] Step 15: PAUSE — commit-push-pr

### Task 8.2 (TBD): Mobile compile gate

**Steps:**
- [ ] Step 1: Verify `cargo check --target aarch64-apple-ios` succeeds (Phase 5+ gate per Q17)
- [ ] Step 2: Verify `cargo check --target aarch64-linux-android` succeeds
- [ ] Step 3: Verify cbindgen emits `sol_wallet_core.h` matching Dart/Swift/Kotlin FFI consumer expectations
- [ ] Step 4: PAUSE — release-cut PR

---

## Phase 9 — Mainnet smoke gate + release cut

### Task 9.1 (TBD): Mainnet self-send smoke test (Q4 Q-gate)

**Files:**
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/mainnet_smoke.rs` (deep-dive row 33 — mainnet $0.001 USDC self-send)
- Create: `rust-wallet-app/crates/sol/tests/cli_mainnet_smoke.rs` (Phase 7.2 created; Phase 9.1 activates the loud-RED gate + operator runbook)

**Steps:**
- [ ] Step 1: Implement gated live test (loud-RED per deep-dive `### Gated live tests`):
  - Mark `#[ignore]`
  - Gate body on `RUN_SOL_MAINNET=1` + `ALCHEMY_API_KEY` + `SOL_OPERATOR_WALLET`
  - If missing, `panic!("missing env: {RUN_SOL_MAINNET, ALCHEMY_API_KEY, SOL_OPERATOR_WALLET}")`
- [ ] Step 2: Operator checklist (runbook):
  - Load Alchemy API key via `ALCHEMY_API_KEY`
  - Ensure operator wallet has ≥ 0.01 USDC + 0.01 SOL for fees + rent buffer
  - `sol wallet send --to <self> --amount 0.001 --token USDC --wait --wait-finalized --priority-fee 1000 --confirm-mainnet`
  - Verify via `sol balance --address <self> --token USDC` — balance changed by 0.001 USDC
  - Cleanup: `sol config set-cluster devnet` (return to dev for ongoing work)
- [ ] Step 3: Success criteria: `getSignatureStatuses` returns `confirmed` then `finalized` commitment; sender + recipient balances update by 0.001 USDC; no `BlockhashNotFound` or `InsufficientFunds` errors
- [ ] Step 4: Failure mode: revert to devnet — investigate locally before retrying mainnet
- [ ] Step 5: Verify gate: `RUN_SOL_MAINNET=1 ALCHEMY_API_KEY=... SOL_OPERATOR_WALLET=... cargo test -p sol-wallet-core --test v12_mainnet_smoke -- --ignored`
- [ ] Step 6: PAUSE — release-cut PR; gate release on V12 PASS evidence

### Task 9.2 (TBD): CHANGELOG + docs + release cut

**Steps:**
- [ ] Step 1: Update `rust-wallet-app/crates/sol-wallet-core/CHANGELOG.md` with V0.1.0 pre-release entry per L24 (Phase 0-9 close)
- [ ] Step 2: Update `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` §"Round-1 grill audit" — flip each "Status: ready" row to "Status: V0.1 SHIPPED" if test scenario PASS confirmed; otherwise demote to "ready (untested)" or remove
- [ ] Step 3: Issue body checkboxes — flip all `[ ]` to `[x]` BEFORE squash-merge (L13 step 14)
- [ ] Step 4: Tag release `v0.1.0` on `main`
- [ ] Step 5: Publish dry-run check: `cargo publish --dry-run -p sol-wallet-core` succeeds
- [ ] Step 6: PAUSE — release PR (one final review pass before tag)

---

## Test Coverage Reconciliation (deep-dive §"Test scenario — sol-wallet-core V0.1" → Phase file ownership)

Deep-dive [§"Test scenario — sol-wallet-core V0.1"](docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md) enumerates **34 test scenarios** + **16 entry-points** + **32 named library test files** + 10 CLI test files. SOL plan tracks these as Phase-owned file creates per `## Test File Structure`. This section reconciles the two: which deep-dive rows map to which Phase test, and which rows are GAPS that require explicit Phase-tickets or V0.1.5 deferral.

### Mapping: deep-dive row → SOL plan Phase file

| Row | Module / scope                         | Deep-dive row summary                                   | SOL plan Phase                  | Coverage                                            |
| --- | -------------------------------------- | ------------------------------------------------------- | ------------------------------- | --------------------------------------------------- |
| 1   | `wallet` (HD)                          | BIP-39 → SLIP-0010 `m/44'/501'/0'/0/0`                  | **V2** (Phase 1.1)               | ✅ covered                                           |
| 2   | `wallet` (HD → base58)                 | XPrv → Ed25519 → base58                                 | **V2** + **V4**                  | ✅ covered                                           |
| 3   | `amount` parsing                       | `1.5 SOL` → `1_500_000_000`; overflow → `AmountOverflow` | **NEW** V0.1 unit                | ⚠️ GAP — add to Phase 0 (stub) or Phase 1.1 as unit |
| 4   | `amount` proptest round-trip           | `display_in(as_lamport(x)) == x` 10k proptest            | **NEW** V0.1 unit                | ⚠️ GAP — same as 3                                  |
| 5   | `crypto` Argon2id determinism           | m=64MB t=3 p=1 fixed; same input → same key              | **V11** Phase 6.1                | ✅ covered                                           |
| 6   | `crypto` AES-GCM round-trip             | round-trip + single-bit flip → `DecryptFailed`          | **V11** Phase 6.1                | ✅ covered                                           |
| 7   | `crypto` mnemonic encrypt-at-rest      | correct + wrong passphrase                              | **V11** Phase 6.1                | ✅ covered                                           |
| 8   | `persist` create → save → load → sign  | round-trip via `tempfile` + `FileWalletStorage`        | **V11** Phase 6.1                | ✅ covered                                           |
| 9   | `persist` mode 0600 + atomic write      | permissions + no `.tmp` residue                         | **V11** Phase 6.1                | ✅ covered                                           |
| 10  | `wallet_manager` UUID + name lookup     | 1000 wallets, no collision, `lookup("cold")` resolves  | **V11** Phase 6.1                | ⚠️ PARTIAL — add explicit UUID test                  |
| 11  | `config` TOML save/load + per-cluster   | round-trip + devnet URL match                           | **NEW** Phase 6.1 unit          | ⚠️ GAP                                                |
| 12  | `config` blockhash cache TTL 60s        | second call within 60s = 0 RPC; 61s = refetch          | **V6** Phase 5.1 (implicit)      | ⚠️ GAP — explicit TTL test missing                   |
| 13  | `disambig` Token-2022 vs classic        | `disambig::reject_wrong_token_program` returns Err      | **V10** Phase 4.1                | ✅ covered                                           |
| 14  | `disambig` decimals via `Mint::unpack` | USDC=6 / PYUSD=6 / BONK=5 / JitoSOL=9                  | **V9** Phase 4.1 (mainnet-gated) | ✅ covered                                           |
| 15  | 7 builders bincode round-trip           | every builder verified                                  | **V5** (SOL part) + **V4** + V9   | ✅ PARTIAL — SPL/builder part needs explicit round-trip |
| 16  | sign flow + recent_blockhash            | verify via `solana_sdk::transaction::verify`           | **V8** Phase 1.2                 | ✅ covered                                           |
| 17  | `sign_only_tx` cold path               | no RPC; deterministic re-sign                          | **V8** Phase 1.2                 | ✅ covered                                           |
| 18  | auto-ATA-create prepend                | `None` → prepended ix                                  | **V10** Phase 4.1                | ✅ covered                                           |
| 19  | preflight balance ≥ amount+fee+rent   | under-funded → `InsufficientFunds`                     | **NEW** Phase 5.1 unit           | ⚠️ GAP                                                |
| 20  | `receipt` TransactionStatus JSON → struct | success + error parse                                 | **NEW** Phase 7.1 unit          | ⚠️ GAP                                                |
| 21  | SPKI pin match / mismatch              | real leaf cert fixture                                 | **V7** Phase 5.5 (V0.1.5 opt-in)  | ⚠️ DEFERRED V0.1.5 (per Q9 Scenario B default)      |
| 22  | 12 RPC methods vs wiremock              | success + 5xx → `Transport`                            | **NEW** Phase 5.1 unit           | ⚠️ GAP                                                |
| 23  | `error.rs` 21 From impls + Debug redact  | no mnemonic/secret leak                                | **NEW** Phase 0 unit            | ⚠️ GAP                                                |
| 24  | FFI panic scrubber fuzz                  | 10k fuzz cases; scrubbed output no secret               | **ffi_scrubber** Phase 8.1       | ⚠️ PARTIAL — explicit fuzz test missing              |
| 25  | C ABI smoke (12 exports)                | create → sign → status code                             | **ffi_smoke** Phase 8.1          | ✅ covered                                           |
| 26  | end-to-end SOL transfer                 | sender/recipient lamport delta                          | **V15** Phase 7.2                 | ✅ covered                                           |
| 27  | end-to-end SPL transfer (held ATA)     | receipt SUCCESS, ~5k CU                                 | **V15** Phase 7.2                 | ✅ covered                                           |
| 28  | end-to-end SPL transfer (fresh ATA)    | 2 ATAs created, ~0.00204 SOL rent                       | **V15** Phase 7.2                 | ✅ covered                                           |
| 29  | end-to-end SPL approve                  | approve + allowance view                               | **V15** Phase 7.2                 | ⚠️ PARTIAL — add explicit spl-approve test           |
| 30  | `send_with_retry` stale blockhash      | `BlockhashNotFound` → retry with fresh                  | **V6** Phase 5.5 (V0.1.5) (devnet-gated)  | ⏸️ V0.1.5                           |
| 31  | `wait_for_confirm` success + timeout    | success returns receipt; bogus sig → `ConfirmTimeout`  | **V6** Phase 5.1 (implicit)     | ⚠️ PARTIAL — explicit timeout test missing         |
| 32  | `get_health` boot probe                  | cluster enum resolves `Localnet`                       | **V15** Phase 7.2                 | ⚠️ PARTIAL                                           |
| 33  | mainnet $0.001 USDC self-send           | confirmed on explorer                                  | **V12** Phase 9.1 (Q4 gate)     | ✅ covered                                           |
| 34  | transport failure (closed port)         | `Error::Transport` within 30s timeout                   | **NEW** Phase 7.1 unit          | ⚠️ GAP                                                |

**Coverage summary:**

| Status   | Count | Rows                              |
| -------- | ----- | --------------------------------- |
| ✅ Covered            | 17 | 1, 2, 5-9, 13, 14, 16-18, 25-28, 30, 33 |
| ⚠️ PARTIAL           | 8  | 10, 15, 24, 29, 31, 32, plus implicit 11-12 |
| ⚠️ GAP (add V0.1)    | 8  | 3, 4, 11, 19, 20, 22, 23, 34 |
| ⏸️ DEFERRED V0.1.5   | 1  | 21 (SPKI live cert, Q9 Scenario A opt-in) |

**Action:** Phase 0 Task 0.1 Step 2 has zero GAP test stubs — Phase 0 creates no tests. Phase 1-9 Tasks retain their owned test files per `## Test File Structure`; module-level tests filled in during Phase implementation per deep-dive row.

### Test file mapping: deep-dive → Phase-owned file

Deep-dive lists 32 library test files in `crates/sol-wallet-core/tests/` + 10 CLI test files in `crates/sol/tests/` per `## Test File Structure` section. Each Phase owns a subset of the 32 library + 10 CLI files per the Per-Step Status table + Phase-to-test-file mapping summary.

### Loud-RED gate consistency

The plan-guide loud-RED gate contract applies to live-network tests: V6 (devnet blockhash refresh), V9 (mainnet token registry live verify), V12 (mainnet self-send Q4 gate), V14 (CLI coverage against `--cluster devnet`). Additional loud-RED tests from the deep-dive matrix (not already on V-ladder): **V15_SPL_APPROVE** (loud-RED for devnet approve test — boundary case for SPL pre-2022), **V16_TRANSPORT_FAIL** (loud-RED for RPC timeout — `RUN_SOL_DEVNET=1` forces real timeout). Filed under Phase 7.2 Step 7 as additions.

### Phasing constraint

GAPS cannot defer to V0.1.5 — Phase 9 mainnet smoke gate (V12) is gated on **all 34 deep-dive rows returning PASS** (or explicit accepted-with-known-issue per owner sign-off). Operator signs off on each row via `cargo test -p sol-wallet-core --test <name>` showing GREEN.

---

## Design Coverage Map (delegates to deep-dive as source-of-truth)

Per CLAUDE.md "every claim links back to a source file" + plan-guide principle of state continuity: this plan is the **implementation vehicle**; the deep-dive ([docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md](docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md)) is the **design source-of-truth**. Phase implementors read deep-dive on demand via the anchors below. Plan does NOT duplicate tables.

| Deep-dive §                                                       | Line  | Plan section that consumes it                                                            | Status   |
| ----------------------------------------------------------------- | ----- | ------------------------------------------------------------------------------------------- | -------- |
| ## TL;DR + ## Chosen crates & SDKs (v0.1)                          | 9-46  | Plan `## Tech Stack` line 31; Phase 0 Task 0.1 Step 2 Cargo.toml                             | ✅       |
| ## Crates used in `sol-wallet-core` (V0.1, projected) — 46 crates   | 47-321 | Plan `## Tech Stack` line 31 + Phase 0 Task 0.1 Step 2 Cargo.toml ordering recommendation    | ✅       |
| ## Solana Networks (clusters + endpoints)                          | 323-993 | Plan Phase Set Up + Plan Q4/Q5/Q11 + Phase 7.2 config handlers                             | ✅       |
| ## Mnemonic-to-broadcast data flow                                  | 1010-1039 | Plan Phase 1 (HD) + Phase 3 (tx::builder) + Phase 5 (broadcast)                          | ✅       |
| ## Network + TLS pinning research                                  | 1041-1057 | Plan Phase 5.5 (V0.1.5 opt-in via SPKI pin Q9)                                            | ✅       |
| ## Solana program feature map (9 crates × 90 features)              | 1077-1242 | Plan Phase 1-5 modules                                                                     | ✅       |
| ## Solana Wallet v0.1 (feature map by CLI top-level, 22 commands)   | 1244-1493 | Plan Phase 7.1 + Phase 7.2                                                                  | ✅       |
| ## Solana Wallet v0.1 — Complete Feature Inventory                  | 1495-2121 | Plan Phase 1-9 + this Design Coverage Map                                                   | ✅ (delegated) |
| └ §A. Native SOL operations                                       | 1543-1553 | Phase 3.1 + Phase 5.1                                                                       | ✅       |
| └ §B. SPL token operations (classic + Token-2022, 12 features)      | 1555-1590 | Phase 4.1                                                                                   | ✅       |
| └ §C. Compute Budget (6 tx-type defaults)                          | 1592-1622 | Phase 3.1 Step 4 (parameter; full row table delegated)                                      | ⚠️ Delegated |
| └ §D. Blockhash lifecycle (retry pseudocode)                       | 1624-1664 | Phase 5.1 Step 3 (`send_with_retry`; `BlockCleanedUp` handling delegated)                    | ⚠️ Delegated |
| └ §E. Confirmation polling (commitment matrix)                     | 1666-1729 | Phase 5.1 Step 4 + Phase 7.1 `tx wait`                                                      | ⚠️ Delegated |
| └ §F. Wallet keypair (Phantom-equivalent surface)                  | 1671-1685 | Plan `### Wallet surface — Phantom-equivalent`                                              | ✅       |
| └ §G. Address encoding (9 features + address comparison)            | 1731-1752 | Phase 2.1                                                                                   | ✅       |
| └ §H. Wallet file encryption (Argon2id + AES-256-GCM JSON schema)   | 1754-1790 | Phase 6.1 Step 1 (parameters; full JSON schema delegated)                                    | ⚠️ Delegated |
| └ §I. RPC client (16 HTTP + 5 WS = 21 methods, full method coverage) | 1792-1823 | Phase 5.1 Step 2 (full 21-method enumeration delegated — 10 inline; 11 in deep-dive)         | ⚠️ Delegated |
| └ §J. Error classification (21 variants + 5 exit codes)             | 1825-1862 | Phase 7.1 Step 11 (`error::classify`; full variant table delegated to deep-dive)             | ⚠️ Delegated |
| └ §K. FFI surface (cdylib, 12 C functions)                          | 1864-1890 | Phase 8.1 (full enumeration in plan)                                                        | ✅       |
| └ §L. PAL (Platform Abstraction Layer) — 4 traits × 14 methods       | 1892-1910 | Plan `### Four-layer PAL design` + Phase 6.1 (4 traits named; per-method table delegated)   | ⚠️ Delegated |
| └ §M. Complete CLI command reference (22 commands, full flag ref)   | 1912-2054 | Phase 7.1 + Phase 7.2                                                                        | ✅       |
| └ §N. Output formats (JSON mode per command)                       | 2056-2082 | Phase 7.1 + Phase 7.2 (--json flag mentioned; full per-command JSON shape delegated)         | ⚠️ Delegated |
| └ §O. Memory hygiene (Zeroizing discipline)                        | 2084-2097 | Plan `## F47 zeroize gap`                                                                    | ✅       |
| └ §P. What's NOT in V0.1 (explicit non-features)                    | 2124-2183 | Plan `## V0.1.5 work` + `## V0.2+ deferred` + `## NEVER (out of scope)`                       | ✅       |
| └ §R. Cross-cutting concerns                                        | 2099-2122 | Plan `### Cross-cutting (apply to all)`                                                     | ⚠️ Delegated (shorter form) |
| ## Solana Wallet Core v0.1 (architecture)                          | 2185-2200+ | Plan `## Architecture (locked 2026-09-08)`                                                   | ✅       |
| ## Public APIs shipped in V0.1 (~190 across 17 modules)             | 2374-2399 | Plan `## File Structure` modules list (full API enumeration delegated to deep-dive)         | ⚠️ Delegated |
| ## License summary                                                  | 2484-2483 | Plan file structure `// License summary`                                                    | ✅       |
| ## Cross-crate feature map (sol-wallet-core × solana-sdk + SPL)      | 2484-2508 | Plan Phase 1-5 modules + Phase 0 Cargo.toml ordering                                        | ✅       |
| ## Risk register (18 risks)                                          | 2510-2531 | Plan `## Risk Register (V0.1)` (full 18 rows in plan, mirrors deep-dive)                    | ✅       |
| ## (removed 2026-09-09) Cycle 1 / Cycle 2 / Cycle 3 spike status      | (removed) | Q4 cleanup removed cycle section from deep-dive; canonical test file structure lives in `## Test File Structure` section above | ✅       |
| ## Confidence summary for V0.1                                      | 2535-2551 | Plan `## Confidence Summary for V0.1`                                                       | ✅       |

**Delegation rule (operator):** when Phase N is in implementation, jump to the deep-dive line anchor and read the corresponding §X.Y. Plan retains the architectural decisions and Phase boundaries; deep-dive carries the per-feature tables and per-test fixtures.

**Severity for ⚠️ Delegated rows:**

- §C, §D, §E (Compute Budget defaults + Blockhash retry + Confirmation matrix) — material for Phase 3-5 implementation. Operator MUST read these before Phase 3.1 + 5.1 kickoff.
- §H (Encrypted wallet JSON schema) — material for Phase 6.1 kickoff.
- §I (RPC 21 methods), §J (Error 21 variants + 5 exit codes) — material for Phase 5.1 + Phase 7.1 kickoff.
- §L (PAL per-method table), §N (Output formats JSON per command), §R (Cross-cutting longer form) — material for Phase 6.1 + 7.1-7.2 + cross-cutting integration.

None of the ⚠️ Delegated rows are blockers for plan structure; all are operator-read-on-demand for phase implementation.

---

## V0.1.5 work (deferred — NOT shipped in V0.1)

| Feature | Notes | Crate |
|---|---|---|
| `solana-test-validator` opt-in (`SolanaTestValidatorGuard`) | BPF loading + epoch boundary tests | `tests/common/solana_test_validator_guard.rs` |
| Remove `directories` crate | replaced by `WalletStorage` trait | `Cargo.toml` |
| Gate `rustls-native-certs` behind `#[cfg(not(mobile))]` | mobile uses `tls_built_in_root_certs(true)` | `Cargo.toml` |
| Add `release-mobile` profile | `opt-level = "z"`, `panic = "abort"` | `Cargo.toml` |
| Drop unused `solana-program` features | shaves ~3-4 MB | `Cargo.toml` + Anza feature flags |
| Token-2022 Transfer Hook CPI dispatch | manual ix append | `spl_token_2022::extension::transfer_hook` |
| Token-2022 Confidential Transfer (zk proofs) | `solana-zk-sdk` 7.0.1 (ElGamal) | V0.3 |
| Address Lookup Tables (ALTs) | reduce tx size | `solana_message::v0` |
| Durable nonces (offline signing >90s) | nonce_account rent | `solana_program::system_instruction::advance_nonce_account` |
| `sol sign message` / `sol sign verify` | CLI flag | `wallet::signMessage` already exists; CLI flag deferred |
| Jito tip routing (MEV bundles) | gate behind `jito` Cargo feature | `jito-sdk-rust` 0.3.2 (stale) |

---

## V0.2+ deferred (NOT shipped in V0.1)

| Feature | Notes |
|---|---|
| Stake account creation + delegate + deactivate + withdraw + merge | `stake::StakeState` program; native SOL staking |
| `sol tokens list` / `register` / `balances` | bundled + custom mint registration |
| `sol compute estimate` / `sol fee history` | priority fee oracle + dry-run cost estimation |
| `sol faucet show` / `sol faucet drip` | devnet airdrop URL + auto-drip |
| Token-2022 mint creation (`sol token create-mint` + `--with-metadata`) | deploy new token |
| `sol stake create/delegate/deactivate/withdraw` | native SOL staking |
| `sol wallet sync` / `sol wallet export` / `sol wallet show --secret` (gated) | wallet history + cold storage export + password + confirmation |
| `sol address show --private-key` | per-address reveal |
| `sol wallet import --keypair-file` | Solflare/Phantom format base58 64-byte secret |
| `sol tx list` | history via `getSignaturesForAddress` |
| `sol config set-spki-pin` | move SPKI pin from env-only to CLI |
| Shell completion (`sol shell completion`) | clap built-in |
| Thread model (FFI deadlock / Zeroizing-across-await / Send+Sync on keypair / concurrent broadcast seriality) | v0.2 — FFI model review |

---

## NEVER (out of scope)

| Feature | Reason |
|---|---|
| Hardware wallet (Ledger/Trezor) | v1.x — separate SDK + transport protocol |
| EIP-712 typed data | Solana has no typed-data spec; uses `signMessage` for off-chain |
| Plausible-deniability multi-bucket wallet | far future |
| gRPC transport | JSON-RPC + WS sufficient; Anza has no gRPC Rust SDK |
| Watch-only import from `xpub` | Ed25519 HD has no parent public key — use `pubkey` only for limited watch-only |
| Metaplex NFT program support | **license blocker** — `Metaplex NFT Open Source License v1.0` non-OSI |
| Testnet cluster | DEPRECATED 2022-23 by Solana Foundation |

---

## Loud-RED Gate Contract (mandatory per deep-dive)

Every test that touches live network or operator-held secret MUST:

1. Mark `#[ignore]` so excluded from default `cargo test` run.
2. Gate body on required env vars (`RUN_SOL_DEVNET`, `RUN_SOL_MAINNET`, `ALCHEMY_API_KEY`, `SOL_OPERATOR_WALLET`).
3. If missing, **`panic!`** with actionable message naming every missing variable.
4. Harness reports `FAILED` when env vars absent, `ignored` when opted in without env.

**Tests with loud-RED gate:** V6 (devnet blockhash refresh), V9 (token registry live verify), V12 (mainnet self-send Q4 gate), V14 (CLI coverage against `--cluster devnet`).

---

## Risk Register (V0.1)

| #   | Risk                                                  | Severity | Mitigation                                                                                                                                                         |
| --- | ----------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Anza bus-factor = single-vendor (90%+ of stack)      | ACCEPTED | Apache-2.0 license, ~3-month release cadence, ~30 contributors, actively maintained. No vendoring required. Monitor `anza-xyz/agave` security advisories.           |
| 2   | Anza subcrate version drift (desync)                  | LOW      | Pin each subcrate with `=x.y.z` exact; `cargo update -p solana-sdk` does NOT roll subcrates. Verified in Task 0.1 Step 6.                                            |
| 3   | Blockhash lifetime ~60-90 sec (no ChainId replay)     | MITIGATED | `send_with_retry` re-fetches + re-signs on `BlockhashNotFound` (3 attempts, exponential backoff). Q7.                                                                |
| 4   | `xpub` not exposed by Ed25519 SLIP-0010               | DOCUMENTED | `address pubkey` CLI command instead of `address xpub`; watch-only via per-leaf pubkey only. Phantom uses same limitation.                                          |
| 5   | SPL token ATA derivation seed includes `token_program_id` → different addresses for classic vs Token-2022 | MITIGATED | `disambig::reject_wrong_token_program` checks `mint.owner`. Q6.                                                                                                     |
| 6   | `Keypair::from_seed(s: &[u8])` does NOT Zeroize input | MITIGATED | Caller wraps seed in `Zeroizing<Vec<u8>>`.                                                                                                                          |
| 7   | `bip39::Seed::as_bytes()` returns `&[u8]` — no Zeroize on parent | MITIGATED | Zeroize-wrap master seed at construction; scope-bounded.                                                                                                            |
| 8   | `ed25519-bip32::XPrv::to_string()` returns `String` (not Zeroize) | MITIGATED | Caller wraps in `Zeroizing<String>`; never log XPrv.                                                                                                              |
| 9   | Solana Labs public RPC no SPKI pinning               | DOCUMENTED | Skip pinning per Solana Labs policy; rely on cert transparency + standard rustls verification. Pinning optional via `pinned://<pin>@host` URL. Q9.                  |
| 10  | `solana-test-validator` flag API unstable across Agave versions | LOW | Wrap flags behind `SolanaTestValidatorGuard` (V0.1.5 opt-in); tests don't break on version bumps.                                                                  |
| 11  | p-token rewrite could change SPL CU economics mid-release | LOW | Assume ~5,000 CU per simple SPL transfer as safe default (1000x safety margin on 150k limit). Q8.                                                                  |
| 12  | Testnet cluster confusion (dev across teams)         | MITIGATED | Cluster enum has NO `Testnet` variant. CLI does not accept `testnet`. `sol config set-cluster testnet` → `Error::InvalidCluster`. Q11.                              |
| 13  | Devnet scheduled wipes (quarterly)                   | DOCUMENTED | Treat devnet as ephemeral; never persist wallets across long time windows. Re-airdrop on wipe.                                                                      |
| 14  | Metaplex NFT license blocker                         | ACCEPTED | Defer NFT support indefinitely (V1.x at earliest). No `mpl-token-metadata` in V0.1 dep tree. Q12. `cargo deny` refuses any PR adding Metaplex.                       |
| 15  | `spl_token_2022` Token-2022 extensions not auto-dispatched in V0.1 | MITIGATED | V0.1 detects program; auto-creates ATA; `transfer_checked` works (extension-aware). Hook CPI dispatch = V0.1.5 manual append.                                       |
| 16  | MSRV bump 1.85 → 1.89.0 may break workspace contract for OTHER crates | LOW | Anza crates pin 1.89.0; sibling workspace crates may have MSRV 1.74-1.81. Bumping workspace MSRV to 1.89.0 may force rebuilds but not break code. Verify with `cargo check --workspace` after pin. |
| 17  | Mobile CI matrix has no device-level smoke gate     | MEDIUM   | V0.1 ships compile-only mobile gate (`cargo check --target aarch64-apple-ios` + `cargo check --target aarch64-linux-android`). Real device smoke deferred V0.1.5.   |
| 18  | FFI panic-message scrubber leak (mnemonic)           | MITIGATED | `regex` crate scrubber filters all STDERR panic msgs (mnemonic + secret + xprv patterns); `Zeroizing` wrap on all secrets; FFI returns exit code 99 + scrubbed msg.   |

---

## Self-Review Checklist (per /superpowers:writing-plans)

Before plan ready for execution:

- [ ] Goal is one sentence with verifiable success criteria
- [ ] Each Phase has clear Phase-N ticket pickup + acceptance criteria
- [ ] Each Phase has explicit PAUSE-before-commit per L13 step 12
- [ ] TDD applies: every Task has its failing test in test file per `## Test File Structure` before implementation
- [ ] verification-before-completion gates: every "Status: ready" claim backed by `cargo test` evidence + `cargo clippy -- -D warnings` clean
- [ ] Plugin stack ordering: brainstorming → grill-with-docs → writing-plans → to-tickets → subagent-driven-development → code-review → receiving-code-review → finishing-a-development-branch
- [ ] Cross-task dependencies: Q1 (Anza choice) blocks Phase 0 onward; Q6 (disambig) blocks Phase 4; Q4 (mainnet gate) gates Phase 9
- [ ] Risk register mirrors deep-dive risks (1-18)
- [ ] Loud-RED gate contract applied to V6, V9, V12, V14
- [ ] Phantom UX parity preserved (no custom HD wrapper; numeric `--account` + `--address-index` flags)

---

## Confidence Summary for V0.1

| Aspect                            | Confidence | Reason                                                                                          |
| --------------------------------- | ---------- | ----------------------------------------------------------------------------------------------- |
| Wire format (Pubkey, Message, Transaction) | HIGH       | Anza = official; 90%+ of stack. Stable for 12+ months                                          |
| Sign + broadcast                  | HIGH       | `RpcClient::send_transaction` + `Transaction::sign` — battle-tested in ecosystem               |
| HD derivation (SLIP-0010)         | MEDIUM     | `ed25519-bip32` 0.4.3 community crate; sole Rust option; low bus factor                        |
| Blockhash refresh retry           | MEDIUM     | Lifetime ~60-90 sec, well-known; retry pattern standard                                         |
| SPL transfer with ATA auto-create | HIGH       | `transfer_checked` + `create_associated_token_account_idempotent` — standard                    |
| Token-2022 extension dispatch     | LOW        | V0.1 supports `transfer_checked`; hook CPI dispatch = V0.1.5                                    |
| Compile targets (desktop + mobile) | HIGH       | Anza MSRV 1.89.0; PAL pattern proven by sibling chains                                          |
| Mainnet smoke gate                | MEDIUM     | Requires Alchemy/Helius API key; $0.001 USDC self-send                                          |
| Local validator (surfpool)        | HIGH       | Matches Anvil pattern from `alloy-node-bindings`; <2s boot                                     |
| FFI surface                       | HIGH       | cdylib + 12 C functions, panic-message scrubber                                                 |
| Phantom UX parity                 | HIGH       | Direct delegation to `solana_signer::Signer` trait — same surface as Phantom                   |
| **Overall V0.1 readiness**        | MEDIUM-HIGH | Standard Solana wallet surface; Anza ecosystem mature; Phantom reference API surface stable    |

V0.1 achievable in 2-3 sessions by an experienced Rust developer following this plan + V0.1.5 ticket backlog for Token-2022 hooks + durable nonces.

---

## Notes

- **`xpub` rename for Ed25519:** unlike BIP-32 secp256k1 HD chains, Ed25519 HD does NOT expose parent public key. Solana equivalent = `address pubkey` = leaf 32-byte verification key (base58). Watch-only import works for leaf pubkey + non-hardened path only. Documented gap.
- **Decimals never hardcoded:** all SPL transfers use `transfer_checked` with dynamically-fetched decimals via `spl_token::state::Mint::unpack(mint_account.data)`. Hardcoding 6 = footgun, fails for non-6-decimal mints (BONK=5, USDS=6, PYUSD=6 Token-2022, JitoSOL=9).
- **Token-2022 vs classic footgun guard:** `disambig::reject_wrong_token_program` checks `mint.owner == TokenkegQ...` (classic) vs `TokenzQdB...` (Token-2022). Mismatched `token_program_id` seed produces DIFFERENT ATA address; never auto-detect without explicit verification.
- **ATA rent pre-flight:** before any transfer, wallet checks sender SOL balance covers `amount + rent + ~0.00204 SOL/ATA + 5000 lamport base sig fee`. Insufficient = `BroadcastFailed::InsufficientFunds` (exit 3).
- **Blockhash refresh retry:** never re-sign identical bytes — signatures are nonces over the full message. Always fetch fresh blockhash + re-sign per attempt (up to 3 attempts, exponential backoff 100ms→200ms→400ms).
- **No 2-resource fee in V0.1:** Solana uses `stake::StakeState` program for native SOL staking — no energy/bandwidth model. Defer native stake ops to V0.2.
- **No NFT in V0.1:** Metaplex license blocker. Foundation never ships for non-OSI. `cargo deny` rejects any Metaplex dependency at CI time.
- **Custom HD wrapper = anti-pattern:** Phantom wallet exposes `Signer` trait (Anza), not custom HD wrapper. `Wallet::fromMnemonic` is the Phantom-equivalent of "Import secret phrase" UI action. Following Phantom shape preserves wallet portability for future Ledger HW integration.

---

## L13 Pipeline Application (per `tasks/lessons.md` L13 + plan-guide Type A)

Maps each L13 step to the superpowers + mattpocock skills invoked for this plan.

### Plugin Path Applied (Type A from plan-guide)

```text
Step   L13 step               Skill invoked                                     Where in this plan
----   ---------------------   ----------------------------------------------   ---------------------------------------------------------------
1      Pre-pickup              /superpowers:brainstorming (classification)      Phase Set Up + Phase 0 ticket pickup
2      ask-matt routing        /mattpocock-skills:ask-matt                      session start (auto via SessionStart hook)
3      Pick up issue           /superpowers:brainstorming (bounded path)        Phase Set Up Step 2 + Phase 0 Task 0.1 Step 1
3a     L11 skill-tag           /superpowers:brainstorming + L11 enumeration     (auto, pre-session hook)
4      Plan review             /superpowers:writing-plans (this doc)           THIS DOCUMENT
5      TDD red-green cycle     /superpowers:test-driven-development             per Task: write test file per `## Test File Structure` first, then impl (Phases 1-9 in order)
6      Refactor phase          /context-engineering-kit:kaizen                   Phase 7 (CLI): refactor handlers if duplication emerges
9      Subagent-driven         /superpowers:subagent-driven-development         per Phase-Ticket (Phase 0-9 each = 1 subagent dispatch)
9a     Design API surface      /mattpocock-skills:codebase-design → /mattpocock-skills:domain-modeling    Phase 1: Wallet(keypair) — Phantom-equivalent shape
11     Verify before claim     /superpowers:verification-before-completion      "Status: ready" → backed by `cargo test` + `cargo clippy -- -D warnings` evidence
12     PAUSE before commit     (manual + memory: never-auto-commit)             per Task Step "PAUSE — commit-push-pr"
13     Commit-push-pr          /superpowers:finishing-a-development-branch      per Task final step (worktree-aware merge/PR/cleanup)
14     Issue body checkboxes   (manual per L13 step 14: flip `[ ]` → `[x]`)      before squash-merge
15a    Tech doc synthesis      /mattpocock-skills:to-spec                       per L24 doc taxonomy updates
15b    L24 doc updates         (manual per L24 doc taxonomy)                    CHANGELOG update per Phase close
17     Ledger / harvest        /mattpocock-skills:grill-with-docs (round 2)     post-Phase 9 retrospective
18     Lessons harvest         /mattpocock-skills:grill-with-docs               update `tasks/lessons.md` with new lessons
19     Reports                 /mattpocock-skills:domain-modeling               update `docs/agents/domain.md` glossary with Solana chain
```

### L13 Step → Skill Cross-Reference (per plan-guide §"Project-Specific Rules")

| Constraint                                                          | Skill/MCP invocation                       | Where in this plan                                                                  |
| ------------------------------------------------------------------- | ------------------------------------------ | ----------------------------------------------------------------------------------- |
| L13 step 3: read-only task pickup                                    | `/superpowers:brainstorming` (bounded)     | Phase 0 ticket pickup                                                               |
| L13 step 9: red-green cycle                                          | `/superpowers:test-driven-development`     | Every Task: test file per `## Test File Structure` first, then impl                                          |
| L13 step 9a: new module interface                                    | `/mattpocock-skills:codebase-design`       | Phase 1: Phantom-equivalent Wallet surface                                          |
| L13 step 11: verify gate                                             | `/superpowers:verification-before-completion` | Every Task "Verify gate" + Phase 9 mainnet smoke gate                              |
| L13 step 12: PAUSE before commit                                     | (manual + memory: never-auto-commit)       | Every Task final step PAUSE                                                          |
| L13 step 13: commit-push-pr                                          | `/superpowers:finishing-a-development-branch` | Phase-0-9 close PRs                                                                 |
| L13 step 14: flip issue checkboxes                                   | (manual)                                    | Each PR squash-merge                                                                 |
| L13 step 15a: tech doc                                               | `/mattpocock-skills:to-spec`               | Phase 9.2 Step 1 → CHANGELOG                                                         |
| Never-auto-commit (CLAUDE.md)                                        | PAUSE before `git commit`                   | Every Task final step PAUSE                                                          |
| Update-issues-before-merge (CLAUDE.md)                              | flip `[ ]` → `[x]` BEFORE squash-merge      | Each PR                                                                              |
| GateGuard `gh-pr classifier` (CLAUDE.md + memory)                   | `--body-file` with content in `/tmp`        | Every PR creation (not inline)                                                      |
| `cargo deny` enforcement (repo policy)                              | `cargo deny check` in CI                    | Phase Set Up Step 5 + every PR (blocks Metaplex Q12)                                |

### Per-Step Status (filled at Phase close)

| Phase | Status | Phase file | Notes |
| ----- | ------ | ----- | ----- |
| Set Up | pending | n/a | branch + labels + CI workflow |
| Phase 0 | pending | V1 (compile + help) | scaffold + Anza pinning verified |
| Phase 1 | pending | V2, V3, V8 | Phantom-equivalent Wallet + HD + signing |
| Phase 2 | pending | V4 | base58 round-trip + is_on_curve |
| Phase 3 | pending | V5 | tx::builder SOL + Compute Budget |
| Phase 4 | pending | V9, V10 | SPL transfer_checked + ATA disambig + decimals |
| Phase 5 | pending | V6 | RPC + send_with_retry + wait_for_confirm |
| Phase 6 | pending | V11 | Wallet persistence + WalletManager CRUD |
| Phase 7 | pending | V14, V15 | CLI 22 commands + surfpool e2e |
| Phase 8 | pending | ffi_smoke | FFI cdylib + 12 C functions + panic scrubber |
| Phase 9 | pending | V12 | mainnet $0.001 USDC self-send (loud-RED gate) |

---

## Conventions

Per `tasks/lessons.md` L24 doc taxonomy + CLAUDE.md project rules:

- **Docs/research layer** (`docs/`): no build, no tests, no lint. Markdown only.
- **Code layer** (`rust-wallet-app/`): standard Rust tooling — `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo geiger`, `cargo deny`.
- **Filenames:** `YYYY-MM-DD-<topic>.md`; ADRs `YYYY-MM-DD-adr-NNNN-<title>.md` (NNNN zero-padded, monotonic). Example: `docs/wallets/2026-09-09-adr-0002-sol-sdk-anza-vs-raw.md` (placeholder for post-Phase-9 ADR capturing the Anza-only decision).
- **Cross-SDK comparison tables:** one index + per-area reports, column per SDK.
- **Use case coverage matrices:** link each user story to the SDK primitive that fulfils it.
- **ADRs:** capture decision + rejected alternatives, not just the chosen path (post-Phase-9, capture the Anza-stack vs custom-crypto reversal with Phantom-equivalent rationale).
- **Plan review before commit:** re-read `docs/superpowers/plans/*.md`; flag drift.
- **Research methodology:** parallel Agent subagents (one per area) + exa/firecrawl MCP web sources. Each finding cites its source.
- **No invented content:** every claim links back to a source file or external URL.
- **Naming:** Phantom-equivalent methods use camelCase (`fromMnemonic`, `signTransaction`) — match Phantom JS SDK surface.
- **Per-Phase workflow** (per `/mattpocock-skills:implement` per `.local/plugins-docs/2026-08-31-mattpocock-skills-deepdive.md`):
  1. Open Phase N's file list (Phase Set Up / Phase N Test File Structure mapping)
  2. Drive `/mattpocock-skills:tdd` slices at pre-agreed seams (one red-green cycle per Task)
  3. Typecheck regularly (`cargo check -p sol-wallet-core` per source Step)
  4. Single test file regularly (`cargo test -p sol-wallet-core --test <descriptive-name>` per test Step)
  5. Full test suite at Task end (`cargo test -p sol-wallet-core` per verify gate)
  6. `/mattpocock-skills:code-review` (Standards + Spec axes run separately; aggregate under separate headings; do not merge; do not rerank; 12-smell Fowler baseline)
  7. Commit on current branch (L13 step 13: PAUSE before commit per `never-auto-commit`; `gh pr create --body-file` per `gate-guard-gh-pr-classifier` memory)
  8. PAUSE for operator review at Phase boundary (L13 step 15: PR review + merge + close)
- **Phase boundaries** (per deep-dive §"Phase boundaries"): 5 options at phase close — Continue / `/clear` / `/handoff` / subagent / `/compact`. Decision order: Continue (free) → subagent (mid-phase) → `/compact` (at boundary, default) → `/handoff` (only for new harness/dir/colleague or side-task fork) → `/clear` (last, when nothing here matters).
- **Ed25519 terms:** use "SLIP-0010" not "BIP-32" (Anza explicitly excludes BIP-32 from scope); use "Ed25519" not "EdDSA" (Anza-precise).
- **HD path:** never expose as string to user; format-construct `m/44'/501'/{account}'/0'/{address_index}` internally only.

---

## v0.1 Release Status (target)

**Pre-release gate (`v0.1.0-rc.1`):** all Phase 0-9 tasks complete; Test scenario file structure full PASS (32 library + 10 CLI test files green); cross-crate feature map confirms >95% coverage; mainnet smoke gate cleared.

**Release gate (`v0.1.0`):**

| Surface                          | Status target                                                | Verification evidence                                              |
| -------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------ |
| `cargo build -p sol-wallet-core` | exit 0 on Linux x86_64 + macOS aarch64 + Windows x86_64      | CI matrix green                                                    |
| `cargo test -p sol-wallet-core`  | all 32 library test files GREEN per `## Test File Structure`; loud-RED gated tests honoured (devnet + mainnet smoke, transport timeout)              | CI matrix green + operator-run mainnet smoke                       |
| `cargo clippy --all-targets`     | `-- -D warnings` clean                                        | CI matrix green                                                    |
| `cargo fmt`                       | no diff                                                       | CI matrix green                                                    |
| `cargo deny check`               | no license violations (no Metaplex per Q12)                  | CI matrix green                                                    |
| `cargo check --target aarch64-apple-ios` | exit 0 (mobile compile-only gate per Q17)            | CI matrix green                                                    |
| `cargo check --target aarch64-linux-android` | exit 0 (mobile compile-only gate per Q17)         | CI matrix green                                                    |
| `cargo publish --dry-run -p sol-wallet-core` | succeeds                                              | operator local                                                     |
| `sol --version`                  | prints `sol 0.1.0`                                            | local smoke                                                        |
| `sol wallet create` end-to-end   | exits 0; mnemonic → STDERR; wallet_id → STDOUT               | local + devnet verification                                        |
| `sol wallet send --amount 0.001 --token USDC --wait --wait-finalized --confirm-mainnet` | txid returned with `finalized` commitment; balance changed by 0.001 USDC | `RUN_SOL_MAINNET=1` operator-run smoke (V12)                       |
| ADR-0002 published               | captures Anza-stack vs raw-crypto reversal + Phantom UX parity rationale | `docs/wallets/2026-09-09-adr-0002-sol-sdk-anza-vs-raw.md`   |
| CHANGELOG entry                  | V0.1.0 release notes with 9 phase close-outs                 | `rust-wallet-app/crates/sol-wallet-core/CHANGELOG.md`              |

**Acceptance Criteria (issue body flip gate):**

Each phase-ticket issue body contains checkboxes like:

```markdown
## Acceptance Criteria
- [ ] Test scenario file PASS (`cargo test --test <descriptive-name>` per `## Test File Structure`)
- [ ] `cargo fmt -- --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] Loud-RED gate contracted (if V6/V9/V12/V14)
- [ ] PR body cites Qn grill Round-1 decision
- [ ] Issue body checkboxes flipped to `[x]` BEFORE squash-merge (L13 step 14)
```

Phase ticket closed ONLY when ALL `[x]`.

---

## Open Questions / Future Plan Adenda

| Q# | Topic | When |
| --- | ----- | ---- |
| Q13 | Token-2022 Transfer Hook CPI dispatch implementation strategy (manual ix append vs SDK wrap) | V0.1.5 ticket |
| Q14 | ALTs (Address Lookup Tables) integration — V0.1.5 vs V0.2 | V0.1.5 ticket |
| Q15 | Durable nonces for offline signing >90s | V0.1.5 ticket |
| Q16 | Stake account creation + delegate (native SOL staking) | V0.2 ticket |
| Q17 | Thread model (FFI deadlock / Zeroizing-across-await / Send+Sync on keypair) | V0.2 ticket (independent design review) |
| Q18 | Ledger HW wallet integration (`@ledgerhq/solana` JS or Rust SDK) | V1.x ticket |
| Q19 | Confidential transfer (Token-2022 zk proofs via `solana-zk-sdk` 7.0.1) | V0.3 ticket |
| Q20 | ADR-0002 publication timing (post-Phase-9 vs paired with V0.1 release) | Phase 9.2 decision |

---

## Plan Cross-Reference

| Compared Section             | TRON plan (2026-09-05)                                        | SOL plan (this doc, 2026-09-09)                                  | Status                                                       |
| ---------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------- | ------------------------------------------------------------ |
| **Pre-banner**               | subagent sub-skill banner + revision history block           | subagent sub-skill banner + plan-guide Type A reference          | ✅ Match (SOL = first cut, no revision history yet)           |
| **Goal**                     | one sentence with verifiable criteria                         | one sentence with verifiable criteria                            | ✅ Match                                                       |
| **Companion docs**           | research + user stories + ADR + CHANGELOG + estimate + cost + supersedes | research + audit + workflow + per-crate + lessons            | ⚠️ Missing: CHANGELOG pointer (added at Phase 9.2 Step 1 reference); estimate/cost reports N/A for first cut; supersedes N/A |
| **Tracks**                   | issue #399 + PR #402 (Ticket A-F)                             | TBD issue + Ticket B-J                                          | ⚠️ TBD (issues created in Phase Set Up)                       |
| **Decisions**                | grill Round 1 Q1-Q13                                          | grill Round 1 Q1-Q12                                             | ✅ Match (SOL = 12 questions vs TRON's 13)                    |
| **Why** subsection           | (implicit)                                                    | "Why Anza-only"                                                 | ✅ SOL has explicit (TRON = implicit in revision history)      |
| **Global Constraints**       | Q1-Q13 verbatim from deep-dive grill                          | Q1-Q12 verbatim from deep-dive grill                             | ✅ Match                                                       |
| **Architecture**             | Five-layer PAL (extra "model versioning" for ABI)             | Four-layer PAL (matches Bitcoin/btc-wallet-core standard)        | ✅ Match (chain-appropriate difference)                        |
| **F47 zeroize gap**          | (per-chapter inline)                                          | dedicated table                                                 | ✅ SOL = explicit table (better than TRON)                     |
| **File Structure**           | detailed tree                                                | detailed tree                                                   | ✅ Match                                                       |
| **Phases**                   | Phase Set Up + 0-7 (8 phases)                                 | Phase Set Up + 0-9 (10 phases)                                  | ✅ SOL = more granular (9 impl phases vs TRON's 7)            |
| **Tasks per phase**          | files / interfaces / verify gate / PAUSE                      | files / steps / verify gate / PAUSE                             | ✅ Match                                                       |
| **Loud-RED Gate**             | dedicated section                                            | dedicated section                                               | ✅ Match                                                       |
| **Risk Register**            | table                                                         | table (18 risks)                                                | ✅ Match (SOL = 18 vs TRON = fewer; SOL more comprehensive)    |
| **Conventions**              | dedicated section                                            | dedicated section                                               | ✅ Match (just added)                                         |
| **V0.1.5 deferred**          | table                                                         | table                                                           | ✅ Match                                                       |
| **V0.2+ deferred**           | table                                                         | table                                                           | ✅ Match                                                       |
| **NEVER out of scope**        | table                                                         | table                                                           | ✅ Match                                                       |
| **Self-Review Checklist**    | (implicit per acceptance criteria)                            | dedicated checklist                                             | ✅ SOL = more explicit                                         |
| **L13 Pipeline Application** | dedicated section                                            | dedicated section (just added)                                  | ✅ Match (just added)                                         |
| **Acceptance Criteria**      | dedicated section                                            | dedicated section (just added)                                  | ✅ Match (just added)                                         |
| **v0.1 Release Status**       | dedicated section                                            | dedicated section (just added)                                  | ✅ Match (just added)                                         |
| **Cost Estimates**           | dedicated section (operator-local)                           | not included                                                    | ⚠️ N/A for first cut (operator can add as needed)              |
| **Open Questions**           | (implicit per ADR-0001 reversal)                              | dedicated section (just added)                                  | ✅ SOL = more explicit                                         |

**Match verdict:** Core framework ✅ identical. SOL plan = TRON framework + added explicitness (Confidence Summary, Self-Review Checklist, Conventions, L13 Pipeline Application, Acceptance Criteria, v0.1 Release Status, Open Questions) + chain-appropriate divergence (Four-layer PAL vs Five-layer; Ed25519 + SLIP-0010 vs secp256k1 + BIP-32; 9 phases vs 7; Phantom UX parity vs raw anychain-vendored). All missing/weak sections added in this verification pass.
