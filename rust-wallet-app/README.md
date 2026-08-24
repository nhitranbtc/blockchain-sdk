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

## `eth` CLI

The `eth` binary (crate `eth/`, subcommand of `eth-wallet-core/`) is the command-line entry point for Ethereum wallet operations. Source: `crates/eth/src/{main,handlers}.rs`. Built on alloy v1.8.x; anvil regtest + Sepolia testnet. v0.3.x PR-A wires the sync + read subcommands (#337); PR-B lands sign + broadcast.

### Subcommands

| Subcommand | Purpose | Source |
|---|---|---|
| `eth wallet create --name --password --network` | Generate BIP-39 mnemonic, persist encrypted wallet. Prints `wallet_id`. | #337 PR-A + #301 Task 2 |
| `eth wallet import --name --password --network --mnemonic \| --private-key` | Import existing BIP-39 mnemonic (12/15/18/21/24 words) OR raw secp256k1 private-key hex. | #337 PR-A + #301 Task 2 |
| `eth wallet list` | List all wallets on disk (reads `<wallet_id>.meta.json` per network). | #337 PR-A |
| `eth wallet show --name \| --id --network` | Show wallet metadata (name, address, derivation path). | #337 PR-A |
| `eth wallet delete --name \| --id --network` | Delete a wallet by name or UUID. | #337 PR-A |
| `eth wallet balance --address --unit [--rpc-url]` | ETH balance of `address` via Anvil/Sepolia RPC. `--unit` ∈ `wei` / `gwei` / `eth`. | #337 PR-A + #305 Task 6 |
| `eth tx get --tx-hash [--rpc-url]` | Look up transaction by 32-byte hex hash. | #337 PR-A + #305 Task 6 |
| `eth wallet send-native` / `eth wallet send-erc20` / `eth tx list` | PR-B. Returns `Error::Rpc("...wired in PR-B follow-up (#337 phase 2)")`. | deferred |
| `eth erc20 *` / `eth fee` / `eth config` / `eth sign-message` / `eth sign-typed` / `eth wallet sync` | PR-B / never (scaffold). | deferred |

### Global flags

| Flag | Notes |
|---|---|
| `--rpc-url <URL>` (env `ETH_RPC_URL`, default `http://127.0.0.1:8545`) | Provider endpoint for read paths. Loopback default; SPKI pin deferred per #330. |
| `--data-dir <PATH>` (env `ETH_DATA_DIR`) | Wallet-store base directory. Default: `$XDG_DATA_HOME/nhitran/eth-wallet-core/wallets`. Tests inject a `tempfile::TempDir`. |

### `eth wallet create` / `eth wallet import` flags

| Flag | Notes |
|---|---|
| `--name <NAME>` | Required. User-facing wallet handle. Must match `^[A-Za-z0-9 _-]{1,32}$` (PR-B hardening — currently any UTF-8 string). |
| `--password <PWD>` | Required. ⚠️ Currently argv-visible (shell history + process list). PR-B replaces with `rpassword` / stdin. |
| `--network <NET>` | Default `sepolia`. Accepts `mainnet` / `sepolia` / `anvil` / chain-id `1` / `11155111` / `31337` / `dev` / `local`. Unknown → exit 2 (`Error::InvalidInput`). |

### `eth wallet balance` flags

| Flag | Notes |
|---|---|
| `--address <ADDR>` | Required. 20-byte hex address (EIP-55 checksum optional). |
| `--unit <UNIT>` | Optional. `wei` / `gwei` / `eth` (default `eth`). |
| `--rpc-url <URL>` | Optional. See global flags. |

### `eth tx get` flags

| Flag | Notes |
|---|---|
| `--tx-hash <HEX>` | Required. 32-byte hex hash with or without `0x` prefix. Unknown → exit 3 (`Error::Rpc`). |

### Environment variables

| Env var | Effect |
|---|---|
| `ETH_RPC_URL` | Fallback for global `--rpc-url` |
| `ETH_DATA_DIR` | Fallback for global `--data-dir` |
| `RUN_ANVIL_E2E` | Set to `1` to enable Anvil-gated binary integration tests (`cargo test -p eth --test cli_localnet -- --ignored`). |

### Build + run

```bash
cd rust-wallet-app
cargo build --release -p eth
./target/release/eth --help
./target/release/eth wallet --help
```

### Examples

```bash
# Local Anvil-backed wallet flow (loopback RPC, ephemeral data dir)
ETH_DATA_DIR=/tmp/eth-demo ANVIL=$(which anvil)
if [ -z "$ANVIL" ]; then echo "install foundry"; exit 1; fi
$ANVIL &  # boots at http://127.0.0.1:8545 with 10 prefunded dev accounts
sleep 1

eth wallet create --name alpha --password p1 --network anvil
eth wallet list
eth wallet show --name alpha --network anvil
eth wallet balance --address 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
eth tx get --tx-hash 0x0000000000000000000000000000000000000000000000000000000000000000

# Hermetic per-test wallet store via env
ETH_DATA_DIR=/tmp/eth-test eth wallet create --name beta --password p2

# Sepolia flow (operator-driven per L29 — needs funded wallet + RPC)
ETH_RPC_URL=https://sepolia.infura.io/v3/<KEY> eth wallet balance --address 0x...
```

### Security notes

- **Mnemonic / private-key on argv** — `eth wallet import --mnemonic "<words>"` and `--password` expose secrets to shell history + `ps auxe`. PR-B replaces `--password` with `rpassword` / stdin. `--mnemonic` will get the same treatment when PR-B lands sign + broadcast.
- **Encrypted blob file mode 0o600** — per #337 PR-A, `WalletManager::write_atomic` sets explicit permissions on Unix targets; default umask 0022 would have left blobs world-readable.
- **RPC SPKI pin removed** — `provider::new_http` uses default rustls TLS + system CAs (per #330). Reintroducing SPKI pin requires `rustls::client::WebPkiServerVerifier` composition + `webpki` verifier (out of scope for #337).
- **Live testnet smoke tests** (`tests/erc20_anvil.rs`, the `#[ignore]` tests in `cli_localnet.rs`) are operator-driven per L29; run with `RUN_ANVIL_E2E=1` (or `RUN_ETH_E2E=1` for Sepolia e2e). Default `cargo test` runs the always-on sync subset.
