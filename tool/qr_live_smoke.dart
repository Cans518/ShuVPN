import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/account.dart' as native;
import 'package:shuvpn/core/rust/generated/auth/model.dart' as errors;
import 'package:shuvpn/core/rust/generated/auth/newsso/account/model.dart'
    as model;
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

/// Optional public-provider probe. Cancels immediately after receiving an image.
/// Uses a temporary empty account and never displays or logs the QR or its URL.
Future<void> main(List<String> args) async {
  if (args.length != 1) {
    throw ArgumentError('Expected a packaged native library path');
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(File(args.single).absolute.path),
  );
  final directory = await Directory.systemTemp.createTemp('shuvpn-qr-probe-');
  native.AccountSession? account;
  try {
    final owner = await native.AccountSession.open(
      directory: directory.path,
      legacyCookies: '[]',
      legacySnapshot: '{}',
    );
    account = owner;
    var imageBytes = 0;
    var cancelled = false;
    try {
      await for (final event in owner.signInQr().timeout(
        const Duration(seconds: 30),
      )) {
        if (event.phase == model.QrSignInPhase.failed) {
          throw event.error ?? StateError('Native failure event has no detail');
        }
        final image = event.image;
        if (event.phase != model.QrSignInPhase.waiting ||
            image == null ||
            image.imageBytes.isEmpty) {
          throw StateError(
            'Unexpected phase before the public QR probe was cancelled',
          );
        }
        imageBytes = image.imageBytes.length;
        owner.cancelQr();
      }
    } on errors.AuthFailure catch (error) {
      if (error.code != 'cancelled' || imageBytes == 0) rethrow;
      cancelled = true;
    }
    final snapshot = owner.snapshot();
    if (!cancelled ||
        snapshot.busy ||
        snapshot.signedIn ||
        snapshot.qrStatus != null) {
      throw StateError('Cancelled QR operation retained active login state');
    }
    stdout.writeln(
      jsonEncode({
        'observed_at': DateTime.now().toIso8601String(),
        'image_bytes': imageBytes,
        'cancelled': cancelled,
        'signed_in': snapshot.signedIn,
      }),
    );
    stdout.writeln('QR_LIVE_NATIVE_IMAGE_AND_CANCEL_PASSED');
  } finally {
    try {
      account?.close();
    } finally {
      account?.dispose();
      await directory.delete(recursive: true);
      ShuRustLib.dispose();
    }
  }
}
