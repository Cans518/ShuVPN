import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/connection/connection_observation.dart';

const flow = {
  'host': 'fixture.example',
  'ips': <String>[],
  'port': 443,
  'network': 'tcp',
  'inbound': 'socks',
};

void main() {
  test(
    'process snapshots preserve native identity and older records stay absent',
    () {
      Map<String, Object?> entry(Map<String, Object?> input) => {
        'flow': input,
        'decision': null,
        'dns': <Object?>[],
        'outbound': null,
      };
      expect(ConnectionObservation.decode(entry(flow)).process, isNull);
      final record = ConnectionObservation.decode(
        entry({
          ...flow,
          'process': {
            'pid': 518,
            'name': 'fixture',
            'path': '/opt/fixture',
            'source': 'linux-proc-socket-snapshot',
            'confidence': 'socket-snapshot',
          },
        }),
      );
      expect(record.process?.pid, 518);
      expect(record.process?.name, 'fixture');
      expect(record.process?.path, '/opt/fixture');
      expect(record.process?.source, 'linux-proc-socket-snapshot');
      expect(record.process?.confidence, 'socket-snapshot');
    },
  );

  test('authorization rejection keeps expected policy separate from actual outbound', () {
    final record = ConnectionObservation.decode({
      'flow': flow,
      'decision': {
        'version': 7,
        'rule_id': 'profile:0',
        'source': 'profile',
        'policy': '校园策略',
        'action': 'SHU-VPN',
        'reason': '首条命中',
      },
      'dns': <Object?>[],
      'outbound': null,
      'error': 'unrouted',
      'message': '规则选择校园隧道，但网关未授权目标',
    });
    expect(record.expectedAction, 'SHU-VPN');
    expect(record.hasDecision, isTrue);
    expect(record.outbound, isNull);
    expect(record.error, 'unrouted');
    expect(record.version, 7);
  });

  test('failure before a rule decision reads the top-level version without inventing a match', () {
    final record = ConnectionObservation.decode({
      'flow': flow,
      'version': 3,
      'decision': null,
      'dns': <Object?>[],
      'outbound': null,
      'error': 'dns',
      'message': '解析失败',
    });
    expect(record.version, 3);
    expect(record.hasDecision, isFalse);
    expect(record.ruleId, isNull);
    expect(record.expectedAction, isNull);
  });

  test('resolved addresses remain DNS evidence rather than an invented socket peer', () {
    final record = ConnectionObservation.decode({
      'flow': flow,
      'decision': {
        'version': 4,
        'rule_id': 'profile:1',
        'source': 'profile',
        'policy': 'DIRECT',
        'action': 'DIRECT',
        'reason': 'MATCH',
      },
      'dns': [
        {
          'host': 'fixture.example',
          'addresses': ['192.0.2.9'],
          'source': {'kind': 'campus', 'server': '10.99.0.53'},
          'cached': true,
          'ttl_seconds': 30,
          'warnings': ['fixture warning'],
        },
      ],
      'outbound': 'DIRECT',
      'error': null,
      'message': null,
    });
    expect(record.addresses, isEmpty);
    expect(record.dns.single.addresses, ['192.0.2.9']);
    expect(record.dns.single.server, '10.99.0.53');
    expect(
      () => record.dns.single.addresses.add('extra'),
      throwsUnsupportedError,
    );
  });
}
