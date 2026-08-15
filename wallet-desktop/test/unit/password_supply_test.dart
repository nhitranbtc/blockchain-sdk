import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path_provider_platform_interface/path_provider_platform_interface.dart';
import 'package:wallet_desktop/core/secrets/password_supply.dart';

class _FakePathProvider extends PathProviderPlatform {
  _FakePathProvider(this.basePath);

  final String basePath;

  @override
  Future<String?> getApplicationSupportPath() async => basePath;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late Directory tmp;
  late PathProviderPlatform originalInstance;

  setUp(() {
    originalInstance = PathProviderPlatform.instance;
    tmp = Directory.systemTemp.createTempSync('wallet_desktop_test_');
    PathProviderPlatform.instance = _FakePathProvider(tmp.path);
    addTearDown(() {
      if (tmp.existsSync()) tmp.deleteSync(recursive: true);
      PathProviderPlatform.instance = originalInstance;
    });
  });

  test('withPasswordFile runs body with a temp file containing the password',
      () async {
    String? seenPath;
    await withPasswordFile('hunter2', (path) async {
      seenPath = path;
      expect(await File(path).readAsString(), 'hunter2');
    });
    expect(seenPath, isNotNull);
    expect(await File(seenPath!).exists(), isFalse,
        reason: 'temp file must be unlinked after callback returns');
  });

  test('withPasswordFile unlinks even when callback throws', () async {
    String? seenPath;
    await expectLater(
      withPasswordFile('hunter2', (path) async {
        seenPath = path;
        throw StateError('boom');
      }),
      throwsA(isA<StateError>()),
    );
    expect(await File(seenPath!).exists(), isFalse);
  });
}
