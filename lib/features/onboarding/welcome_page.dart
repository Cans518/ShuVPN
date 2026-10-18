import 'dart:io';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import '../../app/app_info.dart';
import '../../app/page_transitions.dart';
import '../../app/shu_launch_surface.dart';
import '../../app/shuyo_text_styles.dart';
import '../../app/theme.dart';
import '../../core/auth/auth_constants.dart';
import '../../core/settings/settings_store.dart';
import '../../widgets/settings_save.dart';
import '../../widgets/shu_surfaces.dart';
import '../account/login_form.dart';

/// 桌面首启引导：应用介绍、按需网络权限说明和校园账户登录。
class ShuWelcomePage extends StatefulWidget {
  const ShuWelcomePage({super.key});

  /// 前进的动画时长与曲线，与 `ShuYo` 的 `_continue` 逐字一致。
  static const Duration forwardDuration = Duration(milliseconds: 320);
  static const Duration backwardDuration = Duration(milliseconds: 280);
  static const Curve pageCurve = Curves.easeOutCubic;

  static String permissionSubtitle() => '连接时按需申请系统网络权限';

  @override
  State<ShuWelcomePage> createState() => _ShuWelcomePageState();
}

class _ShuWelcomePageState extends State<ShuWelcomePage> {
  final _pages = PageController();
  final _loginFormKey = GlobalKey<ShuLoginFormState>();

  int _page = 0;

  /// 是否已经把这一页换成登录表单（第 2 页的状态，不是第 4 页）。
  bool _signingIn = false;

  /// 翻页期间禁用按钮与返回键。
  bool _busy = false;

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  // ------------------------------------------------------------------ 流程

  Future<void> _advance() async {
    if (_busy) return;
    switch (_page) {
      case 0:
        await _open(1);
      case 1:
        await _open(2);
      default:
        _setSigningIn(true);
    }
  }

  Future<void> _back() async {
    if (_busy) return;
    if (_signingIn) {
      _setSigningIn(false);
      return;
    }
    if (_page == 0) return;
    await _open(_page - 1, forward: false);
  }

  /// 三页 ⇄ 登录表单。
  ///
  /// 这里只切**逻辑**状态 —— 视觉上的滑动由 [ShuSharedAxisXSwitcher] 演，位置
  /// 由它自己算。分开是必要的：返回键与按钮要在按下的那一刻就切过去，不能等
  /// 动画。
  void _setSigningIn(bool value) {
    if (_signingIn == value) return;
    if (!value) {
      // 表单不再被卸载（见 [ShuSharedAxisXSwitcher] 的类文档），所以焦点得自己
      // 收：否则键盘会跟着一块被藏起来的输入框留在屏幕上。
      FocusManager.instance.primaryFocus?.unfocus();
    }
    _loginFormKey.currentState?.reset();
    setState(() => _signingIn = value);
  }

  Future<void> _open(int page, {bool forward = true}) async {
    setState(() {
      _busy = true;
    });
    await _pages.animateToPage(
      page,
      duration: forward
          ? ShuWelcomePage.forwardDuration
          : ShuWelcomePage.backwardDuration,
      curve: ShuWelcomePage.pageCurve,
    );
    if (!mounted) return;
    setState(() {
      _page = page;
      _busy = false;
    });
  }

  /// 登录成功。写标记 → 回首页。
  ///
  /// 写标记**必须在 `go` 之前**：路由的 `redirect` 会读它，「还没写完就跳」
  /// 会被原地弹回这一页；必须等后端确认保存成功。
  Future<void> _finishLogin() async {
    if (!await saveShuSetting(
          context,
          context.read<SettingsStore>(),
          SettingKey.welcomeCompleted,
          true,
        ) ||
        !mounted) {
      return;
    }
    _loginFormKey.currentState?.reset();
    if (!mounted) return;
    context.go('/connect');
  }

  // ------------------------------------------------------------------ 视图

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Column(
          children: [
            _topBar(context),
            Expanded(
              // 登录表单**滑进来盖在**三页之上，而不是把 `PageView` 换掉。
              //
              // 换掉（`_signingIn ? form : PageView`）看起来更直白，但会毁掉
              // 「现在在第几页」这件事：`PageView` 一旦被移出树，控制器的
              // position 就没了，放回来时新的一份从 `initialPage`（0）重新
              // 开始，而 `_page` 还停在 2 —— 底部按钮说的是一页、内容是另一页。
              //
              // 按钮与三页装在**同一块**里，登录表单占的是整块（连按钮那一条
              // 也算）—— 两边高度因此完全一样，换页过程中布局一动不动，只是
              // 整块往左让出去。
              child: ShuSharedAxisXSwitcher(
                showFront: _signingIn,
                front: _loginForm(),
                back: Column(
                  children: [
                    Expanded(
                      child: PageView(
                        controller: _pages,
                        // 翻页统一通过底部按钮和返回键。
                        physics: const NeverScrollableScrollPhysics(),
                        children: [
                          _welcome(context),
                          _permission(context),
                          _login(context),
                        ],
                      ),
                    ),
                    _footer(context),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// 顶部：只有一条返回键，没有标题。
  ///
  /// 不标题化是有意的 —— 标题就在下面几十像素处的大字里（每页一个），
  /// 顶上再写一遍等于同一句话说两遍。登录状态下这里也管返回（退回第 2 页），
  /// 所以它与 `ShuAppBar` 的 `onBack` 语义一致。
  Widget _topBar(BuildContext context) {
    return SizedBox(
      height: 56,
      child: Row(
        children: [
          if (_page > 0)
            IconButton(
              tooltip: '返回',
              onPressed: _busy ? null : _back,
              icon: const Icon(Icons.arrow_back),
            )
          else
            // 槽位**无论有没有东西都占满**：否则第 0 页的内容会比第 1 页
            // 宽 48，翻页时下面的图标与文字会横跳。
            const SizedBox(width: 56),
        ],
      ),
    );
  }

  Widget _loginForm() => ShuLoginForm(
    key: _loginFormKey,
    onChanged: () => setState(() {}),
    onCompleted: _finishLogin,
  );

  // --------------------------------------------------------------- 第 0 页

  Widget _welcome(BuildContext context) => _layout(
    context,
    header: _header(context, title: '欢迎使用 ${ShuAppInfo.name}'),
    children: [
      _feature(Icons.shield_moon_outlined, '加密隧道', '校园流量经学校网关转发'),
      _feature(Icons.lan_outlined, '校内资源直连', '在校外也能访问图书馆与内网站点'),
      _feature(Icons.badge_outlined, '统一身份认证', '使用上海大学校园账户登录'),
    ],
    pageFooter: _disclaimer(context),
  );

  // --------------------------------------------------------------- 第 1 页

  Widget _permission(BuildContext context) => _layout(
    context,
    header: _header(
      context,
      title: '权限管理',
      subtitle: ShuWelcomePage.permissionSubtitle(),
    ),
    children: [
      _paragraph(
        Platform.isWindows
            ? '启用校园分流时，Windows 会请求管理员确认。'
                  '仅使用 HTTP 或 SOCKS5 代理时无需此项授权。'
            : '启用校园分流前，请安装桌面权限组件并授权当前用户。'
                  '仅使用 HTTP 或 SOCKS5 代理时无需此项授权。',
      ),
    ],
  );

  // --------------------------------------------------------------- 第 2 页

  /// 第 2 页：登录前的数据说明 + 将要连上的系统清单。
  ///
  /// **正文与承诺分开写**：正文说「这次登录做什么」（账户信息交给谁、换回来
  /// 什么、拿去干什么），承诺单独交给 [pageFooter] —— 它固定停在按钮**上方**
  /// 的一格，与第 0 页那行免责声明是同一个位置、同一种语气。分开的理由是这两
  /// 件事的读法不同：前者要读懂，后者看一眼就够了。
  ///
  /// 都用 Apple 的「数据与隐私」句式：一句一件事，主语在每个分句里点明
  /// （谁做的、对谁做），读的人不用回头猜。
  ///
  /// 正文下面那张清单是**将要连上的系统**，与「账户管理」里那三行同一条定义
  /// （[ShuSystemTile]）—— 图标、名字、域名逐字一致：用户登录后在那一页看到的
  /// 东西，在这里已经认识过一遍。
  Widget _login(BuildContext context) => _layout(
    context,
    header: _header(context, title: '登录校园账户', subtitle: '使用上海大学统一认证系统'),
    children: [
      _paragraph(
        '您的上海大学校园账户信息将交给上海大学，用于让您安全登录并访问校内资源。'
        'ShuVPN 凭这次授权向各业务系统换取凭据，用于建立隧道。',
      ),
      for (final target in ShuOAuthTargets.visible)
        ShuSystemTile(kind: target.kind),
    ],
    pageFooter: _privacyNote(context),
  );

  // ------------------------------------------------------------- 公共零件

  /// 一页的骨架：可滚动的正文 + 可选的页脚。
  ///
  /// 正文放进 `SingleChildScrollView` 是因为小屏 + 大字体下这三页都会溢出，
  /// 而按钮**必须固定在底部** —— 它跟着内容滚走的话，最后一屏会看不到「继续」。
  Widget _layout(
    BuildContext context, {
    required Widget header,
    required List<Widget> children,
    Widget? pageFooter,
  }) {
    return Column(
      children: [
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.fromLTRB(24, 8, 24, 12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [header, const SizedBox(height: 28), ...children],
            ),
          ),
        ),
        if (pageFooter != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 0, 24, 8),
            child: pageFooter,
          ),
      ],
    );
  }

  Widget _header(
    BuildContext context, {
    required String title,
    String? subtitle,
  }) {
    final colors = context.shuyoColors;
    return Center(
      child: Column(
        children: [
          Image.asset(
            ShuLaunchSurface.iconAssetFor(colors.brightness),
            width: 88,
            height: 88,
            fit: BoxFit.contain,
            filterQuality: FilterQuality.high,
          ),
          const SizedBox(height: 12),
          Text(
            title,
            textAlign: TextAlign.center,
            // 26 是这套版式里的「大标题」：比 `headerTitle`(17.5) 大一档半，
            // 与 ShuYo 那边 28 的观感一致（那边用的是 `headlineSmall`）。
            style: ShuYoTextStyles.title(
              color: colors.textPrimary,
              size: 26,
              weight: FontWeight.w500,
            ),
          ),
          if (subtitle != null) ...[
            const SizedBox(height: 8),
            Text(
              subtitle,
              textAlign: TextAlign.center,
              style: ShuYoTextStyles.bodyCompact(color: colors.textTertiary),
            ),
          ],
        ],
      ),
    );
  }

  Widget _feature(IconData icon, String title, String description) {
    return Padding(
      padding: const EdgeInsets.only(left: 32, bottom: 8),
      child: ConstrainedBox(
        constraints: const BoxConstraints(minHeight: 76),
        child: Row(
          children: [
            Icon(icon, size: 26),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: ShuYoTextStyles.title(
                      color: context.shuyoColors.textPrimary,
                      size: 17,
                      weight: FontWeight.w500,
                    ),
                  ),
                  Text(
                    description,
                    style: ShuYoTextStyles.bodyCompact(
                      color: context.shuyoColors.textSecondary,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _paragraph(String text) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: Text(
      text,
      textAlign: TextAlign.center,
      style: ShuYoTextStyles.bodyCompact(
        color: context.shuyoColors.textSecondary,
      ).copyWith(height: 1.5),
    ),
  );

  Widget _disclaimer(BuildContext context) => Center(
    child: Text(
      '${ShuAppInfo.disclaimer}账号与密码只发往学校认证服务器。',
      textAlign: TextAlign.center,
      style: ShuYoTextStyles.meta(color: context.shuyoColors.textMuted),
    ),
  );

  /// 第 2 页页脚那行 —— 它固定停在底部按钮**上方**的那一格，与 `_layout` 的
  /// `pageFooter` 语义一致（第 0 页用的是免责声明）。
  ///
  /// 它说的是**结果**（这些信息最后怎么样了），而正文说的是**过程**（这次登录
  /// 会发生什么）。两件事分开读更清楚：过程要读懂，结果看一眼就够。
  Widget _privacyNote(BuildContext context) => Center(
    child: Text(
      '登录会话与账户展示状态会保存在本机。账号与密码仅用于学校认证。',
      textAlign: TextAlign.center,
      style: ShuYoTextStyles.meta(color: context.shuyoColors.textMuted),
    ),
  );

  Widget _footer(BuildContext context) {
    final blocked = _busy;
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 0, 24, 16),
      child: FilledButton(
        onPressed: blocked ? null : _advance,
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(52),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
        child: Text(switch (_page) {
          0 || 1 => '继续',
          _ => '去登录',
        }),
      ),
    );
  }
}
