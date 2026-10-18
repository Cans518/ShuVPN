import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'package:shuvpn/app/desktop_frame.dart';
import 'package:shuvpn/core/desktop/desktop_lifecycle.dart';
import 'package:shuvpn/core/desktop/desktop_port.dart';
import 'package:shuvpn/core/desktop/plugin_desktop_port.dart';
import 'package:shuvpn/core/rust/generated/api/vpn.dart' as native;
import 'package:shuvpn/core/rust/generated/connection/model.dart' as model;
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main() async {
  final _ExternalWindowAcceptance? acceptance;
  try {
    acceptance = _ExternalWindowAcceptance.fromEnvironment();
  } on Object catch (failure) {
    stderr.writeln(failure);
    exit(1);
  }
  WidgetsFlutterBinding.ensureInitialized();
  final port = _WindowProbePort(acceptance);
  await port.initializeWindow();
  runApp(
    MaterialApp(
      home: DesktopFrame(
        port: port,
        child: const Center(child: Text('桌面窗口与托盘验证')),
      ),
    ),
  );
  await WidgetsBinding.instance.endOfFrame;
  try {
    await RustCore.initialize();
    final fixture = await Process.start(
      File(
        acceptance?.fixturePath ??
            'rust/target/release/examples/proxy_fixture${Platform.isWindows ? '.exe' : ''}',
      ).absolute.path,
      [],
    );
    final stderr = fixture.stderr.transform(utf8.decoder).join();
    final plan = await fixture.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .first
        .timeout(const Duration(seconds: 5));
    final session = await native.VpnSession.connect(
      planJson: plan,
      timeoutMs: 3000,
    );
    final proxy = await session.startProxy(
      kind: model.ProxyKind.http,
      address: '127.0.0.1',
      port: 0,
    );
    var disconnected = false;
    final desktop = DesktopLifecycle(
      port: port,
      connect: () async {},
      selectProtocol: (_) {},
      disconnect: () => session.stop(),
      shutdown: () async {
        acceptance?.stopWatching();
        await session.stop();
        final stopped = await session.snapshot();
        session.dispose();
        try {
          final leaked = await Socket.connect('127.0.0.1', proxy.port);
          leaked.destroy();
          throw StateError('Proxy listener survived desktop exit');
        } on SocketException {
          disconnected = true;
        }
        await fixture.stdin.close();
        final code = await fixture.exitCode.timeout(const Duration(seconds: 5));
        if (code != 0) {
          throw StateError('Gateway fixture failed: ${await stderr}');
        }
        await acceptance?.flushEvents();
        await acceptance?.write('cleanup', {
          'session_state': stopped.state.name,
          'upload': stopped.uploadBytes.toInt(),
          'download': stopped.downloadBytes.toInt(),
          'proxy_port': proxy.port,
          'fixture_pid': fixture.pid,
          'fixture_exit': code,
          'proxy_closed': disconnected,
        });
        stdout.writeln('DESKTOP_CLEANUP_PASSED');
      },
    );
    desktop.update(
      TraySnapshot(
        status: acceptance == null ? '已连接' : acceptance.status,
        canConnect: false,
        canDisconnect: true,
        protocols: [
          TrayProtocol(
            id: 'atrust',
            label: 'aTrust',
            enabled: false,
            selected: true,
          ),
        ],
      ),
    );
    await desktop.start();
    runApp(
      MaterialApp(
        home: DesktopFrame(
          port: port,
          lifecycle: desktop,
          child: const Center(child: Text('桌面窗口与托盘验证')),
        ),
      ),
    );
    await WidgetsBinding.instance.endOfFrame;
    await port.show();
    await _wait(() => windowManager.isVisible());
    await port.minimize();
    await _wait(() => windowManager.isMinimized());
    await port.show();
    await _wait(() async => !await windowManager.isMinimized());
    await port.toggleMaximize();
    await _wait(() => windowManager.isMaximized());
    await port.toggleMaximize();
    await _wait(() async => !await windowManager.isMaximized());
    if (acceptance != null) {
      if (!desktop.trayReady) {
        throw StateError('External probe requires a tray');
      }
      acceptance.watch(desktop);
      await acceptance.write('ready', {
        'proxy_port': proxy.port,
        'fixture_pid': fixture.pid,
        'tray_ready': desktop.trayReady,
        'status': acceptance.status,
      });
      return;
    }
    if (desktop.trayReady) {
      await windowManager.close();
      await _wait(() async => !await windowManager.isVisible());
      if (disconnected) {
        throw StateError('Close-to-tray disconnected the session');
      }
      final alive = await Socket.connect('127.0.0.1', proxy.port);
      alive.destroy();
      await desktop.dispatch(DesktopCommand.show);
      await _wait(() => windowManager.isVisible());
    }
    stdout.writeln('DESKTOP_WINDOW_PASSED tray=${desktop.trayReady}');
    if (desktop.trayReady) {
      await desktop.quit();
    } else {
      await windowManager.close();
    }
  } on Object catch (failure, stack) {
    await acceptance?.write('failure', {'error': '$failure'});
    stderr.writeln('$failure\n$stack');
    exitCode = 1;
    await port.destroyTray();
    await port.destroyWindow();
  }
}

// This developer entrypoint never opens application account/settings storage.
// External mode waits for real Explorer input; a separate abort is cleanup only.
final class _ExternalWindowAcceptance {
  _ExternalWindowAcceptance(this.token, this.report, this.fixturePath);
  final String token;
  final Directory report;
  final String fixturePath;
  Timer? _watchdog;
  Timer? _abort;
  bool _aborting = false;
  int _eventSequence = 0;
  Future<void> _events = Future.value();
  String get status => '已连接 · $token';

  static _ExternalWindowAcceptance? fromEnvironment() {
    final environment = Platform.environment;
    final token = environment['SHUVPN_WINDOW_ACCEPTANCE_TOKEN'];
    final path = environment['SHUVPN_WINDOW_ACCEPTANCE_REPORT'];
    final fixture = environment['SHUVPN_WINDOW_ACCEPTANCE_FIXTURE'];
    if (token == null && path == null && fixture == null) return null;
    if (!Platform.isWindows ||
        token == null ||
        !RegExp(r'^[0-9a-f]{32}$').hasMatch(token) ||
        path == null ||
        fixture == null) {
      throw StateError('External Windows probe requires all private inputs');
    }
    final report = Directory(path).absolute;
    final boundary = Directory('build/desktop')
        .resolveSymbolicLinksSync()
        .toLowerCase();
    final resolved = report.resolveSymbolicLinksSync();
    if (!resolved.toLowerCase().startsWith(
          '$boundary${Platform.pathSeparator}',
        ) ||
        resolved.toLowerCase() != report.path.toLowerCase() ||
        File('${report.path}/ready.json').existsSync() ||
        !File(fixture).isAbsolute ||
        !File(fixture).existsSync()) {
      throw StateError('External Windows report/fixture paths are invalid');
    }
    return _ExternalWindowAcceptance(token, report, fixture);
  }

  Future<void> write(String stage, Map<String, Object?> fields) async {
    final file = File('${report.path}/$stage.pending');
    await file.writeAsString(
      jsonEncode({'token': token, 'pid': pid, 'stage': stage, ...fields}),
      flush: true,
    );
    await file.rename('${report.path}/$stage.json');
  }

  void event(String name) {
    final sequence = _eventSequence++;
    _events = _events.then((_) => write('event-$sequence', {'name': name}));
  }

  Future<void> flushEvents() => _events;

  void watch(DesktopLifecycle desktop) {
    _watchdog = Timer(const Duration(seconds: 120), () {
      exitCode = 1;
      unawaited(desktop.quit());
    });
    _abort = Timer.periodic(const Duration(milliseconds: 200), (_) {
      final file = File('${report.path}/abort.json');
      if (_aborting || !file.existsSync()) return;
      _aborting = true;
      final request =
          jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;
      if (request['token'] != token) {
        exitCode = 1;
      }
      unawaited(desktop.quit());
    });
  }

  void stopWatching() {
    _watchdog?.cancel();
    _abort?.cancel();
  }
}

final class _WindowProbePort extends PluginDesktopPort {
  _WindowProbePort(this.acceptance);
  final _ExternalWindowAcceptance? acceptance;

  @override
  Future<bool> createTray(
    void Function(DesktopCommand) command,
    void Function(String) protocol,
  ) => super.createTray((value) {
    acceptance?.event('native-command:${value.name}');
    command(value);
  }, protocol);

  @override
  Future<void> show() async {
    acceptance?.event('show:begin');
    await super.show();
    acceptance?.event('show:visible=${await windowManager.isVisible()}');
  }
}

Future<void> _wait(Future<bool> Function() condition) async {
  final deadline = DateTime.now().add(const Duration(seconds: 5));
  while (!await condition()) {
    if (DateTime.now().isAfter(deadline)) {
      throw StateError('Native window operation timed out');
    }
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }
}
