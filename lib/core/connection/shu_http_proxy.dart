import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_sangfor/flutter_sangfor.dart';

import '../logging/shu_log.dart';

/// 本机 HTTP 代理，把所有请求转发进 aTrust 隧道。
///
/// ## 为什么除了 SOCKS5 还要这一个
///
/// Android 的**系统代理只支持 HTTP**：设置里的「WLAN → 代理」只有 HTTP 一个
/// 选项。没有任何系统开关能把「所有应用都用 SOCKS5」这件事表达出来 ——
/// 于是把浏览器指向一个 SOCKS5 端口，结果就是**一个连接都不会进来**：
/// 那一侧从来不知道要去连它。这不是 SOCKS5 实现的问题，是平台的表达力问题。
///
/// 所以两条通道并存，各自对着不同的消费者：
///
/// | 通道 | 谁在用 |
/// | :--- | :--- |
/// | HTTP（本文件） | 只会填 HTTP(S) 代理的应用（含手动设的系统代理） |
/// | SOCKS5 | 自己支持 SOCKS5 的应用（Firefox、Telegram、部分命令行工具） |
///
/// 两者**共用同一个 [SangforTcpDialer]**，也就是共用同一条隧道、同一套
/// 分流判定（`_dialATrust`），所以「哪个目标走隧道、哪个直连」在两边的答案
/// 一定一样。
///
/// ⚠️ 这条通道**不是**系统 VPN 的出口：VPN 那一半的 TCP 由库里的本机终结器
/// 逐流接管（`ATrustTcpTermination`），所以它不需要谁去当系统代理，也不会
/// 把这条通道强制拉起来。
///
/// ## 支持哪些请求
///
/// * `CONNECT host:port` —— HTTPS 与一切需要隧道的 TCP，直接升级成裸字节管道；
/// * 绝对形式的普通 HTTP（`GET http://host/path`）—— 改写请求行成 origin
///   形式后转发，请求体与后续字节原样透传。
///
/// 不缓存、不改写正文、不看响应 —— 它是一个管道，不是中间人。
class ShuHttpProxy {
  ShuHttpProxy({
    required this.dialer,
    this.listenAddress,
    this.port = 0,
    this.dialTimeout = const Duration(seconds: 30),
    this.onDialError,
    this.cancellationToken,
  }) : assert(port >= 0 && port <= 65535);

  final SangforTcpDialer dialer;
  final InternetAddress? listenAddress;
  final int port;
  final Duration dialTimeout;
  final void Function(String host, int port, Object error)? onDialError;

  /// 取消时停止接受连接，并拆掉所有在途会话（含正在拨号的那些）。
  final SangforCancellationToken? cancellationToken;

  ServerSocket? _server;
  final Set<_HttpProxySession> _sessions = <_HttpProxySession>{};

  bool get isRunning => _server != null;

  int? get boundPort => _server?.port;

  /// 绑定监听，返回真正绑上的端口。
  ///
  /// 首选 [port]，被占用时**退到系统分配的端口**。退一步是故意的：「相邻
  /// 端口恰好被别的程序占着」不该是整条代理起不来的理由 —— 端口退到哪里，
  /// 界面上的地址就跟着显示哪里（`httpListenAddress`）。
  Future<int> start() async {
    if (_server != null) throw StateError('HTTP proxy is already running');
    final address = listenAddress ?? InternetAddress.loopbackIPv4;
    ServerSocket server;
    try {
      server = await ServerSocket.bind(address, port);
    } on SocketException catch (error) {
      if (port == 0) rethrow;
      ShuLog.w(
        ShuLogTag.proxy,
        'HTTP 代理端口 $port 被占用 · ${error.osError?.message ?? error.message}'
        ' · 改用系统分配的端口',
      );
      server = await ServerSocket.bind(address, 0);
    }
    server.listen(
      _accept,
      onError: (Object error) {
        // 监听套接字自己的错误通过会话关闭体现，这里没有可做的事。
      },
    );
    _server = server;
    final token = cancellationToken;
    if (token != null) {
      unawaited(token.whenCancelled.then((_) => close()));
    }
    return server.port;
  }

  void _accept(Socket socket) {
    // 代理的每一次转发都是「小请求、等响应」的形态，Nagle 会平白加上
    // 几十毫秒 —— 浏览器开一堆连接时这个延迟非常明显。
    try {
      socket.setOption(SocketOption.tcpNoDelay, true);
    } on Object {
      // 个别平台不允许在 accept 出来的套接字上设它，不影响正确性。
    }
    final session = _HttpProxySession(
      socket: socket,
      dialer: dialer,
      dialTimeout: dialTimeout,
      onDialError: onDialError,
      onFinished: _sessions.remove,
      cancellationToken: cancellationToken,
    );
    _sessions.add(session);
    session.start();
  }

  Future<void> close() async {
    final server = _server;
    _server = null;
    await server?.close();
    final sessions = List<_HttpProxySession>.of(_sessions);
    _sessions.clear();
    for (final session in sessions) {
      await session.close();
    }
  }
}

/// 一条来自应用的 HTTP 代理连接。
class _HttpProxySession {
  _HttpProxySession({
    required this.socket,
    required this.dialer,
    required this.dialTimeout,
    required this.onDialError,
    required this.onFinished,
    this.cancellationToken,
  });

  /// 请求头上限。超过它说明对面根本不是 HTTP 代理客户端（或者有人往里灌
  /// 垃圾），直接拒掉，别把内存交出去。
  static const int maxHeaderBytes = 16 * 1024;

  final Socket socket;
  final SangforTcpDialer dialer;
  final Duration dialTimeout;
  final void Function(String host, int port, Object error)? onDialError;
  final void Function(_HttpProxySession session) onFinished;
  final SangforCancellationToken? cancellationToken;

  /// 上行排队超过这个量就暂停读客户端。
  ///
  /// 回环 socket 的读速度是几 GB/s，而隧道往往只有几十 Mbps；不设上限的话，
  /// 一个上传请求的几 MB 正文会在几毫秒内全灌进这里，客户端看到的「已写出」
  /// 与隧道实际吞吐完全脱钩（测速会报出回环速度）。暂停之后由 TCP 把背压
  /// 传回客户端，写出去的量重新等于隧道吃掉多少。
  static const int _backpressureBytes = 256 * 1024;

  /// 下游写入累计超过这个量就先 `flush` 再继续读上游。
  ///
  /// `Socket.add` 同样只是往待写队列里塞；客户端读得稍慢一点，队列就会
  /// 一路涨。测速下行每段结束都会强制断开客户端，队列里那一坨在断开前
  /// 都还活着 —— 六条流叠加起来就是一次 OOM。
  static const int _downstreamFlushBytes = 512 * 1024;

  /// 拨号期间最多先攒这么多正文。
  ///
  /// `pause()` 之后的字节不会再进来，这个上限只是兜住「暂停与数据到达"
  /// 之间的那一次竞态，以及不认 `pause` 的实现。
  static const int _bodyBufferBytes = 2 * 1024 * 1024;

  final BytesBuilder _head = BytesBuilder(copy: false);
  SangforTcpStream? _upstream;
  Future<void> _sendChain = Future<void>.value();
  StreamSubscription<Uint8List>? _upstreamSubscription;
  StreamSubscription<List<int>>? _clientSubscription;
  bool _tunnelling = false;
  bool _requestStarted = false;
  bool _closed = false;
  int _queuedBytes = 0;
  bool _pausedForBackpressure = false;
  int _pendingDownstream = 0;
  bool _flushingDownstream = false;

  void start() {
    _clientSubscription = socket.listen(
      (chunk) => _onClientData(Uint8List.fromList(chunk)),
      onDone: _onClientDone,
      onError: (Object _) => close(),
    );
    // 写失败（客户端中途离开）通过 `done` 报出来，不接住就是一条未处理异常。
    unawaited(socket.done.then<void>((_) {}, onError: (Object _) {}));
  }

  void _onClientData(Uint8List chunk) {
    if (_closed) return;
    if (_tunnelling) {
      _enqueueSend(chunk);
      return;
    }
    _head.add(chunk);
    if (_requestStarted) {
      // 请求头已经发出去、隧道还在拨号。这一段是正文，先攒着并暂停读取，
      // 交给 TCP 去背压 —— 否则上传正文会先全落到这里。
      _clientSubscription?.pause();
      if (_head.length > _bodyBufferBytes) {
        // 暂停之后理论上到不了这里；真到了说明对端不认背压，宁可拒掉也
        // 不能无界往下攒。
        unawaited(_fail(413, 'Payload Too Large'));
      }
      return;
    }
    if (_head.length > maxHeaderBytes) {
      unawaited(_fail(431, 'Request Header Fields Too Large'));
      return;
    }
    _parseRequest();
  }

  void _parseRequest() {
    final bytes = _head.toBytes();
    final end = _headerEnd(bytes);
    if (end < 0) return;

    final lines = ascii
        .decode(bytes.sublist(0, end), allowInvalid: true)
        .split('\r\n');
    if (lines.isEmpty) {
      unawaited(_fail(400, 'Bad Request'));
      return;
    }
    final parts = lines.first.split(' ');
    if (parts.length < 3) {
      unawaited(_fail(400, 'Bad Request'));
      return;
    }

    // 头之后的字节属于正文/管道，一个都不能丢 —— 客户端常常把请求体
    // 和请求头一起写出来（Pipelining）。
    final carried = Uint8List.fromList(bytes.sublist(end + 4));
    // 一个连接只解析一次。解析完之后陆续到的字节是正文，由 `_openTunnel`
    // 在隧道建立后按序送出；再解析一遍会把正文当成第二个请求，还会把同一条
    // 请求发两次。
    //
    // 必须清掉 `_head`：`toBytes()` 返回的是副本、**不清空**缓冲区，留着的话
    // 下面 `_openTunnel` 会把整条请求再发一遍 —— 服务端就会看到「同一条
    // 请求来了两次」，多余的响应回到客户端就是 `unsolicited response`。
    _head.clear();
    _requestStarted = true;
    final method = parts[0].toUpperCase();
    final target = parts[1];

    if (method == 'CONNECT') {
      final authority = _splitAuthority(target, 443);
      if (authority == null) {
        unawaited(_fail(400, 'Bad Request'));
        return;
      }
      unawaited(
        _openTunnel(
          authority.$1,
          authority.$2,
          carried,
          replyTunnelEstablished: true,
        ),
      );
      return;
    }

    // `GET http://host/path HTTP/1.1`：把请求行改写成 origin 形式。
    final uri = Uri.tryParse(target);
    if (uri == null || uri.host.isEmpty) {
      unawaited(_fail(400, 'Bad Request'));
      return;
    }
    final port = uri.hasPort ? uri.port : (uri.scheme == 'https' ? 443 : 80);
    final rewritten = StringBuffer()
      ..write(method)
      ..write(' ')
      ..write(uri.path.isEmpty ? '/' : uri.path);
    if (uri.hasQuery) rewritten.write('?${uri.query}');
    rewritten.write(' ${parts[2]}');
    for (final header in lines.skip(1)) {
      // `Proxy-Connection` 是代理专用的跳接头，转发出去只会让对面困惑。
      if (header.toLowerCase().startsWith('proxy-connection:')) continue;
      rewritten
        ..write('\r\n')
        ..write(header);
    }
    rewritten.write('\r\n\r\n');

    unawaited(
      _openTunnel(
        uri.host,
        port,
        Uint8List.fromList(<int>[
          ...ascii.encode(rewritten.toString()),
          ...carried,
        ]),
        replyTunnelEstablished: false,
      ),
    );
  }

  Future<void> _openTunnel(
    String host,
    int port,
    Uint8List carried, {
    required bool replyTunnelEstablished,
  }) async {
    SangforTcpStream stream;
    try {
      final token = cancellationToken;
      final dial = token == null
          ? dialer(host, port)
          : token.race(dialer(host, port));
      stream = await dial.timeout(dialTimeout);
    } on Object catch (error) {
      onDialError?.call(host, port, error);
      // 502 是代理的标准答复：**代理**连不上上游，不是客户端请求有错。
      // 用它而不是 500，用户与工具都能立刻分清是谁的问题。
      await _fail(502, 'Bad Gateway');
      return;
    }
    if (_closed) {
      await stream.close();
      return;
    }
    _upstream = stream;
    _tunnelling = true;
    if (replyTunnelEstablished) {
      try {
        socket.add(ascii.encode('HTTP/1.1 200 Connection Established\r\n\r\n'));
      } on Object {
        // 客户端在这条隧道拨号完成之前就走了。
        await close();
        await stream.close();
        return;
      }
    }
    _upstreamSubscription = stream.incoming.listen(
      (chunk) => _pumpDownstream(chunk),
      onDone: () async {
        try {
          await socket.flush();
          await socket.close();
        } on Object {
          // 客户端可能已经走了。
        }
      },
      onError: (Object _) => close(),
    );
    // 拨号期间暂停过读取，攒下的正文要排在 `carried` 后面。
    //
    // 先恢复读取再入队：`_enqueueSend` 自己可能因为背压再次暂停，顺序反了
    // 会把那次暂停一并撤掉。
    final pending = _head.toBytes();
    _head.clear();
    _clientSubscription?.resume();
    if (carried.isNotEmpty) _enqueueSend(carried);
    if (pending.isNotEmpty) _enqueueSend(pending);
  }

  /// 把上游字节写给客户端，必要时用 `flush` 把队列压回内核。
  ///
  /// 暂停的是**上游**订阅：客户端跟不上时不再继续从隧道里拿数据，
  /// 队列于是被钉在 [_downstreamFlushBytes] 附近。
  void _pumpDownstream(List<int> chunk) {
    if (_closed) return;
    try {
      socket.add(chunk);
    } on Object {
      // 客户端已经走了。测速每一段结束时都会强制关掉客户端，这是常态。
      unawaited(close());
      return;
    }
    _pendingDownstream += chunk.length;
    if (_flushingDownstream || _pendingDownstream < _downstreamFlushBytes) {
      return;
    }
    _flushingDownstream = true;
    _pendingDownstream = 0;
    _upstreamSubscription?.pause();
    unawaited(
      socket
          .flush()
          .then<void>(
            (_) {},
            onError: (Object _) {
              if (!_closed) unawaited(close());
            },
          )
          .whenComplete(() {
            _flushingDownstream = false;
            if (!_closed) _upstreamSubscription?.resume();
          }),
    );
  }

  /// 串行化上行写入：分块必须保序，每块写完再写下一块。
  ///
  /// 排队超过 [_backpressureBytes] 时暂停客户端读取 —— 这是「测出来的上行
  /// 速度」与「隧道实际吐出的速度」保持同一个数的前提。
  void _enqueueSend(Uint8List chunk) {
    final stream = _upstream;
    if (stream == null) return;
    _queuedBytes += chunk.length;
    if (_queuedBytes > _backpressureBytes && !_pausedForBackpressure) {
      _pausedForBackpressure = true;
      _clientSubscription?.pause();
    }
    _sendChain = _sendChain
        .then((_) async {
          if (_closed || stream.isClosed) return;
          await stream.send(chunk);
        })
        .catchError((Object _) {
          // 发送失败会经由流的错误路径把会话拆掉。
        })
        .whenComplete(() {
          _queuedBytes -= chunk.length;
          if (_queuedBytes < 0) _queuedBytes = 0;
          _resumeIfDrained();
        });
  }

  /// 队列排空后恢复读取。
  ///
  /// 只有「因为背压暂停过」才恢复：拨号阶段那次暂停由 `_openTunnel` 自己
  /// 收尾，这里顺手恢复会把它提前放掉。
  void _resumeIfDrained() {
    if (!_pausedForBackpressure || _queuedBytes > 0) return;
    _pausedForBackpressure = false;
    _clientSubscription?.resume();
  }

  Future<void> _onClientDone() async {
    final stream = _upstream;
    if (stream == null) {
      await close();
      return;
    }
    try {
      await _sendChain;
      await stream.closeWrite();
    } on Object {
      await close();
    }
  }

  Future<void> _fail(int status, String reason) async {
    if (_closed) return;
    try {
      socket.add(
        ascii.encode(
          'HTTP/1.1 $status $reason\r\n'
          'Content-Length: 0\r\n'
          'Connection: close\r\n\r\n',
        ),
      );
      await socket.flush();
    } on Object {
      // 对面已经断了，那就只是关掉而已。
    }
    await close();
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    _clientSubscription?.cancel();
    await _upstreamSubscription?.cancel();
    socket.destroy();
    await _upstream?.close();
    onFinished(this);
  }

  /// `\r\n\r\n` 里第一个 `\r` 的下标；没找到返回 `-1`。
  static int _headerEnd(Uint8List bytes) {
    for (var index = 3; index < bytes.length; index++) {
      if (bytes[index] == 0x0a &&
          bytes[index - 1] == 0x0d &&
          bytes[index - 2] == 0x0a &&
          bytes[index - 3] == 0x0d) {
        return index - 3;
      }
    }
    return -1;
  }

  /// `host:port` / `[v6]:port` / 裸主机名（用 [fallbackPort]）→ `(host, port)`。
  static (String, int)? _splitAuthority(String value, int fallbackPort) {
    var host = value;
    var port = fallbackPort;
    if (value.startsWith('[')) {
      final close = value.indexOf(']');
      if (close < 0) return null;
      host = value.substring(1, close);
      final rest = value.substring(close + 1);
      if (rest.startsWith(':')) {
        port = int.tryParse(rest.substring(1)) ?? fallbackPort;
      }
    } else {
      final colon = value.lastIndexOf(':');
      if (colon > 0) {
        host = value.substring(0, colon);
        port = int.tryParse(value.substring(colon + 1)) ?? fallbackPort;
      }
    }
    if (host.isEmpty || port <= 0 || port > 65535) return null;
    return (host, port);
  }
}
