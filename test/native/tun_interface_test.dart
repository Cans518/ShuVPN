import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/rust/generated/api/tun.dart';
import 'package:shuvpn/core/rust/generated/platform/model.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('TUN input errors and platform capability cross the real bridge before OS changes', () async {
    await RustCore.initialize();
    final valid = <String, Object?>{
      'name': 'shuvpnffi',
      'mtu': 1400,
      'addresses': ['10.250.0.2/32'],
      'routes': ['10.21.0.0/16'],
      'protected_ips': ['192.0.2.10'],
    };
    for (final edit in <Map<String, Object?>>[
      {'name': 'eth0'},
      {
        'routes': ['0.0.0.0/0'],
      },
      {
        'addresses': ['192.0.2.10/32'],
      },
      {'full_capture': true},
    ]) {
      await expectLater(
        TunInterface.create(planJson: jsonEncode({...valid, ...edit})),
        throwsA(isA<PlatformFailure>()),
      );
    }
    if (Platform.isWindows) {
      final driver = jsonDecode(await tunDriverInfo()) as Map<String, dynamic>;
      expect(driver['backend'], 'wintun');
      expect(driver['version'], '0.14.1');
      expect(
        driver['sha256'],
        'E5DA8447DC2C320EDC0FC52FA01885C103DE8C118481F683643CACC3220DAFCE',
      );
      expect(driver['capture'], 'split only');
      expect(driver['sessionDataPlaneReady'], isTrue);
      expect(driver['available'], isA<bool>());
      expect(driver['helperReady'], isA<bool>());
      expect(driver['elevated'], isA<bool>());
      // Only probe creation when permission is denied before any OS mutation.
      if (driver['elevated'] == true) return;
      await expectLater(
        TunInterface.create(planJson: jsonEncode(valid)),
        throwsA(
          isA<PlatformFailure>().having(
            (error) => error.code,
            'code',
            'permission',
          ),
        ),
      );
    } else if (Platform.isLinux) {
      final driver = jsonDecode(await tunDriverInfo()) as Map<String, dynamic>;
      expect(driver['backend'], 'native-linux');
      expect(driver['wintun'], isFalse);
    }
  });
}
