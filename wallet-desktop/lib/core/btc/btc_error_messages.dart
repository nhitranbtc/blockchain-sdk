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
///
/// The `other` arm returns a context-neutral phrase (correct in v0.1
/// across all four call sites — Tasks 17/18/20/22): the older
/// "Could not load wallets." copy was leaked across screens that
/// also create / send / transact. Per L12 type-design post-PR
/// (Task 18) we lifted the context specificity to each call site via
/// `BtcError.toString()` fallbacks.
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
      return 'Something went wrong.';
  }
}
