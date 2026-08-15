import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/btc/btc_error.dart';

void main() {
  test('wrongPassword maps to WrongPassword kind', () {
    final err = BtcError.fromStderr('error: wrong password (try again)', exitCode: 2);
    expect(err.kind, BtcErrorKind.wrongPassword);
  });

  test('insufficient funds maps to InsufficientFunds', () {
    final err = BtcError.fromStderr('error: insufficient funds', exitCode: 4);
    expect(err.kind, BtcErrorKind.insufficientFunds);
  });

  test('unknown wallet maps to UnknownWallet', () {
    final err = BtcError.fromStderr("error: wallet 'abc' not found", exitCode: 4);
    expect(err.kind, BtcErrorKind.unknownWallet);
  });

  test('network/esplora unreachable maps to NetworkError', () {
    final err = BtcError.fromStderr('error: esplora unreachable: 504', exitCode: 3);
    expect(err.kind, BtcErrorKind.networkError);
  });

  test('unknown stderr maps to Other', () {
    final err = BtcError.fromStderr('some weird thing', exitCode: 1);
    expect(err.kind, BtcErrorKind.other);
  });

  test('cross-network address rejection maps to UnknownAddressType '
      '(more-specific pattern wins over networkError)', () {
    final err = BtcError.fromStderr(
      'error: address tb1q... does not match network mainnet',
      exitCode: 4,
    );
    expect(err.kind, BtcErrorKind.unknownAddressType);
  });

  test('--confirm-yes required maps to ConfirmRequired', () {
    final err = BtcError.fromStderr(
      'error: mainnet send requires --confirm-yes yes',
      exitCode: 2,
    );
    expect(err.kind, BtcErrorKind.confirmRequired);
  });

  test('toString() omits stderr (forces explicit BtcLogFilter path)', () {
    final err = BtcError.fromStderr(
      'error: signing failed for abandon abandon abandon abandon '
      'abandon abandon abandon abandon abandon abandon abandon about',
      exitCode: 2,
    );
    // Stderr is deliberately omitted; toString references the redaction
    // chokepoint instead of leaking the secret-bearing string.
    expect(err.toString(), isNot(contains('abandon')));
    expect(err.toString(), contains('BtcLogFilter'));
  });

  test('toString() omits --password flag value', () {
    final err = BtcError.fromStderr(
      'error: bad password hunter2 in command --password hunter2',
      exitCode: 2,
    );
    expect(err.toString(), isNot(contains('hunter2')));
  });
}
