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
- [L28] Client product: verify before claiming done (three gates — stub honesty, example verify, real-deps verify)
- [L29] Live testnet smoke is operator-driven, not CI — `#[ignore]` + opt-in env var + manual run script
- [L37] CI workflow action-SHA hygiene (pin verified, tag when unverified)
- [L42] Verify staged set before commit (`git diff --cached --stat` after every `git add`)
- [L45] Issues labeled `rust-eth-core` route to the integration branch, never main directly
- [L46] Branch-identity gate: verify `git branch --show-current` matches expected before `git add`, `git commit`, and `commit-push-pr`
- [L47] Drift-scan can refute issue premise — no-repro closure type
- [L48] `git stash` can carry diagnostic residue into the next task
- [L49] Plugin-structure changes require `plugin-dev:plugin-validator` pre-commit
- [L50] Harness-style work: `metaharness_oia_audit` weekly or pre-release
- [L51] Verify post-commit contents — Edit tool can report success without applying
- [L52] Honest follow-up commit when prior commit message diverges from actual diff
- [L53] Critical-tier L12 cluster (3 sub-agents + security-review standalone) catches bugs TDD alone misses on key-encryption surfaces
- [L54] Defense-in-depth for env-var secrets: read + immediate `std::env::remove_var()` + Mutex-serialized test
- [L55] Step 11 verify gate: scope `cargo test -p <crate>`>` — never `--workspace` (bitcoin-wallet-core FFI tests dominate)
- [L60] `sol!` macro `bytecode` attribute expects **creation (init) bytecode** from `solc --bin`, NOT runtime bytecode from `--bin-runtime` (Issue #419, PR #485)
- [L61] EVM contract-creation input = `creation_bytecode ++ abi_encoded_args` — NO 4-byte selector; constructor args are appended to init code, not invoked via CALL dispatch (Issue #419)

> **Index gaps (L15–L20):** entries were added then trimmed during session 2026-08-10. L15/L16/L17 were `Secret<T>` / ZeroizeOnDrop / Debug patterns. L18/L19 were review findings (doc-test + merge gate). L20 was estimate-report self-improvement (replaced by client-bill pivot). All removed per user direction; rules not currently in scope.
>
> **Audit (2026-08-10):** L10, L22, L23, L27 removed per user direction. L10 (threat-model re-read) — type-system invariants + L11/L12 review pair make the rule redundant. L22 (fact-forcing gate) — enforced at hook layer, captured in `~/.claude/CLAUDE.md` global memory instead. L23 (`git stash -u -- <path>` deletes untracked) — git-native behavior, covered by `git stash` docs. L27 (grep `#[derive(...)]` before using traits) — type-checker errors surface the assumption fast enough; pre-flight grep added latency without saving compile cycles.

### Domain map

| Domain | Lessons |
|---|---|
| Build / Cargo hygiene | L1 |
| Git workflow | L6 (approval gates), L8, L14, L42, L46, L48 |
| Issue/PR protocol | L9, L24, L47, L62 |
| Skill + review pair | L11, L12, L13 |
| Post-merge bookkeeping | L21, L24 |
| Client product | L28 |
| Live testnet smoke | L29 |
| CI workflow hygiene | L37 |
| Flutter verify gate | L39 |
| Security review | (merged into L11/L12 review pair + L13 complexity tiers) |

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

---

## L11 — Scan skills list at session start, tag 3-5 relevant, invoke before doing

**Trigger**: Session 2026-08-07 — 9 Matt Pocock skills (`mattpocock-skills:*`), ~15 superpowers, ~10 compound-engineering, ~10 pr-review-toolkit, ~20 compass/ecc skills were loaded. Used 3 total (karpathy-guidelines, commit-commands:commit-push-pr, pr-review-toolkit:review-pr). Said "I don't see Matt Pocock plugins" when they were loaded — only saw them after user repeated the question and SessionStart hook listed them again.

**Rule**:

1. **At every session start**, enumerate the skills list (`/skills` if listed, or the SessionStart hook output) and tag 3-5 skills that match the active task.
2. **Before starting each task step** (pickup, TDD, verify, pre-PR, post-merge), invoke the relevant skill — don't rely on manual checklist.
3. **If a skill exists for a step I'm doing manually, invoke it.** Manual checklist failure modes: blind spots, missing sub-agents, no parallel review.

**Why**: 47 skills available. Skills encode battle-tested workflows (Pocock's TDD, superpowers:verification-before-completion, pr-review-toolkit:code-review with parallel sub-agents). Each one I skip is a workflow gap. Task 1.5's 4 security findings would have been caught by `pr-review-toolkit:code-review` invoked pre-PR instead of `security-guidance` invoked post-push.

**Skill → task-step mapping** (use this as starting checklist):

| Task step                                    | Skill to invoke first                                                                                                                |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Task pickup (understand + plan)              | `mattpocock-skills:domain-modeling` if new domain; `compound-engineering:ce-plan` if multi-step                                      |
| Task pickup (drift scan, per L13 step 4a)    | `git log --all -- <path>` for every plan/spec SHA cited in the picked-up issue. Empty = drift; commit artifact or file follow-up before feature work starts. |
| Task pickup (new feature, no existing plan)  | `feature-dev:feature-dev` — 4-phase (discover → explore → clarify → architect) producing ad-hoc plan. Use when feature unclear or scope undecided; phases 1-4 → ad-hoc plan, then L13 owns from step 9 onward (implement/review/summary re-absorbed into L13). |
| Brainstorming (pre-implementation design)    | `superpowers:brainstorming` (MUST before any creative work; gates L13 pre-pickup per L11 itself) |
| Workspace isolation                         | `superpowers:using-git-worktrees` (after brainstorming, before plan execution; integration branch per L45) |
| Plan authoring / plan review                 | `tasks/plan-lesson.md` (PL1, PL2, PL3, PL7–PL16) — drift scan, story trace, plugin stack, host-first SDK design, step-by-step workflow |
| Code review / SDK quality                    | `tasks/review-lesson.md` (PL4, PL5, PL6, PL17) — flat re-exports, async mutex, stability policy, review plugins |
| Deep search / content review / code-block    | `tasks/search-lesson.md` (PL18, PL19, PL20) — content review, code-block review, deep search + agent management |
| Plan authoring (multi-step task)            | `superpowers:writing-plans` (after brainstorming approval, before L13 step 9 TDD) |
| Plan execution (current session)            | `superpowers:subagent-driven-development` (default if subagents available; L13 step 5a branch) |
| Plan execution (parallel session)           | `superpowers:executing-plans` (fallback if no subagents) |
| TDD red-green-refactor                       | `superpowers:test-driven-development` (post-re-evaluation; was `mattpocock-skills:tdd`)                                              |
| Build/cargo error cascade                    | `superpowers:systematic-debugging` (post-re-evaluation; was `mattpocock-skills:diagnosing-bugs`)                                     |
| Module interface design                      | `mattpocock-skills:codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair per L13 Q4)                                     |
| Behavioral discipline (every L13 step)       | `andrej-karpathy-skills:karpathy-guidelines` — wrapper at step 4 (branch checkout) + step 15c (broad L13 audit). Per L13 behavioral discipline section (4 principles: think-first, simplicity, surgical, goal-driven). |
| Pre-PR code review (comprehensive)          | `pr-review-toolkit:code-review` wrapped by `superpowers:requesting-code-review` (parallel sub-agents: `type-design-analyzer` + `code-reviewer` per L13 step 10). Scope: correctness, security, tests, structure. |
| Pre-PR security review (critical tier, after L12) | `security-review` (standalone, comprehensive: secrets, SSRF, authz, trust boundaries, crypto, multi-tenancy) |
| Code smell / debt reported (any tier) | `ecc:refactor-clean` (dead-code audit first) → per-language `*-review` (interpret findings) → `ecc:quality-gate` (formatter check) |
| Pre-commit plugin structure validation (when trigger matches per L49) | `plugin-dev:plugin-validator` |
| PR review feedback (L13 step 15, 3-round fix loop) | `superpowers:receiving-code-review` wrapped by `pr-review-toolkit:code-review` |
| Test coverage gap analysis                   | `pr-review-toolkit:pr-test-analyzer`                                                                                                 |
| Doc / threat-model review                    | `mattpocock-skills:domain-modeling` (re-invoke; threat model is a domain artifact; was `compound-engineering:ce-doc-review`)         |
| Document stage (per-task tech doc → PR body) | `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections) |
| Before declaring done                        | `superpowers:verification-before-completion` (L11 recommends; L13 step 11 note says "User rejected adding to L13 spec" — invoke as L11-mapped wrapper, not L13-enforced gate) |
| Commit + push + PR                           | `commit-commands:commit-push-pr`                                                                                                     |
| Rust toolchain static analysis (one-shot verify) | `ecc:rust-build-resolver` (fmt + clippy + test + dedup + cargo audit in one invocation); slash command `/ecc:rust-build` |
| Rust toolchain review (review-paired fmt check)  | `ecc:rust-reviewer` or `/ecc:rust-review`                                                              |

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

  | Lens                                              | Plugin                                       | Position                                  |
  | ------------------------------------------------- | -------------------------------------------- | ----------------------------------------- |
  | Type design (encapsulation, invariants)           | `pr-review-toolkit:type-design-analyzer`     | sub-agent, parallel                       |
  | Code quality (correctness, security, convention)  | `pr-review-toolkit:code-reviewer`            | sub-agent, parallel                       |
  | Security-audit (in L12 code-review lens)          | `pr-review-toolkit:security-auditor`         | sub-agent, critical tier only             |
  | Comprehensive security (secrets, SSRF, authz, crypto) | `security-review`                         | standalone gate, critical tier only       |

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

        | Step | Skill invoked | Plugin / Tool | Status |
        |---|---|---|---|
        | 1 L11 enumerate | `mattpocock-skills:ask-matt` (router) + L11 skill→step mapping table | `mattpocock-skills` | ☐ |
        | 2 complexity tier | self-detect (trivial / normal / critical) + user confirm; `mattpocock-skills:wayfinder` if huge-work (multi-session, multi-PR) | `mattpocock-skills` | ☐ |
        | 3 issue pickup | `mattpocock-skills:triage` (categorise → verify → grill → brief) + `gh issue view` + checklist parse; `mattpocock-skills:to-tickets` if large task per L25; `mattpocock-skills:prototype` if issue asks for design sanity-check / throwaway prototype | `mattpocock-skills` | ☐ |
        | 3a spec synthesis | `mattpocock-skills:to-spec` (no interview; synthesis → publish to tracker) when no spec/plan exists | `mattpocock-skills` | ☐ |
        | 4 branch checkout | `superpowers:using-git-worktrees`; `andrej-karpathy-skills:karpathy-guidelines` wrapper | — | ☐ |
        | 4a drift scan | `git log --all -- <path>` (per L13 step 4a) | — | ☐ |
        | 5-8 skill pair | `mattpocock-skills:ask-matt` (narrow candidates) + per L11 row + Q4 cap (max 2 normal, max 3 critical L12 cluster) | `mattpocock-skills` | ☐ |
        | 9 TDD red-green | `mattpocock-skills:implement` (high-level orchestrator when spec/tickets exist) wrapping `superpowers:test-driven-development` (lower-level driver); `context-engineering-kit:kaizen:kaizen` during REFACTOR phase (anti-overengineering) | `mattpocock-skills` + `superpowers` + `context-engineering-kit` | ☐ |
        | 9a module interface | `mattpocock-skills:codebase-design` (new public types only); `mattpocock-skills:grill-with-docs` if interface decision warrants ADR (new error type, breaking API, cross-crate contract); `mattpocock-skills:domain-modeling` if new domain terms / CONTEXT.md / ADR edits | `mattpocock-skills` | ☐ |
        | 10 L12 review | `superpowers:requesting-code-review` wrapping `pr-review-toolkit:code-review` | `pr-review-toolkit` (`type-design-analyzer` + `code-reviewer`; critical: +`security-auditor`); `/ecc:rust-review` review-paired fmt re-check (`ecc`) | ☐ |
        | 10a test coverage | `pr-review-toolkit:pr-test-analyzer` (separate gate) | `pr-review-toolkit` | ☐ |
        | 10b security-review | `security-review` (critical tier only, standalone) | `security` | ☐ |
        | 10c standards + spec | `mattpocock-skills:code-review` (separate gate; Standards + Spec axes) | `mattpocock-skills` | ☐ |
        | 11 triple gate (local) | prefer `/ecc:rust-build`; bare cargo `fmt --check --workspace` + `clippy --workspace --all-targets -- -D warnings` + `test --workspace --all-targets` (+ `cargo audit` if installed) | `ecc:rust-build-resolver` | ☐ |
        | 11-ci dedup | `cargo tree --workspace --duplicates` runs in `.github/workflows/rust-eth-core-ci.yml` (CI only, per L45) | — | ☐ |
        | 11a backlog triage | `gh issue create` (multi-PR deferred) or in-session backlogs list | — | ☐ |
        | 11b L24 cascade local | CHANGELOG `[Unreleased]` + User Stories flip + "Try it" column (project convention, not skill) | — | ☐ |
        | 11c systematic-debugging | `superpowers:systematic-debugging` (conditional on verify failure); `mattpocock-skills:diagnosing-bugs` for perf-regression shape; `mattpocock-skills:research` for unknown-root-cause needing primary-source facts; `context-engineering-kit:kaizen:root-cause-tracing` for errors deep in execution call stack; `context-engineering-kit:kaizen:why` for symptom fundamentals drill | `superpowers` + `mattpocock-skills` + `context-engineering-kit` | ☐ |
        | 11d plugin structure | `plugin-dev:plugin-validator` (per L49 trigger match) | `plugin-dev` | ☐ |
        | 12 PAUSE | manual gate per L6 + workflow-approval-required memory | — | ☐ |
        | 13 commit-push-pr | `commit-commands:commit-push-pr`; `mattpocock-skills:resolving-merge-conflicts` if push fails with conflict (L51) | `commit-commands` + `mattpocock-skills` | ☐ |
        | 14 flip checkboxes | `gh issue edit N --body "<full body with [x] marks>"` (per step 14 evidence format: file:line, test name, commit SHA, PR number) | — | ☐ |
        | 15 PR review | `superpowers:receiving-code-review` wrapping `pr-review-toolkit:code-review` | `superpowers` + `pr-review-toolkit` | ☐ |
        | 15a tech doc | `mattpocock-skills:grill-with-docs` (Goal/Drift/Tradeoff sharpening + ADR emission) + `compass:docs-writer` (primary, 10-section doc) + `compass:api-designer` (secondary, API surface + Drift sections); `mattpocock-skills:domain-modeling` for glossary emission during tech-doc write; `anthropics/skills:frontend-design` if wallet-desktop files in PR diff (structural UI lens) | `mattpocock-skills` + `compass` + `anthropics` | ☐ |
        | 15b L24 verify merged | (project convention, not skill) | — | ☐ |

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

| Tier                                                                                         | Pipeline                                                                    |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `trivial` (doc-only / single-line)                                                           | doc-review only; skip pre-PR code review. L49 (plugin-validator) + L51 (post-commit verification) + L52 (honest fix-up if discrepancy) ALWAYS apply when their triggers match — trivial doesn't exempt them. EXCEPTION: doc-only commits that change public-facing contracts (README examples, public API docs, CHANGELOG breaking-change notes) still get a doc-review pass via `compass:docs-writer` per L11 Document stage row. |
| `normal` (typical feature)                                                                   | full pipeline: TDD + code-review + verify + PAUSE + commit + post-PR review |
| `critical` (security-sensitive: key material / signing / encryption / network / persistence) | full + `pr-review-toolkit:security-auditor` inside L12 + `security-review` standalone (defense in depth per L13 step 10) |
| `feature-dev path` (no prior plan / scope undecided)                                          | `feature-dev:feature-dev` phases 1-4 (discover → explore → clarify → architect) produce ad-hoc plan; then L13 steps 9-15d own TDD → review → verify → PAUSE → commit-push-pr → PR review → tech doc → ledger |

**10 decisions (the grilling record)**:

| Q   | Decision                                                                                                                                                                                                                                     |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Goals: A (correctness) + C (learning) — speed + reversibility deprioritized                                                                                                                                                                  |
| 2   | Skill-tag: per-task pickup (not session-start, not per-step)                                                                                                                                                                                 |
| 3   | Skill-conflict resolution: domain-tag wins; security > correctness > simplicity                                                                                                                                                              |
| 4   | Max 2 skills per pipeline step. `critical` tier: max 3 sub-agents in the parallel review cluster (L13 step 10) + 1 standalone security gate (`security-review` per L11 row) + 1 plugin-structure validator (`plugin-dev:plugin-validator` per L49 if trigger matches) = 5 effective skills. Sequential gates don't compound with the cluster cap. |
| 5   | Fix-loop limit: 3 rounds per task then PAUSE; round = one review + one fix commit pair. Shared budget across pre-commit (step 12) and post-PR-review (step 15). Exceed → PAUSE + revert-to-last-green + follow-up issue + ledger entry (Q9). |
| 6   | Verify: double-gate (per-step + task-end)                                                                                                                                                                                                    |
| 7   | Pre-PR review: parallel sub-agents (`type-design-analyzer` + `code-reviewer`)                                                                                                                                                                |
| 8   | Review input: squash-candidate state (final commit on PR branch before merge) — not first commit, not uncommitted. For PRs that squash, reviewers see the combined final state. For PRs that merge commit-by-commit (rare), reviewers see the full history. (Re-grill 2026-08-15: corrected from "first commit" wording — Tasks 3-4 squash-merged multiple commits; reviewers read the combined state.)                                                                                                                                                  |
| 9   | Off-rails recovery: PAUSE then revert-to-last-green + follow-up issue + ledger entry                                                                                                                                                         |
| 10  | Complexity: self-detect + user confirm (hybrid of C + D)                                                                                                                                                                                     |

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

| L13 element | Rust (`rust-wallet-app/`) | Flutter (`wallet-desktop/`) |
|---|---|---|
| Format | `cargo fmt --check` | `dart format --set-exit-if-changed --output=none .` |
| Static analysis | `cargo clippy --workspace --all-targets -- -D warnings` | `dart analyze --fatal-warnings --fatal-infos` |
| Tests | `cargo test --workspace` | `flutter test` |
| Reviewer (L13 step 10) | `ecc:rust-reviewer` | `ecc:flutter-reviewer` |
| Branch prefix | `feat/<domain>/<task>` | `feat/wallet-desktop/task-N` |

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

| Tier | Pipeline variation |
|---|---|
| `trivial` (lint config, asset stubs) | Skip TDD; verify gate only; no L12 review subagent |
| `normal` (DTOs, providers, widgets) | Full: failing test → impl → pass → L12 review → verify → PAUSE → commit |
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

---

## L28 — Client product: verify before claiming done (three gates — stub honesty, example verify, real-deps verify)

**Trigger**: Session 2026-08-10 multi-PR churn on Task 9. Three distinct failures, each from skipping a different gate:

- **Stub-vs-done gate**: implemented `#19b` (`Wallet::sync`) as URL-validation stub returning `Err("not yet implemented")`; treated as "minimal viable Option A". User course-corrected: "we are developing client product, and support features, user cases for real users, so we need to choose the best implementation in technical." Stubs are internal-only.
- **Example-verify gate**: wrote `examples/wallet_demo.rs` to demonstrate `Wallet::from_mnemonic`, then ran `cargo run --example wallet_demo` — 2 compile errors caught only at run time (Network not re-exported; `WordCount` path wrong). ~10 min round-trip waste + client confidence loss.
- **Real-deps gate**: built `Wallet::sync` + `Wallet::balance` full impl (PR #55). 164 tests pass, clippy clean, existing demo clean. Then user said "demo on main" — wrote NEW `sync_demo.rs` against live blockstream.info testnet. **3 semantic bugs surfaced** that all 164 unit tests + clippy missed: receive path double-indexed, xprv prefix hardcoded to XPRV (testnet needs TPRV), `reqwest::Url::join` dropped `/api` (no trailing `/` on base).

**Rule**: For any client-facing claim ("Try it works", "Story #N playaround-able", "Demo ready", "Feature X shipped"), three gates must pass in order:

### Gate A — Stub honesty (CHANGELOG state)

Stub vs full impl is binary, not gradient. `Err("not yet implemented")` is **no impl**, not partial.

Three states per feature:

- **`[x] done`** — fully implemented + tested; user can rely on it.
- **`[ ] gated`** — listed in CHANGELOG User Stories but marked not-yet-implemented.
- **(not listed)** — feature doesn't exist; don't tease.

Do NOT introduce a third implicit state: "merged but doesn't actually work."

**PR title honesty**: `feat(wallet): Wallet::sync stub (Task 9 #19b)` ← explicit "stub" in title; `feat(wallet): Wallet::sync implementation (Task 9 #19b)` ← only when real impl lands.

**L24 + L21 cascades**: User Story checkbox flip (L24) is gated on real impl, not merge. Estimate-report row update (L21) is gated on real capability — don't bill for a stub.

### Gate B — Example verify (`cargo check --examples` + `test --examples` + run binary)

Before declaring any of: "Try this command" / "Story #N playaround-able" / "Demo ready" / "Example works" — run the full chain:

```bash
cargo check --examples -p <crate>           # compile errors catch (fast)
cargo test --examples -p <crate>            # runtime errors catch
cargo run --example <name> -p <crate>        # binary runs end-to-end
```

If any fails, the claim is false — don't claim it. CHANGELOG "Try it" is a contract with the client.

### Gate C — Real-deps verify (run binary against live network/fs/db)

For any new example or method that composes third-party APIs (bdk, Esplora, bip32, reqwest, sqlx): write an example that calls it against live testnet / real fs / real db, run it, paste output as evidence in PR description + add comment in example source.

Unit tests + clippy + Gate B all pass when the function *type-checks* + has correct shapes. They do NOT exercise:

- Third-party parser semantics (bdk's `Key(InvalidNetworkKind)` for wrong prefix; bdk's `InvalidHdKeyPath` for double-indexed paths)
- URL composition semantics (`reqwest::Url::join` drops last segment without trailing `/`)
- Live API responses (HTML 200 instead of JSON 404 from a wrong path)

These only surface when the binary actually calls the third-party code against a real input.

**Why**: Client trust is built by honest scope communication, not by inflating delivered-features counts. Each gate catches a distinct trust-erosion mode: Gate A = optimistic CHANGELOG; Gate B = "Try it" doesn't work; Gate C = feature works in test but not in real-world use. All three share the property: the claim was made before the verification ran.

**Apply — per-claim checklist**:

- Before merging any PR that introduces a public API → Gate A: is the API actually functional end-to-end, or does it return `Err/TODO/unimplemented`?
- Before flipping any CHANGELOG User Story checkbox → Gate A: can a client call this API today and get a real result?
- Before billing for an item → Gate A: does the shipped artifact actually deliver the billed capability?
- Before declaring "Try it" in CHANGELOG → Gate B: did you run `cargo check --examples && cargo test --examples && cargo run --example <name>` and paste the output as evidence?
- Before declaring a method "done" that composes bdk / Esplora / reqwest → Gate C: did you write an example calling it against live testnet and run it?

**Anti-patterns**:

- "I'll add the full impl later; ship the stub now and flip the box" — Gate A.
- "Internal placeholder for external feature" — stubs are fine for internal modules (trait method placeholder); wrong for client-facing features.
- "Try this — it should work" (without running it yourself) — Gate B.
- "Tests pass, so the example works" — `cargo test` doesn't build examples unless `--examples` flag used.
- "Tests pass, the example compiles — we're good" (without running against real deps) — Gate C.
- Declaring "playaround-able" on a `#[ignore]` test that's never run.
- Skipping the demo for "trivial" methods — even `Wallet::sync` is non-trivial when it composes bdk + Esplora + bip32.

**Examples in this session** (2026-08-10):

- ✅ PR #48 (`Wallet::from_mnemonic`) — full impl, all tests pass, real capability. Story #10 flipped to `[x]`.
- ❌ PR #50 (`Wallet::sync stub`) — stub returning `Err("not yet implemented")`. **Story #11 should NOT be flipped**. PR #50 must NOT be merged without full impl replacing the stub.
- ❌ PR #55 (`Wallet::sync` + `Wallet::balance` full impl) — gates A + B passed but Gate C missed; 3 semantic bugs caught only when "demo on main" ran against real testnet.

---

## L29 — Live testnet smoke is operator-driven, not CI

**Trigger**: Multiple `#[ignore]`-marked live-Esplora tests across PRs (#55, #63, #84). Referenced in CHANGELOG entries ("Live testnet smoke `#[ignore]` per L29") + lesson L13 step 11b + L13 step 14 "external gate" handling — but never written as a stand-alone lesson.

**Rule**: For any test that requires live network access (blockstream.info, mempool.space, public Esplora endpoints), apply all three:

1. **Mark `#[ignore]`** with a comment naming L29: `#[ignore = "requires live testnet Esplora; run manually before merge per L29"]`. CI must never run it (flake risk on public infra; rate-limit risk on CI IPs).
2. **Provide a CLI opt-in path for operators** — typically an env var like `BTC_TESTNET_RUN=1` gating a shell script that exercises the feature end-to-end (`rust-wallet-app/scripts/btc-quickstart.sh` is the canonical example, ~7 steps covering wallet create → message sign → wallet show → balance → sync).
3. **Document the operator workflow in the PR description** — list the command(s) operator runs against live testnet before approving the PR merge. Issue acceptance checkboxes for `L29 manual smoke` stay `[ ]` until operator confirms; flip to `[x]` with operator's confirmation commit (per L13 step 14 "external gate acceptance").

**Why**: CI runner network access is unreliable + flaky + slow. Public Esplora endpoints can rate-limit CI IPs. Tests that make real network calls belong in operator-driven smoke on hardware with stable egress + a real SPKI pin + the operator's judgment on whether the sync output matches the PR's claims. Demoing on real testnet before merge catches semantic bugs the unit tests miss (PR #55 — Gate C gap).

**Apply — pattern for new live-network features**:

```rust
#[tokio::test]
#[ignore = "requires live testnet Esplora; run manually before merge per L29"]
async fn feature_completes_against_testnet_for_fresh_wallet() {
    // ... test body ...
}
```

And in the PR description:

```markdown
## L29 manual smoke (operator action before merge)

\`\`\`bash
BTC_TESTNET_RUN=1 BTC_ESPLORA_SPKI_PIN=<real-hex> \
  cargo run -p bitcoin-wallet-core --example sync_demo
# Expected: n_utxos=0 total_sat=0 (fresh wallet, no UTXOs)
\`\`\`
```

Operator confirms via PR comment + flips the `L29` acceptance box.

**Anti-patterns**:

- Running live tests in CI (causes flakes + rate-limit failures; PR review noise).
- Skipping the live smoke entirely (Gate C gap — semantic bugs that unit tests miss).
- Auto-flipping the `L29` acceptance box before operator confirmation (false-positive completion; per L13 step 14).

---


## L37 — CI workflow action-SHA hygiene: pin verified SHAs, tag-based when unverified

**Trigger**: Session 2026-08-18 wallet-desktop Task 25 CI workflows pickup. Trivial-tier (per L31). Wrote 2 GitHub Actions workflows (`wallet-desktop-ci.yml`, `btc-bundle.yml`) with action references. Initially used 7 fabricated / guessed SHA pins (`subosito/flutter-action@2f7f8b6...`, `actions/cache@1bd1e32...`, `actions/upload-artifact@65c4c4a...`, `codecov/codecov-action@0565863...`). L12 review's verify-gate step (`python3 -c "import yaml"` + `grep -hE "uses: "`) caught no fabricated-SHA issue directly — but a manual cross-reference against the existing `ci.yml` revealed 4 of 7 SHAs didn't match any known-good reference. Fixed by replacing the 4 unverified SHAs with tag-based references (`@v2`/`@v4`) and documenting the deviation inline.

**Rule**: for every GitHub Actions `uses:` reference in a workflow file:
1. **If the SHA matches an existing pinned reference** in another workflow file (e.g. `ci.yml`), reuse the exact same SHA — pin from the existing reference (supply-chain defense).
2. **If no verified SHA is available**, use the tag-based reference (`@v2` / `@v4`) AND add an inline comment explaining the deviation. Pin to a verified SHA in a follow-up commit after first successful CI run.
3. **Never fabricate a SHA** — even well-formed 40-hex strings. Without a verified reference, the action may not resolve, breaking the workflow at the worst possible time (first PR after introduction).

**Why**: GitHub Actions `uses: <repo>@<SHA>` is the supply-chain defense against action maintainer compromise (a compromised action can exfiltrate secrets, modify repo contents, etc.). The SHA pin locks to a specific commit. Tag-based references (`@v2`) allow the maintainer to push a new commit under the same tag — convenient but defeats the defense.

**How to apply**:
- For wallet-desktop: every workflow file under `.github/workflows/*.yml`. Existing `ci.yml` is the SHA reference (`actions/checkout@11d5960...` v4, `dtolnay/rust-toolchain@4360b5...` stable, `Swatinem/rust-cache@49a0bd...` v2). Use these SHAs directly.
- v0.2 follow-up: pin the 4 currently-tag-based actions (`subosito/flutter-action`, `actions/cache`, `actions/upload-artifact`, `codecov/codecov-action`) to verified SHAs after the first successful CI run captures the resolved SHAs from the Actions log.
- For new workflows in general: when adding a new `uses:` line, check the official action repo for its current SHA; add the SHA to the team's "verified SHA" reference document so future workflows can pin without re-verifying.
- v0.2 follow-up: a pre-commit hook that greps all `.github/workflows/*.yml` for SHA patterns and compares against the verified SHA document — would have caught the 4 fabricated SHAs at write time.

---


## L42 — Verify staged set before commit

**Trigger:** Session 2026-08-20 lessons.md L13 feature-dev hookup. Pre-existing staged-but-uncommitted files in the index from prior session (`CHANGELOG.md` + 2 `native_lib` Dart files). `git add tasks/lessons.md` appended to the existing index; the next `git commit` captured all 3 files in commit `0585adb` (1 intended + 2 unrelated + 1 unrelated CHANGELOG). User caught the bundle during the commit-pause review. Recovery via `git reset --mixed HEAD~` + clean recommit = 3 extra steps (commit, amend, reset, recommit, pop stash) that the L42 check would have eliminated.

**Rule:** After every `git add <files>`, run `git diff --cached --stat` before any commit to verify the staged set matches intent. If the staged set contains unexpected files, unstage them (`git restore --staged <files>`) and re-verify before committing.

**Why:** `git add <specific-file>` does NOT clear pre-existing staged content — it APPENDS to the index. Silent bundling of unrelated changes into a commit breaks the L6 separation principle (one commit = one scope) and forces a recovery via `git reset --mixed HEAD~` + recommit, which is itself state-modifying and gate-prone. The recovery cost (3-5 commands + re-pause) vastly exceeds the one-command pre-check. Session-start state is the worst case: prior sessions may have left files in the index; `git status` alone doesn't surface staged-vs-working-tree distinctions as clearly as `git diff --cached --stat`.

**How to apply:**
- After every `git add <files>`: run `git diff --cached --stat` — confirm the file list matches your intent (one file, one hunk range, no surprises).
- If unexpected files appear in the staged set: `git restore --staged <unexpected-files>` then `git diff --cached --stat` again to confirm the staged set is now correct.
- Session-start defensive check: `git status --short | grep '^[^?]' | grep -v '^.. '` flags staged-but-not-modified files (untracked-looking but in index) — those are the bundles most likely to surprise.
- Combine with L6 (approval gates before `git commit`) and L13 step 12 (PAUSE for commit approval): three-stage guard = `add → verify staged → commit`. The verify-staged step is the cheapest, fastest catch — runs in <1s and prevents the costliest recovery.
- Anti-pattern: relying on `git status` alone. The two-column output (`M file` = staged; ` M file` = unstaged) is hard to scan for unexpected content under load. `--cached --stat` gives a single clean list of what's about to ship.
- Companion check before commit: `git log --oneline @{u}..HEAD` — confirm the commits ahead of upstream match what you intend to push (catches accidental mixed commits in the same way).

---

## L43 — alloy 1.8.x: manual TxEip1559 + sign_transaction_sync for clean broadcast

**Trigger**: PR #300 (`feat(eth-e2e): 3 Sepolia sample tests + operator script (Issue #299)`). The `e2e_sepolia_send_native.rs` rewrite in `rust-wallet-app/spikes/alloy-v1/tests/`. Initial attempt used `EthereumWallet::from(sender).send_transaction(&provider, tx)` → type inference failed for `send_transaction` return type, and `sign_transaction` also failed (Network trait mismatch). Re-running after fixing to `with_signer` + fillers — same generics ambiguity. **Pattern landed by mirroring `tests/v4_anvil_send.rs` (which already worked end-to-end against Anvil).**

**Rule**: For v0.2 production code, build the tx envelope manually:

```rust
use alloy_consensus::{SignableTransaction, TxEip1559};
use alloy_eips::Encodable2718;
use alloy_network::TxSignerSync;
use alloy_primitives::{Address, TxKind, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer_local::{MnemonicBuilder, PrivateKeySigner};

let sender: PrivateKeySigner = MnemonicBuilder::english()
    .phrase(phrase.as_str()).index(0).expect("valid")
    .build().expect("build signer");

let nonce: u64 = /* provider.raw_request("eth_getTransactionCount", (sender.address(), "latest")).await */
let mut tx = TxEip1559 {
    chain_id: SEPOLIA_CHAIN_ID, nonce,
    gas_limit: 21_000,
    max_fee_per_gas: 10_000_000_000,
    max_priority_fee_per_gas: 1_000_000_000,
    to: TxKind::Call(recipient),
    value: U256::from(1_000_000_000_000_000u128),
    access_list: Default::default(), input: Default::default(),
};
let sig = sender.sign_transaction_sync(&mut tx).expect("sign");
let envelope = tx.into_signed(sig);
let pending = provider.send_raw_transaction(&envelope.encoded_2718()).await?;
```

**Why**:
- `EthereumWallet::send_transaction` and `EthereumWallet::sign_transaction` are Network-generic and require turbofish on the `N::UnsignedTx = TransactionRequest` inference path — fails in cargo test contexts where `Provider` is built without an explicit network type.
- `provider.send_raw_transaction(&Vec<u8>)` takes **owned bytes** (or `&[u8]`), so the envelope's `encoded_2718()` (`Vec<u8>`) drops in cleanly.
- Receipt polling mirrors V4: 20 retries x 100ms (Anvil) or 60 retries x 2s (Sepolia = 120s budget).
- F47 key handling: `MnemonicBuilder` already zeroize-aware via the `bip39` feature; wrap `phrase: &str` from env zeroized after use.

**Apply**:
- Any `eth-wallet-core` task involving EIP-1559 broadcast (Stories 5, 6, 13, 14, 17, 21, 25).
- L29 e2e samples (`e2e_sepolia_send_native.rs`) follow this pattern.
- Do NOT carry the pattern into a `Wallet` abstraction until alloy's `EthereumWallet` API stabilizes (currently 1.8.3) — manual sign is the canonical path with verified behavior.

## L44 — alloy 1.8.x: provider.call + abi_decode + SolCall scope quirks

**Trigger**: PR #300 + #295 plan wiring. Four compile errors during e2e_sepolia_erc20_balance.rs scaffolding:

1. `provider.call(&alloy_rpc_types::TransactionRequest::default().to(...).input(...))` → `expected TransactionRequest, found &TransactionRequest`.
2. `IERC20::balanceOfCall::decode(&raw, true)` → `takes 1 argument but 2 supplied`.
3. `decoded.account` returns `Address`, not `U256`.
4. `IERC20::balanceOfCall.abi_encode()` → `no method named abi_encode found`, fix `use alloy_sol_types::SolCall`.

**Rule**:
- `provider.call(tx)` takes an **owned** `TransactionRequest` (per `Provider::call(&self, tx: N::TransactionRequest) -> EthCall<N, Bytes>` in alloy 1.8.3). Bind to a local first: `let req = TransactionRequest::default().to(addr).input(calldata.into()); provider.call(req).await?`.
- `SolCall::abi_decode(&raw)` takes **one argument** in alloy-sol-types 1.6.x (the `validate: bool` overload is on `SolValue::abi_decode_raw`). Don't pass a second bool.
- The `*Call` struct generated by `sol!` only carries **inputs** — for output, use either `SolCall::abi_decode_returns(&raw)` OR slice the raw bytes manually:
  ```rust
  let mut word = [0u8; 32];
  word.copy_from_slice(&raw[..32]);
  let balance = U256::from_be_bytes(word);
  ```
  This works for `uint256` returns (single 32-byte BE word). Multi-return abi-encoded responses need the typed `abi_decode_returns`.
- For sol! blocks that produce `IERC20::balanceOfCall { account: owner }`, the `.abi_encode()` method comes from the `SolCall` trait — `use alloy_sol_types::SolCall;` in scope is mandatory.

**Why**:
- alloy's `alloy_provider::Provider::call` reflects `N::TransactionRequest` (could be owned or borrowed depending on `N`); in default Ethereum binding it's owned.
- alloy-sol-types 1.6.x changed the `decode` signature — the `validate: bool` param went away in favor of separate `decode` and `decode_validate` (or via `abi_decode_raw` on `SolValue`).
- The `sol!` macro emits `IERC20` module with `balanceOfCall { account: Address }` (input) — output is via the `sol!`-generated `Return` struct or via raw bytes.

**Apply**:
- Any `eth-wallet-core` test or task calling `provider.call`, `sol!`-typed `decode`, or `.abi_encode()`.
- For Story 22 (ERC-20 balanceOf) + Story 24 (custom token decimals/symbol) — both use the manual-byte-slice decode since values are single `uint256`.
- For Story 21 (transfer) — outputs are ignored once broadcast accepts; can skip decode entirely.
- For Story 25 (approve) — uses `approveCall` similarly; tx shape only matters.
## L45 — Issues labeled `rust-eth-core` route to the integration branch, never main directly

**Trigger**: 2026-08-23 session bootstrap. PR #294 (alloy v1.8 spike + eth-wallet-core plan) merged to main. User requested a dedicated integration branch for the eth-wallet-core v0.2 work: `docs/superpowers/plans/2026-08-23-eth-wallet-core.md` (12 tasks #295 + #301-#311). Workflow + CODEOWNERS bootstrapped on integration branch `rust-eth-core` (commits `04fb6d2` + `701a294`); user authorized autonomous `auto push and commit, PR, merge on rust-eth-core` for the 12-task duration.

**Rule**:

1. **Integration branch canonical**: New branch `rust-eth-core` is the single landing zone for any PR labeled `rust-eth-core`. Created off `main` immediately after PR #294 landed; carries the new CI workflow `.github/workflows/rust-eth-core-ci.yml` (label-routing + verify-gate) and `CODEOWNERS` entry.
2. **Sub-task branches**: Each Plan Task is one PR. Sub-task branches fork from `rust-eth-core`, NOT from main. Naming convention `task/eth-wallet-core-v0.2/<n>-<slug>` (e.g., `task/eth-wallet-core-v0.2/1-scaffold`).
3. **PR base = `rust-eth-core`**: Every sub-task PR opens with `--base rust-eth-core --head <sub-task-branch>`. The `label-routing` job in `rust-eth-core-ci.yml` fails the CI check if the PR is missing the `rust-eth-core` label — guards base-branch + label consistency.
4. **CODEOWNERS routes review**: `.github/CODEOWNERS` auto-assigns `@nhitranbtc` for eth-wallet-core crate paths + plan + user-stories + deep-dive + agent-docs + the workflow file. Single accountable approver before each sub-task merge.
5. **Merge flow**: Sub-task PRs squash-merge into `rust-eth-core` per `gh pr merge <N> --squash --delete-branch`. No `--admin` needed for the integration branch.
6. **Final cut at v0.2 completion**: After Task 12 #311 ships, ONE PR cuts `rust-eth-core` → main. This PR uses explicit `"merge PR N with --admin and delete-branch, approved"` phrasing per L6 (the bare "approve" triggered the post-action classifier block on PR #294 earlier this session).
7. **No direct-to-main PRs for eth-wallet-core surfaces**: A sub-task PR accidentally targeting main must be closed + re-opened against `rust-eth-core`. The integration branch is the gate.

**Why**:

- **L25 sub-task workflow** in canonical form: each Plan Task is one PR; the integration branch accumulates sub-task work in one place; main stays releasable throughout the v0.2 build. Prevents the "ship half-baked crate to main" failure mode where individual tasks land out-of-order.
- **Label-based CI gating** catches the rare case where base-branch matches but label is missing (operator mis-set via web UI). `label-routing` job is the safety net beneath the CODEOWNERS human review.
- **CODEOWNERS single-approver pattern** keeps one person accountable before each sub-task merge. Per `rust-eth-core` is a 12-PR cadence; the same person who approves sub-task-by-sub-task also cuts the final-cut-at-v0.2 → review consistency.
- **Final cut as one PR**: collapsing 12 sub-task commits into one v0.2 release commit keeps `main` history clean per L25 rule 6. Releases from `rust-eth-core` once, after all 12 Tasks pass CI.

**Apply**:

- **Task pickup (L13 step 3)**: Read the issue's labels FIRST. If the issue carries `rust-eth-core`, branch off `rust-eth-core` — never off main. (12 issues in flight as of 2026-08-23: #295 + #301-#311.)
- **PR open**: `gh pr create --base rust-eth-core --head <sub-task-branch> --label rust-eth-core --body-file <path>`. Keep the `label-routing` CI check green.
- **PR merge (sub-task)**: `gh pr merge <N> --squash --delete-branch` — no `--admin` flag required (integration branch has no admin-bypass-requiring protection).
- **Post-merge housekeeping**: After sub-task merges into `rust-eth-core`, switch back to `rust-eth-core`, pull, branch the next sub-task off the now-included state. Don't carry local edits across merges.
- **Final cut at #311**: ONE PR titled `chore(eth): release cut v0.2.0 — eth-wallet-core landing (#311, `rust-eth-core` → main)`. L24 cascade travels WITH this PR (CHANGELOG `[Unreleased]` → `[v0.2.0]` section + User Stories table checkbox flip). L21 estimate-report + ai-cost-report updates via sub-agent dispatch per L13 step 19.
- **Drift recovery** (if a sub-task PR accidentally targeted main): close the bad PR, redo from `rust-eth-core` base. L13 Q9 off-rails recovery applies — pause + revert-to-last-green + follow-up issue + ledger entry.
- **Workspace hygiene**: After the v0.2 final cut, close `rust-eth-core` (delete from origin). The `.github/workflows/rust-eth-core-ci.yml` + `CODEOWNERS` entries can be retired or repurposed for the next multi-task integration cycle.

---

## L46 — Branch-identity gate: verify `git branch --show-current` before `git add`, `git commit`, and `commit-push-pr`

**Trigger**: User correction during 2026-08-24 session on `rust-eth-core`. First iteration of this lesson covered only L13 step 13 (`commit-push-pr`). User broadened scope (same session): wrong-branch mistake can land at any point in the commit chain — `git add` stages onto the wrong branch, `git commit` freezes the wrong destination, `commit-push-pr` publishes. Catch at the earliest irreversible step (add = unstageable, commit = revert-only, push = public).

**Rule**: Before ANY of `git add`, `git commit`, or `commit-commands:commit-push-pr`, run `git branch --show-current` and confirm the output equals the expected branch (L13 step 4 = `karpathy-guidelines + branch checkout`). If they differ → STOP. Run `git checkout <expected>` and re-verify before continuing. The check is mandatory regardless of how recently the checkout happened (session restart, tab switch, prior `gh pr merge --delete-branch` etc. can all move HEAD silently).

**Why**: Each step in the commit chain has a different blast radius:

- **Wrong `git add`** = staged onto wrong branch. Recovery: `git restore --staged <file>` then re-add on correct branch. Cheap.
- **Wrong `git commit`** = SHA frozen on wrong branch. Recovery: `git reset --soft HEAD~1` (keeps changes staged) or `git reset HEAD~1` (unstages). Local-only revert possible, but the commit SHA appears in reflog until garbage-collected.
- **Wrong `commit-push-pr`** = SHA published to `origin/<wrong-branch>`. Recovery: revert or force-rewrite — both visible in public history per L6.

Cross-checking branch identity at every command catches the mistake at the cheapest recoverable step. Branch name is git's single source of truth; verification is free.

**Apply**:

```bash
# Run BEFORE every git add / git commit / commit-push-pr:
EXPECTED="<branch from L13 step 4>"   # e.g. rust-eth-core, task/eth-wallet-core-v0.2/1-scaffold
ACTUAL=$(git branch --show-current)
if [ "$ACTUAL" != "$EXPECTED" ]; then
  echo "Branch mismatch: expected=$EXPECTED actual=$ACTUAL"
  git checkout "$EXPECTED"
  # re-verify ACTUAL == EXPECTED, then proceed
fi
```

- **Three gates, one rule**: check branch before (1) `git add`, (2) `git commit`, (3) `commit-commands:commit-push-pr`. Even if add passed, re-check before commit. Even if commit passed, re-check before push. The check is cheap; the mistake is expensive.
- **Record the expected branch at step 4**: write it down in the working scratch (chat scratch or `.superpowers/sdd/<plan>/progress.md` per L14). Don't rely on memory — session restarts erase it.
- **Re-check after any branch-modifying operation** in the same session: `gh pr merge --squash --delete-branch`, `git rebase`, `git checkout -`, manual branch switch via IDE.
- **Pair with L42**: L42 audits the staged set (content); L46 audits the destination (branch). Both run at the same commit gate. Two checks, one pause.
- **Pair with L6 prompt shape**: when surfacing the commit for approval, include the branch name verbatim per L6 ("Show branch name in the approval prompt"). Example: *"Commit `chore(eth): scaffold` on branch `rust-eth-core` — approve?"* The L46 destination check is the gate before the prompt; the branch name in the prompt is the gate the reviewer reads.
- **Mismatch recovery by stage**:
  - Wrong `git add` → `git restore --staged <files>` then `git checkout <expected>` and re-add.
  - Wrong `git commit` → `git reset --soft HEAD~1` (keep changes staged), `git checkout <expected>`, re-commit. NEVER `--hard` (loses staged content). Ledger entry required.
  - Wrong `commit-push-pr` → per L6 + L13 Q9 off-rails: pause, surface the mistake, revert via new commit on correct branch + cherry-pick or revert on wrong branch. Do NOT force-push. Ledger entry required.

---

## L47 — Drift-scan can refute issue premise (no-repro closure type)

**Trigger**: 2026-08-24 #323 close. Issue body claimed BTC FFI tests regressed. Drift scan (per L13 step 4a) showed `git log` had no test code change since the last green run; the cited SHA + symptom combination never appeared in the cited file. Issue premise was stale relative to actual repo state.

**Rule**: Before pickup on any "tests regressed" / "feature broken" / "X stopped working" issue, validate the premise against current repo state. If `git log --all -- <cited-path>` + `grep -n "<cited-symptom>"` + a local re-run of the cited failure mode all come back clean, the premise is stale. Close the issue as **no-repro** with drift evidence — do NOT start implementation work against a refuted premise.

**Why**: Implementation work against a stale premise burns hours writing code that solves a problem that does not exist. Drift scan is the cheapest test: if the cited artifact never contained the cited symptom, the issue is asking you to fix a fiction. Closing with evidence (rather than silently no-oping) preserves the audit trail and protects the next reader from re-doing the drift scan.

**Apply**:

- At pickup (L13 step 4a), expand the drift scan to cover the issue's failure claim, not just the plan/spec citations:

  ```bash
  git log --all -- <cited-file-or-path>          # was anything changed since the symptom appeared?
  grep -n "<cited-symptom>" <cited-file>         # does the symptom ever appear in cited history?
  <local re-run of the cited failure mode>       # does it fail today?
  ```

- All three clean → premise is stale → close with drift evidence in body + `[x]` no-repro state + link to L47 in close comment.
- One or more dirty → premise holds → proceed with normal L13 pipeline.

**Anti-patterns**:

- "I'll start the fix and see if the symptom reproduces" — pickup is too late; do the drift scan first.
- Closing as "stale" without drift evidence — leaves the next reader no audit trail to confirm the close was sound.
- Bulk-closing multiple issues on a "no repro" hunch — each close needs its own drift scan.

---

## L48 — `git stash` can carry diagnostic residue into the next task

**Trigger**: 2026-08-24 session. After resolving #323 (no-repro close per L47), `git stash list` held an entry from an earlier diagnostic sequence — a WIP test file referencing a SHA + symbol name that no longer matched the closed issue. Had `git stash pop` run implicitly as part of the next task's setup, the residue would have entered the working tree silently and risked bundling into the next commit.

**Rule**: After closing any task that involved temporary WIP files (debug prints, scratch test cases, diagnostic scripts), audit `git stash list` BEFORE pickup of the next task on the same branch. If a stash entry exists, decide explicitly: drop it (`git stash drop`) or land it (`git stash show -p | git apply` + commit with a real scope). Do NOT let `git stash pop` run implicitly as part of the next task's setup.

**Why**: Stash residue is silent. It does not appear in `git status` until popped. Once popped, the file is in the working tree and looks like normal WIP — the next `git add` will pick it up, the next commit will bundle it, the L42 verify-staged gate may or may not catch it depending on whether the operator notices the unfamiliar filename. Pre-emptive audit before the next task starts is cheaper than post-commit recovery.

**Apply**:

- After task close (any path: merged, no-repro, deferred): run `git stash list` — if non-empty, name each entry by its branch-context + WIP-purpose before deciding.
- Default action: `git stash drop`. WIP diagnostic files are throwaway by definition; if the work was real, it would have been committed long ago.
- Exception: stash contains real-but-uncommitted scope → land it as a named commit (`git stash show -p | git apply` + commit with descriptive subject). Do NOT pop in-place.
- Pair with L42 (verify staged set): same discipline, applied to git's stash namespace instead of the index.

**Anti-patterns**:

- `git stash pop` without an explicit reason — the pop is the residue vector; everything after it inherits the WIP scope.
- "I'll just commit the stash content with the next task's changes" — bundles unrelated work; defeats L6 (one commit = one scope).
- Assuming `git stash list` is empty after a clean merge — stash entries persist across `gh pr merge --delete-branch` if the merge did not consume them.

---

## L49 — Plugin-structure changes require `plugin-dev:plugin-validator` pre-commit

**Trigger** (any one matches before commit):

- Edit to `**/plugin.json` (`.claude-plugin/plugin.json` or `marketplace.json`)
- Edit to `**/hooks/**` (any hooks file)
- Edit to `**/skills/**/SKILL.md` YAML frontmatter (name + description only — content body edits outside trigger)
- Edit to `**/.mcp/servers.json`
- Edit to `**/settings.json` (when changes touch permissions/hooks/MCP)
- Edit to `**/settings.local.json` (local override file — same validation need)
- New plugin scaffold output (e.g. `plugin-dev:create-plugin`)

**Rule**: Invoke `plugin-dev:plugin-validator` post-edit, BEFORE the L13 step 12 commit PAUSE. Read-only agent. Findings feed same fix loop as L12 review (max 3 rounds per L13 Q5 budget, shared budget).

**Why**: Plugin structure bugs (broken manifest, wrong hook event, missing permission, malformed frontmatter) surface only at install/load time on user's machine. Late discovery = bad UX + hotfix cycle. Pre-commit validation = cheap insurance, single agent call.

**Apply**:

```bash
# Quick trigger check before commit:
git diff --cached --name-only | grep -E '(plugin\.json|hooks/.*\.(sh|js|ts|py)|skills/.*SKILL\.md|\.mcp/servers\.json|settings(\.local)?\.json)$' \
  && echo "Plugin-structure change detected — run plugin-dev:plugin-validator before commit per L49"
```

- If no match → skip validator, proceed to commit PAUSE
- If match → invoke `plugin-dev:plugin-validator` as `general-purpose` sub-agent (or direct call). Findings Critical/Important → fix loop. Findings Minor → defer to PR body.
- Skip if change is docs-only within plugin dir (e.g. `README.md` inside `.claude-plugin/`). Validator scope = manifest/hooks/skills/permissions, not prose.

**Where L49 fits in L13 workflow** (cross-reference, not amendment): conditional gate inside L13 step 11 cluster (verify), before step 12 commit PAUSE. See L13 step 11 Apply section for the inline bullet.

**NOT changing**:

- L13 step 10 — `plugin-dev:plugin-validator` is a different lens from `type-design-analyzer` + `code-reviewer`. L13 step 10 = source-code review. L49 = plugin-manifest review. Mutually exclusive by trigger, not by step.
- L13 complexity tiers — L49 fires whenever trigger matches, regardless of trivial/normal/critical.

**In-flight eth-wallet-core v0.2**: no trigger match. L49 dormant until first plugin-structure edit.

**Anti-patterns**:

- Skipping `plugin-dev:plugin-validator` because "manifest looks fine to me" — manual eyeball misses schema drift the validator catches.
- Validating then ignoring Critical findings (commit anyway) — same as skipping validator, just slower.
- Trigger check on `git status` (working tree) instead of `git diff --cached --name-only` (staged) — misses the actual commit contents per L42.

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

## L51 — Verify post-commit contents — Edit tool can report success without applying

**Trigger**: any `git commit` that follows a multi-hunk Edit session (≥2 separate Edit tool calls in the same commit).

**Rule**: after `git commit`, BEFORE any `git push`, run `git show --stat <sha>` and verify:
- All expected files appear in the diff
- Line counts match the expected hunks (insertions + deletions)
- Each file's diff matches the intent of the corresponding Edit call

If mismatch (Edit reported success but the change didn't reach the commit):
- DO NOT amend (per L6, no force-push; commit is destined for push)
- DO write a follow-up commit with L9 honest disclosure (per L52)
- DO document the discrepancy in the new commit message (link prior SHA, state what was missing, why)

**Why**: the Edit tool can report success even when the change didn't apply (observed 2026-08-25 in commit `dc5972c` — claimed "L11 mapping table: new row for security-review" but the actual diff only included 2 of 3 intended hunks; the L11 row silently missed). Without post-commit verification, the discrepancy only surfaces at PR review or remote — far more expensive than a follow-up commit.

**Apply**:

```bash
# After any commit, before push:
git show --stat <sha> | head -30
# Verify: file list matches intent, line counts sane
```

- Single-edit commits: low risk, optional check
- Multi-edit commits: mandatory check
- Multi-Edit where each call is in a separate hunk: mandatory check
- After the check passes → proceed to push per L6 bundling

**Anti-patterns**:

- Trusting Edit tool's success report without verification
- Amending the prior commit when push already happened (force-push risk per L6)
- Silently leaving an inaccurate commit message in history

---

## L52 — Honest follow-up commit when prior commit message diverges from actual diff

**Trigger**: discovered that a prior commit's message claims a change that the actual diff doesn't include (or includes a change the message omits).

**Rule**: write a follow-up commit with:
- Subject prefixed `fix(<scope>):` (e.g. `fix(lessons):`)
- Body explicitly states: which prior commit, what was missing, why (e.g. "L9 honesty: prior commit message inaccurate; this commit documents the fix in the audit trail")
- Link the prior commit SHA explicitly
- DO NOT amend the prior commit (per L6, no force-push on a commit already destined for push; even if not yet pushed, amending breaks the prior commit SHA which other references may point to)
- DO NOT silently leave the prior message inaccurate (L9 honesty violation)

**Why**: L9 schema (issue body = status, PR body = fix analysis) demands honest reporting. Inaccurate commit messages create audit-trail drift that future-self or reviewers can't trust. A follow-up fix commit with explicit disclosure is cheap; amending hides the mistake; silence compounds the drift.

**Apply** (template for the follow-up commit body):

```text
fix(<scope>): <one-line summary of fix>

Fixes omission in <prior-sha> — that commit's message claimed <claim>
but the actual diff only included <what-was-actually-included>. The
<what-was-missing> was silently missing.

This commit:
- Adds the missing <change>
- <other changes>

L9 honesty: prior commit message inaccurate; this commit documents
the fix in the audit trail.
```

**Pair with L51** (post-commit verification): L51 catches the discrepancy. L52 codifies the response.

**Anti-patterns**:

- Amending the prior commit when it was already pushed or referenced
- Rewriting history with `git rebase -i` to "clean up" the inaccurate message
- Apologizing in the new commit message (L9: state the fact, move on)
- Skipping the fix because "the change is in the file now" — the audit trail matters more than the line content

## L53 — Critical-tier L12 cluster (3 sub-agents + security-review standalone) catches bugs TDD alone misses on key-encryption surfaces

**Trigger**: Session 2026-08-26, Issue #351 (cycle 8b / C-1 from #339 — rpassword TTY prompt as primary password source). 5-file / +264/-16 PR on `rust-eth-core`. TDD wrote 4 unit tests + 2 rpassword test-seam integration tests, all GREEN. L12 critical-tier cluster (3 sub-agents: `pr-review-toolkit:type-design-analyzer` + `pr-review-toolkit:code-reviewer` + `compass:security-auditor` because the pr-review-toolkit variant isn't registered in this harness) caught **2 real bugs TDD missed**:

1. **HIGH** — empty `--password ""` accepted as the wallet password (would brick the wallet, since a keystore encrypted with an empty password is unrecoverable). Code-reviewer flagged divergence from `btc/src/handlers.rs:86` which makes `Some(p) if !p.is_empty() => Ok(p)` so empty flag falls through to prompt. Fix: `if !p.is_empty()` guard in the eth kernel + empty-argv-falls-through to env then prompt. Two new unit tests pin both branches.
2. **HIGH** — `map_err(|_| Error::InvalidInput("password required: ..."))` discarded the underlying `io::Error` from `rpassword::prompt_password`. Operator on a CI runner without `/dev/tty` saw the generic "password required" message — same as someone who forgot to supply a password — masking the real diagnostic. Fix: drop the re-wrap so `prompt_password`'s own `Error::InvalidInput(format!("password prompt failed: {e}; ..."))` propagates with full io::Error context.

Plus `compass:security-auditor` M-2 caught a third defense-in-depth gap the kernel-level reviewers couldn't see: `ETH_PASSWORD` env var lingers in process env after read, so any future subprocess spawned by the eth CLI (or by alloy / tokio deps) would silently inherit the cleartext password. Fix: `std::env::remove_var("ETH_PASSWORD")` immediately after read; `ENV_LOCK: Mutex<()>` test serializes the env-mutation check for parallel-safe cargo test runs.

**Rule**: Critical-tier L13 review pays for itself on key-encryption / signing / encryption / network / persistence surfaces. Do not skip the L12 sub-agent cluster or the standalone `security-review` gate even when TDD is thorough. The 5 sub-agent cost (3 L12 + security-review + pr-test-analyzer) is small relative to the cost of a bricked-wallet bug or a leaked env-var secret shipped to production.

**Why**: TDD covers happy paths + boundary cases the author can imagine. Critical-tier review covers:

- **Cross-crate convention divergence** (eth-vs-btc on `--password ""` handling) — only visible when comparing to a sibling CLI's existing pattern.
- **Error-message context preservation** (the `map_err(|_|...)` anti-pattern — invisible from inside the chain; needs a reviewer's eye for "what does the operator actually see at the leaf").
- **Defense-in-depth gaps for future code paths** (subprocess inheritance — no subprocess exists today, so TDD can't write a test for "no future subprocess can inherit the var"). Only a security lens catches "what COULD be inherited by code that doesn't exist yet".

**Apply**:

- For any critical-tier PR per L13 (key material / signing / encryption / network / persistence surfaces), run the full L12 cluster + standalone security-review + pr-test-analyzer. Budget the sub-agent cost upfront.
- When the cluster catches divergent findings across reviewers (same bug surfaced by 2+ sub-agents with different lenses = high-confidence real bug), fix in one pass before verify gate — don't split fixes across multiple rounds.
- After the fix loop, do the quad verify gate (`cargo fmt` + `cargo clippy --all-targets -- -D warnings` + `cargo test` + `cargo audit`) BEFORE the commit PAUSE — per L13 step 11, the verify gate runs on the final fix, not the first pre-review pass.
- For env-var + subprocess concerns, the security-auditor's "no subprocess spawning in current crate" verification is necessary but NOT sufficient — the fix must remove the var from process env post-read as defense-in-depth for code that doesn't exist yet.

**Pair with L13 amendment 2026-08-25**: the `security-review` standalone gate (added to L13 step 10b) is what catches the defense-in-depth gaps like M-2 — `pr-review-toolkit:security-auditor` (L12 cluster) and `security-review` (standalone) are defense in depth, not redundant. The cluster catches code-level issues; the standalone catches "what could go wrong in code that doesn't exist yet".

## L54 — Defense-in-depth for env-var secrets: read + immediate `std::env::remove_var()` + Mutex-serialized test

**Trigger**: Session 2026-08-26, Issue #351 cycle 8b. `compass:security-auditor` M-2 finding: `ETH_PASSWORD` env var lingered in process env after `resolve_password()` read it. Today no subprocess is spawned from the `eth` CLI (no `tokio::process::Command` / `std::process::Command`), so the inheritance risk is zero. But `Cargo.lock` already pins `alloy-node-bindings::Anvil` for dev tests + any future spawn work (PR-B sign + broadcast already wires RPC) would silently inherit the cleartext password.

**Rule**: When reading any env-var secret (password, token, signing key, API key), capture the value then **immediately** `std::env::remove_var("NAME")` after the read. Treat the var as single-use for this invocation; reading it twice would be a security regression. Verify the removal with a `Mutex`-serialized test:

```rust
#[test]
fn reads_and_removes_env_var() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::set_var("THE_SECRET", "value");
    let result = read_secret();
    std::env::remove_var("THE_SECRET"); // cleanup before assertions
    assert_eq!(result.unwrap(), "value");
    assert!(std::env::var("THE_SECRET").is_err());
}
static ENV_LOCK: Mutex<()> = Mutex::new(());
```

**Why**: `std::env::var` reads without clearing — the var stays in the process env block until the process exits. Any subprocess spawned later (today, tomorrow, by a future feature) inherits the parent's env block and can read the secret via `std::env::var` from the child. Removing the var post-read blocks this inheritance class without requiring knowledge of which future subprocesses will exist.

The Mutex is necessary because `cargo test` runs tests in parallel by default; without the lock, one test's `set_var` races with another test's `var()` and produces flaky failures or false-positive assertions (e.g. the cleanup removes the var that another test just set).

**Apply**:
- For any `std::env::var("SECRET")` call in production code (passwords, tokens, keys), follow it with `std::env::remove_var("SECRET")` in the same scope.
- The cleanup must happen unconditionally — not just on the success path. Use a `let _guard = ...` pattern or explicit `remove_var` at the end of the read scope.
- Test the removal in a unit test that uses a static `Mutex<()>` to serialize env mutation within the test binary. The `Mutex` is local to the test module (one per file); other test modules can still mutate env in parallel.
- Cleanup BEFORE assertions — if the assertion panics, the cleanup still runs (Rust drops `_guard` on panic, which doesn't help here because we want explicit `remove_var` not RAII; but the explicit `remove_var` before assertions gives a loud failure if a sibling test already clobbered the var).
- This is defense-in-depth for code that doesn't exist yet — the security lens ("what COULD be inherited by code that doesn't exist yet") catches it; TDD alone can't write a test for "no future subprocess can inherit the var".

## L55 — Step 11 verify gate: scope `cargo test -p <crate>` — never `--workspace`

**Trigger**: Session 2026-08-26, Issue #358 verify gate. `cargo test -p eth -p eth-wallet-core --workspace` ran >5 min and crossed the 300s Bash timeout. The slow part isn't `eth`/`eth-wallet-core` (which finish in <60s combined) — it's `bitcoin-wallet-core` integration tests that the `--workspace` flag pulls in (FFI tests that spawn Dart VMs, threat-model tests, etc., some 3–5 min each).

**Rule**: In step 11 verify gate, run `cargo test` with **only `-p <touched-crate>`** flags, never `--workspace`. L13 step 11 already says "Skip `cargo test --workspace --all-targets` here" — codify the workspace-flag trap explicitly + point at this rule.

**Why**: Workspace-wide test invocations in this repo are dominated by `bitcoin-wallet-core` integration tests that have nothing to do with the active PR. A `rust-eth-core`-only PR still pays the bitcoin FFI cost when `--workspace` is set. Time-cost compounds across multiple fix-loop rounds (3-round max per Q5 = 15+ min wasted per task).

**Apply**:

- Step 11 verify gate (L13): `cargo test -p <touched-1> [-p <touched-2> ...]` — never `--workspace`. `-p` is already workspace-aware.
- Step 11-ci dedup (L13): keep `cargo tree --workspace --duplicates` in CI workflow (cheap, one-time per push).
- If PR diff touches more than 2 crates, add each as a `-p` flag.
- L13 step 11 header line updated to reference L55 + show the scoped-cargo-test command.

## L56 — Permanent regression block via CI audit-grep (Issue #382)

**Trigger**: Session 2026-08-26, Issue #382 (follow-up to PR #374 / Issue #365). PR #374 landed the `Error::rpc(impl Display)` constructor + bulk-replaced 23 leaky `Error::Rpc(format!("...{e}"))` sites + closed AC #5 via a one-shot per-PR `grep` audit. The audit was one-shot — a future author reverting to the leaky pattern would bypass the redaction contract and CI would not catch it. The constructor's doc-comment cited "L18 PAUSE-before-write rule" (L18 retired per the lessons audit) but the enforcement was a PR-time check, not a permanent gate.

**Rule**: For security-critical code paths (RPC redaction, secret handling, key material, signing surfaces), one-shot PR-time audits are insufficient. Convert the audit into a CI job that fails on regression. Pair the job with a doc-comment at the constructor's site so the enforcement mechanism is visible to future authors.

**Apply**:

- When landing a redaction / secret-handling fix that ships an AC audit (`grep -rn '<bad-pattern>'` returns 0), immediately add a CI job running the same grep with exit-on-match.
- The job must run on every PR + push to the protected branch. No `--ignored` skip, no `if: success()` guard. Lightweight (just `actions/checkout` + bash, no rust toolchain needed).
- Reference the job from a doc-comment at the constructor's site so future authors see the enforcement mechanism (e.g. `/// Enforced by CI audit-gate \`rust-error-grep\` job in ... — see Issue #N.`).
- Document the rule in `tasks/lessons.md` per L24 cascade — link the parent issue + the audit grep pattern + the enforcement job path.
- Use a clear error message in the CI step (`echo "::error::<why-this-pattern-bad>"`) so a future author who triggers the gate gets actionable guidance, not just a failing exit code.
- **Drift caveat**: the parent issue body for #382 cited "L18 PAUSE-before-write rule" — L18 was retired per the lessons audit. The rule (PAUSE-before-write + enforcement gate) now lives at L13 step 12 (commit approval) + this lesson. When citing retired lesson numbers in issue bodies, surface the drift in the PR body Drift section.

**Anti-patterns**:

- One-shot audit only — silent regression possible the moment the audit author moves on.
- Doc-comment without CI gate — describes the rule but does not enforce it.
- CI gate without doc-comment — enforcement works but the rationale is invisible to readers of the source.
- Job scoped to `--ignored` / `if: success()` — defeats the permanent-block intent.
- Generic `grep` for `format!` or `Rpc` — too broad, causes false positives in unrelated code. Anchor on the specific leaky pattern (`Error::Rpc(format`).

### L57 — Security-auditor fallback for critical-tier (2026-08-30, PR #462 application)

**Trigger**: `pr-review-toolkit:security-auditor` not in active harness's agent registry during L13 step 10 critical-tier review of #459 (sign dispatch wiring). Existing L53 amendment (2026-08-26) permits substituting the closest equivalent but requires the deviation be documented in PR body AND lessons.md.

**Rule**: When `pr-review-toolkit:security-auditor` is absent, substitute `compass:security-auditor` (closest lens match — description names "trust boundaries, crypto, secrets, authz"). Alternatives: `ecc:security-reviewer` (OWASP-flavored) or `voltagent-qa-sec:security-auditor`. Pick the one whose description explicitly mentions the lens needed (for key-material / signing surfaces: crypto + secrets + trust boundaries). Document the substitution in the PR body alongside the fallback attribution, AND append a per-instance note here with the PR# / tier / outcome. Do NOT skip the security lens entirely per L13 Q4 carve-out — the fallback is mandatory.

**Why**: Critical-tier surfaces (key material / signing / encryption / network / persistence) MUST get the security lens. Skipping it leaves gaps TDD + type-design + code-review cameras don't see. PR #462 application: `compass:security-auditor` caught (a) the `from_slice` error-category fix (keystore corruption → `Error::Rpc`, NOT `InvalidInput` — operator retry-trap), and (b) confirmed 6/7 lenses clean (zeroizing, password chain, wrong-password exit code, Q7 gate, verify round-trip, signer construction error). Two pre-existing gaps (L54 threading invariant, empty `POLYGON_PASSWORD` env) flagged but not regressed by #459 — deferred to separate small-PR follow-ups per L13 surgical.

**Apply**: When `pr-review-toolkit:security-auditor` (or `type-design-analyzer`, `code-reviewer`) is unavailable, substitute the closest equivalent and document the fallback in (a) PR body "L12 review" section + (b) `tasks/lessons.md` with PR# / tier / outcome / lenses-covered. Never silently skip a lens; if no equivalent exists, surface the gap to the user and PAUSE before proceeding (per L13 Q4 budget).

---

### L13 amendment 2026-08-28 — `cancel-in-progress: true` cascade-cancels cargo test in workspace-wide CI

Trigger: PR #430 (Phase 1 `polygon-wallet-core` thin wrapper, issue #423) on `feat/polygon-phase-1-423` against `rust-evm-core`. Three substantive commits in 17 minutes (feature scaffold, L24 CHANGELOG bullet, [ci-skip] lessons amendment) cascade-cancelled the in-flight `Rust test` job twice — Run 7 cancelled at 13m20s, Run 8 cancelled at 14m — before `cargo test --workspace --all-targets` could finish. The 40m `timeout-minutes` was never reached. The cancel signal came from the workflow's `concurrency.cancel-in-progress: true` setting, NOT from the timeout.

**Rule**: When a PR to `rust-evm-core` (or any integration branch) is likely to receive >1 substantive commit during the run of its longest required-check job, either (a) drop `cancel-in-progress: true` from the workflow `concurrency` block, OR (b) bump the relevant job's `timeout-minutes` past `expected_runtime × 2` AND accept that any push during the run kills the in-flight test.

**Why**: `cargo test --workspace --all-targets` against `rust-wallet-app/` includes `bitcoin-wallet-core` FFI tests that deliberately wait for esplora sync timeouts (~60-90s each) plus several FFI tests in the same magnitude. Whole-workspace run takes 30-40 min depending on cache state. The default 40m timeout is tight; a single substantive push mid-run cancels the test, the next push cancels that one too, and the test never gets to complete.

**Apply — pattern**:

```yaml
# BAD (cascade-cancels in-flight runs on every push):
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

# GOOD (let runs queue, each gets full budget):
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}

# GOOD (scope cancel-in-progress per-job, only safe jobs):
jobs:
  rust-fmt:
    concurrency:
      group: fmt-${{ github.ref }}
      cancel-in-progress: true
  rust-test:
    # NO cancel-in-progress here — let it run to completion
```

**Pair with** the [ci-skip] amendment above: doc-only commits no longer churn the required-check list (saved by [ci-skip]); code-bearing commits no longer cascade-cancel (saved by removing `cancel-in-progress`). Two complementary rules, both shipped via PR #430.

**Pair with** the timeout-bump pattern: when `cargo test --workspace --all-targets` is the required gate, `timeout-minutes` should be ≥ `expected_runtime × 1.5` to absorb cache-miss variance. PR #430 bumped `rust-test` from 40m → 60m on the same edit.

**Anti-patterns**:

- Setting `cancel-in-progress: true` globally on an integration-branch workflow that runs `cargo test --workspace` — every push kills in-flight test runs.
- Trusting the operator's push cadence ("I'll just push one typo fix") — substantive commits land more often than expected, especially during plan-driven multi-step work.
- Bumping `timeout-minutes` without also addressing `cancel-in-progress` — if cancel is the killer, more timeout headroom just means more wasted CI minutes per cancelled run.
- Removing `cancel-in-progress` from a per-push-typo-fix workflow (e.g. a fork-and-fix branch) where it's still useful — this amendment is for integration branches with `cargo test --workspace` as a required check.

**Worked example (PR #430 timeline)**:

| Push | Run | Result | Why |
|------|-----|--------|-----|
| 09:17 — `846778b` (feature scaffold) | Run 7 (head `846778b`) | cancelled @ 09:34 | superseded by Run 8 from `17066ba` push at 09:34 |
| 09:34 — `17066ba` (L24 CHANGELOG bullet) | Run 8 (head `17066ba`) | cancelled @ 09:49 | superseded by Run 9 from `e7f1144` push at 09:48 |
| 09:48 — `e7f1144` ([ci-skip] lessons amendment) | Run 9 (head `e7f1144`) | in progress @ 09:49 | awaiting Rust test completion |

`Rust test` (the only required-check that takes >5 min) never had a chance to finish a single cargo test cycle. Fix landed in commit `45d0669` on `feat/polygon-phase-1-423` (folded into squash `92256ad` on `rust-evm-core`).


## L58 — `k256::SigningKey::from_slice` accepts variable-length byte slices (parses as big-endian scalar mod N)

**Trigger**: 2026-08-30, PR #470 (Issue #469). Initial test wrote `vec![0x42u8; 31]` expecting `PrivateKeySigner::from_slice` to reject non-32-byte input. Test failed (impl returned `Ok`). Re-read the alloy-signer-local source at `~/.cargo/registry/.../alloy-signer-local-1.8.3/src/private_key.rs:fn from_slice` — it delegates to `k256::SigningKey::from_slice(bytes)`, which accepts ANY byte slice and interprets it as a big-endian integer mod curve order N. "Wrong length" is not a rejection criterion; "scalar ≥ N" is.

**Rule**: When writing a unit test that asserts `PrivateKeySigner::from_slice` rejects malformed input, use 32 bytes of `0xFF` (which exceeds the secp256k1 curve order N) to trigger scalar rejection — NOT a short or long slice. 31 bytes of `0x42` silently parses as a valid (but unusual) private key. Apply to any test against the alloy signer stack, the `k256::ecdsa` crate directly, or any sister crate built on it.

**Why**: The sister invariant `decode_signer_bytes_rejects_wrong_length_hex` at `evm-wallet-core/src/wallet.rs:1034` only catches malformed HEX STRING length — it doesn't catch malformed BYTE length on the raw-bytes entry point that #469 introduces (`import_private_key_for_network(name, key_bytes: &[u8], ...)`). The byte-length test gap is invisible to TDD cameras focused on the hex entry point. Without this lesson, future contributors will write the same wrong test and either (a) silently pass with bogus coverage, or (b) defensively add a length check to the lib that breaks the sister `from_slice` semantics.

**Apply**:
- When the surface is `from_slice(&[u8])` (raw bytes), test rejection with `0xFF * 32` (scalar ≥ N).
- When the surface is `from_slice(&str)` (hex), test rejection with `&str` of wrong length (existing sister test).
- When the surface is `from_slice(&SecretKey)` (typed), the lib does its own validation; trust the type.

**Anti-patterns**:
- Assuming `from_slice` enforces a fixed 32-byte input. It does not.
- Adding an explicit `key_bytes.len() != 32` check in a new wrapper around `from_slice` without first confirming the wrapped caller doesn't expect variable-length scalar semantics (e.g. hardware-wallet keys that use a different curve).
- Naming a test `rejects_wrong_key_length` when the actual rejection criterion is scalar range, not length — pin the rejection mechanism in the test name (e.g. `rejects_out_of_range_scalar`).

**Apply — example** (from PR #470 commit `b4194e7`):

```rust
let bad_pk = [0xFFu8; 32]; // > secp256k1 N → triggers from_slice scalar rejection
let err = mgr.import_private_key_for_network("bad-scalar", &bad_pk, &pw, net)
    .expect_err("out-of-range scalar must error");
assert!(matches!(err, WalletError::PrivateKey(_)));
```

PR #470 evidence: commit `b4194e7` + 7 new lib tests (this one + 6 sister tests for polygon-amoy happy path, same-name-different-network uniqueness, dup-name rejection, empty-password rejection, 0o600 blob persistence, `unlock_signer` round-trip).

---

## L59 — clap `#[arg(long, conflicts_with = "...")]` uses the FIELD-NAME-derived arg ID, not the long-flag string

**Trigger**: 2026-08-30, PR #470 (Issue #469). Adding a new `--private-key-file` flag to `WalletAction::Import` (which already had `--private-key` and `--mnemonic`). Initial naive write: `#[arg(long, conflicts_with = "private_key_file")]` on the existing `private_key` field — silently did nothing because clap's `conflicts_with` takes the ARG ID, not the literal flag string. The arg ID for `#[arg(long)]` on a Rust field is the field name (snake_case) — so the right string is `"private_key_file"` (matches the field name) NOT `"--private-key-file"` (the user-facing flag).

**Rule**: When adding a new clap field that must conflict with one or more existing fields, append `conflicts_with = "<other_field_name>"` to BOTH sides of the relationship (the new field + every existing field it conflicts with). Arg IDs are derived from field names automatically (snake_case from Rust convention). User-facing flag strings (`--private-key-file`) are irrelevant to `conflicts_with`. To check an arg ID: `clap_derive` exposes it via `#[arg(id = "explicit")]`; without that override, the field name is the ID.

**Why**: Sister-flag conflicts are critical for security (closing argv-exposure holes, preventing dual-secret paths). If `conflicts_with` is silent no-op because of a wrong reference, the conflict check never fires and the user can pass both flags. TDD tests for the clap parse surface (`Cli::try_parse_from`) DO catch this — that's exactly the role of `cli_rejects_private_key_with_private_key_file` in PR #470 — but only if the test author knows to write the test. Without the test, the silent no-op ships.

**Apply**:
- Adding a new conflicting field: enumerate ALL existing fields that should conflict with it; append `conflicts_with = "<their_field_name>"` to the new field; append `conflicts_with = "<new_field_name>"` to every existing field.
- Use `Cli::try_parse_from(["binary", "sub", ...])` in a unit test to confirm the conflict fires — don't rely on docstring or manual smoke.
- The conflict message should mention BOTH flag names (`msg.contains("--private-key") && msg.contains("--private-key-file")` per the PR #470 test) so the operator sees the full conflict.

**Anti-patterns**:
- `conflicts_with = "--private-key-file"` (with leading `--`) — clap treats it as a literal string match against the arg ID; doesn't match; silent no-op.
- Adding `conflicts_with` on only the NEW field, not the existing ones — clap enforces conflicts only one directionally; both sides needed.
- Assuming `conflicts_with` works for long-flag strings — it doesn't; clap's arg ID system is documented but easy to miss.

**Apply — example** (from PR #470 commit `b4194e7`):

```rust
Import {
    #[arg(long, conflicts_with = "private_key", conflicts_with = "private_key_file")]
    mnemonic: Option<SecretMnemonic>,
    #[arg(long, conflicts_with = "mnemonic", conflicts_with = "private_key_file")]
    private_key: Option<String>,
    #[arg(long, conflicts_with = "mnemonic", conflicts_with = "private_key")]
    private_key_file: Option<PathBuf>,
    // ...
}
```

Every pair appears on both sides. Tested via `cli_rejects_private_key_with_private_key_file` (`polygon/src/handlers/wallet.rs:2098`).

---

## L60 — `sol!` macro `bytecode` attribute expects creation (init) bytecode, NOT runtime bytecode

**Trigger**: Issue #419 / PR #485 (2026-08-31). The `sol!` macro in alloy 1.8.x emits `MockUSDC::BYTECODE` const + `deploy()` helper when `#[sol(bytecode = "0x...")]` is set. The macro expects **creation (init) bytecode** from `solc --bin` — the constructor logic + runtime appended after the `fe` INVALID split, NOT the deployed-only bytecode from `--bin-runtime`. Embedding runtime bytecode deploys but reverts in the constructor (~71K gas consumed, empty code on-chain).

**Rule**: When using `sol! { #[sol(bytecode = "0x...")] contract Foo {} }` to make a contract deployable from the macro, compile via `solc 0.8.X --bin` (creation code), NOT `--bin-runtime` (deployed code). The two differ by ~25% (1223-byte runtime vs 2908-byte creation in the MockUSDC example — creation includes constructor dispatch + runtime).

**Why**: Without this distinction, the deploy succeeds (tx mined, `contract_address` populated) but the constructor reverts silently — Anvil returns `status = false` + `gas_used = ~71K` (early revert), no runtime code ends up at the address, and any `eth_call` returns `0x` empty bytes. The wire-format symptom (empty `eth_call` response) is the same as if you'd forgotten the attribute entirely — root-cause diagnosis via receipt.status + `eth_getCode` is the difference (revert shows `status = false`; missing attribute shows `status = true` + empty code).

**Apply**:

- When wiring a deployable `sol!` contract: compile once via `solc <ver> --bin --optimize-runs <N> --metadata-hash none <Contract>.sol`, paste the `bin` output (NOT `bin-runtime`) into the `bytecode` attribute.
- Embed Solidity source above the `sol!` block + documented regeneration protocol in a header comment (mock example at `rust-wallet-app/spikes/polygon-v1/src/erc20.rs:13-29`) so future contributors can recompile if the contract changes.
- For deploy input, concatenate `MockUSDC::BYTECODE` (creation code) ++ `U256(args...).abi_encode()` (raw args, NO selector — see L61).
- After deploy, assert `eth_getCode(token_addr).len() > 0` as defense-in-depth regression guard against the root cause recurring.

**Anti-patterns**:

- Embedding `bin-runtime` into the `bytecode` attribute — deploy tx mined but constructor reverts silently.
- Forgetting to embed the `Solidity` source reference next to the `sol!` block — future contributors can't recompile without reverse-engineering the macro's ABI.
- Skipping the `eth_getCode` post-deploy check — assumes the deploy succeeded because the receipt.status is true, but the runtime can be empty even on success.

---

## L61 — EVM contract-creation input format: `creation_bytecode ++ abi_encoded_args` (NO 4-byte selector)

**Trigger**: Issue #419 / PR #485 (2026-08-31). `MockUSDC::constructorCall { initialSupply }.abi_encode()` returns `selector (4 bytes) ++ abi_encode(initialSupply)` — but for EVM contract-creation input, the selector must NOT be present; constructor args are appended to init code directly (the init code knows its own constructor signature).

**Rule**: For `deploy_tx.input` of an EVM contract-creation transaction, use `[MockUSDC::BYTECODE, SolValue::abi_encode(&ctor_arg)].concat()` where `ctor_arg` is the constructor arg type directly (e.g. `U256` for `constructor(uint256 initialSupply)`). Do NOT use `MockUSDC::constructorCall::abi_encode()` — that prepends the 4-byte function selector (valid for `eth_call` / `eth_sendTransaction` call paths, INVALID for deploy input).

**Why**: If the selector is included, the init code reads 4 bytes it doesn't expect as part of its constructor-arg payload. For MockUSDC's `constructor(uint256 initialSupply)`, the init code reads the next 32 bytes as `initialSupply` — but with the selector prepended, those 32 bytes are the selector (4 bytes) + 28 bytes of garbage from the actual `initialSupply` arg. The `initialSupply` SSTORE writes a nonsense value, the rest of the constructor logic may or may not succeed depending on what the garbage bytes look like — but crucially the contract is deployed (status = true) with `_balances[msg.sender]` set to a wrong value, so subsequent `transfer` + `balanceOf` tests fail with mismatched math. OR — as observed in #419 — the entire constructor reverts because the garbage value fails an internal check (or the gas accounting differs).

**Apply**:

```rust
// CORRECT — for deploy_tx.input on a contract-creation tx:
let initial_supply = usdc_to_raw(10_000_000);  // U256
let ctor_args = initial_supply.abi_encode();     // SolValue::abi_encode on U256
let deploy_input: alloy_primitives::Bytes = {
    let mut v: Vec<u8> = MockUSDC::BYTECODE.to_vec();
    v.extend_from_slice(&ctor_args);
    v.into()
};

// WRONG — selector prepended:
let ctor_calldata = MockUSDC::constructorCall { initialSupply }.abi_encode();  // 4-byte selector ++ args
let deploy_input: alloy_primitives::Bytes = {
    let mut v: Vec<u8> = MockUSDC::BYTECODE.to_vec();
    v.extend_from_slice(&ctor_calldata);  // wrong — selector contaminates init-code's arg read
    v.into()
};

// CORRECT — for `eth_call` (NOT deploy):
let balance_calldata = MockUSDC::balanceOfCall { account: recipient }.abi_encode();  // selector ++ args = correct here
provider.call(&TransactionRequest::default().to(token_addr).input(balance_calldata.into())).await
```

- Use `MockUSDC::constructorCall::abi_encode()` ONLY for the eventual full EIP-712 / typed-call dispatch paths (post-deploy), NOT for deploy input.
- Use `SolValue::abi_encode(&value)` for any single constructor arg type (U256 / Address / bool / bytes / etc.).
- The mnemonic for selector inclusion vs exclusion: "CALL dispatches via selector (include); CREATE reads args directly from init code (exclude)."

**Anti-patterns**:

- Treating `constructorCall::abi_encode()` as a universal encoding helper — it's specifically for the call path, not deploy.
- Trying to "strip the selector" by slicing the first 4 bytes off `abi_encode()` output — fragile, breaks if the ABI ever gains overloads or non-standard layouts.
- Writing deploy input as `ctor_calldata` with no `MockUSDC::BYTECODE` prepended — the original #419 root cause that started this whole investigation.

---

## L62 — Phase-completion sweep SHA must be the post-merge HEAD, not the PR head (Issue #502)

**Trigger**: Issue #502 (2026-09-01). PR #497 (Phase 2 of #495) squash-merge at `eb360c1` lost the `(Some(ref phrase), None, None)` mnemonic dispatch arm that existed at the PR head `b461d68`. The PR body recorded `25 passed; 0 failed` against the PR-head SHA. No post-merge sweep ran. Phases 3/4/5 inherited a broken parent branch; the loss went undetected until #501's full-suite sweep on the Phase 5 branch surfaced `30 passed; 3 failed` — three of which are the dead mnemonic arm plus two tests that could not have passed as merged (wrong CLI flag names, wrong PK-file on-disk format).

**Rule**: When recording a phase-completion sweep in a tracker issue or PR body, the cited SHA must be the **post-merge HEAD** of the parent branch (or the squash commit itself), never the PR-head commit. PR-head evidence is unverifiable once squash erases the diff.

**Why**: Squashing discards every commit between `main..PR-head`, including any commits added during review. A green sweep at PR head proves nothing about the merged state. The PR-body sweep claim in #497 was technically true for the cited SHA but false for the merged tree — and because no one re-ran the sweep post-merge, the regression survived across three subsequent phase PRs (#499, #500, #501).

**Apply**:

- Before citing a sweep SHA in a PR body / issue body / progress ledger, verify the SHA is the post-merge parent-branch HEAD, not the PR head:

```bash
git rev-parse HEAD               # post-merge parent-branch HEAD
git rev-parse origin/<branch>    # remote parent-branch HEAD
```

The cited SHA MUST equal one of these, not the PR head SHA (`git rev-parse origin/<branch>~0` is still PR head if the PR is open).

- For `gh pr merge --squash`, the **squash commit itself** is the verifiable evidence for a green sweep, because the PR head commit is discarded.
- Phase-completion ledger entries should record the SHA + the merge commit SHA + the command (`gh pr merge --squash <N>`) that produced the merge — three points that any future audit can re-verify.
- This rule covers all sweep-bearing phases (Phase 1 through N of any umbrella issue), not just #495. Audit existing phase-completion claims for similar drift before relying on them.
