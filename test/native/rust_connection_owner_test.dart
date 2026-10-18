import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/connection/connection_status.dart';
import 'package:shuvpn/core/rust/generated/api/routing.dart' as routing;
import 'package:shuvpn/core/rust/rust_account_center.dart';
import 'package:shuvpn/core/rust/rust_connection.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

import '../settings_stub.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test('production backend forwards the native account and rules and rejects invalid input without a fallback', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-owner-view-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final backend = RustConnectionBackend();
    await backend.initialize();
    addTearDown(() async {
      try {
        await backend.shutdown();
      } finally {
        backend.dispose();
      }
    });
    final rules = await routing.RoutingManager.open(
      directory: '${directory.path}/rules',
    );
    addTearDown(rules.dispose);
    backend.bindRoutingManager(rules);
    SharedPreferences.setMockInitialValues({
      'auth.newsso.cookies': jsonEncode([
        {'name': 'SHU_OAUTH2', 'value': 'fixture-owner-session', 'path': '/'},
      ]),
    });
    final account = await RustAccountCenter.open(
      '${directory.path}/account',
      await SharedPreferences.getInstance(),
    );
    addTearDown(account.dispose);
    expect(account.hasSession, isTrue);
    final settings = StubSettingsStore()..server = 'https://[';
    addTearDown(settings.dispose);
    final controller = ConnectionController(
      settings,
      nativeConnectionBackend: backend,
    )..ssoSessionProvider = () => account;
    addTearDown(controller.dispose);
    expect(await controller.connect(), isFalse);
    expect(controller.state, ConnectionStatus.error);
    expect(controller.errorMessage, contains('服务器地址'));
    expect(backend.lifecycle.errorCode, 'gateway');
    expect(backend.lifecycle.runId, isNull);
    expect(controller.cleanupPending, isFalse);
    await controller.disconnectForExit();
    expect(backend.lifecycle.closed, isTrue);
    expect(controller.busy, isFalse);
  });
}
