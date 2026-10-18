import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:system_tray/system_tray.dart';
import 'package:window_manager/window_manager.dart';

import '../logging/shu_log.dart';
import '../connection/protocol.dart';
import 'desktop_port.dart';

class PluginDesktopPort with WindowListener implements DesktopPort {
  final SystemTray _tray = SystemTray();
  Future<void> Function()? _close;
  bool _trayCreated = false;
  Menu? _menu;
  late MenuItemLabel _status;
  late MenuItemLabel _connect;
  late MenuItemLabel _disconnect;
  final Map<String, MenuItemCheckbox> _protocols = {};
  void Function(DesktopCommand)? _command;
  void Function(String)? _protocol;

  @override
  Future<void> initializeWindow() async {
    await windowManager.ensureInitialized();
    windowManager.addListener(this);
    await windowManager.waitUntilReadyToShow(
      const WindowOptions(
        title: 'ShuVPN',
        size: Size(1024, 760),
        minimumSize: Size(640, 560),
        center: true,
        titleBarStyle: TitleBarStyle.hidden,
        skipTaskbar: false,
      ),
    );
  }

  @override
  void listenClose(Future<void> Function()? callback) {
    _close = callback;
  }

  @override
  void onWindowClose() {
    final callback = _close;
    if (callback != null) unawaited(callback());
  }

  @override
  Future<void> interceptClose(bool enabled) =>
      windowManager.setPreventClose(enabled);
  @override
  Future<bool> hasTrayHost() async {
    if (Platform.isWindows) return true;
    if (!Platform.isLinux) return false;
    try {
      final result = await Process.run('gdbus', [
        'call',
        '--session',
        '--dest',
        'org.freedesktop.DBus',
        '--object-path',
        '/org/freedesktop/DBus',
        '--method',
        'org.freedesktop.DBus.NameHasOwner',
        'org.kde.StatusNotifierWatcher',
      ]).timeout(const Duration(seconds: 2));
      return result.exitCode == 0 &&
          result.stdout.toString().contains('(true,');
    } on Object {
      return false;
    }
  }

  @override
  Future<bool> createTray(
    void Function(DesktopCommand) command,
    void Function(String) protocol,
  ) async {
    _command = command;
    _protocol = protocol;
    _trayCreated = await _tray.initSystemTray(
      title: 'ShuVPN',
      toolTip: 'ShuVPN',
      iconPath: Platform.isWindows
          ? 'assets/images/tray_icon.ico'
          : 'assets/images/icon_clear_blue.png',
    );
    if (_trayCreated) {
      if (_menu case final menu?) await _tray.setContextMenu(menu);
      _tray.registerSystemTrayEventHandler((event) {
        if (event == kSystemTrayEventClick) {
          _command?.call(DesktopCommand.show);
        } else if (event == kSystemTrayEventRightClick) {
          unawaited(
            _tray.popUpContextMenu().catchError((Object failure) {
              ShuLog.w('desktop', '无法打开托盘菜单：$failure');
            }),
          );
        }
      });
    }
    return _trayCreated;
  }

  @override
  Future<void> updateTray(TraySnapshot snapshot) async {
    if (!_trayCreated) return;
    if (snapshot.protocols.any(
      (choice) => ShuProtocol.fromId(choice.id) == null,
    )) {
      throw ArgumentError('托盘收到未知协议');
    }
    if (_menu == null) {
      _status = MenuItemLabel(label: snapshot.status, enabled: false);
      _connect = MenuItemLabel(
        label: '连接',
        onClicked: (_) => _command?.call(DesktopCommand.connect),
      );
      _disconnect = MenuItemLabel(
        label: '断开',
        onClicked: (_) => _command?.call(DesktopCommand.disconnect),
      );
      for (final known in ShuProtocol.values) {
        _protocols[known.id] = MenuItemCheckbox(
          label: known.label,
          checked: false,
          enabled: false,
          onClicked: (_) => _protocol?.call(known.id),
        );
      }
      final menu = Menu();
      final built = await menu.buildFrom([
        _status,
        MenuSeparator(),
        MenuItemLabel(
          label: '显示窗口',
          onClicked: (_) => _command?.call(DesktopCommand.show),
        ),
        _connect,
        _disconnect,
        SubMenu(label: '协议', children: _protocols.values.toList()),
        MenuSeparator(),
        MenuItemLabel(
          label: '退出',
          onClicked: (_) => _command?.call(DesktopCommand.quit),
        ),
      ]);
      if (!built) throw StateError('无法创建托盘菜单');
      await _tray.setContextMenu(menu);
      _menu = menu;
    }
    await _status.setLabel(snapshot.status);
    await _connect.setEnable(snapshot.canConnect);
    await _disconnect.setEnable(snapshot.canDisconnect);
    for (final known in ShuProtocol.values) {
      final choice = snapshot.protocols
          .where((choice) => choice.id == known.id)
          .firstOrNull;
      final item = _protocols[known.id]!;
      await item.setLabel(choice?.label ?? known.label);
      await item.setEnable(choice?.enabled ?? false);
      await item.setCheck(choice?.selected ?? false);
    }
    await _tray.setToolTip('ShuVPN · ${snapshot.status}');
  }

  @override
  Future<void> destroyTray() async {
    _command = null;
    _protocol = null;
    if (_trayCreated) {
      await _tray.destroy();
      _trayCreated = false;
    }
  }

  @override
  Future<void> show() async {
    if (await windowManager.isMinimized()) await windowManager.restore();
    await windowManager.setSkipTaskbar(false);
    await windowManager.show();
    await windowManager.focus();
  }

  @override
  Future<void> hide() => windowManager.hide();
  @override
  Future<void> destroyWindow() => windowManager.destroy();
  @override
  Future<void> minimize() => windowManager.minimize();
  @override
  Future<void> toggleMaximize() async {
    if (await windowManager.isMaximized()) {
      await windowManager.unmaximize();
    } else {
      await windowManager.maximize();
    }
  }

  @override
  Future<void> drag() => windowManager.startDragging();
}
