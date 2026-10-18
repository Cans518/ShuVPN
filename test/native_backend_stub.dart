import 'package:shuvpn/core/auth/account_authentication.dart';
import 'package:shuvpn/core/connection/native_connection.dart';
import 'package:shuvpn/core/connection/native_tun.dart';

const noNativeServices = NativeProxyPreferences(
  httpEnabled: false,
  httpAddress: '',
  httpPort: 0,
  socksEnabled: false,
  socksAddress: '',
  socksPort: 0,
  systemProxyEnabled: false,
);

class StubNativeBackend implements NativeConnectionBackend {
  StubNativeBackend({this.failure = 'connection not requested'});
  final String failure;
  @override
  NativeLifecycleSnapshot get lifecycle => const NativeLifecycleSnapshot();
  @override
  Stream<NativeLifecycleSnapshot> get lifecycleUpdates => const Stream.empty();
  @override
  Future<NativeConnection> connect({
    required AccountAuthentication auth,
    required String server,
    required Duration timeout,
    required NativeProxyPreferences proxies,
    NativeTunStartup? tun,
  }) async => throw StateError(failure);
  @override
  void cancel() {}
  @override
  Future<void> disconnect() async {}
  @override
  Future<void> shutdown() async {}
  @override
  void dispose() {}
}
