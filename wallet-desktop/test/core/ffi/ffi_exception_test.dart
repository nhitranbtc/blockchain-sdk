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

  group('FfiException.lastError (Issue #265 — Rust thread-local surface)', () {
    test('lastError is null by default', () {
      final e = FfiException.fromCode(code: -1, op: 'wallet_create');
      expect(e.lastError, isNull);
    });

    test(
      'lastError is preserved when provided to fromCode '
      '(facade _ffiError will populate from ffi_last_error_message)',
      () {
        final e = FfiException.fromCode(
          code: -11,
          op: 'esplora_client_new',
          lastError: 'reqwest client build: builder error',
        );
        expect(e.lastError, equals('reqwest client build: builder error'));
      },
    );

    test(
      'toString NEVER includes lastError (L12 CRITICAL #2 — lastError is '
      'display-only in UI, not logged via toString)',
      () {
        final e = FfiException.fromCode(
          code: -11,
          op: 'esplora_client_new',
          lastError: 'rustls crypto provider missing: aws_lc_rs',
        );
        final rendered = e.toString();
        expect(rendered.contains('rustls'), isFalse);
        expect(rendered.contains('aws_lc_rs'), isFalse);
      },
    );

    test(
      '12-word BIP-39 mnemonic-shaped payload in lastError does NOT '
      'leak via toString, the kind-only user copy, or the per-op user '
      'copy (L12 CRITICAL #2 + L12 review MEDIUM #2 — load-bearing '
      'redaction check)',
      () {
        // The 12-word sequence below is the canonical BIP-39 "abandon"
        // test vector — the exact pattern `BtcLogFilter.redact`
        // strips. The previous `rustls crypto provider missing` test
        // only had 5 words and never tripped the BIP-39 regex, so
        // this assertion is the load-bearing one.
        const mnemonic = 'abandon abandon abandon abandon abandon abandon '
            'abandon abandon abandon abandon abandon about';
        final e = FfiException.fromCode(
          code: -11,
          op: 'esplora_client_new',
          lastError: mnemonic,
        );
        // 1. toString — used by `developer.log` / Sentry sinks.
        expect(e.toString().contains('abandon'), isFalse);
        // 2. kind-only user copy — legacy callers.
        expect(
          userMessageForFfiException(e).contains('abandon'),
          isFalse,
        );
        // 3. per-op user copy — the new SendScreen render path.
        expect(
          userMessageForFfiExceptionWithOp(e).contains('abandon'),
          isFalse,
        );
        // 4. Direct field access is allowed (debug-only), but
        // confirms the field actually carries the payload (the
        // sink-side scrubbing is the only defense — verified by
        // BtcLogFilter unit tests separately).
        expect(e.lastError, equals(mnemonic));
      },
    );
  });

  group('userMessageForFfiExceptionWithOp (Issue #265 C1 fix)', () {
    test(
      'esplora_client_new renders Esplora copy (was "Invalid recovery phrase" '
      'pre-fix)',
      () {
        final e = FfiException.fromCode(code: -1, op: 'esplora_client_new');
        final copy = userMessageForFfiExceptionWithOp(e);
        expect(copy, contains('Esplora'));
        expect(copy, isNot(contains('recovery phrase')));
      },
    );

    test(
      'wallet_show renders password copy (was generic "Invalid recovery '
      'phrase")',
      () {
        final e = FfiException.fromCode(code: -34, op: 'wallet_show');
        final copy = userMessageForFfiExceptionWithOp(e);
        expect(copy, contains('password'));
        expect(copy, isNot(contains('recovery phrase')));
      },
    );

    test('wallet_load renders corruption copy', () {
      final e = FfiException.fromCode(code: -34, op: 'wallet_load');
      final copy = userMessageForFfiExceptionWithOp(e);
      expect(copy, contains('corrupted'));
    });

    test('wallet_send renders broadcast copy', () {
      final e = FfiException.fromCode(code: -1, op: 'wallet_send');
      final copy = userMessageForFfiExceptionWithOp(e);
      expect(copy, contains('broadcast'));
    });

    test(
      'unknown op falls back to kind-only userMessageForFfiException '
      '(no regression for legacy callers)',
      () {
        final e = FfiException.fromCode(code: -1, op: 'wallet_create');
        final copy = userMessageForFfiExceptionWithOp(e);
        expect(copy, equals(userMessageForFfiException(e)));
      },
    );
  });
}
