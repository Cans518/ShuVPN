import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/app/app.dart';
import 'package:shuvpn/app/bootstrap.dart';
import 'package:shuvpn/app/shu_launch_surface.dart';

void main() {
  setUp(() => SharedPreferences.setMockInitialValues(<String, Object>{}));

  testWidgets('core startup failure prevents constructing the app', (
    tester,
  ) async {
    await tester.pumpWidget(
      ShuVpnBootstrap(
        minimumSplash: Duration.zero,
        initializeCore: () async => throw StateError('native core unavailable'),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('无法启动应用'), findsOneWidget);
    expect(find.textContaining('native core unavailable'), findsOneWidget);
    expect(find.byType(ShuVpnApp), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('launch surface remains until core initialization finishes', (
    tester,
  ) async {
    final startup = Completer<void>();
    var started = false;
    await tester.pumpWidget(
      ShuVpnBootstrap(
        minimumSplash: Duration.zero,
        initializeCore: () {
          started = true;
          return startup.future;
        },
      ),
    );
    await tester.pumpAndSettle();

    expect(started, isTrue);
    expect(find.byType(ShuLaunchSurface), findsOneWidget);
    expect(find.byType(ShuVpnApp), findsNothing);

    startup.completeError(StateError('core handshake failed'));
    await tester.pumpAndSettle();
    expect(find.text('无法启动应用'), findsOneWidget);
    expect(find.byType(ShuLaunchSurface), findsNothing);
  });

  testWidgets(
    'late core failure after disposal does not update a dead widget',
    (tester) async {
      final startup = Completer<void>();
      await tester.pumpWidget(
        ShuVpnBootstrap(
          minimumSplash: Duration.zero,
          initializeCore: () => startup.future,
        ),
      );
      await tester.pumpAndSettle();
      await tester.pumpWidget(const SizedBox.shrink());
      startup.completeError(StateError('late core failure'));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
    },
  );
}
