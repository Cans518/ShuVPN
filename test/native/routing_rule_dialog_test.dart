import 'dart:convert';
import 'dart:io';
import 'dart:ui' show SemanticsAction, Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shuvpn/core/rust/routing_service.dart';
import 'package:shuvpn/features/settings/routing_settings_page.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  for (final busy in [false, true]) {
    testWidgets(
      'rule dropdowns expose their actual enabled state when busy=$busy',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          final (_, service, _) = await _openEditor(tester);
          final profile = jsonDecode(service.current.profile) as Map;
          profile['proxy-groups'] = [
            {
              'name': 'Office',
              'type': 'select',
              'proxies': ['DIRECT', 'REJECT'],
            },
          ];
          await tester.runAsync(() async {
            await service.prepare(
              jsonEncode(profile),
              service.current.overrides,
            );
            await service.applyPreview();
          });
          await tester.pumpWidget(const SizedBox.shrink());
          service.busy = busy;
          await _mount(tester, service);
          await tester.pump(const Duration(milliseconds: 350));
          for (final (key, label, choice) in [
            ('routing-group-Office', '策略组 · Office', 'REJECT'),
            ('routing-test-network', '网络协议', 'UDP'),
            ('routing-test-inbound', '入站类型', 'SOCKS'),
          ]) {
            final control = find.byKey(ValueKey(key));
            await tester.ensureVisible(control);
            await tester.pump(const Duration(milliseconds: 350));
            final node = _exportedNode(tester.getSemantics(control));
            final data = node.getSemanticsData();
            expect(data.label, contains(label));
            expect(
              data.flagsCollection.isButton,
              isTrue,
              reason: node.toStringDeep(),
            );
            expect(
              data.flagsCollection.isEnabled,
              busy ? Tristate.isFalse : Tristate.isTrue,
              reason: label,
            );
            expect(data.hasAction(SemanticsAction.tap), !busy, reason: label);
            expect(_tapNodes(node), busy ? 0 : 1, reason: label);
            if (!busy) {
              node.owner!.performAction(node.id, SemanticsAction.tap);
              await tester.pumpAndSettle();
              final item = _exportedNode(
                tester.getSemantics(find.text(choice).last),
              );
              final itemData = item.getSemanticsData();
              expect(
                itemData.flagsCollection.isEnabled,
                Tristate.isTrue,
                reason: '$label / $choice',
              );
              expect(itemData.hasAction(SemanticsAction.tap), isTrue);
              expect(_tapNodes(item), 1);
              await tester.tapAt(const Offset(10, 10));
              await tester.pumpAndSettle();
            }
          }
          expect(tester.takeException(), isNull);
        } finally {
          semantics.dispose();
        }
      },
    );
  }

  for (final source in ['prepend', 'profile', 'append']) {
    testWidgets(
      '$source rule dialogs cancel safely and persist only after preview and apply',
      (tester) async {
        final (directory, service, release) = await _openEditor(tester);
        final record = File('${directory.path}/routing-state.json');
        final originalBytes = record.readAsBytesSync();
        final profile = _editor(tester, 'routing-profile');
        final overrides = _editor(tester, 'routing-overrides');
        final originalDraft = (profile.text, overrides.text);
        final added = 'DOMAIN,added.$source.invalid,REJECT';
        final edited = 'DOMAIN,edited.$source.invalid,REJECT';

        await tester.runAsync(
          () => service.prepare(profile.text, overrides.text),
        );
        await tester.pump();
        final previewBeforeCancel = service.preview;
        expect(previewBeforeCancel, isNotNull);
        final add = find.byKey(ValueKey('routing-add-$source'));
        await _openDialog(tester, add);
        await tester.enterText(_promptField(), added);
        await _dialogAction(tester, '取消');
        await tester.pumpAndSettle();
        expect((profile.text, overrides.text), originalDraft);
        expect(service.preview, same(previewBeforeCancel));
        expect(record.readAsBytesSync(), originalBytes);

        await _openDialog(tester, add);
        await tester.enterText(_promptField(), added);
        await _dialogAction(tester, '保存');
        await _pumpUntil(
          tester,
          () => profile.text.contains(added) || overrides.text.contains(added),
        );
        await tester.pumpAndSettle();
        expect(service.preview, isNull);
        expect(service.current.version, 1);
        expect(record.readAsBytesSync(), originalBytes);
        expect(
          await _action(tester, service, 'added.$source.invalid'),
          'DIRECT',
        );
        expect(_applyButton(tester).onPressed, isNull);
        await _expectRuleSource(
          tester,
          service,
          profile,
          overrides,
          added,
          source,
        );
        await tester.runAsync(
          () => service.prepare(profile.text, overrides.text),
        );
        await tester.pump();
        final beforeEditCancel = (profile.text, overrides.text);
        final previewBeforeEdit = service.preview;
        expect(previewBeforeEdit, isNotNull);

        await _openDialog(tester, find.byTooltip('编辑 $added'));
        expect(
          tester.widget<TextField>(_promptField()).controller!.text,
          added,
        );
        await tester.enterText(_promptField(), edited);
        await _dialogAction(tester, '取消');
        await tester.pumpAndSettle();
        expect(find.byTooltip('编辑 $added'), findsOneWidget);
        expect(find.byTooltip('编辑 $edited'), findsNothing);
        expect((profile.text, overrides.text), beforeEditCancel);
        expect(service.preview, same(previewBeforeEdit));
        expect(record.readAsBytesSync(), originalBytes);

        final beforeInvalid = (profile.text, overrides.text);
        await _openDialog(tester, find.byTooltip('编辑 $added'));
        await tester.enterText(_promptField(), 'DOMAIN');
        await _dialogAction(tester, '保存');
        await _pumpUntil(
          tester,
          () => find.textContaining('未修改草稿：').evaluate().isNotEmpty,
        );
        await tester.pumpAndSettle();
        expect(find.textContaining('规则字段数量无效'), findsOneWidget);
        expect((profile.text, overrides.text), beforeInvalid);
        expect(record.readAsBytesSync(), originalBytes);
        expect(_applyButton(tester).onPressed, isNull);

        await _openDialog(tester, find.byTooltip('编辑 $added'));
        const unsupported = 'UNSUPPORTED,example,DIRECT';
        await tester.enterText(_promptField(), unsupported);
        await _dialogAction(tester, '保存');
        await _pumpUntil(
          tester,
          () =>
              profile.text.contains(unsupported) ||
              overrides.text.contains(unsupported),
        );
        await tester.pumpAndSettle();
        expect(service.error, isNull);
        final prepare = find.text('校验并预览');
        await tester.ensureVisible(prepare);
        await tester.tap(prepare);
        await _pumpUntil(tester, () => !service.busy);
        expect(service.error, '规则类型尚未支持');
        expect(service.preview, isNull);
        expect(_applyButton(tester).onPressed, isNull);
        expect(record.readAsBytesSync(), originalBytes);
        expect(
          await _action(tester, service, 'edited.$source.invalid'),
          'DIRECT',
        );

        await _openDialog(tester, find.byTooltip('编辑 $unsupported'));
        await tester.enterText(_promptField(), edited);
        await _dialogAction(tester, '保存');
        await _pumpUntil(
          tester,
          () =>
              profile.text.contains(edited) || overrides.text.contains(edited),
        );
        await tester.pumpAndSettle();
        expect(find.byTooltip('编辑 $added'), findsNothing);
        expect(record.readAsBytesSync(), originalBytes);
        await _expectRuleSource(
          tester,
          service,
          profile,
          overrides,
          edited,
          source,
        );
        await _applyDraft(tester, service, 2);
        expect(
          await _action(tester, service, 'edited.$source.invalid'),
          'REJECT',
        );
        expect(
          await _action(tester, service, 'added.$source.invalid'),
          'DIRECT',
        );
        final committedBytes = record.readAsBytesSync();
        final committed = jsonDecode(utf8.decode(committedBytes)) as Map;
        expect((committed['current'] as Map)['profile'], profile.text);
        expect((committed['current'] as Map)['overrides'], overrides.text);
        final committedDraft = (profile.text, overrides.text);

        await tester.runAsync(
          () => service.prepare(profile.text, overrides.text),
        );
        await tester.pump();
        final previewBeforeDelete = service.preview;
        expect(previewBeforeDelete, isNotNull);
        await _openDialog(tester, find.byTooltip('删除 $edited'));
        expect(find.text('只修改编辑草稿。校验并应用后才会影响新连接。'), findsOneWidget);
        await _dialogAction(tester, '取消');
        await tester.pumpAndSettle();
        expect((profile.text, overrides.text), committedDraft);
        expect(service.preview, same(previewBeforeDelete));
        expect(record.readAsBytesSync(), committedBytes);

        await _openDialog(tester, find.byTooltip('删除 $edited'));
        await _dialogAction(tester, '删除');
        await _pumpUntil(
          tester,
          () =>
              !profile.text.contains(edited) &&
              !overrides.text.contains(edited),
        );
        await tester.pumpAndSettle();
        expect(service.preview, isNull);
        expect(record.readAsBytesSync(), committedBytes);
        expect(
          await _action(tester, service, 'edited.$source.invalid'),
          'REJECT',
        );
        await _applyDraft(tester, service, 3);
        expect(
          await _action(tester, service, 'edited.$source.invalid'),
          'DIRECT',
        );

        await tester.pumpWidget(const SizedBox.shrink());
        release();
        final restored = await tester.runAsync(
          () => RoutingService.open(directory.path),
        );
        if (restored == null) {
          throw StateError('Native reopen did not complete');
        }
        addTearDown(restored.dispose);
        expect(restored.current.version, 3);
        final draft = await tester.runAsync(
          () => restored.inspectDraft(
            restored.current.profile,
            restored.current.overrides,
          ),
        );
        expect(draft, isNotNull);
        expect(draft!.rules.where((rule) => !rule.finalRule).length, 3);
        expect(draft.rules.last.finalRule, isTrue);
        expect(draft.rules.last.expression, 'MATCH,DIRECT');
        for (final section in ['prepend', 'profile', 'append']) {
          final kept = draft.rules.singleWhere(
            (rule) => rule.source == section && !rule.finalRule,
          );
          expect(kept.expression, 'DOMAIN,keep.$section.invalid,DIRECT');
          expect(kept.enabled, isTrue);
        }
        expect(
          await _action(tester, restored, 'edited.$source.invalid'),
          'DIRECT',
        );
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets(
    'profile rename cancels safely and survives apply, reopen and GUI rollback',
    (tester) async {
      final (directory, service, release) = await _openEditor(tester);
      final record = File('${directory.path}/routing-state.json');
      final before = record.readAsBytesSync();
      final originalProfile = service.current.profile;
      final originalOverrides = service.current.overrides;
      final profile = _editor(tester, 'routing-profile');
      final rename = find.byTooltip('重命名配置档');
      await _openDialog(tester, rename);
      await tester.enterText(_promptField(), '取消的名称');
      await _dialogAction(tester, '取消');
      await tester.pumpAndSettle();
      expect(record.readAsBytesSync(), before);
      expect(service.current.name, 'dialog-original');
      expect(profile.text, service.current.profile);

      await _openDialog(tester, rename);
      await tester.enterText(_promptField(), '重命名后的配置');
      await _dialogAction(tester, '保存');
      await _pumpUntil(tester, () => profile.text.contains('重命名后的配置'));
      await tester.pumpAndSettle();
      expect(service.current.name, 'dialog-original');
      expect(record.readAsBytesSync(), before);
      await _applyDraft(tester, service, 2);
      final renamedProfile = service.current.profile;
      final renamedOverrides = service.current.overrides;
      expect(service.current.name, '重命名后的配置');
      await tester.pumpWidget(const SizedBox.shrink());
      release();
      final restored = await tester.runAsync(
        () => RoutingService.open(directory.path),
      );
      if (restored == null) throw StateError('Native reopen did not complete');
      addTearDown(restored.dispose);
      expect(restored.current.name, '重命名后的配置');
      expect(restored.current.profile, renamedProfile);
      expect(restored.current.overrides, renamedOverrides);
      await _mount(tester, restored);
      final rollback = find.text('回退上一版本');
      await tester.ensureVisible(rollback);
      await tester.tap(rollback);
      await _pumpUntil(
        tester,
        () =>
            restored.current.version == 3 &&
            _editor(tester, 'routing-profile').text == originalProfile,
      );
      expect(restored.error, isNull);
      expect(restored.current.name, 'dialog-original');
      expect(restored.current.overrides, originalOverrides);
      expect(_editor(tester, 'routing-overrides').text, originalOverrides);
      final persisted = jsonDecode(record.readAsStringSync()) as Map;
      expect(persisted['version'], 3);
      expect((persisted['current'] as Map)['profile'], originalProfile);
      expect((persisted['current'] as Map)['overrides'], originalOverrides);
      expect(restored.preview, isNull);
      expect(_applyButton(tester).onPressed, isNull);
      expect(tester.takeException(), isNull);
    },
  );
}

SemanticsNode _exportedNode(SemanticsNode node) {
  while (node.isMergedIntoParent) {
    node = node.parent!;
  }
  return node;
}

int _tapNodes(SemanticsNode node) {
  var count = node.getSemanticsData().hasAction(SemanticsAction.tap) ? 1 : 0;
  if (node.mergeAllDescendantsIntoThisNode) return count;
  node.visitChildren((child) {
    count += _tapNodes(child);
    return true;
  });
  return count;
}

Future<(Directory, RoutingService, VoidCallback)> _openEditor(
  WidgetTester tester,
) async {
  final setup = await tester.runAsync(() async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-rule-dialog-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final service = await RoutingService.open(directory.path);
    var disposed = false;
    void release() {
      if (disposed) return;
      disposed = true;
      service.dispose();
    }

    addTearDown(release);
    await service.prepare(
      jsonEncode({
        'name': 'dialog-original',
        'automatic_campus': false,
        'rules': ['DOMAIN,keep.profile.invalid,DIRECT', 'MATCH,DIRECT'],
      }),
      jsonEncode({
        'prepend': ['DOMAIN,keep.prepend.invalid,DIRECT'],
        'append': ['DOMAIN,keep.append.invalid,DIRECT'],
      }),
    );
    await service.applyPreview();
    return (directory, service, release);
  });
  if (setup == null) throw StateError('Native setup did not complete');
  tester.view.physicalSize = const Size(1200, 2600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await _mount(tester, setup.$2);
  return setup;
}

Future<void> _expectRuleSource(
  WidgetTester tester,
  RoutingService service,
  TextEditingController profile,
  TextEditingController overrides,
  String expression,
  String source,
) async {
  final draft = await tester.runAsync(
    () => service.inspectDraft(profile.text, overrides.text),
  );
  expect(draft, isNotNull);
  expect(
    draft!.rules.singleWhere((rule) => rule.expression == expression).source,
    source,
  );
}

Future<void> _mount(WidgetTester tester, RoutingService service) async {
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
}

TextEditingController _editor(WidgetTester tester, String key) => tester
    .widget<TextField>(find.byKey(ValueKey(key), skipOffstage: false))
    .controller!;

Finder _promptField() => find.descendant(
  of: find.byType(AlertDialog),
  matching: find.byType(TextField),
);

OutlinedButton _applyButton(WidgetTester tester) =>
    tester.widget<OutlinedButton>(find.widgetWithText(OutlinedButton, '应用预览'));

Future<void> _openDialog(WidgetTester tester, Finder button) async {
  await tester.ensureVisible(button);
  await tester.tap(button);
  await tester.pumpAndSettle();
  expect(find.byType(AlertDialog), findsOneWidget);
}

Future<void> _dialogAction(WidgetTester tester, String label) async {
  await tester.tap(
    find.descendant(of: find.byType(AlertDialog), matching: find.text(label)),
  );
  await tester.pump();
}

Future<void> _applyDraft(
  WidgetTester tester,
  RoutingService service,
  int version,
) async {
  final prepare = find.text('校验并预览');
  await tester.ensureVisible(prepare);
  await tester.tap(prepare);
  await _pumpUntil(tester, () => !service.busy);
  expect(service.error, isNull);
  expect(service.preview, isNotNull);
  await tester.tap(find.text('应用预览'));
  await _pumpUntil(tester, () => !service.busy);
  expect(service.error, isNull);
  expect(service.current.version, version);
}

Future<String> _action(
  WidgetTester tester,
  RoutingService service,
  String host,
) async {
  final result = await tester.runAsync(
    () => service.test(
      jsonEncode({
        'host': host,
        'port': 443,
        'network': 'tcp',
        'inbound': 'http',
      }),
      '[]',
    ),
  );
  expect(result, isNotNull);
  return ((jsonDecode(result!) as Map)['decision'] as Map)['action'] as String;
}

Future<void> _pumpUntil(WidgetTester tester, bool Function() condition) async {
  for (var attempt = 0; attempt < 500; attempt++) {
    await tester.pump();
    if (condition()) return;
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
  }
  throw StateError('Rule editor did not finish within five seconds');
}
