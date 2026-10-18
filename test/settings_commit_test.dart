import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/settings/settings_store.dart';

import 'settings_stub.dart';

import 'package:shuvpn/features/settings/experimental_settings_page.dart';
import 'package:shuvpn/features/settings/connection_settings_page.dart';

class _DelayedSettings extends StubSettingsStore {
  _DelayedSettings(super.fixture) {
    validation = (key, value) =>
        key == SettingKey.httpPort &&
            const <Object>{'', 'invalid', -1, 65536}.contains(value)
        ? '请输入 0–65535 之间的整数'
        : null;
  }
  final barrier = Completer<void>();
  bool fail = false;

  @override
  Future<void> save(SettingKey key, Object value) async {
    await barrier.future;
    if (fail) throw const ShuSettingsException('injected storage failure');
    await super.save(key, value);
  }
}

class _RecordingConnection extends ConnectionController {
  _RecordingConnection(super.settings);
  int httpRebinds = 0;
  @override
  Future<void> rebindHttpProxy() async {
    httpRebinds++;
  }
}

void main() {
  for (final fail in [false, true]) {
    testWidgets(
      'onboarding navigation waits for ${fail ? 'failed' : 'successful'} settings commit',
      (tester) async {
        SharedPreferences.setMockInitialValues({
          'settings.welcomeCompleted': true,
        });
        final settings = _DelayedSettings(await SharedPreferences.getInstance())
          ..fail = fail;
        final connection = ConnectionController(settings);
        final router = GoRouter(
          initialLocation: '/experimental',
          routes: [
            GoRoute(
              path: '/experimental',
              builder: (_, _) => const ShuExperimentalSettingsPage(),
            ),
            GoRoute(
              path: '/welcome',
              builder: (_, _) =>
                  const Scaffold(body: Text('welcome destination')),
            ),
          ],
        );
        addTearDown(() async {
          await tester.pumpWidget(const SizedBox.shrink());
          router.dispose();
          connection.dispose();
          settings.dispose();
        });
        await tester.pumpWidget(
          MultiProvider(
            providers: [
              ChangeNotifierProvider<SettingsStore>.value(value: settings),
              ChangeNotifierProvider<ConnectionController>.value(
                value: connection,
              ),
            ],
            child: MaterialApp.router(routerConfig: router),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('新用户引导'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('重新开始'));
        await tester.pumpAndSettle();
        expect(settings.welcomeCompleted, isTrue);
        expect(find.text('welcome destination'), findsNothing);

        settings.barrier.complete();
        await tester.pumpAndSettle();
        expect(settings.welcomeCompleted, fail);
        expect(
          find.text('welcome destination'),
          fail ? findsNothing : findsOneWidget,
        );
        if (fail) {
          expect(
            find.textContaining('injected storage failure'),
            findsOneWidget,
          );
        }
        expect(tester.takeException(), isNull);
      },
    );
  }

  for (final fail in [false, true]) {
    testWidgets(
      'HTTP rebind waits for ${fail ? 'failed' : 'successful'} settings commit',
      (tester) async {
        SharedPreferences.setMockInitialValues({});
        final settings = _DelayedSettings(await SharedPreferences.getInstance())
          ..fail = fail;
        final connection = _RecordingConnection(settings);
        addTearDown(() async {
          await tester.pumpWidget(const SizedBox.shrink());
          connection.dispose();
          settings.dispose();
        });
        await tester.pumpWidget(
          MultiProvider(
            providers: [
              ChangeNotifierProvider<SettingsStore>.value(value: settings),
              ChangeNotifierProvider<ConnectionController>.value(
                value: connection,
              ),
            ],
            child: const MaterialApp(home: ShuConnectionSettingsPage()),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('HTTP 代理端口'));
        await tester.pumpAndSettle();
        for (final input in ['', 'invalid', '-1', '65536']) {
          await tester.enterText(find.byType(TextField), input);
          await tester.tap(find.text('保存'));
          await tester.pumpAndSettle();
          expect(find.byType(AlertDialog), findsOneWidget);
          expect(find.text('请输入 0–65535 之间的整数'), findsOneWidget);
          expect(settings.httpPort, 3322);
          expect(connection.httpRebinds, 0);
        }
        await tester.enterText(find.byType(TextField), '4444');
        await tester.tap(find.text('保存'));
        await tester.pumpAndSettle();
        expect(settings.httpPort, 3322);
        expect(connection.httpRebinds, 0);

        settings.barrier.complete();
        await tester.pumpAndSettle();
        expect(settings.httpPort, fail ? 3322 : 4444);
        expect(connection.httpRebinds, fail ? 0 : 1);
        if (fail) {
          expect(
            find.textContaining('injected storage failure'),
            findsOneWidget,
          );
        }
        expect(tester.takeException(), isNull);
      },
    );
  }
}
