import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/account/system_credential.dart';
import 'package:shuvpn/core/auth/auth_constants.dart';
import 'package:shuvpn/core/auth/login_challenge.dart';

void main() {
  test('only available credentials count as usable', () {
    expect(
      const ShuSystemCredential(
        systemId: 'otp',
        state: ShuCredentialState.available,
      ).isUsable,
      isTrue,
    );
    expect(
      const ShuSystemCredential(
        systemId: 'otp',
        state: ShuCredentialState.expired,
      ).isUsable,
      isFalse,
    );
  });

  test('failures carry a code and a message', () {
    final failed = ShuSystemCredential.failed(
      'atrust',
      code: 'atrustNoSid',
      message: '未下发会话凭证',
    );
    expect(failed.state, ShuCredentialState.failing);
    expect(failed.errorCode, 'atrustNoSid');
    expect(failed.errorMessage, '未下发会话凭证');
  });

  test('copyWith can clear a previous error', () {
    final failed = ShuSystemCredential.failed('otp', code: 'x', message: 'y');
    final retried = failed.copyWith(
      state: ShuCredentialState.refreshing,
      clearError: true,
    );
    expect(retried.errorCode, isNull);
    expect(retried.errorMessage, isNull);
    expect(retried.state, ShuCredentialState.refreshing);
  });

  test('the three campus systems retain their display order', () {
    expect(ShuOAuthTargets.all.map((target) => target.kind.id).toList(), [
      'jwxt',
      'atrust',
      'otp',
    ]);
    expect(ShuOAuthTargets.all.first, same(ShuOAuthTargets.jwxt));
    expect(ShuOAuthTargets.visible.first, same(ShuOAuthTargets.jwxt));
  });

  test('byId resolves every displayed system and rejects unknown ids', () {
    for (final target in ShuOAuthTargets.all) {
      expect(ShuOAuthTargets.byId(target.kind.id), same(target));
    }
    expect(ShuOAuthTargets.byId('webvpn'), isNull);
    expect(ShuOAuthTargets.all.map((target) => target.kind.host), [
      'jwxt.shu.edu.cn',
      'atrust.shu.edu.cn',
      'otp.shu.edu.cn',
    ]);
  });

  test('native challenge methods preserve masked destination presentation', () {
    expect(ShuVerificationMethod.fromWire('sms'), ShuVerificationMethod.sms);
    expect(
      ShuVerificationMethod.fromWire('wecom'),
      ShuVerificationMethod.wecom,
    );
    expect(ShuVerificationMethod.fromWire('unknown'), isNull);
    const challenge = ShuLoginChallenge(
      methods: {ShuVerificationMethod.sms: '138****8888'},
      selectedMethod: ShuVerificationMethod.sms,
    );
    expect(
      challenge.describe(ShuVerificationMethod.sms),
      contains('138****8888'),
    );
    expect(challenge.describe(ShuVerificationMethod.wecom), contains('绑定的账号'));
  });
}
