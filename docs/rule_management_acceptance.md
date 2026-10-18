# 桌面规则排序、启停、策略组与回退验收

第86批，2026-10-18。当前工作补充原计划 M4.4 的正式 Linux 窗口验收；完整桌面迁移目标仍为 active。测试只使用 `*.gui.invalid` 合成规则、DIRECT/REJECT 和独立临时配置，不提交登录、不读取真实账户、不建立目标连接，不启用系统代理或 TUN。

## 实现与回归范围

新增 `tool/desktop_rule_management_acceptance.py`。它先执行原有三进程离线设置基线，再用六个不同进程验证：

1. 导入两个同目标但动作相反的规则，以及实际引用手动策略组 Office 的规则；草稿和预览期间持久文件不变，应用后独立读取版本3。
2. 通过具名“上移”按钮交换规则顺序，应用前测试器仍显示版本3的 REJECT，应用后版本4变为 DIRECT。
3. 通过具名开关停用第一条 DIRECT，草稿保留规则索引；应用后版本5命中第二条 REJECT。
4. Office 从 DIRECT 切至 REJECT，明确应用前仍为旧决策、应用后版本6才改变组规则的实际动作。
5. 使用页面“回退上一版本”，得到新版本7；配置与覆写逐字恢复为版本5，Office 恢复 DIRECT。
6. 关闭并重新打开应用，核对原文、Office 选中项、两条测试器决策，再关闭 GUI 并由独立 Dart/原生进程读取同一配置。

测试器每次先切换协议以清除旧结果，确认旧结果消失后才输入目标、触发新测试；不把 UDP 选择动作计作 UDP 匹配或实际拨号验收。图标、开关、菜单操作使用 AT-SPI 动作，不能称为物理鼠标命中测试。

新流程保留原有三来源 CRUD 验收，并独立接入 Linux 本地 opt-in、bundle CI 和 DEB 安装验收入口。报告使用不同目录；CI 只上传顶层与 `baseline` 的 PNG/JSON/log，不递归上传 `profile/data`。

## 本轮发现的展示问题

旧正式构建的网络协议下拉框实际报告 `name=网络协议\nTCP`、当前 PID、`showing=true`、`enabled=false`。新增真实原生页面测试进一步复现策略组下拉框的 enabled 三态为 `none`。没有放宽动作守卫。

对策略组、网络协议、入站类型三个下拉控件及菜单文本补上 `Semantics(enabled: !busy)`，控件使用 `MergeSemantics` 保留合并后的标签和按钮动作。原有 `onChanged` 和业务命令保持，未增加语义 `onTap`。新测试同时验证正常/忙碌状态、名称、按钮语义、明确启用/禁用标记，以及导出的节点启用时恰好一个 Tap、禁用时零个 Tap；实际派发按钮语义动作打开三个菜单并验证选项。合并入父节点的内部语义节点不重复计为平台导出节点。生产改动仅为一个 Dart view 文件，边界清单在亲读后更新对应摘要。

另修复验收器两个问题：基线子目录不能改变原生配置文件位置；外层窗口管理器/会话清理失败后不能留下本次 `passed:true`。后者有先失败后通过的回归，且拒绝已存在的报告目录时不会改写旧报告。失败主因与清理错误分别保留。

随后 `portable-06` 已实际完成具名协议菜单与输入、执行测试，截图显示正确版本2决策；但可选文本的 AT-SPI 名称和文本读取均为空（`diagnostic-05` 保存实际读数）。新增真实原生页面回归确认结果语义 label 为空；为 `SelectableText` 设置与可见原文相同的内置 `semanticsLabel`，验证导出 label 完整且 value 为空，避免重复朗读。原生匹配与可见结果不变。验收器保留名称与文本接口两种读取，并对空名称旧结果新增拒绝测试；只读文本诊断失败不会计作测试器成功。

`diagnostic-06` 至 `diagnostic-12` 又保留了同页提交版本5之后的输入读回失败：截图中的输入值正确，但 AT-SPI Text 会返回字段标签，重新取得节点、显式刷新缓存和中间输入均未消除。完整控件层级及值保存在 `text-input-mismatch.json`，不据此修改原生匹配逻辑。最终输入检查在具名、当前进程的字段聚焦和实际键入后，使用同一私有 Xvfb 显示会话的 Ctrl+A/Ctrl+C 与 GTK clipboard 回调读取实际文本；必须逐字等于目标值，AT-SPI 读数另存，不把标签当成值。该会话由原有 token、独立 DISPLAY/DBus 和普通 UID 守卫限定。复制值错误即使 AT-SPI 值正确也必须失败，已有回归覆盖。复制结果只来自本次合成界面，不是对桌面全部 Text 接口正确性的签收。

## 证据与限制

本轮报告根为 `build/desktop/rule-management/`。总汇 `verification.json` 与复核入口 `record.py` 已完成，输出 `LINUX_RULE_MANAGEMENT_SCOPE_SEALED_MIGRATION_STILL_PARTIAL`。总汇保留 `passed=false`、`partial=true`、`migration_goal_complete=false`，其中 `linux_management_scope_passed=true`；不能把本批 Linux 子范围写成整个迁移或 Windows GUI 已完成。

| 本轮最终检查 | 实际结果 |
| --- | --- |
| 完整 Flutter | Windows326 / Linux325，通过 |
| Python 工具 | 两端各158，通过；Windows19 / Linux7平台跳过 |
| 新驱动守卫 | 13项，通过，包含旧结果、错误复制值、来源/版本和清理失败 |
| analyze / 生产 Dart 边界 | 两端通过；109文件 |
| 正式入口构建 | 两端显式 `--target lib/main.dart` release，通过 |
| Linux shell | 本地验证与 DEB 入口分别 `bash -n` 通过 |
| 便携规则管理 | `portable-07`，完整9普通进程、全部退出0 |
| 实际 DEB 三来源 CRUD | `installed-01/session`，完整9普通进程、全部退出0，最终版本8 |
| 实际 DEB 规则管理 | `installed-01/rule-management`，完整9普通进程、全部退出0 |

两个完整管理流程各保存11条新的 HTTP/TCP GUI测试结果、22次私有显示会话内的字段复制读回，以及6次关闭 GUI owner 后的独立原生读回，版本依次为3、4、5、6、7、7。前后置覆写始终保留，四个网络开关均为 false；全部 GUI 记录 UID1000、CapEff=0。安装 GUI 的 PID 属于私有 PID namespace，不能当成宿主 PID。

完整应用构建及最终四包的来源为488文件、SHA256 `440d034800354bd52d9dc202acac7b307536a493290bd3131a0c4b5f040847b5`；之后仅开发驱动和其守卫测试继续调整，最终工具/两端GUI/158工具测试来源为488文件、SHA256 `49ba360e3fd4502f2a528ced7297fdfa7612408b398e5189222b468641cd97cc`。两树对应来源分别一致。生产 view 最终摘要为 `d94accb8ae32d5f38b7c9645e9285ab442dea5e316b949fc2f4b596e308dc376`；生产 Rust、绑定、运行依赖及字体保持。

最终本地候选在 `build/desktop/rule-management/package-final/`，版本仍0.4.2+9；中间 `package/` 来源47ec7bf8…及旧83/77批候选保留，没有覆盖或发布。

| 最终候选 | SHA256 |
| --- | --- |
| Windows ZIP | `a8e2784c5e9a9287d04d8bb9e4ba9a63dd04e724e468a5b943b5aee33120e9e5` |
| Windows Inno EXE | `4028623aca59e23ed4742e068a47a6055785f189f37e4e1965a7e62bb07f087e` |
| Linux tar.gz | `0e6a9fe88340dec3fd64d0c307107be203c61cafff2d7b11cee725c7206ad408` |
| Linux DEB | `fd56136e85a022eb988094d84751f32ec91a4687fd21aa2590a9cbc60974d3b3` |

独立逐文件复核 ZIP39项、tar32项与 DEB `/opt` 的33项声明和实际集合、大小及摘要通过（均不把 manifest 自身计入声明条数）；两安装器输入/生成来源一致。DEB 的包管理标记、README 与 manifest 按 Debian 分发格式改变，其余 portable 文件逐字节一致。三种可读 payload 的原始 TTF/OFL/FontManifest 与原字体版相同；两端 core 和配套 helper 保持原摘要。新 Inno 的实际签名状态仍为 `NotSigned`；仅生成及外部文件/metadata核对，未执行新 Inno 安装。

实际 DEB 安装/合成旧 control 版本升级/权限拒绝/运行保护/原生账户与非包数据保留/卸载清理均通过，日志保留预期拒绝。宿主账户与包数据库前后比较相同，最终宿主未留 `/opt/shuvpn` 或 `/usr/lib/shuvpn`，本次应用/helper映像扫描为空；这不是全系统泄漏证明。所有本批构建、GUI、安装句柄均已终态，无在途任务。

截图已抽看。`SHOWING` 是平台提供的状态，本批没有用矩形接口证明裁切或物理命中；部分图片中的结果/策略组在视口外，结果依据当前 PID 的实际语义值与独立原生读回，不以截图替代。最终重开截图可见 `profile:2 · DIRECT · 版本 7`。便携版22次输入读回中有两次 AT-SPI 文本与真实复制值不同，记录于 `portable-07-audit.json`；保留此互操作限制，不称全部 Text 接口已修复。

原始失败保留在 `portable-01` 至 `portable-05`：分别涉及基线路径、逐字输入未保留换行、导入后高级区域重建、下拉框启用标记和 GTK 几何接口 dbind 超时。几何查询依赖已移除，未用截图坐标绕过动作守卫。后续诊断只复制封存的两个合成原生记录，不执行基线，因此不计为九进程完整验收；首个诊断因复制文件权限不符合原生存储要求而失败，修正权限后的诊断确认了下拉框启用标记问题。

新增 Flutter 回归最初还暴露测试自身的忙碌动画 `pumpAndSettle` 超时、语义句柄释放时机、对 `void` API 使用 await，以及合并语义节点的查询/计数问题，均保留日志并修正；它们不是新的产品故障。菜单启用标记缺失则有单独失败用例。Windows 全量首轮在未提供 README 要求的独立 Cargo 库 Wintun 路径时失败，后续复验须明确测试环境与随包驱动验证的区别。

Windows 最终只读观察仍为同一前台“Windows 安全中心”（HWND5639302，`Shell_SystemDialog`，PickerHost.exe/PID63020），没有向其输入或启动本批正式应用。需用户自行处理该系统弹窗后才能继续 Windows 实窗验收。

本轮未因此签收 Windows 规则窗口、真实校园、特权完整捕获、长期运行、其它 Linux 桌面/Wayland、远端 CI 或整个 M4.4（拖动与连接详情等仍须补齐）。第84批心跳压力失败及 WSL 宿主绑定机制的未决问题继续保留；旧83批和字体批产物不由本轮工具或文档追溯改写。没有提交或发布。
