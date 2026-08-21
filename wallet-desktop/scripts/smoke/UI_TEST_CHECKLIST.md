# wallet-desktop v0.1.0 UI test checklist

All wired user stories + supporting widgets. Run via `xdotool` against
the live Flutter Linux desktop app (the `flutter run` background
process spawned by `scripts/smoke/v0.1.0.sh`).

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
| `ProcessProgressOverlay` | send / sync | spinner during CLI invocation |

## Per-feature test steps

For each feature:
1. Navigate via UI (use chrome-devtools-mcp click on widget)
2. Verify expected widgets present (`debugDumpApp` query for widget name)
3. Verify state transitions correct (loading → data / error)
4. Verify L12 CRITICAL #2 — no mnemonic/password appears in `developer.log` after the action
5. Verify L7 env-strip — no `BTC_WALLET_MNEMONIC` in spawned subprocess env
6. Mark `[x]` in this checklist after manual confirmation

## Operator-driven gates (deferred to real desktop)

- [ ] Live testnet faucet fund via browser
- [ ] Visual confirmation of address rendering
- [ ] Confirmation count updates in UI after 1 conf
- [ ] Real `btc` binary (not fake_btc.sh) — uses Issue #203's v0.1.0.sh smoke

## Sandbox-L29 substitute (chrome-devtools-mcp + fake_btc.sh)

The Linux app currently running uses the **fake_btc.sh** fixture (the
`btc` binary in PATH points to the test fixture, not real `btc`). This
is sufficient for **UI exercise** but NOT for live testnet verification
(L29 + L28 Gate C). All `flutter test test/integration/` tests already
pass against the same fixture (7/7 per the pre-flight verify).

## Run

```bash
# 1. Confirm Linux app is running + VM service alive
curl -s http://127.0.0.1:37671/ElHrJYsLQTU=/getVM | jq '.result.name'
# 2. Open chrome-devtools-mcp against the VM service URL
# 3. Walk each row above; mark [x] after manual UI verification
```
