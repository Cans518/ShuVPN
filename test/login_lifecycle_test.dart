import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shuvpn/app/theme.dart';
import 'package:shuvpn/core/account/account_center.dart';
import 'package:shuvpn/core/auth/auth_exception.dart';
import 'package:shuvpn/core/auth/login_challenge.dart';
import 'package:shuvpn/features/account/login_form.dart';

import 'account_stub.dart';

void main() {
  Future<GlobalKey<ShuLoginFormState>> open(
    WidgetTester tester,
    _Account account,
    VoidCallback completed,
  ) async {
    final key = GlobalKey<ShuLoginFormState>();
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox.shrink());
      account.dispose();
    });
    await tester.pumpWidget(
      ChangeNotifierProvider<AccountCenter>.value(
        value: account,
        child: MaterialApp(
          theme: buildShuTheme(Brightness.light),
          home: Scaffold(
            body: ShuLoginForm(
              key: key,
              onChanged: () {},
              onCompleted: completed,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    return key;
  }

  Future<void> password(WidgetTester tester) async {
    await tester.enterText(find.byType(TextFormField).at(0), 'student');
    await tester.enterText(find.byType(TextFormField).at(1), 'password');
    await tester.tap(find.text('继续'));
    await tester.pump();
  }

  testWidgets('native login owns the credential deadline and failure message', (
    tester,
  ) async {
    final account = _Account()..progressLabel = '正在获取凭据（1/3）';
    await open(tester, account, () {});
    await password(tester);
    await tester.pump(const Duration(seconds: 91));
    expect(find.text('正在获取凭据（1/3）'), findsOneWidget);
    expect(account.cancellations, 0);
    account.requests.single.completeError(
      const ShuAuthException('timeout', 'native credential timeout'),
    );
    await tester.pumpAndSettle();
    expect(find.text('native credential timeout'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
    expect(account.cancellations, 0);
  });

  for (final stage in ['password', 'send', 'verify']) {
    testWidgets('disposing $stage cancels once and ignores late completion', (
      tester,
    ) async {
      final account = _Account()..challenge = stage != 'password';
      var completions = 0;
      await open(tester, account, () => completions++);
      await password(tester);
      if (stage != 'password') {
        await tester.pumpAndSettle();
        if (stage == 'send') {
          await tester.tap(find.text('发送验证码'));
        } else {
          await tester.enterText(find.byType(TextFormField), '123456');
          await tester.tap(find.text('完成验证'));
        }
        await tester.pump();
      }
      await tester.pumpWidget(const SizedBox.shrink());
      expect(account.cancellations, 1);
      switch (stage) {
        case 'password':
          account.requests.single.complete(ShuSignInOutcome.success);
        case 'send':
          account.sending.complete();
        case 'verify':
          account.verifying.complete(true);
      }
      await tester.pump();
      expect(completions, 0);
      expect(account.cancellations, 1);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets(
    'native failed login renders the snapshot without retaining cancellation ownership',
    (tester) async {
      final account = _Account()..errorMessage = 'native snapshot failure';
      await open(tester, account, () {});
      await password(tester);
      account.requests.single.complete(ShuSignInOutcome.failed);
      await tester.pumpAndSettle();
      expect(find.text('native snapshot failure'), findsOneWidget);
      await tester.pumpWidget(const SizedBox.shrink());
      expect(account.cancellations, 0);
    },
  );

  testWidgets(
    'completed login can reset and leave without cancelling later work',
    (tester) async {
      final account = _Account();
      var completions = 0;
      late GlobalKey<ShuLoginFormState> key;
      key = await open(tester, account, () {
        completions++;
        key.currentState!.reset();
      });
      await password(tester);
      account.requests.single.complete(ShuSignInOutcome.success);
      await tester.pumpAndSettle();
      expect(completions, 1);
      expect(key.currentState!.busy, isFalse);
      await tester.pumpWidget(const SizedBox.shrink());
      expect(account.cancellations, 0);
    },
  );

  testWidgets(
    'reset rejects stale results without clearing the new command owner',
    (tester) async {
      final account = _Account();
      var completions = 0;
      final key = await open(tester, account, () => completions++);
      await password(tester);
      key.currentState!.reset();
      await tester.pumpAndSettle();
      await password(tester);
      account.requests.first.completeError(
        const ShuAuthException('cancelled', 'stale failure'),
      );
      await tester.pump();
      expect(key.currentState!.busy, isTrue);
      expect(find.text('stale failure'), findsNothing);
      key.currentState!.reset();
      expect(account.cancellations, 2);
      account.requests.last.complete(ShuSignInOutcome.success);
      await tester.pumpAndSettle();
      expect(completions, 0);
    },
  );
}

class _Account extends StubAccountCenter {
  final requests = <Completer<ShuSignInOutcome>>[];
  final sending = Completer<void>();
  final verifying = Completer<bool>();
  int cancellations = 0;
  bool challenge = false;
  @override
  void cancelPending() => cancellations++;
  @override
  Future<ShuSignInOutcome> signIn({
    required String username,
    required String password,
  }) {
    if (challenge) {
      loginChallenge = const ShuLoginChallenge(
        methods: {ShuVerificationMethod.sms: 'masked'},
        selectedMethod: ShuVerificationMethod.sms,
      );
      return Future.value(ShuSignInOutcome.needsVerification(loginChallenge!));
    }
    final result = Completer<ShuSignInOutcome>();
    requests.add(result);
    return result.future;
  }

  @override
  Future<void> sendCode(ShuVerificationMethod method) => sending.future;
  @override
  Future<bool> submitCode(ShuVerificationMethod method, String code) =>
      verifying.future;
}
