import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/rust/generated/api/vpn.dart' as native;
import 'package:shuvpn/core/rust/generated/connection/model.dart' as model;
import 'package:shuvpn/core/rust/rust_connection.dart';
import 'package:shuvpn/core/connection/native_connection.dart';
import 'package:shuvpn/core/connection/native_tun.dart';
import 'package:shuvpn/core/rust/rust_core.dart';
import 'package:shuvpn/core/rust/generated/api/routing.dart' as routing;
import 'package:shuvpn/core/rust/generated/api/desktop.dart' as desktop;
import 'package:shuvpn/core/rust/generated/api/account.dart' as account;
import 'package:shuvpn/core/rust/generated/api/desktop_connection.dart'
    as desktop_owner;
import 'package:shuvpn/core/rust/generated/connection/desktop_services.dart'
    as services;
import 'package:shuvpn/core/auth/account_authentication.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';

import '../settings_stub.dart';
import '../native_backend_stub.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    await RustCore.initialize();
  });

  test('native authority survives view disposal and rejects an old frontend after takeover', () async {
    final gateway = await _Gateway.start();
    addTearDown(gateway.close);
    final handles = <desktop_owner.DesktopConnectionOwner>[];
    final views = <native.VpnSession>[];
    addTearDown(() async {
      final recovery = await desktop_owner.DesktopConnectionOwner.open();
      try {
        await recovery.shutdown();
      } finally {
        recovery.dispose();
      }
      for (final view in views) {
        if (!view.isDisposed) view.dispose();
      }
      for (final handle in handles) {
        if (!handle.isDisposed) handle.dispose();
      }
    });
    const startup = desktop_owner.ConnectionStartup(
      proxies: services.ProxyStartup(
        httpEnabled: true,
        httpAddress: '127.0.0.1',
        httpPort: 0,
        socksEnabled: true,
        socksAddress: '127.0.0.1',
        socksPort: 0,
        systemProxyEnabled: false,
      ),
    );
    final first = await desktop_owner.DesktopConnectionOwner.open();
    handles.add(first);
    first.claimFrontend();
    await first.awaitFrontend();
    final view = await first.connectPlan(
      planJson: gateway.plan,
      timeoutMs: 3000,
      startup: startup,
    );
    views.add(view);
    final oldPort = (await view.startupServices())!.proxies.http!.port;
    view.dispose();
    expect(first.snapshot().runId, isNotNull);
    (await Socket.connect('127.0.0.1', oldPort)).destroy();
    final second = await desktop_owner.DesktopConnectionOwner.open();
    handles.add(second);
    second.claimFrontend();
    await second.awaitFrontend();
    await expectLater(
      Socket.connect('127.0.0.1', oldPort),
      throwsA(isA<SocketException>()),
    );
    final replacement = await second.connectPlan(
      planJson: gateway.plan,
      timeoutMs: 3000,
      startup: startup,
    );
    views.add(replacement);
    final newPort = (await replacement.startupServices())!.proxies.http!.port;
    expect(
      first.cancel,
      throwsA(
        isA<model.VpnFailure>().having((error) => error.code, 'code', 'stale'),
      ),
    );
    await expectLater(
      first.shutdown(),
      throwsA(
        isA<model.VpnFailure>().having((error) => error.code, 'code', 'stale'),
      ),
    );
    first.dispose();
    replacement.dispose();
    (await Socket.connect('127.0.0.1', newPort)).destroy();
    await second.shutdown();
    expect(second.snapshot().closed, isTrue);
    expect(second.snapshot().cleanupPending, isFalse);
    await expectLater(
      Socket.connect('127.0.0.1', newPort),
      throwsA(isA<SocketException>()),
    );
  });

  test('controller termination closes real native listeners through one owner command', () async {
    final gateway = await _Gateway.start();
    addTearDown(gateway.close);
    final session = await native.VpnSession.connect(
      planJson: gateway.plan,
      timeoutMs: 3000,
    );
    final connection = _TrackedConnection(await RustConnection.create(session));
    addTearDown(connection.close);
    final settings = StubSettingsStore()
      ..httpProxyEnabled = true
      ..socksProxyEnabled = true
      ..httpPort = 0
      ..socksPort = 0;
    addTearDown(settings.dispose);
    final controller = ConnectionController(
      settings,
      nativeConnectionBackend: _PreparedBackend(connection),
    )..ssoSessionProvider = () => const _AuthenticatedAccount();
    addTearDown(controller.dispose);
    expect(await controller.connect(), isTrue);
    final ports = [controller.httpPort!, controller.socksPort];
    expect(controller.proxyRunning, isTrue);
    await controller.disconnectForExit();
    expect(connection.closeCalls, 1);
    expect(connection.stopCalls, 0);
    expect(connection.startCalls, 0);
    expect(connection.serviceCalls, 1);
    expect(controller.proxyRunning, isFalse);
    expect(controller.cleanupPending, isFalse);
    for (final port in ports) {
      await expectLater(
        Socket.connect('127.0.0.1', port, timeout: const Duration(seconds: 2)),
        throwsA(isA<SocketException>()),
      );
    }
  });

  test('connection record adapter reads real proxy results and distinguishes authorization refusal', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-records-ffi-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final manager = await routing.RoutingManager.open(
      directory: directory.path,
    );
    addTearDown(manager.dispose);
    await manager.apply(
      profile: 'automatic_campus: false\nrules: ["DOMAIN,blocked.internal.invalid,REJECT", "MATCH,SHU-VPN"]',
      overrides: '{}',
      expectedVersion: BigInt.zero,
    );
    final gateway = await _Gateway.start();
    addTearDown(gateway.close);
    final session = await native.VpnSession.connect(
      planJson: gateway.plan,
      timeoutMs: 3000,
    );
    final connection = await RustConnection.create(session);
    addTearDown(connection.close);
    await session.attachRouting(manager: manager);
    final binding = await connection.startProxy(
      NativeProxyKind.http,
      '127.0.0.1',
      0,
    );
    Future<String> request(String host) async {
      final socket = await Socket.connect('127.0.0.1', binding.port);
      final reader = _Reader(socket);
      try {
        socket.write('CONNECT $host:443 HTTP/1.1\r\n\r\n');
        await socket.flush();
        final header = <int>[];
        while (!utf8.decode(header).endsWith('\r\n\r\n')) {
          header.addAll(await reader.read(1));
        }
        final response = utf8.decode(header);
        if (response.startsWith('HTTP/1.1 200')) {
          expect(utf8.decode(await reader.read(7)), 'prelude');
          socket.write('record');
          await socket.flush();
          expect(utf8.decode(await reader.read(6)), 'record');
        }
        return response;
      } finally {
        socket.destroy();
        await reader.close();
      }
    }

    expect(
      await request('blocked.internal.invalid'),
      startsWith('HTTP/1.1 403'),
    );
    expect(await request('127.0.0.1'), startsWith('HTTP/1.1 403'));
    expect(
      await request('allowed.internal.invalid'),
      startsWith('HTTP/1.1 200'),
    );
    final records = await connection.readObservations();
    expect(records.length, 3);
    expect(records[0].expectedAction, 'REJECT');
    expect(records[0].error, 'rejected');
    expect(records[0].outbound, isNull);
    expect(records[1].expectedAction, 'SHU-VPN');
    expect(records[1].outbound, isNull);
    expect(records[1].error, 'unrouted');
    expect(records[1].message, contains('资源表'));
    expect(records[2].host, 'allowed.internal.invalid');
    expect(records[2].outbound, 'SHU-VPN');
    expect(records[2].version, 1);
    expect(records[2].inbound, 'http');
    await connection.close();
    await expectLater(connection.readObservations(), throwsStateError);
  });

  test(
    'desktop TUN preview and interface choices come from the native backend',
    () async {
      final choices = await desktop.desktopNetworkInterfaces();
      expect(choices.length, lessThanOrEqualTo(256));
      expect(
        choices.where((choice) => choice.recommended).length,
        lessThanOrEqualTo(1),
      );
      expect(
        choices.every((choice) => choice.name.isNotEmpty && choice.index > 0),
        isTrue,
      );
      final gateway = await _Gateway.start(mode: 'domain-dns');
      addTearDown(gateway.close);
      final session = await native.VpnSession.connect(
        planJson: gateway.plan,
        timeoutMs: 3000,
      );
      addTearDown(() async {
        await session.stop();
        session.dispose();
      });
      final plan = jsonDecode(
        await session.desktopTunPlan(mtu: 1500, physicalDns: '1.1.1.1'),
      ) as Map<String, dynamic>;
      expect(plan['mtu'], 1400);
      expect(plan['physical_dns_servers'], ['1.1.1.1']);
      expect(plan['addresses'], contains('10.99.0.2/32'));
      expect(plan['protected_ips'], contains('127.0.0.1'));
      expect(plan['dns_capture'], isTrue);
      for (final key in [
        'sid',
        'signKey',
        'sign_key',
        'connectionId',
        'cookie',
      ]) {
        expect(plan.containsKey(key), isFalse);
      }
      expect(
        desktop.validateDesktopDns(value: ' ::ffff:192.0.2.53 '),
        '192.0.2.53',
      );
      expect(
        () => desktop.validateDesktopDns(value: '127.0.2.2'),
        throwsA(isA<model.VpnFailure>()),
      );
      await expectLater(
        session.startDesktopTun(mtu: 1400, physicalInterface: '\u0000'),
        throwsA(
          isA<model.VpnFailure>().having(
            (error) => error.code,
            'code',
            'interface',
          ),
        ),
      );
    },
  );

  test('gateway hyphen ranges authorize signed UDP through real FFI', () async {
    final gateway = await _Gateway.start(mode: 'udp');
    addTearDown(gateway.close);
    final plan = jsonDecode(gateway.plan) as Map<String, dynamic>;
    var replaced = false;
    for (final route in plan['routes'] as List<dynamic>) {
      if (route['host'] == '10.21.0.0/16') {
        route['host'] = '10.21.0.1-10.21.0.254';
        replaced = true;
      }
    }
    expect(replaced, isTrue);
    final session = await native.VpnSession.connect(
      planJson: jsonEncode(plan),
      timeoutMs: 3000,
    );
    addTearDown(() async {
      await session.stop();
      session.dispose();
    });
    final port = await session.openPacketPort();
    addTearDown(() async {
      await port.close();
      port.dispose();
    });
    await port.writePacket(packet: _udpPacket(utf8.encode('ffi-range')));
    final reply = await port.readPacket(waitMs: 3000);
    expect(reply, isNotNull);
    expect(utf8.decode(reply!.sublist(28)), 'ffi-range');
  });

  test(
    'raw UDP packets use signed campus TLS and cancel through real FFI',
    () async {
      final gateway = await _Gateway.start(mode: 'udp');
      addTearDown(gateway.close);
      final session = await native.VpnSession.connect(
        planJson: gateway.plan,
        timeoutMs: 3000,
      );
      addTearDown(() async {
        await session.stop();
        session.dispose();
      });
      final port = await session.openPacketPort();
      final egress =
          jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
      expect(egress['scope'], 'process');
      expect(egress['active'], isFalse);
      addTearDown(() async {
        await port.close();
        port.dispose();
      });
      await port.writePacket(packet: _udpPacket(utf8.encode('ffi-raw-udp')));
      final reply = await port.readPacket(waitMs: 3000);
      expect(reply, isNotNull);
      expect(reply!.sublist(12, 16), [10, 21, 0, 5]);
      expect(reply.sublist(16, 20), [10, 99, 0, 2]);
      expect(utf8.decode(reply.sublist(28)), 'ffi-raw-udp');
      final events = jsonDecode(await session.decisions()) as List<dynamic>;
      expect(events.single['flow']['inbound'], 'tun');
      expect(events.single['outbound'], 'SHU-VPN');
      if (Platform.isWindows || Platform.isLinux) {
        await expectLater(
          session.startTun(planJson: '{}', physicalInterface: ''),
          throwsA(
            isA<model.VpnFailure>().having(
              (error) => error.code,
              'code',
              'interface',
            ),
          ),
        );
      }
      final pending = port
          .readPacket(waitMs: 5000)
          .then(
            (_) => false,
            onError: (Object error) =>
                error is model.VpnFailure && error.code == 'closed',
          );
      await session.stop();
      expect(await pending, isTrue);
      if (Platform.isWindows || Platform.isLinux) {
        await expectLater(
          session.startTun(planJson: '{}', physicalInterface: 'Ethernet'),
          throwsA(
            isA<model.VpnFailure>().having(
              (error) => error.code,
              'code',
              'closed',
            ),
          ),
        );
      }
    },
  );

  test(
    'real TLS proxy traffic and final state cross the FFI event stream',
    () async {
      final gateway = await _Gateway.start();
      addTearDown(gateway.close);
      final session = await native.VpnSession.connect(
        planJson: gateway.plan,
        timeoutMs: 3000,
      );
      final connection = await RustConnection.create(session);
      addTearDown(connection.close);
      final events = <NativeConnectionUpdate>[];
      final traffic = Completer<void>();
      final disconnected = Completer<void>();
      final subscription = connection.updates.listen((event) {
        events.add(event);
        if (event.uploadBytes > 0 &&
            event.downloadBytes > 0 &&
            event.uploadRate > 0 &&
            !traffic.isCompleted) {
          traffic.complete();
        }
        if (event.state == NativeConnectionState.disconnected &&
            !disconnected.isCompleted) {
          disconnected.complete();
        }
      });
      addTearDown(subscription.cancel);
      final binding = await connection.startProxy(
        NativeProxyKind.http,
        '127.0.0.1',
        0,
      );
      final client = await Socket.connect('127.0.0.1', binding.port);
      expect(binding.displayEndpoint, '127.0.0.1:${binding.port}');
      expect(binding.displayWarning, isNull);
      addTearDown(client.destroy);
      final reader = _Reader(client);
      addTearDown(reader.close);
      client.write(
        'CONNECT ffi.internal.invalid:443 HTTP/1.1\r\nHost: ffi.internal.invalid:443\r\n\r\n',
      );
      await client.flush();
      final response = <int>[];
      while (!utf8.decode(response).endsWith('\r\n\r\n')) {
        response.addAll(await reader.read(1));
      }
      expect(utf8.decode(response), startsWith('HTTP/1.1 200'));
      expect(utf8.decode(await reader.read(7)), 'prelude');
      final payload = List<int>.generate(16 * 1024, (index) => index % 256);
      client.add(payload);
      await client.flush();
      expect(await reader.read(payload.length), payload);
      await traffic.future.timeout(const Duration(seconds: 5));
      expect(
        events.any(
          (event) => event.connections == 1 && event.latencyMs != null,
        ),
        isTrue,
      );
      expect(connection.current.virtualAddress, '10.99.0.2');
      await connection.close();
      await disconnected.future.timeout(const Duration(seconds: 2));
      await expectLater(
        Socket.connect('127.0.0.1', binding.port),
        throwsA(isA<SocketException>()),
      );
    },
  );

  if (Platform.isWindows) {
    test('Windows packet DIRECT binds its requested interface through the packaged FFI', () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-ffi-windows-egress-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final manager = await routing.RoutingManager.open(
        directory: directory.path,
      );
      addTearDown(manager.dispose);
      await manager.apply(
        profile: 'automatic_campus: false\nrules: ["MATCH,DIRECT"]',
        overrides: '{}',
        expectedVersion: BigInt.zero,
      );
      final gateway = await _Gateway.start();
      addTearDown(gateway.close);
      final session = await native.VpnSession.connect(
        planJson: gateway.plan,
        timeoutMs: 3000,
      );
      addTearDown(() async {
        await session.stop();
        session.dispose();
      });
      await session.attachRouting(manager: manager);
      await expectLater(
        session.openPacketPort(physicalInterface: 'missing-shuvpn-interface'),
        throwsA(
          isA<model.VpnFailure>().having(
            (error) => error.code,
            'code',
            'interface',
          ),
        ),
      );
      final endpoint = await RawDatagramSocket.bind(
        InternetAddress.loopbackIPv4,
        0,
      );
      addTearDown(endpoint.close);
      final received = Completer<void>();
      final subscription = endpoint.listen((event) {
        if (event != RawSocketEvent.read) return;
        final datagram = endpoint.receive();
        if (datagram == null) return;
        expect(utf8.decode(datagram.data), 'ffi-direct-udp');
        endpoint.send(utf8.encode('physical'), datagram.address, datagram.port);
        if (!received.isCompleted) received.complete();
      });
      addTearDown(subscription.cancel);
      final port = await session.openPacketPort(
        physicalInterface: 'Loopback Pseudo-Interface 1',
      );
      addTearDown(() async {
        await port.close();
        port.dispose();
      });
      await port.writePacket(
        packet: _udpPacket(
          utf8.encode('ffi-direct-udp'),
          destination: [127, 0, 0, 1],
          destinationPort: endpoint.port,
        ),
      );
      final reply = await port.readPacket(waitMs: 3000);
      expect(reply, isNotNull);
      expect(reply!.sublist(12, 16), [127, 0, 0, 1]);
      expect(reply.sublist(16, 20), [10, 99, 0, 2]);
      expect(utf8.decode(reply.sublist(28)), 'physical');
      await received.future.timeout(const Duration(seconds: 3));
      final egress =
          jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
      expect(egress['active'], isFalse);
      expect(
        (egress['sockets'] as List).where(
          (socket) => socket['peer'] == '127.0.0.1' && socket['binding'] == '1',
        ),
        isNotEmpty,
      );
      final decisions = jsonDecode(await session.decisions()) as List<dynamic>;
      expect(decisions.single['outbound'], 'DIRECT');
      await port.close();
      await session.stop();
      final stopped =
          jsonDecode(await session.egressStatus()) as Map<String, dynamic>;
      expect(
        (stopped['sockets'] as List).where(
          (socket) => socket['binding'] != '0',
        ),
        isEmpty,
      );
    });
  }

  test('FFI routing sends DIRECT to a physical endpoint and REJECT opens no target', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-ffi-routing-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final manager = await routing.RoutingManager.open(
      directory: directory.path,
    );
    addTearDown(manager.dispose);
    await manager.apply(
      profile: 'rules: ["DOMAIN,blocked.example,REJECT", "MATCH,DIRECT"]',
      overrides: '{}',
      expectedVersion: BigInt.zero,
    );
    final gateway = await _Gateway.start();
    addTearDown(gateway.close);
    final session = await native.VpnSession.connect(
      planJson: gateway.plan,
      timeoutMs: 3000,
    );
    addTearDown(session.dispose);
    await session.attachRouting(manager: manager);
    final binding = await session.startProxy(
      kind: model.ProxyKind.http,
      address: '127.0.0.1',
      port: 0,
    );
    addTearDown(session.stop);
    final endpoint = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(endpoint.close);
    var accepted = 0;
    final physical = endpoint.listen((socket) {
      accepted++;
      socket.write('physical');
      socket.close();
    });
    addTearDown(physical.cancel);
    Future<String> connect(String target, int port) async {
      final socket = await Socket.connect('127.0.0.1', binding.port);
      final reader = _Reader(socket);
      try {
        socket.write(
          'CONNECT $target:$port HTTP/1.1\r\nHost: $target:$port\r\n\r\n',
        );
        await socket.flush();
        final header = <int>[];
        while (!utf8.decode(header).endsWith('\r\n\r\n')) {
          header.addAll(await reader.read(1));
        }
        final response = utf8.decode(header);
        if (response.startsWith('HTTP/1.1 200')) {
          expect(utf8.decode(await reader.read(8)), 'physical');
        }
        return response;
      } finally {
        socket.destroy();
        await reader.close();
      }
    }

    expect(
      await connect('127.0.0.1', endpoint.port),
      startsWith('HTTP/1.1 200'),
    );
    expect(
      await connect('blocked.example', endpoint.port),
      startsWith('HTTP/1.1 403'),
    );
    expect(accepted, 1);
    final decisions = jsonDecode(await session.decisions()) as List<Object?>;
    expect((decisions[0] as Map<String, Object?>)['outbound'], 'DIRECT');
    expect((decisions[1] as Map<String, Object?>)['error'], 'rejected');
    final adapter = await RustConnection.create(session);
    final records = await adapter.readObservations();
    expect(records[0].outbound, 'DIRECT');
    expect(records[0].host, '127.0.0.1');
    expect(records[1].error, 'rejected');
  });

  test('campus DNS metadata, cache and authorized IP tunnel cross the real FFI boundary', () async {
    final directory = await Directory.systemTemp.createTemp('shuvpn-ffi-dns-');
    addTearDown(() => directory.delete(recursive: true));
    final manager = await routing.RoutingManager.open(
      directory: directory.path,
    );
    addTearDown(manager.dispose);
    await manager.apply(
      profile: 'automatic_campus: false\nrules: ["IP-CIDR,10.99.0.0/24,SHU-VPN", "MATCH,REJECT"]',
      overrides: '{}',
      expectedVersion: BigInt.zero,
    );
    final gateway = await _Gateway.start(mode: 'dns');
    addTearDown(gateway.close);
    final session = await native.VpnSession.connect(
      planJson: gateway.plan,
      timeoutMs: 4000,
    );
    addTearDown(session.dispose);
    await session.attachRouting(manager: manager);
    final binding = await session.startProxy(
      kind: model.ProxyKind.http,
      address: '127.0.0.1',
      port: 0,
    );
    addTearDown(session.stop);
    for (var index = 0; index < 2; index++) {
      final client = await Socket.connect('127.0.0.1', binding.port);
      final reader = _Reader(client);
      try {
        client.write('CONNECT ffi-dns.shu.edu.cn:443 HTTP/1.1\r\n\r\n');
        await client.flush();
        final response = <int>[];
        while (!utf8.decode(response).endsWith('\r\n\r\n')) {
          response.addAll(await reader.read(1));
        }
        expect(utf8.decode(response), startsWith('HTTP/1.1 200'));
        expect(utf8.decode(await reader.read(7)), 'prelude');
        client.write('ffi-dns');
        await client.flush();
        expect(utf8.decode(await reader.read(7)), 'ffi-dns');
      } finally {
        client.destroy();
        await reader.close();
      }
    }
    final decisions = (jsonDecode(await session.decisions()) as List<Object?>)
        .cast<Map<String, Object?>>();
    final dns =
        (decisions[1]['dns'] as List<Object?>).first as Map<String, Object?>;
    expect((dns['source'] as Map<String, Object?>)['kind'], 'campus');
    expect(dns['ttl_seconds'], 1);
    expect(dns['version'], 1);
    expect(dns['cached'], true);
    expect(decisions[1]['outbound'], 'SHU-VPN');
    final adapter = await RustConnection.create(session);
    final records = await adapter.readObservations();
    expect(records[1].dns.single.source, 'campus');
    expect(records[1].dns.single.cached, isTrue);
    expect(records[1].dns.single.ttlSeconds, 1);
    final stats = jsonDecode(await session.dnsStats()) as Map<String, Object?>;
    expect(stats['campus_queries'], 2);
    expect(stats['system_lookups'], 0);
    await session.stop();
    expect(
      (jsonDecode(await session.dnsStats())
          as Map<String, Object?>)['cached_entries'],
      0,
    );
  });

  test(
    'native connection identity and signing key use the original wire format',
    () {
      final first = native.sessionIdentity(deviceId: 'test-device');
      final second = native.sessionIdentity(deviceId: 'test-device');
      expect(first.connectionId, matches(RegExp(r'^[0-9A-F]{32}-[0-9]+$')));
      expect(first.connectionId, isNot(second.connectionId));
      expect(first.signKey, hasLength(32));
      expect(second.signKey, hasLength(32));
    },
  );

  test('account connection validates its native deadline and cancellation before authentication', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-attempt-ffi-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final owner = await account.AccountSession.open(
      directory: directory.path,
      legacyCookies: '[]',
      legacySnapshot: '{}',
    );
    addTearDown(() {
      owner.close();
      owner.dispose();
    });
    final attempt = native.VpnConnectAttempt();
    addTearDown(attempt.dispose);
    Future<native.VpnSession> connect(int budget) => attempt.connectAccount(
      account: owner,
      gatewayOrigin: 'https://foreign.example/',
      strictAntiMitm: false,
      timeoutMs: budget,
    );
    for (final budget in [0, 99, 120001]) {
      await expectLater(
        connect(budget),
        throwsA(
          isA<model.VpnFailure>().having((error) => error.code, 'code', 'plan'),
        ),
      );
    }
    for (final input in [
      '',
      'https://[',
      'ftp://atrust.shu.edu.cn',
      'https://user:secret@atrust.shu.edu.cn',
    ]) {
      await expectLater(
        attempt.connectAccount(
          account: owner,
          gatewayOrigin: input,
          strictAntiMitm: false,
          timeoutMs: 1000,
        ),
        throwsA(
          isA<model.VpnFailure>().having(
            (error) => error.code,
            'code',
            'gateway',
          ),
        ),
      );
    }
    await expectLater(
      connect(1000),
      throwsA(
        isA<model.VpnFailure>().having(
          (error) => error.code,
          'code',
          'atrustOrigin',
        ),
      ),
    );
    attempt.cancel();
    await expectLater(
      connect(1000),
      throwsA(
        isA<model.VpnFailure>().having(
          (error) => error.code,
          'code',
          'cancelled',
        ),
      ),
    );
    expect(owner.snapshot().signedIn, isFalse);
  });

  test(
    'typed runtime errors and cancellation cross the native boundary',
    () async {
      await expectLater(
        native.VpnSession.connect(planJson: '{}', timeoutMs: 1000),
        throwsA(isA<model.VpnFailure>()),
      );
      final attempt = native.VpnConnectAttempt();
      addTearDown(attempt.dispose);
      attempt.cancel();
      await expectLater(
        attempt.connect(planJson: '{}', timeoutMs: 1000),
        throwsA(
          isA<model.VpnFailure>().having(
            (error) => error.code,
            'code',
            'cancelled',
          ),
        ),
      );
    },
  );
}

class _AuthenticatedAccount implements AccountAuthentication {
  const _AuthenticatedAccount();
  @override
  bool get hasSession => true;
}

class _PreparedBackend extends StubNativeBackend {
  _PreparedBackend(this.connection);
  final _TrackedConnection connection;
  bool _connected = false;
  @override
  NativeLifecycleSnapshot get lifecycle => NativeLifecycleSnapshot(
    phase: _connected
        ? NativeLifecyclePhase.connected
        : NativeLifecyclePhase.disconnected,
    runId: _connected ? 1 : null,
  );
  @override
  Future<NativeConnection> connect({
    required AccountAuthentication auth,
    required String server,
    required Duration timeout,
    required NativeProxyPreferences proxies,
    NativeTunStartup? tun,
  }) async {
    connection.initialServices = await connection.startServices(
      proxies,
      tun: tun,
    );
    _connected = true;
    return connection;
  }

  @override
  void cancel() {
    _connected = false;
    connection.cancel();
  }

  @override
  Future<void> disconnect() async {
    await connection.close();
    _connected = false;
  }

  @override
  Future<void> shutdown() => disconnect();
  @override
  void dispose() {}
}

class _TrackedConnection implements NativeConnection {
  _TrackedConnection(this.inner);
  final RustConnection inner;
  int closeCalls = 0;
  int stopCalls = 0;
  int startCalls = 0;
  int serviceCalls = 0;
  @override
  NativeServicesResult? initialServices;
  @override
  // This prepared test backend owns inner; the controller holds only this view.
  void disposeView() {}
  @override
  Future<NativeServicesResult> rebindProxy(
    NativeProxyKind kind,
    String address,
    int port, {
    required bool systemProxyEnabled,
  }) => inner.rebindProxy(
    kind,
    address,
    port,
    systemProxyEnabled: systemProxyEnabled,
  );
  @override
  Future<NativeServicesResult> startServices(
    NativeProxyPreferences preferences, {
    NativeTunStartup? tun,
  }) {
    serviceCalls++;
    return inner.startServices(preferences, tun: tun);
  }

  @override
  NativeConnectionUpdate get current => inner.current;
  @override
  Stream<NativeConnectionUpdate> get updates => inner.updates;
  @override
  Future<NativeProxyBinding> startProxy(
    NativeProxyKind kind,
    String address,
    int port,
  ) {
    startCalls++;
    return inner.startProxy(kind, address, port);
  }

  @override
  Future<void> stopProxy(NativeProxyKind kind) {
    stopCalls++;
    return inner.stopProxy(kind);
  }

  @override
  Future<void> close() {
    closeCalls++;
    return inner.close();
  }

  @override
  void cancel() => inner.cancel();
}

Uint8List _udpPacket(
  List<int> payload, {
  List<int> destination = const [10, 21, 0, 5],
  int destinationPort = 9001,
}) {
  final bytes = Uint8List(28 + payload.length);
  final data = ByteData.sublistView(bytes);
  bytes[0] = 0x45;
  data.setUint16(2, bytes.length);
  bytes[8] = 64;
  bytes[9] = 17;
  bytes.setRange(12, 16, [10, 99, 0, 2]);
  bytes.setRange(16, 20, destination);
  data.setUint16(20, 51012);
  data.setUint16(22, destinationPort);
  data.setUint16(24, 8 + payload.length);
  bytes.setRange(28, bytes.length, payload);
  var sum = 0;
  for (var index = 0; index < 20; index += 2) {
    sum += data.getUint16(index);
  }
  while (sum > 0xffff) {
    sum = (sum & 0xffff) + (sum >> 16);
  }
  data.setUint16(10, (~sum) & 0xffff);
  return bytes;
}

class _Gateway {
  _Gateway(this.process, this.plan);
  final Process process;
  final String plan;
  bool _closed = false;
  static Future<_Gateway> start({String? mode}) async {
    final path = File(
      'rust/target/release/examples/proxy_fixture${Platform.isWindows ? '.exe' : ''}',
    ).absolute.path;
    if (!File(path).existsSync()) {
      throw StateError(
        'Build the native fixture first: cargo build --manifest-path rust/Cargo.toml --lib --examples --release --locked',
      );
    }
    final process = await Process.start(path, [?mode]);
    final errors = process.stderr.transform(utf8.decoder).join();
    try {
      final line = await process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .first
          .timeout(const Duration(seconds: 5));
      final decoded = jsonDecode(line);
      if (decoded is! Map) {
        throw StateError('Native TLS fixture failed: ${await errors}');
      }
      return _Gateway(process, line);
    } on Object {
      process.kill();
      await process.stdin.close();
      rethrow;
    }
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    await process.stdin.close();
    await process.exitCode.timeout(
      const Duration(seconds: 5),
      onTimeout: () {
        process.kill();
        return -1;
      },
    );
  }
}

class _Reader {
  _Reader(Socket socket) : _iterator = StreamIterator<Uint8List>(socket);
  final StreamIterator<Uint8List> _iterator;
  final List<int> _buffer = [];
  Future<List<int>> read(int size) async {
    while (_buffer.length < size) {
      if (!await _iterator.moveNext().timeout(const Duration(seconds: 5))) {
        throw StateError('Proxy stream ended early');
      }
      _buffer.addAll(_iterator.current);
    }
    final result = _buffer.sublist(0, size);
    _buffer.removeRange(0, size);
    return result;
  }

  Future<void> close() => _iterator.cancel();
}
