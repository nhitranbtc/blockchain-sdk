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

- **PR #42** (Task 8) — `chain::network::coin_type_for(Network) → u32`. BIP-44 coin-type lookup; hard rule #1 (no mainnet default) enforced at compile time via exhaustive match.
- **PR #39** (Task 6 / F21 follow-up) — `MessageHash<C>` phantom-typed wrapper. `sign_recoverable` requires `MessageHash<Bip137Message>`; U5 (arbitrary-hash phishing) defended at the type level.
- **PR #38** (audit) — L20 constant audit. All crypto constants (`ARGON2_M/T/P_COST`, `SALT_LEN`, `KEY_LEN`, `MAGIC_PREFIX`, etc.) compile-time pinned.
- **PRs #34 / #33 / #27 / #26** (Tasks 4–7) — Keys (BIP-32), Argon2id KDF + AES-256-GCM, BIP-137 message signing, WalletConfig + EsploraClient (F20 SPKI pinning).
