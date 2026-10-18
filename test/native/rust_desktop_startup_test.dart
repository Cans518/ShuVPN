import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/rust/generated/api/account.dart' as account;
import 'package:shuvpn/core/rust/generated/api/routing.dart' as routing;
import 'package:shuvpn/core/rust/generated/api/vpn.dart' as vpn;
import 'package:shuvpn/core/rust/generated/connection/model.dart' as model;
import 'package:shuvpn/core/rust/rust_core.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test('desktop connection validates budget, gateway and cancellation before recovery or authentication', () async {
    final root = await Directory.systemTemp.createTemp(
      'shuvpn-desktop-startup-',
    );
    addTearDown(() => root.delete(recursive: true));
    final recovery = await Directory('${root.path}/network').create();
    final journal = File('${recovery.path}/system-proxy.json');
    const original = 'invalid fixture recovery journal';
    await journal.writeAsString(original);
    final lock = File('${recovery.path}/.system-proxy.lock');
    final owner = await account.AccountSession.open(
      directory: '${root.path}/auth',
      legacyCookies: '[]',
      legacySnapshot: '{}',
    );
    addTearDown(() {
      owner.close();
      owner.dispose();
    });
    final manager = await routing.RoutingManager.open(
      directory: '${root.path}/rules',
    );
    addTearDown(manager.dispose);
    final attempt = vpn.VpnConnectAttempt();
    addTearDown(attempt.dispose);
    Future<vpn.VpnSession> connect(String gateway, int budget) =>
        attempt.connectDesktopAccount(
          account: owner,
          routing: manager,
          gatewayOrigin: gateway,
          strictAntiMitm: false,
          timeoutMs: budget,
          recoveryDirectory: recovery.path,
        );
    Matcher failure(String code) => throwsA(
      isA<model.VpnFailure>().having((error) => error.code, 'code', code),
    );
    await expectLater(connect('foreign.example', 0), failure('plan'));
    await expectLater(connect('https://[', 1000), failure('gateway'));
    attempt.cancel();
    await expectLater(connect('foreign.example', 1000), failure('cancelled'));
    expect(lock.existsSync(), isFalse);
    final recovering = vpn.VpnConnectAttempt();
    addTearDown(recovering.dispose);
    await expectLater(
      recovering.connectDesktopAccount(
        account: owner,
        routing: manager,
        gatewayOrigin: 'foreign.example',
        strictAntiMitm: false,
        timeoutMs: 1000,
        recoveryDirectory: recovery.path,
      ),
      failure('systemProxy'),
    );
    expect(lock.existsSync(), isTrue);
    expect(await journal.readAsString(), original);
    expect(owner.snapshot().signedIn, isFalse);
  });
}
