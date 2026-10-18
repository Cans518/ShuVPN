# 规则控件名称与三来源桌面弹窗验收

第85批补充：Windows三来源驱动与失败/清理守卫已准备，但当前系统安全弹窗阻挡，Windows真实路径仍未验收；Linux共享检查的新一轮9进程回归通过。详见 [Windows驱动及阻挡记录](windows_rule_dialog_acceptance.md)。下文保留第83批产品和实际范围。

2026-10-18，第83批。规则卡片的编辑、上移、下移、删除按钮此前在 Linux AT-SPI 中名称为空。现在为按钮提供明确名称，并将第82批的前置规则试跑整理为常规三来源验收工具。霞鹜文楷屏幕阅读版继续随包，字体文件及许可均未改变。整个迁移目标仍为 active。

## 展示改动与验证

新增纯展示组件 `LabelledIconButton`，通过 `MergeSemantics` 合并显式名称与原生 `IconButton` 的动作、焦点及禁用状态。视觉 Tooltip 保持，排除其重复语义；不在包装层增加点击回调或覆盖 enabled/focusable。规则列表名称包含来源、绝对行号、动作和表达式，分页后行号不重置。

三个新增按钮分别导出“新增前置规则”“新增配置档规则”“新增后置规则”，仅可见文字子节点排除重复语义，保留原生 TextButton 动作。开关、分页、拖动提示以及规则页重命名/最终兜底图标也获得明确名称。规则解析、编辑、预览、提交、持久化仍由既有 Rust 服务负责。

生产 Dart 变化仅为新增组件、规则列表及规则页三文件。对应边界清单109文件，view37，无新运行依赖、绑定、pubspec 或 Rust 生产源码变化。边界复核与运行证据分别记录，不用清单通过代替窗口验证。

`widget-red-03.log` 保留旧实现实际导出空 label 的失败；更早两次分别是测试导入位置和 finder 假设问题，不算产品语义回归。最终7项控件测试验证 busy/非 busy、四按钮名称及动作/焦点标志、开关、来源唯一新增按钮，并实际执行语义点击和 Tab/Space，确认两次操作各触发一次回调。焦点断言读取 `getSemanticsData().flagsCollection` 的导出合并数据；早期读取未合并节点标志的失败另保留。

| 检查 | 本批结果 |
| --- | --- |
| 完整 Flutter 测试 | Windows324、Linux323，通过 |
| Python 工具测试 | 两端各132，通过；Windows19/Linux7平台跳过 |
| 静态分析 | 两端 `flutter analyze --no-pub` 无问题 |
| Dart 边界 | 两端109文件清单通过 |
| Shell 语法 | 两个脚本分别经 `bash -n` 检查 |
| 正式主入口 | 两端显式 `lib/main.dart` release 构建通过 |

日志与总汇位于 `build/desktop/rule-accessibility/`。产品、工具和测试源树482文件，SHA256 `f8464b55b25cb45d4de1c3e0a372a66302117247602f9a17bb1a6bc6e9c78c27`，两端重算一致。

## Linux 正式窗口

新工具 `tool/desktop_rule_acceptance.py` 继承已有离线设置基线，先执行原3进程的欢迎、表单、主题、端口、捕获模式、规则及非法规则回归，再分别对 prepend/profile/append 执行两个进程的增改删流程。

便携新正式 bundle 的 `portable-01` 已通过，9个不同PID均 UID1000、CapEff0、默认库映射正确、退出0。规则版本由基线2依次提交至3/4、5/6、7/8。三来源各自新增取消、保存，编辑取消、保存，预览、应用，重开编辑内容，删除取消、确认、预览和应用均完成。

定位要求本次进程、完整名称、角色、enabled 和 showing；重复名称立即拒绝，不使用无名图标排列或卡片内按钮槽位回退。共记录41次具名控件动作、3组卡片四按钮/开关能力、6次关闭 GUI 后的独立原生读回。`do_action(0)` 的0是已具名节点的平台动作索引，不是图标顺序。

每阶段交叉检查两份原始草稿、实际文件字节、生效版本。取消、编辑保存和预览均不提前写入生效文件。编辑应用后关闭进程，独立 Dart 进程加载相同 Rust 库，核对 created 域名 REJECT、edited 域名 DIRECT及实际 source/version；重开 GUI 的两份原文与持久记录相同。删除应用后独立匹配变 REJECT；原前后置两个 DIRECT 覆写始终保持。每来源删除后的结果由独立原生读取证明，没有额外再启动第三个窗口签收删除后 raw。

输入和滚动使用实际键盘/滚轮，按钮使用平台辅助功能动作，不宣称物理指针命中。截图已查看前置编辑草稿、配置档删除确认和后置编辑重开。所有网络开关始终 false，没有真实校园、代理数据流或 TUN。

失败报告会覆盖旧 true，保留已完成基线；最终清理成功后才发布整体通过，二次清理错误不掩盖原始失败。这些边界有7项独立工具回归覆盖。外层 CLI 退出及进程清理仍须结合报告核对。

同一新 DEB 的 `installed-01` 也已通过：在独立 mount/net/PID namespace 实际安装后，从 `/opt/shuvpn` 启动9个普通用户 GUI，默认映射 `/opt/shuvpn/lib/libshuvpn_core.so`，全部退出0，三来源及版本2→8、41次具名动作和6次独立读回与便携版相同。这里记录的GUI PID属于私有namespace，不能当宿主PID。

安装脚本还实际验证陌生目录/符号链接拒绝、setcap故障传播、授权与未授权账户、运行中拒绝卸载、合成旧控制版本升级、非包文件与原生账户保留、移除/清除/重装和最终清除。日志中的预期拒绝不是被忽略的错误；最外层命令最终退出0。宿主账户/包数据库摘要前后相同，`/opt/shuvpn`、固定helper目录及本次进程清理完成。未在本批重跑 Full/TUN 或托盘；第78批同一原生载荷的对应证据保持其原范围。

## 产物与 Windows 界面

新四包位于 `build/desktop/rule-accessibility/package/`，版本仍 `0.4.2+9`，为部分桌面迁移候选，未发布。第77批原字体四包保持原摘要。

| 产物 | SHA256 |
| --- | --- |
| Windows ZIP | `d3c15044a82b6e5d2b4927dbf4d1777c4507b8dcd3acd71276ccc6ee48e99348` |
| Windows Inno EXE | `ba63d811568b73242fe64684896b20090ae54ccaa7191564cc28a0cdb88a17d2` |
| Linux tar.gz | `3f308f3fbb163708f5570d8130379a4c2871fcadbb811c97e3b8464f9b3fd7e6` |
| Linux DEB | `a981f2a9b4fe5fc3ad41f18b931745eaa136d79a7149e5908e7041c995e82cb0` |

两端构建均包含同一原始 TTF（25,673,994 bytes、SHA256 `cd1a6fa39c4ea42fd8f4e289945789b0e510cf7016435640f8893cdad9b220f3`）和 OFL（4,455 bytes、SHA256 `4d72e3080cdb3be63638bb7197c7caae55b6e4cbe1bab1ded2830bdff0c9b438`）。FontManifest 同为181 bytes、SHA256 `70ddf17d6bf14dcc9885bf4f8da3608c6f32eacbfad5e18cc31a0fdbfca8ef70`，family `LXGWWenKaiScreen`、weight400。全局、独立标题和日志仍使用本地字体。

独立只读复核检查 ZIP39项、tar32项和 DEB `/opt/shuvpn` 清单33项的集合、大小与摘要；这些数量不含各自 `package.json`。两端原构建的37/30个文件分别逐项等于包中对应文件。DEB两份清单相同、40条md5sums完整匹配，固定helper与程序目录helper一致。安装器input/payload/builder来源、产物摘要及旁路一致。Inno本批仅生成并核对本体/来源，未实际安装，不把metadata当作内部载荷已验证。

Windows core仍为 `13a6d6e7ab641093092cd8033d53c464071cbba14fec7cd3e39afdbdd64bab2c`，Linux core仍为 `d4732d4f1521809dab8f6ef0ab99817da4f19174cce7cde5352a4b719da3cc38`，Linux helper仍为 `061c64a1058778b8a48f2564f45732b05c9a4ae2780f13ceaf64f199965f72c6`；新AOT包含本批展示改动，不能沿用旧AOT签收。

Windows `windows-01` 使用新 ZIP 的正式 AOT 与私有 EXE 资源名称副本，三个普通进程正常退出，原离线设置与非法规则回归通过。AOT 及其它载荷不改，两个资源字符串之外代码不变。宿主两应用目录/两代理键前后摘要相同，两测试 KnownFolder 根和输入互斥锁已移除，当前候选路径无进程。此处不签 Windows 原始 EXE 默认 profile、实际安装 GUI 或 Windows 三来源规则弹窗。

## 常规入口与剩余范围

本地 `SHUVPN_DESKTOP_GUI_ACCEPTANCE=1 bash tool/verify_desktop.sh`、DEB 的 `SHUVPN_DEB_GUI_ACCEPTANCE=1` 和 Linux CI 的正式 bundle 步骤均已切换为三来源工具，输出目录保持既有约定。CI 已配置，未实际运行远端工作流；可选 GDB 观察器仍按原3进程范围，不宣称调试器覆盖9进程。

本批不重复完整 Rust、真实网络、TUN/Full 或托盘矩阵。第82批 Linux 449.938秒提前 EOF 仍未归因，后续642.822秒复验通过不消除原失败。本批窗口成功同样不替代长跑或校园证据。

真实校园账户、Windows 原始/安装 GUI、可恢复 guest 的 Windows 特权 Full、更长/多日运行、其它 Linux 桌面/Wayland、策略组/连接详情等完整窗口矩阵、远端 CI 与最终迁移审计继续保留。当前只签本页明确执行的范围，没有提交或发布。
