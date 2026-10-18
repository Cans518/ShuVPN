import 'package:shuvpn/core/account/account_center.dart';
import 'package:shuvpn/core/account/account_profile.dart';
import 'package:shuvpn/core/account/system_credential.dart';
import 'package:shuvpn/core/auth/account_authentication.dart';
import 'package:shuvpn/core/auth/login_challenge.dart';

class StubAccountCenter extends AccountCenter implements AccountAuthentication {
  @override
  String? accountId;
  @override
  bool signedIn = false;
  @override
  bool get hasSession => signedIn;
  @override
  DateTime? signedInAt;
  @override
  bool busy = false;
  @override
  String? progressLabel;
  @override
  String? errorMessage;
  @override
  ShuJwxtProfile? profile;
  @override
  bool sessionExpired = false;
  @override
  bool get needsReverification => false;
  @override
  String get maskedDeviceId => 'test••••vice';
  @override
  AccountAuthentication get authSession => this;
  @override
  List<ShuSystemCredential> get credentials => [
    for (final id in ['jwxt', 'atrust', 'otp']) ShuSystemCredential.missing(id),
  ];
  @override
  CredentialSummary get summary => CredentialSummary.of(credentials);
  @override
  void clearSessionExpired() => sessionExpired = false;
  @override
  Future<bool> verifyIfStale() async => false;
  @override
  Future<ShuSignInOutcome> signIn({
    required String username,
    required String password,
  }) => throw StateError('test did not configure sign-in');
  @override
  Future<void> sendCode(ShuVerificationMethod method) =>
      throw StateError('test did not configure verification');
  @override
  ShuLoginChallenge? loginChallenge;
  @override
  void selectVerificationMethod(ShuVerificationMethod method) =>
      throw StateError('test did not configure verification selection');
  @override
  Future<bool> submitCode(ShuVerificationMethod method, String code) =>
      throw StateError('test did not configure verification');
  @override
  Future<void> signOut() async {
    signedIn = false;
    notifyListeners();
  }
}
