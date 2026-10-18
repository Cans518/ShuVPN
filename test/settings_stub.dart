import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/connection/protocol.dart';
import 'package:shuvpn/core/connection/proxy_listen.dart';
import 'package:shuvpn/core/logging/shu_log.dart';
import 'package:shuvpn/core/settings/settings_store.dart';

/// Mutable view fixtures only; no persistence, migration or production validation.
class StubSettingsStore extends SettingsStore {
  StubSettingsStore([SharedPreferences? fixture]) {
    welcomeCompleted = fixture?.getBool('settings.welcomeCompleted') ?? false;
    _syncLogging();
  }
  static Future<StubSettingsStore> load() async =>
      StubSettingsStore(await SharedPreferences.getInstance());

  @override
  ThemeMode themeMode = ThemeMode.system;
  @override
  bool welcomeCompleted = false;
  @override
  int socksPort = 2233;
  @override
  ShuProxyListen socksListen = ShuProxyListen.loopback;
  @override
  bool socksProxyEnabled = false;
  @override
  bool httpProxyEnabled = false;
  @override
  bool systemProxyEnabled = false;
  @override
  ShuProxyListen httpListen = ShuProxyListen.loopback;
  @override
  int httpPort = 3322;
  @override
  bool vpnEnabled = false;
  @override
  CaptureMode vpnCaptureMode = CaptureMode.campus;
  @override
  int vpnMtu = 1400;
  @override
  String vpnDns = '';
  @override
  bool vpnTcpWindowScaling = true;
  @override
  Duration timeout = const Duration(seconds: 60);
  @override
  String server = 'atrust.shu.edu.cn';
  @override
  String loginDomain = 'customOAuth76881';
  @override
  bool logEnabled = true;
  @override
  ShuLogLevel logLevel = ShuLogLevel.info;
  final protocols = <ShuProtocol, bool>{ShuProtocol.atrust: true};
  String? Function(SettingKey, Object)? validation;
  ShuProxyListen? Function(String)? listenParser;

  void _syncLogging() =>
      ShuLog.instance.configure(enabled: logEnabled, level: logLevel);

  @override
  bool isProtocolEnabled(ShuProtocol protocol) => protocols[protocol] ?? false;
  @override
  String? validationMessage(SettingKey key, Object value) =>
      validation?.call(key, value);
  @override
  ShuProxyListen? parseListen(String raw) {
    final parser = listenParser;
    return parser == null ? ShuProxyListen.fromBackend(raw) : parser(raw);
  }

  @override
  Future<void> save(SettingKey key, Object value) async {
    switch (key) {
      case SettingKey.themeMode:
        themeMode = ThemeMode.values.byName(value as String);
      case SettingKey.welcomeCompleted:
        welcomeCompleted = value as bool;
      case SettingKey.socksPort:
        socksPort = value as int;
      case SettingKey.socksListen:
        socksListen = ShuProxyListen.fromBackend(value as String);
      case SettingKey.socksProxyEnabled:
        socksProxyEnabled = value as bool;
      case SettingKey.httpProxyEnabled:
        httpProxyEnabled = value as bool;
      case SettingKey.systemProxyEnabled:
        systemProxyEnabled = value as bool;
      case SettingKey.httpListen:
        httpListen = ShuProxyListen.fromBackend(value as String);
      case SettingKey.httpPort:
        httpPort = value as int;
      case SettingKey.vpnEnabled:
        vpnEnabled = value as bool;
      case SettingKey.vpnCaptureMode:
        vpnCaptureMode = CaptureMode.values.byName(value as String);
      case SettingKey.vpnMtu:
        vpnMtu = value as int;
      case SettingKey.vpnDns:
        vpnDns = value as String;
      case SettingKey.vpnTcpWindowScaling:
        vpnTcpWindowScaling = value as bool;
      case SettingKey.timeoutSeconds:
        timeout = Duration(seconds: value as int);
      case SettingKey.server:
        server = value as String;
      case SettingKey.loginDomain:
        loginDomain = value as String;
      case SettingKey.logEnabled:
        logEnabled = value as bool;
      case SettingKey.logLevel:
        logLevel = ShuLogLevel.fromName(value as String);
      case SettingKey.atrustEnabled:
        protocols[ShuProtocol.atrust] = value as bool;
      case SettingKey.easyConnectEnabled:
        protocols[ShuProtocol.easyConnect] = value as bool;
      case SettingKey.openVpnEnabled:
        protocols[ShuProtocol.openVpn] = value as bool;
    }
    _syncLogging();
    notifyListeners();
  }

  @override
  Future<void> restoreConnectionDefaults() =>
      throw StateError('test did not configure restoration');
}
