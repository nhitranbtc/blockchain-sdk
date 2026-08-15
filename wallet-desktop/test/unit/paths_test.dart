import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path_provider_platform_interface/path_provider_platform_interface.dart';
import 'package:wallet_desktop/core/paths.dart';

import '../helpers/fake_path_provider.dart';

class _FakePathProvider extends ThrowingPathProvider {
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

  test('appDataDir returns an existing directory named appDirName', () async {
    final dir = await appDataDir();
    expect(dir.path.split(Platform.pathSeparator).last, appDirName);
    expect(await Directory(dir.path).exists(), isTrue);
  });

  test('subdirFor returns an existing subdirectory under appDataDir', () async {
    final base = await appDataDir();
    final sub = await subdirFor('tmp');
    expect(sub.path.startsWith(base.path), isTrue);
    expect(sub.path.endsWith('tmp'), isTrue);
    expect(await Directory(sub.path).exists(), isTrue);
  });

  test('appDataDir is idempotent across repeated calls', () async {
    final a = await appDataDir();
    final b = await appDataDir();
    expect(a.path, b.path);
  });
}
