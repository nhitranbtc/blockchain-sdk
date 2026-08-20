// Task 8 (#214) — RAII wrapper for raw secret bytes crossing the
// FFI boundary.
//
// L12 CRITICAL #2 closure. The wrapper enforces three invariants:
//
// 1. **Auto-zeroize on dispose.** Every byte in the heap allocation is
//    overwritten with `0` BEFORE `calloc.free` runs. The Rust side
//    already wraps incoming data in `Secret<Vec<u8>>` (zeroize-on-drop);
//    the Dart side mirrors the lifetime so the secret bytes never sit
//    in freed heap.
//
// 2. **No leak path.** No `toString` override — accidental
//    `print(secret)` or Sentry breadcrumbs render
//    `Instance of 'SecretBuffer'` rather than the plaintext. No `==` /
//    `hashCode` override (no implicit string compare).
//
// 3. **Idempotent dispose.** Call sites use
//    `try { ... } finally { secret.dispose(); }` and can call dispose
//    any number of times without coordination.
//
// **Deterministic dispose contract.** Callers MUST invoke `dispose()`
// when the FFI call returns. There is no `NativeFinalizer` registered
// (would require a C trampoline function pointer not exposed by the
// `ffi` package) — if `dispose()` is skipped, the Dart heap retains
// the secret bytes until GC reclaims the object. This is acceptable
// for the v1 facade because (a) every call site is wrapped in a
// `try/finally` (L12 enforced via code-review), and (b) the Rust side
// zeroizes its own copy on drop regardless.
//
// **No log paths.** This class intentionally has no logging hooks.
// Debug builds that want to inspect length must use `.length` only.

import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

/// RAII wrapper for raw secret bytes (mnemonic phrase, password, etc.)
/// crossing the FFI boundary.
final class SecretBuffer {
  SecretBuffer._(this._ptr, this._length);

  /// Allocates a heap buffer and copies `bytes` into it. The returned
  /// `SecretBuffer` owns the allocation; the caller MUST call
  /// [dispose] when done.
  factory SecretBuffer.allocate(List<int> bytes) {
    final len = bytes.length;
    // calloc(0) returns nullptr on some platforms; allocate a
    // single-byte sentinel so ptr is always non-null and length is
    // authoritative.
    final ptr = calloc<Uint8>(len == 0 ? 1 : len);
    if (len > 0) {
      final view = ptr.asTypedList(len);
      for (var i = 0; i < len; i++) {
        view[i] = bytes[i] & 0xff;
      }
    }
    return SecretBuffer._(ptr, len);
  }

  /// Allocates a heap buffer from the UTF-8 encoding of `s`. The
  /// source `String` reference is NOT retained by this wrapper —
  /// however the caller may still hold a reference (Dart strings are
  /// immutable). For sensitive phrases, callers should overwrite any
  /// local reference (`s = ''`) after the call to release the
  /// Dart-heap copy sooner.
  factory SecretBuffer.fromUtf8(String s) {
    final encoded = utf8.encode(s);
    return SecretBuffer.allocate(encoded);
  }

  /// Pointer to the heap allocation. Becomes invalid after [dispose].
  /// Reading from this pointer after dispose is use-after-free.
  Pointer<Uint8> get ptr => _ptr;

  /// Number of bytes originally allocated. Becomes 0 after [dispose].
  int get length => _length;

  /// Zeros the heap allocation and frees it. Idempotent — safe to
  /// call from `finally` blocks without coordination.
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    final allocLen = _length == 0 ? 1 : _length;
    if (_ptr != nullptr) {
      _ptr.asTypedList(allocLen).fillRange(0, allocLen, 0);
      calloc.free(_ptr);
    }
    _ptr = nullptr;
    _length = 0;
  }

  Pointer<Uint8> _ptr;
  int _length;
  bool _disposed = false;
}
