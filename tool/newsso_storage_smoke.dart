import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/newsso.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if (args.length == 3) {
    await _stage(args[0], args[1], args[2]);
    return;
  }
  if (args.length != 1) {
    throw ArgumentError(
      'Usage: dart run tool/newsso_storage_smoke.dart <native-library>',
    );
  }
  final library = File(args.single).absolute.path;
  final directory = await Directory.systemTemp.createTemp(
    'shuvpn-newsso-process-',
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
          !(await output).contains('NEWSso_STAGE_${stage}_PASSED')) {
        throw StateError(
          'Native persistent stage $stage failed: ${await errors}',
        );
      }
      await errors;
    }
    stdout.writeln(
      'NEWSso_CROSS_PROCESS_IMPORT_RESTORE_LOGOUT_AND_MIGRATION_GATE_PASSED',
    );
  } finally {
    await directory.delete(recursive: true);
  }
}

Future<void> _stage(String library, String directory, String stage) async {
  await RustCore.initialize(externalLibrary: ExternalLibrary.open(library));
  final session = await NewssoSession.open(directory: directory);
  switch (stage) {
    case 'import':
      if (!session.snapshot().legacyImportAllowed ||
          session.importLegacy(
                document: '[{"name":"SHU_OAUTH2","value":"process-fixture-only","path":"/"}]',
              ) !=
              BigInt.one) {
        throw StateError('Initial migration failed');
      }
      break;
    case 'restore':
      if (!session.snapshot().hasSession ||
          session.snapshot().legacyImportAllowed) {
        throw StateError('Restart did not restore committed credentials');
      }
      break;
    case 'logout':
      session.clear();
      break;
    case 'logged-out':
      if (session.snapshot().hasSession ||
          session.snapshot().legacyImportAllowed) {
        throw StateError('Restart resurrected logged-out credentials');
      }
      break;
    default:
      throw ArgumentError('Unknown storage stage');
  }
  stdout.writeln('NEWSso_STAGE_${stage}_PASSED');
  // Abrupt process exit proves the next reader uses disk and OS-released locks,
  // rather than reusing a Dart/Rust object or an orderly close callback.
  exit(0);
}
