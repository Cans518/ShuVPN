import 'package:flutter/material.dart';

import '../connection/protocol.dart';
import '../connection/proxy_listen.dart';
import '../logging/shu_log.dart';
import '../rust/generated/settings/model.dart' show SettingKey;
import '../rust/generated/platform/model.dart' show CaptureMode;

export '../rust/generated/settings/model.dart' show SettingKey;
export '../rust/generated/platform/model.dart' show CaptureMode;

class ShuSettingsException implements Exception {
  const ShuSettingsException(this.message);
  final String message;
  @override
  String toString() => message;
}

/// Presents committed settings and forwards commands to their native owner.
abstract class SettingsStore extends ChangeNotifier {
  ThemeMode get themeMode;
  set themeMode(ThemeMode value) => _rejectSynchronousWrite();
  bool get welcomeCompleted;
  set welcomeCompleted(bool value) => _rejectSynchronousWrite();
  int get socksPort;
  set socksPort(int value) => _rejectSynchronousWrite();
  ShuProxyListen get socksListen;
  set socksListen(ShuProxyListen value) => _rejectSynchronousWrite();
  bool get socksProxyEnabled;
  set socksProxyEnabled(bool value) => _rejectSynchronousWrite();
  bool get httpProxyEnabled;
  set httpProxyEnabled(bool value) => _rejectSynchronousWrite();
  bool get systemProxyEnabled;
  set systemProxyEnabled(bool value) => _rejectSynchronousWrite();
  ShuProxyListen get httpListen;
  set httpListen(ShuProxyListen value) => _rejectSynchronousWrite();
  int get httpPort;
  set httpPort(int value) => _rejectSynchronousWrite();
  bool get vpnEnabled;
  set vpnEnabled(bool value) => _rejectSynchronousWrite();
  CaptureMode get vpnCaptureMode;
  set vpnCaptureMode(CaptureMode value) => _rejectSynchronousWrite();
  int get vpnMtu;
  set vpnMtu(int value) => _rejectSynchronousWrite();
  String get vpnDns;
  set vpnDns(String value) => _rejectSynchronousWrite();
  bool get vpnTcpWindowScaling;
  set vpnTcpWindowScaling(bool value) => _rejectSynchronousWrite();
  Duration get timeout;
  set timeout(Duration value) => _rejectSynchronousWrite();
  String get server;
  set server(String value) => _rejectSynchronousWrite();
  String get loginDomain;
  set loginDomain(String value) => _rejectSynchronousWrite();
  bool get logEnabled;
  set logEnabled(bool value) => _rejectSynchronousWrite();
  ShuLogLevel get logLevel;
  set logLevel(ShuLogLevel value) => _rejectSynchronousWrite();

  Future<void> save(SettingKey key, Object value);
  Future<void> restoreConnectionDefaults();
  String? validationMessage(SettingKey key, Object value);
  ShuProxyListen? parseListen(String raw);
  bool isProtocolEnabled(ShuProtocol protocol);

  List<ShuProtocol> get enabledProtocols => [
    for (final protocol in ShuProtocol.values)
      if (isProtocolEnabled(protocol)) protocol,
  ];
  ShuProtocol? get firstEnabledProtocol {
    for (final protocol in enabledProtocols) {
      return protocol;
    }
    return null;
  }

  Never _rejectSynchronousWrite() =>
      throw StateError('Use save to await native settings commit');
}
