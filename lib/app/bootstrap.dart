import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../core/logging/shu_log.dart';
import '../core/rust/rust_core.dart';
import '../core/rust/rust_account_center.dart';
import '../core/rust/rust_settings_store.dart';
import '../core/rust/rust_connection.dart';
import '../core/rust/rust_desktop_tun.dart';
import '../core/rust/routing_service.dart';
import '../core/rust/generated/api/system_proxy.dart' as system_proxy;
import '../core/rust/generated/connection/model.dart' as core;
import '../core/settings/settings_store.dart';
import 'app.dart';
import 'shu_launch_surface.dart';
import 'theme.dart';
import '../core/desktop/desktop_port.dart';
import 'desktop_frame.dart';

/// 启动外壳：**先把第一帧画出来，再去读设置**。
///
/// 顺序与 [`main`] 里的做法相反是有原因的。以前是
/// `await SettingsStore.load()` → `await migrateIfNeeded()` → `runApp(...)`，
/// 于是从原生启动图消失到 Flutter 画出第一个像素之间是一段**纯等待**：
/// 屏幕上什么都没有，用户看到的是启动图直接跳成主页。现在 `runApp` 是第一
/// 件事，等待期间屏幕上已经是 [ShuLaunchSurface]，读完了再换成真正的应用。
///
/// 设置仍然在**任何一行代码读它之前**读完 —— `ShuVpnApp` 是读完才被建出来的，
/// 所以「迁移必须在读取之前跑完」这条约束没有被破坏，只是等待的位置从
/// `main()` 挪到了这里。
///
/// ## 最短展示时长
///
/// 只按真实耗时显示的话，`SharedPreferences` 在多数机器上几十毫秒就好了 ——
/// 图标会**闪一下**，比完全不显示更难看（闪一下读起来像闪屏故障）。所以给它
/// 一个下限：短的补到 [minimumSplash]，长的就等它。这不是「故意拖延启动」，
/// 补的那一段本来也留着给原生那边的首帧渲染。
class ShuVpnBootstrap extends StatefulWidget {
  const ShuVpnBootstrap({
    super.key,
    this.minimumSplash = const Duration(milliseconds: 700),
    this.initializeCore,
    this.desktopPort,
  });

  /// 启动页至少停留多久。
  final Duration minimumSplash;

  /// 可注入的核心启动前置过程；Rust 初始化与健康检查仍须完成。
  final Future<void> Function()? initializeCore;
  final DesktopPort? desktopPort;

  @override
  State<ShuVpnBootstrap> createState() => _ShuVpnBootstrapState();
}

class _ShuVpnBootstrapState extends State<ShuVpnBootstrap> {
  SettingsStore? _settings;
  Object? _error;
  RoutingService? _routing;
  RustAccountCenter? _account;
  RustSettingsStore? _nativeSettings;
  RustConnectionBackend? _connections;
  bool _systemProxyAvailable = false;
  String? _systemProxyUnavailableReason;

  @override
  void initState() {
    super.initState();
    _start();
  }

  Future<void> _start() async {
    final startedAt = DateTime.now();
    try {
      final preferences = await SharedPreferences.getInstance();
      SettingsStore settings;
      final initializeCore = widget.initializeCore;
      if (initializeCore != null) {
        await initializeCore();
      }
      if (!Platform.isWindows && !Platform.isLinux) {
        throw UnsupportedError('ShuVPN 仅支持 Windows 与 Linux');
      }
      final coreVersion = await RustCore.initialize();
      final support = await getApplicationSupportDirectory();
      final proxyDirectory = '${support.path}${Platform.pathSeparator}network';
      final connections = RustConnectionBackend(
        systemProxyDirectory: proxyDirectory,
      );
      _connections = connections;
      await connections.initialize();
      if (!mounted) {
        connections.dispose();
        return;
      }
      final nativeSettings = await RustSettingsStore.open(
        '${support.path}${Platform.pathSeparator}settings',
        preferences,
      );
      if (!mounted) {
        nativeSettings.dispose();
        return;
      }
      _nativeSettings = nativeSettings;
      settings = nativeSettings;
      ShuLog.i('core', 'Rust 核心已初始化 · 版本 $coreVersion');
      try {
        await system_proxy.recoverSystemProxy(directory: proxyDirectory);
      } on core.VpnFailure catch (failure) {
        throw StateError(failure.message);
      }
      try {
        await system_proxy.systemProxyBackend();
        _systemProxyAvailable = true;
      } on core.VpnFailure catch (failure) {
        _systemProxyUnavailableReason = failure.message;
      }
      final routing = await RoutingService.open(
        '${support.path}${Platform.pathSeparator}rules',
      );
      if (!mounted) {
        routing.dispose();
        return;
      }
      _routing = routing;
      connections.bindRoutingManager(routing.manager);
      final account = await RustAccountCenter.open(
        '${support.path}${Platform.pathSeparator}auth',
        preferences,
      );
      if (!mounted) {
        account.dispose();
        return;
      }
      _account = account;
      final remaining =
          widget.minimumSplash - DateTime.now().difference(startedAt);
      if (remaining > Duration.zero) {
        await Future<void>.delayed(remaining);
      }
      if (!mounted) return;
      setState(() => _settings = settings);
    } on Object catch (error) {
      _connections?.dispose();
      _connections = null;
      _account?.dispose();
      _account = null;
      _routing?.dispose();
      _routing = null;
      _nativeSettings?.dispose();
      _nativeSettings = null;
      ShuLog.e('app', '启动失败 · $error');
      if (!mounted) return;
      setState(() => _error = error);
    }
  }

  @override
  void dispose() {
    _connections?.dispose();
    _account?.dispose();
    _routing?.dispose();
    _nativeSettings?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final settings = _settings;
    final account = _account;
    if (settings != null && account != null) {
      return ShuVpnApp(
        settings: settings,
        accountCenter: account,
        nativeConnectionBackend: _connections,
        routingService: _routing,
        desktopTunPlatform: const RustDesktopTunPlatform(),
        desktopPort: widget.desktopPort,
        systemProxyAvailable: _systemProxyAvailable,
        systemProxyUnavailableReason: _systemProxyUnavailableReason,
      );
    }

    // 启动页也要有主题：`ShuLaunchSurface` 的底色与图标取自
    // [ShuYoColors]，而 `themeMode: system` 让它在系统深色下自动换成深色
    // 那一套。
    return MaterialApp(
      title: 'ShuVPN',
      debugShowCheckedModeBanner: false,
      theme: buildShuTheme(Brightness.light),
      darkTheme: buildShuTheme(Brightness.dark),
      themeMode: ThemeMode.system,
      builder: (context, child) {
        final port = widget.desktopPort;
        return port == null
            ? child ?? const SizedBox.shrink()
            : DesktopFrame(port: port, child: child ?? const SizedBox.shrink());
      },
      home: _error == null
          ? const ShuLaunchSurface()
          : _BootstrapError(error: '$_error'),
    );
  }
}

/// 设置或核心初始化失败时的启动页。
class _BootstrapError extends StatelessWidget {
  const _BootstrapError({required this.error});

  final String error;

  @override
  Widget build(BuildContext context) {
    final colors = context.shuyoColors;
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.all(ShuSpacing.page * 2),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              '无法启动应用',
              style: TextStyle(
                color: colors.textPrimary,
                fontSize: 20,
                fontWeight: FontWeight.w600,
              ),
            ),
            const SizedBox(height: 12),
            Text(
              '重开应用再试一次。\n\n$error',
              style: TextStyle(color: colors.textTertiary, height: 1.5),
            ),
          ],
        ),
      ),
    );
  }
}
