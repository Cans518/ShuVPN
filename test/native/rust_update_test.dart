import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/rust/generated/api/update.dart' as native;
import 'package:shuvpn/core/rust/generated/update.dart' as model;
import 'package:shuvpn/core/rust/rust_core.dart';
import 'package:shuvpn/core/rust/rust_update_client.dart';
import 'package:shuvpn/core/update/shu_update_info.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test('closing a ready update owner prevents dispatch after await', () async {
    final owner = await native.UpdateClient.newInstance();
    final client = RustUpdateClient.fromNative(owner);
    final pending = client.checkForUpdate('0.4.2');
    final rejected = expectLater(
      pending,
      throwsA(
        isA<ShuUpdateException>().having(
          (error) => error.message,
          'message',
          contains('已经关闭'),
        ),
      ),
    );
    client.close();
    await rejected;
    expect(owner.isDisposed, isTrue);
  });

  test('real update channel close rejects before network', () async {
    final client = await native.UpdateClient.newInstance();
    addTearDown(client.dispose);
    client.close();
    await expectLater(
      client.check(currentVersion: '0.4.2'),
      throwsA(
        isA<model.UpdateFailure>().having(
          (error) => error.code,
          'code',
          'closed',
        ),
      ),
    );
  });

  test(
    'Flutter update adapter maps native validation and stays closed',
    () async {
      final client = RustUpdateClient();
      addTearDown(client.close);
      await expectLater(
        client.checkForUpdate('x' * 257),
        throwsA(
          isA<ShuUpdateException>().having(
            (error) => error.message,
            'message',
            '当前版本名超过长度限制',
          ),
        ),
      );
      client.close();
      client.close();
      await expectLater(
        client.checkForUpdate('0.4.2'),
        throwsA(isA<ShuUpdateException>()),
      );
    },
  );

  test('closing while native update owner opens discards the result', () async {
    final client = RustUpdateClient();
    final pending = client.checkForUpdate('x' * 257);
    final rejected = expectLater(
      pending,
      throwsA(
        isA<ShuUpdateException>().having(
          (error) => error.message,
          'message',
          '更新通道已经关闭',
        ),
      ),
    );
    client.close();
    await rejected;
  });
}
