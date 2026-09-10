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
- **Phase 5** = RPC client (solana-client::RpcClient) + `send_with_retry` (3 attempts, exponential backoff) + `wait_for_confirm`.
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
| Q7  | Blockhash retry                     | **(a) `send_with_retry`** — 3 attempts max, exponential backoff 100ms→200ms→400ms, fresh blockhash each retry, re-sign each attempt. Ed25519 signs `recent_blockhash` directly, signatures are nonces over full message. |
| Q8  | Compute Budget defaults             | **(a) 150_000 CU default limit + 0 priority fee** — 1000x safety margin for p-token rewrite 2026 (~5 CU per simple transfer). Auto-set 200_000 CU for composite tx (memo + 2 ATA creates). |
| Q9  | SPKI pinning                        | **(a) Scenario B default** (no pin, Solana Labs public RPC rotates certs freely); Scenario A opt-in via `pinned://<pin>@host` URL for paid Helius/QuickNode/Alchemy. Reuse `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` verbatim. |
| Q10 | Decimals                            | **(a) NEVER hardcoded** — always `spl_token::state::Mint::unpack(mint_account.data).decimals` + `transfer_checked` (NOT `transfer`). Prevents USDC=6 / BONK=5 / USDS=6 mismatch. |
| Q11 | Cluster coverage                    | **(a) `MainnetBeta / Devnet / Localnet` ONLY** — Solana testnet DEPRECATED 2022-23 (Foundation abandoned). `Cluster` enum has NO `Testnet` variant; `sol config set-cluster testnet` → `Error::InvalidCluster`. |
| Q12 | Token program support scope         | **(a) V0.1 = classic SPL + Token-2022 awareness (footgun guard)** — `transfer_checked` works for both programs. NFT/Metaplex EXCLUDED (license blocker). Token-2022 extension-specific transactions (transfer hook CPI, confidential proofs) deferred V0.1.5+. |

### Why Anza-only (Anza vs custom crypto)

**Phantom UX parity argument:** Phantom is the reference Solana wallet; users expect `--account` + `--address-index` flags (not `m/44'/501'/0'/0'/N`). Phantom exposes `Signer` trait (via `solana-signer`) + `Keypair` (via `solana-keypair`) + `Pubkey` (via `solana-sdk::pubkey`) — same surface. Why rewrite what Phantom already gets right via stable Anza crates?

**Bus-factor (accepted):** Anza holds 90%+ of the stack (SDK + agave validator + SPL org). Single-vendor trust accepted per Round-1 grill finding 2026-09-08. Apache-2.0 license, ~3-month release cadence, ~30 contributors, actively maintained. **Mitigation = monitor `anza-xyz/agave` security advisories.** No vendoring needed (unlike TRON 2026-09-06 reversal — that was bus-factor = 1 plus active varint bug; Anza has ~30 contributors so cost/benefit is different).

**Anza subcrate version drift audit:** `solana-sdk` = 4.1.0, `solana-message` = 4.6.0, `solana-transaction` = 4.3.0, `solana-keypair` = 3.1.2, `solana-signer` = 3.0.1, `solana-instruction` = 3.5.0 — desynced versions. **MUST pin each individually** with exact `=x.y.z` (NOT caret `^x.y`). Cargo's facade re-export does NOT unify subcrate versions. Verified via `cargo tree -p sol-wallet-core | grep solana-` in Task 0.1 verification.

---

## Global Constraints (verbatim from deep-dive Round-1 grill Q1-Q12)

- **Q1 — SDK choice.** Anza stack = `solana-sdk` 4.1.0 (facade: Keypair, Pubkey, Message, Transaction) + `solana-client` 4.2.2 (RpcClient + PubsubClient) + `solana-program` 4.1.0 (PDA + sysvar + hash) + SPL programs (`spl-token` 9.0.0 classic + `spl-token-2022` 11.0.0 extensions + `spl-associated-token-account` 8.0.0 + `spl-memo` 7.0.0) + `ed25519-bip32` 0.4.3 (SLIP-0010 HD only). Anza = official; 90%+ of stack; active maintenance; Apache-2.0. Bus-factor accepted. **REJECTED:** `mpl-token-metadata` (Metaplex NFT Open Source License v1.0 — non-OSI blocker, deferred indefinitely), `jito-labs/jito-rust-rpc` (stale 2025-06-21, gate behind `jito` Cargo feature if ever needed), `qntx/kobe` (Jito MEV research repo, name collision — NOT a wallet lib), `CRossel87a/solana-light-client` (immature, ~30 all-time downloads), `SergioBenitez/Figment` (config-only, not crypto).
- **Q2 — HD coverage gap.** `solana-sdk` 4.1.0 does NOT cover BIP-32 / SLIP-0010 (Anza explicitly excludes HD derivation from scope; Phantom wallet solves this with a separate crate). **`ed25519-bip32` 0.4.3** (typed-io, MIT/Apache-2.0) provides SLIP-0010 Ed25519 chain key derivation. Phantom-equivalent Wallet API internally: `bip39::Mnemonic::from_phrase` → `bip39::Seed::new(&m, "")` → `ed25519_bip32::XPrv::from_seed(seed)` → `XPrv::derive("m/44'/501'/.../0'/0")` → `solana_sdk::Keypair::try_from(seed_bytes)`. NO custom HD wrapper module exposed in `sol-wallet-core`.
- **Q3 — Derivation path.** Phantom convention = `m/44'/501'/{account}'/0'/{address_index}` (SLIP-44 coin 501 = SOL). Hardened only at `44'`, `501'`, `account'`; non-hardened at `0'` and final `address_index`. CLI flags numeric: `--account <N>` (default 0) + `--address-index <N>` (default 0). Path string NEVER exposed (Ledger-style ZIP-32 users → `sol keygen raw` V0.2 deferred).
- **Q4 — Mainnet smoke gate.** **V0.1 release GATED on one mainnet self-send — $0.001 USDC to self (recipient == sender), real value, real network.** Local + devnet is emulation. Without a real-value smoke, test scenario PASS evidence = "looks like real network" not "real network". Operator checklist: Alchemy API key loaded, operator wallet has ≥ 0.01 USDC + 0.01 SOL for fees + rent, `--confirm-mainnet` prompt typed `yes`, `RUN_SOL_MAINNET=1` env var set, `sol wallet send --to <self> --amount 0.001 --token USDC --wait --wait-finalized --priority-fee 1000`. Verify via `sol balance --address <self> --token USDC` — balance changed by 0.001 USDC.
- **Q5 — Local validator.** `surfpool` (txtx/surfpool, 2025+) sub-second boot, in-memory, no Docker, runtime reset (`surfpool reset`), warp slot (`surfpool warp --slot N`), built-in faucet (`surfpool airdrop`). Spawned as subprocess via `tokio::process::Command` in `tests/common/surfpool_guard.rs::SurfpoolGuard` RAII wrapper (ephemeral port, 10s health-poll deadline). `solana-test-validator` DEFERRED to V0.1.5 opt-in for BPF + epoch boundary tests.
- **Q6 — Token-2022 disambig.** ATA derivation seed = `[owner, token_program_id, mint]`. `token_program_id` IS a seed → classic SPL (`TokenkegQ...`) and Token-2022 (`TokenzQdB...`) produce DIFFERENT ATA addresses for same `(owner, mint)`. `disambig::reject_wrong_token_program` checks `mint.owner` via `getAccountInfo`. Mix detected → `Error::InvalidTokenProgram`. Wallet derives correct ATA programmatically; caller NEVER passes wrong `token_program_id`. **Same ATAs do NOT equal Token-2022 ATAs** — surface this rule in CLI docs.
- **Q7 — Blockhash retry.** Blockhash lifetime ~60-90 sec (~150 slots at 400ms). Ed25519 signs `recent_blockhash` directly → signature covers blockhash + message. Re-sign identical bytes NEVER works (signature is nonce over full message). `tx::broadcast::send_with_retry`: 3 attempts max, fresh blockhash each retry, exponential backoff 100ms→200ms→400ms. `BlockhashNotFound` and `BlockCleanedUp` → retry, NOT user-facing error.
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
│   │   └── broadcast.rs                    # submit_sol, submit_spl, submit_sol_speedup, simulate, send_with_retry, wait_for_confirm
│   ├── chain/
│   │   ├── mod.rs
│   │   ├── client.rs                       # SolanaClient (wraps RpcClient); request_airdrop, get_latest_blockhash, get_balance, get_account_info, get_token_account_balance, get_minimum_balance_for_rent_exemption, get_token_accounts_by_owner, get_recent_prioritization_fees
│   │   ├── account.rs                      # discover_atas, fetch_token_supply, fetch_decimals, mint_token_program, derive_ata_with_program_id
│   │   └── pki.rs                          # SpkiPinnedVerifier (re-export bitcoin-wallet-core::chain::spki)
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
├── rpc_methods_mock.rs               # Phase 5.1: row 22 — 12 RPC methods against wiremock (note: deep-dive §I. enumerates 21; Phase 5.1 expands to full 21 inline)
├── error_mapping.rs                  # Phase 5/6: row 23 — Error From + Debug redaction
├── placeholder.rs                    # Phase 8.1: rows 24, 25 — FFI panic scrubber + C ABI smoke
├── submit_sol_local.rs               # Phase 7.2: row 26 — submit_sol E2E on surfpool
├── submit_spl_local_held.rs          # Phase 7.2: row 27 — submit_spl E2E on held ATA (mock USDC)
├── submit_spl_local_fresh.rs         # Phase 7.2: row 28 — submit_spl E2E on fresh ATA (rent delta)
├── submit_spl_local_approve.rs      # Phase 7.2: row 29 — submit_spl_approve E2E + allowance view
├── send_with_retry.rs                # Phase 5.1: rows 30, 31 — send_with_retry on stale blockhash + wait_for_confirm
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
| **Phase 5.1** (RPC + retry)                               | `send_with_retry` (1)                                                                                          | 1  |
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
- [x] Step 6: Verify Anza subcrate pinning — `cargo tree -p sol-wallet-core | grep solana-` shows exact `=x.y.z` pins, NO version unification **DEFERRED to Phase 1** — crates.io drift: `solana-rpc-client = "=4.2.2"` is not published (only `4.4.0-alpha.3` exists, and its manifest pins `solana-instruction >=3.4.0, <3.5.0` — incompatible with the plan's `=3.5.0`). All Anza + SPL + ed25519-bip32 pins are declared in workspace `[workspace.dependencies]` but NOT wired into `sol-wallet-core/Cargo.toml` so the build resolves; Phase 1 uncomments the Anza block at the top of that file, runs `cargo tree -p sol-wallet-core | grep solana-` to discover the actual constraint graph, picks compatible exact pins, then verifies "no version unification" on its own build before claiming done. Full drift log in `crates/sol-wallet-core/CHANGELOG.md` Phase 0 section.
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
- [ ] Step 1: Implement `Wallet(solana_sdk::signature::Keypair)` tuple struct + Phantom-equivalent API per architecture section
- [ ] Step 2: Implement `fromMnemonic(phrase: &str) -> Result<Self>` → delegates to `bip39::Mnemonic::from_phrase` + `bip39::Seed::new` + `ed25519_bip32::XPrv::from_seed` + `XPrv::derive("m/44'/501'/0'/0'/0")` + `solana_sdk::Keypair::try_from(seed_bytes)`
- [ ] Step 3: Implement `fromMnemonicAt(phrase, account, address_index)` → same chain with `m/44'/501'/{account}'/0'/{address_index}` path string built via `format!`
- [ ] Step 4: Wrap seed + xprv in `Zeroizing<Vec<u8>>` / `Zeroizing<String>` during derivation; drop after `Keypair::try_from` consumes bytes
- [ ] Step 5: Implement `tests/address_derivation.rs` — Mnemonic("abandon ×11 about") → base58 address matches Phantom canonical (cross-verify via Phantom's documented vector or `solana-keygen pubkey "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" prompt://`); same mnemonic with `--address-index 0` and `--address-index 1` produces DIFFERENT addresses (proves HD chain key works); `is_on_curve` true; accept known devnet address `2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH`; reject invalid base58 (`not-base58!!!`); reject off-curve bytes. Also implement `tests/bip39_mnemonic.rs` — assert 12/15/18/21/24-word English mnemonic validity; reject 11-word / 25-word; reject non-English wordlist; reject checksum-failing phrase.
- [ ] Step 6: Verify gate: `cargo fmt + cargo clippy -p sol-wallet-core -- -D warnings + cargo test -p sol-wallet-core --test address_derivation --test bip39_mnemonic` (SLIP-0010 + HD multi-index + English wordlist acceptance)
- [ ] Step 7: PAUSE — commit-push-pr; PR body cites Q1+Q2+Q3 from grill Round-1

### Task 1.2 (TBD): Wallet::fromBase58(secret) + fromPublicKey(pubkey) + sign APIs

**Files:**
- Modify: `rust-wallet-app/crates/sol-wallet-core/src/wallet.rs`
- Create: `rust-wallet-app/crates/sol-wallet-core/src/read_only_wallet.rs` (separate file for ReadOnlyWallet struct)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/sign_tx.rs` (deep-dive row 16 — full sign + send with recent_blockhash)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/sign_only.rs` (deep-dive row 17 — `sign_only_tx` cold path)

**Steps:**
- [ ] Step 1: Implement `Wallet::fromBase58(secret: &str) -> Result<Self>` → delegates to `solana_sdk::Keypair::from_base58_string`; canonical 64-byte base58 secret (32-byte secret + 32-byte pubkey)
- [ ] Step 2: Implement `Wallet::fromPublicKey(pubkey: Pubkey) -> ReadOnlyWallet` — does NOT consume signing material; delegates to `solana_sdk::Pubkey::from_str`
- [ ] Step 3: Implement `ReadOnlyWallet(Pubkey)` with `pubkey(&self) -> Pubkey` getter ONLY (no sign methods)
- [ ] Step 4: Implement `Wallet::signTransaction(&self, tx: VersionedTransaction)` → delegates to `tx.sign(&[keypair], tx.message.recent_blockhash())` (Anza `Signer::sign_transaction`)
- [ ] Step 5: Implement `Wallet::signMessage(&self, msg: &[u8]) -> Signature` → delegates to `keypair.sign_message(msg)` (Anza `Signer::sign_message`)
- [ ] Step 6: Implement `Wallet::publicKey(&self) -> Pubkey` → delegates to `Signer::pubkey`
- [ ] Step 7: Implement `tests/sign_tx.rs` (row 16) — sign arbitrary `Transaction`; verify via `solana_sdk::transaction::verify`; non-default fee-payer reflected in signature count
- [ ] Step 8: Implement `tests/sign_only.rs` (row 17 — `sign_only_tx` cold path) — `sign_only_tx` cold path: returns `(tx_base64, sig)`; performs no RPC; re-signing same input deterministic; sign arbitrary 32-byte message; verify recovered pubkey via `solana_sdk::signature::Signature::verify`
- [ ] Step 9: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test sign_tx --test sign_only` (sign-only acceptance)
- [ ] Step 10: PAUSE — commit-push-pr; PR body cites Q2 (HD coverage) + Q3 (Phantom UX parity)

---

## Phase 2 — Address surface (base58, is_on_curve, PDA)

### Task 2.1 (TBD): Address module + base58 round-trip + is_on_curve

**Files:**
- Create: `rust-wallet-app/crates/sol-wallet-core/src/address.rs`
- Modify: `src/lib.rs` (`pub mod address;`)
- Modify: `rust-wallet-app/crates/sol-wallet-core/tests/address_derivation.rs` (Phase 1.1 file; add base58 round-trip + invalid base58 reject + off-curve bytes reject cases; deep-dive row 2 part)

**Steps:**
- [ ] Step 1: Implement `pubkey_from_bytes(bytes: [u8; 32]) -> Pubkey` → `solana_sdk::Pubkey::new_from_array`
- [ ] Step 2: Implement `pubkey_to_base58(pk: &Pubkey) -> String` → `pk.to_string()` (32-44 chars, no prefix)
- [ ] Step 3: Implement `is_on_curve(bytes: &[u8]) -> bool` → `solana_sdk::Pubkey::is_on_curve` (rejects PDA-from-bytes footgun — PDA may NOT be on Ed25519 curve)
- [ ] Step 4: Implement `parse_user_address(s: &str) -> Result<Pubkey>` → `Pubkey::from_str(s)` + `is_on_curve` check; reject invalid base58 + reject off-curve bytes
- [ ] Step 5: Implement `find_pda(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8)` → `Pubkey::find_program_address` (V0.1.5 staking use; V0.1 internal)
- [ ] Step 6: Extend `tests/address_derivation.rs` — bytes → base58 → bytes round-trip; assert accepts known valid address `2mcFPzAo2kfHkNyNgAniGZvdPYn3kNeJjPV1rCAb5NAH`; reject invalid base58 (`not-base58!!!`); reject off-curve bytes (e.g. all-zeros); accept known SHA-2/256-bip44 vector from Phantom canonical
- [ ] Step 7: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test address_derivation`
- [ ] Step 8: PAUSE — commit-push-pr

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
- [ ] Step 1: Implement `build_sol_transfer(from: &Pubkey, to: &Pubkey, lamports: u64) -> Vec<Instruction>` → `system_instruction::transfer(from, to, lamports)`
- [ ] Step 2: Implement `prepend_compute_budget(units: u32, micro_lamports: u64) -> [Instruction; 2]` → `[ComputeBudgetInstruction::set_compute_unit_limit(units), ComputeBudgetInstruction::set_compute_unit_price(micro_lamports)]`
- [ ] Step 3: Implement `build_sol_transfer_with_budget(from, to, lamports, cu_limit, priority_fee) -> Vec<Instruction>` → calls both; returns 3-ix vector
- [ ] Step 4: Wire defaults: `cu_limit = 150_000`, `priority_fee = 0` (Q8) per `SolanaConfig`; read full per-tx-type 6-row defaults table from deep-dive §C (line 1592) before Phase 5 integration
- [ ] Step 5: Implement `tests/tx_serde.rs` — bincode `Message::new_with_blockhash(&[ix, cu_ix], Some(&from), &blockhash)` round-trip; decoded message equals input for every builder (100% builder coverage)
- [ ] Step 6: Implement `tests/amount_lamport.rs` — `Amount::from_lamports(u64)` + `as_lamport()` + `Amount::from_sol(f64)`; proptest round-trip via `proptest!`; reject overflow at `u64::MAX`; assert `Amount::ZERO.lamports() == 0`
- [ ] Step 7: Implement `tests/compute_budget.rs` — against surfpool; fresh blockhash → sign with keypair → send_tx → assert `get_balance` recipient == lamports transferred - 5000 base fee; verify auto-attach prepends ComputeBudget ix in correct position
- [ ] Step 8: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test amount_lamport --test tx_serde --test compute_budget`
- [ ] Step 9: PAUSE — commit-push-pr

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
- [ ] Step 1: Implement `detect_token_program(mint: &Pubkey) -> TokenProgram` → `getAccountInfo(mint).owner` returns `TokenkegQ...` (`Classic`) or `TokenzQdB...` (`Token2022`); error otherwise
- [ ] Step 2: Implement `derive_ata_with_program_id(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey` → `spl_associated_token_account::get_associated_token_address_with_program_id`
- [ ] Step 3: Implement `prepend_create_ata(payer: &Pubkey, owner: &Pubkey, mint: &Pubkey) -> Instruction` → `spl_associated_token_account::instruction::create_associated_token_account_idempotent(payer, owner, mint)`
- [ ] Step 4: Implement `build_spl_transfer_checked(from, to, mint, amount, decimals) -> Vec<Instruction>` → `spl_token::instruction::transfer_checked(token_program, source, mint, dest, authority, signer, amount, decimals)` (Q6 — use detected `token_program_id`)
- [ ] Step 5: Implement `build_spl_approve(owner, delegate, mint, amount)` → `spl_token::instruction::approve`
- [ ] Step 6: Implement `build_spl_close_account(owner, ata, recipient_rent)` → `spl_token::instruction::close_account`
- [ ] Step 7: Implement `fetch_decimals(mint, rpc_client) -> u8` → `spl_token::state::Mint::unpack(mint_account.data).decimals` (classic) OR `spl_token_2022::state::Mint::unpack(mint_account.data).decimals` (Token-2022); per Q10 NEVER hardcoded
- [ ] Step 8: Implement `disambig::reject_wrong_token_program(mint, attempted_program) -> Result<()>` — if `mint.owner ≠ attempted_program`, error with `Error::InvalidTokenProgram`
- [ ] Step 9: Implement `tests/token2022_disambig.rs` — classic + Token-2022 mint detection via mock `getAccountInfo`; assert classic ATA ≠ Token-2022 ATA for same `(owner, mint)`; `Mint::unpack(mock_account.data).decimals` returns 6 for USDC-shaped, 5 for BONK-shaped; `disambig::reject_wrong_token_program(USDC mint, Token2022 program)` errors with `Error::InvalidTokenProgram`
- [ ] Step 10: Implement `tests/stablecoin_registry.rs` — bundle `tokens/mainnet.json` via `include_str!`; `tokens::by_symbol("USDC")` returns mainnet USDC mint pubkey; `decimals_for_mint(USDC)` returns 6; unknown symbol returns `None`; PYUSD + USDT + USDS all parse from registry
- [ ] Step 11: Implement `tests/spl_instruction.rs` — `build_spl_transfer_checked` round-trip via bincode; `prepend_create_ata` + `build_spl_transfer_checked` = 2 instructions in correct order; rejected `transfer_checked` with mismatched decimals returns builder error
- [ ] Step 12: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test token2022_disambig --test stablecoin_registry --test spl_instruction`
- [ ] Step 13: PAUSE — commit-push-pr; PR body cites Q6 (Token-2022 disambig) + Q10 (decimals never hardcoded)

---

## Phase 5 — RPC client + send_with_retry + wait_for_confirm

### Task 5.1 (TBD): SolanaClient wrapper + send_with_retry retry policy + blockhash refresh

**Files:**
- Create: `src/chain/mod.rs`
- Create: `src/chain/client.rs` (SolanaClient wrapping `solana_client::nonblocking::rpc_client::RpcClient`)
- Modify: `src/tx/broadcast.rs` (`send_with_retry` + `wait_for_confirm`)
- Modify: `src/tx/mod.rs`
- Create: `src/chain/account.rs` (delegate to Anza `RpcClient`; full 21-method coverage per deep-dive §I. line 1792: 16 HTTP + 5 WS)
- Create: `src/chain/pki.rs` (SpkiPinnedVerifier, re-exports `bitcoin-wallet-core::chain::spki`)
- Create: `tests/blockhash_cache.rs` (Phase 5.1 owns row 12 — recent blockhash TTL cache)
- Create: `tests/rpc_methods_mock.rs` (Phase 5.1 owns row 22 — all 21 RPC methods vs `wiremock` stubs; HTTP 5xx → `Error::Transport`, never panic)
- Create: `tests/preflight_balance.rs` (Phase 5.1 owns row 19 — mock RpcClient returning fixed balance; under-funded → `Error::InsufficientFunds` before broadcast)
- Create: `tests/tx_status_parse.rs` (Phase 5.1 owns row 20 — `TransactionStatus` JSON → struct; success parses slot + confirmations; error variant maps per `J. Error classification`)
- Create: `tests/transport_failure.rs` (Phase 5.1 owns row 34 — RPC pointed at closed port → `Error::Transport` within 30s timeout)
- Create: `tests/send_with_retry.rs` (deep-dive rows 30+31 — `send_with_retry` stale-blockhash + `wait_for_confirm` success + bogus-sig `ConfirmTimeout`)
- Create: `rust-wallet-app/crates/sol-wallet-core/tests/common/surfpool_spawn.rs` (Phase 5.1 owns `spawn_surfpool(port) -> Child`)

**Steps:**
- [ ] Step 1: Implement `SolanaClient` wrapping `solana_client::nonblocking::rpc_client::RpcClient`; supports `new(url)`, `new_with_commitment(url, commitment)`
- [ ] Step 2: Implement all **21 RPC methods** per deep-dive §I.: `get_latest_blockhash`, `get_balance`, `get_account_info`, `get_multiple_accounts_info`, `get_minimum_balance_for_rent_exemption`, `get_token_account_balance`, `get_token_supply`, `get_token_accounts_by_owner`, `request_airdrop`, `get_health`, `get_recent_prioritization_fees`, `get_version`, `get_epoch_info` (16 HTTP) + `account_subscribe` / `signature_subscribe` / `program_subscribe` / `logs_subscribe` / `slot_subscribe` (5 WS — internal use only V0.1; CLI does not expose) — all delegate to Anza `RpcClient` + `PubsubClient` (no custom JSON-RPC envelope code)
- [ ] Step 3: Implement `tx::broadcast::send_with_retry(rpc, keypair, message, max_attempts=3)`:
  - For attempt in 1..=max_attempts:
    - Fetch FRESH blockhash via `rpc.get_latest_blockhash()`
    - Re-build `Transaction::new_unsigned(message)` with new blockhash
    - `tx.sign(&[keypair], new_blockhash)` (Ed25519 signs blockhash — Q7)
    - `rpc.send_transaction(&tx)`
    - On success: return signature
    - On `BlockhashNotFound` OR `BlockCleanedUp`: sleep backoff (100ms / 200ms / 400ms), retry
    - On other RPC errors: return `BroadcastFailed`
  - Read deep-dive §D line 1624 for full pseudocode + retry semantics before implementation
- [ ] Step 4: Implement `wait_for_confirm(rpc, sig, timeout) -> Result<SignatureStatus, ConfirmTimeout>` — polls `getSignatureStatuses` with exponential backoff; commitment levels `Processed | Confirmed | Finalized` per deep-dive §E line 1666 (Confirmed ~1 slot / Finalized ~12 slots)
- [ ] Step 5: Implement `tests/send_with_retry.rs` (rows 30+31) — gated `RUN_SOL_DEVNET=1`; spawn surfpool OR use devnet; transfer 0.001 SOL; assert `BlockhashNotFound` triggers retry (mock by forcing stale blockhash); observe fresh blockhash re-sign; bogus sig → `Error::ConfirmTimeout` within budget
- [ ] Step 6: Implement `tests/blockhash_cache.rs` (row 12) — `BlockhashCache::get_or_fetch` returns same hash within TTL; second call within 60s = 0 RPC; TTL expiry triggers refetch; mock clock advances past TTL
- [ ] Step 7: Implement `tests/rpc_methods_mock.rs` (row 22) — 21 RPC methods against `wiremock` stubs; HTTP 5xx → `Error::Transport` (never panic); timeouts → `Error::Transport` within 30s; empty response → `Error::MalformedResponse`
- [ ] Step 8: Implement `tests/preflight_balance.rs` (row 19) — mock `RpcClient` returning fixed balance; under-funded (balance < amount + fee + rent) → `Error::InsufficientFunds` before broadcast; funded → broadcast proceeds
- [ ] Step 9: Implement `tests/tx_status_parse.rs` (row 20) — `TransactionStatus` JSON → struct; success variant parses slot + confirmations; error variant maps per deep-dive §J Error classification; malformed JSON → `Error::MalformedResponse`
- [ ] Step 10: Implement `tests/transport_failure.rs` (row 34) — RPC pointed at closed port (127.0.0.1:1) → `Error::Transport` within 30s timeout; connection refused → `Error::Transport` within 5s
- [ ] Step 11: Implement `tests/common/surfpool_spawn.rs` — `spawn_surfpool(port: u16) -> Child`; ephemeral port; 10s health-poll deadline; RAII guard kills on drop
- [ ] Step 12: Verify gate: `cargo fmt + cargo clippy -- -D warnings + cargo test --test send_with_retry --test rpc_methods_mock --test preflight_balance --test tx_status_parse --test blockhash_cache --test transport_failure --features loud-red-tests`
- [ ] Step 13: PAUSE — commit-push-pr

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
| 30 | `tx/broadcast.rs` send_with_retry stale | Phase 5.1 | `tests/send_with_retry.rs` | ✅ |
| 31 | `tx/wait.rs` wait_for_confirm + timeout | Phase 5.1 | `tests/send_with_retry.rs` | ✅ |
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
- [ ] Step 1: Confirm `crates/sol-wallet-core/src/lib.rs` re-exports all public surface needed by Phase 7 CLI handlers (`Wallet`, `WalletManager`, `SolanaClient`, `SolanaConfig`, `build_sol_transfer`, `build_spl_transfer_checked`, `send_with_retry`, `wait_for_confirm`, `Error`).
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
| 11 | Blockhash retry on stale | `tests/send_with_retry.rs` (Phase 5.1) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 5.1 + 7.2 |
| 12 | Insufficient balance | `tests/preflight_balance.rs` (Phase 5.1) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 5.1 + 7.2 |
| 13 | Dry-run (simulate) | `tests/tx_serde.rs` (extend Phase 3.1 to add `--dry-run` simulate case) + `crates/sol/tests/cli_integration_surfpool.rs` | Phase 3.1 Modify + Phase 7.2 |
| 14 | Sign-only (no broadcast) | `tests/sign_only.rs` (Phase 1.2) + `crates/sol/tests/cli_wallet.rs` | Phase 1.2 + 7.1 |
| 15 | Confirmation polling — success | `tests/send_with_retry.rs` (Phase 5.1) + `crates/sol/tests/cli_tx.rs` | Phase 5.1 + 7.1 |
| 16 | Confirmation polling — timeout | `tests/send_with_retry.rs` (Phase 5.1) + `crates/sol/tests/cli_tx.rs` | Phase 5.1 + 7.1 |
| 17 | Finalized commitment | `tests/send_with_retry.rs` (Phase 5.1) + `crates/sol/tests/cli_tx.rs` | Phase 5.1 + 7.1 |
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
| 30  | `send_with_retry` stale blockhash      | `BlockhashNotFound` → retry with fresh                  | **V6** Phase 5.1 (devnet-gated)  | ✅ covered                                           |
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
