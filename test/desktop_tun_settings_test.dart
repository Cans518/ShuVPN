import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/app/theme.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/connection/native_tun.dart';
import 'package:shuvpn/core/settings/settings_store.dart';

import 'settings_stub.dart';
import 'native_backend_stub.dart';

import 'package:shuvpn/features/settings/connection_settings_page.dart';
import 'package:shuvpn/widgets/settings_rows.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  Future<ConnectionController> pump(
    WidgetTester tester, {
    bool available = false,
    Brightness brightness = Brightness.light,
    double textScale = 1,
    StubSettingsStore? stored,
  }) async {
    SharedPreferences.setMockInitialValues({});
    final settings = stored ?? await StubSettingsStore.load();
    settings.vpnEnabled = false;
    final controller = ConnectionController(
      settings,
      nativeConnectionBackend: StubNativeBackend(),
      desktopTunPlatform: _Platform(available),
    );
    addTearDown(controller.dispose);
    addTearDown(settings.dispose);
    await tester.pumpWidget(
      MultiProvider(
        providers: [
          ChangeNotifierProvider<SettingsStore>.value(value: settings),
          ChangeNotifierProvider<ConnectionController>.value(value: controller),
        ],
        child: MaterialApp(
          theme: buildShuTheme(brightness),
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(textScale)),
            child: child!,
          ),
          home: const ShuConnectionSettingsPage(),
        ),
      ),
    );
    await tester.pumpAndSettle();
    return controller;
  }

  testWidgets(
    'desktop network page shows native permission reason and no Android controls',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 900);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await pump(tester);
      await tester.scrollUntilVisible(find.text('本次底层网络'), 300);
      expect(find.text('Android VPN 服务'), findsNothing);
      expect(find.text('系统授权状态'), findsNothing);
      expect(find.text('需要网络管理权限'), findsOneWidget);
      final switchRow = tester.widget<SettingsSwitchRow>(
        find.widgetWithText(SettingsSwitchRow, '连接时启用 VPN'),
      );
      expect(switchRow.enabled, isFalse);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('capture choice saves full mode and can return to campus', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 900);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final controller = await pump(tester, available: true);
    for (final (label, mode) in [
      ('完整捕获（实验性）', CaptureMode.full),
      ('校园分流', CaptureMode.campus),
    ]) {
      await tester.scrollUntilVisible(find.text('VPN 捕获模式'), 250);
      await tester.tap(find.text('VPN 捕获模式'));
      await tester.pumpAndSettle();
      await tester.tap(find.text(label).last);
      await tester.pumpAndSettle();
      expect(controller.settings.vpnCaptureMode, mode);
      expect(controller.settings.vpnEnabled, isFalse);
    }
    expect(tester.takeException(), isNull);
  });

  testWidgets('capture mode save failure retains the committed mode', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 900);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final controller = await pump(
      tester,
      available: true,
      stored: _FailCaptureSettings(),
    );
    await tester.scrollUntilVisible(find.text('VPN 捕获模式'), 250);
    await tester.tap(find.text('VPN 捕获模式'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('完整捕获（实验性）'));
    await tester.pumpAndSettle();
    expect(controller.settings.vpnCaptureMode, CaptureMode.campus);
    expect(find.textContaining('设置保存失败'), findsOneWidget);
  });

  testWidgets(
    'network choice uses native choices and remains a per-launch draft',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 900);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final controller = await pump(tester, available: true);
      await tester.scrollUntilVisible(find.text('本次底层网络'), 300);
      await tester.ensureVisible(find.text('本次底层网络'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('本次底层网络'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('wifi · 系统默认'));
      await tester.pumpAndSettle();
      expect(controller.desktopPhysicalInterface, 'wifi');
      expect(controller.settings.vpnEnabled, isFalse);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'desktop VPN rows stay scrollable with large text in light and dark themes',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(375, 812);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      for (final brightness in Brightness.values) {
        await pump(
          tester,
          available: true,
          brightness: brightness,
          textScale: 1.8,
        );
        await tester.scrollUntilVisible(find.text('VPN 捕获模式'), 250);
        await tester.ensureVisible(find.text('VPN 捕获模式'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('VPN 捕获模式'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        await tester.ensureVisible(find.text('完整捕获（实验性）'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('完整捕获（实验性）'));
        await tester.pumpAndSettle();
        await tester.scrollUntilVisible(find.text('MTU'), 300);
        expect(find.text('MTU'), findsOneWidget);
        expect(find.text('DNS'), findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pumpAndSettle();
      }
    },
  );
}

class _FailCaptureSettings extends StubSettingsStore {
  @override
  Future<void> save(SettingKey key, Object value) {
    if (key == SettingKey.vpnCaptureMode) {
      throw const ShuSettingsException('fixture capture commit failure');
    }
    return super.save(key, value);
  }
}

class _Platform implements NativeTunPlatform {
  const _Platform(this.available);
  final bool available;
  @override
  String normalizeDns(String value) => value.trim();
  @override
  Future<NativeTunEnvironment> inspect() async => NativeTunEnvironment(
    available: available,
    reason: available ? null : '需要网络管理权限',
    networks: const [
      NativeTunNetwork(name: 'wifi', description: '无线网络', recommended: true),
    ],
  );
}
