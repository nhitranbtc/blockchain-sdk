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

## `btc` CLI

The `btc` binary (crate `btc/`, subcommand of `bitcoin-wallet-core/`) is the
command-line entry point for wallet operations. Source: `crates/btc/src/{main,cli,handlers}.rs`.

### Subcommands

| Subcommand | Purpose | Source |
|---|---|---|
| `btc wallet create` | Generate BIP-39 mnemonic, persist encrypted wallet. Mnemonic → STDERR; wallet_id → STDOUT (L28/F49). | PR #70 |
| `btc wallet show <id>` | Decrypt + sync + print addresses + balance JSON. `<id>` is positional (UUID v4 from `wallet create`). | PR #70 |
| `btc wallet sync` | Stateless chain scan against an Esplora server. Prints `n_utxos=<N> total_sat=<S>`. | PR #63 |
| `btc wallet balance` | Stateless balance query. Prints sats integer. | PR #63 |
| `btc encrypt` | Encrypt a UTF-8 file with Argon2id + AES-256-GCM. Output is `MnemonicCipherBlob` (salt(16) \|\| nonce(12) \|\| ct \|\| tag(16)). | PR #62 |
| `btc decrypt` | Decrypt a `MnemonicCipherBlob`. | PR #62 |
| `btc message sign` | BIP-137 message signing. v0.1: P2PKH only. | PR #61 |
| `btc message verify` | BIP-137 message verification. Exits 0 if valid, 1 if invalid. | PR #61 |

### `btc wallet create` flags

| Flag | Notes |
|---|---|
| `--words <N>` | Required. `N` ∈ {12, 15, 18, 21, 24}. |
| `--network <NET>` | Required. `bitcoin` / `testnet` / `testnet4` / `signet` / `regtest`. |
| `--password <PWD>` | Optional. If omitted, prompts via `/dev/tty` (`rpassword`). |

### `btc wallet show <id>` flags

| Flag | Notes |
|---|---|
| `<id>` | Positional. UUID v4 returned by `wallet create`. |
| `--network <NET>` | Required. |
| `--password <PWD>` | Optional. If omitted, prompts. |
| `--esplora-url <URL>` | Optional. Default: `blockstream.info/<NET>/api` (PR #74). |
| `--esplora-spki-pin <HEX64>` | Optional. 64-char hex (SHA-256 of leaf cert SubjectPublicKeyInfo). Routes `EsploraClient` via `TlsPolicy::Pinned`. Env: `BTC_ESPLORA_SPKI_PIN`. |

### `btc wallet sync` / `btc wallet balance` flags

| Flag | Notes |
|---|---|
| `--mnemonic <words>` | Required. 12/15/18/21/24 words. Visible in shell history — prefer piping. |
| `--network <NET>` | Required. |
| `--esplora-url <URL>` | Required. HTTPS-only (F36); regtest exempt for localhost. |
| `--pin-spki <HEX64>` | Optional. 64-char hex. Required for non-regtest networks (F20). Alias: env `BTC_ESPLORA_SPKI_PIN`. |

### `btc encrypt` / `btc decrypt` flags

| Flag | Notes |
|---|---|
| `--in <PATH>` | Required. Input file (UTF-8 plaintext for encrypt, binary blob for decrypt). |
| `--out <PATH>` | Required. Output file. |
| `--password <PWD>` | Optional. Falls back to env or `/dev/tty` prompt. |
| `--password-file <PATH>` | Optional. Reads password from file. Rejects symlinks + world/group-readable files. |
| `--password-stdin` | Optional. Reads password from stdin. |

The three password flags are **mutually exclusive** at the clap parse layer.

### `btc message sign` flags

| Arg | Notes |
|---|---|
| `--mnemonic <words>` | Required. |
| `--network <NET>` | Required. |
| `--address <ADDR>` | Required. v0.1: must match the first external receive address at `m/44'/coin'/0'/0/0`. |
| `<message>` | Required. Positional (no `--message` flag). Quoted on the command line. |

### `btc message verify` flags

| Arg | Notes |
|---|---|
| `--address <ADDR>` | Required. Bitcoin address that allegedly signed the message. |
| `<message>` | Required. Positional. Message text that was signed. |
| `<signature>` | Required. Positional. Base64-encoded BIP-137 signature (output of `btc message sign`). |

### Environment variables

| Env var | Effect |
|---|---|
| `BTC_ENCRYPT_PASSWORD` | Fallback for `--password` on `btc encrypt` |
| `BTC_DECRYPT_PASSWORD` | Fallback for `--password` on `btc decrypt` |
| `BTC_ESPLORA_SPKI_PIN` | Fallback for `--esplora-spki-pin` (wallet show) and `--pin-spki` (wallet sync/balance) |
| `BTC_WALLET_MNEMONIC` | Fallback for `--mnemonic` on wallet sync/balance |
| `BTC_DATA_DIR` | Wallet storage location (default: `$XDG_DATA_HOME/btc`) |

### Build + run

```bash
cd rust-wallet-app
cargo build --release -p btc
./target/release/btc --help
```

### Examples

```bash
# Create a new testnet wallet
btc wallet create --words 12 --network testnet --password-stdin

# Show a wallet by ID (load, decrypt, sync, print JSON)
btc wallet show <UUID> --network testnet --esplora-spki-pin <HEX64>

# Stateless sync (no wallet persistence)
btc wallet sync \
  --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" \
  --network testnet \
  --esplora-url https://blockstream.info/testnet/api \
  --pin-spki <HEX64>

# Stateless balance
btc wallet balance \
  --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" \
  --network testnet \
  --esplora-url https://blockstream.info/testnet/api

# Encrypt a file
btc encrypt --in secret.txt --out secret.enc --password-stdin

# Decrypt a file
btc decrypt --in secret.enc --out secret.txt --password-file /run/secrets/btc-pwd

# Sign a message
btc message sign \
  --mnemonic "abandon abandon ... about" \
  --network testnet \
  --address mjQi1wx8Xg2tKbEKHW7KBcvz2kELnyfXD3 \
  "hello world"

# Verify a message
btc message verify \
  --address mjQi1wx8Xg2tKbEKHW7KBcvz2kELnyfXD3 \
  "hello world" \
  <BASE64>
```

### Security notes

- **Mnemonic on STDERR**, wallet_id on STDOUT (L28/F49 separation).
- **Manual `Debug` impls redact** `password` / `password_file` / `mnemonic` (L12 CRITICAL #2). See `crates/btc/src/cli.rs` lines 278-452.
- **`--password-file` rejects** symlinks + world/group-readable files (F19 pattern).
- **Three password flags are mutually exclusive** at the clap parse layer.
- **Live testnet smoke tests are `#[ignore]`'d** (L29); run manually with `cargo test --workspace -- --ignored`.
