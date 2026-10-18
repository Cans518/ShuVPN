/// 检查更新在哪些平台上开着。
///
/// 桌面正式发布渠道尚未验收，候选版本暂不自动查询更新。
abstract final class ShuUpdatePolicy {
  const ShuUpdatePolicy._();

  static bool get enabled => false;
}
