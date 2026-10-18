import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../app/app_info.dart';
import '../core/account/account_center.dart';
import '../core/connection/connection_controller.dart';
import '../core/logging/shu_log.dart';
import '../core/update/shu_update_client.dart';
import '../core/update/shu_update_policy.dart';
import '../widgets/shu_update_prompt.dart';
import 'floating_dock.dart';

/// Hosts the three dock destinations.
///
/// [StatefulNavigationShell] keeps one `Navigator` per branch and an
/// `IndexedStack` on top of them, so connection presentation and the log buffer
/// survive every tab switch.
class DockShell extends StatefulWidget {
  const DockShell({super.key, required this.navigationShell});

  final StatefulNavigationShell navigationShell;

  @override
  State<DockShell> createState() => _DockShellState();
}

class _DockShellState extends State<DockShell> {
  @override
  void initState() {
    super.initState();
    // Account login owns interactive challenges; connection receives its owner.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final controller = context.read<ConnectionController>();
      final account = context.read<AccountCenter>();
      controller.ssoSessionProvider = () => account.authSession;
      ShuLog.d(
        ShuLogTag.conn,
        'dock shell 就绪 · 已注入统一认证会话提供者 · '
        '已登录=${account.signedIn}',
      );
      // 这里**不做**任何凭据核对。以前每次启动都会静默重跑一轮交换，代价是
      // 一打开应用就发一串请求 —— 其中一项真的要建一次 aTrust 隧道。而用户
      // 大多只是来连隧道的，根本没打开过账户页。核对挪到账户页真正被打开时
      // （`AccountCenter.verifyIfStale`），结论则用磁盘快照先顶上。

      // 更新检查同理：一次网络往返，不能挡在首帧前面，失败也只记一行日志。
      unawaited(_checkForUpdate());
    });
  }

  /// 启动后查一次更新，有新版本就弹窗。
  ///
  /// 不去重：只要本地版本不是远端最新，每次启动都提示。
  Future<void> _checkForUpdate() async {
    if (!ShuUpdatePolicy.enabled) return;
    final client = context.read<ShuUpdateClient>();
    try {
      final update = await client.checkForUpdate(ShuAppInfo.version);
      if (!mounted || update == null) return;
      final openDownload = await showShuUpdatePrompt(context, update: update);
      if (!mounted || !openDownload) return;
      await openShuUpdateUrl(context, update.targetUrl);
    } on Object catch (error) {
      ShuLog.w(ShuLogTag.update, '启动检查更新失败 · $error');
    }
  }

  void _onSelect(int index) {
    final shell = widget.navigationShell;
    final isCurrent = index == shell.currentIndex;

    // Tapping the middle item while already on the connect page is the same
    // gesture as the orb. From any other page it only navigates, so a stray tap
    // can never tear a working tunnel down.
    if (isCurrent && ShuTab.values[index] == ShuTab.connect) {
      context.read<ConnectionController>().toggle();
      return;
    }
    shell.goBranch(index, initialLocation: isCurrent);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: widget.navigationShell,
      bottomNavigationBar: FloatingDock(
        currentIndex: widget.navigationShell.currentIndex,
        onSelect: _onSelect,
      ),
    );
  }
}
