/// 连接读数的一行：`↑ 1.2 MB/s · ↓ 340 KB/s · 时延 142 ms`。
///
/// 连接页的副标题使用同一套速率和时延格式。
///
/// [latencyMs] 为 `null` 时写 `—` 而**不是** `0 ms`：还没有一次成功的建连
/// 采样时，`0` 是一个测量结果，而这里根本没有测量。
String formatConnectionTelemetry({
  required double upBytesPerSecond,
  required double downBytesPerSecond,
  required double? latencyMs,
}) =>
    '↑ ${formatRate(upBytesPerSecond)}'
    ' · ↓ ${formatRate(downBytesPerSecond)}'
    ' · 时延 ${latencyMs == null ? '—' : '${latencyMs.round()} ms'}';

/// 1024 进制的流量读数，与连接页和日志使用同一量纲。
String formatByteCount(int bytes) {
  if (bytes < 1024) return '$bytes B';
  final kb = bytes / 1024;
  if (kb < 1024) return '${kb.toStringAsFixed(kb < 10 ? 1 : 0)} KB';
  final mb = kb / 1024;
  if (mb < 1024) return '${mb.toStringAsFixed(mb < 10 ? 1 : 0)} MB';
  return '${(mb / 1024).toStringAsFixed(1)} GB';
}

/// 建连耗时：`820ms` / `3.1s` / `1m12s`。
String formatDuration(Duration duration) {
  final millis = duration.inMilliseconds;
  if (millis < 1000) return '${millis}ms';
  final seconds = millis / 1000;
  if (seconds < 60) return '${seconds.toStringAsFixed(1)}s';
  final minutes = duration.inMinutes;
  return '${minutes}m${duration.inSeconds - minutes * 60}s';
}

/// 每秒流量；无效或尚未采样的读数显示为零。
String formatRate(double bytesPerSecond) {
  if (bytesPerSecond.isNaN || bytesPerSecond <= 0) return '0 B/s';
  return '${formatByteCount(bytesPerSecond.round())}/s';
}
