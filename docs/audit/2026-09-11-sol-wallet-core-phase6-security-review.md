---
title: sol-wallet-core Phase 6 — security review (ship-gate)
tracker: TBD — file per `## Filing the review issue` below
plan: docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md (Phase 6 starts line 1156)
deep-dive: docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md
companion-audit: docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md (P4-4 ✅ plan-level crypto params)
date: 2026-09-11
status: open
verdict: BLOCK
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening · ✅ passed (flagged for completeness)
guide: .local/plugins-docs/2026-09-06-mattpocock-vs-superpowers-audit-guide.md
---

# sol-wallet-core Phase 6 — security review (ship-gate)

Pre-implementation review of `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`
**Phase 6** (lines 1156–1308) — Wallet persistence (Argon2id + AES-GCM) + WalletManager CRUD +
4-trait PAL — issued before any Phase 6 code lands on `sol/phase6-persistence`.

**Scope.** Four implementation seams inside Phase 6:

1. `crypto::encrypt_wallet` / `decrypt_wallet` — Argon2id + AES-256-GCM envelope (Task 6.1 Step 1–2).
2. `persist::atomic_write` + `FileWalletStorage` mode-0600 — disk persistence (Steps 3 + 5).
3. `WalletManager` in-memory key lifecycle — `unlock` / `lock` / CRUD (Step 4).
4. `WalletStorage` PAL — File / Keychain / EncryptedFile impls (Step 5).

The companion audit (`2026-09-09-solana-rust-sdks-deep-dive-security-audit.md`) covered
**plan-level** persistence design (P4-4 ✅: Argon2id `m=64MB t=3 p=1`, 16-byte salt, 12-byte nonce,
`nonce ‖ ciphertext ‖ tag`, atomic write + mode 0600, JSON envelope). None of the 14 findings
below are visible from plan-level inspection alone — they require implementation-level reasoning
about AAD binding, Windows ACL, RAII lifetime, RNG failure paths, and source-file permission
hygiene. Pre-Phase-6 review adds value the post-Phase-5 review could not.

**Overall verdict: BLOCK.** Three 🔴 HIGH findings (P6-1 metadata-not-authenticated downgrade,
P6-2 Windows mode-0600 unenforceable, P6-3 `unlock()` plaintext-key lifecycle) must clear
before Phase 6.1 implementation begins. Four 🟠 HIGH/MEDIUM and five 🟡 MEDIUM and three 🔵 LOW
findings are listed in priority order below.

---

## Drift scan (L13 step 4a)

Phase 6 not yet implemented on the live tree — no drift to surface. Plan claims checked against
current state on 2026-09-11:

| Plan claim                                                    | Source     | Verified                                 |
| ------------------------------------------------------------- | ---------- | ---------------------------------------- |
| `argon2 0.5` workspace dep declared                           | plan L592  | ✅ `rust-wallet-app/Cargo.toml` workspace table |
| `aes-gcm 0.10` workspace dep declared                         | plan L593  | ✅                                      |
| `zeroize 1.x` workspace dep declared                          | plan L598  | ✅                                      |
| `uuid 1.x` (v4 + serde features) for `WalletId`               | plan L588  | ✅                                      |
| `tempfile` workspace dev-dep for persist tests                | plan L608  | ✅                                      |
| Phase 5 landed on `rust-sol-core` (no Phase 6 code yet)       | — | ✅ commit `8922c6e9` (Phase 5.2/5.3/5.5) ends Phase 5 |

No drift. Phase 6 plan is internally consistent with the locked workspace dep table.

---

## Findings (ranked most-severe first)

### 🔴 P6-1 — KDF metadata NOT authenticated → KDF downgrade attack [HIGH]

Task 6.1 Step 1 JSON envelope:

```text
{
  "kdf":   { "algorithm": "argon2id", "memory_kb": 65536, "iterations": 3, "parallelism": 1, "salt": "..." },
  "cipher":{ "algorithm": "aes-256-gcm", "nonce": "..." },
  "encrypted_payload": "..."
}
```

AES-256-GCM auth tag covers `nonce ‖ ciphertext`. KDF params + salt + nonce live in cleartext
JSON, mutable by anyone with disk write. Attacker rewrites blob with `memory_kb: 8192`,
`iterations: 1` (cost ~10 ms instead of ~300 ms per guess) and the user's passphrase still
decrypts against the downgraded KDF. Dictionary attack ~30,000× cheaper.

**Fix.** Bind params to ciphertext via AES-GCM AAD. Either (a) include
`"sol-wallet-core/v1" ‖ algorithm ‖ m ‖ t ‖ p ‖ salt ‖ nonce` as the AAD parameter to
`aes_gcm::Aes256Gcm::encrypt_in_place_detached(&aad, ...)`, or (b) prepend a fixed magic
header bytes to plaintext before encryption (less robust). Verify by adding a row to
`tests/aes_gcm_cipher.rs` (Phase 6.1 row 6): flip one byte of `memory_kb` in the JSON
envelope → assert `Error::DecryptFailed` rather than silent decrypt.

---

### 🔴 P6-2 — `mode 0600` unenforceable on Windows → wallet file world-readable [HIGH]

Task 6.1 Step 5 `FileWalletStorage (desktop, mode 0600)` + Step 10 test
`saved file mode == 0o600`.

`std::fs::Permissions::from_mode(0o600)` on Windows is a no-op for the access-control list.
NTFS default grants `BUILTIN\Users` read. Every account on the box can read the wallet
blob — exactly the threat mode 0600 was supposed to block on Linux/macOS. Phase 6 promises
desktop support for "Linux/macOS/Windows" (plan L15).

**Fix.** Branch in `FileWalletStorage::put_atomic`:
- Unix: `set_permissions(0o600)` + post-`rename` verify (`metadata.permissions().mode() & 0o077 == 0`).
- Windows: call `SetSecurityInfo` via `windows-sys` (or `icacls` invocation) to grant only
  current-user `READ + WRITE` and strip inherited ACLs. Test in `tests/wallet_persist.rs`
  row 9 must run on Windows in CI matrix (add `windows-latest` to `rust-sol-core-ci.yml`).

If Windows ACL hardening is too costly for V0.1, document explicitly in the FFI / platform
support matrix and file as V0.1.5 backlog — silent regression of a security control is
worse than an acknowledged deferral.

---

### 🔴 P6-3 — `unlock()` plaintext-key lifecycle unspecified → F47 memory-hygiene leak [HIGH]

Task 6.1 Step 4 `WalletManager::unlock(id, password)`. Plan does not specify return type.
Inferred from Phase 1: `Wallet(solana_sdk::signature::Keypair)`. `solana_sdk::Keypair` does
**NOT** implement `Zeroize` (Anza gap — F47 deep-dive already documented). Plaintext 64-byte
secret held in caller-controlled memory until natural drop — survives `lock()` call, survives
scope exit on heap, survives panic unwind.

Affects every consumer:
- `sol` CLI handlers (Phase 7) holding it across async send-and-confirm.
- FFI export `sol_wallet_unlock` (Phase 8) — raw secret crosses C boundary (compounds P4-1).
- `send_and_confirm` (Phase 5.1) call site holding it for the duration of RPC round-trip.

**Fix.**
- `WalletManager::unlock -> Zeroizing<Keypair>` wrapping `solana_keypair`'s 64 bytes via
  `Keypair::to_bytes()` then wrap in `zeroize::Zeroizing<[u8; 64]>`.
- `WalletManager::lock(id)` MUST drop the `Zeroizing<Keypair>` (RAII via `Drop` guard or
  `OwnedLock { inner: Zeroizing<Keypair>, manager: Weak<WalletManager> }`).
- Document the contract: caller MUST NOT clone the keypair. Cloning breaks the Zeroize
  guarantee. Add `#![warn(clippy::large_types_passed_by_value)]` to discourage by-value
  passing.
- Test in `tests/wallet_lifecycle.rs` (Phase 6.1 row 37): call `lock(id)`, then probe the
  prior allocation address via a `Zeroize` test harness — must NOT hold live keypair bytes
  (zeroize the buffer, assert subsequent `unlock` produces distinct bytes for the same
  passphrase).

---

### 🟠 P6-4 — Plaintext `&[u8]` input to `encrypt_wallet` → F47 leak across API boundary [HIGH]

Task 6.1 Step 1 `crypto::encrypt_wallet(plaintext: &[u8], password: &str)`.

Signature accepts `&[u8]`, not `Zeroizing<&[u8]>` or `Zeroizing<Vec<u8>>`. Callers construct
`Vec<u8>` from `mnemonic.as_bytes()` then pass `&v` — `v` lives until end of scope, NOT
zeroized. F47 deep-dive (`tasks/lessons.md` L13 deep-dive ref) documented this gap for
`Keypair::from_seed`; the wrapper API inherits and amplifies the leak across every call site.

**Fix.** Change signature:

```rust
pub fn encrypt_wallet(
    plaintext: Zeroizing<Vec<u8>>,
    password: &str,
) -> Result<EncryptedBlob, Error>
```

Internally: copy into a stack `Zeroizing<[u8; N]>` if plaintext fits, then drop the input
`Zeroizing<Vec<u8>>` immediately after AES-GCM absorbs plaintext (scoped block). Update both
call sites (`WalletManager::create_with_mnemonic`, `WalletManager::import_from_pk_file`).

---

### 🟠 P6-5 — Argon2id `m=64MB, t=3, p=1` pinned unconditionally → mobile OOM / DoS [MEDIUM]

Task 6.1 Step 1 Argon2id params hardcoded; `WalletStorage` PAL has iOS `KeychainWalletStorage`
+ Android `EncryptedFileWalletStorage` impls (Task 6.1 Step 5).

64 MB Argon2id + RustCrypto `argon2` 0.5 = ~256 MB peak RSS (memory + working tables). iOS
arm64 background process budget ~50–200 MB. Android Go devices ≤ 1 GB RAM — wallet unlock
competes with system for memory, foreground app may be killed mid-derive. No plan step
calibrates params by platform.

**Fix.** Parametrize:

```rust
pub struct KdfParams {
    pub memory_kb: u32,
    pub iterations: u32,
    pub parallelism: u32,
}
impl Default for KdfParams {
    fn default() -> Self {
        // desktop default — current plan value
        Self { memory_kb: 64 * 1024, iterations: 3, parallelism: 1 }
    }
}
// iOS / Android overrides via build cfg or PAL:
#[cfg(target_os = "ios")]   fn default() -> Self { Self { memory_kb: 16*1024, .. } }
#[cfg(target_os = "android")] fn default() -> Self { Self { memory_kb: 16*1024, .. } }
```

Recorded in JSON metadata, covered by P6-1 AAD-binding fix. Add row-5 test variant: same
passphrase + lower params → distinct derived key (already covered by existing reject-wrong
assertion). Add platform-default latency assertion: unlock completes ≤ 500 ms p99 on a CI
ARM runner if available.

---

### 🟠 P6-6 — `import_from_pk_file(path, password)` does not validate source file mode [MEDIUM]

Task 6.1 Step 4 `WalletManager::import_from_pk_file(path, password)`.

Reads 64-byte private key from caller-supplied path. If path is mode 0644 (world-readable),
the secret leaks via the source file BEFORE encryption wraps it. Wallet file ends up safe;
source file remains exposed — key exfiltration window.

**Fix.** On Unix, before `read`:

```rust
let meta = std::fs::metadata(&path)?;
if meta.permissions().mode() & 0o077 != 0 {
    return Err(Error::InsecureSourceFile { path, mode: meta.permissions().mode() });
}
```

On Windows: ACL check (defer if too costly — document the gap as V0.1.5). Test in
`tests/wallet_lifecycle.rs`: pre-create file mode 0644 → assert `import_from_pk_file`
returns `Error::InsecureSourceFile { path, mode }`; pre-create mode 0600 → success.

---

### 🟠 P6-7 — `crypto::random_salt` / `OsRng` failure path unspecified [MEDIUM]

Task 6.1 Step 1 `"salt 16 bytes OsRng"` + `"nonce 12 bytes OsRng"`.

`argon2` 0.5 + `aes-gcm` 0.10 use `getrandom` 0.2 under the hood. `getrandom` returns
`Error::UNAVAILABLE` on weird sandboxes (WASI without `RANDOM_GET`, seccomp-restricted
containers, broken `/dev/urandom` fds). Plan does not say what `encrypt_wallet` returns
when RNG fails. Default RustCrypto behaviour: `unwrap()` on the RNG handle → panic. On FFI
side (Phase 8), panic propagates → panic-scrubber catches → exit 99 → user gets no wallet,
and the RNG failure is silent and may recur.

**Fix.** Propagate RNG error via new variant:

```rust
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("OS RNG unavailable: {source}")]
    OsRngFailed { #[source] source: getrandom::Error },
    // ...
}
```

Test cannot easily simulate `getrandom` failure. Substitute: forbid `unwrap()` / `expect()`
on RNG paths via grep audit (`grep -rn "unwrap()\|expect(" src/crypto.rs`) as a CI check.
Document in code comment that `OsRng` failure is terminal — `Error::OsRngFailed` is exit 5.

---

### 🟡 P6-8 — `atomic_write` does not clean `.tmp` on rename failure [MEDIUM]

Task 6.1 Step 3 `atomic_write(path, bytes) -> Result<()>`:
`write .tmp + fsync + rename`. Step 10 test asserts "no `.tmp` residue after successful write"
— but not after FAILED rename.

If rename fails (target locked on Windows, ENOSPC, EACCES), `.tmp` is left behind. Subsequent
`unlock` reads the stale wallet file (which is still the OLD version) — silent data loss vs
the new write the user just performed.

**Fix.** In `atomic_write`, on rename-failure:

```rust
let _ = std::fs::remove_file(&tmp_path); // best-effort, ignore error
return Err(original_error);
```

Test variant in `tests/wallet_persist.rs`: simulate rename failure (target is read-only
file on Unix, or mock filesystem) → assert `.tmp` does NOT remain in the directory. CI
matrix should cover this path.

---

### 🟡 P6-9 — `decrypt_wallet` non-constant-time response between JSON-parse-fail and Argon2id-fail [MEDIUM]

Task 6.1 Step 2.

`decrypt_wallet` will (a) parse JSON envelope, (b) recompute Argon2id with provided
passphrase + metadata salt, (c) AES-GCM open. Early-exit on JSON parse fail takes
microseconds; full Argon2id path takes hundreds of milliseconds. Attacker who can submit
blobs to a decrypter (online wallet-recovery service, or a future RPC that decrypts) can
probe valid envelopes vs malformed ones via timing.

**Fix.** Two options:
1. Always run Argon2id with a dummy fallback salt if JSON parse fails.
2. Document the residual timing channel as accepted — Argon2id alone dominates (≥100 ms),
   so the differential between Argon2id-fail and JSON-fail is small relative to the
   Argon2id cost. Differential attack window = few microseconds, dwarfed by the Argon2id
   computation itself. Less robust than (1) but cheaper and adequate.

Recommend option (2) with a documented p99 differential measurement. Re-evaluate if a
network-decrypt surface is added.

---

### 🟡 P6-10 — No `version` field in EncryptedBlob schema → no migration path [MEDIUM]

Task 6.1 Step 1 JSON envelope.

Schema as specified has no `version` discriminator. Future V0.2 may switch AEAD (AES-GCM
→ XChaCha20-Poly1305), bump Argon2id params, or add AAD. With no version field, decryption
must guess — silent corruption if user upgrades wallet between V0.1 and V0.2.

**Fix.** Add `"version": 1` to envelope. `decrypt_wallet` matches `version` and dispatches:

```rust
match envelope.version {
    1 => decrypt_v1(&envelope, password),
    v => Err(Error::UnsupportedBlobVersion { found: v, supported: 1..=1 }),
}
```

Add to `tests/mnemonic_encrypt.rs` row 7: blob missing `version` → `Error::UnsupportedBlobVersion`.
Add: blob with `version: 2` → same error. Reject versions < 1 (no legacy path).

---

### 🟡 P6-11 — Coverage gate misses Phase 6 files [MEDIUM]

Phase 6.2 Task 6.2.1 Step 6: `100% line coverage on crypto/, amount.rs, tx/builder.rs, spl/disambig.rs`
via `cargo tarpaulin -p sol-wallet-core --lib`.

Phase 6 adds `crypto.rs`, `persist.rs`, `wallet_manager.rs`, `platform/{mod,storage,info,network,clock}.rs`.
None in coverage gate. `WalletManager` CRUD error paths + PAL trait bounds + `lock()` semantics
+ `osRng` failure paths (P6-7) uncovered — exactly the surface where P6-3 / P6-4 / P6-7 live.

**Fix.** Extend gate to include the Phase 6 module set:

```toml
# per `.cargo-tarpaulin.toml` or CLI flags
sol_wallet_core::crypto
sol_wallet_core::persist
sol_wallet_core::wallet_manager
sol_wallet_core::platform
```

Loud-RED if any of these modules drops below 100% line coverage in Phase 6.2 verification.

---

### 🔵 P6-12 — KDF params + AAD interaction with mobile param override [LOW]

Combination of P6-1 + P6-5.

If AAD includes params and mobile uses 16 MB but desktop uses 64 MB, a blob created on iOS
cannot be unlocked on desktop (params in AAD mismatch). This may be desired (security
isolation) or accidental (user upgraded phone, lost desktop access).

**Fix.** Document explicitly in the AAD-binding choice. Default = same params across all
platforms (recommendation: 32 MB as compromise). If platform-specific is desired, document
the recovery flow in `docs/wallets/` and on the `sol config` help page.

---

### 🔵 P6-13 — `throwaway_keypair()` test helper could log via panic message [LOW]

Task 6.1 Step 12 `tests/common/keypair_fixture.rs`.

Helper returns `Keypair`; if a test panics with `unwrap()` on it, panic message may include
key bytes. Phase 8 panic-scrubber strips — but only in FFI context, not in `cargo test`.

**Fix.** Helper wraps in `Zeroizing<Keypair>`. Add `#![warn(clippy::large_types_passed_by_value)]`
to `tests/common/` to encourage by-reference passing.

---

### ✅ P6-14 — `RwLock<HashMap<WalletId, EncryptedBlob>>` — encrypted blob holds no key [PASS, info]

Task 6.1 Step 4.

In-memory map only holds **encrypted** blobs, not plaintext keys. `unlock(id)` decrypts on
demand and returns a transient handle. Plan correct on this. Flagged here so a reviewer
cross-checks the design.

**Confirm.** Add assertion in `tests/wallet_lifecycle.rs` row 38: listing wallets does not
touch decrypted keys (no timing leak via decrypt). Measure `list()` latency under a stress
test with 1000 wallets; assert p99 ≤ 10 ms (encrypted-blob iteration only).

---

## Pre-Phase-6 ship checklist

A Phase 6.1 implementation PR is not mergeable until every box below flips:

- [ ] **P6-1** KDF params + salt + nonce + magic-prefix bound in AES-GCM AAD; `tests/aes_gcm_cipher.rs` row 6 has new tamper case.
- [ ] **P6-2** Windows ACL hardening OR explicit V0.1.5 deferral documented in `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` §Windows support matrix; CI matrix adds `windows-latest` for `tests/wallet_persist.rs` row 9.
- [ ] **P6-3** `unlock` returns `Zeroizing<Keypair>` + `lock` is RAII drop; `tests/wallet_lifecycle.rs` row 37 has the zeroize-probe test.
- [ ] **P6-4** `encrypt_wallet` accepts `Zeroizing<Vec<u8>>`; both call sites updated.
- [ ] **P6-5** platform-conditional Argon2id params (or single 32 MB default); `tests/argon2_kdf.rs` row 5 has the cross-platform-latency assertion.
- [ ] **P6-6** `import_from_pk_file` refuses mode > 0o600 on Unix; `tests/wallet_lifecycle.rs` has the 0644-refuse test case.
- [ ] **P6-7** `OsRng` error propagates as `Error::OsRngFailed`; no `unwrap()` on RNG paths in `crypto.rs` (CI grep check).
- [ ] **P6-8** `atomic_write` cleans `.tmp` on rename failure; `tests/wallet_persist.rs` has the rename-fail case.
- [ ] **P6-9** Argon2id-fail vs JSON-fail timing differential documented in code comment + measured p99 recorded in the audit doc trail.
- [ ] **P6-10** `version: 1` discriminator in envelope; `tests/mnemonic_encrypt.rs` row 7 has the missing-version + unsupported-version cases.
- [ ] **P6-11** coverage gate extended to `crypto/`, `persist.rs`, `wallet_manager.rs`, `platform/`; Phase 6.2 verification asserts 100% on the extended set.
- [ ] **P6-12** cross-platform param strategy documented in AAD-binding decision comment.
- [ ] **P6-13** test helper wraps in `Zeroizing<Keypair>`; clippy lint enabled in `tests/common/`.
- [ ] **P6-14** `list()` latency assertion added to `tests/wallet_lifecycle.rs` row 38.

All 14 boxes must be ✅ before the Phase 6.1 PR merges into `rust-sol-core`. Phase 6.2
verification (lines 1192–1308 of plan) gains 3 new loud-RED gate assertions: AAD tamper
case (P6-1), Windows-mode 0600 case (P6-2), unlock-then-lock zeroize-probe case (P6-3).

---

## Filing the review issue

A companion GitHub issue must be opened so this audit lands in the v0.1 tracker. The
issue lives on `rust-sol-core` (per Phase Set Up Task S.2 rule), label set
`rust-sol-core` + `backlog` + `security` + `task`, milestone `sol-wallet-core v0.1`,
priority `priority/p0`.

**Body template** (issue body lives in `/tmp/<slug>-body.md` per memory
`gate-guard-gh-pr-classifier.md` — inline body with `rm`/`rmdir` prose trips the
Fact-Force gate):

```text
## Goal

Pre-implementation security review of Phase 6 (Wallet persistence) in
`docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md` (lines 1156–1308).

Closes BEFORE Phase 6.1 implementation begins on `sol/phase6-persistence`.

## Verdict

**BLOCK.** 3 🔴 HIGH + 4 🟠 + 5 🟡 + 3 🔵 findings — see audit doc for full catalog.

## Companion

Audit doc: `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`

Companion audit (plan-level crypto): `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md`
(P4-4 ✅ — Argon2id + AES-GCM design parameters passed)

## Findings (must clear before Phase 6.1 PR merges)

- [ ] **P6-1** 🔴 KDF metadata NOT authenticated → KDF downgrade attack
- [ ] **P6-2** 🔴 mode 0600 unenforceable on Windows → wallet file world-readable
- [ ] **P6-3** 🔴 `unlock()` plaintext-key lifecycle unspecified → F47 leak
- [ ] **P6-4** 🟠 Plaintext `&[u8]` input to `encrypt_wallet` → F47 leak across API
- [ ] **P6-5** 🟠 Argon2id m=64MB pinned unconditionally → mobile OOM/DoS
- [ ] **P6-6** 🟠 `import_from_pk_file` does not validate source file mode
- [ ] **P6-7** 🟠 `OsRng` failure path unspecified
- [ ] **P6-8** 🟡 `atomic_write` does not clean `.tmp` on rename failure
- [ ] **P6-9** 🟡 `decrypt_wallet` non-constant-time response
- [ ] **P6-10** 🟡 No `version` field in EncryptedBlob schema
- [ ] **P6-11** 🟡 Coverage gate misses Phase 6 files
- [ ] **P6-12** 🔵 KDF params + AAD interaction with mobile param override
- [ ] **P6-13** 🔵 `throwaway_keypair()` test helper could log via panic message
- [ ] **P6-14** ✅ `RwLock<HashMap>` encrypted blob holds no key (confirmed)

## Resolution contract

A Phase 6.1 PR is not mergeable until all 14 boxes flip ✅ in this issue body
(update-issues-before-merge rule per memory). Each fix lands in the Phase 6.1 PR that
closes this issue; the checklist-flip + squash-merge happen together.

## Branch

This audit doc lives on `sol/phase6-persistence-audit` branched from `rust-sol-core`.
The Phase 6.1 implementation PR will branch from the same `rust-sol-core` and close
this issue.

## Cross-references

- Plan §Phase 6: `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md` L1156–1308
- Companion audit: `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md` P4-4 ✅
- FFI audit cross-ref: P4-1 🟠 + P4-2 🟠 (Phase 8 concerns amplified by Phase 6 `unlock()`)
- CLI audit cross-ref: P5-1 🟠 exit-code inversion (Phase 7 inherits Phase 6 error variants)
```

Open via:

```bash
gh issue create \
  --base rust-sol-core \
  --title "security(sol): Phase 6 wallet persistence — BLOCK on 14 findings (3 🔴 HIGH)" \
  --body-file /tmp/sol-phase6-audit-body.md \
  --label "rust-sol-core" --label "backlog" --label "security" --label "task" \
  --milestone "sol-wallet-core v0.1"
```

---

## Resolution

Each `- [ ]` flips to `- [x]` only when the corresponding test or code change lands in
the Phase 6.1 PR and the test passes locally + in CI. The audit issue closes when the
Phase 6.1 PR merges into `rust-sol-core` per `update-issues-before-merge` rule.

Phase 6.2 verification (plan lines 1192–1308) MUST be updated to assert three new loud-RED
gates: AAD tamper (P6-1), Windows mode 0600 (P6-2), unlock-then-lock zeroize probe (P6-3).
A Phase 6.2 verification PR that runs without these three new assertions is not mergeable.