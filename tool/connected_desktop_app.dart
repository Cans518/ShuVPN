// Developer-only entrypoint. The normal application never imports this file.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/app/app.dart';
import 'package:shuvpn/core/auth/account_authentication.dart';
import 'package:shuvpn/core/connection/native_connection.dart';
import 'package:shuvpn/core/connection/native_tun.dart';
import 'package:shuvpn/core/desktop/plugin_desktop_port.dart';
import 'package:shuvpn/core/desktop/desktop_port.dart';
import 'package:shuvpn/core/rust/generated/api/account.dart' as account;
import 'package:shuvpn/core/rust/generated/api/desktop_connection.dart'
    as owner;
import 'package:shuvpn/core/rust/generated/api/settings.dart' as settings;
import 'package:shuvpn/core/rust/generated/api/vpn.dart' as vpn;
import 'package:shuvpn/core/rust/generated/connection/owner.dart'
    as owner_model;
import 'package:shuvpn/core/rust/generated/settings/model.dart' as schema;
import 'package:shuvpn/core/rust/routing_service.dart';
import 'package:shuvpn/core/rust/rust_account_center.dart';
import 'package:shuvpn/core/rust/rust_connection.dart';
import 'package:shuvpn/core/rust/rust_core.dart';
import 'package:shuvpn/core/rust/rust_desktop_tun.dart';
import 'package:shuvpn/core/rust/rust_settings_store.dart';

import 'connected_desktop_observation.dart';

Future<void> main() async {
  try {
    await _run();
  } on Object catch (error, stack) {
    stderr.writeln('$error\n$stack');
    exit(1);
  }
}

Future<void> _run() async {
  WidgetsFlutterBinding.ensureInitialized();
  final token = Platform.environment['SHUVPN_GUI_ACCEPTANCE_TOKEN'];
  final reportPath = Platform.environment['SHUVPN_CONNECTED_GUI_REPORT'];
  final fixturePath = Platform.environment['SHUVPN_CONNECTED_GUI_FIXTURE'];
  if ((!Platform.isLinux && !Platform.isWindows) ||
      token == null ||
      reportPath == null ||
      fixturePath == null ||
      Platform.environment['SHUVPN_CONNECTED_GUI_TOKEN'] != token) {
    throw StateError(
      'Connected fixture requires its private desktop GUI harness',
    );
  }
  var network = 'windows-loopback-fixture';
  var uid = -1;
  final Directory support;
  if (Platform.isLinux) {
    final status = Map.fromEntries(
      (await File('/proc/self/status').readAsLines())
          .where((s) => s.contains(':'))
          .map(
            (s) => MapEntry(
              s.split(':').first,
              s.substring(s.indexOf(':') + 1).trim(),
            ),
          ),
    );
    final ids = status['Uid']!.split(RegExp(r'\s+')).map(int.parse).toList();
    uid = ids.first;
    network = await Link('/proc/self/ns/net').target();
    if (ids.any((id) => id == 0 || id != ids.first) ||
        int.parse(status['CapEff']!, radix: 16) != 0 ||
        network == Platform.environment['SHUVPN_CONNECTED_PARENT_NET'] ||
        Platform.environment['SHUVPN_CONNECTED_PARENT_NET'] == null) {
      throw StateError(
        'Run as an ordinary user inside a private network namespace',
      );
    }
    support = await getApplicationSupportDirectory();
    final privateData = Platform.environment['XDG_DATA_HOME'];
    if (privateData == null || !support.path.startsWith('$privateData/')) {
      throw StateError(
        'Application support must belong to the private profile',
      );
    }
  } else {
    if (!RegExp(r'^[0-9a-f]{32}$').hasMatch(token) ||
        Platform.environment['SHUVPN_CONNECTED_WINDOWS_TOKEN'] != token) {
      throw StateError('Windows connected fixture requires its explicit token');
    }
    final report = Directory(reportPath);
    final boundary = await Directory('build/desktop').resolveSymbolicLinks();
    final canonical = await report.resolveSymbolicLinks();
    if (!canonical.startsWith('$boundary${Platform.pathSeparator}') ||
        canonical != report.absolute.path ||
        !File(fixturePath).isAbsolute ||
        !await File(fixturePath).exists()) {
      throw StateError(
        'Windows fixture paths must belong to the private harness',
      );
    }
    support = Directory('$canonical/profile');
    if (await support.exists()) {
      throw StateError('Windows connected profile must be new');
    }
    await support.create();
  }
  final port = Platform.isWindows
      ? _WindowsConnectedPort(token, Directory(reportPath))
      : PluginDesktopPort();
  await port.initializeWindow();
  await RustCore.initialize();
  final seed = await settings.SettingsSession.open(
    directory: '${support.path}/settings',
    legacyJson: '{}',
    defaults: const schema.Defaults(debugLogging: false),
  );
  await seed.apply(
    edits: [
      for (final edit in <schema.SettingKey, Object>{
        schema.SettingKey.welcomeCompleted: true,
        schema.SettingKey.server: 'https://fixture.invalid',
        schema.SettingKey.httpProxyEnabled: true,
        schema.SettingKey.socksProxyEnabled: true,
        schema.SettingKey.httpPort: 18443,
        schema.SettingKey.socksPort: 11080,
        schema.SettingKey.systemProxyEnabled: false,
        schema.SettingKey.vpnEnabled: false,
      }.entries)
        schema.Edit(key: edit.key, valueJson: jsonEncode(edit.value)),
    ],
  );
  final RustSettingsStore viewSettings;
  if (Platform.isWindows) {
    // Avoid both KnownFolder and legacy preferences migration in this harness.
    // ignore: invalid_use_of_visible_for_testing_member
    viewSettings = RustSettingsStore.fromNative(seed);
  } else {
    seed.close();
    seed.dispose();
    final preferences = await SharedPreferences.getInstance();
    viewSettings = await RustSettingsStore.open(
      '${support.path}/settings',
      preferences,
    );
  }
  final nativeAccount = await account.AccountSession.open(
    directory: '${support.path}/auth',
    legacyCookies:
        '[{"name":"SHU_OAUTH2","value":"gui-fixture-only","path":"/"}]',
    legacySnapshot: '{"accountId":"25123456","profile":{"name":"本地协议 fixture"},"credentials":{}}',
    legacyDeviceId: 'connected-gui-fixture',
  );
  final accounts = RustAccountCenter.fromNative(nativeAccount);
  if (!accounts.signedIn) {
    throw StateError('Native fixture account was not imported');
  }
  final rules = await RoutingService.open('${support.path}/rules');
  await rules.prepare(
    jsonEncode({
      'automatic_campus': false,
      'rules': [
        'DST-PORT,19444,REJECT',
        'DST-PORT,19445,SHU-VPN',
        'IP-CIDR,127.0.0.0/8,DIRECT,no-resolve',
        'IP-CIDR,10.21.0.0/16,SHU-VPN,no-resolve',
        'IP-CIDR6,2001:db8:21::/64,SHU-VPN,no-resolve',
        'MATCH,REJECT',
      ],
    }),
    '{}',
  );
  await rules.applyPreview();
  final fixture = await Process.start(fixturePath, ['udp']);
  try {
    if (Platform.isWindows) {
      await File('$reportPath/fixture-started.json').writeAsString(
        jsonEncode({
          'token': token,
          'pid': pid,
          'stage': 'fixture-started',
          'fixture_pid': fixture.pid,
        }),
      );
    }
    unawaited(
      fixture.stderr.pipe(File('$reportPath/fixture-$pid.log').openWrite()),
    );
    final first = await fixture.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter())
        .first
        .timeout(const Duration(seconds: 5));
    final plan = jsonDecode(first) as Map<String, dynamic>;
    (plan['routes'] as List<dynamic>).removeWhere(
      (dynamic route) => (route['host'] as String).startsWith('*.'),
    );
    plan['dnsServers'] = <String>[];
    final authority = await owner.DesktopConnectionOwner.open();
    authority.claimFrontend();
    await authority.awaitFrontend();
    final backend = _FixtureBackend(
      authority,
      fixture,
      jsonEncode(plan),
      rules,
      Directory(reportPath),
      token,
      network,
      uid,
      support.path,
    );
    await backend.capture('initialized');
    runApp(
      ShuVpnApp(
        settings: viewSettings,
        accountCenter: accounts,
        nativeConnectionBackend: backend,
        routingService: rules,
        desktopTunPlatform: const RustDesktopTunPlatform(),
        desktopPort: port,
      ),
    );
    await WidgetsBinding.instance.endOfFrame;
    await port.show();
  } on Object {
    await fixture.stdin.close();
    try {
      await fixture.exitCode.timeout(const Duration(seconds: 5));
    } on TimeoutException {
      fixture.kill();
      await fixture.exitCode.timeout(const Duration(seconds: 5));
    }
    rethrow;
  }
}

// Adds an identity label and records callbacks without replacing their behavior.
final class _WindowsConnectedPort extends PluginDesktopPort {
  _WindowsConnectedPort(this.token, this.report);
  final String token;
  final Directory report;
  Timer? _watchdog, _abort;
  void Function(DesktopCommand)? _command;
  int _sequence = 0;
  Future<void> _events = Future.value();

  @override
  Future<bool> createTray(
    void Function(DesktopCommand) command,
    void Function(String) protocol,
  ) async {
    _command = command;
    final created = await super.createTray((value) {
      final sequence = _sequence++;
      _events = _events
          .then(
            (_) => File('${report.path}/command-$sequence.json').writeAsString(
              jsonEncode({
                'token': token,
                'pid': pid,
                'sequence': sequence,
                'command': value.name,
              }),
            ),
          )
          .then<void>((_) {});
      command(value);
    }, protocol);
    void abort() {
      if (_command == null) return;
      exitCode = 1;
      _watchdog?.cancel();
      _abort?.cancel();
      _command?.call(DesktopCommand.quit);
      _command = null;
    }

    _watchdog = Timer(const Duration(minutes: 3), abort);
    _abort = Timer.periodic(const Duration(milliseconds: 250), (_) {
      final file = File('${report.path}/abort.json');
      if (file.existsSync() &&
          (jsonDecode(file.readAsStringSync()) as Map)['token'] == token) {
        abort();
      }
    });
    return created;
  }

  @override
  Future<void> updateTray(TraySnapshot snapshot) => super.updateTray(
    TraySnapshot(
      status: '${snapshot.status} · $token',
      canConnect: snapshot.canConnect,
      canDisconnect: snapshot.canDisconnect,
      protocols: snapshot.protocols,
    ),
  );

  @override
  Future<void> destroyTray() async {
    _watchdog?.cancel();
    _abort?.cancel();
    _command = null;
    try {
      await _events;
    } finally {
      await super.destroyTray();
    }
  }
}

// Only authentication ingress is replaced: controller commands, native owner,
// services, session views, real FFI and the production widgets remain in use.
final class _FixtureBackend implements NativeConnectionBackend {
  _FixtureBackend(
    this.authority,
    this.fixture,
    this.plan,
    this.rules,
    this.report,
    this.token,
    this.network,
    this.uid,
    this.profile,
  ) {
    _ticker = Timer.periodic(const Duration(milliseconds: 250), (_) {
      unawaited(
        capture('tick').catchError((Object error, StackTrace stack) async {
          await File(
            '${report.path}/fatal-$pid.json',
          ).writeAsString(jsonEncode({'error': '$error', 'stack': '$stack'}));
        }),
      );
    });
  }
  final owner.DesktopConnectionOwner authority;
  final Process fixture;
  final String plan;
  final RoutingService rules;
  final Directory report;
  final String token, network;
  final int uid;
  final String profile;
  Timer? _ticker;
  vpn.VpnSession? _session;
  Map<String, Object?> _ports = {};
  Future<void> _reports = Future.value();
  int connections = 0;
  int? fixtureExit;

  NativeLifecycleSnapshot _map(owner_model.OwnerSnapshot value) =>
      NativeLifecycleSnapshot(
        phase: NativeLifecyclePhase.values.byName(value.phase.name),
        generation: value.generation.toInt(),
        runId: value.runId?.toInt(),
        cleanupPending: value.cleanupPending,
        errorMessage: value.errorMessage,
        errorCode: value.errorCode,
        closed: value.closed,
      );
  @override
  NativeLifecycleSnapshot get lifecycle => _map(authority.snapshot());
  @override
  Stream<NativeLifecycleSnapshot> get lifecycleUpdates =>
      authority.events().map(_map);

  @override
  Future<NativeConnection> connect({
    required AccountAuthentication auth,
    required String server,
    required Duration timeout,
    required NativeProxyPreferences proxies,
    NativeTunStartup? tun,
  }) async {
    if (!auth.hasSession ||
        server != 'https://fixture.invalid' ||
        proxies.systemProxyEnabled ||
        tun?.enabled == true) {
      throw StateError(
        'Fixture ingress is restricted to local proxy GUI acceptance',
      );
    }
    final session = await authority.connectPlan(
      planJson: plan,
      timeoutMs: timeout.inMilliseconds,
      startup: owner.ConnectionStartup(
        proxies: RustConnection.proxyPreferences(proxies),
      ),
    );
    _session = session;
    await session.attachRouting(manager: rules.manager);
    final connection = await RustConnection.create(session);
    final services = await session.startupServices();
    if (services == null ||
        services.failures.isNotEmpty ||
        services.proxies.http == null ||
        services.proxies.socks == null) {
      throw StateError('Real native proxy services did not start');
    }
    _ports = {
      'http': services.proxies.http!.port,
      'socks': services.proxies.socks!.port,
    };
    connections++;
    await capture('connected');
    return connection;
  }

  Future<void> capture(String stage) {
    final task = _reports.then((_) async {
      final current = authority.snapshot();
      final round = connections;
      final ports = _ports;
      final active = await readFixtureSession(_session);
      final value = jsonEncode({
        'token': token,
        'pid': pid,
        'uid': uid,
        'network': network,
        'platform': Platform.operatingSystem,
        'profile': profile,
        'stage': stage,
        'round': round,
        'ports': ports,
        'session': active,
        'fixture_pid': fixture.pid,
        'fixture_exit': fixtureExit,
        'owner': {
          'phase': current.phase.name,
          'closed': current.closed,
          'run_id': current.runId?.toString(),
          'cleanup_pending': current.cleanupPending,
        },
        'scope': 'production widgets/controller/default native library; fixture plan/account, no TUN',
      });
      final pending = File('${report.path}/native-state-$pid.pending');
      await pending.writeAsString(value);
      await pending.rename('${report.path}/native-state.json');
      if (stage != 'tick') {
        await File('${report.path}/native-$pid-$stage-$round.json')
            .writeAsString(value);
      }
    });
    _reports = task.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return task;
  }

  @override
  void cancel() => authority.cancel();
  @override
  Future<void> disconnect() async {
    await authority.disconnect();
    await capture('disconnected');
    _session = null;
  }

  @override
  Future<void> shutdown() async {
    _ticker?.cancel();
    await authority.shutdown();
    await fixture.stdin.close();
    fixtureExit = await fixture.exitCode.timeout(const Duration(seconds: 5));
    if (fixtureExit != 0) throw StateError('Fixture did not exit normally');
    await capture('shutdown');
    _session = null;
  }

  @override
  void dispose() {
    _ticker?.cancel();
    authority.cancel();
    authority.dispose();
  }
}
