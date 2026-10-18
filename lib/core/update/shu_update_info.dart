/// 一条「远端有新版本」的结论。
///
/// 只保留弹窗与跳转用得上的字段。上游 release 的正文与 `*.sha256` 校验文件
/// 不进来 —— 正文是 CI 自动生成的模板，对用户是噪音。
class ShuUpdateInfo {
  const ShuUpdateInfo({
    required this.latestVersion,
    required this.downloadUrl,
    required this.releasePageUrl,
    required this.targetUrl,
    this.publishedAt,
  });

  /// 远端标签去掉 `v` 前缀后的版本名，与 `pubspec.yaml` 的 `version` 同名。
  final String latestVersion;

  /// 当前桌面的完整分发包直链；缺少匹配资产时为空。
  final String downloadUrl;

  /// 发布页地址。它是缺少桌面资产时的入口，也是最坏情况下唯一能给的去处。
  final String releasePageUrl;

  final DateTime? publishedAt;

  bool get hasDownloadUrl => downloadUrl.trim().isNotEmpty;

  /// 点「更新」时该打开的地址。
  final String targetUrl;
}

/// 更新检查失败。
///
/// [toString] 直接返回 [message]：它唯一的去处是 snack 里那句
/// 「检查更新失败：…」，带上异常类名只会让用户多看到一行英文。
class ShuUpdateException implements Exception {
  const ShuUpdateException(this.message);

  final String message;

  @override
  String toString() => message;
}
