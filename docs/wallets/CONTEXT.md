# Wallets CONTEXT — Cross-Chain Domain Glossary

**Date:** 2026-09-05
**Status:** Living document (update on any new chain or term addition)
**Scope:** Shared vocabulary across `rust-wallet-app/crates/{bitcoin,eth,polygon,tron}-wallet-core` + their CLI binaries
**Purpose:** Prevent terminology drift between chain-specific research docs. Enforce consistent naming in FFI surface, CLI flags, error enums.

## Version

| Version label | Meaning (BTC) | Meaning (ETH) | Meaning (Polygon) | Meaning (TRON) |
|---|---|---|---|---|
| **V0.1** | `bitcoin-wallet-core` rlib + `btc` CLI | `eth-wallet-core` + `eth` CLI | `polygon-wallet-core` bin + `polygon` CLI | `tron-wallet-core` rlib + cdylib + `tron` CLI |
| **V0.1.5** | mobile-compatible architecture milestone | (n/a) | (n/a) | mobile-compatible architecture + Stake 2.0 ships |
| **V0.2** | advanced + operator features | advanced features | advanced features | advanced + operator + thread model |
| **V0.3** | advanced + upstream changes | (n/a) | (n/a) | advanced + zk-SNARK + multisig |

## Execution Contexts (cross-chain)

| Context | Threading | Runtime | Use case |
|---|---|---|---|
| **CLI binary** | multi-threaded | `tokio::runtime::Builder::new_multi_thread()` | Desktop operator workflow |
| **FFI consumer** | single-threaded per Dart isolate | `tokio::runtime::Builder::new_current_thread()` (pinned, lazy) | Mobile + desktop Dart apps |
| **Embedded WASM** | single-threaded cooperative | `tokio::runtime::Builder::new_current_thread()` (wasm-bindgen-futures) | Browser wallet (V0.4+) |

## Platform Abstraction Layer (PAL) — 4 traits

All chain-specific wallet-core crates implement these 4 traits for cross-platform compilation:

| Trait | Purpose | Desktop impl | iOS impl | Android impl |
|---|---|---|---|---|
| `WalletStorage` | Wallet file persistence | `FileWalletStorage` (atomic write + 0600 perms) | iOS Keychain via Security.framework | Android EncryptedSharedPreferences |
| `PlatformInfo` | Device/OS metadata | `/etc/os-release` etc. | iOS `UIDevice` via FFI | Android `Build` via FFI |
| `NetworkClient` | HTTP transport | `reqwest` + `rustls-native-certs` | `reqwest` + `tls_built_in_root_certs(true)` | `reqwest` + `tls_built_in_root_certs(true)` |
| `Clock` | Time source | `std::time::SystemTime` | monotonic clock | monotonic clock |

## Security Primitives — cross-chain invariants

| Primitive | All chains | Reference |
|---|---|---|
| **Argon2id** wallet-file KDF | ✓ | `argon2 = "0.5"` |
| **AES-256-GCM** symmetric cipher | ✓ | `aes-gcm = "0.10"` |
| **Zeroizing\<\_\>** wrap on raw sk | ✓ | `zeroize = "1.x"` |
| **SPKI pin** RPC endpoint verifier | ✓ | `bitcoin-wallet-core::chain::spki::SpkiPinnedVerifier` (reusable) |
| **Mnemonic at rest never plaintext** | ✓ | Argon2id + AES-GCM + Zeroizing |
| **Stable exit codes (0/1/2/3/4/5)** | ✓ | `handlers::error::classify` |
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

| Flag | Purpose | All chains |
|---|---|---|
| `--network <name>` | Network selection | ✓ |
| `--rpc <url>` | Override default RPC URL | ✓ |
| `--spki-pin <hex>` | Override default SPKI pin | ✓ |
| `--json` | JSON output mode | ✓ |
| `--dry-run` | Simulate without side effects | ✓ |
| `--sign-only` | Sign but don't broadcast | partial |
| `--wait` | Wait for confirmation | ✓ |
| `--fee-limit <sun|wei|matic>` | Max fee | per-chain unit |
| `--mnemonic-file <path>` | Read mnemonic from file (not argv) | ✓ |

## Common Error Categories (cross-chain)

| Category | Exit code | Examples |
|---|---|---|
| Success | 0 | OK |
| User error | 1 | Invalid args, missing file |
| Network error | 2 | RPC unreachable, timeout |
| Insufficient funds | 3 | Balance below amount + fee |
| Signing error | 4 | Wrong password, invalid key |
| Broadcast error | 5 | Tx REVERTED, node rejected |

(Pattern from `btc/src/main.rs:151-169`.)

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

The TRON stack is built on the `0xcregis/anychain` crate family, pulled **direct
from crates.io at exact versions** (`=X.Y.Z`, never `^`). Vendoring into
`rust-wallet-app/crates/anychain-vendored/` was considered and **rejected
2026-09-05** — operational overhead exceeded the benefit at v0.1 scope. The
bus-factor risk is accepted and mitigated by the exact pin plus regression tests
that assert known-buggy behaviour (dual-SHA256 txid, `Zeroizing` gap), so a
silent upstream "fix" fails CI instead of changing signatures unnoticed.

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
- `docs/wallets/2026-09-05-adr-0001-tron-sdk-anychain-vs-raw-primitives.md` — TRON ADR
- `rust-wallet-app/crates/bitcoin-wallet-core/src/chain/spki.rs` — SPKI pin verifier (cross-chain reference impl)
- `btc/src/main.rs:151-169` — exit code pattern