import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/tun.dart';
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if (!Platform.isLinux ||
      args.length != 2 ||
      !{'ready', 'unavailable'}.contains(args[1])) {
    throw ArgumentError(
      'Usage: linux_install_smoke <native-library> ready|unavailable',
    );
  }
  final status = File('/proc/self/status').readAsStringSync();
  final effective = RegExp(
    r'^CapEff:\s*([0-9a-f]+)$',
    multiLine: true,
  ).firstMatch(status);
  if (effective == null || int.parse(effective.group(1)!, radix: 16) != 0) {
    throw StateError(
      'Run this probe as an ordinary user with no effective capabilities',
    );
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(File(args[0]).absolute.path),
  );
  try {
    final ready = args[1] == 'ready';
    for (var iteration = 0; iteration < 3; iteration++) {
      final driver = jsonDecode(
        await tunDriverInfo().timeout(const Duration(seconds: 15)),
      ) as Map<String, dynamic>;
      if (driver['elevated'] != false ||
          driver['available'] != ready ||
          driver['helperReady'] != ready ||
          (!ready && driver['unavailableReason'] is! String)) {
        throw StateError(
          'Installed helper readiness did not match $ready: $driver',
        );
      }
    }
    stdout.writeln(
      'LINUX_INSTALLED_HELPER_FFI_${args[1].toUpperCase()}_PASSED',
    );
  } finally {
    ShuRustLib.dispose();
  }
}
