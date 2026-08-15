import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;

import '../paths.dart';

final _random = Random.secure();

/// Writes [secret] to a temp file under `appDataDir/tmp/<uuid>.pwd`,
/// invokes [body] with the file path, then unlinks the file in `finally`
/// (including when [body] throws).
///
/// **Atomic create with 0o600 mode (POSIX)**: writes content to a sibling
/// `<path>.lock` file, `chmod 0o600` it, then POSIX `rename(2)`-es the
/// lock file to the final path. POSIX `rename` is atomic within a
/// filesystem — the final path either appears with mode 0o600 or not at
/// all. No world-readable intermediate state.
///
/// On Windows, POSIX `rename` semantics are unavailable; we write
/// directly to the final path. Windows security relies on `appDataDir`
/// being per-user (no further ACL mitigation in v0.1).
///
/// **Unlink guarantee**: the file is deleted in a `finally` block — even
/// on exception. Unlink is best-effort: if the OS refuses (file in use,
/// EPERM), a `WARNING` log fires (path redacted, gated by `kDebugMode`).
/// The secret PERSISTS on the user's app data dir until manual cleanup.
///
/// **Refuses to overwrite** an existing file at the computed path
/// (defense against predictable UUID generation — paranoid defense).
///
/// **Zeroize caveat (Dart)**: `String` is immutable in Dart and cannot
/// be zeroized on drop. The [secret] parameter lingers in heap until
/// GC. Caller SHOULD adopt `Secret<String>` (F47 zeroize wrapper)
/// before v0.2 lands.
///
/// **L12 CRITICAL #2**: Never log [secret]. Never log [path] in
/// production — `TempSecretFileException.path` is `kDebugMode`-gated.
Future<void> withTempSecretFile(
  String secret,
  Future<void> Function(String path) body,
) async {
  final tmpDir = await subdirFor('tmp');
  final path = p.join(tmpDir.path, '${_uuidV4()}.pwd');
  final lockPath = '$path.lock';

  await _stageContent(lockPath, path, secret);

  try {
    await body(path);
  } finally {
    // Best-effort unlink: final path first, then any orphaned lock.
    await _unlinkQuiet(path);
    await _unlinkQuiet(lockPath);
  }
}

/// Stages secret content at [path] with the right POSIX mode applied
/// before any other process can observe [path]. POSIX uses lock +
/// chmod + atomic rename; Windows writes directly (default ACL).
Future<void> _stageContent(String lockPath, String path, String secret) async {
  final lockFile = File(lockPath);
  final finalFile = File(path);

  if (await finalFile.exists()) {
    throw const TempSecretFileException(TempSecretFileFailure.pathInUse);
  }

  if (!Platform.isWindows) {
    try {
      // Write to lock file.
      final lockRaf = await lockFile.open(mode: FileMode.writeOnly);
      try {
        await lockRaf.writeString(secret);
        await lockRaf.flush();
      } finally {
        await lockRaf.close();
      }

      // chmod 0o600 BEFORE the atomic rename. The `0o` Dart literal
      // prefix is invalid POSIX mode syntax — pass the bare octal `600`.
      final chmodResult = await Process.run('chmod', ['600', lockPath]);
      if (chmodResult.exitCode != 0) {
        if (await lockFile.exists()) await lockFile.delete();
        throw TempSecretFileException(
          TempSecretFileFailure.chmodFailed,
          cause: chmodResult.stderr.toString(),
        );
      }

      // Atomic POSIX rename — closes the write→chmod race window.
      await lockFile.rename(path);
    } catch (e) {
      if (await lockFile.exists()) await lockFile.delete();
      if (await finalFile.exists()) await finalFile.delete();
      rethrow;
    }
    return;
  }

  // Windows path (no chmod available).
  final raf = await finalFile.open(mode: FileMode.writeOnly);
  try {
    await raf.writeString(secret);
    await raf.flush();
  } finally {
    await raf.close();
  }
}

Future<void> _unlinkQuiet(String path) async {
  try {
    await File(path).delete();
  } catch (e) {
    if (kDebugMode) {
      // ignore: avoid_print
      print('TempSecretFile: unlink failed for <path-redacted>: $e');
    }
  }
}

String _uuidV4() {
  final bytes = List<int>.generate(16, (_) => _random.nextInt(256));
  bytes[6] = (bytes[6] & 0x0F) | 0x40;
  bytes[8] = (bytes[8] & 0x3F) | 0x80;
  final hex = bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
      '${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
}

/// Typed failure codes (no path or secret content in user-visible message).
enum TempSecretFileFailure {
  /// Computed path already exists (paranoid UUID-collision defense).
  pathInUse,

  /// `chmod 0o600` failed on POSIX (file remains with creation perms).
  chmodFailed,
}

/// Base class for any error originating in the secret-handling subsystem.
/// `sealed` (Dart 3) forces exhaustiveness checks for `switch` on
/// [SecretException]; future Tasks 6 / 7 add more specific subclasses
/// (PasswordSupplyException, SecretDisposalException, etc.). Parallels
/// `BtcException` from Task 4 (`lib/core/binary/btc_extractor.dart:13`).
sealed class SecretException implements Exception {
  const SecretException(this.failure, {this.path, this.cause});

  /// Typed failure code (path-free, secret-free).
  final TempSecretFileFailure failure;

  /// Diagnostic-only — `kDebugMode`-gated at use site.
  final String? path;

  /// Optional underlying cause; redacted if it contains path-shaped strings.
  final Object? cause;

  /// User-visible message (safe to log in production).
  String get message => switch (failure) {
        TempSecretFileFailure.pathInUse =>
          'Refusing to overwrite existing temp file',
        TempSecretFileFailure.chmodFailed =>
          'Failed to chmod temp file to 0o600 (POSIX only)',
      };

  @override
  String toString() {
    final base = '$runtimeType($failure): $message';
    if (kDebugMode && path != null) {
      return '$base [path=<redacted-in-prod>; cause=<redacted>]';
    }
    return base;
  }
}

/// Thrown by [withTempSecretFile] on atomic-create refusal or chmod failure.
/// `message` is safe to log in production (no path or secret content).
/// [path] is `kDebugMode`-gated diagnostic — never include in production logs.
class TempSecretFileException extends SecretException {
  const TempSecretFileException(super.failure, {super.path, super.cause});
}
