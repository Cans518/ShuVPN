import '../update/shu_update_client.dart';
import '../update/shu_update_info.dart';
import 'generated/api/update.dart' as native;
import 'generated/update.dart' as model;
import 'rust_core.dart';

/// Sends update commands to the native owner and maps its display result.
final class RustUpdateClient implements ShuUpdateClient {
  RustUpdateClient();

  /// Takes ownership of an existing native channel.
  RustUpdateClient.fromNative(native.UpdateClient client) : _client = client;

  native.UpdateClient? _client;
  Future<native.UpdateClient>? _opening;
  bool _closed = false;

  void _live() {
    if (_closed) throw const ShuUpdateException('更新通道已经关闭');
  }

  Future<native.UpdateClient> _open() {
    _live();
    final client = _client;
    if (client != null) return Future.value(client);
    return _opening ??= () async {
      await RustCore.initialize();
      _live();
      final client = await native.UpdateClient.newInstance();
      if (_closed) {
        _release(client);
        throw const ShuUpdateException('更新通道已经关闭');
      }
      _client = client;
      return client;
    }();
  }

  @override
  Future<ShuUpdateInfo?> checkForUpdate(String currentVersion) async {
    try {
      final client = await _open();
      _live();
      final result = await client.check(currentVersion: currentVersion);
      _live();
      if (result == null) return null;
      return ShuUpdateInfo(
        latestVersion: result.latestVersion,
        downloadUrl: result.downloadUrl,
        releasePageUrl: result.releasePageUrl,
        targetUrl: result.targetUrl,
        publishedAt: result.publishedAt == null
            ? null
            : DateTime.tryParse(result.publishedAt!),
      );
    } on model.UpdateFailure catch (error) {
      throw ShuUpdateException(error.message);
    }
  }

  @override
  void close() {
    if (_closed) return;
    _closed = true;
    final client = _client;
    _client = null;
    if (client != null) _release(client);
  }

  void _release(native.UpdateClient client) {
    try {
      client.close();
    } finally {
      client.dispose();
    }
  }
}
