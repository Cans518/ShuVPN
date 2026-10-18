import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/rust/generated/routing/model.dart' as model;
import '../../core/rust/routing_draft.dart';
import '../../core/rust/routing_service.dart';
import '../../widgets/labelled_icon_button.dart';
import '../../widgets/settings_scaffold.dart';
import 'routing_rule_list.dart';

class RoutingSettingsPage extends StatefulWidget {
  const RoutingSettingsPage({super.key});
  @override
  State<RoutingSettingsPage> createState() => _RoutingSettingsPageState();
}

class _RoutingSettingsPageState extends State<RoutingSettingsPage> {
  final _profile = TextEditingController();
  final _overrides = TextEditingController();
  final _host = TextEditingController(text: 'oa.shu.edu.cn');
  final _port = TextEditingController(text: '443');
  final _processName = TextEditingController();
  final _processPath = TextEditingController();
  bool _loaded = false;
  String? _testResult;
  bool _loadingTemplate = false;
  String? _templateError;
  RoutingDraft? _draft;
  bool _editing = false;
  int _editorRevision = 0;
  String? _editorError;
  RoutingService? _service;
  String _testNetwork = 'tcp';
  String _testInbound = 'http';

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_loaded) return;
    final service = context.read<RoutingService?>();
    _service = service;
    if (service != null) {
      service.invalidatePreview();
      _profile.text = service.current.profile;
      _overrides.text = service.current.overrides.isEmpty
          ? '{}'
          : service.current.overrides;
      if (_profile.text.isNotEmpty) unawaited(_inspect(service));
    }
    _loaded = true;
  }

  @override
  void dispose() {
    _editorRevision++;
    _service?.invalidatePreview();
    _profile.dispose();
    _overrides.dispose();
    _host.dispose();
    _port.dispose();
    _processName.dispose();
    _processPath.dispose();
    super.dispose();
  }

  Future<void> _template(RoutingService service, String name) async {
    if (_loadingTemplate || service.busy) return;
    setState(() {
      _loadingTemplate = true;
      _templateError = null;
    });
    try {
      final template = await service.template(name);
      final draft = await service.inspectDraft(template, _overrides.text);
      if (!mounted) return;
      service.invalidatePreview();
      setState(() {
        _profile.text = template;
        _draft = draft;
        _editorError = null;
        _editorRevision++;
        _testResult = null;
      });
    } on Object catch (error) {
      if (mounted) {
        setState(() => _templateError = '无法载入模板：${_failure(error)}');
      }
    } finally {
      if (mounted) setState(() => _loadingTemplate = false);
    }
  }

  String _failure(Object failure) =>
      failure is model.RoutingFailure ? failure.message : '$failure';

  Future<void> _inspect(RoutingService service) async {
    final revision = ++_editorRevision;
    if (_profile.text.trim().isEmpty) {
      setState(() => _draft = null);
      return;
    }
    setState(() {
      _editing = true;
      _editorError = null;
    });
    try {
      final draft = await service.inspectDraft(_profile.text, _overrides.text);
      if (!mounted || revision != _editorRevision) return;
      setState(() => _draft = draft);
    } on Object catch (failure) {
      if (mounted && revision == _editorRevision) {
        setState(() => _editorError = '无法载入规则：${_failure(failure)}');
      }
    } finally {
      if (mounted && revision == _editorRevision) {
        setState(() => _editing = false);
      }
    }
  }

  void _rawChanged(RoutingService service) {
    service.invalidatePreview();
    setState(() {
      _editorRevision++;
      _draft = null;
      _editorError = null;
    });
  }

  Future<void> _edit(
    RoutingService service,
    Map<String, Object?> operation,
  ) async {
    if (_editing || service.busy || _loadingTemplate) return;
    if (operation['type'] == 'remove') {
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('删除这条规则？'),
          content: const Text('只修改编辑草稿。校验并应用后才会影响新连接。'),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: const Text('取消'),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: const Text('删除'),
            ),
          ],
        ),
      );
      if (!mounted || confirmed != true || service.busy || _editing) return;
    }
    final revision = ++_editorRevision;
    service.invalidatePreview();
    setState(() {
      _editing = true;
      _editorError = null;
    });
    try {
      final draft = await service.editDraft(
        _profile.text,
        _overrides.text,
        operation,
      );
      if (!mounted || revision != _editorRevision) return;
      setState(() {
        _draft = draft;
        _profile.text = draft.profile;
        _overrides.text = draft.overrides;
      });
    } on Object catch (failure) {
      if (mounted && revision == _editorRevision) {
        setState(() => _editorError = '未修改草稿：${_failure(failure)}');
      }
    } finally {
      if (mounted && revision == _editorRevision) {
        setState(() => _editing = false);
      }
    }
  }

  Future<void> _editRule(
    RoutingService service, {
    RoutingDraftRule? rule,
    String? source,
  }) async {
    final expression = await showShuTextPrompt(
      context: context,
      title: rule == null ? '新增规则' : '编辑规则',
      label: '规则表达式，例如 DOMAIN,oa.shu.edu.cn,SHU-VPN',
      initial: rule?.expression ?? 'DOMAIN-SUFFIX,shu.edu.cn,SHU-VPN',
    );
    if (!mounted || expression == null) return;
    await _edit(
      service,
      rule?.update(expression: expression) ??
          {'type': 'add', 'source': source, 'expression': expression},
    );
  }

  Future<void> _test(RoutingService service) async {
    try {
      final result = await service.test(
        jsonEncode({
          'host': _host.text.trim(),
          'port': int.parse(_port.text),
          'network': _testNetwork,
          'inbound': _testInbound,
          if (_processName.text.isNotEmpty || _processPath.text.isNotEmpty)
            'process': {'name': _processName.text, 'path': _processPath.text},
        }),
        '[]',
      );
      if (!mounted) return;
      final json = jsonDecode(result) as Map<String, Object?>;
      final decision = json['decision'] as Map<String, Object?>?;
      setState(
        () => _testResult = json['needsResolution'] == true
            ? '这项决策需要目标 IP；测试器不会发起 DNS 查询。'
            : '规则 ${decision?['rule_id']} · ${decision?['action']} · 版本 ${decision?['version']}',
      );
    } on Object catch (error) {
      if (mounted) setState(() => _testResult = '无法测试：${_failure(error)}');
    }
  }

  @override
  Widget build(BuildContext context) {
    final service = context.watch<RoutingService?>();
    if (service == null) {
      return const ShuSettingsSubPage(
        title: '规则分流',
        children: [
          Padding(
            padding: EdgeInsets.all(16),
            child: Text('当前平台尚未接入 Rust 规则配置。'),
          ),
        ],
      );
    }
    final busy = service.busy || _loadingTemplate || _editing;
    return ShuSettingsSubPage(
      title: '规则分流',
      children: [
        ListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(service.current.name),
          subtitle: Text('正在使用版本 ${service.current.version}。编辑不会立即改变连接路径。'),
        ),
        for (final warning in service.current.warnings)
          Text(
            warning,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        for (final notice in service.current.notices) Text(notice),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed:
                busy || service.refreshing || service.current.profile.isEmpty
                ? null
                : service.refreshProviders,
            icon: const Icon(Icons.refresh),
            label: Text(service.refreshing ? '正在更新规则集…' : '更新当前配置的规则集'),
          ),
        ),
        const Text('规则集设置 interval（秒）后会自动检查；未设置间隔时，可手动更新当前生效配置。'),
        if (service.lastChecked case final checked?)
          Text(
            '上次检查：${MaterialLocalizations.of(context).formatShortDate(checked)} '
            '${TimeOfDay.fromDateTime(checked).format(context)}',
          ),
        if (service.refreshError case final refreshError?)
          Text(
            '更新失败，当前有效规则继续运行：$refreshError',
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        Wrap(
          spacing: 12,
          runSpacing: 8,
          children: [
            OutlinedButton(
              onPressed: busy ? null : () => _template(service, 'campus-only'),
              child: const Text('仅校园模板'),
            ),
            OutlinedButton(
              onPressed: busy
                  ? null
                  : () => _template(service, 'campus-direct'),
              child: const Text('校园优先模板'),
            ),
          ],
        ),
        const SizedBox(height: 16),
        if (_draft case final draft?) ...[
          Card.outlined(
            child: ListTile(
              title: Text(draft.name),
              subtitle: Text(
                service.preview == null ? '编辑草稿 · 请校验并预览' : '已通过校验 · 应用预览后才生效',
              ),
              trailing: LabelledIconButton(
                tooltip: '重命名配置档',
                onPressed: busy
                    ? null
                    : () async {
                        final name = await showShuTextPrompt(
                          context: context,
                          title: '重命名配置档',
                          label: '配置档名称',
                          initial: draft.name,
                        );
                        if (!mounted || name == null) return;
                        await _edit(service, {'type': 'rename', 'name': name});
                      },
                icon: Icons.edit_outlined,
              ),
            ),
          ),
          for (final group in draft.groups)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 8),
              child: MergeSemantics(
                child: InputDecorator(
                  key: ValueKey('routing-group-${group.name}'),
                  decoration: InputDecoration(labelText: '策略组 · ${group.name}'),
                  child: Semantics(
                    enabled: !busy,
                    child: DropdownButtonHideUnderline(
                      child: DropdownButton<String>(
                        value: group.selected,
                        isExpanded: true,
                        items: [
                          for (final choice in group.choices)
                            DropdownMenuItem(
                              value: choice,
                              child: Semantics(
                                enabled: !busy,
                                child: Text(choice),
                              ),
                            ),
                        ],
                        onChanged: busy
                            ? null
                            : (value) {
                                if (value != null) {
                                  unawaited(
                                    _edit(service, {
                                      'type': 'select',
                                      'group': group.name,
                                      'policy': value,
                                    }),
                                  );
                                }
                              },
                      ),
                    ),
                  ),
                ),
              ),
            ),
          const Text('重复提示仅比较相同条件文本；其他覆盖关系请结合规则测试和连接记录查看。'),
          _rules(service, draft, 'prepend', '前置规则', busy),
          _rules(service, draft, 'profile', '配置档规则', busy),
          SwitchListTile(
            title: const Text('自动校园规则'),
            subtitle: const Text('位于配置档规则之后、后置规则之前；仍受网关授权限制。'),
            value: draft.automaticCampus,
            onChanged: busy
                ? null
                : (value) => unawaited(
                    _edit(service, {
                      'type': 'automatic_campus',
                      'enabled': value,
                    }),
                  ),
          ),
          _rules(service, draft, 'append', '后置规则', busy),
          for (final rule in draft.rules.where((rule) => rule.finalRule))
            Card.outlined(
              child: ListTile(
                title: Text('最终兜底 · ${rule.policy}'),
                subtitle: const Text('未命中前面的规则时使用；始终启用且固定在最后。'),
                trailing: LabelledIconButton(
                  tooltip: '编辑最终兜底',
                  onPressed: busy ? null : () => _editRule(service, rule: rule),
                  icon: Icons.edit_outlined,
                ),
              ),
            ),
        ] else if (!busy)
          const Text('选择模板，或导入配置后载入可视化编辑器。'),
        ExpansionTile(
          key: const ValueKey('routing-advanced'),
          title: const Text('导入与高级编辑'),
          subtitle: const Text('YAML / JSON；可视化修改会保留配置字段并转换为 JSON。'),
          maintainState: true,
          tilePadding: EdgeInsets.zero,
          children: [
            TextField(
              key: const ValueKey('routing-profile'),
              controller: _profile,
              enabled: !busy,
              minLines: 6,
              maxLines: 14,
              decoration: const InputDecoration(
                labelText: '配置档 YAML / JSON',
                alignLabelWithHint: true,
                border: OutlineInputBorder(),
              ),
              onChanged: (_) => _rawChanged(service),
            ),
            const SizedBox(height: 16),
            TextField(
              key: const ValueKey('routing-overrides'),
              controller: _overrides,
              enabled: !busy,
              minLines: 3,
              maxLines: 8,
              decoration: const InputDecoration(
                labelText: '前置 / 后置规则与策略组选择',
                alignLabelWithHint: true,
                border: OutlineInputBorder(),
              ),
              onChanged: (_) => _rawChanged(service),
            ),
            TextButton.icon(
              onPressed: busy ? null : () => _inspect(service),
              icon: const Icon(Icons.view_list_outlined),
              label: const Text('载入可视化编辑器'),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 12,
          runSpacing: 8,
          children: [
            FilledButton(
              onPressed: busy
                  ? null
                  : () async {
                      if (_loadingTemplate) return;
                      await service.prepare(_profile.text, _overrides.text);
                    },
              child: const Text('校验并预览'),
            ),
            OutlinedButton(
              onPressed: busy || service.preview == null
                  ? null
                  : service.applyPreview,
              child: const Text('应用预览'),
            ),
            TextButton(
              onPressed: busy || service.current.version == 0
                  ? null
                  : () async {
                      final before = service.current.version;
                      await service.rollback();
                      if (!mounted ||
                          service.error != null ||
                          service.current.version <= before) {
                        return;
                      }
                      setState(() {
                        _profile.text = service.current.profile;
                        _overrides.text = service.current.overrides;
                      });
                      await _inspect(service);
                    },
              child: const Text('回退上一版本'),
            ),
          ],
        ),
        if (busy)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 12),
            child: LinearProgressIndicator(),
          ),
        if (service.error != null)
          Text(
            service.error!,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        if (_templateError case final templateError?)
          Text(
            templateError,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        if (_editorError case final editorError?)
          Text(
            editorError,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        if (service.preview case final preview?)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 12),
            child: Text(
              '预览 ${preview.name} · 版本 ${preview.version}\n'
              '前置 → 配置档 → 自动校园 → 后置 → 最终 MATCH\n'
              '${[...preview.notices, ...preview.warnings].join('\n')}\n'
              '应用后只影响新连接。SHU-VPN 仍受网关授权限制；旧连接保留建立时的路径。',
            ),
          ),
        const Divider(height: 32),
        const Text('测试当前生效配置'),
        const SizedBox(height: 8),
        TextField(
          controller: _host,
          decoration: const InputDecoration(labelText: '目标域名或 IP'),
        ),
        TextField(
          controller: _port,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(labelText: '目标端口'),
        ),
        TextField(
          key: const ValueKey('routing-process-name'),
          controller: _processName,
          decoration: const InputDecoration(labelText: '模拟进程名称（可选）'),
        ),
        TextField(
          key: const ValueKey('routing-process-path'),
          controller: _processPath,
          decoration: const InputDecoration(labelText: '模拟可执行文件绝对路径（可选）'),
        ),
        const Text('以上进程信息仅供规则预览，不会读取系统或证明实际流量归属。'),
        MergeSemantics(
          child: Semantics(
            enabled: !busy,
            child: DropdownButtonFormField<String>(
              key: const ValueKey('routing-test-network'),
              initialValue: _testNetwork,
              decoration: const InputDecoration(labelText: '网络协议'),
              items: [
                for (final value in ['tcp', 'udp'])
                  DropdownMenuItem(
                    value: value,
                    child: Semantics(
                      enabled: !busy,
                      child: Text(value.toUpperCase()),
                    ),
                  ),
              ],
              onChanged: busy
                  ? null
                  : (value) {
                      if (value != null) {
                        setState(() {
                          _testNetwork = value;
                          _testResult = null;
                        });
                      }
                    },
            ),
          ),
        ),
        MergeSemantics(
          child: Semantics(
            enabled: !busy,
            child: DropdownButtonFormField<String>(
              key: const ValueKey('routing-test-inbound'),
              initialValue: _testInbound,
              decoration: const InputDecoration(labelText: '入站类型'),
              items: [
                for (final value in ['http', 'socks', 'tun'])
                  DropdownMenuItem(
                    value: value,
                    child: Semantics(
                      enabled: !busy,
                      child: Text(value.toUpperCase()),
                    ),
                  ),
              ],
              onChanged: busy
                  ? null
                  : (value) {
                      if (value != null) {
                        setState(() {
                          _testInbound = value;
                          _testResult = null;
                        });
                      }
                    },
            ),
          ),
        ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton(
            onPressed: busy ? null : () => _test(service),
            child: Text(
              '预览 ${_testInbound.toUpperCase()} / ${_testNetwork.toUpperCase()} 决策',
            ),
          ),
        ),
        if (_testResult != null)
          SelectableText(_testResult!, semanticsLabel: _testResult),
        const SizedBox(height: 8),
        Text('已支持：${(service.capabilities['supported'] as List).join('、')}'),
        Text('尚未支持：${(service.capabilities['unsupported'] as List).join('、')}'),
        if (service.capabilities['processIdentityMatrix'] case final Map matrix)
          for (final platform in matrix.entries)
            Text(
              '${platform.key} 进程规则：${(platform.value as Map).entries.map((entry) => '${entry.key} ${entry.value}').join('；')}',
            ),
        const Text(
          '实际连接只使用本机套接字的进程快照。远端 LAN、无法读取归属、进程退出或可见归属歧义时，'
          '仍需进程身份的规则会报错，不会跳过后继续直连。已有流保留原决策。',
        ),
        const Text('权限不可见的共享持有者无法被排除；这些快照不是进程认证或历史发送者证明。'),
        const Text(
          '进程名称和路径按精确文本匹配，不展开通配符。'
          '含逗号或括号等分隔字符时，用 JSON 双引号包裹值。'
          '专用校园 DNS 捕获请求走独立解析流程，不使用普通出站进程规则。',
        ),
        Text(
          '逻辑嵌套最多 ${service.capabilities['logicMaxDepth']} 层；'
          '正则最多 ${service.capabilities['regexMaxBytes']} 字节，不支持回溯引用或环视。',
        ),
        const Text(
          '测试结果仅表示规则意图，不表示网关授权或实际连通性。'
          '规则不能扩展校园授权；TUN 只能执行已进入捕获范围的流量规则。',
        ),
      ],
    );
  }

  Widget _rules(
    RoutingService service,
    RoutingDraft draft,
    String source,
    String title,
    bool busy,
  ) => RoutingRuleList(
    key: ValueKey('routing-rules-$source'),
    title: title,
    source: source,
    rules: draft.rules
        .where((rule) => rule.source == source && !rule.finalRule)
        .toList(),
    busy: busy,
    onEdit: (rule) => unawaited(_editRule(service, rule: rule)),
    onOperation: (operation) => unawaited(_edit(service, operation)),
    onAdd: () => unawaited(_editRule(service, source: source)),
  );
}
