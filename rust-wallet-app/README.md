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
