# 桌面几何与原生点击验证

第90批补充及来源校正：已取得Windows点击前后配对证据，确认离屏新增按钮仍被provider/point报告可见；Linux补socket parent及销毁断环，但远程几何仍超时。另明确旧a804b261…为engine artifact标识，源码快照另核为SDK Git d3b14c…，未独立证明artifact编译映射。新状态优先，见 [原生几何与生命周期](native_accessibility_geometry_acceptance.md)；下文89批原始范围保留。

第89批，2026-10-18，部分完成。字体要求保持已完成；本批继续处理正式窗口几何与输入验收。完整迁移目标仍为 active，Windows 三来源规则弹窗和 Linux 物理拖动均未签收。

## 已修改与验证的布局

实心贴底导航栏仍继承旧悬浮栏的 `extendBody: true`，导致正文布局伸入底栏。新增两个 widget 回归先在 DPR 1/2 下分别复现正文底边 768、底栏顶边 712，差 56 个逻辑像素；随后移除 `extendBody`，在服务、设置、连接三个分支检查正文与底栏恰好相接。

列表尾部 padding 从 84 调为 28；Settings/Services 由外层 Scaffold 另外保留 56，末尾可见余量保持。Connect 保留 SafeArea，更新旧注释。Notifications 在 root Navigator 中，不享受底栏空间，因此给独立页面补 SafeArea；新测试先复现 34 像素系统底部区域未避让，再验证内容底边为 326 而非 360。公告分页测试在追加内容后滚到新的列表末尾，再检查页尾，没有删除分页或页尾断言。

四处生产 Dart 均为 view：`dock_shell.dart`、`theme.dart`、`connect_page.dart`、`notifications_page.dart`。边界清单109项的相应摘要已更新，没有 Rust、FRB、业务、依赖或字体变动。

两端最终完整 Flutter 测试 Windows344/Linux343、工具测试各178（平台跳过19/7）、analyze、边界109以及显式 `lib/main.dart` release 构建通过。来源493文件、SHA256 `615fa36abaa0d64eec01a8b0b3f9c066d59a1940974fd12caf05439834e3cbcd`，Windows/Linux逐字节一致。

首轮完整测试保留原失败：公告追加后未滚到新的末尾；Windows TUN 探测的运行环境未显式指定验收入口中的 FRB/Wintun 路径。最终按入口设置路径后原断言通过，未修改 Rust 或放宽检查。新增 inset 测试的一次 fixture 参数编译错误、修正后的真实 red、原 dock red 均保留，不能把编译错误当产品缺陷复现。

## Windows 实窗

本批补强开发驱动：点击前重新检查最新控件、原窗口/PID/前台和真实 point hit；规则控件用预期角色与名称，有限滚动后再检查 point；等待真实编辑字段及第二次删除确认。报告写入失败撤销成功标记，主错误及清理错误保留。系统安全弹窗守卫覆盖激活路径；外部前台仅记 HWND/PID/class，不读取外部文档标题。

所有输入只发生在本轮正式 main 私有 EXE 资源副本：只改两个等长 CompanyName/ProductName 资源名称，其余载荷与包逐项相同，新随机 KnownFolder 目录与所有权记录限定清理范围。不是独立 Windows 用户或网络沙箱，不替代原始 EXE 或已安装 GUI。

`run-01` 用88批产品完成三进程基线，第四进程在新增字段处失败；`run-02` 丢失前台；`run-03` 新 point 守卫拒绝设置底栏。`run-04/05` 仅作只读 point 诊断，各两进程正常退出；设置按钮可枚举，但实际 point 查询落到匿名 pane，20次重读仍无命中。这些失败未覆盖。

布局修正后的新包 `run-06` 完成三进程基线，各退出0：欢迎/权限/账户说明/仅假密码遮罩与清空、离线深色/70000拒绝与18443保存、Full保存重开/Campus恢复、仅校园模板与非法规则拒绝/原文重开。底栏“设置”已通过 point 守卫。第四进程“新增前置规则”枚举及 point 都给出同名控件，但点击后等不到表达式字段；`passed=false`、CLI退出1。失败截图仍为滚动后的规则列表。

该按钮 provider rect 为 `[2299,369,2520,433]`，point `[2409,401]`，实际指针读回相同。此前为高级编辑区进行14次滚轮请求，新增按钮自身 reveal 没有再滚动。失败时 accessibility 仍给出同一 rect/offscreen=false，而截图看不到前置标题或新增按钮，支持 provider 与渲染/滚动状态不一致的调查方向；没有同步点击前截图时，不能据此断言点击瞬间已过期或精确定位原因。RuntimeID为空，不能宣称稳定节点身份通过。

`run-07` 尝试同候选单次点击的同步截图/provider诊断，但首进程任务栏激活时物理指针请求 `[1625,1872]`、实际 `[2546,728]` 不符，守卫停止；外部前台仅记录 HWND6490016/PID97612/Chrome_WidgetWin_1、不是安全弹窗。没有得到点击前后配对证据，`passed=false`、CLI1，清理记录原 probe 被强制终止。`launches=[]` 不代表未启动 app。后续应先复核指针/前台状态，再用这项有界诊断获取点击前后窗口原点、截图、实时枚举及 point 结果；保留所有归属与命中守卫，不按截图猜坐标。

## Linux 实窗与桥接

新构建 `linux-layout-01` 完成原离线设置与重开基线，三个 UID1000/CapEff0 进程退出0，版本2、前后置原文、HTTP18443、深色、Full重开及恢复Campus、四网络开关关闭均由现有检查确认。运行前后宿主 link/route/rule 相同，实际 runner/AOT/core 摘要与新 tar 相同；截图已抽看。它不执行物理规则拖动，也不是新 DEB 安装验收。

另用88批产品进行普通用户私有 Xvfb/DBus 诊断，`linux-bus-01/bus-geometry.json` 记录 Component.GetExtents 的 `sender=:1.0`、`destination=:1.0`、path `/org/a11y/atspi/accessible/2`，私有总线 owner `:1.0` 的 PID 为同一 app `480816`。这补上88批线程栈之外的实际消息目的地证据。几何仍 `dbind` 超时、`acceptance_passed=false`，没有发出拖动；只终止自有 Popen，退出为 -15，不能写成正常退出。

固定 engine `a804b261645ef8c13eb3d5c44a5c2fb0340c5539` 的 Windows hit test 同步遍历当前 AXTree，没有“等待 point 缓存预热”的契约；ClickablePoint 本身也不证明未遮挡。Linux Flutter 的 accessible plug/socket 不是公开 runner 替换入口，本批没有使用全局 ATK vtable 或私有字段补丁。原88批 Linux 物理拖动门禁仍失败，当前 GTK 环境尚未修复。

## 包与剩余范围

新四包仅位于 `build/desktop/windows-rule-resume/package/`；88及更早包保持。ZIP39项、tar32项、DEB声明33项逐项检查来源、载荷、字体/OFL/FontManifest 和原 core/helper 字节；Inno 仅核外层及输入归档/固定编译器链，未独立解码。新 Inno/DEB 均未安装。

原字体 TTF SHA256 `cd1a6fa39c4ea42fd8f4e289945789b0e510cf7016435640f8893cdad9b220f3`，OFL SHA256 `4d72e3080cdb3be63638bb7197c7caae55b6e4cbe1bab1ded2830bdff0c9b438`，FontManifest SHA256 `70ddf17d6bf14dcc9885bf4f8da3608c6f32eacbfad5e18cc31a0fdbfca8ef70`，均与77批相同；不是仅依赖归档自身 manifest 的一致性检查。

总汇与原日志为 `build/desktop/windows-rule-resume/verification.json`。真实校园、Windows原始/安装及已连接详情GUI、可恢复guest Full、长期/其它桌面Wayland、84批心跳/WSL绑定故障、远端CI及最终审计继续保留。没有提交或发布。
