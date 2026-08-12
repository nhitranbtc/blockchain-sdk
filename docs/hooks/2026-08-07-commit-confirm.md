---
date: 2026-08-07
topic: git commit approval hook
---

# Git commit approval hook

`.claude/hooks/require-commit-confirm.sh` is a `PreToolUse:Bash` hook that
blocks every `git commit` unless the command ends with the approval
marker `#commit-approved` (or `# commit-approved`).

This file documents the marker contract + the patched grep pattern so
the rule survives machine wipes, fresh clones, and devs who hit the
"every commit blocked" failure mode without context.

## What the hook does

1. Reads the bash command Claude is about to run (from `PreToolUse` JSON stdin).
2. Greps for `git commit` as a whole command. Skips false matches like
   `legit-commit`, `echo "git commit"`.
3. If `git commit` matches, requires `#commit-approved` or `# commit-approved`
   somewhere in the command string (usually as a trailing shell comment).
4. If absent: prints the workflow reminder to stderr and exits with
   code 2, blocking the bash call.
5. If present: exits 0, bash runs normally.

## Why the marker

`#commit-approved` is the self-documenting artifact that proves the
agent paused, told the user what was about to be committed, and got
verbal approval. The user never types the marker — the agent appends
it after a successful `AskUserQuestion` confirmation.

This is double-enforcement of the project's `never-auto-commit` rule
(`.claude/CLAUDE.md` memory): the rule says "pause and ask," the hook
makes the agent mark its work to prove it paused.

## Original bug (2026-08-07)

The hook's error message showed `# commit-approved` (with space) but
its grep was for `#commit-approved` (no space). Following the error
message literally produced three retry cycles of blocked commits
before the right form was found.

**Old grep (broken):**

```bash
if ! echo "$cmd" | grep -q '#commit-approved'; then
```

**Patched grep (accepts both forms):**

```bash
if ! echo "$cmd" | grep -qE '#\s?commit-approved'; then
```

`#\s?` matches `#` optionally followed by whitespace. Both `#commit-approved`
and `# commit-approved` now pass.

## Patched error message

The hook's stderr block now shows both accepted forms so the error
message and the grep are aligned:

```
BLOCKED: git commit requires explicit user confirmation.

Workflow:
  1. Tell the user what is about to be committed (files + message).
  2. Wait for the user to say "yes commit" / "approved" / similar.
  3. Retry the commit with the marker appended (either form works):
       git commit -m "..."  #commit-approved
       git commit -m "..."  # commit-approved
```

## Why the hook is gitignored

`.claude/hooks/` is `.gitignore`d by project convention. A
repo-tracked hook runs arbitrary shell with the developer's full
permissions on every Claude Code tool call. That is a supply-chain
attack surface — every clone = trust transfer.

The cost is exactly this: local-only fixes. The patch in this repo
applies only to machines where someone has already applied it. New
clones start with the original (broken) hook.

## How to apply the patch on a fresh clone

If a fresh clone shows the "every commit blocked" failure mode:

1. Read the current hook: `cat .claude/hooks/require-commit-confirm.sh`.
2. If the grep line is `grep -q '#commit-approved'` (no `\s?`):
   change it to `grep -qE '#\s?commit-approved'`.
3. Update the error message to show both `#commit-approved` and
   `# commit-approved` as accepted forms.
4. No commit needed (the file is gitignored).

## Related rules

- `never-auto-commit` — pause and ask before every `git commit`. Lives
  in user memory (`.claude/CLAUDE.md`).
- `workflow-approval-required` — pause before any state-modifying
  action (`gh`, branch ops, file moves). Same source.
- This hook is the mechanical backstop for both rules when the agent
  might forget to pause.
