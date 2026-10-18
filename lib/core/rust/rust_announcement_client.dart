import '../announcements/shu_announcement.dart';
import '../announcements/shu_announcement_client.dart';
import 'generated/api/announcements.dart' as native;
import 'generated/announcements/model.dart' as model;
import 'rust_core.dart';

final class RustAnnouncementClient implements ShuAnnouncementClient {
  RustAnnouncementClient();

  /// Takes ownership of an existing native channel.
  RustAnnouncementClient.fromNative(native.AnnouncementClient client)
    : _client = client;

  native.AnnouncementClient? _client;
  Future<native.AnnouncementClient>? _opening;
  bool _closed = false;

  void _live() {
    if (_closed) throw const ShuAnnouncementException('公告通道已经关闭');
  }

  Future<native.AnnouncementClient> _open() {
    _live();
    final client = _client;
    if (client != null) return Future.value(client);
    return _opening ??= () async {
      await RustCore.initialize();
      _live();
      final client = await native.AnnouncementClient.newInstance();
      if (_closed) {
        _release(client);
        throw const ShuAnnouncementException('公告通道已经关闭');
      }
      _client = client;
      return client;
    }();
  }

  @override
  Future<ShuAnnouncementPage> fetchList({Uri? page}) async {
    try {
      final client = await _open();
      _live();
      final result = await client.fetchList(page: page?.toString());
      _live();
      return ShuAnnouncementPage(
        items: [for (final item in result.items) _item(item)],
        nextPage: result.nextPage == null ? null : Uri.parse(result.nextPage!),
      );
    } on model.AnnouncementFailure catch (error) {
      throw ShuAnnouncementException(
        error.message,
        statusCode: error.statusCode,
      );
    }
  }

  @override
  Future<ShuAnnouncementDetail> fetchDetail(
    ShuAnnouncementListItem item,
  ) async {
    try {
      final client = await _open();
      _live();
      final result = await client.fetchDetail(item: _nativeItem(item));
      _live();
      return _detail(result);
    } on model.AnnouncementFailure catch (error) {
      throw ShuAnnouncementException(
        error.message,
        statusCode: error.statusCode,
      );
    }
  }

  static model.Item _nativeItem(ShuAnnouncementListItem item) => model.Item(
    title: item.title,
    url: item.url.toString(),
    dateText: item.dateText,
    publishedAt: item.publishedAt?.toIso8601String(),
  );

  @override
  Future<ShuAnnouncementPrefetch> prefetch(
    List<ShuAnnouncementListItem> items,
  ) async {
    try {
      final client = await _open();
      _live();
      final batch = await client.prefetch(
        items: items.map(_nativeItem).toList(),
      );
      if (_closed) {
        try {
          batch.cancel();
        } finally {
          batch.dispose();
        }
        _live();
      }
      return _RustAnnouncementPrefetch(batch);
    } on model.AnnouncementFailure catch (error) {
      throw ShuAnnouncementException(
        error.message,
        statusCode: error.statusCode,
      );
    }
  }

  @override
  ShuAnnouncementDetail? cachedDetail(Uri url) {
    try {
      _live();
      final result = _client?.cachedDetail(url: url.toString());
      return result == null ? null : _detail(result);
    } on model.AnnouncementFailure catch (error) {
      throw ShuAnnouncementException(
        error.message,
        statusCode: error.statusCode,
      );
    }
  }

  @override
  void close() {
    if (_closed) return;
    _closed = true;
    final client = _client;
    _client = null;
    if (client != null) _release(client);
  }

  void _release(native.AnnouncementClient client) {
    try {
      client.close();
    } finally {
      client.dispose();
    }
  }

  ShuAnnouncementListItem _item(model.Item item) => ShuAnnouncementListItem(
    title: item.title,
    url: Uri.parse(item.url),
    dateText: item.dateText,
    publishedAt: item.publishedAt == null
        ? null
        : DateTime.tryParse(item.publishedAt!),
  );

  ShuAnnouncementInline _inline(model.Inline inline) => ShuAnnouncementInline(
    inline.text,
    url: inline.url == null ? null : Uri.parse(inline.url!),
  );

  ShuAnnouncementDetail _detail(model.Detail detail) => ShuAnnouncementDetail(
    title: detail.title,
    url: Uri.parse(detail.url),
    dateText: detail.dateText,
    author: detail.author,
    publishedAt: detail.publishedAt == null
        ? null
        : DateTime.tryParse(detail.publishedAt!),
    previewText: detail.preview,
    blocks: [
      for (final block in detail.blocks)
        switch (block) {
          model.Block_Text(:final inlines) => ShuAnnouncementText.rich([
            for (final inline in inlines) _inline(inline),
          ]),
          model.Block_Image(:final url, :final alt) => ShuAnnouncementImage(
            url,
            alt: alt,
          ),
          model.Block_Table(:final rows, :final hasHeaderRow) =>
            ShuAnnouncementTable(
              hasHeaderRow: hasHeaderRow,
              rows: [
                for (final row in rows)
                  [
                    for (final cell in row)
                      ShuAnnouncementTableCell([
                        for (final inline in cell) _inline(inline),
                      ]),
                  ],
              ],
            ),
          model.Block_Attachment(:final name, :final url) =>
            ShuAnnouncementAttachment(name, Uri.parse(url)),
        },
    ],
  );
}

final class _RustAnnouncementPrefetch implements ShuAnnouncementPrefetch {
  _RustAnnouncementPrefetch(this._owner);
  final native.AnnouncementPrefetch _owner;
  bool _cancelled = false;
  Stream<ShuAnnouncementPreview>? _previews;
  @override
  Stream<ShuAnnouncementPreview> get previews {
    if (_cancelled) return const Stream.empty();
    return _previews ??= _owner.events().map(
      (event) => ShuAnnouncementPreview(
        url: Uri.parse(event.url),
        text: event.text,
        error: event.error,
      ),
    );
  }

  @override
  void cancel() {
    if (_cancelled) return;
    _cancelled = true;
    try {
      _owner.cancel();
    } finally {
      _owner.dispose();
    }
  }
}
