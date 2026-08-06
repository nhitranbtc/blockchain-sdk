# Architecture: Rust Bitcoin Wallet Core (`bitcoin-wallet-rs`)

**Date:** 2026-08-06
**Status:** Draft — companion to design spec
**Scope:** Phase 1 of `BlockchainSdk` Rust rewrite. Bitcoin module only.
**Source spec:** [`2026-08-05-rust-bitcoin-wallet-design.md`](2026-08-05-rust-bitcoin-wallet-design.md)
**Source plan:** [`../plans/2026-08-05-rust-bitcoin-wallet.md`](../plans/2026-08-05-rust-bitcoin-wallet.md)
**Source ADR:** [`../../wallets/2026-08-05-adr-0001-signing-model.md`](../../wallets/2026-08-05-adr-0001-signing-model.md)
**Source research:** [`../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md`](../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md) and [`../../2026-08-04-bcs-bitcoin-implementation-reference.md`](../../2026-08-04-bcs-bitcoin-implementation-reference.md)
**Mnemonic decision:** [`../../wallets/2026-08-05-mnemonic-handling-decision.md`](../../wallets/2026-08-05-mnemonic-handling-decision.md)

---

## 1. System overview

Three-crate Cargo workspace. **`bitcoin-wallet-core`** owns all signing + chain logic — host-agnostic library, no I/O outside `async fn`. **`btc`** wraps core in a `clap` CLI for dev/test. **`btc-server`** wraps core in `axum` REST for non-Rust consumers. No hardware wallet, no Lightning, no other UTXO chains in v1 (spec §1).

Core re-exports `secp256k1`, `miniscript`, `bip39` via `bdk_wallet::bitcoin::*` and `bdk_wallet::keys::bip39::*`. Direct deps only where BDK does not re-export (`bip32`). No `anyhow` in core; `anyhow` only in `btc` top-level. `Result<T, Error>` on every public function (plan Global Constraints).

Threat model layered across three releases per ADR 0001: **v0.1** plaintext mnemonic + in-process secp256k1 signing (testnet dev only); **v0.2** Argon2id + AES-256-GCM at rest + same software signing (small-stakes mainnet); **v1.0** hardware-backed via UniFFI `Signer` trait (production end-user). Each release additive — no breaking refactor of the v0.1 surface.

---

## 2. Component model

### 2.1 Layered view

```text
┌────────────────────────────────────────────────────────────────────┐
│ HOST LAYER (not in this repo)                                       │
│   tangem-app-ios (Phase 2) | dev shell (Phase 1 btc CLI)              │
└────────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────────┐
│ BINARY LAYER (thin wrapper — no logic)                              │
│   btc/         (clap 4)         — one-shot commands, no long-lived   │
└────────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────────┐
│ CORE LIBRARY (bitcoin-wallet-core) — single source of truth          │
│   keys  │  script  │  address  │  chain  │  wallet  │  tx  │  config │
│   ─────────────────────────────────────────────────────────────────  │
│   Re-exports: bdk_wallet::bitcoin::* → secp256k1 + miniscript       │
│               bdk_wallet::keys::bip39::* → bip39                     │
│   Direct dep: bip32 0.6 (BDK does not re-export)                    │
└────────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────────┐
│ EXTERNAL SDK LAYER (vendored via Cargo)                              │
│   bdk_wallet 3.1 (Wallet, TxBuilder, PSBT)                          │
│   bdk_esplora 0.22 + bdk_electrum 0.24 (chain sources)               │
│   bdk_file_store 0.15 (SQLite persistence)                           │
│   bitcoin 0.32, secp256k1 0.30, miniscript 12, bip32 0.6            │
└────────────────────────────────────────────────────────────────────┘
```

### 2.2 Dependency rules

- **No upward deps.** Core never imports `btc`. The `btc` binary never re-implements wallet logic.
- **No `unsafe` in user code.** Only FFI is `secp256k1` (Bitcoin Core's libsecp256k1, audited) (plan Global Constraints).

### 2.3 Crate surface (LOC budget reference)

| Crate | Role | Talks to | Talks about |
|---|---|---|---|
| `bitcoin-wallet-core` | Library, signing + chain | bdk_*, bitcoin, bip32 | Everything |
| `btc` | CLI | core | Mnemonic, address, balance, send, tx, fee |

---

## 3. Public API surface (core)

Per spec §4.1. Locked signatures — every plan task binds to one of these.

```rust
// === Construction ===
impl Wallet {
    pub async fn from_mnemonic(m: &Mnemonic, passphrase: &str, config: WalletConfig) -> Result<Self>;
    pub async fn from_descriptor(d: &str, network: Network, esplora_url: &str) -> Result<Self>;
    pub async fn open(config: WalletConfig, db: &Path) -> Result<Self>;  // persisted
    pub fn builder() -> WalletBuilder;
}

// === Read (no I/O, &self) ===
impl Wallet {
    pub fn network(&self) -> Network;
    pub fn balance(&self) -> Result<Balance>;
    pub fn address(&self, kind: AddressType) -> Result<AddressInfo>;
    pub fn addresses(&self) -> Result<Vec<AddressInfo>>;
    pub fn transactions(&self) -> Result<Vec<TransactionRecord>>;
    pub fn fee_estimate(&self) -> Result<FeeEstimate>;
}

// === Read-mut (advances index, &mut self) ===
impl Wallet {
    pub fn new_address(&mut self, kind: AddressType) -> Result<AddressInfo>;
}

// === Write (network I/O, async) ===
impl Wallet {
    pub async fn sync(&mut self) -> Result<()>;
    pub async fn broadcast(&self, tx: &Transaction) -> Result<Txid>;
}

// === Write (CPU only, sync) ===
impl Wallet {
    pub fn build_tx(&self, params: TxParams) -> Result<Psbt>;
    pub fn sign(&self, psbt: &mut Psbt) -> Result<Transaction>;
    pub fn bump_fee(&self, txid: &Txid, new_rate: FeeRate) -> Result<Psbt>;
}
```

**Async rule:** only on true I/O (`sync`, `broadcast`, `from_*`). Pure CPU work stays sync — a v2 server (when added) can offload via `tokio::task::spawn_blocking` (spec §6).

**Mut rule:** only `sync` + `new_address` mutate. Everything else takes `&self`. Rationale: BDK Wallet interior mutability for PSBT/fee; address index advance is the one semantic mutation the host must observe.

**Constructor matrix:**

| Mnemonic | Descriptor | DB on disk | Use case |
|---|---|---|---|
| ✓ | — | — | `from_mnemonic` (new wallet, in-memory) |
| — | ✓ | — | `from_descriptor` (import watch-only) |
| ✓ | — | ✓ | `open` (reopen persisted) |

---

## 4. State management

### 4.1 In-process state (per `Wallet`)

```rust
pub struct Wallet {
    bdk: Mutex<BdkWallet>,         // std::sync::Mutex — see §6
    esplora: EsploraClient,        // reqwest::Client (Send + Sync)
    config: WalletConfig,          // network, urls, db_path
}
```

Single interior lock on BDK Wallet. Lock held only for sync/balance/build/sign — never across `.await`. Esplora client is `Clone + Send + Sync` (reqwest contract).

### 4.2 Multi-wallet server state (deferred to v2)

`btc-server` deferred. When added (spec §6), the shape is:

```rust
pub struct AppState {
    wallets: RwLock<HashMap<WalletId, Arc<tokio::sync::Mutex<Wallet>>>>,
    data_dir: PathBuf,
    esplora_url: String,
}
```

`tokio::sync::Mutex` per wallet — server may `.await` while holding it during sync. `RwLock` on the map — reads (list) do not block each other, writes (create/delete) are exclusive. v1 CLI is per-invocation lifetime — no shared state across commands.

### 4.3 Persistent state

```text
data_dir/
├── {wallet_id}/
│   ├── bdk.sqlite          # bdk_file_store — descriptors, tx graph, UTXO set
│   └── mnemonic.enc        # v0.1 plaintext | v0.2 Argon2id + AES-256-GCM
```

Mnemonic kept **outside** BDK's SQLite. BDK never sees plaintext seed (mnemonic-handling-decision §"Pick per release" — v0.1).

### 4.4 v0.1 hygiene (per mnemonic-handling-decision §"v0.1 add")

- `Secret<Mnemonic>` newtype with `ZeroizeOnDrop` (zeroize crate).
- Refuse world-writable wallet dirs (mode 0600 mandatory).
- Atomic writes: `tmp + fsync + rename` (Trust Wallet Core PR #4756 pattern).

---

## 5. Data flow (reference)

Spec §5. Three canonical flows:

```text
CREATE:  CLI mnemonic(12) → BIP-39 seed → BIP-32 master → BIP-84 account
         → descriptor string (wpkh external + change) → BDK Wallet::create
         → persist SQLite → first full_scan via Esplora

SEND:    sync() → fee_estimate() (live Esplora fetch) → build_tx(to, amount, fee_rate)
         → PSBT → sign(psbt) → Transaction → broadcast(tx) → Esplora /tx POST
         → txid; sync() again on next poll to confirm

REST:    deferred to v2. Planned shape (spec §6): POST /v1/wallets, GET /v1/wallets/{id}/balance, POST /v1/wallets/{id}/tx. No REST surface ships in v1.
```

All flows idempotent on the same input (except `new_address`, which advances the index).

---

## 6. Concurrency model

| Layer | Mutex type | Reason |
|---|---|---|
| `Wallet.bdk` (core, single-wallet) | `std::sync::Mutex` | BDK Wallet is `!Send` in 3.x; lock held only across sync CPU work, never across `.await` |
| CLI (`btc`) | None — per-invocation lifetime | One command = one Wallet = drop at end |
| v2 server (deferred) | `tokio::sync::RwLock` (map) + `Arc<tokio::sync::Mutex<Wallet>>` (per wallet) | Per spec §6. Not implemented in v1. |

**Backpressure:** v1 CLI is single-shot. No cross-command contention. v2 server (if added) may add per-wallet semaphore for concurrent send (RBF race) — deferred.

**Why `std::sync::Mutex` inside `Wallet`:** BDK 3.1's `Wallet` is `!Send` (internal `RefCell` state). `tokio::sync::Mutex` requires `Send`. Guard must NEVER be held across `.await` — would fail to compile under multi-threaded runtimes. This invariant is enforced by clippy::await_holding_lock in CI (post-Task 31 spike). Plan Task 9 sync() was audited and rewritten to acquire the lock *after* the async scan completes. v2 server, if added, wraps in `tokio::sync::Mutex` to add `Send + Sync` for the multi-wallet map.

**Lock-ordering rule (v2 only):** `AppState.wallets` (outer) → `Wallet.bdk` (inner). Never reversed. CLI never holds more than one.

---

## 7. Storage architecture

### 7.1 BDK SQLite (bdk_file_store)

- Lives at `data_dir/{wallet_id}/bdk.sqlite`.
- Stores: descriptor set, key origin metadata, tx graph, UTXO set, chain checkpoints.
- One DB per wallet — multi-wallet via directory, not via shared schema (simpler backups, no cross-wallet coupling).
- Persistence on every state-changing call: `wallet.persist()` after `sync`, `new_address`, `reveal_*`.

### 7.2 Mnemonic file

Per mnemonic-handling-decision:

| Version | Format | Location | Encryption |
|---|---|---|---|
| v0.1 | raw words | `mnemonic.txt` (mode 0600) | None + `Secret<Mnemonic>` in memory |
| v0.2 | `magic(4) \|\| ver(1) \|\| salt(16) \|\| nonce(12) \|\| ciphertext(N+16)` | `mnemonic.enc` (mode 0600) | Argon2id 256 MiB / 10 iter / 500ms + AES-256-GCM |
| v1.0 | iOS Keychain (via Swift host) | n/a on disk | Swift `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` |

### 7.3 Atomic writes (all versions)

```rust
fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    let f = fs::File::open(&tmp)?;
    f.sync_all()?;
    fs::rename(&tmp, path)?;  // POSIX atomic on same fs
    Ok(())
}
```

Cost: 30 LOC. Prevents partial-write corruption on crash.

---

## 8. Security model (per ADR 0001)

| Layer | v0.1 | v0.2 | v1.0 |
|---|---|---|---|
| **Signing** | `secp256k1::Keypair` in process | Same | `Signer` trait; Swift `TangemSdk` impl |
| **Mnemonic at rest** | Plaintext + mode 0600 | Argon2id 256 MiB + AES-256-GCM | iOS Keychain (Swift-side) |
| **Mnemonic in memory** | `Secret<Mnemonic>` + `ZeroizeOnDrop` | + `mlock` (Unix) | Swift `Data` + `kSecAttrAccessible` |
| **In-process key** | Raw `Keypair` | Raw `Keypair` | Never in process (signing on card) |
| **Network** | Testnet default; mainnet opt-in | Same | Mainnet + testnet + regtest |
| **Threat model coverage** | "Don't accidentally leak on your own machine" | + "Stolen disk image" | + "Coercion" + "In-memory extraction" |
| **Acceptable for** | Dev, CI, testnet | Coffee-money mainnet | End-user production |

**Migration triggers:**
- v0.1 → v0.2: `argon2` + `aes-gcm` deps land; CLI gains `--passphrase-prompt` (no shell history).
- v0.2 → v1.0: Phase 2 UniFFI plan; `sign_with_external_signer(&psbt, &impl Signer)` added (new method, not breaking).

---

## 9. Key design decisions (locked)

Each row = decision + why + rejected alternatives. Sources cited inline.

| # | Decision | Rationale | Rejected | Source |
|---|---|---|---|---|
| 1 | **2-crate workspace** (core / btc) | Library reuse + dev CLI; v2 adds btc-server for HTTP | 3-crate from spec §2 (btc-server deferred per plan Global Constraint) | spec §2; plan line 21 |
| 2 | **BDK 3.1** over raw rust-bitcoin | Descriptor/miniscript first; 1.1M+ downloads; Spiral-funded; 35 dependents | Raw `bitcoin` 0.32 (10k LOC reinvented badly); `lwk_wollet` (Liquid-first) | research §BDK, implementation ref §B |
| 3 | **bdk_file_store (SQLite)** for persistence | Built-in feature; same DB used by BDK tests; multi-wallet via partitioning | `bdk_sqlite` (low traction, 3 ★); `bdk_redb` (experimental); custom (reinvented badly) | spec §13.3; research §storage |
| 4 | **bdk_esplora** as primary chain source | Already P0 for sync; no new dep for fee endpoint; standard REST | `rustywallet-mempool` (2 ★, single maintainer); self-hosted `bdk_bitcoind_rpc` (deferred to v2) | research §fee; implementation ref §A |
| 5 | **BIP-84 default** (Native SegWit) | Modern; lower fees; broadly supported; BDK's `create_single` fits | BIP-86 Taproot (newer, less battle-tested); BIP-49 nested (legacy compat) | spec §4.2; ADR 0001 implicitly |
| 6 | **Testnet default** + mainnet opt-in | Phase 1 is dev tool; mainnet is opt-in via `--network mainnet` or confirmation | Mainnet default (v0.1 unsafe with software signing) | spec §13; ADR 0001 v0.1 row |
| 7 | **Mnemonic split** (BDK inside, encrypted file outside) | BDK never sees plaintext seed; backups independent of DB | Mnemonic inside SQLite (couples secrets to BDK schema) | mnemonic-handling-decision §"v0.1 add" |
| 8 | **v0.1 plaintext mnemonic** + `Secret<Mnemonic>` | v0.1 is testnet dev; Secret<T> prevents the common memory-dump attack | Argon2id in v0.1 (overkill for testnet); HSM-only (impossible) | ADR 0001 v0.1; mnemonic-handling-decision §"v0.1" |
| 9 | **Multi-wallet deferred to v2** (server) | v2 server will key wallets by `wallet_id` UUID; v1 CLI is per-invocation — no concurrent wallets | Server in v1 (over-scope for dev tool); per-process-per-wallet (ops pain) | spec §13.5; plan Global Constraint "No REST/HTTP server in v1" |
| 10 | **No hardware in v1** (Signer trait in v1.0) | Phase 1 dev tool; hardware adds 2 weeks + USB transport with no iOS equivalent | Ledger/Trezor in v0.1; remote signer service (privacy/trust) | ADR 0001 v0.1 + Alternatives |
| 11 | **Half-hour default fee** (3-block target), live-fetched | CLI fetches `FeeEstimate` from Esplora at send time; user can override per send with `--fee fastest|half_hour|hour|economy` | Per-tx fee required (UX friction); Esplora `fastestFee` (overpays); hardcoded constant (drifts from current mempool) | spec §13.4; plan Task 17 fix |
| 12 | **No `anyhow` in core** | `Error` enum with `#[from]` conversions is type-checked + machine-readable; anyhow loses variants | anyhow everywhere (loses type info); `Box<dyn Error>` (no variants) | spec §8; plan Global Constraints |
| 13 | **`#![deny(unsafe_code)]` in core** | Forces any FFI through audited `secp256k1` only | Allow `unsafe` with lint exceptions (drift risk) | plan Global Constraints |
| 14 | **No `unwrap`/`panic!` in core** | All public fns return `Result`; panic = bug, caught by tests | unwrap acceptable in tests only (already in plan) | plan Global Constraints |
| 15 | **v0.1 plaintext mnemonic** at testnet — no auth concern (CLI only) | v1 ships CLI only; v2 server (when added) will use 127.0.0.1 bind = no auth surface | n/a (no server in v1) | plan Global Constraint; spec §6 deferred |
| 16 | **License: MIT** | Matches `rust-bitcoin`, `BDK`, `bip39`, `bip32` (all MIT) — maximum reuse | Apache-2.0 only (no differentiator); dual MIT/Apache (more CI surface) | spec §13.2 |

---

## 10. Cross-cutting concerns

### 10.1 Error handling

Per spec §8. `thiserror::Error` everywhere in core. v2 server (when added) will impl `IntoResponse` to map `Error` to RFC 7807 `application/problem+json`. v1 CLI prints `Error: {msg}` via `anyhow`.

Status code mapping (planned v2 server):

| Error variant | HTTP status | Code |
|---|---|---|
| `InvalidMnemonic`, `InvalidDerivationPath` | 400 | `invalid_input` |
| `NotInitialized` | 404 | `not_found` |
| `InsufficientFunds` | 422 | `insufficient_funds` |
| `Network`, `Esplora`, `Electrum` | 502 | `upstream` |
| everything else | 500 | `internal` |

### 10.2 Logging (tracing)

```rust
// btc CLI main
tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,bitcoin_wallet_core=debug")))
    .init();
```

Core uses `tracing::info!`, `tracing::debug!` at module boundaries. No sensitive data in events (mnemonic, xprv never logged). v2 server (when added) will add request-id via `tower-http::trace`.

### 10.3 Configuration precedence

1. CLI flag (highest)
2. Env var (`BTC_DATA_DIR`, `BTC_ESPLORA_URL`)
3. `~/.config/btc/config.toml` (per-wallet override)
4. Built-in default (testnet, `https://blockstream.info/testnet/api` — matches plan Task 16 `default_value`)

### 10.4 Dependency policy

- `cargo deny check` in CI: no copyleft, no unmaintained, no advisories.
- `cargo +nightly udeps` in CI: no unused deps.
- MSRV = 1.85 (matches BDK 3.x).
- `cargo miri test` in slow lane: soundness check on core.

---

## 11. Crate footprint (locked, from plan Task 1)

| Crate | Version | Why this version | Source |
|---|---|---|---|
| `bdk_wallet` | 3.1 | MSRV 1.85; descriptor/Miniscript first; `keys-bip39` feature exposes `bip39` | spec §3; research §B |
| `bdk_chain` | 3.1 | Required by `bdk_wallet` | spec §3 |
| `bdk_esplora` | 0.22 | P0 chain source; P0 fee source | research §fee |
| `bdk_electrum` | 0.24 | Fallback chain source (feature-gated) | spec §3 |
| `bdk_file_store` | 0.15 | SQLite persistence; same DB BDK uses in tests | spec §13.3 |
| `bitcoin` | 0.32 | Re-exports `secp256k1`, `miniscript`; required by BDK | spec §3 |
| `bip32` | 0.6 | BDK does NOT re-export — direct dep needed for `XPrv` | plan Task 1 footnotes |
| `bip39` | (none) | Re-exported via `bdk_wallet::keys::bip39` (feature `keys-bip39`) | spec §3 |
| `secp256k1` | (none direct) | Re-exported via `bdk_wallet::bitcoin::secp256k1` | spec §3 |
| `miniscript` | (none direct) | Re-exported via `bdk_wallet::miniscript` (used by BDK internally for descriptors) | spec §3 |
| `tokio` | 1 | Full features; needed by CLI for `#[tokio::main]` | spec §3 |
| `reqwest` | 0.12 | Esplora + Electrum HTTP | spec §3 |
| `axum` | 0.7 | (deferred to v2 with btc-server) | spec §3; plan line 159 |
| `utoipa` | 5 | (deferred to v2 with btc-server) | spec §3; plan line 159 |
| `thiserror` | 1 | Library errors | spec §3 |
| `anyhow` | 1 | CLI only | spec §3 |
| `tracing` | 0.1 | Structured logging | spec §3 |
| `clap` | 4 | CLI | spec §3 |
| `proptest` | 1 | Property-based tests (script + address modules) | spec §3 |
| `argon2` | 0.5 | v0.2 mnemonic encryption (P1) | spec §3 |
| `aes-gcm` | 0.10 | v0.2 mnemonic encryption (P1) | spec §3 |
| `rand` | 0.8 | v0.2 KDF salt + v0.1 nonce | plan Task 3 |
| `zeroize` | 1 | v0.1 `Secret<T>` newtype | mnemonic-handling-decision |

**Net production deps (v0.1):** 16 direct (axum + utoipa removed; v0.2 deps `argon2`/`aes-gcm`/`rand`/`zeroize` not yet active). **Net v0.2 adds:** 4 more = 20. v2 server (if added) re-introduces axum/utoipa + tower/tower-http/utoipa-swagger-ui (~5 more).

---

## 12. Open questions

Resolved-vs-open split. Open = needs spike during plan execution.

| # | Question | Decision path | Status |
|---|---|---|---|
| 1 | `BdkWallet` Send-ness in 3.1? | Spike Task 31: if `!Send`, keep `std::sync::Mutex` inside `Wallet`; if `Send`, still keep std (simpler; no v1 server to bridge) | Open (still relevant for v2 server design) |
| 2 | Electrum fallback syntax in `bdk_electrum` 0.24 | Task 8: confirm `BdkElectrumClient::new(url)` + `full_scan` API matches plan snippet | Open |
| 3 | `bdk_file_store` 0.15 — `Store` trait + thread-safety | Task 9: confirm `Connection` is `Send + Sync` (for v2 server use in `tokio::sync::Mutex<Wallet>`) | Open |
| 4 | RBF signaling in BDK 3.1 | Task 14: confirm `SignOptions` defaults signal RBF (BIP-125 sequence < 0xfffffffe) | Open |
| 5 | `Mnemonic::from_entropy_in` word count enums (12/15/18/21/24) | Task 3: confirm 5 variants exposed via `bdk_wallet::keys::bip39::MnemonicType` | Open |
| 6 | Wallet ID format | Deferred to v2 server task. UUID per spec; v2+ may switch to ULID for sortable IDs | Deferred |
| 7 | Server authentication v2 | Bearer token via `tower-http::auth`. Trigger: first external user | Deferred (v2 server) |
| 8 | Multi-account support | Spec out-of-scope v1; revisit when 2nd-account UX requested | Deferred (spec §1) |
| 9 | utoipa JSON schema + Swagger UI path | `/swagger-ui/` per spec §6; verify axum 0.7 integration in v2 server work | Open (v2) |

---

## 13. Verification plan (per release)

| Release | Tests required before merge | Source |
|---|---|---|
| **v0.1** | `cargo fmt --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test` (unit + regtest); Trezor BIP-39 test vectors as CI gate | spec §9.4; mnemonic-handling-decision §"Concrete deltas" #10 |
| **v0.2** | + `cargo deny check`; + AES-256-GCM known-vector round-trip; + Argon2id 500ms wall-clock calibration test | spec §9.4; mnemonic-handling-decision §"v0.2" |
| **v1.0** | + miri on core; + e2e REST tests; + property-based tests for script + address; + fuzzing on script parser | spec §9; Week 7 hardening |
| **All** | `cargo +nightly udeps` slow lane | spec §9.4 |

Size audit (v1 CLI): `btc` binary ≤ 15 MB stripped (plan Task 21). Achieved via `RUSTFLAGS="-C opt-level=z -C lto=fat -C strip=symbols -C panic=abort"` (spec §10). v2 server size budget TBD when btc-server is added.

---

## 14. References

### Canonical source documents

- **Spec:** [`docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md`](2026-08-05-rust-bitcoin-wallet-design.md) (commit `e2d51ec`)
- **Plan:** [`docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`](../plans/2026-08-05-rust-bitcoin-wallet.md)
- **Task→SDK map:** [`docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet-task-sdk-map.md`](../plans/2026-08-05-rust-bitcoin-wallet-task-sdk-map.md)
- **ADR 0001 (signing model):** [`docs/wallets/2026-08-05-adr-0001-signing-model.md`](../../wallets/2026-08-05-adr-0001-signing-model.md)
- **Mnemonic decision:** [`docs/wallets/2026-08-05-mnemonic-handling-decision.md`](../../wallets/2026-08-05-mnemonic-handling-decision.md)

### Research

- **Tangem→Rust SDK mapping:** [`docs/blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md`](../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md) (commit `0c20f77`)
- **Implementation reference (crate inventory):** [`docs/2026-08-04-bcs-bitcoin-implementation-reference.md`](../../2026-08-04-bcs-bitcoin-implementation-reference.md)
- **Tangem iOS Bitcoin module (subject of replacement):** `tangem-app-ios/Modules/BlockchainSdk/Blockchains/Bitcoin/` (read-only research material)

### External standards

- [BIP-32 (HD derivation)](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
- [BIP-39 (mnemonic)](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [BIP-44/49/84/86 (derivation paths)](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki)
- [BIP-125 (RBF)](https://github.com/bitcoin/bips/blob/master/bip-0125.mediawiki)
- [BIP-174 (PSBT)](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki)
- [RFC 7807 (problem details for HTTP APIs)](https://www.rfc-editor.org/rfc/rfc7807)
- [RFC 9106 (Argon2)](https://www.rfc-editor.org/rfc/rfc9106.html)
- [Book of BDK](https://bitcoindevkit.github.io/book-of-bdk/)

### Crate registries (verify current versions before pinning)

- [bdk_wallet on crates.io](https://crates.io/crates/bdk_wallet)
- [bdk_esplora on crates.io](https://crates.io/crates/bdk_esplora)
- [bitcoin on crates.io](https://crates.io/crates/bitcoin)
- [bip32 on crates.io](https://crates.io/crates/bip32)

---

## 15. Revision log

| Date | Change | Source |
|---|---|---|
| 2026-08-06 | Initial draft; synthesizes spec §2-§8, ADR 0001, mnemonic-handling-decision, plan §Global Constraints + Task 1 | All listed in §14 |
| 2026-08-06 | Cut `btc-server` to align with plan (3-crate → 2-crate, no REST in v1). Sections updated: §1 overview, §2 component model (host + binary layers, crate surface, dep rules), §3 async rule wording, §4.2 multi-wallet state (deferred to v2), §4.3 persistent state (server.toml removed), §5 data flow (REST flow deferred), §6 concurrency (server rows marked v2), §9 locked decisions (rows 1, 9, 11, 15 updated), §10.1 error handling (server status codes marked v2), §10.2 logging (btc-server main → btc main), §10.3 config (built-in default = testnet URL), §11 crate footprint (axum + utoipa marked deferred, v0.1 count 18 → 16), §12 open questions (server-related Qs marked v2), §13 size audit (server binary → CLI binary) | Plan edits this session |
| 2026-08-06 | Applied ecc:architect review. Plan: fixed 5 criticals (sync lock-across-await, broadcast `build_async()`, derivation test 4-arg signature, `Secret::into_inner` `#[allow(unsafe_code)]`, `--amount_sat`→`--amount-sats`). Plan: applied 4 majors (Argon2id params m=256MiB/t=10, `refuse_world_writable` mask 0o077, Task 33 CI send gate, "verified by inspection" → clippy::await_holding_lock). Plan: 3 minors (coin_type_for wildcard → 1, README links → GitHub URLs, depth). Arch doc: §6 "verified by inspection" rephrased | ecc:architect review (a482102b30d747bc1) |
| 2026-08-06 | Story coverage expanded to 20/20. Plan: added coverage matrix in intro, 4 new core tasks (17.5 = `Wallet::transactions()`, 17.6 = `--fee-rate-sat-per-vb` flag, 18.5 = wallet manager + bump-fee + sign-message + descriptor export, 18.6 = multi-recipient + drain + coin-selection + manual UTXO + --type, 18.7 = mainnet confirmation prompt). Add-on tasks 26-34 re-labeled as optional polish (not required for v0.1) | Story coverage analysis this session |
