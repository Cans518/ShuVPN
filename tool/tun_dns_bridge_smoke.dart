import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/vpn.dart';
import 'package:shuvpn/core/rust/generated/api/routing.dart' as routing;
import 'package:shuvpn/core/rust/generated/api/desktop_connection.dart'
    as owner;
import 'package:shuvpn/core/rust/generated/connection/desktop_services.dart'
    as services;
import 'package:shuvpn/core/rust/generated/connection/model.dart' as model;
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  final installedHelper = args.contains('--installed-helper');
  final processRules = args.contains('--process-rules');
  final paths = args
      .where((arg) => arg != '--installed-helper' && arg != '--process-rules')
      .toList();
  if ((!installedHelper && paths.length > 1) ||
      (installedHelper && paths.length != 2)) {
    throw ArgumentError(
      'Usage: tun_dns_bridge_smoke [native-library] or '
      '--installed-helper <native-library> <gateway-fixture> [--process-rules]',
    );
  }
  if (!Platform.isLinux) {
    throw StateError('Run in an isolated Linux network namespace');
  }
  final current = await Process.run('/usr/bin/readlink', ['/proc/self/ns/net']);
  final initial = installedHelper
      ? Platform.environment['SHUVPN_INSTALL_PARENT_NET']
      : (await Process.run('/usr/bin/readlink', [
          '/proc/1/ns/net',
        ])).stdout.toString().trim();
  if (current.exitCode != 0 ||
      initial == null ||
      !RegExp(r'^net:\[\d+\]$').hasMatch(initial) ||
      current.stdout.toString().trim() == initial) {
    throw StateError('Run in an isolated Linux network namespace');
  }
  if (installedHelper) {
    final status = File('/proc/self/status').readAsStringSync();
    final capabilities = RegExp(
      r'^CapEff:\s*([0-9a-f]+)$',
      multiLine: true,
    ).firstMatch(status);
    final uid = RegExp(
      r'^Uid:\s+\d+\s+(\d+)',
      multiLine: true,
    ).firstMatch(status);
    if (capabilities == null ||
        int.parse(capabilities.group(1)!, radix: 16) != 0 ||
        uid == null ||
        uid.group(1) == '0') {
      throw StateError(
        'Installed-helper mode requires an ordinary UID and CapEff=0',
      );
    }
  } else {
    final setup = await Process.run('/usr/sbin/ip', [
      'link',
      'set',
      'lo',
      'up',
    ]);
    if (setup.exitCode != 0) {
      throw StateError('Loopback setup failed');
    }
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(
      File(
        paths.isEmpty ? 'rust/target/release/libshuvpn_core.so' : paths.first,
      ).absolute.path,
    ),
  );
  final gateway = await Process.start(
    File(
      installedHelper ? paths[1] : 'rust/target/release/examples/proxy_fixture',
    ).absolute.path,
    ['domain-dns'],
  );
  final errors = gateway.stderr.transform(utf8.decoder).join();
  VpnSession? session;
  owner.DesktopConnectionOwner? authority;
  routing.RoutingManager? rules;
  Directory? ruleDirectory;
  var interfaceName = 'shuvpndnsffi';
  final proxyPorts = <int>[];
  Object? primaryFailure;
  var cleanupFailed = false;
  var phase = 'gateway-plan';
  try {
    final plan = await gateway.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .first
        .timeout(const Duration(seconds: 5));
    if (installedHelper) {
      phase = 'desktop-connect';
      authority = await owner.DesktopConnectionOwner.open();
      session = await authority.connectPlan(
        planJson: plan,
        timeoutMs: 3000,
        startup: const owner.ConnectionStartup(
          proxies: services.ProxyStartup(
            httpEnabled: true,
            httpAddress: '127.0.0.1',
            httpPort: 0,
            socksEnabled: true,
            socksAddress: '127.0.0.1',
            socksPort: 0,
            systemProxyEnabled: false,
          ),
          tun: services.TunStartup(
            enabled: true,
            mtu: 1500,
            physicalInterface: '',
            physicalDns: '192.0.2.53',
          ),
        ),
      );
      phase = 'desktop-startup-report';
      final report = await session.startupServices();
      if (report == null) {
        throw StateError('Native owner did not publish its service report');
      }
      if (report.failures.isNotEmpty ||
          report.tun == null ||
          report.tun!.diagnostic != null ||
          report.proxies.http == null ||
          report.proxies.socks == null) {
        throw StateError(
          'Native service startup failed: ${report.tun?.diagnostic}; '
          '${report.failures.map((failure) => '${failure.service}: '
              '${failure.code}: ${failure.message}').join('; ')}',
        );
      }
      final status = jsonDecode(report.tun!.statusJson) as Map<String, dynamic>;
      final installed = status['interface'] as Map<String, dynamic>;
      interfaceName = installed['name'] as String;
      proxyPorts.addAll([
        report.proxies.http!.port,
        report.proxies.socks!.port,
      ]);
      for (final port in proxyPorts) {
        phase = 'proxy-listener-$port';
        final listener = await Socket.connect(
          '127.0.0.1',
          port,
          timeout: const Duration(seconds: 2),
        );
        listener.destroy();
      }
      if (installed['mtu'] != 1400 ||
          status['state'] != 'running' ||
          status['physical_interface'] != 'shuegress') {
        throw StateError(
          'Default desktop plan/physical interface mismatch: $status',
        );
      }
    } else {
      phase = 'session-connect';
      session = await VpnSession.connect(planJson: plan, timeoutMs: 3000);
      phase = 'tun-start';
      await session.startTun(
        planJson: jsonEncode({
          'name': interfaceName,
          'mtu': 1400,
          'addresses': ['10.99.0.2/32', '2001:db8::2/128'],
          'routes': ['10.21.0.0/16'],
          'protected_ips': ['127.0.0.1'],
          'dns_capture': true,
        }),
        physicalInterface: 'lo',
      );
    }
    if (processRules) {
      phase = 'process-rules';
      ruleDirectory = await Directory.systemTemp.createTemp(
        'shuvpn-process-rules-',
      );
      rules = await routing.RoutingManager.open(directory: ruleDirectory.path);
      final image = File('/proc/self/exe').resolveSymbolicLinksSync();
      await rules.apply(
        profile: jsonEncode({
          'automatic_campus': false,
          'rules': [
            'PROCESS-PATH,${jsonEncode(image)},SHU-VPN',
            'MATCH,REJECT',
          ],
        }),
        overrides: '{}',
        expectedVersion: BigInt.zero,
      );
      await session.attachRouting(manager: rules);
    }
    phase = 'kernel-dns';
    await _kernelDns();
    phase = 'dynamic-capture';
    final captures =
        jsonDecode(await session.captureStatus()) as Map<String, dynamic>;
    if (captures['active'] != true ||
        !(captures['dynamic_addresses'] as List<dynamic>).contains(
          '10.99.0.7',
        )) {
      throw StateError(
        'DNS response was sent before dynamic capture publication',
      );
    }
    phase = 'campus-dns-metadata';
    final resolution = jsonDecode(
      await session.resolveCampusDns(host: 'fixture-dns.shu.edu.cn'),
    ) as Map<String, dynamic>;
    if (resolution['source']['kind'] != 'campus' ||
        !(resolution['addresses'] as List<dynamic>).contains('10.99.0.7')) {
      throw StateError('Campus DNS metadata did not cross the real bridge');
    }
    phase = 'kernel-tcp-connect';
    final socket = await Socket.connect(
      '10.99.0.7',
      443,
      sourceAddress: InternetAddress('10.99.0.2'),
      timeout: const Duration(seconds: 3),
    );
    final reader = StreamIterator<Uint8List>(socket);
    final buffered = <int>[];
    Future<List<int>> read(int count) async {
      while (buffered.length < count) {
        if (!await reader.moveNext().timeout(const Duration(seconds: 3))) {
          throw StateError('Mapped TCP stream ended early');
        }
        buffered.addAll(reader.current);
      }
      final result = buffered.sublist(0, count);
      buffered.removeRange(0, count);
      return result;
    }

    try {
      phase = 'kernel-tcp-prelude';
      if (utf8.decode(await read(7)) != 'prelude') {
        throw StateError('Mapped flow did not use signed campus TCP');
      }
      socket.add(utf8.encode('dns-ffi'));
      await socket.flush();
      phase = 'kernel-tcp-echo';
      if (utf8.decode(await read(7)) != 'dns-ffi') {
        throw StateError('Mapped kernel TCP bytes changed');
      }
    } finally {
      socket.destroy();
      await reader.cancel();
    }
    phase = 'domain-decision';
    final decisions = jsonDecode(await session.decisions()) as List<dynamic>;
    if (!decisions.any(
      (dynamic event) =>
          event['flow']['host'] == 'fixture-dns.shu.edu.cn' &&
          event['outbound'] == 'SHU-VPN',
    )) {
      throw StateError(
        'Original domain identity did not cross the real bridge',
      );
    }
    if (processRules) {
      final image = File('/proc/self/exe').resolveSymbolicLinksSync();
      if (!decisions.any(
        (dynamic event) =>
            event['flow']['host'] == 'fixture-dns.shu.edu.cn' &&
            event['outbound'] == 'SHU-VPN' &&
            event['flow']['process']?['pid'] == pid &&
            event['flow']['process']?['path'] == image &&
            event['flow']['process']?['confidence'] == 'socket-snapshot',
      )) {
        throw StateError(
          'Real kernel TCP was not attributed to this executable',
        );
      }
      final name = File(image).uri.pathSegments.last;
      await rules!.apply(
        profile: jsonEncode({
          'automatic_campus': false,
          'rules': ['PROCESS-NAME,${jsonEncode(name)},REJECT', 'MATCH,SHU-VPN'],
        }),
        overrides: '{}',
        expectedVersion: BigInt.one,
      );
      await _kernelDns();
      try {
        final forbidden = await Socket.connect(
          '10.99.0.7',
          443,
          sourceAddress: InternetAddress('10.99.0.2'),
          timeout: const Duration(seconds: 3),
        );
        forbidden.destroy();
        throw StateError('Process rejection unexpectedly opened kernel TCP');
      } on SocketException {
        // Require a native rejected decision below, not just a failed socket.
      }
      final rejected = jsonDecode(await session.decisions()) as List<dynamic>;
      if (!rejected.any(
        (dynamic event) =>
            event['error'] == 'rejected' &&
            event['decision']?['action'] == 'REJECT' &&
            event['flow']['process']?['pid'] == pid &&
            event['flow']['process']?['name'] == name &&
            event['outbound'] == null,
      )) {
        throw StateError('Failed socket had no native process-rule rejection');
      }
      stdout.writeln(
        'TUN_REAL_FFI_PROCESS_NAME_PATH_SOCKET_OWNER_AND_REJECT_PASSED',
      );
    }
    phase = 'session-shutdown';
    if (installedHelper) {
      final diagnostic = await authority!.shutdown();
      if (diagnostic != null) {
        throw StateError('Desktop stop returned a diagnostic: $diagnostic');
      }
    } else {
      await session.stop();
    }
    phase = 'interface-removal';
    final absent = await Process.run('/usr/sbin/ip', [
      'link',
      'show',
      'dev',
      interfaceName,
    ]);
    if (absent.exitCode == 0) {
      throw StateError('Mapped bridge TUN leaked');
    }
    for (final port in proxyPorts) {
      phase = 'proxy-removal-$port';
      try {
        final leaked = await Socket.connect(
          '127.0.0.1',
          port,
          timeout: const Duration(seconds: 2),
        );
        leaked.destroy();
        throw StateError('Native startup proxy survived session shutdown');
      } on SocketException {
        // Native finishStop closed the listeners.
      }
    }
    stdout.writeln(
      'TUN_REAL_FFI_KERNEL_DNS_DYNAMIC_DOMAIN_TCP_AND_CLEANUP_PASSED',
    );
    if (installedHelper) {
      stdout.writeln(
        'LINUX_INSTALLED_DEFAULT_FFI_DESKTOP_TUN_KERNEL_AND_CLEANUP_PASSED',
      );
      stdout.writeln(
        'LINUX_NATIVE_SERVICE_STARTUP_HTTP_SOCKS_TUN_AND_CLEANUP_PASSED',
      );
      stdout.writeln('LINUX_PERSISTENT_OWNER_TUN_AND_EXIT_BARRIER_PASSED');
    }
  } catch (error, stack) {
    primaryFailure = error;
    stderr.writeln(
      'TUN_PROBE_PRIMARY_FAILURE phase=$phase ${_describe(error)}\n$stack',
    );
    if (session != null) {
      try {
        final snapshot = await session.snapshot();
        stderr.writeln(
          'TUN_PROBE_SESSION_FAILURE state=${snapshot.state} '
          'message=${snapshot.message}',
        );
        stderr.writeln('TUN_PROBE_DECISIONS ${await session.decisions()}');
      } catch (snapshotError) {
        stderr.writeln(
          'TUN_PROBE_SNAPSHOT_FAILURE ${_describe(snapshotError)}',
        );
      }
    }
    rethrow;
  } finally {
    try {
      try {
        if (authority != null) {
          final diagnostic = await authority.shutdown();
          if (diagnostic != null) throw StateError(diagnostic);
        } else {
          await session?.stop();
        }
      } catch (error, stack) {
        cleanupFailed = true;
        stderr.writeln('TUN_PROBE_CLEANUP_FAILURE ${_describe(error)}\n$stack');
        if (primaryFailure == null) rethrow;
      } finally {
        session?.dispose();
        authority?.dispose();
        rules?.dispose();
      }
    } finally {
      await gateway.stdin.close();
      await gateway.exitCode.timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          gateway.kill();
          return -1;
        },
      );
      final gatewayErrors = await errors;
      if ((primaryFailure != null || cleanupFailed) &&
          gatewayErrors.isNotEmpty) {
        stderr.writeln('TUN_PROBE_GATEWAY_STDERR\n$gatewayErrors');
      }
      ShuRustLib.dispose();
      await ruleDirectory?.delete(recursive: true);
    }
  }
}

String _describe(Object error) => error is model.VpnFailure
    ? '${error.code}: ${error.message}'
    : error.toString();

Future<void> _kernelDns() async {
  final dns = await RawDatagramSocket.bind('10.99.0.2', 0);
  final received = Completer<Datagram>();
  final events = dns.listen((event) {
    if (event == RawSocketEvent.read) {
      final reply = dns.receive();
      if (reply != null && !received.isCompleted) received.complete(reply);
    }
  });
  try {
    final query = <int>[0, 83, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for (final label in ['fixture-dns', 'shu', 'edu', 'cn']) {
      query.add(label.length);
      query.addAll(ascii.encode(label));
    }
    query.addAll([0, 0, 1, 0, 1]);
    dns.send(query, InternetAddress('10.99.0.53'), 53);
    final reply = await received.future.timeout(const Duration(seconds: 3));
    if (reply.data[1] != 83 ||
        reply.data[3] & 15 != 0 ||
        reply.data.sublist(reply.data.length - 4).join('.') != '10.99.0.7') {
      throw StateError('Kernel DNS did not return the validated campus IP');
    }
  } finally {
    dns.close();
    await events.cancel();
  }
}
