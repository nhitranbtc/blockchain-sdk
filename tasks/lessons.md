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

> **Index gaps (L15–L20):** entries were added then trimmed during session 2026-08-10. L15/L16/L17 were `Secret<T>` / ZeroizeOnDrop / Debug patterns. L18/L19 were review findings (doc-test + merge gate). L20 was estimate-report self-improvement (replaced by client-bill pivot). All removed per user direction; rules not currently in scope.
>
> **Audit (2026-08-10):** L10, L22, L23, L27 removed per user direction. L10 (threat-model re-read) — type-system invariants + L11/L12 review pair make the rule redundant. L22 (fact-forcing gate) — enforced at hook layer, captured in `~/.claude/CLAUDE.md` global memory instead. L23 (`git stash -u -- <path>` deletes untracked) — git-native behavior, covered by `git stash` docs. L27 (grep `#[derive(...)]` before using traits) — type-checker errors surface the assumption fast enough; pre-flight grep added latency without saving compile cycles.

### Domain map

| Domain | Lessons |
|---|---|
| Build / Cargo hygiene | L1 |
| Git workflow | L6 (approval gates), L8, L14, L42, L46, L48 |
| Issue/PR protocol | L9, L24, L47 |
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
| Task pickup (drift scan, per L30)            | `git log --all -- <path>` for every plan/spec SHA cited in the picked-up issue. Empty = drift; commit artifact or file follow-up before feature work starts. |
| Task pickup (new feature, no existing plan)  | `feature-dev:feature-dev` — 7-phase discovery → explore → clarify → architect → implement → review → summary. Use when feature unclear or scope undecided; phases 1-4 produce an ad-hoc plan that L13 then owns from step 9 onward. |
| Plan authoring / plan review                 | `tasks/plan-lesson.md` (PL1, PL2, PL3, PL7–PL16) — drift scan, story trace, plugin stack, host-first SDK design, step-by-step workflow |
| Code review / SDK quality                    | `tasks/review-lesson.md` (PL4, PL5, PL6, PL17) — flat re-exports, async mutex, stability policy, review plugins |
| Deep search / content review / code-block    | `tasks/search-lesson.md` (PL18, PL19, PL20) — content review, code-block review, deep search + agent management |
| TDD red-green-refactor                       | `superpowers:test-driven-development` (post-re-evaluation; was `mattpocock-skills:tdd`)                                              |
| Build/cargo error cascade                    | `superpowers:systematic-debugging` (post-re-evaluation; was `mattpocock-skills:diagnosing-bugs`)                                     |
| Module interface design                      | `mattpocock-skills:codebase-design` + `pr-review-toolkit:type-design-analyzer` (pair per L13 Q4)                                     |
| Pre-PR review (security, tests, structure)   | `pr-review-toolkit:code-review` (parallel sub-agents for Standards + Spec axes)                                                      |
| Test coverage gap analysis                   | `pr-review-toolkit:pr-test-analyzer`                                                                                                 |
| Doc / threat-model review                    | `mattpocock-skills:domain-modeling` (re-invoke; threat model is a domain artifact; was `compound-engineering:ce-doc-review`)         |
| Document stage (per-task tech doc → PR body) | `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections) |
| Before declaring done                        | `superpowers:verification-before-completion`                                                                                         |
| Commit + push + PR                           | `commit-commands:commit-push-pr`                                                                                                     |
| Pre-PR security review (critical tier, after L12) | `security-review` (standalone, comprehensive: secrets, SSRF, authz, trust boundaries, crypto, multi-tenancy) |
| Pre-commit plugin structure validation (when trigger matches per L49) | `plugin-dev:plugin-validator` |

> **Skill-pair wrappers (2026-08-11):** `pr-review-toolkit:code-review` is the
> toolkit; the superpowers meta-skills wrap its invocation. Pre-PR (L13 step 10)
> pairs `superpowers:requesting-code-review` with the toolkit. PR feedback
> (L13 step 15, the 3-round fix loop) pairs `superpowers:receiving-code-review`
> with the toolkit. Treat the superpowers skill as the entry point; the toolkit
> is the parallel-sub-agent driver inside it.

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
- `security-review` is read-only — produces findings; does not modify code. Apply findings via the same fix-loop as L12 review findings.
- Q4 max-3 cap unaffected: `security-review` is a separate gate, not a sub-agent in the parallel cluster. Counts as 1 skill for the next pipeline step (e.g. step 11 verify).

---

## L13 — Per-task pipeline spec (10 decisions, 2026-08-07 grill)

**Trigger**: Session 2026-08-07. User invoked `mattpocock-skills:grilling` to stress-test the per-task pipeline template. 10 questions, 10 decisions. Output: revised pipeline spec.

**Rule** (the spec):

```text
## Pre-pickup
1. L11: enumerate loaded skills; tag 3-5 relevant to active task
2. (Self-detect complexity) → propose "trivial / normal / critical" → user confirms

## Per task
3. Pick up issue. Read body. Check if large task or sub-task — see [Sub-task workflow for large tasks](#sub-task-workflow-for-large-tasks) below.
4. karpathy-guidelines + branch checkout (from integration branch if sub-task per step 3)
    - **L46 — record expected branch:** note the branch name just checked out (e.g., scratch, ledger per L14). Every later L46 check reads from this record.
4a. **Drift scan (per L30):** before starting feature work, verify every plan/spec/SHA citation referenced by the picked-up issue. For each cited `<path>`, run `git log --all -- <path>`. Empty result = drift (artifact never committed or SHA never existed); resolve by committing the artifact or filing a follow-up issue before feature work begins. Drift is silent — cargo fmt/clippy/test don't catch it; only `git log` reveals the gap.

## Per pipeline step
5. Step: pick skill pair (max 2) from L11 map
5a. **No-plan branch:** if no plan/spec exists for the picked-up issue, defer to `feature-dev:feature-dev` instead of L13's TDD→review→verify chain. Output of feature-dev phases 1-4 = ad-hoc plan; resume L13 at step 9 (TDD) once the plan lands.
6. Skill #1: invoke
7. Skill #2: invoke (if applicable)
8. Domain-tag wins on conflict: security > correctness > simplicity

## Per task
9. TDD red-green cycle (superpowers:test-driven-development)
10. L12: pre-PR code review FIRST — `superpowers:requesting-code-review` wrapping `pr-review-toolkit:code-review`
    - Parallel sub-agents: type-design-analyzer + code-reviewer
    - Run on first commit on branch
11. Verify (triple gate): `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace`
    - Run AFTER each fix commit AND at task-end (before final commit-push-pr).
    - All three must pass before the task-end commit. A single failing gate = task is not done.
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
- If the commit touches any plugin-structure file (per L49 trigger), invoke `plugin-dev:plugin-validator` BEFORE the step 12 commit PAUSE. Read-only agent; findings feed the same fix loop as L12 review.
    - **Format-verification plugin** (2026-08-12 grill): the `cargo fmt --check` gate is the only Rust-quality check bundled into a dedicated plugin. Subagent `ecc:rust-build-resolver` runs `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` + `cargo tree --duplicates` (+ `cargo audit` if installed) in one invocation; slash command `/ecc:rust-build` wraps the same agent. `ecc:rust-reviewer` (or `/ecc:rust-review`) runs the same fmt check on modified `.rs` files after a code-review pass. Other Rust-engineer agents (`compass:rust-engineer`, `voltagent-lang:rust-engineer`) apply style by writing idiomatic code on first pass — they do NOT expose a discrete `cargo fmt --check` step. `caveman:cavecrew-reviewer` intentionally skips formatting nits unless they change meaning — wrong tool for rustfmt policing. Use `/ecc:rust-build` for one-shot verify; use `/ecc:rust-review` for fmt-check paired with review.
11a. **Backlog triage** (when verify surfaces an error that can't be fixed in-task):
    - **Fixable now**: fix in current commit, re-verify, continue
    - **Small deferred** (cosmetic, follow-up): log in current session's backlogs list
    - **Big task** (multi-PR, multi-week): create GitHub issue, label `backlog`, link to parent task
    - **Future milestone** (v0.1.1, v0.2): log in current session's backlogs list with priority tag
    - GitHub issue format: title `Backlog: <short description>`, body = acceptance criteria + priority + parent task ref, labels = `backlog` + `priority/p0|p1|p2|p3` + `week/N` (if applicable), milestone = parent task's milestone
    - When in doubt: write the issue. Forgetting backlogs costs more than the 30-60s to file one.
12. PAUSE for commit approval
    - Max 3 fix rounds; round = one review + one fix commit pair
    - **L46 — pre-pause destination check:** run `git branch --show-current` and confirm it equals the branch recorded at step 4. If mismatch → `git checkout <expected>`, re-verify, then proceed. The branch name MUST appear verbatim in the approval prompt per L6 ("Show branch name in the approval prompt").
13. commit-commands:commit-push-pr
    - **L46 — pre-execute destination re-check:** run `git branch --show-current` again immediately before invoking `commit-push-pr`. Even if step 12's check passed, HEAD may have moved (post-merge housekeeping, `git checkout -`, IDE tab switch). Mismatch → STOP, re-checkout expected branch, then proceed. Pair with L42 (`git diff --cached --stat`): L42 audits content, L46 audits destination, both at the same gate.
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
15a. **Write technical document → enrich PR body** (before merge):
    - 10 sections: Goal, Drift from plan, API surface, Threat-model coverage, Implementation, Tests, L12 review, Lessons captured, Backlog (links to `backlog` issues), Migration notes
    - Append/replace existing PR body with the full doc
    - Document lives with the commit (audit trail); no separate file to maintain
    - Skill-tag pair (per L11; Document stage of the 6-stage pipeline): `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections)
15b. **Apply L24** — verify CHANGELOG `[Unreleased]` bullet + User Stories table checkbox flip + "Try it" command landed in the merged code (per step 11b's local-branch rule, they should already be there). At release-cut time: move accumulated `[Unreleased]` entries under `## [vN] — YYYY-MM-DD` and reset `[Unreleased]` empty.
15c. **Review all L13 steps 1-15b completed** (broader pre-merge gate — widens 15d's PR-body checklist to all L13 steps):
    - **Walk each L13 step 1 through 15b** and confirm artifact exists before merging:
        - Step 1 (L11 skill tag — recorded in branch commits or PR body)
        - Step 2 (complexity tier self-detected + user-confirmed)
        - Steps 3-4 (issue picked up, branch checked out)
        - Steps 5-8 (skill pair invoked per L11 map, domain-tag wins on conflict)
        - Step 9 (TDD red-green cycle: failing test first, then GREEN pass)
        - Step 10 (L12 pre-PR review findings applied — commit references each fix)
        - Step 11 (verify gate clean: `cargo fmt --check` + `clippy -- -D warnings` + `cargo test` output captured)
        - Step 11a (backlog triage done; follow-up issues filed for any deferred work)
        - Step 11b (L24 cascade on local branch BEFORE merge — CHANGELOG + Story flip in commits that travel with the feature PR)
        - Step 12 (commit approval PAUSE honored — user said "approved" or "commit" before each `git commit`)
        - Step 13 (commit-push-pr executed — branch pushed + PR opened)
        - Step 14 (issue checkboxes flipped with artifact evidence per L13 step 14 rules)
        - Step 15 (PR review by parallel sub-agents per L13 step 15)
        - Step 15a (10-section tech doc appended to PR body — Goal, Drift, API surface, Threat-model, Implementation, Tests, L12 review, Lessons, Backlog, Migration)
        - Step 15b (L24 cascade verified in merged code path)
    - **Why a separate gate**: 15d's PR-body checklist is narrow (boxes in PR body only). 15c widens to all L13 steps — catches gaps in TDD evidence, L12 review, verify gate, L24 cascade, skill-tag pair, etc. that the PR body doesn't necessarily surface.
    - **Output**: either (a) all steps verified → proceed to 15d merge gate, or (b) gaps found → fix (commit amend, follow-up issue, or PR body update) before merge.
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
    - **Run the merge:** `gh pr merge <N> --squash --admin --delete-branch`. The `--delete-branch` removes both local + remote task branch in one call.
    - **Verify issue closed:** `gh issue view <N> --json state` should report `CLOSED`. Squash-merge commit messages containing `Closes #N` / `Fixes #N` auto-close; otherwise `gh issue close <N>` explicitly.
    - **Verify main updated:** `git fetch origin main && git log --oneline origin/main -1` shows the merge SHA at HEAD. Branch protection + admin merge can be silent — verify explicitly per L28.
    - **No rollback:** if merge landed in a wrong state, use `git revert -m 1 <merge-sha>` rather than `git reset --hard`. Merges are immutable public artifacts (per L6).
    - 10 sections: Goal, Drift from plan, API surface, Threat-model coverage, Implementation, Tests, L12 review, Lessons captured, Backlog (links to `backlog` issues), Migration notes
    - Append/replace existing PR body with the full doc
    - Document lives with the commit (audit trail); no separate file to maintain
    - Skill-tag pair (per L11; Document stage of the 6-stage pipeline): `compass:docs-writer` (primary, generates 10-section doc) + `compass:api-designer` (secondary, refines API surface + Drift sections)

## Per session
16. At session start: enumerate skills (L11); re-grill pipeline if 5+ tasks since last grill. Track grill count in the ledger (per L14) — counter resets after a grill event.
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
| `trivial` (doc-only / single-line)                                                           | doc-review only; skip pre-PR code review                                    |
| `normal` (typical feature)                                                                   | full pipeline: TDD + code-review + verify + PAUSE + commit + post-PR review |
| `critical` (security-sensitive: key material / signing / encryption / network / persistence) | full + extra skill (e.g., `pr-review-toolkit:security-auditor`)             |
| `feature-dev path` (no prior plan / scope undecided)                                          | `feature-dev:feature-dev` phases 1-4 (discover → explore → clarify → architect) produce ad-hoc plan; then L13 steps 9-15d own TDD → review → verify → PAUSE → commit-push-pr → PR review → tech doc → ledger |

**10 decisions (the grilling record)**:

| Q   | Decision                                                                                                                                                                                                                                     |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Goals: A (correctness) + C (learning) — speed + reversibility deprioritized                                                                                                                                                                  |
| 2   | Skill-tag: per-task pickup (not session-start, not per-step)                                                                                                                                                                                 |
| 3   | Skill-conflict resolution: domain-tag wins; security > correctness > simplicity                                                                                                                                                              |
| 4   | Max 2 skills per pipeline step (`critical` tier: max 3 — see complexity tier table)                                                                                                                                                          |
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

  Files:
  1. .superpowers/sdd/<plan-slug>/estimate-report.md — append row to Plan-progress table; update Progress line; update Cost-to-date; update Last-merge footer with new SHA + PR + date.
  2. .superpowers/sdd/<plan-slug>/ai-cost-report.md — append row to Tasks table (1-3 sentence summary, merge SHA in Notes); match existing pipe style with trailing pipe (MD055).

  Both gitignored per L18 — save only, no commit.

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

