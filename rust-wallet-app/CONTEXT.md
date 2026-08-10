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
| `mnemonic` | BIP-39 phrase. `keys::mnemonic::Mnemonic` newtype wrapping `Secret<bip39::Mnemonic>`. Three zeroize layers: inner `bip39::Mnemonic`, `to_seed() → Secret<Vec<u8>>`, `to_phrase() → Secret<String>`. Never persisted in plaintext. |
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

> **Audit (2026-08-10):** All 7 rules remain active. Rule 4 (atty) softened per F48 — atty was unmaintained and replaced by `IsTerminal` abstraction. Other rules remain strict because they defend real security invariants for the client product.

1. **Avoid defaulting to `mainnet`** unless the caller explicitly selects it. See network policy above. (Defends F37 + L28 honest-scope.)
2. **Avoid adding `bdk_esplora` back** unless re-scoped with documented mitigation for `rustls-webpki 0.101.7` (RUSTSEC-2026-0106). EsploraClient is raw `reqwest` + custom `ServerCertVerifier` per F20.
3. **Avoid bumping `bdk_wallet` / `bdk_chain` / `bdk_file_store` / `bdk_electrum` across major versions** unless a threat-model update lands in the same PR. Pinned to 3.x / 0.23.x / 0.22.x / 0.24.x.
4. **Avoid `atty`** unless `IsTerminal` abstraction is unavailable (F48 deferred to v0.1.1). Atty is unmaintained per F48.
5. **Avoid committing secrets, test mnemonics, or addresses mapping to mainnet funds** — even on testnet, never reuse a published BIP-39 test vector.
6. **Avoid dropping the `bip39` `zeroize` feature.** Required for `Secret<bip39::Mnemonic>` to compile. `bdk_wallet`'s transitive `bip39` dep is declared without features, so `bitcoin-wallet-core` declares `bip39` directly in `[workspace.dependencies]` to force feature unification (Task 3, 2026-08-08). If a future maintainer tries to remove the direct dep, `Secret<Mnemonic>` will fail to compile with "the trait bound `bip39::Mnemonic: Zeroize` is not satisfied."
7. **Avoid using BIP-39 wordlist words as `Debug` field names** on types holding mnemonic-derived secrets. Use `std::fmt::DebugStruct::finish_non_exhaustive()` (renders as `Type { .. }`) or non-wordlist field names. The BIP-39 English wordlist has 2048 words including common English terms (`inner`, `secret`, `phrase`, `seed`, `key`); `assert!(!dbg.contains(phrase_word))` flakes ~0.5% per run when the generated mnemonic happens to include the field name (caught by CI on first run for Task 3, 2026-08-08).

(Rules for "never persist xprv", "never sign raw bytes", and "never write
files outside `atomic_write`" are type-enforced via `Secret<T>`,
`sign_message(&str)`, and `atomic_write` respectively. The types speak;
prose would be weaker.)

## Threat-model mapping

Runtime expression lives in `bitcoin-wallet-core/src/threat.rs`:

- `MessageClass` — caller declares intent before signing. Mitigates U5.
- `Sighash` enum — explicit BIP-143 choice. Mitigates U1.

- `Secret<T>` wrapper (Task 3) — zeroize-on-drop on heap-resident Mnemonic /
  seed / phrase bytes. Mitigates U3 (memory leak) + A1 (local read) + A8
  (co-resident `/proc/$pid/mem`).
- `Mnemonic` type (Task 3) — `Secret<bip39::Mnemonic>` enforces F47
  (in-memory zeroize). Methods: `generate`, `from_phrase`, `to_seed`,
  `to_phrase`, `word_count`.

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
