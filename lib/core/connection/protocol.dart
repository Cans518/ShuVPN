import 'package:flutter/material.dart';

/// 本应用能「用哪种协议去连」的那几种可能。
///
/// 列出已实现的协议及当前不可用的预留选项。
///
/// | 值 | 实现 | 默认启用 | 说明 |
/// | :--- | :--- | :--- | :--- |
/// | [atrust] | Rust 核心 | **是** | 上大部署的网关 |
/// | [easyConnect] | 未接入 | 否，且开关封掉 | 预留选项 |
/// | [openVpn] | 没有 | 否，且开关封掉 | 不是 Sangfor 家族的东西 |
///
/// 所以「开关关掉」在这三者身上是两件不同的事：aTrust 是真的可以关
/// （用户只想要一个不联网的状态），另外两个是**关不了也开不了** ——
/// 它们没有可执行的代码路径。设置页把开关画成灰的而不是藏起来，
/// 是为了让人看到「这个位置将来会有一个开关」，而不是以为漏了。
enum ShuProtocol {
  atrust(
    id: 'atrust',
    label: 'aTrust',
    icon: Icons.shield_moon_outlined,
    implemented: true,
  ),
  easyConnect(
    id: 'easyconnect',
    label: 'EasyConnect',
    icon: Icons.hub_outlined,
    implemented: false,
  ),
  openVpn(
    id: 'openvpn',
    label: 'OpenVPN',
    icon: Icons.lock_open_outlined,
    implemented: false,
  );

  const ShuProtocol({
    required this.id,
    required this.label,
    required this.icon,
    required this.implemented,
  });

  /// 持久化与路由里使用的稳定标识。
  final String id;

  /// 界面上的名字。
  final String label;

  /// 设置页与协议选择条上的图标。
  final IconData icon;

  /// 本应用里**有没有**这条可执行的连接路径。
  ///
  /// 为 false 时开关不可点、也不会被自动选中 —— 但是**照样显示**，
  /// 见类注释里那段理由。
  final bool implemented;

  static ShuProtocol? fromId(String? id) {
    for (final protocol in values) {
      if (protocol.id == id) return protocol;
    }
    return null;
  }
}
