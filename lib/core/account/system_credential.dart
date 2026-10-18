/// Presentation metadata; credential secrets remain in the native account.
class ShuSystemCredential {
  const ShuSystemCredential({
    required this.systemId,
    required this.state,
    this.errorCode,
    this.errorMessage,
  });
  factory ShuSystemCredential.missing(String systemId) => ShuSystemCredential(
    systemId: systemId,
    state: ShuCredentialState.missing,
  );
  factory ShuSystemCredential.failed(
    String systemId, {
    required String code,
    required String message,
  }) => ShuSystemCredential(
    systemId: systemId,
    state: ShuCredentialState.failing,
    errorCode: code,
    errorMessage: message,
  );
  final String systemId;
  final ShuCredentialState state;
  final String? errorCode;
  final String? errorMessage;
  bool get isUsable => state == ShuCredentialState.available;

  ShuSystemCredential copyWith({
    ShuCredentialState? state,
    String? errorCode,
    String? errorMessage,
    bool clearError = false,
  }) => ShuSystemCredential(
    systemId: systemId,
    state: state ?? this.state,
    errorCode: clearError ? null : (errorCode ?? this.errorCode),
    errorMessage: clearError ? null : (errorMessage ?? this.errorMessage),
  );
}

enum ShuCredentialState {
  unknown('未知'),
  refreshing('检查中'),
  available('可用'),
  missing('未登录'),
  expired('已过期'),
  failing('交换失败');

  const ShuCredentialState(this.label);
  final String label;
}
