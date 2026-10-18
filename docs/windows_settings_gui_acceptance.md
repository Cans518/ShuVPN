# Windows 离线端口、捕获模式与规则 GUI 验收

第76批恢复，2026-10-18。用户恢复持续推进后，正式 `lib/main.dart` 的私有版本资源副本完成两次验收，每次三个普通权限进程正常退出。总汇为 `build/desktop/windows-settings-gui/verification.json`，最终运行是 `run-03/04`。这些是本地忽略产物；文档不能代替其他 checkout 上的实际证据。

本批仍只改变开发工具和测试。生产入口、AOT、原生库及第72批候选保持原样。后续字体调整属于新的产品变更，不能用这里的旧 AOT 证据证明字体版已经运行。

## 来源与对象

工具来源471文件，SHA256 `7afadab042e990b71fbcef2c79470404735b8c47ea34b88e5c917a303bc30011`，Windows/Linux 两树一致。原始 Windows ZIP 仍为 `0dc3fc111b5afeac5bac83412e849b72c37155c03aa9293f26985d91fe706dfd`，产品来源462文件 `7abf62372522eddd35b7279eddb82ea8b44604181422d616497eadd7d8b78f33`。

准备器排他预留新的实际 KnownFolder 目录，只修改复制 EXE 的两个等长版本资源字符串。两个副本各37项载荷均重新计算摘要；36个非 EXE 文件保持原包内容，AOT SHA256 为 `351535b462f105a5a9274187ba39692dca213626491deac95c7d35dcc8b23981`。EXE 的资源范围外字节保持。它不是原始 EXE 默认 profile、实际安装版 GUI 或独立用户/网络沙箱。

## 已执行流程

1. 首次引导到达权限页、账户说明和登录表单；只输入受控假密码，要求16字符遮罩后清空，不输入用户名或提交。
2. 首进程正常退出后，独立 Rust 探针准备 `welcomeCompleted=true`，并以正式路由 API 提交版本1的“校园优先”测试配置。覆写包含 `before.gui.invalid` 前置与 `after.gui.invalid` 后置 DIRECT 规则。这是离线测试准备，不是认证成功。
3. 第二进程实际点击保存深色主题、Full 捕获模式；输入70000得到原生“设置值无效”，前后设置记录相同；随后保存18443并核对 GUI 行值。
4. 进入规则页之前读取原始路由文件并核对 seed 版本。打开页面、选择仅校园模板及预览均不得改变文件字节。实际点击应用后，版本恰好1→2，profile 为 `MATCH,REJECT`，providers 为空，完整非空 overrides 原文保持。
5. 正常退出后，用独立 native session 读回 dark、18443、Full、版本2及完整两个规则文档。两覆写域名的原生决策均为 DIRECT，`198.51.100.7:443/tcp/tun` 为 REJECT；这些是纯规则测试，没有建立目标连接。
6. 第三进程重开，GUI 核对端口/Full、规则名称/版本/REJECT兜底，改回 Campus，再退出独立读回。打开规则页前后的完整文件保持。三个 native 时点的 HTTP/SOCKS/VPN/系统代理四开关均严格为 false。

最终设置 revision 为5，路由版本2；两次运行六个原 GUI 进程均退出0，默认配套库加载、创建时间、EXE摘要、Explorer会话、无自身监听和托盘消失均由既有驱动核对。主题仍只做单点颜色与截图检查，不签收全页像素或 UIA selected/password 标志。

## 驱动修正与失败保留

原 `run-01` 的指针失败保持未通过。恢复后在3072×1920桌面中，一次相同目标再次偏离，随后三个目标点均准确抵达；没有足够证据归因于固定 DPI 比例。驱动增加桌面/裁剪/显示器边界检查，超时保留归一化输入及 logical/physical 两种指针读数，二者都必须在目标±2像素内。没有删除或放宽抵达断言。

`run-02` 通过首次激活和主题保存，在悬停“返回”时遇到同名控件而失败。定位现在区分按钮与编辑框类型，排除同名提示文本；同名同类型目标仍拒绝，继续检查实时矩形、所属PID及真实鼠标点击。新增纯内存回归覆盖提示文本和两个同名按钮。

独立审查发现的规则页打开前基线缺失及异常后保留 passed 两处问题已修正。异常观察与清理失败不能升格通过。

Windows 已连接界面回归首次在 Explorer overflow 图标定位超时，失败记录为 `connected-regression/verification.json`，cleanup_error=null；第二次 `connected-regression-02` 三轮12条双栈 HTTP/SOCKS 64KiB/prelude/FIN 流及 owner/监听/托盘退出通过。它使用第74批开发 AOT，不冒充本批正式主入口联网或超时原因已修复。

## 验证与清理

两端各110项工具测试通过（Windows19/Linux7平台跳过），两端 `flutter analyze` 与108文件 Dart边界检查通过。共享 native probe 新增非空规则 seed 后，Linux 当前正式 bundle 另执行完整离线设置回归，三个 GUI 均 UID1000、CapEff0、退出0，配套库及产品摘要匹配；最终原生读回也保留两个覆写与四个 false 开关。报告复制到 `linux-regression/`，仅保留测试 settings/rules 文件作为存储旁证。

恢复后的两宿主应用目录及两代理键摘要前后相同，并与暂停后快照相同。第一次恢复快照对二进制值使用了不同编码，保留为 `host-resume-before.json`；跨历史比较使用纠正后的 `host-resume-before-compatible.json`，不是把编码差异当系统状态变化。

第76批最初 `host-before.json` 与 `host-after-pause.json` 的代理差异仍未归因；**不能说整个76批宿主代理保持不变**。本轮没有回写系统代理。全部四次准备的8个 KnownFolder 根逐项不存在，验收互斥锁不存在，最终 CIM 无应用/fixture 进程。Linux 三个 GUI PID 在回归结束后均不存在。

本批未重跑完整 Rust/Flutter/安装矩阵、未重建产品或候选、未提交发布。未完成真实校园/connectAccount、原始EXE默认profile或Windows安装GUI、全部离线设置、独立可恢复guest特权Full、长期运行、其它Linux桌面/Wayland、远端CI及原计划最终逐项审计。持续推进目标保持 active。
