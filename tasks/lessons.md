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
- [L6] approval gates before persistent changes — `git commit` + remote ops; same-scope commit+push bundled in one pause (memory)
- [L8] flip issue checkboxes before squash-merge (memory)
- [L9] issue bodies = status, PR bodies = fix analysis (with table)
- [L11] scan skills list at session start, tag 3-5 relevant, invoke before doing
- [L12] code review runs BEFORE local verify gate, not after
- [L13] per-task pipeline spec (10 decisions, 2026-08-07 grill)
- [L14] ledger rule — `.superpowers/sdd/<plan>/progress.md`, update on pickup/commit/merge/grill, gitignored locally
- [L21] Update estimate-report AND ai-cost-report on every PR merge (status, progress, in-flight count, merge SHAs)
- [L24] On PR merge: update CHANGELOG.md (Keep a Changelog + User Stories table) + "Try it" column. For ≥3 sub-tasks: parent branch + sequential merge + PR-to-parent.
- [L25] Sub-task workflow for large tasks (≥3 sub-tasks: parent branch + sequential merge + PR-to-parent)
- [L46] Git-action hygiene: branch identity + drift-scan + stash audit + post-commit verify
- [L50] Harness-style work: `metaharness_oia_audit` weekly or pre-release
- [L55] Step 11 verify gate: scope `cargo test -p <crate>`>` — never `--workspace` (bitcoin-wallet-core FFI tests dominate)


> **Index gaps (L15–L20):** entries were added then trimmed during session 2026-08-10. L15/L16/L17 were `Secret<T>` / ZeroizeOnDrop / Debug patterns. L18/L19 were review findings (doc-test + merge gate). L20 was estimate-report self-improvement (replaced by client-bill pivot). All removed per user direction; rules not currently in scope.
>
> **Audit (2026-08-10):** L10, L22, L23, L27 removed per user direction. L10 (threat-model re-read) — type-system invariants + L11/L12 review pair make the rule redundant. L22 (fact-forcing gate) — enforced at hook layer, captured in `~/.claude/CLAUDE.md` global memory instead. L23 (`git stash -u -- <path>` deletes untracked) — git-native behavior, covered by `git stash` docs. L27 (grep `#[derive(...)]` before using traits) — type-checker errors surface the assumption fast enough; pre-flight grep added latency without saving compile cycles.

### Domain map

| Domain                       | Lessons                                                            |
| ---------------------------- | ------------------------------------------------------------------ |
| Build / Cargo hygiene        | L1                                                                 |
| Git workflow                 | L6 (approval gates), L8, L14, L46 (merged branch+drift+stash+verify) |
| Issue/PR protocol            | L9, L24, L25                                                       |
| Skill + review pair          | L11, L12, L13                                                      |
| Post-merge bookkeeping       | L21, L24                                                           |
| Harness-style work           | L50                                                                |
| Test scope discipline        | L55                                                                |
| Security review              | (merged into L11/L12 review pair + L13 complexity tiers)           |

---

## L1 — Workspace path consistency

**Trigger**: `c2a64b7 docs(claude): fix workspace refs` + `5943c84 fix(umbrella): post-verify polish`.

**Rule**: When project layout changes (workspace rename, crate relocation), grep all `*.md`, `*.toml`, `*.yml` for old paths in one pass. Update CLAUDE.md + plan + Cargo manifests together.

**Why**: 31 stale path references in plan file alone. Drift between docs and actual tree = wrong file edits, broken links, confused contributors.

**Apply**: After any `mkdir` / `mv` / `cargo new` inside rust-wallet-app/, run:

```bash
grep -rn "old-path" docs/ rust-wallet-app/ rust-wallet-app/crates/*/Cargo.toml
```

before commit.

---

## L6 — Approval gates before persistent changes (memory)

**Rule**: Two related approval gates — both pause + describe intent, await "approved" before executing:

1. **`git commit`** (incl. push, PR open, PR merge, branch delete) — `git commit` is irreversible; surface diff + test output, ask "commit?". Approval required for every commit (including post-merge bookkeeping).
2. **`gh`/branch ops, file moves outside `docs/`, force-push, `--admin` bypass** — remote-surface actions hard to reverse cleanly; describe intent + alternative, await approval.

**Why**: Commits + remote state mutations are immutable public artifacts. No easy undo without rewrite (commits) or coordinated rollback (remote state).

**Apply**:

- Before `git commit` → STOP. Show diff summary + test output. Ask "approved?".
- Before `gh pr merge --admin` / `--force-push` / `gh issue close` → name the bypass explicitly ("merge with --admin, approved"). The auto-mode classifier requires literal phrasing for bypass authorization.
- **Same-scope commit + push — one pause.** When proposing a commit to a feature branch with no pending push, surface BOTH actions in one prompt: *"Commit `<subject>` and push to `origin/<branch>` — approve?"* A literal "approved" / "commit" authorizes the commit AND the subsequent push to the same branch. Required: name the push target + force-flag (or "no force") in the same prompt so the approval scope is explicit.
- **Two-pause cases (override the bundling):** force-push (`--force` / `--force-with-lease`); push to `main` / release branch; push bundling multiple commits with different scopes; any `--admin` bypass or `gh pr merge --admin`; PR open after push (still requires separate approval per L6 + workflow-approval-required memory). Each of these needs its own dedicated pause.
- **Show branch name in the approval prompt.** Every commit / push / PR-open pause MUST include the checked-out branch name (`git branch --show-current`) verbatim in the prompt text — e.g. *"Commit `<subject>` on branch `rust-eth-core` and push to `origin/rust-eth-core` — approve?"* The branch name lets the reviewer confirm the destination matches intent (L46 destination check + L45 integration-branch routing). If the prompt omits the branch name, the approval is incomplete — re-prompt with the branch included.
- For post-merge bookkeeping commits (CHANGELOG/lessons) → still pause. User's "approved" message earlier in the session is for the prior action, not subsequent commits (per the never-auto-commit memory).

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

| Column          | Content                                                                                          |
| --------------- | ------------------------------------------------------------------------------------------------ |
| `Area`          | code area (`Secret`, `atomic_write`, `permissions`, etc.) — not "step N"                         |
| `Drift`         | what changed vs plan/spec                                                                        |
| `Sev`           | LOW / MEDIUM / HIGH / CRITICAL — tagged by impact, not by review-tool severity                   |
| `` File:line `` | code block (e.g. `` `keys/secret.rs:25` ``) — pin to current lines after fix                     |
| `Result`        | what was achieved after the improvement (concrete outcome)                                       |
| `Trade-off`     | explicit cost the fix imposed (perf, complexity, API surface, deps) — required per antipattern 5 |
| `Score`         | `N/10 — <handle>` — honest self-score per row, with attribution                                  |
| `Note`          | future improvements needed (or "None")                                                           |

**Apply — required PR technical-details table (v3):**

| Column                   | Content                                                               |
| ------------------------ | --------------------------------------------------------------------- |
| `Tool / Plugin`          | skill / hook / crate / stdlib function                                |
| `Role`                   | `find` (caught the issue) / `resolve` (fixed it) / `review` (audited) |
| `What it caught / fixed` | one-line summary                                                      |
| `Used at step`           | commit + file:line where applied                                      |

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

**Cross-reference**: issue-body templates (Context + Goal/Repro + Acceptance criteria + References) and the full backlog-issue creation workflow (PL22 pause-then-act, PL23 body template, PL24 two-layer labels, PL25 wayfinder, PL26 canonical create command) live in `tasks/issues-lesson.md` PL21-PL26. L9 owns the status-vs-analysis principle and PR drift-table schema; `tasks/issues-lesson.md` owns the body-template shape + `gh issue create` execution flags + label/milestone format. Read both at pickup: L9 for the schema, PL22-PL26 for the create command shape.

---

## L11 — Scan skills list at session start, tag 3-5 relevant, invoke before doing

**Trigger**: Session 2026-08-07 — 9 Matt Pocock skills (`mattpocock-skills:*`), ~15 superpowers, ~10 compound-engineering, ~10 pr-review-toolkit, ~20 compass/ecc skills were loaded. Used 3 total (karpathy-guidelines, commit-commands:commit-push-pr, pr-review-toolkit:review-pr). Said "I don't see Matt Pocock plugins" when they were loaded — only saw them after user repeated the question and SessionStart hook listed them again.

**Rule**:

1. **At every session start**, enumerate the skills list (`/skills` if listed, or the SessionStart hook output) and tag 3-5 skills that match the active task.
2. **Before starting each task step** (pickup, TDD, verify, pre-PR, post-merge), invoke the relevant skill — don't rely on manual checklist.
3. **If a skill exists for a step I'm doing manually, invoke it.** Manual checklist failure modes: blind spots, missing sub-agents, no parallel review.

**Why**: 47 skills available. Skills encode battle-tested workflows (Pocock's TDD, superpowers:verification-before-completion, pr-review-toolkit:code-review with parallel sub-agents). Each one I skip is a workflow gap. Task 1.5's 4 security findings would have been caught by `pr-review-toolkit:code-review` invoked pre-PR instead of `security-guidance` invoked post-push.

**Skill → task-step mapping** (use this as starting checklist):

| Task step                                                             | Skill to invoke first                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Task pickup (understand + plan)                                       | `mattpocock-skills:domain-modeling` if new domain; `compound-engineering:ce-plan` if multi-step                                                                                                                                                                |
| Task pickup (drift scan, per L13 step 4a)                             | `git log --all -- <path>` for every plan/spec SHA cited in the picked-up issue. Empty = drift; commit artifact or file follow-up before feature work starts.                                                                                                   |
| Task pickup (new feature, no existing plan)                           | `feature-dev:feature-dev` — 4-phase (discover → explore → clarify → architect) producing ad-hoc plan. Use when feature unclear or scope undecided; phases 1-4 → ad-hoc plan, then L13 owns from step 9 onward (implement/review/summary re-absorbed into L13). |
| Brainstorming (pre-implementation design)                             | `superpowers:brainstorming` (MUST before any creative work; gates L13 pre-pickup per L11 itself)                                                                                                                                                               |
| Workspace isolation                                                   | `superpowers:using-git-worktrees` (after brainstorming, before plan execution; integration branch per L45)                                                                                                                                                     |
| Plan authoring / plan review                                          | `tasks/plan-lesson.md` (PL1, PL2, PL3, PL7–PL16) — drift scan, story trace, plugin stack, host-first SDK design, step-by-step workflow                                                                                                                         |
| Code review / SDK quality                                             | `tasks/review-lesson.md` (PL4, PL5, PL6, PL17) — flat re-exports, async mutex, stability policy, review plugins                                                                                                                                                |
| Deep search / content review / code-block                             | `tasks/search-lesson.md` (PL18, PL19, PL20) — content review, code-block review, deep search + agent management                                                                                                                                                |
| Plan authoring (multi-step task)                                      | `superpowers:writing-plans` (after brainstorming approval, before L13 step 9 TDD)                                                                                                                                                                              |
| Plan execution (current session)                                      | `superpowers:subagent-driven-development` (default if subagents available; L13 step 5a branch)                                                                                                                                                                 |
| Plan execution (parallel session)                                     | `superpowers:executing-plans` (fallback if no subagents)                                                                                                                                                                                                       |
| TDD red-green-refactor                                                | `superpowers:test-driven-development` (post-re-evaluation; was `mattpocock-skills:tdd`)                                                                                                                                                                        |
| Build/cargo error cascade                                             | `superpowers:systematic-debugging` (post-re-evaluation; was `mattpocock-skills:diagnosing-bugs`)                                                                                                                                                               |
| Module interface design                                               | `mattpocock-skills:codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair per L13 Q4)                                                                                                                                                               |
| Behavioral discipline (every L13 step)                                | `andrej-karpathy-skills:karpathy-guidelines` — wrapper at step 4 (branch checkout) + step 15c (broad L13 audit). Per L13 behavioral discipline section (4 principles: think-first, simplicity, surgical, goal-driven).                                         |
| Pre-PR code review (comprehensive)                                    | `pr-review-toolkit:code-review` wrapped by `superpowers:requesting-code-review` (parallel sub-agents: `type-design-analyzer` + `code-reviewer` per L13 step 10). Scope: correctness, security, tests, structure.                                               |
| Pre-PR security review (critical tier, after L12)                     | `security-review` (standalone, comprehensive: secrets, SSRF, authz, trust boundaries, crypto, multi-tenancy)                                                                                                                                                   |
| Code smell / debt reported (any tier)                                 | `ecc:refactor-clean` (dead-code audit first) → per-language `*-review` (interpret findings) → `ecc:quality-gate` (formatter check)                                                                                                                             |
| Pre-commit plugin structure validation (when trigger matches per L49) | `plugin-dev:plugin-validator`                                                                                                                                                                                                                                  |
| PR review feedback (L13 step 15, 3-round fix loop)                    | `superpowers:receiving-code-review` wrapped by `pr-review-toolkit:code-review`                                                                                                                                                                                 |
| Test coverage gap analysis                                            | `pr-review-toolkit:pr-test-analyzer`                                                                                                                                                                                                                           |
| Doc / threat-model review                                             | `mattpocock-skills:domain-modeling` (re-invoke; threat model is a domain artifact; was `compound-engineering:ce-doc-review`)                                                                                                                                   |
| Document stage (per-task tech doc → PR body)                          | `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections)                                                                                                                           |
| Before declaring done                                                 | `superpowers:verification-before-completion` (L11 recommends; L13 step 11 note says "User rejected adding to L13 spec" — invoke as L11-mapped wrapper, not L13-enforced gate)                                                                                  |
| Commit + push + PR                                                    | `commit-commands:commit-push-pr`                                                                                                                                                                                                                               |
| Rust toolchain static analysis (one-shot verify)                      | `ecc:rust-build-resolver` (fmt + clippy + test + dedup + cargo audit in one invocation); slash command `/ecc:rust-build`                                                                                                                                       |
| Rust toolchain review (review-paired fmt check)                       | `ecc:rust-reviewer` or `/ecc:rust-review`                                                                                                                                                                                                                      |

> **Skill-pair wrappers (2026-08-11):** `pr-review-toolkit:code-review` is the
> toolkit; the superpowers meta-skills wrap its invocation. Pre-PR (L13 step 10)
> pairs `superpowers:requesting-code-review` with the toolkit. PR feedback
> (L13 step 15, the 3-round fix loop) pairs `superpowers:receiving-code-review`
> with the toolkit. Treat the superpowers skill as the entry point; the toolkit
> is the parallel-sub-agent driver inside it.
>
> **Project-local references** (rows for "Plan authoring", "Code review", "Deep search"): `tasks/plan-lesson.md` / `tasks/review-lesson.md` / `tasks/search-lesson.md` are project-local markdown (PL-prefixed lessons), not skills. They document blockchain-sdk-specific patterns superpowers doesn't cover. Skill format mismatch is intentional.
>
> **Subagent-driven-development vs executing-plans** (rows for "Plan execution"): pick `subagent-driven-development` if subagents are available on the harness; fall back to `executing-plans` if not. Both are mutually exclusive per task — never run both.
>
> **`mattpocock-skills:domain-modeling` re-invoke** (rows for "Task pickup" and "Doc / threat-model review"): pickup = understand the domain; doc/threat-model = re-invoke as the same lens to author artifacts. Same skill, two phases of use.
>
> **Review agents NOT in L11** (alternative lenses, use only when explicitly needed): `compass:code-reviewer`, `ecc:code-reviewer` (general, not the Rust-specific variants in the rows above), `caveman:cavecrew-reviewer` (fast diff triage, severity-tagged, terse output), `compass:rust-engineer` / `voltagent-lang:rust-engineer` (apply Rust style on first pass, no discrete fmt-check step). L11 = canonical, not exclusive — use these when the canonical mapping doesn't fit.

**Apply**:

- After every `Skill` invocation that returns useful guidance, invoke it AGAIN at the next task step (don't skip).
- If a skill invocation feels redundant with manual approach, the redundancy IS the value — manual approach has unknown blind spots; skill approach has known workflow.
- Negative example: in Task 1.5, I ran `cargo test + cargo clippy + cargo fmt` and declared done. `superpowers:verification-before-completion` would have surfaced "did you check security?" — manual checklist didn't.

---

## L12 — Code review runs BEFORE local verify gate, not after

**Trigger**: Task 1.5 (PR #23) — 4 security findings (3 HIGH + 1 MEDIUM) caught by post-push automated review. Local verify gate (`cargo fmt --check && cargo clippy -- -D warnings && cargo test`) had already declared green. Pre-PR review would have caught the gaps before squash-merge.

**Rule**: Run `pr-review-toolkit:code-review` (parallel sub-agents: `type-design-analyzer` + `code-reviewer`) on the first commit on the branch, BEFORE the local verify gate. Output drives the fix commit. Verify gate then runs on the final fix, not the first pre-review pass.

**Why**: Local verify tools (`cargo fmt`, `clippy`, `cargo test`) detect what compiles and what lints. They do not detect missing tests, wrong abstractions, or security gaps. Pre-PR review closes those gaps before merge becomes irreversible. Post-merge review = revert + hotfix cost.

**Apply**:
- First commit on branch → invoke review pair (`type-design-analyzer` + `code-reviewer`) in parallel → fix findings → THEN run verify gate → THEN PAUSE for commit.
- L13 step 10 enforces this sequence; the L12 review-before-verify gate is the same one called out by the L34-trigger precedents (security + type-design + code-reviewer parallel).
- Sub-agent lens coverage: `type-design-analyzer` (encapsulation, invariant expression, type-level soundness) + `code-reviewer` (correctness, security, convention). Run concurrently, both perspectives land at once.
- For `critical` complexity tier (per L13), add `pr-review-toolkit:security-auditor` as a third sub-agent (max 3 skills per step under Q4 carve-out).
- For `critical` complexity tier (per L13), also invoke `security-review` (standalone, after L12 review) as a separate gate. `security-review` is a comprehensive read-only pass (secrets, SSRF, authz, trust boundaries, crypto, multi-tenancy) — different lens from `pr-review-toolkit:security-auditor` which sits inside the L12 code-review pass.
- Both fire on critical-tier tasks: `pr-review-toolkit:security-auditor` inside L12 review (code-review lens), then `security-review` standalone (security-review lens). Defense in depth.
- **Lens coverage**:

  | Lens                                                  | Plugin                                   | Position                            |
  | ----------------------------------------------------- | ---------------------------------------- | ----------------------------------- |
  | Type design (encapsulation, invariants)               | `pr-review-toolkit:type-design-analyzer` | sub-agent, parallel                 |
  | Code quality (correctness, security, convention)      | `pr-review-toolkit:code-reviewer`        | sub-agent, parallel                 |
  | Security-audit (in L12 code-review lens)              | `pr-review-toolkit:security-auditor`     | sub-agent, critical tier only       |
  | Comprehensive security (secrets, SSRF, authz, crypto) | `security-review`                        | standalone gate, critical tier only |

- **Order** (defense in depth):
  1. L12 review (sub-agents: `type-design-analyzer` + `code-reviewer` [+ `security-auditor` if critical]) → fix loop on L12 findings
  2. `security-review` standalone (critical tier only) → fix loop on security findings
  3. Verify gate (cargo fmt + clippy + test per L13 step 11)
  4. Commit PAUSE (L13 step 12)
- `security-review` is read-only — produces findings; does not modify code. Apply findings via the same fix-loop as L12 review findings.
- Q4 max-3 cap unaffected: `security-review` is a separate gate, not a sub-agent in the parallel cluster. Counts as 1 skill for the next pipeline step (e.g. step 11 verify).
- The L11 mapping table's "Pre-PR review" row names `superpowers:requesting-code-review` as the entry point. The wrapper meta-skill orchestrates the toolkit sub-agents (`type-design-analyzer` + `code-reviewer`) below. Always invoke the superpowers wrapper, not the toolkit directly.

---

## L13 — Per-task pipeline spec (10 decisions, 2026-08-07 grill)

**Trigger**: Session 2026-08-07. User invoked `mattpocock-skills:grilling` to stress-test the per-task pipeline template. 10 questions, 10 decisions. Output: revised pipeline spec.

**Rule** (the spec):

```text
## Pre-pickup
1. **Invoke `mattpocock-skills:ask-matt` to route which skill fits the situation**, then L11: enumerate loaded skills; tag 3-5 relevant to active task. ask-matt output + L11 map = candidate set for step 5.
2. (Self-detect complexity) → propose "trivial / normal / critical" → user confirms. **For huge work** (multi-session, multi-PR, cross-crate scope beyond single-session hold), invoke `mattpocock-skills:wayfinder` to plan as a shared map of decision tickets on the issue tracker, then resolve one at a time until the destination is clear. Skip wayfinder for normal / trivial / critical tiers.

## Per task
3. Pick up issue. **Invoke `mattpocock-skills:triage` to categorise, verify, grill if needed, produce agent-ready brief**, then read body. Check if large task or sub-task — see [Sub-task workflow for large tasks](#sub-task-workflow-for-large-tasks) below. **If large task per L25**, invoke `mattpocock-skills:to-tickets` to decompose into tracer-bullet tickets (edges as text per ticket locally, or native blocking links on the configured tracker). **If issue body asks for design sanity-check or throwaway prototype**, invoke `mattpocock-skills:prototype` first — sanity-check state model, logic, or UI feel in a scratch branch before committing to interface design.
3a. **Spec synthesis (when no spec/plan exists for the picked-up issue):** invoke `mattpocock-skills:to-spec` — no interview, just synthesis of what has already been discussed in the conversation or issue body, published to the configured tracker. Output = spec file or issue. Resume L13 at step 4 once the spec lands. Pairs with step 5a's existing "no-plan branch" carve-out.
4. karpathy-guidelines + branch checkout (from integration branch if sub-task per step 3)
    - **L46 — record expected branch:** note the branch name just checked out (e.g., scratch, ledger per L14). Every later L46 check reads from this record.
    - **L45 — integration-branch routing:** for issues labeled `rust-eth-core` (or any future integration-branch label), fork from the integration branch (`rust-eth-core`), NOT main. PR base = integration branch. Sub-task branches name `task/<plan-slug>/<n>-<slug>`. L46's branch record + L45's routing are the two-branch gates.
4a. **Drift scan (per L13 step 4a):** before starting feature work, verify every plan/spec/SHA citation referenced by the picked-up issue. For each cited `<path>`, run `git log --all -- <path>`. Empty result = drift (artifact never committed or SHA never existed); resolve by committing the artifact or filing a follow-up issue before feature work begins. Drift is silent — cargo fmt/clippy/test don't catch it; only `git log` reveals the gap. **Extended drift check (added 2026-08-26 per L53)**: for CLI work in a multi-CLI workspace (e.g. `eth/` + `btc/` under `rust-wallet-app/crates/`), diff the equivalent helper in the sibling CLIs at pickup. Example: when adding `eth`'s `resolve_password()`, read `btc/src/handlers.rs:81-89` `password_or_prompt()` and compare argument-handling. Any divergence (empty-flag-falls-through vs silent-accept, env-var name, warning text) goes in the PR body Drift section. Cross-crate pattern divergence is invisible to TDD — only a reviewer comparing to the sibling CLI surfaces it.

## Per pipeline step
5. Step: invoke `mattpocock-skills:ask-matt` to narrow the L11-tagged candidates, then pick skill pair (max 2) from L11 map
5a. **No-plan branch:** if no plan/spec exists for the picked-up issue, defer to `feature-dev:feature-dev` instead of L13's TDD→review→verify chain. Output of feature-dev phases 1-4 = ad-hoc plan; resume L13 at step 9 (TDD) once the plan lands.
6. Skill #1: invoke
7. Skill #2: invoke (if applicable)
8. Domain-tag wins on conflict: security > correctness > simplicity

## Per task
9. TDD red-green cycle. **When spec/tickets already exist** (paired with step 3 triage + step 3 to-tickets, or step 3a to-spec), invoke `mattpocock-skills:implement` as the high-level orchestrator — wraps the red-green cycle around the spec/ticket acceptance criteria. **`superpowers:test-driven-development` stays as the lower-level driver** (failing test first, then GREEN, then refactor). `implement` does NOT replace TDD; it scopes which failing tests matter per ticket. **During the REFACTOR phase, invoke `context-engineering-kit:kaizen:kaizen`** — anti-overengineering discipline, iterative refactor with explicit minimum-change test.
    - **Skill axis winners** (per `.local/plugins-docs/2026-08-31-mattpocock-vs-superpowers.md` "Step 9 recommendations"):
        - **High-level orchestration**: `mattpocock-skills:implement` (user-invoked, one-shot driver, calls TDD + code-review).
        - **Low-level TDD discipline**: `superpowers:test-driven-development` (Iron Law, "delete code, start over", rationalisation table).
        - **Subagent fan-out**: `superpowers:subagent-driven-development` OR `/dispatching-parallel-agents` (reserve for multi-ticket plans; single-ticket overhead exceeds work).
        - **Interface design (step 9a)**: `mattpocock-skills:codebase-design` + `domain-modeling` (mattpocock-only; superpowers has no equivalent).
        - **Refactor discipline**: `context-engineering-kit:kaizen:kaizen` (anti-overengineering, iterative, minimum-change test).
        - **Code review (post-step 9)**: `mattpocock-skills:code-review` (requestor side, two-axis Standards+Spec) + `superpowers:receiving-code-review` (receiver side, anti-sycophancy).
    - **Canonical invoke order** (full table + decision tree lives in the comparison doc):
        ```text
        step 9a (optional, new module only):
          → /mattpocock-skills:codebase-design
            → /mattpocock-skills:domain-modeling

        step 9 (mandatory):
          → /mattpocock-skills:implement            # if spec + tickets exist
            OR /superpowers:test-driven-development  # if no spec/tickets yet
          inside each cycle: RED → GREEN → REFACTOR(/kaizen:kaizen)

        step 10 (after step 9):
          → /superpowers:requesting-code-review (gate)
            → /mattpocock-skills:code-review (two-axis review)

        step 11 (verify gate):
          → cargo fmt + cargo clippy --all-targets -- -D warnings + cargo test
          → /superpowers:verification-before-completion (claims gate)
        ```
    - **Anti-patterns** (top 3, full list in comparison doc):
        - Invoke `test-driven-development` without invoking `implement` first when spec/tickets exist → agent picks the wrong failing tests.
        - Invoke `implement` without invoking `test-driven-development` per cycle → writes code first = breaks Iron Law.
        - Skip step 9a for new modules → interface designed under TDD pressure = scope creep + drift.
    - **Cross-reference**: full decision tree + axis table + anti-patterns in `.local/plugins-docs/2026-08-31-mattpocock-vs-superpowers.md` "Step 9 recommendations (L13 implementation step)" section.
9a. **Module interface design (before TDD when new module/struct):** invoke `mattpocock-skills:codebase-design` to author the module's public surface (struct fields, trait bounds, error type, async signature) BEFORE writing the failing test. **If the interface decision warrants durable record** (new error type, breaking public API, cross-crate contract), also invoke `mattpocock-skills:grill-with-docs` — the grill interview sharpens the design while emitting an ADR + glossary entry as byproducts. **Pair with `mattpocock-skills:domain-modeling`** when introducing new domain terms or editing CONTEXT.md/ADR — glossary entries land at first use, preventing naming drift. Pairs with `pr-review-toolkit:type-design-analyzer` (which fires later in step 10 L12 review). Skip for trivial edits to existing modules; mandatory for new public types per L12 module-interface row.
    - **Cross-reference**: `mattpocock-skills:grill-with-docs` SKILL body in `.local/plugins-docs/2026-08-31-mattpocock-skills-deepdive.md` (states working-dir requirement, side-by-side with `grill-me`, decision-tree mechanics).
10. L12: pre-PR code review FIRST — `superpowers:requesting-code-review` wrapping `pr-review-toolkit:code-review`
    - Parallel sub-agents: `type-design-analyzer` (encapsulation, invariants) + `code-reviewer` (correctness, security, tests, structure per L11 row scope)
    - **Critical tier** (per Q4 carve-out): add `pr-review-toolkit:security-auditor` as 3rd concurrent sub-agent (max 3 sub-agents per step). triggers for key material / signing / encryption / network / persistence surfaces.
    - **Fallback when named skill is unavailable in active harness** (added 2026-08-26 per L53): if `pr-review-toolkit:security-auditor` (or `type-design-analyzer`, `code-reviewer`) is not in the active harness's agent registry, substitute the closest equivalent — `compass:security-auditor` for the security lens, `ecc:security-reviewer` or `ecc:type-design-analyzer` as alternates. Document the fallback in the PR body + `lessons.md` deviation note. Do NOT skip the lens entirely — fall back, don't skip.
    - **Trivial tier** (per L13 amendment note 2026-08-25): SKIP this step entirely — pre-PR code review not required for doc-only commits. L49 + L51 + L52 + L24 still apply.
    - **Convergent-finding rule** (added 2026-08-26 per L53): when 2+ L12 sub-agents surface the same finding with different lenses (e.g. type-design-analyzer + code-reviewer both flag the same `map_err(|_|)` anti-pattern), fix in ONE pass before the verify gate — don't split fixes across multiple Q5 rounds. Convergent findings are high-confidence real bugs; a single fix addresses both lenses.
    - Run on squash-candidate state (final commit on PR branch before merge), not first commit and not uncommitted. Per Q8 (re-grill 2026-08-15: corrected from "first commit" wording — Tasks 3-4 squash-merged multiple commits; reviewers read the combined state).
    - **Q4 budget**: max 2 sub-agents normal tier; max 3 critical tier (carve-out for L12 cluster). Standalone gates (10a, 10b) don't compound with the cluster cap.
    - **Review-paired fmt re-check** (after L12 review, pre-step 11): invoke `/ecc:rust-review` slash command (or `ecc:rust-reviewer` agent) on modified `.rs` files to catch rustfmt drift the L12 sub-agents missed. Different lens from the cargo quad gate (which runs pre-commit) — this runs post-L12. L11 row "Rust toolchain review" consumer. Skipped for trivial + doc-only edits.
10a. **Test coverage gap analysis (separate gate after L12, all tiers):** invoke `pr-review-toolkit:pr-test-analyzer` on the same squash-candidate state. Distinct lens from `code-reviewer` (which checks existing tests for correctness); `pr-test-analyzer` checks for missing coverage on the changed behavior. Findings drive a follow-up commit before step 11 verify. Not concurrent with L12 sub-agents (separate gate) — Q4 cap preserved.
10b. **Pre-PR security review (critical tier only, standalone):** after step 10a (test coverage), invoke `security-review` (comprehensive: secrets, SSRF, authz, trust boundaries, crypto, multi-tenancy). Distinct lens from `pr-review-toolkit:security-auditor` (which sits inside L12 code-review lens). Q4 cap unaffected (separate gate, not sub-agent). Findings drive a follow-up commit before step 11 verify. Skipped for normal + trivial tiers.
10c. **Standards + Spec review (separate gate after step 10b):** invoke `mattpocock-skills:code-review` on the same squash-candidate state. Distinct lens from pr-review-toolkit (which checks correctness/security/tests/structure) and from security-review (which checks secrets/SSRF/authz/crypto). Standards axis = repo coding conventions (formatting, naming, error patterns). Spec axis = does the code match the originating issue. Parallel sub-agents per the skill's design. Findings drive a follow-up commit before step 11 verify. Q4 cap unaffected (separate gate, not sub-agent). Skipped for trivial tier.
11. Verify (double gate local + CI dedup): `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` (+ `cargo audit` if installed, per L11 row). Skip `cargo test --workspace --all-targets` here — run `cargo test -p <touched-crate> [-p <touched-crate> ...]` instead (L55).
    - Run BEFORE every commit (initial + fix + task-end) — earlier "AFTER each fix commit" wording replaced for consistency with clippy sub-bullet below.
    - All local gates (+ audit if installed) must pass before the task-end commit. A single failing gate = task is not done; on failure → step 11a (triage) or step 11b (debug fallback) BEFORE re-running.
    - **CI 4th gate (`cargo tree --workspace --duplicates`)**: runs in `.github/workflows/<file>` (rust-eth-core-ci.yml per L45 integration-branch routing), NOT in the local per-commit loop. Dedup is cheap but workspace-wide tree walks slow on large workspaces; CI cadence is the right place. Local step 11 = double gate only. Step 11 snapshot row covers CI dedup status separately.
    - **One-shot path**: prefer `/ecc:rust-build` slash command (wraps the local gates in one invocation per L11 row). Use bare cargo commands when the slash command is unavailable or for finer-grained debugging.
    - **Flaky-test retry (CI-side)**: if CI `cargo test` fails on a single test that previously passed (no code change in that test path), re-run the failed CI job with `--test-threads=1` (or rerun workflow). Persistent failure = step 11b. Local step 11 does not run `cargo test` — CI is the test gate.
    - **Trivial tier** (per L13 amendment note 2026-08-25): cargo gate N/A for doc-only commits. L49 + L51 + L52 + L24 still apply.
    - **Q4 budget**: N/A (no skill invoked; cargo commands only).
    - **`cargo fmt --check` is a blocking gate**, not a convenience. Run it BEFORE every commit; CI's `Format check` job fails the PR if any line exceeds rustfmt's max-width.
    - **rustfmt version drift (PR #137, 2026-08-14) — local pass ≠ CI pass.**
        - Local `cargo fmt --check` is necessary but not sufficient. CI's pre-installed rustfmt may differ from the project's `rust-toolchain.toml` channel (e.g. CI installs rustfmt from `dtolnay/rust-toolchain@stable` while project pins `1.94`).
        - **Diagnostic** when CI Format check fails but local fmt passes:
            ```bash
            # Compare local vs CI rustfmt versions
            cargo fmt --version              # local
            # CI's rustfmt appears in the failed job's log header
            ```
            Version mismatch = drift (safe to apply `cargo fmt --all`); version match = real format error (needs investigation).
        - **Permanent fix (one-time CI config):** pin the workflow's rustfmt to match the project's pinned channel. In `.github/workflows/ci.yml` (or btc-cli-demo.yml), change the rustfmt job to use the project toolchain:
            ```yaml
            - uses: dtolnay/rust-toolchain@<channel>   # match rust-toolchain.toml
              with:
                toolchain: <channel>                  # e.g. 1.94
                components: rustfmt, clippy, rust-src
            ```
            Verify after change: PR's Format check log should show `rustfmt <version>` matching local.
        - **Workaround when workflow isn't yet pinned:** if CI fails Format check despite local pass, run `cargo fmt --all` and commit the diff — that's the format CI is asking for.
    - **Post-bulk-edit caveat (PR #85):** the Edit-tool hook auto-formats on save, but bulk-script edits (Python `cat <<EOF` / `sed` / `git checkout --`) bypass the hook. After ANY non-Edit-tool change to a `.rs` file, run `cargo fmt -p <crate>` explicitly before the verify gate. The hook is a safety net, not a guarantee.
    - **`cargo clippy --workspace --all-targets -- -D warnings` is a hard gate**, not advisory (PR #144, 2026-08-15). Skipping L12 review to ship faster lets clippy debt accumulate — `needless_question_mark` + `unnecessary cast` + `unused import` were all flagged on a single round-1 PR. Run the full triple gate (`fmt` + `clippy --all-targets` + `test`) locally before every commit, even when L12 review is skipped for pace. `cargo clippy --workspace` alone (without `--all-targets`) misses test-code + examples + bench lints.
    - **No hardcode in production; test only**: hardcoded literals (URLs, paths, IPs, credentials) belong in `#[cfg(test)]` blocks only. Production routes through `WalletConfig` (or equivalent named config). Test fixtures are exceptions, not defects.
    - *Note*: L11 recommends also invoking `superpowers:verification-before-completion` at this step. User rejected adding it to L13 (2026-08-07) — L11 mapping still recommends it; L13 spec stays literal. If invoking it, do so as a wrapper around the cargo commands, not as a replacement.
11a. **Backlog triage** (when verify surfaces an error that can't be fixed in-task). Sequence: step 11c (systematic-debugging, conditional — see trigger below) → step 11a (triage decision) → either fix-and-rerun-11 OR create-backlog-item.
    - **Step 11c trigger condition**: invoke `superpowers:systematic-debugging` BEFORE the triage decision ONLY when (a) root cause is not immediately visible from the error message, OR (b) ≥2 fix attempts have already failed. For obvious errors (typo, missing import, single-line fix), skip step 11c and go straight to the triage taxonomy below. Performance-regression shapes (slow build, OOM, latency spike) → `mattpocock-skills:diagnosing-bugs` instead (per step 11c fallback).
    - **Triage classes** (7, with deterministic decision criteria):
        - **Fixable now**: ≤10 min + touches only files in the current PR's changed set (verify via `git diff --name-only <base>..HEAD` against the step-4 branch record) + no new test required → fix in current commit, re-verify, continue. "PR-changed-files scope" replaces fuzzy "in scope."
        - **In-PR follow-up**: >10 min OR scope-creep risk → commit in current PR (before merge), not main yet; lands via the feature PR's pipeline
        - **Small deferred** (cosmetic, follow-up): touches adjacent code OR needs new test but doesn't block → log in current session's backlogs list + L14 progress.md events. Sub-classes: `test gap` (missing coverage for existing behavior), `doc follow-up` (docstring, README, or comment drift).
        - **Big task** (multi-PR, multi-week): own PR OR multi-week OR cross-crate → create GitHub issue, label `backlog`, link to parent task. **Foggy sub-trigger**: when the big task is multi-week AND scope isn't clear, invoke `/mattpocock-skills:wayfinder` first (chart the decision tickets on the tracker), THEN collapse the map at `/to-spec` and create the backlog issue from the spec. Don't create a mega-issue from fog — the maintainer can't pick up a foggy single issue.
        - **Code smell / debt** (knip / depcheck / dead-code finding from L12 sub-agent or `refactor-clean` audit): ≤10 min + PR-changed-files scope + no new test → fixable now; touches adjacent code OR scope-creep risk → small deferred with `refactor-clean` audit as acceptance criteria; cross-crate OR multi-PR → big task with backlog issue + parent task ref. Sub-class: `dep update` (cargo update / breaking-API triage).
        - **Future milestone** (v0.1.1, v0.2): doesn't ship before parent task's release → log with `priority/p2` or `priority/p3` tag (see priority decision tree below).
        - **External gate** (operator-driven, L29 manual smoke / L28 Gate B): can't run in CI → mark `[ ]` in PR body with `<!-- TODO: <operator-action> -->` deferral note (per step 14 external-gate discipline)
    - **Decision criteria** (deterministic, not vibes):
        - ≤10 min + PR-changed-files scope + no new test → fixable now
        - >10 min OR scope-creep → in-PR follow-up
        - needs new test OR adjacent code → small deferred
        - multi-week OR cross-crate AND scope clear → big task (backlog issue)
        - multi-week AND scope foggy → wayfinder first, then big task (issue from the spec)
        - doesn't ship before parent release → future milestone
        - operator-driven (L29 / L28 Gate B) → external gate
    - **GitHub issue format**:
        - Title: `Backlog: <short description>`
        - Body: acceptance criteria + priority + parent task ref + L14 progress.md link
        - Labels: `backlog` + `priority/p0|p1|p2|p3` + `week/N` (current sprint) — canonical reference `docs/agents/triage-labels.md`
        - Milestone: parent task's milestone
    - **Priority decision tree** (aligned with `Future milestone` class, ship-vs-post-cutoff split):
        - `priority/p0`: blocks release (ship-stopper; surfaces immediately, fix before any merge)
        - `priority/p1`: blocks merge (must-fix before parent task closes; doesn't block release but blocks the parent PR's merge)
        - `priority/p2`: current milestone (ships before next minor release; example: v0.1 → v0.1.x patch series)
        - `priority/p3`: post-current milestone (v0.1.1, v0.2 backlog; doesn't ship before the next minor cut)
        - **Default when unsure**: `priority/p3`. Lower-priority items are easier to triage out of a sprint than higher; error on the side of "future."
    - **`parent task ref`**: per plan-based work = `#<plan-task-N>` (e.g., #17 for eth-wallet-core task 17); per issue-based work = `#<original-issue>`. State which in the body.
    - **`week/N`**: current sprint number. If sprint unknown, omit label.
    - **L14 ledger cross-ref**: append backlog events to `progress.md` (L14) — `id, decision, class, parent_task_ref, deferred_from_step`. Ephemeral session backlogs list = index; L14 progress.md = durable record.
    - **Backlog size signal**: >10 open `backlog` issues = tech-debt alert (per L14 progress metrics). Surface at session start: `gh issue list --label backlog --state open | wc -l`.
    - **L21 bulk-creation**: when multiple related backlogs surface (e.g., per L29 smoke for each crate), dispatch L21 sub-agent to batch-create per L21 sub-section prompt template. Reduces per-issue ceremony from 30-60s to 5-10s per item.
    - **Anti-patterns**:
        - **Scope creep**: pulling unrelated fixes into "fixable now" — violates L13 karpathy-guidelines principle 3 (surgical changes)
        - **Issue spam**: one big task → many tiny issues — audit-trail noise
        - **Orphan link**: backlog issue without `parent task ref` — drift on follow-up
        - **Eternal defer**: same item in backlog across >3 sessions — either ship or escalate to `priority/p0`
    - When in doubt: write the issue. Forgetting backlogs costs more than the 30-60s to file one.
    - **Cross-reference**: full backlog-issue creation playbook (deterministic decision tree, GitHub issue template, priority ladder, anti-patterns) lives in `tasks/issues-lesson.md` PL26. This step 11a owns the rules; PL26 owns the workflow + `gh issue create` flags + label/milestone format. Read both at pickup: this section for the canonical decision logic, PL26 for the execution template.
    - **Workflow 1 (Direct) execution discipline** (per `.local/plugins-docs/2026-08-31-gh-issue-creation-guide.md` + `tasks/issues-lesson.md` PL22-PL25): backlog-issue creation is workflow 1 in the GH guide. Apply these lessons at issue-creation time:
        - **PL22 pause-then-act**: state facts (title, body source, labels, assignee, milestone, parent task ref) → wait explicit approval → run `gh issue create --body-file /tmp/issue-body.md` (use `--body-file` for any destructive-prose body, GateGuard rejects inline `rm`/`rmdir`) → report URL.
        - **PL23 body template**: feature or bug variant. Always include Context + Goal/Repro + Acceptance criteria + References. Backlog issues = feature template with explicit parent task ref (`Refs #<n>`).
        - **PL24 two-layer labels**: layer 1 triage role (omit for backlog — backlog issue already triaged) + layer 2 chain (`polygon-core` / `rust-eth-core` / `rust-btc-core` / `rust-tron-core` / `rust-evm-core`) + priority ladder (`priority/p0|p1|p2|p3`) + `week/N` + `backlog` + `task`. Multi-axis filterable.
        - **PL25 wayfinder mechanics**: irrelevant for backlog issues (backlog ≠ wayfinder decision ticket). Applies only if the big-task class surfaces a foggy multi-week effort that itself needs `/wayfinder` to chart.
        - **Link to parent + parent milestone**: `Refs #<parent>` in body; `--milestone "<parent task milestone>"` on the create command. Orphan backlog items lose context on follow-up.
    - **Canonical backlog-issue create command** (synthesised from PL26 + GH guide workflow 1 + PL22-PL24):
        ```bash
        gh issue create \
          --title "Backlog: <short description>" \
          --label "backlog,priority/p<N>,week/N,<chain>,task" \
          --milestone "<parent task milestone>" \
          --body-file /tmp/backlog-body.md
        ```
        Body file written via heredoc with `<<'EOF'` per PL22; draft includes Context + Acceptance + Priority + Parent task ref + L14 progress link per PL23 + PL26.
    - **Cross-reference**: full `gh issue create` workflow + flags + pitfalls + anti-patterns in `.local/plugins-docs/2026-08-31-gh-issue-creation-guide.md` (workflow 1 = direct issue create; workflows 2-5 for higher-effort work, not applicable to single backlog-item creation).
11b. **L24 cascade on local branch (pre-commit):** before step 12 PAUSE, confirm L24 doc updates have landed in the commits traveling with the feature PR — CHANGELOG `[Unreleased]` bullet cites the PR number; User Stories table checkbox flipped if a story completes; "Try it" command column populated. Per L24, these live WITH the feature commit (not a separate process branch) so squash-merge carries them. If step 15a (tech doc) lands AFTER this check, re-run L24 cross-check before merge.
11c. **Systematic-debugging fallback (when verify fails non-obviously):** if `cargo test` or `cargo clippy` surfaces a failure whose root cause isn't immediate from the error message, invoke `superpowers:systematic-debugging` BEFORE proposing a fix. Forms hypothesis, proves it, then minimal root-cause change + regression test. Avoid the "guess + cargo test loop" anti-pattern. Conditional — not a per-step add (Q4 cap preserved). **Performance-regression shape** (slow build, OOM, latency spike, throughput cliff): prefer `mattpocock-skills:diagnosing-bugs` as the primary fallback. **Unknown-root-cause needing primary-source facts** (upstream crate bug, framework behavior, protocol interpretation): invoke `mattpocock-skills:research` first to gather authoritative docs before systematic-debugging forms a hypothesis. **Error deep in execution call stack** (bug origin far from symptom): invoke `context-engineering-kit:kaizen:root-cause-tracing` — traces backward, adds instrumentation when needed. **Symptom needing fundamentals drill**: invoke `context-engineering-kit:kaizen:why` — iterative Five Whys. Q4 cap preserved (each skill is a separate invocation, not a sub-agent in the step-10 cluster).
11d. **Plugin-structure validation (when trigger matches per L49):** if the commit touches any plugin-structure file, invoke `plugin-dev:plugin-validator` BEFORE the step 12 commit PAUSE. Read-only agent; findings feed the same fix loop as L12 review.
    - **Format-verification plugin** (2026-08-12 grill): the `cargo fmt --check` gate is the only Rust-quality check bundled into a dedicated plugin. Subagent `ecc:rust-build-resolver` runs `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` + `cargo tree --duplicates` (+ `cargo audit` if installed) in one invocation; slash command `/ecc:rust-build` wraps the same agent. `ecc:rust-reviewer` (or `/ecc:rust-review`) runs the same fmt check on modified `.rs` files after a code-review pass. Other Rust-engineer agents (`compass:rust-engineer`, `voltagent-lang:rust-engineer`) apply style by writing idiomatic code on first pass — they do NOT expose a discrete `cargo fmt --check` step. `caveman:cavecrew-reviewer` intentionally skips formatting nits unless they change meaning — wrong tool for rustfmt policing. Use `/ecc:rust-build` for one-shot verify; use `/ecc:rust-review` for fmt-check paired with review.
12. PAUSE for commit approval
    - Max 3 fix rounds; round = one review + one fix commit pair
    - **L46 — pre-pause destination check:** run `git branch --show-current` and confirm it equals the branch recorded at step 4. If mismatch → `git checkout <expected>`, re-verify, then proceed. The branch name MUST appear verbatim in the approval prompt per L6 ("Show branch name in the approval prompt").
13. commit-commands:commit-push-pr
    - **L46 — pre-execute destination re-check:** run `git branch --show-current` again immediately before invoking `commit-push-pr`. Even if step 12's check passed, HEAD may have moved (post-merge housekeeping, `git checkout -`, IDE tab switch). Mismatch → STOP, re-checkout expected branch, then proceed. Pair with L42 (`git diff --cached --stat`): L42 audits content, L46 audits destination, both at the same gate.
    - **L51 — resolve merge conflict:** if `git push` (or the step-4 rebase from integration branch) fails with conflicts, STOP commit-push-pr; invoke `mattpocock-skills:resolving-merge-conflicts` (canonical flow: read markers, decide ours/theirs/combine, re-run step 11 triple gate after resolution, resume push). Do NOT manually hand-edit conflict markers without the skill — skipping the flow risks silent content loss and missing test regressions.
    - **`[skip ci]` for .md-only commits** (added 2026-08-28 per user rule): before invoking `commit-push-pr`, run `git diff --cached --name-only`. If every staged path ends in `.md`, append `[skip ci]` to the commit subject (GitHub Actions treats `[skip ci]`, `[skip ci]`, `[no ci]`, `[ci skip]` as no-op markers — Rust CI gates are irrelevant for markdown diffs). If any staged path is not `.md` (`.rs` / `.toml` / `.yml` / `.json` / etc.), do NOT add `[skip ci]` — CI must run. Pattern:
        ```bash
        if git diff --cached --name-only | grep -qv '\.md$'; then
          git commit -m "<subject>"            # CI runs normally
        else
          git commit -m "<subject> [skip ci]"  # doc-only — CI skipped
        fi
        ```
        Pair with L42 (verify staged) + L51 (post-commit `git show --stat` confirms marker + scope). Anti-patterns: adding `[skip ci]` to a non-.md commit (bypasses required checks; PR cannot merge red); omitting `[skip ci]` on a pure .md commit (wastes CI minutes).
14. **Flip issue checkboxes [ ]→[x] — only after verifying each box is actually completed** (before PR open, after commit-push-pr):
    - **Walk the issue body before flipping** — for every `[ ]` box, confirm the acceptance criterion is actually met in the committed code (test passes, doc landed, dependency merged, etc.). Per L28 (verify-before-claim) + L24 anti-pattern "Flipping the box speculatively before the merge" — every flip must be backed by a verifiable artifact, not intent.
    - **One-by-one audit** — read each checkbox, name the artifact that satisfies it (test name, file:line, commit SHA, PR number), then flip. Bulk-flipping without per-box evidence is the failure mode.
    - **What "completed" means per L24:**
        - **Code acceptance** (`X implemented`, `Y tested`) → run the test, paste the passing assertion.
        - **Doc acceptance** (`CHANGELOG updated`, `User Stories table row flipped`) → grep the file, confirm the line landed in the committed code (not just the working tree).
        - **Dependency acceptance** (`#N merged first`) → `git log origin/main | grep <#N commit subject>` to confirm.
        - **External gate acceptance** (`L29 manual smoke`, `L28 Gate B macOS verification`) → KEEP UNCHECKED with a `<!-- TODO: <operator-action> -->` comment. These are operator-driven; mark `[x]` after the operator confirms, not before.
    - **Edit issue body via `gh issue edit N --body "<full body with [x] marks>"`** (single command, not per-line) — preserves the rest of the body verbatim and produces one audit-trail entry.
    - **Cross-check with the ledger** (L14) — the ledger's `progress.md` events table should match the issue body's checked boxes. Discrepancy = audit drift; resolve before opening PR.
    - **Anti-patterns:**
        - Flipping all boxes preemptively ("I'll do them later") — Gate A violation per L28.
        - Bulk-flipping without per-box evidence — audit trail collapses.
        - Flipping external-gate boxes (L29, L28 Gate B) before operator confirmation — false-positive completion.
        - Editing the issue body incrementally via partial updates — risks body drift vs commit reality.
15. PR review (parallel sub-agents) — `superpowers:receiving-code-review` wrapping the toolkit
    - If stuck 3 rounds: PAUSE then revert-to-last-green + follow-up issue + ledger entry
    - **Pipeline status snapshot** (render before PR review starts; mirrors L13 steps 1-15b with skill + plugin + status; gate decision = snapshot completeness):

        | Step                     | Skill invoked                                                                                                                                                                                                                                                                                                                                                                          | Plugin / Tool                                                                                                                                        | Status |
        | ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
        | 1 L11 enumerate          | `mattpocock-skills:ask-matt` (router) + L11 skill→step mapping table                                                                                                                                                                                                                                                                                                                   | `mattpocock-skills`                                                                                                                                  | ☐      |
        | 2 complexity tier        | self-detect (trivial / normal / critical) + user confirm; `mattpocock-skills:wayfinder` if huge-work (multi-session, multi-PR)                                                                                                                                                                                                                                                         | `mattpocock-skills`                                                                                                                                  | ☐      |
        | 3 issue pickup           | `mattpocock-skills:triage` (categorise → verify → grill → brief) + `gh issue view` + checklist parse; `mattpocock-skills:to-tickets` if large task per L25; `mattpocock-skills:prototype` if issue asks for design sanity-check / throwaway prototype                                                                                                                                  | `mattpocock-skills`                                                                                                                                  | ☐      |
        | 3a spec synthesis        | `mattpocock-skills:to-spec` (no interview; synthesis → publish to tracker) when no spec/plan exists                                                                                                                                                                                                                                                                                    | `mattpocock-skills`                                                                                                                                  | ☐      |
        | 4 branch checkout        | `superpowers:using-git-worktrees`; `andrej-karpathy-skills:karpathy-guidelines` wrapper                                                                                                                                                                                                                                                                                                | —                                                                                                                                                    | ☐      |
        | 4a drift scan            | `git log --all -- <path>` (per L13 step 4a)                                                                                                                                                                                                                                                                                                                                            | —                                                                                                                                                    | ☐      |
        | 5-8 skill pair           | `mattpocock-skills:ask-matt` (narrow candidates) + per L11 row + Q4 cap (max 2 normal, max 3 critical L12 cluster)                                                                                                                                                                                                                                                                     | `mattpocock-skills`                                                                                                                                  | ☐      |
        | 9 TDD red-green          | `mattpocock-skills:implement` (high-level orchestrator when spec/tickets exist) wrapping `superpowers:test-driven-development` (lower-level driver); `context-engineering-kit:kaizen:kaizen` during REFACTOR phase (anti-overengineering)                                                                                                                                              | `mattpocock-skills` + `superpowers` + `context-engineering-kit`                                                                                      | ☐      |
        | 9a module interface      | `mattpocock-skills:codebase-design` (new public types only); `mattpocock-skills:grill-with-docs` if interface decision warrants ADR (new error type, breaking API, cross-crate contract); `mattpocock-skills:domain-modeling` if new domain terms / CONTEXT.md / ADR edits                                                                                                             | `mattpocock-skills`                                                                                                                                  | ☐      |
        | 10 L12 review            | `superpowers:requesting-code-review` wrapping `pr-review-toolkit:code-review`                                                                                                                                                                                                                                                                                                          | `pr-review-toolkit` (`type-design-analyzer` + `code-reviewer`; critical: +`security-auditor`); `/ecc:rust-review` review-paired fmt re-check (`ecc`) | ☐      |
        | 10a test coverage        | `pr-review-toolkit:pr-test-analyzer` (separate gate)                                                                                                                                                                                                                                                                                                                                   | `pr-review-toolkit`                                                                                                                                  | ☐      |
        | 10b security-review      | `security-review` (critical tier only, standalone)                                                                                                                                                                                                                                                                                                                                     | `security`                                                                                                                                           | ☐      |
        | 10c standards + spec     | `mattpocock-skills:code-review` (separate gate; Standards + Spec axes)                                                                                                                                                                                                                                                                                                                 | `mattpocock-skills`                                                                                                                                  | ☐      |
        | 11 triple gate (local)   | prefer `/ecc:rust-build`; bare cargo `fmt --check --workspace` + `clippy --workspace --all-targets -- -D warnings` + `test --workspace --all-targets` (+ `cargo audit` if installed)                                                                                                                                                                                                   | `ecc:rust-build-resolver`                                                                                                                            | ☐      |
        | 11-ci dedup              | `cargo tree --workspace --duplicates` runs in `.github/workflows/rust-eth-core-ci.yml` (CI only, per L45)                                                                                                                                                                                                                                                                              | —                                                                                                                                                    | ☐      |
        | 11a backlog triage       | `gh issue create` (multi-PR deferred) or in-session backlogs list                                                                                                                                                                                                                                                                                                                      | —                                                                                                                                                    | ☐      |
        | 11b L24 cascade local    | CHANGELOG `[Unreleased]` + User Stories flip + "Try it" column (project convention, not skill)                                                                                                                                                                                                                                                                                         | —                                                                                                                                                    | ☐      |
        | 11c systematic-debugging | `superpowers:systematic-debugging` (conditional on verify failure); `mattpocock-skills:diagnosing-bugs` for perf-regression shape; `mattpocock-skills:research` for unknown-root-cause needing primary-source facts; `context-engineering-kit:kaizen:root-cause-tracing` for errors deep in execution call stack; `context-engineering-kit:kaizen:why` for symptom fundamentals drill  | `superpowers` + `mattpocock-skills` + `context-engineering-kit`                                                                                      | ☐      |
        | 11d plugin structure     | `plugin-dev:plugin-validator` (per L49 trigger match)                                                                                                                                                                                                                                                                                                                                  | `plugin-dev`                                                                                                                                         | ☐      |
        | 12 PAUSE                 | manual gate per L6 + workflow-approval-required memory                                                                                                                                                                                                                                                                                                                                 | —                                                                                                                                                    | ☐      |
        | 13 commit-push-pr        | `commit-commands:commit-push-pr`; `mattpocock-skills:resolving-merge-conflicts` if push fails with conflict (L51)                                                                                                                                                                                                                                                                      | `commit-commands` + `mattpocock-skills`                                                                                                              | ☐      |
        | 14 flip checkboxes       | `gh issue edit N --body "<full body with [x] marks>"` (per step 14 evidence format: file:line, test name, commit SHA, PR number)                                                                                                                                                                                                                                                       | —                                                                                                                                                    | ☐      |
        | 15 PR review             | `superpowers:receiving-code-review` wrapping `pr-review-toolkit:code-review`                                                                                                                                                                                                                                                                                                           | `superpowers` + `pr-review-toolkit`                                                                                                                  | ☐      |
        | 15a tech doc             | `mattpocock-skills:grill-with-docs` (Goal/Drift/Tradeoff sharpening + ADR emission) + `compass:docs-writer` (primary, 10-section doc) + `compass:api-designer` (secondary, API surface + Drift sections); `mattpocock-skills:domain-modeling` for glossary emission during tech-doc write; `anthropics/skills:frontend-design` if wallet-desktop files in PR diff (structural UI lens) | `mattpocock-skills` + `compass` + `anthropics`                                                                                                       | ☐      |
        | 15b L24 verify merged    | (project convention, not skill)                                                                                                                                                                                                                                                                                                                                                        | —                                                                                                                                                    | ☐      |

    - **Snapshot discipline**: render table at PR review start. Fill ☐ → ✓ as evidence lands (file:line, test name, commit SHA, PR number per step 14 format). Gate decision = all ✓ (proceed to 15c walk → 15d merge); any ☐ = fix + re-run. Per L28 (verify-before-claim).
    - **Triggers / skips** (apply when matching):
        - Step 4 wrapper `karpathy-guidelines`: invoked at every L13 step (L13 behavioral discipline); 4 principles visible in commit history.
        - Step 9a skipped for trivial edits to existing modules; mandatory for new public types.
        - Step 10 critical tier: +`security-auditor` sub-agent (Q4 carve-out, max 3 L12 cluster). Triggers for key material / signing / encryption / network / persistence.
        - Step 10 trivial tier: SKIP entire step 10 cluster (per L13 amendment note 2026-08-25).
        - Step 10b critical tier only.
        - Step 11 trivial tier: cargo quad gate N/A (doc-only commits); L49 + L51 + L52 + L24 still apply.
        - Step 11c conditional on non-obvious verify failure only (not per-step add).
        - Step 11d conditional on L49 trigger match (plugin-structure file touched).
15a. **Write technical document → enrich PR body** (before merge):
    - **Invoke `mattpocock-skills:grill-with-docs` to sharpen the Goal/Drift/Tradeoff sections + emit any decision-bearing ADRs as byproducts**, then write the 10-section doc
    - 10 sections: Goal, Drift from plan, API surface, Threat-model coverage, Implementation, Tests, L12 review, Lessons captured, Backlog (links to `backlog` issues), Migration notes
    - Append/replace existing PR body with the full doc
    - Document lives with the commit (audit trail); no separate file to maintain
    - Skill-tag pair (per L11; Document stage of the 6-stage pipeline): `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections), with `mattpocock-skills:grill-with-docs` as ADR-emission shim. **For wallet-desktop PRs** (files in `wallet-desktop/web/` or `wallet-desktop/native/`), also invoke `anthropics/skills:frontend-design` (structural UI patterns lens, sister to taste-skill's `design-taste-frontend`).
    - **Cross-reference**: `mattpocock-skills:grill-with-docs` SKILL body in `.local/plugins-docs/2026-08-31-mattpocock-skills-deepdive.md` (states Goal/Drift/Tradeoff sharpening + ADR emission contract).
    - **Verification before claim** (per `superpowers:verification-before-completion`): before declaring the doc done, run `git diff <base>..HEAD` and confirm every claim in the 10 sections is grounded in cited code or docs at the paths shown. Iron Law analog for the doc: NO DOCUMENTATION CLAIMS WITHOUT CITED EVIDENCE. Surface mismatches (claim in doc but no diff match) before merge — the reviewer catches drift; the author prevents it.
    - **Doc-as-code review gate**: after writing, invoke `mattpocock-skills:code-review` on the doc itself. Standards axis = doc style (clarity, structure, audience-appropriateness); Spec axis = matches the originating issue's intent. Two-axis review treats docs as code. Skip for trivial doc-only commits (single typo, one-line clarification).
    - **Review/receive split for the doc** (per `superpowers:requesting-code-review` + `superpowers:receiving-code-review`): when the doc receives reviewer feedback, invoke `receiving-code-review` for technical-rigor + verification-not-performance-agreement discipline. Doc review feedback is high-volume and high-bias-prone (reviewers push for brevity, find nits, miss bigger gaps). Verify each piece of feedback against the actual diff before editing.
    - **Anti-patterns:**
        - Writing the doc without verifying the diff (no evidence per claim) = verification-before-completion violation.
        - Skipping `code-review` on the doc = ship doc that's wrong; review is a gate, not a hint.
        - Agreeing with doc reviewer without verification = anti-sycophancy violation (per `receiving-code-review`).
        - Designing the PR body in a separate file instead of in the commit message = audit-trail drift (per step 15a "Document lives with the commit").
    - **Attach research + reference links in PR body**: every PR body MUST include a `## References` section linking every `.local/plugins-docs/` file + `tasks/<topic>-lesson.md` entry + `tasks/lessons.md` amendment whose decisions this PR implements. The PR body is the audit trail; references tie the change to its reasoning. Without references, future readers cannot trace WHY this change was made.
        - **Plugin deep-dive references** (cite when PR touches the corresponding plugin area):
            - `.local/plugins-docs/2026-08-31-mattpocock-skills-deepdive.md` — PRs that install/configure `mattpocock-skills` or invoke its skills (grill-with-docs, tdd, code-review, triage, etc.)
            - `.local/plugins-docs/2026-08-31-superpowers-6.3.0-deepdive.md` — PRs that install/configure `superpowers` plugin or invoke its iron laws (TDD, verification-before-completion, brainstorming)
            - `.local/plugins-docs/2026-08-31-mattpocock-vs-superpowers.md` — PRs that compare, stack, or migrate between the two plugins
            - `.local/plugins-docs/2026-08-31-gh-issue-creation-guide.md` — PRs that change `docs/agents/issue-tracker.md`, `triage-labels.md`, or the issue-creation workflow
        - **Lesson references** (cite when PR changes the corresponding lesson):
            - `tasks/issues-lesson.md` (PL21-PL26) — PRs that touch backlog triage, issue creation, or any workflow 1-5
            - `tasks/lessons.md` (L1-L61 + amendments) — PRs that amend pipeline rules (L13, L46, L24, L11, L25, L42, etc.)
            - `tasks/plan-lesson.md`, `tasks/review-lesson.md`, `tasks/search-lesson.md`, `tasks/task-map-lesson.md` — PRs that amend the corresponding phase rules
    - **Invoke used plugins + list skill names**: every PR body MUST include a `## Skills invoked` (or `## Plugins used`) section enumerating the skills actually called during this PR's work. Lists audit evidence: `grill-with-docs`, `tdd`, `code-review`, `brainstorming`, `verification-before-completion`, `receiving-code-review`, etc. Reader can reproduce the agent's reasoning by re-invoking the same skills. Empty section = unverified work (per `superpowers:verification-before-completion` iron law).
    - **Reference format template** (paste into PR body):
        ```markdown
        ## References

        - <doc-path-1>: <one-line why>
        - <doc-path-2>: <one-line why>

        ## Skills invoked

        - `<plugin>:<skill>` — <when invoked + outcome>
        - `<plugin>:<skill>` — <when invoked + outcome>
        ```
15b. **Apply L24**
15b. **Apply L24** — verify CHANGELOG `[Unreleased]` bullet + User Stories table checkbox flip + "Try it" command landed in the merged code (per step 11b's local-branch rule, they should already be there). At release-cut time: move accumulated `[Unreleased]` entries under `## [vN] — YYYY-MM-DD` and reset `[Unreleased]` empty.
    - **Sub-step 15b.1 — agent-driven verification (per 2026-08-26 amendment)**: tier-gated (`normal` + `critical` only, skip `trivial`). Invoke Explore subagent with PR diff + user-stories.md + active plan + deep-dive.md paths. Agent outputs: (a) checkboxes to flip `[ ]` → `[x]` with file:line evidence, (b) drift findings (impl exists, doc missing or stale), (c) new-issue suggestions (planned-but-not-implemented stories). Human reviews the report + applies edits via separate commit(s). Never auto-merge doc edits from subagent output — the subagent reports, the human edits, the PR review gates the doc commit. Trigger on every `rust-eth-core` PR + every `main` release cut; opt-in elsewhere.
15c. **Review all L13 steps 1-15b completed** (broader pre-merge gate — widens 15d's PR-body checklist to all L13 steps):
    - **Walk each L13 step 1 through 15b** and confirm artifact exists before merging:
        - Step 1 (L11 skill tag — recorded in branch commits or PR body)
        - Step 2 (complexity tier self-detected + user-confirmed; wayfinder invoked if huge-work)
        - Steps 3-4 (issue picked up via triage (agent-ready brief) + to-tickets for large tasks; prototype if design sanity-check; branch checked out; karpathy-guidelines wrapper applied — 4 principles visible in commit history)
        - Steps 5-8 (skill pair invoked per L11 map, domain-tag wins on conflict; Q4 cap honored)
        - Step 9 (TDD red-green cycle: failing test first, then GREEN pass; `implement` orchestrator wraps TDD when spec/tickets exist; `implement` does NOT replace TDD; `kaizen:kaizen` during REFACTOR for anti-overengineering)
        - Step 9a (module interface design: codebase-design invoked for new public types; grill-with-docs invoked if interface decision warrants ADR; domain-modeling invoked if new domain terms / CONTEXT.md / ADR edits)
        - Step 10 (L12 pre-PR review findings applied — commit references each fix; critical-tier 3rd sub-agent security-auditor if applicable; trivial-tier skipped per amendment)
        - Step 10a (test coverage gap analysis: pr-test-analyzer applied; follow-up commit if gaps)
        - Step 10b (security-review applied for critical tier; findings follow-up committed before step 11)
        - Step 10c (standards + spec review: mattpocock-skills:code-review applied — Standards + Spec axes; follow-up commit if drift found)
        - Step 11 (verify quad gate clean: `cargo fmt --check` + `clippy --all-targets -- -D warnings` + `cargo test` + `cargo tree --duplicates` output captured)
        - Step 11a (backlog triage done; follow-up issues filed for any deferred work)
        - Step 11b (L24 cascade on local branch — CHANGELOG + Story flip + "Try it" column in commits traveling with feature PR)
        - Step 3a (spec synthesis: to-spec invoked when no plan existed for picked-up issue; spec landed before step 4)
        - Step 11c (systematic-debugging applied on non-obvious verify failures; hypothesis + regression test landed; diagnosing-bugs for perf-regression shape; research for unknown-root-cause needing primary-source facts; root-cause-tracing for errors deep in call stack; why for symptom fundamentals drill)
        - Step 11d (plugin-structure validation per L49 trigger)
        - Step 12 (commit approval PAUSE honored — user said "approved" or "commit" before each `git commit`)
        - Step 13 (commit-push-pr executed — branch pushed + PR opened; L51 resolving-merge-conflicts invoked if conflict surfaced during push or step-4 rebase)
        - Step 14 (issue checkboxes flipped with artifact evidence per L13 step 14 rules — file:line, test name, commit SHA, PR number per step 14 evidence format)
        - Step 15 (PR review by parallel sub-agents per L13 step 15)
        - Step 15a (10-section tech doc appended to PR body — Goal, Drift, API surface, Threat-model, Implementation, Tests, L12 review, Lessons, Backlog, Migration; grill-with-docs sharpening + ADR emission as shim; domain-modeling for glossary emission; frontend-design if wallet-desktop files in PR diff)
        - Step 15b (L24 cascade verified in merged code path)
        - **Critical-tier check (if applicable)**: 5-skill bundle present — type-design-analyzer + code-reviewer + security-auditor (L12 cluster) + security-review (standalone) + plugin-validator (L49 trigger). Q4 carve-out honored.
        - **Trivial-tier shortcut (if applicable)**: per L13 amendment note2026-08-25, skip pre-PR code review but L49 + L51 + L52 + L24 still apply. Cargo quad gate N/A for doc-only commits.
    - **Why a separate gate**: 15d's PR-body checklist is narrow (boxes in PR body only). 15c widens to all L13 steps — catches gaps in TDD evidence, L12 review, verify gate, L24 cascade, skill-tag pair, etc. that the PR body doesn't necessarily surface.
    - **Output**: either (a) all steps verified → proceed to 15d merge gate, or (b) gaps found → fix (commit amend, follow-up issue, or PR body update) before merge; re-run step 11 triple gate (local) + step 11-ci dedup status after any fix commit.
    - **Anti-patterns**:
        - Skipping the walk because "I did it all" — the walk is what proves you did it all. This is the documented gap from #64/#66/#68 PRs (unchecked boxes in merged PRs without deferral notes).
        - Speculatively flipping boxes "to clean up the body" — L28 honesty violation.
        - Confusing 15c with 15d — 15c is broad L13 audit; 15d is narrow PR-body checklist. Both run.
15d. **Merge code + close** (separate gate from review — explicit PAUSE before the bypass arms):
    - **Verify the PR body checklist is fully resolved BEFORE the merge PAUSE** (per L9 + L24). Every `[ ]` / `[x]` box in the PR body must be either:
        - **`[x]`** with the artifact behind it (test name, file:line, commit SHA, PR number) — same evidence standard as step 14.
        - **`[ ]` left unchecked with explicit deferral note** (e.g., `<!-- TODO: L29 manual smoke pending operator action -->`) — external gates that can't be satisfied before merge.
        - **Anti-pattern:** `[ ]` left unchecked without a deferral note = unfulfilled promise baked into the merged PR. Future-self audits the merged PR and finds the gap.
        - Use `gh pr edit <N> --body "<full body with all boxes resolved>"` (single command) to update the PR body in one audit-trail entry, mirroring step 14's single-edit pattern.
    - **PAUSE for explicit "admin bypass" / "force-push" / "delete-branch" authorization** before `gh pr merge --squash --admin --delete-branch` per L6. The auto-mode classifier requires literal phrasing for bypass arms; generic "approved" is insufficient (this is the documented gap from #64/#66/#68 PRs).
    - **Run the merge:**
        - **Integration-branch merge (per L45):** `gh pr merge <N> --squash --delete-branch` — no `--admin` flag required. Integration branches have no admin-bypass-requiring protection.
        - **Main final cut (per L45 v0.2 release pattern):** `gh pr merge <N> --squash --admin --delete-branch` with explicit PAUSE for the `--admin` bypass arm per L6. The auto-mode classifier requires literal "admin bypass" / "force-push" authorization; generic "approved" is insufficient.
        - The `--delete-branch` removes both local + remote task branch in one call.
    - **Verify issue closed:** `gh issue view <N> --json state` should report `CLOSED`. Squash-merge commit messages containing `Closes #N` / `Fixes #N` auto-close; otherwise `gh issue close <N>` explicitly.
    - **Verify main updated:** `git fetch origin main && git log --oneline origin/main -1` shows the merge SHA at HEAD. Branch protection + admin merge can be silent — verify explicitly per L28.
    - **No rollback:** if merge landed in a wrong state, use `git revert -m 1 <merge-sha>` rather than `git reset --hard`. Merges are immutable public artifacts (per L6).
    - 10 sections: Goal, Drift from plan, API surface, Threat-model coverage, Implementation, Tests, L12 review, Lessons captured, Backlog (links to `backlog` issues), Migration notes
    - Append/replace existing PR body with the full doc
    - Document lives with the commit (audit trail); no separate file to maintain
    - Skill-tag pair (per L11; Document stage of the 6-stage pipeline): `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections)

## Per session
16. At session start: enumerate skills (L11); re-grill pipeline if 5+ tasks since last grill. **When re-grilling, invoke `mattpocock-skills:improve-codebase-architecture`** to scan for deepening opportunities (HTML report) → grill one. Becomes the actionable vehicle for the re-grill. **For the analysis itself, invoke `context-engineering-kit:kaizen:analyse`** — auto-selects best Kaizen method (Gemba Walk / Value Stream / Muda) per target. **For new-skill candidates surfaced during re-grill, invoke `anthropics/skills:skill-creator`** — guides creation of new L13 skill bindings or replacement skills. Track grill count in the ledger (per L14) — counter resets after a grill event.
17. Update ledger after merge
18. Add new lessons if user corrections or novel patterns (L9 schema) — **PAUSE first**: surface candidate + rationale, await explicit user approval before writing to lessons.md
19. Apply L21 — dispatch the L21 sub-agent cascade (see L21 sub-section "Sub-agent dispatch at L13 step 19" for the agent prompt template). Sub-agent isolates the mechanical ledger cascade from the main user-facing flow.
```

### Behavioral discipline (karpathy-guidelines)

Apply at every L13 step (pickup, plan, code, review, verify, commit). Distilled from `andrej-karpathy-skills:karpathy-guidelines` per L11 skill→step mapping.

1. **Think Before Coding.** State assumptions explicitly. If uncertain, ask. If multiple interpretations exist, present them — don't pick silently. If a simpler approach exists, say so. Push back when warranted. If something is unclear, stop and name what's confusing. Ask.

2. **Simplicity First.** Minimum code that solves the problem. Nothing speculative. No features beyond what was asked. No abstractions for single-use code. No "flexibility" / "configurability" that wasn't requested. No error handling for impossible scenarios. If you write 200 lines and it could be 50, rewrite it. Test: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

3. **Surgical Changes.** Touch only what you must. Don't "improve" adjacent code, comments, or formatting. Don't refactor things that aren't broken. Match existing style, even if you'd do it differently. If you notice unrelated dead code, mention it — don't delete it. When your changes create orphans, remove imports/variables/functions that YOUR changes made unused. Don't remove pre-existing dead code unless asked. Test: every changed line should trace directly to the user's request.

4. **Goal-Driven Execution.** Transform tasks into verifiable goals. "Add validation" → "Write tests for invalid inputs, then make them pass." "Fix the bug" → "Write a test that reproduces it, then make it pass." "Refactor X" → "Ensure tests pass before and after." For multi-step tasks, state a brief plan with verify-checks per step.

**Anti-patterns:** overengineering "for safety"; bulk refactors bundled with feature work; picking the first interpretation silently; declaring done without verifying against success criteria.

**Cross-references:** Principle 1 → L11 (skill selection), L13 step 2 (complexity tier self-detect). Principle 2 → L12 (review), L42 (verify-staged). Principle 3 → L1 (workspace path changes), L6 (commit hygiene), L42 (verify-staged). Principle 4 → L13 step 1 (skill tag), L28 (verify-before-claim).

**Complexity tier → pipeline variation** (self-detect + user confirm):

| Tier                                                                                         | Pipeline                                                                                                                                                                                                                                                                                                                                                                                                                           |
| -------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `trivial` (doc-only / single-line)                                                           | doc-review only; skip pre-PR code review. L49 (plugin-validator) + L51 (post-commit verification) + L52 (honest fix-up if discrepancy) ALWAYS apply when their triggers match — trivial doesn't exempt them. EXCEPTION: doc-only commits that change public-facing contracts (README examples, public API docs, CHANGELOG breaking-change notes) still get a doc-review pass via `compass:docs-writer` per L11 Document stage row. |
| `normal` (typical feature)                                                                   | full pipeline: TDD + code-review + verify + PAUSE + commit + post-PR review                                                                                                                                                                                                                                                                                                                                                        |
| `critical` (security-sensitive: key material / signing / encryption / network / persistence) | full + `pr-review-toolkit:security-auditor` inside L12 + `security-review` standalone (defense in depth per L13 step 10)                                                                                                                                                                                                                                                                                                           |
| `feature-dev path` (no prior plan / scope undecided)                                         | `feature-dev:feature-dev` phases 1-4 (discover → explore → clarify → architect) produce ad-hoc plan; then L13 steps 9-15d own TDD → review → verify → PAUSE → commit-push-pr → PR review → tech doc → ledger                                                                                                                                                                                                                       |

**10 decisions (the grilling record)**:

| Q   | Decision                                                                                                                                                                                                                                                                                                                                                                                                |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Goals: A (correctness) + C (learning) — speed + reversibility deprioritized                                                                                                                                                                                                                                                                                                                             |
| 2   | Skill-tag: per-task pickup (not session-start, not per-step)                                                                                                                                                                                                                                                                                                                                            |
| 3   | Skill-conflict resolution: domain-tag wins; security > correctness > simplicity                                                                                                                                                                                                                                                                                                                         |
| 4   | Max 2 skills per pipeline step. `critical` tier: max 3 sub-agents in the parallel review cluster (L13 step 10) + 1 standalone security gate (`security-review` per L11 row) + 1 plugin-structure validator (`plugin-dev:plugin-validator` per L49 if trigger matches) = 5 effective skills. Sequential gates don't compound with the cluster cap.                                                       |
| 5   | Fix-loop limit: 3 rounds per task then PAUSE; round = one review + one fix commit pair. Shared budget across pre-commit (step 12) and post-PR-review (step 15). Exceed → PAUSE + revert-to-last-green + follow-up issue + ledger entry (Q9).                                                                                                                                                            |
| 6   | Verify: double-gate (per-step + task-end)                                                                                                                                                                                                                                                                                                                                                               |
| 7   | Pre-PR review: parallel sub-agents (`type-design-analyzer` + `code-reviewer`)                                                                                                                                                                                                                                                                                                                           |
| 8   | Review input: squash-candidate state (final commit on PR branch before merge) — not first commit, not uncommitted. For PRs that squash, reviewers see the combined final state. For PRs that merge commit-by-commit (rare), reviewers see the full history. (Re-grill 2026-08-15: corrected from "first commit" wording — Tasks 3-4 squash-merged multiple commits; reviewers read the combined state.) |
| 9   | Off-rails recovery: PAUSE then revert-to-last-green + follow-up issue + ledger entry                                                                                                                                                                                                                                                                                                                    |
| 10  | Complexity: self-detect + user confirm (hybrid of C + D)                                                                                                                                                                                                                                                                                                                                                |

### L13 amendment 2026-08-25 — `security-review` for critical tier

User noticed L13 referenced `pr-review-toolkit:security-auditor` for critical tier but not `security-review` (standalone). Added per amendment:

- L11 mapping table: new row for `security-review` (critical tier, after L12).
- L13 step 10 Apply: `security-review` fires as separate gate after L12 review, in addition to the existing `pr-review-toolkit:security-auditor` sub-agent inside L12. Defense in depth.
- Q4 max-3 cap unaffected (separate gate, not sub-agent).
- Triggers forward-looking from next picked-up task on `rust-eth-core` (eth-wallet-core v0.2 critical-tier surfaces: key material, signing, encryption, network, persistence). In-flight eth-wallet-core tasks not retroactively re-reviewed; each task's L11 skill-tag includes `security-review` from next pickup.
- **Status (2026-08-25 → 2026-08-26):** trigger not yet honored (no sub-task PRs since amendment landed). **FIRST HONORED 2026-08-26** — Issue #351 PR #368 (cycle 8b / C-1 from #339, critical-tier key-encryption material). See L53 for the post-mortem: L12 critical-tier cluster (3 sub-agents + standalone `security-review` + `pr-test-analyzer`) caught 2 real bugs TDD alone missed — empty `--password ""` accepted (would brick wallets, diverges from `btc/src/handlers.rs:86`) + `map_err(|_|)` discarded IO error context — plus a defense-in-depth gap the kernel-level reviewers couldn't see (ETH_PASSWORD env var lingering in process env post-read → future subprocess inheritance risk).
- **Fix-up (commit `74e2c88`):** original commit `dc5972c` claimed the L11 row was added but the actual diff only included 2 of 3 intended hunks. Follow-up commit added the missing L11 row with L9 honest disclosure. See L51 (post-commit verification) + L52 (honest fix-up pattern) for the discipline that prevents recurrence.

### L13 amendment 2026-08-26 — agent-driven user-stories verification (15b sub-step)

User confirmed subagent-driven verification should be codified in L13 step 15b (the L24 cascade step). Rationale: today's post-PR-#386 verification sweep found 14 partial + 5 missing stories that the manual `[ ]` → `[x]` cascade missed. Drift accumulates silently across releases when only humans update docs.

Added per amendment:

- L13 step 15b: new tier-gated sub-bullet — Explore subagent reads PR diff + user-stories.md + active plan + deep-dive.md; outputs checkbox flips + drift findings + gap findings + new-issue suggestions.
- Tier gate: `normal` + `critical` complexity only. `trivial` skips (single-story flip doesn't justify ~5–10k token burn).
- Human-in-the-loop: agent surfaces findings, human edits. Never auto-commit doc edits from subagent output.
- Trigger: every PR on `rust-eth-core` branch (eth-wallet-core + eth CLI active dev line) and on `main` release cuts. Other branches opt-in.
- First honored on PR #394 (this PR — feat/spki-pin-localnet-tests, Stories 28/29 added with research section, no checkbox flips needed since both are new).

## Flutter / Dart adaptation (wallet-desktop)

Same 19-step spec as L13, with toolchain + reviewer + sweep substitutions. Applies to every `wallet-desktop/` task. Formerly L31; merged into L13 per user direction.

### Toolchain substitutions (L13 step 11)

| L13 element            | Rust (`rust-wallet-app/`)                               | Flutter (`wallet-desktop/`)                         |
| ---------------------- | ------------------------------------------------------- | --------------------------------------------------- |
| Format                 | `cargo fmt --check`                                     | `dart format --set-exit-if-changed --output=none .` |
| Static analysis        | `cargo clippy --workspace --all-targets -- -D warnings` | `dart analyze --fatal-warnings --fatal-infos`       |
| Tests                  | `cargo test --workspace`                                | `flutter test`                                      |
| Reviewer (L13 step 10) | `ecc:rust-reviewer`                                     | `ecc:flutter-reviewer`                              |
| Branch prefix          | `feat/<domain>/<task>`                                  | `feat/wallet-desktop/task-N`                        |

### Verify gate (Flutter) — all three must pass before commit

```bash
export PATH="$HOME/flutter/bin:$PATH"
cd wallet-desktop
dart format --set-exit-if-changed --output=none .
dart analyze --fatal-warnings --fatal-infos
flutter test
```

**No hardcode in production; test only**: hardcoded literals (URLs, paths, IPs, credentials) belong in `test/` blocks only. Production routes through `EsploraConfig` (or equivalent named config). Test fixtures are exceptions, not defects.

### Secret-leak sweep (Flutter L12 CRITICAL #2 mirror)

```bash
cd wallet-desktop/lib
rg -n -e '(password|mnemonic|secret)\s*[:=]\s*"[^"]+"'
rg -n -e 'print\s*\(.*password|print\s*\(.*mnemonic'
```

**Rule**: zero matches outside `test/`. Mnemonic-shaped strings (12/15/18/21/24 lowercase words) in source = defect. Routes through `BtcLogFilter` (Task 7) before any logger call.

### Complexity tier variation (Flutter)

| Tier                                                                             | Pipeline variation                                                                                                              |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `trivial` (lint config, asset stubs)                                             | Skip TDD; verify gate only; no L12 review subagent                                                                              |
| `normal` (DTOs, providers, widgets)                                              | Full: failing test → impl → pass → L12 review → verify → PAUSE → commit                                                         |
| `critical` (BtcInvoker, TempSecretFile, BtcLogFilter, password/mnemonic widgets) | Full + `pr-review-toolkit:security-auditor` subagent + explicit L12 CRITICAL #2 sweep + custom lint for mnemonic-shaped strings |

### Flutter-specific anti-patterns

- **Skipping widget tests for feature tasks.** Per design §8.3, every screen has a widget test matrix (loading/data/error/validation/dispose).
- **Bypassing verify gate** to ship faster. Analyzer debt compounds like clippy debt.
- **Logging mnemonic-shaped strings** in widget code. `BtcLogFilter` is the only path.
- **Committing secrets** to git (`.dart_tool/`, `coverage/`). `.gitignore` mandatory; verify `git status --ignored` after scaffold.
- **Spawning `btc` without stripping inherited env vars** (Task 10 `BtcInvoker`). Strip `BTC_WALLET_MNEMONIC`, `BTC_ENCRYPT_PASSWORD`, `BTC_DECRYPT_PASSWORD` from parent env before `Process.start`.
- **Committing `dart analyze` auto-edits** to `analysis_options.yaml` (flutter-tools adds platform excludes on first run). Revert after verify gate; defer platform excludes to CI workflow task.
- **Pre-warming async providers before `pumpWidget`** trips `!timersPending` (Task 17). `container.read(provider.future)` before `pumpWidget` leaves autoDispose provider without listener; idle-dispose timer stays pending. Fix: drive loading → data via widget's own `ref.watch` (`pumpWidget` + `pumpAndSettle`).
- **Test fakes that mock `BtcInvoker.invoke` MUST override `invoke`** (Task 17). Without override, real `Process.start` → timeout. Always override `invoke<T>(cmd, parse)` to return `parse(fixture)` synchronously.
- **`BlockSemantics` ≠ `ExcludeSemantics`** (Task 18). `BlockSemantics` strips siblings painted BEFORE it, NOT descendants. For "drop my subtree", use `ExcludeSemantics(child: ...)`. Audit `BlockSemantics` near any secret/credential surface.
- **`unawaited(notifier.refresh())` leaks uncaught zone errors** (Task 18). `AsyncNotifier.refresh()` returns Future; `unawaited` doesn't attach `.catchError`. Use `ref.invalidate(provider)` (void; cannot leak).
- **`flutter_test`'s `enterText` unreliable on `obscureText: true`** (Task 18). `enterText` doesn't trigger `onChanged`. Workaround: bypass form, drive `_submit` via test seam OR use real `fake_btc.sh` integration test (L29 operator-driven).
- **`SelectableText` is clipboard + screen-reader exfil vector for secrets** (Task 18). Long-press → Copy leaks to OS pasteboard. Fix: `SelectionContainer.disabled(child: ExcludeSemantics(child: Text(secret)))`.
- **Verify L12 review suggestions against type system** before applying (Task 19). Paste suggestion, run `dart analyze`, commit only if clean.

### Branch + PR model (Flutter)

Originally scoped as direct-commit-on-main deviation (2026-08-15 early session, speed-over-reversibility rationale). User reversed same day → canonical L13 model adopted: per-task branch + L13 steps 13-15d fire as canonical. Tasks 1-2 (`26dfec9`, `a342597`) remain on main as historical deviation artifacts; Task 3+ follows canonical L13.

**Apply**: every new task follows this spec literally. If a step doesn't apply, log why in the ledger. If a step fails, escalate per Q9. Re-grill the pipeline after 5 tasks (or when a pattern emerges that the spec doesn't cover).

---

## L14 — Ledger rule

**Trigger**: Session 2026-08-07 dedup — `### Ledger` section removed from CLAUDE.md but rule wasn't re-added to lessons.md; the rule was lost in transit. Caught by `grep -n Ledger tasks/lessons.md` returning empty after the dedup commit.

**Rule**:

- **Track progress in `.superpowers/sdd/<plan>/progress.md`** (gitignored locally — survives compaction, never pushes to remote).
- **Update on four events**:
  - **Pick up** (task start): record issue #, branch name, plan link
  - **Commit** (task progress): record commit SHA, drift notes
  - **Merge** (task complete): record merge commit, closing issue #, all commits
  - **Grill** (per L13 step 16): record grill date, decisions altered, lessons captured. Resets the tasks-since-last-grill counter that triggers the re-grill loop.
- **After compaction**: trust the ledger over session memory. If they conflict, ledger wins (it was written deliberately; session memory may be compacted and lose detail).
- **Apply**: when re-grilling per L13 step 16, append a Grill event to the ledger with date, decisions altered, and lessons captured. Resets the counter that triggers the next re-grill (L13 step 16 = "5+ tasks since last grill").
- **Recovery pattern**: if you delete a rule from CLAUDE.md, add it to lessons.md in the same commit. Dedup requires two steps: remove + re-insert. One without the other is a silent rule loss.

**Why**: Workflow rules need a single source. CLAUDE.md is read on every session start, so duplicate rules are confusing ("which version wins?"). lessons.md is the project-local corrections ledger — versioned via L1-L14, append-only. Rules go in lessons.md; agent setup, plugin inventory, and visual templates stay in CLAUDE.md.

**Apply**: For every CLAUDE.md dedup, do a 2-step: (1) add rule to lessons.md in the same commit, (2) remove from CLAUDE.md. Verify with `grep <keyword> lessons.md` after the commit.

### Path collision caveat (formerly L18)

**Trigger**: Session 2026-08-11 #64 retry. Working `ai-cost-report.md` at `.superpowers/sdd/2026-08-05-rust-bitcoin-wallet/` was modified locally; `git add` rejected by `.superpowers/sdd/.gitignore = "*"`. Prior sessions had committed this path (commits `4be98ac`, `f5ddbb2`) BEFORE the `*` rule was applied.

**Rule**:
- `.superpowers/sdd/<plan>/{progress,ai-cost,estimate}-report.md` = operator-local-only per L14. Do NOT force-add.
- Canonical L21 record travels in the PR body (merge SHA + cost row + applied-findings table). Squash-merge = published ledger.
- When a non-PR record is required (cumulative cost across PRs, retrospective cleanup), target `docs/{ai-cost,estimate}-report.md` — survives L14 gitignore.

**Why**: L14 says ledger is gitignored for a reason (working-state churn out of public history). L21 says update on every merge — also valid. Without clarification, every session-end re-triages "force-add or skip?" — convention drift caused silent rule contradiction in this session.

**Apply**:
- `git add .superpowers/sdd/...` rejected by gitignore → `git restore --staged --worktree`. Working copy stays for next session; canonical record in PR body.
- Path-aware L21 update → write to `docs/`, not `.superpowers/sdd/`.
- L13 step 18 harvest: include this drift class in retrospectives.

---
---
## L21 — Update estimate-report AND ai-cost-report on every PR merge

**Trigger**: Session 2026-08-10. User asked: "Is estimate report file updated when every task completed?" + follow-up "you should update ai-cost report when every task completed" — current state: both reports are static snapshots, no update rule. Decided to capture as a lesson so future-self updates both reports as work progresses.

> **File paths:** As of 2026-08-10, the reports live at `.superpowers/sdd/2026-08-05-rust-bitcoin-wallet/estimate-report.md` and `.superpowers/sdd/2026-08-05-rust-bitcoin-wallet/ai-cost-report.md` (same path as progress.md, gitignored per L14). Earlier in the session they were at `docs/`. The rule's principle is path-agnostic — only the file location matters; update wherever the files live.

**Rule**: When a task-PR merges into `main`:

**For `docs/estimate-report.md` (client-facing bill):**

1. **Update the "Plan progress" section** — change the affected row's status from "In-flight" → "Complete", update the merged PR number reference, and update the merged-vs-pending count. Include the merge commit SHA + timestamp in the per-PR row notes (e.g., `PR #N merged (SHA abc1234, 2026-08-10)`) — makes each row independently auditable.
2. **Update the "Progress" line** — recalculate the percentage (e.g., 11/13 → 12/13 → 13/13) and update the progress bar characters.
3. **Update "Last updated" footer** in estimate-report.md with merge commit SHA + date.
4. **Excluded list** — if the merged work surfaced a new L-rule (e.g., from pre-PR review), add to the excluded list.
5. **Total stays fixed-fee** — unless scope changes (new task added, new plan section), the total amount does NOT change. Fixed-fee invoices don't recalculate per task.

**For `docs/ai-cost-report.md` (internal AI spend):**

1. **Move the merged task's row from "estimate" to "actual"** — replace `~` qualified retroactive figures with measured token counts from the session `usage` field. Include merge commit SHA in the row's "Notes" column.
2. **Recompute "Total (retroactive est.)"** row — drop the `retroactive est.` qualifier as more rows have actuals.
3. **Add any new tasks / process work** that emerged during the merge (e.g., review-fix commits, CI-blocker fixes) with their merge SHAs.

**Why**: A static invoice misrepresents progress. Client expects the bill to track deliverable completion; a bill that says "11/13" a week after the 12th task merged looks stale. Same principle for AI cost tracking — token spend is a real ledger item, not a one-time estimate.

**Apply**:

- After `gh pr merge <N>` succeeds (per L13.15): pause, ask user whether to update both reports alongside the L13.17 ledger update.
- Edit each report's relevant section (Plan progress for estimate-report; rows + totals for ai-cost-report) in a separate commit per file for clean audit trail.
- For multi-task PRs (rare): one update per merge event.
- When scope shifts (new task added to plan): update estimate-report.md "Scope" line + re-quote fixed-fee if scope growth is significant.

**Anti-patterns**:

- Update only the progress bar without updating individual rows (status drift).
- Update the total fixed-fee on every merge — defeats the purpose of fixed-fee billing.
- Skip update because "the bill is a snapshot" — the bill IS the source of truth for client; update it.
- Update ai-cost-report without per-task token usage data — falls back to estimates; mark `~` qualified until measured.

### Sub-agent dispatch at L13 step 19

**Trigger**: Session 2026-08-19, post-Task 27 merge (PR #204). User correction: "in lesson.md L13 at step 19 after merged run sub-agent to update estimate report and ai cost report" — L21 cascade belongs in a sub-agent, not main thread.

**Rule**: dispatch a `general-purpose` sub-agent (via Agent tool) at L13 step 19 to apply the L21 ledger cascade. Do NOT edit `estimate-report.md` + `ai-cost-report.md` from the main thread.

**Why**: mechanical long-file edits, gate-prone (GateGuard fires on first edits per session), high-context-cost. Sub-agent isolates:
- Main thread stays on user-facing flow (post-merge report, next-task pickup).
- Sub-agent handles gate retries independently (no main-thread pollution if gate denies).
- Cascade surfaces as discrete pipeline step in Agent dispatch log.
- Parallelizes with other post-merge work.

**Apply** — Agent prompt template:

```text
Agent(subagent_type: "general-purpose", prompt: "
  Apply the L21 ledger cascade for the just-merged PR.

  Inputs:
  - PR number: #<N>
  - Merge commit SHA: <sha>
  - Merge date: YYYY-MM-DD
  - Task title: <short>
  - Tier: trivial | normal | critical
  - Cost estimate (USD): ~$<amount>
  - Diff scope: \`git show --stat <sha>\` (or \`git diff <prev>..<sha>\`) — list of files changed

  Files:
  1. .superpowers/sdd/<plan-slug>/estimate-report.md — append row to Plan-progress table; update Progress line; update Cost-to-date; update Last-merge footer with new SHA + PR + date.
  2. .superpowers/sdd/<plan-slug>/ai-cost-report.md — append row to Tasks table (1-3 sentence summary, merge SHA in Notes); match existing pipe style with trailing pipe (MD055).

  Both gitignored per L18 — save only, no commit.

  CONSTRAINT (added 2026-08-26 per L53 sub-agent inaccuracy): the Notes column MUST reference ONLY symbols (function names, env var names, flag names, variant names) that appear in the diff scope listed above. Do NOT invent function names, env var names, or flag names that are absent from the diff. If no new public symbols were added, write 'no new public symbols'. Example: Issue #351's actual fallbacks were \`--password\` argv + \`ETH_PASSWORD\` env — the L21 sub-agent's first draft invented \`--password-file\` / \`--password-stdin\` / \`ETH_WALLET_PASSWORD\`, none of which exist; L52 honest fix-up corrected the row.

  Report: confirmation + line counts + any gate denials.
")
```

**Anti-patterns**:
- Edit ledger files inline post-merge — pollutes main-thread context, blocks user flow on gate retries.
- Combine L21 cascade with other post-merge work in same dispatch — keep L21 step discrete + auditable.
- Reformat pre-existing rows that don't match new style — out of scope; defer to cleanup PR.

**Recovery** (sub-agent fails mid-cascade):
1. Sub-agent reports partial + which file/lines failed.
2. Re-dispatch with partial state noted + retry only failed edits.
3. Retry fails 2x → pause + surface to user (gate denials, file format drift).

**Example**: Task 27 (post-release L29 operator smoke prep), PR #204 at `63ea2b3` on 2026-08-19, ~$5.00 (trivial). Sub-agent would have updated estimate-report (27/27, ~$349.30, footer `63ea2b3`) + ai-cost-report (script + README + UI_TEST_CHECKLIST + 12-file format drift + Issues #203/#205 + PR #204 merge). Inline execution captured as L38 retroactive; future-self dispatches sub-agent.

---

## L24 — On PR merge: update CHANGELOG.md (Keep a Changelog + User Stories table) + "Try it" column

**Trigger**: Session 2026-08-10. Two user feedback moments: (1) after merging PR #42 (Task 8 coin_type_for), "I have a feebacks are add changlog, after merged PR need to update user facing that handled" — two artifacts: a CHANGELOG.md for cumulative release history and a User Stories table for capability tracking. (2) "Read user stories, check list boxes after merged, update what user cases finished and we can playaround with them" — per-user-story lens separate from per-PR.

**Revision 2026-08-12**: README "What's New" section removed. CHANGELOG is the single source of truth for release history. User Stories table remains for at-a-glance capability status. README updates are out of scope for L24 cascade.

**Rule**: After every PR merge into `main`, update two surfaces in one commit on a fresh branch (e.g., `docs/changelog-update-pr-N`):

1. **`CHANGELOG.md` `[Unreleased]` section** (Keep a Changelog format): append an entry under one of `### Added` / `### Changed` / `### Fixed` / `### Security` / `### Deprecated` / `### Removed`. One bullet per user-visible change. Cite the PR number. For breaking changes: use `### Changed` with `**BREAKING**:` prefix.
2. **`CHANGELOG.md` User Stories table** (columns `#`, `Story`, `Status`, `Try it`): if the merged PR completes a user story (adds public API, ships CLI command, makes a previously-gated feature testable), flip the corresponding checkbox `[ ]` → `[x]` AND update the "Try it" column with one-line instruction (`cargo test -p bitcoin-wallet-core <module>` for library demos, `<subcommand>` for CLI commands). Drift detection: defense-in-depth changes (compile-time check, audit, lint) don't flip story boxes but still get a per-PR `[Unreleased]` entry.

**At release time**: cut a versioned section (e.g., `## [v0.1] — 2026-08-10`) by moving all `[Unreleased]` entries under the new version header. Then reset `[Unreleased]` to empty.

**Why**: Two audiences for the same change:

- **Per-PR changelog**: cumulative record, machine-parseable, git-blame-friendly, future contributors ask "what changed between v0.1 and v0.2?" — answers without reading commit history.
- **Per-story changelog**: at-a-glance for clients, "what can I do with this codebase today?" The user-story view answers "is feature X ready to use?" without reading git history.

**Apply**:

- **Apply L24 on the local working branch BEFORE merging** — append CHANGELOG `[Unreleased]` bullet + User Stories row flip in commits that travel WITH the feature PR. The doc updates ride along with the code change as one PR to main.
- Why local-branch-not-post-merge: keeps each feature self-contained (one PR = one feature + its docs). Reviewers see the user-facing change in the same review pass as the code. No "where's the doc for PR #N?" archeology.
- L21 stays post-merge (needs the actual merge SHA in the footer); only L24 travels with the feature PR.
- For sub-task workflows (≥3 sub-tasks per step 3 + sub-task section): L24 doc updates still travel with each sub-task PR on the integration branch — same principle, scoped to the integration branch.
- CHANGELOG entries are terse — one line per change, PR number, no prose.
- Story titles use user-facing verbs: "Sign messages", "Encrypt with password", "Sync wallet" — not implementation details.
- For Task 9 (Wallet end-to-end) stories (#10–#13 in the User Stories table), the stories get flipped when Issue #19 merges. No need to flip them per-task during #19 work — the merge is the trigger.
- Each User Stories row has 3 attributes: descriptive title, status checkbox, "Try it" command. All three must update together.
- **Maintenance caveat:** before v0.2, add a CI step that runs `cargo test --no-run` for each "Try it" module path and fails the build if the path resolves to zero tests. Without automation, the column goes stale silently (Story #8 had a test name that no longer existed after the wrapper refactor on session 2026-08-10 — caught by review).

**Anti-patterns**:

- Updating only one of the two surfaces — losing cumulative record OR capability status.
- Long CHANGELOG prose paragraphs — bullets only, terse.
- Forgetting to bump `[Unreleased]` to a versioned section at release — leaves history unreleased forever.
- Opening a separate `docs/changelog-update-pr-N` branch AFTER the feature PR merges — splits one feature into two PRs, forces reviewer to revisit completed work, breaks "one PR = one feature." Doc updates travel WITH the feature PR.
- One user story per commit / per PR line — confuses "feature" with "commit." Group multi-commit features under one story.
- Forgetting to update "Try it" — the column is the value; checkbox flips without command examples is just busywork.
- Flipping the box speculatively before the merge ("I'll merge it later") — drift problem.
- Marking a story "done" when the implementation is partial (e.g., "Create wallet from mnemonic" works but doesn't yet sync) — split into smaller stories instead.
- Fabricating "Try it" commands without verifying the path exists.

## L25 — Sub-task workflow for large tasks (≥3 sub-tasks: parent branch + sequential merge + PR-to-parent)

**Trigger**: Session 2026-08-10. Task 9 (`Wallet::from_mnemonic` + sync + balance) was too large for one PR. User directed: "complete all sub-tasks in task 9, then call merge" + "I have a tree for sub-task handling: task/19 is main task branch, check from main task branch for sub-task" + "we only merge by order number, complete task 19a merge into main task branch 19, then create new branch from 19 for task 19b".

**Rule**: For a task that you split into 3+ sub-tasks:

1. **Create a parent task branch** as the integration point:
   ```bash
   git checkout -b task/<N>-<slug>   # e.g. task/19-wallet-end-to-end
   ```
   The parent branch is empty initially (no code) — it's a ref branch for sub-task rebases, not a PR target.

2. **Merge sub-tasks into the parent by order**:
   ```bash
   git checkout task/<N>-<slug>
   git merge <sub-task-1-commit> --no-ff -m "Merge PR #X (#Na): <sub-task>"
   ```
   Each merge is `--no-ff` (merge commit preserved) so history shows the integration.

3. **Branch each subsequent sub-task from the parent** (NOT from main):
   ```bash
   git checkout task/<N>-<slug>
   git checkout -b task/<N>b-<sub-task-slug>   # e.g. task/19b-sync
   ```
   This way the sub-task branch inherits all prior sub-tasks' code; rebases onto parent as siblings land.

4. **Sub-task PRs target the parent branch** (NOT main):
   ```bash
   gh pr create --base task/<N>-<slug> --head task/<N>b-<sub-task>
   gh pr merge <PR> --squash --admin
   ```
   The parent accumulates sub-tasks as you go.

5. **Final cut to main** happens once all sub-tasks are merged into the parent:
   ```bash
   git checkout main
   git merge task/<N>-<slug> --no-ff -m "Task <N>: <slug> complete (sub-tasks #Na/#Nb/#Nc)"
   git push origin main
   ```
   Single cut to main = one merge commit = clean main history.

6. **L21 + L24 doc updates travel WITH each sub-task PR** on the working branch — no separate process branch. Each sub-task PR carries its CHANGELOG bullet + User Stories flip + estimate-report row update; the L13 post-merge cascade fires once at the final cut to main.

**Why**:

- **Main stays releasable** throughout the work — bug fixes, hot patches, and other tasks can land on main while the large task is in flight.
- **Sub-task integration happens on parent** — each merge is a checkpoint where sub-tasks combine. Easier to bisect and roll back individual sub-tasks than rolling back a mega-PR.
- **PR reviews are scoped** — each sub-task PR targets parent; reviewers see one sub-task at a time, not the whole feature.
- **Audit trail** — git history shows the parent as the integration lineage; future-self can trace sub-task → parent → main.

**When to apply** (decision criteria):

- **≥3 sub-tasks**: use parent + rebase (this rule). Clean integration tree.
- **2 sub-tasks**: borderline. Direct-from-main is acceptable if sub-tasks share no files.
- **1 task / 0 sub-tasks**: don't apply. Single branch + PR to main.

**Anti-patterns**:

- Sub-task branches rebase on `main` instead of the parent — defeats the parent's purpose.
- Sub-task PRs target `main` directly — defeats integration.
- Skip the `--no-ff` flag — flattens merge history; you lose the parent line.
- Parent branch stays empty after all sub-tasks land (the final cut is a fast-forward instead of a merge) — the parent becomes a dead branch with no audit trail.
- Open a separate `docs/changelog-update-pr-N` branch for sub-task doc updates — defeats the working-branch-travel rule (step 6).

**Recovery pattern** if a sub-task was merged to `main` accidentally:

```bash
git checkout main
git revert -m 1 <commit-sha>   # revert the merge
git push origin main
git checkout task/<N>-<slug>
git cherry-pick <commit-sha>   # or git merge <commit-sha>
```

Then re-merge sub-task onto parent (not main) per the rule.

**Examples in this session** (2026-08-10):

- Task 9 split into #19a / #19b / #19c (issues #45 / #46 / #47)
- Parent branch: `task/19-wallet-end-to-end` (empty stub at creation)
- #19a implemented on `task/19a-from-mnemonic`, merged to main via PR #48
- After #48 merged: `git merge a34fe0e --no-ff` into `task/19-wallet-end-to-end` (parent now has #19a)
- `task/19b-sync` branched from parent — inherits #19a's code
- #19b will PR to parent, not main
- After #19b merged into parent: branch `task/19c-balance` from parent
- Final cut: `task/19-wallet-end-to-end` → main as single merge commit when all 3 sub-tasks are integrated


## L46 — Git-action hygiene (branch identity + drift-scan + stash audit + post-commit verify)

**Trigger**: Four recurring mistakes observed in 2026-08-24 / 2026-08-25 sessions — wrong-branch commit, issue-premise drift, stash residue carry-over, Edit-tool silent drop. Each caught post-hoc; cost was revert + recommit + post-mortem. All four are preventable with a per-stage discipline check.

**Rule**: Four pre/post gates, each at its cheapest-recoverable step in the commit chain:

### Gate 1 — Branch identity (per `git add`, `git commit`, `commit-push-pr`)

Before ANY of those commands, run `git branch --show-current` and confirm it equals the expected branch (recorded at L13 step 4). Mismatch → `git checkout <expected>`, re-verify, then proceed. The check is mandatory regardless of how recently the checkout happened (`gh pr merge --delete-branch`, `git rebase`, IDE tab switch, session restart can all move HEAD silently).

```bash
EXPECTED="<branch from L13 step 4>"   # rust-eth-core, task/<slug>/<n>-<slug>, etc.
ACTUAL=$(git branch --show-current)
[ "$ACTUAL" != "$EXPECTED" ] && git checkout "$EXPECTED"
```

Pair with L42 (staged-set audit) — L42 audits content, L46 audits destination. Pair with L6: branch name verbatim in the approval prompt.

### Gate 2 — Drift-scan refutes stale issue premise (at pickup, L13 step 4a)

Before pickup on any "tests regressed" / "feature broken" / "X stopped working" issue, validate the premise against current repo state:

```bash
git log --all -- <cited-file-or-path>          # anything changed since symptom appeared?
grep -n "<cited-symptom>" <cited-file>         # does symptom ever appear in cited history?
<local re-run of the cited failure mode>       # does it fail today?
```

All three clean → premise stale → close as **no-repro** with drift evidence in body + `[x]` no-repro state + link to L46 in close comment. Don't start implementation against a refuted premise (costs hours fixing a fiction).

### Gate 3 — Stash residue audit (before next-task pickup on same branch)

After closing any task that involved WIP files (debug prints, scratch tests, diagnostic scripts), audit `git stash list` BEFORE pickup. If non-empty, name each entry by branch-context + WIP-purpose. Default action: `git stash drop` (WIP is throwaway by definition). Exception: stash contains real-but-uncommitted scope → land as a named commit (`git stash show -p | git apply` + commit). Do NOT let `git stash pop` run implicitly — pop is the residue vector.

### Gate 4 — Post-commit contents verify (after multi-hunk Edit commits)

After `git commit` (≥2 Edit calls), BEFORE push, run `git show --stat <sha>` and verify: file list matches intent, line counts sane, each file's diff matches the corresponding Edit. If mismatch (Edit reported success but change didn't reach commit):

- DO NOT amend (L6, no force-push; commit is destined for push)
- DO write a follow-up commit with L9 honest disclosure (state prior SHA, what was missing, why)
- DO document discrepancy in the new commit message

**Why** (one blast radius per gate):

| Gate            | Failure mode                                      | Recovery cost                                                                 |
| --------------- | ------------------------------------------------- | ----------------------------------------------------------------------------- |
| 1 (branch)      | `commit-push-pr` to wrong branch = public revert  | revert or force-rewrite, both visible in history                              |
| 1 (branch)      | `git commit` to wrong branch = SHA frozen locally | `git reset --soft HEAD~1` + re-commit; SHA appears in reflog                  |
| 1 (branch)      | `git add` to wrong branch = staged silently       | cheap (`git restore --staged`) but easy to miss                               |
| 2 (drift)       | Implementation against stale premise              | hours writing code that solves a non-existent problem                         |
| 3 (stash)       | `git stash pop` during next-task setup            | silent WIP injection, may bundle into next commit                             |
| 4 (post-commit) | Edit tool silent drop                             | only caught at PR review or remote — far more expensive than follow-up commit |

Cross-checking all four gates catches mistakes at the cheapest recoverable step. Each check is free; the mistake is expensive.

**Anti-patterns**:
- Skipping gate 1 because "I just checked out" — IDE tab switch + session restart are silent movers.
- Skipping gate 2 with "I'll start the fix and see if symptom reproduces" — pickup is too late; do the drift scan first.
- `git stash pop` without explicit reason — pop is the residue vector.
- Trusting Edit tool's success report without verification (commit `dc5972c`, 2026-08-25).
- Bulk-closing multiple issues on "no repro" hunch without per-issue drift scan.

**Pair-with**: L42 (staged-set audit) at the same commit gate as Gate 1. L6 (approval gates) for the commit/push PAUSE shape. L9 (issue body = status) for honest disclosure in Gate 4 follow-up commits.

---

## L50 — Harness-style work: `metaharness_oia_audit` weekly or pre-release

**Trigger**: working on a Claude Code plugin, MCP server, agent harness, or any project with metaharness `harnessFit` score > 70 (per `metaharness_score`).

**Rule**: run `metaharness_oia_audit` weekly OR pre-release for the harness repo. Persist record to `metaharness-audit` memory namespace. Use `metaharness_drift_from_history` to detect week-over-week drift.

**Apply**:

- Tool exits with verdict: clean / low / medium / high / critical
- Clean → log to memory, continue
- Low/medium → log, plan fix in next cycle
- High/critical → PAUSE, surface to user, fix before next release
- `metaharness_drift_from_history` requires a baseline (first audit) — pass `baselineKey` or `baselineFile` to subsequent runs for structural-distance comparison
- Default `threshold: 0.95` (alert when similarity falls below); tighten to 0.98 for production harnesses

**Why**: harness-style projects ship to user machines. Schema drift, mcp-scan regressions, threat-model gaps surface at install time. Weekly audit catches before release. Memory-persisted record enables trend analysis (week-over-week score drift).

**NOT for blockchain-sdk** (current state): eth-wallet-core = wallet library, not a harness. Metaharness scores it as `unknown_ci` repo type. L50 dormant until first harness-style work.

**Anti-patterns**:

- Running audit then ignoring Critical findings — defeats the gate
- Tightening threshold to 0.99+ without justification — alert fatigue
- Treating `unknown_ci` verdict as a failure (it's a misclassification, not a real risk)

---

## L55 — Step 11 verify gate: scope `cargo test -p <crate>` — never `--workspace`

**Trigger**: Session 2026-08-26, Issue #358 verify gate. `cargo test -p eth -p eth-wallet-core --workspace` ran >5 min and crossed the 300s Bash timeout. The slow part isn't `eth`/`eth-wallet-core` (which finish in <60s combined) — it's `bitcoin-wallet-core` integration tests that the `--workspace` flag pulls in (FFI tests that spawn Dart VMs, threat-model tests, etc., some 3–5 min each).

**Rule**: In step 11 verify gate, run `cargo test` with **only `-p <touched-crate>`** flags, never `--workspace`. L13 step 11 already says "Skip `cargo test --workspace --all-targets` here" — codify the workspace-flag trap explicitly + point at this rule.

**Why**: Workspace-wide test invocations in this repo are dominated by `bitcoin-wallet-core` integration tests that have nothing to do with the active PR. A `rust-eth-core`-only PR still pays the bitcoin FFI cost when `--workspace` is set. Time-cost compounds across multiple fix-loop rounds (3-round max per Q5 = 15+ min wasted per task).

**Apply**:

- Step 11 verify gate (L13): `cargo test -p <touched-1> [-p <touched-2> ...]` — never `--workspace`. `-p` is already workspace-aware.
- Step 11-ci dedup (L13): keep `cargo tree --workspace --duplicates` in CI workflow (cheap, one-time per push).
- If PR diff touches more than 2 crates, add each as a `-p` flag.
- L13 step 11 header line updated to reference L55 + show the scoped-cargo-test command.
