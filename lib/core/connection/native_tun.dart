import '../rust/generated/platform/model.dart' show CaptureMode;

class NativeTunNetwork {
  const NativeTunNetwork({
    required this.name,
    required this.description,
    required this.recommended,
  });
  final String name;
  final String description;
  final bool recommended;
}

class NativeTunStartup {
  const NativeTunStartup({
    this.enabled = true,
    required this.mtu,
    required this.physicalInterface,
    this.physicalDns,
    this.captureMode = CaptureMode.campus,
  });
  final int mtu;
  final bool enabled;
  final String physicalInterface;
  final String? physicalDns;
  final CaptureMode captureMode;
}

class NativeTunEnvironment {
  const NativeTunEnvironment({
    required this.available,
    required this.networks,
    this.reason,
  });
  final bool available;
  final List<NativeTunNetwork> networks;
  final String? reason;
}

class NativeTunPreview {
  const NativeTunPreview({
    required this.mtu,
    required this.addresses,
    required this.routes,
    required this.capturesCampusDns,
    this.captureMode = CaptureMode.campus,
    this.protectedAddresses = const [],
    this.physicalDnsServers = const [],
  });
  final int mtu;
  final List<String> addresses;
  final List<String> routes;
  final bool capturesCampusDns;
  final CaptureMode captureMode;
  final List<String> protectedAddresses;
  final List<String> physicalDnsServers;
}

class NativeTunSnapshot {
  const NativeTunSnapshot({
    required this.state,
    required this.cleanupPending,
    this.name,
    this.physicalInterface,
    this.mtu,
    this.routes = const [],
    this.diagnostic,
    this.captureMode = CaptureMode.campus,
  });
  const NativeTunSnapshot.stopped()
    : state = 'stopped',
      cleanupPending = false,
      name = null,
      physicalInterface = null,
      mtu = null,
      routes = const [],
      captureMode = CaptureMode.campus,
      diagnostic = null;
  final String state;
  final bool cleanupPending;
  final String? name;
  final String? physicalInterface;
  final int? mtu;
  final List<String> routes;
  final String? diagnostic;
  final CaptureMode captureMode;
  bool get running => state == 'running';
}

abstract interface class NativeTunPlatform {
  Future<NativeTunEnvironment> inspect();
  String normalizeDns(String value);
}

abstract interface class NativeTunConnection {
  Future<NativeTunPreview> previewTun(
    int mtu, {
    String? physicalDns,
    String physicalInterface = '',
    CaptureMode captureMode = CaptureMode.campus,
  });
  Future<NativeTunSnapshot> startTun({
    required int mtu,
    required String physicalInterface,
    String? physicalDns,
    CaptureMode captureMode = CaptureMode.campus,
  });
  Future<NativeTunSnapshot> stopTun();
}
