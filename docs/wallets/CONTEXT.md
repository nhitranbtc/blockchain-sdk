# Wallets CONTEXT — Cross-Chain Domain Glossary

**Date:** 2026-09-05
**Status:** Living document (update on any new chain or term addition)
**Scope:** Shared vocabulary across `rust-wallet-app/crates/{bitcoin-wallet-core, evm-wallet-core, eth-wallet-core, polygon-wallet-core, tron-wallet-core, sol-wallet-core}` + their CLI binaries (`btc`, `eth`, `polygon`, `tron`, `sol`).
**Purpose:** Prevent terminology drift between chain-specific research docs. Enforce consistent naming in FFI surface, CLI flags, error enums.

> **EVM shared base (Q1 Option A, Issue #416 Phase 0):** `eth-wallet-core` and `polygon-wallet-core` are thin re-export wrappers (`pub use evm_wallet_core::*;`). All signing/RPC/ABI code lives in `evm-wallet-core` (10 modules). Treat `evm-wallet-core` as the canonical EVM crate; `eth-wallet-core` / `polygon-wallet-core` are import-path shims for historical consumers.

## Version

| Version label | Meaning (BTC) | Meaning (ETH) | Meaning (Polygon) | Meaning (TRON) | Meaning (SOL) |
|---|---|---|---|---|---|
| **V0.1** | `bitcoin-wallet-core` rlib + `btc` CLI | `eth-wallet-core` thin wrapper (delegates to `evm-wallet-core`) + `eth` CLI | `polygon-wallet-core` thin wrapper (delegates to `evm-wallet-core`) + `polygon` CLI | `tron-wallet-core` rlib + cdylib + `tron` CLI | `sol-wallet-core` rlib + cdylib + `sol` CLI |
| **V0.1.5** | mobile-compatible architecture milestone | (n/a) | (n/a) | mobile-compatible architecture + Stake 2.0 ships | mobile-compatible architecture (post-Phase 8 release-mobile profile) |
| **V0.2** | advanced + operator features | advanced features | advanced features | advanced + operator + thread model | advanced + Token-2022 disambig |
| **V0.3** | advanced + upstream changes | (n/a) | (n/a) | advanced + zk-SNARK + multisig | (n/a) |

## Execution Contexts (cross-chain)

| Context | Threading | Runtime | Use case |
|---|---|---|---|
| **CLI binary** | multi-threaded | `tokio::runtime::Builder::new_multi_thread()` | Desktop operator workflow |
| **FFI consumer** | single-threaded per Dart isolate | `tokio::runtime::Builder::new_current_thread()` (pinned, lazy) | Mobile + desktop Dart apps |
| **Embedded WASM** | single-threaded cooperative | `tokio::runtime::Builder::new_current_thread()` (wasm-bindgen-futures) | Browser wallet (V0.4+) |

## Platform Abstraction Layer (PAL) — 4 traits

The 4 PAL traits are defined per chain crate. **Status as of 2026-09-17: NOT yet uniform across chains.**

| Trait | Defined in | Desktop impl | iOS impl | Android impl |
|---|---|---|---|---|
| `WalletStorage` | `tron-wallet-core/src/platform`, `sol-wallet-core/src/platform` | `FileWalletStorage` (atomic write + 0600 perms) | iOS Keychain via Security.framework (not yet impl) | Android EncryptedSharedPreferences (not yet impl) |
| `PlatformInfo` | `tron-wallet-core/src/platform`, `sol-wallet-core/src/platform` | `/etc/os-release` etc. | iOS `UIDevice` via FFI (not yet impl) | Android `Build` via FFI (not yet impl) |
| `NetworkClient` | `tron-wallet-core/src/platform` | `reqwest` + `rustls-native-certs` | `reqwest` + `tls_built_in_root_certs(true)` (not yet impl) | `reqwest` + `tls_built_in_root_certs(true)` (not yet impl) |
| `Clock` | `tron-wallet-core/src/platform` | `std::time::SystemTime` | monotonic clock (not yet impl) | monotonic clock (not yet impl) |

**Gaps (as of 2026-09-17):**

- `bitcoin-wallet-core` does NOT define any of the 4 PAL traits. Storage is via `directories` crate + ad-hoc file I/O.
- `evm-wallet-core` does NOT define any of the 4 PAL traits. `WalletManager` directly persists via `directories` + `tempfile` (test fixture).
- `chain-traits/src/lib.rs` defines only a single `ChainWallet` trait (with `chain_id`, `sync`, `next_receive_address`, `balance`) — not the 4 PAL traits. Commented-out `bitcoin-wallet-core` dep at `chain-traits/Cargo.toml:9` indicates the umbrella was deferred to a later phase.

## Security Primitives — cross-chain invariants

| Primitive | All chains | Reference |
|---|---|---|
| **Argon2id** wallet-file KDF | ✓ | `argon2 = "0.5"` |
| **AES-256-GCM** symmetric cipher | ✓ | `aes-gcm = "0.10"` |
| **Zeroizing\<\_\>** wrap on raw sk | ✓ | `zeroize = "1.x"` |
| **SPKI pin** RPC endpoint verifier | ✓ | 3 parallel impls: `bitcoin-wallet-core::chain::spki`, `evm-wallet-core` (per `tests/spki_pin_localnet.rs`), `tron-wallet-core::chain::SpkiPinnedVerifier`. No shared crate yet. |
| **Mnemonic at rest never plaintext** | ✓ | Argon2id + AES-GCM + Zeroizing |
| **Stable exit codes (0/1/2/3/4/5)** | ✓ | `wallet-errors::ErrorCategory` (per-CLI `classify`; see below) |
| **`--json` mode on list/show/sync/tx-list/config-show** | ✓ | `serde_json` + clap value-conditional |

## Network Naming — Capitalization Rules

| Form | Correct | Examples |
|---|---|---|
| Network name | TitleCase | `Mainnet`, `Shasta`, `Nile`, `Regtest`, `Sepolia`, `Amoy` |
| Network in code | snake_case | `Network::Mainnet`, `Network::Nile`, `Network::Shasta` |
| Env var | UPPER_SNAKE | `RUN_TRON_MAINNET=1`, `RUN_POLYGON_AMOY=1` |
| CLI flag value | TitleCase or kebab-case | `--network mainnet`, `--network nile` |

**Anti-pattern:** `mainnet` (lowercase) in prose, except when quoting CLI/env literals.

## SPKI Pin Format

SPKI pins are SHA-256 of SubjectPublicKeyInfo DER. Format: 64 hex chars.

**Example (TRON Mainnet, verified 2026-09-05):** `0e43f6110bbee5e199c6775cf88a3050a9bd51f3bb4a31aeefb7122f79119f0d`

**Pin rotation:** support `pinned://<pin1>,<pin2>@host[:port]` comma-separated list (out of scope for v0.1/V0.2).

## Common CLI Flags (cross-chain)

Measured by building all five CLIs and crawling `--help` across every
subcommand — not by reading the old table, and not by reading the clap
attributes.

| Flag | btc | eth | polygon | tron | sol |
|---|:-:|:-:|:-:|:-:|:-:|
| `--address` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--data-dir` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--dry-run` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--id` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--json` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--mnemonic` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--to` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `--network` | ✓ | ✓ | ✓ | ✓ | — |
| `--cluster` | — | — | — | — | ✓ |
| `--amount` | — | ✓ | ✓ | ✓ | ✓ |
| `--amount-sat` | ✓ | — | — | — | — |
| `--mnemonic-file` | — | — | ✓ | ✓ | ✓ |
| `--password` | ✓ | ✓ | ✓ | ✓ | — |
| `--password-file` | ✓ | — | — | — | — |
| `--private-key-file` | — | — | ✓ | ✓ | ✓ |
| `--name` | — | ✓ | ✓ | ✓ | ✓ |
| `--unit` | — | ✓ | ✓ | ✓ | ✓ |
| `--token` | — | ✓ | ✓ | ✓ | ✓ |
| `--sign-only` | — | — | ✓ | ✓ | ✓ |
| `--wait` | — | ✓ | ✓ | ✓ | ✓ |
| `--limit` | ✓ | ✓ | ✓ | — | — |
| `--spender` | — | ✓ | ✓ | ✓ | — |
| `--fee-limit` | — | — | — | ✓ | — |
| `--gas-limit` | — | ✓ | ✓ | — | — |
| `--max-fee-gwei` | — | ✓ | ✓ | — | — |
| `--fee-rate` | ✓ | — | — | — | — |
| `--priority-fee` | — | — | — | — | ✓ |
| endpoint override | `--esplora-url` | `--rpc-url` | `--rpc-url` | `--rpc-url`, `--rpc` | `--rpc`, `--url` |
| SPKI pin override | `--pin-spki`, `--esplora-spki-pin` | — | — | — | `--spki-pin` |

Notes:

- **Seven flags are common to all five CLIs:** `--address`, `--data-dir`,
  `--dry-run`, `--id`, `--json`, `--mnemonic`, `--to`. (The grilling of the
  recommendations document recorded **six** and omitted `--mnemonic`; measuring
  against the built binaries gives seven. Solana has both `--mnemonic` and
  `--mnemonic-file`.)
- **The endpoint override has four different spellings** across the five CLIs
  and means something slightly different on each — this is genuinely per-chain
  and is *not* unified by this phase. On `sol` the two are genuinely
  different things: `--rpc` is the **global runtime override** (present on
  every subcommand's `--help`), while `--url` is the argument of
  **`sol config set-rpc`**, which writes the endpoint into the config instead
  of overriding it for one invocation.
- **The SPKI pin has three names over two chains.** `eth`, `polygon` and `tron`
  take their pin from configuration, not from a flag.
- `--network` is not universal: Solana selects a `--cluster`.
- `--mnemonic-file` exists on three CLIs, not all five; Bitcoin uses
  `--password-file` / `--password-stdin`, and `eth` has no mnemonic-file flag.
- `--wait` exists on four CLIs, not all five.
- `--sign-only` exists on three CLIs (polygon, tron, sol), not two — the old
  table said "partial", which was both imprecise and wrong.

## Common Error Categories (cross-chain)

| Category | Exit code | Examples |
|---|---|---|
| Success | 0 | OK |
| User error | 1 | Invalid args, missing file, unknown wallet, invalid mnemonic or address, ABI decode failure |
| Network error | 2 | RPC unreachable, gas estimate failed, receipt timeout |
| Insufficient funds | 3 | Balance below amount + fee |
| Signing error | 4 | Wrong password, key derivation failure, corrupt or tampered wallet file, encryption or KDF failure, transaction construction failure, internal machinery failure, feature not yet implemented |
| Broadcast error | 5 | Tx REVERTED, node rejected |

**Implementation (2026-10-30, unified).** The category→exit-code table lives in one
place: the `wallet-errors` crate (`rust-wallet-app/crates/wallet-errors/`), and
this table is asserted against it by
`wallet-errors/tests/context_doc_parity.rs` — the table is the specification,
not a description of it.

Each CLI keeps its own variant→category mapping (chain-specific error granularity
is a security feature and is deliberately not unified):

| CLI | Classifier |
|---|---|
| `btc` | `crates/btc/src/error.rs::classify` |
| `eth` | `crates/eth/src/error.rs::classify` |
| `polygon` | `crates/polygon/src/error.rs::classify` |
| `tron` | `crates/tron/src/handlers/mod.rs::classify` |
| `sol` | `crates/sol/src/handlers/error.rs::classify` |

Four intentional divergences from the categories above:

- **Bitcoin's "wallet not found" exits 4, not 1.** `bitcoin-wallet-core`'s
  `WalletStore` variant deliberately collapses not-found, wrong-password,
  wrong-network and corrupt-blob into one observable outcome to close the
  file-existence oracle (`crates/bitcoin-wallet-core/src/error.rs:110-120`).
  Because not-found and wrong-password share the variant, they must share the
  code.
- **An empty passphrase exits 1 where the CLI refuses it up front, and 4 on
  the paths that open an existing wallet.** The library cannot tell an empty
  passphrase from a wrong one — both are a failed KDF, both surface as
  `WalletError::Crypto` — so only the CLI can make the distinction, and the
  CLIs draw that line at different points:
  - **`eth` and `polygon`** refuse it on **create and import**
    (`eth/src/handlers.rs:127`, `polygon/src/handlers/wallet.rs:373` →
    `InvalidPassword` → **1**). On the paths that **open an existing wallet**
    the passphrase is a credential for a wallet that already exists, so an
    empty one is indistinguishable from a wrong one and exits **4**, exactly
    as a wrong one does. Which paths those are differs per CLI: in `eth`
    they are native send, ERC-20 send and speed-up; in `polygon` they are
    sign-message, sign-typed, ERC-20 send and ERC-20 approve. `polygon`
    additionally refuses it on its **native-send** and **speed-up** paths
    (**1**), where `eth` does not.
  - **`eth`'s sign-message, sign-typed and ERC-20 approve are deferred stubs**
    that return `Error::Rpc` (**2**) without touching the wallet, so on those
    commands the question does not arise.
  - **`tron`** refuses it **on every path** — `tron/src/handlers/mod.rs:302`
    rejects an empty passphrase centrally, from argv, the env var and the TTY
    prompt — so it exits **1** everywhere, sign included. That is TRON being
    stricter, not a different taxonomy.
  - **`btc` accepts one where the other three chains refuse it.** Its
    `password_or_prompt` (`btc/src/handlers.rs:84`) treats an empty
    `--password` argv as "not supplied" and falls through to the TTY prompt,
    and argon2 0.5.3 derives a key from an empty password without complaint
    (`bitcoin-wallet-core/src/crypto/argon2.rs:104` calls
    `Argon2::hash_password_into`, whose `verify_inputs` check rejects only
    passwords *longer than* `u32::MAX`) — so `btc wallet create` **succeeds
    with exit 0** and stores a wallet encrypted under an empty passphrase. On
    the paths that open a wallet **created with a non-empty passphrase** the
    empty passphrase derives a *different* key, so the **AES-GCM tag check**
    fails and the error lands on `WalletStore` → **4**, like a wrong
    passphrase; a wallet that was itself created under an empty passphrase
    re-derives the *same* key and opens. **`sol` accepts one the same way**
    — its only password source is the `SOL_WALLET_PASSWORD` env var, and
    nothing downstream checks emptiness. This is pre-existing, unchanged by
    the migration, and recorded as a deferred finding.

  The EVM CLIs' differences on the wallet-open paths are a **recorded deferred
  finding**: closing them means changing `eth` and `polygon` together, against
  the already-merged ETH migration, with both `CHANGELOG.md` bullets
  corrected.
- **A chain reaches only the categories its engine can express. Solana reaches
  all six; `eth`, `polygon`, `btc` and `tron` each reach four** — 0, 1, 2 and 4.
  None of the EVM, Bitcoin or TRON engines constructs an insufficient-funds or
  a broadcast-rejection error outside its own variant-coverage tests, so **3
  and 5 are unreachable in all four**; a node-side transaction rejection is
  reported as `Network` (2), the same code as an unreachable node. This is a
  property of the engines, not of the taxonomy — the table above is the
  specification for every chain that *can* reach a category.
- **The `polygon` CLI translates the wallet library's errors the same way `eth`
  does.** Its `map_wallet_err` maps the library's errors into `Rpc`,
  `WalletCorrupt`, `DecryptionFailed`, `InvalidMnemonic`, `InvalidPrivateKey`,
  `WalletNotFound`, `WalletNotFoundByName` and `WalletExists`, and the CLI
  itself raises `InvalidInput` and `InvalidPassword` — so it exits **1**
  (operator input), **2** (node unreachable, RPC failure, local I/O) or **4**
  (a wrong passphrase or a corrupt wallet file), exactly as `eth` does. A
  wrong passphrase exits 4 rather than 1 so that it stays indistinguishable
  from a corrupt wallet file, exactly as in the other four chains.

**This changed the exit codes of all five CLIs.** Automation that keys on exit
codes must be updated — see the `**BREAKING**` entries in `CHANGELOG.md`.

## Address Encoding — per chain

| Chain | Format | Example |
|---|---|---|
| Bitcoin | bech32 (native segwit), base58 (legacy) | `bc1q...`, `1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa` |
| Ethereum | EIP-55 checksummed hex (0x + 40 hex) | `0x742d35Cc6634C0532925a3b844Bc9e7595f7E2c8` |
| Polygon | EIP-55 checksummed hex (same as ETH) | `0x742d35Cc6634C0532925a3b844Bc9e7595f7E2c8` |
| TRON | T-base58check (T + 33 base58) | `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` |

## Chain ID — per chain

| Chain | Chain ID |
|---|---|
| Bitcoin | (n/a, no chain ID) |
| Ethereum Mainnet | `0x1` (1) |
| Ethereum Sepolia | `0xaa36a7` (11155111) |
| Polygon Mainnet | `0x89` (137) |
| Polygon Amoy | `0x13882` (80002) |
| TRON Mainnet | `0x2b6653dc` (728126428) |
| TRON Nile (testnet) | `0xcd8690dc` (3448148188) |
| TRON Shasta | `0x94a9059e` (2494104990) |

## Cross-chain Anti-Patterns

These mistakes recur across chain research docs. Audit before each new chain addition:

1. **Don't assume chain-id is global.** TRON uses `0x41` prefix across mainnet/Shasta/Nile; chain-id disambiguates. Ethereum mainnet vs Sepolia share EIP-55 shape; chain-id disambiguates.
2. **Don't trust the user's "mainnet" claim.** Network selector in code is the source of truth. SPKI pin per network.
3. **Don't bypass SPKI pin on localhost.** Localhost uses Scenario B (no pin); cross-network checks apply.
4. **Don't reuse `Arc<Zeroizing<Vec<u8>>>` on secrets.** FFI boundary leaks the reference lifetime.
5. **Don't assume third-party SDK bus-factor.** Always check `git log --format='%an' --since='12 months ago'` before adoption.
6. **Don't ship V0.1 without mainnet self-send smoke.** Local + Nile = emulation, not real value.
7. **Don't mark V0.1 features "ready" by inspection only.** Tie each to spike PASS evidence.

## Third-Party SDK Vocabulary — anychain (TRON)

The TRON stack is built on the `0xcregis/anychain` crate family. **Vendoring was reconsidered and adopted 2026-09-06** (one day after initial rejection) under `rust-wallet-app/crates/anychain-vendored/`, pinned to commit `cf3aa2d59afb2c50dc961919fca011c401238ed6` (subject `build: rustup toolchain update to 1.98.0`, date 2026-09-04). Workspace `Cargo.toml:30-36` uses path deps exclusively so consumer builds never see `crates.io/index` for these names.

**Pin deviation from plan:** Plan reference cites upstream tag `v0.2.14` for `anychain-tron`. Tag does not exist on upstream — `git ls-remote` returned only `0.0.2` and `0.1.5` as of 2026-09-06. Pinned to commit SHA instead. Plan amendment deferred (post-Phase 0).

**Local patches** layered per plan Task 0.7:

1. **Q13 varint fix** — `src/protocol/Tron.rs::Raw::write_to_with_cached_sizes` patched to emit canonical varint for `fee_limit` (strip spurious `0x01` prefix). Issue #540.
2. **Q2 dual-SHA256 txid fix** — `src/transaction.rs::TronTransaction::to_transaction_id` replaces single `sha256(raw_bytes)` with `sha256(sha256(raw_bytes))`. Issue #399 historical bug.

Bus-factor risk (single author, 3 commits trailing 12 months) accepted and mitigated by exact pin + regression tests that assert known-buggy behaviour, so a silent upstream "fix" fails CI instead of changing signatures unnoticed. Quarterly sync cadence per plan Q3.

| Term | Meaning |
|---|---|
| **anychain** | Umbrella crate family from `0xcregis/anychain`, MIT OR Apache-2.0. Not a single crate — `-core`, `-tron`, and `-kms` are published separately and version independently. |
| **anychain-core** `=0.1.8` | Shared traits (`Address`, `PublicKey`, `Transaction`, `Format`, `Network`) + crypto utilities (`keccak256`, `sha256`, `func_selector`) and a `hex` re-export. |
| **anychain-tron** `=0.2.14` | Wire format — T-base58check address, protobuf `Transaction` envelope, 17 contract builders (TRX transfer, TRC-20 transfer/approve, Stake 2.0 freeze/unfreeze/delegate/cancel/withdraw, witness vote, withdraw vote, account create, generic trigger), `abi::encode_call`. Bus-factor 1 (single author, 3 commits trailing 12 months) — see anti-pattern 5 below. |
| **anychain-kms** `=0.1.23` | BIP-39 mnemonic (8 languages), BIP-32 HD derivation (SLIP-44 coin 195), secp256k1 signing, xprv serialization with `Zeroizing<String>`. |

**Wrapping rule:** `tron-wallet-core` never re-exports anychain types across its
public surface. Each is wrapped so an upstream break is a one-file change, and so
raw `sk` bytes get a `Zeroizing<[u8; 32]>` wrap before `secp256k1_sign` (closes
the anychain gap).

## References

- `docs/wallets/2026-08-23-ethereum-rust-sdks-deep-dive.md` — ETH precedent
- `docs/wallets/2026-08-27-tron-anychain-sdks-deep-dive.md` — TRON primary
- `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs` — SPKI pin verifier (BTC impl; TRON + EVM have parallel impls, see security-primitive table above)
- `rust-wallet-app/crates/wallet-errors/src/lib.rs` — the shared exit-code table (`ErrorCategory`)
- `rust-wallet-app/crates/sol/src/handlers/error.rs:68` — exit code classifier (`fn classify`)
- `rust-wallet-app/crates/tron/src/handlers/mod.rs:87` — exit code classifier (`fn classify`)
- `rust-wallet-app/Cargo.toml:240-247` — `release-mobile` profile (mobile cross-compilation, undocumented elsewhere)
- `rust-wallet-app/crates/anychain-vendored/*/SOURCE.md` — vendored commit SHA + patch log per crate
