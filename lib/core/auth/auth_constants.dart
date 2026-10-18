/// Display metadata only. OAuth registration and protocol policy live in Rust.
class ShuOAuthTarget {
  const ShuOAuthTarget({required this.kind});
  final ShuOAuthTargetKind kind;
}

abstract final class ShuOAuthTargets {
  static const jwxt = ShuOAuthTarget(kind: ShuOAuthTargetKind.jwxt);
  static const atrust = ShuOAuthTarget(kind: ShuOAuthTargetKind.atrust);
  static const otp = ShuOAuthTarget(kind: ShuOAuthTargetKind.otp);
  static const all = <ShuOAuthTarget>[jwxt, atrust, otp];
  static const visible = <ShuOAuthTarget>[jwxt, atrust, otp];

  static ShuOAuthTarget? byId(String id) {
    for (final target in all) {
      if (target.kind.id == id) return target;
    }
    return null;
  }
}

enum ShuOAuthTargetKind {
  atrust('atrust', 'aTrust 网关', 'atrust.shu.edu.cn', '校园隧道连接'),
  otp('otp', 'OTP 令牌', 'otp.shu.edu.cn', '动态口令验证'),
  jwxt('jwxt', '教务系统', 'jwxt.shu.edu.cn', '校园身份与教务服务');

  const ShuOAuthTargetKind(
    this.id,
    this.displayName,
    this.host,
    this.description,
  );
  final String id;
  final String displayName;
  final String host;
  final String description;

  static ShuOAuthTargetKind? fromId(String? id) {
    for (final kind in values) {
      if (kind.id == id) return kind;
    }
    return null;
  }
}
