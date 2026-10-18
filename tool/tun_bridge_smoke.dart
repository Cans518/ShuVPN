import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/tun.dart';
import 'package:shuvpn/core/rust/generated/api/vpn.dart' as vpn;
import 'package:shuvpn/core/rust/generated/platform/model.dart';
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if (args.length > 1) {
    throw ArgumentError(
      'Usage: dart run tool/tun_bridge_smoke.dart [native-library]',
    );
  }
  if (!Platform.isLinux) {
    throw StateError('This bridge probe requires an isolated Linux namespace');
  }
  final currentNs = await Process.run('/usr/bin/readlink', [
    '/proc/self/ns/net',
  ]);
  final initialNs = await Process.run('/usr/bin/readlink', ['/proc/1/ns/net']);
  if (currentNs.exitCode != 0 ||
      initialNs.exitCode != 0 ||
      currentNs.stdout == initialNs.stdout) {
    throw StateError(
      'Run this probe inside unshare -n, never in the host namespace',
    );
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(
      File(args.isEmpty ? 'rust/target/release/libshuvpn_core.so' : args.single)
          .absolute
          .path,
    ),
  );
  final device = await TunInterface.create(
    planJson: jsonEncode({
      'name': 'shuvpnffi',
      'mtu': 1400,
      'addresses': ['10.251.0.2/32'],
      'routes': ['10.22.0.0/16'],
      'protected_ips': ['192.0.2.10'],
    }),
  );
  try {
    final socket = await RawDatagramSocket.bind('10.251.0.2', 0);
    try {
      socket.send(utf8.encode('ffi-tun'), InternetAddress('10.22.0.5'), 9001);
      final deadline = DateTime.now().add(const Duration(seconds: 3));
      var captured = false;
      while (DateTime.now().isBefore(deadline)) {
        final packet = await device.readPacket(waitMs: 500);
        if (packet != null &&
            packet.length >= 28 &&
            packet[0] >> 4 == 4 &&
            packet[9] == 17) {
          if (utf8.decode(packet.sublist(28)) == 'ffi-tun') {
            captured = true;
            break;
          }
        }
      }
      if (!captured) {
        throw StateError('Kernel packet did not cross the real TUN bridge');
      }
      while (await device.readPacket(waitMs: 10) != null) {}
      final pending = device
          .readPacket(waitMs: 5000)
          .then(
            (_) => false,
            onError: (Object error) =>
                error is PlatformFailure && error.code == 'closed',
          );
      await Future<void>.delayed(const Duration(milliseconds: 30));
      await device.close();
      if (!await pending) {
        throw StateError('Close did not cancel the pending bridge read');
      }
      final info = jsonDecode(await device.info()) as Map<String, Object?>;
      if (info['closed'] != true) {
        throw StateError('Bridge interface did not report closure');
      }
      final absent = await Process.run('/usr/sbin/ip', [
        'link',
        'show',
        'dev',
        'shuvpnffi',
      ]);
      if (absent.exitCode == 0) {
        throw StateError('Bridge-owned interface leaked');
      }
      stdout.writeln('TUN_REAL_FFI_PACKET_AND_CLOSE_PASSED');
    } finally {
      socket.close();
    }
    await _routedKernelPacket();
  } finally {
    await device.close();
    device.dispose();
    ShuRustLib.dispose();
  }
}

Future<void> _routedKernelPacket() async {
  final loopback = await Process.run('/usr/sbin/ip', [
    'link',
    'set',
    'lo',
    'up',
  ]);
  if (loopback.exitCode != 0) {
    throw StateError('Isolated loopback setup failed: ${loopback.stderr}');
  }
  for (final command in <List<String>>[
    ['link', 'add', 'shuegress', 'type', 'dummy'],
    ['link', 'set', 'shuegress', 'up'],
    ['address', 'add', '192.0.2.1/24', 'dev', 'shuegress'],
    ['route', 'add', 'default', 'via', '192.0.2.254', 'dev', 'shuegress'],
  ]) {
    final configured = await Process.run('/usr/sbin/ip', command);
    if (configured.exitCode != 0) {
      throw StateError(
        'Isolated default-interface setup failed: ${configured.stderr}',
      );
    }
  }
  final process = await Process.start(
    File('rust/target/release/examples/proxy_fixture').absolute.path,
    ['udp'],
  );
  final errors = process.stderr.transform(utf8.decoder).join();
  vpn.VpnSession? session;
  try {
    final plan = await process.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .first
        .timeout(const Duration(seconds: 5));
    final resources = jsonDecode(plan) as Map<String, dynamic>;
    // This fixture has no authorized DNS. Keep its explicit campus IP routes
    // so the desktop builder can exercise the IP-only capture contract.
    (resources['routes'] as List<dynamic>).removeWhere(
      (dynamic route) => (route['host'] as String).startsWith('*.'),
    );
    session = await vpn.VpnSession.connect(
      planJson: jsonEncode(resources),
      timeoutMs: 3000,
    );
    final installed = jsonDecode(
      await session.startDesktopTun(mtu: 1500, physicalInterface: ''),
    ) as Map<String, dynamic>;
    final interfaceName = installed['name'] as String;
    final status =
        jsonDecode(await session.tunStatus()) as Map<String, dynamic>;
    if (status['state'] != 'running' ||
        status['physical_interface'] != 'shuegress' ||
        installed['mtu'] != 1400) {
      throw StateError(
        'Default desktop TUN plan or selected interface mismatch',
      );
    }
    final egress =
        jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
    if (egress['active'] != true || egress['interface'] != 'shuegress') {
      throw StateError('Real bridge did not publish process egress protection');
    }
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
      socket.send(
        utf8.encode('ffi-routed-tun'),
        InternetAddress('10.21.0.5'),
        9001,
      );
      final reply = await received.future.timeout(const Duration(seconds: 3));
      if (utf8.decode(reply.data) != 'ffi-routed-tun' ||
          reply.address.address != '10.21.0.5' ||
          reply.port != 9001) {
        throw StateError(
          'Routed FFI packet did not return to the kernel socket',
        );
      }
    } finally {
      socket.close();
      await events.cancel();
    }
    final tcp = await Socket.connect(
      '10.21.0.5',
      443,
      sourceAddress: InternetAddress('10.99.0.2'),
      timeout: const Duration(seconds: 3),
    );
    final reader = StreamIterator<List<int>>(tcp);
    final buffered = <int>[];
    Future<List<int>> read(int count) async {
      while (buffered.length < count) {
        if (!await reader.moveNext().timeout(const Duration(seconds: 3))) {
          throw StateError('Kernel TCP FFI stream ended early');
        }
        buffered.addAll(reader.current);
      }
      final result = buffered.sublist(0, count);
      buffered.removeRange(0, count);
      return result;
    }

    try {
      if (utf8.decode(await read(7)) != 'prelude') {
        throw StateError('Kernel TCP FFI did not use the signed campus stream');
      }
      final payload = List<int>.generate(64 * 1024, (index) => index % 251);
      tcp.add(payload);
      await tcp.flush();
      final reply = await read(payload.length);
      for (var index = 0; index < reply.length; index++) {
        if (reply[index] != payload[index]) {
          throw StateError('Kernel TCP FFI payload changed at $index');
        }
      }
    } finally {
      tcp.destroy();
      await reader.cancel();
    }
    final decisions = jsonDecode(await session.decisions()) as List<dynamic>;
    if (!decisions.any((dynamic event) => event['flow']['network'] == 'tcp') ||
        !decisions.any((dynamic event) => event['flow']['network'] == 'udp') ||
        decisions.any((dynamic event) => event['outbound'] != 'SHU-VPN')) {
      throw StateError(
        'Kernel FFI flow did not use the authorized campus path',
      );
    }
    final diagnostic = await session.finishStop();
    if (diagnostic != null) {
      throw StateError('Default TUN cleanup required recovery: $diagnostic');
    }
    final stoppedEgress =
        jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
    if (stoppedEgress['active'] != false ||
        stoppedEgress['pending_cleanup'] != false) {
      throw StateError('Session stop left process egress protection active');
    }
    final absent = await Process.run('/usr/sbin/ip', [
      'link',
      'show',
      'dev',
      interfaceName,
    ]);
    if (absent.exitCode == 0) {
      throw StateError('Session stop returned before bridge TUN cleanup');
    }
    stdout.writeln(
      'TUN_REAL_FFI_ROUTED_KERNEL_TCP_UDP_AND_SESSION_STOP_PASSED',
    );
    stdout.writeln('DESKTOP_TUN_DEFAULT_PLAN_KERNEL_AND_CLEANUP_PASSED');
  } finally {
    try {
      if (session != null) {
        await session.finishStop();
        session.dispose();
      }
    } finally {
      await process.stdin.close();
      final exitCode = await process.exitCode.timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          process.kill();
          return -1;
        },
      );
      if (exitCode != 0) {
        throw StateError('Routed fixture exit failed: ${await errors}');
      }
    }
  }
}
