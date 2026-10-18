# Linux 正式托盘验收

第66批，2026-10-17。使用 `lib/main.dart` 的完整 release 应用、真实 XFCE StatusNotifier 宿主和原生 DBusMenu。新建 Xvfb、DBus、Openbox 和 XDG profile，以普通用户运行；不读取真实用户账户，不替换 Rust 账户、设置或连接后端。

## 修复范围

固定依赖 `system_tray 2.0.3` 的 Linux `menu.cc` 中，运行时 label/enable/check setter 是空实现，且两个布尔参数检查为 string。Flutter mock channel 的成功返回不能证明 GTK 状态更新。原生观察实际复现了连接/断开仍启用、aTrust 未勾选的问题；等待菜单发布完成也不能使这些状态更新。

`linux/third_party/system_tray/menu.cc` 保留上游结构、MIT 许可证和 native ID，补齐当前应用使用的三个 setter，拒绝非法参数，并避免程序勾选产生用户选择回调。不通过反复重建菜单处理状态变化。Linux CMake 只替换这一编译单元，验证原始文件 SHA 后才构建，不改共享 pubcache 或 Windows 实现。许可证单独安装到 `data/licenses/system_tray/LICENSE`。运行时菜单图片替换仍是上游行为，不属于本批使用或验收范围。

原生 GTK 回归检查原菜单对象不变、普通/子菜单标签、禁用和恢复、64位嵌套 ID 的勾选/取消、程序更新不触发用户回调，以及错误类型、未知 ID 和非 checkbox 的拒绝。测试目标 `shuvpn_tray_menu_test` 不进入分发 bundle。

## 场景与证据

`tool/desktop_tray_acceptance.py` 复用离线 GUI 验收的私有显示、profile、PID/库检查及清理。它启动自己的 XFCE panel，等待 `IsStatusNotifierHostRegistered`，并将 watcher PID/exe/摘要及父进程链绑定到这个 panel。应用的 SNI service 必须属于本次应用 PID；菜单路径及条目 ID 从实际导出读取。

三个不同应用 PID 覆盖：

1. 首启欢迎页中，连接/断开和未启用协议的真实菜单状态。Alt+F4 隐藏后，右键打开面板菜单，以 Home/Return 选择“显示窗口”；再次隐藏后通过原生 DBusMenu Event 退出，要求进程退出0、注册项移除。
2. 关闭 owner 后仅由原生准备 `welcomeCompleted=true`，再启动 Guest。原生托盘“连接”进入账户页，从实际控件打开空登录表单；不提交凭据。隐藏后通过原生菜单恢复，再关闭宿主；此时关闭窗口须退出，不能隐藏成无入口进程。
3. 重启宿主和应用；保持同一应用运行，再次重启宿主，要求原进程重新注册菜单。随后隐藏，以真实右键打开菜单，以 End/Return 选择“退出”，要求退出0和注册项移除。

右键目标来自所属 watcher 的可见 XEmbed 窗口，排除 tooltip。弹出菜单须是受控 panel/watcher/应用 PID 的实际 X11 popup menu。选择前核对导出菜单中首个/最后一个启用条目，不用坐标猜测菜单行，也不把直接 DBus Event 称为鼠标点击。截图、native layout、窗口/PID 和实际输入路径分别记录。

AT-SPI 在页面切换时可能返回已被移除的空子项；遍历跳过它，业务断言仍等待完整控件。退出清理逐项执行，保留原始失败，清理本身失败也会使正常场景失败。

## 运行

需要 GTK/AT-SPI 的 Python GI、Xvfb、Openbox、xdotool、xwininfo/xprop、XFCE panel、xauth 和 dbus-run-session。当前本地宿主为 XFCE 4.16、GTK3、X11；不据此签收其他桌面或 Wayland。

```bash
cmake --build build/linux/x64/release --target shuvpn_tray_menu_test
xvfb-run -a build/linux/x64/release/shuvpn_tray_menu_test

python3 tool/desktop_tray_acceptance.py \
  --bundle "$PWD/build/linux/x64/release/bundle" \
  --report "$PWD/build/desktop/tray-new" \
  --dart /absolute/path/to/flutter/bin/cache/dart-sdk/bin/dart
```

report 必须是新目录。不要传真实 XDG profile，不能使用 Python 优化模式。DEB 隔离 harness 接收 `SHUVPN_DEB_TRAY_REPORT` 和 `SHUVPN_DART_RUNTIME`，对实际 `/opt/shuvpn` 执行同一场景。CI 已接入 bundle、原生 GTK 和安装版托盘验证，上传非 profile 截图/JSON/日志；本地完整 GUI 矩阵使用 `SHUVPN_DESKTOP_GUI_ACCEPTANCE=1 bash tool/verify_desktop.sh`。

本批汇总位于 `build/desktop/tray-acceptance/verification-final.json`。新 Linux 验证包只用于本轮验证，没有构成新的 Windows/Linux 成对发布候选，也没有提交或发布。

## 本地结果

444文件来源 SHA `2acc46e6a54034d49b0f0b8f3aa161d7b67dbac85df196364d28e09f49dbed4d`，Windows 工作区与 Linux 快照一致。Linux release、311项 Flutter、analyze、4项原生 GTK、两端各57项工具（Windows19项平台skip）和108文件 Dart 边界检查通过。新 core/helper 与第65批受测字节一致，未新跑 Rust 或 Full 全矩阵。

新 DEB 两轮完整隔离安装生命周期均退出0，包含原离线设置三进程、托盘三进程、一轮校园 TUN 内核数据/退出屏障、升级/卸载/purge/重装、账户与非包文件保留、宿主账户及dpkg保持。托盘三次启动均退出0，三个 panel/watchers 的注册和父进程链通过；第三个应用在同一 PID 内完成宿主热重启后重新注册，真实右键菜单的截图已检查。

首个独立 maps 观察器启动晚，记录5进程后明确失败，未按6进程计数。第二轮从 driver 启动即附着，独立核对全部6个 GUI 的 namespace PID、实际可执行文件及包内 core/tray 库 maps、SHA、UID1000/CapEff0，并用pidfd确认观察期间进程未退出；最后与两份 GUI 报告的6个 PID逐项对应。该记录不把 GUI 观察扩展为已连接数据面或长期验收。

产物保存在 `build/desktop/tray-acceptance/package/`：tar 20,853,760字节、SHA `655f9faeffb9573f7d587f2c5b2e5f8cc5e0d2aaa03847c109d07e68f7b7c009`；DEB 16,455,112字节、SHA `802f99a22ad68790735d6e87e6092078fa4394502f36b123217b4f1dc4ac074d`。归档、安装器输入/来源/校验旁路文件、DEB实际主程序/core/tray/helper载荷及MIT许可证均核对，profile数据不在上传材料中。

## 限制

第67批补充（2026-10-17）：原生再次激活当前 aTrust 会先取消 GTK 勾选，Dart 的同值选择不发新快照，实际复现为 `toggle-state=0`。现由桌面 UI 生命周期在协议事件后重新确认最新快照，刷新标记在原生更新前消费，保留在途再次事件；禁用输入不派发选择。两条原生回调/在途回归先失败后通过，普通流量等价快照仍去重。首启、Guest及同进程宿主热恢复三处真实 DBusMenu 激活后都要求勾选恢复。证据 `build/desktop/tray-protocol/verification.json`；这属于 UI 状态确认，不引入后端协议或清理策略。

第71批完成[本地协议已连接 GUI](desktop_connected_gui_acceptance.md)的三轮连接、按钮/托盘普通断开与重连、连接中物理菜单退出，并修复普通断开误用终止清理及 busy 完成通知缺失。使用真实默认库和生产界面，但替换账号/连接计划入口，不代表校园登录或正式 connectAccount 成功。

物理输入已验证显示/退出，协议再次激活使用真实原生 Event；协议子菜单的指针操作和 Windows 真实托盘仍需独立验证。未完成真实登录、真实校园已连接 GUI、其他桌面/Wayland、长期运行或整项 GUI 签收。账户页的原生准备标志不证明首启认证成功。既有 Flutter implicit-view/messenger/OpenGL 及私有 XFCE session manager 提示保留，不由正常退出推断其已修复。Windows 特权 Full、真实校园、远端 CI 和原计划最终审计继续独立验收。
