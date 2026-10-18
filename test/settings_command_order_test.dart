import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/rust/generated/api/settings.dart' as native;
import 'package:shuvpn/core/rust/generated/settings/model.dart' as model;
import 'package:shuvpn/core/rust/rust_settings_store.dart';
import 'package:shuvpn/core/settings/settings_store.dart';

model.SettingsSnapshot _snapshot(
  String server,
  int revision, {
  CaptureMode captureMode = CaptureMode.campus,
}) => model.SettingsSnapshot(
  revision: revision,
  schemaVersion: 4,
  themeMode: 'system',
  welcomeCompleted: false,
  socksPort: 2233,
  socksListen: '127.0.0.1',
  socksProxyEnabled: false,
  httpProxyEnabled: false,
  systemProxyEnabled: false,
  httpListen: '127.0.0.1',
  httpPort: 3322,
  vpnEnabled: false,
  vpnMtu: 1400,
  vpnDns: '',
  vpnTcpWindowScaling: true,
  vpnCaptureMode: captureMode,
  timeoutSeconds: 60,
  server: server,
  loginDomain: 'customOAuth76881',
  logEnabled: false,
  logLevel: 'INFO',
  enabledProtocols: ['atrust'],
);

class _DeferredOwner implements native.SettingsSession {
  final requests = <List<model.Edit>>[];
  final operations = <String>[];
  @override
  bool isDisposed = false;
  final replies = [
    Completer<model.SettingsSnapshot>(),
    Completer<model.SettingsSnapshot>(),
  ];
  @override
  model.SettingsSnapshot snapshot() => _snapshot('original.example', 0);
  @override
  Future<model.SettingsSnapshot> apply({required List<model.Edit> edits}) {
    operations.add('save');
    requests.add(edits);
    return replies[requests.length - 1].future;
  }

  @override
  Future<model.SettingsSnapshot> restoreConnectionDefaults() {
    operations.add('restore');
    requests.add([]);
    return replies[requests.length - 1].future;
  }

  @override
  void close() {}
  @override
  void dispose() => isDisposed = true;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('capture mode changes only after the native commit', () async {
    final owner = _DeferredOwner();
    final settings = RustSettingsStore.fromNative(owner);
    addTearDown(settings.dispose);
    expect(settings.vpnCaptureMode, CaptureMode.campus);
    final saving = settings.save(SettingKey.vpnCaptureMode, 'full');
    await Future<void>.delayed(Duration.zero);
    expect(owner.requests.single.single.key, SettingKey.vpnCaptureMode);
    expect(owner.requests.single.single.valueJson, '"full"');
    expect(settings.vpnCaptureMode, CaptureMode.campus);
    owner.replies[0].complete(
      _snapshot('original.example', 1, captureMode: CaptureMode.full),
    );
    await saving;
    expect(settings.vpnCaptureMode, CaptureMode.full);
  });

  for (final restoreFirst in [false, true]) {
    test('save and restore share one command order ($restoreFirst)', () async {
      final owner = _DeferredOwner();
      final settings = RustSettingsStore.fromNative(owner);
      addTearDown(settings.dispose);
      final first = restoreFirst
          ? settings.restoreConnectionDefaults()
          : settings.save(SettingKey.server, 'first.example');
      final second = restoreFirst
          ? settings.save(SettingKey.server, 'last.example')
          : settings.restoreConnectionDefaults();
      await Future<void>.delayed(Duration.zero);
      expect(owner.operations, [restoreFirst ? 'restore' : 'save']);
      expect(settings.server, 'original.example');
      owner.replies[0].complete(_snapshot('intermediate.example', 1));
      await first;
      await Future<void>.delayed(Duration.zero);
      expect(
        owner.operations,
        restoreFirst ? ['restore', 'save'] : ['save', 'restore'],
      );
      owner.replies[1].complete(_snapshot('final.example', 2));
      await second;
      expect(settings.server, 'final.example');
    });
  }

  for (final failFirst in [false, true]) {
    test(
      'settings sends commands in input order after ${failFirst ? 'failure' : 'success'}',
      () async {
        SharedPreferences.setMockInitialValues({});
        final owner = _DeferredOwner();
        final settings = RustSettingsStore.fromNative(owner);
        addTearDown(settings.dispose);
        final first = settings.save(SettingKey.server, 'first.example');
        final firstResult = failFirst
            ? expectLater(first, throwsA(isA<ShuSettingsException>()))
            : first;
        final second = settings.save(SettingKey.server, 'last.example');
        await Future<void>.delayed(Duration.zero);
        final dispatchedBeforeCommit = owner.requests.length;
        expect(settings.server, 'original.example');
        if (failFirst) {
          owner.replies[0].completeError(
            const model.SettingsFailure(code: 'storage', message: 'injected'),
          );
        } else {
          owner.replies[0].complete(_snapshot('first.example', 1));
        }
        await firstResult;
        await Future<void>.delayed(Duration.zero);
        expect(owner.requests.length, 2);
        expect(owner.requests[1].single.valueJson, '"last.example"');
        owner.replies[1].complete(_snapshot('last.example', 2));
        await second;
        expect(
          dispatchedBeforeCommit,
          1,
          reason: 'A later command must not overtake a pending commit',
        );
        expect(settings.server, 'last.example');
      },
    );
  }

  test(
    'disposing the adapter rejects queued commands before native dispatch',
    () async {
      SharedPreferences.setMockInitialValues({});
      final owner = _DeferredOwner();
      final settings = RustSettingsStore.fromNative(owner);
      final first = expectLater(
        settings.save(SettingKey.server, 'first.example'),
        throwsA(isA<ShuSettingsException>()),
      );
      final second = expectLater(
        settings.save(SettingKey.server, 'queued.example'),
        throwsA(isA<ShuSettingsException>()),
      );
      await Future<void>.delayed(Duration.zero);
      settings.dispose();
      owner.replies[0].complete(_snapshot('first.example', 1));
      if (owner.requests.length > 1) {
        owner.replies[1].complete(_snapshot('queued.example', 2));
      }
      await Future.wait([first, second]);
      expect(owner.requests.length, 1);
    },
  );
}
