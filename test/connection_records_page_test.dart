import 'dart:async';
import 'dart:ui' show SemanticsAction, Tristate;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/app/app.dart';
import 'package:shuvpn/app/theme.dart';
import 'package:shuvpn/core/connection/connection_controller.dart';
import 'package:shuvpn/core/connection/connection_observation.dart';

import 'settings_stub.dart';
import 'native_backend_stub.dart';

import 'package:shuvpn/features/connect/connection_records_page.dart';

import 'account_stub.dart';
import 'shu_update_stub.dart';

ConnectionObservation _record(
  String host, {
  bool denied = false,
  bool dnsFailure = false,
  bool withProcess = false,
}) => ConnectionObservation.decode({
  'flow': {
    'host': host,
    'ips': <String>[],
    'port': 443,
    'network': 'tcp',
    'inbound': 'http',
    if (withProcess)
      'process': {
        'pid': 518,
        'name': 'fixture',
        'path': '/opt/fixture',
        'source': 'linux-proc-socket-snapshot',
        'confidence': 'socket-snapshot',
      },
  },
  'version': 3,
  'decision': dnsFailure
      ? null
      : {
          'version': 2,
          'rule_id': 'profile:0',
          'source': 'profile',
          'policy': denied ? '校园策略' : 'DIRECT',
          'action': denied ? 'SHU-VPN' : 'DIRECT',
          'reason': '首条匹配',
        },
  'dns': <Object?>[],
  'outbound': denied || dnsFailure ? null : 'DIRECT',
  'error': dnsFailure
      ? 'dns'
      : denied
      ? 'unrouted'
      : null,
  'message': dnsFailure
      ? '解析失败'
      : denied
      ? '网关未授权目标'
      : null,
});

class _RecordsController extends ConnectionController {
  _RecordsController(super.settings);
  bool available = true;
  int generation = 1;
  int reads = 0;
  List<ConnectionObservation> records = [];
  Future<List<ConnectionObservation>> Function()? reader;

  @override
  bool get observationsAvailable => available;
  @override
  int get observationGeneration => generation;
  @override
  Future<List<ConnectionObservation>> readObservations() {
    reads++;
    return reader?.call() ?? Future.value(records);
  }

  void changeSession(bool nextAvailable) {
    available = nextAvailable;
    generation++;
    notifyListeners();
  }
}

void main() {
  Future<_RecordsController> setup(WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final settings = await StubSettingsStore.load();
    final controller = _RecordsController(settings);
    addTearDown(controller.dispose);
    addTearDown(settings.dispose);
    tester.view.physicalSize = const Size(1200, 1600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    return controller;
  }

  Future<void> pump(
    WidgetTester tester,
    _RecordsController controller, {
    Brightness brightness = Brightness.light,
    double scale = 1,
  }) async {
    await tester.pumpWidget(
      ChangeNotifierProvider<ConnectionController>.value(
        value: controller,
        child: MaterialApp(
          key: ValueKey(brightness),
          theme: buildShuTheme(brightness),
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(scale)),
            child: child!,
          ),
          home: const ConnectionRecordsPage(),
        ),
      ),
    );
    await tester.pump();
  }

  for (final available in [false, true]) {
    testWidgets(
      'record refresh has an accessible name and capability when available=$available',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          final controller = await setup(tester)
            ..available = available;
          await pump(tester, controller);
          await tester.pumpAndSettle();
          final node = tester.getSemantics(
            find.widgetWithIcon(IconButton, Icons.refresh),
          );
          final data = node.getSemanticsData();
          expect(data.label, '刷新连接记录');
          expect(data.flagsCollection.isButton, isTrue);
          expect(
            data.flagsCollection.isEnabled,
            available ? Tristate.isTrue : Tristate.isFalse,
          );
          expect(data.hasAction(SemanticsAction.tap), available);
          if (available) {
            node.owner!.performAction(node.id, SemanticsAction.tap);
            await tester.pumpAndSettle();
            expect(controller.reads, 2);
          } else {
            expect(controller.reads, 0);
          }
        } finally {
          semantics.dispose();
        }
      },
    );
  }

  testWidgets('record detail semantics identify each label and value once', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      final controller = await setup(tester)
        ..records = [_record('semantic.example', denied: true)];
      await pump(tester, controller);
      await tester.pumpAndSettle();
      await tester.tap(find.text('semantic.example · 443'));
      await tester.pumpAndSettle();
      for (final (label, value) in [
        ('规则版本', '2'),
        ('命中规则', 'profile:0'),
        ('期望动作', 'SHU-VPN'),
        ('实际出站', '本条未记录成功出站'),
        ('错误代码', 'unrouted'),
        ('失败或拒绝原因', '网关未授权目标'),
      ]) {
        final field = find.byWidgetPredicate(
          (widget) => widget is SelectableText && widget.data == value,
        );
        final data = tester.getSemantics(field).getSemanticsData();
        expect(data.label, '$label：$value');
        expect(data.value, isEmpty);
        expect(find.semantics.byLabel(label).evaluate(), isEmpty);
      }
    } finally {
      semantics.dispose();
    }
  });

  testWidgets(
    'without a native session no request is made and refresh is disabled',
    (tester) async {
      final controller = await setup(tester)
        ..available = false;
      await pump(tester, controller);
      expect(controller.reads, 0);
      expect(find.textContaining('当前没有可读取的原生会话'), findsOneWidget);
      expect(
        tester
            .widget<IconButton>(find.widgetWithIcon(IconButton, Icons.refresh))
            .onPressed,
        isNull,
      );
    },
  );

  testWidgets(
    'history is newest first and distinguishes expected campus policy from rejection',
    (tester) async {
      final controller = await setup(tester)
        ..records = [
          _record('older.example'),
          _record('denied.example', denied: true),
        ];
      await pump(tester, controller);
      await tester.pumpAndSettle();
      final entries = tester
          .widgetList<ExpansionTile>(find.byType(ExpansionTile))
          .toList();
      expect((entries.first.title as Text).data, 'denied.example · 443');
      expect(find.textContaining('历史快照，不是活动连接列表'), findsOneWidget);
      await tester.tap(find.text('denied.example · 443'));
      await tester.pumpAndSettle();
      expect(find.text('SHU-VPN'), findsOneWidget);
      expect(find.text('本条未记录成功出站'), findsOneWidget);
      expect(find.text('网关未授权目标'), findsOneWidget);
      expect(find.text('unrouted'), findsOneWidget);
      expect(controller.reads, 1);
    },
  );

  testWidgets('native process identity appears only when recorded', (
    tester,
  ) async {
    final controller = await setup(tester)
      ..records = [_record('process.example', withProcess: true)];
    await pump(tester, controller);
    await tester.pumpAndSettle();
    await tester.tap(find.text('process.example · 443'));
    await tester.pumpAndSettle();
    expect(find.text('fixture'), findsOneWidget);
    expect(find.text('518'), findsOneWidget);
    expect(find.text('/opt/fixture'), findsOneWidget);
    expect(find.text('linux-proc-socket-snapshot'), findsOneWidget);
    expect(find.textContaining('不是历史发送者证明'), findsOneWidget);
  });

  testWidgets(
    'DNS failure before decision shows its version without a fabricated matched rule',
    (tester) async {
      final controller = await setup(tester)
        ..records = [_record('dns.example', dnsFailure: true)];
      await pump(tester, controller);
      await tester.pumpAndSettle();
      await tester.tap(find.text('dns.example · 443'));
      await tester.pumpAndSettle();
      expect(find.text('无规则决策记录'), findsOneWidget);
      expect(find.text('3'), findsOneWidget);
      expect(find.text('解析失败'), findsOneWidget);
    },
  );

  testWidgets(
    'session change clears old data and ignores a late snapshot without polling traffic updates',
    (tester) async {
      final controller = await setup(tester);
      final pending = Completer<List<ConnectionObservation>>();
      controller.reader = () => pending.future;
      await pump(tester, controller);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      controller.changeSession(false);
      await tester.pump();
      pending.complete([_record('old-private.example')]);
      await tester.pumpAndSettle();
      expect(find.textContaining('old-private.example'), findsNothing);
      expect(find.byType(LinearProgressIndicator), findsNothing);
      controller.reader = null;
      controller.records = [_record('new.example')];
      controller.changeSession(true);
      await tester.pumpAndSettle();
      expect(find.text('new.example · 443'), findsOneWidget);
      expect(controller.reads, 2);
      controller.notifyListeners();
      await tester.pump();
      expect(controller.reads, 2);
    },
  );

  testWidgets('failed refresh keeps and labels the previous snapshot', (
    tester,
  ) async {
    final controller = await setup(tester)
      ..records = [_record('retained.example')];
    await pump(tester, controller);
    await tester.pumpAndSettle();
    controller.reader = () => Future.error(StateError('fixture failure'));
    await tester.tap(find.byTooltip('刷新连接记录'));
    await tester.pumpAndSettle();
    expect(find.textContaining('保留上次快照'), findsOneWidget);
    expect(find.text('retained.example · 443'), findsOneWidget);
    expect(
      tester
          .widget<IconButton>(find.widgetWithIcon(IconButton, Icons.refresh))
          .onPressed,
      isNotNull,
    );
  });

  testWidgets(
    'new session snapshot wins even when the old request fails later',
    (tester) async {
      final controller = await setup(tester);
      final old = Completer<List<ConnectionObservation>>();
      final current = Completer<List<ConnectionObservation>>();
      controller.reader = () => old.future;
      await pump(tester, controller);
      controller.reader = () => current.future;
      controller.changeSession(true);
      await tester.pump();
      current.complete([_record('current.example')]);
      await tester.pumpAndSettle();
      old.completeError(StateError('obsolete session failure'));
      await tester.pumpAndSettle();
      expect(find.text('current.example · 443'), findsOneWidget);
      expect(find.textContaining('obsolete session failure'), findsNothing);
      expect(controller.reads, 2);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('disposing a pending record page ignores a late failure', (
    tester,
  ) async {
    final controller = await setup(tester);
    final pending = Completer<List<ConnectionObservation>>();
    controller.reader = () => pending.future;
    await pump(tester, controller);
    await tester.pumpWidget(const SizedBox.shrink());
    pending.completeError(StateError('late disposed failure'));
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'DNS evidence renders nullable server and TTL without inventing peer metadata',
    (tester) async {
      final controller = await setup(tester);
      controller.records = [
        ConnectionObservation.decode({
          'flow': {
            'host': 'resolved.example',
            'ips': <String>[],
            'port': 443,
            'network': 'tcp',
            'inbound': 'socks',
          },
          'decision': {
            'version': 1,
            'rule_id': 'profile:0',
            'source': 'profile',
            'policy': 'DIRECT',
            'action': 'DIRECT',
            'reason': 'fixture',
          },
          'dns': [
            {
              'host': 'resolved.example',
              'addresses': ['192.0.2.9'],
              'source': {'kind': 'system'},
              'cached': false,
              'ttl_seconds': null,
              'warnings': ['fixture DNS warning'],
            },
          ],
          'outbound': 'DIRECT',
          'error': null,
          'message': null,
        }),
      ];
      await pump(tester, controller);
      await tester.pumpAndSettle();
      await tester.tap(find.text('resolved.example · 443'));
      await tester.pumpAndSettle();
      expect(find.text('192.0.2.9'), findsOneWidget);
      expect(find.text('系统解析'), findsOneWidget);
      expect(find.text('fixture DNS warning'), findsOneWidget);
      expect(find.text('DNS TTL'), findsNothing);
      expect(find.text('实际连接 IP'), findsNothing);
    },
  );

  for (final brightness in Brightness.values) {
    testWidgets(
      'expanded record stays scrollable with large text in $brightness',
      (tester) async {
        final controller = await setup(tester)
          ..records = [_record('long.target.internal.invalid', denied: true)];
        tester.view.physicalSize = const Size(375, 812);
        await pump(tester, controller, brightness: brightness, scale: 1.8);
        await tester.pumpAndSettle();
        await tester.tap(find.text('long.target.internal.invalid · 443'));
        await tester.pumpAndSettle();
        await tester.scrollUntilVisible(
          find.text('网关未授权目标'),
          250,
          scrollable: find
              .descendant(
                of: find.byType(ListView),
                matching: find.byType(Scrollable),
              )
              .first,
        );
        expect(find.text('网关未授权目标'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets('desktop connect page opens records through its real router', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      SharedPreferences.setMockInitialValues({});
      final settings = await StubSettingsStore.load()
        ..welcomeCompleted = true;
      await tester.pumpWidget(
        ShuVpnApp(
          settings: settings,
          accountCenter: StubAccountCenter(),
          updateClient: StubShuUpdateClient(),
          nativeConnectionBackend: StubNativeBackend(failure: 'not dialing'),
        ),
      );
      await tester.pumpAndSettle();
      final history = tester.getSemantics(
        find.widgetWithIcon(IconButton, Icons.history),
      );
      expect(history.getSemanticsData().label, '连接记录');
      expect(history.getSemanticsData().hasAction(SemanticsAction.tap), isTrue);
      history.owner!.performAction(history.id, SemanticsAction.tap);
      await tester.pumpAndSettle();
      expect(find.byType(ConnectionRecordsPage), findsOneWidget);
      expect(find.textContaining('当前没有可读取的原生会话'), findsOneWidget);
      final back = tester.getSemantics(
        find.widgetWithIcon(IconButton, Icons.arrow_back),
      );
      expect(back.getSemanticsData().label, '返回');
      back.owner!.performAction(back.id, SemanticsAction.tap);
      await tester.pumpAndSettle();
      expect(find.byType(ConnectionRecordsPage), findsNothing);
      await tester.pumpWidget(const SizedBox.shrink());
      settings.dispose();
    } finally {
      semantics.dispose();
    }
  });
}
