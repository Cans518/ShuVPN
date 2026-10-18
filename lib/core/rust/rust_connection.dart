import 'dart:async';
import 'dart:convert';

import '../auth/account_authentication.dart';
import 'native_account_authentication.dart';
import '../logging/shu_log.dart';
import '../connection/native_connection.dart';
import '../connection/connection_observation.dart';
import '../connection/native_tun.dart';
import 'generated/platform/model.dart' show CaptureMode;
import 'generated/api/vpn.dart' as native;
import 'generated/api/routing.dart' as routing;
import 'generated/api/desktop_connection.dart' as desktop;
import 'generated/connection/owner.dart' as owner_model;
import 'generated/connection/model.dart' as model;
import 'generated/connection/desktop_services.dart' as services;
import 'rust_core.dart';

final class RustConnectionBackend implements NativeConnectionBackend {
  RustConnectionBackend({this.systemProxyDirectory});
  routing.RoutingManager? _routingManager;
  final String? systemProxyDirectory;
  desktop.DesktopConnectionOwner? _owner;
  Future<desktop.DesktopConnectionOwner>? _opening;
  final _updates = StreamController<NativeLifecycleSnapshot>.broadcast();
  StreamSubscription<owner_model.OwnerSnapshot>? _events;
  NativeLifecycleSnapshot _last = const NativeLifecycleSnapshot();
  bool _disposed = false;
  int _inputGeneration = 0;

  Future<void> initialize() async {
    await _ready();
  }

  void bindRoutingManager(routing.RoutingManager manager) {
    if (_routingManager != null) throw StateError('规则所有者已配置');
    _routingManager = manager;
  }

  Future<desktop.DesktopConnectionOwner> _ready() => _opening ??= _open();
  Future<desktop.DesktopConnectionOwner> _open() async {
    try {
      return await _openNative();
    } on model.VpnFailure catch (error) {
      throw StateError(error.message);
    }
  }

  Future<desktop.DesktopConnectionOwner> _openNative() async {
    await RustCore.initialize();
    if (_disposed) throw StateError('连接后端已关闭');
    final owner = await desktop.DesktopConnectionOwner.open();
    if (_disposed) {
      owner.dispose();
      throw StateError('连接后端已关闭');
    }
    try {
      owner.claimFrontend();
    } on Object {
      owner.dispose();
      rethrow;
    }
    _owner = owner;
    await owner.awaitFrontend();
    if (_disposed) {
      if (!owner.isDisposed) {
        try {
          owner.cancel();
        } finally {
          owner.dispose();
        }
      }
      throw StateError('连接后端已关闭');
    }
    _last = _lifecycle(owner.snapshot());
    _events = owner.events().listen(
      (event) {
        if (_disposed) return;
        _last = _lifecycle(event);
        _updates.add(_last);
      },
      onError: (Object error) {
        if (_disposed) return;
        _last = NativeLifecycleSnapshot(
          phase: NativeLifecyclePhase.failed,
          cleanupPending: true,
          errorMessage: '$error',
        );
        _updates.add(_last);
      },
    );
    return owner;
  }

  static NativeLifecycleSnapshot _lifecycle(owner_model.OwnerSnapshot value) =>
      NativeLifecycleSnapshot(
        phase: NativeLifecyclePhase.values.byName(value.phase.name),
        generation: value.generation.toInt(),
        runId: value.runId?.toInt(),
        cleanupPending: value.cleanupPending,
        errorMessage: value.errorMessage,
        errorCode: value.errorCode,
        closed: value.closed,
      );
  @override
  NativeLifecycleSnapshot get lifecycle {
    if (_disposed || _owner == null) return _last;
    try {
      return _last = _lifecycle(_owner!.snapshot());
    } on model.VpnFailure catch (error) {
      return NativeLifecycleSnapshot(
        phase: NativeLifecyclePhase.failed,
        cleanupPending: true,
        errorMessage: error.message,
        errorCode: error.code,
      );
    }
  }

  @override
  Stream<NativeLifecycleSnapshot> get lifecycleUpdates => _updates.stream;

  @override
  Future<NativeConnection> connect({
    required AccountAuthentication auth,
    required String server,
    required Duration timeout,
    required NativeProxyPreferences proxies,
    NativeTunStartup? tun,
  }) async {
    if (_disposed) throw StateError('连接后端已关闭');
    if (auth is! NativeAccountAuthentication) {
      throw StateError('桌面连接需要原生账户，不能使用旧认证会话');
    }
    final manager = _routingManager;
    if (manager == null) throw StateError('桌面连接需要原生规则配置');
    final generation = ++_inputGeneration;
    final owner = await _ready();
    if (_disposed || generation != _inputGeneration) throw StateError('连接已取消');
    try {
      final session = await owner.connectAccount(
        account: auth.nativeAccount,
        gatewayOrigin: server,
        timeoutMs: timeout.inMilliseconds,
        routing: manager,
        startup: desktop.ConnectionStartup(
          proxies: RustConnection.proxyPreferences(proxies),
          tun: tun == null ? null : RustConnection.tunPreferences(tun),
          recoveryDirectory: systemProxyDirectory,
        ),
      );
      try {
        if (_disposed || generation != _inputGeneration) {
          throw StateError('连接已取消');
        }
        for (final warning in session.preparationWarnings()) {
          ShuLog.w(ShuLogTag.atrust, warning);
        }
        final connection = await RustConnection.create(
          session,
          systemProxyDirectory: systemProxyDirectory,
        );
        if (_disposed || generation != _inputGeneration) {
          connection.disposeView();
          throw StateError('连接已取消');
        }
        return connection;
      } on Object {
        try {
          if (!_disposed && generation == _inputGeneration) owner.cancel();
        } finally {
          if (!session.isDisposed) session.dispose();
        }
        rethrow;
      }
    } on model.VpnFailure catch (error) {
      throw StateError(error.message);
    }
  }

  @override
  void cancel() {
    _inputGeneration++;
    if (_disposed) return;
    try {
      _owner?.cancel();
    } on model.VpnFailure catch (error) {
      _last = NativeLifecycleSnapshot(
        phase: NativeLifecyclePhase.failed,
        cleanupPending: true,
        errorMessage: error.message,
      );
      _updates.add(_last);
    }
  }

  @override
  Future<void> disconnect() => _closeOwner(false);
  @override
  Future<void> shutdown() => _closeOwner(true);
  Future<void> _closeOwner(bool terminal) async {
    _inputGeneration++;
    if (_opening == null) return;
    final owner = await _ready();
    try {
      final diagnostic = terminal
          ? await owner.shutdown()
          : await owner.disconnect();
      if (diagnostic != null) ShuLog.w(ShuLogTag.conn, diagnostic);
    } on model.VpnFailure catch (error) {
      throw StateError(error.message);
    }
  }

  @override
  void dispose() {
    if (_disposed) return;
    cancel();
    _disposed = true;
    unawaited(_events?.cancel());
    _owner?.dispose();
    unawaited(_updates.close());
  }
}

final class RustConnection
    implements
        NativeConnection,
        NativeConnectionObservations,
        NativeSystemProxyConnection,
        NativeTunConnection {
  RustConnection._(
    this._session,
    this._current,
    this._systemProxyDirectory,
    this.initialServices,
  );
  final native.VpnSession _session;
  final String? _systemProxyDirectory;
  NativeConnectionUpdate _current;
  Stream<NativeConnectionUpdate>? _updates;
  Future<void>? _closing;
  bool _viewDisposed = false;
  @override
  final NativeServicesResult? initialServices;

  static Future<RustConnection> create(
    native.VpnSession session, {
    String? systemProxyDirectory,
  }) async {
    final snapshot = await session.snapshot();
    final startup = await session.startupServices();
    return RustConnection._(
      session,
      _map(snapshot),
      systemProxyDirectory,
      startup == null ? null : _mapServices(startup),
    );
  }

  @override
  void disposeView() {
    if (_viewDisposed) return;
    _viewDisposed = true;
    _session.dispose();
  }

  @override
  Future<void> enableSystemProxy() async {
    if (_closing != null) throw StateError('连接已断开');
    final directory = _systemProxyDirectory;
    if (directory == null) throw StateError('系统代理恢复目录未配置');
    try {
      await _session.enableSystemProxy(directory: directory);
    } on model.VpnFailure catch (failure) {
      throw StateError(failure.message);
    }
  }

  @override
  Future<bool> restoreSystemProxy() async {
    if (_closing != null) {
      await _closing;
      return false;
    }
    try {
      return await _session.restoreSystemProxy();
    } on model.VpnFailure catch (failure) {
      throw StateError(failure.message);
    }
  }

  @override
  NativeConnectionUpdate get current => _current;

  @override
  Stream<NativeConnectionUpdate> get updates => _updates ??= _session
      .events()
      .map((event) => _current = _map(event))
      .asBroadcastStream();

  @override
  Future<List<ConnectionObservation>> readObservations() async {
    if (_closing != null) throw StateError('连接正在关闭，记录不可读取');
    try {
      final records = jsonDecode(await _session.decisions()) as List<Object?>;
      return List.unmodifiable(
        records.map(
          (record) =>
              ConnectionObservation.decode(record as Map<String, Object?>),
        ),
      );
    } on model.VpnFailure catch (failure) {
      throw StateError(failure.message);
    }
  }

  @override
  Future<NativeServicesResult> startServices(
    NativeProxyPreferences preferences, {
    NativeTunStartup? tun,
  }) async {
    if (_closing != null) throw StateError('连接已断开');
    final report = await _session.startDesktopServices(
      proxies: proxyPreferences(preferences),
      tun: tun == null ? null : tunPreferences(tun),
      systemProxyDirectory: _systemProxyDirectory,
    );
    return _mapServices(report);
  }

  static services.ProxyStartup proxyPreferences(
    NativeProxyPreferences preferences,
  ) => services.ProxyStartup(
    httpEnabled: preferences.httpEnabled,
    httpAddress: preferences.httpAddress,
    httpPort: preferences.httpPort,
    socksEnabled: preferences.socksEnabled,
    socksAddress: preferences.socksAddress,
    socksPort: preferences.socksPort,
    systemProxyEnabled: preferences.systemProxyEnabled,
  );
  static services.TunStartup tunPreferences(NativeTunStartup tun) =>
      services.TunStartup(
        enabled: tun.enabled,
        mtu: tun.mtu,
        physicalInterface: tun.physicalInterface,
        physicalDns: tun.physicalDns,
        captureMode: tun.captureMode,
      );
  static NativeServicesResult _mapServices(
    services.DesktopServicesReport report,
  ) => NativeServicesResult(
    http: report.proxies.http == null ? null : _binding(report.proxies.http!),
    socks: report.proxies.socks == null
        ? null
        : _binding(report.proxies.socks!),
    systemProxyEnabled: report.proxies.systemProxyEnabled,
    systemProxyError:
        report.proxies.systemProxyError ??
        report.failures
            .where((failure) => failure.service == 'systemProxy')
            .firstOrNull
            ?.message,
    failures: report.failures.map((failure) => failure.message).toList(),
    tun: report.tun == null ? null : _mapTunOutcome(report.tun!),
  );

  static NativeProxyBinding _binding(model.ProxyBinding binding) =>
      NativeProxyBinding(
        binding.address,
        binding.port,
        displayEndpoint: binding.displayEndpoint,
        displayWarning: binding.displayWarning,
      );

  @override
  Future<NativeServicesResult> rebindProxy(
    NativeProxyKind kind,
    String address,
    int port, {
    required bool systemProxyEnabled,
  }) async {
    if (_closing != null || _viewDisposed) throw StateError('连接已断开');
    return _mapServices(
      await _session.rebindProxy(
        kind: _kind(kind),
        address: address,
        port: port,
        systemProxyDirectory: _systemProxyDirectory,
        systemProxyEnabled: systemProxyEnabled,
      ),
    );
  }

  @override
  Future<NativeProxyBinding> startProxy(
    NativeProxyKind kind,
    String address,
    int port,
  ) async {
    if (_closing != null) throw StateError('连接已断开');
    final binding = await _session.startProxy(
      kind: _kind(kind),
      address: address,
      port: port,
    );
    return _binding(binding);
  }

  @override
  Future<void> stopProxy(NativeProxyKind kind) async {
    final closing = _closing;
    if (closing != null) {
      await closing;
      return;
    }
    await _session.stopProxy(kind: _kind(kind));
  }

  @override
  Future<NativeTunPreview> previewTun(
    int mtu, {
    String? physicalDns,
    String physicalInterface = '',
    CaptureMode captureMode = CaptureMode.campus,
  }) async {
    if (_closing != null) throw StateError('连接已断开');
    final plan = jsonDecode(
      await _session.desktopTunPlan(
        mtu: mtu,
        physicalDns: physicalDns,
        physicalInterface: physicalInterface,
        captureMode: captureMode,
      ),
    ) as Map<String, dynamic>;
    return NativeTunPreview(
      mtu: plan['mtu'] as int,
      addresses: List<String>.from(plan['addresses'] as List),
      routes: List<String>.from(plan['routes'] as List),
      capturesCampusDns: plan['dns_capture'] == true,
      captureMode: _captureMode(plan['capture_mode']),
      protectedAddresses: List<String>.from(plan['protected_ips'] as List),
      physicalDnsServers: List<String>.from(
        plan['physical_dns_servers'] as List? ?? const [],
      ),
    );
  }

  @override
  Future<NativeTunSnapshot> startTun({
    required int mtu,
    required String physicalInterface,
    String? physicalDns,
    CaptureMode captureMode = CaptureMode.campus,
  }) async {
    if (_closing != null) throw StateError('连接已断开');
    return _mapTunOutcome(
      await _session.startTunService(
        request: services.TunStartup(
          enabled: true,
          mtu: mtu,
          physicalInterface: physicalInterface,
          physicalDns: physicalDns,
          captureMode: captureMode,
        ),
      ),
    );
  }

  @override
  Future<NativeTunSnapshot> stopTun() async {
    final closing = _closing;
    if (closing != null) {
      await closing;
      return const NativeTunSnapshot.stopped();
    }
    return _mapTunOutcome(await _session.stopTunService());
  }

  static NativeTunSnapshot _mapTunOutcome(services.TunOperation outcome) {
    final snapshot = jsonDecode(outcome.statusJson) as Map<String, dynamic>;
    final interface = snapshot['interface'] as Map<String, dynamic>?;
    return NativeTunSnapshot(
      state: snapshot['state'] as String,
      cleanupPending: snapshot['cleanup_pending'] == true,
      name: interface?['name'] as String?,
      physicalInterface: snapshot['physical_interface'] as String?,
      mtu: interface?['mtu'] as int?,
      routes: interface == null
          ? const []
          : List<String>.from(interface['routes'] as List),
      diagnostic: outcome.diagnostic,
      captureMode: _captureMode(snapshot['capture_mode']),
    );
  }

  static CaptureMode _captureMode(Object? value) => switch (value) {
    null || 'campus' => CaptureMode.campus,
    'full' => CaptureMode.full,
    _ => throw StateError('原生捕获模式无效：$value'),
  };

  @override
  Future<void> close() =>
      _closing ??= _close().catchError((Object error, StackTrace stack) {
        _closing = null;
        Error.throwWithStackTrace(error, stack);
      });

  Future<void> _close() async {
    final diagnostic = await _session.finishStop();
    if (diagnostic != null) {
      ShuLog.w(ShuLogTag.conn, '连接已完成清理，保留关闭诊断：$diagnostic');
    }
    disposeView();
  }

  @override
  void cancel() {
    if (_closing == null) _session.cancel();
  }

  static model.ProxyKind _kind(NativeProxyKind kind) => switch (kind) {
    NativeProxyKind.http => model.ProxyKind.http,
    NativeProxyKind.socks => model.ProxyKind.socks,
  };

  static NativeConnectionUpdate _map(model.VpnEvent event) =>
      NativeConnectionUpdate(
        state: switch (event.state) {
          model.VpnState.connected => NativeConnectionState.connected,
          model.VpnState.disconnected => NativeConnectionState.disconnected,
          model.VpnState.failed => NativeConnectionState.failed,
        },
        virtualAddress: event.virtualAddress,
        dnsServers: event.dnsServers,
        message: event.message,
        uploadBytes: event.uploadBytes.toInt(),
        downloadBytes: event.downloadBytes.toInt(),
        uploadRate: event.uploadBps,
        downloadRate: event.downloadBps,
        latencyMs: event.latencyMs,
        connections: event.connections.toInt(),
        unrouted: event.unrouted.toInt(),
        tunnelFailures: event.tunnelFailures.toInt(),
      );
}
