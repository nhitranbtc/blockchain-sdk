/// UI-facing copy for [BtcError] instances.
///
/// Single source of truth for the kind → message mapping. Every
/// wallet-area screen (Task 17 list, Task 18 detail, Task 20 send,
/// Task 22 transactions) faces the same `BtcError` set, so the
/// mapping belongs in a shared module.
///
/// **Defence-in-depth**: the messages are LITERAL strings — never
/// interpolate `e.stderr` / `e.toString()` here. The redacted stderr
/// surface is logged separately (Task 7 BtcLogFilter) and the
/// kind-mapped UI text is what reaches the user.
///
/// **v0.2 follow-up**: per-kind actionable copy + i18n keys (one ARB
/// string per BtcErrorKind variant) + richer "what to try next" hints.
library;

import 'btc_error.dart';

/// Maps a [BtcError] into a user-facing message based on its kind.
/// Does not expose the redacted stderr surface — that lives in the
/// log layer for ops triage.
String userMessageForBtcError(BtcError err) {
  switch (err.kind) {
    case BtcErrorKind.wrongPassword:
      return 'Wrong password.';
    case BtcErrorKind.insufficientFunds:
      return 'Not enough funds.';
    case BtcErrorKind.unknownWallet:
      return 'Wallet not found.';
    case BtcErrorKind.networkError:
      return 'Network error. Check your connection.';
    case BtcErrorKind.unknownAddressType:
      return 'Unsupported address type.';
    case BtcErrorKind.confirmRequired:
      return 'Confirmation required.';
    case BtcErrorKind.other:
      return 'Could not load wallets.';
  }
}
