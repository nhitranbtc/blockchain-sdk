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
- [L6] approval gates before persistent changes — `git commit` + remote ops (memory)
- [L8] flip issue checkboxes before squash-merge (memory)
- [L9] issue bodies = status, PR bodies = fix analysis (with table)
- [L11] scan skills list at session start, tag 3-5 relevant, invoke before doing
- [L12] code review runs BEFORE local verify gate, not after
- [L13] per-task pipeline spec (10 decisions, 2026-08-07 grill)
- [L14] ledger rule — `.superpowers/sdd/<plan>/progress.md`, update on pickup/commit/merge/grill, gitignored locally
- [L18] ledger path collision — `.superpowers/sdd/` gitignored; canonical L21 record in PR body
- [L21] Update estimate-report AND ai-cost-report on every PR merge (status, progress, in-flight count, merge SHAs)
- [L24] On PR merge: update CHANGELOG.md (Keep a Changelog + User Stories table) + "Try it" column. For ≥3 sub-tasks: parent branch + sequential merge + PR-to-parent.
- [L28] Client product: verify before claiming done (three gates — stub honesty, example verify, real-deps verify)
- [L29] Live testnet smoke is operator-driven, not CI — `#[ignore]` + opt-in env var + manual run script
- [L30] Verify plan-cited SHAs with `git log --all -- <path>` before trusting — drift detector for plan/spec headers

> **Index gaps (L15–L20):** entries were added then trimmed during session 2026-08-10. L15/L16/L17 were `Secret<T>` / ZeroizeOnDrop / Debug patterns. L18/L19 were review findings (doc-test + merge gate). L20 was estimate-report self-improvement (replaced by client-bill pivot). All removed per user direction; rules not currently in scope.
>
> **Audit (2026-08-10):** L10, L22, L23, L27 removed per user direction. L10 (threat-model re-read) — type-system invariants + L11/L12 review pair make the rule redundant. L22 (fact-forcing gate) — enforced at hook layer, captured in `~/.claude/CLAUDE.md` global memory instead. L23 (`git stash -u -- <path>` deletes untracked) — git-native behavior, covered by `git stash` docs. L27 (grep `#[derive(...)]` before using traits) — type-checker errors surface the assumption fast enough; pre-flight grep added latency without saving compile cycles.

### Domain map

| Domain | Lessons |
|---|---|
| Build / Cargo hygiene | L1 |
| Git workflow | L6 (approval gates), L8, L14 |
| Issue/PR protocol | L9, L24 |
| Skill + review pair | L11, L12, L13 |
| Post-merge bookkeeping | L21, L24 |
| Client product | L28 |
| Live testnet smoke | L29 |
| Doc drift detection | L30 |
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
4a. **Drift scan (per L30):** before starting feature work, verify every plan/spec/SHA citation referenced by the picked-up issue. For each cited `<path>`, run `git log --all -- <path>`. Empty result = drift (artifact never committed or SHA never existed); resolve by committing the artifact or filing a follow-up issue before feature work begins. Drift is silent — cargo fmt/clippy/test don't catch it; only `git log` reveals the gap.

## Per pipeline step
5. Step: pick skill pair (max 2) from L11 map
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
    - **Hardcode-sweep gate** (Issue #148, 2026-08-15): in addition to fmt/clippy/test, sweep the diff for runtime hardcoded values that bypass operator config. Grep targets:
        ```bash
        # From repo root:
        rg -n --type rust -e '127\.0\.0\.1|localhost|0\.0\.0\.0' rust-wallet-app/crates/
        rg -n --type rust -e 'blockstream\.info|mempool\.space|blockchair|btc\.com' rust-wallet-app/crates/
        rg -n --type rust -e 'm/.*'\''/[0-9]+'\''/[0-9]+'\''' rust-wallet-app/crates/
        rg -n --type rust -e '/tmp/|/usr/share/|/etc/|XDG_(DATA|CONFIG)' rust-wallet-app/crates/
        ```
        **Rule**: anything outside `#[cfg(test)]`, doc comments, or named cryptographic constants (BIP-32 `0x80000000`, BIP-44/86 paths) is a defect — either route through `WalletConfig` (Esplora URL) or extract a named `const`. `WalletConfig` has no `Default` impl and no `const DEFAULT_ESPLORA_URL`; every URL must arrive via the `--esplora-url` CLI flag. Tests legitimately bake `https://blockstream.info/testnet/api` and `/tmp/db` in `#[cfg(test)]` blocks — those are fixtures, not defects. The sweep must distinguish "test fixture" from "production hardcode" — the `#[cfg(test)]` boundary is the discriminator.
    - *Note*: L11 recommends also invoking `superpowers:verification-before-completion` at this step. User rejected adding it to L13 (2026-08-07) — L11 mapping still recommends it; L13 spec stays literal. If invoking it, do so as a wrapper around the cargo commands, not as a replacement.
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
13. commit-commands:commit-push-pr
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
18. Add new lessons if user corrections or novel patterns (L9 schema)
19. Apply L21 — update `estimate-report.md` (Plan-progress row + progress % + footer with merge SHA + date) + `ai-cost-report.md` (move row estimate→actual with measured tokens + recompute totals). Separate commits per file.
```

**Complexity tier → pipeline variation** (self-detect + user confirm):

| Tier                                                                                         | Pipeline                                                                    |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `trivial` (doc-only / single-line)                                                           | doc-review only; skip pre-PR code review                                    |
| `normal` (typical feature)                                                                   | full pipeline: TDD + code-review + verify + PAUSE + commit + post-PR review |
| `critical` (security-sensitive: key material / signing / encryption / network / persistence) | full + extra skill (e.g., `pr-review-toolkit:security-auditor`)             |

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
## L18 — Ledger path collision: `.superpowers/sdd/` gitignored; canonical L21 record lives in PR body

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

## Sub-task workflow for large tasks (≥3 sub-tasks: parent branch + sequential merge + PR-to-parent)

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

## L30 — Verify plan-cited SHAs with `git log --all -- <path>` before trusting

**Trigger**: Session 2026-08-12 PR #88 triage. Plan header cited
`e2d51ec` (design spec) and `0c20f77` (tangem research). `git log --all
-- <path>` returned empty for both — files existed on disk but were
never `git add`-ed. Plan referenced SHAs that resolved to nothing.
PR #88 landed both files at squash-merge SHA `d846564`.

**Rule**: At task pickup, scan plan / spec / review headers for SHA
citations. For each cited SHA, verify with `git log --all -- <path>`:

- **Returns ≥1 commit**: SHA is real; cross-ref works.
- **Returns empty**: drift; the cited SHA never existed. Either commit
  the artifact (resolve the drift) or update the header to point to a
  real SHA / a PR number.

**Why**: Plan headers are written once and referenced often. A future
self chasing a plan-cited SHA finds nothing — wasted investigation or
worse, silently broken cross-refs in published docs. The drift class is
silent: cargo fmt / clippy / test don't surface it; only `git log`
against the cited path reveals the gap.

**Apply**:

```bash
# At task pickup, before trusting any plan-cited SHA:
for path in $(grep -oE 'docs/[^ )]+\.md' plans/*.md specs/*.md reviews/*.md 2>/dev/null); do
  hits=$(git log --oneline --all -- "$path" | wc -l)
  if [ "$hits" -eq 0 ]; then
    echo "DRIFT: $path has no git history"
  fi
done
```

Or per-file: `git log --all -- <path>` after spotting a `(commit \`SHA\`)`
citation in the header.

**Anti-patterns**:

- Trusting a SHA citation without verification — broken cross-ref silently propagates.
- Citing a SHA "aspirationally" (file doesn't exist yet) — drift class repeats on every reference.
- Removing the SHA citation instead of resolving the drift — hides the cross-ref problem rather than fixing it.

---

## L31 — L13 per-task pipeline adapted for Flutter / Dart (`wallet-desktop`)

**Trigger**: Session 2026-08-15 wallet-desktop execution. L13 (Rust pipeline) is the canonical per-task spec, but its toolchain references (`cargo fmt`, `cargo clippy`, `cargo test`) and PR model don't apply to the Flutter project at `wallet-desktop/`. Adapted below; **apply for every wallet-desktop task**.

**Rule**: Run the L13 pipeline with these substitutions. Where the rule diverges, the divergence is noted.

### Step-by-step mapping (L13 → wallet-desktop)

| L13 step | Rust equivalent | Flutter/Dart equivalent (wallet-desktop) |
|---|---|---|
| 1 L11 skill tag | Skill list scan | Same: load 3-5 relevant (flutter_lints, riverpod_generator, dart:io, package:logging, etc.) |
| 2 Complexity self-detect | trivial / normal / critical | Same tiers; "critical" for L12 secret-handling, password fields, mnemonic lifecycle, network TLS pinning |
| 3 Pick up issue | Read issue body | Same; check task is sub-task or top-level (plan §Task Index) |
| 4 karpathy-guidelines + branch | Worktree + branch from main | **Same — checkout new branch per task (e.g. `feat/wallet-desktop/task-3`). Direct commit on `main` is NOT permitted.** |
| 4a Drift scan (L30) | `git log --all -- <path>` | Same |
| 5-8 Skill pair | L11 map, domain-tag wins | Same |
| **9 TDD red-green** | failing test → impl → pass | **Same for tasks 3-24**. **Skipped for** config tasks (Task 2 lint, Task 25 CI workflow) and asset-stub tasks (Task 1). For UI feature tasks (Tasks 17-23), write failing widget test FIRST (per design §8.3 widget test matrix). |
| **10 L12 pre-PR review** | code-review + type-design parallel | **Same; but invoke `ecc:flutter-reviewer`** instead of `ecc:rust-reviewer`. For "critical" tier, add `pr-review-toolkit:security-auditor`. |
| **11 Verify gate** | `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test --workspace` | `dart format --set-exit-if-changed --output=none .` + `dart analyze --fatal-warnings --fatal-infos` + `flutter test` |
| 11a Backlog triage | Same | Same |
| **12 PAUSE commit approval** | Wait for explicit "yes commit" | Same |
| **13 Commit + push + PR** | PR model | **Same** — `commit-commands:commit-push-pr`. Branch + PR required for every task. |
| **14 Flip issue checkboxes** | `gh issue edit N --body` | **Same** — flip issue checkboxes with artifact evidence per L13 step 14 rules. |
| 15-15d PR review + merge + tech doc + L24 cascade | PR model | **Same** — full PR review + merge + close + L24 cascade. |
| 16-19 Session-level | Same | Same |

### Verify gate details (Step 11)

```bash
# Format check (idempotent — re-run after any non-Edit tool change)
export PATH="$HOME/flutter/bin:$PATH"
cd wallet-desktop
dart format --set-exit-if-changed --output=none .

# Static analysis (CLAUDE.md `-D warnings` equivalent)
dart analyze --fatal-warnings --fatal-infos

# Unit + widget tests (L29: live smoke excluded)
flutter test
```

**All three must pass before commit.** A single failing gate = task is not done.

### Hardcode sweep (L13 step 11 Flutter adaptation)

Mirrors Rust hardcode-sweep, scoped to Dart files:

```bash
# From wallet-desktop/lib:
rg -n -e '127\.0\.0\.1|localhost|0\.0\.0\.0'
rg -n -e 'blockstream\.info|mempool\.space|blockchair|btc\.com'
rg -n -e '/tmp/|XDG_(DATA|CONFIG)'
```

**Rule**: anything outside `// @TestOn('vm')` or `test/` blocks is a defect — route through `EsploraConfig` (already in Task 12) or extract a named constant. Tests legitimately bake fixtures in `test/` blocks — those are fixtures, not defects.

### Secret-leak sweep (L12 CRITICAL #2 mirror)

```bash
# From wallet-desktop/lib:
rg -n -e '(password|mnemonic|secret)\s*[:=]\s*"[^"]+"'   # string literal assignment
rg -n -e 'print\s*\(.*password|print\s*\(.*mnemonic'        # logging mnemonic-shaped
```

**Rule**: zero matches outside `test/`. Mnemonic-shaped strings (12/15/18/21/24 lowercase words) in source = defect. Routes through `BtcLogFilter` (Task 7) before any logger call.

### Complexity tier variation

| Tier | Pipeline variation for wallet-desktop |
|---|---|
| `trivial` (lint config, asset stubs) | Skip TDD; verify gate only; no L12 review subagent (self-review); commit + push to PR |
| `normal` (DTOs, providers, widgets) | Full: failing test → impl → pass → L12 review → verify → PAUSE → commit |
| `critical` (BtcInvoker, TempSecretFile, BtcLogFilter, password/mnemonic widgets) | Full + `pr-review-toolkit:security-auditor` subagent + explicit L12 CRITICAL #2 sweep + flutter analyze with extra `unsafe_html` + custom lint rule for mnemonic-shaped strings |

### Branch + PR model — historical archive

**2026-08-15 (early session):** wallet-desktop was originally scoped with a "direct commit on `main`" deviation, skipping canonical L13's branch + PR model. Reasoning captured in the original L31 draft: speed over reversibility, single developer, no merge queue.

**2026-08-15 (same day):** user reversed the deviation. `feat/wallet-desktop/task-N` branches now follow canonical L13:

1. Each task branches off `main` (`git checkout -b feat/wallet-desktop/task-N`).
2. L13 steps 13 (commit-push-pr), 14 (issue edit), 15 (PR review), 15a (10-section tech doc), 15b-15c (L24 cascade + L13 audit), 15d (merge + close) all fire as canonical.
3. L21 (estimate-report + ai-cost-report) updates on every PR merge — recorded in PR body.
4. L24 (CHANGELOG `[Unreleased]` + User Stories table) cascade runs on each PR merge; accumulated entries release-cut at Task 26.
5. Tasks 1 + 2 (`26dfec9`, `a342597`) remain on `main` as historical artifacts of the original deviation. Task 3 onwards follows canonical L13.

**Rationale for reversal:** even on solo dev branches, the L12 review + L24 cascade + L21 cost tracking provide audit trail value that direct commits lack. Reverting to canonical L13 trades ~2 min/branch for a complete per-task audit record.

### Anti-patterns

- **Skipping widget tests** for feature tasks (17-23). Per design §8.3, every screen has a widget test matrix (loading/data/error/validation/dispose).
- **Bypassing the verify gate** to ship faster. Flutter analyzer debt (`unused_element`, `prefer_const_constructors`) compounds the same way clippy debt does.
- **Logging mnemonic-shaped strings** in widget code. L12 CRITICAL #2 is a hard gate; `BtcLogFilter` is the only path for secret-bearing logs.
- **Committing secrets** to git (`.dart_tool/`, `coverage/`, `~/.local/share/flutter_btc_wallet/`). `.gitignore` is mandatory; verify with `git status --ignored` after scaffold.
- **Spawning `btc` without stripping inherited env vars** (Task 10 `BtcInvoker`). L7: strip `BTC_WALLET_MNEMONIC`, `BTC_ENCRYPT_PASSWORD`, `BTC_DECRYPT_PASSWORD` from parent env before `Process.start`.
- **Committing `dart analyze` auto-edits** to `analysis_options.yaml` (flutter-tools adds `build/`, `android/`, `ios/`, `web/`, `windows/`, `macos/`, `linux/` to `analyzer.exclude` on first run). Revert after verify gate unless intentional. Defer the platform excludes to Task 25 (CI workflow) where they belong with the GitHub Actions matrix.
- **Merging blind on missing PR review notifications (Task 6, PR #182)**: post-PR review sub-agents (code-reviewer + type-design-analyzer) launched but did not return verdicts before merge command ran. Merged on pre-PR L12 pass + user pre-authorized admin bypass. Fix: surface "PR review not received — proceed?" before merging blind. Don't assume pre-PR pass means post-PR is unneeded. If notification doesn't arrive within ~3 min, ask user instead of merging.
- **L13 step 15 PR review — always launch security-auditor (Task 8+)**: PR review (L13 step 15) traditionally launched 2 sub-agents (`code-reviewer` + `type-design-analyzer`) per L13 Q4 max-2 limit. For *all* tiers — including normal — defense-in-depth requires a third sub-agent: `compass:security-auditor` (or `pr-review-toolkit:security-auditor` for critical). Apply per L13 step 15: launch sub-agents + CONFIRM agents launched (do not assume pre-PR L12 pass replaces step 15). Capture notifications before acting on merge.
- **Pre-warming async providers before pumpWidget trips `!timersPending` (Task 17)**: `container.read(provider.future)` before `pumpWidget` leaves the autoDispose provider without a listener; its idle-dispose timer stays pending and trips the widget-binding tear-down invariant. Fix: drive loading → data via the widget's own `ref.watch` (i.e. `pumpWidget` + `pumpAndSettle`). Never pre-warm when the screen will be the listener.
- **Test fakes that mock `BtcInvoker.invoke` MUST override the method (Task 17)**: `_FakeBtcInvoker extends BtcInvoker(binaryPath: '/tmp/fake_btc')` without `invoke` override inherits the real `Process.start` path → test hangs until `BtcInvoker._timeout` (30s). Fix: always override `invoke<T>(cmd, parse)` to return `parse(fixture)` synchronously (or throw) so the parent's subprocess path is short-circuited. Misleading `binaryPath` string is a secondary issue (deferred — the value is unused if `invoke` is overridden).
- **Extract format / error-mapping helpers BEFORE the second screen needs them (Task 17 type-design post-PR feedback)**: `_displayWalletId` and `_userMessageForBtcError` started private to Task 17; the type-design post-PR sub-agent correctly flagged that Tasks 18/20/21/22 would copy-paste. Fix: lift to `lib/core/format/wallet_id.dart` and `lib/core/btc/btc_error_messages.dart` at Task 17 (before Tasks 18+ pickup), not after a copy-paste incident. L11 lesson: pre-emptive abstraction when 3+ call sites are within one task pickup cycle.
- **Dart `dart:developer.log` bypasses `package:logging` redaction (Task 17 sec-auditor post-PR)**: `BtcLogFilter.redact` sits behind `Logger.root.onRecord`; `developer.log(...)` lands in DevTools / VM-service / OS syslog and skips that pipeline. Fix: pipe the exception's `toString()` through `BtcLogFilter().redact(...)` before passing to `developer.log`. The `stackTrace` arg (StackTrace object, not String) carries file/line info from Dart's catch site — not user input — pass unredacted. Mirror in main.dart's `runZonedGuarded` block before Task 26 ships.

### Apply

For every wallet-desktop task, run this adapted pipeline. If a step doesn't apply (e.g., TDD skipped for trivial config), log why in the ledger. If a step fails, escalate per L13 Q9. Re-grill after 5 tasks or when a Flutter-specific pattern emerges.

