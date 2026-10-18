import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;
import 'package:shuvpn/core/rust/generated/api/announcements.dart' as native;
import 'package:shuvpn/core/rust/generated/announcements/model.dart' as model;
import 'package:shuvpn/core/rust/generated/frb_generated.dart';
import 'package:shuvpn/core/rust/rust_core.dart';

/// Optional public-site acceptance, deliberately limited to one list and three details.
Future<void> main(List<String> args) async {
  if (args.length != 1) {
    throw ArgumentError('Expected a packaged native library path');
  }
  await RustCore.initialize(
    externalLibrary: ExternalLibrary.open(File(args.single).absolute.path),
  );
  final client = await native.AnnouncementClient.newInstance();
  native.AnnouncementPrefetch? batch;
  try {
    final page = await client.fetchList();
    if (page.items.isEmpty) {
      throw StateError('Live announcement list was empty');
    }
    final selected = <model.Item>[];
    for (final item in page.items) {
      try {
        client.cachedDetail(url: item.url);
        selected.add(item);
      } on model.AnnouncementFailure catch (error) {
        if (error.code != 'origin' && error.code != 'url') rethrow;
      }
      if (selected.length == 3) break;
    }
    if (selected.isEmpty) throw StateError('No native-fetchable announcements');
    batch = await client.prefetch(items: selected);
    final previews = await batch
        .events()
        .timeout(const Duration(seconds: 45))
        .toList();
    if (previews.length != selected.length ||
        previews.any((preview) => preview.error != null)) {
      throw StateError(
        'Live preview failure: ${previews.map((preview) => preview.error).whereType<String>().join('; ')}',
      );
    }
    for (final preview in previews) {
      if (client.cachedDetail(url: preview.url) == null) {
        throw StateError(
          'Successful preview was not retained in the native detail cache',
        );
      }
    }
    stdout.writeln(
      jsonEncode({
        'observed_at': DateTime.now().toIso8601String(),
        'list_items': page.items.length,
        'sampled_details': selected.length,
        'preview_events': previews.length,
        'empty_previews': previews
            .where((preview) => preview.text?.isEmpty ?? true)
            .length,
        'next_page': page.nextPage,
      }),
    );
    stdout.writeln('ANNOUNCEMENTS_LIVE_NATIVE_LIST_PREFETCH_AND_CACHE_PASSED');
  } finally {
    try {
      batch?.cancel();
    } finally {
      batch?.dispose();
    }
    try {
      client.close();
    } finally {
      client.dispose();
    }
    ShuRustLib.dispose();
  }
}
