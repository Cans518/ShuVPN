# Linux 已连接 GUI 与托盘验收

2026-10-18 第74批补充：同一开发入口现增加显式私有原生存储的 Windows 分支，生产 widgets/ConnectionController/持久 owner 的真实按钮与 Explorer 托盘三轮已重复通过，见 [Windows 已连接界面验收](windows_connected_gui_acceptance.md)。Linux 共用入口/四流核验函数也已在私有 namespace/XFCE 中重新运行三轮通过，记录位于 windows-connected-gui/linux-run-01。以下保留第71批原始范围与证据，不把新结果扩大为正式 bootstrap/真实账户登录或特权 Full。

第71批，2026-10-18。使用实际 Flutter Linux release 窗口、生产页面/连接控制器、默认随包 Rust 库及原生持久连接 owner。开发入口 `tool/connected_desktop_app.dart` 导入本地测试账户并将连接入口换为本地 TLS fixture 的 `connectPlan`；正式 `lib/main.dart` 不导入该入口。本场景不签收生产 bootstrap、`connectAccount`、真实校园登录、TUN 或系统代理。

## 发现与修复

`DesktopHost` 原先把 `disconnectForExit` 同时提供给托盘“断开”和退出。普通断开会终止 owner，后续连接无法复用。新增 host 组件回归实际复现错误接线，再将普通断开接到 `disconnect`，退出保留 `disconnectForExit`；窗口销毁仍等待原生退出屏障。

真实 GUI 的第一次按钮断开后，原生 owner 已无 run、监听已关闭，托盘“连接”及协议仍禁用。`ConnectionController` 在 `_disconnectTask` / `_exitTask` 清除前发出最后一次通知，监听者读到 busy=true，完成时没有再次通知。两条监听回归先失败，现于命令 future 清除后再次通知，恢复 UI 能力。未把原生连接、清理、协议或恢复逻辑移入 Dart。

## 实际场景

root 准备器仅创建私有 net namespace 并启用 loopback，之后切回 UID1000。GUI、协议 fixture 和独立客户端均在该 namespace，无物理接口或默认出口；GUI 四项 UID 相同、CapEff=0。普通用户验证器另外创建私有 Xvfb、DBus、Openbox、XFCE 托盘及 XDG profile。

验证器按本次 PID 找窗口、AT-SPI 连接状态按钮及 native DBusMenu；核对实际 exe、随包 `libshuvpn_core.so` 映射及 bundle SHA，拒绝其它进程的 telemetry/token/namespace。fixture 的 PID、PPid、UID、CapEff、exe/SHA 与 namespace 被核对，保留 pidfd 至退出。

三轮在同一应用进程和同一非终止 owner 上完成：

1. 实际连接按钮连接，检查 GUI“已连接”和原生菜单能力；按钮断开，要求 owner.closed=false、无 run、无 pending cleanup、代理监听拒绝。
2. 原生托盘“连接”，完成数据后隐藏/恢复同一窗口；原生“断开”，再次检查非终止清理及监听拒绝。
3. 原生托盘再次连接并完成数据；隐藏窗口后，在实际 XFCE 图标上右键打开所属弹出菜单，键盘选择“退出”。要求 GUI exit=0、原 fixture pidfd 已退出且 exit=0、owner.closed=true/无 run/无 pending cleanup、TUN stopped、egress inactive/无 socket、监听拒绝。

每轮由独立 Python socket 客户端，通过真实 HTTP CONNECT 与 SOCKS5 监听端口分别访问 fixture 的 IPv4/IPv6 目标。四条流各收到 `prelude`，逐字节回显64 KiB，半关闭后收到 EOF；原生 decisions 同时匹配各入站、目标、443/TCP 与 SHU-VPN。最终验证器等待每轮至少上传262144、下载262172字节，避免只观察到部分流的统计。三轮共12条流。截图已人工查看，“已连接”和虚拟地址10.99.0.2可见。

`gui-01` / `gui-02` 保留 busy 通知缺陷的真实失败。`gui-03` 已通过转发和重连，但统计仅要求大于零；只读复核指出第二轮在部分统计结算时采样。收紧统计及最终 session 清理断言、增加 fixture 身份和 bundle 前后摘要检查后，`gui-04` 再次通过，作为最终本地协议场景证据。

证据在 `build/desktop/connected-gui/`：`gui-04/verification.json`、`gui-04/namespace.json`、`gui-04-namespace.json`、截图及日志。namespace 快照比较的是链路、IPv4/IPv6全部路由表及策略规则在前后两个时点相同；不宣称持续监控地址/DNS/代理/netfilter等所有宿主状态。

## 复现

依赖与离线 GUI 验证相同，另需 XFCE panel、Linux `ip`、`unshare`、`runuser`、pidfd 及本地编译的 `proxy_fixture`。命令中的报告和 fixture bundle 目录须为新目录；以普通用户构建，只以 root 准备 namespace。

```bash
cargo build --manifest-path rust/Cargo.toml --locked --example proxy_fixture
flutter build linux --release --target tool/connected_desktop_app.dart
mkdir -p build/desktop/connected-local
cp -a build/linux/x64/release/bundle build/desktop/connected-local/fixture-bundle
sudo python3 tool/connected_desktop_namespace.py \
  --user "$(id -un)" \
  --bundle "$PWD/build/desktop/connected-local/fixture-bundle" \
  --fixture "$PWD/rust/target/debug/examples/proxy_fixture" \
  --dart /absolute/path/to/flutter/bin/cache/dart-sdk/bin/dart \
  --report "$PWD/build/desktop/connected-local/session"
# 完成或失败后均恢复正式入口的 build 输出。
flutter build linux --release --target lib/main.dart
```

开发入口需要私有 GUI token、profile、普通 UID/CapEff0 和不同于父环境的网络 namespace；不能直接用于日常启动。不要把 fixture bundle 作为产品打包。CI 在正式 Linux 包验收之后单独构建该入口，并以 EXIT trap 恢复正式入口；仅接线，尚未远端运行。两端 verify 脚本及 CI 已纳入 GUI 验证器单元测试选择器。

## 验证范围

Windows/Linux 全量 Flutter317/316、两端 analyze、Python工具74（Windows19/Linux5项平台 skip）、Dart边界108通过。Windows测试首次因未设置 Wintun DLL 路径有一项失败；按正式验证脚本环境重跑后通过。Linux镜像不含 `.git`，工具测试必须显式提供源 revision/dirty；初次缺少该环境的失败保留。验证器测试保留超时不能当作端口关闭成功的规则；实际监听关闭在隔离 namespace 中验证。

两端正式入口 release 已重新构建，Linux另对正式入口执行原有首启/guest/托盘宿主恢复场景。具体完成结果以总汇 `verification.json` 为准；fixture AOT 与正式 AOT 分别记录摘要，不能互换。本批没有新增安装包或完整 Rust/Full 矩阵；既有 Windows68/Linux67安装候选保留原来源，当前源码修复尚未进入这些旧包。

第72批补充：同一462文件来源的正式产品已生成新成对安装候选，位于 `build/desktop/connection-candidate/package/`。Windows实际安装/升级/卸载与存储保留、新DEB完整隔离安装、3轮校园TUN、10阶段安装默认库Full和6个设置/托盘GUI均通过；总汇 `build/desktop/connection-candidate/verification.json`。新候选已包含本页两处UI修复，旧68/67包仍保持原样。它不扩大本页fixture入口为真实校园/正式connectAccount成功，候选仍未签发布。

仍需 Windows 实际托盘/已连接 GUI、独立可恢复 Windows guest 的提升观察者/普通 probe/真实 helper Full 验收、真实校园登录与长期运行、其它 Linux 桌面/Wayland、远端 CI 和原计划逐项最终审计。目标保持 active，未提交或发布。
