# Design: Cross-chain abstraction for the blockchain-sdk Rust workspace

> **Status:** Proposed — **approved for restructuring 2026-10-30**.
> **Reconciled** against the grilling (`blockchain-sdk-vault/reports/reviews/20261030-155200_grilling.html`)
> and its two-axis review; corrections are marked **⟵ corrected on review**. ADR 0003 is
> amended by **ADR 0004** (`docs/adr/2026-10-30-adr-0004-chain-traits-deprecation.md`).
> **ADR:** `docs/adr/2026-10-30-adr-0003-cross-chain-abstraction-strategy.md`
> **Plan (Phase 1):** `docs/superpowers/plans/2026-10-30-wallet-errors.md`
> **Related tracker:** #494 (family-reuse precedent), #579, #580 (defects filed alongside).
> **Review inputs:** three independent ECC review passes (architecture/layering, Rust API and type
> design, security) plus a coordinator pass over source. Full reports: `/tmp/archreview/*.md`.

## Summary

The workspace is five well-built, largely independent chain engines plus one abandoned cross-chain
scaffold. `chain-traits` was delivered as a scaffold under issue #1 and never adopted; its one
trait is shaped so that only Bitcoin could implement it. Meanwhile four genuinely cross-chain
concerns — error taxonomy, wallet-at-rest crypto, atomic write, SPKI pinning — have each been
re-implemented per chain, and the copies have drifted apart in security posture.

This design retires `chain-traits` and replaces it with a **facade at the edge**: a new
`wallet-facade` crate that depends on the five engines and unifies them at the top, so no engine
has to change. It is preceded by `wallet-errors`, which fixes the cheapest real divergence (four
error taxonomies that disagree) and is valuable on its own.

## Findings

Three ECC review passes produced 26 findings. The load-bearing ones:

| ID | Sev | Finding | Evidence |
|---|---|---|---|
| F1 | HIGH | `polygon-wallet-core` re-exports `new_http_insecure`, which `evm-wallet-core` gates behind `#[cfg(any(debug_assertions, feature = "insecure_tls"))]`. No crate enables the feature, so **`cargo build --release --workspace` fails** with `E0432`. No CI job compiles polygon in release. | `polygon-wallet-core/src/lib.rs:39-41`; `evm-wallet-core/src/provider.rs:59`; `evm-polygon-ci.yml:115` |
| F2 | HIGH | The two C ABIs are mutually incompatible: Bitcoin returns negative `i32` codes with unprefixed symbols; Solana returns positive codes with `sol_wallet_`-prefixed symbols and a different last-error mechanism. A host cannot share one error-mapping layer. | `bitcoin-wallet-core/src/ffi/error.rs:35-100`; `sol-wallet-core/src/ffi.rs:76-124` |
| F3 | HIGH | Solana's FFI has **no panic barrier** — `FfiError::Panic` is documented but never constructed, and there is no `catch_unwind` in the crate. Bitcoin's FFI has one (`ffi_catch_unwind` + `scrub_panic_message`). The release-mobile profile sets `panic = "abort"`, so a panic becomes a host process abort. | `sol-wallet-core/src/ffi.rs:12-26`; `bitcoin-wallet-core/src/ffi/panic.rs:58-195` |
| F4 | HIGH | Solana's FFI maps every unmapped error to `FfiError::Unimplemented` (=98, "not yet implemented"). A bad mnemonic is reported to the host as a missing feature. Bitcoin's `From<Error> for FfiError` has **no catch-all arm**, so a new error variant is a compile error — the discipline Solana lacks. | `sol-wallet-core/src/ffi.rs:152`; `bitcoin-wallet-core/src/ffi/error.rs:126-131` |
| F5 | HIGH | `eth` and `polygon` each hand-write a `WalletError → Error` table and they disagree: `AlreadyExists` 4 vs 2, `NotFound` 4 vs 2, `Corrupt` 5 vs 3, `Crypto` 5 vs 2. `btc` has no classifier; `polygon` collapses everything to 1. The documented table (`CONTEXT.md:89-98`) matches none of them. | `evm-wallet-core/src/error.rs:184-208`; `sol/src/handlers/error.rs:27-71`; `tron/src/handlers/mod.rs:46-93`; `polygon/src/main.rs:104-113` |
| F6 | HIGH | `sol-wallet-core::platform::ReqwestClient` is public and re-exported, but its `NetworkClient` impl always returns `Unsupported` with "wire to chain::client in Phase 7". Phase 7 shipped; the stub was never replaced. | `sol-wallet-core/src/platform/` |
| F7 | HIGH | `evm-wallet-core::mnemonic::generate_12_word` calls `.expect()` on the RNG path — a panic inside a library function with no error variant — and `derive_address` copies the secret phrase into a plain `String`. Solana solved both (`Error::OsRngFailed`, `Zeroizing`). | `evm-wallet-core/src/mnemonic.rs` |
| F8 | HIGH | Solana accepts, persists and prints an SPKI pin that is **never enforced** — `pinned_spki` is `None` in every production client; `new_with_pinned_spki` is only called from tests. | `sol-wallet-core/src/chain/client.rs:205,328,414-424` |
| F9 | HIGH | Solana's Argon2id is 64 MiB / t=3 / p=1 desktop (16 MiB mobile) against 256 MiB / t=10 / p=4 elsewhere — and the cap check `memory_kb >= MAX` makes exactly 256 MiB **inadmissible**. | `sol-wallet-core/src/crypto.rs:35,59-70,89-105` |
| F10 | HIGH | `WalletStorage` exists twice with incompatible shapes (Solana: 4 methods, `&str` key, `Err` on missing; TRON: 5 methods, `WalletId` key, `Ok(None)` on missing), and Bitcoin and EVM have no trait at all. | `sol-wallet-core/src/platform/storage.rs:16-25`; `tron-wallet-core/src/platform/storage.rs:36-66`; `bitcoin-wallet-core/src/wallet/store.rs`; `evm-wallet-core/src/wallet.rs` |
| F11 | HIGH | Four atomic-write implementations at three different security postures. Bitcoin refuses symlinked destinations *and* parents and fsyncs the parent dir; EVM's private `write_atomic` does none of that. | `bitcoin-wallet-core/src/util/atomic_write.rs:26-79`; `evm-wallet-core/src/wallet.rs:830-846` |
| F12 | HIGH | `WalletId` is implemented four times with four shapes; the token-registry concept three times with three incompatible return conventions. | `evm-wallet-core/src/wallet.rs`; `sol-wallet-core/src/`; `tron-wallet-core/src/` |

Plus the two correctness defects now filed as issues: **#579** (Bitcoin scan never inserts the
containing transaction, so confirmed balance is not aggregated) and **#580** (EVM re-derives the
signer at index 0 — a wallet imported at `--account-index N` signs with the wrong key).

### Correction to an earlier claim

The review **refuted** one claim from the report set: tracker #411 says TRON's SPKI verifier is
fail-open, citing `rpc.rs:121-124`. **No `rpc.rs` exists in the workspace.** TRON's only client is
`TronGridClient`, whose verifier is correctly composed (webpki chain check first, then a
constant-time pin compare) and bound to the transport via `use_preconfigured_tls`
(`tron-wallet-core/src/chain/spki.rs:155-178`, `chain/client.rs:76-81`). #411's file reference is
stale and should be re-pointed or closed. TRON's real transport defect is different: cleartext
`http://` is accepted for any host (`chain/client.rs:71-92`).

### Already good — do not refactor away

- The **EVM family pattern** is the correct template: one `evm-wallet-core`, thin chain wrappers,
  with `Network::all()` derived by per-family iteration so a new chain family is a compile error
  at every call site (`evm-wallet-core/src/network.rs:98-117`).
- **`polygon-wallet-core` deliberately does not glob re-export** (unlike `eth-wallet-core`), which
  stops a Polygon caller passing `chain_id = 1` and receiving Ethereum USDC. The narrower list is
  a feature (`polygon-wallet-core/src/tokens.rs:10-17`).
- **Chain-specific error granularity is a security feature.** Bitcoin keeps `WalletStore` separate
  from `Storage` to preserve a file-existence oracle closure
  (`bitcoin-wallet-core/src/error.rs:109-120`); Solana does the same for `WalletDecryptFailed` vs
  `RecordCorrupt`. Any shared error model must be a **category mapping**, not a merged enum.
- **Blob/AAD divergence is intentional.** Bitcoin binds AAD to the network discriminant to stop
  cross-network ciphertext reuse; Solana binds a versioned AAD prefix. Do not unify envelopes.
- The **TRON anychain wrapping rule** — never re-export upstream types, wrap them so an exact-pin
  bump is a one-file change (`docs/wallets/CONTEXT.md:156-159`) — is the right precedent.
- The **vendored anychain stack** (path deps, `SOURCE.md` per crate, exact pins) means consumer
  builds never resolve crates.io for those names.
- Bitcoin's `Secret<T>`, `XPrvHolder`'s deliberately narrow surface, Solana's `pub(crate)
  as_keypair`, Bitcoin's panic scrubber, and TRON's compile-time `const _: ()` blob-layout
  assertions are all exemplary.

## Architecture

### Current dependency edges

```
btc ──────────────► bitcoin-wallet-core          (rlib+cdylib; FFI inside the engine)
eth ──────────────► eth-wallet-core ────────────► evm-wallet-core    (pub use evm_wallet_core::*)
polygon ──────────► polygon-wallet-core ────────► evm-wallet-core    (explicit re-export + disambig/tokens)
polygon ──────────► evm-wallet-core              (direct — bypasses its own wrapper)
polygon ──────────► bitcoin-wallet-core          (direct but UNUSED — links BDK for nothing)
tron ─────────────► tron-wallet-core ───────────► anychain-{core,tron,kms}   (vendored)
sol ──────────────► sol-wallet-core              (rlib+cdylib; FFI inside the engine)
chain-traits ◄──── NO INBOUND EDGE               (zero dependents)
```

### Target dependency edges

```
btc ──────────────► bitcoin-wallet-core ──┐
eth ──────────────► eth-wallet-core ──────┤
polygon ──────────► polygon-wallet-core ──┼──► wallet-facade ──► wallet-errors
tron ─────────────► tron-wallet-core ─────┤
sol ──────────────► sol-wallet-core ──────┘
```

Two new crates; **no engine edge changes**. `chain-traits` is **deprecated with a release note,
then deleted** (⟵ corrected on review, Q3 — ADR 0003 said "removed"; the amendment is a new
monotonic ADR, not an in-place edit). The dependency direction is
inverted: the facade depends on the engines rather than the engines depending on an abstraction.

### Why a facade, not a trait

A trait spanning five engines needs signatures all five can satisfy — which is precisely where
`chain-traits` failed (`next_receive_address() -> bitcoin::Address`). A facade does not require
agreement; it dispatches. The chain-neutral surface can therefore be as narrow as it needs to be,
because the parts that genuinely differ are simply not in it.

The facade also mirrors the repo's own successful pattern. The EVM family split worked because it
did **not** ask Bitcoin to look like EVM; it said "these two chains share an engine, these others
don't." The facade says the same thing at the cross-family level.

## Components

### 1. `wallet-errors` (new crate, Phase 1)

One shared error **category** enum and one exit-code table.

```rust
pub enum ErrorCategory { Success, User, Network, InsufficientFunds, Signing, Broadcast }

/// ⟵ corrected on review (Q6/Q8): the variant→category mapping lives in the CLI layer,
/// NOT in the engines. `evm_wallet_core::Error::exit_code()` is public engine API today;
/// Phase 1 deletes or delegates it, because leaving it in place makes it a *second*
/// competing table — which is the F5 defect itself. Putting the mapping in the engines
/// would give every engine a `wallet-errors` edge, contradicting "engines change nothing".

/// The category SET is fixed to the SIX already in `docs/wallets/CONTEXT.md:91-98`.
/// The earlier draft of this spec proposed eight (adding `Storage` and `Crypto`); that
/// would contradict the document Phase 1 promotes to the tested spec. Either the table
/// grows to eight and the doc moves with it, or the set stays at six — but not both.
/// `CONTEXT.md:96` additionally needs a disambiguation column: eth maps `InvalidPassword`
/// → 2 and `DecryptionFailed` → 5, so "wrong password" is ambiguous as written.
```

The per-chain error enums **stay** — their granularity is a security feature (§ above). What
unifies is the *category* each variant maps to. Every CLI maps its error to a category and then to
an exit code through one shared function.

`docs/wallets/CONTEXT.md:89-100` becomes the specification, and a CI test asserts every CLI's
mapping matches it.

### 2. `wallet-facade` (new crate, Phase 2)

A chain-tagged enum plus chain-neutral value types.

**⟵ corrected on review (Q7):** the enum is the whole shape. Per-chain *extension traits* are
**not** added now: chain-specific operations stay behind each engine's own API and are reached
by matching the variant, so a trait layer would have no consumer (Speculative Generality).
Extension traits are added when a caller genuinely needs a homogeneous collection.

**⟵ corrected on review (Q17):** the exit criterion names a consumer that does not exist yet.
This spec must therefore carry **that consumer's required surface**, and the facade is built to
it rather than to an imagined general one. This is the PL12 sign-off artefact for Phase 2 and it
is gated on the answer to Open question 1.

```rust
pub enum Chain { Bitcoin, Ethereum, Polygon, Tron, Solana }

pub struct WalletSummary {
    pub chain: Chain,
    pub network: NetworkId,     // chain-scoped, e.g. "bitcoin/testnet"
    pub id: WalletId,           // opaque, chain-scoped
    pub name: String,
    pub address: Address,       // { kind: AddressKind, bytes: Vec<u8>, display: String }
}

pub enum AddressKind { Base58, Bech32, Bech32m, Eip55, Base58Check }

pub struct Balance { pub base_units: u128, pub decimals: u8, pub symbol: String }

pub enum AnyWallet { Bitcoin(..), Evm(..), Polygon(..), Tron(..), Solana(..) }

impl AnyWallet {
    pub fn chain(&self) -> Chain;
    pub fn list(&self) -> Result<Vec<WalletSummary>, Error>;
    pub fn unlock(&self, id: &WalletId, secret: &SecretString) -> Result<Session, Error>;
}

pub struct Session { /* chain-tagged, opaque */ }
impl Session {
    pub fn address(&self) -> &Address;
    pub fn balance(&self) -> Result<Balance, Error>;
}
```

`Balance` carries base units *and* decimals rather than a single integer, because the native units
differ in scale (sats, wei, sun, lamports) and a generic caller cannot compare them without the
decimals. `Address` keeps raw bytes plus a display form, because a generic caller must not assume
one string format.

**`send` is deliberately absent from the facade.** Transaction construction, signing, broadcast and
confirmation differ between UTXO and account models by construction; a generic `send` would either
be wrong or a lowest-common-denominator that forces every chain through a lossy shape. The caller
matches on the chain tag for that.

### 3. `wallet-storage` (new crate, Phase 3 — deferred)

One `WalletStorage` trait and one audited `atomic_write`, migrated across the four engines. The
Bitcoin implementation is the reference: refuse symlinked destination **and** parent, refuse a
`.tmp` destination, set mode before write, fsync the file and the parent directory.

**⟵ corrected on review (Q12):** three corrections.
1. **The key type is a prerequisite, not a detail.** bitcoin has `WalletId(Uuid)`, tron
   `WalletId([u8;16])`, sol `WalletId(uuid::Uuid)`, and **EVM has no `WalletId` at all** — bare
   `Uuid`. Settle the key before freezing the trait (this is Open question 2, and it must be
   answered *before* Phase 3, not during it).
2. **TRON's trait has five methods, not four** — `put_atomic`, `put`, `get`, `list`, `delete`.
3. **Write policy does not belong in the trait.** The trait exposes `put`; the atomic-write
   implementation is a shared audited helper it calls. Putting `put_atomic` in the trait is where
   the four divergent copies live, and collapsing them requires the policy to live in one place.

This is the single highest-value security consolidation in the workspace, because the copies have
already drifted in the unsafe direction.

### 4. `wallet-crypto` (new crate, Phase 4 — deferred)

Argon2id + AEAD **primitives only**. Never the envelope. Envelope format is per-chain data.

## Data flow

Today a multi-chain program writes:

```rust
match chain {
    Chain::Bitcoin => bitcoin_wallet_core::Wallet::open(...)?,   // different types
    Chain::Ethereum => evm_wallet_core::WalletManager::open(...)?,
    ...
}
```

After the facade:

```rust
let wallet = facade::open_wallet(&chain, &network, &data_dir)?;   // AnyWallet
let session = wallet.unlock(&id, &secret)?;                       // one signature
println!("{}", session.address().display);
println!("{}", session.balance()?.base_units);
```

The chain-specific path stays available, one match arm away:

```rust
if let AnyWallet::Evm(evm) = &wallet { evm.send(...)?; }
```

## Error handling

- **Per-chain error enums stay.** Their granularity is deliberate and security-relevant.
- **One category mapping.** Each per-chain variant maps to exactly one `ErrorCategory`.
- **One exit-code table.** `ErrorCategory -> i32` is defined once; every CLI calls it.
- **No catch-all in the FFI error conversion.** Bitcoin's `From<Error> for FfiError` deliberately
  has no catch-all arm, so a new error variant is a compile error forcing an explicit ABI
  decision. Solana's FFI must adopt this (F4) and add the `catch_unwind` barrier (F3).
- **Wrong-password and corrupt-blob stay indistinguishable** in every chain.

## Testing

- **`wallet-errors`:** table-driven tests per CLI asserting each error variant maps to the expected
  category and exit code, driven from the `CONTEXT.md` table so the doc is the spec.
- **`wallet-facade`:** conformance tests — each engine gets a test that constructs it through the
  facade and asserts `chain()`, `list()`, `address()` and `balance()`. The facade's own tests cover
  dispatch, not each engine's internals.
- **`wallet-storage`:** a shared suite asserting the security properties — mode `0o600`, parent
  `0o700`, symlinked destination refused, symlinked parent refused, parent fsync attempted, temp
  file removed on any early return.
- **No new Rust coverage tooling is assumed.** The workspace has no Rust coverage measurement
  today; this design does not depend on adding any.
- **⟵ corrected on review (Q18):** CI is an **added gate, not the verification authority.** L12
  requires the review pair to run *before* the verify gate, and L13 steps 10/10a/10c sit ahead of
  the step-11 verify gate. The nightly conformance run does not substitute for either.
- **The conformance suite (Q16) is one suite run against every engine behind the facade** — the
  facade's value is that every engine behaves the same way through it; without a shared suite,
  "the same way" is an assertion rather than a checked property.

## Sequenced plan

| Phase | Scope | Depends on | Risk |
|---|---|---|---|
| **0** | The two filed defects: #579 (Bitcoin balance aggregation), #580 (EVM wrong-key signing). Plus the release-build break (F1) and the two Solana FFI CRITICALs (F3, F4). | nothing | Low — each is an isolated fix with a regression test. **None depend on the architecture decision.** |
| **1** | `wallet-errors`: one category enum (six, per `CONTEXT.md:91-98`), one exit-code table, five CLIs migrate, `CONTEXT.md` becomes the tested spec — and `CONTEXT.md:79-81` is corrected (it marks `--rpc`/`--spki-pin` "All chains ✓", which is false). The mapping lives in the CLI layer; `evm_wallet_core::Error::exit_code()` is deleted or delegated. | nothing | Low — no engine internals change; a CLI migrates independently. |
| **2** | `wallet-facade`: chain-neutral types, chain-tagged enum, five engine adapters. Gated on this spec carrying the consumer's required surface (Open question 1). | Phase 1 (needs the shared `Error`) | Medium — new crate, additive, no engine change. |
| **3** | `wallet-storage`: one `WalletStorage` trait + one audited `atomic_write`; four engines migrate. | nothing; runs parallel to Phase 2 | Medium-High — touches every engine's write path. Bitcoin's is the reference. |
| **4** | `wallet-crypto` primitives (KDF/AEAD only), and — only if a second chain becomes host-reachable — the FFI convention unification (F2). ⟵ corrected on review (Q10/Q14): the FFI deferral is **one** decision with **one** backlog item (`priority/p2` + `backlog` + a `progress.md` entry), not two. | Phase 3 | High — the FFI rename touches 32 Bitcoin symbols and the Dart bindings. |

Phase 0 and Phase 1 can run in parallel. Phases 2 and 3 can run in parallel once Phase 1 lands.

**⟵ Process compliance (Q9/Q15/Q18, standards axis).** Every phase has three or more sub-tasks,
so each names a parent branch under L25 (`task/<N>-<slug>`, ordered `--no-ff` merges,
PR-to-parent) and carries the L24 doc set with its PR (CHANGELOG `[Unreleased]`, the per-chain
user-stories table, the "Try it" column). What releases independently: `wallet-errors` ships
alone at the end of Phase 1, the facade alone at the end of Phase 2, storage only after Phase 3.
Every deferral is tracked as `priority/p2|p3` + `backlog` plus a `progress.md` entry (L13 step
11a, L14). Issue filing carries a triage role label, a chain label, `task`, `backlog` +
`priority/pN`, severity in the body, and a PAUSE before `gh issue create`
(`docs/agents/triage-labels.md`, L9, L13). **CI is an added gate, not the verification
authority:** L12's review pair runs *before* the step-11 verify gate (L13 10/10a/10c).

## Open questions

1. **Does the multi-chain program need chain-generic `send`?** If yes, the facade's boundary is
   wrong and Option A (family-scoped traits) must be reconsidered. This design assumes no.
2. **What is the caller's unit of identity?** `WalletId` is chain-scoped in the facade. If a caller
   needs a stable id across chains, the facade needs a composite key — and the four existing
   `WalletId` shapes (F12) must be reconciled.
3. **Should `wallet-errors` be its own crate, or live in `wallet-facade`?** They are separable
   because Phase 1 is useful without Phase 2, but one crate is simpler to version.
4. **Should Bitcoin's free-function store API become trait methods?** Bitcoin and EVM have no
   storage trait today; adopting one changes their public shape.
5. ~~**Is deleting `chain-traits` safe for downstream consumers?**~~ **ANSWERED (Q3).** The
   premise was false: `chain-traits` is **not** a published crate — crates.io has no such crate,
   as with every workspace member. The reason to act is hygiene: a dead member of every workspace
   build. **Decision: deprecate with a release note, then delete**, via **ADR 0004**.
6. **Which `WalletStorage` shape wins** — TRON's shape wins (five methods, `Ok(None)` on a
   missing key: a missing key is not an error). **But the key type is still open (Q2)** and is a
   **prerequisite for Phase 3**, not a detail of it: four incompatible `WalletId` shapes exist and
   EVM has none. This must be answered *before* Phase 3 is planned, not during it.

## Verification status

- Structural claims are source-verified with file:line; the reviewers read the cited files directly.
- **The baseline has now been MEASURED** (2026-10-30) — see
  `blockchain-sdk-vault/reports/reviews/20261030-155200_measured-baseline.md`. Five crates build in
  debug and **718 tests pass**; `cargo build --release --workspace` **fails** with `E0432` on the
  `new_http_insecure` re-export, because the `debug_assertions` gate that hides the symbol in
  release is exactly what keeps the debug suite green. A flaky BIP-39 checksum test was found and
  fixed (it failed ~1 run in 256).
- **Still unmeasured:** the `#[ignore]`d live-network suites, the five CLI binaries' own suites,
  the Flutter desktop host, `cargo deny` / `cargo audit` / `cargo geiger`, and any coverage.
- The two correctness defects' *mechanisms* are confirmed from source; their *runtime effects* are
  inferred and explicitly labelled as such in both issues.
