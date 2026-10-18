import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../connection/protocol.dart';
import '../connection/proxy_listen.dart';
import '../logging/shu_log.dart';
import '../settings/settings_store.dart';
import 'generated/api/settings.dart' as native;
import 'generated/settings/model.dart' as model;

final class RustSettingsStore extends SettingsStore {
  /// Takes ownership of an already opened session.
  @visibleForTesting
  RustSettingsStore.fromNative(this._owner) : _snapshot = _owner.snapshot() {
    _syncLogging();
  }

  static Future<RustSettingsStore> open(
    String directory,
    SharedPreferences preferences,
  ) async {
    final keys = preferences
        .getKeys()
        .where((key) => key.startsWith('settings.'))
        .toList();
    final native.SettingsSession owner;
    try {
      owner = await native.SettingsSession.open(
        directory: directory,
        legacyJson: jsonEncode({
          for (final key in keys) key: preferences.get(key),
        }),
        defaults: model.Defaults(debugLogging: kDebugMode),
      );
    } on model.SettingsFailure catch (error) {
      throw ShuSettingsException(error.message);
    }
    try {
      for (final key in keys) {
        if (!await preferences.remove(key)) {
          throw const ShuSettingsException('设置已迁移，旧偏好清理失败，请重试启动');
        }
      }
      return RustSettingsStore.fromNative(owner);
    } on Object {
      owner.close();
      owner.dispose();
      rethrow;
    }
  }

  final native.SettingsSession _owner;
  model.SettingsSnapshot _snapshot;
  bool _disposed = false;
  Future<void> _commands = Future<void>.value();

  Future<void> _call(Future<model.SettingsSnapshot> Function() command) {
    final result = _commands.then((_) => _execute(command));
    // The caller retains the error; completion releases the next UI command.
    _commands = result.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return result;
  }

  Future<void> _execute(
    Future<model.SettingsSnapshot> Function() command,
  ) async {
    if (_disposed) throw const ShuSettingsException('设置服务已经关闭');
    try {
      final snapshot = await command();
      if (_disposed) throw const ShuSettingsException('设置服务已经关闭');
      if (snapshot.revision >= _snapshot.revision) {
        _snapshot = snapshot;
        _syncLogging();
        notifyListeners();
      }
    } on model.SettingsFailure catch (error) {
      throw ShuSettingsException(error.message);
    }
  }

  @override
  Future<void> save(SettingKey key, Object value) => _call(
    () => _owner.apply(
      edits: [model.Edit(key: key, valueJson: jsonEncode(value))],
    ),
  );

  @override
  Future<void> restoreConnectionDefaults() =>
      _call(_owner.restoreConnectionDefaults);

  @override
  String? validationMessage(SettingKey key, Object value) {
    try {
      native.validateSetting(key: key, valueJson: jsonEncode(value));
      return null;
    } on model.SettingsFailure catch (error) {
      return error.message;
    }
  }

  @override
  ShuProxyListen? parseListen(String raw) {
    try {
      final encoded = native.validateSetting(
        key: SettingKey.socksListen,
        valueJson: jsonEncode(raw),
      );
      return ShuProxyListen.fromBackend(jsonDecode(encoded) as String);
    } on model.SettingsFailure {
      return null;
    }
  }

  void _syncLogging() =>
      ShuLog.instance.configure(enabled: logEnabled, level: logLevel);

  @override
  ThemeMode get themeMode => switch (_snapshot.themeMode) {
    'light' => ThemeMode.light,
    'dark' => ThemeMode.dark,
    _ => ThemeMode.system,
  };
  @override
  bool get welcomeCompleted => _snapshot.welcomeCompleted;
  @override
  int get socksPort => _snapshot.socksPort;
  @override
  ShuProxyListen get socksListen =>
      ShuProxyListen.fromBackend(_snapshot.socksListen);
  @override
  bool get socksProxyEnabled => _snapshot.socksProxyEnabled;
  @override
  bool get httpProxyEnabled => _snapshot.httpProxyEnabled;
  @override
  bool get systemProxyEnabled => _snapshot.systemProxyEnabled;
  @override
  ShuProxyListen get httpListen =>
      ShuProxyListen.fromBackend(_snapshot.httpListen);
  @override
  int get httpPort => _snapshot.httpPort;
  @override
  bool get vpnEnabled => _snapshot.vpnEnabled;
  @override
  CaptureMode get vpnCaptureMode => _snapshot.vpnCaptureMode;
  @override
  int get vpnMtu => _snapshot.vpnMtu;
  @override
  String get vpnDns => _snapshot.vpnDns;
  @override
  bool get vpnTcpWindowScaling => _snapshot.vpnTcpWindowScaling;
  @override
  Duration get timeout => Duration(seconds: _snapshot.timeoutSeconds);
  @override
  String get server => _snapshot.server;
  @override
  String get loginDomain => _snapshot.loginDomain;
  @override
  bool get logEnabled => _snapshot.logEnabled;
  @override
  ShuLogLevel get logLevel => ShuLogLevel.values.firstWhere(
    (level) => level.label == _snapshot.logLevel,
  );
  @override
  bool isProtocolEnabled(ShuProtocol protocol) =>
      _snapshot.enabledProtocols.contains(protocol.id);

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _owner.close();
    _owner.dispose();
    super.dispose();
  }
}
