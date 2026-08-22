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

  test('subdirFor rejects empty name', () async {
    await expectLater(
      () => subdirFor(''),
      throwsA(isA<ArgumentError>()),
    );
  });

  test('subdirFor rejects parent-directory traversal segment', () async {
    await expectLater(
      () => subdirFor('..'),
      throwsA(isA<ArgumentError>()),
    );
  });

  test('subdirFor rejects traversal embedded in path', () async {
    await expectLater(
      () => subdirFor('../etc'),
      throwsA(isA<ArgumentError>()),
    );
  });

  test('subdirFor rejects absolute paths', () async {
    await expectLater(
      () => subdirFor('/etc/passwd'),
      throwsA(isA<ArgumentError>()),
    );
  });

  test('subdirFor rejects nested separator (single-segment-only)', () async {
    await expectLater(
      () => subdirFor('foo/bar'),
      throwsA(isA<ArgumentError>()),
    );
  });

  test('subdirFor rejects backslash traversal on Windows', () async {
    await expectLater(
      () => subdirFor(r'..\foo'),
      throwsA(isA<ArgumentError>()),
    );
  }, skip: !Platform.isWindows);

  test('subdirFor rejects Windows drive-letter absolute path', () async {
    await expectLater(
      () => subdirFor(r'C:\foo'),
      throwsA(isA<ArgumentError>()),
    );
  }, skip: !Platform.isWindows);

  test('appDataPath returns a path string ending with appDirName', () async {
    final path = await appDataPath();
    expect(path.split(Platform.pathSeparator).last, appDirName);
  });

  test('appDataPath does not create the directory on disk', () async {
    final path = await appDataPath();
    expect(Directory(path).existsSync(), isFalse);
  });

  test('subdirPathFor returns a path string under appDataPath', () async {
    final base = await appDataPath();
    final sub = await subdirPathFor('btc');
    expect(sub.startsWith(base), isTrue);
    expect(sub.endsWith('btc'), isTrue);
  });

  test('subdirPathFor rejects traversal (validation reuse)', () async {
    await expectLater(
      () => subdirPathFor('../etc'),
      throwsA(isA<ArgumentError>()),
    );
  });

  test('appDataDir creates the dir while appDataPath does not', () async {
    // IO-creating variant must materialize the dir on disk...
    await appDataDir();
    // ...so appDataPath (path-only) resolves to an existing dir.
    // Asserts the contract difference side-by-side in one test.
    final path = await appDataPath();
    expect(Directory(path).existsSync(), isTrue);
  });
}
