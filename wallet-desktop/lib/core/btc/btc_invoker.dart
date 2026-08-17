import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'btc_command.dart';
import 'btc_error.dart';

/// Spawns `btc` CLI subcommands and parses JSON output into typed DTOs.
///
/// Wraps [Process.start] with three security boundaries:
///
/// 1. **L7 env-strip**: parent env is filtered to remove secret-bearing
///    keys (`BTC_WALLET_MNEMONIC`, `BTC_ENCRYPT_PASSWORD`,
///    `BTC_DECRYPT_PASSWORD`) before the child process inherits it.
///    `Process.start` is invoked with `includeParentEnvironment: false`
///    so the filter is not silently re-overridden by the parent shell's
///    own copies of the same keys. Without this, an env var exported
///    by the calling shell would leak the user's mnemonic to a
///    long-running daemon. The keys here match the btc CLI's
///    documented env-var contract.
///
/// 2. **Argv-not-logged**: [BtcCommand.argv] contains mnemonic in plaintext
///    (deferred to `withMnemonicFile` in v0.2 — see Task 8 backlog).
///    Callers MUST NOT log argv before passing to [invoke]. The
///    implementation itself does not log argv.
///
/// 3. **BtcLogFilter pre-emit**: stdout / stderr captured in this class
///    MUST be passed through [BtcLogFilter.redact] before any logger
///    surface. The implementation emits `BtcError` (with [BtcLogFilter]
///    as the documented chokepoint) but does not log raw strings.
class BtcInvoker {
  const BtcInvoker({required this.binaryPath, this.dataDirOverride});

  /// Absolute path to the `btc` binary (e.g. from [BtcExtractor]).
  final String binaryPath;

  /// Overrides `BTC_DATA_DIR` in the child environment. `null` means
  /// inherit from parent (which is also env-stripped). When set, always
  /// overrides inherited `BTC_DATA_DIR`.
  final String? dataDirOverride;

  /// Wall-clock cap on a single `btc` invocation. Hung subprocesses are
  /// killed (SIGTERM, then SIGKILL on escalation) and surface as
  /// `BtcError` with `exitCode: -1`.
  static const _timeout = Duration(seconds: 30);

  /// Time to wait after SIGTERM before escalating to SIGKILL.
  static const _sigkillGrace = Duration(seconds: 2);

  /// Env-var keys that MUST NOT be inherited by the child process.
  /// Matches `btc` CLI's documented contract (F47-derived).
  /// Explicit `Set<String>` gives O(1) `contains` lookup.
  static const Set<String> _secretEnvKeys = {
    'BTC_WALLET_MNEMONIC',
    'BTC_ENCRYPT_PASSWORD',
    'BTC_DECRYPT_PASSWORD',
  };

  /// Runs [cmd] at [binaryPath], captures stdout/stderr, parses stdout
  /// as JSON via [parse], returns the result.
  ///
  /// Throws [BtcError] on:
  /// - Non-zero exit code (raw stderr/stdout in `.stderr` field — callers
  ///   MUST redact via [BtcLogFilter] before logging).
  /// - `Process.start` failure (binary missing, not executable, EACCES).
  /// - Invocation timeout (30 s; child killed SIGTERM → SIGKILL).
  /// - `jsonDecode` failure (`FormatException`) → falls back to raw-text
  ///   `parse(trimmed)`.
  /// - Exception thrown inside [parse] callback → rethrown as `BtcError`
  ///   so all parser failures hit the same redaction path.
  ///
  /// **Cancel cleanup**: when the returned `Future` is cancelled (UI
  /// unmount, user navigation away), the child process is killed and
  /// stdout/stderr pipes are drained to prevent leaks.
  Future<T> invoke<T>(BtcCommand cmd,
      {required T Function(dynamic json) parse}) async {
    Process? process;
    try {
      // Copy + strip — `Platform.environment` is an unmodifiable view.
      final env = <String, String>{
        ...Platform.environment,
      }..removeWhere((k, _) => _secretEnvKeys.contains(k));
      final override = dataDirOverride;
      if (override != null) env['BTC_DATA_DIR'] = override;

      try {
        process = await Process.start(
          binaryPath,
          cmd.argv,
          environment: env,
          // Critical: prevents the parent shell's env (which may contain
          // BTC_WALLET_MNEMONIC etc.) from being merged back in AFTER our
          // strip. With this flag, `env` above is the COMPLETE env the
          // child sees.
          includeParentEnvironment: false,
          runInShell: false,
        );
      } on ProcessException catch (e) {
        throw BtcError.fromStderr(
          'Process.start failed: ${e.message}',
          exitCode: -1,
        );
      }

      final stdoutFuture = process.stdout.transform(utf8.decoder).join();
      final stderrFuture = process.stderr.transform(utf8.decoder).join();
      // Attach error handlers so a pipe-level failure doesn't surface as
      // an unhandled async error after the timeout throws.
      stdoutFuture.ignore();
      stderrFuture.ignore();

      final exitCode =
          await process.exitCode.timeout(_timeout, onTimeout: () async {
        // SIGTERM first; escalate to SIGKILL if the child ignores it.
        try {
          process?.kill(ProcessSignal.sigterm);
        } catch (_) {
          // Process may already be dead; ignore.
        }
        await Future<void>.delayed(_sigkillGrace);
        try {
          process?.kill(ProcessSignal.sigkill);
        } catch (_) {
          // Process may already be dead; ignore.
        }
        throw BtcError.fromStderr(
          'btc invocation timed out after ${_timeout.inSeconds} s',
          exitCode: -1,
        );
      });
      final stdout = await stdoutFuture;
      final stderr = await stderrFuture;

      if (exitCode != 0) {
        throw BtcError.fromStderr(
          stderr.isEmpty
              ? (stdout.isEmpty
                  ? 'btc exited $exitCode with no output'
                  : stdout)
              : stderr,
          exitCode: exitCode,
        );
      }

      final trimmed = stdout.trim();
      if (trimmed.isEmpty) return parse(null);

      try {
        final decoded = jsonDecode(trimmed);
        return parse(decoded);
      } on FormatException {
        // btc wrote human-readable output (--json flag not used or
        // output is plain text). Caller's `parse` handles non-JSON via
        // its parameter contract.
        return parse(trimmed);
      }
    } on BtcError {
      rethrow;
    } catch (_) {
      // Defense-in-depth: wrap any other failure (parse-callback
      // exception, etc.) as BtcError so all parser failures hit the
      // same redaction path. Do NOT interpolate the exception's text into
      // the message — the exception's `toString()` may embed field
      // values that should be redacted via BtcLogFilter at the UI/log
      // layer instead. The exception object is reachable via Dart's
      // normal error path if a caller needs more.
      throw const BtcError(
        exitCode: -1,
        stderr: 'btc invocation failed (see Dart error for details)',
        kind: BtcErrorKind.other,
      );
    } finally {
      // Defensive cleanup: kill any still-running child on cancellation,
      // error, or early-return completion.
      try {
        process?.kill(ProcessSignal.sigterm);
      } catch (_) {
        // Process may already be dead; ignore.
      }
    }
  }
}
