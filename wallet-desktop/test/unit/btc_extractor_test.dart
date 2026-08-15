import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:path_provider_platform_interface/path_provider_platform_interface.dart';
import 'package:wallet_desktop/core/binary/btc_extractor.dart';

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

  test('hostTarget returns a platform-appropriate asset path', () {
    final target = hostTarget();
    if (Platform.isLinux) {
      expect(target.arch, anyOf('linux-x64', 'linux-arm64'));
      expect(target.assetPath, startsWith('assets/btc/linux-'));
      expect(target.binaryName, 'btc');
    } else if (Platform.isMacOS) {
      expect(target.arch, anyOf('macos-x64', 'macos-arm64'));
      expect(target.assetPath, startsWith('assets/btc/macos-'));
      expect(target.binaryName, 'btc');
    } else if (Platform.isWindows) {
      expect(target.arch, 'windows-x64');
      expect(target.assetPath, startsWith('assets/btc/windows-'));
      expect(target.binaryName, 'btc.exe');
    } else {
      fail('Unsupported platform: ${Platform.operatingSystem}');
    }
  });

  test('extractBtc throws ExtractionException when asset is missing', () async {
    // rootBundle has no registered asset on unit tests → MissingPluginException
    // surfaces before the empty-bytes check. Both ExtractionException and
    // MissingPluginException are acceptable here; the former when the asset
    // is registered-but-empty (host CI stub), the latter when it's not
    // registered at all (default unit-test context).
    try {
      await extractBtc();
      fail('extractBtc should have thrown on missing/empty bundled asset');
    } on ExtractionException {
      // expected — empty-bytes guard tripped
    } on MissingPluginException {
      // expected in default unit-test context (asset not registered)
    } catch (e) {
      fail('extractBtc threw unexpected exception type: $e');
    }
  });
}
