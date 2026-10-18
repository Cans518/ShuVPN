// Developer-only reporting for the connected desktop acceptance fixture.
import 'dart:convert';

import 'package:shuvpn/core/rust/generated/api/vpn.dart' as vpn;
import 'package:shuvpn/core/rust/generated/connection/model.dart' show VpnEvent;

Future<Map<String, Object?>?> readFixtureSession(
  vpn.VpnSession? session,
) async {
  if (session == null || session.isDisposed) return null;
  // FRB clones the Arc while encoding each call. Start every call before
  // yielding, so controller disposal cannot invalidate a later request.
  final values = await Future.wait<Object>([
    session.snapshot(),
    session.tunStatus(),
    session.egressStatus(),
    session.decisions(),
  ]);
  final snapshot = values[0] as VpnEvent;
  return {
    'state': snapshot.state.name,
    'upload': snapshot.uploadBytes.toInt(),
    'download': snapshot.downloadBytes.toInt(),
    'connections': snapshot.connections.toInt(),
    'tun': jsonDecode(values[1] as String),
    'egress': jsonDecode(values[2] as String),
    'decisions': jsonDecode(values[3] as String),
  };
}
