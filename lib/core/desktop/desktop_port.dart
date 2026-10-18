enum DesktopCommand { show, connect, disconnect, quit }

class TrayProtocol {
  const TrayProtocol({
    required this.id,
    required this.label,
    required this.enabled,
    required this.selected,
  });
  final String id;
  final String label;
  final bool enabled;
  final bool selected;
}

class TraySnapshot {
  const TraySnapshot({
    required this.status,
    required this.canConnect,
    required this.canDisconnect,
    required this.protocols,
  });
  final String status;
  final bool canConnect;
  final bool canDisconnect;
  final List<TrayProtocol> protocols;

  bool sameAs(TraySnapshot other) =>
      status == other.status &&
      canConnect == other.canConnect &&
      canDisconnect == other.canDisconnect &&
      protocols.length == other.protocols.length &&
      List.generate(
        protocols.length,
        (i) =>
            protocols[i].id == other.protocols[i].id &&
            protocols[i].label == other.protocols[i].label &&
            protocols[i].enabled == other.protocols[i].enabled &&
            protocols[i].selected == other.protocols[i].selected,
      ).every((same) => same);
}

abstract interface class DesktopPort {
  Future<void> initializeWindow();
  void listenClose(Future<void> Function()? callback);
  Future<void> interceptClose(bool enabled);
  Future<bool> hasTrayHost();
  Future<bool> createTray(
    void Function(DesktopCommand) command,
    void Function(String) protocol,
  );
  Future<void> updateTray(TraySnapshot snapshot);
  Future<void> destroyTray();
  Future<void> show();
  Future<void> hide();
  Future<void> destroyWindow();
  Future<void> minimize();
  Future<void> toggleMaximize();
  Future<void> drag();
}
