import 'dart:async';

import 'package:flutter/material.dart';

import '../core/desktop/desktop_lifecycle.dart';
import '../core/desktop/desktop_port.dart';
import '../core/logging/shu_log.dart';

class DesktopFrame extends StatelessWidget {
  const DesktopFrame({
    super.key,
    required this.port,
    required this.child,
    this.lifecycle,
  });
  final DesktopPort port;
  final DesktopLifecycle? lifecycle;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final lifecycle = this.lifecycle;
    // The caption sits above the router's Navigator and needs its own tooltip overlay.
    return Overlay.wrap(
      child: lifecycle == null
          ? _frame(context)
          : ListenableBuilder(
              listenable: lifecycle,
              builder: (context, _) => _frame(context),
            ),
    );
  }

  Future<void> _run(
    BuildContext context,
    Future<void> Function() action,
  ) async {
    try {
      await action();
    } on Object catch (failure) {
      ShuLog.w('desktop', '窗口操作失败：$failure');
      if (context.mounted) {
        ScaffoldMessenger.maybeOf(context)
            ?.showSnackBar(const SnackBar(content: Text('窗口操作失败，请重试。')));
      }
    }
  }

  Widget _frame(BuildContext context) {
    final theme = Theme.of(context);
    final lifecycle = this.lifecycle;
    return Material(
      color: theme.scaffoldBackgroundColor,
      child: Column(
        children: [
          SizedBox(
            height: 44,
            child: Row(
              children: [
                Expanded(
                  child: GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onPanStart: (_) => unawaited(_run(context, port.drag)),
                    onDoubleTap: () =>
                        unawaited(_run(context, port.toggleMaximize)),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      child: Row(
                        children: [
                          Image.asset(
                            'assets/images/icon_clear_blue.png',
                            width: 22,
                            height: 22,
                          ),
                          const SizedBox(width: 10),
                          Text(
                            lifecycle?.quitting == true
                                ? 'ShuVPN · 正在退出…'
                                : 'ShuVPN',
                            style: theme.textTheme.labelLarge,
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
                IconButton(
                  tooltip: '最小化',
                  onPressed: () => unawaited(_run(context, port.minimize)),
                  icon: const Icon(Icons.remove, size: 18),
                ),
                IconButton(
                  tooltip: '最大化或还原',
                  onPressed: () =>
                      unawaited(_run(context, port.toggleMaximize)),
                  icon: const Icon(Icons.crop_square, size: 16),
                ),
                IconButton(
                  tooltip: lifecycle?.trayReady == true ? '隐藏到托盘' : '关闭并退出',
                  onPressed: lifecycle?.quitting == true
                      ? null
                      : () => unawaited(
                          _run(
                            context,
                            lifecycle?.windowClose ?? port.destroyWindow,
                          ),
                        ),
                  icon: const Icon(Icons.close, size: 18),
                ),
              ],
            ),
          ),
          if (lifecycle?.error case final error?)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
              child: Text(error, style: theme.textTheme.bodySmall),
            ),
          Expanded(child: child),
        ],
      ),
    );
  }
}
