import 'shu_update_info.dart';

/// 远端最新版本的一次查询。
///
/// 抽成接口只为让测试塞替身：启动路径每次都会跑它，而 widget 测试不该真的
/// 发请求。
abstract interface class ShuUpdateClient {
  /// 远端比 [currentVersion] 新时返回结论；已经是最新、或上游还没有正式发布
  /// 时返回 null。失败抛 [ShuUpdateException]。
  Future<ShuUpdateInfo?> checkForUpdate(String currentVersion);

  /// Cancels pending checks and releases the channel when the app closes.
  void close();
}
