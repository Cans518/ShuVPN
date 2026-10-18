enum ShuVerificationMethod {
  wecom('wecom', '企业微信'),
  sms('sms', '手机号');

  const ShuVerificationMethod(this.wireName, this.label);
  final String wireName;
  final String label;

  static ShuVerificationMethod? fromWire(String? value) => switch (value) {
    'wecom' => wecom,
    'sms' => sms,
    _ => null,
  };
}

/// Verification methods and masked destinations supplied by the native owner.
class ShuLoginChallenge {
  const ShuLoginChallenge({
    required this.methods,
    required this.selectedMethod,
    this.resendAfterSeconds = 0,
  });
  final Map<ShuVerificationMethod, String> methods;
  final ShuVerificationMethod selectedMethod;
  final int resendAfterSeconds;
  bool get isEmpty => methods.isEmpty;

  String describe(ShuVerificationMethod method) {
    final target = methods[method];
    if (target == null || target.isEmpty) return '发送至统一身份认证绑定的账号';
    return method == ShuVerificationMethod.wecom
        ? '发送至企业微信账号 $target'
        : '发送至手机号 $target';
  }
}
