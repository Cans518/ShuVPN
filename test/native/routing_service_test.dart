import 'dart:io';
import 'dart:convert';

import 'package:crypto/crypto.dart';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shuvpn/core/rust/routing_service.dart';
import 'package:shuvpn/core/rust/generated/routing/model.dart' as native_model;
import 'package:shuvpn/features/settings/routing_settings_page.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
    'process preview inputs reach the native matcher and show platform scope',
    (tester) async {
      final setup = await tester.runAsync(() async {
        final directory = await Directory.systemTemp.createTemp(
          'shuvpn-process-ui-',
        );
        final service = await RoutingService.open(directory.path);
        await service.prepare(
          '{"automatic_campus":false,"rules":["PROCESS-NAME,fixture,REJECT","MATCH,DIRECT"]}',
          '{}',
        );
        await service.applyPreview();
        return (directory, service);
      });
      if (setup == null) throw StateError('Native setup did not complete');
      final (directory, service) = setup;
      addTearDown(() {
        service.dispose();
        return directory.delete(recursive: true);
      });
      tester.view.physicalSize = const Size(1000, 4000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        ChangeNotifierProvider<RoutingService?>.value(
          value: service,
          child: const MaterialApp(home: RoutingSettingsPage()),
        ),
      );
      final name = find.byKey(const ValueKey('routing-process-name'));
      await _pumpUntil(tester, () => name.evaluate().isNotEmpty);
      final run = find.widgetWithText(TextButton, '预览 HTTP / TCP 决策');
      await _pumpUntil(
        tester,
        () =>
            run.evaluate().isNotEmpty &&
            tester.widget<TextButton>(run).onPressed != null,
      );
      await tester.enterText(name, 'fixture');
      await tester.ensureVisible(run);
      await tester.tap(run);
      await _pumpUntil(
        tester,
        () => find.textContaining('· REJECT · 版本').evaluate().isNotEmpty,
      );
      await _expectPreviewSemantics(tester, '规则 profile:0 · REJECT · 版本 1');
      expect(find.textContaining('Windows 进程规则'), findsOneWidget);
      expect(find.textContaining('Linux 进程规则'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  test('native process rules expose capabilities and require explicit preview identity', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-process-ffi-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final service = await RoutingService.open(directory.path);
    addTearDown(service.dispose);
    expect(
      service.capabilities['supported'],
      containsAll(['PROCESS-NAME', 'PROCESS-PATH']),
    );
    expect(
      service.capabilities['processIdentityMatrix'],
      containsPair('Windows', isA<Map>()),
    );
    expect(
      service.capabilities['processIdentityMatrix'],
      containsPair('Linux', isA<Map>()),
    );
    await service.prepare(
      '{"automatic_campus":false,"rules":["PROCESS-NAME,fixture,REJECT","MATCH,DIRECT"]}',
      '{}',
    );
    expect(service.error, isNull);
    await service.applyPreview();
    const flow = {
      'host': 'process.example',
      'port': 443,
      'network': 'tcp',
      'inbound': 'http',
    };
    await expectLater(
      service.test(jsonEncode(flow), '[]'),
      throwsA(
        isA<native_model.RoutingFailure>().having(
          (error) => error.code,
          'code',
          'process',
        ),
      ),
    );
    final result = jsonDecode(
      await service.test(
        jsonEncode({
          ...flow,
          'process': {'name': 'fixture'},
        }),
        '[]',
      ),
    ) as Map<String, Object?>;
    expect((result['decision'] as Map)['action'], 'REJECT');
  });

  test('native draft and compiled preview explain exact duplicate condition priority', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-shadow-ffi-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final service = await RoutingService.open(directory.path);
    addTearDown(service.dispose);
    const profile =
        'automatic_campus: false\nrules: ["DOMAIN,duplicate.example,DIRECT", "MATCH,DIRECT"]';
    const overrides = 'prepend: ["DOMAIN,duplicate.example,REJECT"]';
    final draft = await service.inspectDraft(profile, overrides);
    expect(draft.rules[1].shadowedBy, 'prepend:0');
    await service.prepare(profile, overrides);
    expect(service.error, isNull);
    expect(service.preview!.notices.join(' '), contains('profile:0'));
    await service.applyPreview();
    final result = jsonDecode(
      await service.test(
        '{"host":"duplicate.example","port":443,"network":"tcp","inbound":"http"}',
        '[]',
      ),
    ) as Map<String, Object?>;
    expect((result['decision'] as Map)['rule_id'], 'prepend:0');
    expect((result['decision'] as Map)['action'], 'REJECT');
  });

  test('pinned GeoSite dat previews version provenance and matches through real FFI', () async {
    final directory = await Directory.systemTemp.createTemp('shuvpn-geo-ffi-');
    addTearDown(() => directory.delete(recursive: true));
    final domain = [8, 3, 18, 11, ...utf8.encode('geo.example')];
    final group = [10, 4, ...utf8.encode('TEST'), 18, domain.length, ...domain];
    final bytes = [10, group.length, ...group];
    final file = File('${directory.path}${Platform.pathSeparator}site.dat');
    await file.writeAsBytes(bytes);
    final service = await RoutingService.open(directory.path);
    addTearDown(service.dispose);
    expect(
      service.capabilities['supported'],
      containsAll(['GEOIP', 'GEOSITE']),
    );
    await service.prepare(
      jsonEncode({
        'automatic_campus': false,
        'rules': ['GEOSITE,test,REJECT', 'MATCH,DIRECT'],
        'rule-providers': {
          'site': {
            'type': 'file',
            'behavior': 'geosite',
            'format': 'dat',
            'path': 'site.dat',
            'data': {
              'code': 'TEST',
              'version': 'ffi-v1',
              'sha256': sha256.convert(bytes).toString(),
            },
          },
        },
      }),
      '{}',
    );
    expect(service.error, isNull);
    expect(service.preview?.notices.join(' '), contains('ffi-v1'));
    await service.applyPreview();
    final selected = jsonDecode(
      await service.test(
        '{"host":"geo.example","port":443,"network":"tcp","inbound":"http"}',
        '[]',
      ),
    ) as Map<String, Object?>;
    expect((selected['decision'] as Map)['action'], 'REJECT');
    await file.writeAsString('wrong digest');
    await service.refreshProviders();
    expect(service.error, isNotNull);
    expect(service.current.version, 1);
  });

  test(
    'logic and regex capabilities preview, apply and survive an invalid update',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-logic-ffi-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final service = await RoutingService.open(directory.path);
      addTearDown(service.dispose);
      expect(
        service.capabilities['supported'],
        containsAll(['AND', 'OR', 'NOT', 'DOMAIN-REGEX']),
      );
      expect(service.capabilities['unsupported'], contains('SCRIPT'));
      expect(service.capabilities['supported'], contains('PROCESS-NAME'));
      await service.prepare(
        jsonEncode({
          'name': 'logic',
          'automatic_campus': false,
          'rules': [
            r'AND,((DOMAIN-REGEX,^api[0-9]{1,3}[.]example$),(NOT,((NETWORK,udp)))),REJECT',
            'MATCH,DIRECT',
          ],
        }),
        '{}',
      );
      expect(service.error, isNull);
      expect(service.preview?.version, 1);
      await service.applyPreview();
      final selected = jsonDecode(
        await service.test(
          jsonEncode({
            'host': 'API123.EXAMPLE.',
            'port': 443,
            'network': 'tcp',
            'inbound': 'http',
          }),
          '[]',
        ),
      ) as Map<String, Object?>;
      expect((selected['decision'] as Map)['action'], 'REJECT');
      await service.prepare(
        jsonEncode({
          'rules': [
            'NOT,((DOMAIN,a.example),(DOMAIN,b.example)),REJECT',
            'MATCH,DIRECT',
          ],
        }),
        '{}',
      );
      expect(service.error, isNotNull);
      expect(service.current.version, 1);
    },
  );

  test(
    'native rule preview commits once and survives restart and rollback',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-routing-test-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final service = await RoutingService.open(directory.path);
      expect(service.current.version, 0);
      await service.prepare(
        'name: explicit\nrules: ["DOMAIN,blocked.example,REJECT", "MATCH,DIRECT"]',
        '{}',
      );
      expect(service.error, isNull);
      expect(service.preview?.version, 1);
      expect(service.current.version, 0);
      await service.applyPreview();
      expect(service.current.version, 1);
      expect(service.preview, isNull);
      final result = jsonDecode(
        await service.test(
          '{"host":"blocked.example","port":443,"network":"tcp","inbound":"http"}',
          '[]',
        ),
      ) as Map<String, Object?>;
      expect((result['decision'] as Map<String, Object?>)['action'], 'REJECT');
      await service.prepare('rules: ["GEOIP,CN,DIRECT", "MATCH,DIRECT"]', '{}');
      expect(service.error, isNotNull);
      expect(service.current.version, 1);
      service.dispose();
      final restored = await RoutingService.open(directory.path);
      expect(restored.current.version, 1);
      await restored.rollback();
      expect(restored.current.version, 2);
      expect(restored.current.profile, isEmpty);
      restored.dispose();
    },
  );

  test('native refresh events invalidate stale previews without overwriting edits', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-refresh-ffi-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final source = File(
      '${directory.path}${Platform.pathSeparator}domains.yaml',
    );
    await source.writeAsString("payload: ['old.example']");
    final service = await RoutingService.open(directory.path);
    addTearDown(service.dispose);
    const profile =
        'name: subscribed\nrules: ["RULE-SET,p,REJECT", "MATCH,DIRECT"]\n'
        'rule-providers:\n p:\n  type: file\n  behavior: domain\n  path: domains.yaml\n';
    const edits = 'prepend: ["DOMAIN,override.example,DIRECT"]';
    await service.prepare(profile, edits);
    await service.applyPreview();
    await service.prepare(profile, edits);
    expect(service.preview, isNotNull);
    await source.writeAsString("payload: ['new.example']");
    await service.manager.refresh(expectedVersion: BigInt.one);
    await _waitFor(
      () => service.current.version == 2 && service.lastChecked != null,
    );
    expect(service.preview, isNull);
    expect(service.current.overrides, edits);
    expect(service.error, contains('重新'));
    expect(service.lastChecked, isNotNull);
    await source.writeAsString('payload: [');
    await service.refreshProviders();
    expect(service.current.version, 2);
    expect(service.error, isNotNull);
    await _waitFor(() => service.refreshError != null);
    expect(service.refreshing, isFalse);
    await source.writeAsString("payload: ['newest.example']");
    await service.prepare(profile, edits);
    expect(service.preview, isNotNull);
    await service.refreshProviders();
    expect(service.preview, isNull);
    expect(service.current.version, 3);
  });

  test('invalidating edits while native preparation runs discards the late preview', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-preview-revision-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final service = await RoutingService.open(directory.path);
    addTearDown(service.dispose);
    final preparation = service.prepare('rules: ["MATCH,DIRECT"]', '{}');
    service.invalidatePreview();
    await preparation;
    expect(service.preview, isNull);
    expect(service.error, contains('内容已修改'));
    expect(service.current.version, 0);
  });

  testWidgets(
    'rule editor requires a successful preview before applying a template',
    (tester) async {
      final setup = await tester.runAsync(() async {
        final directory = await Directory.systemTemp.createTemp(
          'shuvpn-routing-ui-test-',
        );
        final service = await RoutingService.open(directory.path);
        return (directory, service);
      });
      if (setup == null) throw StateError('Native test setup did not complete');
      final (directory, service) = setup;
      addTearDown(() => directory.delete(recursive: true));
      addTearDown(service.dispose);
      tester.view.physicalSize = const Size(1200, 2400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        ChangeNotifierProvider<RoutingService?>.value(
          value: service,
          child: const MaterialApp(home: RoutingSettingsPage()),
        ),
      );
      expect(
        tester
            .widget<OutlinedButton>(find.widgetWithText(OutlinedButton, '应用预览'))
            .onPressed,
        isNull,
      );
      await tester.runAsync(() async {
        await tester.tap(find.text('校园优先模板'));
        final editor = tester.widget<TextField>(
          find.byKey(const ValueKey('routing-profile'), skipOffstage: false),
        );
        await _waitFor(() => editor.controller!.text.contains('MATCH,DIRECT'));
      });
      await tester.pump();
      await tester.runAsync(() async {
        await tester.tap(find.text('校验并预览'));
        await _waitFor(() => !service.busy);
      });
      await tester.pump();
      expect(service.preview, isNotNull, reason: service.error);
      expect(
        tester
            .widget<OutlinedButton>(find.widgetWithText(OutlinedButton, '应用预览'))
            .onPressed,
        isNotNull,
      );
      await tester.runAsync(() async {
        await tester.tap(find.text('应用预览'));
        await _waitFor(() => !service.busy);
      });
      await tester.pump();
      expect(service.current.version, 1);
      expect(find.text('校园优先'), findsNWidgets(2));
      const draft = 'name: draft\nrules: ["MATCH,REJECT"]';
      await tester.tap(find.byKey(const ValueKey('routing-advanced')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('routing-profile')));
      await tester.enterText(
        find.byKey(const ValueKey('routing-profile')),
        draft,
      );
      await tester.runAsync(() async {
        await tester.tap(find.text('校验并预览'));
        await _waitFor(() => !service.busy);
        final saved = File(
          '${directory.path}${Platform.pathSeparator}routing-state.json',
        );
        await saved.delete();
        await Directory(saved.path).create();
      });
      await tester.pump();
      expect(service.preview?.name, 'draft');
      await tester.runAsync(() async {
        await tester.tap(find.text('回退上一版本'));
        await _waitFor(() => !service.busy);
      });
      await tester.pump();
      expect(service.error, isNotNull);
      expect(service.preview?.name, 'draft');
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('routing-profile')))
            .controller
            ?.text,
        draft,
      );
    },
  );

  testWidgets(
    'native rule cards change only the draft, invalidate preview and persist after apply',
    (tester) async {
      final setup = await tester.runAsync(() async {
        final directory = await Directory.systemTemp.createTemp(
          'shuvpn-rule-cards-',
        );
        final service = await RoutingService.open(directory.path);
        await service.prepare(
          jsonEncode({
            'name': 'visual',
            'automatic_campus': false,
            'rules': [
              'DOMAIN,target.example,REJECT',
              'DOMAIN,target.example,DIRECT',
              'MATCH,DIRECT',
            ],
            'proxy-groups': [
              {
                'name': 'choice',
                'type': 'select',
                'proxies': ['DIRECT', 'SHU-VPN'],
              },
            ],
          }),
          '{}',
        );
        await service.applyPreview();
        await service.prepare(
          service.current.profile,
          service.current.overrides,
        );
        return (directory, service);
      });
      if (setup == null) throw StateError('Native setup did not complete');
      final (directory, service) = setup;
      expect(service.preview, isNotNull);
      addTearDown(() => directory.delete(recursive: true));
      var serviceDisposed = false;
      addTearDown(() {
        if (!serviceDisposed) service.dispose();
      });
      tester.view.physicalSize = const Size(1000, 2400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        ChangeNotifierProvider<RoutingService?>.value(
          value: service,
          child: const MaterialApp(home: RoutingSettingsPage()),
        ),
      );
      final toggle = find.byKey(const ValueKey('routing-toggle-profile-0'));
      await _pumpUntil(tester, () => toggle.evaluate().isNotEmpty);
      expect(service.preview, isNull);
      final editor = tester
          .widget<TextField>(
            find.byKey(const ValueKey('routing-profile'), skipOffstage: false),
          )
          .controller!;
      await tester.runAsync(
        () =>
            service.prepare(service.current.profile, service.current.overrides),
      );
      await tester.pump();
      expect(service.preview, isNotNull);
      await tester.ensureVisible(toggle);
      await tester.tap(toggle);
      await _pumpUntil(tester, () => editor.text.contains('"enabled": false'));
      expect(service.preview, isNull);
      expect(service.current.version, 1);
      final activeBeforeApply = await tester.runAsync(
        () => service.test(
          '{"host":"target.example","port":443,"network":"tcp","inbound":"http"}',
          '[]',
        ),
      );
      expect(
        (jsonDecode(activeBeforeApply!) as Map)['decision']['action'],
        'REJECT',
      );
      expect(
        tester
            .widget<OutlinedButton>(find.widgetWithText(OutlinedButton, '应用预览'))
            .onPressed,
        isNull,
      );

      final move = find.byTooltip('上移 DOMAIN,target.example,DIRECT');
      await tester.ensureVisible(move);
      await tester.tap(move);
      await _pumpUntil(
        tester,
        () =>
            (jsonDecode(editor.text) as Map)['rules'][0] ==
            'DOMAIN,target.example,DIRECT',
      );
      final group = find.byKey(const ValueKey('routing-group-choice'));
      await tester.ensureVisible(group);
      await tester.tap(group);
      await tester.pumpAndSettle();
      await tester.tap(find.text('SHU-VPN').last);
      await tester.pump();
      final overrides = tester
          .widget<TextField>(
            find.byKey(
              const ValueKey('routing-overrides'),
              skipOffstage: false,
            ),
          )
          .controller!;
      await _pumpUntil(tester, () => overrides.text.contains('SHU-VPN'));
      final previewButton = find.text('校验并预览');
      await tester.ensureVisible(previewButton);
      await tester.runAsync(() async {
        await tester.tap(previewButton);
        await _waitFor(() => !service.busy);
      });
      await tester.pump();
      expect(service.error, isNull);
      expect(service.preview, isNotNull);
      await tester.runAsync(() async {
        await tester.tap(find.text('应用预览'));
        await _waitFor(() => !service.busy);
      });
      await tester.pump();
      expect(service.current.version, 2);
      await tester.pumpWidget(const SizedBox.shrink());
      service.dispose();
      serviceDisposed = true;
      final restored = await tester.runAsync(
        () => RoutingService.open(directory.path),
      );
      if (restored == null) throw StateError('Native reopen did not complete');
      addTearDown(restored.dispose);
      expect(restored.current.version, 2);
      final restoredDraft = await tester.runAsync(
        () => restored.inspectDraft(
          restored.current.profile,
          restored.current.overrides,
        ),
      );
      expect(restoredDraft!.rules[1].enabled, isFalse);
      expect(restoredDraft.rules.last.finalRule, isTrue);
      expect(restoredDraft.groups.single.selected, 'SHU-VPN');
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'rule tester sends the selected network and inbound to native matching',
    (tester) async {
      final setup = await tester.runAsync(() async {
        final directory = await Directory.systemTemp.createTemp(
          'shuvpn-flow-tester-',
        );
        final service = await RoutingService.open(directory.path);
        await service.prepare(
          'automatic_campus: false\nrules: ["AND,((NETWORK,udp),(IN-TYPE,socks)),REJECT", "MATCH,DIRECT"]',
          '{}',
        );
        await service.applyPreview();
        return (directory, service);
      });
      if (setup == null) throw StateError('Native setup did not complete');
      final (directory, service) = setup;
      addTearDown(() => directory.delete(recursive: true));
      addTearDown(service.dispose);
      tester.view.physicalSize = const Size(1000, 2400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        ChangeNotifierProvider<RoutingService?>.value(
          value: service,
          child: const MaterialApp(home: RoutingSettingsPage()),
        ),
      );
      await _pumpUntil(
        tester,
        () => find
            .byKey(const ValueKey('routing-toggle-profile-0'))
            .evaluate()
            .isNotEmpty,
      );
      final firstTest = find.text('预览 HTTP / TCP 决策');
      await tester.ensureVisible(firstTest);
      await tester.tap(firstTest);
      await _pumpUntil(
        tester,
        () => find.textContaining('· DIRECT · 版本').evaluate().isNotEmpty,
      );
      for (final selection in [
        ('routing-test-network', 'UDP'),
        ('routing-test-inbound', 'SOCKS'),
      ]) {
        final field = find.byKey(ValueKey(selection.$1));
        await tester.ensureVisible(field);
        await tester.tap(field);
        await tester.pumpAndSettle();
        await tester.tap(find.text(selection.$2).last);
        await tester.pumpAndSettle();
      }
      final selectedTest = find.text('预览 SOCKS / UDP 决策');
      await tester.ensureVisible(selectedTest);
      await tester.tap(selectedTest);
      await _pumpUntil(
        tester,
        () => find.textContaining('· REJECT · 版本').evaluate().isNotEmpty,
      );
      expect(service.current.version, 1);
      expect(tester.takeException(), isNull);
    },
  );
}

Future<void> _expectPreviewSemantics(
  WidgetTester tester,
  String expected,
) async {
  final semantics = tester.ensureSemantics();
  try {
    await tester.pump();
    final data = tester.getSemantics(find.text(expected)).getSemanticsData();
    expect(data.label, expected);
    expect(data.value, isEmpty);
  } finally {
    semantics.dispose();
  }
}

Future<void> _pumpUntil(WidgetTester tester, bool Function() condition) async {
  for (var attempt = 0; attempt < 500; attempt++) {
    if (condition()) return;
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    await tester.pump();
  }
  throw StateError('Editor did not update within five seconds');
}

Future<void> _waitFor(bool Function() condition) async {
  final deadline = DateTime.now().add(const Duration(seconds: 5));
  while (!condition()) {
    if (DateTime.now().isAfter(deadline)) {
      throw StateError('Native operation did not finish within five seconds');
    }
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
}
