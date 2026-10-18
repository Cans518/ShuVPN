import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge.dart'
    show AnyhowException;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/diagnostics.dart';
import 'package:shuvpn/core/rust/generated/api/atrust_l3.dart' as l3;
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

/// Requires a real native library. No mock API or skipped assertions.
Future<void> main(List<String> args) async {
  if (args.length > 1) {
    throw ArgumentError(
      'Usage: dart run tool/rust_bridge_smoke.dart [library]',
    );
  }
  final libraryPath = args.isEmpty
      ? 'rust/target/release/'
            '${Platform.isWindows ? 'shuvpn_core.dll' : 'libshuvpn_core.so'}'
      : args.single;
  final version = await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(File(libraryPath).absolute.path),
  );
  try {
    if (version.isEmpty) throw StateError('Missing native core version');
    // Unicode and empty payloads exercise length and UTF-8 handling across FFI.
    for (final nonce in <String>['bridge-roundtrip', '上海大学 🔗', '']) {
      final events = await probeCore(nonce: nonce)
          .timeout(const Duration(seconds: 10))
          .toList();
      if (events.length != 3 ||
          events[0].kind != CoreEventKind.ready ||
          events[0].message != version ||
          events[1].kind != CoreEventKind.log ||
          events[2].kind != CoreEventKind.pong ||
          events[2].message != nonce) {
        throw StateError('Native event stream roundtrip failed');
      }
    }
    final versions = await Future.wait(
      List<Future<String>>.generate(16, (_) => coreVersion()),
    );
    if (versions.any((value) => value != version)) {
      throw StateError(
        'Concurrent native calls returned inconsistent versions',
      );
    }
    await _verifyL3();
    stdout.writeln('Rust bridge smoke passed · core $version');
  } finally {
    ShuRustLib.dispose();
  }
}

Future<void> _verifyL3() async {
  final heartbeat = await l3.heartbeatRequest();
  _require(_sameBytes(heartbeat, <int>[5, 0x15, 0, 0]), 'Heartbeat encoding');
  final opening = await l3.authTunnelRequest(sid: 'REDACTED_SID');
  _require(
    _sameBytes(opening, <int>[
      5,
      1,
      0xd0,
      0x53,
      0,
      0,
      22,
      ...utf8.encode('{"sid":"REDACTED_SID"}'),
      5,
      4,
      0,
      1,
      0,
      0,
      0,
      0,
      0,
      0,
    ]),
    'L3 auth request encoding',
  );
  final decoder = l3.L3FrameDecoder();
  final handshakeDecoder = l3.L3HandshakeDecoder();
  try {
    final frames = <l3.L3Frame>[];
    for (final byte in <int>[5, 0x94, 0, 2, 42, 43, 5, 0x95, 0, 0]) {
      frames.addAll(await decoder.push(chunk: <int>[byte]));
    }
    _require(
      frames.length == 2 &&
          frames[0].command == 0x94 &&
          _sameBytes(frames[0].payload, <int>[42, 43]) &&
          frames[1].command == 0x95,
      'L3 fragmented frames',
    );
    await _requireNativeFailure(() => decoder.push(chunk: <int>[5, 0]));
    await _requireNativeFailure(() => decoder.push(chunk: heartbeat));
    await decoder.reset();
    _require(
      (await decoder.push(chunk: heartbeat)).length == 1,
      'L3 explicit reset',
    );
    final payload = utf8.encode('{"code":0,"data":{"deviceId":"test-device"}}');
    final response = <int>[
      5,
      0xd0,
      0x53,
      0,
      payload.length >> 8,
      payload.length & 255,
      ...payload,
      5,
      0,
      0,
      1,
      10,
      95,
      178,
      77,
      0,
      0,
      5,
      0x95,
      0,
      0,
    ];
    _require(
      await handshakeDecoder.push(chunk: response.sublist(0, 7)) == null,
      'L3 incomplete handshake',
    );
    final handshake = await handshakeDecoder.push(chunk: response.sublist(7));
    _require(
      handshake != null &&
          handshake.deviceId == 'test-device' &&
          handshake.virtualAddresses.single == '10.95.178.77' &&
          _sameBytes(handshake.remainingBytes, <int>[5, 0x95, 0, 0]),
      'L3 VIP and handshake leftovers',
    );
  } finally {
    decoder.dispose();
    handshakeDecoder.dispose();
  }
}

bool _sameBytes(List<int> actual, List<int> expected) =>
    actual.length == expected.length &&
    Iterable<int>.generate(actual.length)
        .every((index) => actual[index] == expected[index]);

void _require(bool condition, String description) {
  if (!condition) throw StateError('$description failed');
}

Future<void> _requireNativeFailure(Future<Object?> Function() operation) async {
  try {
    await operation();
  } on AnyhowException {
    return;
  }
  throw StateError('Rust accepted malformed input');
}
