import 'shu_announcement.dart';

/// 公告站点的主机名。这个应用只请求它。
const String shuAnnouncementHost = 'newits.shu.edu.cn';

/// [url] 是不是本应用该去取的公告地址。
///
/// 列表页里混着指向外站的条目 —— 站点把「换个地方看」的链接也当成一条排进
/// 列表，解析出来的地址就在别的域上。那些条目照常显示（标题、日期都是真的），
/// 但不该把它们当公告去拉：这不是公告站的内容，拉回来也排不出正文，而且一次
/// 预取十篇就等于替站点去访问十个它指到的地方。
///
/// 判据是主机名**精确**相等，不做子域通配：这个站没有子域，「以它结尾」反而
/// 会把 `newits.shu.edu.cn.example.com` 放进来。CNAME 到 `web10.shu.edu.cn`
/// 是 DNS 层的事，请求里的 Host 仍然是这里写的那个。
bool isShuAnnouncementUrl(Uri url) => url.host == shuAnnouncementHost;

/// 信息办公告的一个通道。
///
/// 抽成接口只为让测试塞替身：通知页一打开就会拉列表，而 widget 测试既不该
/// 真的发请求，也不该依赖 shu.edu.cn 的返回。
///
/// 接口里有 [cachedDetail] 这样一个**同步**读法，是这一层与别处不同的地方：
/// 通知页会把列表里每一篇的正文先取回来（为了那两行预览），于是点开时正文
/// 已经在手上了。详情页因此能不经过 `FutureBuilder` 直接把它画出来 —— 没有
/// 第一帧的转圈，也没有「先白一下再出内容」。
abstract interface class ShuAnnouncementClient {
  /// 拉一页列表；[page] 为 null 时拉第一页。
  Future<ShuAnnouncementPage> fetchList({Uri? page});

  Future<ShuAnnouncementDetail> fetchDetail(ShuAnnouncementListItem item);

  /// 已经取回来过的那一篇；没有则为 null，调用方自己去 [fetchDetail]。
  ShuAnnouncementDetail? cachedDetail(Uri url);
  Future<ShuAnnouncementPrefetch> prefetch(List<ShuAnnouncementListItem> items);

  /// 收掉这条通道背后的资源。
  ///
  /// 关闭原生通道并释放所有者。
  void close();
}

class ShuAnnouncementPreview {
  const ShuAnnouncementPreview({required this.url, this.text, this.error});
  final Uri url;
  final String? text;
  final String? error;
}

abstract interface class ShuAnnouncementPrefetch {
  Stream<ShuAnnouncementPreview> get previews;
  void cancel();
}
