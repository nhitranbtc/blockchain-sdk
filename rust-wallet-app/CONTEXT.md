# CONTEXT.md — rust-wallet-app

> Shared vocabulary and hard rules for `rust-wallet-app/`. Read before
> any meaningful work. Update as decisions get made.

Generic engineering vocab lives in `CLAUDE.md` at the repo root (workflow
+ git policy). This file is **only** for project-local terms and rules
that, if violated, break the threat model.

## Crate boundaries

| Crate | Owns | Does NOT own |
|---|---|---|
| `bitcoin-wallet-core` (v0.1 library) | signing, chain sync, encryption, descriptor/PSBT construction, address generation, fee estimation | CLI flags, persistence layout, network policy |
| `btc` (v0.1 CLI) | arg parsing, terminal UX, subcommand dispatch | crypto, signing, chain |
| `chain-traits` (v0.2 scaffold) | `ChainWallet` trait shape for future ETH/SOL | Bitcoin-specific logic |

If a change touches signing, key material, or the network → `bitcoin-wallet-core`.
If it touches CLI flags → `btc`. `chain-traits` is v0.2 — don't extend it
from Bitcoin work.

## Domain vocabulary

| Term | Meaning |
|---|---|
| `signer` | `keys::signer` trait — secp256k1 signing primitive |
| `wallet` | `Wallet` instance bound to one mnemonic + one network + one descriptor set |
| `descriptor` | BIP-380+ output descriptor string. Single source of truth for address derivation |
| `mnemonic` | BIP-39 phrase. Wrapped in `Secret<Mnemonic>`. Never persisted in plaintext |
| `xprv` | BIP-32 extended private key. Wrapped in `Secret<XPrv>`, in-memory only |
| `xpub` | BIP-32 extended public key. Plain bytes |
| `PSBT` | BIP-174/370 partially signed transaction. Function-arg or stdin only |
| `UTXO` | Unspent transaction output. Tracked in `bdk_file_store` SQLite |
| `sighash` | BIP-143 sighash choice. See `threat.rs` for enum |
| `sign-message` | BIP-137 message signing. Accepts `&str` only — never raw bytes |
| `atomic_write` | Write-to-temp + rename + parent fsync. Files created mode `0o600` |
| `SPKI pinning` | TLS cert verification by SPKI hash, not CA chain. Per F20 |

### Example

Generic engineering vocab compresses to the project term: "private key"
becomes "xprv wrapped in `Secret<XPrv>`". The first names a concept; the
second names the type wrapper that enforces zeroize-on-drop. Use the
project term, not the generic one, in code, comments, and PR text.

## Network policy

| Network | Default? | When allowed |
|---|---|---|
| `testnet` | **Yes.** All dev, all CI, all default CLI invocations | Always |
| `regtest`, `signet` | Via `--network` flag | Local dev, integration tests |
| `mainnet` | **Never default.** Requires explicit `--network mainnet` | Only when the user typed it consciously |

`mainnet` anywhere in code, tests, or default configs = bug.

## Hard rules

1. **Never default to `mainnet`.** See network policy above.
2. **Never add `bdk_esplora` back.** Dropped per 2026-08-07 drift update — pulls `rustls-webpki 0.101.7` (RUSTSEC-2026-0106). EsploraClient is raw `reqwest` + custom `ServerCertVerifier` per F20.
3. **Never bump `bdk_wallet` / `bdk_chain` / `bdk_file_store` / `bdk_electrum` across major versions without a threat-model update in the same PR.** Pinned to 3.x / 0.23.x / 0.22.x / 0.24.x.
4. **Never add `atty`.** Replaced by `IsTerminal` abstraction per F48.
5. **Never commit secrets, test mnemonics, or addresses mapping to mainnet funds** — even on testnet, never reuse a published BIP-39 test vector.

(Rules for "never persist xprv", "never sign raw bytes", and "never write
files outside `atomic_write`" are type-enforced via `Secret<T>`,
`sign_message(&str)`, and `atomic_write` respectively. The types speak;
prose would be weaker.)

## Threat-model mapping

Runtime expression lives in `bitcoin-wallet-core/src/threat.rs`:

- `MessageClass` — caller declares intent before signing. Mitigates U5.
- `Sighash` enum — explicit BIP-143 choice. Mitigates U1.

Full model: [`docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md`](../../docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md).
Any threat-model change updates `threat.rs` in the same PR.

## Not in v0.1 (do not preempt)

- Hardware wallets (v0.2+)
- `mlock` for secret pages (v0.2+)
- Full PSBT review UI (v0.1.1)
- Multi-sig (v0.2+)
- Lightning, other UTXO chains, FFI bindings (separate specs)
- Watch-only mode — trivial add, deferred

## Update protocol

Vocabulary, crate boundary, hard rule, or threat-model change → edit this
file in the same PR as the code change. Reviewers reject PRs that change
a term without a `CONTEXT.md` update. Missing or inconsistent term → add
it; don't wait for someone else.
