import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart' show sha256;
import 'package:flutter/services.dart' show rootBundle;
import 'package:path/path.dart' as p;

import '../paths.dart';

/// Base class for any error originating in the bundled-`btc` subsystem.
/// Future Tasks 6 / 8 / 10 add more specific subclasses; callers can catch
/// [BtcException] once and handle the whole family.
abstract class BtcException implements Exception {
  const BtcException(this.message, {this.cause});

  /// Human-readable description of the failure.
  final String message;

  /// Optional underlying cause (retained so loggers can print chained stack).
  final Object? cause;

  @override
  String toString() => cause == null
      ? '$runtimeType: $message'
      : '$runtimeType: $message ($cause)';
}

/// Triplet returned by [hostTarget] describing the bundled asset for the
/// running platform. Public so tests can assert arch/asset-path selection
/// without launching the actual binary.
class HostTarget {
  HostTarget({
    required this.arch,
    required this.assetPath,
    required this.binaryName,
  })  : assert(arch.isNotEmpty, 'arch must be non-empty'),
        assert(
          !binaryName.contains('/') && !binaryName.contains(r'\'),
          'binaryName must be a bare filename, not a path',
        );

  /// Asset directory slug — e.g. `linux-x64`, `macos-arm64`, `windows-x64`.
  final String arch;

  /// Path to the bundled asset relative to the package root.
  final String assetPath;

  /// Final binary filename on disk after extraction.
  final String binaryName;
}

/// Returns the bundled asset triplet for the running host. Linux + macOS
/// distinguish x64 / arm64 by inspecting [Platform.resolvedExecutable];
/// Windows is x64-only for v0.1. The arm64 detection is heuristic — a
/// dev-install path that happens to contain `aarch64` / `arm64` will
/// false-positive; accepted for v0.1.
HostTarget hostTarget() {
  if (Platform.isLinux) {
    if (Platform.resolvedExecutable.contains('aarch64') ||
        Platform.resolvedExecutable.contains('arm64')) {
      return HostTarget(
        arch: 'linux-arm64',
        assetPath: 'assets/btc/linux-arm64/btc',
        binaryName: 'btc',
      );
    }
    return HostTarget(
      arch: 'linux-x64',
      assetPath: 'assets/btc/linux-x64/btc',
      binaryName: 'btc',
    );
  }
  if (Platform.isMacOS) {
    if (Platform.resolvedExecutable.contains('arm64')) {
      return HostTarget(
        arch: 'macos-arm64',
        assetPath: 'assets/btc/macos-arm64/btc',
        binaryName: 'btc',
      );
    }
    return HostTarget(
      arch: 'macos-x64',
      assetPath: 'assets/btc/macos-x64/btc',
      binaryName: 'btc',
    );
  }
  if (Platform.isWindows) {
    return HostTarget(
      arch: 'windows-x64',
      assetPath: 'assets/btc/windows-x64/btc.exe',
      binaryName: 'btc.exe',
    );
  }
  throw const ExtractionException(
    'Unsupported platform — cannot extract bundled btc binary',
  );
}

/// Extracts the bundled `btc` binary into `appDataDir/btc/`. Side effects:
/// reads from the Flutter asset bundle, writes the binary + a JSON manifest
/// to disk (`manifest.json` keyed by sha256 + arch), `chmod 0o755`s the
/// binary on POSIX, and spawns `btc --version` to verify before returning.
///
/// Subsequent calls with the same `(arch, sha256)` skip re-extraction and
/// reuse the cached binary. Throws [ExtractionException] (extends
/// [BtcException]) on empty asset, unsupported platform, corrupt manifest,
/// or post-extract `--version` failure.
Future<String> extractBtc() async {
  final target = hostTarget();
  final btcDir = await subdirFor('btc');
  final manifestFile = File(p.join(btcDir.path, 'manifest.json'));
  final outFile = File(p.join(btcDir.path, target.binaryName));

  final data = await rootBundle.load(target.assetPath);
  final bytes = data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes);
  if (bytes.isEmpty) {
    throw const ExtractionException(
      'Bundled btc asset is empty — populate assets/btc/<arch>/ first',
    );
  }
  final hash = sha256.convert(bytes).toString();

  if (await manifestFile.exists()) {
    try {
      final cached =
          jsonDecode(await manifestFile.readAsString()) as Map<String, dynamic>;
      if (cached['hash'] == hash && cached['arch'] == target.arch) {
        if (await outFile.exists()) return outFile.path;
      }
    } on FormatException catch (e) {
      throw ExtractionException(
        'Corrupt manifest at ${manifestFile.path}',
        cause: e,
      );
    }
  }

  if (await outFile.exists()) await outFile.delete();
  await outFile.writeAsBytes(bytes, flush: true);
  if (!Platform.isWindows) {
    try {
      await Process.run('chmod', ['0o755', outFile.path]);
    } on ProcessException catch (e) {
      throw ExtractionException('Failed to chmod extracted binary', cause: e);
    }
  }

  final ProcessResult result;
  try {
    result = await Process.run(outFile.path, ['--version']);
  } on ProcessException catch (e) {
    await outFile.delete();
    throw ExtractionException(
      'Failed to spawn extracted btc for --version',
      cause: e,
    );
  }
  if (result.exitCode != 0) {
    await outFile.delete();
    throw ExtractionException(
      'Extracted btc failed --version: ${result.stderr}',
    );
  }

  await manifestFile.writeAsString(
    jsonEncode({'hash': hash, 'arch': target.arch}),
  );
  return outFile.path;
}

/// Thrown by [extractBtc] when the bundled asset cannot be staged.
class ExtractionException extends BtcException {
  const ExtractionException(super.message, {super.cause});
}
