import 'dart:async';

import 'package:flutter/foundation.dart';

import '../auth/account_authentication.dart';
import '../logging/shu_log.dart';
import '../settings/settings_store.dart';
import 'connection_observation.dart';
import 'connection_status.dart';
import 'native_connection.dart';
import 'native_tun.dart';
import 'protocol.dart';
import 'proxy_listen.dart';

/// What the button will connect to.
///
/// There is no username or password here on purpose: the aTrust gateway on this
/// campus authenticates through the unified-identity provider, so credentials
/// live in the account layer and never touch the connection draft.
///
/// 三次信息都是**从 [SettingsStore] 带过来的快照**，不是另一份真值：
/// `_onSettingsChanged` 会在设置页改完的同一帧把它们重新灌进来。
/// 这样保留 `draft` 的意义就只剩一个 —— 日志与提示里要说「刚才连的是什么」
/// 时不必把 `SettingsStore` 传得到处都是。
@immutable
class ConnectionDraft {
  const ConnectionDraft({
    this.protocol = ShuProtocol.atrust,
    this.server = '',
    this.loginDomain = '',
  });

  /// 界面中选中的协议；可用性由原生设置快照提供。
  final ShuProtocol protocol;

  final String server;
  final String loginDomain;

  ConnectionDraft copyWith({
    ShuProtocol? protocol,
    String? server,
    String? loginDomain,
  }) => ConnectionDraft(
    protocol: protocol ?? this.protocol,
    server: server ?? this.server,
    loginDomain: loginDomain ?? this.loginDomain,
  );

  /// Nothing to fill in any more beyond a host.
  bool get isComplete => server.trim().isNotEmpty;
}

/// Presents native session updates and forwards desktop commands.
class ConnectionController extends ChangeNotifier {
  ConnectionController(
    this._settings, {
    ConnectionDraft? draft,
    NativeConnectionBackend? nativeConnectionBackend,
    NativeTunPlatform? desktopTunPlatform,
    this.systemProxyAvailable = false,
    this.systemProxyUnavailableReason,
  }) : _backend = nativeConnectionBackend,
       _tunPlatform = desktopTunPlatform,
       _draft = draft ?? _draftFrom(_settings) {
    // 设置页改完协议开关或服务器地址之后，这一层要立刻知道 ——
    // 「关掉当前协议 → 自动切到另一个可用的」就发生在这个回调里。
    _settings.addListener(_onSettingsChanged);
    _lifecycleSubscription = _backend?.lifecycleUpdates.listen(_onLifecycle);
    if (_backend case final backend?) _onLifecycle(backend.lifecycle);
  }

  final SettingsStore _settings;
  final NativeConnectionBackend? _backend;
  StreamSubscription<NativeLifecycleSnapshot>? _lifecycleSubscription;
  final NativeTunPlatform? _tunPlatform;
  NativeTunEnvironment? _tunEnvironment;
  NativeTunSnapshot _nativeTun = const NativeTunSnapshot.stopped();
  NativeTunPreview? _tunPreview;
  String? _desktopTunError;
  String _physicalInterface = '';
  String _physicalDns = '';
  bool _tunInspecting = false;
  bool _tunPreviewLoading = false;
  Future<bool>? _tunInspection;
  final _nativeTunStops = Map<NativeConnection, Future<void>>.identity();
  final _nativeProxyStops =
      Map<NativeConnection, Map<NativeProxyKind, Future<void>>>.identity();
  NativeTunEnvironment? get desktopTunEnvironment => _tunEnvironment;
  NativeTunSnapshot get desktopTunStatus => _nativeTun;
  NativeTunPreview? get desktopTunPreview => _tunPreview;
  String? get desktopTunError => _desktopTunError;
  String get desktopPhysicalInterface => _physicalInterface;
  String get desktopPhysicalDns => _physicalDns;
  bool get desktopTunInspecting => _tunInspecting;
  bool get desktopTunPreviewLoading => _tunPreviewLoading;
  final bool systemProxyAvailable;
  final String? systemProxyUnavailableReason;
  bool get desktopProxySettingsVisible => _backend != null;
  bool get observationsAvailable =>
      !_disposed && _nativeConnection is NativeConnectionObservations;
  int get observationGeneration => _nativeGeneration;

  Future<List<ConnectionObservation>> readObservations() async {
    final connection = _nativeConnection;
    final generation = _nativeGeneration;
    if (_disposed || connection is! NativeConnectionObservations) {
      throw StateError('当前没有可读取记录的原生会话');
    }
    final records = await (connection as NativeConnectionObservations)
        .readObservations();
    if (_disposed ||
        generation != _nativeGeneration ||
        !identical(connection, _nativeConnection)) {
      throw StateError('会话已变化，请重新读取连接记录');
    }
    return records;
  }

  bool _systemProxyRunning = false;
  bool get systemProxyRunning => _systemProxyRunning;
  String? _systemProxyRecoveryError;
  String? get systemProxyRecoveryError => _systemProxyRecoveryError;
  NativeConnection? _nativeConnection;
  NativeConnectionUpdate? _nativeUpdate;
  StreamSubscription<NativeConnectionUpdate>? _nativeSubscription;
  bool _nativeHttpRunning = false;
  bool _nativeSocksRunning = false;
  bool _disposed = false;
  int _nativeGeneration = 0;
  bool _nativeCancelRequested = false;
  String? _nativeTunCleanupError;
  Future<void>? _disconnectTask;
  Future<void>? _exitTask;
  bool _exiting = false;
  String? get cleanupError {
    final current = _backend?.lifecycle;
    if (current != null && current.runId == null && !current.cleanupPending) {
      return null;
    }
    if (current?.cleanupPending == true && current?.errorMessage != null) {
      return current!.errorCode == 'systemProxy'
          ? '系统代理恢复失败：${current.errorMessage}'
          : '连接清理失败：${current.errorMessage}';
    }
    return _systemProxyRecoveryError ?? _nativeTunCleanupError;
  }

  bool get cleanupPending {
    final current = _backend?.lifecycle;
    if (current != null && current.runId == null && !current.cleanupPending) {
      return false;
    }
    return current?.cleanupPending == true ||
        _nativeTunCleanupError != null ||
        _nativeTun.cleanupPending;
  }

  bool get canDisconnect =>
      _state != ConnectionStatus.disconnected ||
      _nativeConnection != null ||
      _backend?.lifecycle.busy == true ||
      cleanupPending;

  /// 从设置里拼出初始草稿。
  static ConnectionDraft _draftFrom(SettingsStore settings) => ConnectionDraft(
    protocol: settings.firstEnabledProtocol ?? ShuProtocol.atrust,
    server: settings.server,
    loginDomain: settings.loginDomain,
  );

  /// 设置变了：把三项快照重新灌进草稿，并把关掉的协议换掉。
  ///
  /// 为什么要在这里处理而不是留给 UI：连接页的协议选择条、设置页的开关、
  /// 球下面那行状态，三处都要说「现在连的是谁」。把这个判断放在一处，
  /// 三者才不会各算一遍、算出三个答案。
  void _onSettingsChanged() {
    var protocol = _draft.protocol;
    if (!_settings.isProtocolEnabled(protocol)) {
      // 一个都没开时 `firstEnabledProtocol` 是 null，那就保留原选择不动 ——
      // 页面上三个选项会全部置灰，保留选中项比清空更能说明「本来选的是它」。
      protocol = _settings.firstEnabledProtocol ?? protocol;
    }
    final next = ConnectionDraft(
      protocol: protocol,
      server: _settings.server,
      loginDomain: _settings.loginDomain,
    );
    if (next.protocol == _draft.protocol &&
        next.server == _draft.server &&
        next.loginDomain == _draft.loginDomain) {
      return;
    }
    _draft = next;
    notifyListeners();
  }

  /// Supplies the live unified-identity session for the aTrust OAuth path.
  ///
  /// Set by the shell from the account center. Returning `null` (or a session
  /// without `SHU_OAUTH2`) is what produces the “先去账号管理登录” error, so the
  /// controller never has to know how the account page is organised.
  AccountAuthentication? Function()? ssoSessionProvider;

  // ------------------------------------------------------------------ draft

  ConnectionDraft _draft;
  ConnectionDraft get draft => _draft;

  /// 当前选中的协议是不是真的可以连。
  ///
  /// 连接页在点球之前看它一眼：为 false 时不再往下走，改为弹一条提示。
  bool get hasEnabledProtocol => _settings.isProtocolEnabled(_draft.protocol);

  /// 用户在选择条上换了一个协议。
  ///
  /// 关掉的协议**静默忽略**（选择条上那一段本来就不可点）；
  /// 这样即使某条路径把一个禁用项递进来，也不会真的切换过去。
  void selectProtocol(ShuProtocol protocol) {
    if (!_settings.isProtocolEnabled(protocol)) return;
    if (protocol == _draft.protocol) return;
    _draft = _draft.copyWith(protocol: protocol);
    notifyListeners();
  }

  // ------------------------------------------------------------- live state

  ConnectionStatus _state = ConnectionStatus.disconnected;
  ConnectionStatus get state => _state;

  bool _busy = false;

  /// True while an operation is in flight; disables the orb and the settings
  /// that would race it.
  bool get busy =>
      _busy ||
      _exiting ||
      _disconnectTask != null ||
      _exitTask != null ||
      _backend?.lifecycle.busy == true;

  ConnectionErrorCode? _errorCode;
  String? _errorMessage;

  ConnectionErrorCode? get errorCode => _errorCode;

  /// 失败原因，**直接给用户看**。
  ///
  /// 界面只拿这一个字符串，不把 `ConnectionErrorCode` 拼进去：`certificateMismatch:
  /// 证书指纹不匹配` 这种写法是给人翻日志的，不是给人读的。枚举名在日志里
  /// （`ShuLog.e('conn', '[$code] $message')`），而日志在设置 → 日志里能翻到。
  String? get errorMessage => _errorMessage;

  String? _virtualAddress;
  String? get virtualAddress => _virtualAddress;

  List<String> _dnsServers = const <String>[];
  List<String> get dnsServers => _dnsServers;

  DateTime? _connectedAt;
  DateTime? get connectedAt => _connectedAt;

  bool get tunnelUp =>
      _nativeConnection != null &&
      _nativeUpdate?.state == NativeConnectionState.connected &&
      _backend?.lifecycle.phase == NativeLifecyclePhase.connected;

  /// 本机 SOCKS5 代理是否在跑。
  bool get socksProxyRunning => _nativeSocksRunning && tunnelUp;

  /// 本机 HTTP 代理是否在跑。
  bool get httpProxyRunning => _nativeHttpRunning && tunnelUp;

  /// 两个本机代理里只要有任意一个在跑就是 true。
  ///
  /// 界面用它决定「复制地址」这类行要不要置灰 —— 两个通道的服务对象不同，
  /// 只跑其中一个也是正常状态。
  bool get proxyRunning => socksProxyRunning || httpProxyRunning;

  /// 当前桌面 TUN 是否正在运行。
  bool get vpnRunning => _nativeTun.running && tunnelUp;

  bool get vpnSupported =>
      _backend != null && _tunEnvironment?.available == true;

  int? _boundSocksPort;

  /// 代理**实际绑定**的那个地址；没在跑时为 `null`。
  ///
  /// 它和 [_boundSocksPort] 是同一件事的两半，也为了同一个理由存在：设置
  /// 可能在代理跑着的时候被改掉，而**绑定已经发生了** —— 这时候界面该说
  /// 的是「现在实际绑在哪」，不是「设置里写着什么」。之前这两个值都直接
  /// 取自设置（地址甚至是写死的 `127.0.0.1`），于是改成监听所有网卡之后
  /// 页面上依然显示 `127.0.0.1`，抄出去的那个地址根本连不上。
  ShuProxyListen? _boundSocksListen;

  String? _socksDisplayEndpoint;
  String? _httpDisplayEndpoint;

  /// The port actually served right now, falling back to the configured one.
  int get socksPort => _boundSocksPort ?? _settings.socksPort;

  /// 现在实际监听在哪。代理没在跑时就是设置里那一个 —— 界面在两种状态下
  /// 都要有东西可显示。
  ShuProxyListen get socksListen => _boundSocksListen ?? _settings.socksListen;

  /// 需要抄给别的应用 / 别的设备的那一串 `地址:端口`。
  ///
  /// 两处都不再写死：地址跟着 [socksListen] 走，端口跟着真正绑上的那个走。
  String get proxyAddress =>
      _socksDisplayEndpoint ?? _endpoint(socksListen, socksPort);

  int? _boundHttpPort;

  /// HTTP 通道实际绑定的监听地址。
  ///
  /// 与 [_boundSocksListen] 分开存：两个通道各有一个监听地址设置，
  /// 「现在到底绑在哪」必须按各自的那一份回答。
  ShuProxyListen? _boundHttpListen;

  /// HTTP 通道现在实际监听的端口；没在跑时为 `null`。
  ///
  /// 它和 [_boundSocksPort] 是同一件事的两半：端口被占时会退到系统随便给
  /// 一个，而界面该显示的是真的在听的那一个。
  int? get httpPort => _boundHttpPort;

  /// 现在实际监听的 HTTP 地址。代理没在跑时就是设置里那一个。
  ShuProxyListen get httpListen => _boundHttpListen ?? _settings.httpListen;

  /// 给用户看的 HTTP 地址；没在跑时为 `null`。
  String? get httpProxyAddress {
    final port = _boundHttpPort;
    final listen = _boundHttpListen;
    if (port == null || listen == null) return null;
    return _httpDisplayEndpoint ?? _endpoint(listen, port);
  }

  /// 与 [proxyAddress] 同形的那一串，给 HTTP 通道用。
  ///
  /// 与 [httpProxyAddress] 的分工：那个**只在真的在听时非空**（只能填真实
  /// 绑定的端口），而这一条任何时候都有值 —— 抽屉里那一行讲的是「这一路
  /// 怎么连」，还没连上时照着设置里那个值写。
  String get httpListenAddress =>
      _httpDisplayEndpoint ??
      _endpoint(httpListen, _boundHttpPort ?? _settings.httpPort);

  String _endpoint(ShuProxyListen listen, int port) =>
      '${listen.address.contains(':') ? '[${listen.address}]' : listen.address}:$port';

  int get proxyUnroutedCount => _nativeUpdate?.unrouted ?? 0;
  int get proxyTunnelFailureCount => _nativeUpdate?.tunnelFailures ?? 0;
  double get uploadBytesPerSecond => _nativeUpdate?.uploadRate ?? 0;
  double get downloadBytesPerSecond => _nativeUpdate?.downloadRate ?? 0;
  double? get latencyMs => _nativeUpdate?.latencyMs;
  int get trafficUpBytes => _nativeUpdate?.uploadBytes ?? 0;
  int get trafficDownBytes => _nativeUpdate?.downloadBytes ?? 0;
  int get trafficConnections => _nativeUpdate?.connections ?? 0;

  List<String> get vpnTunnelRoutes => _nativeTun.routes;

  int get vpnRouteCount => vpnTunnelRoutes.length;

  SettingsStore get settings => _settings;

  @override
  void dispose() {
    _disposed = true;
    _nativeGeneration++;
    _backend?.cancel();
    unawaited(_nativeSubscription?.cancel());
    unawaited(_lifecycleSubscription?.cancel());
    _nativeConnection?.disposeView();
    _settings.removeListener(_onSettingsChanged);
    _boundHttpPort = null;
    _boundHttpListen = null;
    super.dispose();
  }

  void _fail(ConnectionErrorCode code, String message) {
    _errorCode = code;
    _errorMessage = message;
    _state = ConnectionStatus.error;
    _busy = false;
    ShuLog.e(ShuLogTag.conn, '[$code] $message');
    notifyListeners();
  }

  void _clearError() {
    _errorCode = null;
    _errorMessage = null;
  }

  // --------------------------------------------------------------- commands

  Future<bool> _connectNative(NativeConnectionBackend backend) async {
    _nativeCancelRequested = false;
    _busy = true;
    _clearError();
    _state = ConnectionStatus.connecting;
    final generation = ++_nativeGeneration;
    notifyListeners();
    var succeeded = false;
    try {
      final auth = ssoSessionProvider?.call();
      if (auth == null || !auth.hasSession) {
        throw const _ConnectionValidationException(
          ConnectionErrorCode.authenticationFailed,
          '请先在账号管理中登录校园账户',
        );
      }
      await _releaseView();
      if (_disposed ||
          _exiting ||
          generation != _nativeGeneration ||
          _nativeCancelRequested) {
        if (!_disposed && generation == _nativeGeneration) {
          _state = ConnectionStatus.disconnected;
        }
        return false;
      }
      final connection = await backend.connect(
        auth: auth,
        server: _draft.server,
        timeout: _settings.timeout,
        proxies: _proxyPreferences,
        tun: NativeTunStartup(
          enabled: _settings.vpnEnabled,
          mtu: _settings.vpnMtu,
          physicalInterface: _physicalInterface,
          physicalDns: _physicalDns.isEmpty ? null : _physicalDns,
          captureMode: _settings.vpnCaptureMode,
        ),
      );
      if (connection.current.state != NativeConnectionState.connected) {
        connection.disposeView();
        throw StateError(connection.current.message ?? 'Rust 通道已断开');
      }
      if (_disposed ||
          generation != _nativeGeneration ||
          _nativeCancelRequested) {
        connection.disposeView();
        if (!_disposed &&
            generation == _nativeGeneration &&
            _nativeCancelRequested) {
          _state = ConnectionStatus.disconnected;
        }
        return false;
      }
      _nativeConnection = connection;
      _systemProxyRecoveryError = null;
      _nativeUpdate = connection.current;
      _virtualAddress = connection.current.virtualAddress;
      _dnsServers = List.unmodifiable(connection.current.dnsServers);
      _connectedAt = DateTime.now();
      _state = ConnectionStatus.connected;
      _nativeSubscription = connection.updates.listen(
        (update) => _onNativeUpdate(connection, update),
        onError: (Object error) {
          if (!_disposed && identical(_nativeConnection, connection)) {
            _fail(ConnectionErrorCode.tunnelFailed, '$error');
          }
        },
      );
      succeeded = true;
      ShuLog.i(ShuLogTag.conn, 'Rust 隧道已建立 · 虚拟地址 $_virtualAddress');
      if (connection.initialServices case final services?) {
        _showServices(services);
      }
    } on Object catch (error) {
      if (_disposed || generation != _nativeGeneration) return false;
      if (!_disposed) {
        if (cleanupError case final cleanup?) {
          _fail(ConnectionErrorCode.tunnelFailed, cleanup);
        } else if (_nativeCancelRequested) {
          _busy = false;
          _clearError();
          _state = ConnectionStatus.disconnected;
          notifyListeners();
        } else {
          _fail(
            error is _ConnectionValidationException
                ? error.code
                : ConnectionErrorCode.tunnelFailed,
            error is _ConnectionValidationException ? error.message : '$error',
          );
        }
      }
    } finally {
      if (!_disposed && generation == _nativeGeneration) {
        _busy = false;
        notifyListeners();
      }
    }
    return succeeded &&
        !_nativeCancelRequested &&
        !_disposed &&
        generation == _nativeGeneration &&
        tunnelUp;
  }

  void _onLifecycle(NativeLifecycleSnapshot snapshot) {
    if (_disposed) return;
    if (snapshot.generation <
        (_backend?.lifecycle.generation ?? snapshot.generation)) {
      return;
    }
    if (snapshot.phase == NativeLifecyclePhase.failed &&
        snapshot.cleanupPending &&
        snapshot.errorMessage != null) {
      _errorMessage = snapshot.errorMessage;
      _errorCode = ConnectionErrorCode.tunnelFailed;
      _state = ConnectionStatus.error;
      if (snapshot.errorCode == 'systemProxy') {
        _systemProxyRecoveryError = '系统代理恢复失败：${snapshot.errorMessage}';
      }
    }
    if (snapshot.runId == null && !snapshot.cleanupPending) {
      _clearRuntimePresentation();
      if (!snapshot.busy && !_busy) {
        _state = snapshot.phase == NativeLifecyclePhase.failed
            ? ConnectionStatus.error
            : ConnectionStatus.disconnected;
        _errorMessage = snapshot.errorMessage;
        _errorCode = snapshot.errorMessage == null
            ? null
            : ConnectionErrorCode.tunnelFailed;
      }
    }
    notifyListeners();
  }

  void _onNativeUpdate(
    NativeConnection connection,
    NativeConnectionUpdate update,
  ) {
    if (_disposed ||
        _disconnectTask != null ||
        _exitTask != null ||
        !identical(_nativeConnection, connection)) {
      return;
    }
    _nativeUpdate = update;
    _virtualAddress = update.virtualAddress;
    _dnsServers = List.unmodifiable(update.dnsServers);
    if (update.state != NativeConnectionState.connected) {
      _nativeTun = const NativeTunSnapshot.stopped();
      _tunPreview = null;
      if (update.message?.contains('系统代理') == true) {
        _systemProxyRecoveryError = update.message;
      }
      _systemProxyRunning = false;
      _nativeHttpRunning = false;
      _nativeSocksRunning = false;
      _boundHttpPort = null;
      _boundSocksPort = null;
      _boundHttpListen = null;
      _boundSocksListen = null;
      _httpDisplayEndpoint = null;
      _socksDisplayEndpoint = null;
      _state = update.state == NativeConnectionState.failed
          ? ConnectionStatus.error
          : ConnectionStatus.disconnected;
      _errorMessage = update.message;
      _errorCode = update.state == NativeConnectionState.failed
          ? ConnectionErrorCode.tunnelFailed
          : null;
    }
    notifyListeners();
  }

  Future<bool> _startNativeProxy(NativeProxyKind kind) async {
    final connection = _nativeConnection;
    final generation = _nativeGeneration;
    if (connection == null || !tunnelUp || _nativeCancelRequested) return false;
    await _nativeProxyStops[connection]?[kind];
    if (_disposed ||
        generation != _nativeGeneration ||
        !identical(connection, _nativeConnection) ||
        !tunnelUp ||
        _nativeCancelRequested) {
      return false;
    }
    if (kind == NativeProxyKind.http
        ? _nativeHttpRunning
        : _nativeSocksRunning) {
      return true;
    }
    _busy = true;
    _clearError();
    notifyListeners();
    try {
      final listen = kind == NativeProxyKind.http
          ? _settings.httpListen
          : _settings.socksListen;
      final port = kind == NativeProxyKind.http
          ? _settings.httpPort
          : _settings.socksPort;
      final binding = await connection.startProxy(kind, listen.address, port);
      if (_disposed ||
          _nativeCancelRequested ||
          !identical(connection, _nativeConnection) ||
          !tunnelUp) {
        return false;
      }
      if (kind == NativeProxyKind.http) {
        _nativeHttpRunning = true;
        _boundHttpPort = binding.port;
        _boundHttpListen = ShuProxyListen.fromBackend(binding.address);
        _httpDisplayEndpoint = binding.displayEndpoint;
      } else {
        _nativeSocksRunning = true;
        _boundSocksPort = binding.port;
        _boundSocksListen = ShuProxyListen.fromBackend(binding.address);
        _socksDisplayEndpoint = binding.displayEndpoint;
      }
      if (binding.displayWarning case final warning?) {
        ShuLog.w(ShuLogTag.proxy, warning);
      }
      return true;
    } on Object catch (error) {
      if (!_disposed &&
          generation == _nativeGeneration &&
          identical(connection, _nativeConnection) &&
          tunnelUp) {
        _fail(ConnectionErrorCode.tunnelFailed, '启动代理失败：$error');
      }
      return false;
    } finally {
      if (!_disposed && generation == _nativeGeneration) {
        _busy = false;
        notifyListeners();
      }
    }
  }

  /// Sends a connection command only to the configured native backend.
  Future<bool> connect() async {
    if (busy || _disconnectTask != null || _exitTask != null) return false;
    if (!hasEnabledProtocol) {
      _fail(ConnectionErrorCode.unsupported, '无可用协议，请先在设置中启用一个');
      return false;
    }
    if (_draft.protocol != ShuProtocol.atrust) {
      _fail(
        ConnectionErrorCode.unsupported,
        '桌面原生核心尚未支持 ${_draft.protocol.label}',
      );
      return false;
    }
    final backend = _backend;
    if (backend == null) {
      _fail(ConnectionErrorCode.unsupported, '桌面原生核心未配置，请重新启动完整应用');
      return false;
    }
    return _connectNative(backend);
  }

  void cancel() {
    if (!_busy) return;
    _nativeCancelRequested = true;
    _backend?.cancel();
    ShuLog.i(ShuLogTag.conn, '已请求取消连接');
  }

  // --------------------------------------------------------------- 本机代理

  Future<bool> startProxy() => _startServices();

  NativeProxyPreferences get _proxyPreferences => NativeProxyPreferences(
    httpEnabled: _settings.httpProxyEnabled,
    httpAddress: _settings.httpListen.address,
    httpPort: _settings.httpPort,
    socksEnabled: _settings.socksProxyEnabled,
    socksAddress: _settings.socksListen.address,
    socksPort: _settings.socksPort,
    systemProxyEnabled: _settings.systemProxyEnabled,
  );

  Future<bool> _startServices({NativeTunStartup? tun}) async {
    final connection = _nativeConnection;
    final generation = _nativeGeneration;
    if (connection == null || !tunnelUp || _nativeCancelRequested) return false;
    _busy = true;
    notifyListeners();
    try {
      final result = await connection.startServices(
        _proxyPreferences,
        tun: tun,
      );
      if (_disposed ||
          generation != _nativeGeneration ||
          !identical(connection, _nativeConnection) ||
          _nativeCancelRequested ||
          !tunnelUp) {
        return false;
      }
      _showServices(result);
      return result.failures.isEmpty &&
          result.systemProxyError == null &&
          (result.tun == null || result.tun!.running);
    } on Object catch (error) {
      if (!_disposed &&
          generation == _nativeGeneration &&
          !_nativeCancelRequested &&
          tunnelUp) {
        _errorMessage = '启动桌面服务失败：$error';
        _errorCode = ConnectionErrorCode.tunnelFailed;
        _state = ConnectionStatus.error;
      }
      return false;
    } finally {
      if (!_disposed && generation == _nativeGeneration) {
        _busy = false;
        notifyListeners();
      }
    }
  }

  void _showServices(NativeServicesResult result) {
    _nativeHttpRunning = result.http != null;
    _nativeSocksRunning = result.socks != null;
    _boundHttpPort = result.http?.port;
    _boundSocksPort = result.socks?.port;
    _boundHttpListen = result.http == null
        ? null
        : ShuProxyListen.fromBackend(result.http!.address);
    _boundSocksListen = result.socks == null
        ? null
        : ShuProxyListen.fromBackend(result.socks!.address);
    _httpDisplayEndpoint = result.http?.displayEndpoint;
    _socksDisplayEndpoint = result.socks?.displayEndpoint;
    for (final binding in [result.http, result.socks]) {
      if (binding?.displayWarning case final warning?) {
        ShuLog.w(ShuLogTag.proxy, warning);
      }
    }
    _systemProxyRunning = result.systemProxyEnabled;
    _systemProxyRecoveryError = result.systemProxyError;
    final errors = {...result.failures, ?result.systemProxyError}.toList();
    _errorMessage = errors.isEmpty ? null : errors.join('；');
    _errorCode = errors.isEmpty ? null : ConnectionErrorCode.tunnelFailed;
    if (result.tun case final snapshot?) {
      _nativeTun = snapshot;
      _desktopTunError = snapshot.diagnostic;
      _nativeTunCleanupError = snapshot.cleanupPending
          ? snapshot.diagnostic ?? 'VPN 清理尚未完成'
          : null;
    }
    if (errors.isNotEmpty) {
      _state = ConnectionStatus.error;
      ShuLog.w(ShuLogTag.conn, _errorMessage!);
    } else if (!cleanupPending) {
      _state = ConnectionStatus.connected;
    }
  }

  Future<bool> startSystemProxy() async {
    final connection = _nativeConnection;
    if (!systemProxyAvailable ||
        connection is! NativeSystemProxyConnection ||
        !tunnelUp ||
        !httpProxyRunning) {
      _fail(
        ConnectionErrorCode.unsupported,
        systemProxyUnavailableReason ?? '系统代理需要已连接的桌面 HTTP 代理',
      );
      return false;
    }
    final generation = _nativeGeneration;
    _busy = true;
    notifyListeners();
    try {
      await (connection as NativeSystemProxyConnection).enableSystemProxy();
      if (_disposed ||
          _nativeCancelRequested ||
          generation != _nativeGeneration ||
          !identical(_nativeConnection, connection) ||
          !tunnelUp) {
        return false;
      }
      _systemProxyRunning = true;
      ShuLog.i(ShuLogTag.proxy, '系统代理已启用');
      return true;
    } on Object catch (error) {
      if (!_disposed && generation == _nativeGeneration) {
        _systemProxyRunning = false;
        _errorMessage = '启用系统代理失败：$error';
        _errorCode = ConnectionErrorCode.tunnelFailed;
        ShuLog.w(ShuLogTag.proxy, _errorMessage!);
      }
      return false;
    } finally {
      if (!_disposed && generation == _nativeGeneration) {
        _busy = false;
        notifyListeners();
      }
    }
  }

  Future<void> stopSystemProxy() async {
    final connection = _nativeConnection;
    try {
      if (connection is NativeSystemProxyConnection) {
        final changed = await (connection as NativeSystemProxyConnection)
            .restoreSystemProxy();
        if (changed) {
          ShuLog.w(ShuLogTag.proxy, '系统代理已被其他应用更改，保留外部配置');
        }
      }
      _systemProxyRecoveryError = null;
    } on Object catch (error) {
      _systemProxyRecoveryError = '系统代理恢复失败：$error';
      rethrow;
    }
    _systemProxyRunning = false;
    if (!_disposed) notifyListeners();
  }

  Future<bool> startHttpProxy() async {
    if (_nativeConnection == null) {
      _fail(ConnectionErrorCode.invalidOptions, '尚未建立隧道，无法启动 HTTP 代理');
      return false;
    }
    return _startNativeProxy(NativeProxyKind.http);
  }

  Future<bool> startSocksProxy() async {
    if (_nativeConnection == null) {
      _fail(ConnectionErrorCode.invalidOptions, '尚未建立隧道，无法启动 SOCKS5');
      return false;
    }
    return _startNativeProxy(NativeProxyKind.socks);
  }

  /// 只停 HTTP 那一半。
  Future<void> stopHttpProxy() async {
    if (!httpProxyRunning) return;
    await _stopHttpProxy();
    if (!_disposed && !_busy) notifyListeners();
  }

  /// 只停 SOCKS5 那一半。
  Future<void> stopSocksProxy() async {
    if (!socksProxyRunning) return;
    await _stopSocksProxy();
    if (!_disposed && !_busy) notifyListeners();
  }

  Future<void> _stopHttpProxy() => _stopNativeProxy(NativeProxyKind.http);
  Future<void> _stopSocksProxy() => _stopNativeProxy(NativeProxyKind.socks);

  Future<void> _stopNativeProxy(NativeProxyKind kind) {
    final connection = _nativeConnection;
    if (connection == null) return Future<void>.value();
    final pending = _nativeProxyStops.putIfAbsent(connection, () => {});
    final existing = pending[kind];
    if (existing != null) return existing;
    final operation = _stopNativeProxyOperation(
      connection,
      kind,
      _nativeGeneration,
    );
    pending[kind] = operation;
    return operation.whenComplete(() {
      if (identical(pending[kind], operation)) pending.remove(kind);
      if (pending.isEmpty) _nativeProxyStops.remove(connection);
    });
  }

  Future<void> _stopNativeProxyOperation(
    NativeConnection connection,
    NativeProxyKind kind,
    int generation,
  ) async {
    bool isCurrent() =>
        !_disposed &&
        generation == _nativeGeneration &&
        identical(connection, _nativeConnection);
    try {
      await connection.stopProxy(kind);
      if (!isCurrent()) return;
      if (kind == NativeProxyKind.http) {
        _nativeHttpRunning = false;
        _systemProxyRunning = false;
        _systemProxyRecoveryError = null;
        _boundHttpPort = null;
        _boundHttpListen = null;
        _httpDisplayEndpoint = null;
      } else {
        _nativeSocksRunning = false;
        _boundSocksPort = null;
        _boundSocksListen = null;
        _socksDisplayEndpoint = null;
      }
    } on Object catch (error) {
      if (!isCurrent()) return;
      if (kind == NativeProxyKind.http && _systemProxyRunning) {
        _systemProxyRecoveryError = '系统代理恢复失败或 HTTP 停止失败：$error';
      }
      final label = kind == NativeProxyKind.http ? 'HTTP' : 'SOCKS5';
      _errorMessage = '关闭 $label 代理失败：$error';
      _errorCode = ConnectionErrorCode.tunnelFailed;
      ShuLog.w(ShuLogTag.proxy, _errorMessage!);
    }
  }

  /// Native rebind updates an existing listener and preserves independent
  /// services; a stopped listener is not enabled by a settings edit.
  Future<void> rebindHttpProxy() async {
    await _rebindProxy(NativeProxyKind.http);
  }

  Future<void> rebindSocksProxy() async {
    await _rebindProxy(NativeProxyKind.socks);
  }

  Future<void> _rebindProxy(NativeProxyKind kind) async {
    final connection = _nativeConnection;
    if (connection == null || !tunnelUp) return;
    if (kind == NativeProxyKind.http ? !httpProxyRunning : !socksProxyRunning) {
      return;
    }
    final generation = _nativeGeneration;
    _busy = true;
    notifyListeners();
    try {
      final result = await connection.rebindProxy(
        kind,
        kind == NativeProxyKind.http
            ? _settings.httpListen.address
            : _settings.socksListen.address,
        kind == NativeProxyKind.http ? _settings.httpPort : _settings.socksPort,
        systemProxyEnabled: _settings.systemProxyEnabled,
      );
      if (!_disposed &&
          generation == _nativeGeneration &&
          identical(connection, _nativeConnection)) {
        _showServices(result);
      }
    } on Object catch (error) {
      if (!_disposed && generation == _nativeGeneration) {
        _errorMessage = '重绑代理失败：$error';
        _errorCode = ConnectionErrorCode.tunnelFailed;
        _state = ConnectionStatus.error;
      }
    } finally {
      if (!_disposed && generation == _nativeGeneration) {
        _busy = false;
        notifyListeners();
      }
    }
  }

  /// 按需启动原生桌面校园分流。
  Future<bool> startVpn() async {
    if (_backend != null) return _startDesktopVpn();
    _fail(ConnectionErrorCode.unsupported, '当前后端未提供桌面 VPN');
    return false;
  }

  Future<void> stopVpn() async {
    if (_nativeConnection is NativeTunConnection) {
      await _stopDesktopVpn();
    }
  }

  Future<bool> refreshDesktopTun() => _tunInspection ??= _inspectDesktopTun()
      .whenComplete(() => _tunInspection = null);

  Future<bool> _inspectDesktopTun() async {
    _tunInspecting = true;
    if (!_disposed) notifyListeners();
    try {
      final platform = _tunPlatform;
      final environment = platform == null
          ? const NativeTunEnvironment(
              available: false,
              networks: [],
              reason: '当前后端未提供桌面 VPN，仍可使用本机代理',
            )
          : await platform.inspect();
      if (_disposed) return false;
      _tunEnvironment = environment;
      return environment.available;
    } on Object catch (error) {
      if (!_disposed) {
        _tunEnvironment = NativeTunEnvironment(
          available: false,
          networks: const [],
          reason: '读取 VPN 状态失败，请刷新重试：$error',
        );
      }
      return false;
    } finally {
      _tunInspecting = false;
      if (!_disposed) notifyListeners();
    }
  }

  void selectDesktopInterface(String value) {
    if (busy || tunnelUp || cleanupPending) return;
    if (value.isNotEmpty &&
        !(_tunEnvironment?.networks.any((network) => network.name == value) ??
            false)) {
      return;
    }
    _physicalInterface = value;
    _tunPreview = null;
    notifyListeners();
  }

  String? validateDesktopDns(String value) {
    try {
      final platform = _tunPlatform;
      if (platform == null) return '当前后端未提供桌面 DNS 校验';
      platform.normalizeDns(value);
      return null;
    } on Object catch (error) {
      return '$error';
    }
  }

  void selectDesktopDns(String value) {
    if (busy || tunnelUp || cleanupPending) return;
    final platform = _tunPlatform;
    if (platform == null) return;
    _physicalDns = platform.normalizeDns(value);
    _tunPreview = null;
    notifyListeners();
  }

  Future<void> previewDesktopTun() async {
    final connection = _nativeConnection;
    if (_tunPreviewLoading || connection is! NativeTunConnection || !tunnelUp) {
      return;
    }
    final generation = _nativeGeneration;
    _tunPreviewLoading = true;
    _tunPreview = null;
    notifyListeners();
    try {
      final preview = await (connection as NativeTunConnection).previewTun(
        _settings.vpnMtu,
        physicalDns: _physicalDns.isEmpty ? null : _physicalDns,
        physicalInterface: _physicalInterface,
        captureMode: _settings.vpnCaptureMode,
      );
      if (!_disposed &&
          generation == _nativeGeneration &&
          identical(connection, _nativeConnection)) {
        _tunPreview = preview;
        _desktopTunError = null;
      }
    } on Object catch (error) {
      if (!_disposed && generation == _nativeGeneration) {
        _desktopTunError = '无法准备接管范围：$error';
      }
    } finally {
      _tunPreviewLoading = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> _startDesktopVpn() async {
    if (vpnRunning) return true;
    final connection = _nativeConnection;
    if (busy ||
        cleanupPending ||
        connection is! NativeTunConnection ||
        !tunnelUp) {
      return false;
    }
    final generation = _nativeGeneration;
    final wasBusy = _busy;
    _busy = true;
    _desktopTunError = null;
    notifyListeners();
    try {
      final snapshot = await (connection as NativeTunConnection).startTun(
        mtu: _settings.vpnMtu,
        physicalInterface: _physicalInterface,
        physicalDns: _physicalDns.isEmpty ? null : _physicalDns,
        captureMode: _settings.vpnCaptureMode,
      );
      if (_disposed ||
          generation != _nativeGeneration ||
          !identical(connection, _nativeConnection)) {
        return false;
      }
      _nativeTun = snapshot;
      if (!snapshot.running) {
        _desktopTunError = snapshot.diagnostic ?? 'VPN 尚未完成启动，请断开后重试';
        if (snapshot.cleanupPending) _nativeTunCleanupError = _desktopTunError;
      }
      return snapshot.running;
    } on Object catch (error) {
      if (!_disposed && generation == _nativeGeneration) {
        _desktopTunError = '启动 VPN 失败：$error';
        _nativeTunCleanupError = '无法确认 VPN 启动结果，请断开后重试：$error';
      }
      return false;
    } finally {
      if (!_disposed && generation == _nativeGeneration) _busy = wasBusy;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> _stopDesktopVpn() {
    final connection = _nativeConnection;
    if (connection == null || connection is! NativeTunConnection) {
      return Future<void>.value();
    }
    final current = _nativeTunStops[connection];
    if (current != null) return current;
    final completed = Completer<void>();
    _nativeTunStops[connection] = completed.future;
    unawaited(
      _stopDesktopVpnOperation(connection, _nativeGeneration).then(
        (_) {
          _nativeTunStops.remove(connection);
          completed.complete();
        },
        onError: (Object error, StackTrace stack) {
          _nativeTunStops.remove(connection);
          completed.completeError(error, stack);
        },
      ),
    );
    return completed.future;
  }

  Future<void> _stopDesktopVpnOperation(
    NativeConnection connection,
    int generation,
  ) async {
    final wasBusy = _busy;
    _busy = true;
    if (!_disposed) notifyListeners();
    try {
      final snapshot = await (connection as NativeTunConnection).stopTun();
      if (_disposed ||
          generation != _nativeGeneration ||
          !identical(connection, _nativeConnection)) {
        return;
      }
      _nativeTun = snapshot;
      _desktopTunError = snapshot.diagnostic;
      _nativeTunCleanupError = snapshot.cleanupPending
          ? snapshot.diagnostic ?? 'VPN 清理尚未完成，请断开后重试'
          : null;
    } on Object catch (error) {
      if (!_disposed && generation == _nativeGeneration) {
        _desktopTunError = '停止 VPN 失败：$error';
        _nativeTunCleanupError = _desktopTunError;
      }
    } finally {
      if (!_disposed && generation == _nativeGeneration) _busy = wasBusy;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> toggleVpn() async {
    if (busy) return;
    if (vpnRunning || cleanupPending) {
      await stopVpn();
    } else {
      await startVpn();
    }
  }

  Future<void> disconnect() {
    final current = _disconnectTask;
    if (current != null) return current;
    final completed = Completer<void>();
    _disconnectTask = completed.future;
    unawaited(
      _disconnect().then(
        (_) {
          _disconnectTask = null;
          if (!_disposed) notifyListeners();
          completed.complete();
        },
        onError: (Object error, StackTrace stack) {
          _disconnectTask = null;
          if (!_disposed) notifyListeners();
          completed.completeError(error, stack);
        },
      ),
    );
    return completed.future;
  }

  Future<void> disconnectForExit() {
    final current = _exitTask;
    if (current != null) return current;
    final completed = Completer<void>();
    _exitTask = completed.future;
    unawaited(
      _finishExit().then(
        (_) {
          _exitTask = null;
          if (!_disposed) notifyListeners();
          completed.complete();
        },
        onError: (Object error, StackTrace stack) {
          _exitTask = null;
          if (!_disposed) notifyListeners();
          completed.completeError(error, stack);
        },
      ),
    );
    return completed.future;
  }

  Future<void> _finishExit() async {
    _exiting = true;
    _nativeGeneration++;
    _state = ConnectionStatus.disconnecting;
    if (!_disposed) notifyListeners();
    try {
      await _backend?.shutdown();
      await _releaseView();
      _clearRuntimePresentation();
      _clearError();
      if (!_disposed) _state = ConnectionStatus.disconnected;
    } on Object catch (error) {
      if (!_disposed) {
        _errorMessage = cleanupError ?? '$error';
        _errorCode = ConnectionErrorCode.tunnelFailed;
        _state = ConnectionStatus.error;
      }
      rethrow;
    } finally {
      _exiting = false;
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> _disconnect() async {
    if (!canDisconnect) return;

    _busy = true;
    _nativeGeneration++;
    _state = ConnectionStatus.disconnecting;
    notifyListeners();

    try {
      await _backend?.disconnect();
      await _releaseView();
      _clearRuntimePresentation();
      _clearError();
      _state = ConnectionStatus.disconnected;
      ShuLog.i(ShuLogTag.conn, '已断开');
    } on Object catch (error) {
      _errorCode = ConnectionErrorCode.tunnelFailed;
      _errorMessage = cleanupError ?? '$error';
      _state = ConnectionStatus.error;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  /// Single entry point for the orb and the dock.
  Future<void> toggle() async {
    if (_exiting || _disconnectTask != null || _exitTask != null) return;
    if (_busy && _state != ConnectionStatus.connecting) return;
    switch (_state) {
      case ConnectionStatus.connected:
      case ConnectionStatus.authenticated:
        await disconnect();
      case ConnectionStatus.connecting:
      case ConnectionStatus.disconnecting:
        cancel();
      case ConnectionStatus.disconnected:
      case ConnectionStatus.error:
        await connect();
    }
  }

  Future<void> _releaseView() async {
    await _nativeSubscription?.cancel();
    _nativeSubscription = null;
    _nativeConnection?.disposeView();
    _nativeConnection = null;
    _nativeUpdate = null;
  }

  void _clearRuntimePresentation() {
    _systemProxyRecoveryError = null;
    _systemProxyRunning = false;
    _nativeHttpRunning = false;
    _nativeSocksRunning = false;
    _boundHttpPort = null;
    _boundSocksPort = null;
    _boundHttpListen = null;
    _boundSocksListen = null;
    _httpDisplayEndpoint = null;
    _socksDisplayEndpoint = null;
    _nativeTunCleanupError = null;
    _nativeTun = const NativeTunSnapshot.stopped();
    _tunPreview = null;
    _desktopTunError = null;
    _httpDisplayEndpoint = null;
    _socksDisplayEndpoint = null;
    _virtualAddress = null;
    _dnsServers = const <String>[];
    _connectedAt = null;
  }
}

class _ConnectionValidationException implements Exception {
  const _ConnectionValidationException(this.code, this.message);
  final ConnectionErrorCode code;
  final String message;
}
