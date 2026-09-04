# `polygon_amoy/` — Amoy wallet store

This directory is the **polygon CLI's local Amoy wallet store**. Created
by `polygon wallet create` / `polygon wallet import` invocations;
consumed by `--data-dir` / `POLYGON_DATA_DIR` on every subsequent CLI
call.

```text
polygon_amoy/
├── README.md                          ← this file
├── cf3e7139-…317c.enc                 ← amoy-smoke-1 (encrypted mnemonic, 0600)
├── cf3e7139-…317c.meta.json           ← amoy-smoke-1 (public metadata, 0600)
├── cfc9b996-…54b6.enc                 ← amoy-smoke-2 (encrypted mnemonic, 0600)
└── cfc9b996-…54b6.meta.json           ← amoy-smoke-2 (public metadata, 0600)
```

## Wallets present

| name | address (EIP-55) | derivation path | created |
|---|---|---|---|
| `amoy-smoke-1` | `0x971200F83562896Ff7049Cb8f6686c4eB5Cb1717` | `m/44'/60'/0'/0/0` | 2026-09-03 (session) |
| `amoy-smoke-2` | `0x2055ba398775b9aa890bd02222a948f4978c3661` | `m/44'/60'/0'/0/0` | 2026-09-03 (session) |

Both wallets are **Amoy testnet (chain_id 80002)**. They are not mainnet
accounts; do not use for any real value.

`.enc` files are **Argon2id + ChaCha20-Poly1305** encrypted wallet
blobs — useless without the wallet password. The wallet password is
held by the operator (set per invocation via `POLYGON_PASSWORD` env or
`--password` argv; the CLI removes the env var immediately after read
per L54).

## Password (testnet session — Amoy only)

```bash
POLYGON_PASSWORD=0987654321
```

**Strength:** 10 numeric digits, ~33 bits entropy. Brute-forceable in
days to weeks against the Argon2id KDF with consumer hardware. **Amoy
testnet only — do not reuse for mainnet or any real-value account.**
Rotate to a stronger password (≥12 random alphanumerics) before any
non-testnet use.

## Usage

```bash
# List wallets
polygon wallet list --data-dir /home/nhitran/Projects/blockchain-sdk/.local/polygon-data-amoy

# Check native POL balance
polygon wallet balance --name amoy-smoke-1 --network amoy \
  --data-dir /home/nhitran/Projects/blockchain-sdk/.local/polygon-data-amoy

# Sign a message
POLYGON_PASSWORD='<password>' polygon sign-message \
  --name amoy-smoke-1 --message "hello" --network amoy \
  --data-dir /home/nhitran/Projects/blockchain-sdk/.local/polygon-data-amoy

# Send USDC
POLYGON_PASSWORD='<password>' polygon erc20 send \
  --name amoy-smoke-1 --to 0x<recipient> --amount 1 --token USDC --network amoy \
  --data-dir /home/nhitran/Projects/blockchain-sdk/.local/polygon-data-amoy
```

## Security

- **The `.enc` files ARE the wallet material.** Treat the directory as
  you would a hardware-wallet seed phrase backup: encrypted-at-rest but
  recoverable by anyone with the password.
- The **password is the security boundary**, not the filesystem. If
  the password leaks, the `.enc` files are equivalent to plaintext
  mnemonics.
- **Do not commit this directory to git.** The repo's `.gitignore`
  blocks `docs/superpowers/engineering/` but **does not** ignore this
  path. Add a project-root `.gitignore` rule for `.local/` if you want
  belt-and-suspenders coverage.
- **Avoid `--password` on argv** (shell history + `/proc/<pid>/cmdline`).
  Use `POLYGON_PASSWORD` env; the CLI removes it from process env
  immediately after read (L54 defense-in-depth).
- The polygon CLI has **no `wallet export` subcommand** (intentional).
  Mnemonics cannot be recovered from the encrypted blobs without the
  password + a custom decrypt path. If you need the 12 words for a
  given wallet, delete + recreate it (destructive) or write a
  one-off Rust decryptor against `polygon-wallet-core`.

## Origin

Migrated from `/tmp/polygon-data-amoy/` on 2026-09-03 (session) to
keep wallet state with the project checkout rather than ephemeral
`/tmp` storage. The `/tmp` source is still present — operator cleanup
needed (`rm -rf /tmp/polygon-data-amoy`).
