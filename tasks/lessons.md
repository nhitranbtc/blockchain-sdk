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
- [L13] per-task pipeline spec (10 decisions, 2026-08-07 grill)
- [L14] ledger rule — `.superpowers/sdd/<plan>/progress.md`, update on pickup/commit/merge/grill, gitignored locally
- [L21] Update estimate-report AND ai-cost-report on every PR merge (status, progress, in-flight count, merge SHAs)
- [L22] Fact-forcing gates: state facts, then retry
- [L23] `git stash -u -- <path>` deletes untracked files matched by path
- [L24] On PR merge: update CHANGELOG.md (Keep a Changelog) + README "What's New"
- [L25] On PR merge: flip CHANGELOG.md user-story checkboxes + update "Try it" instructions (edit on working branch + commit with the sub-task/code change — no separate process branch needed)
- [L26] Sub-task workflow for large tasks: parent branch + sequential merge + PR-to-parent (not main). L21/L24/L25 doc updates travel WITH the sub-task PR on the working branch — no separate process branch.
- [L27] Read API before assuming — grep `#[derive(...)]` and variant names on the actual type before writing code that uses those traits

> **Index gaps (L15–L20):** entries were added then trimmed during session 2026-08-10. L15/L16/L17 were Secret<T> / ZeroizeOnDrop / Debug patterns. L18/L19 were review findings (doc-test + merge gate). L20 was estimate-report self-improvement (replaced by client-bill pivot). All removed per user direction; rules not currently in scope.

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
| `Trade-off` | explicit cost the fix imposed (perf, complexity, API surface, deps) — required per antipattern 5 |
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
| TDD red-green-refactor | `superpowers:test-driven-development` (post-re-evaluation; was `mattpocock-skills:tdd`) |
| Build/cargo error cascade | `superpowers:systematic-debugging` (post-re-evaluation; was `mattpocock-skills:diagnosing-bugs`) |
| Module interface design | `mattpocock-skills:codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair per L13 Q4) |
| Pre-PR review (security, tests, structure) | `pr-review-toolkit:code-review` (parallel sub-agents for Standards + Spec axes) |
| Test coverage gap analysis | `pr-review-toolkit:pr-test-analyzer` |
| Doc / threat-model review | `mattpocock-skills:domain-modeling` (re-invoke; threat model is a domain artifact; was `compound-engineering:ce-doc-review`) |
| Document stage (per-task tech doc → PR body) | `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections) |
| Before declaring done | `superpowers:verification-before-completion` |
| Commit + push + PR | `commit-commands:commit-push-pr` |

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
- L13 step 10 enforces this sequence; L15 trigger "L12 review (security + type-design) caught multiple CRITICAL/HIGH findings" refers to this same gate.
- Sub-agent lens coverage: `type-design-analyzer` (encapsulation, invariant expression, type-level soundness) + `code-reviewer` (correctness, security, convention). Run concurrently, both perspectives land at once.
- For `critical` complexity tier (per L13), add `pr-review-toolkit:security-auditor` as a third sub-agent (max 3 skills per step under Q4 carve-out).

---

## L13 — Per-task pipeline spec (10 decisions, 2026-08-07 grill)

**Trigger**: Session 2026-08-07. User invoked `mattpocock-skills:grilling` to stress-test the per-task pipeline template. 10 questions, 10 decisions. Output: revised pipeline spec.

**Rule** (the spec):

```text
## Pre-pickup
1. L11: enumerate loaded skills; tag 3-5 relevant to active task
2. (Self-detect complexity) → propose "trivial / normal / critical" → user confirms

## Per task
3. Pick up issue; read threat model (L10); read CONTEXT.md hard rules
4. karpathy-guidelines + branch checkout

## Per pipeline step
5. Step: pick skill pair (max 2) from L11 map
6. Skill #1: invoke
7. Skill #2: invoke (if applicable)
8. Domain-tag wins on conflict: security > correctness > simplicity

## Per task
9. TDD red-green cycle (superpowers:test-driven-development)
10. L12: pre-PR code review FIRST — pr-review-toolkit:code-review
    - Parallel sub-agents: type-design-analyzer + code-reviewer
    - Run on first commit on branch
11. Verify (double gate): cargo fmt + clippy -D warnings + test
    - Per-step AND task-end
    - *Note*: L11 recommends also invoking `superpowers:verification-before-completion` at this step. User rejected adding it to L13 (2026-08-07) — L11 mapping still recommends it; L13 spec stays literal. If invoking it, do so as a wrapper around the cargo commands, not as a replacement.
11a. **Backlog triage** (when verify surfaces an error that can't be fixed in-task):
    - **Fixable now**: fix in current commit, re-verify, continue
    - **Small deferred** (cosmetic, follow-up): log in current session's backlogs list
    - **Big task** (multi-PR, multi-week): create GitHub issue, label `backlog`, link to parent task
    - **Future milestone** (v0.1.1, v0.2): log in current session's backlogs list with priority tag
    - GitHub issue format: title `Backlog: <short description>`, body = acceptance criteria + priority + parent task ref, labels = `backlog` + `priority/p0|p1|p2|p3` + `week/N` (if applicable), milestone = parent task's milestone
    - When in doubt: write the issue. Forgetting backlogs costs more than the 30-60s to file one.
12. PAUSE for commit approval
    - Max 3 fix rounds; round = one review + one fix commit pair
13. commit-commands:commit-push-pr
14. Flip issue checkboxes [ ]→[x]
15. PR review (parallel sub-agents) + merge + close
    - If stuck 3 rounds: PAUSE then revert-to-last-green + follow-up issue + ledger entry
15a. **Write technical document → enrich PR body** (before merge):
    - 10 sections: Goal, Drift from plan, API surface, Threat-model coverage, Implementation, Tests, L12 review, Lessons captured, Backlog (links to `backlog` issues), Migration notes
    - Append/replace existing PR body with the full doc
    - Document lives with the commit (audit trail); no separate file to maintain
    - Skill-tag pair (per L11; Document stage of the 6-stage pipeline): `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections)

## Per session
16. At session start: enumerate skills (L11); re-grill pipeline if 5+ tasks since last grill. Track grill count in the ledger (per L14) — counter resets after a grill event.
17. Update ledger after merge
18. Add new lessons if user corrections or novel patterns (L9 schema)
```

**Complexity tier → pipeline variation** (self-detect + user confirm):

| Tier | Pipeline |
| --- | --- |
| `trivial` (doc-only / single-line) | doc-review only; skip pre-PR code review |
| `normal` (typical feature) | full pipeline: TDD + code-review + verify + PAUSE + commit + post-PR review |
| `critical` (security-sensitive: key material / signing / encryption / network / persistence) | full + extra skill (e.g., `pr-review-toolkit:security-auditor`) |

**10 decisions (the grilling record)**:

| Q | Decision |
| --- | --- |
| 1 | Goals: A (correctness) + C (learning) — speed + reversibility deprioritized |
| 2 | Skill-tag: per-task pickup (not session-start, not per-step) |
| 3 | Skill-conflict resolution: domain-tag wins; security > correctness > simplicity |
| 4 | Max 2 skills per pipeline step (`critical` tier: max 3 — see complexity tier table) |
| 5 | Fix-loop limit: 3 rounds per task then PAUSE; round = one review + one fix commit pair. Shared budget across pre-commit (step 12) and post-PR-review (step 15). Exceed → PAUSE + revert-to-last-green + follow-up issue + ledger entry (Q9). |
| 6 | Verify: double-gate (per-step + task-end) |
| 7 | Pre-PR review: parallel sub-agents (`type-design-analyzer` + `code-reviewer`) |
| 8 | Review input: first commit on branch (not uncommitted, not squash-merge candidate) |
| 9 | Off-rails recovery: PAUSE then revert-to-last-green + follow-up issue + ledger entry |
| 10 | Complexity: self-detect + user confirm (hybrid of C + D) |

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

---

## L22 — Fact-forcing gates: state facts, then retry

**Trigger**: Session 2026-08-10 (F21 typed Sighash work, process docs work). Multiple Write/Edit calls were denied by `pre:edit-write:gateguard-fact-force` requiring pre-tool facts:

1. List ALL files that import/require the file (or its dependents).
2. List public functions/classes affected by the change.
3. If reads/writes data files, show field names, structure, date format.
4. Quote the user's current instruction verbatim.

**Rule**: When a Write/Edit call is denied by the gate:

1. **State the 4 facts concisely** in plain text — under 10 lines. No preamble, no narrative.
2. **Retry the same tool call** with `old_string`/`new_string` unchanged (or Write unchanged).
3. **The gate is not personal** — it's a fact-checker. State, retry, continue. Don't argue or work around (e.g., use Bash to bypass).
4. **For a new file**: importers = future-self readers; callers = functions that depend on it; affected API = types/functions in the new file; schemas = data shape; verbatim = current user message.
5. **For an existing file edit**: importers = files that include/use it; callers = public API consumers; affected = changed function/struct surface; schemas = data shape; verbatim = current user message.

**Why**: The gate enforces the principle "before changing X, know what depends on X and what changes." Bypassing it via Bash or different tools skips the consideration. State-the-facts also serves as a mental pre-flight check.

**Apply**:

- Hit a gate denial → state 4 facts, retry, don't waste turns explaining.
- First edit of a new file = heaviest fact load (no existing context to reference).
- Repeat edits to same file in a session = lighter fact load (already-stated facts may suffice; restate only the changed aspects).
- Per-edit fact-statement overhead is real (~5-15 sec per edit). Budget accordingly in cost-sensitive work.

---

## L23 — `git stash -u -- <path>` deletes untracked files matched by path

**Trigger**: Session 2026-08-10. Attempted to isolate process docs (`docs/estimate-report.md`, `docs/ai-cost-report.md`) from `#31` branch via `git stash push -u -m "..." -- tasks/lessons.md docs/estimate-report.md docs/ai-cost-report.md`. The two `docs/` files were **untracked** (newly created, never committed). With `-u` (include untracked) + path matching, git **removed** the untracked files from working tree. The stash held them, but `git stash drop` later discarded the stash with no recovery path.

**Rule**:

- `git stash push -u -- <path>` (with `-u`) **removes** untracked files matched by `<path>` from working tree, stashing them. This is `git stash`'s documented behavior, but surprising if you expect stash to preserve files.
- For **isolating work across branches**: use `git stash push -u -- <specific-tracked-paths>` — make sure the paths you pass are tracked files. Don't include untracked file paths unless you intend to delete them.
- **Recovery**: stash reflog (`git stash list`) shows dropped stashes if you have it enabled. Default reflog retains dropped stashes for ~30 days (configurable via `gc.reflogExpire`).
- **Better alternative**: stage the work, create a new branch (`git checkout -b`), commit on the new branch. Avoids the surprise-delete entirely.

**Why**: Untracked files are first-class stash targets with `-u`. The `-- <path>` filter applies to BOTH tracked and untracked, but the deletion only affects untracked. Counterintuitive.

**Apply**:

- Before `git stash -u -- <paths>`, verify each path is tracked (`git ls-files --error-unmatch <path>`). For untracked paths, prefer branch-and-commit instead.
- After accidental delete: check `git fsck --lost-found` + `git stash list` (reflog may have it).
- For doc/process work that doesn't deserve its own branch yet: commit in-place on a fresh branch from main, not via stash.

---

## L24 — On PR merge: update CHANGELOG.md (Keep a Changelog) + README "What's New"

**Trigger**: Session 2026-08-10. After merging PR #42 (Task 8 coin_type_for), user said: "I have a feebacks are add changlog, after merged PR need to update user facing that handled". Two artifacts captured the feedback: a CHANGELOG.md for cumulative release history, and a README "What's New" section for at-a-glance user visibility.

**Rule**: After every PR merge into `main`:

1. **`CHANGELOG.md`** (top-level, Keep a Changelog format): append an entry to the `[Unreleased]` section under one of `### Added` / `### Changed` / `### Fixed` / `### Security` / `### Deprecated` / `### Removed`. One bullet per user-visible change. Cite the PR number.
2. **`rust-wallet-app/README.md`** "What's New" section: add a one-line summary of the merged PR. Format: `- **PR #N** (Task X) — <feature summary>`. Link to `CHANGELOG.md` for full history.
3. **At release time**: cut a versioned section (e.g., `## [v0.1] — 2026-08-10`) by moving all `[Unreleased]` entries under the new version header. Then reset `[Unreleased]` to empty.

**Why**: Two audiences for the same change:
- **CHANGELOG.md**: cumulative record, machine-parseable, git-blame-friendly, future contributors ask "what changed between v0.1 and v0.2?" — answers without reading commit history.
- **README "What's New"**: at-a-glance for users evaluating the project. Without it, the README doesn't reflect the current state.

**Apply**:

- After `gh pr merge <N>` succeeds (alongside L21 reports update): append CHANGELOG entry + README line in the same commit on a fresh branch (e.g., `docs/changelog-update-pr-N`), then push + PR.
- README "What's New" rolls up the most recent 5-10 PRs; older entries stay in CHANGELOG only.
- CHANGELOG entries are terse — one line per change, PR number, no prose.
- For breaking changes: use `### Changed` with a `**BREAKING**:` prefix on the bullet.

**Anti-patterns**:

- Updating only README (loses cumulative record) OR only CHANGELOG (loses at-a-glance surface).
- Long CHANGELOG prose paragraphs — bullets only, terse.
- Forgetting to bump `[Unreleased]` to a versioned section at release — leaves history unreleased forever.
- Committing CHANGELOG/README updates in the same PR as the code change (couples release notes to feature commit; harder to amend notes independently).

---

## L25 — On PR merge: flip CHANGELOG.md user-story checkboxes + update "Try it"

**Trigger**: Session 2026-08-10. User feedback: "Read user stories, check list boxes after merged, update what user cases finished and we can playaround with them". The Keep a Changelog format (L24) tracks per-PR changes; this rule tracks per-**user-story** capabilities — a separate lens. User stories = what users *can do* with the codebase, distinct from what *changed*.

**Rule**: `CHANGELOG.md` has a **User Stories** section: a table with columns `#`, `Story`, `Status`, `Try it`. After every PR merge:

1. **If the merged PR completes a user story** (adds a public API, ships a CLI command, makes a previously-gated feature testable): flip the corresponding checkbox from `[ ]` to `[x]` in the Status column.
2. **Update the "Try it" column** with a one-line instruction — `cargo test -p bitcoin-wallet-core <module>` for library demos, `<subcommand>` for CLI commands, etc.
3. **Drift detection**: if the merged PR doesn't complete any story but introduces a defense-in-depth change (compile-time check, audit, lint), it doesn't get a user-story checkbox — but it should still get a per-PR entry under the regular `[Unreleased]` section.

**Why**: Three audiences for the same change:

- **Per-PR changelog** (L24): cumulative record, machine-parseable, "what changed between v0.1 and v0.2?"
- **Per-story changelog** (L25): at-a-glance for clients, "what can I do with this codebase today?"
- **README "What's New"** (L24): top-of-funnel visibility, "what's new since I last looked?"

The user-story view answers the client question "is feature X ready to use?" without reading git history.

**Apply**:

- After `gh pr merge <N>` succeeds (alongside L21 reports update + L24 changelog update): check the User Stories table for any story completed by the PR.
- Each story has 3 attributes: descriptive title, status checkbox, "Try it" command. Update in the same commit on a fresh branch.
- Story titles use user-facing verbs: "Sign messages", "Encrypt with password", "Sync wallet" — not implementation details.
- For Task 9 (Wallet end-to-end) stories (#10–#13 in the User Stories table), the stories get flipped when Issue #19 merges. No need to flip them per-task during #19 work — the merge is the trigger.

**Anti-patterns**:

- One user story per commit / per PR line — confuses "feature" with "commit." Group multi-commit features under one story.
- Forgetting to update "Try it" — the column is the value; checkbox flips without command examples is just busywork.
- Flipping the box speculatively before the merge ("I'll merge it later") — same drift problem L24 solves for changelog entries.
- Marking a story "done" when the implementation is partial (e.g., "Create wallet from mnemonic" works but doesn't yet sync) — split into smaller stories instead.
- Fabricating "Try it" commands without verifying the path exists. Story #8 (`default_is_testnet_per_hard_rule_1`) on session 2026-08-10 had a test name that no longer existed after the wrapper refactor — caught by review. **Maintenance caveat:** before v0.2, add a CI step that runs `cargo test --no-run` for each "Try it" module path and fails the build if the path resolves to zero tests. Without automation, the column goes stale silently.

## L26 — Sub-task workflow for large tasks: parent branch + sequential merge + PR-to-parent (not main)

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

## L27 — Read API before assuming: grep `#[derive(...)]` and variant names on the actual type before writing code that uses those traits

**Trigger**: Session 2026-08-10. Multiple compile errors from incorrect API assumptions, all caught late (after impl was written, during TDD GREEN):

1. `Secret<String> is Clone` — **false**. Only `ZeroizeOnDrop` derived. No `Clone` impl. Original `#19a` impl stored `Secret<String>` field via `mnemonic.clone()` which doesn't compile (`Mnemonic` wraps `Secret<bip39::Mnemonic>`, also not Clone).
2. `Error::Mnemonic(String)` — **doesn't exist**. Actual variant is `Error::InvalidMnemonic(String)`. Caused 2+ minutes of compile-error back-and-forth.
3. `static_assertions::assert_not_impl_all!` — crate **not in deps**. Tried to use as Default-impossible witness; had to fall back to compile-time absence comment.

**Rule**: Before writing code that uses a type's traits or enum variants:

1. **For derives**: `grep -A 5 'pub struct X' path/to/type.rs` or `cargo doc --document-private-items` — see the actual `#[derive(...)]` line. **Never** assume `Clone`, `Debug`, `Default`, `Eq`, or any other trait is implemented.
2. **For enum variants**: `grep -E '^\s+[A-Z][a-zA-Z]+,?$' path/to/enum.rs` — list all variants. **Never** assume `MyEnum::MyVariant` exists; variants may be renamed, removed, or have different shape.
3. **For external crates**: check `Cargo.toml` for the crate before using its types. `cargo tree --package <name>` confirms the version.

**Why**: Each compile error costs ~2-5 min of round-trip + state-modifying attempts (gate denials, re-Reads, re-Edits). 3 errors in #19a = ~10-15 min wasted. Pre-flight grep costs ~30 sec. Net savings on every multi-error task.

**Apply**:

- Before writing a struct field of type `Secret<T>`: confirm `T: Zeroize + ZeroizeOnDrop` (required by `Secret<T>`'s generic bound). If T isn't `Zeroize`, wrap differently (per L15).
- Before calling `Foo::default()`: confirm `Foo: Default` (grep `impl Default for Foo` or look for `#[derive(Default)]`).
- Before matching `Enum::Variant`: `grep 'Variant,' path/to/enum.rs` or check the actual definition.
- Before using an external crate's macro or type: confirm the crate is in `[dependencies]`.

**Anti-patterns**:

- Writing the impl, hitting a compile error, fixing, hitting another error, fixing, etc. — each iteration costs more than a single grep would have.
- "I'll just try it and see if it compiles" — multiplies error rounds. Read once, write once.
- Trusting LLM memory of crate APIs — crates change between versions; what's true for crate X v0.32 may not be true for v0.31.

---
