import 'package:meta/meta.dart';

/// Typed classification of `btc` CLI stderr output.
///
/// Maps a stderr + exit-code pair from a `Process.run(btc, argv)` call
/// (Task 10 BtcInvoker) to a [BtcErrorKind] for UI routing. Pattern
/// matching is case-insensitive and uses `RegExp` so partial matches work
/// (e.g. `error: wrong password (try again)` → [wrongPassword]).
enum BtcErrorKind {
  wrongPassword,
  insufficientFunds,
  unknownWallet,
  networkError,
  unknownAddressType,
  confirmRequired,
  other,
}

/// Exception class for `btc` CLI failures.
///
/// Carries the original stderr + exit code for diagnostics + the
/// classified [kind] for UI routing.
///
/// **L12 CRITICAL #2**: `stderr` may contain user-input echo (e.g.
/// address validation errors that print the rejected address). Do NOT log
/// `stderr` raw — pass through [BtcLogFilter] (Task 7) first.
///
/// **Pattern ordering**: list is order-dependent. Place more-specific
/// patterns BEFORE general ones (e.g. `wrong network` before
/// `network`/`esplora`). The first match wins.
@immutable
class BtcError implements Exception {
  const BtcError({
    required this.exitCode,
    required this.stderr,
    required this.kind,
  });

  final int exitCode;
  final String stderr;
  final BtcErrorKind kind;

  static final _patterns = <(RegExp, BtcErrorKind)>[
    (
      RegExp(r'wrong\s*password', caseSensitive: false),
      BtcErrorKind.wrongPassword
    ),
    (
      RegExp(r'insufficient\s*funds', caseSensitive: false),
      BtcErrorKind.insufficientFunds
    ),
    (
      RegExp(r'wallet.*not\s*found|unknown\s*wallet', caseSensitive: false),
      BtcErrorKind.unknownWallet
    ),
    // More-specific network-mismatch patterns BEFORE the general
    // esplora/network/unreachable regex (which would catch them too).
    (
      RegExp(r'does\s*not\s*match.*network|wrong\s*network',
          caseSensitive: false),
      BtcErrorKind.unknownAddressType
    ),
    (
      RegExp(r'esplora|network|unreachable|timed?\s*out', caseSensitive: false),
      BtcErrorKind.networkError
    ),
    (
      RegExp(r'--confirm-yes|mainnet.*confirm', caseSensitive: false),
      BtcErrorKind.confirmRequired
    ),
  ];

  factory BtcError.fromStderr(String stderr, {required int exitCode}) {
    for (final (pattern, kind) in _patterns) {
      if (pattern.hasMatch(stderr)) {
        return BtcError(exitCode: exitCode, stderr: stderr, kind: kind);
      }
    }
    return BtcError(
        exitCode: exitCode, stderr: stderr, kind: BtcErrorKind.other);
  }

  /// Deliberately omits [stderr] (per security-auditor: stderr may
  /// echo user-input — addresses, mnemonic fragments, etc.). Callers
  /// MUST log stderr via [BtcLogFilter] (Task 7) explicitly. Returns
  /// only `kind` + `exitCode` (no secret surface).
  ///
  /// If callers need stderr in error messages, render it via the
  /// documented `BtcLogFilter.redact()` path before interpolation.
  @override
  String toString() =>
      'BtcError(kind: $kind, exit: $exitCode, stderr: <redacted — '
      'log via BtcLogFilter>)';
}
