// Task 6 (#212) test — FFI DynamicLibrary loader.
//
// Verifies:
//   1. defaultBasePath() returns the platform-specific subdirectory
//      (arch-aware on macOS arm64 vs x64)
//   2. libNameForPlatform(os) returns the correct file name
//   3. open(basePath:) resolves and looks up a known symbol
//   4. open(basePath: '') throws ArgumentError (contract)
//   5. open(basePath: <missing>) surfaces the underlying load failure
//
// `ffi_version` is the one FFI symbol guaranteed to be present in every
// build (Task 1 spike artifact, exported from the `wallet` module). Tasks
// 2-5 add more exports; it's not the loader's job to assert their presence
// before build-native infra (Task 18) exists.

import 'dart:ffi';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/native_lib.dart';

void main() {
  group('NativeLib.defaultBasePath', () {
    test(
      'returns native/linux-x64 on Linux',
      () {
        expect(NativeLib.defaultBasePath(), 'native/linux-x64');
      },
      skip: !Platform.isLinux,
    );

    test(
      'returns native/macos-arm64 or macos-x64 on macOS per host arch',
      () {
        final expected = Platform.resolvedExecutable.contains('arm64')
            ? 'native/macos-arm64'
            : 'native/macos-x64';
        expect(NativeLib.defaultBasePath(), expected);
      },
      skip: !Platform.isMacOS,
    );

    test(
      'returns native/windows-x64 on Windows',
      () {
        expect(NativeLib.defaultBasePath(), 'native/windows-x64');
      },
      skip: !Platform.isWindows,
    );
  });

  group('NativeLib.libNameForPlatform', () {
    test('returns librust_wallet_core.so on linux', () {
      expect(
        NativeLib.libNameForPlatform('linux'),
        'librust_wallet_core.so',
      );
    });

    test('returns librust_wallet_core.dylib on macos', () {
      expect(
        NativeLib.libNameForPlatform('macos'),
        'librust_wallet_core.dylib',
      );
    });

    test('returns rust_wallet_core.dll on windows', () {
      expect(
        NativeLib.libNameForPlatform('windows'),
        'rust_wallet_core.dll',
      );
    });

    test('throws UnsupportedError on unknown platform', () {
      expect(
        () => NativeLib.libNameForPlatform('plan9'),
        throwsUnsupportedError,
      );
    });
  });

  group('NativeLib.open', () {
    test(
      'loads library from explicit basePath on Linux',
      () {
        final lib = NativeLib.open(basePath: 'native/linux-x64');
        // ffi_version is exported by the FFI module in every build.
        final sym = lib.lookup<NativeFunction<Int32 Function()>>('ffi_version');
        expect(sym, isNotNull);
      },
      skip: !Platform.isLinux,
    );

    test(
      'default open() resolves on Linux when lib is next to cwd',
      () {
        // Default basePath assumes cwd = wallet-desktop/. The Task 18
        // build-native.sh will guarantee the lib is in the right place at
        // runtime; this test exercises the no-arg path explicitly.
        final lib = NativeLib.open();
        final sym = lib.lookup<NativeFunction<Int32 Function()>>('ffi_version');
        expect(sym, isNotNull);
      },
      skip: !Platform.isLinux,
    );

    test('throws ArgumentError on empty basePath', () {
      expect(
        () => NativeLib.open(basePath: ''),
        throwsArgumentError,
      );
    });

    test('throws ArgumentError on whitespace-only basePath', () {
      expect(
        () => NativeLib.open(basePath: '   '),
        throwsArgumentError,
      );
    });

    test(
      'propagates load failure when lib is missing',
      () {
        expect(
          () => NativeLib.open(basePath: 'native/does-not-exist'),
          throwsA(isA<ArgumentError>().having(
            (e) => e.message,
            'message',
            contains('Failed to load dynamic library'),
          )),
        );
      },
      skip: !Platform.isLinux,
    );
  });
}
