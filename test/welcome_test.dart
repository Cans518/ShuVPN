import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/app/app.dart';
import 'package:shuvpn/app/theme.dart';
import 'package:shuvpn/core/account/account_center.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/connection/native_tun.dart';
import 'package:shuvpn/core/connection/protocol.dart';
import 'package:shuvpn/core/logging/shu_log.dart';
import 'package:shuvpn/core/settings/settings_store.dart';

import 'settings_stub.dart';
import 'native_backend_stub.dart';

import 'package:shuvpn/features/onboarding/welcome_page.dart';
import 'package:shuvpn/features/settings/permission_info_page.dart';
import 'package:shuvpn/features/settings/settings_page.dart';
import 'package:shuvpn/shell/floating_dock.dart';

import 'account_stub.dart';
import 'shu_update_stub.dart';

/// 一台真手机的视口，与 `widget_test.dart` 的 `_pumpApp` 同一套数字。
Future<void> _viewport(WidgetTester tester) async {
  tester.view.physicalSize = const Size(720, 3600);
  tester.view.devicePixelRatio = 2;
  addTearDown(tester.view.reset);
}

/// 整个应用，**不**写 `welcomeCompleted` —— 也就是全新安装的样子。
Future<void> _pumpFreshInstall(WidgetTester tester) async {
  await _viewport(tester);
  SharedPreferences.setMockInitialValues(<String, Object>{});
  final settings = await StubSettingsStore.load();
  await tester.pumpWidget(
    ShuVpnApp(
      settings: settings,
      accountCenter: StubAccountCenter(),
      updateClient: StubShuUpdateClient(),
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> _pumpStandalone(
  WidgetTester tester, {
  NativeTunPlatform? desktopTunPlatform,
}) async {
  await _viewport(tester);
  SharedPreferences.setMockInitialValues(<String, Object>{});
  final settings = await StubSettingsStore.load();
  await tester.pumpWidget(
    MultiProvider(
      providers: [
        ChangeNotifierProvider<SettingsStore>.value(value: settings),
        ChangeNotifierProvider<ConnectionController>(
          create: (_) => ConnectionController(
            settings,
            draft: ConnectionDraft(
              protocol: ShuProtocol.atrust,
              server: settings.server,
              loginDomain: settings.loginDomain,
            ),
            nativeConnectionBackend: desktopTunPlatform == null
                ? null
                : StubNativeBackend(),
            desktopTunPlatform: desktopTunPlatform,
          ),
        ),
        ChangeNotifierProvider<AccountCenter>(
          create: (_) => StubAccountCenter(),
        ),
      ],
      child: MaterialApp(
        theme: buildShuTheme(Brightness.light),
        home: const ShuWelcomePage(),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> _tap(WidgetTester tester, String label) async {
  await tester.tap(find.text(label));
  await tester.pumpAndSettle();
}

/// 底部那颗主按钮。置灰是拿它自己的 `onPressed` 判的，不是颜色 —— 颜色
/// 断言不到，而「点了没反应」才是这件事的真行为。
FilledButton _footerButton(WidgetTester tester) =>
    tester.widget<FilledButton>(find.byType(FilledButton));

void main() {
  setUp(ShuLog.instance.clear);

  test('桌面引导说明按需申请网络权限', () {
    expect(ShuWelcomePage.permissionSubtitle(), '连接时按需申请系统网络权限');
  });

  for (final platform in [TargetPlatform.windows, TargetPlatform.linux]) {
    testWidgets('$platform 权限说明使用桌面权限文案', (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildShuTheme(Brightness.light).copyWith(platform: platform),
          home: const PermissionInfoPage(),
        ),
      );
      expect(find.text('VPN 权限'), findsOneWidget);
      expect(find.text('VPN 服务'), findsNothing);
      expect(find.text('通知'), findsNothing);
      expect(
        find.textContaining(
          platform == TargetPlatform.windows ? 'Windows 管理员确认' : '应用界面以普通用户运行',
        ),
        findsOneWidget,
      );
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('设置目录不再固定显示 Android 网络服务', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildShuTheme(Brightness.light),
        home: const SettingsPage(),
      ),
    );
    await tester.scrollUntilVisible(find.text('网络连接'), 200);
    expect(find.text('HTTP 与 SOCKS5 代理、VPN 捕获模式'), findsOneWidget);
    expect(find.textContaining('Android'), findsNothing);
  });

  group('首启门', () {
    testWidgets('全新安装的第一屏是引导，dock 还没有出现', (tester) async {
      await _pumpFreshInstall(tester);

      expect(find.text('欢迎使用 ShuVPN'), findsOneWidget);
      // 三页的按钮都要在，而底栏不能 —— 引导挡在主界面之前，这是它存在的
      // 全部意义。
      expect(find.byType(FloatingDock), findsNothing);
    });

    testWidgets('走完引导之前翻不进主页', (tester) async {
      await _pumpFreshInstall(tester);

      // 第 2 页的按钮只是进入登录，不会绕过最后一步把人放进主页。
      // 桌面上没有权限要申请（清单是空的），第 1 页的按钮直接就是可点的。
      await _tap(tester, '继续');
      await _tap(tester, '继续');
      await _tap(tester, '去登录');

      expect(find.byType(FloatingDock), findsNothing);
      // 落到的是账户管理那一套表单本身（`ShuLoginForm` 的第一屏）。
      expect(find.text('用户名/学号'), findsOneWidget);
      expect(find.text('密码'), findsOneWidget);
      expect(find.text('使用企业微信登录'), findsOneWidget);
    });
  });

  group('三页的顺序', () {
    testWidgets('欢迎 → 授权 → 登录，且每一步只有一个主题', (tester) async {
      await _pumpFreshInstall(tester);
      expect(find.text('欢迎使用 ShuVPN'), findsOneWidget);

      await _tap(tester, '继续');
      expect(find.text('连接时按需申请系统网络权限'), findsOneWidget);
      expect(find.text('欢迎使用 ShuVPN'), findsNothing);

      // 桌面上没有系统 VPN 可授权 —— 权限清单是空的，「全部申请到」因此
      // 一开始就成立，按钮一上来就可点。真机上是同一条代码路径，只是
      // 清单里多出一项、得先点它弹出系统对话框。
      await _tap(tester, '继续');
      expect(find.text('登录校园账户'), findsOneWidget);
      expect(find.text('连接时按需申请系统网络权限'), findsNothing);
    });

    testWidgets('返回键按原路退回上一页', (tester) async {
      await _pumpFreshInstall(tester);
      await _tap(tester, '继续');
      expect(find.text('连接时按需申请系统网络权限'), findsOneWidget);

      await tester.tap(find.byTooltip('返回'));
      await tester.pumpAndSettle();

      expect(find.text('欢迎使用 ShuVPN'), findsOneWidget);
      // 第一页没有上一页，所以返回键整个不画（槽位仍占着，标题不横跳）。
      expect(find.byTooltip('返回'), findsNothing);
    });

    testWidgets('登录页的返回键退回说明页，而不是退出引导', (tester) async {
      await _pumpFreshInstall(tester);
      await _tap(tester, '继续');
      await _tap(tester, '继续');
      await _tap(tester, '去登录');
      expect(find.text('用户名/学号'), findsOneWidget);

      await tester.tap(find.byTooltip('返回'));
      await tester.pumpAndSettle();

      // 回到的是第 2 页的说明（标题 + 底部的「去登录」），不是主页。
      expect(find.text('登录校园账户'), findsOneWidget);
      expect(find.text('去登录'), findsOneWidget);
      expect(find.byType(FloatingDock), findsNothing);
    });

    testWidgets('登录页列将要连上的系统，承诺停在按钮上方', (tester) async {
      await _pumpFreshInstall(tester);
      await _tap(tester, '继续');
      await _tap(tester, '继续');

      // 三个系统逐条列出，每条带自己的域名。行是 `ShuSystemTile` —— 与
      // 账户管理那三行同一个组件，所以这里查得到的东西那边也一定查得到。
      for (final host in <String>[
        'jwxt.shu.edu.cn',
        'atrust.shu.edu.cn',
        'otp.shu.edu.cn',
      ]) {
        expect(
          find.descendant(of: find.byType(ListTile), matching: find.text(host)),
          findsOneWidget,
          reason: '$host 应该有且只有一条系统行',
        );
      }

      // 承诺那一行在按钮**上方**，而且是**贴着**按钮的那一格 ——
      // `_layout` 的 `pageFooter`，与它里面那段可滚动的正文分开。
      //
      // ⚠️ 只断言「y 在按钮上方」是没有分辨力的：正文里随便哪一行都在按钮
      // 上方。所以先拆开结构（它不在 `SingleChildScrollView` 里），再量它
      // 离按钮有多近（贴住时差值是底距 8 + 行高，放进正文末尾会变成几百）。
      final note = find.textContaining('登录会话与账户展示状态会保存在本机');
      expect(find.textContaining('不保存您的校园账户信息'), findsNothing);
      expect(
        find.descendant(of: find.byType(SingleChildScrollView), matching: note),
        findsNothing,
      );
      final gap =
          tester.getTopLeft(find.text('去登录')).dy - tester.getTopLeft(note).dy;
      expect(gap, greaterThan(0));
      expect(gap, lessThan(120));
    });
  });

  group('VPN 授权这一步', () {
    testWidgets('桌面支持 VPN 时不显示 Android 授权，也不阻止登录', (tester) async {
      await _pumpStandalone(
        tester,
        desktopTunPlatform: const _DesktopTunPlatform(),
      );
      final controller = tester
          .element(find.byType(ShuWelcomePage))
          .read<ConnectionController>();
      await controller.refreshDesktopTun();
      await tester.pumpAndSettle();
      expect(controller.vpnSupported, isTrue);

      await _tap(tester, '继续');
      expect(find.textContaining('Android'), findsNothing);
      expect(find.text('VPN 服务'), findsNothing);
      expect(find.text('通知'), findsNothing);
      expect(_footerButton(tester).onPressed, isNotNull);
      await _tap(tester, '继续');
      expect(find.text('登录校园账户'), findsOneWidget);
    });
  });
}

class _DesktopTunPlatform implements NativeTunPlatform {
  const _DesktopTunPlatform();

  @override
  String normalizeDns(String value) => value.trim();

  @override
  Future<NativeTunEnvironment> inspect() async =>
      const NativeTunEnvironment(available: true, networks: []);
}
