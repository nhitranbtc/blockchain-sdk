# ADR 0001: Bitcoin Wallet Signing Model

**Date:** 2026-08-05
**Status:** Accepted
**Author:** Architecture review (companion to `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`)
**Deciders:** Tangem engineering
**Supersedes:** none
**Superseded by:** none

## Context

Tangem's iOS Bitcoin module (`tangem-app-ios/Modules/BlockchainSdk/Blockchains/Bitcoin/`, 2,070 Swift LOC) signs transactions via a hardware card (`TangemSdk`): private keys never enter the process, the host builds PSBTs and ships sighashes to the card, signatures come back as `SignatureInfo` and are attached.

The Phase 1 Rust rewrite (`bitcoin-wallet-core` + `btc` CLI, 29 tasks) deliberately ships **without** the hardware-signing boundary. Phase 1 is dev/test on Bitcoin testnet; the Rust core signs in-process with `secp256k1::Keypair` derived from a BIP-39 mnemonic stored on disk. This is fine for a developer tool, but it does not match Tangem's security model and cannot ship to end users as-is.

The Phase 2 mobile migration (UniFFI into iOS, replacing `Blockchains/Bitcoin/`) needs the `Signer` boundary back. The plan already has Task 28 (`tx::sign_external::Signer` trait) to preserve it. This ADR makes the security roadmap explicit so every contributor knows which release ships which threat model.

## Decision

| Release | Signing model | Mnemonic storage | Network | Audience |
|---|---|---|---|---|
| **v0.1** (current plan) | Software, in-process `secp256k1::Keypair` | Plaintext at `~/.local/share/btc/{name}/mnemonic.txt`, mode 0600, with strong console warning | Testnet default; mainnet opt-in via confirmation prompt | Internal developers, CI, dev workflows |
| **v0.2** | Same as v0.1 + Argon2id-encrypted mnemonic on disk | AES-256-GCM ciphertext at `~/.local/share/btc/{name}/mnemonic.enc`, key derived from user passphrase via Argon2id (m=64MB, t=3, p=4) | Same | Power users running btc as a daily CLI; small-stakes mainnet acceptable if user understands |
| **v1.0** (mobile, Phase 2) | Hardware, via `Signer` trait; iOS host provides `TangemSdk`-backed impl; Rust core never holds raw keys | Mnemonic generated on card, never exposed; existing `TangemSdk` flow | Mainnet + testnet + regtest | End users via the iOS app |

## Consequences

### v0.1 (current)

- **Acceptable for:** testnet dev, CI smoke tests, "scratch an itch" Bitcoin work.
- **NOT acceptable for:** any real-money mainnet use. Anyone with `read` access to `~/.local/share/btc/{name}/` can spend the funds. The CLI prints a `WARNING` line at wallet creation; that is the only mitigation.
- **Does not regress Tangem iOS:** the iOS app still uses `TangemSdk`. This release is a parallel CLI for developers, not a replacement of the production wallet.

### v0.2 (encryption milestone)

- **Adds:** Argon2id key derivation + AES-256-GCM at rest. `btc wallet create --passphrase "..."` and `btc wallet unlock` flow.
- **Threat model upgrade:** stolen disk image no longer reveals plaintext mnemonic. Attacker needs the passphrase too.
- **Remaining gap:** still software signing, still vulnerable to in-memory extraction (memory dumper, /proc/pid/mem on Linux, etc.). Acceptable for small-stakes mainnet ("coffee money"), NOT for high-value storage.
- **Blockers for v0.2:** recommendation 1 from `docs/wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md` — needs `argon2` and `aes-gcm` crate additions, a `keys::encrypted_mnemonic` module, and changes to `commands/wallet.rs` to accept/pass passphrase.

### v1.0 (mobile, Phase 2)

- **Threat model:** equivalent to today's Tangem iOS — keys never in process, signed by card, mnemonic generation on card.
- **Implementation path:** `Signer` trait in Task 28 (already in plan) is the bridge. Phase 2 plan wraps `TangemSdk` in a Swift-side `Signer` impl, calls `Wallet::build_tx` + `Wallet::sign_with_external_signer(psbt, signer)`, attaches the returned `SignatureInfo`.
- **Rust core change required:** add `Wallet::sign_with_external_signer(&self, psbt: &mut Psbt, signer: &impl Signer) -> Result<Transaction>` method. The Plan does not have this task yet — add as a Phase 2 task, NOT a v0.1 task.

## Alternatives considered

### A. Ship v0.1 with hardware-signing support (Ledger/Trezor)

**Rejected for v0.1.** Adds ~2 weeks of work, plus a USB transport layer (HIDAPI, `ledger-transport-hid`, `trezor-client`) that has no equivalent on iOS yet. Phase 1's goal is to validate the core API and CLI surface, not to ship a production security model.

### B. Use a remote signer service (e.g. Breez-style)

**Rejected for v0.1.** Requires either a hosted service (privacy + trust issues) or a custom transport for users to run their own signer. Out of scope for a CLI dev tool.

### C. Use Apple's Secure Enclave / Android Keystore on mobile only

**Already what Tangem does** (via `TangemSdk` calling the card). Not a Rust concern — it is the Swift host's job. Rust core stays hardware-agnostic via the `Signer` trait.

## Open questions

1. **Argon2id parameters:** the values proposed (m=64MB, t=3, p=4) are standard for desktop-class hardware. Should we tune for mobile (lower m, higher t)? Phase 1 is desktop-only, so the desktop values are correct. Mobile tuning lives in v1.0.
2. **Passphrase format:** BIP-39 passphrase is 7-bit ASCII, any length. The CLI should accept a passphrase via `--passphrase-prompt` (read from TTY) rather than `--passphrase "..."` (visible in shell history) by default in v0.2. Trivial change; defer to v0.2 plan.
3. **Key rotation:** if a user suspects mnemonic compromise in v0.1, the only fix is to create a new wallet and send funds to it. v0.2 should add a `btc wallet rotate` command that generates a new mnemonic and updates the address index. Defer.

## References

- `docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md` §1 Goal & non-goals
- `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` Task 28 (Signer trait)
- `docs/wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md` (Phase 1 vs Phase 2 coverage)
- `tangem-app-ios/Modules/BlockchainSdk/Blockchains/Bitcoin/BitcoinWalletManager.swift:183` (`func send(_:signer:)` — the `TransactionSigner` boundary the Phase 2 port must preserve)
- [BIP-39 passphrase spec](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [Argon2 RFC 9106](https://www.rfc-editor.org/rfc/rfc9106.html)
