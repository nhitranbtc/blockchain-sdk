// Task 7 (#213) — typed FFI wrappers for the tokio runtime handle
// (Task 3 surface).
//
// Mirrors the `extern "C"` exports in
// `rust-wallet-app/crates/bitcoin-wallet-core/src/ffi/runtime.rs`.
//
// The runtime handle is constructed once at app startup and passed
// to every async FFI export (see `esplora_bindings.dart`). The Dart
// side blocks the calling isolate until the future resolves — no
// Dart-side `Future` plumbing, no callback registration.
//
// **Why `static` fields, not a global struct?** The runtime is
// created exactly once per process. Exposing the lifecycle as a
// pair of static functions (`runtimeNew` + `runtimeDrop`) keeps the
// surface minimal — no singleton handle, no global state on the
// Dart side. The handle pointer is opaque to Dart and only used as
// a void-pointer argument to other FFI exports.

// `unused_field` on `_lib` is intentional — see wallet_ops_bindings.
// ignore_for_file: unused_field

import 'dart:ffi';

import 'package:wallet_desktop/core/ffi/native_lib.dart';

typedef _RuntimeNewC = Pointer<Void> Function();
typedef _RuntimeNewDart = Pointer<Void> Function();

typedef _RuntimeDropC = Void Function(Pointer<Void>);
typedef _RuntimeDropDart = void Function(Pointer<Void>);

/// Typed FFI wrappers for the tokio runtime handle (Task 3).
class RuntimeBindings {
  RuntimeBindings._();

  static final DynamicLibrary _lib = NativeLib.open();

  /// Constructs a new tokio runtime. Returns an opaque `*mut c_void`
  /// that the Dart side stores and passes to every async FFI export.
  /// Caller MUST eventually call [runtimeDrop] on the returned pointer.
  static final Pointer<Void> Function() runtimeNew =
      _lib.lookupFunction<_RuntimeNewC, _RuntimeNewDart>('runtime_new');

  /// Drops the tokio runtime previously created by [runtimeNew].
  /// Null is a no-op. After this call, the handle MUST NOT be used.
  static final void Function(Pointer<Void> handle) runtimeDrop =
      _lib.lookupFunction<_RuntimeDropC, _RuntimeDropDart>('runtime_drop');
}
