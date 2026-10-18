import '../auth/account_authentication.dart';
import 'generated/api/account.dart';

abstract interface class NativeAccountAuthentication
    implements AccountAuthentication {
  AccountSession get nativeAccount;
}
