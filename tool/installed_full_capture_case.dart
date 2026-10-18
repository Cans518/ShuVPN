import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/desktop_connection.dart'
    as owner;
import 'package:shuvpn/core/rust/generated/api/routing.dart';
import 'package:shuvpn/core/rust/generated/api/vpn.dart';
import 'package:shuvpn/core/rust/generated/connection/desktop_services.dart';
import 'package:shuvpn/core/rust/generated/connection/model.dart';
import 'package:shuvpn/core/rust/generated/connection/owner.dart';
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/generated/platform/model.dart';
import 'package:shuvpn/core/rust/rust_connection.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

const _library = '/opt/shuvpn/lib/libshuvpn_core.so';
const _helper = '/usr/lib/shuvpn/shuvpn_tun_helper';
const _basic = [
  'IP-CIDR,10.21.0.0/16,SHU-VPN,no-resolve',
  'IP-CIDR6,2001:db8:21::/64,SHU-VPN,no-resolve',
  'MATCH,DIRECT',
];

void registerInstalledFullCapture(String path) {
  test(
    'installed default core and fixed helper capture ordinary-user traffic',
    () async {
      final config =
          jsonDecode(await File(path).readAsString()) as Map<String, dynamic>;
      final process = await File('/proc/self/status').readAsString();
      String field(String name) => RegExp(
        '^$name:\\s*(.+)\$',
        multiLine: true,
      ).firstMatch(process)!.group(1)!.trim();
      expect(
        field('Uid').split(RegExp(r'\s+')).map(int.parse),
        everyElement(config['uid']),
      );
      expect(config['uid'], isNot(0));
      for (final name in ['CapInh', 'CapPrm', 'CapEff', 'CapAmb']) {
        expect(int.parse(field(name), radix: 16), 0, reason: name);
      }
      expect(field('NoNewPrivs'), '0');
      expect(Platform.environment['SHUVPN_LINUX_TUN_HELPER'], isNull);
      for (final kind in ['net', 'mnt']) {
        final current = (await Process.run('/usr/bin/readlink', [
          '/proc/self/ns/$kind',
        ])).stdout.toString().trim();
        expect(current, config['inside_$kind']);
        expect(current, isNot(config['parent_$kind']));
      }
      Future<void> verifyFiles() async {
        expect(
          sha256.convert(await File(_library).readAsBytes()).toString(),
          config['library_sha256'],
        );
        expect(
          sha256.convert(await File(_helper).readAsBytes()).toString(),
          config['helper_sha256'],
        );
      }

      await verifyFiles();
      await RustCore.initialize(
        externalLibrary: ExternalLibrary.open(_library),
      );
      addTearDown(ShuRustLib.dispose);
      expect(await File('/proc/self/maps').readAsString(), contains(_library));
      final fixture = await Process.start(config['fixture'] as String, ['udp']);
      final fixtureErrors = fixture.stderr.transform(utf8.decoder).join();
      addTearDown(() async {
        await fixture.stdin.close();
        expect(
          await fixture.exitCode.timeout(
            const Duration(seconds: 5),
            onTimeout: () {
              fixture.kill();
              return -1;
            },
          ),
          0,
          reason: await fixtureErrors,
        );
      });
      final raw = await fixture.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .first
          .timeout(const Duration(seconds: 5));
      final plan = jsonDecode(raw) as Map<String, dynamic>;
      (plan['routes'] as List<dynamic>).removeWhere(
        (dynamic route) => (route['host'] as String).startsWith('*.'),
      );
      expect(
        (plan['routes'] as List<dynamic>).every(
          (dynamic route) =>
              route['host'] == '10.21.0.0/16' ||
              route['host'] == '2001:db8:21::/64',
        ),
        isTrue,
      );
      plan['dnsServers'] = <String>[];
      final ruleDirectory = await Directory(config['work'] as String)
          .createTemp('routing-');
      final rules = await RoutingManager.open(directory: ruleDirectory.path);
      addTearDown(() async {
        rules.dispose();
        await ruleDirectory.delete(recursive: true);
      });
      final probe = _InstalledCase(config, rules);
      final faults = <Map<String, dynamic>>[];
      final bindings = <String, dynamic>{};
      await probe.apply(_basic);
      for (final phase in ['automatic', 'manual-crash', 'manual-normal']) {
        final authority = await owner.DesktopConnectionOwner.open();
        addTearDown(() async {
          if (!authority.isDisposed) {
            await authority.shutdown();
            authority.dispose();
          }
        });
        final session = await authority.connectPlan(
          planJson: jsonEncode(plan),
          timeoutMs: 3000,
          startup: owner.ConnectionStartup(
            proxies: const ProxyStartup(
              httpEnabled: true,
              httpAddress: '127.0.0.1',
              httpPort: 0,
              socksEnabled: true,
              socksAddress: '127.0.0.1',
              socksPort: 0,
              systemProxyEnabled: false,
            ),
            tun: TunStartup(
              enabled: phase == 'automatic',
              mtu: 1500,
              physicalInterface: 'shuphys',
              physicalDns: '10.99.0.53',
              captureMode: CaptureMode.full,
            ),
          ),
        );
        addTearDown(() async {
          if (!session.isDisposed) {
            await session.finishStop();
            session.dispose();
          }
        });
        await session.attachRouting(manager: rules);
        final adapter = await RustConnection.create(session);
        addTearDown(adapter.disposeView);
        final report = await session.startupServices();
        expect(report!.failures, isEmpty);
        final ports = [report.proxies.http!.port, report.proxies.socks!.port];
        if (phase == 'manual-normal') {
          bindings['before'] = jsonDecode(await session.egressStatus());
        }
        if (phase != 'automatic') {
          final started = await adapter.startTun(
            mtu: 1500,
            physicalInterface: 'shuphys',
            physicalDns: '10.99.0.53',
            captureMode: CaptureMode.full,
          );
          expect(started.running, isTrue, reason: started.diagnostic);
          expect(started.captureMode, CaptureMode.full);
        }
        final status =
            jsonDecode(await session.tunStatus()) as Map<String, dynamic>;
        expect(status['capture_mode'], 'full');
        expect(status['state'], 'running');
        final name =
            (status['interface'] as Map<String, dynamic>)['name'] as String;
        final preview = await adapter.previewTun(
          1,
          physicalDns: 'invalid',
          physicalInterface: 'missing-interface',
          captureMode: CaptureMode.campus,
        );
        expect(preview.captureMode, CaptureMode.full);
        expect(preview.mtu, 1400);
        expect(preview.physicalDnsServers, ['10.99.0.53']);
        expect(preview.protectedAddresses, contains('192.0.2.1'));
        await probe.barrier('$phase-ready', name);
        await probe.udp('10.22.0.5', probe.ports[0], '192.0.2.1');
        await probe.udp('2001:db8:22::5', probe.ports[1], '2001:db8:ffff::1');
        if (phase == 'automatic') {
          await probe.matrix(session);
          await probe.barrier('coverage-change', name);
        } else if (phase == 'manual-crash') {
          await probe.barrier('helper-crash', name);
        } else {
          final stopped = await adapter.stopTun();
          expect(stopped.state, 'stopped');
          expect(stopped.cleanupPending, isFalse);
          expect((await session.snapshot()).state, VpnState.connected);
          final restored =
              jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
          expect(restored['active'], isFalse);
          expect(restored['pending_cleanup'], isFalse);
          List<String> controls(Map<String, dynamic> status) =>
              (status['sockets'] as List<dynamic>)
                  .where((dynamic socket) => socket['peer'] == '127.0.0.1')
                  .map(jsonEncode)
                  .toList()
                ..sort();
          final original = controls(bindings['before'] as Map<String, dynamic>);
          expect(original, isNotEmpty);
          expect(controls(restored), original);
          bindings['after'] = restored;
        }
        if (phase != 'manual-normal') {
          final deadline = DateTime.now().add(const Duration(seconds: 8));
          while ((await session.snapshot()).state != VpnState.failed) {
            if (DateTime.now().isAfter(deadline)) {
              throw StateError('fault was not reported');
            }
            await Future<void>.delayed(const Duration(milliseconds: 20));
          }
          final failed = await session.snapshot();
          final snapshot = authority.snapshot();
          expect(failed.message, isNotEmpty);
          expect(snapshot.phase, OwnerPhase.failed);
          expect(snapshot.errorMessage, failed.message);
          if (phase == 'automatic') {
            expect(failed.message, contains('完整捕获'));
          }
          faults.add({
            'phase': phase,
            'session_message': failed.message,
            'owner_message': snapshot.errorMessage,
          });
        }
        final diagnostic = await authority.shutdown();
        if (phase == 'manual-normal') {
          expect(diagnostic, isNull);
        }
        expect(authority.snapshot().closed, isTrue);
        expect(authority.snapshot().cleanupPending, isFalse);
        expect(authority.snapshot().runId, isNull);
        final egress =
            jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
        expect(egress['active'], isFalse);
        expect(egress['pending_cleanup'], isFalse);
        expect(
          (await Process.run('/usr/sbin/ip', [
            'link',
            'show',
            'dev',
            name,
          ])).exitCode,
          isNot(0),
        );
        for (final port in ports) {
          await expectLater(
            Socket.connect(
              '127.0.0.1',
              port,
              timeout: const Duration(seconds: 2),
            ),
            throwsA(isA<SocketException>()),
          );
        }
        await probe.barrier('$phase-cleanup', name);
        adapter.disposeView();
        authority.dispose();
      }
      await verifyFiles();
      await File(config['report'] as String).writeAsString(
        jsonEncode({
          'uid': config['uid'],
          'pid': pid,
          'library': _library,
          'helper': _helper,
          'library_sha256': config['library_sha256'],
          'helper_sha256': config['helper_sha256'],
          'phases': ['automatic', 'manual-crash', 'manual-normal'],
          'faults': faults,
          'control_bindings': bindings,
          'uid_fields': field('Uid'),
          'cap_eff': field('CapEff'),
          'no_new_privs': field('NoNewPrivs'),
          'full_direct_dual_stack_dns_https_reject_authorization_cleanup': true,
        }),
      );
      stdout.writeln('INSTALLED_DEFAULT_FULL_ADAPTER_DATA_AND_RECOVERY_PASSED');
    },
    timeout: const Timeout(Duration(seconds: 120)),
  );
}

class _InstalledCase {
  _InstalledCase(this.config, this.rules);
  final Map<String, dynamic> config;
  final RoutingManager rules;
  List<int> get ports => (config['ports'] as List<dynamic>).cast<int>();
  Future<void> apply(List<String> lines) async {
    final current = jsonDecode(await rules.current()) as Map<String, dynamic>;
    await rules.apply(
      profile: jsonEncode({'automatic_campus': false, 'rules': lines}),
      overrides: '{}',
      expectedVersion: BigInt.from(current['version'] as int),
    );
  }

  Future<void> barrier(String stage, String name) async {
    stdout.writeln(
      'INSTALLED_FULL_EVENT ${jsonEncode({'token': config['token'], 'stage': stage, 'pid': pid, 'interface': name})}',
    );
    final file = File('${config['barriers']}/$stage');
    final deadline = DateTime.now().add(const Duration(seconds: 12));
    while (!await file.exists()) {
      if (DateTime.now().isAfter(deadline)) {
        throw StateError('root did not confirm $stage');
      }
      await Future<void>.delayed(const Duration(milliseconds: 30));
    }
    expect(await file.readAsString(), config['token']);
  }

  Future<List<int>> datagram(
    String host,
    int port,
    String source,
    List<int> bytes,
  ) async {
    final socket = await RawDatagramSocket.bind(source, 0);
    final received = Completer<Datagram>();
    final subscription = socket.listen((event) {
      if (event == RawSocketEvent.read) {
        final packet = socket.receive();
        if (packet != null && !received.isCompleted) received.complete(packet);
      }
    });
    try {
      socket.send(bytes, InternetAddress(host), port);
      final packet = await received.future.timeout(const Duration(seconds: 5));
      expect(packet.address.address, host);
      expect(packet.port, port);
      return packet.data;
    } finally {
      socket.close();
      await subscription.cancel();
    }
  }

  Future<void> udp(String host, int port, String source) async {
    final payload = utf8.encode('installed-full-$host');
    expect(await datagram(host, port, source, payload), payload);
  }

  Future<void> tcp(String host, int port, {bool campus = false}) async {
    final socket = await Socket.connect(
      host,
      port,
      sourceAddress: InternetAddress(
        host.contains(':') ? '2001:db8:ffff::1' : '192.0.2.1',
      ),
      timeout: const Duration(seconds: 4),
    );
    final reader = StreamIterator<Uint8List>(socket);
    final buffer = <int>[];
    Future<List<int>> read(int count) async {
      while (buffer.length < count) {
        if (!await reader.moveNext().timeout(const Duration(seconds: 4))) {
          throw StateError('TCP ended early');
        }
        buffer.addAll(reader.current);
      }
      final value = buffer.sublist(0, count);
      buffer.removeRange(0, count);
      return value;
    }

    try {
      if (campus) expect(utf8.decode(await read(7)), 'prelude');
      final payload = List<int>.generate(campus ? 1024 : 65536, (i) => i % 251);
      socket.add(payload);
      await socket.flush();
      expect(await read(payload.length), payload);
      if (!campus) {
        await socket.close();
        expect(
          await reader.moveNext().timeout(const Duration(seconds: 4)),
          isFalse,
        );
      }
    } finally {
      socket.destroy();
      await reader.cancel();
    }
  }

  List<int> question(String host, int kind) => [
    2,
    6,
    1,
    0,
    0,
    1,
    0,
    0,
    0,
    0,
    0,
    0,
    for (final label in host.split('.')) ...[
      label.length,
      ...ascii.encode(label),
    ],
    0,
    0,
    kind,
    0,
    1,
  ];
  void answer(List<int> bytes, int code, {int? length}) {
    final header = ByteData.sublistView(Uint8List.fromList(bytes));
    expect(header.getUint16(0), 518);
    expect(header.getUint16(2) & 15, code);
    if (length != null) {
      expect(header.getUint16(6), 1);
      expect(
        bytes.sublist(bytes.length - length),
        InternetAddress(length == 4 ? '10.22.0.5' : '2001:db8:22::5')
            .rawAddress,
      );
    }
  }

  Future<void> dnsUdp(String host, int kind, {int code = 0}) async {
    answer(
      await datagram('8.8.8.8', 53, '192.0.2.1', question(host, kind)),
      code,
      length: code == 0 ? (kind == 1 ? 4 : 16) : null,
    );
  }

  Future<void> dnsTcp() async {
    final socket = await Socket.connect(
      '8.8.4.4',
      53,
      sourceAddress: InternetAddress('192.0.2.1'),
      timeout: const Duration(seconds: 4),
    );
    try {
      final bytes = question('truncated.fixture.test', 1);
      socket.add([bytes.length >> 8, bytes.length & 255, ...bytes]);
      await socket.flush();
      final response = <int>[];
      await for (final chunk in socket.timeout(const Duration(seconds: 5))) {
        response.addAll(chunk);
        if (response.length >= 2 &&
            response.length >= 2 + (response[0] << 8 | response[1])) {
          break;
        }
      }
      answer(response.sublist(2), 0, length: 4);
    } finally {
      socket.destroy();
    }
  }

  Future<void> blocked(VpnSession session, String action) async {
    final before =
        (jsonDecode(await session.decisions()) as List<dynamic>).length;
    await expectLater(
      Socket.connect(
        '10.22.0.5',
        ports[2],
        sourceAddress: InternetAddress('192.0.2.1'),
        timeout: const Duration(seconds: 4),
      ),
      throwsA(isA<SocketException>()),
    );
    final rows = (jsonDecode(await session.decisions()) as List<dynamic>)
        .skip(before)
        .cast<Map<String, dynamic>>();
    final row = rows.lastWhere(
      (row) =>
          row['flow']['port'] == ports[2] && row['flow']['inbound'] == 'tun',
    );
    expect(row['outbound'], isNull);
    expect(row['error'], isNotNull);
    if (action == 'dns_ambiguous') {
      expect(row['error'], action);
    } else {
      expect(row['decision']['action'], action);
    }
  }

  Future<void> matrix(VpnSession session) async {
    await udp('10.21.0.5', 9001, '192.0.2.1');
    await tcp('10.21.0.5', 443, campus: true);
    await tcp('2001:db8:21::5', 443, campus: true);
    await tcp('10.22.0.5', ports[2]);
    await tcp('2001:db8:22::5', ports[3]);
    final context = SecurityContext(withTrustedRoots: false)
      ..setTrustedCertificates(config['certificate'] as String);
    final secure = await SecureSocket.connect(
      '10.22.0.5',
      ports[4],
      context: context,
      timeout: const Duration(seconds: 4),
    );
    secure.add(
      ascii.encode(
        'GET / HTTP/1.1\r\nHost: provider.fixture.test\r\nConnection: close\r\n\r\n',
      ),
    );
    await secure.flush();
    expect(
      await secure
          .cast<List<int>>()
          .transform(utf8.decoder)
          .join()
          .timeout(const Duration(seconds: 5)),
      contains("payload:\n - '.physical.fixture.test'\n"),
    );
    secure.destroy();
    await expectLater(
      SecureSocket.connect(
        '10.22.0.5',
        ports[4],
        context: SecurityContext(withTrustedRoots: false),
        timeout: const Duration(seconds: 4),
      ),
      throwsA(isA<HandshakeException>()),
    );
    await dnsUdp('direct.fixture.test', 1);
    await dnsUdp('ipv6.fixture.test', 28);
    final rows = (jsonDecode(await session.decisions()) as List<dynamic>)
        .cast<Map<String, dynamic>>();
    for (final (port, network, action) in [
      (ports[0], 'udp', 'DIRECT'),
      (ports[1], 'udp', 'DIRECT'),
      (ports[2], 'tcp', 'DIRECT'),
      (ports[3], 'tcp', 'DIRECT'),
      (9001, 'udp', 'SHU-VPN'),
      (443, 'tcp', 'SHU-VPN'),
    ]) {
      expect(
        rows.any(
          (row) =>
              row['flow']['port'] == port &&
              row['flow']['network'] == network &&
              row['flow']['inbound'] == 'tun' &&
              row['outbound'] == action &&
              row['decision']['action'] == action,
        ),
        isTrue,
      );
    }
    await apply(['DOMAIN,direct.fixture.test,DIRECT', 'MATCH,REJECT']);
    await dnsUdp('direct.fixture.test', 1);
    await tcp('10.22.0.5', ports[2]);
    await barrier('before-blocked', '');
    await dnsTcp();
    await blocked(session, 'dns_ambiguous');
    await dnsUdp('badid.fixture.test', 1, code: 2);
    await apply(['DST-PORT,${ports[2]},REJECT', 'MATCH,DIRECT']);
    await blocked(session, 'REJECT');
    await apply(['IP-CIDR,10.22.0.5/32,SHU-VPN,no-resolve', 'MATCH,DIRECT']);
    await blocked(session, 'SHU-VPN');
    await barrier('after-blocked', '');
    await apply(_basic);
  }
}
