---
name: audit-issue
description: Use when the user asks to audit a plan or implementation document for security and hack scenarios, and file the findings as a GitHub issue backed by an audit doc in `docs/audit/`. Triggers on phrases like "audit this plan", "security audit", "hack scenarios", "create audit issue", "review plan for vulnerabilities", "find threats in this plan", "ship-gate security review", or any variant that names a specific plan file under `docs/superpowers/plans/` or `docs/wallets/`. Reads the plan, performs a drift scan (L13 step 4a — check SHA pins, file paths, version claims against live state), drafts a phase-by-phase threat catalog with severity tags + a minimum ship-gate checklist, files the audit issue via `gh issue create` with the correct chain-specific triage label (`rust-tron-core`, `rust-eth-core`, `rust-btc-core`, etc.) + `backlog` + `task`, and commits the audit doc to `docs/audit/<YYYY-MM-DD>-<plan-slug>-security-audit.md`. **Skip** when the user asks for a code review of existing code (use `code-review-and-quality`), a plain doc review (use `doc-review`), or wants to fix the issues immediately rather than file them.
---

# audit-issue

End-to-end: given a plan file → drift scan → threat catalog → gh issue + audit doc → commit. Built to make audit work **repeatable across chains** (TRON, ETH, BTC, future chains) without rewriting the workflow each time.

## Inputs

- **Plan file path** under `docs/superpowers/plans/`, `docs/wallets/`, or similar (e.g. `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`).
- **Optional** chain label override; otherwise infer from filename/contents (`tron` → `rust-tron-core`, `eth` → `rust-eth-core`, `bitcoin`/`btc` → `rust-btc-core`).
- **Optional** parent issue (e.g. issue #399 for the TRON plan).

If any input missing or ambiguous → ask before scanning.

## Workflow (10 steps)

### 1. Read the plan
`Read` the full plan file. If `>2000` lines, read in chunks. Capture: phase structure, tech stack, every pinned SHA / version, every `crate::*` import, every external endpoint.

### 2. Drift scan (L13 step 4a)
For every SHA / path / version cited:
- `sha256sum` of vendored files → compare to plan's pinned SHA
- `ls` of cited paths → confirm exists
- `gh issue view` / `gh pr view` of cited references → confirm open/merged state

Record drift as `🟠` finding in the audit doc — drift IS a finding (silent supply-chain threat).

### 3. Walk each phase, build threat catalog
For Phase 0.0 → Phase N, scan for:
- **Plaintext secrets at rest** (mnemonic, key, password)
- **Replay vectors** (missing chain_id binding, missing expiration)
- **Off-by-one hazards** (proto field numbers, recovery-byte `v` ∈ {0,1} vs {27,28}, endian swaps)
- **Default-insecure values** (`Default::default()` for `rpc_url`, `fee_limit`, `network`)
- **Untrusted-input parsers** (URL schemes, hex strings, JSON envelopes)
- **Hardcoded unit confusion** (SUN vs TRX, wei vs ETH, sat vs BTC)
- **CLI arg leaks** (`--api-key` in `ps` / shell history)
- **Test/smoke that spends real funds**

Each finding gets a row: `| Finding | Severity | Detail | Required control |`

### 4. Map cross-cutting threats
Threats that span phases → Cross-cutting section (e.g. chain_id replay, v-byte convention, mnemonic-at-rest, RPC pinning). These become `C1..C9` controls.

### 5. Phase controls
Phase-specific threats → `P00-*`, `P0-*`, `P1-*`, ... numbered per phase + sequence. Each row ships-in-task column mapping to plan task #.

### 6. Draft minimum ship-gate checklist
The 8-12 controls that MUST be green for v0.1 release. Reverse-engineered from critical/high findings. Becomes PR-review checklist.

### 7. Output: audit doc
File: `docs/audit/<YYYY-MM-DD>-<plan-slug>-security-audit.md` (slug = lowercase, hyphenate, drop `.md`).

YAML frontmatter (required):
```yaml
---
title: <plan-name> security audit (ship-gate)
tracker: https://github.com/<owner>/<repo>/issues/<NNN>
plan: <relative-plan-path>
deep-dive: <optional relative path>
date: <YYYY-MM-DD>
status: open
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening
---
```

Body sections (in order):
1. Intro paragraph (link tracker, link plan, link deep-dive)
2. Drift scan table
3. Cross-cutting controls table (with "Ships in" column)
4. Per-phase control tables
5. Minimum ship-gate checklist (12-item `[ ]` list)
6. Out-of-scope (deferred items from plan)
7. References (issue tracker, plan, deep-dive, related PRs, source files)

Length: 100-200 lines. If exceeded, push phase detail into a sibling file.

### 8. File the issue
Use `gh issue create` with `--body-file` pointing to a tmp scratch body (NOT the audit doc itself). Issue body mirrors the audit doc but uses GitHub-flavored markdown only — no YAML frontmatter.

Required flags:
```bash
gh issue create \
  --repo <owner>/<repo> \
  --title "Security audit + hack-scenario review: <plan-name>" \
  --label "<chain-label>,backlog,task" \
  --body-file /tmp/audit-<date>-<slug>.md \
  --assignee <gh-user>
```

Label matrix:
| Filename contains | Label |
|---|---|
| `tron`, `tron-wallet` | `rust-tron-core` |
| `eth`, `ethereum` | `rust-eth-core` |
| `bitcoin`, `btc` | `rust-btc-core` |
| unclear / new chain | ask user |

Always add `backlog` + `task`. Never add `priority/p0` unless explicitly told.

### 9. Update audit doc frontmatter
After filing, write the returned issue URL back into the audit doc's `tracker:` frontmatter field. Now the doc is a permanent reference.

### 10. Commit the audit doc
Per `never-auto-commit` + `workflow-approval-required` (memory): pause for approval, then `git add` + `git commit -m "docs(audit): ... (#<NNN>)"` (use `#commit-approved` marker).

Branch: stay on whatever branch the user is on (don't create new branches without approval). If the branch already has an open PR, the audit commit lands on that PR (no separate PR needed) — `gh pr create` returns error and we note the existing PR + commit list.

## Severity legend (use these emoji + consistent phrasing)

- 🔴 **critical** — direct loss-of-funds / wallet compromise path
- 🟠 **high** — exploitable but needs precondition
- 🟡 **medium** — degrades UX or expands attack surface
- 🔵 **low/hardening** — defense-in-depth, defer-acceptable

Never invent new severity levels. Never drop the legend from the audit doc.

## Hard rules (pause before violating)

- **Never** auto-commit. Always pause for `#commit-approved` marker.
- **Never** file the issue without first showing the body to the user (use `AskUserQuestion` or surface preview).
- **Never** invent SHAs or file paths — quote real `sha256sum` output and `ls` results.
- **Never** suggest a fix that requires changes outside the plan's scope (scope discipline from agent-skills).
- **Always** link the issue back into the audit doc and vice-versa.
- **Always** run the drift scan BEFORE drafting threats — drift shapes which controls are 🔴 vs 🟠.
- **Always** preserve the plan's existing tech-stack choices; flag security issues, not architecture changes.

## Output summary template (for the chat reply)

After completing steps 1-10, surface:

```
| Artifact | State |
|---|---|
| Issue   | #NNN — OPEN, labels ... |
| Doc     | docs/audit/<path> — N lines, on <branch> |
| Commit  | <sha> — <message> |
| PR      | #NNN (if new) or added to existing PR #NNN (audit commit in commits list) |
| Drift   | <sha-pin-mismatch | path-missing | etc.> |
```

## Verification (post-completion)

- [ ] Issue URL returned by `gh issue create` is real and reachable
- [ ] Audit doc `tracker:` frontmatter matches issue URL
- [ ] All severity-🔴 findings have a row in the ship-gate checklist
- [ ] Drift scan section includes real `sha256sum` output (not invented)
- [ ] No invented commands or external URLs in the audit doc

## References (for the model using this skill)

- Reference audit produced via this skill: `docs/audit/2026-08-27-tron-wallet-core-security-audit.md`
- Reference issue: #407 on `nhitranbtc/blockchain-sdk`
- Plan that produced the first audit: `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`
- L13 step 4a (drift scan rule): `tasks/lessons.md` in any blockchain-sdk checkout
- Project memory: `~/.claude/projects/-home-nhitran-Projects-blockchain-sdk/memory/MEMORY.md` (never-auto-commit, workflow-approval-required)