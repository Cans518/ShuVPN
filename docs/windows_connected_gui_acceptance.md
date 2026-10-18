# Windows 已连接应用界面验收

第75批补充：另已对正式 main AOT 的私有版本资源副本完成首启表单、离线主题保存与重启恢复，见 [正式主入口离线验收](windows_formal_gui_acceptance.md)。该证据与本页 fixture 入口分开；不将其扩大为正式账户连接、原始 EXE 默认 profile 或 Windows Full。本页开发入口另使用第75批共用驱动完成三轮回归。

第74批，2026-10-18。已使用 Windows 实际 Flutter release 窗口、生产 ShuVpnApp/ConnectionController、默认随包 Rust 库及持久 DesktopConnectionOwner，完成按钮与 Explorer 原生托盘的三轮本地协议验收。

最终汇总为 `build/desktop/windows-connected-gui/verification.json`，最终重复运行是 `run-11/12/verification.json`；`run-08/09/10` 是此前成功记录。最终 builder 与两轮产品、来源一致：468文件 SHA256 `eeed2e27796d99139f3a0283b7202d169d6c895feac3264396d4f13c52f5c9a0`。归档复核时将继承自窗口探针的合并范围字段拆成明确的 controller=true、production main=false、私有原生存储=true，再重建复验。没有修改生产 Dart、Rust、绑定或 Windows runner/插件源码，没有新建或覆盖安装候选。

## 私有存储与入口范围

复用开发入口 `tool/connected_desktop_app.dart`，新增 Windows 分支；正式 `lib/main.dart` 不导入它。Windows 在本轮报告目录下新建 `profile`，显式打开真实 Rust settings/auth/rules 存储，经现有 `fromNative` 接口交给生产界面。没有调用 Windows KnownFolder 支持目录或 SharedPreferences 的旧数据迁移入口，也没有依赖 APPDATA 环境变量隔离。

私有账户是导入的本地 fixture 账户，账户文件仍使用当前 Windows 用户 DPAPI；这不是独立 Windows 用户或 OS 沙箱。连接后端只把认证入口换成本地 TLS fixture 的 connectPlan，仍使用真实原生持久 owner、代理服务、生产控制器与 widgets。不签收正式 bootstrap、connectAccount、真实校园登录、TUN、系统代理、特权 Full 或 UAC。

开发入口拒绝缺失/错误 token、报告路径越界或重复 profile、非绝对 fixture 路径；fixture 启动后立即输出身份屏障，驱动保留其进程句柄。就绪前异常会关闭 fixture stdin，并等待或终止自己启动的子进程。abort 与三分钟 watchdog 通过开发 port 请求退出并设置失败退出码；失败清理不升格验收结果。命令记录写入失败时，finally 仍尝试销毁托盘。

Windows 分支没有 Linux net namespace 等价隔离。可签的宿主保护证据仅为本轮前后两个时点：Roaming/Local KnownFolder 下 ShuVPN/ShuVPN 两目录的存在状态/文件摘要，以及 HKCU Internet Settings 与 Connections 两键的值摘要相同。没有持续监控所有用户文件、系统配置、DPAPI 内部文件或网络状态。

## 实际操作和屏障

GUI、fixture 和 Explorer 均记录 PID、映像路径/SHA、创建时间、会话与非提升状态。GUI 保留句柄的创建时间与原 CreateProcess 句柄一致；fixture 必须为本 GUI 的子进程，Explorer 必须为实际交互会话的系统 Explorer。实际加载的 shuvpn_core.dll 必须来自匹配 bundle。此处没有新增用户 SID 比较，不能把同会话/普通权限称为独立用户隔离。

每次应用启动均依次完成：

1. 点击本轮 nonce 对应的 Explorer 图标，使应用在前台显示；观察初始未连接按钮及原生菜单能力。
2. 真实鼠标点击生产连接按钮，等待 round=1、原生 owner/session connected，核对已连接按钮和菜单；四流量完成后，再点击按钮断开。
3. 通过真实原生托盘菜单“连接”开始 round=2；Alt+F4 隐藏后传送四流量，左击 Explorer 图标恢复同一窗口，再实际选择托盘“断开”。
4. 再选择托盘“连接”开始 round=3，完成四流量；隐藏后右击本轮图标，实际 Up 高亮原生“退出”，Enter 选择退出。

每轮独立 Python 客户端通过 HTTP CONNECT / SOCKS5，分别访问 `10.21.0.5` / `2001:db8:21::5` 的443端口；四条流各有7字节 prelude、64 KiB逐字节回显和 half-close EOF。每轮原生累计上传262144、下载262172，四条决策均匹配 inbound/host/TCP/443 与 SHU-VPN。两次最终运行各12条流，不把此目标双栈证明扩大为物理网络的 IPv6 接入验收。

两次普通断开读取稳定的 disconnected 屏障文件：owner.closed=false、无 run、无 cleanup pending，session disconnected、TUN stopped、egress inactive/无 pending cleanup/无 sockets。随后两种代理监听端口都从系统 IPv4/IPv6 OWNER_PID_LISTENER 表中消失，UI 回到未连接，原生菜单连接和 aTrust 恢复启用。三个 run ID 不同，均来自同一非终止 owner。

最终退出要求 owner.closed=true、无 run/待清理资源、完整最后一轮统计，原 GUI 与 fixture 的留存句柄均确认退出码0，图标移除且 bundle 产品摘要未变。真实原生命令严格为 show、connect、show、disconnect、connect、quit。端口负向连接在本机是 timeout，清理结论还要求独立 OS 监听表为空；没有将 timeout 写成 connection refused。

## 无障碍与物理输入

当前安装的 Flutter 引擎 `flutter_window.cc` 保留 MSAA 的 OBJID_CLIENT 查询路径。单独枚举 Windows UIA 时最初仅有 FLUTTERVIEW，没有生产按钮；驱动通过 AccessibleObjectFromWindow 请求本进程唯一 Flutter 子窗口的 MSAA provider，随后 UIA 才能读出生产按钮语义。没有修改 Flutter 引擎、添加测试按钮或发送连接回调消息。

此版本按钮没有有效的 UIA ClickablePoint，RuntimeID 也为空。驱动限定所属 PID、唯一名称首行、ButtonControlType、可见启用状态和有效矩形，取实时矩形中心；移动后重新核对名称/PID/矩形/坐标，用 WindowFromPoint 再核对窗口进程，再通过 SendInput 实际鼠标点击。前后真实连接/断开状态屏障和按钮截图支持这个交互结论，不能声称 RuntimeID 证明同一无障碍元素身份，或 MSAA hit-test 能准确返回按钮。探索中的该项额外 hit-test 没有命中预期按钮，失败记录保留，最终没有使用它作通过依据。

Explorer 图标仍采用第73批的精确 nonce tooltip、Shell HWND/uID、UIA当前 ClickablePoint和移动后命中核验；原生 HMENU 来自所属进程及其窗口的菜单 owner。托盘菜单保持生产能力和行为，仅开发 port 在状态末尾附加 nonce、记录并原样转发命令。

截图已检查：已连接及虚拟地址10.99.0.2可见。累计流量来自原生 session 和独立客户端；截图中的瞬时0 B/s不能签成实时速率显示已验收。首轮窗口未显示/前台激活失败、UIA语义未启用、ClickablePoint缺失及探索 hit-test 失败均保留为失败，没有升格。

## 复现与回归

需要交互 Windows Explorer、普通权限 Python、comtypes/Pillow、固定 Flutter 和本地编译的 proxy_fixture.exe。运行期间不要并发操作桌面；驱动用与窗口探针相同的 presence mutex 排斥重叠运行，并恢复鼠标位置。输出目录须为 build/desktop 下的新目录。

```powershell
python tool/build_windows_desktop_probe.py --flutter C:/Users/xiema/.cache/shuvpn/flutter/bin/flutter.bat --entrypoint tool/connected_desktop_app.dart --output build/desktop/connected-windows-local
python tool/windows_connected_acceptance.py --bundle build/desktop/connected-windows-local/bundle --fixture rust/target/release/examples/proxy_fixture.exe --report build/desktop/connected-windows-local-run
```

builder 的入口 choices 仅允许两个开发入口，校验生成目标并记录产品；无论成功失败均尝试重建正式 lib/main.dart，恢复失败仍为失败。bundle marker 是受信本机构建的用途分类与摘要校验，不是签名或恶意二进制认证。

两端工具各93项通过（Windows跳过19、Linux跳过7）；两端 analyze 与 Dart边界108通过。新工具失败判定覆盖入口混淆、外来身份/目录、非终止断开资源、终态泄漏、部分统计/旧round/错误决策。现有 verify/CI 的 `test_*desktop*acceptance.py` 选择器已自动包含这些测试，实际 Windows 桌面交互仍为本地执行，未远端运行。

Linux 共用开发入口和提取的流量核验函数已重新构建，并在普通 UID1000/CapEff0、私有 loopback net namespace、X11/DBus/XDG、真实 XFCE 宿主下通过同样三轮12流；证据 `linux-run-01` 及外层 namespace 报告，链路/路由/规则两时点相同。上述 Windows 报告字段调整后，Linux 又按最终来源构建，逐项核对关键产品与本轮已实测字节相同；没有把第一次GUI执行时间改写为重建后重新运行。原有 Windows 独立窗口探针以第73批 bundle 和当前驱动回归通过，不能把它的旧 AOT来源改写成本轮。

两端正式入口构建已恢复。本轮不重跑完整 Rust/Flutter、安装器或 Windows特权Full矩阵；完整套件引用第71批，安装候选引用第72批。无提交、无发布，整体迁移目标保持 active。仍需正式启动/真实账户连接、Windows独立可恢复guest特权Full、真实校园与长期运行、其他Linux桌面/Wayland、远端CI和最终逐项审计。
