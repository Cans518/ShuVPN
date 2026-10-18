import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/account/qr_account.dart';
import 'package:shuvpn/core/account/account_center.dart';
import 'package:shuvpn/core/auth/login_challenge.dart';
import 'package:shuvpn/core/auth/auth_exception.dart';
import 'package:shuvpn/core/rust/generated/api/account.dart' as native;
import 'package:shuvpn/core/rust/generated/auth/model.dart' as errors;
import 'package:shuvpn/core/rust/generated/auth/newsso/account/model.dart'
    as model;
import 'package:shuvpn/core/rust/generated/auth/newsso/model.dart' as sso;
import 'package:shuvpn/core/rust/generated/auth/newsso/account/verification.dart'
    as delivery;
import 'package:shuvpn/core/rust/rust_account_center.dart';

void main() {
  test('password and verification adapters dispatch only the combined native commands', () async {
    final owner = _Account();
    final account = RustAccountCenter.fromNative(owner);
    expect(
      (await account.signIn(username: 'student', password: 'secret')).isSuccess,
      isTrue,
    );
    expect(
      await account.submitCode(ShuVerificationMethod.sms, '123456'),
      isTrue,
    );
    expect(owner.passwordInput, ['student', 'secret']);
    expect(owner.codeInput, ['sms', '123456']);
    owner.passwordState = model.SignInState.verificationRequired;
    final outcome = await account.signIn(
      username: 'student',
      password: 'secret',
    );
    expect(outcome.needsVerification, isTrue);
    expect(outcome.challenge!.selectedMethod, ShuVerificationMethod.sms);
    expect(outcome.challenge!.resendAfterSeconds, 17);
    account.dispose();
    owner.events.stream.listen((_) {});
    await owner.events.close();
  });

  test('combined credential failure is reported without pretending authentication was completed', () async {
    final owner = _Account()
      ..failure = const errors.AuthFailure(
        code: 'credentialsUnavailable',
        message: 'native credential failure',
      );
    final account = RustAccountCenter.fromNative(owner);
    expect(
      (await account.signIn(username: 'student', password: 'secret')).status,
      ShuSignInStatus.failed,
    );
    expect(
      await account.submitCode(ShuVerificationMethod.sms, '123456'),
      isFalse,
    );
    expect(account.errorMessage, 'native credential failure');
    account.dispose();
    owner.events.stream.listen((_) {});
    await owner.events.close();
  });

  test(
    'disposing a password adapter rejects its late completed result',
    () async {
      final owner = _Account()..pendingPassword = Completer();
      final account = RustAccountCenter.fromNative(owner);
      final pending = account.signIn(username: 'student', password: 'secret');
      account.dispose();
      owner.pendingPassword!.complete(model.SignInState.completed);
      expect((await pending).status, ShuSignInStatus.failed);
      expect(owner.closes, 1);
      owner.events.stream.listen((_) {});
      await owner.events.close();
    },
  );

  testWidgets(
    'QR adapter maps native stages and snapshot status without dispatching stage commands',
    (tester) async {
      final owner = _Account();
      final account = RustAccountCenter.fromNative(owner);
      final updates = <QrLoginUpdate>[];
      final statuses = <QrLoginStatus>[];
      final result = account.signInQr(
        onUpdate: updates.add,
        onStatus: statuses.add,
      );
      owner.events.add(
        model.QrSignInEvent(
          phase: model.QrSignInPhase.waiting,
          image: sso.QrSession(
            imageBytes: Uint8List.fromList([1, 2, 3]),
            confirmUrl: 'https://fixture/confirm',
            wxWorkSchemeUrl: 'wxwork://fixture',
          ),
        ),
      );
      owner.status = sso.QrStatus.confirmedPending;
      await tester.pump(const Duration(milliseconds: 250));
      expect(statuses, [QrLoginStatus.confirmedPending]);
      expect(updates.single.image!.bytes, [1, 2, 3]);
      expect(updates.single.image!.schemeUrl, 'wxwork://fixture');
      owner.events.add(
        const model.QrSignInEvent(phase: model.QrSignInPhase.credentials),
      );
      owner.events.add(
        const model.QrSignInEvent(phase: model.QrSignInPhase.completed),
      );
      await owner.events.close();
      expect(await result, isTrue);
      expect(updates.last.phase, QrLoginPhase.credentials);
      expect(owner.starts, 1);
      final count = statuses.length;
      await tester.pump(const Duration(seconds: 1));
      expect(statuses.length, count);
      account.dispose();
    },
  );

  test('QR adapter preserves native errors and rejects a stream without completion', () async {
    for (final nativeError in [true, false]) {
      final owner = _Account();
      final account = RustAccountCenter.fromNative(owner);
      final result = account.signInQr(onUpdate: (_) {}, onStatus: (_) {});
      final check = expectLater(
        result,
        throwsA(
          isA<ShuAuthException>().having(
            (error) => error.code,
            'code',
            nativeError ? 'qrExpired' : 'cancelled',
          ),
        ),
      );
      if (nativeError) {
        owner.events.add(
          const model.QrSignInEvent(
            phase: model.QrSignInPhase.failed,
            error: errors.AuthFailure(
              code: 'qrExpired',
              message: 'native expired',
            ),
          ),
        );
      }
      await owner.events.close();
      await check;
      account.dispose();
    }
  });

  test(
    'QR adapter reports unavailable and cancels through the native owner',
    () async {
      final owner = _Account();
      final account = RustAccountCenter.fromNative(owner);
      final result = account.signInQr(onUpdate: (_) {}, onStatus: (_) {});
      account.cancelQr();
      expect(owner.cancels, 1);
      owner.events.add(
        const model.QrSignInEvent(phase: model.QrSignInPhase.unavailable),
      );
      await owner.events.close();
      expect(await result, isFalse);
      account.dispose();
    },
  );

  test(
    'disposed QR adapter ignores late events and rejects new dispatch',
    () async {
      final owner = _Account();
      final account = RustAccountCenter.fromNative(owner);
      var updates = 0;
      final result = account.signInQr(
        onUpdate: (_) => updates++,
        onStatus: (_) {},
      );
      final check = expectLater(result, throwsA(isA<ShuAuthException>()));
      account.dispose();
      owner.events.add(
        const model.QrSignInEvent(phase: model.QrSignInPhase.credentials),
      );
      owner.events.add(
        const model.QrSignInEvent(phase: model.QrSignInPhase.completed),
      );
      await owner.events.close();
      await check;
      expect(updates, 0);
      await expectLater(
        account.signInQr(onUpdate: (_) {}, onStatus: (_) {}),
        throwsA(isA<ShuAuthException>()),
      );
      expect(owner.starts, 1);
      expect(owner.closes, 1);
    },
  );
}

class _Account implements native.AccountSession {
  model.SignInState passwordState = model.SignInState.completed;
  Completer<model.SignInState>? pendingPassword;
  errors.AuthFailure? failure;
  List<String>? passwordInput;
  List<String>? codeInput;
  final events = StreamController<model.QrSignInEvent>();
  sso.QrStatus? status;
  int starts = 0;
  int cancels = 0;
  int closes = 0;
  @override
  bool isDisposed = false;
  @override
  Future<model.SignInState> signInPassword({
    required String username,
    required String password,
  }) async {
    passwordInput = [username, password];
    if (failure != null) throw failure!;
    return pendingPassword == null
        ? passwordState
        : await pendingPassword!.future;
  }

  @override
  Future<void> signInCode({
    required String method,
    required String code,
  }) async {
    codeInput = [method, code];
    if (failure != null) throw failure!;
  }

  @override
  delivery.VerificationState? verificationState() =>
      const delivery.VerificationState(
        methods: [sso.VerificationTarget(method: 'sms', target: 'masked')],
        selectedMethod: 'sms',
        resendAfterSeconds: 17,
      );
  @override
  model.Snapshot snapshot() => model.Snapshot(
    signedIn: false,
    busy: true,
    credentials: [],
    availableCount: 0,
    totalCount: 3,
    needsReverification: false,
    sessionExpired: false,
    maskedDeviceId: 'fixture',
    qrStatus: status,
  );
  @override
  Stream<model.QrSignInEvent> signInQr() {
    starts++;
    return events.stream;
  }

  @override
  void cancelQr() => cancels++;
  @override
  void close() => closes++;
  @override
  void dispose() => isDisposed = true;
  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw StateError('Unexpected stage command: ${invocation.memberName}');
}
