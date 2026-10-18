import 'package:flutter/material.dart';

import '../core/settings/settings_store.dart';
import 'shu_surfaces.dart';

/// Failure keeps the current page and committed values; callers stop follow-up actions.
Future<bool> runShuSettingsCommand(
  BuildContext context,
  Future<void> Function() command,
) async {
  try {
    await command();
    return true;
  } on ShuSettingsException catch (error) {
    if (context.mounted) showShuSnack(context, '设置保存失败：${error.message}');
    return false;
  }
}

Future<bool> saveShuSetting(
  BuildContext context,
  SettingsStore settings,
  SettingKey key,
  Object value,
) => runShuSettingsCommand(context, () => settings.save(key, value));
