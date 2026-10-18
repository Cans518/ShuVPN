import 'package:flutter/foundation.dart';

import '../auth/account_authentication.dart';
import '../auth/login_challenge.dart';
import 'account_profile.dart';
import 'system_credential.dart';

/// 已经交换到凭据的系统集合。
///
/// 这是「账户」这一层对外暴露的全部内容：一个统一身份认证会话，加上它换来的
/// 三个系统凭据。UI 只依赖这个接口，所以换实现时不需要改任何 widget。
abstract class AccountCenter extends ChangeNotifier {
  /// 统一身份认证账号（学号），未登录时为 null。
  String? get accountId;

  /// 认证会话是否可用。
  bool get signedIn;

  /// 会话建立的时刻。
  DateTime? get signedInAt;

  /// 一轮授权 / 交换是否在进行中。
  bool get busy;

  /// 正在进行的操作说明，例如 `正在交换 OTP 令牌`。
  String? get progressLabel;

  /// 上一次失败的原因，展示给用户；成功时为 null。
  String? get errorMessage;

  /// 三个系统各自的凭据状态。
  List<ShuSystemCredential> get credentials;

  /// 可用的系统数 / 总数。
  CredentialSummary get summary;

  /// 上次核对到的教务系统档案（姓名 / 学号 / 年级 / 学院 / 专业）。
  ///
  /// 页面从**这里**取身份信息，而不是去 [credentials] 的载荷里现翻 ——
  /// 载荷是「这一轮交换换到了什么」，这里是「上次核对看到了什么」。
  /// 冷启动时前者是空的，后者有值（从磁盘快照读回），这正是账户页
  /// 不必一打开就联网的原因。
  ShuJwxtProfile? get profile;

  /// 会话是否已被服务端判定失效。
  ///
  /// 核对失败有很多种（网络抖动、教务系统改版），只有这一种意味着
  /// 「必须重新输密码」。UI 用它来决定弹不弹那个提示，不弹的方式是
  /// [clearSessionExpired]。
  bool get sessionExpired;

  /// 用户看过提示之后清掉标记，避免同一件事弹第二次。
  void clearSessionExpired();

  /// 是否需要在渲染账户页之前先核一遍。
  bool get needsReverification;

  /// 按需核对：磁盘上的结论还新鲜就什么都不做，否则跑一轮交换。
  ///
  /// 这是账户页在 `initState` 里调的那一个 —— **不在应用启动时调**。
  /// 核对由原生账户所有者负责：核对里有一项真的要建一次 aTrust 隧道，
  /// 让每个没打开过账户页的启动都付这笔钱是浪费。
  Future<bool> verifyIfStale();

  /// 用统一身份认证凭据登录。
  ///
  /// 返回「成功 / 需要两步验证 / 失败」三态，把 challenge 交给 UI，
  /// 而不是把它藏在可变字段里 —— 登录事务是原子的，状态也应该一次交卷。
  ///
  /// 无需二步验证时，原生操作继续获取凭据，完成后才返回成功。
  Future<ShuSignInOutcome> signIn({
    required String username,
    required String password,
  });

  /// 两步验证：发送验证码。
  Future<void> sendCode(ShuVerificationMethod method);

  /// Current native verification selection and resend countdown.
  ShuLoginChallenge? get loginChallenge;
  void selectVerificationMethod(ShuVerificationMethod method);

  /// 两步验证及后续凭据获取由同一个原生操作完成。
  Future<bool> submitCode(ShuVerificationMethod method, String code);

  /// 退出登录并清空所有凭据。
  Future<void> signOut();

  /// 供 aTrust 隧道复用的原生认证会话。
  ///
  /// aTrust 的网关把统一身份认证当作 IdP，所以建立隧道时需要拿这里已经
  /// 登录好的会话去换授权码 —— 两边必须是同一个会话，否则用户得登两次。
  AccountAuthentication get authSession;

  String? get maskedDeviceId => null;
  void cancelPending() {}
}

/// 登录结果。
@immutable
class ShuSignInOutcome {
  const ShuSignInOutcome._(this.status, this.challenge);

  /// 身份已建立，且至少一个系统凭据可用。
  static const success = ShuSignInOutcome._(ShuSignInStatus.success, null);

  /// 密码通过，但学校要求两步验证。
  factory ShuSignInOutcome.needsVerification(ShuLoginChallenge challenge) =>
      ShuSignInOutcome._(ShuSignInStatus.needsVerification, challenge);

  /// 登录失败，原因在 [AccountCenter.errorMessage]。
  static const failed = ShuSignInOutcome._(ShuSignInStatus.failed, null);

  final ShuSignInStatus status;

  /// 仅当 [status] 为 `needsVerification` 时非空。
  final ShuLoginChallenge? challenge;

  bool get isSuccess => status == ShuSignInStatus.success;

  bool get needsVerification => status == ShuSignInStatus.needsVerification;
}

enum ShuSignInStatus { success, needsVerification, failed }

/// 汇总数字。
@immutable
class CredentialSummary {
  const CredentialSummary({required this.available, required this.total});

  factory CredentialSummary.of(Iterable<ShuSystemCredential> credentials) {
    var available = 0;
    var total = 0;
    for (final credential in credentials) {
      total++;
      if (credential.isUsable) available++;
    }
    return CredentialSummary(available: available, total: total);
  }

  final int available;
  final int total;

  bool get allAvailable => total > 0 && available == total;

  String get label => total == 0 ? '未接入系统' : '$available/$total 系统凭据可用';
}
