import '../auth/account_authentication.dart';
import 'connection_observation.dart';
import 'native_tun.dart';

enum NativeLifecyclePhase {
  disconnected,
  connecting,
  connected,
  disconnecting,
  failed,
}

class NativeLifecycleSnapshot {
  const NativeLifecycleSnapshot({
    this.phase = NativeLifecyclePhase.disconnected,
    this.generation = 0,
    this.runId,
    this.cleanupPending = false,
    this.errorMessage,
    this.errorCode,
    this.closed = false,
  });
  final NativeLifecyclePhase phase;
  final int generation;
  final int? runId;
  final bool cleanupPending;
  final String? errorMessage;
  final String? errorCode;
  final bool closed;
  bool get busy =>
      phase == NativeLifecyclePhase.connecting ||
      phase == NativeLifecyclePhase.disconnecting;
}

abstract interface class NativeConnectionBackend {
  NativeLifecycleSnapshot get lifecycle;
  Stream<NativeLifecycleSnapshot> get lifecycleUpdates;
  Future<NativeConnection> connect({
    required AccountAuthentication auth,
    required String server,
    required Duration timeout,
    required NativeProxyPreferences proxies,
    NativeTunStartup? tun,
  });
  void cancel();
  Future<void> disconnect();
  Future<void> shutdown();
  void dispose();
}

enum NativeConnectionState { connected, disconnected, failed }

enum NativeProxyKind { http, socks }

class NativeConnectionUpdate {
  const NativeConnectionUpdate({
    required this.state,
    this.virtualAddress,
    this.dnsServers = const [],
    this.message,
    this.uploadBytes = 0,
    this.downloadBytes = 0,
    this.uploadRate = 0,
    this.downloadRate = 0,
    this.latencyMs,
    this.connections = 0,
    this.unrouted = 0,
    this.tunnelFailures = 0,
  });
  final NativeConnectionState state;
  final String? virtualAddress;
  final List<String> dnsServers;
  final String? message;
  final int uploadBytes;
  final int downloadBytes;
  final double uploadRate;
  final double downloadRate;
  final double? latencyMs;
  final int connections;
  final int unrouted;
  final int tunnelFailures;
}

class NativeProxyBinding {
  const NativeProxyBinding(
    this.address,
    this.port, {
    required this.displayEndpoint,
    this.displayWarning,
  });
  final String address;
  final int port;
  final String displayEndpoint;
  final String? displayWarning;
}

class NativeProxyPreferences {
  const NativeProxyPreferences({
    required this.httpEnabled,
    required this.httpAddress,
    required this.httpPort,
    required this.socksEnabled,
    required this.socksAddress,
    required this.socksPort,
    required this.systemProxyEnabled,
  });
  final bool httpEnabled;
  final String httpAddress;
  final int httpPort;
  final bool socksEnabled;
  final String socksAddress;
  final int socksPort;
  final bool systemProxyEnabled;
}

class NativeServicesResult {
  const NativeServicesResult({
    this.http,
    this.socks,
    this.systemProxyEnabled = false,
    this.systemProxyError,
    this.failures = const [],
    this.tun,
  });
  final NativeProxyBinding? http;
  final NativeProxyBinding? socks;
  final bool systemProxyEnabled;
  final String? systemProxyError;
  final List<String> failures;
  final NativeTunSnapshot? tun;
}

abstract interface class NativeConnection {
  NativeConnectionUpdate get current;
  NativeServicesResult? get initialServices;
  Stream<NativeConnectionUpdate> get updates;

  /// Releases only the presentation handle. The native backend owns resources.
  void disposeView();
  Future<NativeServicesResult> startServices(
    NativeProxyPreferences preferences, {
    NativeTunStartup? tun,
  });
  Future<NativeProxyBinding> startProxy(
    NativeProxyKind kind,
    String address,
    int port,
  );
  Future<NativeServicesResult> rebindProxy(
    NativeProxyKind kind,
    String address,
    int port, {
    required bool systemProxyEnabled,
  });

  /// Native HTTP stop first restores any owned system proxy configuration.
  Future<void> stopProxy(NativeProxyKind kind);

  /// Waits for native services and system configuration cleanup.
  /// Failure retains a retryable owner; callers must keep the handle.
  Future<void> close();
  void cancel();
}

abstract interface class NativeSystemProxyConnection {
  Future<void> enableSystemProxy();
  Future<bool> restoreSystemProxy();
}

abstract interface class NativeConnectionObservations {
  Future<List<ConnectionObservation>> readObservations();
}
