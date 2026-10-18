import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/connection/connection_controller.dart';
import '../../core/connection/proxy_listen.dart';
import '../../core/settings/settings_store.dart';
import '../../widgets/settings_save.dart';
import '../../widgets/settings_rows.dart';
import '../../widgets/settings_scaffold.dart';
import '../../widgets/shu_surfaces.dart';

/// 桌面网络设置：HTTP/SOCKS5 代理、系统代理与校园分流。
///
/// 运行期间锁定监听和接管参数，断开后才允许修改。
class ShuConnectionSettingsPage extends StatefulWidget {
  const ShuConnectionSettingsPage({super.key});

  @override
  State<ShuConnectionSettingsPage> createState() =>
      _ShuConnectionSettingsPageState();
}

/// 两条本机代理通道。
///
/// 它们的设置项完全同形（开关 / 监听地址 / 端口），只有取值来源不同 ——
/// 用一个枚举把「同形」写进类型里，比把同一段 UI 复制两遍少一半错误。
/// 它不出现在任何选择器里：这是同一件事的两个实例，不是给用户挑的选项。
enum _ProxyChannel {
  http('HTTP', Icons.public),
  socks('SOCKS5', Icons.settings_ethernet);

  const _ProxyChannel(this.label, this.icon);

  final String label;
  final IconData icon;

  bool enabledIn(SettingsStore settings) =>
      this == http ? settings.httpProxyEnabled : settings.socksProxyEnabled;

  ShuProxyListen listenIn(SettingsStore settings) =>
      this == http ? settings.httpListen : settings.socksListen;

  int portIn(SettingsStore settings) =>
      this == http ? settings.httpPort : settings.socksPort;
}

class _ShuConnectionSettingsPageState extends State<ShuConnectionSettingsPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final connection = context.read<ConnectionController>();
      if (connection.desktopProxySettingsVisible) {
        unawaited(connection.refreshDesktopTun());
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final settings = context.watch<SettingsStore>();
    final connection = context.watch<ConnectionController>();
    // 隧道在跑时整页封住。判据是 `tunnelUp` 而不是某个数据面的运行标志：
    // VPN 没开、只有代理在跑时，改 DNS / MTU 同样没有意义 —— 那些值要等
    // 下一次建接口才被读。授权状态那一行例外（见下）：它是只读的观测。
    final locked = connection.tunnelUp || connection.cleanupPending;
    final editable = !locked && !connection.busy;
    return ShuSettingsSubPage(
      title: '网络连接',
      banner: locked ? const ShuNoticeBar(shuSettingsLockedNotice) : null,
      children: [
        if (connection.desktopProxySettingsVisible) ...[
          const SectionHeader(title: '桌面系统代理'),
          SettingsSwitchRow(
            icon: Icons.desktop_windows_outlined,
            title: '连接时启用系统代理',
            value: settings.systemProxyEnabled,
            enabled:
                editable &&
                (connection.systemProxyAvailable ||
                    settings.systemProxyEnabled),
            onChanged: (value) => saveShuSetting(
              context,
              settings,
              SettingKey.systemProxyEnabled,
              value,
            ),
          ),
          SettingsRow(
            icon: Icons.info_outline,
            title: '系统代理状态',
            value: connection.systemProxyRecoveryError != null
                ? '恢复失败'
                : connection.systemProxyRunning
                ? '已启用'
                : connection.systemProxyAvailable
                ? '未启用'
                : '当前桌面不支持',
            subtitle:
                connection.systemProxyRecoveryError ??
                (connection.systemProxyAvailable
                    ? '仅支持系统代理的应用会使用此连接；断开后恢复原设置'
                    : connection.systemProxyUnavailableReason),
          ),
        ],
        for (final channel in _ProxyChannel.values) ...[
          SectionHeader(title: '${channel.label} 代理'),
          SettingsSwitchRow(
            icon: channel.icon,
            title: '启用 ${channel.label} 代理',
            value: channel.enabledIn(settings),
            enabled: editable,
            onChanged: (value) =>
                _setProxyEnabled(settings, connection, channel, value),
          ),
          SettingsRow(
            icon: Icons.lan_outlined,
            title: '${channel.label} 监听地址',
            value: channel.listenIn(settings).address,
            enabled: editable,
            onTap: () => _editListen(settings, connection, channel),
          ),
          SettingsRow(
            icon: Icons.numbers,
            title: '${channel.label} 代理端口',
            value: _portLabel(channel.portIn(settings)),
            enabled: editable,
            onTap: () => _editPort(settings, connection, channel),
          ),
        ],

        if (connection.desktopProxySettingsVisible) ...[
          const SectionHeader(title: '桌面 VPN'),
          SettingsSwitchRow(
            icon: Icons.vpn_lock_outlined,
            title: '连接时启用 VPN',
            value: settings.vpnEnabled,
            enabled:
                editable && (connection.vpnSupported || settings.vpnEnabled),
            onChanged: (value) => _toggleVpn(settings, connection, value),
          ),
          SettingsRow(
            icon: Icons.filter_alt_outlined,
            title: 'VPN 捕获模式',
            value: _captureLabel(settings.vpnCaptureMode),
            subtitle: '断开后可修改；完整捕获仍按规则选择校园、直连或拒绝',
            enabled: editable,
            onTap: () => _chooseCaptureMode(settings),
          ),
          SettingsRow(
            icon: Icons.info_outline,
            title: 'VPN 状态',
            value: connection.desktopTunInspecting
                ? '读取中'
                : connection.cleanupPending
                ? '待清理'
                : connection.vpnRunning
                ? _captureLabel(connection.desktopTunStatus.captureMode)
                : connection.vpnSupported
                ? '未启用'
                : '暂不可用',
            subtitle:
                connection.desktopTunError ??
                connection.desktopTunEnvironment?.reason ??
                '校园分流仅接管校园目标；完整捕获接管可路由单播并转发 DNS',
          ),
          SettingsRow(
            icon: Icons.lan_outlined,
            title: '本次底层网络',
            value: connection.desktopPhysicalInterface.isEmpty
                ? '自动选择'
                : connection.desktopPhysicalInterface,
            subtitle: '自动模式使用系统默认出口；关闭应用后恢复自动',
            enabled: editable && !connection.desktopTunInspecting,
            onTap: () => _chooseDesktopInterface(connection),
          ),
          SettingsRow(
            icon: Icons.refresh,
            title: '刷新网络与权限',
            value: connection.desktopTunInspecting ? '读取中' : null,
            enabled: !connection.desktopTunInspecting && !connection.busy,
            onTap: () => connection.refreshDesktopTun(),
          ),
          SettingsRow(
            icon: Icons.dns_outlined,
            title: '本次 DNS 上游',
            value: connection.desktopPhysicalDns.isEmpty
                ? '自动读取'
                : connection.desktopPhysicalDns,
            subtitle: '底层网络只有本地 DNS 代理时，请填明确上游 IP；关闭应用后恢复自动',
            enabled: editable,
            onTap: () => _chooseDesktopDns(connection),
          ),
          SettingsRow(
            icon: Icons.route_outlined,
            title: '查看接管计划',
            value: connection.desktopTunPreviewLoading ? '准备中' : null,
            subtitle: connection.tunnelUp
                ? '查看原生确认的模式、初始路由与排除地址；运行中显示当前计划'
                : '连接校园账户通道后可查看',
            enabled:
                connection.tunnelUp &&
                !connection.desktopTunPreviewLoading &&
                !connection.busy,
            onTap: () => _showDesktopPlan(connection),
          ),
        ],
        SettingsRow(
          icon: Icons.straighten,
          title: 'MTU',
          value: '${settings.vpnMtu}',
          enabled: editable,
          onTap: () => _pickMtu(settings),
        ),
      ],
    );
  }

  static String _captureLabel(CaptureMode mode) => switch (mode) {
    CaptureMode.campus => '校园分流',
    CaptureMode.full => '完整捕获（实验性）',
  };

  Future<void> _chooseCaptureMode(SettingsStore settings) async {
    final next = await showShuChoiceSheet<CaptureMode>(
      context: context,
      title: 'VPN 捕获模式',
      current: settings.vpnCaptureMode,
      options: const [
        ShuChoice(CaptureMode.campus, '校园分流', '默认；校园网段及已解析校园域名进入规则，其余流量不受接管'),
        ShuChoice(
          CaptureMode.full,
          '完整捕获（实验性）',
          'IPv4/IPv6 单播和 DNS 进入规则；保留本机、控制节点、链路本地与组播，网络覆盖变化时停止并恢复',
        ),
      ],
    );
    if (next != null && mounted) {
      final connection = context.read<ConnectionController>();
      if (connection.busy || connection.tunnelUp || connection.cleanupPending) {
        return;
      }
      await saveShuSetting(
        context,
        settings,
        SettingKey.vpnCaptureMode,
        next.name,
      );
    }
  }

  Future<void> _chooseDesktopDns(ConnectionController connection) async {
    final value = await showShuTextPrompt(
      context: context,
      title: '本次 DNS 上游',
      label: '上游 IP，留空自动读取',
      initial: connection.desktopPhysicalDns,
      validate: connection.validateDesktopDns,
    );
    if (value != null && mounted) connection.selectDesktopDns(value);
  }

  Future<void> _chooseDesktopInterface(ConnectionController connection) async {
    final networks = connection.desktopTunEnvironment?.networks ?? const [];
    final value = await showShuChoiceSheet<String>(
      context: context,
      title: '本次底层网络',
      current: connection.desktopPhysicalInterface,
      options: [
        const ShuChoice('', '自动选择', '按当前系统默认出口选择'),
        for (final network in networks)
          ShuChoice(
            network.name,
            '${network.name}${network.recommended ? " · 系统默认" : ""}',
            network.description,
          ),
      ],
    );
    if (value != null && mounted) connection.selectDesktopInterface(value);
  }

  Future<void> _showDesktopPlan(ConnectionController connection) async {
    await connection.previewDesktopTun();
    if (!mounted) return;
    final preview = connection.desktopTunPreview;
    if (preview == null) return;
    await showDialog<void>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('VPN 接管计划'),
        content: SizedBox(
          width: 520,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('捕获模式：${_captureLabel(preview.captureMode)}'),
                Text('实际 MTU：${preview.mtu}'),
                Text('虚拟地址：${preview.addresses.join("、")}'),
                Text(
                  preview.captureMode == CaptureMode.full
                      ? 'DNS 接管：启用，校园名称走校园 DNS，其余使用物理上游'
                      : preview.capturesCampusDns
                      ? '校园 DNS 映射：启用'
                      : '校园 DNS 映射：未启用',
                ),
                if (preview.physicalDnsServers.isNotEmpty)
                  Text('物理 DNS 上游：${preview.physicalDnsServers.join("、")}'),
                if (preview.captureMode == CaptureMode.full)
                  const Text('不接管本机、控制节点、回环、链路本地、组播及保留广播范围'),
                if (preview.protectedAddresses.isNotEmpty)
                  Text('排除地址：${preview.protectedAddresses.join("、")}'),
                const SizedBox(height: 12),
                Text('初始接管网段：${preview.routes.length}'),
                const SizedBox(height: 8),
                SelectableText(preview.routes.join('\n')),
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('关闭'),
          ),
        ],
      ),
    );
  }

  static String _portLabel(int port) => port == 0 ? '自动' : '$port';

  /// 拨某个通道的启用开关：设置写下来，顺手把这一层拉起来或停下去。
  ///
  /// 两个通道各自处理：HTTP 端口被占不该把 SOCKS5 也关掉，反过来也一样。
  Future<void> _setProxyEnabled(
    SettingsStore settings,
    ConnectionController connection,
    _ProxyChannel channel,
    bool value,
  ) async {
    final key = channel == _ProxyChannel.http
        ? SettingKey.httpProxyEnabled
        : SettingKey.socksProxyEnabled;
    if (!await saveShuSetting(context, settings, key, value)) return;
    if (!connection.tunnelUp || connection.busy) return;
    if (value) {
      if (channel == _ProxyChannel.http) {
        await connection.startHttpProxy();
      } else {
        await connection.startSocksProxy();
      }
    } else {
      if (channel == _ProxyChannel.http) {
        await connection.stopHttpProxy();
      } else {
        await connection.stopSocksProxy();
      }
    }
  }

  /// 拨「启用 VPN 服务」。
  ///
  /// 与代理不同，这一条有个前置：**系统授权**。没授权就先把开关退回关，
  /// 并说清原因 —— 直接写进设置会让下次连接在无声中失败。
  Future<void> _toggleVpn(
    SettingsStore settings,
    ConnectionController connection,
    bool value,
  ) async {
    if (value &&
        connection.desktopProxySettingsVisible &&
        !connection.vpnSupported) {
      showShuSnack(
        context,
        connection.desktopTunEnvironment?.reason ?? '当前不能启动 VPN',
      );
      return;
    }
    if (!mounted ||
        !await saveShuSetting(
          context,
          settings,
          SettingKey.vpnEnabled,
          value,
        )) {
      return;
    }
    if (!connection.tunnelUp || connection.busy) return;
    if (value && !connection.vpnRunning) {
      await connection.startVpn();
    } else if (!value && (connection.vpnRunning || connection.cleanupPending)) {
      await connection.stopVpn();
    }
  }

  /// 填监听地址。
  ///
  /// 从两个预设放成自由填写，是因为只有「仅本机 / 所有网卡」表达不了
  /// 「只放行某一张网卡」这种需求；放开之后只想让 USB 网络共享那一台连进来
  /// 也有了写法。代价是错误输入的入口也变宽了，所以两道门都留着：
  ///
  /// 1. 通过原生设置接口校验地址，并在对话框上直接报错；
  /// 2. 任何非回环地址都要过一次确认 —— 隧道是「以你的身份进校园网」的
  ///    东西，把入口开给别的设备等于把校园账号借出去。
  ///
  /// 改完还要重绑：绑定地址是**建立监听那一刻**定下来的，不重绑就是旧值。
  /// 两个通道各绑各的，所以重绑也分开做 —— 改 HTTP 的地址不该让 SOCKS5
  /// 那一侧断一下。
  Future<void> _editListen(
    SettingsStore settings,
    ConnectionController connection,
    _ProxyChannel channel,
  ) async {
    final current = channel.listenIn(settings);
    final value = await showShuTextPrompt(
      context: context,
      title: '${channel.label} 监听地址',
      label: '绑定地址',
      initial: current.address,
      validate: (raw) =>
          settings.validationMessage(SettingKey.socksListen, raw),
    );
    if (value == null) return;
    final next = settings.parseListen(value);
    // 对话框已经校过一遍，这里只是让类型收窄 —— 多一道门不亏。
    if (next == null || next == current) return;
    if (!mounted) return;

    if (next.exposesToNetwork) {
      final confirmed = await _confirmExpose(next, channel);
      if (confirmed != true) return;
    }

    final key = channel == _ProxyChannel.http
        ? SettingKey.httpListen
        : SettingKey.socksListen;
    if (!mounted ||
        !await saveShuSetting(context, settings, key, next.address)) {
      return;
    }
    if (channel == _ProxyChannel.http) {
      await connection.rebindHttpProxy();
    } else {
      await connection.rebindSocksProxy();
    }
  }

  /// 「把入口开给别的设备」的确认框。两个通道共用同一段话术。
  Future<bool?> _confirmExpose(ShuProxyListen next, _ProxyChannel channel) {
    return showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('允许同网络的设备连入？'),
        content: Text(
          '${channel.label} 代理会监听 ${next.address}，从这个地址能连过来的'
          '设备都可以通过你的校园账号访问校园网，而且不需要密码。\n'
          '只在自己的可信网络里这样做。',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('取消'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('继续'),
          ),
        ],
      ),
    );
  }

  Future<void> _editPort(
    SettingsStore settings,
    ConnectionController connection,
    _ProxyChannel channel,
  ) async {
    final current = channel.portIn(settings);
    final key = channel == _ProxyChannel.http
        ? SettingKey.httpPort
        : SettingKey.socksPort;
    final input = await showShuTextPrompt(
      context: context,
      title: '${channel.label} 端口',
      label: '端口',
      initial: '$current',
      validate: (raw) =>
          settings.validationMessage(key, int.tryParse(raw) ?? raw),
    );
    final port = input == null ? null : int.tryParse(input);
    if (port == null || port == current) return;
    if (!mounted || !await saveShuSetting(context, settings, key, port)) return;
    // 端口和地址一样是绑定参数，改完不重绑就还是旧的 —— 而界面上已经
    // 显示新的了，那种不一致比「没改」更难查。
    if (channel == _ProxyChannel.http) {
      await connection.rebindHttpProxy();
    } else {
      await connection.rebindSocksProxy();
    }
  }

  Future<void> _pickMtu(SettingsStore settings) async {
    final next = await showShuChoiceSheet<int>(
      context: context,
      title: 'MTU',
      current: settings.vpnMtu,
      options: const [
        ShuChoice(1280, '1280', 'IPv6 下限，最保守'),
        ShuChoice(1400, '1400', '默认'),
        ShuChoice(1500, '1500', '以太网值，可能被隧道封装撑破'),
      ],
    );
    if (next != null && mounted) {
      await saveShuSetting(context, settings, SettingKey.vpnMtu, next);
    }
  }
}
