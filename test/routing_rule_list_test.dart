import 'dart:ui' show PointerDeviceKind, SemanticsAction, Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/app/theme.dart';
import 'package:shuvpn/core/rust/routing_draft.dart';
import 'package:shuvpn/features/settings/routing_rule_list.dart';
import 'package:shuvpn/widgets/labelled_icon_button.dart';

List<RoutingDraftRule> _rules(int count) => [
  for (var index = 0; index < count; index++)
    RoutingDraftRule.decode({
      'source': 'profile',
      'index': index,
      'expression': 'DOMAIN,host$index.example,DIRECT',
      'enabled': true,
      'kind': 'DOMAIN',
      'value': 'host$index.example',
      'policy': 'DIRECT',
      'no_resolve': false,
      'final_rule': false,
    }),
];

void main() {
  for (final (from, to, page) in [(1, 0, 0), (0, 2, 0), (26, 25, 1)]) {
    testWidgets('pointer drag moves absolute rule $from to $to', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1200, 1400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final operations = <Map<String, Object?>>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SingleChildScrollView(
              child: RoutingRuleList(
                title: '配置档规则',
                source: 'profile',
                rules: _rules(page == 0 ? 3 : 28),
                busy: false,
                onEdit: (_) {},
                onOperation: operations.add,
                onAdd: () {},
              ),
            ),
          ),
        ),
      );
      if (page > 0) {
        final next = find.byTooltip('配置档规则下一页');
        await tester.ensureVisible(next);
        await tester.tap(next);
        await tester.pumpAndSettle();
      }
      final handle = find.descendant(
        of: find.byKey(ValueKey('routing-rule-profile-$from')),
        matching: find.byType(ReorderableDragStartListener),
      );
      await tester.ensureVisible(handle);
      await tester.pumpAndSettle();
      final start = tester.getCenter(handle);
      final end = Offset(
        start.dx,
        tester.getCenter(find.byKey(ValueKey('routing-rule-profile-$to'))).dy,
      );
      final gesture = await tester.startGesture(
        start,
        kind: PointerDeviceKind.mouse,
      );
      var released = false;
      try {
        for (var step = 1; step <= 8; step++) {
          await gesture.moveTo(Offset.lerp(start, end, step / 8)!);
          await tester.pump(const Duration(milliseconds: 80));
        }
        await gesture.up();
        released = true;
        await tester.pumpAndSettle();
        expect(operations, [
          {'type': 'move', 'source': 'profile', 'index': from, 'to': to},
        ]);
        expect(tester.takeException(), isNull);
      } finally {
        if (!released) await gesture.cancel();
      }
    });
  }

  testWidgets('canceling a pointer drag emits no rule edit', (tester) async {
    final operations = <Map<String, Object?>>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: RoutingRuleList(
            title: '配置档规则',
            source: 'profile',
            rules: _rules(2),
            busy: false,
            onEdit: (_) {},
            onOperation: operations.add,
            onAdd: () {},
          ),
        ),
      ),
    );
    final handle = find.byType(ReorderableDragStartListener).last;
    final gesture = await tester.startGesture(
      tester.getCenter(handle),
      kind: PointerDeviceKind.mouse,
    );
    await gesture.moveTo(
      tester.getCenter(find.byKey(const ValueKey('routing-rule-profile-0'))),
    );
    await tester.pump(const Duration(milliseconds: 400));
    await gesture.cancel();
    await tester.pumpAndSettle();
    expect(operations, isEmpty);
    expect(find.text('host0.example'), findsOneWidget);
    expect(find.text('host1.example'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('drag handle exposes its own bounded named image semantics', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: RoutingRuleList(
              title: '配置档规则',
              source: 'profile',
              rules: _rules(2),
              busy: false,
              onEdit: (_) {},
              onOperation: (_) {},
              onAdd: () {},
            ),
          ),
        ),
      );
      const label = '配置档规则 第2条：拖动排序 DOMAIN,host1.example,DIRECT；也可使用上移和下移按钮';
      final handle = find.bySemanticsLabel(label);
      expect(handle, findsOneWidget);
      final data = tester.getSemantics(handle).getSemanticsData();
      expect(data.label, label);
      expect(data.flagsCollection.isImage, isTrue);
      expect(data.hasAction(SemanticsAction.tap), isFalse);
      expect(data.rect.size, const Size(48, 48));
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('busy cards ignore a pointer drag at the former handle', (
    tester,
  ) async {
    final operations = <Map<String, Object?>>[];
    Widget view(bool busy) => MaterialApp(
      home: Scaffold(
        body: RoutingRuleList(
          title: '配置档规则',
          source: 'profile',
          rules: _rules(2),
          busy: busy,
          onEdit: (_) {},
          onOperation: operations.add,
          onAdd: () {},
        ),
      ),
    );
    await tester.pumpWidget(view(false));
    final start = tester.getCenter(
      find.byType(ReorderableDragStartListener).last,
    );
    final end = tester.getCenter(
      find.byKey(const ValueKey('routing-rule-profile-0')),
    );
    await tester.pumpWidget(view(true));
    expect(find.byType(ReorderableDragStartListener), findsNothing);
    final gesture = await tester.startGesture(
      start,
      kind: PointerDeviceKind.mouse,
    );
    await gesture.moveTo(end);
    await tester.pump(const Duration(milliseconds: 400));
    await gesture.up();
    await tester.pumpAndSettle();
    expect(operations, isEmpty);
    expect(tester.takeException(), isNull);
  });

  for (final busy in [false, true]) {
    testWidgets(
      'rule action names retain enabled and focus semantics when busy=$busy',
      (tester) async {
        final semantics = tester.ensureSemantics();
        try {
          tester.view.physicalSize = const Size(1200, 1600);
          tester.view.devicePixelRatio = 1;
          addTearDown(tester.view.reset);
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(
                body: RoutingRuleList(
                  title: '配置档规则',
                  source: 'profile',
                  rules: _rules(3),
                  busy: busy,
                  onEdit: (_) {},
                  onOperation: (_) {},
                  onAdd: () {},
                ),
              ),
            ),
          );
          for (final action in {
            '编辑': Icons.edit_outlined,
            '上移': Icons.arrow_upward,
            '下移': Icons.arrow_downward,
            '删除': Icons.delete_outline,
          }.entries) {
            final tooltip = '${action.key} DOMAIN,host1.example,DIRECT';
            final button = find.descendant(
              of: find.byKey(const ValueKey('routing-rule-profile-1')),
              matching: find.widgetWithIcon(IconButton, action.value),
            );
            final data = tester.getSemantics(button).getSemanticsData();
            expect(data.label, '配置档规则 第2条：$tooltip');
            expect(data.tooltip, isEmpty);
            expect(data.flagsCollection.isButton, isTrue);
            expect(
              data.flagsCollection.isEnabled,
              busy ? Tristate.isFalse : Tristate.isTrue,
            );
            expect(data.hasAction(SemanticsAction.tap), !busy);
            expect(data.hasAction(SemanticsAction.focus), !busy);
            expect(data.flagsCollection.isFocused != Tristate.none, !busy);
          }
          final toggle = find.byKey(const ValueKey('routing-toggle-profile-1'));
          final data = tester.getSemantics(toggle).getSemanticsData();
          expect(data.label, '配置档规则 第2条：启用或停用 DOMAIN,host1.example,DIRECT');
          expect(data.tooltip, isEmpty);
          expect(data.flagsCollection.isToggled, Tristate.isTrue);
          expect(data.hasAction(SemanticsAction.tap), !busy);
          final add = tester
              .getSemantics(find.byKey(const ValueKey('routing-add-profile')))
              .getSemanticsData();
          expect(add.label, '新增配置档规则');
          expect(add.flagsCollection.isButton, isTrue);
          expect(
            add.flagsCollection.isEnabled,
            busy ? Tristate.isFalse : Tristate.isTrue,
          );
          expect(add.hasAction(SemanticsAction.tap), !busy);
          expect(tester.takeException(), isNull);
        } finally {
          semantics.dispose();
        }
      },
    );
  }

  testWidgets(
    'a named icon invokes once by semantic action and once by keyboard',
    (tester) async {
      final semantics = tester.ensureSemantics();
      var calls = 0;
      try {
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: Center(
                child: LabelledIconButton(
                  tooltip: '编辑规则',
                  semanticLabel: '前置规则 第1条：编辑',
                  icon: Icons.edit_outlined,
                  onPressed: () => calls++,
                ),
              ),
            ),
          ),
        );
        tester.semantics.tap(find.semantics.byLabel('前置规则 第1条：编辑'));
        await tester.pumpAndSettle();
        expect(calls, 1);
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pumpAndSettle();
        expect(
          tester
              .getSemantics(find.byType(IconButton))
              .getSemanticsData()
              .flagsCollection
              .isFocused,
          Tristate.isTrue,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.space);
        await tester.pumpAndSettle();
        expect(calls, 2);
      } finally {
        semantics.dispose();
      }
    },
  );

  testWidgets('rule card displays native duplicate-condition provenance', (
    tester,
  ) async {
    final rule = RoutingDraftRule.decode({
      'source': 'profile',
      'index': 0,
      'expression': 'DOMAIN,example,DIRECT',
      'enabled': true,
      'kind': 'DOMAIN',
      'value': 'example',
      'policy': 'DIRECT',
      'no_resolve': false,
      'final_rule': false,
      'shadowed_by': 'prepend:0',
    });
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: RoutingRuleList(
            title: '配置档规则',
            source: 'profile',
            rules: [rule],
            busy: false,
            onEdit: (_) {},
            onOperation: (_) {},
            onAdd: () {},
          ),
        ),
      ),
    );
    expect(find.textContaining('prepend:0'), findsOneWidget);
    expect(find.textContaining('条件文本重复'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'large rule lists page their cards and allow movement across page boundaries',
    (tester) async {
      tester.view.physicalSize = const Size(1000, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final operations = <Map<String, Object?>>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SingleChildScrollView(
              child: RoutingRuleList(
                title: '配置档规则',
                source: 'profile',
                rules: _rules(80),
                busy: false,
                onEdit: (_) {},
                onOperation: operations.add,
                onAdd: () {},
              ),
            ),
          ),
        ),
      );
      expect(find.byType(Switch), findsNWidgets(25));
      final next = find.byTooltip('配置档规则下一页');
      await tester.ensureVisible(next);
      await tester.tap(next);
      await tester.pumpAndSettle();
      final move = find.byTooltip('上移 DOMAIN,host25.example,DIRECT');
      await tester.ensureVisible(move);
      await tester.tap(move);
      expect(operations.single, {
        'type': 'move',
        'source': 'profile',
        'index': 25,
        'to': 24,
      });
      expect(find.byType(Switch), findsNWidgets(25));
      expect(find.text('host0.example'), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );

  for (final brightness in Brightness.values) {
    testWidgets(
      'rule controls wrap and expose disabled states with large text in $brightness',
      (tester) async {
        tester.view.physicalSize = const Size(375, 812);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        await tester.pumpWidget(
          MaterialApp(
            theme: buildShuTheme(brightness),
            builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: const TextScaler.linear(1.8)),
              child: child!,
            ),
            home: Scaffold(
              body: SingleChildScrollView(
                child: RoutingRuleList(
                  title: '配置档规则',
                  source: 'profile',
                  rules: _rules(1),
                  busy: true,
                  onEdit: (_) => throw StateError('busy edit'),
                  onOperation: (_) => throw StateError('busy operation'),
                  onAdd: () => throw StateError('busy add'),
                ),
              ),
            ),
          ),
        );
        expect(tester.widget<Switch>(find.byType(Switch)).onChanged, isNull);
        expect(find.byType(ReorderableDragStartListener), findsNothing);
        expect(tester.takeException(), isNull);
      },
    );
  }
}
