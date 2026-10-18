import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/settings.dart';
import 'package:shuvpn/core/rust/generated/settings/model.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if (args.length == 3) {
    await _stage(args[0], args[1], args[2]);
    return;
  }
  if (args.length != 1) {
    throw ArgumentError(
      'Usage: dart run tool/settings_storage_smoke.dart <native-library>',
    );
  }
  final library = File(args.single).absolute.path;
  final directory = await Directory.systemTemp.createTemp(
    'shuvpn-settings-process-',
  );
  try {
    for (final stage in ['import', 'restore', 'reset', 'restored-defaults']) {
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
          !(await output).contains('SETTINGS_STAGE_${stage}_PASSED')) {
        throw StateError('Settings stage $stage failed: ${await errors}');
      }
      await errors;
    }
    stdout.writeln(
      'SETTINGS_CROSS_PROCESS_MIGRATION_RESTORE_RESET_AND_IMPORT_GATE_PASSED',
    );
  } finally {
    await directory.delete(recursive: true);
  }
}

Future<void> _stage(String library, String directory, String stage) async {
  await RustCore.initialize(externalLibrary: ExternalLibrary.open(library));
  final session = await SettingsSession.open(
    directory: directory,
    legacyJson: stage == 'import'
        ? '{"settings.schema.version":2,"settings.server":"fixture.example","settings.socksPort":1080,"auth.session":"must-not-store"}'
        : 'must-ignore-stale-legacy',
    defaults: const Defaults(debugLogging: false),
  );
  final snapshot = session.snapshot();
  if (!snapshot.welcomeCompleted || snapshot.socksPort != 1080) {
    throw StateError('Migration lost unrelated settings');
  }
  switch (stage) {
    case 'import':
    case 'restore':
    case 'reset':
      if (snapshot.server != 'fixture.example') {
        throw StateError('Committed server not restored');
      }
      if (stage == 'reset') await session.restoreConnectionDefaults();
    case 'restored-defaults':
      if (snapshot.server != 'atrust.shu.edu.cn') {
        throw StateError('Reset not durable');
      }
    default:
      throw ArgumentError('Unknown settings stage');
  }
  stdout.writeln('SETTINGS_STAGE_${stage}_PASSED');
  // Exercise process exit and OS lock release without a graceful close.
  exit(0);
}
