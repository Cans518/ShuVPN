// Developer-only native plan ingress; the production application does not import this.
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/desktop_connection.dart';
import 'package:shuvpn/core/rust/generated/api/routing.dart';
import 'package:shuvpn/core/rust/generated/api/vpn.dart';
import 'package:shuvpn/core/rust/generated/connection/desktop_services.dart';
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if (args.length != 4 ||
      !RegExp(r'^[0-9a-f]{32}$').hasMatch(args[3]) ||
      Platform.environment['SHUVPN_SOAK_TOKEN'] != args[3]) {
    throw ArgumentError(
      'Usage: connection_soak_probe library new-rules plan token',
    );
  }
  final profile = Directory(args[1]);
  if (await profile.exists()) throw StateError('Rules directory must be new');
  final plan = jsonDecode(await File(args[2]).readAsString()) as Map;
  final nodes = plan['nodes'] as Map;
  if (nodes.length != 1 ||
      (nodes.values.single as List).length != 1 ||
      !RegExp(r'^127[.]0[.]0[.]1:[0-9]+$')
          .hasMatch((nodes.values.single as List).single as String) ||
      plan['sid'] != 'test-sid' ||
      plan['acceptAnyCertificate'] != false ||
      (plan['dnsServers'] as List).isNotEmpty) {
    throw StateError(
      'Only a pinned loopback fixture with DNS disabled is allowed',
    );
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(File(args[0]).absolute.path),
  );
  final rules = await RoutingManager.open(directory: profile.absolute.path);
  DesktopConnectionOwner? authority;
  VpnSession? session;
  final token = args[3];
  var round = 0;
  Map<String, int> ports = {};

  Future<Map<String, Object?>> snapshot() async {
    final owner = authority!.snapshot();
    final current = session;
    Map<String, Object?>? native;
    if (current != null) {
      final state = await current.snapshot();
      native = {
        'state': state.state.name,
        'message': state.message,
        'upload': state.uploadBytes.toInt(),
        'download': state.downloadBytes.toInt(),
        'connections': state.connections.toInt(),
        'tun': jsonDecode(await current.tunStatus()),
        'egress': jsonDecode(await current.egressStatus()),
        'decisions': jsonDecode(await current.decisions()),
      };
    }
    return {
      'token': token,
      'pid': pid,
      'round': round,
      'ports': ports,
      'owner': {
        'phase': owner.phase.name,
        'generation': owner.generation.toString(),
        'run_id': owner.runId?.toString(),
        'cleanup_pending': owner.cleanupPending,
        'closed': owner.closed,
        'error_code': owner.errorCode,
        'error_message': owner.errorMessage,
      },
      'session': native,
    };
  }

  try {
    await rules.apply(
      profile: jsonEncode({
        'automatic_campus': false,
        'rules': [
          'IP-CIDR,10.21.0.0/16,SHU-VPN,no-resolve',
          'IP-CIDR6,2001:db8:21::/64,SHU-VPN,no-resolve',
          'MATCH,REJECT',
        ],
      }),
      overrides: '{}',
      expectedVersion: BigInt.zero,
    );
    authority = await DesktopConnectionOwner.open();
    authority.claimFrontend();
    await authority.awaitFrontend();
    stdout.writeln(jsonEncode({'ready': true, ...await snapshot()}));
    await for (final line
        in stdin.transform(utf8.decoder).transform(const LineSplitter())) {
      final request = jsonDecode(line) as Map;
      if (request['token'] != token || request['id'] is! int) {
        throw StateError('Command identity does not match this probe');
      }
      final command = request['command'];
      switch (command) {
        case 'connect':
          if (session != null) {
            throw StateError('Disconnect before reconnecting');
          }
          session = await authority.connectPlan(
            planJson: jsonEncode(plan),
            timeoutMs: 5000,
            startup: const ConnectionStartup(
              proxies: ProxyStartup(
                httpEnabled: true,
                httpAddress: '127.0.0.1',
                httpPort: 0,
                socksEnabled: true,
                socksAddress: '127.0.0.1',
                socksPort: 0,
                systemProxyEnabled: false,
              ),
            ),
          );
          await session.attachRouting(manager: rules);
          final services = await session.startupServices();
          if (services == null ||
              services.failures.isNotEmpty ||
              services.tun != null ||
              services.proxies.systemProxyEnabled ||
              services.proxies.http?.address != '127.0.0.1' ||
              services.proxies.socks?.address != '127.0.0.1') {
            throw StateError(
              'Both ordinary loopback proxies must start without TUN/system proxy',
            );
          }
          ports = {
            'http': services.proxies.http!.port,
            'socks': services.proxies.socks!.port,
          };
          round++;
        case 'snapshot':
          break;
        case 'disconnect':
          final diagnostic = await authority.disconnect();
          if (diagnostic != null) throw StateError(diagnostic);
        case 'shutdown':
          final diagnostic = await authority.shutdown();
          if (diagnostic != null) throw StateError(diagnostic);
        default:
          throw ArgumentError('Unknown probe command');
      }
      final value = await snapshot();
      if (command == 'disconnect' || command == 'shutdown') {
        session?.dispose();
        session = null;
      }
      stdout.writeln(
        jsonEncode({'id': request['id'], 'command': command, ...value}),
      );
      if (command == 'shutdown') return;
    }
    throw StateError('Probe input ended without explicit shutdown');
  } finally {
    try {
      if (authority != null && !authority.snapshot().closed) {
        await authority.shutdown();
      }
    } finally {
      session?.dispose();
      authority?.dispose();
      rules.dispose();
      ShuRustLib.dispose();
    }
  }
}
