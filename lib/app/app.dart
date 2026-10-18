import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../core/account/account_center.dart';
import '../core/announcements/shu_announcement_client.dart';
import '../core/connection/connection_controller.dart';
import '../core/connection/protocol.dart';
import '../core/connection/native_connection.dart';
import '../core/connection/native_tun.dart';
import '../core/settings/settings_store.dart';
import '../core/rust/routing_service.dart';
import '../core/rust/rust_announcement_client.dart';
import '../core/rust/rust_update_client.dart';
import '../core/desktop/desktop_port.dart';
import 'desktop_host.dart';
import '../core/update/shu_update_client.dart';
import 'app_info.dart';
import 'router.dart';
import 'theme.dart';

/// Root widget.
///
/// It owns the three long-lived objects of the app — settings, the connection
/// controller and the account center — and nothing else. Screens read them
/// through `provider`; no screen owns connection state of its own.
class ShuVpnApp extends StatefulWidget {
  const ShuVpnApp({
    super.key,
    required this.settings,
    required this.accountCenter,
    this.updateClient,
    this.announcementClient,
    this.nativeConnectionBackend,
    this.desktopTunPlatform,
    this.routingService,
    this.desktopPort,
    this.systemProxyAvailable = false,
    this.systemProxyUnavailableReason,
  });

  final SettingsStore settings;
  final AccountCenter accountCenter;
  final NativeConnectionBackend? nativeConnectionBackend;
  final NativeTunPlatform? desktopTunPlatform;
  final RoutingService? routingService;
  final DesktopPort? desktopPort;
  final bool systemProxyAvailable;
  final String? systemProxyUnavailableReason;

  /// 更新检查的通道。
  ///
  /// 可注入只为测试：启动路径每次都会跑一次检查，而 widget 测试既不该真的
  /// 发请求，也不该依赖 GitHub 的返回。桌面使用 [RustUpdateClient]。
  final ShuUpdateClient? updateClient;

  /// 信息办公告的通道。
  ///
  /// 同样可注入只为测试：通知页一打开就拉列表，而 widget 测试不该依赖
  /// shu.edu.cn 的返回。桌面使用 [RustAnnouncementClient]。
  final ShuAnnouncementClient? announcementClient;

  @override
  State<ShuVpnApp> createState() => _ShuVpnAppState();
}

class _ShuVpnAppState extends State<ShuVpnApp> {
  @override
  void dispose() {
    widget.accountCenter.dispose();
    _router.dispose();
    super.dispose();
  }

  /// Built once per app so navigation state cannot leak between instances.
  ///
  /// 设置要传进路由：首启门（`welcomeCompleted`）是**重定向**的判据，而重定向
  /// 必须能读到它。传的是实例而不是值 —— 引导页写完标记后，同一次跳转会重新
  /// 求值，读到的是新值。
  late final GoRouter _router = createShuRouter(settings: widget.settings);

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider<RoutingService?>.value(
          value: widget.routingService,
        ),
        ChangeNotifierProvider<SettingsStore>.value(value: widget.settings),
        ChangeNotifierProvider<ConnectionController>(
          create: (_) => ConnectionController(
            widget.settings,
            draft: _draft(widget.settings),
            nativeConnectionBackend: widget.nativeConnectionBackend,
            desktopTunPlatform: widget.desktopTunPlatform,
            systemProxyAvailable: widget.systemProxyAvailable,
            systemProxyUnavailableReason: widget.systemProxyUnavailableReason,
          ),
        ),
        ChangeNotifierProvider<AccountCenter>.value(
          value: widget.accountCenter,
        ),
        Provider<ShuUpdateClient>(
          create: (_) => widget.updateClient ?? RustUpdateClient(),
          dispose: (_, client) => client.close(),
        ),
        Provider<ShuAnnouncementClient>(
          create: (_) => widget.announcementClient ?? RustAnnouncementClient(),
          dispose: (_, client) => client.close(),
        ),
      ],
      child: Consumer<SettingsStore>(
        builder: (context, settings, _) => MaterialApp.router(
          title: ShuAppInfo.name,
          debugShowCheckedModeBanner: false,
          theme: buildShuTheme(Brightness.light),
          darkTheme: buildShuTheme(Brightness.dark),
          themeMode: settings.themeMode,
          routerConfig: _router,
          builder: (context, child) {
            final port = widget.desktopPort;
            if (port == null) return child ?? const SizedBox.shrink();
            return DesktopHost(
              port: port,
              navigate: _router.go,
              child: child ?? const SizedBox.shrink(),
            );
          },
        ),
      ),
    );
  }
}

/// Restores the endpoint so the button is usable straight after a cold start.
///
/// There is only one gateway now, so this is no longer a "which stack" question
/// — it just reads back the host the user last used, and the protocol the user
/// last had enabled.
ConnectionDraft _draft(SettingsStore settings) {
  return ConnectionDraft(
    protocol: settings.firstEnabledProtocol ?? ShuProtocol.atrust,
    server: settings.server,
    loginDomain: settings.loginDomain,
  );
}
