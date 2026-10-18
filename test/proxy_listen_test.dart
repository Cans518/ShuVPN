import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/connection/proxy_listen.dart';

void main() {
  test('normalized listener values retain exposure presentation', () {
    expect(ShuProxyListen.loopback.isLoopback, isTrue);
    expect(ShuProxyListen.loopback.exposesToNetwork, isFalse);
    for (final value in [
      ShuProxyListen.anyNetwork,
      ShuProxyListen.anyNetworkV6,
      const ShuProxyListen.fromBackend('192.168.99.144'),
    ]) {
      expect(value.exposesToNetwork, isTrue);
    }
    expect(ShuProxyListen.anyNetwork.allInterfaces, isTrue);
    expect(ShuProxyListen.anyNetworkV6.allInterfaces, isTrue);
    expect(
      const ShuProxyListen.fromBackend('192.168.99.144').allInterfaces,
      isFalse,
    );
  });
  test('listener display equality uses the native address value', () {
    expect(
      const ShuProxyListen.fromBackend('10.0.0.1'),
      const ShuProxyListen.fromBackend('10.0.0.1'),
    );
    expect(
      const ShuProxyListen.fromBackend('10.0.0.1'),
      isNot(ShuProxyListen.loopback),
    );
  });
}
