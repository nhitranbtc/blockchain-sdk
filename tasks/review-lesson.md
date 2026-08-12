# review-lesson.md

Review-phase discipline playbook. Companion to `plan-lesson.md` (planning) and `lessons.md` (corrections ledger).

Scope: code review, content review, plugin patterns that apply during review. Excludes planning-only lessons (PL1–PL3, PL7–PL16) and deep-search tools (PL18–PL20).

Read at session start when running L12 review, audit, or content review.

---

## Index

- [PL4] Plugin/SDK patterns: flat re-exports + `#[non_exhaustive]` on error enums
- [PL5] Async mutex over sync mutex across `.await` points (plugin host safety)
- [PL6] SDK SemVer + MSRV stability policy belongs in plan header
- [PL17] Plugins for REVIEWING (use during plan/code review)

---

## PL4 — Plugin/SDK patterns: flat re-exports + `#[non_exhaustive]` on error enums

**Trigger:** Session 2026-08-12 review of `bitcoin-wallet-core` plan. Found `lib.rs` had no `pub use` re-exports (consumers write `bitcoin_wallet_core::wallet::Wallet` deeply nested) and `Error` enum had no `#[non_exhaustive]` (adding a variant breaks downstream `match`).

**Rule:** For any library crate published as an SDK:
1. **`lib.rs` root must re-export the primary types** (`Wallet`, `WalletConfig`, `Error`, `Result`) so consumers write `bitcoin_wallet_core::Wallet`, not `bitcoin_wallet_core::wallet::Wallet`.
2. **`Error` enum must be `#[non_exhaustive]`** so adding variants is non-breaking for consumers matching on `_ =>`.
3. **MSRV must be declared in `[package].rust-version`** and tested in CI.

**Why:** Plugin/SDK consumers (Swift host, JS host, Python host, internal services) match on public types. Deep nesting = verbose imports. Non-exhaustive errors = every new variant is a SemVer break. SDK ergonomics + stability policy is the difference between an SDK and a library.

**How to apply (at review time):**

1. Open `lib.rs`. Check for `pub use <module>::<PrimaryType>` at the crate root.
2. Open the `Error` enum. Check for `#[non_exhaustive]` attribute above `pub enum Error`.
3. Open `Cargo.toml`. Check for `rust-version.workspace = true` (or explicit) on the crate package.
4. If any missing, file findings: "Add flat re-export" / "Annotate Error with #[non_exhaustive]" / "Declare MSRV".

**Anti-patterns:**
- "Consumers can write the long path." They can, but every consumer pays the same tax. Fix the root, not the docs.
- "Adding a variant is SemVer anyway." Not with `#[non_exhaustive]` — that's the whole point.
- "MSRV is in the README." README drifts. `Cargo.toml` is the source of truth.

**Companion:** PL6 (stability policy in plan header), PL8 (umbrella SDK design rule, in `plan-lesson.md`).

---

## PL5 — Async mutex over sync mutex across `.await` points

**Trigger:** Session 2026-08-12 review. Plan showed `Wallet { bdk: Mutex<BdkWallet> }` with `.lock().unwrap()` across `.await` points in `sync.rs` and `balance.rs`. Real `main` code uses `tokio::sync::Mutex`.

**Rule:** For any library holding state accessed by async methods:
1. **Default to `tokio::sync::Mutex`** when lock spans an `.await`.
2. **`std::sync::Mutex` only for sync-only critical sections** (e.g., in-memory cache lookups that don't await).
3. **No `.lock().unwrap()`** — use `.lock().await` (tokio) or handle `PoisonError` (std).

**Why:** `std::sync::Mutex` held across `.await` blocks the runtime thread. In plugin hosts (UniFFI → Swift, NAPI → Node, PyO3 → Python), the host event loop stalls. Result: UI freezes, requests time out, test suite hangs. `tokio::sync::Mutex` integrates with the runtime scheduler.

**How to apply (at review time):**

1. Grep for `std::sync::Mutex` and `use std::sync::Mutex` near `.await` calls.
2. If found: file finding "Replace `std::sync::Mutex` with `tokio::sync::Mutex` (lock spans await)."
3. For sync-only locks, suggest `parking_lot::Mutex` (faster, poisoning-free).
4. Also flag any `.lock().unwrap()` — replace with `.lock().await` or handle `PoisonError`.

**Anti-patterns:**
- "It's just an internal cache, sync mutex is fine." Internal cache + future `.await` = silent bug.
- "We don't have plugin hosts yet." Adding them later = forced refactor. Get the lock type right now.
- "The unwrap can't panic." It can — `PoisonError` is a real failure mode.

**Companion:** PL4 (SDK patterns), PL8 (host-first SDK design).

---

## PL6 — SDK SemVer + MSRV stability policy belongs in plan header

**Trigger:** Session 2026-08-12 review. Plan picked `0.1.0` in tech-stack line but said nothing about SemVer guarantees, MSRV bumps, or deprecation windows. SDK consumers cannot plan adoption without this.

**Rule:** Every plan for an SDK/library crate must include a **Stability Policy** section in the header (next to "Goal" and "Architecture") stating:
1. **Pre-1.0 SemVer rule** (typically: breaking changes allowed at any minor bump).
2. **Post-1.0 SemVer rule** (typically: SemVer strict, MSRV bumps are minor).
3. **MSRV bump policy** (typically: MSRV bumps require a one-release deprecation cycle).
4. **Deprecation window** (typically: deprecated APIs marked for ≥1 minor version before removal).

**Why:** SDK consumers (especially cross-language hosts: Swift, JS, Python) lock to specific SDK versions for years. Silent SemVer breaks = silent consumer breakage. MSRV bumps without notice = consumer CI breaks on Rust toolchain update.

**How to apply (at review time):**

1. Read plan header. Look for "Stability Policy" or "SemVer" or "MSRV" section.
2. If missing: file finding "Add Stability Policy section above Global Constraints using PL6 template."
3. If present: verify it covers all 4 bullets (pre-1.0, post-1.0, MSRV, deprecation).

**Stability template (paste into plan):**

```markdown
## Stability Policy

- **Pre-1.0** (`0.y.z`): breaking changes allowed at any minor bump. Patch bumps = bug fixes only.
- **Post-1.0** (`1.y.z`): strict SemVer. Minor = additive features. Major = breaking.
- **MSRV:** bumps require one prior minor release with `rust-version` warning. No silent MSRV bumps.
- **Deprecation:** deprecated APIs marked `#[deprecated]` for ≥1 minor version before removal.
```

**Anti-patterns:**
- "Pre-1.0 means anything goes." Yes, but the policy must still be stated so consumers know.
- "MSRV bump is internal." No — it's a downstream CI break. Warn first.
- Deprecating and removing in the same release. Consumers can't migrate.

**Companion:** PL4 (flat re-exports + `#[non_exhaustive]`), PL8 (umbrella SDK rule), L9 (PR body = stability-impact analysis on breaking changes).

---

## PL17 — Plugins for REVIEWING (use during plan/code review)

**Trigger:** User types "review this" / "audit X" / "L12 review" / "drift scan." Quick reference.

| Review type | Plugin / skill | When |
|---|---|---|
| Code review (PR) | `superpowers:code-reviewer` (pr-review-toolkit) | L12 step — before local verify gate |
| Comment quality | `pr-review-toolkit:comment-analyzer` | After generating large docstrings / comments |
| PR test coverage | `pr-review-toolkit:pr-test-analyzer` | After PR created/updated |
| Silent failures | `pr-review-toolkit:silent-failure-hunter` | After error-handling logic added |
| Type design | `pr-review-toolkit:type-design-analyzer` | When introducing a new type |
| Code simplification | `pr-review-toolkit:code-simplifier` | After logical chunk of code written |
| Plan drift scan | `tasks/plan-lesson.md` PL1, PL2, L30 | Before committing plan edits |
| Doc review | `superpowers:claude-code-guide` (or human reviewer) | For doc-only PRs |
| Cross-SDK comparison | `mcp__exa__web_search_exa` + `mcp__firecrawl__firecrawl_scrape` | For research-area PRs |
| Library docs lookup | `mcp__plugin_context7_context7__query-docs` | When reviewing API usage against latest docs |

**L12 review order (per L13 + L12):**

1. Drift scan cited SHAs (L30) and code blocks (PL1).
2. Story ↔ task trace (PL3).
3. Plugin-pattern check (PL4, PL5, PL6, PL8).
4. Code review via `pr-review-toolkit:code-reviewer`.
5. Test coverage via `pr-review-toolkit:pr-test-analyzer`.
6. Silent-failure scan via `pr-review-toolkit:silent-failure-hunter`.
7. Comment accuracy via `pr-review-toolkit:comment-analyzer`.
8. Type design via `pr-review-toolkit:type-design-analyzer` (only if new types).
9. Local verify gate (cargo fmt + clippy + test).

**Anti-patterns:**
- Running code review AFTER local verify gate. Per L12: code review runs BEFORE.
- Skipping drift scan on plan-only PRs. Plans drift too.
- Skipping silent-failure scan on error-handling code. Catches swallowed errors before merge.

---

## Self-Review

1. **Scope:** All 4 lessons (PL4, PL5, PL6, PL17) apply to code review + SDK quality + plugin selection for review.
2. **Removed from this file:** PL1–PL3, PL7–PL16 (planning-only, in `plan-lesson.md`), PL18–PL20 (search-only, in `search-lesson.md`).
3. **Companion mapping:**
   - PL4 + PL5 + PL6 + PL8 = SDK quality bar (PL8 in `plan-lesson.md`).
   - PL17 = review-phase plugin quick reference.
   - L12 (review runs before verify), L30 (drift scan) are the parent rules.
4. **Anti-pattern audit:** Each lesson names the failure mode it prevents.

---

## Update protocol

When a review surfaces a reusable lesson:

1. **Draft** new PL entry in this file (terse: trigger → rule → why → how to apply → anti-patterns).
2. **Add** to Index above.
3. **Cross-link** to related `plan-lesson.md` and `lessons.md` entries.
4. **Commit** on its own PR titled `docs(review-lesson): PL<N> <short title>`.
5. **Wire** into L12 review checklist.

Do NOT add to `lessons.md` (corrections ledger) or `plan-lesson.md` (planning-only).