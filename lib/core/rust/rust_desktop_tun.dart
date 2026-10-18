import 'dart:convert';

import '../connection/native_tun.dart';
import 'generated/api/desktop.dart' as desktop;
import 'generated/api/tun.dart' as tun;
import 'generated/connection/model.dart' as model;

final class RustDesktopTunPlatform implements NativeTunPlatform {
  const RustDesktopTunPlatform();

  @override
  String normalizeDns(String value) {
    try {
      return desktop.validateDesktopDns(value: value);
    } on model.VpnFailure catch (error) {
      throw FormatException(error.message);
    }
  }

  @override
  Future<NativeTunEnvironment> inspect() async {
    final driver =
        jsonDecode(await tun.tunDriverInfo()) as Map<String, dynamic>;
    final networks = await desktop.desktopNetworkInterfaces();
    return NativeTunEnvironment(
      available: driver['available'] == true,
      reason: driver['unavailableReason'] as String?,
      networks: List.unmodifiable([
        for (final network in networks)
          NativeTunNetwork(
            name: network.name,
            description: network.description,
            recommended: network.recommended,
          ),
      ]),
    );
  }
}
