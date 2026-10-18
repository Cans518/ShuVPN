import 'dart:async';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;

import 'generated/api/diagnostics.dart';
import 'generated/frb_generated.dart';

/// Loads and verifies the native core before any network command is dispatched.
final class RustCore {
  RustCore._();

  static Future<String>? _initialization;

  static Future<String> initialize({ExternalLibrary? externalLibrary}) =>
      _initialization ??= _initialize(externalLibrary);

  static Future<String> _initialize(ExternalLibrary? externalLibrary) async {
    await ShuRustLib.init(
      externalLibrary: externalLibrary ?? _bundledLibrary(),
    );
    try {
      final version = await coreVersion().timeout(const Duration(seconds: 10));
      const nonce = 'shuvpn-startup';
      final events = await probeCore(nonce: nonce)
          .timeout(const Duration(seconds: 10))
          .toList();
      if (events.length != 3 ||
          events[0].kind != CoreEventKind.ready ||
          events[0].message != version ||
          events[1].kind != CoreEventKind.log ||
          events[2].kind != CoreEventKind.pong ||
          events[2].message != nonce) {
        throw StateError('Rust core bridge health check failed');
      }
      return version;
    } on Object {
      ShuRustLib.dispose();
      rethrow;
    }
  }

  static ExternalLibrary? _bundledLibrary() {
    if (!Platform.isWindows && !Platform.isLinux) return null;
    final executable = File(Platform.resolvedExecutable);
    final library = File.fromUri(
      executable.parent.uri.resolve(
        Platform.isWindows ? 'shuvpn_core.dll' : 'lib/libshuvpn_core.so',
      ),
    );
    // Packaged apps must not pick an unrelated development build from the cwd.
    if (library.existsSync()) return ExternalLibrary.open(library.path);
    final name = executable.uri.pathSegments.last.toLowerCase();
    if (name == 'shuvpn' || name == 'shuvpn.exe') {
      throw StateError('缺少应用随附的核心文件，请重新解压完整桌面包');
    }
    return null;
  }
}
