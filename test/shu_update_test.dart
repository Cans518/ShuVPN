import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/app/app_info.dart';
import 'package:shuvpn/core/update/shu_update_info.dart';
import 'package:shuvpn/widgets/shu_update_prompt.dart';

void main() {
  group('showShuUpdatePrompt', () {
    Future<List<bool>> openPrompt(
      WidgetTester tester,
      ShuUpdateInfo update,
    ) async {
      final answers = <bool>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) => TextButton(
              onPressed: () async => answers.add(
                await showShuUpdatePrompt(context, update: update),
              ),
              child: const Text('打开'),
            ),
          ),
        ),
      );
      await tester.tap(find.text('打开'));
      await tester.pumpAndSettle();
      return answers;
    }

    testWidgets('shows both versions and offers the download', (tester) async {
      final answers = await openPrompt(
        tester,
        const ShuUpdateInfo(
          latestVersion: '99.0.0',
          downloadUrl: 'https://example.com/ShuVPN-windows-x64.zip',
          targetUrl: 'https://example.com/ShuVPN-windows-x64.zip',
          releasePageUrl: 'https://example.com/releases',
        ),
      );

      expect(find.text('发现新版本'), findsOneWidget);
      expect(find.text('当前版本：${ShuAppInfo.version}'), findsOneWidget);
      expect(find.text('最新版本：99.0.0'), findsOneWidget);

      await tester.tap(find.text('更新'));
      await tester.pumpAndSettle();
      expect(answers, <bool>[true]);
    });

    testWidgets('drops the download button when there is no link', (
      tester,
    ) async {
      final answers = await openPrompt(
        tester,
        const ShuUpdateInfo(
          latestVersion: '99.0.0',
          downloadUrl: '',
          targetUrl: 'https://example.com/releases',
          releasePageUrl: 'https://example.com/releases',
        ),
      );

      expect(find.text('更新'), findsNothing);

      await tester.tap(find.text('知道了'));
      await tester.pumpAndSettle();
      expect(answers, <bool>[false]);
    });
  });
}
