import 'package:flutter_test/flutter_test.dart';
import 'package:logging/logging.dart';
import 'package:wallet_desktop/core/logging/btc_log_filter.dart';

void main() {
  const filter = BtcLogFilter();

  test('redacts a 12-word mnemonic, preserving context', () {
    const msg = 'about to sign with: abandon abandon abandon abandon '
        'abandon abandon abandon abandon abandon abandon abandon about';
    expect(
      filter.redact(msg),
      'about to sign with: <redacted-mnemonic>',
    );
  });

  test('redacts 24-word mnemonic, preserving context', () {
    const msg = 'words: abandon abandon abandon abandon abandon abandon '
        'abandon abandon abandon abandon abandon abandon abandon abandon '
        'abandon abandon abandon abandon abandon abandon abandon abandon '
        'abandon art';
    expect(filter.redact(msg), 'words: <redacted-mnemonic>');
  });

  test('redacts capitalized mnemonic (case-insensitive)', () {
    const msg = 'Abandon Abandon Abandon Abandon Abandon Abandon '
        'Abandon Abandon Abandon Abandon Abandon About';
    expect(filter.redact(msg), '<redacted-mnemonic>');
  });

  test('redacts tab-separated mnemonic (no false-marker leak)', () {
    const msg = 'abandon\tabandon\tabandon\tabandon\tabandon\tabandon\t'
        'abandon\tabandon\tabandon\tabandon\tabandon\tabout';
    // CRITICAL: must NOT leak the secret. If the regex misses, it must
    // miss cleanly without stamping a `<redacted>` marker on a leak.
    final out = filter.redact(msg);
    expect(out, isNot(contains('abandon')),
        reason: 'tab-separated mnemonic must be fully redacted');
    expect(out, contains('<redacted-mnemonic>'));
  });

  test('redacts newline-separated mnemonic', () {
    const msg = 'abandon\nabandon\nabandon\nabandon\nabandon\nabandon\n'
        'abandon\nabandon\nabandon\nabandon\nabandon\nabout';
    final out = filter.redact(msg);
    expect(out, isNot(contains('abandon')));
    expect(out, contains('<redacted-mnemonic>'));
  });

  test('does NOT redact 11-word partial mnemonic (below 12-word floor)', () {
    // EXACTLY 11 lowercase words — no more, no less.
    const msg = 'abandon abandon abandon abandon abandon abandon abandon '
        'abandon abandon abandon about';
    expect(filter.redact(msg), msg);
  });

  test('redacts 13-word sequence (above floor)', () {
    const msg = 'abandon abandon abandon abandon abandon abandon abandon '
        'abandon abandon abandon abandon abandon about';
    expect(filter.redact(msg), '<redacted-mnemonic>');
  });

  test('does NOT redact random 5-word English phrase', () {
    const msg = 'hello world from the test runner';
    expect(filter.redact(msg), msg);
  });

  test('redacts --password flag value', () {
    expect(
      filter.redact('cmd --password hunter2 --network testnet'),
      'cmd --password <redacted> --network testnet',
    );
  });

  test('redacts --password-file flag value', () {
    expect(
      filter.redact('cmd --password-file /run/secrets/btc-pwd'),
      'cmd --password-file <redacted>',
    );
  });

  test('redacts --password=value equals form', () {
    expect(
      filter.redact('cmd --password=hunter2 --network testnet'),
      'cmd --password <redacted> --network testnet',
    );
  });

  test('redacts --password-file=value equals form', () {
    expect(
      filter.redact('cmd --password-file=/run/secrets/btc-pwd'),
      'cmd --password-file <redacted>',
    );
  });

  test('redacts tab-separated --password value', () {
    expect(
      filter.redact('cmd --password\thunter2 --network testnet'),
      'cmd --password <redacted> --network testnet',
    );
  });

  test('does NOT absorb next flag as --password-stdin value', () {
    // --password-stdin reads from stdin; no value to redact. Regex must
    // not falsely match the following flag as if it were a value.
    expect(
      filter.redact('cmd --password-stdin --network testnet'),
      'cmd --password-stdin --network testnet',
    );
  });

  test('format() applies redaction on message', () {
    // package:logging 1.3.0 LogRecord signature: (level, message,
    // loggerName, {error, stackTrace, time, zone}). Plan §Task 7 used
    // a different positional order; corrected here.
    final record = LogRecord(
      Level.INFO,
      'sign with: abandon abandon abandon abandon '
          'abandon abandon abandon abandon abandon abandon abandon about',
      'wallet',
    );
    final out = filter.format(record);
    expect(out, contains('<redacted-mnemonic>'));
    expect(out, isNot(contains('abandon')));
  });

  test('format() redacts error and stackTrace (secrets in exceptions)', () {
    final record = LogRecord(
      Level.SEVERE,
      'signing failed',
      'short message',
      'error: signing failed for mnemonic abandon abandon abandon abandon '
          'abandon abandon abandon abandon abandon abandon abandon about',
      StackTrace.current,
    );
    final out = filter.format(record);
    expect(out, contains('<redacted-mnemonic>'));
    expect(out, isNot(contains('abandon')));
    expect(out, contains('err='));
    expect(out, isNot(contains('error: signing failed for mnemonic')));
  });
}
