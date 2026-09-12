# sol-wallet-core

Solana wallet engine on Anza SDK 4.1.0. Phantom-equivalent Wallet API + SPL/Token-2022 builders + JSON-RPC client + Argon2id/AES-GCM persistence + FFI cdylib.

**Branch:** `rust-sol-core` · **Status:** v0.1 release candidate (Phase 8.1 merged, Phase 10 mainnet smoke gate pending)

> ⚠️ **Devnet only** — the example below targets `https://api.devnet.solana.com`. Do NOT point at mainnet or testnet without first reviewing the [Phase 9 security audit](../../audit/2026-09-12-sol-wallet-core-phase9-security-audit.md) controls P9-4 + C-P9-2.

## Quick Start — create a wallet on devnet

### Prerequisites

- Rust 1.98.1+ (workspace MSRV)
- Solana CLI (optional — for `solana-keygen recover` cross-check)
- Unix-like OS for mode 0o600 enforcement (Windows `SetSecurityInfo` deferred to V0.1.5)

### Run the example

```text
cargo run -p sol-wallet-core --example create_wallet_devnet -- /tmp/sol-wallet-example
```

The example:

1. Generates a random 12-word BIP-39 mnemonic (no hardcoded phrase)
2. Derives a Phantom-compatible wallet at `m/44'/501'/0'/0'`
3. Encrypts the 64-byte secret via Argon2id + AES-256-GCM (AAD-bound)
4. Writes the mnemonic to `/tmp/sol-wallet-example/mnemonic.txt` with mode 0o600
5. Prints the base58 wallet address to stderr

### Environment variables

| Var | Default | Purpose | Audit |
|-----|---------|---------|-------|
| `EXAMPLE_WALLET_PASSWORD` | random `change-me-<timestamp>` | Wallet encryption password (no plaintext literal) | **P9-1** |
| `SOL_RPC_URL` | `https://api.devnet.solana.com` | RPC endpoint override | **P9-4** |
| `RUN_SOL_DEVNET_AIRDROP` | unset (no airdrop) | Explicit opt-in for devnet airdrop | **C-P9-2** |
| `CI` | unset | When set to any value, example early-returns without side effects | **P9-8** |

### With devnet airdrop (gated)

```text
RUN_SOL_DEVNET_AIRDROP=1 \
    cargo run -p sol-wallet-core --example create_wallet_devnet -- /tmp/sol-wallet-example
```

Airdrop cap: 0.5 SOL (P9-7 — below typical devnet per-request limit). Verify on Solana Explorer:
`https://explorer.solana.com/tx/<SIGNATURE>?cluster=devnet`.

### With custom password

```text
EXAMPLE_WALLET_PASSWORD='my-secret' \
    cargo run -p sol-wallet-core --example create_wallet_devnet -- /tmp/sol-wallet-example
```

> Set `EXAMPLE_WALLET_PASSWORD` to a non-default value for repeatable dev cycles. The time-based default exists so the example never embeds a plaintext password in source.

## Library use

```rust
use sol_wallet_core::{
    generate_12_word_english,
    wallet::Wallet,
    wallet_manager::WalletManager,
    platform::storage::FileWalletStorage,
};

let phrase = generate_12_word_english()?;
let wallet = Wallet::from_mnemonic(&phrase)?;
let pubkey = wallet.public_key();
```

## FFI (Dart / Swift / Kotlin)

```c
#include "sol_wallet_core.h"

sol_wallet_init("/path/to/data", 14);
sol_wallet_set_rpc("https://api.devnet.solana.com", 32);
char mnemonic[256];
sol_wallet_create_mnemonic(out_id, ..., out_mnemonic, sizeof(mnemonic), password, ...);
```

Full FFI surface: [src/ffi.rs](src/ffi.rs) + generated [sol_wallet_core.h](sol_wallet_core.h).

## Security

| Audit | Phase | Link |
|-------|-------|------|
| Phase 6 (wallet persistence) | 6 | [2026-09-11-sol-wallet-core-phase6-security-review.md](../../audit/2026-09-11-sol-wallet-core-phase6-security-review.md) |
| Phase 7 (CLI) | 7 | [2026-09-11-sol-wallet-core-phase7-security-review.md](../../audit/2026-09-11-sol-wallet-core-phase7-security-review.md) |
| Phase 8 (FFI cdylib) | 8.1 | [2026-09-11-sol-wallet-core-phase8-security-review.md](../../audit/2026-09-11-sol-wallet-core-phase8-security-review.md) |
| Phase 9 (create-wallet example) | 9 | [2026-09-12-sol-wallet-core-phase9-security-audit.md](../../audit/2026-09-12-sol-wallet-core-phase9-security-audit.md) |

## Architecture

See [docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md](../../superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md) and [docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md](../../wallets/2026-09-08-solana-rust-sdks-deep-dive.md).

## License

Workspace default (MIT OR Apache-2.0).
