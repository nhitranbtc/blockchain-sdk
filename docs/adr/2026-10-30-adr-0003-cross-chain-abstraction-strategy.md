# ADR 0003 — Cross-chain abstraction strategy

- **Status:** Proposed (awaiting maintainer sign-off)
- **Date:** 2026-10-30
- **Supersedes:** nothing. `chain-traits` (scaffolded under tracker issue #1) is **retired**, not
  superseded. ADR 0001 and ADR 0002 are untouched.
- **Amended by:** [ADR 0004 — Deprecate `chain-traits`, then delete it](2026-10-30-adr-0004-chain-traits-deprecation.md).
  ADR 0004 changes only the **disposition** of `chain-traits`: this ADR said "delete outright", ADR
  0004 says "deprecate with a release note, then delete", because the "published crate path"
  premise this ADR relied on is false. The retirement itself, and every other decision here, stand.
- **Related:** tracker #494 (the family-level reuse precedent this generalises), #579 and #580
  (defects filed alongside this ADR).
- **Companion spec:** `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md`

## Context

`rust-wallet-app` ships five chain engines — Bitcoin, Ethereum, Polygon, TRON, Solana — as five
independent library+CLI pairs, plus an umbrella crate `chain-traits`. The engines are individually
strong; the crypto and key-management layers are among the best in the workspace
(`bitcoin-wallet-core/src/keys/secret.rs:15-59`, `sol-wallet-core/src/wallet.rs:258-317`,
`tron-wallet-core/src/crypto/mod.rs:302-311`).

Three facts frame this decision:

1. **`chain-traits` is dead.** Zero `impl ChainWallet` in the tree; zero dependents; its dependency
   line is commented out (`chain-traits/Cargo.toml:9`). It shipped as a scaffold under issue #1 and
   was never adopted. Its one trait is also **unimplementable** for two of the three chains it
   names: `next_receive_address() -> Result<Address, ChainError>` returns `bitcoin::Address`
   (`chain-traits/src/lib.rs:12,50`), and `ChainId` has no TRON or Polygon variant
   (`chain-traits/src/lib.rs:17-24`). `sync(&self)` also takes no client parameter while every
   engine's sync takes one — so the signature forces each impl to own a long-lived transport.

2. **The working unit of reuse in this repo is the FAMILY, not the chain.** The Polygon plan
   chose "one `evm-wallet-core`, thin `eth` + `polygon` wrappers", and `docs/wallets/CONTEXT.md:8`
   names `evm-wallet-core` as the canonical EVM crate. Family-level reuse demonstrably works;
   cross-family reuse has never been attempted successfully.

3. **There is a real cross-family consumer.** A single Rust program is expected to drive wallets on
   several chains. That consumer is a *library* consumer — not a native host, and not a shell
   script. It needs one type to hold and drive multi-chain wallets.

Meanwhile four cross-chain concerns have each been re-implemented per chain, and the copies have
drifted: wallet-at-rest KDF parameters (three identical Argon2id constant blocks plus one
deliberately weaker), atomic-write-with-0600 (four implementations at three different security
postures), SPKI pinning (three verifiers plus one that stores a pin and never checks it), and the
error/exit-code taxonomy (four implementations that disagree — `AlreadyExists` is exit 4 in `eth`
and exit 2 in `polygon`; `Corrupt` is 5 vs 3).

## Decision

**Adopt the facade-at-the-edge pattern: a new `wallet-facade` crate that depends on the five
engine crates and unifies them at the top. Retire `chain-traits`. Do not ask the engines to
implement a shared trait.**

- **`chain-traits` is deleted** from the workspace `members` list. Its trait shape is the proven
  cause of its own failure; it is not repaired, because repairing it would require every engine to
  change — the cost that was already paid once and rejected.
- **`wallet-facade` is added.** It depends on the five engine crates and exposes a chain-tagged
  enum (`AnyWallet`) plus chain-neutral value types (`Address`, `Balance`, `WalletId`,
  `NetworkId`) and a single shared error category. **Engines change nothing to adopt it.**
- **The dependency direction is inverted.** `chain-traits` asked five engines to implement a trait
  they could not all satisfy. `wallet-facade` asks nothing of the engines; it depends on them. A
  sixth chain touches one crate, not five.
- **Chain-specific operations stay chain-specific.** Transaction construction, signing, broadcast
  and confirmation differ between UTXO and account models by construction. The facade covers the
  wallet lifecycle — enumerate, unlock, identify, balance — and returns a chain-tagged handle for
  everything else. A caller needing chain-specific behaviour matches on the chain tag.
- **`wallet-errors` is added ahead of it.** One error-category table, one shared `Error`, every CLI
  routing through it. `docs/wallets/CONTEXT.md:89-100` becomes the specification that CI tests,
  replacing the current state where the documented table matches none of the three implementations.
- **Envelope formats are never unified.** The wallet-file envelope is per-chain data. Only the KDF
  and AEAD *primitives* are candidates for a shared crate, and only after the layers above.

### Why the facade and not a trait

A trait spanning five engines must have signatures all five can satisfy. `chain-traits` failed
exactly there: it generalised Bitcoin's method signatures by name, so the address type stayed
Bitcoin's. A facade inverts the problem — it does not require agreement, it *dispatches*. The
chain-neutral surface can then be as narrow as it needs to be, because the parts that genuinely
differ are simply not in it.

This also respects the repo's own demonstrated judgement: the EVM family split was the right call
at *family* granularity. This ADR extends that instinct to the cross-family case without asking
any engine to change.

## Alternatives considered

### A. Make `chain-traits` real — family-scoped traits

Replace `ChainWallet` with `UtxoChain` / `EvmChain` / `AccountChain` using the account-vs-UTXO
split as the real boundary; add TRON and Polygon to `ChainId`; make addresses chain-neutral bytes.

*Rejected.* It reproduces the failure mode rather than avoiding it. A trait still has to be
implementable by all its implementors, and the account-vs-UTXO split shows there is no single
signature that fits both. It also requires every engine to change and re-verify — the highest
churn — to serve a consumer a facade serves without touching an engine. Adopt only if a caller
must hold chain-specific behaviour in a *homogeneous* collection; that need has not been shown.

### B. Delete `chain-traits`, share only genuinely-shared primitives

Delete the crate; add `wallet-crypto` (Argon2id + AEAD primitives, never the envelope),
`wallet-storage` (one `WalletStorage` trait + one audited `atomic_write`), `wallet-errors`.

*Rejected as the complete answer; adopted as its subset.* Right about the primitives — three
divergent copies of "write a wallet file safely" is a real security cost. But it does not answer
the consumer's question, "give me one thing to drive several chains with". It was also derived
under the assumption that no multi-chain consumer exists; that assumption has since been answered,
and the answer is yes.

### C. Unify only the host-facing surface

Standardise the two C ABIs (one returns negative error codes with unprefixed symbols, the other
positive codes with `sol_wallet_`-prefixed symbols) and the CLI exit-code contract.

*Deferred, not rejected.* This serves native hosts and shell automation. Today exactly **one** of
five chains is reachable from a native host in this repo, and the stated consumer is a Rust
program, not a native host. The exit-code half is pulled forward into `wallet-errors`; the ABI
half waits until a second chain needs to be reachable from a host, at which point renaming 32
Bitcoin symbols plus the Dart bindings is worth paying once rather than twice.

### D. Do nothing

*Rejected.* It leaves `chain-traits` in every workspace build, four divergent error taxonomies,
four atomic-write implementations at three security postures, and no answer to the consumer's
question.

## Consequences

**Good**

- The multi-chain Rust program gets one type to work with, and no engine has to change for it.
- The dead crate leaves the workspace. `chain-traits` currently drags `bitcoin` + `tokio` (full
  features) into the dependency graph for a trait nobody implements.
- One error taxonomy replaces four, and the documented table becomes testable.
- `wallet-facade` is additive: every existing crate and CLI keeps working.

**Bad / accepted**

- The facade couples to all five engines; a sixth chain means touching it. One crate, not five.
- The facade cannot offer chain-generic `send`. A caller that wants to send on any chain matches on
  the chain tag. Honest — the transaction models genuinely differ — but less ergonomic.
- `wallet-storage` and `wallet-crypto` are deferred, so the atomic-write drift persists through
  Phase 2. Bitcoin stays the reference; the EVM implementation (no symlink defence, no parent
  fsync) remains the weakest until Phase 3.

**Neutral**

- Nothing in this ADR changes engine behaviour, wallet-file formats, or any on-disk layout.
- The two correctness defects found during this review (#579, #580) are **not** resolved by this
  ADR and do not depend on it.

## Status of the review that produced this ADR

Three independent ECC review passes (architecture/layering, Rust API and type design, security)
plus a coordinator pass over source. **12 CRITICAL/HIGH findings were produced; only 2 are filed as
issues** (#579, #580) because the rest are already covered by existing tracker items, are
documentation drift rather than architecture, or are addressed by later phases of the companion
spec. The full finding list with evidence is in the companion spec §Findings.
