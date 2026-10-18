import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shuvpn/app/theme.dart';
import 'package:shuvpn/core/account/account_center.dart';
import 'package:shuvpn/core/account/qr_account.dart';
import 'package:shuvpn/core/auth/login_challenge.dart';
import 'package:shuvpn/core/auth/auth_exception.dart';
import 'package:shuvpn/features/account/login_form.dart';
import 'package:shuvpn/features/account/wecom_scan_page.dart';

import 'account_stub.dart';

void main() {
  Future<void> pump(
    WidgetTester tester,
    StubAccountCenter account,
    Widget home,
  ) async {
    tester.view.physicalSize = const Size(1280, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox.shrink());
      account.dispose();
    });
    await tester.pumpWidget(
      ChangeNotifierProvider<AccountCenter>.value(
        value: account,
        child: MaterialApp(theme: buildShuTheme(Brightness.light), home: home),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets(
    'a missing native QR capability fails without creating an HTTP fallback',
    (tester) async {
      await pump(tester, StubAccountCenter(), const WeComScanPage());
      expect(find.textContaining('未提供原生扫码登录'), findsOneWidget);
      expect(find.byType(Image), findsNothing);
      final button = tester.widget<OutlinedButton>(find.byType(OutlinedButton));
      expect(button.onPressed, isNull);
      expect(tester.takeException(), isNull);
    },
  );

  Future<List<bool?>> openQr(
    WidgetTester tester,
    _QrAccount account, {
    bool settle = true,
  }) async {
    final results = <bool?>[];
    await pump(
      tester,
      account,
      Scaffold(
        body: Builder(
          builder: (context) => TextButton(
            onPressed: () async => results.add(
              await Navigator.of(context).push<bool>(
                MaterialPageRoute(builder: (_) => const WeComScanPage()),
              ),
            ),
            child: const Text('open QR'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open QR'));
    if (settle) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
    }
    return results;
  }

  testWidgets(
    'native QR operation completes without a second credential exchange',
    (tester) async {
      final account = _QrAccount();
      final results = await openQr(tester, account);
      expect(account.starts, 1);
      expect(find.byType(Image), findsOneWidget);
      expect(find.text('等待扫码…'), findsOneWidget);
      account.finish(QrLoginStatus.succeeded);
      await tester.pumpAndSettle();
      expect(results, [true]);
      expect(account.cancels, 0);
      expect(account.signedIn, isTrue);
    },
  );

  testWidgets('closing native QR ignores a late operation result', (
    tester,
  ) async {
    final account = _QrAccount();
    final results = await openQr(tester, account);
    await tester.tap(find.byTooltip('返回'));
    await tester.pumpAndSettle();
    expect(account.cancels, 1);
    account.finish(QrLoginStatus.succeeded);
    await tester.pumpAndSettle();
    expect(results, [false]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('expired native QR displays the native failure', (tester) async {
    final account = _QrAccount();
    await openQr(tester, account);
    account.finish(QrLoginStatus.expired);
    await tester.pumpAndSettle();
    expect(find.text('二维码已过期或已取消，请重新尝试'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'QR login form completes once without dispatching another exchange',
    (tester) async {
      final account = _QrAccount();
      var completed = 0;
      await pump(
        tester,
        account,
        Scaffold(
          body: ShuLoginForm(onChanged: () {}, onCompleted: () => completed++),
        ),
      );
      await tester.tap(find.text('使用企业微信登录'));
      await tester.pumpAndSettle();
      account.finish(QrLoginStatus.succeeded);
      await tester.pumpAndSettle();
      expect(account.starts, 1);
      expect(completed, 1);
    },
  );

  testWidgets(
    'native credential failure keeps the QR page open and reports failure',
    (tester) async {
      final account = _QrAccount();
      final results = await openQr(tester, account);
      account.update(const QrLoginUpdate(phase: QrLoginPhase.credentials));
      await tester.pump();
      expect(find.text('正在完成登录'), findsOneWidget);
      account.result.complete(false);
      await tester.pumpAndSettle();
      expect(find.text('凭据交换失败，请重新登录'), findsOneWidget);
      expect(results, isEmpty);
    },
  );

  testWidgets('disposal during QR creation ignores the late image and result', (
    tester,
  ) async {
    final account = _QrAccount()..autoImage = false;
    await openQr(tester, account, settle: false);
    await tester.pumpWidget(const SizedBox.shrink());
    expect(account.cancels, 1);
    account.showImage();
    account.finish(QrLoginStatus.succeeded);
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  for (final phase in [QrLoginPhase.redeeming, QrLoginPhase.credentials]) {
    testWidgets('disposal during $phase ignores late errors and updates', (
      tester,
    ) async {
      final account = _QrAccount();
      await openQr(tester, account);
      account.update(QrLoginUpdate(phase: phase));
      await tester.pump();
      await tester.pumpWidget(const SizedBox.shrink());
      expect(account.cancels, 1);
      account.update(const QrLoginUpdate(phase: QrLoginPhase.waiting));
      account.result.completeError(
        const ShuAuthException('cancelled', 'native cancelled'),
      );
      await tester.pump();
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets(
    'the login form sends credentials, SMS and code through the account interface',
    (tester) async {
      final account = _MfaAccount();
      var completed = 0;
      await pump(
        tester,
        account,
        Scaffold(
          body: ShuLoginForm(onChanged: () {}, onCompleted: () => completed++),
        ),
      );
      await tester.enterText(find.byType(TextFormField).at(0), '25123456');
      await tester.enterText(
        find.byType(TextFormField).at(1),
        'fixture-password',
      );
      await tester.tap(find.text('继续'));
      await tester.pumpAndSettle();
      expect(account.username, '25123456');
      expect(account.password, 'fixture-password');
      expect(find.textContaining('138****8888'), findsOneWidget);
      await tester.tap(find.text('发送验证码'));
      await tester.pumpAndSettle();
      expect(account.sent, ShuVerificationMethod.sms);
      await tester.enterText(find.byType(TextFormField), '123456');
      await tester.tap(find.text('完成验证'));
      for (var frame = 0; frame < 20 && completed == 0; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
      }
      await tester.pumpAndSettle();
      expect(account.code, '123456');
      expect(account.verified, ShuVerificationMethod.sms);
      expect(account.verifications, 1);
      expect(completed, 1);
    },
  );
  testWidgets(
    'a send limit changes the selected method without sending another request',
    (tester) async {
      final account = _MfaAccount()
        ..dualMethods = true
        ..rejectSend = true;
      await pump(
        tester,
        account,
        Scaffold(
          body: ShuLoginForm(onChanged: () {}, onCompleted: () {}),
        ),
      );
      await tester.enterText(find.byType(TextFormField).at(0), '25123456');
      await tester.enterText(
        find.byType(TextFormField).at(1),
        'fixture-password',
      );
      await tester.tap(find.text('继续'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('发送验证码'));
      await tester.pumpAndSettle();
      expect(account.sends, 1);
      expect(account.sent, ShuVerificationMethod.wecom);
      expect(find.text('native send limit'), findsOneWidget);
      expect(find.textContaining('138****8888'), findsOneWidget);
      account.rejectSend = false;
      await tester.tap(find.text('发送验证码'));
      await tester.pumpAndSettle();
      expect(account.sends, 2);
      expect(account.sent, ShuVerificationMethod.sms);
    },
  );
  testWidgets(
    'verification uses owner selection and countdown without a local deadline',
    (tester) async {
      final account = _MfaAccount()..dualMethods = true;
      await pump(
        tester,
        account,
        Scaffold(
          body: ShuLoginForm(onChanged: () {}, onCompleted: () {}),
        ),
      );
      await tester.enterText(find.byType(TextFormField).at(0), '25123456');
      await tester.enterText(
        find.byType(TextFormField).at(1),
        'fixture-password',
      );
      await tester.tap(find.text('继续'));
      await tester.pumpAndSettle();
      account.loginChallenge = ShuLoginChallenge(
        methods: account.loginChallenge!.methods,
        selectedMethod: ShuVerificationMethod.sms,
        resendAfterSeconds: 37,
      );
      await tester.pump(const Duration(seconds: 65));
      expect(find.text('37 s 后可重新发送'), findsOneWidget);
      expect(find.textContaining('138****8888'), findsOneWidget);
      final button = tester.widget<OutlinedButton>(
        find.byType(OutlinedButton).first,
      );
      expect(button.onPressed, isNull);
      expect(account.sends, 0);
      account.selectVerificationMethod(ShuVerificationMethod.sms);
      await tester.pump(const Duration(seconds: 1));
      await tester.tap(find.text('发送验证码'));
      await tester.pumpAndSettle();
      expect(account.sent, ShuVerificationMethod.sms);
      expect(account.sends, 1);
    },
  );
  testWidgets(
    'failed credential exchange does not return to a consumed challenge',
    (tester) async {
      final account = _MfaAccount()..exchangeResult = false;
      await pump(
        tester,
        account,
        Scaffold(
          body: ShuLoginForm(onChanged: () {}, onCompleted: () {}),
        ),
      );
      await tester.enterText(find.byType(TextFormField).at(0), '25123456');
      await tester.enterText(
        find.byType(TextFormField).at(1),
        'fixture-password',
      );
      await tester.tap(find.text('继续'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextFormField), '123456');
      await tester.tap(find.text('完成验证'));
      await tester.pumpAndSettle();
      expect(find.text('native credential failure'), findsOneWidget);
      expect(find.text('继续'), findsOneWidget);
      expect(find.text('完成验证'), findsNothing);
      expect(account.loginChallenge, isNull);
      expect(account.verifications, 1);
    },
  );
}

class _QrAccount extends StubAccountCenter implements QrAccount {
  final result = Completer<bool>();
  void Function(QrLoginStatus)? changed;
  late void Function(QrLoginUpdate) update;
  int starts = 0;
  int cancels = 0;
  bool autoImage = true;
  @override
  Future<bool> signInQr({
    required void Function(QrLoginUpdate) onUpdate,
    required void Function(QrLoginStatus) onStatus,
  }) {
    starts++;
    update = onUpdate;
    changed = onStatus;
    if (autoImage) showImage();
    return result.future;
  }

  void showImage() {
    update(
      QrLoginUpdate(
        phase: QrLoginPhase.waiting,
        image: QrLoginImage(
          bytes: File('assets/images/icon_light.png').readAsBytesSync(),
          confirmUrl: 'https://example.com/confirm',
          schemeUrl: 'wxwork://fixture',
        ),
      ),
    );
  }

  void finish(QrLoginStatus status) {
    changed?.call(status);
    if (status == QrLoginStatus.expired) {
      result.completeError(
        const ShuAuthException('qrExpired', '二维码已过期或已取消，请重新尝试'),
      );
    } else {
      signedIn = true;
      result.complete(true);
    }
  }

  @override
  void cancelQr() => cancels++;
}

class _MfaAccount extends StubAccountCenter {
  String? username;
  String? password;
  String? code;
  ShuVerificationMethod? sent;
  ShuVerificationMethod? verified;
  int verifications = 0;
  int sends = 0;
  bool dualMethods = false;
  bool rejectSend = false;
  bool exchangeResult = true;
  @override
  Future<ShuSignInOutcome> signIn({
    required String username,
    required String password,
  }) async {
    this.username = username;
    this.password = password;
    loginChallenge = ShuLoginChallenge(
      methods: {
        ShuVerificationMethod.sms: '138****8888',
        if (dualMethods) ShuVerificationMethod.wecom: 'masked wecom',
      },
      selectedMethod: dualMethods
          ? ShuVerificationMethod.wecom
          : ShuVerificationMethod.sms,
    );
    return ShuSignInOutcome.needsVerification(loginChallenge!);
  }

  @override
  void selectVerificationMethod(ShuVerificationMethod method) {
    loginChallenge = ShuLoginChallenge(
      methods: loginChallenge!.methods,
      selectedMethod: method,
    );
  }

  @override
  Future<void> sendCode(ShuVerificationMethod method) async {
    sent = method;
    sends++;
    if (rejectSend) {
      // Fixture response, not a reproduction of the native fallback policy.
      selectVerificationMethod(ShuVerificationMethod.sms);
      throw const ShuAuthException('senderror', 'native send limit');
    }
  }

  @override
  Future<bool> submitCode(ShuVerificationMethod method, String code) async {
    verifications++;
    verified = method;
    this.code = code;
    signedIn = true;
    loginChallenge = null;
    if (!exchangeResult) errorMessage = 'native credential failure';
    return exchangeResult;
  }
}
