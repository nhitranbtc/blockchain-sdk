import 'package:path_provider_platform_interface/path_provider_platform_interface.dart';

/// Base `PathProviderPlatform` fake for unit tests. Every method throws
/// `UnimplementedError` so any future code that calls a sibling method
/// surfaces a loud error rather than a confusing platform-channel
/// `MissingPluginException`.
///
/// Subclasses override only the methods the test exercises:
/// ```dart
/// class _TestPathProvider extends ThrowingPathProvider {
///   _TestPathProvider(this.basePath);
///   final String basePath;
///   @override
///   Future<String?> getApplicationSupportPath() async => basePath;
/// }
/// ```
abstract class ThrowingPathProvider extends PathProviderPlatform {
  ThrowingPathProvider();

  @override
  Future<String?> getApplicationDocumentsPath() async =>
      throw UnimplementedError(
        'Test fake: getApplicationDocumentsPath not configured',
      );

  @override
  Future<String?> getTemporaryPath() async => throw UnimplementedError(
        'Test fake: getTemporaryPath not configured',
      );

  @override
  Future<String?> getDownloadsPath() async => throw UnimplementedError(
        'Test fake: getDownloadsPath not configured',
      );

  @override
  Future<String?> getLibraryPath() async => throw UnimplementedError(
        'Test fake: getLibraryPath not configured',
      );

  @override
  Future<List<String>?> getExternalStoragePaths(
          {StorageDirectory? type}) async =>
      throw UnimplementedError(
        'Test fake: getExternalStoragePaths not configured',
      );

  @override
  Future<String?> getExternalStoragePath() async => throw UnimplementedError(
        'Test fake: getExternalStoragePath not configured',
      );

  @override
  Future<List<String>?> getExternalCachePaths() async =>
      throw UnimplementedError(
        'Test fake: getExternalCachePaths not configured',
      );
}
