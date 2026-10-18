import 'dart:convert';
import 'dart:io';
import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shuvpn/core/rust/routing_service.dart';
import 'package:shuvpn/core/rust/rust_core.dart';
import 'package:shuvpn/features/settings/routing_settings_page.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  for (final source in ['prepend', 'profile', 'append']) {
    testWidgets(
      '$source pointer reorder is a draft until apply and survives reopen',
      (tester) async {
        tester.view.physicalSize = const Size(1400, 2800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        final setup = await tester.runAsync(() async {
          final directory = await Directory.systemTemp.createTemp(
            'shuvpn-rule-drag-',
          );
          addTearDown(() => directory.delete(recursive: true));
          final service = await RoutingService.open(directory.path);
          var released = false;
          void release() {
            if (released) return;
            released = true;
            service.dispose();
          }

          addTearDown(release);
          await service.prepare(
            jsonEncode({
              'name': 'pointer-drag',
              'automatic_campus': false,
              'rules': [..._expressions('profile'), 'MATCH,REJECT'],
            }),
            jsonEncode({
              'prepend': _expressions('prepend'),
              'append': _expressions('append'),
            }),
          );
          await service.applyPreview();
          return (directory, service, release);
        });
        if (setup == null) throw StateError('Native fixture did not open');
        final (directory, service, release) = setup;
        await _mount(tester, service, source);
        final profile = _editor(tester, 'routing-profile');
        final overrides = _editor(tester, 'routing-overrides');
        final original = (profile.text, overrides.text);
        final record = File('${directory.path}/routing-state.json');
        final originalBytes = record.readAsBytesSync();
        await tester.runAsync(
          () => service.prepare(profile.text, overrides.text),
        );
        await tester.pump();
        final preview = service.preview;
        expect(preview, isNotNull);
        expect(await _decision(tester, service, source), (
          'REJECT',
          1,
          '$source:0',
        ));

        final canceled = await _crossFirstRule(tester, source);
        await canceled.cancel();
        await tester.pumpAndSettle();
        expect((profile.text, overrides.text), original);
        expect(service.preview, same(preview));
        expect(record.readAsBytesSync(), originalBytes);

        final dropped = await _crossFirstRule(tester, source);
        await dropped.up();
        await _pumpUntil(
          tester,
          () =>
              !service.busy &&
              service.preview == null &&
              (profile.text, overrides.text) != original,
        );
        expect(service.error, isNull);
        final edited = await tester.runAsync(
          () => service.inspectDraft(profile.text, overrides.text),
        );
        expect(edited, isNotNull);
        expect(
          edited!.rules
              .where((rule) => rule.source == source && !rule.finalRule)
              .map((rule) => rule.expression),
          _expressions(source).reversed,
        );
        expect(edited.rules.last.finalRule, isTrue);
        expect(edited.rules.where((rule) => !rule.enabled), isEmpty);
        expect(service.current.version, 1);
        expect(record.readAsBytesSync(), originalBytes);
        expect(await _decision(tester, service, source), (
          'REJECT',
          1,
          '$source:0',
        ));
        expect(
          tester
              .widget<OutlinedButton>(
                find.widgetWithText(OutlinedButton, '应用预览'),
              )
              .onPressed,
          isNull,
        );
        final prepare = find.text('校验并预览');
        await tester.ensureVisible(prepare);
        await tester.tap(prepare);
        await _pumpUntil(tester, () => !service.busy);
        expect(service.error, isNull);
        expect(service.preview, isNotNull);
        expect(record.readAsBytesSync(), originalBytes);
        await tester.tap(find.text('应用预览'));
        await _pumpUntil(
          tester,
          () => !service.busy && service.current.version == 2,
        );
        expect(service.error, isNull);
        expect(await _decision(tester, service, source), (
          'DIRECT',
          2,
          '$source:0',
        ));
        final committed =
            jsonDecode(utf8.decode(record.readAsBytesSync())) as Map;
        expect(committed['version'], 2);
        expect((committed['current'] as Map)['profile'], profile.text);
        expect((committed['current'] as Map)['overrides'], overrides.text);
        final expectedDocuments = (profile.text, overrides.text);

        await tester.pumpWidget(const SizedBox.shrink());
        release();
        final restored = await tester.runAsync(
          () => RoutingService.open(directory.path),
        );
        if (restored == null) throw StateError('Native fixture did not reopen');
        addTearDown(restored.dispose);
        expect(restored.current.version, 2);
        expect((
          restored.current.profile,
          restored.current.overrides,
        ), expectedDocuments);
        final restoredDraft = await tester.runAsync(
          () => restored.inspectDraft(
            restored.current.profile,
            restored.current.overrides,
          ),
        );
        expect(restoredDraft, isNotNull);
        for (final section in ['prepend', 'profile', 'append']) {
          expect(
            restoredDraft!.rules
                .where((rule) => rule.source == section && !rule.finalRule)
                .map((rule) => rule.expression),
            section == source
                ? _expressions(section).reversed
                : _expressions(section),
          );
        }
        expect(restoredDraft!.rules.last.expression, 'MATCH,REJECT');
        expect(await _decision(tester, restored, source), (
          'DIRECT',
          2,
          '$source:0',
        ));
        await _mount(tester, restored, source);
        expect((
          _editor(tester, 'routing-profile').text,
          _editor(tester, 'routing-overrides').text,
        ), expectedDocuments);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }
}

List<String> _expressions(String source) => [
  'DOMAIN,drag.$source.invalid,REJECT',
  'DOMAIN,drag.$source.invalid,DIRECT',
];

TextEditingController _editor(WidgetTester tester, String key) => tester
    .widget<TextField>(find.byKey(ValueKey(key), skipOffstage: false))
    .controller!;

Future<void> _mount(
  WidgetTester tester,
  RoutingService service,
  String source,
) async {
  await tester.pumpWidget(
    ChangeNotifierProvider<RoutingService?>.value(
      value: service,
      child: const MaterialApp(home: RoutingSettingsPage()),
    ),
  );
  await _pumpUntil(
    tester,
    () => find.byKey(ValueKey('routing-rule-$source-1')).evaluate().isNotEmpty,
  );
}

Future<TestGesture> _crossFirstRule(WidgetTester tester, String source) async {
  final handle = find.descendant(
    of: find.byKey(ValueKey('routing-rule-$source-1')),
    matching: find.byType(ReorderableDragStartListener),
  );
  await tester.ensureVisible(handle);
  await tester.pumpAndSettle();
  final start = tester.getCenter(handle);
  final end = Offset(
    start.dx,
    tester.getCenter(find.byKey(ValueKey('routing-rule-$source-0'))).dy,
  );
  final gesture = await tester.startGesture(
    start,
    kind: PointerDeviceKind.mouse,
  );
  try {
    for (var step = 1; step <= 8; step++) {
      await gesture.moveTo(Offset.lerp(start, end, step / 8)!);
      await tester.pump(const Duration(milliseconds: 80));
    }
    return gesture;
  } on Object {
    await gesture.cancel();
    rethrow;
  }
}

Future<(String, int, String)> _decision(
  WidgetTester tester,
  RoutingService service,
  String source,
) async {
  final result = await tester.runAsync(
    () => service.test(
      jsonEncode({
        'host': 'drag.$source.invalid',
        'port': 443,
        'network': 'tcp',
        'inbound': 'http',
      }),
      '[]',
    ),
  );
  if (result == null) throw StateError('Native decision did not return');
  final decision = (jsonDecode(result) as Map)['decision'] as Map;
  expect(decision['source'], source);
  return (
    decision['action'] as String,
    decision['version'] as int,
    decision['rule_id'] as String,
  );
}

Future<void> _pumpUntil(WidgetTester tester, bool Function() condition) async {
  for (var attempt = 0; attempt < 500; attempt++) {
    await tester.pump(const Duration(milliseconds: 16));
    if (condition()) return;
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
  }
  throw StateError('Native pointer edit did not finish within five seconds');
}
