import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/rust/generated/api/vpn.dart' as vpn;
import 'package:shuvpn/core/rust/generated/connection/model.dart';

import '../tool/connected_desktop_observation.dart';

void main() {
  test(
    'fixture reads survive view disposal after native requests start',
    () async {
      final session = _Session();
      final reading = readFixtureSession(session);
      session.dispose();
      session.complete();
      final value = await reading;
      expect(value, {
        'state': 'connected',
        'upload': 11,
        'download': 17,
        'connections': 2,
        'tun': {'state': 'stopped'},
        'egress': {'active': false},
        'decisions': [
          {'outbound': 'DIRECT'},
        ],
      });
      expect(session.requests, ['snapshot', 'tun', 'egress', 'decisions']);
      expect(session.isDisposed, isTrue);
    },
  );

  test('fixture does not read absent or already disposed views', () async {
    expect(await readFixtureSession(null), isNull);
    final session = _Session()..dispose();
    expect(await readFixtureSession(session), isNull);
    expect(session.requests, isEmpty);
  });

  test('fixture preserves native observation failures', () async {
    final session = _Session();
    final failure = StateError('native observation failed');
    final reading = readFixtureSession(session);
    final checked = expectLater(reading, throwsA(same(failure)));
    session.tun.completeError(failure);
    session.complete();
    await checked;
  });
}

final class _Session implements vpn.VpnSession {
  final requests = <String>[];
  final event = Completer<VpnEvent>();
  final tun = Completer<String>();
  final egress = Completer<String>();
  final records = Completer<String>();
  @override
  bool isDisposed = false;

  Future<T> _request<T>(String name, Completer<T> response) {
    if (isDisposed) throw StateError('view already disposed');
    requests.add(name);
    return response.future;
  }

  @override
  Future<VpnEvent> snapshot() => _request('snapshot', event);
  @override
  Future<String> tunStatus() => _request('tun', tun);
  @override
  Future<String> egressStatus() => _request('egress', egress);
  @override
  Future<String> decisions() => _request('decisions', records);
  @override
  void dispose() => isDisposed = true;

  void complete() {
    event.complete(
      VpnEvent(
        kind: VpnEventKind.state,
        state: VpnState.connected,
        uploadBytes: BigInt.from(11),
        downloadBytes: BigInt.from(17),
        uploadBps: 0,
        downloadBps: 0,
        connections: BigInt.from(2),
        unrouted: BigInt.zero,
        tunnelFailures: BigInt.zero,
        dnsServers: const [],
      ),
    );
    if (!tun.isCompleted) tun.complete('{"state":"stopped"}');
    egress.complete('{"active":false}');
    records.complete('[{"outbound":"DIRECT"}]');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
