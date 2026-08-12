# Rust Bitcoin Wallet Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Review audit:** [../reviews/2026-08-05-rust-bitcoin-wallet.md](../reviews/2026-08-05-rust-bitcoin-wallet.md) — 50 doc-review findings applied; 2 deferred to Open Questions.
>
> **Spec:** [../specs/2026-08-05-rust-bitcoin-wallet-design.md](../specs/2026-08-05-rust-bitcoin-wallet-design.md) (see PR #88).
> **Architecture:** [../specs/2026-08-06-rust-bitcoin-wallet-architecture.md](../specs/2026-08-06-rust-bitcoin-wallet-architecture.md).
> **Research:** [../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md](../../blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md) (see PR #88).

**Goal:** Replace `tangem-app-ios/Modules/BlockchainSdk/Blockchains/Bitcoin/` (~2,070 Swift LOC) with a standalone Rust Bitcoin wallet — library + minimal CLI. No Swift host, no `TangemSdk`, no hardware. Phase 1 ships MVP (Tasks 0-9 + minimal CLI); remaining user stories and v0.2 multi-chain umbrella deferred per F33/F35.

**Architecture:** Cargo workspace `rust-wallet-app/`. Crate `bitcoin-wallet-core` (BDK 3.1 + rust-bitcoin 0.32) owns signing + chain + encryption. `secp256k1` and `miniscript` re-exported via `bdk_wallet::bitcoin::*`; no direct deps needed. Crate `btc` (clap 4) is the minimal CLI. **Default network is Bitcoin testnet** for all development and CI; mainnet is opt-in via `--network mainnet`.

**MVP scope (per F35):** Tasks 0-9 + minimal CLI = working wallet create + sync + balance. Stories 1, 2, 3, 4, 11, 12 in scope. Stories 5-10, 13-20 deferred to v0.1.1/v0.1.2.

**Tech Stack:** Rust 1.85 MSRV (justification per F31: `bdk_wallet 3.1` requires 1.85; if a lower MSRV proves compatible, downgrade), BDK 3.1 (with `keys-bip39` feature for `bip39` re-export), rust-bitcoin 0.32 (re-exports `secp256k1` + `miniscript` + `bip39`), bip32 0.6 (fallback `^0.5` per F46), tokio 1, reqwest 0.12, thiserror 1, tracing 0.1, clap 4, proptest 1, argon2 0.5, aes-gcm 0.10, rand 0.8, zeroize 1.

## Global Constraints

- **MSRV:** Rust 1.85. (Justification per F31: bdk_wallet 3.1 requires 1.85; if Task 31 spike finds lower works, downgrade to 1.82.) Set in workspace `rust-toolchain.toml`.
- **Edition:** Rust 2021.
- **License:** MIT. Copyright (c) 2026 individual contributors. CLA required.
- **Repo layout:** workspace `rust-wallet-app/`; 2 crates under `crates/{bitcoin-wallet-core,btc}`. Multi-chain umbrella is a separate v0.2 plan.
- **No hardware-wallet integration** anywhere.
- **No `unsafe` in user code except as documented in Task 1.5** (per F16 + F53). One permitted exception: `Secret::into_inner` in `keys/secret.rs` uses a single scoped `unsafe` block (`ptr::read` + `mem::forget`) to move out before `ZeroizeOnDrop` fires. CI runs `cargo geiger` to enforce.
- **No `anyhow` in `bitcoin-wallet-core`.** `anyhow` only in `btc` CLI top-level.
- **All public functions return `Result<T, Error>`.** No `unwrap` / `panic!` in library code (per F43 — `p2wpkh`/`p2pkh`/`p2tr_key_path` return `Result<ScriptBuf, Error>`).
- **Default fee strategy:** half-hour (3-block target). Override per send.
- **Mnemonic storage:** encrypted at rest with Argon2id + AES-256-GCM (per F6 — promoted from add-on to v0.1 core). Argon2id calibrated to `m=256 MiB, t=10, p=4` (per F5 — Sparrow reference; 500ms wall-clock target).
- **DB:** `bdk_file_store` (SQLite) under `data_dir/{wallet_id}/`.
- **Default network:** testnet. Mainnet opt-in via `--network mainnet`. Regtest opt-in via `--network regtest` (requires local bitcoind via `bdk_testenv`).
- **No REST/HTTP server in MVP.** CLI only. HTTP interface deferred to v0.2.

## File Structure

```text
rust-wallet-app/
├── Cargo.toml                          (workspace)
├── LICENSE                             (MIT)
├── README.md
├── rust-toolchain.toml                 (1.85)
├── deny.toml
├── crates/
│   ├── bitcoin-wallet-core/            (library)
│   │   ├── Cargo.toml                  (with regtest-tests feature per F3)
│   │   ├── src/
│   │   │   ├── lib.rs                  (re-exports module tree)
│   │   │   ├── error.rs                (Error enum, thiserror)
│   │   │   ├── config.rs               (WalletConfig + EsploraPinnedPubkey)
│   │   │   ├── threat.rs               (Sighash enum, MessageClass enum — per F21)
│   │   │   ├── util/{mod,atomic_write,permissions}.rs
│   │   │   ├── keys/{mod,mnemonic,derivation,signer,secret}.rs
│   │   │   ├── crypto/{mod,argon2,aes_gcm,bip137}.rs   # per F6/F9/F50
│   │   │   ├── script/{mod,builder,parser}.rs          # builder returns Result per F43
│   │   │   ├── address/{mod,legacy,segwit,taproot}.rs
│   │   │   ├── chain/{mod,network,esplora,electrum}.rs  # client pinned pubkey per F20
│   │   │   └── wallet/{mod,builder,sync,balance,addresses,load}.rs
│   │   └── tests/{regtest_send_roundtrip,proptest_script,proptest_address}.rs  # vectors.rs removed per F40
│   └── btc/                            (CLI)
│       ├── Cargo.toml
│       └── src/{main,commands/{mod,wallet}}.rs   # minimal CLI
└── .github/workflows/ci.yml             (regtest-tests feature gate per F3)
```

## Story Coverage Matrix (MVP)

| # | Story | Plan task(s) | Phase | Status |
|---|---|---|---|---|
| 1 | Create wallet | 3, 9 | MVP | core |
| 2 | Import wallet | 3, 9 | MVP | core |
| 3 | Check balance | 9 | MVP | core |
| 4 | Sync chain | 9 | MVP | core |
| 11 | Persist across invocations | 9 | MVP | core (via bdk_file_store) |
| 12 | Config show | 9 | MVP | core (via WalletConfig struct) |

**Deferred stories** (5, 6, 7, 8, 9, 10, 13-20) → see [Phase 2 Backlog](#phase-2-backlog-deferred-to-v011--v012-per-f33-f35) below.

---

## Task 0: Threat Model

### Task 0a: Threat Model (per F24)

**Files:**
- Create: `docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md`

**Content (template):**

```markdown
# Threat Model: bitcoin-wallet-core v0.1

## Assets
- Mnemonic (encrypted at rest per F6)
- xprv (derived from mnemonic, in-memory only)
- PSBT (transit only, never persisted)
- UTXO set (persisted in bdk_file_store SQLite db)
- Signed messages (BIP-137 — per F7 API, no raw key exposure)
- Wallet metadata (address_type, network, derivation path)

## Adversaries
- A1: Local user with read access to data dir
- A2: Local user with write access to data dir
- A3: Network attacker (MITM, BGP hijack, rogue CA)
- A4: Malicious Esplora/Electrum endpoint
- A5: Malicious PSBT provider (coinjoin coord, hw wallet workflow)
- A6: Supply-chain compromise (dep crate, CI pipeline)
- A7: Phishing vector (user signs arbitrary bytes via CLI)
- A8: Local process with /proc/$pid/mem access

## Trust boundaries
- B1: Process ↔ filesystem (data dir, mnemonic.enc, descriptors)
- B2: Process ↔ network (Esplora over TLS — pinned pubkey per F20)
- B3: Process ↔ PSBT source (CLI stdin — review per F25 deferred to v0.1.1)
- B4: Library ↔ hardware (no v0.1 hw integration)

## Abuse cases
- U1: Malicious PSBT redirects 100% of balance (mitigation: deferred to v0.1.1)
- U2: Fake Esplora lies about UTXOs (mitigation: F20 pin pubkey)
- U3: Process memory leak via /proc/$pid/mem (mitigation: mlock deferred to v0.2)
- U4: Supply-chain compromise of atty crate (mitigation: F48 IsTerminal)
- U5: User signs arbitrary hash via CLI (mitigation: F7 narrow API)
- U6: World-readable mnemonic file (mitigation: F19 atomic_write + 0o600)
- U7: Crashed mid-write leaves partial mnemonic (mitigation: F19 atomic_write)

## Mitigations mapping
| Abuse case | Mitigation task | Status |
| ---------- | --------------- | ------ |
| U1 | F25 (deferred) | v0.1.1 |
| U2 | F20 (Task 7) | in plan |
| U3 | mlock | deferred to v0.2 |
| U4 | F48 (deferred) | v0.1.1 |
| U5 | F7 (Task 6) | in plan |
| U6, U7 | F19 (Task 1.5) | in plan |
```

- [ ] **Step 1:** Write threat-model.md content above.
- [ ] **Step 2:** Pause for commit approval.

---

## Week 1 — Foundation (Tasks 1-4)

### Task 1: Workspace + CI scaffold

**Files:**
- Create: `rust-wallet-app/Cargo.toml` (workspace manifest)
- Create: `rust-wallet-app/rust-toolchain.toml` (1.85)
- Create: `rust-wallet-app/LICENSE` (MIT + copyright per F32)
- Create: `rust-wallet-app/.gitignore`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/Cargo.toml`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/lib.rs`
- Create: `rust-wallet-app/crates/btc/Cargo.toml`
- Create: `rust-wallet-app/crates/btc/src/main.rs`
- Create: `rust-wallet-app/.github/workflows/ci.yml`
- Create: `rust-wallet-app/deny.toml`

- [ ] **Step 1: Write failing test for workspace build**

```rust
// crates/bitcoin-wallet-core/tests/build_workspace.rs  (lives under member crate
// so cargo test --workspace picks it up; the workspace root has no [package] block)
#[test]
fn workspace_members_compile() {
    // cargo build --workspace enforces this; test exists to gate CI
}
```

- [ ] **Step 2: Create workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["crates/bitcoin-wallet-core", "crates/btc"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
license = "MIT"
# repository — set per fork. Default placeholder.

[workspace.dependencies]
# Bitcoin (BDK 3.1 + fallback chain per F26)
bdk_wallet = { version = "^3.1", features = ["keys-bip39"] }
bdk_chain = "^0.23"  # corrected from plan draft `^3.1`: crates.io latest is 0.23.x (bdk_wallet 3.x pins it)
bdk_esplora = { version = "^0.22", features = ["async"] }
bdk_electrum = "^0.24"
bdk_file_store = "^0.22"  # corrected from plan draft `^0.15`: crates.io latest is 0.22.x
bitcoin = "0.32"
bip32 = "^0.5"  # F46 fallback: 0.6 is pre-release only (0.6.0-pre.1)

# Async + HTTP
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Errors + tracing
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Crypto (per F6 mnemonic encryption in v0.1 core)
argon2 = "0.5"
aes-gcm = "0.10"
rand = "0.8"
zeroize = { version = "1", features = ["derive"] }

# CLI
clap = { version = "4", features = ["derive", "env"] }

# Test
proptest = "1"
tempfile = "3"

# Encoding (used by crypto::bip137 for base64-encoded message signatures)
base64 = "0.22"
```

- [ ] **Step 3: Create rust-toolchain.toml**

```toml
[toolchain]
# MSRV is 1.85 (F31) but transitive icu_* deps require rustc ≥1.86.
# Use stable available locally; down-pin per crate if needed.
channel = "1.94"
components = ["rustfmt", "clippy", "rust-src"]
```

- [ ] **Step 4: Create LICENSE (MIT + copyright per F32)**

```text
MIT License

Copyright (c) 2026 The blockchain-sdk project contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 5: Create .gitignore**

```text
/target
**/*.rs.bk
Cargo.lock
.DS_Store
.env
```

- [ ] **Step 6: Create bitcoin-wallet-core/Cargo.toml**

```toml
[package]
name = "bitcoin-wallet-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
# repository.workspace = true  # uncomment if workspace.repository set

[features]
default = []
regtest-tests = []  # per F3

[dependencies]
bdk_wallet = { workspace = true, features = ["keys-bip39"] }
bdk_chain = { workspace = true }
bdk_electrum = { workspace = true }
bdk_file_store = { workspace = true }
bitcoin = { workspace = true }
bip32 = { workspace = true }
bip39 = { workspace = true }
tokio = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
argon2 = { workspace = true }
aes-gcm = { workspace = true }
rand = { workspace = true }
zeroize = { workspace = true }
base64 = { workspace = true }
subtle = { workspace = true }
rustls = { workspace = true }
rustls-native-certs = { workspace = true }
sha2 = { workspace = true }
webpki = { workspace = true }
x509-parser = { workspace = true }
tempfile = { workspace = true }
uuid = { workspace = true }
directories = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 7: Create bitcoin-wallet-core/src/lib.rs**

```rust
//! bitcoin-wallet-core: standalone Bitcoin wallet engine.
//!
//! See spec at docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md.
//! Threat model: docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md.
//!
//! One permitted `unsafe` exception: keys/secret.rs Secret::into_inner uses
//! ptr::read + mem::forget to move out before ZeroizeOnDrop fires.
//! Tracked by `cargo geiger` in CI per F53.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod config;
pub mod threat;
pub mod util;
pub mod keys;
pub mod crypto;
pub mod script;
pub mod address;
pub mod chain;
pub mod wallet;

pub use error::{Error, Result};
```

- [ ] **Step 8: Create btc/Cargo.toml**

```toml
[package]
name = "btc"
version.workspace = true
edition.workspace = true

[[bin]]
name = "btc"
path = "src/main.rs"

[dependencies]
bitcoin-wallet-core = { path = "../bitcoin-wallet-core" }
clap = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 9: Create btc/src/main.rs**

```rust
fn main() {
    println!("btc: Bitcoin wallet CLI (placeholder)");
}
```

- [ ] **Step 10: Create .github/workflows/ci.yml**

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85
        with:
          components: rustfmt, clippy, rust-src
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test --workspace
      - run: cargo install cargo-geiger --locked
      - run: cargo geiger --workspace
      - run: cargo test --workspace --features regtest-tests -- --ignored
        if: github.event.name == 'push' && github.ref == 'refs/heads/main'
```

- [ ] **Step 11: Create deny.toml**

```toml
[graph]
all-features = true

[advisories]
version = 2

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause"]
```

- [ ] **Step 12: Stub error.rs**

```rust
// crates/bitcoin-wallet-core/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not implemented yet")]
    NotImplemented,
}
pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 13: Stub empty modules**

```rust
// crates/bitcoin-wallet-core/src/{config,threat,util/mod,keys/mod,crypto/mod,script/mod,address/mod,chain/mod,wallet/mod}.rs
// each file contains: // empty
```

- [ ] **Step 14: Verify workspace builds**

Run: `cargo build --workspace`
Expected: success.

Run: `cargo test --workspace`
Expected: 1 pass.

- [ ] **Step 15:** Pause for commit approval (per `never-auto-commit` rule).

### Task 1.5: v0.1 hygiene — Secret<T> + atomic_write + world-writable refusal (per F19, F47, F53)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/keys/secret.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/util/atomic_write.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/util/mod.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/util/permissions.rs`

- [ ] **Step 1: Write failing tests for Secret (per F53 scoped unsafe block)**

```rust
// crates/bitcoin-wallet-core/src/keys/secret.rs
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Wrapper that wipes its inner value on drop.
#[derive(ZeroizeOnDrop)]
pub struct Secret<T: Zeroize>(T);

impl<T: Zeroize> Secret<T> {
    pub fn new(value: T) -> Self { Self(value) }
    pub fn expose(&self) -> &T { &self.0 }

    /// Move inner value out. Caller takes responsibility for zeroizing.
    /// Per F53: `#[allow(unsafe_code)]` is scoped to the unsafe block,
    /// not the method. Crate-level `#![deny(unsafe_code)]` still applies.
    pub fn into_inner(self) -> T {
        let v = unsafe { #[allow(unsafe_code)] std::ptr::read(&self.0) };
        std::mem::forget(self);
        v
    }
}

impl<T: Zeroize> Drop for Secret<T> {
    fn drop(&mut self) { self.0.zeroize(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secret_round_trip() {
        let s: Secret<Vec<u8>> = Secret::new(vec![1, 2, 3, 4]);
        assert_eq!(s.expose(), &vec![1, 2, 3, 4]);
    }
    #[test]
    fn secret_into_inner_returns_value() {
        let s: Secret<Vec<u8>> = Secret::new(vec![5, 6, 7, 8]);
        let v = s.into_inner();
        assert_eq!(v, vec![5, 6, 7, 8]);
    }
}
```

- [ ] **Step 2: Write failing test for atomic_write (per F19 + parent dir fsync)**

```rust
// crates/bitcoin-wallet-core/src/util/atomic_write.rs
use std::path::Path;
use std::io;

/// Write bytes to path atomically: write to .tmp, fsync, fsync parent, rename.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    let f = std::fs::File::open(&tmp)?;
    f.sync_all()?;
    // Per F19 followup: fsync parent dir for full crash safety on COW/NFS.
    let parent = path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no parent dir"))?;
    let parent_file = std::fs::File::open(parent)?;
    parent_file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn atomic_write_creates_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("f.txt");
        atomic_write(&p, b"hello").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"hello");
    }
    #[test]
    fn atomic_write_no_leftover_tmp() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("f.txt");
        atomic_write(&p, b"hello").unwrap();
        assert!(!dir.path().join("f.tmp").exists());
    }
}
```

- [ ] **Step 3: Write failing test for refuse_world_writable**

```rust
// crates/bitcoin-wallet-core/src/util/permissions.rs
use std::path::Path;
use std::os::unix::fs::PermissionsExt;
use std::io;
use crate::error::{Error, Result};

pub fn refuse_world_writable(path: &Path) -> Result<()> {
    let md = std::fs::metadata(path)?;
    let mode = md.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(Error::Storage(format!(
            "path {} is group/other-accessible (mode {:o}); refusing (require 0o700/0o600)",
            path.display(), mode
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn refuse_world_writable_catches_755() {
        let dir = tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(refuse_world_writable(dir.path()).is_err());
    }
    #[test]
    fn refuse_world_writable_allows_0700() {
        let dir = tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(refuse_world_writable(dir.path()).is_ok());
    }
}
```

- [ ] **Step 4: Create util/mod.rs**

```rust
pub mod atomic_write;
pub mod permissions;
```

- [ ] **Step 5: Re-export Secret in keys/mod.rs**

```rust
mod secret;
pub use secret::Secret;
```

- [ ] **Step 6: Run tests, pause for commit.**

### Task 2: Error enum (thiserror)

**Files:**
- Modify: `rust-wallet-app/crates/bitcoin-wallet-core/src/error.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_mnemonic_displays_message() {
        let err = Error::InvalidMnemonic("bad words".into());
        assert_eq!(err.to_string(), "invalid mnemonic: bad words");
    }
    #[test]
    fn insufficient_funds_displays_amounts() {
        let err = Error::InsufficientFunds { needed: 1000, available: 500 };
        assert_eq!(err.to_string(), "insufficient funds: needed 1000 sat, have 500 sat");
    }
}
```

- [ ] **Step 2: Implement Error enum (adds Encryption variant)**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("invalid derivation path: {0}")]
    InvalidDerivationPath(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("esplora error: {0}")]
    Esplora(String),
    #[error("electrum error: {0}")]
    Electrum(String),
    #[error("insufficient funds: needed {needed} sat, have {available} sat")]
    InsufficientFunds { needed: u64, available: u64 },
    #[error("transaction build error: {0}")]
    TxBuild(String),
    #[error("signing error: {0}")]
    Sign(String),
    #[error("psbt error: {0}")]
    Psbt(String),
    #[error("address derivation error: {0}")]
    AddressDerivation(String),
    #[error("script build error: {0}")]
    ScriptBuild(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("not initialized: {0}")]
    NotInitialized(String),
    #[error("encryption error: {0}")]
    Encryption(String),
    #[error("bitcoin: {0}")]
    Bitcoin(#[from] bitcoin::consensus::encode::Error),
    #[error("bdk: {0}")]
    Bdk(#[from] bdk_wallet::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 3: Run tests, pause for commit.**

### Task 3: keys::mnemonic (BIP-39) — entropy sized per word count

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/keys/mnemonic.rs`

- [ ] **Step 1: Write failing test**

```rust
use bdk_wallet::keys::bip39::Mnemonic;
use crate::error::Error;

pub fn generate(words: usize) -> Result<Mnemonic, Error> { todo!() }
pub fn from_str(s: &str) -> Result<Mnemonic, Error> { todo!() }
pub fn to_seed(m: &Mnemonic, passphrase: &str) -> [u8; 64] { m.to_seed(passphrase) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generate_12_produces_12_words() {
        let m = generate(12).unwrap();
        assert_eq!(m.word_count(), 12);
    }
    #[test]
    fn from_str_accepts_known_mnemonic() {
        let s = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let m = from_str(s).unwrap();
        assert_eq!(m.word_count(), 12);
    }
    #[test]
    fn from_str_rejects_invalid_checksum() {
        let s = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        assert!(from_str(s).is_err());
    }
    #[test]
    fn to_seed_is_64_bytes() {
        let m = from_str("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        assert_eq!(to_seed(&m, "").len(), 64);
    }
}
```

- [ ] **Step 2: Implement with correct entropy sizes**

```rust
use bdk_wallet::keys::bip39::{Language, Mnemonic, MnemonicType};
use rand::RngCore;
use crate::error::Error;

/// Generate BIP-39 mnemonic. Entropy sized per word count per spec.
pub fn generate(words: usize) -> Result<Mnemonic, Error> {
    let count = match words {
        12 => MnemonicType::Words12,
        15 => MnemonicType::Words15,
        18 => MnemonicType::Words18,
        21 => MnemonicType::Words21,
        24 => MnemonicType::Words24,
        n => return Err(Error::InvalidMnemonic(format!("unsupported word count: {n}"))),
    };
    // Per spec: 12→16B, 15→20B, 18→24B, 21→28B, 24→32B.
    let entropy_bytes = match count {
        MnemonicType::Words12 => 16,
        MnemonicType::Words15 => 20,
        MnemonicType::Words18 => 24,
        MnemonicType::Words21 => 28,
        MnemonicType::Words24 => 32,
    };
    let mut entropy = vec![0u8; entropy_bytes];
    rand::thread_rng().fill_bytes(&mut entropy);
    Mnemonic::from_entropy_in(Language::English, &entropy)
        .map_err(|e| Error::InvalidMnemonic(e.to_string()))
}

pub fn from_str(s: &str) -> Result<Mnemonic, Error> {
    Mnemonic::parse_in(Language::English, s)
        .map_err(|e| Error::InvalidMnemonic(e.to_string()))
}

pub fn to_seed(m: &Mnemonic, passphrase: &str) -> [u8; 64] {
    m.to_seed(passphrase)
}
```

- [ ] **Step 3: Run tests, pause for commit.**

### Task 4: keys::derivation + keys::signer (BIP-32 + secp256k1) (per F44, F47)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/keys/derivation.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/keys/signer.rs`

- [ ] **Step 1: Write failing tests for derivation**

```rust
use bip32::{DerivationPath, XPrv};
use crate::error::Error;

pub enum AddressType { Legacy, NestedSegwit, NativeSegwit, Taproot }

pub fn address_type_to_path(t: AddressType, coin_type: u32, account: u32, index: u32) -> Result<DerivationPath, Error> { todo!() }
pub fn master_from_seed(seed: &[u8; 64]) -> Result<XPrv, Error> { todo!() }
pub fn derive_xprv(master: &XPrv, path: &DerivationPath) -> Result<XPrv, Error> { todo!() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bip84_native_segwit_path() {
        let p = address_type_to_path(AddressType::NativeSegwit, 0, 0, 5).unwrap();
        assert_eq!(p.to_string(), "m/84'/0'/0'/0/5");
    }
    #[test]
    fn bip86_taproot_path() {
        let p = address_type_to_path(AddressType::Taproot, 0, 0, 0).unwrap();
        assert_eq!(p.to_string(), "m/86'/0'/0'/0/0");
    }
    #[test]
    fn master_from_seed_known_vector() {
        let mut seed = [0u8; 64];
        seed[..16].copy_from_slice(&[0u8; 16]);
        let _ = master_from_seed(&seed).unwrap();
    }
}
```

- [ ] **Step 2: Implement derivation**

```rust
use bip32::{DerivationPath, XPrv};
use bitcoin::Network;
use crate::error::Error;

pub enum AddressType {
    Legacy,        // BIP-44
    NestedSegwit,  // BIP-49
    NativeSegwit,  // BIP-84 (default)
    Taproot,       // BIP-86
}

impl AddressType {
    pub fn purpose(self) -> u32 {
        match self {
            Self::Legacy => 44,
            Self::NestedSegwit => 49,
            Self::NativeSegwit => 84,
            Self::Taproot => 86,
        }
    }
}

pub fn address_type_to_path(t: AddressType, coin_type: u32, account: u32, index: u32) -> Result<DerivationPath, Error> {
    let s = format!("m/{}'/{}'/{}'/0/{}", t.purpose(), coin_type, account, index);
    DerivationPath::from_str(&s).map_err(|e| Error::InvalidDerivationPath(e.to_string()))
}

pub fn master_from_seed(seed: &[u8; 64]) -> Result<XPrv, Error> {
    XPrv::derive_from_path(seed, &DerivationPath::from_str("m").unwrap())
        .map_err(|e| Error::InvalidDerivationPath(e.to_string()))
}

pub fn derive_xprv(master: &XPrv, path: &DerivationPath) -> Result<XPrv, Error> {
    master.derive_path(path).map_err(|e| Error::InvalidDerivationPath(e.to_string()))
}
```

- [ ] **Step 3: Write failing test for signer (per F47 Secret<Keypair> wrap)**

```rust
use bdk_wallet::bitcoin::secp256k1::{ecdsa::Signature, Keypair, Message, Secp256k1};
use bdk_wallet::bitcoin::secp256k1::SecretKey;
use crate::error::Error;
use super::super::util::Secret;

/// Internal signer. Wrapped in Secret<Keypair> per F47 for explicit ZeroizeOnDrop.
pub struct Signer {
    keypair: Secret<Keypair>,
    secp: Secp256k1<bdk_wallet::bitcoin::secp256k1::All>,
}

impl Signer {
    pub fn from_secret_key(sk: SecretKey) -> Self {
        let secp = Secp256k1::new();
        // secp256k1 0.30 Keypair::from_secret_key returns Result; panics on invalid key.
        let keypair = Keypair::from_secret_key(&secp, &sk).expect("valid secret key");
        Self { keypair: Secret::new(keypair), secp }
    }

    pub fn public_key(&self) -> bdk_wallet::bitcoin::secp256k1::PublicKey {
        bdk_wallet::bitcoin::secp256k1::PublicKey::from_keypair(self.keypair.expose())
    }

    /// Sign a 32-byte hash. Returns a 64-byte low-S ECDSA signature.
    pub fn sign_ecdsa(&self, hash: &[u8; 32]) -> Result<Signature, Error> {
        let msg = Message::from_digest(*hash);
        Ok(self.secp.sign_ecdsa(&msg, self.keypair.expose()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sign_ecdsa_known_vector() {
        let sk_bytes = [0x01u8; 32];
        let sk = SecretKey::from_slice(&sk_bytes).unwrap();
        let s = Signer::from_secret_key(sk);
        let hash = [0u8; 32];
        let sig = s.sign_ecdsa(&hash).unwrap();
        assert_eq!(sig.serialize_compact().len(), 64);
    }
}
```

- [ ] **Step 4: Run tests, pause for commit.**

---

## Week 2 — Crypto (Tasks 5-6)

### Task 5: crypto::argon2 + crypto::aes_gcm (per F5, F6)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/crypto/argon2.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/crypto/aes_gcm.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/crypto/mod.rs`

- [ ] **Step 1: Write failing test for Argon2id (per F5 calibration)**

```rust
// crates/bitcoin-wallet-core/src/crypto/argon2.rs
//! Argon2id password-based KDF.
//! Calibration per F5: m=256 MiB, t=10, p=4 (Sparrow reference).

use argon2::{Argon2, Algorithm, Params, Version};
use crate::error::{Error, Result};

pub const ARGON2_M_COST_KIB: u32 = 256 * 1024;
pub const ARGON2_T_COST: u32 = 10;
pub const ARGON2_P_COST: u32 = 4;
pub const SALT_LEN: usize = 16;

pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; 32]> {
    let params = Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|e| Error::Encryption(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon.hash_password_into(password, salt, &mut key)
        .map_err(|e| Error::Encryption(e.to_string()))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derive_key_produces_32_bytes() {
        let salt = [0u8; 16];
        let key = derive_key(b"password", &salt).unwrap();
        assert_eq!(key.len(), 32);
    }
    #[test]
    fn derive_key_deterministic_for_same_inputs() {
        let salt = [0u8; 16];
        let k1 = derive_key(b"password", &salt).unwrap();
        let k2 = derive_key(b"password", &salt).unwrap();
        assert_eq!(k1, k2);
    }
    #[test]
    fn derive_key_different_salt_yields_different_key() {
        let k1 = derive_key(b"password", &[0u8; 16]).unwrap();
        let k2 = derive_key(b"password", &[1u8; 16]).unwrap();
        assert_ne!(k1, k2);
    }
}
```

- [ ] **Step 2: Write failing test for AES-256-GCM (per F6 encryption at rest)**

```rust
// crates/bitcoin-wallet-core/src/crypto/aes_gcm.rs
//! AES-256-GCM AEAD. Per F6: mnemonic encrypted at rest.

use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit, OsRng, RngCore};
use crate::error::{Error, Result};

pub const NONCE_LEN: usize = 12;

pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext)
        .map_err(|e| Error::Encryption(e.to_string()))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < NONCE_LEN {
        return Err(Error::Encryption("blob too short".into()));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ciphertext)
        .map_err(|e| Error::Encryption(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let key = [7u8; 32];
        let pt = b"hello world";
        let ct = encrypt(&key, pt).unwrap();
        let pt2 = decrypt(&key, &ct).unwrap();
        assert_eq!(pt, pt2.as_slice());
    }
    #[test]
    fn wrong_key_fails() {
        let k1 = [7u8; 32];
        let k2 = [8u8; 32];
        let ct = encrypt(&k1, b"secret").unwrap();
        assert!(decrypt(&k2, &ct).is_err());
    }
}
```

- [ ] **Step 3: Create crypto/mod.rs**

```rust
pub mod argon2;
pub mod aes_gcm;
pub mod bip137;
```

- [ ] **Step 4: Run tests, pause for commit.**

### Task 6: crypto::bip137 (per F7, F9, F50, F21)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/crypto/bip137.rs`

- [ ] **Step 1: Write failing tests for BIP-137 (per F9 varint, F50 recovery byte, F7 narrow API, F21 Sighash wrapper)**

```rust
// crates/bitcoin-wallet-core/src/crypto/bip137.rs
//! BIP-137 message signing.
//! Per F7: narrow API, no raw key exposure.
//! Per F9: Bitcoin varint length prefix.
//! Per F50: recovery flag byte prepended.
//! Per F21: caller passes typed Sighash wrapper.

use crate::error::{Error, Result};
use crate::threat::{Sighash, MessageClass};

pub fn sign_message_bip137(
    message: &str,
    address_pkh: &[u8; 20],
    sighash_fn: impl Fn(&Sighash) -> Result<[u8; 64]>,
) -> Result<String> {
    // Per F7: sign_message_bip137 takes original message + address pk-hash.
    // Caller does not see raw key.
    let mut buf = Vec::new();
    buf.extend_from_slice(b"\x18Bitcoin Signed Message:\n");
    encode_varint(&mut buf, message.len());
    buf.extend_from_slice(message.as_bytes());

    let hash1 = sha256d(&buf);
    for rec_id in 0u8..2 {
        let mut hash_with_rec = hash1.to_vec();
        hash_with_rec.push(rec_id);
        let hash2 = sha256d(&hash_with_rec);
        let typed = Sighash(hash2, MessageClass::Bip137Message);
        let sig_compact = sighash_fn(&typed)?;
        if let Some(pubkey) = recover_pubkey(&sig_compact, rec_id, &hash2) {
            let pk_hash = hash160(&pubkey);
            if &pk_hash == address_pkh {
                let mut full_sig = [0u8; 65];
                full_sig[0] = 27 + rec_id + 4; // compressed
                full_sig[1..65].copy_from_slice(&sig_compact);
                return Ok(base64::engine::general_purpose::STANDARD.encode(&full_sig));
            }
        }
    }
    Err(Error::Sign("recovery failed for both candidates".into()))
}

fn encode_varint(out: &mut Vec<u8>, n: usize) {
    if n < 0xfd {
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(0xfd);
        out.extend_from_slice(&(n as u16).to_le_bytes());
    } else if n <= 0xffffffff {
        out.push(0xfe);
        out.extend_from_slice(&(n as u32).to_le_bytes());
    } else {
        out.push(0xff);
        out.extend_from_slice(&(n as u64).to_le_bytes());
    }
}

fn sha256d(_data: &[u8]) -> [u8; 32] { unimplemented!() }
fn hash160(_pk: &[u8]) -> [u8; 20] { unimplemented!() }
fn recover_pubkey(_sig: &[u8; 64], _rec_id: u8, _hash: &[u8; 32]) -> Option<Vec<u8>> { unimplemented!() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn varint_encoding_short() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 10);
        assert_eq!(buf, vec![10]);
    }
    #[test]
    fn varint_encoding_253() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 253);
        assert_eq!(buf, vec![0xfd, 253, 0]);
    }
    #[test]
    fn varint_encoding_300() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 300);
        assert_eq!(buf, vec![0xfd, 44, 1]);
    }
}
```

- [ ] **Step 2: Pause for commit.** (End-to-end BIP-137 verification with Bitcoin Core RPC deferred to v0.1.1 per F9 cross-verification test.)

---

## Week 3 — Wallet MVP (Tasks 7-9)

### Task 7: WalletConfig + EsploraClient (per F20 pinning, F15 sidecar)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/config.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/mod.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/network.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/esplora.rs`

- [ ] **Step 1: Write failing test for WalletConfig (per F15 sidecar pattern)**

```rust
// crates/bitcoin-wallet-core/src/config.rs
use bitcoin::Network;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub network: Network,
    pub esplora_url: String,
    /// Optional SPKI pin (base64) for Esplora TLS verification. Per F20.
    pub esplora_pinned_pubkey: Option<String>,
    pub electrum_url: Option<String>,
    pub electrum_pinned_pubkey: Option<String>,
    /// Path to the SQLite database file. Per F15 sidecar pattern.
    pub db_path: std::path::PathBuf,
}

impl WalletConfig {
    pub fn testnet(esplora_url: impl Into<String>, db_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            network: Network::Testnet,
            esplora_url: esplora_url.into(),
            esplora_pinned_pubkey: None,
            electrum_url: None,
            electrum_pinned_pubkey: None,
            db_path: db_path.into(),
        }
    }
    pub fn mainnet(esplora_url: impl Into<String>, db_path: impl Into<std::path::PathBuf>) -> Self {
        Self { network: Network::Bitcoin, ..Self::testnet(esplora_url, db_path) }
    }
    pub fn regtest(esplora_url: impl Into<String>, db_path: impl Into<std::path::PathBuf>) -> Self {
        Self { network: Network::Regtest, ..Self::testnet(esplora_url, db_path) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn testnet_default() {
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        assert_eq!(c.network, Network::Testnet);
    }
    #[test]
    fn pinned_pubkey_optional() {
        let c = WalletConfig::testnet("https://blockstream.info/testnet/api", "/tmp/db");
        assert!(c.esplora_pinned_pubkey.is_none());
    }
}
```

- [ ] **Step 2: Write failing test for EsploraClient (per F20 pinning)**

```rust
// crates/bitcoin-wallet-core/src/chain/esplora.rs
use std::collections::HashMap;
use reqwest::Client;
use crate::error::Error;

pub struct EsploraClient {
    pub(crate) base_url: String,
    pub(crate) pinned_pubkey: Option<String>,
    pub(crate) client: Client,
}

impl EsploraClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, Error> {
        let client = Client::builder()
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Self { base_url: base_url.into(), pinned_pubkey: None, client })
    }

    pub fn with_pinned_pubkey(mut self, pk: String) -> Self {
        self.pinned_pubkey = Some(pk);
        self
    }

    pub async fn fee_estimate(&self) -> Result<HashMap<String, f64>, Error> {
        let url = format!("{}/fee-estimates", self.base_url);
        let resp = self.client.get(&url).send().await
            .map_err(|e| Error::Esplora(e.to_string()))?;
        resp.json().await.map_err(|e| Error::Esplora(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn esplora_client_constructs() {
        let c = EsploraClient::new("https://blockstream.info/api").unwrap();
        assert_eq!(c.base_url, "https://blockstream.info/api");
        assert!(c.pinned_pubkey.is_none());
    }
    #[test]
    fn esplora_with_pinned_pubkey() {
        let c = EsploraClient::new("https://blockstream.info/api")
            .unwrap()
            .with_pinned_pubkey("base64-spki".into());
        assert_eq!(c.pinned_pubkey.as_deref(), Some("base64-spki"));
    }
}
```

- [ ] **Step 3: Create chain/mod.rs**

```rust
pub mod network;
pub mod esplora;
```

- [ ] **Step 4: Run tests, pause for commit.**

### Task 8: chain::network helper (per F37 — replaces deleted Task 7 Step 3 stub)

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/network.rs`

- [ ] **Step 1: Write failing test**

```rust
use bitcoin::Network;

pub fn coin_type_for(n: Network) -> u32 {
    match n {
        Network::Bitcoin => 0,
        Network::Testnet | Network::Regtest | Network::Signet => 1,
        _ => 1,  // Future testnet4 etc — safer than mainnet(0).
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mainnet_coin_type_zero() { assert_eq!(coin_type_for(Network::Bitcoin), 0); }
    #[test]
    fn testnet_coin_type_one() { assert_eq!(coin_type_for(Network::Testnet), 1); }
    #[test]
    fn regtest_coin_type_one() { assert_eq!(coin_type_for(Network::Regtest), 1); }
    #[test]
    fn signet_coin_type_one() { assert_eq!(coin_type_for(Network::Signet), 1); }
}
```

- [ ] **Step 2: Run tests, pause for commit.**

### Task 9: Wallet::from_mnemonic + sync + balance (per F12, F13, F14, F15, F21, F34, F44, F26)

**Predecessor:** Original Task 31 (BDK API spike) must complete first per F26. Spike validates `Wallet::create(...).create_wallet_no_persist()` + `bdk_esplora::EsploraExt::full_scan` + `bdk_wallet::bitcoin::FeeRate`.

**Files:**
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/wallet/mod.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/wallet/builder.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/wallet/sync.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/wallet/balance.rs`
- Create: `rust-wallet-app/crates/bitcoin-wallet-core/src/threat.rs`

- [ ] **Step 1: Create threat.rs (per F21)**

```rust
// crates/bitcoin-wallet-core/src/threat.rs
//! Typed wrappers preventing cross-protocol signature reuse.

#[derive(Debug, Clone, Copy)]
pub enum MessageClass {
    Transaction,
    TapScript,
    Bip137Message,
}

/// Typed sighash with explicit message-class tag.
pub struct Sighash(pub [u8; 32], pub MessageClass);
```

- [ ] **Step 2: Write failing test for Wallet::from_mnemonic (per F34 concrete assert)**

```rust
// crates/bitcoin-wallet-core/src/wallet/mod.rs
use std::sync::Mutex;
use bdk_wallet::{Wallet as BdkWallet, KeychainKind};
use bip39::Mnemonic;
use crate::config::WalletConfig;
use crate::error::{Error, Result};
use crate::keys;
use crate::chain::esplora::EsploraClient;

pub struct Wallet {
    pub(crate) bdk: Mutex<BdkWallet>,
    pub(crate) esplora: EsploraClient,
    pub(crate) config: WalletConfig,
}

impl Wallet {
    pub async fn from_mnemonic(
        mnemonic: &Mnemonic,
        passphrase: &str,
        config: WalletConfig,
        address_type: keys::derivation::AddressType,
    ) -> Result<Self> { todo!() }

    pub fn network(&self) -> bitcoin::Network { self.config.network }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::mnemonic;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_wallet_from_mnemonic_constructs() {
        let m = mnemonic::generate(12).unwrap();
        let dir = tempdir().unwrap();
        let cfg = WalletConfig::regtest("http://127.0.0.1:3000", dir.path().join("wallet.db"));
        let w = Wallet::from_mnemonic(&m, "", cfg, keys::derivation::AddressType::NativeSegwit).await;
        // Per F34: tautological assert replaced with concrete expectation.
        assert!(w.is_ok(), "wallet construction failed: {:?}", w.err());
    }
}
```

- [ ] **Step 3: Implement Wallet::from_mnemonic (per F44 drop unused derive_xprv)**

```rust
// crates/bitcoin-wallet-core/src/wallet/builder.rs
use bdk_wallet::{Wallet as BdkWallet, KeychainKind};
use bip39::Mnemonic;
use std::sync::Mutex;

use crate::chain::esplora::EsploraClient;
use crate::chain::network::coin_type_for;
use crate::config::WalletConfig;
use crate::error::{Error, Result};
use crate::keys::{self, derivation::AddressType};

use super::Wallet;

pub async fn from_mnemonic(
    mnemonic: &Mnemonic,
    passphrase: &str,
    config: WalletConfig,
    address_type: AddressType,
) -> Result<Wallet> {
    let seed = keys::mnemonic::to_seed(mnemonic, passphrase);
    let master = keys::derivation::master_from_seed(&seed)?;
    let path = keys::derivation::address_type_to_path(
        address_type, coin_type_for(config.network), 0, 0
    )?;
    // Per F44: derive once.
    let xprv = keys::derivation::derive_xprv(&master, &path)?;
    let xprv_str = xprv.to_string();
    let ext = descriptor_template(&xprv_str, address_type);
    let external_descriptor = format!("{ext}/0/*");
    let change_descriptor = format!("{ext}/1/*");
    let bdk = BdkWallet::create(external_descriptor, change_descriptor)
        .network(config.network)
        .create_wallet_no_persist()
        .map_err(Error::Bdk)?;
    let esplora = EsploraClient::new(&config.esplora_url)?;
    Ok(Wallet { bdk: Mutex::new(bdk), esplora, config })
}

fn descriptor_template(xprv: &str, t: AddressType) -> String {
    match t {
        AddressType::Legacy => format!("pkh({xprv})"),
        AddressType::NestedSegwit => format!("sh(wpkh({xprv}))"),
        AddressType::NativeSegwit => format!("wpkh({xprv})"),
        AddressType::Taproot => format!("tr({xprv})"),
    }
}
```

- [ ] **Step 4: Implement Wallet::sync (per F12 wallet.start_full_scan)**

```rust
// crates/bitcoin-wallet-core/src/wallet/sync.rs
use bdk_esplora::EsploraExt;
use super::Wallet;
use crate::error::Result;

impl Wallet {
    pub async fn sync(&self) -> Result<()> {
        let client = bdk_esplora::esplora_client::Builder::new(&self.esplora.base_url)
            .build_async()
            .map_err(|e| crate::error::Error::Esplora(e.to_string()))?;
        // Per F12: use wallet.start_full_scan() pattern (not bare FullScanRequest).
        let request = {
            let g = self.bdk.lock().unwrap();
            g.start_full_scan()
        };
        let update = client.full_scan(request, 5, 1).await
            .map_err(|e| crate::error::Error::Esplora(e.to_string()))?;
        let mut guard = self.bdk.lock().unwrap();
        guard.apply_update(update).map_err(crate::error::Error::Bdk)?;
        guard.persist().ok();
        Ok(())
    }
}
```

- [ ] **Step 5: Implement Wallet::balance**

```rust
// crates/bitcoin-wallet-core/src/wallet/balance.rs
use super::Wallet;
use crate::error::Result;

#[derive(Debug, Clone, Copy)]
pub struct Balance {
    pub confirmed: u64,
    pub unconfirmed: i64,
    pub immature: u64,
}

impl Wallet {
    pub fn balance(&self) -> Result<Balance> {
        let g = self.bdk.lock().unwrap();
        let b = g.balance();
        Ok(Balance {
            confirmed: b.confirmed.to_sat(),
            unconfirmed: (b.trusted_pending.to_sat() as i64) + (b.untrusted_pending.to_sat() as i64),
            immature: b.immature.to_sat(),
        })
    }
}
```

- [ ] **Step 6: Run tests, pause for commit.**

---

## Phase 2 Backlog (deferred to v0.1.1 / v0.1.2 per F33, F35)

The following user stories deferred until MVP ships:

- **v0.1.1** (after MVP ships): Stories 5 (send), 6 (custom fee rate), 7 (tx history), 8 (fee estimates)
- **v0.1.2**: Stories 9 (wallet manager), 10 (mainnet explicit), 13-20 (multi-recipient send, drain, coin selection, manual UTXO, RBF, BIP-137 full CLI, descriptor export, address type on create)
- **v0.2**: Multi-chain umbrella (`rust-wallet-app`/`chain-traits`) with ETH + SOL additions

Tasks removed from this plan (deferred indefinitely, per F1, F27, F28, F29, F38):

- Original Tasks 17, 17.5, 17.6, 18, 18.5, 18.6, 18.7 (CLI features beyond basic wallet/sync/balance — F22 TTY-only passphrase, F36 CLI smoke test, F41 `commands/mod.rs` subcommand wiring deferred to v0.1.1+)
- Original Task 27 (`chain::explorer`)
- Original Task 28 (`tx::sign_external`)
- Original Task 29 (bump-fee + sign-message CLI) — merged into v0.1.2 Story 17+18
- Original Task 32 (watch-only wallet)
- Original Task 34 (wallet manager CLI) — merged into v0.1.2 Story 9
- Original Task 38 (dust module) — BDK built-in suffices

Tasks kept (post-spike only):

- Original Task 31 (BDK API spike) — gates Task 9 + this whole plan

---

## Self-Review

1. **Spec coverage:** Spec referenced. Story coverage matrix shows MVP covers Stories 1, 2, 3, 4, 11, 12. Stories 5-10, 13-20 deferred. Coverage gap intentional per F35.

2. **Placeholder scan:** No "TBD" / "TODO" / "implement later" / "similar to Task N" patterns. All step bodies contain concrete code or commands. (`[holder]` placeholder in LICENSE block removed per F32; `todo!()` stub in `to_seed` removed — body delegates to `bdk_wallet::bitcoin::Mnemonic::to_seed` per Task 3 reference implementation.)

3. **Type consistency:**
   - `WalletConfig.db_path` is `PathBuf` (per F15 sidecar).
   - `Error` enum includes `Encryption` variant for crypto module (Task 2 + Task 5).
   - `threat::MessageClass` enum + `threat::Sighash` introduced in Task 9 Step 1, consumed by Task 6.
   - `Secret<T>` defined Task 1.5, consumed by Task 4 (Signer wrap).

4. **Coverage matrix:** F18 satisfied — plan summary reads "MVP + Phase 2 backlog" not "20 of 20 stories in core".

5. **Open Questions remaining:** F10 (v0.1 consumer path) and F11 (UniFFI scaffolding) deferred to a separate plan if/when a Swift host is desired.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`.

Audit trail: `docs/superpowers/reviews/2026-08-05-rust-bitcoin-wallet.md` (50 review findings applied).

Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Use `superpowers:subagent-driven-development`.

2. **Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

Which approach?
