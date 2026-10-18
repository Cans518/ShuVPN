import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/auth/account_authentication.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/connection/connection_status.dart';
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_connection.dart';

import 'settings_stub.dart';
import 'native_backend_stub.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'an absent native backend rejects commands without starting a fallback',
    () async {
      expect(ShuRustLib.instance.initialized, isFalse);
      SharedPreferences.setMockInitialValues({});
      final settings = await StubSettingsStore.load();
      addTearDown(settings.dispose);
      final controller =
          ConnectionController(
              settings,
              draft: const ConnectionDraft(server: 'https://['),
            )
            ..ssoSessionProvider = () =>
                throw StateError('must not read authentication');
      addTearDown(controller.dispose);
      expect(await controller.connect(), isFalse);
      expect(controller.errorCode, ConnectionErrorCode.unsupported);
      expect(controller.errorMessage, contains('原生核心未配置'));
      expect(await controller.startHttpProxy(), isFalse);
      expect(await controller.startSocksProxy(), isFalse);
      expect(controller.proxyRunning, isFalse);
      expect(controller.busy, isFalse);
      expect(ShuRustLib.instance.initialized, isFalse);
    },
  );

  test(
    'rejecting a legacy account does not initialize the native core',
    () async {
      expect(ShuRustLib.instance.initialized, isFalse);
      final attempt = RustConnectionBackend(systemProxyDirectory: '\u0000');
      addTearDown(attempt.dispose);
      await expectLater(
        attempt.connect(
          auth: const _LegacyAccount(),
          server: 'https://fixture.invalid/',
          timeout: const Duration(seconds: 1),
          proxies: noNativeServices,
        ),
        throwsA(
          isA<StateError>().having(
            (failure) => failure.message,
            'message',
            contains('原生账户'),
          ),
        ),
      );
      expect(ShuRustLib.instance.initialized, isFalse);
    },
  );

  test('a disposed backend rejects before native initialization', () async {
    expect(ShuRustLib.instance.initialized, isFalse);
    final attempt = RustConnectionBackend()..dispose();
    addTearDown(attempt.dispose);
    await expectLater(
      attempt.connect(
        auth: const _LegacyAccount(),
        server: 'https://fixture.invalid/',
        timeout: const Duration(seconds: 1),
        proxies: noNativeServices,
      ),
      throwsA(
        isA<StateError>().having(
          (failure) => failure.message,
          'message',
          contains('已关闭'),
        ),
      ),
    );
    expect(ShuRustLib.instance.initialized, isFalse);
  });
}

class _LegacyAccount implements AccountAuthentication {
  const _LegacyAccount();

  @override
  bool get hasSession => true;
}
