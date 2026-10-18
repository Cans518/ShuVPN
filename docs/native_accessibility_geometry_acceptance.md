# 原生无障碍几何与生命周期验证

第91批补充：私有同连接库实验20项及跨进程/上游对照通过，正式拖动越过超时后仍因几何错位失败，未将库加入默认包。另独立内容哈希重算补上SDK输入与artifact元数据一致的证据，仍不等同二进制认证。见 [运行库隔离实验](accessibility_runtime_acceptance.md)，新状态优先，下文90批原始范围保持。

第90批，2026-10-18，部分完成。生产改动只在 Linux runner：补齐 Flutter accessible socket 的父关系，并在 view 销毁时解除新增反向强引用。Windows 三来源规则弹窗和 Linux 正式窗口物理拖动仍未签收；字体要求保持已完成。

## Linux runner 修复

普通用户、私有 Xvfb/DBus 中，以实际 Flutter Linux 库创建一个不 realize 的 `FlView`。项目 AOT/assets/ICU 路径全部指向不存在的测试路径，不启动 Dart main、不读取真实账户。公开 API 取得 `FlSocketAccessible → AtkSocket` 后确认 parent 缺失，直接 socket 几何为 `[-1,-1,-1,-1]`；设置 parent 后变为 `[-2147483648,-2147483648,640,480]`，与 wrapper 完全一致。前两个值是未映射 widget 的坐标，不能当有效屏幕位置。

GTK 3.24.30 原版 socket wrapper 确实设置这条关系，当前 SDK 源码快照中的 Flutter wrapper 没有。新增 `linux/runner/view_accessibility.h` 仅对无 parent 的实际 `AtkSocket` 补关系，保留已经存在的 parent；`my_application.cc` 在 `fl_view_new` 后、realize 前调用。没有使用 private 字段、全局 vtable 或修改系统 ATK 库。

独立复核发现 ATK parent 是强引用，初版只有几何修复会造成 wrapper/socket 引用环。已补真实弱通知销毁测试：无 parent 的负对照退出0，parent-only 版本退出134，`accessible` 没有释放。最终以 `g_signal_connect_object` 在 GTK destroy 的普通处理阶段清除仍指向同一 view 的 parent，先于 GTK 的 RUN_CLEANUP 默认处理释放 accessible；弱连接保护回调对象生命周期，不永久持有 socket。

最终四项原生测试通过：socket 与 wrapper 几何一致及重复准备、保留既有 parent、普通无 child widget 保持、销毁后 wrapper/socket 均释放。另用同一测试源码、no-op shim 的负对照复现缺失 parent 断言，退出134；core dump 已禁用。初版三项绿色与后续泄漏 red 均保留，不能用初版绿色签收生命周期。

`shuvpn_view_accessibility_test` 是独立 CMake target，接入 `tool/verify_desktop.sh` 和 Linux CI。本地实际执行，远端 CI 未运行。测试未 realize view，销毁未启动的 engine 时 SDK 打印 `FlutterEngineRemoveView` invalid-handle 日志，四项仍正常完成；这不是生产 main 退出证据。

## 正式构建与剩余 GTK 阻塞

最终来源495文件、SHA256 `0ecc5605c0f6950ef23a14b532dab56c429612e1a711d85732dc2ce12557ee22`，Windows/Linux逐字节一致。两端工具178（平台跳过19/7）、边界109、Linux shell语法、两端显式 `lib/main.dart` release 及新 native 四项通过。没有 Dart、Rust、FRB、依赖或字体变更，未重跑全部 Flutter/Rust；89批完整 Flutter344/343是此前各自来源的记录，不能改记成90批重跑。

新 Linux main 的 `linux-offline-01` 三普通进程退出0，离线主题、端口拒绝/保存、Full重开/Campus恢复、规则版本2及前后置原文保持，四网络开关关闭，前后宿主 link/route/rule 相同。

同一新 main 的 `linux-bus-01` 仍 `Component.GetExtents/dbind` 超时。私有总线 `sender=destination=:1.0`，owner 为同一 app PID492231，path `/org/a11y/atspi/accessible/2`；没有发出拖动，定向终止自有 Popen，退出-15。父关系修复解决缺失几何回退，不解除 plug→socket 同进程同步 D-Bus 调用。`diagnosis_passed=true` 仅指诊断取得证据，`acceptance_passed=false`。

所查 GNOME bridge 固定快照 `675ad4f16a7a4e0c22873ec96118fce084fc6326` 仍保留同步 socket GetExtents，没有得到可直接升级解决问题的 runtime 候选。该有界检查不证明所有版本或其它全局调度代码都没有修复。下一步应针对同连接同步调用构造最小复现及隔离修复实验，验证后再决定依赖或 embedder 方案；不把单独升级 SONAME、猜坐标或跳过几何检查作为验收。

## Windows 配对证据

三次使用89批正式 AOT/资源名称私有副本，验证器来源仍为493/`615fa36a…`，不是90批新增 Linux 代码的窗口验证。

`run-01` 在任务栏激活阶段再次遇到请求 `[1625,1872]` 而指针保持 `[2546,728]`，守卫停止并定向强杀，外部 Chrome 前台不是安全弹窗。只读检查之后发现前台已变为同 session1、未提升的终端；这证明环境状态改变，不证明旧失败由 UIPI 引起。

`run-02` 三进程基线正常退出后，第四进程缺少第二个高级编辑字段，尚未取得新增按钮配对证据，原失败保留。驱动随后将第四进程中设置、规则、高级编辑、新增四次点击都记录点击前后截图/provider窗口原点。

`run-03` 四进程全部正常退出0，`diagnosis_passed=true`、`passed=false`。点击“新增前置规则”前后的配对截图均是滚动后的列表，没有前置标题或新增按钮；provider 同时给出 rect `[2299,369,2520,433]`、point `[2409,401]`、enabled=true/offscreen=false，同点真实 UIA 查询也是该名称/PID，实际指针匹配。点击后十次字段采样均为空，草稿和已提交文件不变。此轮补齐89批缺少的点击前证据，确认 provider 可见性/几何与画面不一致，不再只是事后截图推测。它仍不证明编译引擎内部的具体根因。

开发期两端分别运行两个真实 RoutingSettingsPage/Rust 规则夹具的 framework 几何诊断，在 DPR1/2、多个滚动位置、展开高级编辑及回顶部时记录语义链、变换、RenderBox布局 rect 和裁切 viewport。可见部分的 framework rect 与布局 rect/viewport 的交集一致，不是像素或 golden 检查；SDK AX 源码父 bounds 原点算法的数学投影可把完全离屏节点重新投到视口。例如一个场景布局 top=-102、framework top=-102，而投影 top=64。投影不是对正在运行的 C++ AXTree 的读取，不能将它写成已证实的 runtime 根因。

前几轮诊断自身的完整 rect/裁切 rect 混用、lazy child 未创建和 SemanticsHandle 释放时机失败均保留，最终两端各2项通过。该探针位于 build，不属于完整 Flutter 常规套件。

## 来源校正、包与边界

此前把 `a804b261…` 同时当作 engine artifact 标识和可引用源码 Git commit 的说法证据不足。本轮明确区分：SDK Git HEAD 为 `d3b14c876900e553bc736ca19295fc09e3853e8e`，engine artifact revision 为 `a804b261645ef8c13eb3d5c44a5c2fb0340c5539`。六份本地 SDK 源码逐字节对应 `git show d3b14c…`，官方固定 URL 和摘要保存在 `upstream/sources.json`；没有建立编译 artifact 到该源码提交的独立映射。旧报告不覆盖，此校正优先。

新四包在 `build/desktop/rule-geometry/package/`。ZIP39/tar32/DEB声明33项及字体/OFL/FontManifest/core/helper逐项核对。Inno只核外层、输入归档与固定编译器链，不独立解码；新安装器均未安装，旧89/88及更早候选保持。

总汇 `build/desktop/rule-geometry/verification.json` 仍为 partial。真实校园、Windows原始/安装/已连接详情GUI、guest Full、长期及其它桌面Wayland、84批心跳/WSL绑定、远端CI和最终审计保持未完成；没有提交或发布。
