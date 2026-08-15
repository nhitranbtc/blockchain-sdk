import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:path_provider_platform_interface/path_provider_platform_interface.dart';
import 'package:wallet_desktop/core/paths.dart';

/// Fake `PathProviderPlatform` for unit tests. Only `getApplicationSupportPath`
/// is wired to [basePath]; every other method throws so future bugs surface
/// as a clear `UnimplementedError` instead of a platform-channel surprise.
class _FakePathProvider extends PathProviderPlatform {
  _FakePathProvider(this.basePath);

  final String basePath;

  @override
  Future<String?> getApplicationSupportPath() async => basePath;

  @override
  Future<String?> getApplicationDocumentsPath() async =>
      throw UnimplementedError(
          'Test fake: getApplicationDocumentsPath not configured');

  @override
  Future<String?> getTemporaryPath() async =>
      throw UnimplementedError('Test fake: getTemporaryPath not configured');

  @override
  Future<String?> getDownloadsPath() async =>
      throw UnimplementedError('Test fake: getDownloadsPath not configured');

  @override
  Future<String?> getLibraryPath() async =>
      throw UnimplementedError('Test fake: getLibraryPath not configured');

  @override
  Future<List<String>?> getExternalStoragePaths(
          {StorageDirectory? type}) async =>
      throw UnimplementedError(
          'Test fake: getExternalStoragePaths not configured');

  @override
  Future<String?> getExternalStoragePath() async => throw UnimplementedError(
      'Test fake: getExternalStoragePath not configured');

  @override
  Future<List<String>?> getExternalCachePaths() async =>
      throw UnimplementedError(
          'Test fake: getExternalCachePaths not configured');
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late Directory tmp;

  setUp(() {
    tmp = Directory.systemTemp.createTempSync('wallet_desktop_test_');
    PathProviderPlatform.instance = _FakePathProvider(tmp.path);
    addTearDown(() {
      if (tmp.existsSync()) tmp.deleteSync(recursive: true);
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
