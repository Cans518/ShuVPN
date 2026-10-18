import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/announcements/shu_announcement.dart';
import 'package:shuvpn/core/rust/generated/api/announcements.dart'
    as announcements;
import 'package:shuvpn/core/rust/generated/announcements/model.dart' as news;
import 'package:shuvpn/core/rust/generated/api/update.dart' as updates;
import 'package:shuvpn/core/rust/generated/update/release.dart' as releases;
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_announcement_client.dart';
import 'package:shuvpn/core/rust/rust_update_client.dart';
import 'package:shuvpn/core/update/shu_update_info.dart';
import 'package:shuvpn/core/connection/native_tun.dart';
import 'package:shuvpn/core/rust/rust_connection.dart';
import 'package:shuvpn/core/settings/settings_store.dart' show CaptureMode;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'native startup mapping preserves capture mode and physical choices',
    () {
      const defaults = NativeTunStartup(mtu: 1400, physicalInterface: '');
      expect(
        RustConnection.tunPreferences(defaults).captureMode,
        CaptureMode.campus,
      );
      const full = NativeTunStartup(
        enabled: false,
        mtu: 1280,
        physicalInterface: 'wifi',
        physicalDns: '1.1.1.1',
        captureMode: CaptureMode.full,
      );
      final mapped = RustConnection.tunPreferences(full);
      expect(mapped.captureMode, CaptureMode.full);
      expect(mapped.enabled, isFalse);
      expect(mapped.mtu, 1280);
      expect(mapped.physicalInterface, 'wifi');
      expect(mapped.physicalDns, '1.1.1.1');
    },
  );

  test('native preview batches forward metadata and map preview/error events without core initialization', () async {
    final owner = _Announcements();
    final client = RustAnnouncementClient.fromNative(owner);
    final page = await client.fetchList();
    final batch = await client.prefetch(page.items);
    expect(owner.prefetchItems!.single.title, 'native title');
    expect(
      owner.prefetchItems!.single.publishedAt,
      page.items.single.publishedAt!.toIso8601String(),
    );
    final events = await batch.previews.toList();
    expect(events.first.text, 'native preview');
    expect(events.last.error, 'native detail failure');
    batch.cancel();
    batch.cancel();
    expect(owner.batch.cancels, 1);
    expect(owner.batch.isDisposed, isTrue);
    client.close();
    expect(ShuRustLib.instance.initialized, isFalse);
  });

  test(
    'closing during native batch creation releases the late batch',
    () async {
      final owner = _Announcements()..pendingPrefetch = Completer();
      final client = RustAnnouncementClient.fromNative(owner);
      final pending = client.prefetch([]);
      await owner.prefetchStarted.future;
      client.close();
      owner.pendingPrefetch!.complete(owner.batch);
      await expectLater(pending, throwsA(isA<ShuAnnouncementException>()));
      expect(owner.batch.cancels, 1);
      expect(owner.batch.isDisposed, isTrue);
    },
  );

  test(
    'closing a ready client before prefetch dispatch does not invoke native',
    () async {
      final owner = _Announcements();
      final client = RustAnnouncementClient.fromNative(owner);
      final pending = client.prefetch([]);
      client.close();
      await expectLater(pending, throwsA(isA<ShuAnnouncementException>()));
      expect(owner.prefetchCalls, 0);
    },
  );

  test('native announcement values map every display block without HTTP or core initialization', () async {
    expect(ShuRustLib.instance.initialized, isFalse);
    final owner = _Announcements();
    final client = RustAnnouncementClient.fromNative(owner);
    final page = await client.fetchList(
      page: Uri.parse('https://newits.shu.edu.cn/index/tzgg/1.htm'),
    );
    expect(owner.listPage, 'https://newits.shu.edu.cn/index/tzgg/1.htm');
    expect(page.items.single.title, 'native title');
    expect(page.items.single.publishedAt, DateTime(2026, 10, 16));
    expect(
      page.nextPage.toString(),
      'https://newits.shu.edu.cn/index/tzgg/2.htm',
    );
    final detail = await client.fetchDetail(page.items.single);
    expect(owner.detailItem?.url, page.items.single.url.toString());
    expect(owner.detailItem?.title, page.items.single.title);
    expect(owner.detailItem?.dateText, page.items.single.dateText);
    expect(
      owner.detailItem?.publishedAt,
      page.items.single.publishedAt?.toIso8601String(),
    );
    expect(detail.title, 'native title');
    expect(detail.url, page.items.single.url);
    expect(detail.author, 'native author');
    expect(detail.dateText, '2026-10-16');
    expect(detail.publishedAt, DateTime(2026, 10, 16));
    expect(detail.preview, 'native preview');
    expect(detail.blocks, hasLength(4));
    final text = detail.blocks[0] as ShuAnnouncementText;
    expect(text.plainText, 'body link');
    expect(text.inlines.last.url.toString(), 'https://example.com/link');
    expect(text.inlines.first.url, isNull);
    final image = detail.blocks[1] as ShuAnnouncementImage;
    expect(image.url, 'https://newits.shu.edu.cn/image.png');
    expect(image.alt, 'native image');
    final table = detail.blocks[2] as ShuAnnouncementTable;
    expect(table.hasHeaderRow, isTrue);
    expect(table.rows.single.map((cell) => cell.plainText), ['cell', '']);
    expect(
      table.rows.single.first.inlines.single.url.toString(),
      'https://example.com/cell',
    );
    final attachment = detail.blocks[3] as ShuAnnouncementAttachment;
    expect(attachment.name, 'native attachment');
    expect(attachment.url.toString(), 'https://newits.shu.edu.cn/download.jsp');
    expect(
      client.cachedDetail(page.items.single.url)?.preview,
      'native preview',
    );
    expect(owner.cacheReads, 1);
    client.close();
    client.close();
    expect(owner.closeCalls, 1);
    expect(owner.isDisposed, isTrue);
    expect(
      () => client.cachedDetail(page.items.single.url),
      throwsA(isA<ShuAnnouncementException>()),
    );
    expect(ShuRustLib.instance.initialized, isFalse);
  });

  test(
    'native update choice and publication date are passed through',
    () async {
      expect(ShuRustLib.instance.initialized, isFalse);
      final owner = _Updates()
        ..result = const releases.UpdateInfo(
          latestVersion: '0.5.0',
          downloadUrl: 'https://example.com/windows.zip',
          releasePageUrl: 'https://example.com/releases',
          targetUrl: 'https://example.com/native-choice',
          publishedAt: '2026-10-16T02:00:00Z',
        );
      final client = RustUpdateClient.fromNative(owner);
      final result = await client.checkForUpdate('0.4.2');
      expect(owner.version, '0.4.2');
      expect(result?.latestVersion, '0.5.0');
      expect(result?.downloadUrl, 'https://example.com/windows.zip');
      expect(result?.releasePageUrl, 'https://example.com/releases');
      expect(result?.targetUrl, 'https://example.com/native-choice');
      expect(result?.publishedAt, DateTime.utc(2026, 10, 16, 2));
      owner.result = null;
      expect(await client.checkForUpdate('0.5.0'), isNull);
      client.close();
      client.close();
      expect(owner.closeCalls, 1);
      expect(owner.isDisposed, isTrue);
      await expectLater(
        client.checkForUpdate('0.4.2'),
        throwsA(isA<ShuUpdateException>()),
      );
      expect(ShuRustLib.instance.initialized, isFalse);
    },
  );
  test(
    'closing ready adapters prevents any fake native RPC dispatch',
    () async {
      final newsOwner = _Announcements();
      final newsClient = RustAnnouncementClient.fromNative(newsOwner);
      final newsPending = expectLater(
        newsClient.fetchList(),
        throwsA(isA<ShuAnnouncementException>()),
      );
      newsClient.close();
      await newsPending;
      expect(newsOwner.listCalls, 0);
      final updateOwner = _Updates();
      final updateClient = RustUpdateClient.fromNative(updateOwner);
      final updatePending = expectLater(
        updateClient.checkForUpdate('0.4.2'),
        throwsA(isA<ShuUpdateException>()),
      );
      updateClient.close();
      await updatePending;
      expect(updateOwner.checkCalls, 0);
    },
  );

  test(
    'announcement disposal still happens when a supplied owner close fails',
    () {
      final owner = _Announcements()..failClose = true;
      final client = RustAnnouncementClient.fromNative(owner);
      expect(client.close, throwsStateError);
      expect(owner.isDisposed, isTrue);
      client.close();
      expect(owner.closeCalls, 1);
    },
  );

  test('update disposal still happens when a supplied owner close fails', () {
    final owner = _Updates()..failClose = true;
    final client = RustUpdateClient.fromNative(owner);
    expect(client.close, throwsStateError);
    expect(owner.isDisposed, isTrue);
    client.close();
    expect(owner.closeCalls, 1);
  });
}

class _Announcements implements announcements.AnnouncementClient {
  int closeCalls = 0;
  int cacheReads = 0;
  int listCalls = 0;
  String? listPage;
  bool failClose = false;
  int prefetchCalls = 0;
  List<news.Item>? prefetchItems;
  final prefetchStarted = Completer<void>();
  Completer<announcements.AnnouncementPrefetch>? pendingPrefetch;
  final batch = _Prefetch();
  news.Item? detailItem;
  @override
  bool isDisposed = false;
  static const item = news.Item(
    title: 'native title',
    url: 'https://newits.shu.edu.cn/info/1095/1.htm',
    dateText: '2026-10-16',
    publishedAt: '2026-10-16',
  );
  static const detail = news.Detail(
    title: 'native title',
    url: 'https://newits.shu.edu.cn/info/1095/1.htm',
    dateText: '2026-10-16',
    publishedAt: '2026-10-16',
    author: 'native author',
    preview: 'native preview',
    blocks: [
      news.Block.text(
        inlines: [
          news.Inline(text: 'body '),
          news.Inline(text: 'link', url: 'https://example.com/link'),
        ],
      ),
      news.Block.image(
        url: 'https://newits.shu.edu.cn/image.png',
        alt: 'native image',
      ),
      news.Block.table(
        rows: [
          [
            [news.Inline(text: 'cell', url: 'https://example.com/cell')],
            [],
          ],
        ],
        hasHeaderRow: true,
      ),
      news.Block.attachment(
        name: 'native attachment',
        url: 'https://newits.shu.edu.cn/download.jsp',
      ),
    ],
  );
  @override
  Future<news.Page> fetchList({String? page}) async {
    listCalls++;
    if (isDisposed) throw StateError('disposed native owner');
    listPage = page;
    return const news.Page(
      items: [item],
      nextPage: 'https://newits.shu.edu.cn/index/tzgg/2.htm',
    );
  }

  @override
  Future<news.Detail> fetchDetail({required news.Item item}) async {
    detailItem = item;
    return detail;
  }

  @override
  Future<announcements.AnnouncementPrefetch> prefetch({
    required List<news.Item> items,
  }) async {
    prefetchCalls++;
    prefetchItems = items;
    if (!prefetchStarted.isCompleted) prefetchStarted.complete();
    return pendingPrefetch == null ? batch : await pendingPrefetch!.future;
  }

  @override
  news.Detail? cachedDetail({required String url}) {
    cacheReads++;
    return detail;
  }

  @override
  void close() {
    closeCalls++;
    if (failClose) throw StateError('fixture close failure');
  }

  @override
  void dispose() => isDisposed = true;
}

class _Prefetch implements announcements.AnnouncementPrefetch {
  int cancels = 0;
  @override
  bool isDisposed = false;
  @override
  Stream<news.Preview> events() => Stream.fromIterable([
    const news.Preview(
      url: 'https://newits.shu.edu.cn/info/1.htm',
      text: 'native preview',
    ),
    const news.Preview(
      url: 'https://newits.shu.edu.cn/info/2.htm',
      error: 'native detail failure',
    ),
  ]);
  @override
  void cancel() => cancels++;
  @override
  void dispose() => isDisposed = true;
}

class _Updates implements updates.UpdateClient {
  releases.UpdateInfo? result;
  String? version;
  int closeCalls = 0;
  int checkCalls = 0;
  bool failClose = false;
  @override
  bool isDisposed = false;
  @override
  Future<releases.UpdateInfo?> check({required String currentVersion}) async {
    checkCalls++;
    if (isDisposed) throw StateError('disposed native owner');
    version = currentVersion;
    return result;
  }

  @override
  void close() {
    closeCalls++;
    if (failClose) throw StateError('fixture close failure');
  }

  @override
  void dispose() => isDisposed = true;
}
