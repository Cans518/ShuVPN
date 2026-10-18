import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:shuvpn/core/auth/account_authentication.dart';
import 'package:shuvpn/core/rust/generated/api/atrust_auth.dart' as native;
import 'package:shuvpn/core/rust/generated/auth/model.dart' as model;
import 'package:shuvpn/core/rust/rust_account_center.dart';
import 'package:shuvpn/core/rust/rust_connection.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

import '../native_backend_stub.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustCore.initialize);

  test('desktop native attempt rejects legacy account objects before recovery or preparation', () async {
    const legacy = _LegacyAccount();
    final attempt = RustConnectionBackend(systemProxyDirectory: '\u0000');
    addTearDown(attempt.dispose);
    await expectLater(
      attempt.connect(
        auth: legacy,
        server: 'http://fixture.invalid/',
        timeout: const Duration(seconds: 1),
        proxies: noNativeServices,
      ),
      throwsA(
        isA<StateError>().having(
          (error) => error.message,
          'message',
          contains('原生账户'),
        ),
      ),
    );
  });

  test(
    'native account adopts the existing device and owns durable generation',
    () async {
      SharedPreferences.setMockInitialValues({
        'atrust_device_id': 'existing-device',
      });
      final preferences = await SharedPreferences.getInstance();
      final directory = await Directory.systemTemp.createTemp(
        'shuvpn-device-owner-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final restored = await RustAccountCenter.open(
        directory.path,
        preferences,
      );
      expect(restored.maskedDeviceId, 'exis••••vice');
      expect(preferences.containsKey('atrust_device_id'), isFalse);
      restored.dispose();
      final reopened = await RustAccountCenter.open(
        directory.path,
        preferences,
      );
      expect(reopened.maskedDeviceId, 'exis••••vice');
      reopened.dispose();
      SharedPreferences.setMockInitialValues({});
      final freshPreferences = await SharedPreferences.getInstance();
      final freshDirectory = await Directory.systemTemp.createTemp(
        'shuvpn-new-device-owner-',
      );
      addTearDown(() => freshDirectory.delete(recursive: true));
      final fresh = await RustAccountCenter.open(
        freshDirectory.path,
        freshPreferences,
      );
      addTearDown(fresh.dispose);
      expect(
        fresh.maskedDeviceId,
        matches(RegExp(r'^[0-9a-f]{4}••••[0-9a-f]{4}$')),
      );
      expect(freshPreferences.containsKey('atrust_device_id'), isFalse);
    },
  );

  test(
    'full native OAuth exchange returns scoped cookies and reuses login',
    () async {
      final fixture = await _AuthFixture.start();
      addTearDown(fixture.close);
      final client = await fixture.client();
      addTearDown(client.dispose);
      final session = await client.signIn(seed: fixture.seed());
      expect(session.username, 'test-account');
      expect(session.sid, 'native-sid');
      expect(session.loginDomain, 'gateway-domain');
      expect(session.ticket, 'test-ticket');
      expect(
        session.gatewayCookies
            .singleWhere((cookie) => cookie.name == 'gateway_scoped')
            .path,
        '/passport',
      );
      expect(
        session.gatewayCookies
            .singleWhere((cookie) => cookie.name == 'sid')
            .value,
        'native-sid',
      );
      expect(
        session.ssoCookies
            .singleWhere((cookie) => cookie.name == 'SHU_OAUTH2')
            .value,
        'sso-refreshed',
      );
      expect(fixture.paths, [
        '/passport/v1/public/authConfig',
        '/oauth/authorize',
        '/passport/v1/auth/httpsOauth2',
        '/passport/v1/public/authConfig',
        '/controller/v1/public/reportEnv',
        '/passport/v1/auth/authCheck',
        '/passport/v1/user/onlineInfo',
      ]);
      expect(fixture.ssoCookies.single, 'SHU_OAUTH2=sso-original');
      expect(
        fixture.gatewayCookies.every(
          (header) => !header.contains('SHU_OAUTH2'),
        ),
        isTrue,
      );
      expect(fixture.report?['deviceId'], 'test-device');
      final endpoint =
          (fixture.report?['env'] as Map<String, Object?>?)?['endpoint']
              as Map<String, Object?>?;
      expect(endpoint?['device_id'], 'test-device');
      expect(fixture.reportCsrf, 'csrf-after');
      expect(fixture.reportCookies, isNot(contains('gateway_scoped')));
      final second = await client.signIn(
        seed: model.AuthSeed(
          deviceId: 'test-device',
          gatewayCookies: [
            ...session.gatewayCookies,
            model.AuthCookie(
              name: 'expired-export',
              value: 'old',
              domain: fixture.gateway.host,
              path: '/',
              expiresUnixSeconds: 946684800,
            ),
          ],
          ssoCookies: session.ssoCookies,
        ),
      );
      expect(second.ticket, isEmpty);
      expect(
        second.gatewayCookies.any((cookie) => cookie.name == 'expired-export'),
        isFalse,
      );
      expect(
        fixture.gatewayCookies.every(
          (header) => !header.contains('expired-export'),
        ),
        isTrue,
      );
      expect(fixture.paths.skip(7), [
        '/passport/v1/public/authConfig',
        '/passport/v1/user/onlineInfo',
      ]);
    },
  );

  test(
    'two callers share one serialized native authentication lifecycle',
    () async {
      final fixture = await _AuthFixture.start();
      fixture.pauseFirstRequest = true;
      addTearDown(fixture.close);
      final client = await fixture.client();
      addTearDown(client.dispose);
      final first = client.signIn(seed: fixture.seed());
      await fixture.entered.future.timeout(const Duration(seconds: 5));
      final second = client.signIn(seed: fixture.seed());
      fixture.continueFirstRequest.complete();
      final sessions = await Future.wait([first, second]);
      expect(sessions.first.ticket, 'test-ticket');
      expect(sessions.last.ticket, isEmpty);
      expect(
        fixture.paths.where((path) => path == '/oauth/authorize'),
        hasLength(1),
      );
    },
  );

  test('same-name SSO cookies retain paths in the native exchange', () async {
    final fixture = await _AuthFixture.start();
    fixture.deleteScoped = true;
    addTearDown(fixture.close);
    final client = await fixture.client();
    addTearDown(client.dispose);
    final seed = fixture.seed();
    final session = await client.signIn(
      seed: model.AuthSeed(
        deviceId: seed.deviceId,
        gatewayCookies: seed.gatewayCookies,
        ssoCookies: [
          ...seed.ssoCookies,
          model.AuthCookie(
            name: 'session',
            value: 'root-value',
            domain: fixture.sso.host,
            path: '/',
          ),
          model.AuthCookie(
            name: 'session',
            value: 'oauth-value',
            domain: fixture.sso.host,
            path: '/oauth',
          ),
        ],
      ),
    );
    expect(fixture.ssoCookies.single, startsWith('session=oauth-value; '));
    expect(fixture.ssoCookies.single, contains('session=root-value'));
    final cookies = session.ssoCookies
        .where((cookie) => cookie.name == 'session')
        .toList();
    expect(cookies, hasLength(1));
    expect(cookies.single.path, '/');
    expect(cookies.single.value, 'root-value');
  });

  test('native typed authentication errors cross the FFI boundary', () async {
    final fixture = await _AuthFixture.start();
    fixture.challenge = true;
    addTearDown(fixture.close);
    final client = await fixture.client();
    addTearDown(client.dispose);
    await expectLater(
      client.signIn(seed: fixture.seed()),
      throwsA(
        isA<model.AuthFailure>().having(
          (failure) => failure.code,
          'code',
          'atrustChallenge',
        ),
      ),
    );
  });

  test('native cancellation rejects active and queued authentication without returning sessions', () async {
    final fixture = await _AuthFixture.start();
    fixture.stall = true;
    addTearDown(fixture.close);
    final client = await fixture.client();
    addTearDown(client.dispose);
    final failures = Future.wait([
      expectLater(
        client.signIn(seed: fixture.seed()),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'cancelled',
          ),
        ),
      ),
      expectLater(
        client.signIn(seed: fixture.seed()),
        throwsA(
          isA<model.AuthFailure>().having(
            (failure) => failure.code,
            'code',
            'cancelled',
          ),
        ),
      ),
    ]);
    await fixture.entered.future.timeout(const Duration(seconds: 5));
    client.cancel();
    await failures.timeout(const Duration(seconds: 2));
    expect(fixture.paths, hasLength(1));
  });

  test('Rust config and error data cross the actual FFI boundary', () async {
    await expectLater(
      native.ATrustAuthClient.newInstance(
        config: const model.AuthConfig(
          gatewayBase: 'http://remote.invalid/',
          ssoBase: 'https://sso.invalid/',
          clientId: 'test',
          redirectUri: 'http://remote.invalid/passport/v1/auth/httpsOauth2',
          scope: '',
          fallbackLoginDomain: 'fallback',
          userAgent: 'ShuVPN test',
        ),
      ),
      throwsA(
        isA<model.AuthFailure>().having(
          (failure) => failure.code,
          'code',
          'atrustConfig',
        ),
      ),
    );
  });

  test(
    'native resource preparation rotates cookies and builds the plan',
    () async {
      final fixture = await _AuthFixture.start();
      addTearDown(fixture.close);
      final client = await fixture.client();
      addTearDown(client.dispose);
      final prepared = await client.prepareTunnel(
        seed: fixture.seed(),
        options: _options(),
      );
      final plan = jsonDecode(prepared.planJson) as Map<String, Object?>;
      expect(plan['sid'], 'resource-sid');
      expect(plan['deviceId'], 'test-device');
      expect(plan['processPlatform'], Platform.isWindows ? 'Windows' : 'Linux');
      expect((plan['nodes'] as Map)['campus'], ['127.0.0.1:441']);
      expect(plan['dnsServers'], ['10.99.0.53']);
      expect((plan['routes'] as List).single['host'], 'fixture.shu.edu.cn');
      expect(prepared.warnings, isEmpty);
      expect(
        prepared.session.gatewayCookies
            .singleWhere((cookie) => cookie.name == 'sid')
            .value,
        'resource-sid',
      );
      expect(fixture.paths.last, '/controller/v1/user/clientResource');
      expect(fixture.resourceCsrf, 'csrf-before');
      expect(fixture.resourceBody, {
        'resourceType': {
          'sdpPolicy': {},
          'appList': {},
          'favoriteAppList': {},
          'featureCenter': {},
          'uemSpace': {
            'params': {'action': 'login'},
          },
        },
      });
      client.cancel();
    },
  );

  test('native attempt cancellation stops resource preparation without publishing a result', () async {
    final fixture = await _AuthFixture.start();
    fixture.stallResources = true;
    addTearDown(fixture.close);
    final client = await fixture.client();
    addTearDown(client.dispose);
    final failure = expectLater(
      client.prepareTunnel(seed: fixture.seed(), options: _options()),
      throwsA(
        isA<model.AuthFailure>().having(
          (failure) => failure.code,
          'code',
          'cancelled',
        ),
      ),
    );
    await fixture.entered.future.timeout(const Duration(seconds: 5));
    client.cancel();
    await failure.timeout(const Duration(seconds: 2));
    expect(fixture.paths.last, '/controller/v1/user/clientResource');
    expect(fixture.paths, hasLength(9));
  });
}

model.TunnelOptions _options() => model.TunnelOptions(
  connectionId: 'test-connection',
  signKey: Uint8List.fromList(List.generate(32, (index) => index + 1)),
  processPlatform: Platform.isWindows ? 'Windows' : 'Linux',
  strictAntiMitm: false,
);

class _LegacyAccount implements AccountAuthentication {
  const _LegacyAccount();
  @override
  bool get hasSession => true;
}

class _AuthFixture {
  _AuthFixture(this._gateway, this._sso);

  final HttpServer _gateway;
  final HttpServer _sso;
  final paths = <String>[];
  final gatewayCookies = <String>[];
  final ssoCookies = <String>[];
  final entered = Completer<void>();
  final continueFirstRequest = Completer<void>();
  bool pauseFirstRequest = false;
  bool loggedIn = false;
  bool challenge = false;
  bool stall = false;
  bool stallResources = false;
  bool deleteScoped = false;
  Map<String, Object?>? report;
  String? reportCsrf;
  String? reportCookies;
  Map<String, Object?>? resourceBody;
  String? resourceCsrf;

  Uri get gateway => Uri.parse('http://127.0.0.1:${_gateway.port}/');
  Uri get sso => Uri.parse('http://localhost:${_sso.port}/');

  static Future<_AuthFixture> start() async {
    final gateway = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final sso = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final fixture = _AuthFixture(gateway, sso);
    gateway.listen(fixture._handle);
    sso.listen(fixture._handle);
    return fixture;
  }

  model.AuthSeed seed() => model.AuthSeed(
    deviceId: 'test-device',
    gatewayCookies: const [],
    ssoCookies: [
      model.AuthCookie(
        name: 'SHU_OAUTH2',
        value: 'sso-original',
        domain: sso.host,
        path: '/',
      ),
    ],
  );

  Future<native.ATrustAuthClient> client() =>
      native.ATrustAuthClient.newInstance(
        config: model.AuthConfig(
          gatewayBase: gateway.toString(),
          ssoBase: sso.toString(),
          clientId: 'u5kTNw6T59kw3wkrwZ76e44kBec4HTvv',
          redirectUri: gateway
              .resolve('/passport/v1/auth/httpsOauth2')
              .toString(),
          scope: '',
          fallbackLoginDomain: 'customOAuth76881',
          userAgent: 'ShuVPN-auth-fixture',
        ),
      );

  Future<void> close() async {
    if (!continueFirstRequest.isCompleted) continueFirstRequest.complete();
    await _gateway.close(force: true);
    await _sso.close(force: true);
  }

  Future<void> _handle(HttpRequest request) async {
    paths.add(request.uri.path);
    final cookie = request.headers.value(HttpHeaders.cookieHeader) ?? '';
    if (request.uri.path.startsWith('/oauth/')) {
      ssoCookies.add(cookie);
    } else {
      gatewayCookies.add(cookie);
    }
    if (pauseFirstRequest && paths.length == 1) {
      if (!entered.isCompleted) entered.complete();
      await continueFirstRequest.future;
    }
    if (stall ||
        (stallResources &&
            request.uri.path == '/controller/v1/user/clientResource')) {
      if (!entered.isCompleted) entered.complete();
      return;
    }
    final response = request.response;
    response.headers.contentType = ContentType.json;
    switch (request.uri.path) {
      case '/passport/v1/public/authConfig':
        final after = request.uri.queryParameters['mod'] == '1';
        response.cookies.add(Cookie('gateway_hop', 'bootstrap')..path = '/');
        response.cookies.add(
          Cookie('gateway_scoped', 'passport')..path = '/passport',
        );
        response.write(
          jsonEncode({
            'code': 0,
            'data': {
              'isLogin': loggedIn || after ? 1 : 0,
              'csrfToken': after ? 'csrf-after' : 'csrf-before',
              'authServerInfoList': [
                {
                  'authType': 'auth/httpsOauth2',
                  'loginDomain': 'gateway-domain',
                },
              ],
            },
          }),
        );
      case '/oauth/authorize':
        response.statusCode = HttpStatus.found;
        response.headers.set(
          HttpHeaders.locationHeader,
          gateway.resolve('/passport/v1/auth/httpsOauth2?code=test-code'),
        );
        response.cookies.add(Cookie('SHU_OAUTH2', 'sso-refreshed')..path = '/');
        if (deleteScoped) {
          response.cookies.add(
            Cookie('session', '')
              ..path = '/oauth'
              ..maxAge = 0,
          );
        }
      case '/passport/v1/auth/httpsOauth2':
        expect(request.uri.queryParameters['sfDomain'], 'gateway-domain');
        expect(request.uri.queryParameters['state'], 'null');
        response.statusCode = HttpStatus.found;
        response.headers.set(
          HttpHeaders.locationHeader,
          gateway
              .resolve('/portal/shortcut.html')
              .replace(queryParameters: {'data': '{"ticket":"test-ticket"}'}),
        );
      case '/controller/v1/public/reportEnv':
        report = jsonDecode(
          await utf8.decoder.bind(request).join(),
        ) as Map<String, Object?>;
        reportCsrf = request.headers.value('x-csrf-token');
        reportCookies = request.headers.value(HttpHeaders.cookieHeader);
        response.write('{"code":0,"data":{}}');
      case '/passport/v1/auth/authCheck':
        response.write(
          jsonEncode({
            'code': 0,
            'data': {'nextService': challenge ? 'auth/sendSms' : ''},
          }),
        );
      case '/passport/v1/user/onlineInfo':
        loggedIn = true;
        response.cookies.add(Cookie('sid', 'native-sid')..path = '/');
        response.write('{"code":0,"data":{"username":"test-account"}}');
      case '/controller/v1/user/clientResource':
        resourceBody = jsonDecode(
          await utf8.decoder.bind(request).join(),
        ) as Map<String, Object?>;
        resourceCsrf = request.headers.value('x-csrf-token');
        response.cookies.add(Cookie('sid', 'resource-sid')..path = '/');
        response.write(
          jsonEncode({
            'code': 0,
            'data': {
              'appList': {
                'data': {
                  'appInfo': [
                    {
                      'apps': [
                        {
                          'id': 'campus-app',
                          'nodeGroupId': 'campus',
                          'accessModel': 'L3VPN',
                          'addressList': [
                            {
                              'host': 'fixture.shu.edu.cn',
                              'protocol': 'tcp',
                              'port': '443',
                            },
                          ],
                        },
                      ],
                    },
                  ],
                  'config': {
                    'nodeGroupConf': {
                      'majorNodeGroup': 'campus',
                      'nodeGroupList': [
                        {
                          'id': 'campus',
                          'addressInfo': [
                            {'address': '{{sdpcHost}}', 'type': 'wan'},
                          ],
                        },
                      ],
                    },
                  },
                },
              },
              'sdpPolicy': {
                'data': {
                  'clientOption': {
                    'dnsOption': {'firstDNS': '10.99.0.53'},
                  },
                },
              },
            },
          }),
        );
      default:
        response.statusCode = HttpStatus.notFound;
    }
    await response.close();
  }
}
