# ADR 0004 — Deprecate `chain-traits`, then delete it

- **Status:** Proposed — awaiting maintainer sign-off
- **Date:** 2026-10-30
- **Amends:** [ADR 0003 — Cross-chain abstraction strategy](2026-10-30-adr-0003-cross-chain-abstraction-strategy.md)
- **Related:** tracker #1 ("[umbrella] Scaffold rust-wallet-app workspace + chain-traits crate", closed),
  #494 (family-reuse precedent), #579 and #580.
- **Companion spec:** `docs/superpowers/specs/2026-10-30-cross-chain-abstraction-design.md`
- **Review basis:** `blockchain-sdk-vault/reports/reviews/20261030-155200_grilling.html` (Q3, then
  verified on the spec and standards axes).

## Context

ADR 0003 decided that `chain-traits` **is deleted**, on the stated grounds that it is "a published
crate path" and that "external consumers may exist even though nothing in-repo depends on it".

That premise is **false**. `chain-traits` is a workspace member, not a published crate — crates.io
returns `crate 'chain-traits' does not exist`, as it does for every other crate in this workspace.
The justification therefore has to change, and with it the shape of the decision.

What is *not* in doubt is the underlying fact that motivated ADR 0003:

- zero `impl ChainWallet` anywhere in the tree;
- zero crates depend on it — `Cargo.lock:1977-1985` records the package with only external
  dependencies and no workspace member;
- its only would-be dependency line is commented out
  (`chain-traits/Cargo.toml:9` — `# bitcoin-wallet-core = { workspace = true }  # v0.1 — uncomment when on disk`);
- it drags `bitcoin` and `tokio` (full features) into the dependency graph for a trait that
  nothing implements;
- and its one trait is unimplementable for two of the three chains it names —
  `next_receive_address() -> Result<Address, ChainError>` returns `bitcoin::Address`
  (`chain-traits/src/lib.rs:12,50`), and `ChainId` has no TRON or Polygon variant
  (`chain-traits/src/lib.rs:17-24`).

So the crate is dead code in every workspace build, and the reason to act is **hygiene, not
downstream-consumer risk**.

## Decision

**Deprecate `chain-traits` with a release note and a removal milestone; delete it afterwards.**

1. Mark the crate deprecated in its own documentation and in the workspace `Cargo.toml`
   description, naming ADR 0004 and the removal condition.
2. Announce the removal in the next release notes, with the removal condition stated.
3. Remove the crate from the workspace `members` list once the condition is met.

**Removal condition:** the crate has been marked deprecated for one release cycle **and** no
crate in the workspace depends on it. The second half is already true and must be re-checked at
removal time.

## Alternatives considered

### A. Delete it outright, as ADR 0003 said

*Rejected — this ADR supersedes that decision.* The crate is unpublishable in the sense that
nothing consumes it, so an abrupt removal carries no consumer risk. But it is a workspace member
of a workspace that ships five chain engines, and a deprecation window is the cheapest way to
leave a trail for anyone whose build breaks for a reason nobody predicted — the same reasoning
that produced the review's other "hygiene, not risk" corrections.

### B. Revive it as family-scoped traits (`UtxoChain` / `EvmChain` / `AccountChain`)

*Rejected.* This repeats the failure that killed the crate. A trait spanning engines must have
signatures all of them can satisfy, and the account-vs-UTXO split shows there is no single
signature that fits both. It also requires every engine to change, to serve a consumer that
ADR 0003's facade serves without touching an engine.

### C. Keep it, but fix the trait shape

*Rejected.* A trait whose only implementor could be Bitcoin is not an abstraction. Fixing the
address type to chain-neutral bytes would leave `ChainId` without TRON or Polygon, `sync(&self)`
without a client parameter that every engine's sync needs, and no surface for signing, transfer,
token operations, fee estimation, history or confirmation — the capabilities that actually
dominate this SDK. A corrected trait would still have no implementors.

### D. Do nothing

*Rejected.* The crate stays in every workspace build, carrying `bitcoin` and `tokio` for nothing,
and the README's claim that "per-chain crates implement" it stays false.

## Consequences

**Good**

- The crate leaves the workspace and the dependency graph sheds `bitcoin` + `tokio` for a trait
  nobody implements.
- The decision is recorded where a reader of ADR 0003 will find it, rather than by silent edit.
- The removal condition is explicit and re-checkable, so the deprecation cannot become permanent.

**Bad / accepted**

- One release cycle carries a deprecated crate. That is the cost of the trail.
- The deprecation touches `Cargo.toml` and the release notes — L24's doc set travels with it.

**Neutral**

- No engine's behaviour, public API, wallet-file format, or on-disk layout changes.
- ADR 0003 stands otherwise; only the disposition of `chain-traits` changes.

## Note on process

This amendment is a **new ADR**, not an edit to ADR 0003 — `CLAUDE.md` requires monotonic
numbering and that ADRs capture the decision *and* the rejected alternatives. Editing 0003 in
place would erase the record that the original rationale was wrong.

The correction itself came from the grilling of the recommendations document: answer Q3 was
originally supported by the false "published crate" premise, and the spec-axis review of that
answer's evidence is what caught it. The design spec was updated in the same pass
(`§Components`, `§Open questions`).
