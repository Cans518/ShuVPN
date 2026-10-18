import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/vpn.dart';
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/generated/platform/model.dart';
import 'package:shuvpn/core/rust/rust_connection.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

import 'installed_full_capture_case.dart';

// Run with flutter test in an isolated root network namespace. The Flutter
// runner is required because the production adapter uses the UI logging sink.
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final installed = Platform.environment['SHUVPN_INSTALLED_FULL_CONFIG'];
  if (installed != null) {
    registerInstalledFullCapture(installed);
    return;
  }

  test(
    'real desktop adapter previews, starts and stops both capture modes',
    () async {
      if (!Platform.isLinux) {
        throw StateError('Run in an isolated Linux network namespace');
      }
      final current = await Process.run('/usr/bin/readlink', [
        '/proc/self/ns/net',
      ]);
      final initial = await Process.run('/usr/bin/readlink', [
        '/proc/1/ns/net',
      ]);
      if (current.exitCode != 0 ||
          initial.exitCode != 0 ||
          current.stdout == initial.stdout) {
        throw StateError('Run in an isolated Linux network namespace');
      }
      Future<ProcessResult> ip(List<String> args) =>
          Process.run('/usr/sbin/ip', args);
      for (final args in [
        ['link', 'set', 'lo', 'up'],
        ['link', 'add', 'shuegress', 'type', 'dummy'],
        ['link', 'set', 'shuegress', 'up'],
        ['address', 'add', '192.0.2.1/24', 'dev', 'shuegress'],
        ['route', 'add', 'default', 'via', '192.0.2.254', 'dev', 'shuegress'],
      ]) {
        final result = await ip(args);
        expect(result.exitCode, 0, reason: '${result.stderr}');
      }
      final baseline = (await ip(['-j', 'route', 'show', 'table', 'main']))
          .stdout;
      await RustCore.initialize(
        externalLibrary: ExternalLibrary.open(
          File(
            Platform.environment['SHUVPN_ADAPTER_LIBRARY'] ??
                'rust/target/release/libshuvpn_core.so',
          ).absolute.path,
        ),
      );
      addTearDown(ShuRustLib.dispose);
      final fixture = await Process.start(
        File('rust/target/release/examples/proxy_fixture').absolute.path,
        ['udp'],
      );
      final errors = fixture.stderr.transform(utf8.decoder).join();
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
          reason: await errors,
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
      final session = await VpnSession.connect(
        planJson: jsonEncode(plan),
        timeoutMs: 3000,
      );
      addTearDown(() async {
        if (!session.isDisposed) {
          await session.finishStop();
          session.dispose();
        }
      });
      final adapter = await RustConnection.create(session);
      addTearDown(adapter.close);
      for (final mode in [CaptureMode.full, CaptureMode.campus]) {
        final prepared = await adapter.previewTun(
          1500,
          physicalDns: '192.0.2.53',
          physicalInterface: 'shuegress',
          captureMode: mode,
        );
        expect(prepared.captureMode, mode);
        expect(prepared.physicalDnsServers, ['192.0.2.53']);
        if (mode == CaptureMode.full) {
          expect(prepared.capturesCampusDns, isTrue);
          expect(prepared.protectedAddresses, contains('192.0.2.1'));
        }
        final started = await adapter.startTun(
          mtu: 1500,
          physicalInterface: 'shuegress',
          physicalDns: '192.0.2.53',
          captureMode: mode,
        );
        expect(started.running, isTrue, reason: started.diagnostic);
        expect(started.captureMode, mode);
        expect(started.mtu, 1400);
        final running = await adapter.previewTun(
          1,
          physicalDns: 'invalid',
          physicalInterface: 'missing-interface',
          captureMode: mode == CaptureMode.full
              ? CaptureMode.campus
              : CaptureMode.full,
        );
        expect(running.captureMode, mode);
        expect(running.routes, started.routes);
        expect(running.mtu, started.mtu);
        final socket = await RawDatagramSocket.bind('10.99.0.2', 0);
        final received = Completer<Datagram>();
        final events = socket.listen((event) {
          if (event == RawSocketEvent.read) {
            final reply = socket.receive();
            if (reply != null && !received.isCompleted) {
              received.complete(reply);
            }
          }
        });
        try {
          final payload = 'adapter-${mode.name}';
          socket.send(utf8.encode(payload), InternetAddress('10.21.0.5'), 9001);
          final reply = await received.future.timeout(
            const Duration(seconds: 3),
          );
          expect(utf8.decode(reply.data), payload);
          expect(reply.address.address, '10.21.0.5');
          expect(reply.port, 9001);
        } finally {
          socket.close();
          await events.cancel();
        }
        final stopped = await adapter.stopTun();
        expect(stopped.state, 'stopped');
        expect(stopped.cleanupPending, isFalse);
        expect(
          (await ip(['link', 'show', 'dev', started.name!])).exitCode,
          isNot(0),
        );
        expect(
          (await ip(['-j', 'route', 'show', 'table', 'main'])).stdout,
          baseline,
        );
      }
      stdout.writeln(
        'REAL_CAPTURE_ADAPTER_FULL_AND_CAMPUS_KERNEL_PREVIEW_START_STOP_PASSED',
      );
    },
    timeout: const Timeout(Duration(seconds: 60)),
  );
}
