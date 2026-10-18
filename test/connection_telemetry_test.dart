import 'package:flutter_test/flutter_test.dart';
import 'package:shuvpn/core/connection/connection_telemetry.dart';

void main() {
  group('formatRate', () {
    test('速率与累计量共用同一套量纲，只是带上 /s', () {
      expect(formatRate(0), '0 B/s');
      expect(formatRate(-1), '0 B/s');
      expect(formatRate(double.nan), '0 B/s');
      expect(formatRate(812), '812 B/s');
      expect(formatRate(2048), '2.0 KB/s');
      expect(formatRate(3 * 1024 * 1024), '3.0 MB/s');
    });

    test('四舍五入到整字节，不出现 0.4 B/s 这种数', () {
      expect(formatRate(0.4), '0 B/s');
      expect(formatRate(0.6), '1 B/s');
    });
  });

  group('formatDuration', () {
    test('毫秒 / 秒 / 分三档', () {
      expect(formatDuration(const Duration(milliseconds: 0)), '0ms');
      expect(formatDuration(const Duration(milliseconds: 820)), '820ms');
      expect(formatDuration(const Duration(milliseconds: 3120)), '3.1s');
      expect(formatDuration(const Duration(seconds: 72)), '1m12s');
    });
  });

  group('formatByteCount', () {
    test('按 1024 进制换档，且和系统流量统计能对上', () {
      expect(formatByteCount(0), '0 B');
      expect(formatByteCount(812), '812 B');
      expect(formatByteCount(1024), '1.0 KB');
      expect(formatByteCount(20 * 1024), '20 KB');
      expect(formatByteCount(1024 * 1024), '1.0 MB');
      expect(formatByteCount(30 * 1024 * 1024), '30 MB');
      expect(formatByteCount(2 * 1024 * 1024 * 1024), '2.0 GB');
    });
  });

  group('formatConnectionTelemetry', () {
    test('上下行与时延合成一行，量纲与日志一致', () {
      expect(
        formatConnectionTelemetry(
          upBytesPerSecond: 2048,
          downBytesPerSecond: 3 * 1024 * 1024,
          latencyMs: 142,
        ),
        '↑ 2.0 KB/s · ↓ 3.0 MB/s · 时延 142 ms',
      );
    });

    test('还没有建连样本时写 — 而不是 0 ms', () {
      expect(
        formatConnectionTelemetry(
          upBytesPerSecond: 0,
          downBytesPerSecond: 0,
          latencyMs: null,
        ),
        '↑ 0 B/s · ↓ 0 B/s · 时延 —',
      );
    });

    test('时延四舍五入到整毫秒', () {
      expect(
        formatConnectionTelemetry(
          upBytesPerSecond: 0,
          downBytesPerSecond: 0,
          latencyMs: 142.4,
        ),
        endsWith('时延 142 ms'),
      );
      expect(
        formatConnectionTelemetry(
          upBytesPerSecond: 0,
          downBytesPerSecond: 0,
          latencyMs: 142.6,
        ),
        endsWith('时延 143 ms'),
      );
    });
  });
}
