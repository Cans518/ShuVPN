import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/connection/connection_controller.dart';
import '../../core/connection/connection_observation.dart';
import '../../widgets/labelled_icon_button.dart';
import '../../widgets/shu_app_bar.dart';

class ConnectionRecordsPage extends StatefulWidget {
  const ConnectionRecordsPage({super.key});

  @override
  State<ConnectionRecordsPage> createState() => _ConnectionRecordsPageState();
}

class _ConnectionRecordsPageState extends State<ConnectionRecordsPage> {
  ConnectionController? _controller;
  (int, bool)? _source;
  int _revision = 0;
  bool _loading = false;
  String? _error;
  List<ConnectionObservation> _records = const [];

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final controller = context.watch<ConnectionController>();
    final source = (
      controller.observationGeneration,
      controller.observationsAvailable,
    );
    if (identical(controller, _controller) && source == _source) return;
    _controller = controller;
    _source = source;
    _revision++;
    _records = const [];
    _error = null;
    _loading = false;
    if (source.$2) unawaited(_reload());
  }

  Future<void> _reload() async {
    final controller = _controller;
    if (controller == null || !controller.observationsAvailable || _loading) {
      return;
    }
    final revision = ++_revision;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final records = await controller.readObservations();
      if (!mounted || revision != _revision) return;
      setState(() => _records = records.reversed.toList());
    } on Object catch (failure) {
      if (mounted && revision == _revision) {
        setState(
          () => _error = _records.isEmpty
              ? '读取连接记录失败：$failure'
              : '刷新失败，以下保留上次快照：$failure',
        );
      }
    } finally {
      if (mounted && revision == _revision) {
        setState(() => _loading = false);
      }
    }
  }

  @override
  void dispose() {
    _revision++;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: ShuAppBar(
        title: '连接记录',
        onBack: () => Navigator.of(context).pop(),
        actions: [
          LabelledIconButton(
            tooltip: '刷新连接记录',
            onPressed: _loading || _source?.$2 != true ? null : _reload,
            icon: Icons.refresh,
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          const Text(
            '当前会话最近最多128条出站结果，最新记录在前；这是历史快照，不是活动连接列表。'
            '成功记录不代表请求已完成，同一流可能有多条结果，未被捕获或未能记下的流量不会显示。',
          ),
          const SizedBox(height: 16),
          if (_loading) const LinearProgressIndicator(),
          if (_source?.$2 != true)
            const Text('当前没有可读取的原生会话。连接校园网后，可在这里刷新记录。')
          else if (!_loading && _error == null && _records.isEmpty)
            const Text('当前会话还没有出站结果记录。'),
          if (_error case final error?)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12),
              child: Text(
                error,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          for (final record in _records) _record(record),
        ],
      ),
    );
  }

  Widget _record(ConnectionObservation record) => Card.outlined(
    margin: const EdgeInsets.symmetric(vertical: 6),
    child: ExpansionTile(
      key: ObjectKey(record),
      title: Text('${record.host} · ${record.port}'),
      subtitle: Text(
        '${record.network.toUpperCase()} · ${record.inbound.toUpperCase()} · '
        '${record.outbound == null ? '无成功出站记录' : '已打开 ${record.outbound} 出站'}',
      ),
      childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
      expandedCrossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (record.domainCandidates.isNotEmpty)
          _detail('DNS 候选名称（未认定实际域名）', record.domainCandidates.join('、')),
        if (record.process case final process?) ...[
          if (process.name case final name?) _detail('进程名称', name),
          if (process.pid case final pid? when pid > 0)
            _detail('进程 PID', '$pid'),
          if (process.path case final path?) _detail('可执行文件路径', path),
          if (process.source case final source?) _detail('进程快照来源', source),
          if (process.confidence case final confidence?)
            _detail(
              '进程身份依据',
              confidence == 'socket-snapshot'
                  ? '当前可见套接字归属快照，不是历史发送者证明'
                  : '模拟输入',
            ),
        ],
        _detail(
          '已知目标 IP',
          record.addresses.isEmpty ? '未记录' : record.addresses.join('、'),
        ),
        _detail('规则版本', record.version?.toString() ?? '未记录'),
        _detail(
          '命中规则',
          record.ruleId ?? (record.hasDecision ? '未记录规则 ID' : '无规则决策记录'),
        ),
        if (record.source case final source?)
          _detail('规则来源', _sourceLabel(source)),
        if (record.policy case final policy?) _detail('策略', policy),
        _detail('期望动作', record.expectedAction ?? '未记录'),
        _detail('实际出站', record.outbound ?? '本条未记录成功出站'),
        if (record.reason case final reason?) _detail('命中原因', reason),
        if (record.error case final error?) _detail('错误代码', error),
        if (record.message case final message?) _detail('失败或拒绝原因', message),
        for (final dns in record.dns) ...[
          const Divider(),
          _detail('DNS 名称', dns.host),
          _detail('解析地址', dns.addresses.join('、')),
          _detail(
            '解析来源',
            '${dns.source == 'campus'
                ? '校园 DNS'
                : dns.source == 'system'
                ? '系统解析'
                : dns.source}${dns.server == null ? '' : ' · ${dns.server}'}',
          ),
          _detail('DNS 缓存', dns.cached ? '使用缓存' : '本次解析'),
          if (dns.ttlSeconds case final ttl?) _detail('DNS TTL', '$ttl 秒'),
          for (final warning in dns.warnings) _detail('解析提示', warning),
        ],
      ],
    ),
  );

  Widget _detail(String label, String value) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 5),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ExcludeSemantics(
          child: Text(label, style: Theme.of(context).textTheme.labelMedium),
        ),
        SelectableText(value, semanticsLabel: '$label：$value'),
      ],
    ),
  );

  String _sourceLabel(String source) => switch (source) {
    'prepend' => '前置规则',
    'profile' => '配置档规则',
    'campus' => '自动校园规则',
    'append' => '后置规则',
    _ => source,
  };
}
