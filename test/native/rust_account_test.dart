import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/rust/generated/api/account.dart' as native;
import 'package:shuvpn/core/rust/generated/auth/model.dart' as errors;
import 'package:shuvpn/core/rust/generated/auth/newsso/account/model.dart'
    as model;
import 'package:shuvpn/core/rust/rust_core.dart';
import 'package:shuvpn/core/rust/rust_account_center.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test('combined native password and code commands validate before dispatch and reject closed accounts', () async {
    final directory = await Directory.systemTemp.createTemp(
      'shuvpn-sign-in-boundary-',
    );
    addTearDown(() => directory.delete(recursive: true));
    final session = await native.AccountSession.open(
      directory: directory.path,
      legacyCookies: '[]',
      legacySnapshot: '{}',
    );
    addTearDown(session.dispose);
    await expectLater(
      session.signInPassword(username: '', password: ''),
      throwsA(
        isA<errors.AuthFailure>().having(
          (error) => error.code,
          'code',
          'credentials',
        ),
      ),
    );
    expect(session.snapshot().busy, isFalse);
    session.close();
    await expectLater(
      session.signInPassword(username: 'student', password: 'fixture'),
      throwsA(
        isA<errors.AuthFailure>().having(
          (error) => error.code,
          'code',
          'closed',
        ),
      ),
    );
    await expectLater(
      session.signInCode(method: 'sms', code: '123456'),
      throwsA(
        isA<errors.AuthFailure>().having(
          (error) => error.code,
          'code',
          // Verification observes the client's cancelled token before dispatch.
          'cancelled',
        ),
      ),
    );
  });

  test(
    'closed native account rejects the unified QR stream before network work',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-qr-closed-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final session = await native.AccountSession.open(
        directory: directory.path,
        legacyCookies: '[]',
        legacySnapshot: '{}',
      );
      addTearDown(session.dispose);
      session.close();
      final events = await session.signInQr().toList();
      expect(events, hasLength(1));
      expect(events.single.phase, model.QrSignInPhase.failed);
      expect(events.single.error?.code, 'closed');
      expect(events.single.image, isNull);
    },
  );

  test(
    'native frontend owner removes legacy keys only after durable adoption',
    () async {
      SharedPreferences.setMockInitialValues({
        'auth.newsso.cookies':
            '[{"name":"SHU_OAUTH2","value":"ffi-owner-fixture","path":"/"}]',
        'auth.account.snapshot':
            '{"accountId":"25123456","profile":{"name":"fixture",'
            '"studentId":"25123456","grade":"2025级",'
            '"college":"fixture college","major":"fixture major"}}',
        'atrust_device_id': 'original-device',
      });
      final preferences = await SharedPreferences.getInstance();
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-owner-ui-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final account = await RustAccountCenter.open(directory.path, preferences);
      final masked = account.maskedDeviceId;
      expect(account.signedIn, isTrue);
      expect(account.accountId, '25123456');
      expect(account.profile?.name, 'fixture');
      expect(account.profile?.studentId, '25123456');
      expect(account.profile?.grade, '2025级');
      expect(account.profile?.college, 'fixture college');
      expect(account.profile?.major, 'fixture major');
      for (final key in RustAccountCenter.legacyKeys) {
        expect(preferences.containsKey(key), isFalse);
      }
      account.dispose();
      final restored = await RustAccountCenter.open(
        directory.path,
        preferences,
      );
      expect(restored.maskedDeviceId, masked);
      expect(restored.accountId, '25123456');
      expect(restored.profile?.name, 'fixture');
      expect(restored.profile?.studentId, '25123456');
      expect(restored.profile?.grade, '2025级');
      expect(restored.profile?.college, 'fixture college');
      expect(restored.profile?.major, 'fixture major');
      await restored.signOut();
      restored.dispose();
      final loggedOut = await RustAccountCenter.open(
        directory.path,
        preferences,
      );
      addTearDown(loggedOut.dispose);
      expect(loggedOut.signedIn, isFalse);
      expect(loggedOut.profile, isNull);
      expect(loggedOut.maskedDeviceId, masked);
      expect(preferences.containsKey('atrust_device_id'), isFalse);
    },
  );

  test(
    'failed native adoption leaves all legacy preference inputs intact',
    () async {
      SharedPreferences.setMockInitialValues({
        'auth.newsso.cookies': 'invalid fixture',
        'auth.account.snapshot': '{"accountId":"25123456"}',
        'atrust_device_id': 'original-device',
      });
      final preferences = await SharedPreferences.getInstance();
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-owner-failure-',
      );
      addTearDown(() => directory.delete(recursive: true));
      await expectLater(
        RustAccountCenter.open(directory.path, preferences),
        throwsA(isA<errors.AuthFailure>()),
      );
      expect(preferences.getString('auth.newsso.cookies'), 'invalid fixture');
      expect(preferences.containsKey('auth.account.snapshot'), isTrue);
      expect(preferences.getString('atrust_device_id'), 'original-device');
    },
  );

  test('real account adopts a complete legacy bundle and preserves device on logout', () async {
    final directory = await Directory.systemTemp.createTemp('shuvpn-account-');
    addTearDown(() => directory.delete(recursive: true));
    final session = await native.AccountSession.open(
      directory: directory.path,
      legacyCookies: jsonEncode([
        {'name': 'SHU_OAUTH2', 'value': 'ffi-account-fixture', 'path': '/'},
      ]),
      legacyDeviceId: 'persisted-device',
      legacySnapshot: jsonEncode({
        'accountId': '25123456',
        'profile': {'name': 'fixture name', 'studentId': '25123456'},
        'credentials': {'jwxt': 'available', 'atrust': 'refreshing'},
        'verifiedAt': '2026-01-01T00:00:00Z',
      }),
    );
    addTearDown(session.dispose);
    final adopted = session.snapshot();
    expect(adopted.signedIn, isTrue);
    expect(adopted.accountId, '25123456');
    expect(adopted.profile?.name, 'fixture name');
    expect(adopted.availableCount, 1);
    expect(adopted.credentials[1].state.name, 'unknown');
    expect(adopted.needsReverification, isTrue);
    session.close();
    final restored = await native.AccountSession.open(
      directory: directory.path,
      legacyCookies: 'not read after adoption',
      legacyDeviceId: 'different-device',
      legacySnapshot: '{}',
    );
    addTearDown(restored.dispose);
    expect(restored.snapshot().maskedDeviceId, adopted.maskedDeviceId);
    expect(restored.snapshot().accountId, adopted.accountId);
    restored.signOut();
    expect(restored.snapshot().signedIn, isFalse);
    restored.close();
    final loggedOut = await native.AccountSession.open(
      directory: directory.path,
      legacyCookies: jsonEncode([
        {'name': 'SHU_OAUTH2', 'value': 'must-not-return', 'path': '/'},
      ]),
      legacyDeviceId: null,
      legacySnapshot: '{}',
    );
    addTearDown(() {
      loggedOut.close();
      loggedOut.dispose();
    });
    expect(loggedOut.snapshot().signedIn, isFalse);
    expect(loggedOut.snapshot().profile, isNull);
    expect(loggedOut.snapshot().availableCount, 0);
    expect(loggedOut.snapshot().maskedDeviceId, adopted.maskedDeviceId);
  });

  test(
    'real signed-out account avoids network and closed owner rejects commands',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-account-empty-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final session = await native.AccountSession.open(
        directory: directory.path,
        legacyCookies: '[]',
        legacyDeviceId: null,
        legacySnapshot: '{}',
      );
      addTearDown(session.dispose);
      expect(session.verificationState(), isNull);
      expect(
        () => session.selectVerificationMethod(method: 'sms'),
        throwsA(
          isA<errors.AuthFailure>().having(
            (error) => error.code,
            'code',
            'invalidMethod',
          ),
        ),
      );
      await expectLater(
        session.sendCode(method: 'sms'),
        throwsA(
          isA<errors.AuthFailure>().having(
            (error) => error.code,
            'code',
            'invalidMethod',
          ),
        ),
      );
      expect(await session.completeSignIn(), isFalse);
      expect(await session.verifyIfStale(), isFalse);
      await expectLater(
        session.login(username: '', password: ''),
        throwsA(
          isA<errors.AuthFailure>().having(
            (error) => error.code,
            'code',
            'credentials',
          ),
        ),
      );
      session.close();
      expect(
        session.snapshot,
        throwsA(
          isA<errors.AuthFailure>().having(
            (error) => error.code,
            'code',
            'closed',
          ),
        ),
      );
    },
  );
}
