import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:provider/provider.dart';

import '../core/account/account_center.dart';
import '../core/connection/connection_controller.dart';
import '../core/connection/connection_status.dart';
import '../core/connection/protocol.dart';
import '../core/desktop/desktop_lifecycle.dart';
import '../core/desktop/desktop_port.dart';
import '../core/settings/settings_store.dart';
import 'desktop_frame.dart';

class DesktopHost extends StatefulWidget {
  const DesktopHost({
    super.key,
    required this.port,
    required this.navigate,
    required this.child,
  });
  final DesktopPort port;
  final void Function(String) navigate;
  final Widget child;
  @override
  State<DesktopHost> createState() => _DesktopHostState();
}

class _DesktopHostState extends State<DesktopHost> {
  late ConnectionController _connection;
  late SettingsStore _settings;
  late AccountCenter _account;
  late DesktopLifecycle _lifecycle;
  @override
  void initState() {
    super.initState();
    _connection = context.read<ConnectionController>();
    _settings = context.read<SettingsStore>();
    _account = context.read<AccountCenter>();
    _lifecycle = DesktopLifecycle(
      port: widget.port,
      connect: () async {
        widget.navigate(_settings.welcomeCompleted ? '/connect' : '/welcome');
        await WidgetsBinding.instance.endOfFrame;
        if (!mounted || !_settings.welcomeCompleted) return;
        if (!_account.signedIn) {
          widget.navigate('/settings/account');
          return;
        }
        await _connection.connect();
      },
      disconnect: _connection.disconnect,
      shutdown: _connection.disconnectForExit,
      selectProtocol: (id) {
        final protocol = ShuProtocol.fromId(id);
        if (protocol != null) _connection.selectProtocol(protocol);
      },
    );
    _connection.addListener(_refresh);
    _settings.addListener(_refresh);
    _refresh();
    unawaited(_lifecycle.start());
  }

  void _refresh() {
    final state = _connection.state;
    final idle =
        state == ConnectionStatus.disconnected ||
        state == ConnectionStatus.error;
    final labels = <ConnectionStatus, String>{
      ConnectionStatus.disconnected: '未连接',
      ConnectionStatus.connecting: '正在连接',
      ConnectionStatus.authenticated: '已认证',
      ConnectionStatus.connected: '已连接',
      ConnectionStatus.disconnecting: '正在断开',
      ConnectionStatus.error: '连接失败',
    };
    _lifecycle.update(
      TraySnapshot(
        status:
            '${labels[state] ?? state.name} · ${_connection.draft.protocol.label}',
        canConnect:
            idle &&
            !_connection.busy &&
            !_connection.cleanupPending &&
            _connection.hasEnabledProtocol &&
            _settings.welcomeCompleted,
        canDisconnect:
            state != ConnectionStatus.disconnecting &&
            _connection.canDisconnect,
        protocols: [
          for (final protocol in ShuProtocol.values)
            TrayProtocol(
              id: protocol.id,
              label: protocol.label,
              enabled:
                  idle &&
                  !_connection.busy &&
                  _settings.isProtocolEnabled(protocol),
              selected: _connection.draft.protocol == protocol,
            ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _connection.removeListener(_refresh);
    _settings.removeListener(_refresh);
    _lifecycle.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {
      const SingleActivator(LogicalKeyboardKey.keyQ, control: true): () =>
          unawaited(_lifecycle.dispatch(DesktopCommand.quit)),
    },
    child: DesktopFrame(
      port: widget.port,
      lifecycle: _lifecycle,
      child: widget.child,
    ),
  );
}
