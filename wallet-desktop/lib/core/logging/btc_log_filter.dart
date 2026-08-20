import 'package:logging/logging.dart';

/// Scrubs mnemonic-shaped strings + `--password` / `--password-file`
/// flag values from log messages. Mirrors the btc CLI's L12 CRITICAL #2
/// redaction pattern.
///
/// **Mnemonic pattern**: matches 12/15/18/21/24 consecutive lowercase-word
/// runs separated by single spaces (BIP-39 mnemonic shape). Each match is
/// replaced with `<redacted-mnemonic>`. The regex is heuristic — false
/// positives on long English sentences are possible but rare (12+
/// consecutive lowercase words is uncommon in technical logs).
///
/// **Password flag pattern**: matches `--password` and `--password-file`
/// (both take a value). `--password-stdin` reads from stdin and has no
/// following value to redact, so it is NOT in the alternation —
/// including it would falsely absorb the next flag as if it were a value.
/// Each match keeps the flag name and replaces the value with
/// `<redacted>`. Note: only matches the flag-value form on a single line
/// with a space separator (`--password foo`, not `--password=foo`); does
/// NOT redact `BTC_ENCRYPT_PASSWORD=…` env-var echoes (defense-in-depth
/// gap — caller should not echo env vars).
///
/// **Format redaction**: [format] passes [LogRecord.message] through
/// [redact] AND also redacts [LogRecord.error] / [LogRecord.stackTrace] —
/// exceptions frequently embed raw inputs (mnemonic-shaped strings from
/// signing failures, password values from CLI arg parsing errors), so
/// secrets leak via the error/stack trace path despite the message being
/// scrubbed.
///
/// **Limitations**:
/// - Word-list check omitted (any 12+ lowercase words trigger). A
///   `Future<UseCase>` with 12 lowercase names would false-positive.
///   Trade-off: word-list check costs ~40 KiB of static data; v0.2 can add.
/// - Multi-line mnemonics split across lines NOT detected (single-line
///   regex). The btc CLI logs each line independently, so this is the
///   realistic attack surface.
/// - `--password=value` equals-form passes through (regex requires `\s+`).
///   Documented; add coverage if btc CLI starts emitting that form.
///
/// **L12 CRITICAL #2**: this filter is the last line of defense before
/// Flutter's log handler persists/shows the line. Callers upstream MUST
/// not log raw secrets either — defense-in-depth.
/// Contract for any log filter in `wallet-desktop`.
///
/// `package:logging` 1.3.0 does NOT export a `LogFilter` class (plan §Task 7
/// drift). Define our own interface here so Task 10 (`BtcInvoker`) can
/// inject fakes + v0.2 can introduce additional implementations.
abstract interface class LogFilter {
  /// Scrub secrets from a single log message.
  String redact(String message);

  /// Format a [LogRecord] for sink consumption. Should pass every
  /// secret-bearing field through [redact] before concatenation.
  String format(LogRecord record);
}

class BtcLogFilter implements LogFilter {
  const BtcLogFilter();

  /// Width for right-padded level name in [format] output. Matches the
  /// length of `WARNING`, the longest Level name.
  static const _levelNameWidth = 7;

  /// Match 12-24 consecutive word-runs (letters, case-insensitive)
  /// separated by whitespace (single ASCII space, tab, newline, etc.) —
  /// BIP-39 mnemonic shape. Case-insensitive so user-typed or copied
  /// mnemonics with capital letters are still scrubbed. Uses `\s+` so
  /// tab/newline-separated words also match (prevents the
  /// "false-redaction-marker-on-leaked-secret" bypass where the regex
  /// misses the secret but emits `<redacted-mnemonic>`).
  static final _mnemonicPattern = RegExp(
    r'\b[A-Za-z]+(?:\s+[A-Za-z]+){11,23}\b',
    caseSensitive: false,
  );

  /// Match `--password` and `--password-file` (and their `=`-form
  /// variants `--password=value`, `--password-file=value`) followed by a
  /// single non-whitespace token (the value). `--password-stdin` excluded
  /// — it reads from stdin with no value to redact; including it would
  /// falsely absorb the next flag.
  static final _passwordFlagPattern = RegExp(
    r'--password(?:-file)?(?:\s+|=)\S+',
  );

  /// Scrub mnemonic-shaped strings + `--password*` flag values from
  /// [message]. Returns the redacted message (or [message] unchanged if
  /// no patterns match).
  @override
  String redact(String message) => message
          .replaceAll(_mnemonicPattern, '<redacted-mnemonic>')
          .replaceAllMapped(_passwordFlagPattern, (m) {
        final match = m.group(0);
        if (match == null) return '';
        final flag = match.split(RegExp(r'[\s=]')).first;
        return '$flag <redacted>';
      });

  /// Format a [LogRecord] with ISO-8601 timestamp + level + logger name
  /// + redacted message + optional redacted error + optional redacted stack
  /// trace. Used by `HierarchicalLogger`'s `Logger.root.onRecord`
  /// (Task 10 BtcInvoker will wire this in via
  /// `Logger.root.onRecord = filter.format`).
  @override
  String format(LogRecord record) {
    final ts = record.time.toIso8601String();
    final redacted = redact(record.message);
    final err =
        record.error != null ? ' err=${redact(record.error.toString())}' : '';
    final st = record.stackTrace != null
        ? '\n${redact(record.stackTrace.toString())}'
        : '';
    return '$ts ${record.level.name.padRight(_levelNameWidth)} '
        '${record.loggerName}: $redacted$err$st';
  }
}
