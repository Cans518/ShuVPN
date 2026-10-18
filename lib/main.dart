// ShuVPN — Sangfor aTrust RVPN client for Windows and Linux.
//
// Architecture: `lib/app` owns the theme, the router and the provider scope;
// `lib/core` owns the state that outlives a screen (connection, account,
// settings); `lib/features` owns the screens; `lib/shell` owns the floating
// dock that ties the three top-level destinations together.
//
// The Rust core owns the desktop TUN and HTTP/SOCKS5 proxy data planes.
//
// Error handling: every failure path throws a typed `SangforException`. Nothing
// is swallowed — a cancelled prompt, an unreachable server and a certificate
// mismatch all surface with a distinct `SangforErrorCode` in the UI.

import 'package:flutter/material.dart';

import 'dart:io';

import 'app/bootstrap.dart';
import 'core/desktop/plugin_desktop_port.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // 这里**故意不 await**：设置要等读完，而等待期间屏幕上应该有东西。
  // 读取与模式迁移都挪进了 `ShuVpnBootstrap` —— 它加载时显示启动图，加载完
  // 才建出真正的应用。顺序上的要求（迁移必须在任何设置被读取之前跑完）没有
  // 变，变的只是等待发生在哪一帧。
  final desktop = Platform.isWindows || Platform.isLinux
      ? PluginDesktopPort()
      : null;
  await desktop?.initializeWindow();
  runApp(ShuVpnBootstrap(desktopPort: desktop));
}
