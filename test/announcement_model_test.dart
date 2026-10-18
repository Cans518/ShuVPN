import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/announcements/shu_announcement.dart';
import 'package:shuvpn/core/announcements/shu_announcement_client.dart';

void main() {
  final url = Uri.parse('https://newits.shu.edu.cn/info/1095/6432.htm');
  test('cuts a manually constructed view preview and marks the cut', () {
    final detail = ShuAnnouncementDetail(
      title: '标题',
      url: url,
      blocks: [ShuAnnouncementText('字' * 200)],
    );
    expect(detail.preview.length, ShuAnnouncementDetail.previewLength + 1);
    expect(detail.preview, endsWith('…'));
  });
  test('an image-only view has no text preview', () {
    final detail = ShuAnnouncementDetail(
      title: '标题',
      url: url,
      blocks: const [
        ShuAnnouncementImage('https://newits.shu.edu.cn/image.png'),
      ],
    );
    expect(detail.preview, isEmpty);
  });
  test('a supplied native preview is authoritative', () {
    final detail = ShuAnnouncementDetail(
      title: '标题',
      url: url,
      blocks: const [ShuAnnouncementText('body')],
      previewText: 'native preview',
    );
    expect(detail.preview, 'native preview');
  });
  test('only the exact notice host routes to the in-app detail page', () {
    expect(isShuAnnouncementUrl(url), isTrue);
    expect(
      isShuAnnouncementUrl(Uri.parse('http://newits.shu.edu.cn/x.htm')),
      isTrue,
    );
    for (final address in [
      'https://www.shu.edu.cn/',
      'https://web10.shu.edu.cn/',
      'https://blog.newits.shu.edu.cn/',
      'https://newits.shu.edu.cn.example.com/x',
      'file:///etc/hosts',
    ]) {
      expect(isShuAnnouncementUrl(Uri.parse(address)), isFalse);
    }
  });
}
