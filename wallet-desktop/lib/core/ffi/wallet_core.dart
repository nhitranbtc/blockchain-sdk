// Minimal Dart-side FFI binding — Task 1 spike.
//
// Loads `librust_wallet_core.so` (or platform equivalent) via
// `DynamicLibrary.open`, looks up the two C exports, exercises one.
//
// Goal: prove the FFI path works end-to-end before Phase 1 expands the
// surface (Task 2: error mapping, Task 3: runtime bridge, Task 4: wallet
// ops, Task 5: Esplora).

// `unused_field` on `_lib` is intentional — the field is a GC anchor
// that keeps the DynamicLibrary loaded while cached function pointers
// are alive. The Dart analyzer doesn't recognize this usage pattern.
// ignore_for_file: unused_field

import 'dart:ffi';
import 'dart:io' show Platform;
import 'package:ffi/ffi.dart';

typedef _WalletListC = Int32 Function(Int8, Pointer<Pointer<Utf8>>);
typedef _WalletListDart = int Function(int, Pointer<Pointer<Utf8>>);

typedef _WalletListFreeC = Void Function(Pointer<Utf8>);
typedef _WalletListFreeDart = void Function(Pointer<Utf8>);

typedef _FfiVersionC = Pointer<Utf8> Function();
typedef _FfiVersionDart = Pointer<Utf8> Function();

typedef _FfiVersionFreeC = Void Function(Pointer<Utf8>);
typedef _FfiVersionFreeDart = void Function(Pointer<Utf8>);

class WalletCore {
  WalletCore._(this._lib)
      : _walletList =
            _lib.lookupFunction<_WalletListC, _WalletListDart>('wallet_list'),
        _walletListFree =
            _lib.lookupFunction<_WalletListFreeC, _WalletListFreeDart>(
                'wallet_list_free'),
        _ffiVersion =
            _lib.lookupFunction<_FfiVersionC, _FfiVersionDart>('ffi_version'),
        _ffiVersionFree =
            _lib.lookupFunction<_FfiVersionFreeC, _FfiVersionFreeDart>(
                'ffi_version_free');

  /// Opens the native library for the current platform.
  ///
  /// Lookup order:
  /// 1. `<cwd>/native/<arch>/librust_wallet_core.so` (operator-built)
  /// 2. `DynamicLibrary.open('librust_wallet_core.so')` (system path)
  factory WalletCore.open() {
    final lib = _openLib();
    return WalletCore._(lib);
  }

  static DynamicLibrary _openLib() {
    final candidates = <String>[
      // Operator-built native lib (Phase 4 Task 18 canonical location)
      if (Platform.isLinux) 'native/linux-x64/librust_wallet_core.so',
      if (Platform.isMacOS) 'native/macos-arm64/librust_wallet_core.dylib',
      if (Platform.isMacOS) 'native/macos-x64/librust_wallet_core.dylib',
      if (Platform.isWindows) 'native/windows-x64/rust_wallet_core.dll',
      // Fallback: system path lookup
      if (Platform.isLinux) 'librust_wallet_core.so',
      if (Platform.isMacOS) 'librust_wallet_core.dylib',
      if (Platform.isWindows) 'rust_wallet_core.dll',
    ];
    Object? lastErr;
    for (final path in candidates) {
      try {
        return DynamicLibrary.open(path);
      } catch (e) {
        lastErr = e;
      }
    }
    throw StateError(
      'Could not open librust_wallet_core. Tried: $candidates. Last error: $lastErr',
    );
  }

  // `_lib` is a GC anchor: keeps the DynamicLibrary loaded while the
  // cached function pointers below are alive. Without this reference,
  // GC could unload the .so + crash on the next FFI call.
  final DynamicLibrary _lib;
  final _WalletListDart _walletList;
  final _WalletListFreeDart _walletListFree;
  final _FfiVersionDart _ffiVersion;
  final _FfiVersionFreeDart _ffiVersionFree;

  /// Returns the rust crate version (sanity check).
  String version() {
    final ptr = _ffiVersion();
    if (ptr == nullptr) throw StateError('ffi_version returned null');
    try {
      return ptr.toDartString();
    } finally {
      _ffiVersionFree(ptr);
    }
  }

  /// Lists wallets for the given network. Returns a list of UUID strings.
  ///
  /// Throws [StateError] if the FFI call fails (negative return code).
  List<String> listWallets({required String network}) {
    final outPtr = calloc<Pointer<Utf8>>();
    try {
      final rc = _walletList(_networkByte(network), outPtr);
      if (rc != 0) {
        throw StateError('wallet_list failed: rc=$rc');
      }
      final ptr = outPtr.value;
      if (ptr == nullptr) return const <String>[];
      try {
        final joined = ptr.toDartString();
        if (joined.isEmpty) return const <String>[];
        return joined.split('\n').where((s) => s.isNotEmpty).toList();
      } finally {
        _walletListFree(ptr);
      }
    } finally {
      calloc.free(outPtr);
    }
  }

  static int _networkByte(String network) {
    switch (network) {
      case 'testnet':
        return 1;
      case 'mainnet':
        return 2;
      case 'regtest':
        return 3;
      case 'signet':
        return 4;
      case 'testnet4':
        return 5;
      default:
        throw ArgumentError('unknown network: $network');
    }
  }
}
