# Windows Explorer 托盘交互验收

第74批补充：已进一步完成生产 ShuVpnApp/ConnectionController/持久 owner 的 Windows 私有存储本地协议三轮 GUI，见 [Windows 已连接界面验收](windows_connected_gui_acceptance.md)。本页窗口探针仍保持原来的较窄范围；其第73批 bundle 已使用第74批共用驱动回归通过。

第73批，2026-10-18。在 Windows 11 10.0.26300、普通权限交互会话中，实际操作 Explorer 托盘图标、原生菜单和 Flutter 窗口。最终证据在 `build/desktop/windows-tray-input/verification.json`；完整逐步记录为其中 `run-21` 至 `run-24/verification.json`。

## 验收范围

开发入口 `tool/desktop_window_smoke.dart` 使用生产 DesktopFrame、DesktopLifecycle、PluginDesktopPort、默认随包 Rust 库及本地 TLS fixture 的真实 VpnSession。它不打开账户或设置存储，不经过正式 main、登录、ConnectionController 或持久 DesktopConnectionOwner；不启用 TUN、系统代理或 UAC。不要把这些结果签成完整应用、真实校园或 Windows Full。

每轮实际完成：

1. 通过 HTTP CONNECT 传送 64 KiB，检查 prelude、回显和半关闭。
2. SendInput 实际 Alt+F4 隐藏窗口，隐藏后再传送 64 KiB。
3. 点击 Explorer 中本轮 nonce 对应的图标，恢复同一 HWND。
4. 再次隐藏并右击图标，读取本进程原生 HMENU，核对连接禁用、断开可用以及 aTrust 禁用且勾选；悬停打开协议子菜单，再点击“显示窗口”。
5. 再次隐藏并右击，实际 Up 高亮“退出”（原生状态含 128），Enter 选择；Dart 收到的原生命令严格为 show、show、quit。
6. 原 GUI 和 fixture 句柄均见证退出 0，最终统计为上传 131072、下载 131086，session disconnected，图标删除，系统两种地址族的目标端口监听行均为空。

没有发送 WM_COMMAND 或托盘回调消息来代替用户输入。Shell 图标标识为主 HWND/uID 0；UIA 限定真实 Explorer PID、唯一 nonce 名称，移动鼠标后再核对命中项。点击采用 UIA 当前 ClickablePoint，不能使用可能陈旧的 Shell 矩形中心。Explorer、GUI、fixture 均记录映像、SHA、会话、权限及进程创建时间；GUI/fixture 使用保留的 OS 句柄等待，防止 PID 重用混淆。

退出后负向连接在本机表现为 timeout，不能单独据此宣称端口关闭。验收同时要求 GetExtendedTcpTable 的 IPv4/IPv6 OWNER_PID_LISTENER 查询成功、启动时精确归属 GUI 的监听行存在、退出后目标监听行为空，原进程退出以及原生清理屏障全部成立。真实双栈 socket 工具测试验证表结构与监听行消失；GUI 流量本身只覆盖上述 IPv4 HTTP 路径。

## 来源与结果

本轮选定来源为 466 文件，SHA256 `d33a2b4dc29fd2b835b6bf1478d389e0ebeabaacd76966bbd127f67948027f1a`。`verified-probe/verification.json` 记录开发入口构建、独立 bundle 分类及随后正式 `lib/main.dart` 构建恢复。`run-21/22` 使用此开发 bundle。

为直接覆盖第72批已交付原生字节，`candidate-native-probe` 从该批 ZIP（SHA256 `0dc3fc111b5afeac5bac83412e849b72c37155c03aa9293f26985d91fe706dfd`）复制全部37个清单文件，只覆盖 `data/app.so` 为本轮开发入口。`run-23/24` 均通过；EXE、Rust 库、托盘 DLL 等仍为第72批原始字节，开发 AOT 来源则为本轮。该混合物明确标记 developer_overlay，不是新的正式安装候选，也不是正式入口启动证据。

本轮重新链接后的 EXE/托盘 DLL 摘要与第72批不同，不能声称可重复构建字节一致。临时 runner/托盘前台焦点实验已全部撤销，生产原生源码恢复；保留改动只有开发入口、三个 Python 开发工具及工具测试。正式 AOT 恢复为 SHA256 `351535b462f105a5a9274187ba39692dca213626491deac95c7d35dcc8b23981`。第72批四种安装候选未覆盖。

两端工具各87项通过（Windows跳过19、Linux跳过7），包含真实子进程退出0/7以及 Windows 双栈监听表测试；两端 analyze、Dart边界108、桌面宿主/生命周期15、Win32原生菜单4通过。开发入口原有自动模式也退出0并打印窗口及清理通过标记。本批不冒充重跑完整 Rust/Flutter/安装矩阵；完整套件仍引用第71批，安装证据引用第72批。

## 复现

需要 Windows 交互桌面及正在运行的 Explorer、Flutter、Python 的 comtypes/Pillow，以及已构建的 `rust/target/release/examples/proxy_fixture.exe`。不要并行操作桌面；驱动用命名 mutex 拒绝重叠运行，结束恢复鼠标位置。产物和报告必须放在 `build/desktop` 的新目录。

```powershell
python tool/build_windows_desktop_probe.py --flutter C:/Users/xiema/.cache/shuvpn/flutter/bin/flutter.bat --output build/desktop/window-probe-local
python tool/windows_tray_acceptance.py --bundle build/desktop/window-probe-local/bundle --fixture rust/target/release/examples/proxy_fixture.exe --report build/desktop/window-probe-local-run
```

builder 在 finally 中恢复正式入口，恢复失败仍判失败。bundle marker 是受信本机构建的用途分类和摘要屏障，不是签名或抵御恶意二进制的认证。驱动启动前检查 marker；外部开发模式要求 nonce、独立报告目录和 fixture 三项齐全，带超时退出与失败清理。账户隔离不能只依赖修改 APPDATA：正式 path_provider 使用 KnownFolder API，本入口绕开这些存储。

早期失败证据保留，包括图标定位、End 导航、端口超时判定及焦点实验；`run-19` 即使进程退出成功仍因旧判定失败，没有升格。最终验收的是 Up/Enter 路径，未声称 End 行为已修复。临时注入键观察仅针对本轮标记的自有输入且已移除 hook。截图只截取目标窗口或菜单，菜单透明效果使背景可见，状态以实际 HMENU 及事件记录为准。

下一步仍是 Windows 完整生产界面/控制器的私有存储已连接 GUI、独立可恢复 guest 的特权 Full、真实校园及长期运行、其它 Linux 桌面/Wayland、远端 CI 和最终逐项审计。
