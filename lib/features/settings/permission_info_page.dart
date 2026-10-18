import 'package:flutter/material.dart';

import '../../app/shuyo_text_styles.dart';
import '../../app/theme.dart';
import '../../widgets/shu_app_bar.dart';

/// 关于页「隐私与声明 → 权限说明」进来的地方。
///
/// 版式照 `ShuYo` 的 `_PermissionInfoPage`：页首一句总说明，下面一行一项权限
/// （图标 + 名称 + 一句用途），页尾一句收尾。
///
/// 说明桌面校园分流与网络访问所需权限。
class PermissionInfoPage extends StatelessWidget {
  const PermissionInfoPage({super.key});

  @override
  Widget build(BuildContext context) {
    final colors = context.shuyoColors;
    final platform = Theme.of(context).platform;
    return Scaffold(
      appBar: ShuAppBar(
        title: '权限说明',
        onBack: () => Navigator.of(context).pop(),
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(20, 16, 20, 32),
        children: [
          Text(
            '为了实现对应功能，ShuVPN 可能会在你使用功能时申请以下权限。'
            '具体项目会因系统版本而异。',
            style: ShuYoTextStyles.bodyCompact(color: colors.textSecondary),
          ),
          const SizedBox(height: 18),
          _PermissionItem(
            icon: Icons.admin_panel_settings_outlined,
            title: 'VPN 权限',
            body: platform == TargetPlatform.windows
                ? '启用 VPN 时通过 Windows 管理员确认，创建虚拟网卡并设置所选模式的捕获路由。'
                : '安装桌面权限组件并授权当前用户后，由组件创建虚拟网卡并设置所选模式的捕获路由。'
                      '应用界面以普通用户运行。',
          ),
          const _PermissionItem(
            icon: Icons.language,
            title: '网络访问',
            body: '用于连接网关、校验校园账户与建立隧道。',
          ),
          const SizedBox(height: 8),
          Text(
            'HTTP 与 SOCKS5 代理无需 VPN 权限。'
            '关闭 VPN 或退出应用时，会撤销本次建立的虚拟网卡与路由。',
            style: ShuYoTextStyles.meta(color: colors.textMuted, height: 1.5),
          ),
        ],
      ),
    );
  }
}

/// 权限列表里的一行。
class _PermissionItem extends StatelessWidget {
  const _PermissionItem({
    required this.icon,
    required this.title,
    required this.body,
  });

  final IconData icon;
  final String title;
  final String body;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      leading: Icon(icon),
      title: Text(title),
      subtitle: Padding(
        padding: const EdgeInsets.only(top: 4),
        child: Text(body),
      ),
    );
  }
}
