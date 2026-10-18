import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import 'generated/api/routing.dart' as native;
import 'generated/routing/model.dart' as model;
import 'routing_draft.dart';
import 'rust_core.dart';

class RoutingConfiguration {
  const RoutingConfiguration({
    required this.version,
    required this.name,
    required this.profile,
    required this.overrides,
    this.notices = const [],
    this.warnings = const [],
  });
  final int version;
  final String name;
  final String profile;
  final String overrides;
  final List<String> notices;
  final List<String> warnings;
  factory RoutingConfiguration.decode(String text) {
    final json = jsonDecode(text) as Map<String, Object?>;
    return RoutingConfiguration(
      version: json['version'] as int,
      name: json['name'] as String,
      profile: json['profile_document'] as String,
      overrides: json['overrides_document'] as String,
      notices: (json['notices'] as List<Object?>).cast<String>(),
      warnings: (json['warnings'] as List<Object?>).cast<String>(),
    );
  }
}

final class RoutingService extends ChangeNotifier {
  RoutingService._(this.manager, this._current, this.capabilities) {
    _updates = manager.updates().listen(
      _receiveStatus,
      onError: (Object failure) {
        if (_disposed) return;
        error = '无法接收规则更新：$failure';
        notifyListeners();
      },
    );
  }
  final native.RoutingManager manager;
  final Map<String, Object?> capabilities;
  RoutingConfiguration _current;
  RoutingConfiguration get current => _current;
  RoutingConfiguration? preview;
  native.RoutingPreview? _prepared;
  bool busy = false;
  bool _disposed = false;
  String? error;
  int _previewRevision = 0;
  late final StreamSubscription<String> _updates;
  bool refreshing = false;
  String? refreshError;
  DateTime? lastChecked;

  static Future<RoutingService> open(String directory) async {
    await RustCore.initialize();
    final manager = await native.RoutingManager.open(directory: directory);
    try {
      final status = jsonDecode(await manager.status()) as Map<String, Object?>;
      final service = RoutingService._(
        manager,
        RoutingConfiguration.decode(jsonEncode(status['configuration'])),
        Map.unmodifiable(
          jsonDecode(await native.routingCapabilities())
              as Map<String, Object?>,
        ),
      );
      service._receiveStatus(jsonEncode(status));
      return service;
    } on Object {
      manager.dispose();
      rethrow;
    }
  }

  Future<void> prepare(String profile, String overrides) =>
      _operation(() async {
        invalidatePreview();
        final baseVersion = _current.version;
        final revision = _previewRevision;
        final prepared = await manager.prepare(
          profile: profile,
          overrides: overrides,
          expectedVersion: BigInt.from(baseVersion),
        );
        try {
          final info = RoutingConfiguration.decode(await prepared.info());
          if (_disposed) {
            prepared.dispose();
            return;
          }
          if (_current.version != baseVersion) {
            throw StateError('生效配置已更新，请重新校验并预览');
          }
          if (_previewRevision != revision) {
            throw StateError('配置内容已修改，请重新校验并预览');
          }
          _prepared = prepared;
          preview = info;
        } on Object {
          prepared.dispose();
          rethrow;
        }
      });

  Future<void> applyPreview() => _operation(() async {
    final prepared = _prepared;
    if (prepared == null) throw StateError('请先校验并预览配置');
    final applied = RoutingConfiguration.decode(await prepared.commit());
    if (_disposed) return;
    _adoptConfiguration(applied);
    invalidatePreview();
  });

  Future<void> rollback() => _operation(() async {
    final restored = RoutingConfiguration.decode(
      await manager.rollback(expectedVersion: BigInt.from(_current.version)),
    );
    if (_disposed) return;
    _adoptConfiguration(restored);
    invalidatePreview();
  });

  Future<void> refreshProviders() => _operation(() async {
    final refreshed = RoutingConfiguration.decode(
      await manager.refresh(expectedVersion: BigInt.from(_current.version)),
    );
    if (_disposed) return;
    _adoptConfiguration(refreshed);
  });

  void _receiveStatus(String text) {
    if (_disposed) return;
    final status = jsonDecode(text) as Map<String, Object?>;
    final configuration = RoutingConfiguration.decode(
      jsonEncode(status['configuration']),
    );
    if (configuration.version < _current.version) return;
    _adoptConfiguration(configuration);
    refreshing = status['refreshing'] as bool;
    refreshError = status['refresh_error'] as String?;
    final checked = status['last_checked_unix_ms'] as int?;
    lastChecked = checked == null
        ? null
        : DateTime.fromMillisecondsSinceEpoch(checked);
    notifyListeners();
  }

  void _adoptConfiguration(RoutingConfiguration configuration) {
    if (configuration.version <= _current.version) return;
    if (preview != null && !busy) error = '生效配置已更新，请重新校验并预览';
    invalidatePreview();
    _current = configuration;
  }

  Future<String> template(String name) => native.profileTemplate(name: name);

  Future<RoutingDraft> inspectDraft(String profile, String overrides) async =>
      RoutingDraft.decode(
        await native.routingEditor(profile: profile, overrides: overrides),
      );

  Future<RoutingDraft> editDraft(
    String profile,
    String overrides,
    Map<String, Object?> operation,
  ) async => RoutingDraft.decode(
    await native.editRouting(
      profile: profile,
      overrides: overrides,
      operation: jsonEncode(operation),
    ),
  );

  Future<String> test(String flowJson, String routesJson) =>
      manager.test(flowJson: flowJson, routesJson: routesJson);

  void invalidatePreview() {
    _previewRevision++;
    _prepared?.dispose();
    _prepared = null;
    preview = null;
  }

  Future<void> _operation(Future<void> Function() operation) async {
    if (busy || _disposed) return;
    busy = true;
    error = null;
    notifyListeners();
    try {
      await operation();
    } on model.RoutingFailure catch (failure) {
      error = failure.message;
    } on Object catch (failure) {
      error = '$failure';
    } finally {
      busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_updates.cancel());
    invalidatePreview();
    manager.dispose();
    super.dispose();
  }
}
