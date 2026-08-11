# rust-wallet-app

Multi-chain Rust wallet.

## Status

- **v0.1** (Bitcoin): `bitcoin-wallet-core/` — done in sibling repo
- **v0.2** (umbrella cut): scaffold + `bitcoin-wallet-core/` integration — in progress
- **v0.3+**: ETH, SOL, LTC, DOGE, BCH, ... per
  [`docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md`](../docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md) §2

## Layout

```text
rust-wallet-app/
├── Cargo.toml                 (workspace)
└── crates/
    └── chain-traits/          (defines ChainWallet trait)
```

`bitcoin-wallet-core/` will be path-dep'd into the workspace when present
on disk; pending verification.

## ChainWallet trait

Defined in `crates/chain-traits/src/lib.rs`:

```rust
#[async_trait]
pub trait ChainWallet: Send + Sync {
    fn chain_id(&self) -> ChainId;
    async fn sync(&self) -> Result<(), ChainError>;
    async fn next_receive_address(&self) -> Result<Address, ChainError>;
    async fn balance(&self) -> Result<u128, ChainError>;
}
```

Per-chain crates (BTC, ETH, SOL, ...) implement this trait. Umbrella code
dispatches via trait. Per-chain crates own their own DB + signer + RPC.

## Build

```bash
cd rust-wallet-app
cargo build --workspace
cargo test --workspace
```

## What's New

Recent merges (full history in [`CHANGELOG.md`](../../CHANGELOG.md)):

- **PR #61** (Issue #61 / Task 54a) — `btc message sign` + `btc message verify` subcommands (stateless BIP-137). `sign` derives first external address from BIP-39 mnemonic + signs message; `verify` returns `true`/`false`. Manual `Debug` redacts mnemonic + signature. v0.1: P2PKH only.
- **PR #74** (Issue #74) — `btc wallet show --network <NET>` defaults to network-appropriate Esplora URL: mainnet (`blockstream.info/api`), testnet (`blockstream.info/testnet/api`), signet (`blockstream.info/signet/api`), testnet4 (`mempool.space/testnet4/api`). Regtest has no default — operator must pass `--esplora-url` (HTTPS-only per F20; regtest localhost needs stunnel).
- **PR #73** (Issue #73, F20 enforcement) — `btc wallet show --esplora-spki-pin <HEX64>` + `BTC_ESPLORA_SPKI_PIN` env. Routes `EsploraClient` via `from_config` with `TlsPolicy::Pinned` when set; preserves PR-2 `SystemRoots` default when unset (testnet-suitable). Closes F20 gap on mainnet/signet/regtest production endpoints.
- **PR #70** (Task 54d, PR-2 of #64) — `btc wallet create` + `btc wallet show` clap 4 subcommands on top of the PR-1 wallet-store lib. `create` persists encrypted wallet (mnemonic → STDERR; wallet_id → STDOUT per L28/F49). `show` decrypts + syncs + prints addresses + balance JSON. Manual `Debug` redaction on `Cli`/`Commands`/`WalletAction` (L12 CRITICAL #2). Closes Story #13.
- **PR #55** (Task 9 #19b.2) — `Wallet::sync(&EsploraClient)` + `Wallet::balance(&EsploraClient)`. Full chain scan via Esplora `/address/{addr}/utxo` + `bdk_wallet::Wallet::insert_txout`. F12 + F13 defended; F14 persistence deferred to v0.1.1. Caller builds `EsploraClient` with explicit `TlsPolicy` for F20 SPKI pinning.
- **PR #48** (Task 9a) — `Wallet::from_mnemonic(&Mnemonic, Network) -> Result<Wallet>`. F34 BIP-39 word-count assertion; no `Default` impl per CONTEXT.md hard rule #1.
- **PR #42** (Task 8) — `chain::network::coin_type_for(Network) → u32`. BIP-44 coin-type lookup; hard rule #1 (no mainnet default) enforced at compile time via exhaustive match.
- **PR #39** (Task 6 / F21 follow-up) — `MessageHash<C>` phantom-typed wrapper. `sign_recoverable` requires `MessageHash<Bip137Message>`; U5 (arbitrary-hash phishing) defended at the type level.
- **PR #38** (audit) — L20 constant audit. All crypto constants (`ARGON2_M/T/P_COST`, `SALT_LEN`, `KEY_LEN`, `MAGIC_PREFIX`, etc.) compile-time pinned.
- **PRs #34 / #33 / #27 / #26** (Tasks 4–7) — Keys (BIP-32), Argon2id KDF + AES-256-GCM, BIP-137 message signing, WalletConfig + EsploraClient (F20 SPKI pinning).
