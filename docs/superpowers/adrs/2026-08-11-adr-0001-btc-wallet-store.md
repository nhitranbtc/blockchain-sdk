# ADR 0001: `btc` wallet-store filesystem layout — pure `MnemonicCipherBlob` persistence (F14 stays deferred)

> **Status:** Proposed (2026-08-11)
> **Deciders:** Nhi Tran
> **Issue:** [#60](https://github.com/nhitranbtc/blockchain-sdk/issues/60) (Task 54-precursor)
> **Blocks:** [#64](https://github.com/nhitranbtc/blockchain-sdk/issues/64) (Task 54d — wallet create/show)
> **Supersedes:** None
> **Related:** [#28](https://github.com/nhitranbtc/blockchain-sdk/issues/28) (MnemonicCipherBlob, merged), [#23](https://github.com/nhitranbtc/blockchain-sdk/issues/23) (`atomic_write` + 0o600, merged), `wallet/mod.rs:60-63` (current F14 deferral comment), `config.rs:50` (`db_path` field), `error.rs` (needs new variant, see Acceptance), `crypto/mnemonic_cipher.rs` (needs AAD support, see Cross-references)

## Context

Issue #60 asks two questions blocking #54d:

1. **F14 un-defer or keep deferred?** `wallet/mod.rs:60-63` currently defers `bdk_file_store` SQLite persistence to v0.1.1. #54d (`btc wallet create` / `btc wallet show`) needs an encrypted wallet on disk. Two paths:
   - **A — un-defer F14:** add `bdk_file_store` dep, implement atomic-flush, audit SQLite snapshot format + encryption-at-rest.
   - **B — keep F14 deferred:** persist only the encrypted mnemonic blob (`MnemonicCipherBlob`, already shipped in #28). `btc wallet show` re-syncs from Esplora on demand (current `Wallet::sync` behavior).
2. **Filesystem layout** for whichever path above — path, naming, perms, format.

The current code already foreshadows a sidecar pattern:

- `WalletConfig.db_path: PathBuf` (`config.rs:50`) — typed `PathBuf`, serde-serialized, present in every network constructor.
- `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` Task 7 — "Step 1: Write failing test for WalletConfig (per F15 sidecar pattern)" with `/// Path to the SQLite database file. Per F15 sidecar pattern.`
- `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` Task 9 — "F14, F15" both listed in scope.

**Note on F-numbering used in this ADR.** The project's F1–F53 numbers are *plan-review findings* defined in `docs/superpowers/reviews/2026-08-05-rust-bitcoin-wallet.md`, not closed categories. The actual mapping (verified 2026-08-11):

- **F14** = "Task 32 uses BDK 1.x `Wallet::new_single`" (BDK API migration, applied) — **not** persistence atomicity.
- **F15** = "Task 17 `load_wallet` calls undefined `detect_network_from_dir`" (the `network.txt` sidecar convention, applied) — **not** the F15 wording in this ADR's draft.
- **F19** = "`atomic_write` never called at mnemonic write site" (applied) — **this is the real "persistence atomicity" F-number.**
- **F49** = "Wallet create echoes mnemonic to STDOUT" (applied — fixed via `atomic_write` + `--show-mnemonic`).

This ADR uses F19 (not F14) for persistence atomicity. It also redefines the "sidecar" convention from F15 (a `network.txt` file) to a directory-name-as-network layout — see Cross-references for the cleanup PR that will update the plan wording.

So the layout is partially specced but never implemented. This ADR closes the design question so #54d can land.

## Decision

**Path B — keep F14 deferred. Persist only `MnemonicCipherBlob`. Re-sync from Esplora on `btc wallet show`.**

### Filesystem layout

```text
$XDG_DATA_HOME/btc/wallets/<network>/<wallet_id>.enc
└─ defaults: ~/.local/share/btc/wallets/<network>/<wallet_id>.enc
   (macOS: ~/Library/Application Support/btc/wallets/<network>/<wallet_id>.enc)
   (no XDG on Windows in v0.1 — Windows support deferred; print clear error)
```

| Component | Value | Rationale |
|---|---|---|
| Root | `$XDG_DATA_HOME/btc/wallets/` | XDG-compliant (`~/.local/share` on Linux). Avoids `~`-root clutter. Distinct from `XDG_CONFIG_HOME` (config) and `XDG_CACHE_HOME` (transient). |
| `<network>` | `testnet` / `mainnet` / `regtest` / `signet` | Cross-network isolation is **operator-driven**: `wallet_path` derives from the CLI `--network` flag. Add a #54d test asserting `wallet_path(testnet, id)` and `wallet_path(mainnet, id)` are disjoint for identical `<wallet_id>`. Defense-in-depth: blob format is network-agnostic, so a CLI bug (wrong `--network`) yields a "no wallet found" error, not silent cross-network load. |
| `<wallet_id>` | UUID v4 (36 chars, hyphenated, lowercase) — generated via `Uuid::new_v4()` | No PII (random). Globally unique. Easy to copy-paste from CLI output. Easy to grep in bug reports. `v4` enforced by `WalletId::new()` constructor (no `From<Uuid>` public impl). |
| `.enc` | extension only — no MIME registration | Self-documenting; signals "encrypted, don't grep me." |
| Format | `MnemonicCipherBlob` bytes verbatim (per #28): `salt(16) \|\| nonce(12) \|\| ciphertext \|\| tag(16)` plus **AES-GCM AAD = `bitcoin::Network` discriminant bytes** (one-byte tag: `0x01` testnet / `0x02` mainnet / `0x03` regtest / `0x04` signet) | Self-contained (no out-of-band salt). AAD binding closes the cross-network footgun (N5): copying a testnet blob into the mainnet dir fails AEAD verification at decrypt time, not silently. Length: variable (~44–150 bytes for BIP-39 phrases 12–24 words); AES-GCM authenticates length, so truncation detected at decrypt. |
| KDF | Argon2id m=256 MiB, t=10, p=4 (Sparrow reference, ~500ms wall-clock target) | Per F5. Pinned in `crypto/argon2.rs` with L20 compile-time witnesses. Defends A1 (offline cracker). |
| Permissions | Files `0o600`; newly-created parent dirs `0o700` via explicit `set_permissions` after `create_dir_all` | Reuses `atomic_write` from #23 (defends U6, U7). Explicit `set_permissions` + `refuse_world_writable` walk closes the umask-leak gap (the umask-masked `mkdir` mode can land at `0o755` if not explicitly re-set). |
| Trust boundary | The wallet directory (`<network>/`) — NOT `$XDG_DATA_HOME` itself | `~/.local/share` is conventionally `0o755`; an operator's other local users can list wallet filenames. UUIDs are non-PII (per F34); the ciphertext is opaque (per F6); wallet-existence is not considered sensitive at A1's threat level. The boundary is documented to prevent implementers from adding a useless `refuse_world_writable` on `~/.local/share` (would always fail on legitimate installs). |
| Atomic flush | `tempfile::NamedTempFile` in same dir → `rename` over target + parent dir `fsync` | Crash safety: never observe a partial blob. Standard pattern. Already used by `atomic_write`. |
| Read-path symlink defense | `std::fs::symlink_metadata` before `std::fs::read` — refuse if symlink | The write path is already defended by `atomic_write` (per #23). The read path needs the same check: a symlink at `<id>.enc` pointing elsewhere would cause `read` to load attacker-controlled bytes, fail AEAD verification, and surface as "wrong password" — confidentiality-preserving but DoS + operator confusion. |
| Residual filesystem attacks (out of scope at file level) | **Hardlink attack** — attacker (A2) who has write access to the parent dir creates a hardlink at `<network>/<victim-uuid>.enc` pointing to the same inode as a file they own. POSIX has no "no hardlinks" open flag. **Rename attack during read** — between `stat` and `open`, attacker (A2) renames the target to `*.bak` and replaces with tampered file. **TOCTOU on parent dir** — between `create_dir_all` and `atomic_write`, attacker replaces `<network>/` with a symlink. | These attacks require A2 (write access to the parent dir). A2 is mitigated by `0o700` parent (renames/hardlinks require write on parent, which A2 already has at threat-model level). The ciphertext is encrypted (F6) so file-level tampering only triggers an integrity failure → surfaces as `Error::WalletStore` and does NOT leak the mnemonic. We accept these residual risks at the file level; defense-in-depth via `O_NOFOLLOW` on read is the only filesystem-layer mitigation we apply. |

### Path resolution

```rust
fn wallet_path(network: Network, id: WalletId) -> Result<PathBuf, Error> {
    let mut p = data_dir()?;
    p.push("btc");
    p.push("wallets");
    p.push(network.as_str());
    p.push(format!("{id}.enc"));
    Ok(p)
}
```

`data_dir()` returns `Err(Error::WalletStore("wallet store not supported on this OS in v0.1"))` on Windows in v0.1. **#54d must verify on macOS** (per L28 Gate B) that the chosen `data_dir()` crate returns a path beginning with `btc/` after `data_dir()` — if the crate vendor-prefixes (`directories` crate uses `ProjectDirs::from("bt", "btc", "btc")`), the layout must absorb that prefix or the wallet will land in an unexpected location.

### Wallet ID

`WalletId(Uuid)` newtype around `uuid::Uuid`. Why newtype (type-system defense-in-depth, parallel to `MnemonicCipherBlob` newtype in PR #28):

- `pub fn new() -> Self` calls `Uuid::new_v4()` internally. No public `From<Uuid>` constructor — only v4 (random) IDs reach `WalletId`.
- Cannot pass a `String` (or arbitrary `Uuid`) where a `WalletId` is expected.
- `Display` uppercases + hyphenates consistently (no locale-dependent formatting).
- `FromStr` parses with strict format; rejects nil UUID (`00000000-0000-0000-0000-000000000000`) and any version ≠ v4 at compile time via `const _: () = { assert!(<v4 discriminant>) }` (L20-style compile-time witness).
- `Serialize` / `Deserialize` round-trips through serde.

### `btc wallet create` flow

1. Generate mnemonic (`Mnemonic::generate(words)`).
2. Prompt for password via `--password <pwd>` flag or `rpassword` secure prompt.
3. Compute `aad = [network_discriminant(network)]` — one byte binding `bitcoin::Network` to the ciphertext.
4. Encrypt mnemonic → `MnemonicCipherBlob` via `crypto::encrypt_mnemonic(phrase, password, aad)` (the `MnemonicCipherBlob` API gains an `aad: &[u8]` parameter as part of this ADR's implementation; see Cross-references for the precursor PR).
5. Generate `WalletId::new()` (random UUID v4).
6. `wallet_path(network, id)` → ensure parent dir chain exists (`create_dir_all`), then for every newly-created dir in the chain call `set_permissions(0o700)` then `refuse_world_writable` walk (defends against umask-leak per F19; defends against create_dir_all race window).
7. Write blob via `atomic_write` (tempfile + rename + `0o600`).
8. Print mnemonic **to STDERR** (not stdout, per L28 stdout-is-clean rule) with a `WARNING: shown ONCE — record now` banner. This is a UX cliff: a v0.1 demo that creates a wallet the operator cannot recover from violates L28 "verify before claim." The mnemonic-on-stderr pattern is required. Add regression test: `tests::create_writes_mnemonic_to_stderr_not_stdout` must pass.
9. Print `wallet_id` to STDOUT (random UUID, no PII, no crypto material — safe to surface).
10. Exit 0.

### `btc wallet show <id>` flow

1. Parse `<id>` → `WalletId`.
2. Resolve `wallet_path(network, id)` (network from `--network` CLI flag; cross-check at step 5 via AAD).
3. `std::fs::symlink_metadata(path)` — refuse if symlink (closes symlink-DoS on read).
4. Read blob file → `MnemonicCipherBlob::try_from(&bytes)`.
5. Compute `expected_aad = [network_discriminant(network)]`. Decrypt via `crypto::decrypt_mnemonic(blob, password, &expected_aad)` (#28 + AAD extension). If AEAD tag verification fails (wrong network embedded, wrong password, or tamper), return `Error::WalletStore("wallet not accessible (wrong password, wrong network, or corrupt blob)")` — a single indistinguishable message (oracle-attack mitigation; collapses file-existence, wrong-password, wrong-network, and corrupt-blob into one error shape, closes the file-existence oracle per N2).
6. If the wallet does not exist at the path, perform a dummy `argon2::derive_key(dummy_password, dummy_salt)` (~500ms) before returning the error, so wall-clock timing matches the wrong-password path (constant-time padding per N8).
7. `Wallet::from_mnemonic(&mnemonic, network)` (#19a, merged).
8. `wallet.sync(&esplora_client).await?` to populate UTXO set (re-sync from Esplora — current `Wallet::sync` behavior).
9. `wallet.balance(&esplora_client).await?` for confirmed sat amount.
10. Print receive addresses (external chain, gap limit 5) + confirmed balance.
11. Exit 0.

**Failure modes** (per L28 honesty — no silent fallbacks):

| Condition | Behavior |
|---|---|
| `data_dir()` returns `UnsupportedPlatform` (Windows in v0.1) | `Error::WalletStore("wallet store not supported on this OS in v0.1")` — clear, no fallback to `~/btc/wallets`. |
| `wallet_path` parent dir unwritable or `refuse_world_writable` fails | `Error::WalletStore("cannot create secure wallet dir: {path}: {io_error}")`. |
| `symlink_metadata` reveals symlink at blob path | `Error::WalletStore("wallet blob is a symlink — refusing to follow (security check)")`. |
| Wallet blob not found (after constant-time padding) | `Error::WalletStore("wallet not accessible (wrong password, wrong network, or corrupt blob)")` — indistinguishable from decrypt failure (closes file-existence oracle per N2). |
| Wallet blob present but AEAD verify fails (wrong password, wrong network AAD, or tamper) | `Error::WalletStore("wallet not accessible (wrong password, wrong network, or corrupt blob)")` — same message as above. |
| Esplora unreachable during `sync` | `Error::Esplora` (existing variant) — propagates. Note: this is a **T4** (malicious/down Esplora) availability failure with no offline fallback until F14 un-defer lands. |
| Timing oracle on missing-file path | Closed by step 6 above (dummy Argon2id run pads missing-file latency to match wrong-password latency). |

## Alternatives considered

### A — Un-defer F14: add `bdk_file_store` + atomic-flush

| Aspect | Notes |
|---|---|
| Dep added | `bdk_file_store` + `rusqlite` (or `sqlite` bundled). ~50-100 KB compiled. New build-script dep chain. |
| Schema | bdk-internal SQLite snapshot (versioned, opaque). Audit surface = SQLite binary format + bdk's encryption-at-rest design. |
| Atomicity | bdk_file_store handles atomic flush internally. We audit it, don't reimplement. |
| Boot time | Instant — no Esplora sync needed for `btc wallet show` (snapshot already has UTXO set). |
| Offline viewing | Yes — `btc wallet show` works without network (assuming encrypted snapshot). |
| Risk | bdk_file_store API is **not stable across bdk major versions** (churn risk per bdk 1.x → 2.x → 3.x history). Audit cost = significant (SQLite + encryption-at-rest). |
| Scope | Doubles #54d implementation cost (mnemonic blob + bdk snapshot). |

**Rejected for v0.1** because: (1) MVP demo doesn't need offline viewing — fresh-wallet re-sync is fast; (2) audit cost is high for an MVP demo; (3) defers to v0.1.1 when we have UX evidence that offline viewing matters.

### C — Hybrid: persist MnemonicCipherBlob + small metadata file (no bdk_file_store)

Per-wallet companion `<id>.meta.json` containing `{ network, created_at, label? }`. Rationale: operator might want to label a wallet (`"hot"`, `"cold"`, `"test"`).

**Rejected** because: (1) labels are PII-adjacent (operator habit leaks); (2) network is already encoded in the directory path + AAD; (3) `created_at` provides no recovery value. If operators ask for labels later, add as v0.1.1 with explicit consent.

### D — `sled` / `redb` / `rusqlite` (custom)

Roll our own encrypted KV store or SQLite wrapper instead of `bdk_file_store`.

**Rejected** because: (1) re-implements what bdk_file_store already provides; (2) increases audit surface (we own the format); (3) no clear advantage over Path A.

### E — Pure in-memory-only for v0.1 (no on-disk persistence)

Operator re-enters the mnemonic every `btc wallet show` invocation. No blob persistence. No disk-resident secret material.

**Rejected** because: (1) defeats the goal of #54d (CLI demo of `wallet create`/`show`); (2) breaks L28-style usability — operator must re-enter mnemonic every session; (3) leaves no recovery story when the demo's mnemonic copy-paste is lost. Path B persists ciphertext only; E persists nothing.

## Consequences

### Positive

- `#54d` implementation cost ≈ 1 PR (path resolution + create + show + tests) plus a small precursor PR updating `MnemonicCipherBlob` to accept `aad`. No new heavy deps. No new crypto primitives. No new audit lane.
- Reuses battle-tested primitives from #23 (`atomic_write` + 0o600) and #28 (`MnemonicCipherBlob`).
- Network discriminant binding via AAD closes the cross-network footgun (N5): copying a testnet blob into a mainnet directory fails AEAD verification, not silent cross-network load.
- Symlink-attack defense (`symlink_metadata` on read + `O_NOFOLLOW` via `atomic_write` on write) closes a class of TOCTOU bugs around the wallet directory.
- Wallet IDs are random UUID v4 (no PII leakage through the filesystem layer; v4 enforced by `WalletId::new()` constructor + compile-time version witness).
- Indistinguishable error messages (file-existence vs wrong-password vs wrong-network vs corrupt-blob) close the file-existence oracle (N2).
- Constant-time padding on missing-file path closes the timing oracle (N8).

### Negative

- `btc wallet show` requires live Esplora access. Offline viewing deferred to v0.1.1 (when/if Path A is un-deferred).
- Fresh-wallet re-sync on every `btc wallet show` is fast for empty wallets (5 addresses × 2 chains = 10 Esplora calls) but slow for wallets with deep history (no UTXO snapshot = full re-scan of every address in the gap limit). v0.1 demo target is fresh wallets, so acceptable.
- **T4 (malicious/down Esplora)** = availability DoS on `btc wallet show` with no offline fallback until Path A ships. Operator's threat model must accept this.
- **A4 (malicious Esplora operator)** can correlate repeated `btc wallet show` invocations with the wallet's address set, since UTXO scan re-occurs every show call. v0.1.1 Path A un-defer (UTXO snapshot persistence) eliminates this; until then, the operator's threat model must accept the correlation.
- Windows support deferred. Operators on Windows must wait or run WSL. Documented as a known limitation.
- No atomic durability across `create + show` of the ID: if a wallet is created and immediately crashes before the operator sees the wallet ID on stdout, the encrypted blob is on disk but the operator never recorded the ID. Mitigated by the operator copying the ID from stdout before the shell prompt returns.

### Threat-model coverage

This decision affects:

- **F19** (persistence atomicity via `atomic_write`) — partially defended: the encrypted blob is atomically written, but UTXO state is not persisted. Remains deferred per F19's original wording scope.
- **F15** (`network.txt` sidecar convention) — **redefined**: the original F15 assumed a sidecar file. This ADR replaces F15's "sidecar" semantics with a directory-name-as-network convention. The plan wording should be updated in a separate cleanup PR (no change to F-number assignment, just to the plan's prose).
- **F49** (mnemonic echoes to STDOUT) — addressed by printing mnemonic to **STDERR** with banner, not STDOUT (closes the F49 echo leak for the create flow).
- **U6** (file-permission leak) — defended by `0o600` files + `0o700` newly-created parent dirs (explicit `set_permissions` after `create_dir_all`).
- **U7** (directory traversal) — defended by XDG path + UUID (no `..` injection possible — operator-supplied path components are parsed, not concatenated).
- **A1** (offline cracker) — defended by Argon2id m=256 MiB / t=10 / p=4 (~500ms per attempt; per F5).
- **A2** (local user with write access to data directory) — defended by `0o700` parent dir + `refuse_world_writable` walk; residual risks (hardlink, rename, parent-dir TOCTOU) require A2 write access on the parent dir, which A2 already has at threat-model level; ciphertext is encrypted so file-level tampering surfaces as `Error::WalletStore` and does NOT leak the mnemonic.
- **T4** (malicious/down Esplora) — availability DoS on `btc wallet show` until Path A ships (see Negative consequences).

### Out of scope (deferred to follow-up PRs)

- UTXO snapshot persistence (F14 un-defer) — defer to v0.1.1.
- Windows path resolution — defer to separate backlog.
- Wallet labels / metadata files — defer to v0.1.1.
- `btc wallet export` / `btc wallet import` — defer to v0.1.1.
- Hardware-wallet integration — already deferred per `chain-traits/` umbrella (v0.2).
- **`MnemonicCipherBlob` version field** (defends silent crypto downgrade if KDF/AEAD params change) — defer to a precursor PR that updates `MnemonicCipherBlob` to prepend a `version: u8 = 0x01` field. Old blobs without the version field fail with a clear migration error.
- **`MnemonicCipherBlob` manual `Debug` impl** (the current `#[derive(Debug)]` leaks raw bytes via `tracing::debug!(?blob)` patterns; same pattern as `Secret<T>` redaction) — defer to the same precursor PR as the version field.
- Wallet ID print timing: UUID is random; no PII or crypto material; A8 has memory access and wins regardless. Not a real side channel.

## Cross-references

- **Code to update** (this PR — design only, no code changes yet):
  - `wallet/mod.rs:60-63` — F14 deferral comment: Point at this ADR instead of "v0.1.1". Deferral is still in effect, but rationale is now documented.
  - `config.rs:50` — `db_path` field: **Deprecate** in v0.1 (`#[deprecated(since = "0.1.1", note = "use --wallet-dir CLI flag or $XDG_DATA_HOME/btc/wallets/")]`); remove in v0.2. Operator override via `wallet_dir: Option<PathBuf>` is a v0.2 concern (separate ADR if operator demand surfaces post-v0.1 demo UX data).
  - `error.rs` — add new variant `WalletStore(String)` per F43 pattern (per-protocol variant; distinct from `Storage` for caller UX). `Storage("disk full")` is generic IO; `WalletStore` is wallet-persistence-specific.
- **Code to update** (precursor PR — touches #28's `MnemonicCipherBlob` module):
  - `crypto/mnemonic_cipher.rs` — extend `encrypt_mnemonic` / `decrypt_mnemonic` signatures to accept `aad: &[u8]`. Pass AAD through to `aes_gcm::Aes256Gcm::encrypt_in_place_detached` / `decrypt_in_place_detached`. Reuse the AAD parameter for the version-field work (deferred follow-up PR).
  - `crypto/mnemonic_cipher.rs` — replace `#[derive(Debug)]` with manual `impl Debug` that returns `"MnemonicCipherBlob(<redacted: {} bytes>)"`.
- **Code to update** (separate cleanup PR after #60 lands):
  - `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` Task 7 + Task 9 — update F15 sidecar wording to match the new path-based layout.
  - `docs/superpowers/reviews/2026-08-05-rust-bitcoin-wallet.md` — F15's prose can stay as-is (it documents the *original* finding); the new convention is captured in this ADR.
- **Issues**:
  - [#60](https://github.com/nhitranbtc/blockchain-sdk/issues/60) — closes when this ADR merges.
  - [#64](https://github.com/nhitranbtc/blockchain-sdk/issues/64) — unblocked when this ADR merges (acceptance criterion: "#54-precursor ADR merged first").
  - [#54](https://github.com/nhitranbtc/blockchain-sdk/issues/54) — umbrella; flips F14 row in the dependency graph when #60 merges.
- **Lessons** (active in `tasks/lessons.md`, not retired):
  - **L24** — doc updates travel with feature PR. This ADR IS the doc for #54d's persistence design; #54d PRs reference back to it.
  - **L28** — verify-before-claim. The `btc wallet show` failure modes table above enumerates every silent-fallback risk; each surfaces a clear error.
  - L34 is retired per the SoT audit (CLAUDE.md: "Gaps L2-L5, L7, L10, L15-L20, L22-L23, L25-L27, L29-L34 retired per audit"). The newtype rationale here follows the same pattern as `MnemonicCipherBlob` newtype (PR #28) — type-system defense-in-depth at the persistence boundary.

## Acceptance

### ADR-merge acceptance (this PR)

- [ ] ADR merged to `main` (this file under `docs/superpowers/adrs/`)
- [ ] `wallet/mod.rs:60-63` comment updated to point at ADR 0001
- [ ] `config.rs:50` `db_path` field marked `#[deprecated]` with the note above
- [ ] `error.rs` gains `WalletStore(String)` variant per F43 pattern
- [ ] No public API change yet — CHANGELOG `[Unreleased]` entry under `### Security` with bold inline code tag **`[internal]`** (no public API change yet — design only)
- [ ] L9 v3 verdict filled at merge: Correctness / Security / Test coverage / Code simplicity

### Implementation-merge acceptance (#54d, separate PR)

- [ ] Precursor PR: `crypto/mnemonic_cipher.rs` accepts `aad: &[u8]` + manual `Debug` impl (lands before #54d)
- [ ] `#54d` PR opens only after ADR-merge acceptance complete
- [ ] `btc wallet create --help` + `btc wallet show --help` documented
- [ ] Wallet create persists encrypted blob per ADR layout with AAD-bound Network
- [ ] Wallet show roundtrips: create → show → addresses + balance match
- [ ] L28 fix: mnemonic appears on STDERR (with banner), NOT stdout — regression test `tests::create_writes_mnemonic_to_stderr_not_stdout` enforced
- [ ] F19 path implemented per ADR (atomic_write + set_permissions(0o700) walk + refuse_world_writable)
- [ ] `symlink_metadata` check on read path (closes symlink-DoS)
- [ ] Constant-time padding on missing-file path (closes timing oracle)
- [ ] Tests: unit (no-stdout mnemonic, wallet store roundtrip, AAD network mismatch rejection), live testnet #[`ignore`]
- [ ] CHANGELOG `[Unreleased]` `### Added` entry for `btc wallet create` / `btc wallet show` subcommands
- [ ] CHANGELOG User Stories table: Story #13 ("Use btc CLI subcommand") flipped to `[x]` (Story #10 "Create wallet from mnemonic" already `[x]` from #48; Story #13's CLI aspect now lands)
- [ ] README "What's New" one-liner updated
- [ ] estimate-report.md Plan-progress row updated per L21
- [ ] macOS data_dir() verification per L28 Gate B (test on macOS before merge)
- [ ] L29 manual smoke before merge
- [ ] L9 v3 verdict at merge

## L9 v3 verdict (TBD at merge)

Per L9 v3 schema (per-dimension PASS / PARTIAL / FAIL rubric; from `tasks/lessons.md`):

- **Correctness**: TBD
- **Security**: TBD
- **Test coverage**: TBD (this ADR has no tests — design only; #54d adds tests)
- **Code simplicity**: TBD
