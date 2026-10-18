import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/rust/generated/api/newsso.dart' as native;
import 'package:shuvpn/core/rust/generated/auth/model.dart' as model;
import 'package:shuvpn/core/rust/rust_core.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test(
    'real SSO handle owns imported cookies and closes import on logout',
    () async {
      final session = await native.NewssoSession.newInstance();
      addTearDown(session.dispose);
      expect(session.snapshot().hasSession, isFalse);
      final imported = session.importLegacy(
        document: jsonEncode([
          {'name': 'SHU_OAUTH2', 'value': 'ffi-fixture-only', 'path': '/'},
          {'name': 'business-cookie', 'value': 'not-imported'},
          {
            'name': 'SHU_OAUTH2',
            'value': 'expired-fixture',
            'expires': '2000-01-01T00:00:00Z',
          },
        ]),
      );
      expect(imported, BigInt.one);
      final snapshot = session.snapshot();
      expect(snapshot.hasSession, isTrue);
      expect(snapshot.legacyImported, isTrue);
      expect(snapshot.cookieNames, ['SHU_OAUTH2']);
      expect(snapshot.challengeMethods, isEmpty);
      session.clear();
      expect(session.snapshot().hasSession, isFalse);
      expect(session.snapshot().cookieNames, isEmpty);
      expect(
        () => session.importLegacy(document: '[]'),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'alreadyImported',
          ),
        ),
      );
    },
  );

  test(
    'real SSO handle rejects local invalid operations before HTTP',
    () async {
      final session = await native.NewssoSession.newInstance();
      addTearDown(session.dispose);
      await expectLater(
        session.login(username: '', password: 'not-a-real-password'),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'credentials',
          ),
        ),
      );
      await expectLater(
        session.sendCode(method: 'sms'),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'challengeExpired',
          ),
        ),
      );
      session.clear();
      expect(
        () => session.importLegacy(
          document: '[{"name":"SHU_OAUTH2","value":"old"}]',
        ),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'importClosed',
          ),
        ),
      );
      expect(session.snapshot().hasSession, isFalse);
    },
  );

  test(
    'real persistent SSO commits import and logout across native handles',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-newsso-ffi-',
      );
      final handles = <native.NewssoSession>[];
      addTearDown(() async {
        for (final handle in handles) {
          handle.close();
          handle.dispose();
        }
        await directory.delete(recursive: true);
      });
      final first = await native.NewssoSession.open(directory: directory.path);
      handles.add(first);
      expect(first.snapshot().persistent, isTrue);
      expect(first.snapshot().legacyImportAllowed, isTrue);
      await expectLater(
        native.NewssoSession.open(directory: directory.path),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'ssoStorage',
          ),
        ),
      );
      final old = jsonEncode([
        {'name': 'SHU_OAUTH2', 'value': 'persistent-fixture-only', 'path': '/'},
      ]);
      expect(first.importLegacy(document: old), BigInt.one);
      first.close();
      final reopened = await native.NewssoSession.open(
        directory: directory.path,
      );
      handles.add(reopened);
      expect(reopened.snapshot().hasSession, isTrue);
      expect(reopened.snapshot().legacyImported, isTrue);
      expect(reopened.snapshot().legacyImportAllowed, isFalse);
      reopened.clear();
      reopened.close();
      final loggedOut = await native.NewssoSession.open(
        directory: directory.path,
      );
      handles.add(loggedOut);
      expect(loggedOut.snapshot().hasSession, isFalse);
      expect(loggedOut.snapshot().legacyImportAllowed, isFalse);
      expect(
        () => loggedOut.importLegacy(document: old),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'alreadyImported',
          ),
        ),
      );
    },
  );

  test(
    'real persistent SSO fresh logout closes import and rejects corrupt state',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-newsso-tombstone-',
      );
      final session = await native.NewssoSession.open(
        directory: directory.path,
      );
      addTearDown(() async {
        session.close();
        session.dispose();
        await directory.delete(recursive: true);
      });
      session.clear();
      session.close();
      final restored = await native.NewssoSession.open(
        directory: directory.path,
      );
      expect(restored.snapshot().hasSession, isFalse);
      expect(restored.snapshot().legacyImported, isFalse);
      expect(restored.snapshot().legacyImportAllowed, isFalse);
      restored.close();
      restored.dispose();
      final state = File(
        '${directory.path}${Platform.pathSeparator}newsso-state.bin',
      );
      await state.writeAsString('corrupt');
      await expectLater(
        native.NewssoSession.open(directory: directory.path),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'ssoStorage',
          ),
        ),
      );
    },
  );
  test(
    'real QR API rejects missing state and cancellation before HTTP',
    () async {
      final session = await native.NewssoSession.newInstance();
      addTearDown(session.dispose);
      expect(session.snapshot().qrStatus, isNull);
      await expectLater(
        session.pollQr(),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'qrMissing',
          ),
        ),
      );
      await expectLater(
        session.redeemQr(),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'qrMissing',
          ),
        ),
      );
      await expectLater(
        session.waitQr(timeoutSeconds: 1),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'qrMissing',
          ),
        ),
      );
      await expectLater(
        session.waitQr(timeoutSeconds: 181),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'qrTimeout',
          ),
        ),
      );
      session.cancelQr();
      expect(session.snapshot().qrStatus, isNull);
      expect(session.snapshot().hasSession, isFalse);
    },
  );
  test(
    'real shared authorization API rejects local inputs before HTTP',
    () async {
      final session = await native.NewssoSession.newInstance();
      addTearDown(session.dispose);
      await expectLater(
        session.authorizeSystem(systemId: 'unknown'),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'unknownSystem',
          ),
        ),
      );
      await expectLater(
        session.authorizeSystem(systemId: 'jwxt'),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'sessionMissing',
          ),
        ),
      );
      await expectLater(
        session.signInAtrust(deviceId: ''),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'atrustDeviceId',
          ),
        ),
      );
      await expectLater(
        session.signInAtrust(deviceId: 'fixture-device'),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'sessionMissing',
          ),
        ),
      );
      expect(session.snapshot().hasSession, isFalse);
    },
  );
  test(
    'real business data API requires an account and respects close',
    () async {
      final session = await native.NewssoSession.newInstance();
      addTearDown(session.dispose);
      for (final read in [session.fetchOtp, session.fetchJwxtAccount]) {
        await expectLater(
          read(),
          throwsA(
            isA<model.AuthFailure>().having(
              (failure) => failure.code,
              'code',
              'sessionMissing',
            ),
          ),
        );
      }
      session.close();
      await expectLater(
        session.fetchOtp(),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'closed',
          ),
        ),
      );
    },
  );
  test('real opaque preparation and probe APIs reject before HTTP', () async {
    final session = await native.NewssoSession.newInstance();
    addTearDown(session.dispose);
    await expectLater(
      session.prepareTunnel(deviceId: '', strictAntiMitm: false),
      throwsA(
        isA<model.AuthFailure>().having(
          (failure) => failure.code,
          'code',
          'atrustDeviceId',
        ),
      ),
    );
    await expectLater(
      session.prepareTunnel(deviceId: 'fixture-device', strictAntiMitm: false),
      throwsA(
        isA<model.AuthFailure>().having(
          (failure) => failure.code,
          'code',
          'sessionMissing',
        ),
      ),
    );
    await expectLater(
      session.verifyAtrust(deviceId: 'fixture-device', strictAntiMitm: false),
      throwsA(
        isA<model.AuthFailure>().having(
          (failure) => failure.code,
          'code',
          'sessionMissing',
        ),
      ),
    );
    session.close();
    await expectLater(
      session.prepareTunnel(deviceId: 'fixture-device', strictAntiMitm: true),
      throwsA(
        isA<model.AuthFailure>().having(
          (failure) => failure.code,
          'code',
          'closed',
        ),
      ),
    );
  });
}
