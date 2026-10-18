/// A native account failure presented to the user.
class ShuAuthException implements Exception {
  const ShuAuthException(this.code, this.message);
  final String code;
  final String message;

  @override
  String toString() => message;
}
