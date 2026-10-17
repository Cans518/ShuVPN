// 测速页的离线渲染测试。
//
// 真实现要打十几秒网络，这里换成按脚本回调的替身：验证按钮在三个状态下的
// 文案、读数落在哪个位置、完成之后回到待测、以及失败时那句校园网提示。

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/app/theme.dart';
import 'package:shuvpn/core/speedtest/shu_speedtest.dart';
import 'package:shuvpn/core/speedtest/shu_speedtest_service.dart';
import 'package:shuvpn/features/services/speedtest_page.dart';

class _ScriptedRunner implements ShuSpeedTestRunner {
  final Completer<void> _gate = Completer<void>();

  /// 页面不该在测量中调它；测试拿它验证按钮已锁住。
  int cancelCount = 0;

  /// 让 [run] 走到返回。测试用它把「跑到一半」与「跑完」分成两帧。
  void finish() => _gate.complete();

  @override
  ShuSpeedTestServer get server => ShuSpeedTestServer.shu;

  @override
  void cancel() => cancelCount++;

  @override
  Future<ShuSpeedTestReport> run({
    required void Function(ShuSpeedTestProgress progress) onUpdate,
  }) async {
    onUpdate(
      const ShuSpeedTestProgress(
        phase: ShuSpeedTestPhase.ping,
        progress: 1,
        pingMs: 12,
        jitterMs: 1.5,
      ),
    );
    // 与服务一致：阶段进行中不回传「下载 / 上传」两格的数，等阶段收尾才填。
    onUpdate(
      const ShuSpeedTestProgress(
        phase: ShuSpeedTestPhase.download,
        speedMbps: 88.5,
        progress: 1,
        pingMs: 12,
        jitterMs: 1.5,
        downloadLatencyMs: 18.5,
      ),
    );
    onUpdate(
      const ShuSpeedTestProgress(
        phase: ShuSpeedTestPhase.upload,
        speedMbps: 31.2,
        progress: 1,
        pingMs: 12,
        jitterMs: 1.5,
        downloadLatencyMs: 18.5,
        uploadLatencyMs: 22,
        downloadMbps: 88.5,
      ),
    );
    await _gate.future;
    return ShuSpeedTestReport(
      server: ShuSpeedTestServer.shu,
      pingMs: 12,
      jitterMs: 1.5,
      downloadLatencyMs: 18.5,
      uploadLatencyMs: 22,
      downloadMbps: 88.5,
      uploadMbps: 31.2,
      elapsed: const Duration(seconds: 1),
      clientIp: '10.10.1.23',
    );
  }
}

class _UnreachableRunner implements ShuSpeedTestRunner {
  @override
  ShuSpeedTestServer get server => ShuSpeedTestServer.shu;

  @override
  void cancel() {}

  @override
  Future<ShuSpeedTestReport> run({
    required void Function(ShuSpeedTestProgress progress) onUpdate,
  }) async {
    throw const ShuSpeedTestException(
      '无法连接测速服务器',
      kind: ShuSpeedTestFailure.unreachable,
    );
  }
}

/// 只停在时延阶段的替身：用来确认那一刻表盘与读数区不给任何东西。
class _PingRunner implements ShuSpeedTestRunner {
  final Completer<void> _gate = Completer<void>();

  @override
  ShuSpeedTestServer get server => ShuSpeedTestServer.shu;

  @override
  void cancel() {}

  @override
  Future<ShuSpeedTestReport> run({
    required void Function(ShuSpeedTestProgress progress) onUpdate,
  }) async {
    onUpdate(
      const ShuSpeedTestProgress(phase: ShuSpeedTestPhase.ping, progress: 0.4),
    );
    await _gate.future;
    throw const ShuSpeedTestException(
      '测速已停止',
      kind: ShuSpeedTestFailure.cancelled,
    );
  }
}

Future<void> _pumpPage(
  WidgetTester tester,
  ShuSpeedTestRunner runner, {
  Size physical = const Size(720, 1600),
}) async {
  tester.view.physicalSize = physical;
  tester.view.devicePixelRatio = 2;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShuTheme(Brightness.light),
      home: SpeedTestPage(runner: runner),
    ),
  );
}

/// 表盘的目标值（0..1）。页面里只有表盘这一个补间。
double _gaugeTarget(WidgetTester tester) {
  final gauge = tester.widget<TweenAnimationBuilder<double>>(
    find.byType(TweenAnimationBuilder<double>),
  );
  return gauge.tween.end!;
}

void main() {
  // 上一轮结果记在进程内的历史里，测试之间必须互相清干净。
  setUp(ShuSpeedTestHistory.reset);

  testWidgets('一轮测速：测量中锁住按钮，完成后回到待测但留着四个读数', (tester) async {
    final runner = _ScriptedRunner();
    await _pumpPage(tester, runner);

    expect(find.text('开始测速'), findsOneWidget);
    expect(find.text('0.00'), findsWidgets, reason: '待测时读数区是 0.00');
    expect(find.text('测速支持：上海大学校内测速站 speedtest.shu.edu.cn'), findsOneWidget);

    await tester.tap(find.text('开始测速'));
    await tester.pump();

    // 测量中：按钮锁住，不能停下来。
    expect(find.text('测速中'), findsOneWidget);
    final button = tester.widget<FilledButton>(
      find.byWidgetPredicate((widget) => widget is FilledButton),
    );
    expect(button.onPressed, isNull, reason: '测量中不该还能点');
    await tester.tap(find.text('测速中'), warnIfMissed: false);
    await tester.pump();
    expect(runner.cancelCount, 0, reason: '按钮不该触发取消');

    // 测量中：表盘固定两位小数；下载格已经填上，上传格还是 `--`。
    expect(find.text('31.20'), findsOneWidget);
    expect(find.text('88.50'), findsOneWidget);
    expect(_gaugeTarget(tester), greaterThan(0));

    runner.finish();
    await tester.pumpAndSettle();

    // 完成之后看起来和刚进来一样：表盘归 0、按钮回到「开始测速」。
    expect(find.text('开始测速'), findsOneWidget);
    expect(find.text('测速完成'), findsNothing);
    expect(find.text('再测一次'), findsNothing);
    // 读数区不写任何状态字。
    expect(find.text('准备就绪'), findsNothing);
    expect(find.text('正在下载'), findsNothing);
    expect(find.text('正在上传'), findsNothing);
    expect(_gaugeTarget(tester), 0);

    // 唯一留下的是四个读数，上下行固定两位小数；三个延迟不带文字标签。
    expect(find.text('88.50'), findsOneWidget);
    expect(find.text('31.20'), findsOneWidget);
    expect(find.text('12.0 ms'), findsOneWidget);
    expect(find.text('18.5 ms'), findsOneWidget);
    expect(find.text('22.0 ms'), findsOneWidget);
    expect(find.text('抖动 1.50 ms'), findsOneWidget);

    // 页脚只有一行固定文案，不附带走线或本机地址。
    expect(find.textContaining('本机'), findsNothing);
  });

  testWidgets('时延阶段表盘与读数区都不显示数值', (tester) async {
    await _pumpPage(tester, _PingRunner());

    await tester.tap(find.text('开始测速'));
    await tester.pump();

    // 时延阶段读数区全空，连单位也不报。
    expect(find.text('正在检测时延'), findsNothing);
    expect(find.text('0.00'), findsNothing);
    // 只剩下面下载 / 上传两格各一个。
    expect(find.text('Mbps'), findsNWidgets(2));
  });

  testWidgets('离开页面再进来仍显示上一轮结果', (tester) async {
    final runner = _ScriptedRunner();
    await _pumpPage(tester, runner);
    await tester.tap(find.text('开始测速'));
    await tester.pump();
    runner.finish();
    await tester.pumpAndSettle();
    expect(find.text('88.50'), findsOneWidget);

    // 路由销毁页面（返回服务列表）后再建一颗新的 State。
    await tester.pumpWidget(const MaterialApp(home: SizedBox.shrink()));
    await tester.pump();
    await _pumpPage(tester, _ScriptedRunner());

    // 重新进来也是待测状态，只是四个读数还在。
    expect(find.text('开始测速'), findsOneWidget);
    expect(find.text('88.50'), findsOneWidget);
    expect(find.text('31.20'), findsOneWidget);
    expect(find.text('12.0 ms'), findsOneWidget);
    expect(_gaugeTarget(tester), 0);
  });

  testWidgets('连不上时给出校园网提示与重试', (tester) async {
    await _pumpPage(tester, _UnreachableRunner());

    await tester.tap(find.text('开始测速'));
    await tester.pumpAndSettle();

    // 读数区只剩一个破折号，不写「测速失败」这类状态字。
    expect(find.text('—'), findsOneWidget);
    expect(find.text('无法连接测速服务器'), findsOneWidget);
    expect(find.textContaining('校园网内可达'), findsOneWidget);
    expect(find.text('重试'), findsOneWidget);
  });

  testWidgets('窄屏不溢出：内容宽度扣掉页边距', (tester) async {
    // 320 逻辑宽的旧机型。页面曾经按 `constraints.maxWidth` 取内容宽度、
    // 外面又加左右各 16 的 padding，于是整页右移出屏幕 32 像素。
    final runner = _ScriptedRunner();
    await _pumpPage(tester, runner, physical: const Size(640, 1600));

    await tester.tap(find.text('开始测速'));
    await tester.pump();
    runner.finish();
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull, reason: '窄屏上不应有布局溢出');
    expect(find.text('开始测速'), findsOneWidget);
    expect(find.text('88.50'), findsOneWidget);
  });
}
