# 语义几何的原生复现与有限清理

第92批，2026-10-18，部分完成。确认 Linux 正式 AOT 的真实语义输入能在原始 SDK C++ 适配层中重现两只手柄的实际 AT-SPI 错位；实验源码补丁通过限定原生检查。补丁尚未进入正式引擎，真实窗口拖动仍未签收。按用户追加要求，测试收尾后完成工作区缓存的有限清理。

总汇：`build/desktop/accessibility-geometry/verification.json`。本批没有改生产代码、开发工具或常规测试，两个工作树仍为495文件、SHA256 `f4a84895e16e9c1fb9998610826f2e5e480890e27ddf675aeba1d60df82321ad`。正式应用/四包仍为90批来源 `0ecc5605…`；没有新应用构建、安装器、提交或发布。

## 原始 C++ 与实验补丁

从固定 SDK Git HEAD `d3b14c876900e553bc736ca19295fc09e3853e8e` 核对并复制 `fl_accessible_node.cc`、`fl_view_accessible.cc`、`fl_value.cc` 到本批 build。编译独立可执行夹具，使用真实 GLib/ATK 对象和 extents 调用；不链接 Flutter DSO、不启动 Dart/引擎、不输入账户、不操作网络。

夹具以普通 GObject 代替仅用于弱引用的未启动 engine；动作派发和可编辑文本创建桩在意外调用时 abort。原生窗口边界用固定 ATK anchor 表示。因而它证明几何处理路径，不是完整 SDK、编辑文本或真实窗口的验收。

| 检查 | 原始源码 | 最终实验源码 |
| --- | --- | --- |
| 6场景、20项坐标断言 | 18项失败；普通平移的SCREEN/WINDOW两项通过 | 20项通过 |
| children-changed新增回调内查询 | 通过 | 初版回归为零尺寸；调整提交顺序后通过 |
| 两条真实捕获节点链回放 | 两项均重现实际AT-SPI错位 | 两项均返回完整变换后的预期位置 |
| 清理后的既有原生二进制复验 | 保留原失败记录 | 三个可执行文件、合计23项通过 |

20项覆盖非零父矩形原点、叶矩形原点、根缩放2、嵌套缩放、SCREEN/WINDOW/PARENT、仅祖先和仅叶节点的局部更新。“根缩放2”是输入矩阵测试，不等于真实 GTK 高DPI环境已通过。

补丁缓存各节点的原始 rect/transform，组合父子坐标变换，再以 child全局bounds−parent全局bounds写入 ATK 相对坐标；不把父节点的矩形原点当作子坐标系原点。PARENT 查询不再继续累加父节点位置。初版只在 children-changed 通知之后提交几何，独立审查提出风险，新增原生测试实际得到 `[120,50,0,0]`，原版为 `[130,70,100,50]`；最终先按待提交子树计算几何、设置所有 parent，再发子节点变化通知，恢复预期结果。

最早5项实验、view-only版的PARENT失败、通知回归red及编译兼容失败均保留。当前编译收据分别记录固定原版来源、实际编译输入及补丁/header/二进制摘要；不能把 pinned_original_sources 当成补丁输入摘要。旧通知red收据未在执行时绑定全部输入摘要，归档补丁及记录保留，不追写成当时已有完整绑定。

另一个审查意见认为父查询传NULL尺寸指针会崩溃。实际原生调用没有崩溃；系统 `atk_component_get_extents` 的反汇编 `d623–d64f` 显示公开包装函数把NULL替换为栈地址，再调用实现。该意见未被采纳为已证实缺陷。

## 真实窗口输入与原生回放

只在私有诊断 runner 中连接现有 `update-semantics` 信号，读取节点ID、rect、transform、children和测试标签。严格核对结构大小，限制更新数量；不修改信号输入，也不替换无障碍类或 vtable。诊断 bundle 与90批正式 bundle 全文件比较，仅 runner 不同；AOT、Flutter DSO、Rust、helper、字体及其它资源相同。

`capture-drag-01` 首个进程在窗口管理器关闭后未按时退出，保留失败并清理。`capture-drag-02` 用应用显式 Ctrl+Q 退出：前三个基线和第四个导入进程退出0，第五个进程仍在真实鼠标拖动后等不到规则原文顺序变化。所有原有几何/所属窗口/按钮释放/完整原文守卫保留，未把显式退出诊断写成原Alt+F4路径通过。

拖动前同步捕获的两只节点：

| 节点 | 实际AT-SPI矩形 | 原始源码在相同输入上的原生结果 | 实验补丁原生结果 |
| --- | --- | --- | --- |
| 245，第二条DIRECT手柄 | `[368,488,48,48]` | `[368,488,48,48]` | `[368,402,48,48]` |
| 232，第一条REJECT手柄 | `[368,381,48,48]` | `[368,381,48,48]` | `[368,241,48,48]` |

两个节点分别多偏86、140像素，推算出的原生边界同为 `[120,50]`。已查看点击前截图，完整变换后的中心 `(392,426)` / `(392,265)` 与图中手柄一致。没有使用这些推算坐标去代替失败的 provider 坐标操作窗口。

`capture-native-replay.json` 回放两条祖先链的并集，共16个节点；保持其几何输入，过滤其它分支，flags/label/value使用桩。它比单纯公式投影更强：原始 C++ 的实际 ATK 输出逐项等于本次真实 AT-SPI 读数，补丁 C++ 输出也逐项核对。但它仍不是把补丁装入真实 Flutter DSO 后拖动成功的证明。

两轮诊断的已知GUI/driver PID及引擎准备的Git PID均已消失，最终无工作树build路径映像；两轮宿主link/route/rule前后相同。系统ATK桥摘要仍为 `13670b19f87dbf32b697104cb5019c91ad886d8122e8d645c27162c97e41e877`，未安装私有库或固定helper。

## 正式接入路线与限制

现有 `libflutter_linux_gtk.so` 将GTK适配层与内部embedder合为一个库。固定GN将核心入口设为内部前缀且不导出；实际release DSO的247个已定义动态符号中，没有FlutterEngine/Embedder入口，165个为fl_开头。已查缓存范围没有可单独链接的核心库或静态归档。因此不能仅把两个C++文件重编后，通过公开ABI替换现有缓存中的GTK层。

官方合仓源码流程使用根 `.gclient`、固定DEPS及host构建。已有SDK缺少depot_tools、Skia/Dart源码及引擎构建工具。先核实WSL VHDX在D盘；当时D盘仅余约32GiB，没有按虚拟磁盘显示的821GiB启动大规模依赖同步。

私有C盘目录已取得depot_tools。独立、非hardlink的源码clone在300秒边界内未完成检出，已定向终止-15，记录 `engine-preparation.json`；没有gclient sync或引擎构建。原SDK HEAD、Git配置摘要及受查跟踪文件状态保持。该不完整检出目录的删除被自动审批拒绝，理由仅为“blocked by policy”，目录保留且没有在途clone。后续不得假定它已是可用完整checkout。

实验源码尚未验证实际GTK缩放、全部非仿射变换、所有新增/删除/重挂接通知、异常拓扑/深树/线程、极端外部原点等；不能发布为通用引擎修复。91批私有ATK桥自身的线程/重入/分发评估也仍未完成。

## 用户要求的有限清理

完成本批原生测试后，只删除预先列举的可重建缓存。Windows操作在同一PowerShell脚本中核对绝对路径、工作区边界及reparse points；Linux核对明确路径、父路径和子项不含符号链接。删除前未发现cargo/rustc/dart/gradle等构建消费者。没有递归清整个build、target、SDK或用户配置目录。

| 范围 | 实际删除 |
| --- | --- |
| Windows 8个目录 | Rust debug增量、独立codegen增量、Flutter test_cache、三处Python字节码、build_runner/build_resolvers缓存 |
| Linux 6个目录 | Rust debug增量、Flutter test_cache、三处Python字节码、已停用Android项目的.gradle缓存 |

共120,802个文件，123,106,819,665逻辑字节，约114.652GiB。Windows记录的D盘可用空间从32.182GiB增至85.573GiB；Linux文件系统内部可用空间增加49.747GiB。未压缩WSL VHDX，不能将两者或逻辑删除量相加宣称宿主物理回收量。

有界审查 `test/`、`tool/` 的64个Dart文件与581条引用，未发现失效的本地导入；共享stub、live smoke、native/工具守卫均有用途或入口。含“Android”的断言多数是在验证桌面界面不再出现旧控件，不能因此删除。此轮没有可确信废弃的源码/测试删除项，也不声称已经完成全仓死代码审计。

清理后两树来源摘要仍一致，23项既有原生检查重新运行通过，明确缓存目录均不存在；正式Linux bundle全部载荷、90批四包及字体/OFL摘要保持。应用全量Flutter/Rust/工具套件没有因缓存删除而重编重跑。

下一步应在完整隔离源码环境中建立可用引擎构建，将修复接入真实窗口，再跑原规则拖动门禁。Windows三来源/原始安装及详情、真实校园、可恢复guest Full、长期/其它桌面/84批故障、远端CI与最终逐项审计继续保留，整体goal active。
