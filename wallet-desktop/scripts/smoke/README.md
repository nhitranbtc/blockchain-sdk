# wallet-desktop L29 operator smoke

Operator-driven end-to-end verification of wallet-desktop releases. Per
[L29](../../../tasks/lessons.md) + [L28](../../../tasks/lessons.md) the
live testnet smoke cannot run in CI — it requires a graphical host, a
real `btc` binary, testnet connectivity, and the operator's judgment on
whether each step's output matches the release contract.

## When to run

Before publishing the GitHub Release notes for any wallet-desktop tag.

For v0.1.0: run `bash wallet-desktop/scripts/smoke/v0.1.0.sh` after
checking out the `v0.1.0` tag locally. The script covers all headless
steps (CLI invocations, file checks, log scans). GUI-only steps pause
for operator confirmation.

## Prerequisites

- **Flutter SDK 3.x stable** on PATH (`flutter --version` works)
- **`btc` binary** on PATH (build with `cargo build --release -p btc`
  from `rust-wallet-app/`; cross-arch bundles per `.github/workflows/btc-bundle.yml`)
- **Testnet connectivity** to `blockstream.info`
- **~1 GB free disk** at `$XDG_DATA_HOME/flutter_btc_wallet/` for the
  wallet DB + log capture
- **A browser** to click the faucet at <https://coinfaucet.eu/btc-testnet/>
- **~20 minutes** of operator attention (fund + 2 confirmation waits)
- **L29 mindset**: this is a release gate. If a step fails, the release
  is not ready. Do NOT paper over failures.

## Quick start

```bash
# 1. Checkout the tag
git fetch origin --tags
git checkout v0.1.0

# 2. Build btc (if not already on PATH)
cargo build --release -p btc
export PATH="$PWD/rust-wallet-app/target/release:$PATH"

# 3. Run the headless portion of the smoke
bash wallet-desktop/scripts/smoke/v0.1.0.sh
```

The script pauses at each operator-only step with a clear `─── operator
action required ───` banner. Read the action, do it, press Enter.

## Step-by-step walkthrough

| Step | Headless / GUI | What it proves |
|------|---------------|----------------|
| Pre-flight | Headless | tag exists, btc works, testnet reachable |
| 2 — Launch app | **GUI** | app starts, no crash on launch |
| 3 — Create wallet | Headless (CLI) | wallet creation + blob persist |
| 4 — Fund via faucet | **GUI** (browser) | address rendering + copy |
| 5 — Wait 1 conf | **GUI** (clock) | Esplora sync, balance refresh |
| 6 — Show wallet | Headless (CLI) | balance matches faucet |
| 7 — Send 0.001 BTC | Headless (CLI) | tx broadcast + txid |
| 8 — Wait 1 conf | **GUI** (clock) | send appears in tx history |
| 9 — Delete wallet | Headless (CLI) | wallet deletion |
| 11 — Verify blob gone | Headless (file check) | F47 temp-file lifecycle |
| 12 — Log grep | Headless (grep) | L12 CRITICAL #2 invariant |

## Threat-model coverage

### L12 CRITICAL #2 — mnemonic + password NEVER logged

Step 12 is the load-bearing check. The script:

1. Greps `~/.local/share/flutter_btc_wallet/logs/app.log` for any line
   matching a BIP-39 12/15/18/21/24-word shape (lowercase single-space
   separated).
2. Greps for the operator's own freshly-created mnemonic's first two
   words appearing contiguously in any line.

Any cleartext match = **exit 2, release blocker**. Do NOT publish the
Release notes; investigate + fix + retag.

### L7 — env-strip

`BtcInvoker._secretEnvKeys` strips `BTC_WALLET_MNEMONIC`,
`BTC_ENCRYPT_PASSWORD`, `BTC_DECRYPT_PASSWORD` from spawned subprocess
env before `Process.start`. The script doesn't directly test L7 (it's
a Dart-side filter; the existing Task 24 `with_secret_env.sh` fixture
covers it in CI). Operators can spot-check with:

```bash
BTC_WALLET_MNEMONIC=probe-secret-must-be-stripped \
  "$btc_bin" --network testnet wallet list
# Expected: stdout does NOT contain 'probe-secret-must-be-stripped'
```

### F5 / F47 — temp-file lifecycle

Step 11 sweeps `/tmp/btc-secret-*` after wallet delete. Any stray file
= release blocker. The Task 5 `TempSecretFile` implementation creates
these with mode 0600 + unlinks in `finally`; the smoke confirms the
invariant holds end-to-end.

## Acceptance criteria

The script's exit code is the load-bearing signal:

| Exit | Meaning | Action |
|------|---------|--------|
| `0` | headless GREEN + L12 CRITICAL #2 clean | Flip Issue #203 boxes after GUI verification |
| `1` | preflight or headless step failed | Investigate, fix, re-run |
| `2` | CRITICAL #2 grep matched cleartext | **Release blocker** — abort + investigate |

After the script exits `0`, the operator runs the GUI portion manually
(steps 2, 4-5, 7-8) and confirms each in Issue #203. Per L13 step 14,
**external-gate acceptance boxes stay `[ ]` until the operator confirms
each one** — auto-flipping is a false-positive completion.

## Recovery

If a step fails:

1. Do NOT re-run the script blindly. The script deletes the wallet blob
   at pre-flight (line ~50); a re-run starts from a clean slate.
2. Read the failure message + the surrounding `btc` output.
3. Check the GitHub Issues for known issues with the failing command.
4. If the failure is in wallet-desktop code (not `btc`):
   - File a follow-up issue with the failing step + output.
   - Do NOT publish the Release notes until the issue is fixed + the
     smoke passes against the fix.
5. If the failure is in `btc` CLI:
   - File a follow-up issue in the `rust-bitcoin-wallet` umbrella.
   - Block the wallet-desktop Release until the umbrella issue is fixed.

## Operator notes

- **GUI clicks can't be automated**: this script covers the headless
  parts of the 12-step plan §Task 26 procedure. Launching the app,
  clicking buttons, and waiting for visual confirmation remain
  operator-driven. This is intentional per L29 + L28 (Gate C).
- **The mnemonic IS printed to stdout by the script** (line ~80): the
  operator needs it to verify the GUI-side display. Stdout is the
  operator's responsibility — do NOT redirect to a log file. If you
  must capture output, use `tee` and then `rm` the captured file
  after the run.
- **Testnet faucet rate-limits**: if `coinfaucet.eu` returns 429,
  switch to `https://testnet-faucet.com/btc-testnet/` or use the
  `bitcoin-cli` regtest mode for local testing. Note the change in
  Issue #203.
- **Time budget**: the two confirmation waits total ~20 minutes. The
  script pauses once per wait; do NOT background the script and walk
  away without reading the pause banners.
