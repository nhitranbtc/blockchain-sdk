---
title: sol-wallet-core Phase 8 — security review (ship-gate)
tracker: #561 — filed 2026-09-11 via `gh issue create` (base inferred from cwd; labels rust-sol-core + backlog + security + task; milestone sol-wallet-core v0.1)
plan: docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md (Phase 8 starts line 1677)
deep-dive: docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md §K FFI surface (L1864–1890)
companion-audit: docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md
phase7-audit: docs/audit/2026-09-11-sol-wallet-core-phase7-security-review.md (P7-5 eprintln! mnemonic leak in STDERR reused as panic-scrubber corpus seed)
date: 2026-09-11
status: open
verdict: BLOCK
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening · ✅ passed (flagged for completeness)
guide: .local/plugins-docs/2026-09-06-mattpocock-vs-superpowers-audit-guide.md
---

# sol-wallet-core Phase 8 — security review (ship-gate)

Pre-implementation review of `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`
**Phase 8** (lines 1677–1710) — FFI cdylib surface (12 C functions) + mobile
compile gate — issued before any Phase 8 code lands on `sol/phase8-ffi`.

**Scope.** Two implementation seams inside Phase 8:

1. `src/ffi.rs` + `src/panic_scrubber.rs` — 12 `extern "C"` functions (Task 8.1 Steps 2–11), panic-message scrubber (Step 1), negative-test fuzz + ABI smoke (Steps 12–13).
2. Mobile compile gate — `cargo check` for iOS/Android targets + cbindgen header emission (Task 8.2 Steps 1–3).

The companion audit (`2026-09-09-solana-rust-sdks-deep-dive-security-audit.md`) covers
plan-level design drift. The Phase 7 audit (`2026-09-11-sol-wallet-core-phase7-security-review.md`)
flagged P7-5 (`eprintln!("{e:?}")` mnemonic leak in STDERR) — that corpus seeds the
Phase 8 panic-scrubber fuzz and is reused verbatim. Phase 6 audit (`2026-09-11-sol-wallet-core-phase6-security-review.md`)
P6-3 (plaintext-key lifecycle) and P6-4 (zeroize gap) propagate into every FFI fn that
touches `WalletManager::unlock`. None of the 20 findings below are visible from
plan-level inspection alone — they require implementation-level reasoning about FFI
buffer ownership, thread-safety on static state, panic-vs-abort profile trade-offs,
cdylib symbol leakage, and mobile ABI verification.

**Overall verdict: BLOCK.** Eight 🔴 HIGH findings (H1 buffer overflow on `*out_*`,
H2 input length missing, H3 unlock secret caller-zeroize not enforceable, H5 FFI send
lacks policy gate, H6 static error buffer thread race, H7 cdylib Rust runtime symbol
leak, H8 panic=abort vs scrubber trade-off, H17 mobile ABI link test absent) must clear
before Phase 8.1 implementation begins. Seven 🟠 MEDIUM findings, five 🔵 LOW / hardening
findings; one ✅ passed confirm for completeness.

---

## Drift scan (L13 step 4a)

Phase 8 not yet implemented on the live tree — no drift to surface. Plan claims checked
against current state on 2026-09-11:

| Plan claim                                                    | Source     | Verified                                 |
| ------------------------------------------------------------- | ---------- | ---------------------------------------- |
| `sol-wallet-core` library exists + all Phase 0–7 modules       | plan L1682 | ✅ PRs #549–#560 landed on `rust-sol-core` |
| `WalletManager` + Argon2id + AES-GCM persistence (Phase 6)   | plan L1689 | ✅ PR #558 landed commit `56a5bfe3` |
| 17-method reqwest JSON-RPC client (Phase 5)                   | plan L1693 | ✅ PR #556 landed commit `2934d040` |
| 22 CLI commands GREEN (Phase 7)                                | plan L1688 | ✅ PR #560 landed commit `3abab9c9` (21/22 rows) |
| `panic_scrubber` module is NEW in Phase 8.1                   | plan L1683 | ✅ no collision — `grep -r panic_scrubber crates/sol-wallet-core/src/` returns empty |
| `cdylib` crate-type declared in Cargo.toml                    | plan L1682 | 🔴 open — Phase 8.1 must add `[lib] crate-type = ["cdylib", "lib"]` to `Cargo.toml` |
| `cbindgen` dev-dependency declared                              | plan L1708 | 🔴 open — Phase 8.1 must add `cbindgen = "0.27"` to `[dev-dependencies]` |
| `libloading` dev-dependency declared (for in-process FFI smoke) | plan L1699 | 🔴 open — Phase 8.1 must add `libloading = "0.8"` to `[dev-dependencies]` |
| iOS target installed (`aarch64-apple-ios`)                     | plan L1706 | 🔴 open — operator must `rustup target add aarch64-apple-ios aarch64-linux-android` |
| `once_cell` already a dep                                      | plan L1687 | ✅ confirmed via `Cargo.toml` (Phase 6 dep) |
| `Zeroizing` already a dep                                      | plan L1687 | ✅ confirmed via `Cargo.toml` (Phase 6 dep) |
| `proptest` already a dev-dep                                   | plan L1698 | ✅ confirmed via `Cargo.toml` (Phase 7 dev-dep) |

No drift. Plan is internally consistent but **8 Phase 8 audit findings will land in
`crates/sol-wallet-core/src/{ffi,panic_scrubber}.rs`** if executed as written.

---

## Findings (ranked most-severe first)

### 🔴 H1 — Output buffer overflow on `*out_*` params [HIGH]

Plan Task 8.1 Steps 2–10 specify 12 `extern "C"` fns with `*out_id`, `*out_mnemonic`,
`*out_secret_bytes`, `*out_pubkey`, `*out_signature`, `*out_sig`, `*out_balance`,
`*out_decimals` — **none carry a `buf_len` parameter**. `sol_wallet_create_mnemonic`
writes a 24-word mnemonic (~180 B including spaces) to caller-supplied buffer; if the
mobile consumer allocates a 16-byte buffer (e.g. reuses a keypair buffer by mistake),
Rust writes past the end → heap corruption. The corruption is attacker-influenced if
the surrounding process exposes FFI to untrusted code (plugin host, embedded web view,
third-party FFI consumer in the same process).

**Fix.** Every `*out_*` param paired with explicit `out_len: *mut usize` (in/out: caller
sets capacity, library sets required size) and a `buf_len: usize` (caller capacity).
Return `BufTooSmall` from FFI error enum when buffer too small; required size reported
via `sol_wallet_last_error_message`. Test row: oversized / undersized / exact-fit buffer
matrix for each of the 8 `*out_*` params.

### 🔴 H2 — Input buffer length missing on `*in_*` params [HIGH]

Plan Steps 3, 4, 7, 8, 9, 10 specify `*in_mnemonic`, `*in_message`, `*to`, `*mint` —
**no `in_len` parameter**. C `char*` is null-terminated, but FFI contract is binary:
caller may pass a buffer that is not null-terminated (e.g. embedded in a fixed-size
record). Without explicit length, library must scan for `\0`, which is (a) wrong
semantics for binary data (mnemonic word list with embedded nulls is malformed but
defensive code should reject not crash), and (b) unbounded read past the intended
buffer.

**Fix.** Add `in_len: usize` to every `*in_*` param. cbindgen emits `size_t` in
header. Test row: null ptr + valid len, valid ptr + len=0, valid ptr + len > actual,
oversized len.

### 🔴 H3 — `sol_wallet_unlock` returns secret; caller-side zeroize not enforceable [HIGH]

Plan Step 4: `sol_wallet_unlock(*out_secret_bytes, id, password)` returns 32-byte
secret; doc-comment "caller MUST Zeroize". Comment ≠ enforcement. A buggy mobile
consumer that forgets the zeroize call leaks the seed through subsequent process
memory dumps (iOS `os_log` capture, Android `dumpsys meminfo`, core file from native
crash, swap on jailbroken devices).

**Fix.**
- Provide paired `sol_wallet_zeroize(buf: *mut u8, len: usize)` in Step 11 surface.
- Auto-zero `out_secret_bytes` on `sol_wallet_lock(id)` call (Rust side: `buf.zeroize()`
  via volatile write, then drop the source `Zeroizing<Vec<u8>>`).
- Rust side: hold secret in `Zeroizing<Vec<u8>>` while writing into caller buffer; drop
  the Vec immediately after the memcpy.
- Header doc-comment MUST show the zeroize call sequence as a code snippet, not prose.
- Test row: call `unlock`, then `lock`, then read `out_secret_bytes` — assert all-zero.

### 🔴 H5 — `sol_wallet_send_sol` / `sol_wallet_send_spl` lack policy gate [HIGH]

Plan Steps 8 + 9 take raw `*to`, `amount`, `*mint` — no per-wallet cap, no destination
allowlist, no rate limit. **Mobile context = a hostile app calling into this cdylib can
drain the wallet by repeated FFI calls.** CLI (Phase 7) has user-confirm gate; FFI
deliberately skips UI for UX. The skip-UI design is correct for legitimate mobile UX
but enables unattended drain by any process that loaded the cdylib (compromised
app, malicious SDK, XSS in embedded web view, malicious plugin).

**Fix (V0.1 must-haves).** Add three policy gates to Step 8/9:
- `max_per_tx_lamports: u64` per wallet (stored in Phase 6.1 `WalletMetadata`); FFI
  rejects `amount > max_per_tx_lamports` with `ExceedsMaxPerTx`.
- `allowed_destinations: Option<Vec<Pubkey>>` allowlist per wallet (None = unrestricted,
  Some(non-empty) = strict allowlist, Some(empty) = reject all). Set at wallet-create
  time, mutable only via explicit `sol_wallet_set_policy` FFI fn (which itself requires
  password re-auth).
- `daily_limit_lamports: Option<u64>` per wallet; tracked in PAL secure-storage (file
  on disk with mode 0600); reset at 24h rolling window.

Test row: each policy gate rejects the disallowed case; legitimate within-policy case
succeeds.

**Operator decision needed.** Ship in V0.1.0 or defer to V0.2? Argument for V0.1: mobile
hostile-app risk is acute. Argument for V0.2: scope creep, FFI UX changes require
mobile consumer coordination. Recommendation: ship in V0.1, but allow `unrestricted`
default for early adopters via `allowed_destinations: None`.

### 🔴 H6 — No `Send + Sync` audit on static FFI state [HIGH]

`sol_wallet_last_error_message` (Step 11) stores msg in a static buffer. Plan does not
specify the buffer's synchronization. Two threads racing on `sol_wallet_unlock` then
`sol_wallet_last_error_message` = data race on the static buffer (UB per Rust memory
model; observed behavior: torn reads returning the wrong error msg, or buffer
overrun when one thread's msg exceeds buffer while other thread overwrites). On
mobile, FFI is called from arbitrary threads (UI thread, network thread, async
dispatch); race is the default not the exception.

**Fix.**
- Either `thread_local!` for last-error (per-thread message, no lock needed; matches
  Rust panic-message-per-thread convention).
- Or `Mutex<[u8; 1024]>` with explicit lock + documented lock-ordering.
- cbindgen header documents the threading model.

Test row in Step 13: spawn 8 threads, each calls `sol_wallet_unlock` with bad id,
then reads `sol_wallet_last_error_message`. Assert each thread sees a consistent
msg (no torn read, no overwrite).

### 🔴 H7 — cdylib Rust runtime symbol leakage [HIGH]

Task 8.2 Step 1 + 2 verify `cargo check` + cbindgen. **No audit that the exported
symbol set = exactly the 12 intended functions.** Rust cdylib by default exports
runtime symbols: `__rust_alloc`, `__rust_dealloc`, `__rust_realloc`, `panic_impl`,
`core_*`, `std::*`, `backtrace_*`. Mobile attackers use these for ROP gadget
discovery and to fingerprint the Rust toolchain version (then look up known Rust
stdlib CVEs).

**Fix.**
- Build with `-C link-arg=-Wl,--exclude-libs,ALL` (Apple ld) or
  `-C link-arg=-Wl,--version-script=...` (Linux) — explicit symbol whitelist via a
  version script listing only the 12 `sol_wallet_*` symbols.
- Or use `cargo-bloat` / `nm -gU` post-build to assert no non-intentional symbols.
- `#[no_mangle]` + `pub extern "C"` only on the 12 entry points; everything else
  `pub(crate)` or `private`.
- `panic = "abort"` in release-mobile profile (also addresses H8) eliminates
  `panic_impl` symbol.

Test row in Task 8.2 Step 3 (NEW): `nm -gU libsol_wallet_core.dylib | grep -vE
'^\s*(_|sol_wallet_)' | wc -l` must equal 0 (or only documented libc deps).

### 🔴 H8 — `panic = "abort"` vs panic scrubber trade-off [HIGH]

Plan Step 1 builds panic scrubber assuming Rust panic unwinds + catch + scrub msg
+ return exit code 99. On mobile (Task 8.2 targets iOS/Android), release profile is
`panic = "abort"` per Rust mobile convention. With abort, the scrubber path never
runs — panic msg dies with the process unmasked in iOS unified log / Android
logcat. The whole scrubber investment is wasted in production builds.

**Fix.**
- Dev profile = `panic = "unwind"` + scrubber fires.
- Release-mobile profile = `panic = "abort"` + `setrlimit(RLIMIT_CORE, 0)` in PAL
  init to disable core dumps + `mlock` secret pages (L14).
- Document the trade-off in `panic_scrubber.rs` module-level doc-comment.
- Add `[profile.release-mobile]` in `Cargo.toml` with `panic = "abort"`,
  `opt-level = "z"`, `lto = true` (V0.1.5 item already in plan L1876 — promote to
  V0.1 since it's security-relevant).
- cbindgen header documents the dev/release difference for mobile consumers.

Test row in Step 13: trigger a panic from inner Rust fn via test-only `force_panic`
fn, assert either (a) dev build catches + scrubs + returns 99, or (b) release-mobile
build aborts cleanly without writing panic msg to logcat/unified-log.

### 🔴 H17 — No ABI link test on real device / simulator [HIGH]

Task 8.2 Steps 1 + 2 run `cargo check` (type-check only). Step 3 emits cbindgen
header (no verification). **No actual link/exec on aarch64 device or simulator.**
Header ABI mismatch with real dylib = runtime crash on first FFI call from
Dart/Swift/Kotlin consumer. The crash happens in production after release, not
in CI.

**Fix.** Add `cargo build --release --target aarch64-apple-ios` (actual link).
Write a 30-line C harness `tests/abi_smoke.c` that links the dylib and calls each
of the 12 exports with NULL inputs; assert return code = documented error code
(NOT segfault). Repeat for `aarch64-apple-ios-sim` and `aarch64-linux-android`.
Run via `cargo build` + `cc tests/abi_smoke.c -L target/aarch64-apple-ios-sim/release
-lsol_wallet_core -o abi_smoke && ./abi_smoke`.

Test row in Task 8.2 Step 3 (NEW): 12-export matrix × 3 targets (ios, ios-sim,
android).

---

### 🟠 H4 — Password not zeroized in Rust [HIGH]

Plan Step 4 takes `password` from caller. Does Rust zeroize its local copy after
Argon2id derivation? Plan does not specify. Argon2id runs at 64 MB / 3 iters per
plan (deep-dive §H); derivation holds the password in stack for the duration.
Password leak path: process memory dump (already discussed in H3), but also via
swapfile if `mlock` not applied (L14).

**Fix.** Step 1 scrubber spec already requires Zeroizing discipline — extend
explicitly to `password: *const u8, password_len: usize` parameter: copy into
`Zeroizing<Vec<u8>>` on heap, drop after derivation. Stack copies via `std::ptr::read`
are unsafe; use heap allocation only. Add password mnemonic-like content to
panic-scrubber fuzz corpus (Step 12).

### 🟠 M9 — Domain separation absent on `sol_wallet_sign_transaction` [HIGH→MEDIUM]

Plan Step 7 signs any 32-byte hash the caller hands it. Solana Ed25519 has no
chain-id binding; cross-Solana-cluster replay possible if attacker can swap the
`cluster` parameter (which determines RPC endpoint). Step 8/9 high-level paths
include cluster in the constructed tx; low-level Step 7 does not. An attacker who
captures a Step 7 signature from `devnet` cannot directly replay on `mainnet-beta`
because the message bytes differ (different recent blockhash), but a buggy
caller that signs a fixed preimage (e.g. a stored message template) creates a
replay risk.

**Fix.** Doc-comment on `sol_wallet_sign_transaction` that caller MUST include
Solana recent blockhash + cluster identifier in the preimage. Or: library prepends
a domain tag `b"\x00sol-wallet-core/v0.1.0"` (4-byte magic + semver) to the
signed message before Ed25519 sign; recipient must verify the tag. The latter is
defense-in-depth and prevents accidental cross-cluster replay.

### 🟠 M10 — Error msg enumeration via `sol_wallet_last_error_message` [HIGH→MEDIUM]

Errors that include the input `id`, `cluster`, or address leak caller state to
attacker controlling subsequent `sol_wallet_last_error_message` reads. Plan does
not redact error messages. The Phase 0 row23 GAP (`error.rs Debug redact`) covers
library-side; the FFI error path needs the same treatment.

**Fix.** Phase 8.1 error enum must follow the scrubber discipline from Step 1:
error msgs stored in `last_error` buffer pass through the same panic scrubber
before being written. Add a `redact_for_ffi()` fn on the error enum.

### 🟠 M11 — ReDoS risk in panic scrubber regex [HIGH→MEDIUM]

`once_cell::Lazy<regex::Regex>` over user-controlled panic msgs. Plan patterns
(L1687): mnemonic words (BIP-39 wordlist), 64-byte base58, 32-byte hex, `xprv...`
prefix. Need to verify regexes are linear-time (no backtracking). The BIP-39
wordlist is 2048 words; a pattern like `(abandon|ability|able|...|zoo)` (alternation
of 2048 literals) is linear-time in the Rust `regex` crate's NFA simulation, but a
catch-all `(?:[a-z]+\s+){12,24}` matcher (12–24 word sequences) could backtrack if
not anchored. Plan does not specify the regex.

**Fix.** Use `aho-corasick` (literal Aho-Corasick, O(n+m)) for the literal
patterns (mnemonic words, xprv prefix). For 64-byte base58 and 32-byte hex, use
explicit character-class matchers in hand-written code, not regex. Avoid the
`regex` crate entirely on FFI-controlled input.

---

### 🟡 M12 — Wallet storage path injection via `cluster` param [MEDIUM]

`sol_wallet_create_mnemonic(..., cluster)` — does `cluster` influence the storage
path? Plan does not say. If yes, attacker controls path → path traversal
(`cluster = "../../etc/passwd"`). `WalletManager` from Phase 6 owns the path
computation; the FFI passes through.

**Fix.** Validate `cluster` against fixed enum `Cluster::{Mainnet, Devnet,
Localnet}` (no `Testnet` per P7-24 confirm). Reject unknown enum values at FFI
boundary with `InvalidCluster`. Sanitize path component to `[a-z0-9-]` only as
defense-in-depth.

### 🟡 M13 — Negative test coverage gaps in Step 13 [MEDIUM]

Step 13 ABI smoke covers status codes only. Missing:
- null pointer on every `*in_*` and `*out_*` arg
- zero-length buffers
- concurrent calls from N threads (race on static error buffer, see H6)
- sign-after-lock returns `WalletLocked` not silent success
- panic from inner Rust fn does not cross FFI (caught at `extern "C"` boundary)
- `sol_wallet_unlock` + `sol_wallet_lock` sequence followed by read of
  `out_secret_bytes` returns all-zero (H3)

**Fix.** Address these in Step 13 (expand from "12 fns × 1 input" to "12 fns × N
inputs"). Or file V0.1.5 GAP tickets with explicit acceptance per operator
sign-off.

### 🟡 M14 — `sol_wallet_unlock` signature value lifetime after `lock` [MEDIUM]

Plan does not specify what happens to `out_secret_bytes` after `sol_wallet_lock`.
Per H3 fix: auto-zero on `lock`. Plan must state this explicitly.

**Fix.** Plan Step 5 (`sol_wallet_lock`) becomes "zero `out_secret_bytes` for this
id + remove from in-memory map". Phase 8.1 plan amendment.

### 🟡 M15 — Output buffer overflow on `sol_wallet_create_mnemonic` mnemonic length [MEDIUM]

24-word mnemonic is ~180 bytes (24 × 7 chars + 23 spaces). cbindgen header must
document `SOL_WALLET_MNEMONIC_BUF_LEN = 256` as a `#define` constant. Mobile
consumer allocates a buffer using the constant.

**Fix.** Plan Step 2 amendment: cbindgen emits `#define SOL_WALLET_MNEMONIC_BUF_LEN
256` etc. for each `*out_*` type. Test row: caller allocates via `#define`, library
writes within bound.

### 🟡 M16 — `sol_wallet_create_mnemonic` cluster param has no default [MEDIUM]

Plan Step 2: `sol_wallet_create_mnemonic(*out_id, *out_mnemonic, password, cluster)`
— `cluster` is required. A mobile consumer that wants "default cluster" must look
up the current config first (extra FFI roundtrip). Or: add an optional
`cluster: Cluster` param where `Cluster::Devnet = 0` is the default.

**Fix.** Document the default-cluster behavior in the FFI safety contract
amendment. Add a `sol_wallet_default_cluster(*out_cluster)` fn for explicit
discovery.

### 🟡 M18 — No `strip` on release cdylib [MEDIUM]

Task 8.2 does not specify `strip = "symbols"` in release profile. Without strip,
mobile binary carries 5–15 MB of debug symbols = larger app, slower load, larger
attack surface for symbol-based exploitation.

**Fix.** Task 8.2 Step 3 amendment: build with `--release` profile that includes
`strip = "symbols"` in `Cargo.toml` `[profile.release]`.

---

### 🔵 L11 — Constant-time Ed25519 not asserted [LOW]

Solana ed25519-dalek is constant-time by default since 1.0, but plan does not
assert which crate version + which features. Timing oracle on Ed25519 sign would
leak the secret key.

**Fix.** `Cargo.toml` pin: `ed25519-dalek = { version = "2", features = ["std"],
default-features = false, "fast", "zeroize"] }`. Add to Phase 0 Cargo.toml
ordering recommendation. Verify with `cargo tree` that no other ed25519 impl
transitively included.

### 🔵 L14 — No `mlock` on secret pages [LOW → V0.1.5]

Zeroizing discipline covers heap buffers. OS may swap heap pages to disk; secret
persists in swapfile. Mobile devices usually no-swap, but desktop / Linux CI =
swap on. macOS and iOS use memory encryption (Apple Secure Enclave for keys), so
practical risk is low. Defense-in-depth only.

**Fix (V0.1.5).** Wrap secret allocations in `mlock`-backed allocator (e.g.
`malloc_buf` crate). Document in Step 1 scrubber rationale.

### 🔵 L15 — ABI version not in symbol names [LOW]

`sol_wallet_*` symbols. If mobile loads both v0.1.0 and v0.2.0 cdylibs (unlikely
but possible via dynamic linking), symbol collision = silent ABI mismatch.

**Fix.** cbindgen header prefix + symbol versioning `sol_wallet_0_1_unlock`
or embed version in dylib filename, document in mobile setup README.

### 🔵 L19 — cbindgen output not diff-checked [LOW]

Plan: "cbindgen emits `sol_wallet_core.h` matching consumer expectations" —
but no verification command. cbindgen re-run with different version produces
different output.

**Fix.** Run cbindgen, commit header to repo, CI diff against committed version.
Drift = manual review required.

### 🔵 L20 — iOS/Android target specifiers absent [LOW]

Plan says `cargo check --target aarch64-apple-ios` (Step 1) but no iOS minimum
version, no `aarch64-linux-android` API level.

**Fix.** Phase 0 Cargo.toml `[target.*]` sections + PAL trait gating already
covers per Q17; cross-link here. Add `[target.aarch64-apple-ios]` rustflags for
iOS 13+ minimum.

---

### ✅ PASS-1 — `proptest` already a dev-dependency [PASS]

Phase 7 added `proptest` to dev-deps. Step 12 panic-scrubber fuzz can reuse it
without new dep.

### ✅ PASS-2 — `Zeroizing` already a dep [PASS]

Phase 6 added `zeroize` dep. Step 1 scrubber + Step 4 unlock secret handling
reuse it.

### ✅ PASS-3 — `once_cell` already a dep [PASS]

Phase 6 added `once_cell`. Step 1 panic-scrubber regex cache reuses it.

---

## Resolution contract

A Phase 8.1 PR is not mergeable until all 20 finding boxes flip ✅ in the
companion issue body (update-issues-before-merge rule per memory). Each fix
lands in the Phase 8.1 PR that closes the issue; the checklist-flip +
squash-merge happen together.

## Recommended Phase 8 plan amendments

The following plan amendments are recommended BEFORE Phase 8.1 implementation
starts. Each is filed as an inline edit to `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`.

### Phase 8.1 amendments

After Step 11, insert:

> **Step 11a — FFI safety contract (BLOCK on H1–H8):**
> - [ ] Every `*out_*` param paired with `out_len: *mut usize` + `buf_len: usize`; return `BufTooSmall` with required size via `sol_wallet_last_error_message`.
> - [ ] Every `*in_*` param paired with `in_len: usize`; reject null ptr + valid len mismatch.
> - [ ] `sol_wallet_zeroize(buf, len)` added to Step 11 surface.
> - [ ] `sol_wallet_lock` auto-zeros the matching `out_secret_bytes` for the unlocked id.
> - [ ] Rust-side `Zeroizing<Vec<u8>>` discipline on password + secret; heap allocation, no stack copies.
> - [ ] `Send + Sync` audit on all static FFI state; either `thread_local!` or `Mutex<[u8; N]>`.
> - [ ] cbindgen emits `#define SOL_WALLET_MNEMONIC_BUF_LEN 256`, `SOL_WALLET_SECRET_LEN 32`, etc.
> - [ ] Error enum `redact_for_ffi()` applies panic scrubber before writing to `last_error`.

After Step 13, insert:

> **Step 13a — Negative test matrix (BLOCK on H3, H6, M13):**
> - [ ] Null pointer test for each of the 8 `*out_*` + 6 `*in_*` args (14 cases).
> - [ ] Undersized buffer test (caller allocates ½ required size, library returns `BufTooSmall`).
> - [ ] Oversized buffer test (caller allocates 4× required size, library writes within bound).
> - [ ] Concurrent test (8 threads × `unlock` + `last_error_message` race, assert no torn read).
> - [ ] Sign-after-lock test (`unlock` → `lock` → `sign_transaction` returns `WalletLocked`).
> - [ ] Panic-during-FFI test (test-only `force_panic` fn → extern "C" boundary catches).
> - [ ] Unlock-then-lock-then-read test (H3: `out_secret_bytes` all-zero after `lock`).

### Phase 8.2 amendments

After Step 3, insert:

> **Step 3a — Release link + ABI smoke (BLOCK on H17, H7):**
> - [ ] `cargo build --release --target aarch64-apple-ios` succeeds (actual link, not just check).
> - [ ] `cargo build --release --target aarch64-apple-ios-sim` succeeds.
> - [ ] `cargo build --release --target aarch64-linux-android` succeeds.
> - [ ] Write `tests/abi_smoke.c` — 30-line C harness linking the dylib + calling each of 12 exports with NULL inputs.
> - [ ] CI job runs `cc tests/abi_smoke.c -L target/<triple>/release -lsol_wallet_core -o abi_smoke && ./abi_smoke && echo "ABI GREEN"`.
> - [ ] Assert return code = documented error code for each NULL input (NOT segfault).

> **Step 3b — Symbol audit (BLOCK on H7):**
> - [ ] `nm -gU target/aarch64-apple-ios-sim/release/libsol_wallet_core.dylib | sort` output captured to `target/symbol-audit.txt`.
> - [ ] Assert only `sol_wallet_*` symbols + documented libc deps (`_$s...`, `_strerror`, etc.) present.
> - [ ] Zero Rust runtime symbols (`__rust_alloc`, `panic_impl`, `core_*`) in exported set.
> - [ ] If violations found, add `-C link-arg=-Wl,--exclude-libs,ALL` to `[profile.release-mobile]` rustflags.
> - [ ] Or write a version script `sol_wallet_core.version` listing only the 12 symbols; pass via `-C link-arg=-Wl,--version-script=...`.
> - [ ] `[profile.release-mobile]` in `Cargo.toml`: `panic = "abort"`, `opt-level = "z"`, `lto = true`, `strip = "symbols"` (promotes V0.1.5 plan L1876 to V0.1).

---

## Cross-references

- Plan §Phase 8 (amended): `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md` L1677–1710
- Plan §Phase 8.1 Step 11a + 13a: NEW amendments per this audit
- Plan §Phase 8.2 Step 3a + 3b: NEW amendments per this audit
- Plan §Phase 8 verification: Step 14 (8.1) + Step 4 (8.2) include new amendments
- Plan §F47 zeroize gap: deep-dive L2084–2097 (memory hygiene) — informs H3, H4, L14
- Deep-dive §K FFI surface: L1864–1890 (12 C functions enumeration)
- Deep-dive §H encrypted wallet JSON: L1754–1790 (Phase 6.1 schema; H3 zeroize on unlock)
- Phase 6 audit: `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md` P6-3 (plaintext-key), P6-4 (zeroize gap) → propagate to FFI unlock fn
- Phase 7 audit: `docs/audit/2026-09-11-sol-wallet-core-phase7-security-review.md` P7-5 (eprintln mnemonic leak) → reuse as panic-scrubber fuzz corpus seed
- Companion audit: `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md` P5-3 (broken clap sample) does NOT apply to Phase 8 (no clap in FFI surface)
- Testnet DEPRECATED 2022-23 (L378, L426, L436, L2557): M12 cluster validation confirms V0.1 has only Mainnet/Devnet/Localnet
- cbindgen docs: https://github.com/eqrion/cbindgen/blob/master/docs.md (12-export version script template)
- libloading docs: https://docs.rs/libloading/latest/libloading/ (in-process FFI smoke pattern)
- Solana FFI precedent: https://github.com/solana-labs/solana/blob/master/sdk/wasm/js-solana-wallet-adapter/src/ (browser FFI patterns to mirror)

---

## Severity rollup

| Sev      | Count | Must-fix for V0.1.0?                                       |
|----------|-------|------------------------------------------------------------|
| 🔴 HIGH  | 8     | All H items block V0.1.0 release                            |
| 🟠 MED   | 6     | H4 + M9 + M10 + M11 fix in Phase 8.1 impl; L11 + L14 deferable to V0.1.5 |
| 🟡 MED   | 7     | Fix in Phase 8.1 impl, deferable to 8.1 only with operator sign-off |
| 🔵 LOW   | 5     | V0.1.5 backlog (file tickets)                              |
| ✅ PASS  | 3     | flagged for completeness                                   |
| **Total**| **29**|                                                            |

Note: H4 reclassified MED (down from HIGH) because plan does call out Zeroizing
discipline in Step 1 scrubber — the gap is password-specific extension, not the
whole discipline. M9-M11 also reclassified MED for same reason.

## H items blocking V0.1.0

H1, H2, H3, H5, H6, H7, H8, H17.