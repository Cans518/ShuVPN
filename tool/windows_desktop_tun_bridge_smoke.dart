import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:shuvpn/core/rust/generated/api/routing.dart';
import 'package:shuvpn/core/rust/generated/api/tun.dart';
import 'package:shuvpn/core/rust/generated/api/vpn.dart';
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

const _target4 = '192.0.2.77';
const _target6 = '2001:db8:77::7';
const _source4 = '10.99.0.2';
const _source6 = '2001:db8::2';

Future<void> main(List<String> args) async {
  if (!Platform.isWindows || args.length != 4) {
    throw ArgumentError(
      'Usage: windows_desktop_tun_bridge_smoke <default-dll> '
      '<proxy-fixture> <physical-interface> <explicit-ipv4-dns>',
    );
  }
  final watchdog = Timer(const Duration(seconds: 180), () {
    stderr.writeln('Default Windows FFI watchdog expired; cleanup unproven');
    exit(124);
  });
  try {
    await _verify(args);
  } finally {
    watchdog.cancel();
  }
}

Future<void> _verify(List<String> args) async {
  final dns = InternetAddress(args[3]);
  if (dns.type != InternetAddressType.IPv4 ||
      dns.isLoopback ||
      dns.isMulticast ||
      dns.address == '0.0.0.0') {
    throw ArgumentError('Choose a unicast non-loopback IPv4 DNS upstream');
  }
  final baseline = await _helperProcesses();
  final expectedLibrary = File(args[0]).resolveSymbolicLinksSync();
  final bundledLibrary = File.fromUri(
    File(Platform.resolvedExecutable).parent.uri.resolve('shuvpn_core.dll'),
  ).resolveSymbolicLinksSync();
  if (expectedLibrary != bundledLibrary) {
    throw ArgumentError(
      'Put the compiled probe beside the actual bundle library',
    );
  }
  await RustCore.initialize();
  final driver = jsonDecode(await tunDriverInfo()) as Map<String, dynamic>;
  if (driver['elevated'] != false ||
      driver['available'] != true ||
      driver['helperReady'] != true ||
      driver['sessionDataPlaneReady'] != true) {
    ShuRustLib.dispose();
    throw StateError(
      'Run an ordinary executable beside the packaged helper: $driver',
    );
  }
  final fixture = await Process.start(File(args[1]).absolute.path, ['udp']);
  final errors = fixture.stderr.transform(utf8.decoder).join();
  final directory = await Directory.systemTemp.createTemp('shuvpn-win-ffi-');
  final temporaryDirectory = directory.resolveSymbolicLinksSync();
  VpnSession? session;
  RoutingManager? manager;
  try {
    final encoded = await fixture.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .first
        .timeout(const Duration(seconds: 5));
    final plan = jsonDecode(encoded) as Map<String, dynamic>;
    plan['dnsServers'] = <String>[];
    final routes = plan['routes'] as List<dynamic>;
    routes.removeWhere(
      (dynamic route) => (route['host'] as String).startsWith('*.'),
    );
    for (final dynamic route in routes) {
      if (route['host'] == '10.21.0.0/16') route['host'] = '$_target4/32';
      if (route['host'] == '2001:db8:21::/64') route['host'] = '$_target6/128';
    }
    routes.add({
      'host': '${dns.address}/32',
      'protocol': 'tcp',
      'portMin': 53,
      'portMax': 53,
      'appId': 'test-app',
      'nodeGroupId': 'group',
      'addrPretend': false,
      'enableTcpPrefL3': false,
    });
    session = await VpnSession.connect(
      planJson: jsonEncode(plan),
      timeoutMs: 3000,
    );
    manager = await RoutingManager.open(directory: directory.path);
    await manager.apply(
      profile:
          "automatic_campus: false\nrules:\n"
          " - 'IP-CIDR,${dns.address}/32,DIRECT,no-resolve'\n - MATCH,SHU-VPN\n",
      overrides: '{}',
      expectedVersion: BigInt.zero,
    );
    await session.attachRouting(manager: manager);
    final preview = jsonDecode(
      await session.desktopTunPlan(mtu: 1500, physicalDns: dns.address),
    ) as Map<String, dynamic>;
    final expected = {'$_target4/32', '$_target6/128', '${dns.address}/32'};
    if ((preview['routes'] as List<dynamic>)
            .toSet()
            .difference(expected)
            .isNotEmpty ||
        (preview['routes'] as List<dynamic>).toSet().length !=
            expected.length ||
        preview['mtu'] != 1400 ||
        preview['dns_capture'] != false) {
      throw StateError(
        'Default FFI plan changed the approved capture scope: $preview',
      );
    }
    stdout.writeln('WINDOWS_DEFAULT_FFI_PLAN_VALIDATED ${jsonEncode(preview)}');
    await session.startDesktopTun(
      mtu: 1500,
      physicalInterface: args[2],
      physicalDns: dns.address,
    );
    final status =
        jsonDecode(await session.tunStatus()) as Map<String, dynamic>;
    if (status['state'] != 'running' || status.containsKey('helperIo')) {
      throw StateError(
        'Expected a running TUN from a library without tun-probe: $status',
      );
    }
    final helpers = (await _helperProcesses()).difference(baseline);
    if (helpers.length != 1) {
      throw StateError('Expected one owned helper: $helpers');
    }
    final payload = utf8.encode('default-ffi-campus');
    final echo = await _udp(InternetAddress(_target4), 9001, payload);
    if (utf8.decode(echo.data) != 'default-ffi-campus' ||
        echo.address.address != _target4 ||
        echo.port != 9001) {
      throw StateError('Default FFI campus UDP changed');
    }
    for (final pair in [(_source4, _target4), (_source6, _target6)]) {
      await _campusTcp(pair.$1, pair.$2);
    }
    final query = <int>[0x74, 0x11, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for (final label in ['shuvpn-probe', 'invalid']) {
      query.addAll([label.length, ...ascii.encode(label)]);
    }
    query.addAll([0, 0, 1, 0, 1]);
    final answer = await _udp(dns, 53, query);
    if (answer.address.address != dns.address || answer.port != 53) {
      throw StateError('Default DIRECT DNS peer changed');
    }
    _checkDns(answer.data);
    final tcp = await Socket.connect(
      dns,
      53,
      sourceAddress: InternetAddress(_source4),
      timeout: const Duration(seconds: 5),
    );
    final reader = _Reader(tcp);
    try {
      tcp.add([query.length >> 8, query.length & 255, ...query]);
      await tcp.flush();
      final prefix = await reader.read(2);
      _checkDns(await reader.read(prefix[0] * 256 + prefix[1]));
    } finally {
      tcp.destroy();
      await reader.close();
    }
    final diagnostic = await session.finishStop().timeout(
      const Duration(seconds: 8),
    );
    final stopped =
        jsonDecode(await session.tunStatus()) as Map<String, dynamic>;
    final egress =
        jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
    if (diagnostic != null ||
        stopped['state'] != 'stopped' ||
        stopped['cleanup_pending'] != false ||
        egress['active'] != false ||
        egress['pending_cleanup'] != false ||
        (await _helperProcesses()).intersection(helpers).isNotEmpty) {
      throw StateError('Default FFI cleanup was not confirmed');
    }
  } finally {
    try {
      if (session != null) {
        await session.finishStop().timeout(const Duration(seconds: 8));
        session.dispose();
      }
      manager?.dispose();
    } finally {
      await fixture.stdin.close();
      final code = await fixture.exitCode.timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          fixture.kill();
          return -1;
        },
      );
      final fixtureErrors = await errors;
      ShuRustLib.dispose();
      if (directory.resolveSymbolicLinksSync() != temporaryDirectory) {
        throw StateError(
          'The owned temporary directory changed before cleanup',
        );
      }
      await directory.delete(recursive: true);
      if (code != 0) throw StateError('Fixture failed: $fixtureErrors');
    }
  }
  stdout.writeln(
    'WINDOWS_DEFAULT_FFI_DESKTOP_TUN_CAMPUS_DIRECT_DNS_AND_HELPER_EXIT_PASSED',
  );
}

Future<Datagram> _udp(
  InternetAddress target,
  int port,
  List<int> payload,
) async {
  final socket = await RawDatagramSocket.bind(_source4, 0);
  final received = Completer<Datagram>();
  final events = socket.listen((event) {
    if (event == RawSocketEvent.read) {
      final packet = socket.receive();
      if (packet != null && !received.isCompleted) received.complete(packet);
    }
  });
  try {
    if (socket.send(payload, target, port) != payload.length) {
      throw StateError('Kernel did not accept the full datagram');
    }
    return await received.future.timeout(const Duration(seconds: 5));
  } finally {
    socket.close();
    await events.cancel();
  }
}

Future<void> _campusTcp(String source, String target) async {
  final socket = await Socket.connect(
    target,
    443,
    sourceAddress: InternetAddress(source),
    timeout: const Duration(seconds: 5),
  );
  final reader = _Reader(socket);
  try {
    if (ascii.decode(await reader.read(7)) != 'prelude') {
      throw StateError('Missing signed campus TCP prelude');
    }
    final payload = List<int>.generate(64 * 1024, (index) => index % 251);
    socket.add(payload);
    await socket.flush();
    final reply = await reader.read(payload.length);
    for (var index = 0; index < reply.length; index++) {
      if (reply[index] != payload[index]) throw StateError('TCP bytes changed');
    }
  } finally {
    socket.destroy();
    await reader.close();
  }
}

void _checkDns(List<int> answer) {
  if (answer.length < 12 ||
      answer[0] != 0x74 ||
      answer[1] != 0x11 ||
      answer[2] & 0x80 == 0) {
    throw StateError('DIRECT DNS response identity changed');
  }
}

Future<Set<int>> _helperProcesses() async {
  final result = await Process.run(
    '${Platform.environment['SystemRoot'] ?? r'C:\Windows'}/System32/WindowsPowerShell/v1.0/powershell.exe',
    [
      '-NoProfile',
      '-WindowStyle',
      'Hidden',
      '-Command',
      r"Get-Process | Where-Object { $_.ProcessName -eq 'shuvpn_tun_helper' } | ForEach-Object { $_.Id }",
    ],
  );
  if (result.exitCode != 0) {
    throw StateError('Cannot observe helper processes: ${result.stderr}');
  }
  return (result.stdout as String)
      .split(RegExp(r'\s+'))
      .where((value) => value.isNotEmpty)
      .map(int.parse)
      .toSet();
}

final class _Reader {
  _Reader(Stream<List<int>> stream) : _events = StreamIterator(stream);
  final StreamIterator<List<int>> _events;
  final _pending = <int>[];
  Future<List<int>> read(int count) async {
    if (count < 1 || count > 65536) throw StateError('Invalid reply size');
    while (_pending.length < count) {
      if (!await _events.moveNext().timeout(const Duration(seconds: 5))) {
        throw StateError('TCP ended early');
      }
      _pending.addAll(_events.current);
    }
    final result = _pending.sublist(0, count);
    _pending.removeRange(0, count);
    return result;
  }

  Future<void> close() => _events.cancel();
}
