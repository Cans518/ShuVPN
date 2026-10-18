# Windows 正式主入口离线 GUI 验收

当前更新（2026-10-18）：用户已恢复推进，第76批端口、捕获模式及非空规则覆写的两轮GUI验收已通过，见 [76批恢复验收](windows_settings_gui_acceptance.md)。下文75批及76批最初暂停均保留为历史，当前goal active。

当前暂停说明（2026-10-18）：后续76批已扩展端口/捕获模式/规则驱动，但首次尝试在任务栏激活阶段失败，尚未进入新增操作；目标已由用户暂停。见 [76批暂停记录](windows_settings_gui_paused.md)。下文只描述75批历史通过结果，不能用于签收当前已改动驱动或76批宿主状态。

第75批，2026-10-18。使用第72批 Windows 安装候选的真实生产 `lib/main.dart` AOT、原生库、插件及资源，完成首次引导表单可达、离线主题保存、正常退出与重启恢复。最终两次运行是 `build/desktop/windows-formal-gui/run-10/11/verification.json`，各三个普通权限进程；总汇为该目录 `verification.json`。

这次没有替换 bootstrap、账户、设置、规则、连接后端或 root widget，也没有启动 fixture。为使正式存储使用新目录，复制 EXE 的两个版本资源字符串经过修改。因此准确对象是“正式包的私有资源副本”，不是原始 EXE 字节完全未改的安装版运行。

## 产品与工具来源

原始 ZIP SHA256 为 `0dc3fc111b5afeac5bac83412e849b72c37155c03aa9293f26985d91fe706dfd`，来源462文件 SHA256 `7abf62372522eddd35b7279eddb82ea8b44604181422d616497eadd7d8b78f33`。准备器先验证原包全部清单文件，再要求其 `data/app.so` 与当前已恢复的 `lib/main.dart` 参考构建逐字节相同，SHA256 为 `351535b462f105a5a9274187ba39692dca213626491deac95c7d35dcc8b23981`。

全部37个清单文件复制后重新核对；除 EXE 外，36个文件包括 AOT、core/helper、所有 DLL 与 assets 均保持原始摘要。EXE 仅将 CP1252 英文版本表的 `CompanyName`、`ProductName` 从 `ShuVPN` 改为两个六位小写 hex 名称。两个 UTF-16LE 值的区间分别为 `[86680,86694)`、`[87060,87074)`，实际改变12字节；文件大小、区间外所有字节保持。准备器拒绝有证书目录的 EXE、可执行或越界资源节，并通过 Win32 版本 API 读回修改结果。没有重建或修改生产源码、Flutter 引擎、AOT 或插件。

本轮验收工具来源为471文件 SHA256 `264c8d9ebfa008f9a3e5b5ab1c8cb0ca17ede6b05eff8a4134a7032f2dd1b2bf`，Windows/Linux 两树一致。准备器与最终驱动各自记录该来源并拒绝执行期间来源变化。工具来源与旧包的产品来源分开保存，不能改写成“新工具与包完全同一源码树”。marker 是受信本机构建的用途分类与摘要屏障，不是签名或恶意二进制认证。

## 存储路径与宿主范围

已安装的 path_provider_windows 2.3.0 通过 `GetModuleFileName(0)` 读取主 EXE 版本信息，support 取 RoamingAppData KnownFolder 后追加 company/product。shared_preferences_windows 2.4.1 的独立 PathProviderWindows 对象也走该路径；只改 APPDATA 或 PathProviderPlatform singleton 并不足以分开它。

准备器在实际 Roaming/Local KnownFolder 下排他预留全新的六位 company 目录，保存本次 ownership marker。正式应用在其 product 子目录创建实际 native settings/auth/rules；原有 `ShuVPN/ShuVPN` 目录不作为本轮存储。版本字段修改不提供独立 Windows 用户、ACL、网络或系统代理隔离，当前用户 DPAPI、临时目录及系统服务仍按其真实实现工作。

本轮只读比较了原有 Roaming/Local `ShuVPN/ShuVPN` 两目录的存在状态/文件摘要，以及 HKCU Internet Settings 与 Connections 两键的值摘要，前后相同。不能扩大为整个 OS、所有用户文件或所有网络状态持续不变。所有测试进程保持普通权限，没有启用代理监听、TUN 或 UAC。

已进入运行阶段的清理在 finally 中执行：核对 KnownFolder 根、完整 ownership、解析路径和 reparse 状态，删除内容期间保留 owner，使部分失败后可重试；创建第二个预留目录失败会以非递归操作回滚本次已创建的目录。相应失败回归已实际运行。构造/参数预检在运行 finally 之前，失败时可能保留尚未使用的预留目录，不能宣称所有参数错误都自动清理。本轮另逐项检查所有实际准备过的根目录均无遗留；清理错误不能升格通过。

## 实际流程

每次完整验收使用同一新 profile 的三个不同进程：

1. 新 profile 正式 main 启动，原生 settings 显示 welcome 尚未完成。真实鼠标操作两个“继续”和“去登录”，依次检查权限页、当前账户保存说明和实际登录表单。
2. 表单内只有本轮局部测试文本：向密码框输入 `gui-fixture-only`，不输入用户名、不提交登录。读取该所属进程字段的 LegacyIAccessible.CurrentValue，要求16字符全部为遮罩；随即 Ctrl+A/Backspace，并确认其值为空。截图及字段观测保存，实际校园认证不计成功。
3. Ctrl+Q 正常退出第一个进程，等原留存句柄与 Popen 均为0、托盘移除；之后才用独立原生探针仅准备 `welcomeCompleted=true`。这是离线测试准备，不能称 GUI 完成登录或完整认证引导。
4. 第二进程从正式 bootstrap 进入离线首页；实际点击“设置→外观→深色”，等待 Rust 文件提交 `settings.themeMode=dark`，观察深色表面并截图。正常退出后，独立 native settings probe 读回 dark。
5. 第三进程重新打开同 profile，经实际控件进入外观页，再次观察暗色表面并截图；正常退出后，再用独立 native session 读回 dark、welcome=true，HTTP/SOCKS/VPN 开关仍为 false。

主题的运行时链为 GUI 修改→原生提交→关闭后独立 API 读回→新进程重新渲染→再次独立读回。自动颜色断言是窗口相对位置单点 `max(RGB)<100`，不是全页逐像素验证；截图已查看，可见深色与蓝色单选图示。UIA 没有返回主题 selected=true，本批不据此签收主题选中语义。

当前 Flutter Windows 的 UIA IsPassword 与 MSAA protected 标志均未返回 true，早期相关断言真实失败。最终通过依据是只针对自有局部测试文本读取到完整遮罩值、清空确认和截图；不把缺失标志改写为已修复，也不扩大为通用密码安全审计。准备、字段聚焦、输入和读取均限定新 profile 的本次进程，没有读取真实用户密码。

## 真实输入与进程核验

GUI 使用实际主窗口和 MSAA 启用后的 UIA 语义。控件按所属 PID、唯一名称、有效可见矩形定位，移动鼠标后重复读取，等待页面动画稳定，再核对 WindowFromPoint 的进程并以 SendInput 点击。按钮没有可靠 RuntimeID/ClickablePoint 时使用当前矩形中心，不以空 RuntimeID 证明元素身份。

正式图标没有开发 nonce，早期 Explorer overflow/Shell 矩形瞬态定位失败保留。最终初始激活使用 Explorer 任务栏按钮，要求 AutomationId 的 `Appid: <完整EXE路径>` 精确匹配本轮已核验的 EXE，标题仅归本次进程；布局与移动后观察稳定，再实际点击并确认前台为精确 main HWND。没有发送应用命令消息来替代这些操作，本场景也不重复签收第73/74批的托盘菜单交互。

每个 GUI 保留原进程 handle，核对其创建时间与原 CreateProcess handle 一致、EXE 路径/SHA、非提升状态、同 Explorer 会话，以及默认加载的配套 shuvpn_core.dll 路径。启动时系统双地址族监听表中本 PID 无监听行。正常退出检查原留存 handle 和 Popen 两者退出0，不能把超时终止当通过。

## 复现和验证

需要普通权限 Windows 交互桌面、Explorer、Python comtypes/Pillow，以及固定 Dart SDK。输入 ZIP 必须与当前正式 main 参考 AOT 一致；不一致会拒绝准备。输出和报告均须为 build/desktop 下的新目录。

```powershell
python tool/windows_private_profile.py --archive build/desktop/connection-candidate/package/ShuVPN-0.4.2+9-windows-x64.zip --output build/desktop/formal-main-private
python tool/windows_formal_gui_acceptance.py --bundle build/desktop/formal-main-private/bundle --dart C:/Users/xiema/.cache/shuvpn/flutter/bin/cache/dart-sdk/bin/dart.exe --report build/desktop/formal-main-run
```

运行期间不要并发操作桌面，驱动与现有 Windows 探针共享 presence mutex，并恢复鼠标位置。保留的 profile 仅为本次新账户/设置数据，不上传真实 profile；私有资源副本不是新的安装候选。

两端工具各102项通过（Windows19/Linux7平台跳过），Dart边界108通过。守卫覆盖资源区间/代码变化、签名拒绝、错误名称、AOT/DLL/入口混淆、局部密码遮罩、部分 claim 回滚及部分 cleanup 重试。已有 verify/CI 的 `test_*desktop*acceptance.py` 选择器会包含新测试，真实桌面只在本地执行。共用 Windows API 更新后，另以第74批开发 AOT 回归三轮12流的已连接界面；它保持独立的旧 AOT 来源。

本批未重建正式产品、未新建安装归档、未重跑完整 Rust/Flutter/安装矩阵，未提交发布。原有正式 build 与第72批四种候选保持；完整套件仍引用第71批，安装证据引用第72批。未签收真实登录、原始未修改 EXE 的默认 profile 实际运行、Windows 完整离线设置矩阵、TUN/系统代理、独立可恢复 guest 的特权 Full、校园/长期、其它 Linux 桌面/Wayland、远端 CI 或最终逐项审计。
