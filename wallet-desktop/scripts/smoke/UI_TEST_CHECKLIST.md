# wallet-desktop v0.2.x UI test checklist

All wired user stories + supporting widgets. Run via `xdotool` against
the live Flutter Linux desktop app (the `flutter run` background
process spawned by `scripts/smoke/v0.1.0.sh`).

**v0.2.x FFI-only:** wallet-desktop no longer spawns a `btc` subprocess
(PRs #255 + #256 deleted `BtcInvoker`, `BtcExtractor`, `assets/btc/`,
`fake_btc.sh`). Every wallet op routes through Rust FFI via
`bitcoin-wallet-core`. The checklist reflects the FFI surface; the
threat-model checks (L12 CRITICAL #2, L33.4) replaced the subprocess
ones (L7 env-strip). See Issue #259 for the full threat-model mapping.

**Prerequisites:** the `btc` CLI binary is NO LONGER required — wallet-desktop
is FFI-only after PRs #255 + #256. Instead, the operator must build
the Rust cdylib via `wallet-desktop/tool/build_native.sh` (Task 18,
Issue # 224). See `scripts/smoke/README.md` for the full prereq list.

## 11 wired user stories

| # | Story | Screen / Widget | Test steps |
|---|-------|----------------|-----------|
| 1 | Create wallet | `WalletCreateScreen` + `MnemonicDisplayDialog` | navigate → Create → enter name → CreateWallet → mnemonic dialog → verify 12 words → backup done → unlock |
| 2 | Import wallet | `WalletImportScreen` | navigate → Import → paste 12-word mnemonic → name → Import → unlock |
| 3 | Wallet details (balance + first address) | `WalletDetailScreen` | tap wallet → verify balance + first receive address + copyable |
| 4 | Wallet transactions | `WalletDetailScreen` tx list | tap wallet → tap Transactions tab → verify tx list |
| 5 | Send BTC | `SendScreen` | tap wallet → Send → enter address + amount → fee estimate → confirm |
| 6 | Send with fee selection | `SendScreen` fee picker | tap wallet → Send → select fee tier → verify fee rate updates |
| 7 | Transaction history | `TransactionsScreen` | from WalletDetail → Transactions tab → verify pagination |
| 9 | List wallets | `WalletListScreen` | initial route → verify wallet list + FAB + empty state |
| 11 | Lock wallet | `WalletDetailScreen` lock button | unlock → tap Lock → verify locked state + return to list |
| 12 | Settings / Esplora config | `SettingsScreen` | navigate to Settings → change Esplora URL → save → verify persist |
| 20 | Mnemonic backup | `MnemonicDisplayDialog` | Create wallet → mnemonic dialog → toggle reveal → copy disabled → backup done |

## Supporting widgets to verify

| Widget | Where | Verify |
|--------|-------|--------|
| `HomeShell` | root | tab navigation (List / Settings) + Bitcoin orange theme |
| `MaterialApp` + theme | root | Bitcoin orange `#F7931A` accent + monospace addresses |
| `AddressChip` | wallet list / detail | truncated address + tap-to-copy |
| `BalanceCard` | detail | confirmed balance + unconfirmed |
| `StatusBadge` | detail / send | sufficientFunds / insufficientFunds / pending |
| `NetworkPicker` | settings | dropdown default testnet + emit on selection |
| `PasswordField` | unlock | obscureText defaults true + reveal toggle |
| `MnemonicPasteField` | import / send re-entry | word count validation (12/15/18/21/24) + paste handler |
| `MnemonicDisplayDialog` | create | reveal/hide toggle + Copy disabled + ExcludeSemantics |
| `ProcessProgressOverlay` | send / sync | spinner during FFI call (no subprocess) |

## Per-feature test steps

For each feature:
1. Navigate via UI (use chrome-devtools-mcp click on widget)
2. Verify expected widgets present (`debugDumpApp` query for widget name)
3. Verify state transitions correct (loading → data / error)
4. Verify L12 CRITICAL #2 — no mnemonic/password appears in `developer.log` after the action
5. Verify L33.4 mnemonic-never-in-argv — `ps -ef` during Send must not show the mnemonic (FFI passes via `phrase: SecretBuffer` only)
6. Mark `[x]` in this checklist after manual confirmation

## Operator-driven gates (deferred to real desktop)

- [ ] Live testnet faucet fund via browser
- [ ] Visual confirmation of address rendering
- [ ] Confirmation count updates in UI after 1 conf
- [ ] Real Rust cdylib via `wallet-desktop/tool/build_native.sh` — uses Issue #259's checklist + `scripts/smoke/v0.1.0.sh` (PR #257)

## Run

The operator-driven walk runs via `scripts/smoke/v0.1.0.sh` (PR #257
auto-captures `import -window root` screenshots per story into
`$XDG_DATA_HOME/flutter_btc_wallet/smoke-screenshots/v$TAG/`). Walk
each row above; mark `[x]` after manual UI verification per Issue #259.
