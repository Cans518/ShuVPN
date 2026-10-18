import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/desktop/desktop_port.dart';
import 'package:shuvpn/core/desktop/plugin_desktop_port.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('native menu reuses its structure, updates labels and disables absent protocols', () async {
    final calls = <MethodCall>[];
    const tray = MethodChannel('flutter/system_tray/tray');
    const menu = MethodChannel('flutter/system_tray/menu_manager');
    for (final channel in [tray, menu]) {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            calls.add(call);
            return true;
          });
      addTearDown(
        () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(channel, null),
      );
    }
    final port = PluginDesktopPort();
    await port.createTray((_) {}, (_) {});
    await port.updateTray(
      const TraySnapshot(
        status: 'idle',
        canConnect: true,
        canDisconnect: false,
        protocols: [
          TrayProtocol(
            id: 'atrust',
            label: 'aTrust',
            enabled: true,
            selected: true,
          ),
          TrayProtocol(
            id: 'openvpn',
            label: 'OpenVPN',
            enabled: true,
            selected: false,
          ),
        ],
      ),
    );
    calls.clear();
    await port.updateTray(
      const TraySnapshot(
        status: 'active',
        canConnect: false,
        canDisconnect: true,
        protocols: [
          TrayProtocol(
            id: 'atrust',
            label: '当前 aTrust',
            enabled: false,
            selected: true,
          ),
        ],
      ),
    );
    expect(calls.where((call) => call.method == 'CreateContextMenu'), isEmpty);
    expect(
      calls.any(
        (call) =>
            call.method == 'SetLabel' &&
            (call.arguments as Map<Object?, Object?>)['label'] == '当前 aTrust',
      ),
      isTrue,
    );
    expect(
      calls
          .where(
            (call) =>
                call.method == 'SetEnable' &&
                (call.arguments as Map<Object?, Object?>)['enabled'] == false,
          )
          .length,
      greaterThanOrEqualTo(3),
    );
    await port.destroyTray();
  });
}
