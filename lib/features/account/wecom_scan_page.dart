import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../app/shuyo_text_styles.dart';
import '../../app/theme.dart';
import '../../core/account/account_center.dart';
import '../../core/account/qr_account.dart';
import '../../core/auth/auth_exception.dart';

/// 企业微信扫码登录等待页。
///
/// 展示原生登录操作的二维码、扫码状态与凭据获取进度。
class WeComScanPage extends StatefulWidget {
  const WeComScanPage({super.key});

  @override
  State<WeComScanPage> createState() => _WeComScanPageState();
}

class _WeComScanPageState extends State<WeComScanPage> {
  QrAccount? _native;
  QrLoginImage? _nativeImage;

  QrLoginStatus _status = QrLoginStatus.waiting;
  bool _busy = true;
  bool _launchingLink = false;
  bool _credentials = false;
  bool _running = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final account = context.read<AccountCenter>();
    if (account is QrAccount) {
      _native = account as QrAccount;
    }
    unawaited(_start());
  }

  @override
  void dispose() {
    // 已完成页面的退场不能取消随后开始的新操作。
    if (_running) _native?.cancelQr();
    super.dispose();
  }

  Future<void> _start() async {
    try {
      final native = _native;
      if (native == null) {
        setState(() {
          _busy = false;
          _error = '当前账户未提供原生扫码登录，请重新启动完整应用';
        });
        return;
      }
      _running = true;
      final completed = await native.signInQr(
        onUpdate: (update) {
          if (!mounted) return;
          setState(() {
            if (update.image != null) _nativeImage = update.image;
            _busy = update.phase != QrLoginPhase.waiting;
            _credentials = update.phase == QrLoginPhase.credentials;
            if (_credentials) _nativeImage = null;
          });
        },
        onStatus: (status) {
          if (!mounted) return;
          setState(
            () => _status = status == QrLoginStatus.succeeded
                ? QrLoginStatus.confirmedPending
                : status,
          );
        },
      );
      _running = false;
      if (!mounted) return;
      if (!completed) {
        setState(() {
          _busy = false;
          _error = context.read<AccountCenter>().errorMessage ?? '凭据交换失败，请重新登录';
        });
        return;
      }
      setState(() => _status = QrLoginStatus.succeeded);
      Navigator.of(context).pop(true);
    } on Object catch (error) {
      _running = false;
      if (!mounted) return;
      setState(() {
        _busy = false;
        _error = error is ShuAuthException ? error.message : '企业微信授权失败，请重新尝试';
      });
    }
  }

  /// 调用系统打开 wxwork:// scheme（或复制链接）。
  Future<void> _openWeCom() async {
    final scheme = _nativeImage?.schemeUrl;
    final confirm = _nativeImage?.confirmUrl;
    if (scheme == null || confirm == null || _launchingLink) return;
    setState(() => _launchingLink = true);
    try {
      final ok = await launchUrl(
        Uri.parse(scheme),
        mode: LaunchMode.externalApplication,
      );
      if (!ok) {
        await Clipboard.setData(ClipboardData(text: confirm));
        if (mounted) {
          ScaffoldMessenger.of(
            context,
          ).showSnackBar(const SnackBar(content: Text('无法打开企业微信，已复制链接，请手动打开')));
        }
      }
    } on Object {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(const SnackBar(content: Text('无法打开企业微信，请手动打开企业微信')));
      }
    } finally {
      if (mounted) setState(() => _launchingLink = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.shuyoColors;
    final progress = context.watch<AccountCenter>().progressLabel;
    return Scaffold(
      backgroundColor: colors.background,
      appBar: AppBar(
        title: const Text('企业微信登录'),
        // 这是压在登录表单上的一层，左上角应当是「返回上一步」而不是关闭。
        leading: IconButton(
          tooltip: '返回',
          onPressed: _busy ? null : () => Navigator.of(context).pop(false),
          icon: const Icon(Icons.arrow_back),
        ),
      ),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            children: [
              const SizedBox(height: 24),
              Text(
                _credentials ? '正在完成登录' : '使用企业微信扫一扫',
                style: ShuYoTextStyles.title(color: colors.textPrimary),
              ),
              const SizedBox(height: 12),
              Text(
                _credentials
                    ? (progress ?? '正在获取凭据')
                    : '打开手机企业微信，点击右上角扫码，或点击下方按钮直接唤起企业微信。',
                textAlign: TextAlign.center,
                style: ShuYoTextStyles.body(color: colors.textSecondary),
              ),
              const SizedBox(height: 24),
              if (_busy)
                const SizedBox.square(
                  dimension: 220,
                  child: Center(child: CircularProgressIndicator()),
                )
              else if (_nativeImage != null)
                ClipRRect(
                  borderRadius: BorderRadius.circular(ShuRadii.tile),
                  child: Container(
                    width: 220,
                    height: 220,
                    color: Colors.white,
                    padding: const EdgeInsets.all(8),
                    child: Image.memory(
                      _nativeImage!.bytes,
                      fit: BoxFit.contain,
                      errorBuilder: (context, error, stackTrace) =>
                          const Center(
                            child: Icon(Icons.broken_image_outlined, size: 48),
                          ),
                    ),
                  ),
                ),
              const SizedBox(height: 24),
              if (_error != null)
                Text(
                  _error!,
                  textAlign: TextAlign.center,
                  style: ShuYoTextStyles.body(color: colors.danger),
                )
              else if (!_credentials)
                _statusWidget(colors),
              const SizedBox(height: 16),
              OutlinedButton.icon(
                onPressed:
                    _launchingLink ||
                        _busy ||
                        _error != null ||
                        _nativeImage == null
                    ? null
                    : _openWeCom,
                icon: const Icon(Icons.open_in_new),
                label: const Text('打开企业微信登录'),
              ),
              const SizedBox(height: 8),
              Text(
                _credentials ? '请稍候' : '扫码确认后请返回本应用',
                style: ShuYoTextStyles.meta(color: colors.textTertiary),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _statusWidget(ShuYoColors colors) {
    final (text, icon) = switch (_status) {
      QrLoginStatus.waiting => ('等待扫码…', Icons.schedule),
      QrLoginStatus.confirmedPending => ('已扫码，请在手机企业微信确认', Icons.smartphone),
      QrLoginStatus.succeeded => ('登录成功', Icons.check_circle),
      QrLoginStatus.expired => ('二维码已过期', Icons.error_outline),
    };
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Icon(icon, size: 18, color: colors.accent),
        const SizedBox(width: 8),
        Text(text, style: ShuYoTextStyles.label(color: colors.accent)),
      ],
    );
  }
}
