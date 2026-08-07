# Threat Model: bitcoin-wallet-core v0.1

> **Status:** Proposed for v0.1.
> **Plan ref:** [`2026-08-05-rust-bitcoin-wallet.md` §Task 0a](../plans/2026-08-05-rust-bitcoin-wallet.md)
> **Spec ref:** [`2026-08-05-rust-bitcoin-wallet-design.md` §Threat Model](./2026-08-05-rust-bitcoin-wallet-design.md)
> **Audit basis:** 50 doc-review findings (see [`2026-08-05-rust-bitcoin-wallet.md`](../plans/2026-08-05-rust-bitcoin-wallet.md)); key references F5, F6, F7, F9, F12, F19, F20, F21, F25, F47, F48, F53.

This document enumerates what `bitcoin-wallet-core` v0.1 must protect, who might
attack it, where the trust boundaries lie, the realistic abuse cases those
boundaries permit, and the mitigations tied back to specific plan findings and
implementation tasks.

## Assets

| Asset | Sensitivity | Storage | Notes |
| ----- | ----------- | ------- | ----- |
| Mnemonic (BIP-39 phrase) | Critical | Encrypted at rest per F6 | AES-256-GCM ciphertext + Argon2id-derived key (m=256 MiB, t=10, p=4 per F5) |
| xprv (extended private key) | Critical | In-memory only | Wrapped in `Secret<XPrv>` per F47; never persisted, never logged |
| PSBT (Partially Signed Bitcoin Transaction) | High | Transit only | Never persisted; flows through function args or stdin |
| UTXO set | Medium | `bdk_file_store` SQLite DB | Per-wallet directory; cleared on wallet deletion |
| Signed messages (BIP-137) | Low–Medium | Function return value | `crypto::bip137::sign_message` returns base64 sig per F9; raw key never exposed per F7 |
| Wallet metadata | Low | Same SQLite DB | address_type, network, derivation path, wallet_id |

## Adversaries

- **A1: Local user with read access to the data directory.** May inspect
  wallet files; can also copy the encrypted mnemonic offline for later
  cracking.
- **A2: Local user with write access to the data directory.** May tamper
  with descriptors, replace the SQLite DB, or append transactions.
- **A3: Network attacker (MITM, BGP hijack, rogue CA).** Sits between the
  wallet and any Esplora/Electrum endpoint; can replay, drop, or forge
  responses.
- **A4: Malicious Esplora/Electrum endpoint operator.** Returns fake UTXO
  sets, hides transactions, or lies about chain tip.
- **A5: Malicious PSBT provider.** A coinjoin coordinator, hardware-wallet
  workflow, or counterparty that hands the wallet a PSBT redirecting
  funds to attacker-controlled addresses.
- **A6: Supply-chain compromise.** A malicious dep crate (transitive or
  direct) or a compromised CI pipeline inserts code into the wallet or
  its build artifacts.
- **A7: Phishing vector.** Tricks the user into signing arbitrary bytes
  (or a misleading message) via the CLI's `sign-message` subcommand.
- **A8: Local process with `/proc/$pid/mem` read access.** Reads the
  wallet process's RAM to lift the in-memory mnemonic or xprv while the
  wallet is running.

## Trust boundaries

- **B1: Process ↔ filesystem.** Data directory holds `mnemonic.enc`,
  descriptors, and the `bdk_file_store` SQLite DB. All writes go through
  `util::atomic_write` (temp + rename + parent fsync per F19). All files
  are created mode `0o600`; world-writable parent directories are refused.
- **B2: Process ↔ network.** Esplora/Electrum over TLS with **SPKI pubkey
  pinning** per F20. The pinned SPKI hash is loaded from
  `WalletConfig::EsploraPinnedPubkey` and verified by a custom
  `ServerCertVerifier` before any HTTP request.
- **B3: Process ↔ PSBT source.** PSBTs enter via function arguments or
  CLI stdin. **Full PSBT-review UX (per F25) is deferred to v0.1.1.**
  v0.1 surfaces the destination addresses and amount on stdout and
  requires an explicit `--yes` flag to proceed.
- **B4: Library ↔ hardware.** v0.1 ships **no hardware-wallet
  integration**. All signing is software-only via `keys::signer`.

## Abuse cases

- **U1: Malicious PSBT redirects 100% of balance.** A PSBT that spends
  every UTXO to an attacker address is accepted and signed. **Mitigation:
  F25 PSBT review deferred to v0.1.1** (v0.1 surfaces outputs and
  requires `--yes`).
- **U2: Fake Esplora lies about UTXOs.** Endpoint reports unspent UTXOs
  that don't actually exist on chain, tricking the wallet into signing a
  transaction that will never confirm. **Mitigation: F20 pubkey pinning
  (Task 7).**
- **U3: Process memory leak via `/proc/$pid/mem`.** A co-resident
  process reads the wallet's address space to lift the mnemonic or xprv.
  **Mitigation: `mlock` of secret pages deferred to v0.2.** v0.1 limits
  exposure via `Secret<T>` zeroization on drop (F47, F53).
- **U4: Supply-chain compromise of a small utility crate (e.g. `atty`).**
  A widely depended-on but lightly-audited crate ships malicious code
  that runs at build time or runtime. **Mitigation: F48 `IsTerminal`
  abstraction deferred to v0.1.1** (avoids the `atty` dep).
- **U5: User signs arbitrary hash via CLI.** A phishing page tells the
  user "paste this into `btc sign-message`" and the user signs a hash
  that is, in fact, a Bitcoin transaction sighash. **Mitigation: F7
  narrow `sign_message(msg: &str)` API (Task 6) — only accepts
  human-readable strings, never raw bytes.**
- **U6: World-readable mnemonic file.** Default file mode lets other
  local users read the encrypted mnemonic. **Mitigation: F19
  `atomic_write` creates files mode `0o600` (Task 1.5).**
- **U7: Crashed mid-write leaves partial mnemonic.** Power loss or
  crash during file write leaves a truncated ciphertext that fails to
  decrypt. **Mitigation: F19 `atomic_write` write-to-temp + rename +
  parent fsync (Task 1.5).**

## Mitigations mapping

| Abuse case | Mitigation | Plan finding | Implementation task | Status |
| ---------- | ---------- | ------------ | ------------------- | ------ |
| U1 (malicious PSBT) | F25 PSBT review | F25 | n/a | **Deferred to v0.1.1** |
| U2 (fake Esplora) | SPKI pubkey pinning | F20 | Task 7 (WalletConfig + EsploraClient) | In plan |
| U3 (memory leak) | `mlock` secret pages | n/a | n/a | **Deferred to v0.2** |
| U4 (supply-chain `atty`) | `IsTerminal` abstraction | F48 | n/a | **Deferred to v0.1.1** |
| U5 (arbitrary-hash phishing) | Narrow `sign_message` API | F7 | Task 6 (crypto::bip137) | In plan |
| U6, U7 (insecure / partial mnemonic file) | `atomic_write` + mode `0o600` | F19 | Task 1.5 (hygiene) | In plan |

### Cross-link: threat.rs (Task 1 / Task 9 §21)

The `Sighash` and `MessageClass` enums in
`bitcoin-wallet-core/src/threat.rs` (per plan finding F21) are the
runtime type-level expression of this threat model:

- `MessageClass::Generic` — rejects ambiguous bytes; forces the caller to
  declare intent. Directly mitigates **U5**.
- `Sighash::{All, AllAnyoneCanPay, None, NoneAnyoneCanPay, Single,
  SingleAnyoneCanPay}` — explicit BIP-143 sighash choice; prevents a
  buggy caller from accidentally signing a different sighash than the
  one shown to the user. Indirectly mitigates **U1** by forcing reviewable
  intent.

Any future change to this threat model must update `threat.rs` in the same
commit (or in a follow-up commit before the next release).

## Deferred threats (out of v0.1 scope)

These are tracked for future milestones and are explicitly **not** in v0.1:

- **T1: Physical seizure of an unlocked machine** — full-disk encryption
  is the user's responsibility; documented in CLI `--help`.
- **T2: Compromised developer machine pushing a malicious commit** —
  mitigated by signed commits (out of scope for v0.1 library).
- **T3: Side-channel timing on signing** — v0.1 uses standard
  `secp256k1` which is constant-time; no custom scalar math.
- **T4: Denial-of-service against the wallet via a malicious Esplora**
  — out of scope; user can switch endpoints via `WalletConfig`.
