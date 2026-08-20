// Task 8 (#214) — typed wrapper around the Rust-side `MnemonicHandle`
// returned by `wallet_create`.
//
// L12 CRITICAL #2 closure (UI side). The wrapper enforces:
//
// 1. **Single source of phrase bytes.** The phrase String lives ONLY
//    inside this object, populated lazily on the first `read()` call
//    from `phraseViewCopy`. The String reference is nulled out on
//    [dispose] so the Dart heap releases the bytes back to GC.
//
// 2. **Handle release contract.** [dispose] calls
//    `WalletOpsBindings.phraseViewFree` exactly once, even if called
//    multiple times. Idempotent.
//
// 3. **No leak path.** No `toString` override — accidental
//    `print(view)` renders `Instance of 'MnemonicView'`. No `==` /
//    `hashCode` override (no implicit phrase compare).
//
// **No log paths.** This class intentionally has no logging hooks.

import 'dart:ffi';

import 'package:ffi/ffi.dart';
import 'package:wallet_desktop/core/ffi/wallet_ops_bindings.dart';

/// Typed wrapper around the opaque Rust-side `MnemonicHandle`. The
/// handle is allocated by `wallet_create` and must be freed via
/// `phraseViewFree` (L12 CRITICAL #2).
final class MnemonicView {
  /// Wraps an opaque `MnemonicHandle` returned by
  /// `WalletOpsBindings.walletCreate`. For tests, any non-null
  /// `Pointer<Void>` is acceptable — dispose is the only operation
  /// that touches the binding (and it null-checks before the call).
  MnemonicView(Pointer<Void> handle) : _handle = handle;

  Pointer<Void>? _handle;
  String? _cached;
  bool _disposed = false;

  /// Whether [dispose] has been called.
  bool get isDisposed => _disposed;

  /// Reads the phrase from the handle. The Rust side returns a
  /// NUL-terminated `*const c_char` borrowed from the handle's heap;
  /// this wrapper copies it into a Dart `String` and drops the
  /// borrowed pointer (Rust retains ownership).
  ///
  /// Throws [StateError] if [dispose] has been called.
  String read() {
    if (_disposed) {
      throw StateError('MnemonicView disposed');
    }
    final cached = _cached;
    if (cached != null) return cached;
    final handle = _handle;
    if (handle == null || handle == nullptr) {
      throw StateError('MnemonicView handle is null');
    }
    final ptr = WalletOpsBindings.phraseViewCopy(handle);
    final phrase = ptr.toDartString();
    _cached = phrase;
    return phrase;
  }

  /// Frees the Rust-side `MnemonicHandle` via `phrase_view_free`,
  /// nulls the cached phrase String, and invalidates the wrapper.
  /// Idempotent.
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    final handle = _handle;
    if (handle != null && handle != nullptr) {
      WalletOpsBindings.phraseViewFree(handle);
    }
    _handle = nullptr;
    _cached = null;
  }
}