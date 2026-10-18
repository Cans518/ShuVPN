import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/system_proxy.dart'
    as system_proxy;
import 'package:shuvpn/core/rust/generated/api/vpn.dart';
import 'package:shuvpn/core/rust/generated/api/desktop_connection.dart'
    as owner;
import 'package:shuvpn/core/rust/generated/connection/model.dart'
    show ProxyKind, VpnFailure;
import 'package:shuvpn/core/rust/generated/connection/desktop_services.dart'
    as services;
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if (args.length != 1) {
    throw ArgumentError('Expected packaged native library path');
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(File(args.single).absolute.path),
  );
  final directory = await Directory.systemTemp.createTemp(
    'shuvpn-system-proxy-ffi-',
  );
  final originalDirectory = await directory.resolveSymbolicLinks();
  final gateway = await Process.start(
    File(
      'rust/target/release/examples/proxy_fixture${Platform.isWindows ? '.exe' : ''}',
    ).absolute.path,
    [],
  );
  final errors = gateway.stderr.transform(utf8.decoder).join();
  VpnSession? session;
  owner.DesktopConnectionOwner? authority;
  Socket? socket;
  StreamIterator<Uint8List>? incoming;
  try {
    final plan = await gateway.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .first
        .timeout(const Duration(seconds: 5));
    authority = await owner.DesktopConnectionOwner.open();
    session = await authority.connectPlan(
      planJson: plan,
      timeoutMs: 3000,
      startup: owner.ConnectionStartup(
        proxies: const services.ProxyStartup(
          httpEnabled: true,
          httpAddress: '127.0.0.1',
          httpPort: 0,
          socksEnabled: true,
          socksAddress: '127.0.0.1',
          socksPort: 0,
          systemProxyEnabled: true,
        ),
        recoveryDirectory: directory.path,
      ),
    );
    final started = await session.startupServices();
    if (started == null) {
      throw StateError('Native owner did not publish its service report');
    }
    if (started.failures.isNotEmpty ||
        !started.proxies.systemProxyEnabled ||
        started.proxies.systemProxyError != null ||
        started.proxies.http == null ||
        started.proxies.socks == null) {
      throw StateError('Native system-proxy startup failed');
    }
    var binding = started.proxies.http!;
    if (!await system_proxy.systemProxyUsesPort(port: binding.port)) {
      throw StateError('OS proxy does not match the real FFI listener');
    }
    socket = await Socket.connect('127.0.0.1', binding.port);
    incoming = StreamIterator<Uint8List>(socket);
    final buffer = <int>[];
    Future<List<int>> read(int count) async {
      while (buffer.length < count) {
        if (!await incoming!.moveNext().timeout(const Duration(seconds: 5))) {
          throw StateError('Proxy stream ended early');
        }
        buffer.addAll(incoming!.current);
      }
      final result = buffer.sublist(0, count);
      buffer.removeRange(0, count);
      return result;
    }

    socket.write(
      'CONNECT ffi.internal.invalid:443 HTTP/1.1\r\nHost: ffi.internal.invalid:443\r\n\r\n',
    );
    await socket.flush();
    final header = <int>[];
    while (!utf8.decode(header).endsWith('\r\n\r\n')) {
      header.addAll(await read(1));
      if (header.length > 4096) {
        throw StateError('HTTP response exceeds budget');
      }
    }
    if (!utf8.decode(header).startsWith('HTTP/1.1 200') ||
        utf8.decode(await read(7)) != 'prelude') {
      throw StateError('Campus TLS proxy did not connect');
    }
    socket.add(utf8.encode('system-proxy-ffi'));
    await socket.flush();
    if (utf8.decode(await read(16)) != 'system-proxy-ffi') {
      throw StateError('Real proxy bytes differ');
    }
    final rebound = await session.rebindProxy(
      kind: ProxyKind.http,
      address: '127.0.0.1',
      port: 0,
      systemProxyDirectory: directory.path,
      systemProxyEnabled: true,
    );
    if (rebound.failures.isNotEmpty ||
        !rebound.proxies.systemProxyEnabled ||
        rebound.proxies.http == null ||
        rebound.proxies.socks?.port != started.proxies.socks?.port) {
      throw StateError(
        'Native HTTP rebind lost system proxy or independent SOCKS',
      );
    }
    binding = rebound.proxies.http!;
    if (!await system_proxy.systemProxyUsesPort(port: binding.port)) {
      throw StateError('System proxy did not follow the rebound actual port');
    }
    await session.stopProxy(kind: ProxyKind.http);
    if (await system_proxy.systemProxyUsesPort(port: binding.port)) {
      throw StateError('Stopping HTTP did not restore the OS proxy');
    }
    socket.destroy();
    await incoming.cancel();
    socket = await Socket.connect('127.0.0.1', started.proxies.socks!.port);
    incoming = StreamIterator<Uint8List>(socket);
    buffer.clear();
    socket.add([5, 1, 0]);
    await socket.flush();
    if ((await read(2)).join(',') != '5,0') {
      throw StateError(
        'Stopping HTTP also stopped the independent SOCKS listener',
      );
    }
    await authority.shutdown();
    stdout.writeln(
      'PACKAGED_FFI_SYSTEM_PROXY_ACTUAL_PORT_BYTES_AND_RESTORE_PASSED',
    );
    stdout.writeln(
      'NATIVE_SERVICE_STARTUP_SYSTEM_PROXY_HTTP_SOCKS_RESTORE_PASSED',
    );
    stdout.writeln('NATIVE_OWNER_REBIND_SYSTEM_PROXY_AND_EXIT_BARRIER_PASSED');
  } on VpnFailure catch (failure) {
    throw StateError('${failure.code}: ${failure.message}');
  } finally {
    socket?.destroy();
    await incoming?.cancel();
    try {
      try {
        await authority?.shutdown();
      } finally {
        session?.dispose();
        authority?.dispose();
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
      await errors;
      await system_proxy.recoverSystemProxy(directory: directory.path);
      if (await File('${directory.path}/system-proxy.json').exists()) {
        throw StateError('Recovery record retained at ${directory.path}');
      }
      if (await directory.resolveSymbolicLinks() != originalDirectory) {
        throw StateError('Smoke-test cleanup directory changed');
      }
      await directory.delete(recursive: true);
    }
  }
}
