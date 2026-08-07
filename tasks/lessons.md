# lessons.md

Project-local corrections ledger. Seeded from recent commits + ready for new entries.

**Rules** (from Boris Cherny workflow):
1. After ANY correction from user/review/CI: write new lesson here.
2. Review this file at session start for relevant project.
3. Iterate until mistake rate drops.
4. Keep entries terse: trigger → rule → why → how to apply.

---

## Index

- [L1] Workspace path consistency across docs + Cargo manifests
- [L2] Schema-validate config keys before commit
- [L3] Major-version pins stay mutable; bump via Dependabot
- [L4] Post-scaffold verify pass catches lint/doc issues
- [L5] Workspace inheritance only when parent defines it
- [L6] never auto-commit (memory)
- [L7] pause before state-modifying actions (memory)
- [L8] flip issue checkboxes before squash-merge (memory)
- [L9] issue bodies = status, PR bodies = fix analysis (with table)
- [L10] threat model is the answer key — read before writing code
- [L11] scan skills list at session start, tag 3-5 relevant, invoke before doing
- [L12] code review runs BEFORE local verify gate, not after

---

## L1 — Workspace path consistency

**Trigger**: `c2a64b7 docs(claude): fix workspace refs` + `5943c84 fix(umbrella): post-verify polish`.

**Rule**: When project layout changes (workspace rename, crate relocation), grep all `*.md`, `*.toml`, `*.yml` for old paths in one pass. Update CLAUDE.md + plan + Cargo manifests together.

**Why**: 31 stale path references in plan file alone. Drift between docs and actual tree = wrong file edits, broken links, confused contributors.

**Apply**: After any `mkdir` / `mv` / `cargo new` inside rust-wallet-app/, run:
```
grep -rn "old-path" docs/ rust-wallet-app/ rust-wallet-app/crates/*/Cargo.toml
```
before commit.

---

## L2 — Schema-validate config keys before commit

**Trigger**: `2a46af1 fix(deny): drop unused-allowed key (not in cargo-deny schema)`.

**Rule**: For any tool config file (`deny.toml`, `Cargo.toml`, `ci.yml`), run the tool's dry-run/check before committing. Don't paste config from docs without verifying.

**Why**: `cargo deny check` rejected `unused-allowed = warn` as unexpected key. CI failed. Reviewer caught it; CI caught it; should have caught at write time.

**Apply**: After editing `deny.toml` → `cargo deny check`. After `ci.yml` → `act` or push to branch. After `Cargo.toml` → `cargo metadata --no-deps`.

---

## L3 — Major-version pins stay mutable

**Trigger**: `bd8499d ci: revert checkout to @v4 major tag` + `0e59e85 ci(deps): bump actions/checkout from 4.1.1 to 7.0.1`.

**Rule**: Pin third-party GitHub Actions to MAJOR tag only (`@v4`, not `@v4.1.1`). Dependabot handles minor/patch. Bumps between majors require user approval.

**Why**: Floating SHA = supply-chain risk. Frozen patch = no security updates. Major tag = auto-patch with manual major review. Dependabot config at `.github/dependabot.yml` covers the loop.

**Apply**: When adding/updating CI action → use `@vN` form. Before bumping major → pause, ask user, document rationale in commit body.

---

## L4 — Post-scaffold verify pass

**Trigger**: `5943c84 fix(umbrella): post-verify polish` (cleanup chain-traits missing_docs, repository inheritance).

**Rule**: After scaffolding a new crate/module, run full verify suite immediately: `cargo fmt --check && cargo clippy -- -D warnings && cargo test && cargo doc --no-deps`. Fix lint/doc warnings before declaring scaffold done.

**Why**: `missing_docs` on `SolanaCluster` variants + `ChainError` variants surfaced only when `cargo doc` ran. Scooping these up post-merge = larger PR noise. Cleaning at scaffold origin = one atomic commit.

**Apply**: After `cargo new` / `cargo init` → run verify in same commit. Use `cargo doc --no-deps` (not just `cargo build`) to catch doc-only warnings.

---

## L5 — Workspace inheritance only when parent defines it

**Trigger**: `5943c84 fix(umbrella): post-verify polish` — `chain-traits/Cargo.toml: remove repository.workspace (parent workspace doesn't define repository)`.

**Rule**: `[workspace]` inheritance in member crates only inherits keys the parent WORKSPACE `[workspace]` table defines. If key lives in parent package, not allowed via `.workspace = true`.

**Why**: Cargo error: `the workspace specified by ... does not have the field "repository"`. Field belongs on package, not workspace. Common confusion since `rust-version`, `edition`, `license` ARE workspace-allowed.

**Apply**: Before adding `[field.workspace] true` to member → check parent `Cargo.toml` `[workspace]` table. Workspace-allowed: `package.*` subset (edition, version, authors, license, repository, homepage, rust-version, etc.) — see Cargo docs.

---

## L6 — Never auto-commit (memory)

**Rule**: Pause and ask before any `git commit`. User wants final say on history.

**Why**: Commits = immutable public artifact. No easy undo without rewrite.

**Apply**: Always STOP before `commit`. Report diff + test output. Ask: "commit?". Resume only after approval.

---

## L7 — Pause before state-modifying actions (memory)

**Rule**: Pause before: `gh` calls, branch ops, file moves outside `docs/`. Discuss first, execute after approval.

**Why**: External surface actions (PR create, branch delete, issue close) hard to reverse cleanly.

**Apply**: For any tool that mutates remote state — describe intent, await approval, then execute.

---

## L8 — Flip issue checkboxes before squash-merge (memory)

**Rule**: Before `gh pr merge --squash`, run `gh issue edit N --body "..."` to flip all completed `[ ]` to `[x]`. Audit trail must reflect actual work done.

**Why**: Issue body = source of truth for task completion. Stale checkbox = misleading retrospective.

**Apply**: Per task: at PR-open time, issue body must match code reality. Final check before merge.

---

## L9 — Issue bodies = status, PR bodies = fix analysis

**Trigger**: Task 1.5 security review (2026-08-07) — 4 findings on PR #23. Initially put the before/after + score table in the issue body; user redirected: tables with detailed analysis belong in the PR, issue stays a status tracker.

**Rule**:

- **Issue body** = concise status tracker. Checkboxes for steps, brief drift summary, acceptance criteria, link to PR. Stays readable at-a-glance across many issues.
- **PR body** = detailed analysis when there's a fix worth documenting. Before/after table with explicit trade-offs (not "pros/cons"). Carries the rationale reviewers need.
- When a fix happens mid-merge (e.g. security review post-push), the detailed table goes in the **PR body**, not the issue body. Update PR via `gh pr edit`; simplify issue body.

**Why**: PRs are reviewed by humans reading code; issues are tracking artifacts that get archived. Long-lived rationale lives where people read once (PR); current state lives where people check often (issue). Detailed analysis in an issue body bloats every list/search result and obscures the checklist.

**Apply — required PR drift table schema (v3):**

| Column | Content |
| --- | --- |
| `Area` | code area (`Secret`, `atomic_write`, `permissions`, etc.) — not "step N" |
| `Drift` | what changed vs plan/spec |
| `Sev` | LOW / MEDIUM / HIGH / CRITICAL — tagged by impact, not by review-tool severity |
| `` File:line `` | code block (e.g. `` `keys/secret.rs:25` ``) — pin to current lines after fix |
| `Result` | what was achieved after the improvement (concrete outcome) |
| `Score` | `N/10 — <handle>` — honest self-score per row, with attribution |
| `Note` | future improvements needed (or "None") |

**Apply — required PR technical-details table (v3):**

| Column | Content |
| --- | --- |
| `Tool / Plugin` | skill / hook / crate / stdlib function |
| `Role` | `find` (caught the issue) / `resolve` (fixed it) / `review` (audited) |
| `What it caught / fixed` | one-line summary |
| `Used at step` | commit + file:line where applied |

Plus, after the tables:

- **Test gaps**: any code path in the fix that lacks a test. Name the path + line. Required.
- **Migration impact**: any behavior change visible to callers. If API is unchanged but behavior is stricter, document. Required for security/permissive-related fixes.
- **Per-dimension verdict**: PASS / PARTIAL / FAIL with explicit rubric per dimension:
  - Correctness (plan + spec compliance)
  - Security (threat-model coverage)
  - Test coverage (happy path + N negative cases)
  - Code simplicity (karpathy §2)
  No single overall score — let reader average.
- **Main points** (numbered list): trigger for the fix, plan-compliance vs hardening distinction, threat-model mapping, drift-from-plan to record.

Anti-patterns to avoid:

- "Pros / Cons" columns — conflate fix-correctness with code quality. Replace with single Trade-off column.
- 1-10 score scale with 0.5 precision as overall number — PASS/PARTIAL/FAIL per dimension for verdicts; per-row self-score with handle is OK.
- Single overall score — averages hide dimension-specific failure.
- "Step N" as row category — use code area instead.
- Burying costs in prose — every cost goes in Trade-off column.
- Skipping test-gap callout — if you wrote code without a test, name the missing test.

Apply this schema to: drift fixes, security findings, refactors, breaking changes. Skip for trivial typo/style PRs (one-line body is fine).

---

## L11 — Scan skills list at session start, tag 3-5 relevant, invoke before doing

**Trigger**: Session 2026-08-07 — 9 Matt Pocock skills (`mattpocock-skills:*`), ~15 superpowers, ~10 compound-engineering, ~10 pr-review-toolkit, ~20 compass/ecc skills were loaded. Used 3 total (karpathy-guidelines, commit-commands:commit-push-pr, pr-review-toolkit:review-pr). Said "I don't see Matt Pocock plugins" when they were loaded — only saw them after user repeated the question and SessionStart hook listed them again.

**Rule**:

1. **At every session start**, enumerate the skills list (`/skills` if listed, or the SessionStart hook output) and tag 3-5 skills that match the active task.
2. **Before starting each task step** (pickup, TDD, verify, pre-PR, post-merge), invoke the relevant skill — don't rely on manual checklist.
3. **If a skill exists for a step I'm doing manually, invoke it.** Manual checklist failure modes: blind spots, missing sub-agents, no parallel review.

**Why**: 47 skills available. Skills encode battle-tested workflows (Pocock's TDD, superpowers:verification-before-completion, pr-review-toolkit:code-review with parallel sub-agents). Each one I skip is a workflow gap. Task 1.5's 4 security findings would have been caught by `pr-review-toolkit:code-review` invoked pre-PR instead of `security-guidance` invoked post-push.

**Skill → task-step mapping** (use this as starting checklist):

| Task step | Skill to invoke first |
| --- | --- |
| Task pickup (understand + plan) | `mattpocock-skills:domain-modeling` if new domain; `compound-engineering:ce-plan` if multi-step |
| TDD red-green-refactor | `mattpocock-skills:tdd` (NOT manual red→green) |
| Build/cargo error cascade | `mattpocock-skills:diagnosing-bugs` |
| Module interface design | `mattpocock-skills:codebase-design` |
| Pre-PR review (security, tests, structure) | `pr-review-toolkit:code-review` (parallel sub-agents for Standards + Spec axes) |
| Test coverage gap analysis | `pr-review-toolkit:pr-test-analyzer` |
| Doc / threat-model review | `compound-engineering:ce-doc-review` |
| Before declaring done | `superpowers:verification-before-completion` |
| Commit + push + PR | `commit-commands:commit-push-pr` |

**Apply**:

- After every `Skill` invocation that returns useful guidance, invoke it AGAIN at the next task step (don't skip).
- If a skill invocation feels redundant with manual approach, the redundancy IS the value — manual approach has unknown blind spots; skill approach has known workflow.
- Negative example: in Task 1.5, I ran `cargo test + cargo clippy + cargo fmt` and declared done. `superpowers:verification-before-completion` would have surfaced "did you check security?" — manual checklist didn't.

---

## L10 — Threat model is the answer key; read before writing code

**Trigger**: Task 1.5 (PR #23) — 4 security findings (3 HIGH + 1 MEDIUM) caught only by post-push automated review. Original `atomic_write` + `permissions` implementation copied plan §Task 1.5 reference code verbatim without re-reading the threat model spec. The plan author had omitted `0o600`, symlink rejection, and RAII cleanup from the code template; the implementation inherited those omissions.

**Rule**: Before writing any code that touches signing / keys / network / secrets / persistence / permissions:

1. **Read the threat model spec** — [`docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md`](../docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-threat-model.md) and [`rust-wallet-app/CONTEXT.md`](../../rust-wallet-app/CONTEXT.md). For each Adversary (A1-A8) and Abuse case (U1-U7), name which task defends against it.
2. **List attacker-model inputs** for every new function: symlinks? umask? race window? caller-controlled path? Does this implementation actually defend?
3. **Negative-path tests required.** For atomic_write-style code: "what if dest is symlink?", "what if parent is symlink?", "what if permission can't be read?" Each branch needs a test.
4. **Verification = Security + Threat-model coverage**, not just Correctness + Test. L9 v3 per-dimension verdict enforces this; apply it.
5. **At PR-creation time**, answer: "which A1-A8 / U1-U7 does this PR defend against?" If you can't answer, you haven't read the threat model.

**Why**: The threat model IS the answer key. Plan code templates are drafts that may omit mitigations; copying a plan that doesn't enumerate attackers produces code that doesn't defend against them. Power-loss atomicity (U7) is not the same as attacker atomicity (A2) — both are required, neither is implied by the other.

**Apply**:

- Task pickup (per-task loop step 1): list which A/U each task defends. Write this in the per-task pipeline block.
- Code-writing: for each function with security relevance, ask "attacker-controlled inputs: which?". Symlinks? Permissions? Path traversal? TOCTOU?
- Test-writing: name each branch in the function; if no test exists, write one before declaring done.
- Verification: per-dimension verdict (L9 v3) must include Security + Threat-model coverage.
- PR-creation: include "Defends against: A2, U6, U7" in the body or drift table.

**Anti-pattern (what I did)**: treat the plan's reference code as ground truth. It's a draft. Validate it.
