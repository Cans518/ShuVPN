import 'dart:typed_data';

class QrLoginImage {
  const QrLoginImage({
    required this.bytes,
    required this.confirmUrl,
    required this.schemeUrl,
  });
  final Uint8List bytes;
  final String confirmUrl;
  final String schemeUrl;
}

enum QrLoginStatus { waiting, confirmedPending, succeeded, expired }

enum QrLoginPhase { waiting, redeeming, credentials }

class QrLoginUpdate {
  const QrLoginUpdate({required this.phase, this.image});
  final QrLoginPhase phase;
  final QrLoginImage? image;
}

abstract interface class QrAccount {
  Future<bool> signInQr({
    required void Function(QrLoginUpdate) onUpdate,
    required void Function(QrLoginStatus) onStatus,
  });
  void cancelQr();
}
