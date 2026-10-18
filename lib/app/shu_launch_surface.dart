import 'package:flutter/material.dart';

import 'theme.dart';

/// 应用的第一屏：**底色 + 居中的图标**，没有文字、没有指示器。
///
/// 第一帧显示静止的应用图标，颜色跟随当前主题。
class ShuLaunchSurface extends StatelessWidget {
  const ShuLaunchSurface({super.key});

  /// 图标边长（逻辑像素）。
  static const double iconSize = 112;

  @override
  Widget build(BuildContext context) {
    final colors = context.shuyoColors;
    // `ColoredBox` 而不是 `Scaffold`：这一屏下面没有 `Navigator` 之外的东西
    // 需要它（没有 AppBar、没有 SnackBar 通道、也不该有 SafeArea 的上下留白
    // 变化），而 `Scaffold` 会额外铺一层 `Material` 与手势区。
    return ColoredBox(
      color: colors.background,
      child: Center(
        child: Image.asset(
          iconAssetFor(colors.brightness),
          width: iconSize,
          height: iconSize,
          fit: BoxFit.contain,
          // 启动图标通常比 112dp 大得多（这里源图是 1024），缩小绘制的默认
          // 采样会让它发糊。这一档正是给「缩得很小的位图」准备的。
          filterQuality: FilterQuality.high,
        ),
      ),
    );
  }

  /// 该亮度用哪一张图。
  ///
  /// 浅色底用**蓝色**那一张（应用的主标识），深色底用**白色**那一张 ——
  /// 蓝色的对比度在近黑底上掉得厉害，而白色图标在纸白底上等于没有。
  ///
  /// 没有第三张：`assets/images/icon_clear_black.png` 是**不透明白底**上
  /// 的黑色图标，铺在纸白底上会露出一个白色方块，所以这里不用它。
  static String iconAssetFor(Brightness brightness) =>
      brightness == Brightness.dark
      ? 'assets/images/ic_launcher_white.png'
      : 'assets/images/icon_clear_blue.png';
}
