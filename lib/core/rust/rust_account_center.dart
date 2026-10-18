import 'dart:async';

import 'package:shared_preferences/shared_preferences.dart';

import '../account/account_center.dart';
import '../account/qr_account.dart';
import '../auth/account_authentication.dart';
import '../account/system_credential.dart';
import '../account/account_profile.dart';
import '../auth/auth_exception.dart';
import '../auth/login_challenge.dart';
import 'generated/api/account.dart' as native;
import 'generated/auth/model.dart' as errors;
import 'generated/auth/newsso/account/model.dart' as model;
import 'native_account_authentication.dart';

final class RustAccountCenter extends AccountCenter
    implements NativeAccountAuthentication, QrAccount {
  RustAccountCenter.fromNative(this.nativeAccount)
    : _snapshot = nativeAccount.snapshot();

  static const legacyKeys = [
    'auth.newsso.cookies',
    'auth.account.snapshot',
    'atrust_device_id',
  ];

  static Future<RustAccountCenter> open(
    String directory,
    SharedPreferences preferences,
  ) async {
    final account = await native.AccountSession.open(
      directory: directory,
      legacyCookies: preferences.getString(legacyKeys[0]) ?? '[]',
      legacySnapshot: preferences.getString(legacyKeys[1]) ?? '{}',
      legacyDeviceId: preferences.getString(legacyKeys[2]),
    );
    try {
      for (final key in legacyKeys) {
        if (preferences.containsKey(key) && !await preferences.remove(key)) {
          throw StateError('Rust 已提交账户迁移，旧偏好清理失败，请重试启动');
        }
      }
      return RustAccountCenter.fromNative(account);
    } on Object {
      account.close();
      account.dispose();
      rethrow;
    }
  }

  @override
  final native.AccountSession nativeAccount;
  model.Snapshot _snapshot;
  String? _lastError;
  bool _disposed = false;

  void _refresh() {
    if (_disposed) return;
    _snapshot = nativeAccount.snapshot();
    notifyListeners();
  }

  Future<T> _call<T>(
    Future<T> Function() command, {
    void Function()? onRefresh,
  }) async {
    if (_disposed) {
      throw const ShuAuthException('closed', '账户已经关闭');
    }
    _lastError = null;
    final ticker = Timer.periodic(const Duration(milliseconds: 250), (_) {
      if (!_disposed) {
        _refresh();
        onRefresh?.call();
      }
    });
    try {
      final result = await command();
      if (_disposed) {
        throw const ShuAuthException('closed', '账户已经关闭');
      }
      return result;
    } on errors.AuthFailure catch (error) {
      _lastError = error.message;
      throw ShuAuthException(error.code, error.message);
    } finally {
      ticker.cancel();
      _refresh();
    }
  }

  @override
  String? get accountId => _snapshot.accountId;
  @override
  bool get signedIn => _snapshot.signedIn;
  @override
  bool get hasSession => signedIn;
  @override
  DateTime? get signedInAt => _snapshot.signedInAt == null
      ? null
      : DateTime.fromMillisecondsSinceEpoch(_snapshot.signedInAt! * 1000);
  @override
  bool get busy => _snapshot.busy;
  @override
  String? get progressLabel => _snapshot.progressLabel;
  @override
  String? get errorMessage => _snapshot.errorMessage ?? _lastError;
  @override
  String get maskedDeviceId => _snapshot.maskedDeviceId;
  @override
  bool get sessionExpired => _snapshot.sessionExpired;
  @override
  bool get needsReverification => _snapshot.needsReverification;
  @override
  AccountAuthentication get authSession => this;
  @override
  CredentialSummary get summary => CredentialSummary(
    available: _snapshot.availableCount,
    total: _snapshot.totalCount,
  );
  @override
  List<ShuSystemCredential> get credentials => [
    for (final item in _snapshot.credentials)
      ShuSystemCredential(
        systemId: item.systemId,
        state: ShuCredentialState.values.byName(item.state.name),
        errorCode: item.errorCode,
        errorMessage: item.errorMessage,
      ),
  ];
  @override
  ShuJwxtProfile? get profile {
    final profile = _snapshot.profile;
    final name = profile?.name;
    if (profile == null || name == null) return null;
    return ShuJwxtProfile(
      name: name,
      studentId: profile.studentId,
      grade: profile.grade,
      college: profile.college,
      major: profile.major,
    );
  }

  @override
  void clearSessionExpired() {
    nativeAccount.clearSessionExpired();
    _refresh();
  }

  @override
  void cancelPending() {
    if (_disposed) return;
    nativeAccount.cancelWork();
    _refresh();
  }

  ShuSignInOutcome _outcome(model.SignInState result) {
    if (result == model.SignInState.completed) return ShuSignInOutcome.success;
    final challenge = loginChallenge;
    if (challenge == null) return ShuSignInOutcome.failed;
    return ShuSignInOutcome.needsVerification(challenge);
  }

  @override
  ShuLoginChallenge? get loginChallenge {
    if (_disposed) return null;
    final state = nativeAccount.verificationState();
    if (state == null) return null;
    final methods = <ShuVerificationMethod, String>{};
    for (final target in state.methods) {
      final method = ShuVerificationMethod.fromWire(target.method);
      if (method != null) {
        methods[method] = target.target;
      }
    }
    final selected = ShuVerificationMethod.fromWire(state.selectedMethod);
    if (selected == null) {
      throw StateError('原生账户返回了不支持的验证方式');
    }
    return ShuLoginChallenge(
      methods: methods,
      selectedMethod: selected,
      resendAfterSeconds: state.resendAfterSeconds,
    );
  }

  @override
  void selectVerificationMethod(ShuVerificationMethod method) {
    try {
      nativeAccount.selectVerificationMethod(method: method.wireName);
    } on errors.AuthFailure catch (error) {
      throw ShuAuthException(error.code, error.message);
    }
    _refresh();
  }

  @override
  Future<ShuSignInOutcome> signIn({
    required String username,
    required String password,
  }) async {
    try {
      return _outcome(
        await _call(
          () => nativeAccount.signInPassword(
            username: username,
            password: password,
          ),
        ),
      );
    } on ShuAuthException {
      return ShuSignInOutcome.failed;
    }
  }

  @override
  Future<void> sendCode(ShuVerificationMethod method) =>
      _call(() => nativeAccount.sendCode(method: method.wireName));
  @override
  Future<bool> submitCode(ShuVerificationMethod method, String code) async {
    try {
      await _call(
        () => nativeAccount.signInCode(method: method.wireName, code: code),
      );
      return true;
    } on ShuAuthException {
      return false;
    }
  }

  @override
  Future<bool> verifyIfStale() async {
    try {
      return await _call(nativeAccount.verifyIfStale);
    } on ShuAuthException {
      return false;
    }
  }

  @override
  Future<void> signOut() => _call(() async => nativeAccount.signOut());

  @override
  Future<bool> signInQr({
    required void Function(QrLoginUpdate) onUpdate,
    required void Function(QrLoginStatus) onStatus,
  }) {
    return _call(
      () async {
        bool? completed;
        await for (final event in nativeAccount.signInQr()) {
          if (_disposed) continue;
          switch (event.phase) {
            case model.QrSignInPhase.failed:
              throw event.error ??
                  const ShuAuthException('bridge', '扫码登录错误事件缺少详情');
            case model.QrSignInPhase.completed:
              completed = true;
            case model.QrSignInPhase.unavailable:
              completed = false;
            case model.QrSignInPhase.waiting:
            case model.QrSignInPhase.redeeming:
            case model.QrSignInPhase.credentials:
              final image = event.image;
              onUpdate(
                QrLoginUpdate(
                  phase: QrLoginPhase.values.byName(event.phase.name),
                  image: image == null
                      ? null
                      : QrLoginImage(
                          bytes: image.imageBytes,
                          confirmUrl: image.confirmUrl,
                          schemeUrl: image.wxWorkSchemeUrl,
                        ),
                ),
              );
          }
        }
        if (completed == null) {
          throw const ShuAuthException('cancelled', '扫码登录未完成');
        }
        return completed;
      },
      onRefresh: () {
        final status = _snapshot.qrStatus;
        if (status != null) {
          onStatus(QrLoginStatus.values.byName(status.name));
        }
      },
    );
  }

  @override
  void cancelQr() {
    if (_disposed) return;
    nativeAccount.cancelQr();
    _refresh();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    nativeAccount.close();
    nativeAccount.dispose();
    super.dispose();
  }
}
