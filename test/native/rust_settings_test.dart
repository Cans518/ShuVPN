import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart' show ThemeMode;
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/rust/generated/api/settings.dart' as native;
import 'package:shuvpn/core/rust/generated/settings/model.dart' as model;
import 'package:shuvpn/core/rust/rust_core.dart';
import 'package:shuvpn/core/rust/rust_settings_store.dart';
import 'package:shuvpn/core/settings/settings_store.dart';
import 'package:shuvpn/core/connection/protocol.dart';
import 'package:shuvpn/core/logging/shu_log.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test('fresh native settings supply the complete desktop defaults', () async {
    SharedPreferences.setMockInitialValues({});
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-settings-defaults-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final settings = await RustSettingsStore.open(
      directory.path,
      await SharedPreferences.getInstance(),
    );
    addTearDown(settings.dispose);
    expect(settings.themeMode, ThemeMode.system);
    expect(settings.welcomeCompleted, isFalse);
    expect(settings.socksPort, 2233);
    expect(settings.httpPort, 3322);
    expect(settings.socksProxyEnabled, isFalse);
    expect(settings.httpProxyEnabled, isFalse);
    expect(settings.systemProxyEnabled, isFalse);
    expect(settings.vpnEnabled, isFalse);
    expect(settings.vpnCaptureMode, CaptureMode.campus);
    expect(settings.vpnMtu, 1400);
    expect(settings.vpnDns, isEmpty);
    expect(settings.vpnTcpWindowScaling, isTrue);
    expect(settings.socksListen.address, '127.0.0.1');
    expect(settings.httpListen.address, '127.0.0.1');
    expect(settings.timeout, const Duration(seconds: 60));
    expect(settings.server, 'atrust.shu.edu.cn');
    expect(settings.loginDomain, 'customOAuth76881');
    expect(settings.logLevel, ShuLogLevel.info);
    expect(settings.enabledProtocols, [ShuProtocol.atrust]);
  });

  test(
    'native listener, port and capture settings survive a real reopen',
    () async {
      SharedPreferences.setMockInitialValues({});
      final preferences = await SharedPreferences.getInstance();
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-listen-native-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final settings = await RustSettingsStore.open(
        directory.path,
        preferences,
      );
      for (final address in [
        '127.0.0.1',
        '192.168.99.144',
        '0.0.0.0',
        '::1',
        '::',
        'fe80::1',
      ]) {
        expect(settings.parseListen(address)?.address, address);
      }
      expect(settings.parseListen('  10.0.0.2  ')?.address, '10.0.0.2');
      expect(
        settings.parseListen('2001:0db8:0:0:0:0:0:1')?.address,
        '2001:db8::1',
      );
      for (final address in [
        '',
        '   ',
        'localhost',
        'example.com',
        '999.1.1.1',
        '127.0.0',
        '127.0.0.1/24',
      ]) {
        expect(settings.parseListen(address), isNull);
      }
      await settings.save(SettingKey.httpListen, '2001:0db8:0:0:0:0:0:1');
      await settings.save(SettingKey.httpPort, 65535);
      await settings.save(SettingKey.socksPort, 65535);
      await settings.save(SettingKey.vpnCaptureMode, 'full');
      expect(settings.vpnCaptureMode, CaptureMode.full);
      await expectLater(
        settings.save(SettingKey.vpnCaptureMode, 'unknown'),
        throwsA(isA<ShuSettingsException>()),
      );
      expect(settings.vpnCaptureMode, CaptureMode.full);
      settings.dispose();
      final restored = await RustSettingsStore.open(
        directory.path,
        preferences,
      );
      addTearDown(restored.dispose);
      expect(restored.httpListen.address, '2001:db8::1');
      expect(restored.httpPort, 65535);
      expect(restored.socksPort, 65535);
      expect(restored.vpnCaptureMode, CaptureMode.full);
      await restored.save(SettingKey.vpnCaptureMode, 'campus');
      expect(restored.vpnCaptureMode, CaptureMode.campus);
    },
  );

  test(
    'native settings owns migration and preserves nonsettings preferences',
    () async {
      SharedPreferences.setMockInitialValues({
        'settings.schema.version': 2,
        'settings.socksPort': 1080,
        'settings.socksListen': 'any',
        'settings.proxyDirectFallback': true,
        'auth.session': '{"cookies":[]}',
        'tofu_sha256_example': 'fingerprint',
        'atrust_device_id': 'device',
      });
      final preferences = await SharedPreferences.getInstance();
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-settings-ffi-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final settings = await RustSettingsStore.open(
        directory.path,
        preferences,
      );
      expect(settings.welcomeCompleted, isTrue);
      expect(settings.socksPort, 1080);
      expect(settings.socksListen.address, '0.0.0.0');
      expect(
        preferences.getKeys().where((key) => key.startsWith('settings.')),
        isEmpty,
      );
      expect(preferences.getString('auth.session'), '{"cookies":[]}');
      expect(preferences.getString('tofu_sha256_example'), 'fingerprint');
      expect(preferences.getString('atrust_device_id'), 'device');
      await settings.save(SettingKey.server, 'fixture.example');
      await settings.save(SettingKey.socksPort, 0);
      await settings.save(SettingKey.loginDomain, 'fixture-domain');
      await settings.save(SettingKey.timeoutSeconds, 120);
      await settings.save(SettingKey.atrustEnabled, false);
      settings.dispose();
      final restored = await RustSettingsStore.open(
        directory.path,
        preferences,
      );
      addTearDown(restored.dispose);
      expect(restored.server, 'fixture.example');
      expect(restored.socksPort, 0);
      expect(preferences.containsKey('settings.server'), isFalse);
      await restored.restoreConnectionDefaults();
      expect(restored.server, 'atrust.shu.edu.cn');
      expect(restored.socksPort, 0);
      expect(restored.welcomeCompleted, isTrue);
      expect(restored.loginDomain, 'customOAuth76881');
      expect(restored.timeout, const Duration(seconds: 60));
      expect(restored.enabledProtocols, [ShuProtocol.atrust]);
      expect(preferences.getString('auth.session'), '{"cookies":[]}');
      expect(preferences.getString('tofu_sha256_example'), 'fingerprint');
      expect(preferences.getString('atrust_device_id'), 'device');
    },
  );

  test(
    'native validation rejects before publishing and sync writers are blocked',
    () async {
      SharedPreferences.setMockInitialValues({});
      final preferences = await SharedPreferences.getInstance();
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-settings-validation-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final settings = await RustSettingsStore.open(
        directory.path,
        preferences,
      );
      addTearDown(settings.dispose);
      var notifications = 0;
      settings.addListener(() => notifications++);
      await expectLater(
        settings.save(SettingKey.httpPort, -1),
        throwsA(isA<ShuSettingsException>()),
      );
      expect(settings.httpPort, 3322);
      expect(notifications, 0);
      for (final input in [-1, 65536, 'invalid', '']) {
        expect(
          settings.validationMessage(SettingKey.httpPort, input),
          isNotNull,
        );
      }
      for (final input in [0, 65535]) {
        expect(settings.validationMessage(SettingKey.httpPort, input), isNull);
      }
      expect(() => settings.server = 'legacy-writer.example', throwsStateError);
      expect(preferences.containsKey('settings.server'), isFalse);
      expect(
        settings.validationMessage(SettingKey.vpnDns, '999.1.2.3'),
        isNotNull,
      );
      expect(settings.parseListen(' 2001:db8::1 ')?.address, '2001:db8::1');
      await Future.wait([
        settings.save(SettingKey.server, 'parallel.example'),
        settings.save(SettingKey.socksPort, 4567),
      ]);
      expect(settings.server, 'parallel.example');
      expect(settings.socksPort, 4567);
      expect(notifications, greaterThan(0));
    },
  );

  test(
    'failed adoption keeps legacy keys and released locks allow retry',
    () async {
      SharedPreferences.setMockInitialValues({
        'settings.schema.version': 2,
        'settings.socksPort': -1,
      });
      final preferences = await SharedPreferences.getInstance();
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-settings-failed-',
      );
      addTearDown(() => directory.delete(recursive: true));
      await expectLater(
        RustSettingsStore.open(directory.path, preferences),
        throwsA(isA<ShuSettingsException>()),
      );
      expect(preferences.getInt('settings.socksPort'), -1);
      await preferences.setInt('settings.socksPort', 3456);
      final retried = await RustSettingsStore.open(directory.path, preferences);
      addTearDown(retried.dispose);
      expect(retried.socksPort, 3456);
    },
  );

  test(
    'actual settings owner restores data without reimport and closes',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-settings-owner-',
      );
      addTearDown(() => directory.delete(recursive: true));
      const defaults = model.Defaults(debugLogging: false);
      final owner = await native.SettingsSession.open(
        directory: directory.path,
        legacyJson: '{}',
        defaults: defaults,
      );
      await expectLater(
        native.SettingsSession.open(
          directory: directory.path,
          legacyJson: '{}',
          defaults: defaults,
        ),
        throwsA(isA<model.SettingsFailure>()),
      );
      final saved = await owner.apply(
        edits: [
          model.Edit(
            key: SettingKey.systemProxyEnabled,
            valueJson: jsonEncode(true),
          ),
        ],
      );
      expect(saved.httpProxyEnabled, isTrue);
      expect(saved.systemProxyEnabled, isTrue);
      owner.close();
      owner.close();
      await expectLater(
        owner.apply(edits: []),
        throwsA(isA<model.SettingsFailure>()),
      );
      owner.dispose();
      final reopened = await native.SettingsSession.open(
        directory: directory.path,
        legacyJson: 'malformed ignored after migration',
        defaults: defaults,
      );
      addTearDown(() {
        reopened.close();
        reopened.dispose();
      });
      expect(reopened.snapshot().systemProxyEnabled, isTrue);
    },
  );
}
