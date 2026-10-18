# 霞鹜文楷屏幕阅读版接入与验证

第93批补充（2026-10-18）：隔离Linux引擎与正式main构建通过三来源物理拖动闭环，私有bundle字体/OFL/FontManifest与原产品逐项相同，原90四包保持。引擎补丁尚未进入默认分发，隔离GUI成功不追溯为原四包全部验收通过。字体要求保持完成，见 [隔离构建范围](engine_source_build_acceptance.md)。

第92批补充（2026-10-18）：语义几何源码实验及用户要求的有限缓存清理后，正式Linux bundle全载荷、90四包、原字体及许可摘要保持；23项实验原生检查复验通过，但不替代正式GTK拖动或字体全场景验收。无新产品构建，字体要求保持完成，详见 [语义几何与有限清理](semantics_geometry_acceptance.md)。

第91批补充（2026-10-18）：只改开发拖动守卫及测试，正式main、字体、许可、FontManifest与90四包未变，无新应用构建或包。私有ATK运行库实验不作为字体或默认GUI整体通过的替代证据，见 [运行库隔离实验](accessibility_runtime_acceptance.md)。

第90批补充（2026-10-18）：Linux原生accessible parent及销毁生命周期修复后，两端正式main release与四新包rule-geometry/package继续保持原TTF/OFL/FontManifest字节。新Linux三普通离线GUI进程0，独立native四项通过，字体要求仍完成；远程GTK几何/Windows规则可见性问题保持未签收，不冒充重跑完整Flutter或新安装器验收。见 [原生几何与生命周期](native_accessibility_geometry_acceptance.md)。

第89批补充（2026-10-18）：底栏/通知安全区域修复后，两端正式main release及新四包继续逐字节保持原TTF/OFL/FontManifest；完整Flutter344/343、工具178及静态检查通过。Linux新构建离线设置三进程0；Windows新包私有资源副本三进程基线0后仍在规则新增字段失败，字体要求保持已完成，Windows三来源和Linux物理拖动未签收。新安装器未安装，范围见 [几何与原生点击](desktop_geometry_acceptance.md)。

第88批补充（2026-10-18）：手柄独立语义改动后的两端正式构建及rule-drag新四包保持原TTF/OFL/FontManifest/core/helper；完整Flutter341/340通过。真实手势widget/FFI通过，但正式Linux物理拖动被GTK几何查询阻塞，未签收；新安装器也未安装，见 [拖动回归与阻塞](rule_drag_acceptance.md)。字体要求保持已完成，不把该GTK失败或旧候选证据混写成新包GUI全通过。

第87批补充（2026-10-18）：连接详情图标及字段语义修复后，两端正式构建和新四包继续保留原TTF/OFL/FontManifest字节；ZIP/tar/DEB实载荷逐项核对，Inno核对输入链、本批未安装新候选。Linux合成已连接实窗四类详情和三轮重连通过，详见 [连接详情验收](connection_details_acceptance.md)。正式应用来源ecc18a75…与后续开发观测修复来源0881aa9a…分开记录，字体要求仍已完成，整体迁移active。

第86批补充（2026-10-18）：规则下拉/菜单和测试结果语义修复后已重建两端正式入口及新的四包，原始字体/OFL/FontManifest仍相同；Linux新候选便携与DEB规则管理及安装回归通过，详见 [规则管理验收](rule_management_acceptance.md)。新应用构建来源440d0348…与后续开发驱动来源49ba360e…分别记录，不把新报告追溯用于77/83旧候选。字体要求仍已完成，整体迁移保持active。

第83批补充（2026-10-18）：后续规则控件无障碍改动已形成新的成对正式构建与四包，字体/OFL/FontManifest逐项保持相同。当前产物与独立范围见 [规则控件验收](rule_accessibility_acceptance.md)；本页第77批四包仍保留原样。

当前补充（2026-10-18）：第78批已实际执行同一新字体Inno/DEB安装验收，并完成Linux安装路径Full、设置GUI与托盘；见 [78批安装验收](wenkai_install_acceptance.md)。下文77批“未重跑安装”保留其当时范围，不能据此忽略新增证据；Windows正式安装GUI仍未签收。

第77批，2026-10-18。用户要求字体使用 `lxgw/LxgwWenKai-Screen`，已在 Windows/Linux Flutter 界面接入本地字体并完成两端构建、测试和离线 GUI 验证。本项字体要求已完成；整个 Rust/桌面迁移目标仍为 active。

## 字体与展示

使用上游 `v1.522` 的未修改 `LXGWWenKaiScreen.ttf`，25,673,994 bytes。文件 SHA256 为 `cd1a6fa39c4ea42fd8f4e289945789b0e510cf7016435640f8893cdad9b220f3`，与 GitHub 发布资产声明相同。读取 TTF 内部 name/OS2 表确认原 family `LXGW WenKai Screen`、Regular、weight 400。

原始 `OFL.txt` 保留完整版权与 SIL OFL 1.1 文本，SHA256 `4d72e3080cdb3be63638bb7197c7caae55b6e4cbe1bab1ded2830bdff0c9b438`。上游版本、来源及两个摘要另见 [字体说明](../assets/fonts/README.md)。字体许可证随应用以可读文本资源分发，应用的 AGPL 不替代字体许可。

pubspec 注册 `LXGWWenKaiScreen` family/400，字体与许可证均随包，不在运行时下载。全局 ThemeData、明暗 TextTheme、九个独立 ShuYoTextStyles 工厂使用同一 family；日志不再覆盖为系统 monospace。现有字号、行高与字重请求保持，只有一个上游 Regular 字体文件，没有冒充独立 Medium/Bold 文件。原生 OS 菜单和字体不覆盖的字符仍依照平台实际能力处理。

三处生产 Dart 改动均为展示：`theme.dart`、`shuyo_text_styles.dart`、`log_page.dart`。没有新增运行依赖或锁变化；逐项复核后更新108文件边界清单中的对应摘要与 pubspec 摘要。打包门禁现在要求字体及 OFL 同时存在，两平台缺任意文件都在创建输出前拒绝；CI 的源码触发路径新增 assets。

## 当前证据

总汇是 `build/desktop/wenkai-font/verification.json`，复核脚本是同目录 `record.py`。工具/产品共同来源474文件、SHA256 `9a9ebbf5575f7a63f174437a04a8a8340a6654439ad1b72404ff49fd1af05c90`，Windows/Linux 两树一致。

| 检查 | 本批实际结果 |
| --- | --- |
| 完整 Flutter 测试 | Windows317 / Linux316，通过 |
| 实际字体加载 | 应用外壳测试用 FontLoader 加载随包 TTF 后运行页面、抽屉、日志、账户和弹窗用例 |
| Python 工具测试 | 两端各111，通过；Windows19/Linux7平台跳过 |
| 静态分析、Dart边界 | 两端 analyze 无问题，108文件清单通过 |
| 正式入口 release | 两端 `lib/main.dart` 构建通过 |
| Windows GUI | 新字体 ZIP 的私有 EXE 资源副本，三个普通进程退出0 |
| Linux GUI | 新字体正式 bundle，三个 UID1000/CapEff0 进程退出0 |

两端 GUI 均执行欢迎/权限/账户说明/登录表单可达、离线主题、70000拒绝/18443保存、Full保存与重开/Campus恢复、仅校园规则应用与重开。Windows还核对完整非空覆写与独立原生决策；Linux最终原生读回同样保留两个覆写。未输入真实账户或提交登录，网络四开关保持 false。截图已查看，不能扩展为所有字形、全部缩放或全页像素审计。

Windows 的第一次准备在启动应用前被严格入口检查拒绝：默认 Flutter 构建写出 `lib\main.dart`，准备器只接受 `lib/main.dart`。没有改写生成文件或放宽守卫；使用显式 `flutter build windows --release --target lib/main.dart` 重建后，EXE/AOT/core/helper与已打包字节逐项相同，再完成 GUI。原失败日志 `prepare-windows.log` 保留。

Windows 测试的两个新 KnownFolder 根和互斥锁均无遗留，最终 CIM 无应用/fixture 进程。宿主两应用目录、两代理键在本批前后相同；Linux三个 GUI PID 回归结束后均不存在。此观察范围仍不是全系统持续监控。

## 新的本地候选

四种产物位于 `build/desktop/wenkai-font/package/`，版本仍 `0.4.2+9`。原第72批四种候选保持原摘要，没有覆盖或发布。

| 产物 | SHA256 |
| --- | --- |
| Windows ZIP | `6e9c4503de857f08d1a98c3749f0b76deacfe01f3c63d230467733b67c29c679` |
| Windows Inno EXE | `a96fac8cfc03bd9309bbbcca80cb18174369a0de8fd7e842837e4ca232376eda` |
| Linux tar.gz | `57e63552302f2334682f3310e87d2ea702be428463e965a503e9fa4414b1d356` |
| Linux DEB | `317355b3900f19e0cde7fea952a07137d5f5dde443eaf2f428bf39db4ee2f9f1` |

独立只读复核逐文件验证 ZIP39项、tar32项与 DEB `/opt/shuvpn` 的33项清单及其实际文件集合、大小和摘要。三个归档中的 FontManifest family/400、TTF与 OFL 均符合固定原文件；DEB固定helper与程序目录helper一致。`/usr/lib/shuvpn/package.json` 是授权helper旁的同一清单副本，不表示该目录也包含完整GUI载荷。

Inno 编译生成、输入摘要、payload/builder来源及结果摘要已核对，安装器仍 NotSigned。本批没有重新安装/升级/卸载这两个新安装器；此前72批安装证据不改写为77批已安装 GUI。不能只凭 Inno metadata 宣称已核验其内部全部载荷。

本批没有重复完整 Rust、网络或安装矩阵，没有远端CI、发布或提交。真实校园、原始EXE默认profile/安装WindowsGUI、独立可恢复guest的特权Full、长期运行、其它Linux桌面/Wayland及最终迁移审计继续保留。

回退本批时一并恢复三处展示接线、pubspec字体声明、匹配的打包必需项及边界清单；不涉及用户账户/设置格式迁移。旧候选和第76批字体改动前的独立GUI证据仍保留。
