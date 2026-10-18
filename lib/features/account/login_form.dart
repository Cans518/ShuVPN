import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../app/shuyo_text_styles.dart';
import '../../app/theme.dart';
import '../../core/account/account_center.dart';
import '../../core/auth/auth_exception.dart';
import '../../core/auth/login_challenge.dart';
import 'wecom_scan_page.dart';

/// 「上大校园账户」登录表单。
///
/// 由 [AccountPage] 复用，本身**不是**一个完整的页面 —— 没有 `Scaffold`、
/// 没有 `AppBar`：标题栏由宿主页面提供。
///
/// 布局逐项照抄 ShuYo 的 `NativeLoginPage`：
///   `ListView(padding: EdgeInsets.all(24))`
///   `titleLarge` + w700 的标题，紧跟一行次级色说明
///   输入框，**提交按钮就在表单里**（不是外挂底栏）
///   企业微信按钮与说明文字依次跟在下面
///
/// 两个步骤共用一张表单：
///   0 → 用户名/学号 + 密码 →「继续」+「使用企业微信登录」
///   1 → 两步验证 → 方式切换 + 发送验证码 + 6 位码 →「完成验证」
class ShuLoginForm extends StatefulWidget {
  const ShuLoginForm({
    super.key,
    required this.onChanged,
    required this.onCompleted,
  });

  /// 步骤或忙碌状态变化时通知宿主，让它刷新标题栏。
  final VoidCallback onChanged;

  /// 登录成功（凭据已交换完毕）。
  final VoidCallback onCompleted;

  @override
  State<ShuLoginForm> createState() => ShuLoginFormState();
}

class ShuLoginFormState extends State<ShuLoginForm> {
  final _studentId = TextEditingController();
  final _password = TextEditingController();
  final _code = TextEditingController();
  final _credentialsKey = GlobalKey<FormState>();
  final _verificationKey = GlobalKey<FormState>();

  int _step = 0;
  bool _busy = false;
  bool _passwordVisible = false;
  String? _error;
  ShuLoginChallenge? _challenge;
  ShuVerificationMethod _method = ShuVerificationMethod.wecom;
  Timer? _countdownTimer;
  int _countdown = 0;
  int _generation = 0;
  AccountCenter? _commandOwner;

  /// 当前步骤，宿主据此决定标题栏文案。
  int get step => _step;

  bool get busy => _busy;

  @override
  void dispose() {
    _generation++;
    _cancelCommand();
    _countdownTimer?.cancel();
    _studentId.dispose();
    _password.dispose();
    _code.dispose();
    super.dispose();
  }

  AccountCenter get _account => context.read<AccountCenter>();

  bool _current(int generation) => mounted && generation == _generation;

  void _cancelCommand() {
    final owner = _commandOwner;
    _commandOwner = null;
    owner?.cancelPending();
  }

  Future<T> _command<T>(
    int generation,
    Future<T> Function(AccountCenter) send,
  ) async {
    final owner = _account;
    _commandOwner = owner;
    try {
      return await send(owner);
    } finally {
      if (generation == _generation) _commandOwner = null;
    }
  }

  // ------------------------------------------------------------- 宿主接口

  /// 回退一步。已经在第一步时返回 `false`，宿主据此决定是返回上一页。
  bool back() {
    if (_step == 0 || _busy) return false;
    _countdownTimer?.cancel();
    setState(() {
      _step = 0;
      _countdown = 0;
      _error = null;
    });
    widget.onChanged();
    return true;
  }

  /// 离开时清空输入，避免下次进来还留着上一个学号的密码。
  void reset() {
    _generation++;
    _cancelCommand();
    _countdownTimer?.cancel();
    _password.clear();
    _code.clear();
    if (!mounted) return;
    setState(() {
      _step = 0;
      _challenge = null;
      _countdown = 0;
      _error = null;
      _busy = false;
    });
    widget.onChanged();
  }

  // ------------------------------------------------------------------ 视图

  @override
  Widget build(BuildContext context) {
    final progress = context.watch<AccountCenter>().progressLabel;
    return Column(
      children: [
        if (_busy)
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 16, 24, 0),
            child: Text(progress ?? '正在处理，请稍候', textAlign: TextAlign.center),
          ),
        Expanded(child: _form()),
      ],
    );
  }

  Widget _form() {
    return AnimatedSwitcher(
      duration: const Duration(milliseconds: 260),
      child: _step == 0 ? _credentials() : _verification(),
    );
  }

  Widget _credentials() => Form(
    key: _credentialsKey,
    child: ListView(
      key: const ValueKey('credentials'),
      padding: const EdgeInsets.all(24),
      children: [
        Text(
          '登录校园账户',
          style: Theme.of(context).textTheme.titleLarge
              ?.copyWith(fontWeight: FontWeight.w700),
        ),
        const SizedBox(height: 8),
        Text(
          '使用上海大学统一认证系统',
          style: TextStyle(color: context.shuyoColors.textTertiary),
        ),
        const SizedBox(height: 28),
        TextFormField(
          controller: _studentId,
          enabled: !_busy,
          keyboardType: TextInputType.text,
          autofillHints: const [AutofillHints.username],
          textInputAction: TextInputAction.next,
          decoration: const InputDecoration(
            labelText: '用户名/学号',
            prefixIcon: Icon(Icons.badge_outlined),
          ),
          validator: (value) => value?.trim().isEmpty == true ? '请输入学号' : null,
        ),
        const SizedBox(height: 16),
        TextFormField(
          controller: _password,
          enabled: !_busy,
          obscureText: !_passwordVisible,
          autofillHints: const [AutofillHints.password],
          onFieldSubmitted: (_) => _submitCredentials(),
          decoration: InputDecoration(
            labelText: '密码',
            prefixIcon: const Icon(Icons.lock_outline),
            suffixIcon: IconButton(
              tooltip: _passwordVisible ? '隐藏密码' : '显示密码',
              onPressed: () =>
                  setState(() => _passwordVisible = !_passwordVisible),
              icon: Icon(
                _passwordVisible
                    ? Icons.visibility_off_outlined
                    : Icons.visibility_outlined,
              ),
            ),
          ),
          validator: (value) => value?.isEmpty == true ? '请输入密码' : null,
        ),
        const SizedBox(height: 28),
        FilledButton(
          onPressed: _busy ? null : _submitCredentials,
          style: FilledButton.styleFrom(minimumSize: const Size.fromHeight(50)),
          child: _buttonContent('继续'),
        ),
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: _busy ? null : _startWeComLogin,
          icon: const Icon(Icons.qr_code_scanner_outlined),
          label: const Text('使用企业微信登录'),
          style: OutlinedButton.styleFrom(
            minimumSize: const Size.fromHeight(50),
          ),
        ),
        const SizedBox(height: 8),
        Text(
          '使用企业微信扫码登录，可在手机企业微信中确认登录。',
          textAlign: TextAlign.center,
          style: TextStyle(
            fontSize: 12,
            color: context.shuyoColors.textTertiary,
          ),
        ),
        ?_errorLine,
      ],
    ),
  );

  Widget _verification() {
    final challenge = _challenge;
    if (challenge == null) return const SizedBox.shrink();
    final methods = challenge.methods;
    return Form(
      key: _verificationKey,
      child: ListView(
        key: const ValueKey('verification'),
        padding: const EdgeInsets.all(24),
        children: [
          Text(
            '二步验证',
            style: Theme.of(context).textTheme.titleLarge
                ?.copyWith(fontWeight: FontWeight.w700),
          ),
          const SizedBox(height: 8),
          Text(
            challenge.describe(_method),
            style: TextStyle(color: context.shuyoColors.textTertiary),
          ),
          const SizedBox(height: 24),
          SegmentedButton<ShuVerificationMethod>(
            segments: [
              if (methods.containsKey(ShuVerificationMethod.wecom))
                const ButtonSegment(
                  value: ShuVerificationMethod.wecom,
                  label: Text('企业微信'),
                  icon: Icon(Icons.business_center_outlined),
                ),
              if (methods.containsKey(ShuVerificationMethod.sms))
                const ButtonSegment(
                  value: ShuVerificationMethod.sms,
                  label: Text('手机号'),
                  icon: Icon(Icons.sms_outlined),
                ),
            ],
            selected: {_method},
            onSelectionChanged: _busy
                ? null
                : (value) => _selectMethod(value.first),
          ),
          const SizedBox(height: 16),
          OutlinedButton.icon(
            onPressed: _busy || _countdown > 0 ? null : _sendCode,
            icon: const Icon(Icons.send_outlined),
            label: Text(_countdown > 0 ? '$_countdown s 后可重新发送' : '发送验证码'),
          ),
          const SizedBox(height: 16),
          TextFormField(
            controller: _code,
            enabled: !_busy,
            keyboardType: TextInputType.number,
            maxLength: 6,
            textInputAction: TextInputAction.done,
            onFieldSubmitted: (_) => _verifyCode(),
            decoration: const InputDecoration(
              labelText: '验证码',
              prefixIcon: Icon(Icons.password_outlined),
            ),
            validator: (value) =>
                value?.trim().length == 6 ? null : '请输入 6 位验证码',
          ),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: _busy ? null : _verifyCode,
            style: FilledButton.styleFrom(
              minimumSize: const Size.fromHeight(50),
            ),
            child: _buttonContent('完成验证'),
          ),
          ?_errorLine,
        ],
      ),
    );
  }

  /// 请求进行中时把按钮文字换成进度圈，避免看起来没响应。
  Widget _buttonContent(String label) {
    if (!_busy) return Text(label);
    return const SizedBox.square(
      dimension: 20,
      child: CircularProgressIndicator(strokeWidth: 2),
    );
  }

  Widget? get _errorLine {
    final message = _error;
    if (message == null) return null;
    final colors = context.shuyoColors;
    return Padding(
      padding: const EdgeInsets.only(top: 14),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.error_outline, size: 16, color: colors.danger),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              message,
              style: ShuYoTextStyles.meta(color: colors.danger),
            ),
          ),
        ],
      ),
    );
  }

  // ------------------------------------------------------------------ 动作

  Future<void> _submitCredentials() async {
    if (_busy || _credentialsKey.currentState?.validate() != true) return;
    final generation = ++_generation;
    final username = _studentId.text.trim();
    final password = _password.text;
    _password.clear();
    setState(() {
      _busy = true;
      _error = null;
    });
    widget.onChanged();
    try {
      final outcome = await _command(
        generation,
        (account) => account.signIn(username: username, password: password),
      );
      if (!_current(generation)) return;
      switch (outcome.status) {
        case ShuSignInStatus.success:
          widget.onCompleted();
        case ShuSignInStatus.needsVerification:
          final challenge = outcome.challenge;
          if (challenge == null) return;
          setState(() {
            _challenge = challenge;
            _method = challenge.selectedMethod;
            _countdown = challenge.resendAfterSeconds;
            _step = 1;
          });
          _watchChallenge();
        case ShuSignInStatus.failed:
          setState(() => _error = _account.errorMessage ?? '登录失败，请重试');
      }
    } on ShuAuthException catch (error) {
      if (_current(generation)) setState(() => _error = error.message);
    } on Object {
      if (_current(generation)) setState(() => _error = '无法连接学校认证服务，请稍后再试');
    } finally {
      if (_current(generation)) {
        setState(() => _busy = false);
        widget.onChanged();
      }
    }
  }

  Future<void> _startWeComLogin() async {
    if (_busy) return;
    final generation = ++_generation;
    setState(() {
      _busy = true;
      _error = null;
    });
    widget.onChanged();
    try {
      final completed = await Navigator.of(context)
          .push<bool>(MaterialPageRoute(builder: (_) => const WeComScanPage()));
      if (completed != true || !_current(generation)) return;
      widget.onCompleted();
    } finally {
      if (_current(generation)) {
        setState(() => _busy = false);
        widget.onChanged();
      }
    }
  }

  Future<void> _sendCode() async {
    if (_busy || _countdown > 0) return;
    final generation = ++_generation;
    setState(() {
      _busy = true;
      _error = null;
    });
    widget.onChanged();
    try {
      await _command(generation, (account) => account.sendCode(_method));
      if (!mounted || generation != _generation) return;
      _refreshChallenge();
      ScaffoldMessenger.of(context)
        ..hideCurrentSnackBar()
        ..showSnackBar(const SnackBar(content: Text('验证码已发送')));
    } on ShuAuthException catch (error) {
      if (_current(generation)) setState(() => _error = error.message);
    } on Object {
      if (_current(generation)) setState(() => _error = '验证码发送失败，请稍后重试');
    } finally {
      if (_current(generation)) {
        _refreshChallenge();
        setState(() => _busy = false);
        widget.onChanged();
      }
    }
  }

  Future<void> _verifyCode() async {
    if (_busy || _verificationKey.currentState?.validate() != true) return;
    final generation = ++_generation;
    setState(() {
      _busy = true;
      _error = null;
    });
    widget.onChanged();
    try {
      final ok = await _command(
        generation,
        (account) => account.submitCode(_method, _code.text.trim()),
      );
      if (!_current(generation)) return;
      if (ok) {
        _countdownTimer?.cancel();
        _code.clear();
        widget.onCompleted();
        return;
      }
      setState(() => _error = _account.errorMessage ?? '验证失败，请重试');
    } on ShuAuthException catch (error) {
      if (_current(generation)) setState(() => _error = error.message);
    } on Object {
      if (_current(generation)) setState(() => _error = '验证失败，请检查网络后重试');
    } finally {
      if (_current(generation)) {
        _refreshChallenge();
        setState(() => _busy = false);
        widget.onChanged();
      }
    }
  }

  void _selectMethod(ShuVerificationMethod method) {
    if (!mounted) return;
    try {
      _account.selectVerificationMethod(method);
      _refreshChallenge();
    } on ShuAuthException catch (error) {
      setState(() => _error = error.message);
    }
  }

  void _watchChallenge() {
    _countdownTimer?.cancel();
    _countdownTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!_busy) _refreshChallenge();
    });
  }

  void _refreshChallenge() {
    if (!mounted || _step != 1) return;
    final challenge = _account.loginChallenge;
    if (challenge == null) {
      _code.clear();
      _countdownTimer?.cancel();
      setState(() {
        _step = 0;
        _challenge = null;
        _countdown = 0;
      });
      widget.onChanged();
      return;
    }
    setState(() {
      _challenge = challenge;
      _method = challenge.selectedMethod;
      _countdown = challenge.resendAfterSeconds;
    });
  }
}
