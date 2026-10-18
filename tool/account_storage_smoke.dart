import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/account.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if (args.length == 3) {
    await _stage(args[0], args[1], args[2]);
    return;
  }
  if (args.length != 1) {
    throw ArgumentError(
      'Usage: dart run tool/account_storage_smoke.dart <native-library>',
    );
  }
  final library = File(args.single).absolute.path;
  final directory = await Directory.systemTemp.createTemp(
    'shuvpn-account-process-',
  );
  try {
    for (final stage in ['import', 'restore', 'logout', 'logged-out']) {
      final child = await Process.start(Platform.resolvedExecutable, [
        'run',
        Platform.script.toFilePath(),
        library,
        directory.path,
        stage,
      ]);
      final output = child.stdout.transform(utf8.decoder).join();
      final errors = child.stderr.transform(utf8.decoder).join();
      int status;
      try {
        status = await child.exitCode.timeout(const Duration(seconds: 45));
      } on TimeoutException {
        child.kill();
        await child.exitCode;
        rethrow;
      }
      if (status != 0 ||
          !(await output).contains('ACCOUNT_STAGE_${stage}_PASSED')) {
        throw StateError('Native account stage $stage failed: ${await errors}');
      }
      await errors;
    }
    stdout.writeln(
      'ACCOUNT_CROSS_PROCESS_BUNDLE_RESTORE_LOGOUT_AND_DEVICE_PRESERVATION_PASSED',
    );
  } finally {
    await directory.delete(recursive: true);
  }
}

Future<void> _stage(String library, String directory, String stage) async {
  await RustCore.initialize(externalLibrary: ExternalLibrary.open(library));
  final session = await AccountSession.open(
    directory: directory,
    legacyCookies:
        '[{"name":"SHU_OAUTH2","value":"process-fixture-only","path":"/"}]',
    legacyDeviceId: stage == 'import'
        ? 'original-device'
        : 'must-not-overwrite',
    legacySnapshot: '{"accountId":"25123456","profile":{"name":"fixture"},"credentials":{"jwxt":"available"}}',
  );
  final snapshot = session.snapshot();
  if (snapshot.maskedDeviceId != 'orig••••vice') {
    throw StateError('Restart changed stable device identity');
  }
  switch (stage) {
    case 'import':
    case 'restore':
      if (!snapshot.signedIn ||
          snapshot.accountId != '25123456' ||
          snapshot.profile?.name != 'fixture' ||
          snapshot.availableCount != 1) {
        throw StateError('Complete committed account bundle not restored');
      }
      break;
    case 'logout':
      session.signOut();
      break;
    case 'logged-out':
      if (snapshot.signedIn ||
          snapshot.accountId != null ||
          snapshot.profile != null ||
          snapshot.availableCount != 0) {
        throw StateError('Restart resurrected logged-out account bundle');
      }
      break;
    default:
      throw ArgumentError('Unknown account stage');
  }
  stdout.writeln('ACCOUNT_STAGE_${stage}_PASSED');
  // Intentionally skip close: the next process must restore disk and OS locks.
  exit(0);
}
