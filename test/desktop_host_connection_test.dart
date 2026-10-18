import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shuvpn/app/desktop_host.dart';
import 'package:shuvpn/core/account/account_center.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/connection/connection_status.dart';
import 'package:shuvpn/core/desktop/desktop_port.dart';
import 'package:shuvpn/core/settings/settings_store.dart';

import 'account_stub.dart';
import 'desktop_lifecycle_test.dart' show FakeDesktopPort;
import 'settings_stub.dart';

class _Commands extends ConnectionController {
  _Commands(super.settings);
  int disconnects = 0;
  int shutdowns = 0;
  @override
  ConnectionStatus get state => ConnectionStatus.connected;
  @override
  bool get canDisconnect => true;
  @override
  Future<void> disconnect() async => disconnects++;
  @override
  Future<void> disconnectForExit() async => shutdowns++;
}

void main() {
  testWidgets(
    'native tray disconnect preserves the frontend owner until quit',
    (tester) async {
      final settings = StubSettingsStore()..welcomeCompleted = true;
      final account = StubAccountCenter();
      final connection = _Commands(settings);
      final port = FakeDesktopPort();
      addTearDown(() {
        connection.dispose();
        account.dispose();
        settings.dispose();
      });
      await tester.pumpWidget(
        MultiProvider(
          providers: [
            ChangeNotifierProvider<SettingsStore>.value(value: settings),
            ChangeNotifierProvider<AccountCenter>.value(value: account),
            ChangeNotifierProvider<ConnectionController>.value(
              value: connection,
            ),
          ],
          child: MaterialApp(
            home: DesktopHost(
              port: port,
              navigate: (_) {},
              child: const SizedBox.shrink(),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(port.snapshots.last.canDisconnect, isTrue);
      port.command!(DesktopCommand.disconnect);
      await tester.pumpAndSettle();
      expect(connection.disconnects, 1);
      expect(connection.shutdowns, 0);
      port.command!(DesktopCommand.quit);
      await tester.pumpAndSettle();
      expect(connection.disconnects, 1);
      expect(connection.shutdowns, 1);
      expect(port.events, contains('window:destroy'));
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pumpAndSettle();
    },
  );
}
