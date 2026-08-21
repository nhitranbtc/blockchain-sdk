# wallet-desktop L29 operator smoke

Operator-driven end-to-end verification of wallet-desktop releases. Per
[L29](../../../tasks/lessons.md) + [L28](../../../tasks/lessons.md) the
live testnet smoke cannot run in CI — it requires a graphical host,
the native lib build, testnet connectivity, and the operator's judgment
on whether each step's output matches the release contract.

## When to run

Before publishing the GitHub Release notes for any wallet-desktop tag.

For v0.1.0: run `bash wallet-desktop/scripts/smoke/v0.1.0.sh` after
checking out the `v0.1.0` tag locally. The script covers preflight
(native lib + testnet reachability), launches the Flutter desktop app,
walks the 11 wired stories via `xdotool`, captures a screenshot per
story, and runs the L12 CRITICAL #2 log sweep.

## Prerequisites

- **Flutter SDK 3.x stable** on PATH (`flutter --version` works)
- **Rust toolchain** matching `rust-toolchain.toml` (for the native
  lib build via `wallet-desktop/tool/build_native.sh`; Task 18 #224).
  Replaces the v0.1.0 `btc` binary requirement — wallet-desktop is
  now FFI-only (PRs #255 + #256).
- **Linux desktop host** with **X11** (for `import -window root`
  screenshots) and **`xdotool`** (GUI automation / window detection)
- **ImageMagick** (`import` command) for screenshots
- **Testnet connectivity** to `blockstream.info`
- **~1 GB free disk** at `$XDG_DATA_HOME/flutter_btc_wallet/` for the
  wallet DB + screenshot directory
- **A browser** to click the faucet at <https://coinfaucet.eu/btc-testnet/>
- **~20 minutes** of operator attention (fund + 2 confirmation waits)
- **L29 mindset**: this is a release gate. If a step fails, the release
  is not ready. Do NOT paper over failures.

## Quick start

```bash
# 1. Checkout the tag
git fetch origin --tags
git checkout v0.1.0

# 2. Run the headless preflight + GUI walk
bash wallet-desktop/scripts/smoke/v0.1.0.sh
```

The script pauses at each operator-only step with a clear `─── operator
action required ───` banner. Read the action, do it, press Enter.

## Step-by-step walkthrough

| Step | Headless / GUI | What it proves |
|------|---------------|----------------|
| Pre-flight | Headless | tag exists, native lib builds, testnet reachable, 1 GB free |
| 1 — Launch app | Headless (flutter run) + **GUI** | app starts, native lib loads, no crash on launch |
| 3 — Create wallet | **GUI** | FFI `walletCore.createWallet` + blob persist |
| 4 — Fund via faucet | **GUI** (browser) | address rendering + copy (FFI `walletLoad` peek_addresses) |
| 5 — Wait 1 conf | **GUI** (clock) | Esplora sync, balance refresh (FFI `walletCore.feeEstimate` + Esplora) |
| 6 — Show wallet | **GUI** | balance matches faucet (FFI `walletCore.walletShow`) |
| 7 — Send 0.001 BTC | **GUI** | tx broadcast + txid (FFI `walletCore.walletSend`) |
| 8 — Wait 1 conf | **GUI** (clock) | send appears in tx history (FFI `walletCore.walletTxids`) |
| 9 — Delete wallet | **GUI** | wallet deletion |
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

### F5 / F47 — temp-file lifecycle

Step 11 sweeps `/tmp/btc-secret-*` after wallet delete. Any stray file
= release blocker. Under the v0.1.0 subprocess path, the Task 5
`TempSecretFile` implementation created `/tmp/btc-secret-*` files
with mode 0600 + unlinked in `finally`. Under the post-FFI path (v0.2.0+),
the Rust side wraps phrases in `Secret<String>` (zeroize-on-drop per
`bdk_extras.rs:430-431`); no `/tmp` files are created. The sweep
remains as a defensive check on legacy residue from prior runs.

## Acceptance criteria

The script's exit code is the load-bearing signal:

| Exit | Meaning | Action |
|------|---------|--------|
| `0` | preflight GREEN + all stories walked + L12 CRITICAL #2 clean | Flip Issue #203 boxes after operator verification |
| `1` | preflight or any headless step failed | Investigate, fix, re-run |
| `2` | CRITICAL #2 grep matched cleartext | **Release blocker** — abort + investigate |

After the script exits `0`, the operator verifies each of the 11 story
screenshots in `$SCREENSHOT_DIR` matches the release contract, then
confirms each in Issue #203. Per L13 step 14, **external-gate
acceptance boxes stay `[ ]` until the operator confirms each one** —
auto-flipping is a false-positive completion.

## Recovery

If a step fails:

1. Do NOT re-run the script blindly. The preflight clears the wallet
   blob at `$WALLET_BLOB_DIR`; a re-run starts from a clean slate.
2. Read the failure message + the surrounding log output (Flutter
   desktop log at `$APP_DATA_DIR/flutter-run.log`, native build log at
   `/tmp/native-build.log`).
3. Check the GitHub Issues for known issues with the failing step.
4. If the failure is in wallet-desktop code:
   - File a follow-up issue with the failing step + log excerpt.
   - Do NOT publish the Release notes until the issue is fixed + the
     smoke passes against the fix.
5. If the failure is in `bitcoin-wallet-core` Rust FFI:
   - File a follow-up issue in the `rust-bitcoin-wallet` umbrella
     referencing the FFI op (`walletCreate`, `walletShow`, `walletSend`,
     `walletTxids`, etc.).
   - Block the wallet-desktop Release until the umbrella issue is fixed.

## Operator notes

- **xdotool drives the walk, but operator confirms each pause.** Per
  L29 and L28 (Gate C), live testnet smoke is operator-driven;
  xdotool plus screenshot capture is the audit-trail mechanism, not
  full automation. If xdotool misses a click (window focus drift), the
  operator intervenes manually and re-runs the failed step.
- **Screenshots land at `$SCREENSHOT_DIR`** (default
  `$XDG_DATA_HOME/flutter_btc_wallet/smoke-screenshots/v$TAG/`). Each
  story gets a PNG; the operator reviews them post-walk.
- **Testnet faucet rate-limits**: if `coinfaucet.eu` returns 429,
  switch to `https://testnet-faucet.com/btc-testnet/`. Note the change
  in Issue #203.
- **Time budget**: the two confirmation waits total ~20 minutes. The
  script pauses once per wait; do NOT background the script and walk
  away without reading the pause banners.
