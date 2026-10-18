import 'package:flutter/material.dart';

import '../../core/rust/routing_draft.dart';
import '../../widgets/labelled_icon_button.dart';

/// A bounded page keeps large imported lists from building thousands of cards.
class RoutingRuleList extends StatefulWidget {
  const RoutingRuleList({
    super.key,
    required this.title,
    required this.source,
    required this.rules,
    required this.busy,
    required this.onEdit,
    required this.onOperation,
    required this.onAdd,
  });

  final String title;
  final String source;
  final List<RoutingDraftRule> rules;
  final bool busy;
  final ValueChanged<RoutingDraftRule> onEdit;
  final ValueChanged<Map<String, Object?>> onOperation;
  final VoidCallback onAdd;

  @override
  State<RoutingRuleList> createState() => _RoutingRuleListState();
}

class _RoutingRuleListState extends State<RoutingRuleList> {
  static const _pageSize = 25;
  int _page = 0;

  void _move(RoutingDraftRule rule, RoutingDraftRule target) {
    widget.onOperation({
      'type': 'move',
      'source': rule.source,
      'index': rule.index,
      'to': target.index,
    });
  }

  @override
  Widget build(BuildContext context) {
    final rules = widget.rules;
    final pageCount = rules.isEmpty ? 1 : (rules.length / _pageSize).ceil();
    final page = _page < pageCount ? _page : pageCount - 1;
    final start = page * _pageSize;
    final visible = rules.skip(start).take(_pageSize).toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(
              '${widget.title} · ${rules.length} 条',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            MergeSemantics(
              child: Semantics(
                label: '新增${widget.title}',
                child: TextButton.icon(
                  key: ValueKey('routing-add-${widget.source}'),
                  onPressed: widget.busy ? null : widget.onAdd,
                  icon: const Icon(Icons.add),
                  label: const ExcludeSemantics(child: Text('新增规则')),
                ),
              ),
            ),
          ],
        ),
        if (rules.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 8),
            child: Text('暂无规则'),
          ),
        ReorderableListView(
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          buildDefaultDragHandles: false,
          onReorderItem: (oldIndex, newIndex) {
            if (widget.busy) return;
            if (oldIndex == newIndex) return;
            _move(visible[oldIndex], visible[newIndex]);
          },
          children: [
            for (var position = 0; position < visible.length; position++)
              _card(context, visible[position], position, start + position),
          ],
        ),
        if (pageCount > 1)
          Wrap(
            alignment: WrapAlignment.center,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              LabelledIconButton(
                tooltip: '${widget.title}上一页',
                onPressed: widget.busy || page == 0
                    ? null
                    : () => setState(() => _page = page - 1),
                icon: Icons.chevron_left,
              ),
              Text('${page + 1} / $pageCount'),
              LabelledIconButton(
                tooltip: '${widget.title}下一页',
                onPressed: widget.busy || page + 1 >= pageCount
                    ? null
                    : () => setState(() => _page = page + 1),
                icon: Icons.chevron_right,
              ),
            ],
          ),
        const SizedBox(height: 16),
      ],
    );
  }

  Widget _card(
    BuildContext context,
    RoutingDraftRule rule,
    int pageIndex,
    int absoluteIndex,
  ) {
    return Card.outlined(
      key: ValueKey('routing-rule-${rule.source}-${rule.index}'),
      margin: const EdgeInsets.only(bottom: 8),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              '${absoluteIndex + 1}. ${rule.kind} · ${rule.policy}',
              style: Theme.of(context).textTheme.titleSmall,
            ),
            if (rule.value.isNotEmpty) SelectableText(rule.value),
            Text(
              '${rule.enabled ? '已启用' : '已停用'}'
              '${rule.noResolve ? ' · 不主动解析 DNS' : ''}',
            ),
            if (rule.shadowedBy case final earlier?)
              Text('条件文本重复；校验通过后由更早规则 $earlier 优先匹配。'),
            Wrap(
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                MergeSemantics(
                  child: Semantics(
                    label:
                        '${widget.title} 第${absoluteIndex + 1}条：'
                        '启用或停用 ${rule.expression}',
                    child: Tooltip(
                      message: '启用或停用 ${rule.expression}',
                      excludeFromSemantics: true,
                      child: Switch(
                        key: ValueKey(
                          'routing-toggle-${rule.source}-${rule.index}',
                        ),
                        value: rule.enabled,
                        onChanged: widget.busy
                            ? null
                            : (value) => widget.onOperation(
                                rule.update(enabled: value),
                              ),
                      ),
                    ),
                  ),
                ),
                LabelledIconButton(
                  tooltip: '编辑 ${rule.expression}',
                  semanticLabel:
                      '${widget.title} 第${absoluteIndex + 1}条：'
                      '编辑 ${rule.expression}',
                  onPressed: widget.busy ? null : () => widget.onEdit(rule),
                  icon: Icons.edit_outlined,
                ),
                LabelledIconButton(
                  tooltip: '上移 ${rule.expression}',
                  semanticLabel:
                      '${widget.title} 第${absoluteIndex + 1}条：'
                      '上移 ${rule.expression}',
                  onPressed: widget.busy || absoluteIndex == 0
                      ? null
                      : () => _move(rule, widget.rules[absoluteIndex - 1]),
                  icon: Icons.arrow_upward,
                ),
                LabelledIconButton(
                  tooltip: '下移 ${rule.expression}',
                  semanticLabel:
                      '${widget.title} 第${absoluteIndex + 1}条：'
                      '下移 ${rule.expression}',
                  onPressed:
                      widget.busy || absoluteIndex + 1 >= widget.rules.length
                      ? null
                      : () => _move(rule, widget.rules[absoluteIndex + 1]),
                  icon: Icons.arrow_downward,
                ),
                LabelledIconButton(
                  tooltip: '删除 ${rule.expression}',
                  semanticLabel:
                      '${widget.title} 第${absoluteIndex + 1}条：'
                      '删除 ${rule.expression}',
                  onPressed: widget.busy
                      ? null
                      : () => widget.onOperation({
                          'type': 'remove',
                          'source': rule.source,
                          'index': rule.index,
                        }),
                  icon: Icons.delete_outline,
                ),
                if (!widget.busy)
                  ReorderableDragStartListener(
                    index: pageIndex,
                    child: Semantics(
                      container: true,
                      image: true,
                      label:
                          '${widget.title} 第${absoluteIndex + 1}条：'
                          '拖动排序 ${rule.expression}；也可使用上移和下移按钮',
                      child: ExcludeSemantics(
                        child: Tooltip(
                          message: '拖动排序；也可使用上移和下移按钮',
                          excludeFromSemantics: true,
                          child: const Padding(
                            padding: EdgeInsets.all(12),
                            child: Icon(Icons.drag_handle),
                          ),
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
