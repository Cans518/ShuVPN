import 'dart:async';

import 'package:flutter/foundation.dart';

import 'desktop_port.dart';

class DesktopLifecycle extends ChangeNotifier {
  DesktopLifecycle({
    required this.port,
    required this.connect,
    required this.disconnect,
    required this.shutdown,
    required this.selectProtocol,
  });
  final DesktopPort port;
  final Future<void> Function() connect;
  final Future<void> Function() disconnect;
  final Future<void> Function() shutdown;
  final void Function(String) selectProtocol;
  bool trayReady = false;
  bool quitting = false;
  String? error;
  bool _disposed = false;
  TraySnapshot? _snapshot;
  TraySnapshot? _published;
  TraySnapshot? _attempted;
  bool _menuNeedsRefresh = false;
  Future<void>? _quitTask;
  Future<void>? _menuTask;
  Future<void>? _startTask;

  Future<void> start() => _startTask ??= _start();
  Future<void> _start() async {
    try {
      port.listenClose(windowClose);
      await port.interceptClose(true);
      if (_disposed) {
        await port.interceptClose(false);
        return;
      }
      if (await port.hasTrayHost()) {
        final created = await port.createTray(
          (command) => unawaited(dispatch(command)),
          (id) => protocol(id),
        );
        if (_disposed || quitting) {
          if (created) await port.destroyTray();
          return;
        }
        trayReady = created;
      }
      if (!trayReady) error = '当前没有可用托盘，关闭窗口会断开并退出。';
      _notify();
      await _publish();
    } on Object catch (failure) {
      trayReady = false;
      error = '托盘不可用，关闭窗口会断开并退出：$failure';
      _notify();
    }
  }

  void update(TraySnapshot snapshot) {
    _snapshot = snapshot;
    unawaited(_publish());
  }

  Future<void> _publish() {
    if (_menuTask case final pending?) return pending;
    final task = _flushMenu();
    _menuTask = task;
    return task.whenComplete(() {
      _menuTask = null;
      final latest = _snapshot;
      if (!_disposed &&
          !quitting &&
          trayReady &&
          latest != null &&
          (_menuNeedsRefresh || !(_attempted?.sameAs(latest) ?? false))) {
        unawaited(_publish());
      }
    });
  }

  Future<void> _flushMenu() async {
    while (!_disposed && !quitting && trayReady) {
      final next = _snapshot;
      if (next == null ||
          (!_menuNeedsRefresh && (_published?.sameAs(next) ?? false))) {
        return;
      }
      _menuNeedsRefresh = false;
      _attempted = next;
      try {
        await port.updateTray(next);
        _published = next;
      } on Object catch (failure) {
        error = '更新托盘状态失败：$failure';
        _notify();
        return;
      }
    }
  }

  Future<void> windowClose() async {
    if (_disposed || quitting) return;
    try {
      if (trayReady && await port.hasTrayHost()) {
        if (!_disposed && !quitting) await port.hide();
      } else {
        await quit();
      }
    } on Object catch (failure) {
      error = '无法隐藏窗口：$failure';
      await port.show();
      _notify();
    }
  }

  Future<void> dispatch(DesktopCommand command) async {
    if (_disposed || quitting) return;
    try {
      switch (command) {
        case DesktopCommand.show:
          await port.show();
        case DesktopCommand.connect:
          if (_snapshot?.canConnect != true) return;
          await port.show();
          if (!_disposed && !quitting) await connect();
        case DesktopCommand.disconnect:
          if (_snapshot?.canDisconnect == true) await disconnect();
        case DesktopCommand.quit:
          await quit();
      }
    } on Object catch (failure) {
      error = '桌面操作失败：$failure';
      if (!_disposed) await port.show();
      _notify();
    }
  }

  void protocol(String id) {
    if (_disposed || quitting) return;
    // Native check items can toggle before their callback, even for a no-op.
    _menuNeedsRefresh = true;
    final allowed =
        _snapshot?.protocols.any(
          (choice) => choice.id == id && choice.enabled,
        ) ??
        false;
    try {
      if (allowed) selectProtocol(id);
    } finally {
      unawaited(_publish());
    }
  }

  Future<void> quit() {
    if (_quitTask case final pending?) return pending;
    final task = _quit();
    _quitTask = task;
    return task.whenComplete(() {
      if (!quitting) _quitTask = null;
    });
  }

  Future<void> _quit() async {
    quitting = true;
    _notify();
    try {
      await shutdown();
      await _startTask;
      await _menuTask;
      await port.destroyTray();
      trayReady = false;
      await port.interceptClose(false);
      await port.destroyWindow();
    } on Object catch (failure) {
      quitting = false;
      _quitTask = null;
      error = '退出清理失败，请重试：$failure';
      await port.interceptClose(true);
      await port.show();
      _notify();
    }
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    port.listenClose(null);
    unawaited(_release());
    super.dispose();
  }

  Future<void> _release() async {
    try {
      await _startTask;
      await _menuTask;
      await port.interceptClose(false);
      await port.destroyTray();
    } on Object catch (failure) {
      // The native window may already have destroyed its platform channels.
      debugPrint('Desktop resource cleanup: $failure');
    }
  }
}
