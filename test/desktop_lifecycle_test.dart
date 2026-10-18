import 'dart:async';
import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';

import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:shuvpn/core/desktop/desktop_lifecycle.dart';
import 'package:shuvpn/core/desktop/desktop_port.dart';
import 'package:shuvpn/app/desktop_frame.dart';

const idle = TraySnapshot(
  status: '未连接',
  canConnect: true,
  canDisconnect: false,
  protocols: [
    TrayProtocol(id: 'atrust', label: 'aTrust', enabled: true, selected: true),
    TrayProtocol(
      id: 'openvpn',
      label: 'OpenVPN',
      enabled: false,
      selected: false,
    ),
  ],
);
const active = TraySnapshot(
  status: '已连接',
  canConnect: false,
  canDisconnect: true,
  protocols: [
    TrayProtocol(id: 'atrust', label: 'aTrust', enabled: false, selected: true),
  ],
);

class FakeDesktopPort implements DesktopPort {
  final events = <String>[];
  final snapshots = <TraySnapshot>[];
  bool host = true;
  bool failDestroy = false;
  Completer<bool>? trayGate;
  Completer<void>? menuGate;
  Future<void> Function()? closeHandler;
  void Function(DesktopCommand)? command;
  void Function(String)? protocol;
  @override
  Future<void> initializeWindow() async {}
  @override
  void listenClose(Future<void> Function()? callback) {
    closeHandler = callback;
  }

  @override
  Future<void> interceptClose(bool enabled) async {
    events.add('intercept:$enabled');
  }

  @override
  Future<bool> hasTrayHost() async => host;
  @override
  Future<bool> createTray(
    void Function(DesktopCommand) command,
    void Function(String) protocol,
  ) async {
    this.command = command;
    this.protocol = protocol;
    events.add('tray:create');
    return trayGate == null ? true : await trayGate!.future;
  }

  @override
  Future<void> updateTray(TraySnapshot snapshot) async {
    events.add('menu:${snapshot.status}');
    snapshots.add(snapshot);
    await menuGate?.future;
  }

  @override
  Future<void> destroyTray() async {
    events.add('tray:destroy');
  }

  @override
  Future<void> show() async {
    events.add('window:show');
  }

  @override
  Future<void> hide() async {
    events.add('window:hide');
  }

  @override
  Future<void> destroyWindow() async {
    events.add('window:destroy');
    if (failDestroy) {
      failDestroy = false;
      throw StateError('native destroy failed');
    }
  }

  @override
  Future<void> minimize() async {
    events.add('window:minimize');
  }

  @override
  Future<void> toggleMaximize() async {
    events.add('window:maximize');
  }

  @override
  Future<void> drag() async {}
}

DesktopLifecycle lifecycle(
  FakeDesktopPort port, {
  Future<void> Function()? disconnect,
  Future<void> Function()? shutdown,
}) => DesktopLifecycle(
  port: port,
  connect: () async {
    port.events.add('connect');
  },
  disconnect:
      disconnect ??
      () async {
        port.events.add('disconnect');
      },
  selectProtocol: (id) => port.events.add('protocol:$id'),
  shutdown:
      shutdown ??
      () async {
        port.events.add('shutdown');
      },
);

void main() {
  testWidgets(
    'desktop caption hover works above router overlay across route and theme updates',
    (tester) async {
      final port = FakeDesktopPort();
      final theme = ValueNotifier(ThemeMode.light);
      final router = GoRouter(
        initialLocation: '/first',
        routes: [
          GoRoute(
            path: '/first',
            builder: (_, _) => const Scaffold(body: Text('first page')),
          ),
          GoRoute(
            path: '/second',
            builder: (_, _) => const Scaffold(body: Text('second page')),
          ),
        ],
      );
      addTearDown(() async {
        await tester.pumpWidget(const SizedBox.shrink());
        router.dispose();
        theme.dispose();
      });
      await tester.pumpWidget(
        ValueListenableBuilder<ThemeMode>(
          valueListenable: theme,
          builder: (_, mode, _) => MaterialApp.router(
            theme: ThemeData.light(),
            darkTheme: ThemeData.dark(),
            themeMode: mode,
            routerConfig: router,
            builder: (_, child) => DesktopFrame(port: port, child: child!),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('first page'), findsOneWidget);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      for (final label in ['最小化', '最大化或还原', '关闭并退出']) {
        await mouse.moveTo(tester.getCenter(find.byTooltip(label)));
        await tester.pump(const Duration(seconds: 1));
        await tester.pump(const Duration(milliseconds: 200));
        expect(tester.takeException(), isNull);
        expect(find.text(label), findsOneWidget);
        await mouse.moveTo(const Offset(1, 100));
        await tester.pump(const Duration(milliseconds: 500));
      }
      router.go('/second');
      theme.value = ThemeMode.dark;
      await tester.pumpAndSettle();
      expect(find.text('first page'), findsNothing);
      expect(find.text('second page'), findsOneWidget);
      expect(
        Theme.of(tester.element(find.byTooltip('最小化'))).brightness,
        Brightness.dark,
      );
      await mouse.moveTo(tester.getCenter(find.byTooltip('最小化')));
      await tester.pump(const Duration(seconds: 1));
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.text('最小化'), findsOneWidget);
      await tester.pumpWidget(const SizedBox.shrink());
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'desktop caption exposes labelled window controls and preserves page content',
    (tester) async {
      final port = FakeDesktopPort();
      await tester.pumpWidget(
        MaterialApp(
          home: DesktopFrame(
            port: port,
            child: const Scaffold(body: Text('existing page')),
          ),
        ),
      );
      expect(find.text('existing page'), findsOneWidget);
      await tester.tap(find.byTooltip('最小化'));
      await tester.pump();
      await tester.tap(find.byTooltip('最大化或还原'));
      await tester.pump();
      expect(port.events, containsAll(['window:minimize', 'window:maximize']));
      await tester.tap(find.byTooltip('关闭并退出'));
      await tester.pump();
      expect(port.events, contains('window:destroy'));
    },
  );

  test(
    'dispose waits for an in-flight menu update before destroying its tray',
    () async {
      final port = FakeDesktopPort();
      final desktop = lifecycle(port)..update(idle);
      await desktop.start();
      port.menuGate = Completer<void>();
      desktop.update(active);
      desktop.dispose();
      await Future<void>.delayed(Duration.zero);
      expect(port.events, isNot(contains('tray:destroy')));
      port.menuGate!.complete();
      await Future<void>.delayed(Duration.zero);
      expect(port.events.last, 'tray:destroy');
    },
  );

  test(
    'failed native window destruction restores close interception for retry',
    () async {
      final port = FakeDesktopPort()..failDestroy = true;
      final desktop = lifecycle(port)..update(idle);
      addTearDown(desktop.dispose);
      await desktop.start();
      await desktop.quit();
      expect(
        port.events.where((event) => event.startsWith('intercept:')).last,
        'intercept:true',
      );
      expect(desktop.quitting, isFalse);
      await desktop.windowClose();
      expect(port.events.where((event) => event == 'shutdown'), hasLength(2));
    },
  );

  test('closing with a tray hides without disconnecting and connect first restores the window', () async {
    final port = FakeDesktopPort();
    final desktop = lifecycle(port)..update(idle);
    addTearDown(desktop.dispose);
    await desktop.start();
    await port.closeHandler!();
    expect(port.events, contains('window:hide'));
    expect(port.events, isNot(contains('disconnect')));
    await desktop.dispatch(DesktopCommand.connect);
    expect(
      port.events.indexOf('window:show'),
      lessThan(port.events.indexOf('connect')),
    );
  });

  test(
    'without a tray host close disconnects before destroying the window',
    () async {
      final port = FakeDesktopPort()..host = false;
      final desktop = lifecycle(port)..update(idle);
      addTearDown(desktop.dispose);
      await desktop.start();
      await desktop.windowClose();
      expect(port.events, isNot(contains('window:hide')));
      expect(
        port.events.indexOf('shutdown'),
        lessThan(port.events.indexOf('window:destroy')),
      );
      expect(desktop.error, contains('托盘'));
    },
  );

  test('losing the tray host does not hide an unreachable window', () async {
    final port = FakeDesktopPort();
    final desktop = lifecycle(port)..update(idle);
    addTearDown(desktop.dispose);
    await desktop.start();
    port.host = false;
    await desktop.windowClose();
    expect(port.events, contains('window:destroy'));
    expect(port.events, isNot(contains('window:hide')));
  });

  test(
    'protocol and connection commands respect the published capabilities',
    () async {
      final port = FakeDesktopPort();
      final desktop = lifecycle(port)..update(idle);
      addTearDown(desktop.dispose);
      await desktop.start();
      desktop.protocol('openvpn');
      desktop.protocol('atrust');
      expect(port.events, contains('protocol:atrust'));
      expect(port.events, isNot(contains('protocol:openvpn')));
      desktop.update(active);
      await desktop.dispatch(DesktopCommand.connect);
      desktop.protocol('atrust');
      expect(
        port.events.where((event) => event == 'protocol:atrust'),
        hasLength(1),
      );
      expect(port.events, isNot(contains('connect')));
      await desktop.dispatch(DesktopCommand.disconnect);
      expect(port.events, contains('disconnect'));
    },
  );

  test(
    'native protocol activation republishes an unchanged selection',
    () async {
      final port = FakeDesktopPort();
      final desktop = lifecycle(port)..update(idle);
      addTearDown(desktop.dispose);
      await desktop.start();
      port.protocol!('atrust');
      await Future<void>.delayed(Duration.zero);
      expect(port.snapshots, hasLength(2));
      expect(port.snapshots.last.protocols.first.selected, isTrue);
      expect(
        port.events.where((event) => event == 'protocol:atrust'),
        hasLength(1),
      );
    },
  );

  test('native protocol activation during a menu update restores the latest capabilities', () async {
    final port = FakeDesktopPort();
    final desktop = lifecycle(port)..update(idle);
    addTearDown(desktop.dispose);
    await desktop.start();
    port.menuGate = Completer<void>();
    desktop.update(active);
    port.protocol!('atrust');
    port.menuGate!.complete();
    await Future<void>.delayed(Duration.zero);
    expect(port.snapshots, hasLength(3));
    expect(port.snapshots.last.sameAs(active), isTrue);
    expect(port.events.where((event) => event == 'protocol:atrust'), isEmpty);
  });

  test(
    'duplicate quit waits for one cleanup and a late tray init is removed',
    () async {
      final port = FakeDesktopPort()..trayGate = Completer<bool>();
      final cleanup = Completer<void>();
      final desktop = lifecycle(
        port,
        shutdown: () {
          port.events.add('shutdown');
          return cleanup.future;
        },
      )..update(idle);
      addTearDown(desktop.dispose);
      final start = desktop.start();
      await Future<void>.delayed(Duration.zero);
      final first = desktop.quit();
      final second = desktop.quit();
      cleanup.complete();
      expect(port.events, isNot(contains('window:destroy')));
      port.trayGate!.complete(true);
      await start;
      await Future.wait([first, second]);
      expect(port.events.where((event) => event == 'shutdown'), hasLength(1));
      expect(
        port.events.where((event) => event == 'window:destroy'),
        hasLength(1),
      );
      expect(desktop.trayReady, isFalse);
    },
  );

  test(
    'failed cleanup keeps the window usable and quit can be retried',
    () async {
      final port = FakeDesktopPort();
      var attempts = 0;
      final desktop = lifecycle(
        port,
        shutdown: () async {
          if (attempts++ == 0) throw StateError('cleanup failed');
        },
      )..update(idle);
      addTearDown(desktop.dispose);
      await desktop.start();
      await desktop.quit();
      expect(port.events, contains('window:show'));
      expect(port.events, isNot(contains('window:destroy')));
      expect(desktop.quitting, isFalse);
      await desktop.quit();
      expect(port.events, contains('window:destroy'));
      expect(attempts, 2);
    },
  );

  test('menu updates coalesce and traffic-equivalent snapshots do not rebuild the menu', () async {
    final port = FakeDesktopPort();
    final desktop = lifecycle(port)..update(idle);
    addTearDown(desktop.dispose);
    await desktop.start();
    desktop.update(idle);
    await Future<void>.delayed(Duration.zero);
    expect(
      port.events.where((event) => event.startsWith('menu:')),
      hasLength(1),
    );
    port.menuGate = Completer<void>();
    desktop.update(active);
    desktop.update(idle);
    port.menuGate!.complete();
    await Future<void>.delayed(Duration.zero);
    expect(
      port.events.where((event) => event.startsWith('menu:')).last,
      'menu:未连接',
    );
  });

  test('dispose during startup releases close interception and discards the late icon', () async {
    final port = FakeDesktopPort()..trayGate = Completer<bool>();
    final desktop = lifecycle(port)..update(idle);
    final start = desktop.start();
    await Future<void>.delayed(Duration.zero);
    desktop.dispose();
    port.trayGate!.complete(true);
    await start;
    await Future<void>.delayed(Duration.zero);
    expect(port.closeHandler, isNull);
    expect(port.events.last, 'tray:destroy');
    expect(port.events, contains('intercept:false'));
  });
}
