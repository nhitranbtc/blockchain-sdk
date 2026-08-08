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
- [L14] ledger rule — `.superpowers/sdd/<plan>/progress.md`, update on pickup/commit/merge, gitignored locally
- [L15] `Secret<T>` where T: Copy defeats zeroize-on-drop
- [L16] `#[derive(ZeroizeOnDrop)]` requires ALL fields to impl Zeroize
- [L17] `Secret<T>` (and any sensitive newtype) needs manual `Debug` impl using `finish_non_exhaustive()` — auto-derive leaks plaintext via `{:?}` formatting; manual impl also satisfies `Result::expect_err` ergonomics

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
| TDD red-green-refactor | `superpowers:test-driven-development` (post-re-evaluation; was `mattpocock-skills:tdd`) |
| Build/cargo error cascade | `superpowers:systematic-debugging` (post-re-evaluation; was `mattpocock-skills:diagnosing-bugs`) |
| Module interface design | `mattpocock-skills:codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair per L13 Q4) |
| Pre-PR review (security, tests, structure) | `pr-review-toolkit:code-review` (parallel sub-agents for Standards + Spec axes) |
| Test coverage gap analysis | `pr-review-toolkit:pr-test-analyzer` |
| Doc / threat-model review | `mattpocock-skills:domain-modeling` (re-invoke; threat model is a domain artifact; was `compound-engineering:ce-doc-review`) |
| Before declaring done | `superpowers:verification-before-completion` |
| Commit + push + PR | `commit-commands:commit-push-pr` |

**Apply**:

- After every `Skill` invocation that returns useful guidance, invoke it AGAIN at the next task step (don't skip).
- If a skill invocation feels redundant with manual approach, the redundancy IS the value — manual approach has unknown blind spots; skill approach has known workflow.
- Negative example: in Task 1.5, I ran `cargo test + cargo clippy + cargo fmt` and declared done. `superpowers:verification-before-completion` would have surfaced "did you check security?" — manual checklist didn't.

**Score-based re-evaluation (2026-08-07, after user asked "why these plugins?"):**

Scored all 9 steps on 5 dimensions (description match / prior use / suite consistency / specificity / caveat awareness, 1-5 each, max 25). Compared L11 pick to score-based winner:

| Step | L11 pick | Score | Score-based pick | Score | Winner | Reason for change |
| --- | --- | --- | --- | --- | --- | --- |
| 1. Task pickup | `mattpocock-skills:domain-modeling` if new domain; `compound-engineering:ce-plan` if multi-step | 21 + 15 | `domain-modeling` + `ce-plan` (same) | 21 + 15 | **Same** | Score-aligned. domain-modeling wins on specificity; ce-plan on multi-step structure. |
| 2. TDD | `mattpocock-skills:tdd` | 17 | `superpowers:test-driven-development` | **19** | **CHANGE → superpowers:tdd** | superpowers suite is more rigorous + well-documented. "NOT manual" was gatekeeping dressed as recommendation. |
| 3. Build error cascade | `superpowers:systematic-debugging` (post-re-evaluation; was `mattpocock-skills:diagnosing-bugs`) | 17 | `superpowers:systematic-debugging` | **19** | **CHANGE → superpowers:systematic-debugging** | Same suite-rigor pattern as #2. systematic-debugging has structured engineering workflow. |
| 4. Module interface | `mattpocock-skills:codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair per L13 Q4) | 19 | `codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair) | 19 + 19 | **CHANGE → pair (per L13 Q4)** | Different lens: codebase-design = seam/deep-module; type-design-analyzer = type-level invariants. Rust-natural. Per L13 max 2 skills/step, pair them. |
| 5. Pre-PR review | `pr-review-toolkit:code-review` | 20 | `pr-review-toolkit:code-review` (same) | 20 | **Same** | Unique parallel sub-agents (Standards + Spec). No competitor. |
| 6. Coverage gap | `pr-review-toolkit:pr-test-analyzer` | 20 | `pr-test-analyzer` (same) | 20 | **Same** | Direct match. No competitor. |
| 7. Doc / threat-model | `mattpocock-skills:domain-modeling` (re-invoke; threat model is a domain artifact; was `compound-engineering:ce-doc-review`) | 14 | `mattpocock-skills:domain-modeling` (re-invoke) | **18** | **CHANGE → domain-modeling re-invoke** | Threat model IS a domain artifact. The same skill that built it can review it. Generic doc-review lacks threat-model awareness. |
| 8. Before declaring done | `superpowers:verification-before-completion` | 20 | `verification-before-completion` (same) | 20 | **Same** | Direct name match. (User rejected adding to L13 step 11 — L11 mapping still recommends it.) |
| 9. Commit + push + PR | `commit-commands:commit-push-pr` | **22** | `commit-push-pr` (same) | 22 | **Same** | Validated 3x (Tasks 0/1/1.5). Only entry with prior-use evidence. |

**Summary of changes:**

| Step | Action | New pick |
| --- | --- | --- |
| 2 | CHANGE | `superpowers:test-driven-development` (was `mattpocock-skills:tdd`) |
| 3 | CHANGE | `superpowers:systematic-debugging` (was `superpowers:systematic-debugging` (post-re-evaluation; was `mattpocock-skills:diagnosing-bugs`)) |
| 4 | CHANGE (add pair) | `mattpocock-skills:codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair per L13 Q4) + `pr-review-toolkit:type-design-analyzer` (was just `codebase-design`) |
| 7 | CHANGE | `mattpocock-skills:domain-modeling` (re-invoke; was `mattpocock-skills:domain-modeling` (re-invoke; threat model is a domain artifact; was `compound-engineering:ce-doc-review`)) |

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
9. TDD red-green cycle (mattpocock-skills:tdd)
10. L12: pre-PR code review FIRST — pr-review-toolkit:code-review
    - Parallel sub-agents: type-design-analyzer + code-reviewer
    - Run on first commit on branch
11. Verify (double gate): cargo fmt + clippy -D warnings + test
    - Per-step AND task-end
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
    - Template at \`docs/templates/per-task-tech-doc.md\`
    - For trivial tasks: 1-line goal + drift table + migration notes minimum
    - Skill-tag pair (per L11): \`compass:docs-writer\` (primary, generates 10-section doc) + \`compass:api-designer\` (secondary, refines API surface + Drift sections)

## Per session
16. At session start: enumerate skills (L11); re-grill pipeline if 5+ tasks since last grill
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
| 4 | Max 2 skills per pipeline step |
| 5 | Fix-loop limit: 3 rounds then PAUSE; round = one review + one fix commit pair |
| 6 | Verify: double-gate (per-step + task-end) |
| 7 | Pre-PR review: parallel sub-agents (`type-design-analyzer` + `code-reviewer`) |
| 8 | Review input: first commit on branch (not uncommitted, not squash-merge candidate) |
| 9 | Off-rails recovery: PAUSE then revert-to-last-green + follow-up issue + ledger entry |
| 10 | Complexity: self-detect + user confirm (hybrid of C + D) |

**Why this spec**:

- Skill-tag per-task pickup (Q2) matches the per-task commit granularity. Per-session was too coarse (Task 0/1/1.5 each had different needs); per-step was overkill.
- Domain-tag-wins (Q3) encodes threat-model-first as the priority order. Without it, simplicity skills (e.g., `code-simplifier`) could undo security-tag work (e.g., `type-design-analyzer`).
- Max 2 skills per step (Q4) bounds cost (~$6-18 per task) without sacrificing rigor.
- Pre-PR review before verify (L12) catches *missing* tests + security gaps that local verify tools cannot.
- Parallel sub-agents (Q7) match the `pr-review-toolkit:review-pr` pattern: both reviews run concurrently, both perspectives land at once.
- Review on first commit (Q8) works for multi-commit PRs (Task 1.5 was 6 commits; squash-merge-candidate broke).
- Self-detect + user confirm (Q10) avoids the inference-error pattern (agent under-estimates trivial-looking security code — exactly what happened in Task 1.5).
- Off-rails recovery (Q9) ensures the codebase never ships broken; PAUSE + revert-to-last-green is the safe default.

**Apply**: every new task follows this spec literally. If a step doesn't apply, log why in the ledger. If a step fails, escalate per Q9. Re-grill the pipeline after 5 tasks (or when a pattern emerges that the spec doesn't cover).

**Display layer (cross-reference)**: every pipeline diagram must use the 6 canonical stages from `CLAUDE.md` Task display rule — **Intent → Rebase → Review → Test → Document → Lint**. L13 owns the process decisions (above); CLAUDE.md owns the display stages. Sub-activities (TDD, L12 review, fix round, merge pause, ledger update) belong in the row's progress detail or as separate notes — not as additional stages.

**Why display layer matters**: Task 5 folded Document + Lint into ad-hoc stages (`Commit + push + PR`, `Ledger + lessons`, `Verify`). The Document work actually happened (module docs, PR body, lessons, threat-model mapping) but was invisible in the pipeline diagram. From Task 6 onward, render pipelines as the 6 stages.

**Document stage checklist** (what Task 5 missed showing):

- Module doc on each new public type (defends / does-not-defend + drift table)
- Threat-model mapping (F5/F6 references in module docs)
- PR body with L9 v3 schema (drift table + technical details + test gaps + migration + per-dimension verdict)
- Lessons.md update (if any new L-number emerged)
- Ledger update (`.superpowers/sdd/<plan>/progress.md`) — pickup / commit / merge events

**Lint stage checklist**:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --workspace --lib`
- `cargo geiger` (geiger count must not grow beyond F53-permitted 2)

**Task 5 retroactive 6-stage pipeline** (for reference):

```text
Pipeline
task/5-encryption                                                       running
  ✓ Intent     (pick up #16, branch task/5-encryption from main, L10 threat model)
  ✓ Rebase     (branched from main; trunk-based, no rebase during task)
  ✓ Review     (L12: 3 parallel sub-agents — security-auditor + type-design + code-review)
  ✓ Test       (TDD + 85 tests passing; 20 new crypto tests)
  ✓ Document   (module docs x4 + drift tables + PR body L9 v3 + lessons L17 + threat-model mapping)
  ✓ Lint       (cargo fmt + clippy -D warnings + cargo test + cargo geiger)
```

Future tasks (Task 6 bip137 onward) must use this 6-stage format.


---

## L14 — Ledger rule

**Trigger**: Session 2026-08-07 dedup — `### Ledger` section removed from CLAUDE.md but rule wasn't re-added to lessons.md; the rule was lost in transit. Caught by `grep -n Ledger tasks/lessons.md` returning empty after the dedup commit.

**Rule**:

- **Track progress in `.superpowers/sdd/<plan>/progress.md`** (gitignored locally — survives compaction, never pushes to remote).
- **Update on three events**:
  - **Pick up** (task start): record issue #, branch name, plan link
  - **Commit** (task progress): record commit SHA, drift notes
  - **Merge** (task complete): record merge commit, closing issue #, all commits
- **After compaction**: trust the ledger over session memory. If they conflict, ledger wins (it was written deliberately; session memory may be compacted and lose detail).
- **Recovery pattern**: if you delete a rule from CLAUDE.md, add it to lessons.md in the same commit. Dedup requires two steps: remove + re-insert. One without the other is a silent rule loss.

**Why**: Workflow rules need a single source. CLAUDE.md is read on every session start, so duplicate rules are confusing ("which version wins?"). lessons.md is the project-local corrections ledger — versioned via L1-L14, append-only. Rules go in lessons.md; agent setup, plugin inventory, and visual templates stay in CLAUDE.md.

**Apply**: For every CLAUDE.md dedup, do a 2-step: (1) add rule to lessons.md in the same commit, (2) remove from CLAUDE.md. Verify with `grep <keyword> lessons.md` after the commit.

---

## L15 — `Secret<T>` where T: Copy defeats zeroize-on-drop

**Trigger**: Tasks 3 + 4 (`Mnemonic::to_seed`, `Signer::secret_bytes`). L12 review (security + type-design) caught multiple CRITICAL/HIGH findings. `Secret<[u8; 32]>`, `Secret<Keypair>`, `Secret<SecretKey>` all wrap Copy types where `*secret.expose()` clones the secret to a fresh stack copy — zeroize-on-drop protects only the wrapper's copy, not the caller's clone.

**Rule**: For secret material, prefer one of:
- `Secret<Vec<u8>>` (heap, non-Copy) — direct storage; zeroize on drop works
- `Secret<Box<T>>` (heap, non-Copy) — for non-Byte array types
- `Zeroizing<T>` + manual `non_secure_erase` after each use — for FFI types where Drop doesn't exist (e.g., `secp256k1::SecretKey: Copy` has no Drop)

For Crypto-style types where the library uses Copy + manual erase (e.g., secp256k1), accept that `Secret<Copy T>` won't compile (no Zeroize impl without `DefaultIsZeroes`); instead reconstruct the Copy type on demand and call `non_secure_erase()` immediately after each FFI use.

**Why**: Zeroize-on-drop is a defense-in-depth mechanism, not a guarantee. It protects only the bytes that pass through the wrapper. Any caller copy, log, or `*` dereference escapes the protection. Copy types are escape hatches by design.

**Apply**: When wrapping secret material, check `T: Copy` first. If yes, either wrap a non-Copy heap type (`Vec`, `Box`) or accept manual `non_secure_erase` per FFI call. Update CONTEXT.md hard rules when the rule applies to project-internal types only.

---

## L16 — `#[derive(ZeroizeOnDrop)]` requires ALL fields to impl Zeroize

**Trigger**: Task 4 `Signer` — tried `#[derive(ZeroizeOnDrop)] struct Signer { secret_bytes: Secret<Vec<u8>>, secp: Secp256k1<All> }`. Compile failed: `Secp256k1<All>: Zeroize` not satisfied because `DefaultIsZeroes` not implemented (it's precomputed-table data, not zeroizable).

**Rule**: When using `#[derive(ZeroizeOnDrop)]`, every field must impl `Zeroize`. If any field is a non-zeroizable type (FFI context, cached computation, precomputed table), either:
1. Replace `#[derive]` with manual `impl Drop` that drops only the secret fields and ignores non-secret fields (e.g., `Secp256k1<All>` — it's not secret material)
2. Wrap the non-secret field in `ManuallyDrop<T>` to suppress its Drop
3. Hide the field behind a `Secret<()>` no-op (overkill)

For `Signer`, manual `impl Drop` is correct: `Secret<Vec<u8>>` field's own `ZeroizeOnDrop` derive fires when `Signer` drops; the `Secp256k1<All>` field is precomputed-table data, not secret material.

**Why**: The derive macro is "all or nothing." It silently skips fields that don't impl Zeroize (per the `ZeroizeOnDrop` derive source) but the type itself still doesn't impl Zeroize. The hidden no-op Drop is a footgun — type-level witness tests (e.g., `assert_zeroize_on_drop::<T>()`) pass, but the actual zeroize is incomplete.

**Apply**: When deriving `ZeroizeOnDrop` on a struct, audit every field's `Zeroize` impl. For precomputed-table / FFI-context types, use manual `impl Drop` instead of derive. Add `// Compile-time witness: <type> drops via Secret<Vec<u8>>::drop` comment in the manual Drop impl.

---

## L17 — Manual `Debug` impl required for `Secret<T>` (and any sensitive newtype)

**Trigger**: Task 5 — added `derive_key -> Result<Secret<Vec<u8>>>` and tests used `Result::expect_err(...)`. Compile failed: `expect_err` requires `T: Debug` where `T` is the `Ok` variant. `Secret<T>` has no `Debug` impl (intentionally — auto-derive would leak plaintext via `{:?}` formatting).

**Rule**: For any newtype that wraps sensitive material (`Secret<T>`, `Mnemonic`, `XPrvHolder`, `Signer`, future `EncryptedBlob` etc.), provide a manual `impl Debug` that hides the inner value. Use `f.debug_struct("TypeName").finish_non_exhaustive()` — renders as `TypeName { .. }` with no field names (avoids field-name collisions with the BIP-39 wordlist per CONTEXT.md hard rule #7).

**Why**:

1. Auto-derive `Debug` defeats the wrapper's purpose: `format!("{secret:?}")` would print the plaintext key bytes.
2. `Result::expect_err()`, `Result::unwrap()`, and most error-handling combinators require `T: Debug` on the `Ok` variant. Without it, tests can't write `expect_err(...)` for the negative path.
3. `finish_non_exhaustive()` is the canonical pattern across `Mnemonic`, `XPrvHolder`, `Signer`, and now `Secret` — consistent project convention.

**Apply**: At every new sensitive newtype, add the manual Debug impl at the same time as the type declaration (don't defer to the test-author to discover it). Caught early in Task 5 (post-write, compile-error on first test run). Preview for Task 6 `bip137` types and any future `EncryptedBlob` / `MnemonicCipher` newtypes.

---
