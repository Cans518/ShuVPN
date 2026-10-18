import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/auth/account_authentication.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/connection/connection_status.dart';
import 'package:shuvpn/core/connection/native_connection.dart';
import 'package:shuvpn/core/connection/connection_observation.dart';
import 'package:shuvpn/core/connection/native_tun.dart';
import 'package:shuvpn/core/connection/protocol.dart';
import 'package:shuvpn/core/settings/settings_store.dart' show CaptureMode;

import 'settings_stub.dart';
import 'native_backend_stub.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  for (final protocol in [ShuProtocol.easyConnect, ShuProtocol.openVpn]) {
    test(
      'native desktop rejects $protocol before any legacy endpoint handling',
      () async {
        SharedPreferences.setMockInitialValues({});
        final settings = _PermissiveSettings(
          await SharedPreferences.getInstance(),
        );
        addTearDown(settings.dispose);
        final controller = ConnectionController(
          settings,
          draft: ConnectionDraft(
            protocol: protocol,
            server: 'https://[',
            loginDomain: 'fixture',
          ),
          nativeConnectionBackend: StubNativeBackend(
            failure: 'unsupported protocol reached native dial',
          ),
        );
        addTearDown(controller.dispose);
        expect(await controller.connect(), isFalse);
        expect(controller.errorCode, ConnectionErrorCode.unsupported);
        expect(controller.errorMessage, contains('桌面'));
        expect(controller.busy, isFalse);
      },
    );
  }

  Future<(ConnectionController, _Attempt, _Connection)> setup({
    bool pending = false,
    bool systemProxy = false,
    bool desktopTun = false,
    bool vpnEnabled = false,
    bool tunAvailable = true,
    String? server,
  }) async {
    SharedPreferences.setMockInitialValues({});
    final settings = await StubSettingsStore.load();
    if (server != null) settings.server = server;
    settings.vpnEnabled = vpnEnabled;
    settings.httpProxyEnabled = true;
    settings.socksProxyEnabled = true;
    settings.systemProxyEnabled = systemProxy;
    const auth = _Account();
    final connection = _Connection()..tunAvailable = tunAvailable;
    final attempt = _Attempt(connection, pending: pending);
    final backend = _Backend(() => attempt.replacement ?? attempt);
    final controller = ConnectionController(
      settings,
      nativeConnectionBackend: backend,
      systemProxyAvailable: true,
      desktopTunPlatform: desktopTun ? _TunPlatform(tunAvailable) : null,
    )..ssoSessionProvider = () => auth;
    addTearDown(() {
      if (!attempt.controllerDisposed) controller.dispose();
      backend.dispose();
      settings.dispose();
    });
    return (controller, attempt, connection);
  }

  for (final terminal in [false, true]) {
    test(
      'listeners receive idle state after terminal=$terminal cleanup',
      () async {
        final (controller, _, _) = await setup();
        await controller.connect();
        final observedBusy = <bool>[];
        controller.addListener(() => observedBusy.add(controller.busy));
        if (terminal) {
          await controller.disconnectForExit();
        } else {
          await controller.disconnect();
        }
        expect(controller.busy, isFalse);
        expect(controller.state, ConnectionStatus.disconnected);
        expect(
          observedBusy.last,
          isFalse,
          reason:
              'The UI must be told that the command future is no longer busy',
        );
      },
    );
  }

  test('a native factory failure clears the connecting state', () async {
    SharedPreferences.setMockInitialValues({});
    final settings = await StubSettingsStore.load();
    addTearDown(settings.dispose);
    final controller = ConnectionController(
      settings,
      nativeConnectionBackend: StubNativeBackend(
        failure: 'fixture factory failed',
      ),
    )..ssoSessionProvider = () => const _Account();
    addTearDown(controller.dispose);
    expect(await controller.connect(), isFalse);
    expect(controller.busy, isFalse);
    expect(controller.state, ConnectionStatus.error);
    expect(controller.errorMessage, contains('fixture factory failed'));
    await controller.disconnectForExit();
    expect(controller.state, ConnectionStatus.disconnected);
  });

  test('the connection button cancels a pending native attempt', () async {
    final (controller, attempt, connection) = await setup(pending: true);
    connection.allowLate = true;
    final connecting = controller.connect();
    await attempt.started.future;
    expect(controller.state, ConnectionStatus.connecting);
    expect(controller.busy, isTrue);
    await controller.toggle();
    expect(attempt.cancelled, isTrue);
    attempt.complete(connection);
    expect(await connecting, isFalse);
    expect(connection.closed, isTrue);
    expect(controller.busy, isFalse);
  });

  test(
    'connection records are exposed only for the current native owner',
    () async {
      final (controller, _, connection) = await setup();
      expect(controller.observationsAvailable, isFalse);
      await controller.connect();
      connection.records = [_record()];
      expect(controller.observationsAvailable, isTrue);
      expect(
        (await controller.readObservations()).single.host,
        'fixture.example',
      );
      await controller.disconnect();
      expect(controller.observationsAvailable, isFalse);
      await expectLater(controller.readObservations(), throwsStateError);
    },
  );

  test(
    'a late record snapshot cannot be adopted after its session disconnects',
    () async {
      final (controller, _, connection) = await setup();
      await controller.connect();
      final gate = Completer<List<ConnectionObservation>>();
      connection.pendingRecords = gate;
      final checked = expectLater(
        controller.readObservations(),
        throwsA(
          isA<StateError>().having(
            (error) => error.message,
            'message',
            contains('会话已变化'),
          ),
        ),
      );
      await controller.disconnect();
      gate.complete([_record()]);
      await checked;
    },
  );

  test('system proxy follows HTTP readiness and restores before the listener closes', () async {
    final (controller, _, connection) = await setup(systemProxy: true);
    await controller.connect();
    expect(controller.systemProxyRunning, isTrue);
    expect(connection.proxyLifecycle.take(2), ['start-http', 'enable-system']);
    await controller.stopHttpProxy();
    expect(controller.systemProxyRunning, isFalse);
    expect(controller.httpProxyRunning, isFalse);
    expect(controller.socksProxyRunning, isTrue);
    expect(connection.proxyLifecycle.skip(3).take(2), [
      'restore-system',
      'stop-http',
    ]);
    await controller.disconnect();
  });

  test('a system proxy restoration failure keeps the listener and exposes cleanup errors', () async {
    final (controller, _, connection) = await setup(systemProxy: true);
    await controller.connect();
    connection.restoreFailure = true;
    await controller.stopHttpProxy();
    expect(controller.httpProxyRunning, isTrue);
    expect(controller.systemProxyRecoveryError, contains('恢复失败'));
    expect(connection.stopped, isNot(contains(NativeProxyKind.http)));
    await controller.disconnect();
    expect(controller.state, ConnectionStatus.error);
    expect(controller.errorMessage, contains('恢复失败'));
    expect(controller.systemProxyRecoveryError, isNotNull);
  });

  test(
    'session termination delegates service cleanup to one native close',
    () async {
      final (controller, _, connection) = await setup(
        systemProxy: true,
        desktopTun: true,
        vpnEnabled: true,
      );
      await controller.connect();
      expect(controller.proxyRunning, isTrue);
      expect(controller.vpnRunning, isTrue);
      connection.proxyLifecycle.clear();
      connection.tunLifecycle.clear();
      connection.stopped.clear();
      await controller.disconnectForExit();
      expect(connection.closed, isTrue);
      expect(connection.closeCalls, 1);
      expect(connection.proxyLifecycle, isEmpty);
      expect(connection.tunLifecycle, isEmpty);
      expect(connection.stopped, isEmpty);
      expect(controller.proxyRunning, isFalse);
      expect(controller.systemProxyRunning, isFalse);
      expect(controller.vpnRunning, isFalse);
      expect(controller.cleanupPending, isFalse);
    },
  );

  test(
    'a native close failure is reported and the same owner can be retried',
    () async {
      final (controller, _, connection) = await setup();
      await controller.connect();
      connection.closeFailure = true;
      await controller.disconnect();
      expect(controller.state, ConnectionStatus.error);
      expect(controller.errorMessage, contains('清理'));
      expect(connection.closed, isFalse);
      expect(connection.closeCalls, 1);
      connection.closeFailure = false;
      await controller.disconnect();
      expect(connection.closeCalls, 2);
      expect(connection.closed, isTrue);
      expect(controller.state, ConnectionStatus.disconnected);
    },
  );

  test(
    'desktop TUN follows the native session and stops independently of proxies',
    () async {
      final (controller, _, connection) = await setup(
        desktopTun: true,
        vpnEnabled: true,
      );
      expect(await controller.connect(), isTrue);
      expect(controller.vpnRunning, isTrue);
      expect(controller.vpnTunnelRoutes, ['10.21.0.0/16']);
      await controller.previewDesktopTun();
      expect(controller.desktopTunPreview?.mtu, 1400);
      await controller.stopVpn();
      expect(controller.vpnRunning, isFalse);
      expect(controller.proxyRunning, isTrue);
      expect(connection.closed, isFalse);
      await controller.disconnectForExit();
      expect(connection.closed, isTrue);
    },
  );

  test('missing desktop TUN permission preserves working proxies and reports the reason', () async {
    final (controller, _, connection) = await setup(
      desktopTun: true,
      vpnEnabled: true,
      tunAvailable: false,
    );
    expect(await controller.connect(), isTrue);
    expect(controller.vpnRunning, isFalse);
    expect(controller.proxyRunning, isTrue);
    expect(controller.desktopTunError, contains('权限'));
    expect(connection.tunLifecycle, isNot(contains('start')));
    await controller.disconnectForExit();
  });

  test(
    'desktop TUN cleanup failure keeps ownership until an explicit retry',
    () async {
      final (controller, _, connection) = await setup(
        desktopTun: true,
        vpnEnabled: true,
      );
      await controller.connect();
      connection.tunStopFailure = true;
      await controller.stopVpn();
      expect(controller.cleanupPending, isTrue);
      await expectLater(controller.disconnectForExit(), throwsStateError);
      expect(connection.closed, isFalse);
      connection.tunStopFailure = false;
      await controller.disconnectForExit();
      expect(connection.closed, isTrue);
      expect(controller.cleanupPending, isFalse);
    },
  );

  test('a successful TUN-only cleanup retry permits another start', () async {
    final (controller, _, connection) = await setup(
      desktopTun: true,
      vpnEnabled: true,
    );
    await controller.connect();
    connection.tunStopFailure = true;
    await controller.stopVpn();
    expect(controller.cleanupPending, isTrue);
    connection.tunStopFailure = false;
    await controller.stopVpn();
    expect(controller.cleanupPending, isFalse);
    expect(await controller.startVpn(), isTrue);
    await controller.disconnectForExit();
  });

  test(
    'automatic services finish before the connected controller becomes idle',
    () async {
      final (controller, _, connection) = await setup();
      final gate = Completer<NativeServicesResult>();
      connection.pendingServices = gate;
      var finished = false;
      var earlyIdle = false;
      controller.addListener(() {
        if (controller.tunnelUp && !controller.busy && !finished) {
          earlyIdle = true;
        }
      });
      final connecting = controller.connect();
      await connection.servicesStarted.future;
      expect(earlyIdle, isFalse);
      expect(controller.busy, isTrue);
      finished = true;
      gate.complete(const NativeServicesResult());
      expect(await connecting, isTrue);
      await controller.disconnect();
    },
  );

  test(
    'exit drains a late desktop TUN result without publishing it as running',
    () async {
      final (controller, _, connection) = await setup(
        desktopTun: true,
        vpnEnabled: true,
      );
      connection.pendingTun = Completer<NativeTunSnapshot>();
      final connecting = controller.connect();
      await connection.tunStarted.future;
      final exiting = controller.disconnectForExit();
      connection.pendingTun!.complete(_runningTun);
      await exiting;
      expect(await connecting, isFalse);
      expect(controller.vpnRunning, isFalse);
      expect(connection.closed, isTrue);
      expect(connection.tunLifecycle.last, 'stop');
    },
  );

  for (final rollbackThrows in [false, true]) {
    test(
      'a late failed-start rollback cannot overwrite a new TUN session ($rollbackThrows)',
      () async {
        final (controller, attempt, connection) = await setup(desktopTun: true);
        await controller.connect();
        connection.tunStartFailure = true;
        final rollback = Completer<NativeTunSnapshot>();
        connection.nextTunStop = rollback;
        final oldStart = controller.startVpn();
        await connection.tunStopped.future;
        await controller.disconnect();
        final next = _Connection();
        attempt.replacement = _Attempt(next, pending: false);
        await controller.connect();
        expect(await controller.startVpn(), isTrue);
        if (rollbackThrows) {
          rollback.completeError(StateError('old rollback failure'));
        } else {
          rollback.complete(const NativeTunSnapshot.stopped());
        }
        expect(await oldStart, isFalse);
        expect(controller.vpnRunning, isTrue);
        expect(controller.cleanupPending, isFalse);
        await controller.disconnectForExit();
      },
    );
  }

  test(
    'concurrent desktop TUN stops do not leave the controller busy',
    () async {
      final (controller, _, connection) = await setup(
        desktopTun: true,
        vpnEnabled: true,
      );
      await controller.connect();
      final stopGate = Completer<NativeTunSnapshot>();
      connection.nextTunStop = stopGate;
      final first = controller.stopVpn();
      await connection.tunStopped.future;
      final secondGate = Completer<NativeTunSnapshot>();
      connection.nextTunStop = secondGate;
      final second = controller.stopVpn();
      stopGate.complete(const NativeTunSnapshot.stopped());
      await first;
      secondGate.complete(const NativeTunSnapshot.stopped());
      await Future.wait([first, second]);
      expect(controller.busy, isFalse);
      expect(controller.vpnRunning, isFalse);
      await controller.disconnectForExit();
    },
  );

  test('desktop VPN toggle stops the running native TUN', () async {
    final (controller, _, _) = await setup(desktopTun: true, vpnEnabled: true);
    await controller.connect();
    await controller.toggleVpn();
    expect(controller.vpnRunning, isFalse);
    await controller.disconnectForExit();
  });

  test('per-launch DNS draft reaches native preview and start', () async {
    final (controller, _, connection) = await setup(desktopTun: true);
    await controller.refreshDesktopTun();
    controller.selectDesktopDns(' 1.1.1.1 ');
    controller.selectDesktopInterface('wifi');
    controller.settings.vpnCaptureMode = CaptureMode.full;
    await controller.connect();
    await controller.previewDesktopTun();
    expect(connection.previewDns, '1.1.1.1');
    expect(connection.previewMode, CaptureMode.full);
    expect(connection.previewInterface, 'wifi');
    await controller.startVpn();
    expect(connection.startedDns, '1.1.1.1');
    expect(connection.startedMode, CaptureMode.full);
    expect(connection.startedInterface, 'wifi');
    await controller.disconnectForExit();
  });

  for (final mode in CaptureMode.values) {
    test('automatic TUN startup forwards $mode', () async {
      final (controller, _, connection) = await setup(
        desktopTun: true,
        vpnEnabled: true,
      );
      controller.settings.vpnCaptureMode = mode;
      expect(await controller.connect(), isTrue);
      expect(connection.startedMode, mode);
      await controller.disconnectForExit();
    });
  }

  test('failed preview removes the previous capture plan', () async {
    final (controller, _, connection) = await setup(desktopTun: true);
    await controller.connect();
    await controller.previewDesktopTun();
    expect(controller.desktopTunPreview, isNotNull);
    connection.previewFailure = true;
    await controller.previewDesktopTun();
    expect(controller.desktopTunPreview, isNull);
    expect(controller.desktopTunError, contains('fixture preview failed'));
    await controller.disconnectForExit();
  });

  test(
    'a failed old owner cleanup prevents starting a replacement connection',
    () async {
      final (controller, attempt, connection) = await setup();
      await controller.connect();
      connection.closeFailure = true;
      await controller.disconnect();
      final replacement = _Attempt(_Connection(), pending: false);
      attempt.replacement = replacement;
      expect(await controller.connect(), isFalse);
      expect(replacement.started.isCompleted, isFalse);
      expect(connection.closed, isFalse);
      connection.closeFailure = false;
      expect(await controller.connect(), isTrue);
      expect(connection.closed, isTrue);
      expect(replacement.started.isCompleted, isTrue);
      await controller.disconnect();
    },
  );

  test(
    'desktop exit reports cleanup failure and succeeds after retry',
    () async {
      final (controller, _, connection) = await setup();
      await controller.connect();
      connection.closeFailure = true;
      await expectLater(controller.disconnectForExit(), throwsStateError);
      expect(controller.cleanupPending, isTrue);
      expect(connection.closed, isFalse);
      connection.closeFailure = false;
      await controller.disconnectForExit();
      expect(controller.cleanupPending, isFalse);
      expect(connection.closed, isTrue);
    },
  );

  test('concurrent disconnects wait for one native close', () async {
    final (controller, _, connection) = await setup();
    await controller.connect();
    connection.pendingClose = Completer<void>();
    final first = controller.disconnect();
    await connection.closeStarted.future;
    final second = controller.disconnect();
    expect(connection.closeCalls, 1);
    expect(controller.busy, isTrue);
    connection.pendingClose!.complete();
    await Future.wait([first, second]);
    expect(connection.closed, isTrue);
    expect(connection.closeCalls, 1);
    expect(controller.busy, isFalse);
  });

  test(
    'exit during reconnect cleanup cannot start a replacement attempt',
    () async {
      final (controller, attempt, connection) = await setup();
      await controller.connect();
      connection.closeFailure = true;
      await controller.disconnect();
      connection.closeFailure = false;
      connection.pendingClose = Completer<void>();
      connection.closeStarted = Completer<void>();
      final replacement = _Attempt(_Connection(), pending: false);
      attempt.replacement = replacement;
      final reconnecting = controller.connect();
      await connection.closeStarted.future;
      final exiting = controller.disconnectForExit();
      connection.pendingClose!.complete();
      await exiting;
      expect(await reconnecting, isFalse);
      expect(replacement.started.isCompleted, isFalse);
      expect(controller.state, ConnectionStatus.disconnected);
    },
  );

  test('a failed late connection close retains an owner for retry', () async {
    final (controller, attempt, connection) = await setup(pending: true);
    connection.allowLate = true;
    final connecting = controller.connect();
    await attempt.started.future;
    final disconnecting = controller.disconnect();
    connection.closeFailure = true;
    attempt.complete(connection);
    await disconnecting;
    expect(await connecting, isFalse);
    expect(controller.cleanupPending, isTrue);
    expect(connection.closed, isFalse);
    connection.closeFailure = false;
    await controller.disconnectForExit();
    expect(connection.closed, isTrue);
    expect(controller.cleanupPending, isFalse);
  });

  test(
    'desktop exit waits for a cancelled attempt and closes its late result',
    () async {
      final (controller, attempt, connection) = await setup(pending: true);
      connection.allowLate = true;
      final connecting = controller.connect();
      await attempt.started.future;
      var exited = false;
      final exiting = controller.disconnectForExit().then((_) => exited = true);
      await Future<void>.delayed(Duration.zero);
      expect(exited, isFalse);
      attempt.complete(connection);
      await exiting;
      expect(await connecting, isFalse);
      expect(connection.closed, isTrue);
    },
  );

  test('cancelling a pending system proxy enable cannot publish a late active state', () async {
    final (controller, _, connection) = await setup(systemProxy: true);
    connection.pendingSystemProxy = Completer<void>();
    final connecting = controller.connect();
    await connection.systemProxyStarted.future;
    controller.cancel();
    connection.pendingSystemProxy!.complete();
    expect(await connecting, isFalse);
    expect(connection.cancelled, isTrue);
    expect(controller.systemProxyRunning, isFalse);
    expect(controller.httpProxyRunning, isFalse);
    expect(controller.busy, isFalse);
    expect(connection.systemEnabled, isFalse);
    expect(connection.restorations, 1);
    await controller.disconnect();
  });

  test('native connection and proxy bindings update the existing public UI contract', () async {
    final (controller, attempt, connection) = await setup();
    expect(await controller.connect(), isTrue);
    expect(attempt.authSeen, isNotNull);
    expect(controller.state, ConnectionStatus.connected);
    expect(controller.tunnelUp, isTrue);
    expect(controller.virtualAddress, '10.99.0.2');
    expect(controller.vpnRunning, isFalse);
    expect(controller.httpProxyRunning, isTrue);
    expect(controller.socksProxyRunning, isTrue);
    expect(controller.httpPort, 41080);
    expect(controller.socksPort, 41081);
    connection.publish(
      const NativeConnectionUpdate(
        state: NativeConnectionState.connected,
        virtualAddress: '10.99.0.2',
        uploadBytes: 123,
        downloadBytes: 456,
        uploadRate: 12,
        downloadRate: 34,
        latencyMs: 5,
        connections: 2,
        unrouted: 1,
        tunnelFailures: 3,
      ),
    );
    expect(controller.uploadBytesPerSecond, 12);
    expect(controller.downloadBytesPerSecond, 34);
    expect(controller.trafficUpBytes, 123);
    expect(controller.trafficDownBytes, 456);
    expect(controller.trafficConnections, 2);
    expect(controller.latencyMs, 5);
    expect(controller.proxyUnroutedCount, 1);
    expect(controller.proxyTunnelFailureCount, 3);
    await controller.disconnect();
    expect(controller.state, ConnectionStatus.disconnected);
    expect(controller.tunnelUp, isFalse);
    expect(controller.proxyRunning, isFalse);
    expect(connection.closed, isTrue);
  });

  test('HTTP rebind preserves the independent SOCKS service', () async {
    final (controller, _, connection) = await setup();
    await controller.connect();
    await controller.rebindHttpProxy();
    expect(connection.stopped, [NativeProxyKind.http]);
    expect(controller.socksProxyRunning, isTrue);
    expect(controller.httpProxyRunning, isTrue);
    await controller.disconnect();
  });

  test(
    'each listener displays its native endpoint without sharing LAN hints',
    () async {
      final (controller, _, connection) = await setup();
      connection.httpBinding = const NativeProxyBinding(
        '0.0.0.0',
        41080,
        displayEndpoint: '192.0.2.10:41080',
      );
      connection.socksBinding = const NativeProxyBinding(
        '::',
        41081,
        displayEndpoint: '[2001:db8::10]:41081',
      );
      await controller.connect();
      expect(controller.httpListen.address, '0.0.0.0');
      expect(controller.socksListen.address, '::');
      expect(controller.httpProxyAddress, '192.0.2.10:41080');
      expect(controller.proxyAddress, '[2001:db8::10]:41081');
      await controller.stopHttpProxy();
      expect(controller.httpProxyAddress, isNull);
      expect(controller.httpListenAddress, isNot(contains('192.0.2.10')));
      expect(controller.proxyAddress, '[2001:db8::10]:41081');
      await controller.disconnect();
      expect(controller.proxyAddress, isNot(contains('2001:db8')));
    },
  );

  test(
    'a failed SOCKS stop retains its binding for an explicit retry',
    () async {
      final (controller, _, connection) = await setup();
      await controller.connect();
      final before = controller.proxyAddress;
      connection.socksStopFailure = true;
      await controller.stopSocksProxy();
      expect(controller.socksProxyRunning, isTrue);
      expect(controller.proxyAddress, before);
      expect(controller.errorMessage, contains('SOCKS5'));
      connection.socksStopFailure = false;
      await controller.stopSocksProxy();
      expect(controller.socksProxyRunning, isFalse);
      expect(controller.httpProxyRunning, isTrue);
      await controller.disconnect();
    },
  );

  test('native partial startup failure stays visible and can retry without dropping the tunnel', () async {
    final (controller, _, connection) = await setup();
    const socks = NativeProxyBinding(
      '127.0.0.1',
      41081,
      displayEndpoint: '127.0.0.1:41081',
    );
    connection.pendingServices = Completer<NativeServicesResult>()
      ..complete(
        const NativeServicesResult(
          socks: socks,
          failures: ['native HTTP bind failed'],
        ),
      );
    expect(await controller.connect(), isTrue);
    expect(controller.tunnelUp, isTrue);
    expect(controller.socksProxyRunning, isTrue);
    expect(controller.httpProxyRunning, isFalse);
    expect(controller.state, ConnectionStatus.error);
    expect(controller.errorMessage, 'native HTTP bind failed');
    connection.pendingServices = Completer<NativeServicesResult>()
      ..complete(
        const NativeServicesResult(
          http: NativeProxyBinding(
            '127.0.0.1',
            41080,
            displayEndpoint: '127.0.0.1:41080',
          ),
          socks: socks,
        ),
      );
    expect(await controller.startProxy(), isTrue);
    expect(controller.state, ConnectionStatus.connected);
    expect(controller.errorMessage, isNull);
    expect(connection.closed, isFalse);
    await controller.disconnect();
  });

  test('gateway input crosses the presentation boundary unchanged', () async {
    const input = ' HTTP://fixture.invalid:80/path ';
    final (controller, attempt, _) = await setup(server: input);
    await controller.connect();
    expect(attempt.serverSeen, input);
    await controller.disconnect();
  });

  for (final kind in NativeProxyKind.values) {
    test('a $kind start during stop waits for the pending command', () async {
      final (controller, _, connection) = await setup();
      await controller.connect();
      final gate = Completer<void>();
      connection.pendingStop = gate;
      final stopping = kind == NativeProxyKind.http
          ? controller.stopHttpProxy()
          : controller.stopSocksProxy();
      await connection.stopStarted.future;
      final starting = kind == NativeProxyKind.http
          ? controller.startHttpProxy()
          : controller.startSocksProxy();
      gate.complete();
      await stopping;
      expect(await starting, isTrue);
      expect(controller.httpProxyRunning, isTrue);
      expect(controller.socksProxyRunning, isTrue);
      await controller.disconnect();
    });
    for (final fails in [false, true]) {
      test(
        'late $kind stop (failure=$fails) cannot overwrite a replacement owner',
        () async {
          final (controller, attempt, old) = await setup();
          await controller.connect();
          final gate = Completer<void>();
          old.pendingStop = gate;
          final stopping = kind == NativeProxyKind.http
              ? controller.stopHttpProxy()
              : controller.stopSocksProxy();
          await old.stopStarted.future;
          await controller.disconnect();
          final replacement = _Connection();
          attempt.replacement = _Attempt(replacement, pending: false);
          expect(await controller.connect(), isTrue);
          if (fails) {
            gate.completeError(StateError('old stop failure'));
          } else {
            gate.complete();
          }
          await stopping;
          expect(controller.httpProxyRunning, isTrue);
          expect(controller.socksProxyRunning, isTrue);
          expect(controller.errorMessage, isNull);
          expect(controller.systemProxyRecoveryError, isNull);
          await controller.disconnect();
        },
      );
    }
  }

  test(
    'native failure clears live bindings and exposes the failure message',
    () async {
      final (controller, _, connection) = await setup();
      await controller.connect();
      connection.publish(
        const NativeConnectionUpdate(
          state: NativeConnectionState.failed,
          message: 'heartbeat lost',
        ),
      );
      expect(controller.state, ConnectionStatus.error);
      expect(controller.errorMessage, 'heartbeat lost');
      expect(controller.tunnelUp, isFalse);
      expect(controller.proxyRunning, isFalse);
      expect(controller.httpPort, isNull);
      await controller.disconnect();
    },
  );

  test('cancellation reaches an active native connect attempt', () async {
    final (controller, attempt, _) = await setup(pending: true);
    final result = controller.connect();
    await attempt.started.future;
    controller.cancel();
    expect(await result, isFalse);
    expect(attempt.cancelled, isTrue);
    expect(controller.busy, isFalse);
  });

  test(
    'cancellation during initial cleanup prevents starting the native attempt',
    () async {
      final (controller, attempt, _) = await setup();
      final result = controller.connect();
      controller.cancel();
      expect(await result, isFalse);
      expect(attempt.started.isCompleted, isFalse);
      expect(controller.busy, isFalse);
    },
  );

  test(
    'a late native result after disposal is closed without notifying dead UI',
    () async {
      final (controller, attempt, connection) = await setup(pending: true);
      final result = controller.connect();
      await attempt.started.future;
      connection.allowLate = true;
      attempt.controllerDisposed = true;
      controller.dispose();
      attempt.complete(connection);
      expect(await result, isFalse);
      expect(connection.closed, isTrue);
    },
  );
  test(
    'late failure from an old attempt cannot tear down a new connection',
    () async {
      final (controller, oldAttempt, oldConnection) = await setup(
        pending: true,
      );
      oldConnection.allowLate = true;
      final oldResult = controller.connect();
      await oldAttempt.started.future;
      await controller.disconnect();
      final next = _Connection();
      oldAttempt.replacement = _Attempt(next, pending: false);
      expect(await controller.connect(), isTrue);
      oldAttempt.fail(StateError('old failure'));
      expect(await oldResult, isFalse);
      expect(controller.state, ConnectionStatus.connected);
      expect(next.closed, isFalse);
      await controller.disconnect();
    },
  );

  test(
    'a connected result arriving after cancellation is rejected and closed',
    () async {
      final (controller, attempt, connection) = await setup(pending: true);
      connection.allowLate = true;
      final result = controller.connect();
      await attempt.started.future;
      controller.cancel();
      attempt.complete(connection);
      expect(await result, isFalse);
      expect(connection.closed, isTrue);
      expect(controller.state, ConnectionStatus.disconnected);
      expect(controller.busy, isFalse);
    },
  );

  test(
    'a late proxy binding cannot restore running flags after a terminal event',
    () async {
      final (controller, _, connection) = await setup();
      connection.pendingProxy = Completer<NativeProxyBinding>();
      final result = controller.connect();
      await connection.proxyStarted.future;
      connection.publish(
        const NativeConnectionUpdate(
          state: NativeConnectionState.failed,
          message: 'lost',
        ),
      );
      connection.pendingProxy!.complete(
        const NativeProxyBinding(
          '127.0.0.1',
          49999,
          displayEndpoint: '127.0.0.1:49999',
        ),
      );
      await result;
      expect(controller.state, ConnectionStatus.error);
      expect(controller.httpProxyRunning, isFalse);
      expect(controller.httpPort, isNull);
      expect(controller.busy, isFalse);
      await controller.disconnect();
    },
  );
}

class _Account implements AccountAuthentication {
  const _Account();
  @override
  bool get hasSession => true;
}

class _PermissiveSettings extends StubSettingsStore {
  _PermissiveSettings(super.fixture);

  @override
  bool isProtocolEnabled(ShuProtocol protocol) => true;
}

class _Attempt {
  _Attempt(this.connection, {required this.pending});
  final _Connection connection;
  final bool pending;
  final started = Completer<void>();
  final _result = Completer<NativeConnection>();
  bool cancelled = false;
  bool controllerDisposed = false;
  _Attempt? replacement;
  AccountAuthentication? authSeen;
  String? serverSeen;
  Future<NativeConnection> connect({
    required AccountAuthentication auth,
    required String server,
    required Duration timeout,
  }) {
    authSeen = auth;
    serverSeen = server;
    started.complete();
    return pending ? _result.future : Future.value(connection);
  }

  void complete(NativeConnection value) {
    if (!_result.isCompleted) _result.complete(value);
  }

  void fail(Object error) {
    if (!_result.isCompleted) _result.completeError(error);
  }

  void cancel() {
    cancelled = true;
    // Disposal tests can supply a late result after cancellation.
    if (pending && !connection.allowLate && !_result.isCompleted) {
      _result.completeError(StateError('cancelled'));
    }
  }

  void dispose() {}
}

/// A controlled backend response simulator. Native owner/resource invariants
/// are tested separately against the Rust actor and real FFI listeners.
class _Backend implements NativeConnectionBackend {
  _Backend(this.factory);
  final _Attempt Function() factory;
  final _updates = StreamController<NativeLifecycleSnapshot>.broadcast(
    sync: true,
  );
  final _owned = <_Connection>{};
  final _closing = <_Connection, Future<void>>{};
  final _jobs = <Future<void>>{};
  _Attempt? _attempt;
  _Connection? _current;
  Future<void>? _disconnecting;
  String? _error;
  int _generation = 0;
  bool _exiting = false;
  bool _disposed = false;
  NativeLifecyclePhase _phase = NativeLifecyclePhase.disconnected;
  @override
  NativeLifecycleSnapshot get lifecycle => NativeLifecycleSnapshot(
    generation: _generation,
    phase: _phase,
    runId: _current?.hashCode ?? _owned.firstOrNull?.hashCode,
    cleanupPending: _error != null && _owned.isNotEmpty,
    errorMessage: _error,
    errorCode: _owned.any((owner) => owner.restoreFailure)
        ? 'systemProxy'
        : 'cleanup',
  );
  @override
  Stream<NativeLifecycleSnapshot> get lifecycleUpdates => _updates.stream;
  void _publish(NativeLifecyclePhase phase) {
    _phase = phase;
    if (!_disposed) _updates.add(lifecycle);
  }

  Future<void> _close(_Connection connection) {
    final existing = _closing[connection];
    if (existing != null) return existing;
    final completed = Completer<void>();
    _closing[connection] = completed.future;
    unawaited(() async {
      try {
        await connection.close();
        _owned.remove(connection);
        if (identical(_current, connection)) _current = null;
        if (_owned.isEmpty) _error = null;
        completed.complete();
      } on Object catch (error, stack) {
        _owned.add(connection);
        _error = '$error';
        _publish(NativeLifecyclePhase.failed);
        completed.completeError(error, stack);
      } finally {
        _closing.remove(connection);
      }
    }());
    return completed.future;
  }

  @override
  Future<NativeConnection> connect({
    required AccountAuthentication auth,
    required String server,
    required Duration timeout,
    required NativeProxyPreferences proxies,
    NativeTunStartup? tun,
  }) async {
    final generation = ++_generation;
    final completed = Completer<void>();
    _jobs.add(completed.future);
    _publish(NativeLifecyclePhase.connecting);
    try {
      for (final old in _owned.toList()) {
        await _close(old);
      }
      if (_exiting || generation != _generation) throw StateError('cancelled');
      final attempt = factory();
      _attempt = attempt;
      _current = attempt.connection;
      _owned.add(attempt.connection);
      final connection = await attempt.connect(
        auth: auth,
        server: server,
        timeout: timeout,
      ) as _Connection;
      if (_exiting || generation != _generation) {
        await _close(connection);
        throw StateError('cancelled');
      }
      connection.initialServices = await connection.startServices(
        proxies,
        tun: tun,
      );
      if (_exiting ||
          generation != _generation ||
          connection.current.state != NativeConnectionState.connected) {
        await _close(connection);
        throw StateError(connection.current.message ?? 'cancelled');
      }
      _error = null;
      _publish(NativeLifecyclePhase.connected);
      return connection;
    } on Object {
      final closing = _disconnecting;
      if (generation != _generation && closing != null) {
        try {
          await closing;
        } on Object {
          /* The retained native error is asserted by the caller. */
        }
      }
      if (generation == _generation && _error == null) {
        _publish(NativeLifecyclePhase.failed);
      }
      rethrow;
    } finally {
      _jobs.remove(completed.future);
      completed.complete();
    }
  }

  @override
  void cancel() {
    _generation++;
    _attempt?.cancel();
    _current?.cancel();
    unawaited(disconnect().catchError((Object _) {}));
  }

  @override
  Future<void> disconnect() {
    final existing = _disconnecting;
    if (existing != null) return existing;
    _generation++;
    _attempt?.cancel();
    final completed = Completer<void>();
    _disconnecting = completed.future;
    _publish(NativeLifecyclePhase.disconnecting);
    unawaited(() async {
      try {
        for (final connection in _owned.toList()) {
          await _close(connection);
        }
        _publish(NativeLifecyclePhase.disconnected);
        completed.complete();
      } on Object catch (error, stack) {
        completed.completeError(error, stack);
      } finally {
        _disconnecting = null;
      }
    }());
    return completed.future;
  }

  @override
  Future<void> shutdown() async {
    _exiting = true;
    try {
      await disconnect();
      while (_jobs.isNotEmpty) {
        await Future.wait(_jobs.toList());
      }
      if (_owned.isNotEmpty) await disconnect();
      if (_error != null) throw StateError(_error!);
    } finally {
      _exiting = false;
    }
  }

  @override
  void dispose() {
    if (_disposed) return;
    cancel();
    _disposed = true;
    unawaited(_updates.close());
  }
}

const _runningTun = NativeTunSnapshot(
  state: 'running',
  cleanupPending: false,
  name: 'shuvpn_fake',
  physicalInterface: 'wifi',
  mtu: 1400,
  routes: ['10.21.0.0/16'],
);

class _TunPlatform implements NativeTunPlatform {
  _TunPlatform(this.available);
  final bool available;
  @override
  String normalizeDns(String value) => value.trim();
  @override
  Future<NativeTunEnvironment> inspect() async => NativeTunEnvironment(
    available: available,
    reason: available ? null : '需要网络管理权限',
    networks: const [
      NativeTunNetwork(name: 'wifi', description: 'Wi-Fi', recommended: true),
    ],
  );
}

class _Connection
    implements
        NativeConnection,
        NativeConnectionObservations,
        NativeSystemProxyConnection,
        NativeTunConnection {
  bool closed = false;
  @override
  NativeServicesResult? initialServices;
  @override
  void disposeView() {}
  bool closing = false;
  int restorations = 0;
  List<ConnectionObservation> records = [];
  Completer<List<ConnectionObservation>>? pendingRecords;

  @override
  Future<List<ConnectionObservation>> readObservations() async {
    final pending = pendingRecords;
    if (pending != null) return pending.future;
    return records;
  }

  bool allowLate = false;
  final stopped = <NativeProxyKind>[];
  final proxyLifecycle = <String>[];
  bool restoreFailure = false;
  bool systemEnabled = false;
  bool socksStopFailure = false;
  Completer<void>? pendingStop;
  final stopStarted = Completer<void>();
  NativeProxyBinding? httpBinding;
  NativeProxyBinding? socksBinding;
  bool closeFailure = false;
  int closeCalls = 0;
  Completer<void>? pendingClose;
  bool tunStopFailure = false;
  bool tunAvailable = true;
  bool tunStartFailure = false;
  String? previewDns;
  String? startedDns;
  Completer<NativeTunSnapshot>? nextTunStop;
  final tunStopped = Completer<void>();
  final tunLifecycle = <String>[];
  Completer<NativeTunSnapshot>? pendingTun;
  final tunStarted = Completer<void>();
  Completer<void> closeStarted = Completer<void>();
  bool cancelled = false;
  Completer<void>? pendingSystemProxy;
  final systemProxyStarted = Completer<void>();
  Completer<NativeProxyBinding>? pendingProxy;
  final proxyStarted = Completer<void>();
  final _events = StreamController<NativeConnectionUpdate>.broadcast(
    sync: true,
  );
  @override
  NativeConnectionUpdate current = const NativeConnectionUpdate(
    state: NativeConnectionState.connected,
    virtualAddress: '10.99.0.2',
    dnsServers: ['10.99.0.53'],
  );
  void publish(NativeConnectionUpdate update) {
    current = update;
    _events.add(update);
  }

  @override
  Stream<NativeConnectionUpdate> get updates => _events.stream;
  final _bindings = <NativeProxyKind, NativeProxyBinding>{};
  @override
  Future<NativeServicesResult> rebindProxy(
    NativeProxyKind kind,
    String address,
    int port, {
    required bool systemProxyEnabled,
  }) async {
    if (_bindings.containsKey(kind)) {
      await stopProxy(kind);
      await startProxy(kind, address, port);
      if (kind == NativeProxyKind.http && systemProxyEnabled) {
        await enableSystemProxy();
      }
    }
    return NativeServicesResult(
      http: _bindings[NativeProxyKind.http],
      socks: _bindings[NativeProxyKind.socks],
      systemProxyEnabled: systemEnabled,
    );
  }

  Completer<NativeServicesResult>? pendingServices;
  final servicesStarted = Completer<void>();

  // A simulated native command for presentation/lifecycle tests. Policy and
  // actual listeners are separately exercised by Rust and real-FFI tests.
  @override
  Future<NativeServicesResult> startServices(
    NativeProxyPreferences preferences, {
    NativeTunStartup? tun,
  }) async {
    if (!servicesStarted.isCompleted) servicesStarted.complete();
    if (pendingServices case final pending?) return pending.future;
    if (preferences.httpEnabled || preferences.systemProxyEnabled) {
      await startProxy(
        NativeProxyKind.http,
        preferences.httpAddress,
        preferences.httpPort,
      );
    }
    if (preferences.systemProxyEnabled) await enableSystemProxy();
    if (cancelled || closed) {
      await close();
      return const NativeServicesResult();
    }
    if (preferences.socksEnabled) {
      await startProxy(
        NativeProxyKind.socks,
        preferences.socksAddress,
        preferences.socksPort,
      );
    }
    final snapshot = tun?.enabled == true
        ? await startTun(
            mtu: tun!.mtu,
            physicalInterface: tun.physicalInterface,
            physicalDns: tun.physicalDns,
            captureMode: tun.captureMode,
          )
        : null;
    return NativeServicesResult(
      http: _bindings[NativeProxyKind.http],
      socks: _bindings[NativeProxyKind.socks],
      systemProxyEnabled: systemEnabled,
      tun: snapshot,
    );
  }

  @override
  Future<NativeProxyBinding> startProxy(
    NativeProxyKind kind,
    String address,
    int port,
  ) async {
    proxyLifecycle.add(
      kind == NativeProxyKind.http ? 'start-http' : 'start-socks',
    );
    if (!proxyStarted.isCompleted) proxyStarted.complete();
    final pending = pendingProxy;
    final supplied = kind == NativeProxyKind.http ? httpBinding : socksBinding;
    final boundPort = kind == NativeProxyKind.http ? 41080 : 41081;
    final binding = pending != null
        ? await pending.future
        : supplied ??
              NativeProxyBinding(
                address,
                boundPort,
                displayEndpoint: '$address:$boundPort',
              );
    _bindings[kind] = binding;
    return binding;
  }

  @override
  Future<void> stopProxy(NativeProxyKind kind) async {
    if (!stopStarted.isCompleted) stopStarted.complete();
    await pendingStop?.future;
    // Mirrors the native command contract, including restoration before stop.
    if (kind == NativeProxyKind.http && systemEnabled) {
      await restoreSystemProxy();
    }
    if (kind == NativeProxyKind.socks && socksStopFailure) {
      throw StateError('fixture SOCKS stop failed');
    }
    proxyLifecycle.add(
      kind == NativeProxyKind.http ? 'stop-http' : 'stop-socks',
    );
    stopped.add(kind);
    _bindings.remove(kind);
  }

  @override
  Future<void> enableSystemProxy() async {
    proxyLifecycle.add('enable-system');
    if (!systemProxyStarted.isCompleted) systemProxyStarted.complete();
    await pendingSystemProxy?.future;
    systemEnabled = true;
  }

  @override
  Future<bool> restoreSystemProxy() async {
    proxyLifecycle.add('restore-system');
    return _restoreOwnedSystemProxy();
  }

  bool _restoreOwnedSystemProxy() {
    if (restoreFailure) throw StateError('fixture restoration failed');
    restorations++;
    systemEnabled = false;
    return false;
  }

  @override
  Future<void> close() async {
    if (closed) return;
    closing = true;
    closeCalls++;
    if (!closeStarted.isCompleted) closeStarted.complete();
    if (systemProxyStarted.isCompleted) await pendingSystemProxy?.future;
    await pendingClose?.future;
    if (closeFailure) throw StateError('fixture native cleanup failed');
    if (tunStopFailure) throw StateError('fixture TUN cleanup failed');
    if (restoreFailure) throw StateError('fixture restoration failed');
    if (systemEnabled) _restoreOwnedSystemProxy();
    closed = true;
    await _events.close();
  }

  CaptureMode? previewMode;
  String? previewInterface;
  CaptureMode? startedMode;
  String? startedInterface;
  bool previewFailure = false;

  @override
  Future<NativeTunPreview> previewTun(
    int mtu, {
    String? physicalDns,
    String physicalInterface = '',
    CaptureMode captureMode = CaptureMode.campus,
  }) async {
    if (previewFailure) throw StateError('fixture preview failed');
    previewDns = physicalDns;
    previewMode = captureMode;
    previewInterface = physicalInterface;
    return const NativeTunPreview(
      mtu: 1400,
      addresses: ['10.99.0.2/32'],
      routes: ['10.21.0.0/16'],
      capturesCampusDns: true,
    );
  }

  @override
  Future<NativeTunSnapshot> startTun({
    required int mtu,
    required String physicalInterface,
    String? physicalDns,
    CaptureMode captureMode = CaptureMode.campus,
  }) async {
    if (!tunAvailable) {
      return const NativeTunSnapshot(
        state: 'stopped',
        cleanupPending: false,
        diagnostic: '需要网络管理权限',
      );
    }
    try {
      tunLifecycle.add('start');
      startedDns = physicalDns;
      startedMode = captureMode;
      startedInterface = physicalInterface;
      if (!tunStarted.isCompleted) tunStarted.complete();
      if (tunStartFailure) throw StateError('fixture TUN start failed');
      final snapshot = pendingTun == null
          ? _runningTun
          : await pendingTun!.future;
      if (closed || closing || cancelled) {
        await stopTun();
        return const NativeTunSnapshot.stopped();
      }
      return snapshot;
    } on Object catch (error) {
      try {
        final stopped = await stopTun();
        return NativeTunSnapshot(
          state: stopped.state,
          cleanupPending: stopped.cleanupPending,
          diagnostic: '$error',
        );
      } on Object catch (cleanup) {
        return NativeTunSnapshot(
          state: 'failed',
          cleanupPending: true,
          diagnostic: '$error; $cleanup',
        );
      }
    }
  }

  @override
  Future<NativeTunSnapshot> stopTun() async {
    tunLifecycle.add('stop');
    if (!tunStopped.isCompleted) tunStopped.complete();
    final pending = nextTunStop;
    nextTunStop = null;
    if (pending != null) return pending.future;
    if (tunStopFailure) {
      return const NativeTunSnapshot(
        state: 'failed',
        cleanupPending: true,
        diagnostic: 'fixture TUN cleanup failed',
      );
    }
    return const NativeTunSnapshot.stopped();
  }

  @override
  void cancel() {
    cancelled = true;
    if (!closed) {
      publish(
        const NativeConnectionUpdate(state: NativeConnectionState.disconnected),
      );
    }
  }
}

ConnectionObservation _record() => ConnectionObservation.decode({
  'flow': {
    'host': 'fixture.example',
    'ips': <String>[],
    'port': 443,
    'network': 'tcp',
    'inbound': 'http',
  },
  'decision': {
    'version': 1,
    'rule_id': 'profile:0',
    'source': 'profile',
    'policy': 'DIRECT',
    'action': 'DIRECT',
    'reason': 'fixture',
  },
  'dns': <Object?>[],
  'outbound': 'DIRECT',
  'error': null,
  'message': null,
});
