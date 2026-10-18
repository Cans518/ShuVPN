import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/settings.dart';
import 'package:shuvpn/core/rust/generated/api/routing.dart' as routing;
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/generated/settings/model.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

Future<void> main(List<String> args) async {
  if ((args.length != 3 && args.length != 4) ||
      !['read', 'prepare-offline-settings'].contains(args[2]) ||
      (args.length == 4 && args[2] != 'read')) {
    throw ArgumentError(
      'Usage: gui_settings_probe <library> <settings-directory> '
      'read|prepare-offline-settings [synthetic-rule-hosts-json]',
    );
  }
  final ruleHosts = args.length == 4
      ? (jsonDecode(args[3]) as List).cast<String>()
      : const <String>[];
  if (ruleHosts.length > 16 ||
      ruleHosts.any((host) => !host.endsWith('.gui.invalid'))) {
    throw ArgumentError(
      'Rule readback hosts must be bounded synthetic GUI targets',
    );
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(File(args[0]).absolute.path),
  );
  final settings = await SettingsSession.open(
    directory: File(args[1]).absolute.path,
    legacyJson: '{}',
    defaults: const Defaults(debugLogging: false),
  );
  routing.RoutingManager? rules;
  try {
    if (args[2] == 'prepare-offline-settings') {
      await settings.apply(
        edits: [
          const Edit(key: SettingKey.welcomeCompleted, valueJson: 'true'),
        ],
      );
    }
    final snapshot = settings.snapshot();
    rules = await routing.RoutingManager.open(
      directory: Directory(args[1]).absolute.parent.uri
          .resolve('rules')
          .toFilePath(),
    );
    if (args[2] == 'prepare-offline-settings') {
      await rules.apply(
        profile: await routing.profileTemplate(name: 'campus-direct'),
        overrides: jsonEncode({
          'prepend': ['DOMAIN,before.gui.invalid,DIRECT'],
          'append': ['DOMAIN,after.gui.invalid,DIRECT'],
        }),
        expectedVersion: BigInt.zero,
      );
    }
    final currentRules =
        jsonDecode(await rules.current()) as Map<String, dynamic>;
    final tested = jsonDecode(
      await rules.test(
        flowJson: jsonEncode({
          'host': '198.51.100.7',
          'ips': ['198.51.100.7'],
          'port': 443,
          'network': 'tcp',
          'inbound': 'tun',
        }),
        routesJson: '[]',
      ),
    ) as Map<String, dynamic>;
    final overrideActions = <String, Object?>{};
    for (final host in ['before.gui.invalid', 'after.gui.invalid']) {
      final evaluation = jsonDecode(
        await rules.test(
          flowJson: jsonEncode({
            'host': host,
            'port': 443,
            'network': 'tcp',
            'inbound': 'tun',
          }),
          routesJson: '[]',
        ),
      ) as Map<String, dynamic>;
      overrideActions[host] =
          (evaluation['decision'] as Map<String, dynamic>?)?['action'];
    }
    final customDecisions = <String, Object?>{};
    for (final host in ruleHosts) {
      final evaluation = jsonDecode(
        await rules.test(
          flowJson: jsonEncode({
            'host': host,
            'port': 443,
            'network': 'tcp',
            'inbound': 'http',
          }),
          routesJson: '[]',
        ),
      ) as Map<String, dynamic>;
      customDecisions[host] = evaluation['decision'];
    }
    final result = {
      'revision': snapshot.revision,
      'welcome': snapshot.welcomeCompleted,
      'theme': snapshot.themeMode,
      'capture_mode': snapshot.vpnCaptureMode.name,
      'http_port': snapshot.httpPort,
      'http_enabled': snapshot.httpProxyEnabled,
      'socks_enabled': snapshot.socksProxyEnabled,
      'vpn_enabled': snapshot.vpnEnabled,
      'system_proxy_enabled': snapshot.systemProxyEnabled,
      'routing_version': currentRules['version'],
      'routing_name': currentRules['name'],
      'routing_profile': currentRules['profile_document'],
      'routing_overrides': currentRules['overrides_document'],
      'routing_override_actions': overrideActions,
      if (ruleHosts.isNotEmpty) 'routing_custom_decisions': customDecisions,
      'routing_test_action':
          (tested['decision'] as Map<String, dynamic>?)?['action'],
    };
    stdout.writeln('GUI_NATIVE_SETTINGS ${jsonEncode(result)}');
  } finally {
    settings.close();
    settings.dispose();
    rules?.dispose();
    ShuRustLib.dispose();
  }
}
