// Task 9 (#215) test — typed `FfiException` hierarchy translating
// the 21 `FfiError` codes from `rust-wallet-app/crates/bitcoin-wallet-core/
// src/ffi/error.rs` into Dart exception classes.
//
// L12 CRITICAL #1 closure: the facade's `_ffiError` helper no longer
// throws a generic `Exception` — callers (UI in Tasks 10-16) match
// against typed `FfiException.kind` to render user-facing messages.
//
// L12 CRITICAL #2 closure (defense-in-depth): `FfiException.toString`
// MUST NOT include the Rust error message when the message could
// plausibly contain mnemonic or password bytes. Tests cover the
// redaction behavior.

import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/ffi_exception.dart';

void main() {
  group('FfiException.fromCode (all 23 error cases)', () {
    // Every non-Ok FfiError variant + sentinel maps to a typed kind.
    // The Ok (0) code is forbidden — see the debug-build assertion
    // in `fromCode`. `panic` and `unknown` are FFI-layer sentinels
    // that never come from `bitcoin_wallet_core::Error` but can
    // surface at the boundary.
    final cases = <int, FfiErrorKind>{
      -1: FfiErrorKind.invalidMnemonic,
      -2: FfiErrorKind.invalidDerivationPath,
      -3: FfiErrorKind.addressDerivation,
      -4: FfiErrorKind.scriptBuild,
      -10: FfiErrorKind.network,
      -11: FfiErrorKind.esplora,
      -12: FfiErrorKind.electrum,
      -20: FfiErrorKind.insufficientFunds,
      -21: FfiErrorKind.txBuild,
      -22: FfiErrorKind.sign,
      -23: FfiErrorKind.psbt,
      -30: FfiErrorKind.storage,
      -31: FfiErrorKind.notInitialized,
      -32: FfiErrorKind.encryption,
      -33: FfiErrorKind.mnemonicCipher,
      -34: FfiErrorKind.walletStore,
      -40: FfiErrorKind.bitcoin,
      -41: FfiErrorKind.bdk,
      -42: FfiErrorKind.io,
      -50: FfiErrorKind.bip137,
      -51: FfiErrorKind.spkiPin,
      -100: FfiErrorKind.panic,
      -127: FfiErrorKind.unknown,
    };

    for (final entry in cases.entries) {
      test('code ${entry.key} -> ${entry.value.name}', () {
        final e = FfiException.fromCode(
          code: entry.key,
          op: 'wallet_create',
        );
        expect(e.kind, equals(entry.value));
        expect(e.code, equals(entry.key));
        expect(e.op, equals('wallet_create'));
      });
    }

    test('unknown future code falls back to FfiErrorKind.unknown', () {
      final e = FfiException.fromCode(code: -999, op: 'op');
      expect(e.kind, equals(FfiErrorKind.unknown));
    });

    test('all FfiErrorKind enum values are reachable from some code', () {
      // Drift guard (L12 type-design M2): if a maintainer adds a
      // new enum value but forgets to add a `case` in
      // `_kindFromCode`, that kind becomes unreachable. This test
      // iterates every enum value and asserts at least one code
      // produces it.
      for (final kind in FfiErrorKind.values) {
        final matched = cases.entries
            .where((entry) => entry.value == kind)
            .map((entry) => entry.key);
        expect(matched, isNotEmpty,
            reason: 'FfiErrorKind.${kind.name} unreachable from '
                'FfiException.fromCode — check _kindFromCode switch');
      }
    });
  });

  group('FfiException.sealed hierarchy', () {
    test('is an Exception (caught by `on Exception` in Dart)', () {
      final e = FfiException.fromCode(code: -34, op: 'wallet_create');
      expect(e, isA<Exception>());
    });

    test('exhaustive switch on kind compiles without defaults', () {
      // Pattern-match exhaustiveness check: this compiles only if the
      // kind enum has no missing cases.
      final FfiException e = FfiException.fromCode(code: -1, op: 'op');
      final name = switch (e.kind) {
        FfiErrorKind.invalidMnemonic => 'mnemonic',
        FfiErrorKind.invalidDerivationPath => 'derivation',
        FfiErrorKind.addressDerivation => 'addr',
        FfiErrorKind.scriptBuild => 'script',
        FfiErrorKind.network => 'net',
        FfiErrorKind.esplora => 'esplora',
        FfiErrorKind.electrum => 'electrum',
        FfiErrorKind.insufficientFunds => 'funds',
        FfiErrorKind.txBuild => 'txbuild',
        FfiErrorKind.sign => 'sign',
        FfiErrorKind.psbt => 'psbt',
        FfiErrorKind.storage => 'storage',
        FfiErrorKind.notInitialized => 'notInit',
        FfiErrorKind.encryption => 'encrypt',
        FfiErrorKind.mnemonicCipher => 'mcipher',
        FfiErrorKind.walletStore => 'wstore',
        FfiErrorKind.bitcoin => 'btc',
        FfiErrorKind.bdk => 'bdk',
        FfiErrorKind.io => 'io',
        FfiErrorKind.bip137 => 'bip137',
        FfiErrorKind.spkiPin => 'spki',
        FfiErrorKind.panic => 'panic',
        FfiErrorKind.unknown => 'unknown',
      };
      expect(name, equals('mnemonic'));
    });
  });

  group('FfiException.toString redaction (L12 CRITICAL #2)', () {
    test('includes op + kind + code but NEVER the message', () {
      final e = FfiException.fromCode(
        code: -34,
        op: 'wallet_create',
        messageForDebug: 'abandon abandon abandon about',
      );
      final rendered = e.toString();
      expect(rendered, contains('wallet_create'));
      expect(rendered, contains('-34'));
      expect(rendered, contains('walletStore'));
      // CRITICAL: plaintext mnemonic must not appear in toString.
      expect(rendered.contains('abandon'), isFalse);
    });

    test('includes op + kind + code when no message provided', () {
      final e = FfiException.fromCode(code: -1, op: 'wallet_create');
      final rendered = e.toString();
      expect(rendered, contains('wallet_create'));
      expect(rendered, contains('-1'));
      expect(rendered, contains('invalidMnemonic'));
    });

    test('omits message when message could contain password bytes', () {
      // The Rust sanitizer (sanitize_for_ffi) already scrubs
      // 12/15/18/21/24-word sequences and 64-char hex. We rely on
      // the Rust-side scrubbing for the FFI payload; the Dart
      // toString contract is independent defense-in-depth: never
      // include the raw `messageForDebug` field.
      const password = 'hunter2-secret-password';
      final e = FfiException.fromCode(
        code: -32, // Encryption error
        op: 'wallet_unlock',
        messageForDebug: password,
      );
      final rendered = e.toString();
      expect(rendered.contains(password), isFalse);
    });

    test('omits message when message looks like 64-char hex digest', () {
      // Defense-in-depth: even if the Rust `sanitize_for_ffi`
      // regresses (it doesn't redact hex today — see Rust
      // error.rs), the Dart `toString` still won't leak.
      const hexDigest =
          '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
      final e = FfiException.fromCode(
        code: -42, // Io
        op: 'wallet_storage',
        messageForDebug: hexDigest,
      );
      expect(e.toString().contains(hexDigest), isFalse);
    });

    test('messageForDebug field round-trips (debug-only access)', () {
      final e = FfiException.fromCode(
        code: -34,
        op: 'op',
        messageForDebug: 'debug-info-string',
      );
      expect(e.messageForDebug, equals('debug-info-string'));
    });
  });

  group('FfiException.cause chaining', () {
    test('cause is preserved when provided', () {
      final original = StateError('underlying');
      final e = FfiException.fromCode(
        code: -42,
        op: 'wallet_storage',
        cause: original,
      );
      expect(e.cause, same(original));
    });

    test('cause is null by default', () {
      final e = FfiException.fromCode(code: -1, op: 'op');
      expect(e.cause, isNull);
    });
  });
}
