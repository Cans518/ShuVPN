import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart'
    show AnyhowException;
import 'package:shuvpn/core/rust/generated/api/announcements.dart' as native;
import 'package:shuvpn/core/rust/generated/announcements/model.dart' as model;
import 'package:shuvpn/core/rust/rust_core.dart';
import 'package:shuvpn/core/rust/rust_announcement_client.dart';
import 'package:shuvpn/core/announcements/shu_announcement.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test(
    'reusing a native preview batch reports its error on the stream',
    () async {
      final client = await native.AnnouncementClient.newInstance();
      final batch = await client.prefetch(items: []);
      addTearDown(() {
        batch.dispose();
        client.close();
        client.dispose();
      });
      expect(await batch.events().toList(), isEmpty);
      await expectLater(
        batch.events().toList(),
        throwsA(
          isA<AnyhowException>().having(
            (error) => error.message,
            'message',
            contains('[state]'),
          ),
        ),
      );
    },
  );

  test('real native preview batches reject budgets and do not fetch offsite or cancelled items', () async {
    final client = await native.AnnouncementClient.newInstance();
    addTearDown(() {
      client.close();
      client.dispose();
    });
    const external = model.Item(
      title: 'external',
      url: 'https://foreign.invalid/item',
      dateText: '',
    );
    final filtered = await client.prefetch(items: [external]);
    addTearDown(filtered.dispose);
    expect(await filtered.events().toList(), isEmpty);
    const allowed = model.Item(
      title: 'cancelled',
      url: 'https://newits.shu.edu.cn/info/1.htm',
      dateText: '',
    );
    final cancelled = await client.prefetch(items: [allowed]);
    addTearDown(cancelled.dispose);
    cancelled.cancel();
    expect(await cancelled.events().toList(), isEmpty);
    await expectLater(
      client.prefetch(items: List.filled(2049, external)),
      throwsA(
        isA<model.AnnouncementFailure>().having(
          (error) => error.code,
          'code',
          'size',
        ),
      ),
    );
    final parentClosed = await client.prefetch(items: [allowed]);
    addTearDown(parentClosed.dispose);
    client.close();
    expect(await parentClosed.events().toList(), isEmpty);
  });

  test(
    'closing a ready announcement owner prevents dispatch after await',
    () async {
      for (final detail in [false, true]) {
        final owner = await native.AnnouncementClient.newInstance();
        final client = RustAnnouncementClient.fromNative(owner);
        final item = ShuAnnouncementListItem(
          title: 'fixture',
          url: Uri.parse('https://foreign.invalid/detail'),
        );
        final Future<Object> pending = detail
            ? client.fetchDetail(item)
            : client.fetchList(page: Uri.parse('https://foreign.invalid/list'));
        final rejected = expectLater(
          pending,
          throwsA(
            isA<ShuAnnouncementException>().having(
              (error) => error.message,
              'message',
              contains('已经关闭'),
            ),
          ),
        );
        client.close();
        await rejected;
        expect(owner.isDisposed, isTrue);
      }
    },
  );

  test(
    'real native announcement origin and closed boundaries precede HTTP',
    () async {
      final client = await native.AnnouncementClient.newInstance();
      addTearDown(client.dispose);
      await expectLater(
        client.fetchList(page: 'https://newits.shu.edu.cn.example.com/list'),
        throwsA(
          isA<model.AnnouncementFailure>().having(
            (error) => error.code,
            'code',
            'origin',
          ),
        ),
      );
      client.close();
      await expectLater(
        client.fetchList(),
        throwsA(
          isA<model.AnnouncementFailure>().having(
            (error) => error.code,
            'code',
            'closed',
          ),
        ),
      );
      expect(
        () => client.cachedDetail(url: 'https://newits.shu.edu.cn/info/1.htm'),
        throwsA(
          isA<model.AnnouncementFailure>().having(
            (error) => error.code,
            'code',
            'closed',
          ),
        ),
      );
    },
  );

  test(
    'native Flutter adapter rejects offsite detail and remains closed',
    () async {
      final client = RustAnnouncementClient();
      await expectLater(
        client.fetchDetail(
          ShuAnnouncementListItem(
            title: 'external',
            url: Uri.parse('https://www.shu.edu.cn/'),
          ),
        ),
        throwsA(isA<ShuAnnouncementException>()),
      );
      expect(
        () => client.cachedDetail(Uri.parse('https://foreign.example/')),
        throwsA(isA<ShuAnnouncementException>()),
      );
      client.close();
      client.close();
      await expectLater(
        client.fetchList(),
        throwsA(isA<ShuAnnouncementException>()),
      );
    },
  );

  test('closing while announcement owner opens rejects pending work', () async {
    final client = RustAnnouncementClient();
    final pending = client.fetchList();
    final rejected = expectLater(
      pending,
      throwsA(isA<ShuAnnouncementException>()),
    );
    client.close();
    await rejected;
    expect(
      () => client.cachedDetail(
        Uri.parse('https://newits.shu.edu.cn/info/1.htm'),
      ),
      throwsA(isA<ShuAnnouncementException>()),
    );
  });
}
