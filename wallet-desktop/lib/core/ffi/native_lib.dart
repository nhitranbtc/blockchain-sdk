// Task 6 (#212) — DynamicLibrary loader for `bitcoin-wallet-core` FFI.
//
// Resolves the platform-specific native library at `native/<arch>/` and
// returns a `DynamicLibrary` for downstream `lookup<NativeFunction<...>>`
// calls (Tasks 7-9 will build typed wrappers on top of this).
//
// Platform layout (canonical, single source of truth in `_HostOs`):
//   native/linux-x64/librust_wallet_core.so
//   native/macos-arm64/librust_wallet_core.dylib
//   native/macos-x64/librust_wallet_core.dylib
//   native/windows-x64/rust_wallet_core.dll
//
// macOS arm64 vs x64 is detected at runtime via `Platform.resolvedExecutable`
// — Task 18's `build_native.sh` (per the plan) emits the dylib into the
// `native/<arch>/` directory the loader asks for. The arch-detection
// pattern mirrors `lib/core/binary/btc_extractor.dart:hostTarget()`.
//
// The native lib is operator-built via `build_native.sh` (Task 18) and
// lives alongside the wallet-desktop CWD at runtime. This module only
// does path resolution + `DynamicLibrary.open` — it does NOT assert
// symbol presence (build infra is the source of truth for that).

import 'dart:ffi';
import 'dart:io';

import 'package:path/path.dart' as p;

/// Canonical platform mapping for the native library.
///
/// Adding a new platform = adding one enum entry. No other call site
/// needs to change. The enum is private because the public surface
/// remains `String`-typed (callers should pass lowercase OS strings or
/// rely on `_HostOs.detect()`).
enum _HostOs {
  linux(subdir: 'native/linux-x64', libName: 'librust_wallet_core.so'),
  macosArm64(
      subdir: 'native/macos-arm64', libName: 'librust_wallet_core.dylib'),
  macosX64(subdir: 'native/macos-x64', libName: 'librust_wallet_core.dylib'),
  windows(subdir: 'native/windows-x64', libName: 'rust_wallet_core.dll');

  const _HostOs({required this.subdir, required this.libName});

  /// The directory (relative to CWD) that contains the runtime library.
  final String subdir;

  /// The file name of the runtime library.
  final String libName;

  /// Detect the host OS + arch for the running process. Throws
  /// [UnsupportedError] if the host is none of the 4 supported combos.
  static _HostOs detect() {
    if (Platform.isLinux) return _HostOs.linux;
    if (Platform.isMacOS) {
      // Mirrors `btc_extractor.dart:hostTarget()` — see L12 code-review
      // H1 (Task 6): Intel macs would otherwise hit `FileSystemException`
      // because the loader hard-coded `macos-arm64` and the dylib gets
      // emitted into `native/macos-x64/` by Task 18's `build_native.sh`.
      return Platform.resolvedExecutable.contains('arm64')
          ? _HostOs.macosArm64
          : _HostOs.macosX64;
    }
    if (Platform.isWindows) return _HostOs.windows;
    throw UnsupportedError(
      'Unsupported platform for native lib: ${Platform.operatingSystem}',
    );
  }

  /// Parse a lowercase OS string. Throws [UnsupportedError] on unknown.
  /// Used by the public `libNameForPlatform` helper for testability —
  /// does NOT perform arch detection (arch is only knowable at runtime).
  /// For `defaultBasePath` callers, use [_HostOs.detect] instead.
  static _HostOs parse(String os) {
    switch (os) {
      case 'linux':
        return _HostOs.linux;
      case 'macos':
        // Default to arm64 for the string-only path (matches the
        // Task 1 spike's `Platform.resolvedExecutable` fallback path).
        return _HostOs.macosArm64;
      case 'windows':
        return _HostOs.windows;
      default:
        throw UnsupportedError('Unsupported platform for native lib: $os');
    }
  }
}

/// Static-only loader for the `bitcoin-wallet-core` native library.
///
/// No instance state; private constructor prevents instantiation.
class NativeLib {
  const NativeLib._();

  /// Open the native library for the current platform.
  ///
  /// [basePath] is the directory containing the platform-specific library
  /// (e.g. `native/linux-x64`). Defaults to [defaultBasePath], which
  /// resolves to the platform-specific subdirectory of `native/`.
  ///
  /// Throws [ArgumentError] if [basePath] is empty or whitespace-only.
  /// Throws [UnsupportedError] on an unsupported host platform.
  /// Propagates any `ArgumentError` / `FileSystemException` from
  /// `DynamicLibrary.open` (e.g. lib not found at the resolved path).
  static DynamicLibrary open({String? basePath}) {
    final resolved = (basePath ?? defaultBasePath()).trim();
    if (resolved.isEmpty) {
      throw ArgumentError.value(
        basePath,
        'basePath',
        'must be non-empty (use defaultBasePath() for the platform default)',
      );
    }
    final host = _HostOs.detect();
    return DynamicLibrary.open(p.join(resolved, host.libName));
  }

  /// Returns the platform-specific `native/<arch>/` subdirectory that
  /// contains the runtime library. Performs runtime arch detection on
  /// macOS (arm64 vs x64) so the same build script (Task 18) works for
  /// both Intel and Apple Silicon hosts.
  static String defaultBasePath() => _HostOs.detect().subdir;

  /// Returns the file name of the native library for the given OS string.
  ///
  /// Pure helper — does not touch `Platform`. Exposed (not private) so
  /// tests can verify the platform → file-name mapping without spawning
  /// a sub-process and so future Tasks 7-9 can verify which file was
  /// loaded.
  ///
  /// Throws [UnsupportedError] on an unknown OS string. The `'macos'`
  /// branch returns the arm64 dylib name (same filename on x64; the
  /// arch is conveyed by the directory, not the file).
  static String libNameForPlatform(String os) => _HostOs.parse(os).libName;
}
