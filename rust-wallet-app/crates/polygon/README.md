# `polygon` — Polygon PoS wallet CLI

## Build

```bash
cargo build -p polygon
cargo build -p polygon --examples   # operator-driven binaries (see examples/README.md)
```

## Conventions

| Convention | Default | Override |
|---|---|---|
| Network | `amoy` (`mainnet` = 137) | `--network` / `POLYGON_NETWORK` |
| RPC endpoint (Amoy) | `https://polygon-amoy-bor-rpc.publicnode.com` | `--rpc-url` / `POLYGON_RPC_URL` |
| RPC endpoint (Mainnet) | `https://polygon-bor-rpc.publicnode.com` | `--rpc-url` / `POLYGON_RPC_URL` |
| Wallet data dir | `$XDG_DATA_HOME/polygon/` | `--data-dir` / `POLYGON_DATA_DIR` |
| Wallet unlock secret | TTY prompt | `--password` / `POLYGON_PASSWORD` (single-use) |

## Command surface

```text
polygon <command> [subcommand] [flags]
```

Nine top-level commands: `version`, `wallet` (CRUD + send/sync),
`tx`, `erc20`, `fee`, `config`, `faucet`, `sign-message`,
`sign-typed`. Full flag reference below.

### Flag reference

One table per command (same `Command` column collapsed). `XOR` in **Req**
marks flags bound by `clap` `ArgGroup` / `conflicts_with` — collectively
required or mutually exclusive (see Notes).

#### `version`

No flags.

```bash
polygon version
```

#### `wallet create`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `String` | ✓ | — | unique label for this wallet in the store. |
| `--password` | `Option<String>` | — | — | L54 chain (argv → env → TTY). Skip flag + env to trigger prompt. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. Overrides env `POLYGON_NETWORK`. |
| `--derivation-path` | `String` | — | `m/44'/60'/0'/0/0` | BIP-44 path used to derive the secret key from the generated mnemonic. |
| `--account-index` | `u32` | — | `0` | child index appended to `--derivation-path` for HD derivation. |
| `--legacy-token-symbol` | `bool` | — | `false` | emit legacy `MATIC` symbol on output (deprecated post-rename to POL). |
| `--rpc-url` | `Option<String>` | — | — | per-action override of global `--rpc-url` / `POLYGON_RPC_URL`. |

```bash
polygon wallet create --name ops --network amoy
# password via TTY prompt (omit --password + POLYGON_PASSWORD)
```

#### `wallet import`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `String` | ✓ | — | unique label for the imported wallet. |
| `--password` | `Option<String>` | — | — | L54 chain. Encrypts the imported secret at rest. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--mnemonic` | `Option<SecretMnemonic>` | XOR | — | 12/24-word BIP-39 phrase. Wrapped for zero-on-drop. Visible to sibling processes via `/proc/<pid>/cmdline`; PR #456. |
| `--mnemonic-file` | `Option<PathBuf>` | XOR | — | mode-0600 file containing the whitespace-separated phrase. Closes argv-exposure hole; #528. |
| `--private-key` | `Option<String>` | XOR | — | hex PK (`0x`-prefixed or bare). Argv-exposed; prefer `--private-key-file`. #469. |
| `--private-key-file` | `Option<PathBuf>` | XOR | — | mode-0600 file containing raw PK bytes (no `0x` prefix). #469. |
| `--account-index` | `u32` | — | `0` | child index for BIP-44 derivation (mnemonic paths only). |
| `--legacy-token-symbol` | `bool` | — | `false` | emit legacy `MATIC` symbol on output. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon wallet import --name ops --network amoy --mnemonic "word1 word2 ... word12"
polygon wallet import --name ops --network amoy --private-key-file ./pk.hex
```

#### `wallet list`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--network` | `String` | — | `amoy` | only wallets under this network are listed. |
| `--all` | `bool` | — | `false` | list across every known network. |
| `--json` | `bool` | — | `false` | emit JSON array of wallet names instead of one-per-line text. |

```bash
polygon wallet list --network amoy
polygon wallet list --all --json
```

#### `wallet show`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `Option<String>` | XOR | — | look up by label. XOR `--id`. |
| `--id` | `Option<String>` | XOR | — | look up by wallet UUID. XOR `--name`. |
| `--network` | `String` | — | `amoy` | scopes the `--name` lookup to one network. |
| `--addresses` | `bool` | — | `false` | include derived addresses in the output. |
| `--export` | `bool` | — | `false` | emit extended fields suitable for backup / migration. |
| `--json` | `bool` | — | `false` | JSON formatter for the wallet info record. |

```bash
polygon wallet show --name ops --network amoy
polygon wallet show --id 0206db54-6e4b-47e0-9713-183187f2b97d --json
```

#### `wallet delete`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `Option<String>` | XOR | — | resolve then delete by label. XOR `--id`. |
| `--id` | `Option<String>` | XOR | — | delete by UUID. XOR `--name`. |
| `--network` | `String` | — | `amoy` | scopes the `--name` lookup. |

```bash
polygon wallet delete --id 0206db54-6e4b-47e0-9713-183187f2b97d --network amoy
polygon wallet delete --name ops --network amoy
```

#### `wallet balance`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--address` | `Address` | ✓ | — | EIP-55 checksum or lowercase hex. Holder address, not wallet-name. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--unit` | `String` | — | `pol` | `pol` (18-decimal) or `wei` (raw U256). |
| `--legacy-token-symbol` | `bool` | — | `false` | emit legacy `MATIC` symbol on output. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon wallet balance --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --network amoy
polygon wallet balance --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --unit wei
```

#### `wallet sync`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--address` | `Address` | ✓ | — | holder address to scan for inbound + outbound Transfer events. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |
| `--json` | `bool` | — | `false` | emit `Vec<TxSummary>` JSON (fields: `block_number`, `tx_hash`, `from`, `to`, `value`) instead of human-readable text. |

```bash
polygon wallet sync --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --network amoy
polygon wallet sync --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --json | jq '.[0]'
```

#### `wallet send`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `String` | ✓ | — | source wallet label. |
| `--password` | `Option<String>` | — | — | L54 chain. Decrypts the keystore before signing. |
| `--to` | `Address` | ✓ | — | recipient EIP-55 / lowercase hex address. |
| `--amount` | `String` | ✓ | — | value to send. Unit follows `--unit`. Accepts fractional POL (`0.01`) or integer wei. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--unit` | `String` | — | `pol` | `pol` (18-decimal) or `wei` (raw). |
| `--batch` | `Option<String>` | — | — | batch send spec (multi-recipient in one tx). |
| `--drain` | `bool` | — | `false` | send the full balance minus gas. |
| `--nonce` | `Option<u64>` | — | — | manual nonce override; bypasses wallet-managed nonce counter. |
| `--gas-limit` | `Option<u64>` | — | — | manual gas limit. Skip for alloy's estimate. |
| `--fee` | `String` | — | `half_hour` | EIP-1559 tier: `fastest` / `half_hour` / `hour` / `economy`. |
| `--max-fee-gwei` | `Option<f64>` | — | — | explicit max fee per gas in gwei. Overrides `--fee` tier. |
| `--priority-fee-gwei` | `Option<f64>` | — | — | explicit priority fee per gas in gwei. Overrides `--fee` tier. |
| `--dry-run` | `bool` | — | `false` | compute tx hash without signing or broadcasting. Loses to `--sign-only` (stderr note). |
| `--sign-only` | `bool` | — | `false` | sign + print raw RLP envelope, no broadcast. Cold-sign pipeline. #514. |
| `--wait` | `bool` | — | `false` | block until receipt mined. Ignored with `--sign-only`. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon wallet send --name ops --to 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 \
    --amount 0.01 --unit pol --wait
polygon wallet send --name ops --to 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 \
    --amount 0.01 --unit pol --sign-only
```

#### `wallet send-speedup`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--tx-hash` | `String` | ✓ | — | B256 hash of the original stuck tx to replace (RBF). |
| `--max-fee-gwei` | `f64` | ✓ | — | replacement max fee per gas in gwei (must be higher than original). |
| `--priority-fee-gwei` | `f64` | ✓ | — | replacement priority fee per gas in gwei. |
| `--name` | `String` | ✓ | — | wallet that signed the original tx (signs the replacement). |
| `--password` | `Option<String>` | — | — | L54 chain. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon wallet send-speedup --tx-hash 0xabc...123 --max-fee-gwei 60 \
    --priority-fee-gwei 40 --name ops --network amoy
```

#### `tx list`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--address` | `Address` | ✓ | — | filter txs where address is sender or recipient. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--since-block` | `Option<u64>` | — | — | earliest block height to scan (inclusive). |
| `--limit` | `Option<u32>` | — | — | cap on returned rows. |
| `--json` | `bool` | — | `false` | JSON array of tx summaries. |

```bash
polygon tx list --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --network amoy \
    --since-block 60000000 --limit 25
polygon tx list --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --json
```

#### `tx get`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--tx-hash` | `String` | ✓ | — | B256 tx hash to look up. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--json` | `bool` | — | `false` | JSON formatter for the full tx record. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon tx get --tx-hash 0xabc...123 --network amoy
polygon tx get --tx-hash 0xabc...123 --json | jq '.blockNumber'
```

#### `erc20 send`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `String` | ✓ | — | source wallet label. |
| `--password` | `Option<String>` | — | — | L54 chain. Decrypts the keystore before signing. |
| `--token` | `String` | XOR | — | token symbol (`USDC` / `USDT` / `DAI`). XOR `--token-address`. |
| `--token-address` | `Option<Address>` | XOR | — | raw token contract address. XOR `--token`. |
| `--to` | `Address` | ✓ | — | recipient EIP-55 / lowercase hex. |
| `--amount` | `String` | ✓ | — | raw base units in token decimals (NOT human units). USDC = 6 decimals. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--gas-limit` | `Option<u64>` | — | — | manual gas limit override. |
| `--max-fee-gwei` | `Option<f64>` | — | — | explicit max fee per gas in gwei. |
| `--priority-fee-gwei` | `Option<f64>` | — | — | explicit priority fee per gas in gwei. |
| `--dry-run` | `bool` | — | `false` | simulate via `eth_call`; no signing, no broadcast. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon erc20 send --name ops --token USDC \
    --to 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --amount 1000000 \
    --network amoy --wait
polygon erc20 send --name ops --token-address 0x8B0180f2101c8260d49339abfEe87927412494B4 \
    --to 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --amount 1000000 --dry-run
```

#### `erc20 balance`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--address` | `Address` | ✓ | — | holder EIP-55 / lowercase hex. |
| `--token` | `String` | XOR | — | symbol. XOR `--token-address`. |
| `--token-address` | `Option<Address>` | XOR | — | raw contract. XOR `--token`. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--all` | `bool` | — | `false` | sum across every registered token. Currently deferred. |
| `--decimals` | `Option<u8>` | — | — | skip the secondary `decimals()` eth_call when set. Saves one RPC round-trip. |
| `--json` | `bool` | — | `false` | JSON object: holder, token, decimals, raw, formatted. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon erc20 balance --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 \
    --token USDC --network amoy
polygon erc20 balance --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 \
    --token-address 0x8B0180f2101c8260d49339abfEe87927412494B4 --decimals 6 --json
```

#### `erc20 list`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--network` | `String` | — | `amoy` | built-in token registry for this network. |
| `--json` | `bool` | — | `false` | JSON array of token records. |

```bash
polygon erc20 list --network amoy
polygon erc20 list --json | jq '.[].symbol'
```

#### `erc20 register`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--address` | `Address` | ✓ | — | token contract address to add to user registry. |
| `--network` | `String` | — | `amoy` | scopes the user registry to one network. |
| `--list` | `bool` | — | `false` | print the user registry instead of mutating it. |
| `--remove` | `Option<String>` | — | — | symbol to drop from the user registry. |

```bash
polygon erc20 register --address 0x8B0180f2101c8260d49339abfEe87927412494B4 --network amoy
polygon erc20 register --list --network amoy
polygon erc20 register --address 0x... --remove USDC --network amoy
```

#### `erc20 approve`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `String` | ✓ | — | source wallet label. |
| `--password` | `Option<String>` | — | — | L54 chain. |
| `--token` | `String` | ✓ | — | token symbol whose allowance is being set. |
| `--spender` | `Address` | ✓ | — | address authorized to pull tokens on behalf of `--name`. |
| `--amount` | `String` | — | `0` | allowance in raw base units (token decimals). |
| `--unlimited` | `bool` | — | `false` | set allowance to `u256::MAX`. Equivalent to `--amount 115792089237316195423570985008687907853269984665640564039457584007913129639935`. |
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--gas-limit` | `Option<u64>` | — | — | manual gas limit override. |
| `--max-fee-gwei` | `Option<f64>` | — | — | explicit max fee per gas in gwei. |
| `--priority-fee-gwei` | `Option<f64>` | — | — | explicit priority fee per gas in gwei. |
| `--dry-run` | `bool` | — | `false` | simulate via `eth_call`; no signing, no broadcast. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon erc20 approve --name ops --token USDC \
    --spender 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --unlimited --network amoy
polygon erc20 approve --name ops --token USDC \
    --spender 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --amount 5000000 --dry-run
```

#### `fee`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--network` | `String` | — | `amoy` | `amoy` or `mainnet`. |
| `--json` | `bool` | — | `false` | JSON `FeeEstimate`: `max_fee_per_gas`, `max_priority_fee_per_gas`, `base_fee`. |
| `--rpc-url` | `Option<String>` | — | — | per-action override. |

```bash
polygon fee --network amoy
polygon fee --json --network amoy | jq '.max_fee_per_gas'
```

#### `config show`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--network` | `String` | — | `amoy` | reported network label (does not call `eth_chainId`). |
| `--json` | `bool` | — | `false` | JSON formatter. RPC URL credentials redacted. |

```bash
polygon config show --network amoy
polygon config show --network amoy --json
```

#### `faucet`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--address` | `Address` | ✓ | — | drip target. Address printed in the faucet URL. |
| `--network` | `String` | — | `amoy` | `amoy` only (mainnet has no canonical faucet). |
| `--faucet-token` | `String` | — | `POL` | token label shown in the instructions. |
| `--auto` | `bool` | — | `false` | auto-claim reserved for T7 (operator-driven per L29). |

```bash
polygon faucet --address 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369 --network amoy
```

#### `sign-message`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--name` | `String` | ✓ | — | source wallet label. |
| `--password` | `Option<String>` | — | — | L54 chain. |
| `--message` | `String` | ✓ | — | arbitrary UTF-8 bytes wrapped by the EIP-191 prefix. |
| `--address` | `Option<Address>` | — | — | signer address hint; used when the wallet has multiple derived addresses. |
| `--verify` | `Option<Address>` | — | — | expected recovered address; dispatch fails if signature recovers to a different address. |
| `--rpc-url` | `Option<String>` | — | — | declared for parity; sign-message never calls RPC. |

```bash
polygon sign-message --name ops --message "hello, polygon"
polygon sign-message --name ops --message "attest 0x42" \
    --verify 0xB954c8fEfAb71e8478ebb288cB11b1c9d4aCF369
```

#### `sign-typed`

| Flag | Type | Req | Default | Notes |
|---|---|---|---|---|
| `--chain-id` | `u64` | ✓ | — | Q7 gate. Only `137` (mainnet) or `80002` (Amoy) pass; others rejected before unlock. |
| `--typed-data` | `Option<String>` | XOR | — | inline EIP-712 JSON. XOR `--typed-data-file`. |
| `--typed-data-file` | `Option<PathBuf>` | XOR | — | path to EIP-712 JSON file. XOR `--typed-data`. |
| `--name` | `String` | ✓ | — | source wallet label. |
| `--password` | `Option<String>` | — | — | L54 chain. |
| `--address` | `Option<Address>` | — | — | signer address hint. |
| `--verify` | `Option<Address>` | — | — | expected recovered address; dispatch fails if signature recovers elsewhere. |
| `--rpc-url` | `Option<String>` | — | — | declared for parity; sign-typed never calls RPC. |

```bash
polygon sign-typed --chain-id 80002 \
    --typed-data '{"types":{"EIP712Domain":[]},"primaryType":"EIP712Domain","message":{}}' \
    --name ops
polygon sign-typed --chain-id 137 --typed-data-file ./permit2.json --name ops
```

#### Global flags

Declared on top-level `Cli`. Available on every subcommand:

| Flag | Type | Default | Env |
|---|---|---|---|
| `--rpc-url` | `Option<String>` | — | `POLYGON_RPC_URL` |
| `--data-dir` | `Option<PathBuf>` | XDG data home | `POLYGON_DATA_DIR` |

## Exit codes

| Code | Meaning                                                |
|------|--------------------------------------------------------|
| `0`  | Success.                                               |
| `1`  | `Error::Rpc` — transport / chain / decoding failures. |
| `2`  | `Error::InvalidInput` — caller-side input rejected (bad address, wrong password, Q7 chain_id rejection). |

## Operator examples

L29 — no CI gate. Full catalogue at [`examples/README.md`](examples/README.md).

```bash
# Create wallet + fund via Amoy faucet + verify drip
cargo run -p polygon --example amoy_faucet_and_verify -- \
    --name test --network amoy --timeout 60
```

## Integration tests

Operator-driven Amoy tests live in [`tests/`](tests/) and require a
funded wallet (see `tests/polygon-data-amoy/README.md`). Network config
loads from `tokens/amoy.json` (single source of truth — shell env
overrides individual fields for paid-tier RPC).

| Test file                       | Phase   | Status |
|---------------------------------|---------|--------|
| `amoy_smoke.rs`                 | P8-T1   | Live   |
| `amoy_erc20_balance.rs`         | P8-T3   | Live   |
| `amoy_erc20_send.rs`            | P8-T3   | Live   |
| `amoy_error_paths.rs`           | P8-T4   | Live   |
| `amoy_p9_pk_import.rs`          | P9-T    | Live   |
| `amoy_sign_only.rs`             | P8-T2   | Live   |
| `polygon_wallet_scenario.rs`   | local Anvil | Live   |
| `local_testnet_smoke.rs`        | local   | Live   |
| `mainnet_smoke.rs`              | mainnet | Manual gate |

## Related

- [`polygon-wallet-core`](../polygon-wallet-core) — Phase 1 thin wrapper over `evm-wallet-core`.
- [`evm-wallet-core`](../evm-wallet-core) — Phase 0 umbrella crate (`Network` enum: Ethereum / Polygon).
- [`eth`](../eth) — sibling EVM CLI; same alloy / `WalletManager` plumbing.
- [`bitcoin-wallet-core`](../bitcoin-wallet-core) — Bitcoin sibling; `SpkiPin` reuse.
- Plan: `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md`.
- Interface design: `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`.
