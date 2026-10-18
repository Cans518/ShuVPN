# Rust 核心与桌面迁移实施记录

2026-10-19 提交整理：整理现有桌面迁移快照，补齐Git忽略与生成/上游文件属性。Rust格式、109文件Dart边界、Flutter静态分析及Windows344项Flutter测试通过；工具191项中171通过、19跳过、1项仍缺引擎source.json。提交仅保存当前工作，不代表默认引擎、最终平台矩阵或正式发布验收完成。详情见[项目状态](project_status.md)。

2026-10-18 整理补充：此前有限清理已复核，并清除两端再生的16个pyc及本轮5个空测试临时目录。当前工具测试191项中171通过、19跳过、1项因缺少引擎source.json而错误；后续engine-tree生命周期诊断另有ASan释放后访问，不能被93批GUI成功覆盖。简明进展、保留范围与本轮验证见[项目状态](project_status.md)。本轮不改变迁移未完成状态。

2026-10-18 第九十三批（部分完成）：独立构建固定SDK私有Linux引擎及正式main，本地ATK子树消除self-call并修正几何；20几何/34生命周期/8跨进程/54相关SDK项通过。实际GUI前两轮已拖动但fixture缺Rust默认字段，完整JSON超时；仅补三个空值，回归red/green、两端183工具（19/7跳过）及109边界通过。最终三来源六次物理拖动/放弃恢复/应用到版本6/新PID完整原文及首匹配通过，八GUI进程全0。隔离应用495/f4a8、开发驱动495/c451f0e8，原90产品/四包与字体保持；默认SDK/CI/发布尚未接补丁，完整provider适用性未签收，保留children重排/移除parent静态疑点。前批有限清理保持；本批再生16个pyc共341027字节，Windows删除被自动审批拒绝，两端小缓存保留，其余12目录不存在。工作产物/失败证据保留，无在途/提交发布。总汇engine-geometry/verification.json及 [完整范围](engine_source_build_acceptance.md)，goal active，原其它未完成范围保持。

2026-10-18 第九十二批（部分完成）：原始SDK C++实际ATK复现父rect重复加/缩放忽略/PARENT累加，20断言原版18失败；实验补丁20通过。独立通知检查先red确认新child零尺寸回归，再提前几何/全部parent提交恢复。私有只读信号runner捕获90正式AOT真实输入，capture01关闭超时保留，capture02显式退出前4进程0、第5拖动失败；两只错位86/140的实际AT-SPI矩形经16节点原始C++回放逐项一致，补丁原生结果与截图对应，但未进入真实DSO，不签正式拖动。来源仍495/f4a8、产品90/0ecc与四包，正式载荷/字体保持；清理后23原生断言复验过，未重跑应用全套。用户追加有限清理已删除两树14个缓存目录/120802文件/114.652GiB逻辑量，D可用32.182→85.573GiB；WSL内部+49.747GiB，无VHDX压缩。源码/测试有界审查无确认废弃项，删除0。单体引擎不能adapter-only公开ABI替换，depot_tools成功、私有C源码clone300秒超时终止，SDK保持；外部半成品删除自动审批拒绝，目录保留、无在途，未依赖同步/引擎构建。总汇accessibility-geometry/verification.json及 [详细范围](semantics_geometry_acceptance.md)，goal active；默认GTK真实修复和原全部未完成范围保持，无提交发布。

2026-10-18 第九十一批（部分完成）：同连接ATK最小复现证实系统/新原版都self-GetExtents阻塞；私有v1协议20项10失败，v2经规范路径/分别分派修正后同连接20项过、跨进程两版各7项过、上游两版droute/atk测试各两组过。库只进本轮私有prefix/LD会话，无系统安装或进入分发。真实90main在private01/02/03各4进程0后第5失败，前置排序未变；第三轮点击前后证据说明SHOWING+VISIBLE+48方形仍可坐标错位，几何超时越过不等于物理拖动通过。driver/test两文件增强严格VISIBLE/方形/有限32步搜索及probe/截图，先red后green；最终495/f4a84895…两树工具181（19/7跳过）、针对12及边界109过，中间8859/180项保留。产品仍90批0ecc及旧四包，无新应用构建/完整Flutter/Rust重跑。Windows原包私有副本两进程0，ScrollItem10017实测空，未调用滚动/磁盘宿主保持；源码无IA2不冒充runtime验证。SDK输入两端独立内容hash1cf1…匹配artifact元数据，新增内容映射仍非二进制认证。总汇accessibility-runtime/verification.json和 [详细范围](accessibility_runtime_acceptance.md)保持partial/goal active；SDK坐标与库生产适用性、旧所有未完成范围继续，无提交发布。

2026-10-18 第九十批（部分完成）：Linux runner用公开API补socket parent几何，初版强引用环经独立复核及销毁弱通知测试实际134复现，无parent控制0；最终destroy提前断环，四项native通过并接本地/CI。495文件两树0ecc5605…，两端工具178（19/7跳过）、边界109、shell语法及main release通过，Dart/Rust/绑定/依赖/字体未改，全量Flutter/Rust未重跑。新Linux main三普通离线GUI0/设置与覆写重开/宿主网络相同；新GetExtents诊断仍同进程self-D-Bus超时，PID492231定向终止-15、无拖动。Windows89产品run-01激活失败，前台实际改变后run-02基线三进程0但高级字段缺失；run-03四进程全0/四组配对证据，点击前新增按钮已不在画面，provider/point仍同名可见，字段不出现且草稿/磁盘保持，仅diagnosis_passed而passed=false。两端各2项framework原生Rust几何诊断通过，AX源算法投影与runtime内树读取明确区分，未归因冒充修复。源码出处校正：a804是artifact标识，SDK源Git d3b14c…六文件逐字节核对，未证明二者编译映射。新四包rule-geometry/package逐项含字体/OFL/core/helper及来源，Inno仅输入链、新安装器未安装、旧候选保留。总汇rule-geometry/verification.json及 [详细范围](native_accessibility_geometry_acceptance.md)仍partial/goal active；校园、Windows原始安装/详情/三来源、Linux拖动、guest Full、长期/其它桌面、原84故障、远端CI和最终审计保持，无提交发布。

2026-10-18 第八十九批（部分完成）：修复实心底栏extendBody覆盖正文56逻辑像素，列表尾距同步84→28、root通知页补系统SafeArea；DPR1/2与34像素底部inset先red后green，公告追加后滚到新末尾再查footer。四处纯view/边界109，字体/Rust/绑定/依赖不变。最终来源493/615fa36a…两树一致，两端完整Flutter344/343、工具178（19/7跳过）、analyze/边界及正式main release通过。新四包windows-rule-resume/package逐项含原TTF/OFL/FontManifest/core/helper，Inno仅外层/输入链，两安装器未安装，旧候选保留。Linux新main离线GUI三普通进程0/版本2与覆写/重开/宿主网络相同；Windows新包私有资源副本三进程基线0，底栏point拒绝解除，但第四进程新增前置按钮point/名称/实际指针一致仍无字段，passed=false/CLI1，失败时UIA与截图的滚动可见性不一致尚未归因。后续run-07同步诊断在首进程任务栏指针请求/实际不符被守卫拒绝并强制清理，外部Chrome非安全弹窗，没有取得配对截图。所有早期run01–05及测试失败保留。普通Linux私有总线另确认旧88产品GetExtents同sender/destination=:1.0、同appPID480816，仍dbind超时、无拖动、定向终止-15，未修复。总汇windows-rule-resume/verification.json及 [详细范围](desktop_geometry_acceptance.md)仍partial/goal active；Windows三来源/原始安装与详情、Linux物理拖动、校园、guest Full、长期/其它桌面、原84故障、远端CI/最终审计全部保留，无提交发布。

2026-10-18 第八十八批（部分完成）：新增真实鼠标拖动widget/FFI三来源取消/草稿隔离/预览失效/应用v2与释放重开完整顺序，另上/下/第二页绝对索引/忙碌指针；两端针对16通过。生产一处纯view补48×48手柄独立具名image语义，旧red保留，排序业务/Rust/绑定/依赖/字体不改。新物理X11driver/9守卫接入本地/CI/DEB，但portable-01/02各4正常退出后第5进程在拖动前失败，**当前新GUI门禁会失败且物理拖动未签收**；Tap+Focus计数问题修正后Component.GetExtents/dbind超时。geometry-04正式main PID480238主线程栈确认ATK桥同步D-Bus阻塞及Flutter父几何链，查询后正常退出也超时；固定SDK及实际2.38桥原始源码支持同进程plug/socket等待机制，目的地仍未单独抓取、未修复。连续三坐标超时不是独立冷启动结果，GetSize/Position因节点缺失未直接执行；失败/字段更正/初诊WSL日志覆盖保留。GDB仅本批build下载解包、仅新子进程授权具体debuggerPID，无全局安装/ptrace修改，最终GUI/GDB映像清空。正式两端main构建、完整Flutter341/340、analyze/边界109、最终工具两端172（19/7跳过）通过。应用四包493/e610553f…，后续工具493/65ed60b0…分别两树一致；rule-drag/package ZIP/tar/DEB逐项及字体/OFL/core/helper保持，Inno仅输入链，本批新安装器未安装，旧候选保持。Windows末次前台已变DSH，旧安全弹窗HWND不存在，只读无输入，87批旧阻挡不能沿用；下一轮重新启动前守卫推进Windows实窗，同时继续GTK几何修复。总汇rule-drag/verification.json及 [详细范围](rule_drag_acceptance.md)仍partial，goal active；全部校园/Windows安装详情/guest Full/长期/其它桌面/原84故障/远端CI/最终审计继续，无在途/提交发布。

2026-10-18 第八十七批（部分完成）：Linux连接详情实际窗口核对成功SHU/DIRECT、规则REJECT及资源表外SHU拒绝四类原生结果和32字段，记录页保持打开时托盘断开清空/禁刷新，三轮12原有代理流、DIRECT独立65536字节回显、两HTTP403及两个受控拒绝目标无连接通过。生产三处纯view补具名历史/刷新/返回与字段语义，清单109，字体/业务/Rust/绑定/依赖不变。gui-01第三轮夹具共享视图释放竞争失败保留，新增开发观测模块/可控释放red→green，所有FRB读取首次await前发出并冻结轮次端口；gui-02普通UID1000/CapEff0单进程退出0，网关原pidfd/owner/listener/TUN/egress清理与私有/宿主网络相同，无本轮GUI/fixture/面板/宿主遗留。正式应用/四包488/ecc18a75…，修复后夹具/工具490/0881aa9a…分别两树一致；最终Flutter332/331、工具两端163（19/7跳过）、analyze/边界通过，正式两端release及Linux夹具/恢复main通过，恢复字节等于正式tar。新四包connection-details/package逐项含字体/OFL/core/helper核对，DEB分发元数据单列，Inno仅外层/输入来源链，本批新安装器未安装；旧86/83/77保持。一次Windows工具UDP10013失败原端口未知/原因未确认，单项附参3过和未改源全套163过保留各自证据。Windows仍安全中心PickerHost前台，无输入/启动GUI。总汇connection-details/verification.json与 [详细范围](connection_details_acceptance.md)仍overall partial；M4.4实际拖动、Windows/校园/guest Full/长期/其它桌面/原84故障/远端CI/最终审计继续，goal active，无在途/提交发布。

2026-10-18 第八十六批（部分完成）：新增Linux规则排序/停用/被引用Office组/当前配置tester/回退重开常规GUI验收，保留原CRUD，接入本地/CI/DEB与限定artifact路径。实际red定位下拉/菜单无enabled和只读结果无label，生产一处Dart view补语义，业务/Rust/绑定/字体不变；路径、外层清理假成功、旧结果与错误复制值守卫共13项。两端Flutter326/325、analyze/边界109、正式main release通过；最终两端工具158（19/7跳过）通过。应用四包488/440d0348…，后续工具488/49ba360e…分别两树一致。新四包rule-management/package-final逐文件及字体/OFL/core/helper/来源核对，Inno仅生成；中间47ec与旧83/77候选不覆盖。便携portable-07与实际DEB管理各9普通进程退出0、11新HTTP/TCP决策/22私有字段复制/6独立native读回，版本3/4/5/6/7/7，回退v7内容等于v5并重开；安装另CRUD9进程/最终8，生命周期、权限、账户及非包数据保留、宿主账户/包数据库比较通过。部分GTKText仍返回标签/空值，原始失败与读数保留，复制在独占Xvfb内严格匹配，不称元数据全面修复；SHOWING不是物理裁切，截图已抽看。Windows仍安全中心PickerHost前台，无输入/正式app启动；Linux固定安装与本次进程清空，所有句柄终态，无在途。总汇rule-management/verification.json及 [详细范围](rule_management_acceptance.md)仍overall partial，M4.4拖动/详情、Windows/校园/guest Full/长期/其它桌面/原84故障/远端CI/最终审计继续，goal active，无提交发布。

2026-10-18 第八十五批（部分完成）：新增Windows三来源规则常规驱动/共享fixture读回检查，保留正式main资源副本、基线3/扩展9原进程守卫；修复原错误被二次清理覆盖、清理前PASSED标记问题，新增系统弹窗启动前拒绝。486文件两树cd7cde4b…，工具两端145（19/7跳过）及边界109通过，10新守卫。Windows前两次停在激活并强杀清理，规则未开始；当前前台确认为Windows安全中心PickerHost的允许/取消弹窗，第三次在启动前正确拒绝且正常清理，未代选系统授权，Windows真实三来源仍待。Linux同83产品九普通进程退出0、三来源版本2→8及6独立读回通过，四实际输入文件与最终一致但全树运行来源为03fa4fc2…，不冒充最终树或新安装GUI。六KnownFolder根/mutex/本轮进程清空，宿主两目录/代理保持，83四包/生产代码未变，无在途/新包/提交发布。总汇windows-rule-dialogs/verification.json及 [详细范围](windows_rule_dialog_acceptance.md)，需用户处理系统弹窗后继续Windows，整体goal active，原84故障及其余迁移范围保持。

2026-10-18 第八十四批（部分通过）：工具/测试网关增加默认关闭的有界心跳观测、逐连接编号/64尾事件/累计数及严格日志门禁，分离原50毫秒与独立5秒时序，主错误/清理回归先红后绿。483文件两树b4a9d1f1…，两端135工具（19/7跳过）、相关18 Rust/选定Clippy/rustfmt/边界通过。Linux50毫秒600/100/10新复现整体198.256秒EOF，owner明确心跳判失活，11条trace末连接3824/3824、最大回复gap0.584616457秒；直接原因有证据，延迟来源和旧449秒仍未归因。仅自有fixture pidfd暂停0.400909秒验证诊断，不用它反推旧失败。Windows1预热handles+33预算失败保留，同32上限的正式10预热另测通过。两端5秒600/100/10各111session/444流通过，Win626.843秒/303628288字节、Linux650.564秒/303497216字节，原进程0退出、预算/trace/清理完整。完整Rust Windows365通过；Linux宿主3 HTTP测试超时且单组仍失败，最小socket也证实绑定lo超时，私有loopback netns同UID1000同源码/二进制socket/相关5项/完整349项通过，环境边界保留。宿主/进程清理一致，83四包/生产代码不变，无新包/提交发布。总汇heartbeat-diagnostics/verification.json仍partial及 [详细范围](heartbeat_diagnostics_acceptance.md)，无在途；延迟来源/WSL绑定机制、校园/Windows窗口与guest Full/长期/其它桌面/远端CI/最终审计继续，goal active。

2026-10-18 第八十三批：修复规则图标/开关/新增按钮名称，显式来源和行号语义保留原生动作/焦点/禁用，无业务/Rust/绑定/依赖改动。三来源GUI试跑收敛进常规工具、本地/CI/DEB入口。482文件两树f8464b55…，两端完整Flutter324/323、工具132（19/7跳过）、analyze、边界109及正式main release通过。新ZIP/Inno/tar/DEB位于rule-accessibility/package；原字体/core/helper摘要及77四包保持，独立清单/来源校验通过，新Inno未实际安装。Linux便携与实际新DEB安装各9普通进程退出0，三来源取消/增改删/预览/应用/编辑重开及独立native6次读回，版本2→8、各41具名动作；安装生命周期/账户保留/宿主状态和清理通过，非TUN/Full/托盘重跑。Windows新包私有资源副本离线回归三进程0退出，宿主/清理一致；不签原始/安装GUI及Windows三来源弹窗。总汇rule-accessibility/verification.json及 [控件与窗口说明](rule_accessibility_acceptance.md)。82批原EOF未归因，整体迁移/校园/guest特权/长期/其它桌面/远端CI/最终审计继续，goal active，无提交发布。

2026-10-18 第八十二批（部分通过）：同一原生owner四流600秒+100轮重连及OS占用观测落地；Windows111session/444流、持久305201152字节、615.375秒、原进程退出0及固定预算/监听屏障通过，Linux449.938秒提前EOF失败、只完成10轮预热，原因未归因，原始总汇verification-baseline.json保持passed=false/partial=true。479文件旧来源224734f6…；两端124工具/analyze/边界108通过。Linux另补正式字体GUI prepend增改删/取消/显式应用/重开/独立匹配，五普通进程退出0，图标名空/动作索引及视口失败边界明确。随后开发诊断来源0c5618da…两端125工具/analyze/边界通过，增加即时失败快照和每流进度；受控暂停自有fixture验证可抓心跳失败，但不反推449秒根因。新诊断long-linux-02同600/100/10/50ms配置复验已完成并消费退出0，整体642.822秒、111session/444流、持久310509568字节、原probe/fixture退出0和预算/监听屏障通过，宿主link/路由相同。当前总汇verification.json仍partial，原失败不因复验绿色消除，无在途长跑。说明 [长跑](native_owner_soak_acceptance.md)与[窗口弹窗](linux_rule_dialog_gui_acceptance.md)。生产/字体四包未变、无生产构建/提交发布；真实校园/特权/更长运行及原计划剩余范围保持，goal active。

2026-10-18 第八十一批：补M4.4真实Rust/widget弹窗闭环，新4项分别覆盖前置/profile/后置的增改删取消/保存/来源、字段格式错误/未知类型prepare拒绝、预览/应用与实际磁盘/原生意图、删除重开；第四项覆盖重命名重开和页面回退/文件恢复。两端与旧11合计15项、analyze、边界108通过。仅新增一测试文件，475文件两树来源7df2976a…；无生产、字体、绑定、四包改动或新构建提交发布。首轮校验时机假设失败及中间analyze info失败保留，最终补强独立复核指出的来源/取消状态/回退持久文件/失败清理。总汇rule-dialogs/verification.json及 [M4.4增量对应](routing_editor_acceptance.md)，不称实际桌面窗口或网络出站，不冒称全套Flutter/Rust/117工具重跑。正式窗口剩余矩阵、持久owner长期占用/真实校园、Windows安装GUI/guest特权Full及最终审计继续，goal active。

2026-10-18 第八十批：Linux便携及实际字体DEB安装各三普通GUI进程退出0，非法规则原生错误显示，Apply disabled/动作0，0.539/0.535秒观察后原记录字节、独立native读回/新PID编辑器值保持版本2及非空前后置覆写。无物理Apply点击；视口外编辑器值由AT-SPI读取。仅驱动/对应测试两文件改变，两端工具117（19/7跳过）、边界108通过，工具474文件fdfa7855…两树一致，产品9a9ebbf5…及77四包未变。run-01几何超时/run-02错误role失败保留；辅助安装载荷观察误用便携README清单失败另存，归档32/33项核对与本次安装GUI三项摘要区分。DEB安装权限/升级/卸载/原生账户保留、宿主账户/包数据库相同及进程/安装清理通过，未重跑TUN/Full/托盘或Rust/Flutter/analyze、未构建提交发布。总汇linux-invalid-rules/verification.json及 [详细范围](linux_invalid_rule_gui_acceptance.md)，原计划其余校园/Windows安装GUI/guest特权/长期/最终审计范围保持，goal active。

2026-10-18 第七十九批：仅开发驱动/对应测试两文件新增Windows实际无效规则GUI。最终run-03三进程退出0；原生unsupported错误、物理点击Apply的0.812秒无提交效果、原文件字节/SHA及独立native/重开raw值保持版本2与完整覆写通过。UIA仍enabled=true，不称标志修复；无收据守卫保持严格，额外收据必须绑定真实点击并匹配SHA/有限观察时长，不能当离线认证。run-01类型假设/run-02缺禁用状态与收据、Linux首轮缺来源环境失败保留。两端工具114（19/7跳过）、定向17、边界108通过；工具474来源916160ce…两树相同，产品仍77批9a9ebbf5…，无新构建/候选/提交发布。6个私有KnownFolder根/mutex/应用fixture清理，宿主两目录/两代理键前后相同。总汇rules-invalid-gui/verification.json及 [详细范围](invalid_rule_gui_acceptance.md)，下一步Linux同类路径与原计划其余项目，goal active。

2026-10-18 第七十八批：77批新字体Inno实际安装/合成旧版本升级/卸载、39载荷SHA循环、运行/事务/runner1618、四套已安装DLL和账户/非包文件保留通过。Windows不是正式安装GUI签收。新DEB实际权限/安装事务、3轮校园TUN、默认库/固定helperFull十阶段全数据及覆盖变化/强杀重建/路由恢复、安装设置GUI与托盘各三个普通进程退出0通过，截图已看。源仍474文件9a9ebbf5…，原77四包未变；无代码/新构建/提交发布。宿主/注册/mutex/进程清理核对通过；LinuxGUI PID为私有namespace，不能当hostPID。完整Rust/Flutter/analyze沿用77。总汇wenkai-install/verification.json及 [安装验收说明](wenkai_install_acceptance.md)，整体goal active。

2026-10-18 第七十七批：按用户指定接入霞鹜文楷屏幕阅读版v1.522，未修改TTF及原始OFL随包。全局明暗/独立标题弹窗/日志使用同一family，字号行高字重请求保持；生产三处展示文件及pubspec资源改变，无新增运行依赖或锁变化。两端完整Flutter317/316（外壳实际加载TTF）、工具111（19/7平台跳过）、analyze、边界108、正式main release及各三个普通GUI进程离线设置回归通过，截图已查看。474文件两树SHA `9a9ebbf5575f7a63f174437a04a8a8340a6654439ad1b72404ff49fd1af05c90`。

新的ZIP/Inno/tar/DEB位于wenkai-font/package，原72候选未覆盖。ZIP39/tar32/DEB opt目录33项清单和FontManifest/字体/OFL经只读独立复核；Inno已生成、来源链及摘要核对，但本批未重跑新安装器实际安装。Windows默认target反斜杠在准备前被拒绝，显式lib/main.dart重建与包产品字节相同后GUI通过，失败保留。两宿主目录/两代理键本批前后相同，两KnownFolder根/mutex/应用fixture清空；完整Rust/网络/安装矩阵、远端CI未重复，无提交发布。字体要求完成，整体迁移goal active。总汇wenkai-font/verification.json及 [字体接入说明](wenkai_font_acceptance.md)，其余校园/特权Full/长期/最终审计范围保持。

2026-10-18 第七十六批恢复：用户恢复推进，正式main私有资源副本 run-03/04 各三进程退出0，完成70000拒绝/18443保存、Full重开/Campus恢复及仅校园预览不落盘/应用版本1→2/非空前后置规则覆写原文与两个DIRECT决策保留，公网规则测试REJECT。新增指针失败诊断及边界检查、类型区分同名悬停提示、进入规则页前的基线和异常撤销passed回归。两端工具110（19/7跳过）、analyze、边界108通过；Linux正式bundle离线三进程及Windows旧74开发AOT已连接三轮12流回归通过，首次overflow超时保留。

471文件两树SHA `7afadab042e990b71fbcef2c79470404735b8c47ea34b88e5c917a303bc30011`；原产品/参考构建/候选保持，无新包/提交发布。恢复时段两宿主目录与两代理键前后相同、8个测试KnownFolder根/mutex/应用fixture无遗留；最初暂停前后的代理差异仍未归因，不称整批代理不变。总汇windows-settings-gui/verification.json及 [恢复验收说明](windows_settings_gui_acceptance.md)。目标active，完整迁移剩余范围保持。用户随后要求改用霞鹜文楷屏幕阅读版，字体变更的产物及证据另记，不能沿用本批旧AOT签收。

2026-10-18 第七十六批暂停：用户暂停持续推进，当前goal paused；本轮仅完成Windows正式main私有资源副本离线端口/捕获模式/规则GUI驱动扩展，未完成验收。4个工具/测试文件保留，定向11项通过，Dart格式化无改动；首次真实run-01在欢迎流程的任务栏激活阶段因物理指针目标/读回不符退出1，passed=false，新增设置路径均未执行成功。运行来源471文件f7d88a492ac77e8c22d8cf670f48119ed62cace3c1cfd1306e843c2490606c01；Linux镜像仍75批，未同步、未全量104项/analyze或新构建。

暂停后CLI已结束，未见应用/fixture进程，私有KnownFolder两根及互斥锁不存在。原有应用目录摘要相同，但两处HKCU代理键的三个值摘要变化，来源未归因，不能宣称宿主代理保持，未擅自回写设置。证据windows-settings-gui/run-01/verification.json、host-before/host-after-pause.json及pause-state.json。最后已通过里程碑仍75批；无新包/提交发布。交接顶段及 [76批暂停记录](windows_settings_gui_paused.md) 已记录失败、待补overrides校验和明确恢复后的步骤；历史active不覆盖当前暂停。

2026-10-18 第七十五批：Windows正式main AOT私有版本资源副本最终两轮各3进程退出0，完成真实首启表单、受控局部密码掩码/清空、仅离线预置welcome后GUI深色保存/独立native读回/重开恢复。EXE仅2个资源字符串改12字节、其它字节不变，生产AOT351535b4…与36项载荷保持72批原包；未替换bootstrap或账户等backend。保护标志false、主题selected未签收，渲染仅单点暗色+截图，未提交登录。实际KnownFolder新目录、任务栏完整EXE Appid/真实输入、原句柄与退出、默认库及无自身监听核对。

工具471来源264c8d9ebfa008f9a3e5b5ab1c8cb0ca17ede6b05eff8a4134a7032f2dd1b2bf两树一致，原包仍462来源7abf6237…；两端工具102（19/7跳过）、边界108、旧74已连接三轮回归通过。两应用宿主目录/两代理键前后摘要相同、实际测试目录逐项清空；预检失败保留预留目录的边界明确。总汇windows-formal-gui/verification.json及说明windows_formal_gui_acceptance.md。无生产源码/参考build或候选覆盖，无新包/提交发布，完整Rust/Flutter/安装与远端CI未冒充重跑，goal active。

继续真实账户与校园、原始EXE默认profile/Windows安装GUI、完整离线设置、独立guest特权Full、长期/其它Linux桌面与最终审计；正式资源副本离线运行不扩大为全部迁移完成。

2026-10-18 第七十四批：Windows生产界面/ConnectionController/持久Rust owner的私有存储已连接GUI最终两轮通过，各3轮12条HTTP/SOCKS×IPv4/IPv6目标64KiB/prelude/FIN，统计262144/262172及决策匹配。按钮连接断开、原生托盘重连断开、隐藏后流量/恢复、第三次连接后Up/Enter退出均走真实输入；普通断开session/owner清理可复用，终态owner关闭、原GUI/fixture句柄退出0、监听与托盘清空。timeout依赖OS表额外见证，未称拒连，累计流量未称UI实时速率通过。

开发入口Windows分支使用新profile的真实原生账户/设置/规则，绕开KnownFolder与旧偏好迁移；账户仍当前用户DPAPI、非用户/网络沙箱。只读宿主两目录与两代理键前后摘要一致。MSAA请求启用后UIA真实按钮可读，ClickablePoint/RuntimeID限制明确，未改引擎或生产代码。Linux共用路径私有namespace/XFCE三轮回归和旧Windows窗口探针回归通过。两端工具93（19/7跳过）、analyze、Dart边界108通过；最后拆分Windows范围字段后来源为468文件eeed2e27796d99139f3a0283b7202d169d6c895feac3264396d4f13c52f5c9a0，两树相同。Windows最终run-11/12，Linux最终来源重建关键产品与本轮GUI受测字节一致，正式main构建均恢复。

总汇windows-connected-gui/verification.json，说明windows_connected_gui_acceptance.md；早期驱动失败保留，无新包/提交发布，未重跑完整Rust/Flutter/安装矩阵或远端CI，goal active。fixture账户/connectPlan仍不签正式bootstrap/connectAccount/校园/TUN/系统代理/Windows Full；继续独立guest特权组合、真实校园/长期、其它Linux桌面和最终审计。

2026-10-18 第七十三批：Windows Explorer真实托盘开发入口四轮最终验收通过（run-21/22当前开发bundle，run-23/24为72批原生文件仅覆盖开发AOT）。物理隐藏/隐藏后代理流量/左击恢复/右击原生菜单与协议子菜单/显示/Up及Enter退出，实际命令show/show/quit；原GUI和fixture句柄退出0、图标移除、IPv4/IPv6目标监听清空。timeout不独立作为清理证据；End失败未冒充修复。

开发工具/入口及测试改动，生产原生焦点实验全部撤销。两端工具87（19/7跳过）、analyze、边界108、宿主生命周期15及原生菜单4通过；正式入口构建恢复且自动开发模式正常退出。466文件来源d33a2b4dc29fd2b835b6bf1478d389e0ebeabaacd76966bbd127f67948027f1a，两树匹配；重链接产品不冒充与72批字节相同。总汇windows-tray-input/verification.json、说明windows_explorer_tray_acceptance.md。未新建安装候选、未全量Rust/Flutter或远端CI，旧失败保留，无提交发布、goal active。

真实Rust代理与桌面生命周期通过，但仍是无账户开发入口，不包含正式main/ConnectionController/持久owner、校园、TUN/系统代理或Windows Full。继续私有存储生产界面已连接GUI、独立guest特权Full、长期/其他Linux桌面及最终审计。

2026-10-18 第七十二批：71批断开/重连修复已进入新的成对ZIP/Inno/tar/DEB安装候选，目录connection-candidate/package，仍0.4.2+9及partial-desktop-candidate。正式AOT/core/helper/tray逐项等于71批受测产品；462文件来源7abf62372522eddd35b7279eddb82ea8b44604181422d616497eadd7d8b78f33两树重算一致，安装器input/payload/builder/产物SHA及旁路一致。Linux未混入fixture AOT，旧68/67候选不覆盖。

Windows实际安装/升级/卸载、37文件载荷、运行/事务/runner1618保护、4套安装DLL存储探针和原生账户/非包数据保留通过，安装器仍NotSigned。新DEB隔离安装生命周期、3轮校园TUN、普通UID/CapEff0默认库+固定helper的10阶段Full数据/拒绝/覆盖变化/helper崩溃重建/恢复通过；已安装设置与托盘6个GUI进程均0退出，实际库映射/SHA匹配。宿主账户/包数据库摘要保持，最终注册/固定路径进程与安装状态清空；总汇connection-candidate/verification.json。

本批仅重新编译两项Linux安装探针，未改生产代码/绑定，未重跑完整Rust/Flutter矩阵；明确复用71批317/316 Flutter等证据并校验产物一致。独立复核验证四种包与来源，主线程实际安装补足Inno载荷证据。贡献指南移除已删除Android工具命令，修正helper正式接线说明，LLM平台示例与仅Windows/Linux一致。未提交发布，goal active。

Windows真实托盘入口与消息/菜单链已定位，尚未执行Explorer交互；支持目录使用KnownFolder API，不能只凭环境变量假设正式账户已隔离。下一步先补无账户开发窗口探针的外部驱动，再推进Windows已连接GUI；独立可恢复guest特权Full、真实校园/长期、其他桌面/Wayland、远端CI和最终逐项审计仍未完成。

2026-10-18 第七十一批：真实Linux本地协议已连接GUI三轮通过，修复托盘普通断开误用终止shutdown及命令future清除后漏通知busy=false两处生产UI问题；host组件与两条监听回归先red再green。生产3个Dart文件边界复核与SHA更新，后端生命周期仍归原生owner。

私有loopback net/X11/DBus/XDG、普通UID1000/CapEff0与真实XFCE宿主中，生产界面/控制器/默认Rust库的按钮连接断开、托盘连接断开、再次连接后实际popup退出均通过。每轮HTTP/SOCKS×IPv4/IPv6四流各64KiB/prelude/FIN，native决策与最终262144/262172统计匹配；普通断开owner可复用、退出owner关闭且TUN/egress/监听清空，原fixture pidfd及应用退出0。账户/plan入口为fixture，production bootstrap/connectAccount、真实校园/TUN/系统代理不计成功。独立复核促成统计完整结算、最终session和进程身份/SHA检查；最终gui-04，失败及收紧前gui-03均保留。

两端正式release、全量Flutter317/316、analyze、Python74（19/5平台skip）、Dart108通过；Windows新bundle FFI67、桥接、随包helper普通IPC4、原生菜单4通过。Linux正式入口恢复后首启/guest/宿主热恢复三进程均退出0；Linux25个本轮PID最终消失、Windows工作区应用/helper0。462文件两树来源7abf62372522eddd35b7279eddb82ea8b44604181422d616497eadd7d8b78f33，总汇connected-gui/verification.json，说明desktop_connected_gui_acceptance.md。CI/verify已接guard与独立fixture场景，未远端执行。

未重跑完整Rust/Full/安装矩阵，未新建安装候选；Windows68/Linux67旧包保留来源、未包含本批UI修复。无提交发布，goal active。继续Windows实际GUI/托盘、安装候选集成、独立可恢复guest特权Full、真实校园/长期、其他桌面/Wayland、远端CI及最终逐项审计。

2026-10-18 第七十批（承接10月17日）：Windows独立观察者与普通probe接线完成，OS管道对端PID/保留handle、创建时间/映像/SHA/SID/角色校验、严格阶段及原helper/probe退出见证均已实现。helper身份从tun-probe正式Client持有的handle采集，wire/普通FRB/业务策略不变；快照脚本编译固定进探针，按probe SID读HKU，最后等probe退出0后再核对网络，失败不升格pass。

两个真实普通进程的Current只读Plan已运行通过：observer81120/probe81344、启动方Process创建时间、三阶段、正式Full预览无TUN、probe与observer各退出0，46/18/14/23/14非空网络表前后相同。PATH缺powershell.exe及系统脚本策略两轮失败保留；系统路径/子进程参数修正、脚本编译固定后通过。普通token runtime observer会拒绝且不自行提升；不是实际提升观察者/UAC helper/All/Full数据验收。

457文件两树来源2ddaa84eec83e2cd8465216db2265baf0d1aa3918034dbdc73f86a7bc1170078；Windows example10、平台默认49/特性50及普通helper IPC4，Linux example3/平台35，两端工具66（19/5跳过）、特性Clippy及Dart边界108通过。观察者/监督器回归由现有verify/CI选择器覆盖，未远端执行。汇总windows-full-observer/verification.json及验收说明/交接已更新，未全量Rust/Flutter或完整包重建，无提交发布、goal active。剩余提升组合及正常Full数据、故障/重建、Flutter owner/安装默认库、GUI/校园/长期、其他桌面/远端CI与最终审计继续保留。

2026-10-17 第六十九批：Windows Full专用原生探针正式API预览已实际通过：真实fixture会话、显式Full、系统路由/地址读取、780分区与DNS接管、对端覆盖/本机保护、无TUN启动及会话退出。新增独立双栈TCP/UDP/DNS peer账本和两轮正常运行分支，后者仅编译、未特权运行；不再用旧campus --plan冒充Full正式计划。

监督器最初对拒绝查询后的空表假通过已明确作废、保留证据；查询错误/空表现在阻止启动。当前compartment只读快照46路由/18地址/14IP接口/23适配器/14DNS条目前后相同；all-compartment权限拒绝在无害占位程序启动前停止，不降级运行模式。独立复核修正UDP拒绝端口碰撞、坏ID需新独立DNS见证，重复策略更新使用实际版本；peer真实socket回归兼容Windows与Linux Python3.10。

453文件两树来源7247e1672020b3fcee2a916367c64098ff58cb689812d990472176af8686e737。两端新example契约4/3、针对性Clippy、工具63（Windows19/Linux2跳过）及Dart边界108通过，verify/CI已接线未远端运行。汇总windows-full-driver/verification.json、说明docs/windows_full_capture_acceptance.md；无完整应用/安装器、Rust/Flutter全量矩阵或生产Dart/API修改，无提交发布，旧配套包仍各自原来源。

Windows ordinary-helper的特权独立观察者/精确进程身份仍待接线；正常Full数据、覆盖变化、helper崩溃重建、DNS歧义/HTTPS、Flutter owner及安装默认库仍需独立可恢复Windows环境验证。未确认现成Windows guest；未触发UAC/Full或改变虚拟化配置。GUI/校园/长期/其他桌面/远端CI及最终逐项审计继续，goal active。

2026-10-17 第六十八批：真实Win32 HMENU复现Windows system_tray初始化checked覆盖enabled及两个setter互相覆盖，四组原生测试三组red，定点适配后全部green。顶层/子菜单、四种初态、反复切换与独立default位通过；生产只编译仓库内一份menu.cpp，上游SHA门控/原头文件/许可保留，不改共享缓存。测试及生产使用同源，verify/CI显式接线，未远端执行；真实Explorer tray popup与应用回调仍待验收。

448文件两树来源82ad3b92ec15542c393e55204b8a6b789a87a942edb8739881df0a842b214ae6。新Windows release、针对性Flutter82、两端工具58（Windows19平台skip）、ZIP/Inno校验和实际安装升级卸载/保护/账户保留通过。core/helper等于67批，tray DLL改变；未重跑Rust/全量Flutter/Linux release，Linux保持67批产物来源。新Windows包及汇总在windows-tray-state，Inno仍未签名，无提交发布；限界/下一步见交接及docs/windows_tray_menu_acceptance.md。Windows Full入口现仍campus定向且--plan提前返回，专用驱动/隔离运行、已连接GUI/校园/长期、其他桌面/远端CI及最终审计继续推进，goal active。

2026-10-17 第六十七批：原生再次选择当前协议会先取消GTK勾选，但Dart同值不通知/快照去重，已实测red并由UI刷新确认修复。在途标记不会被旧更新吞掉，拒绝选择仍恢复最新显示，普通等价快照仍去重。两条回归红/绿、Linux首启/Guest/宿主热恢复原生Event三处通过；Dart边界增量复核已记录。另补Full双栈父子重叠和物理DNS必需性纯契约，Windows/Linux平台过滤49/35项通过，不代替Windows Full运行。

444文件来源458203f79ef0c63231b50d373c94387f37ffff44e2ef29333218267e33fdfd12两树一致。两端release、Flutter314/313、analyze、Clippy lib/tests、工具57（Windows19平台skip）、GTK4与边界108通过；四种配套产物来源/载荷/SHA核验，Windows安装事务/账户保留及随包helper真实IPC4、Linux安装版六GUI进程/maps/退出和完整安装生命周期通过。core字节未变、helper摘要变化，故对新DEB另补固定安装路径默认库/helper的普通用户Full数据与故障矩阵，十阶段、拒绝边界与完整恢复通过。

总汇tray-protocol/verification.json，当前包在其package目录；Full记录及限界见交接和验收说明。没有新Rust全量/all-features、Windows特权Full、真实校园或远端CI证据；仍partial-desktop-candidate，未提交发布，goal active。后续保留Windows Full驱动/环境、Windows原生菜单状态位验证、已连接GUI与校园/长期、其他桌面及最终逐项审计。

2026-10-17 第六十六批：补上真实Linux托盘并修复system_tray2.0.3的GTK更新空实现及bool类型错误。仓库内定点编译单元适配由上游SHA门控，保留MIT许可、菜单对象和ID；程序勾选不触发用户回调，非法更新明确拒绝，未改共享pubcache或Dart/Rust后端。GTK4项原生回归及真实菜单红/绿证据通过。

真实XFCE host注册/父链、应用SNI PID和三个普通用户app的隐藏、原生菜单恢复/退出、Guest账户页/空表单、host丢失后的关闭退出、重启及同进程host热恢复通过；以所属XEmbed窗口右键打开实际popup，再键盘选择显示/退出，截图已检查。修正测试中的任务栏/托盘定位、tooltip干扰、AT-SPI动态空child，以及独立复核指出的身份/等待/清理问题，失败日志保留。

444文件两树来源SHA `2acc46e6a54034d49b0f0b8f3aa161d7b67dbac85df196364d28e09f49dbed4d`。Linux完整release/Flutter311/analyze、GTK4、两端工具57（Windows19平台skip）、Dart边界108通过。新tar/DEB来源、载荷、许可证及SHA核验，实际安装版两轮完整生命周期、离线设置/托盘六进程和校园TUN一轮清理、账户/非包文件/宿主状态保持通过。独立maps观察器初轮晚启动仅5PID失败；提前附着后的重跑完整核对6PID的exe/core/tray、摘要、UID/cap和pidfd存活，逐一对应GUI报告。core/helper未变，未重跑Rust/Full全矩阵。

证据tray-acceptance/verification-final.json，新Linux验证产物在同目录package；非新的成对发布候选，未提交发布。CI/verify接入native GTK及bundle/安装tray，未远端执行。详细范围、回退和Windows Full入口调查见交接及托盘验收说明。已连接GUI、Windows特权Full、真实校园/长期、其他桌面、远端CI及最终审计仍待推进，goal active。

2026-10-17 第六十五批续作：补齐已在工作树中的正式离线GUI验收并修复Linux关闭崩溃。DBus私有XDG目录移到启动前，新增回归红/绿；该问题修正后旧安装包仍关闭-6，确认与renderer释放竞态独立。runner现保留view子树至engine Shutdown完成，再由GTK idle释放，application hold保持关闭后的回调处理。未改SDK、Rust或退出断言。

新Linux完整release/tar/DEB来源438文件SHA `74dbae1ac5981e8ddfa3c91d98d0fcfb65d9a495b9525357b47bf4b5d31aa645`，两端源树一致。新安装版三次真实窗口操作与退出0，验证引导至表单、离线设置深色/捕获模式/端口校验、规则预览应用和重开后的GUI/native读回。另完成一轮校园TUN内核数据和退出屏障、完整安装生命周期、账户/非包文件保留及宿主状态不变。真实登录与联网GUI未执行，测试单键准备welcome不代表首启认证完成。

Linux Flutter311/analyze、两端工具51（Windows19平台skip）、Dart边界108通过；core/helper等于既有受测字节，未新跑Rust/Full全矩阵。CI新增安装版GUI和失败证据上传，未远端执行。证据gui-acceptance/verification-resume.json及shutdown-package新Linux验证产物，非新的成对发布候选；旧失败日志和implicit-view/messenger/OpenGL警告保留。详见GUI验收说明与交接。更多关闭/托盘/联网GUI、Windows特权Full、真实校园/长期、远端CI及最终逐项审计继续待办，未提交发布，目标active。

2026-10-17 第六十四批：建立生产Dart108文件（31生成/77手写）的角色/SHA/例外清单及持续门禁，同时封存11个运行直接依赖、pubspec/锁和生成输入。core36文件全文复核、UI30文件枚举/搜索与关键候选执行代码核验、主线程app/main/装配及原生落点复核未发现确证独立Dart后台；具体UI/迁移/平台例外和审查深度在`docs/dart_backend_boundary_review.md`明确，不单凭清单或搜索签收整个M5.4。

固定FRB2.11.1重生成32输出逐字节一致。新增检查器与15回归拦截漏文件、内容/依赖变化、条件后台导入、跨入测试代码和未说明平台IO；实际红/绿修复metadata提前结束及注释伪装JS条件两个缺口。两条CI/两端verify已接线，Flutter CI固定3.47.2/锁文件。Windows/Linux工具各45项通过（Windows17平台skip）、边界覆盖各108、analyze及配套d26c库的针对性运行各99项通过。未重跑未变更全量产品矩阵、未重建或发布包，证据dart-boundary/verification-current.json。

只读环境核对未发现现成Windows VM/Sandbox入口，本批未在开发主机启Full/UAC或更改虚拟化功能；Windows特权Full继续保留，不能由审计门禁替代。完整GUI/校园/长期/远端CI和最终逐项签收仍待完成，三个只读代理/全部句柄已结束，无生成物变化，goal active、未提交发布。

2026-10-17 第六十三批：第62批DEB的默认`/opt`安装库与固定`/usr/lib` helper已完成普通UID1000/CapEff0/NoNewPrivs0的Full验收，不使用临时helper locator或专用feature例程。库/helper前后SHA核对原始DEB及62批清单，maps、实际Flutter父进程、helper exe/inode/SHA/netns/capabilities及pidfd身份/退出均有实测。

原生owner自动与RustConnection手动Full完成校园UDP本机源转换、双栈签名TCP、DIRECT双栈TCP/UDP、64KiB及FIN、DNS A/AAAA/UDP/TCP/回退/错误回答、域名映射/歧义、HTTPS信任与拒绝。REJECT/授权不足/歧义无新增物理TCP接入；外来路由覆盖冲突保留外来路由，helper强杀后重建，正常停止时在关闭会话前确认控制绑定恢复。每轮检查owner/出口/监听/TUN/原helper与全部IPv4/IPv6路由表及策略规则恢复。

测试捕获owner自动收尾后的shutdown诊断可空、DAD完成后新增物理local路由等初轮假设错误，均按源码/无VPN对照修正，保留严格业务与全表断言。新root协议经复核增加固定十阶段完整性和目录fd安全写确认，5项边界回归通过。/opt SDK以私有overlay保留并核验缓存写入不触及底层，修复外层模拟清理EBUSY与findmnt隐藏挂载误判后，最终SDK模拟、完整DEB生命周期/授权拒绝/账户与宿主状态保持均退出0。

Flutter analyze、原root adapter Full/Campus回归、Bash/Python检查通过；工具Linux30/Windows30项通过，Windows17项平台skip。本批无生产lib/Rust变更、无新应用或安装归档，不冒充新跑62批未变化的全量产品矩阵。产品继续为d26c2827760b；新证据在`build/desktop/installed-full/verification-current.json`及候选`verification-installed-full.json`，详见Linux安装版Full验收说明。四名只读代理与全部任务结束，未提交发布；Windows特权Full、完整GUI/校园/长期/远端CI及M5.4签收仍待完成，goal active。

2026-10-17 第六十二批：捕获模式已贯通正式设置、自动连接、手动启动与指定物理接口的预览。默认校园分流保持，完整捕获标为实验性，连接/清理期间锁定修改。原生保留安全初始计划，运行中预览直接返回实际模式/路由/DNS与保护地址，不把自己的TUN路由重新规划；停止/清理中的计划不能展示。前端清除旧预览，连接抽屉按原生快照显示“模式 · 已启用”。保存失败、大字主题、三入口转发、实际模式优先与原生预览均有回归。

新Flutter真实适配器探针对归档默认so执行Full及campus的预览/启动/状态、校园UDP内核往返、停止与路由恢复通过；CI已接线，未远端运行。新归档默认helper在普通UID/CapEff0的受控临时locator下通过Full双栈数据、DNS/HTTPS、源地址审计、停止重启和helper崩溃新会话，另有低层helper/父进程崩溃和失败回滚证据。该场景核心为专用feature例程，不代表已安装固定路径默认so的普通用户Full验证。

最终Windows365/Linux348 Rust、Windows312/Linux311 Flutter全量、两端Clippy/analyze与完整release通过；解压库FFI67/66、桥接及三套跨进程存储、Windows新helper4项IPC通过。工具25项两端通过，Windows12项平台skip。全量7处UI失败分别修正状态标题重名和新文案断言，并新增实际模式回归；源码快照来源环境、探针runner启动lo/末尾CR、Clippy和格式初轮失败均有保留日志，没有跳过或弱化功能检查。

新配套Windows/Linux完整应用、ZIP/tar、Inno/DEB在`build/desktop/d26c2827760b/`，430来源文件两端SHA `d26c2827760b196bfc21a256bc7c35e1c0f69c7a3359868454c9c05d692b78f1`。所有归档文件、来源、安装器输入与payload/builder、SHA/size、core/helper与Flutter Cargo目标一致；DEB两份安装清单、每文件SHA及完整md5sums通过。详细产物SHA、日志和范围见候选verification.json及capture-ui/verification-current.json。

最终Windows EXE非管理员实际安装/升级/卸载与保护、runner1618、账户/非包文件保留和注册清理通过，仍NotSigned。最终DEB安装生命周期/失败重试/授权拒绝/账户及宿主状态保持通过，3轮已安装默认库/固定helper校园TUN和退出屏障通过；此3轮不是Full。已安装欢迎页、实际/opt核心加载和无托盘关闭0通过，截图已检查，既有OpenGL及释放链警告保留。回退先停止确认清理并切回campus，再使用60批成对包，不混用绑定和库。剩余已安装普通用户Full、Windows特权Full、完整GUI/校园/长期/远端CI及M5.4签收继续保留；全部本批任务/四名只读代理结束，未提交发布，goal active。

2026-10-17 第六十一批：按当前工作树推进原生完整捕获及路由恢复验证。Linux基线真实隔离内核复现local表单播路由绕过检查、缺失main规则仍接受计划；修复逐地址族要求默认local/main规则存在且不重复，local表目的前缀必须完全位于保留范围或本机地址集合。六种外来local/缺失规则场景最终均以明确原因在OS修改前拒绝，运行中新加local路由会停止、完成清理并保留外来路由。失败计划探针要求具体interface诊断，避免无关网络失败冒充正确拒绝；missing-local在网关已连通后注入。

正常完整捕获、live-local变化、仅IPv4网关和默认校园分流均通过真实内核测试，包含双栈DIRECT TCP/UDP、校园UDP转换、DNS UDP/TCP接管/物理上游及重试、域名策略/歧义拒绝、物理源地址审计、覆盖变化清理和新会话。此为root隔离namespace原生入口，不代表普通helper、正式GUI或Windows特权完整捕获。

生成绑定同步SettingsSnapshot新增捕获字段，修复编译失败；Dart展示适配、原生提交/非法模式拒绝/重开和切回campus均有测试。修复捕获代码两处Clippy问题，本地验证脚本纳入既有安装包测试。最终Windows364/Linux347 Rust、Windows305/Linux304 Flutter全量、两端Clippy/analyze和默认库/helper构建、桥接及三套跨进程存储通过；Linux打包14项含DEB通过。429来源文件两端SHA `99b5192d9c40a46d661b32978d98fd8adaecae789b7d7a50848e27e87d59a328`一致；库、helper、例程及红/绿日志摘要见`build/desktop/full-capture/verification-current.json`。

本批未生成完整应用或新安装归档，配套分发候选仍为60批ccb6beba4cba。模式尚未贯通正式Flutter预览/启动/手动命令，不能以已保存设置宣称正式GUI支持；默认保持校园分流。下一步完成正式接线、普通helper与Windows特权/配套产物，再继续原计划完整GUI、校园、长期、远端CI及最终逐项签收。回退先停止并确认清理，再使用60批成对完整包，绑定与库一并替换。所有本批句柄与三名只读代理结束，未提交发布，goal active。

2026-10-17 第六十批：进程规则与捕获代次修复已重建为配套Windows/Linux完整应用、ZIP/tar与Inno/DEB，候选`build/desktop/ccb6beba4cba/`，427文件来源SHA `ccb6beba4cbaff07b1588df5913bf5319b3c064ff1ef0062f11d309f4c82878d`。新归档库304/303 Flutter、两端analyze、桥接/三套跨进程存储、新helper IPC/内核场景通过，core/helper逐字节对应实际Cargo输出/bundle/归档；357/339 Rust和Clippy沿用59批同源码证据。

最终EXE普通权限安装/升级/卸载及保护、账户保留与注册项清理通过；仍无项目证书签名。最终DEB安装/失败配置重试/授权拒绝/升级/卸载/purge/重装与账户及宿主状态保留通过；100轮直接加载安装库和固定helper，每轮核对文件摘要并严格验证进程路径放行、名称拒绝、真实PID/路径/依据和完整退出清理。任务自有临时规则目录无残留。安装欢迎页/包内库/退出通过，既有关闭警告保留；完整GUI/校园/长期/Windows特权/远端CI等仍待验收。证据候选verification.json及process-packages目录，未提交发布，goal active。

2026-10-17 第五十九批：开发源码接入进程名称/映像路径规则与真实套接字快照，后端查询、预算、未知身份逻辑及匹配均在Rust；前端增加模拟输入、平台入站矩阵和有依据的连接记录。427文件两端来源SHA `ccb6beba4cbaff07b1588df5913bf5319b3c064ff1ef0062f11d309f4c82878d`，默认无进程规则不查系统，未知必需身份不落直连。双表映射、含分隔符字面量、IPv6-only误归属、排队及DNS捕获新代次均有回归；配置切换首DNS问题经真实内核与暂停时钟证明后修复。

最终357/339 Rust、两端Clippy/analyze、默认库/helper和304/303 Flutter通过。HTTP/SOCKS真实出站及授权边界、无身份TUN不落直连、Root和普通UID/CapEff0固定helper的实际内核TCP进程路径放行/名称拒绝/清理通过。普通helper测试为私有覆盖目录授权，不是新DEB验收。快照不能排除隐藏共享持有者、不是进程认证；专用DNS捕获不参与普通进程规则，跨地址族UDP通配不能确定时不归属。证据`build/desktop/process-rules/verification.json`及交接。暂无包含该能力的新完整分发包，旧1f1a2715c619不代表新功能；原目标继续active。

2026-10-17 第五十八批：已将第57批修复重建进Windows/Linux完整应用、ZIP/tar、Inno/DEB。新候选`build/desktop/1f1a2715c619/`，422文件来源SHA `1f1a2715c6190ea4c862ef09657c485088d75f9c952e601e3a40578ea1400164`；相对旧完整源码仅有已验7文件变更。新归档core/helper等于实际Cargo输出及bundle，来源和安装器输入/输出摘要一致，DEB内部SHA与dpkg md5sums通过。两端归档库Flutter300/299、analyze、桥接及三套跨进程存储通过；原生341/323沿用第57批相同变更源码的回归。

最终Windows EXE安装/升级/卸载、运行/事务保护、runner1618、实际安装DLL及账户保留通过；非管理员、临时注册项清理，仍无项目证书签名。最终DEB完整安装生命周期、失败配置重试/授权拒绝、账户保留及宿主状态恢复通过；100轮普通用户FFI直接使用新DEB安装库和固定helper，DNS/校园TCP/退出屏障全部通过。新默认helper内核场景、安装欢迎页/包内核心加载/无托盘退出通过；截图已检查，既有Flutter/OpenGL关闭警告保留。证据为候选`verification.json`及`build/desktop/pidfd-packages/`。没有新Windows特权、完整GUI/校园/长期或远端CI验收，没有提交发布；完整目标仍active。

2026-10-17 第五十七批：修复Linux helper退出状态回收竞争。旧候选真实FFI第30轮shutdown重现worker错误；仅加日志的独立基线随后捕获wait ECHILD与kill ESRCH，确定现有退出确认路径会把已被其他线程回收的子进程判为未确认。子进程pre_exec为自身打开pidfd并同步传回，父进程首次await前接管；actor/Hello0/协议2不变。退出以pidfd可读为前提，之后才允许ECHILD；强杀/析构也按稳定pidfd，等待各限3秒。Identity覆盖异步注册失败，启动帧重试EINTR；需Linux5.3+。session保留具体清理原因，探针记录phase和原生snapshot。

最终Windows341/Linux323 Rust、Linux Clippy/analyze及299 Flutter通过；三个pidfd生命周期回归和退出原因红/绿回归通过。正式默认特性helper的权限/内核包/动态路由/绑定恢复/失败回滚/崩溃场景通过。最终库搭配既有DEB安装的helper完成100轮普通用户真实FFI启动、DNS/校园TCP和退出清理，随后完整安装生命周期与宿主账户/dpkg状态恢复通过；它不是新DEB验收，也没有新GUI/校园/长期验收。证据与源码/库/日志SHA在`build/desktop/installers/helper-pidfd-verification.json`，具体范围和原始失败见接手文档。旧候选包尚未重建，FRB与数据schema不变，完整工程目标继续active。

2026-10-16 永久范围修订：**项目仅支持 Windows 与 Linux，不再开发或移植 Android。**
Android 工程、专用运行入口、原生适配与构建/发布工具已移除；下文 Android 内容只保留为历史记录，不属于待办或后续验收。
本次删除改变原生桥接接口，旧候选归档不代表当前源码，必须重新构建配套库、绑定与完整桌面包。

本次 Android 永久移除的 Windows 验证：静态分析、全目标全特性 Clippy、原生 Debug 库/examples 构建通过；独立源码/DLL 快照中 Flutter 441 项、Rust 集成 93 项通过。验证以快照为准；主线程同期继续增加设置端口验证断言，本次不把后来修改计入已验结果。Rust 全量单元轮次 199 通过/1 失败：主线程在删除期间新增的 account_connection_establishes_the_tunnel_and_preserves_origin_checks 报 preparation warnings were lost，未改动该用例或对应新接口。共享目录 Flutter 首轮因 NativeAssetsManifest.json 复制冲突未进入测试，独立快照复测通过。Linux 本轮未重建；未生成新分发归档。证据在 build/desktop/android-removal/verification.json。


本记录对应 [工程计划](desktop_migration_rust_core_plan.md)，用于记录实际产物与验证证据。计划中的五个阶段仍是完整目标；只有通过对应验证后才勾选完成。

2026-10-14 本次交付调整为 **Windows + Linux 桌面版**。Android 不属于本次交付或验收前置，第三十三批保留为未完成的历史工作。当前优先完成桌面正式功能与完整分发包，桌面验收条件见主计划开头；本次范围调整不表示下文未验收项目已经完成。

2026-10-12 中断后的现场、Windows TUN 在途改动及接手步骤见 [接手文档](desktop_migration_handoff.md)。第十八批续作已验证构建、绑定及普通权限路径，尚未完成整批验收。

## 分支与基线

- 实施分支：`refactor/architecture-cleanup`
- 基线：`1e055d6`（0.4.2）
- 保持现有 Flutter 页面与 Android 连接能力，逐步迁移核心。
- Rust 与 Dart 的 FRB 运行库、生成器使用同一精确版本 `2.11.1`。

## 当前实施顺序

1. 创建 `rust/` 工作区，打通真实原生库调用与事件流。
2. 添加 Windows/Linux runner 与 Cargo 联合构建，验证打包包含原生库。
3. 迁移 aTrust 报文、认证与用户态代理，以固定向量和本地服务验证；入站接入统一分流决策接口。
4. 实现基础规则引擎、策略组、配置档与规则集更新，验证 DIRECT、REJECT、SHU-VPN 的真实出口和校园授权。
5. 实现 TUN、路由、DNS、权限和系统代理的创建与撤销，接入统一规则；高级规则按平台能力逐步扩展。
6. 接入连接控制器、桌面窗口与托盘，完成响应式布局、规则编辑与命中原因展示。
7. 多平台 CI、安装包、校园连通性、分流一致性与长期运行验证。

## 验收账本

| 里程碑 | 状态 | 所需证据 |
| --- | --- | --- |
| M1.1 Rust 工作区 | 已验证 | Windows/Linux Cargo 编译、Rust 全量回归、Clippy；各批数量见下表 |
| M1.2 FRB 双向调用与流 | 已验证 | 日志/Ping/Pong、真实代理字节与速率、最终断开事件、绑定重复生成通过 |
| M1.3 平台联合构建 | 已验证（x64） | Windows/Linux release 构建，打包库路径的真实 FFI 检查通过 |
| M2.1 OAuth/SSO | 桌面实现已验证，整体部分完成 | Windows/Linux 已接入 Rust OAuth2、换票、环境上报、认证链、设备 ID；真实校园认证和 Android Rust 接入待验证 |
| M2.2 aTrust 控制与 L3 | 桌面控制/代理、双栈 TCP 包桥及 IPv4 UDP 通道已验证，整体部分完成 | Rust TLS 固定、L3 握手/心跳/VIP、TCP 签名通道、本地双栈 TCP 终止、UDP 原始包逐流签名/token 接入；资源解析暂用 SDK，完整包协议与校园联调待完成 |
| M2.3 SOCKS5/HTTP | 桌面规则代理已验证，整体部分完成 | Rust HTTP/CONNECT/SOCKS5 接入统一决策，实际 DIRECT/REJECT/SHU-VPN、半关闭、重绑与清理通过；Android Rust 接入待完成 |
| M2.4 统一规则引擎与策略组 | 基础规则与 DNS 数据已验证，整体部分完成 | 基础规则、策略组、实际出口/授权、校园 TCP DNS 元数据及缓存通过；校园生产联调及 TUN 数据面待完成 |
| M2.5 配置档、规则集与原子更新 | 基础实现与刷新已验证，整体部分完成 | 前置/后置、三类 RULE-SET、固定预览、原子切换、恢复/回滚、独立间隔刷新及真实 HTTPS 下载通过；独立多档管理和生产订阅联调待完成 |
| M2.6 高级规则与兼容扩展 | 逻辑/正则/Geo 已验证，整体部分完成 | 第十六/十七批支持逻辑、受限正则与固定 Geo 数据；进程身份与完整兼容矩阵待完成 |
| M3.1 Windows TUN/路由/系统代理 | 系统代理已验证，TUN 与出口部分完成 | 系统代理恢复、驱动状态与原生单播 socket 绑定已有证据；实际 Wintun 网卡、路由、ring 与高层会话待完成 |
| M3.2 Linux TUN/路由/系统代理 | TUN/Netlink 与 TCP/UDP 数据面已验证，整体部分完成 | 隔离内核双栈校园 TCP、IPv4 校园 UDP、真实 DIRECT 双栈 TCP/UDP、拒绝与撤销通过；系统代理、权限及桌面接入待完成 |
| M3.3 权限与保活 | 待实施 | 提权/拒绝/退出行为及后台运行验证 |
| M3.4 TUN 规则路由与 DNS 联动 | 原生 DNS 映射/动态捕获与接口出口已验证，整体部分完成 | 已授权校园 DNS 入站、正 TTL 双栈主机路由、命名资源 TCP/IPv4 UDP、版本/租约清理通过；OS DNS、完整出口覆盖与全协议能力待完成 |
| M4.1 窗口/托盘 | x64 基础实现已验证，整体部分完成 | Windows 原生窗口/托盘、Linux X11/Xfce 面板及无托盘环境通过；真实校园认证操作、Wayland/GNOME/KDE 环境待验 |
| M4.2 响应式布局 | 待实施 | 移动与桌面宽度的布局测试和实际窗口检查 |
| M4.3 Rust 连接事件接入 | 桌面代理已接入，整体部分完成 | Rust 状态/速率/字节/时延接入，取消、重连、终态和销毁竞态通过；移动/TUN 接入待完成 |
| M4.4 规则与连接管理界面 | 基础编辑与更新状态已验证，整体部分完成 | 桌面原文编辑、模板、预览/应用/回退、规则测试、手动更新及实时刷新状态已接入；卡片、排序/启停、策略组选项及连接详情待完成 |
| M5.1 校园联调与长期运行 | 待实施 | 校园账号/网络连通性、断线重连、资源占用记录 |
| M5.2 多平台分发 | 部分完成 | Windows/Linux 验证与构建 CI 已配置，尚未在 GitHub 运行；安装包待实施 |
| M5.3 分流一致性与回滚验收 | TCP 三入站基础一致性已验证，整体部分完成 | HTTP/SOCKS/TUN 相同目标 TCP 的规则决策与真实 DIRECT 端点一致；完整 DNS/UDP/捕获范围、未捕获展示及所有系统设置恢复待完成 |

## 已落地产物

- `rust/`：`shuvpn_core` 工作区、FRB API、Rust/Dart 生成绑定。
- `rust_builder/`：Windows MSBuild/CMake 与 Linux CMake 的 Cargo 联合构建；原生库随 Flutter 产物安装。
- `windows/`、`linux/`：桌面 runner。Windows 图标由现有 `assets/images/icon_light.png` 转换，未引入新的品牌素材。
- `lib/core/rust/rust_core.dart`：单次初始化、原生健康检查和健康检查失败后的桥接清理。桌面启动前加载核心；Android 暂沿用现有引擎。
- `rust/src/api/atrust_l3.rs`：L3 编解码桥接。每个解码器有 128 KiB 缓冲上限；帧格式出错后必须显式重置；握手完成后保留下一帧的字节。
- `tool/rust_bridge_smoke.dart`：直接加载指定库，验证异步事件、并发调用、报文编码、分片、错误传播、重置、VIP 和剩余字节。没有 mock 或跳过检查。
- `test/bootstrap_test.dart`：核心启动失败、初始化等待、组件销毁后迟到错误的回归测试。
- `.github/workflows/native-core.yml`：Windows/Linux 的 Rust、Flutter、FFI、生成一致性和 release 构建检查。

## 已确定的实现选择

复用现有 Dart 依赖同一 fork 提交中的 `sangfor-core`，Git revision 为 `91a7efb2533ae87708ce6c221b3dea4d7fdf1098`，锁定在 `rust/Cargo.lock`。该依赖提供无 I/O 的 Rust 协议实现；`shuvpn_core` 负责应用接口、资源边界和宿主集成。没有将依赖源码复制到本仓库。

L3 开隧道与心跳测试向量来自该提交的 `packages/flutter_sangfor/test/fixtures/native_atrust.json`。分片握手响应是合成测试输入，不能替代生产网关联调。Rust 核心使用压缩 IPv6 文本；现有 Dart 观测器保持原行为，连接控制器接入时需要核验地址展示契约。

Flutter SDK 使用仓库声明的 `3.47.2` / Dart `3.13.2`。依赖解析按 SDK 要求将 `meta` 和 `vector_math` 锁为 `1.18.3` / `2.4.0`；其余既有运行库没有主动升级。

## 本地验证证据（2026-10-11）

| 验证 | 结果 |
| --- | --- |
| 修改启动代码前的 Flutter 全量测试 | Windows 318 项通过 |
| 修改后的 `flutter analyze` | Windows、WSL Ubuntu 22.04 均无问题 |
| 修改后的 `flutter test` | Windows、Linux 均 345 项通过，包含真实原生认证与代理测试 |
| `cargo test --manifest-path rust/Cargo.toml --locked` | Windows、Linux 均 31 项通过 |
| `cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings` | Windows、Linux 通过 |
| FRB 生成重复执行 | Rust/Dart 生成文件的 SHA-256 保持一致 |
| `flutter build windows --release` | x64 构建通过，包含 `shuvpn_core.dll` |
| `flutter build linux --release` | WSL Ubuntu 22.04 x64 构建通过，包含 `libshuvpn_core.so` |
| 打包库的 `dart run tool/rust_bridge_smoke.dart <library>` | Windows DLL、Linux SO 均通过 |
| 指定打包库目录运行 `test/native/rust_authentication_test.dart` | Windows/Linux 各 7 项真实 FFI 认证测试通过 |
| 指定打包库目录运行 `test/native/rust_proxy_test.dart` | Windows/Linux 各 3 项真实 TLS 代理、事件流和类型错误测试通过 |
| 缺失原生库的 smoke | 按预期非零退出，没有挂起 |
| `flutter pub get --enforce-lockfile` | Windows、Linux 通过 |
| 结构与依赖边界检查 | `rust/`、`lib/core/rust/` 结构映射完成；仓库没有机器可检查的层声明，边界检查器返回 2，改用导入关系人工审查 |

Linux 在 `/home/mzee/.cache/shuvpn/linux-worktree` 的独立源码快照中构建，避免 Windows/Linux 覆盖彼此的 `.dart_tool` 与平台缓存。Windows 构建产物位于 `build/windows/x64/runner/Release/`；Linux 构建产物位于上述快照中的 `build/linux/x64/release/bundle/`。

此阶段未验证实际窗口与托盘交互、Android 构建、ARM64、Debug/Profile、真实校园登录、系统 TUN 或长期运行，也尚未执行远端 CI。桌面 OAuth2、L3 控制与校园代理使用 Rust；资源解析和 anti-MITM 配置读取暂用原 SDK，Android 保留原数据面。测试网关证明真实 TLS 和字节转发，不替代校园真实协议、权限及内网联调。

本批跨越原生边界，按 HIGH 风险执行全量回归与真实 FFI 检查。依赖方向为应用启动 → Dart 核心适配 → FRB API → `sangfor-core`；协议库不依赖 Flutter。Rust 构造器的形状重复来自独立的帧与握手状态机，不合并它们的生命周期。独立只读审查未发现本批范围内的具体缺陷；它没有替代主线程的构建和测试结果。

## 下一个实施入口

1. M2.1 的桌面实现已落地。Android 仍使用现有 Dart 认证，需要在 Android 原生库构建接通后迁移；校园真实登录仍需要授权账号与网络。
2. M2.2：继续将客户端资源和 anti-MITM 配置读取下沉到 Rust，并补齐 L3 包路由/流认证。桌面 TLS/L3 控制与 TCP 代理已接通，当前资源边界为 `ATrustSSOLoginSession → ATrustSessionPlan → Rust`。
3. M2.4/M2.5：基础规则、实际出口、配置快照、RULE-SET、定时刷新与校园 TCP DNS 元数据/缓存已接入。下一步完成多配置档管理、DNS 实际校园环境验证及生产订阅联调；进入 M3 前补齐 UDP DNS 和平台捕获/出口保护。选择 SHU-VPN 后继续独立检查网关授权。
4. 上游同 revision 的 `sangfor-host` 提供 `Host`、事件与统计回调；`sangfor-tun` 提供 Wintun/TUN/fd。复用前需核对 Wintun reader 与 close 的生命周期契约；上游 Windows 配置使用 netsh，Linux 不配置地址/路由，没有系统代理快照与回滚，不能据此勾选 M3。
5. M4.1 的窗口/托盘基础行为已接入；继续验证 Wayland/GNOME/KDE 与认证交互，并实施响应式布局。M4.4 继续实现配置档卡片、排序/启停、策略组控件和连接详情。M3 仍需平台配置恢复、权限、Android fd 所有权；M2.6 高级规则需单独验证数据集和平台元数据。

## 规则分流规划修订（2026-10-11）

按用户要求，工程计划新增参考 Clash Verge Rev 的规则管理与配置覆写、参考 Mihomo 匹配语义的规则系统。具体语法、优先级、模块职责与官方参考资料见工程计划 4.2–4.7；本次只更新规划文档，没有交付规则引擎，也没有改变以上已验证的代码与测试状态。

SOCKS5、HTTP 与 TUN 共用 Rust 分流决策，支持 DIRECT、REJECT、SHU-VPN 与策略组。用户前置规则、配置档规则、校园自动规则、用户后置规则依次匹配，最后使用单一 MATCH；校园网关授权独立于可编辑规则，授权失败、断线或出口能力不足不得静默改为直连。网关资源匹配本身不等于完整的自定义规则系统。

新建配置档计划默认校园资源走 SHU-VPN、其他目标走 DIRECT，同时提供 MATCH,REJECT 的校园专用档。已有严格代理配置不能自动改成直连，需显示迁移预览并明确选择。默认 TUN 仍只捕获校园范围；可选全流量接管须在直连 TCP/UDP/IPv6、DNS、出口保护及路由恢复实际验证后开放。未进入 TUN 的流量不能显示成已由规则引擎处理。

配置档原文、用户覆写与编译快照分开保存；规则集更新先验证再原子切换，失败保留有效版本。基础阶段交付域名、CIDR、端口、协议、入站类型、RULE-SET 和 MATCH；地理库、逻辑规则、进程匹配、fake-IP 与嗅探按依赖和平台能力另行验收，不承诺完整 Clash 配置兼容。

新增范围涉及 M2.4/M2.5/M2.6、M3.4、M4.4 与 M5.3，均为待实施。原工程计划的阶段工期不包含这些新增项，在基础接口与测试向量确定后重新估算。

## 第二批：桌面 aTrust 认证迁移（2026-10-11）

`rust/src/auth/` 实现完整 `authConfig → OAuth code → ticket → mod=1 → reportEnv → authCheck → onlineInfo` 链路；`rust/src/api/atrust_auth.rs` 暴露带类型错误、取消能力与安全随机设备 ID 的 FRB API。HTTP 使用固定版本 `reqwest 0.12.28` 与 rustls，禁止自动重定向和系统代理回环；JSON 响应上限 1 MiB，每次请求超时 15 秒，后续认证步骤最多 8 次。

`lib/core/auth/atrust_authentication.dart` 定义账户和连接共用的认证接口。桌面启动通过 `createRustAuthSession` 注入 Rust 后端；现有设备持久化键 `atrust_device_id` 与统一身份认证持久化格式保持不变，已有设备 ID 不重新生成。Android 保留 Dart 后端。账户的密码/企微登录、OTP/JWXT 与 SDK 的资源解析仍使用原实现。

Rust 按请求来源隔离网关与 SSO Cookie，并保留 Cookie 域、路径与过期时间。同名条目以名称、域和路径共同区分；Dart HTTP 出口保留同名条目并优先发送更具体的路径。SDK 接收的 Cookie 保留作用域和过期时间，且隧道网关来源必须与认证来源一致。快捷授权回调验证来源和路径，无 code 的快捷入口仍回退到登记的授权端点。

认证调用在 Dart 侧排队，避免账户刷新与连接入口重复登录。注销提升 generation、取消在途请求并阻止迟到结果恢复 Cookie；Rust 取消不等待 HTTP 或状态锁。每次新的认证 seed 清理旧 CSRF，错误通过 `AuthFailure` 映射到既有 `ShuAuthException`。网络错误移除请求 URL，避免一次性 code/ticket 出现在错误中。

新增 11 项 Rust 认证测试，连同原 8 项共 19 项；新增 7 项真实 FFI 认证测试与 5 项生命周期/Cookie 回归测试。测试使用本地 HTTP 服务，覆盖完整流程、已登录复用、快捷入口、CSRF 更换、增强认证、Cookie 隔离/路径/过期、超限响应、拒绝、取消与队列。TLS 证书及校园实际协议行为尚未通过真实网关联调验证。

普通 CI 和 Android 发布 CI 在 Flutter 测试前构建本机 Rust 库，实际 FFI 测试不使用 mock、不跳过缺库失败。两次只读审查发现的快捷回调、CSRF、Cookie 路径/域和过期导出问题已修正并补充回归覆盖。

## 第三批：桌面 Rust 控制通道与校园代理（2026-10-11）

`rust/src/connection/` 实现异步 TLS、L3 握手/心跳/VIP 更新、资源授权匹配、签名 TCP tunnel、HTTP absolute-form/CONNECT 与 SOCKS5 CONNECT。证书固定复用同 revision 的 `sangfor-tls::PinVerifier`；固定列表存在时始终校验，只有计划明确允许且没有固定材料时才接受未固定证书。资源外目标仍被拒绝，没有隐式 DIRECT。

Rust 拥有控制通道与代理 socket，Dart 不搬运代理字节。每个监听最多 256 个客户端，HTTP 请求头上限 32 KiB，拒绝非法头名及歧义长度，移除代理凭证和 hop-by-hop 字段。半关闭保留另一方向的流量；关闭/销毁撤监听并取消客户端。端口占用时回退至系统端口并返回实际绑定值。连接数为累计成功拨号数，沿用原 UI 契约。

`rust/src/api/vpn.rs` 提供可取消连接 attempt、会话、独立代理 start/stop、状态/速率事件，以及原格式连接 ID 和安全随机签名密钥。事件任务持有共享状态与接收器，不持有 FFI 会话对象。速率来自实际转发的字节增量，断开后给出最终状态。

`lib/core/connection/native_connection.dart` 定义控制器端口，`lib/core/rust/rust_connection.dart` 提供适配。桌面启动注入 Rust 工厂，`ConnectionController` 保留现有 UI 的状态、地址、端口、字节、速率和时延契约。Android 与其他协议继续走既有路径。桌面系统 TUN 未接入，当前需启用 HTTP/SOCKS5 代理使用已实现的数据面。

新增十项真实 TLS 测试、两项路由边界测试、九项控制器回归和三项真实 FFI 测试。覆盖固定证书拒绝、签名/域名转发、160 KiB 往返、握手剩余数据、尾随 VIP 帧、缺心跳、半关闭、HTTP 改写/拒绝/非法头、取消、重绑、并发停止、旧 attempt 迟到失败和终态后迟到绑定。Windows/Linux 全量测试、Clippy、release 构建及打包库 FFI 测试均通过。

TLS 网关共用 `rust/tests/support/gateway.rs`，FFI 测试通过 `rust/examples/proxy_fixture.rs` 启动。证书与密钥在运行时生成，没有落盘私钥；helper 不打包进应用。完整测试前构建命令为 `cargo build --manifest-path rust/Cargo.toml --locked --release --lib --examples`。

新修订的 M2.4/M2.5/M2.6、M3.4、M4.4、M5.3 仍为完整目标的一部分。本批保留严格代理的旧配置语义，尚未交付统一规则决策、DIRECT/REJECT、配置档或规则集。已授权域名资源直接交给网关；按 IP 发布的资源仍使用系统解析结果匹配授权，完整校园 DNS 策略待规则/DNS 数据面完成。不能以本批资源匹配测试证明自定义规则系统已实现。

## 第四批：基础规则、配置快照与桌面编辑页（2026-10-11）

`rust/src/routing/` 将解析编译、纯匹配、校园授权、规则集 I/O 和持久化分开。HTTP/SOCKS5 共用首条命中决策；DIRECT 拨号物理端点，REJECT 不拨号，SHU-VPN 仍独立核对网关协议、端口和 appId。失败不会继续匹配或改为直连。已有配置默认保留严格校园代理，只有手动预览并应用模板后才改变默认出站。

基础语法包含 DOMAIN、DOMAIN-SUFFIX、DOMAIN-KEYWORD、IPv4/IPv6 CIDR、DST-PORT、NETWORK、IN-TYPE、RULE-SET、MATCH 与 no-resolve。手动 select 组解析到实际内置出站；未知规则、策略、字段及不支持的 mrs 格式明确拒绝。domain 规则集处理 IDNA 和整标签通配符；共享规则集载荷使用 Arc，端口段数和总展开匹配预算有上限。

配置档与用户覆写分别保存；有效顺序为前置、配置档、自动校园、后置、单一最终 MATCH。预览持有编译快照，提交不会再次下载，版本比较阻止并发覆盖。临时文件同步后替换持久化状态，再切换内存快照；失败保留旧配置。上一有效版本可回滚，版本号递增。规则集文件/下载不可用时，仅复用描述符相同的有效缓存；格式或语义损坏拒绝整个新快照。缓存回退提示随当前及上一版本持久化，重启后仍可见。

规则集支持 inline/file/HTTPS 来源、domain/ipcidr/classical 行为及 YAML/text 载荷。当前通过重新校验配置触发更新，尚未实现 interval 定时刷新；HTTPS 下载器已实现大小和超时限制，但成功下载路径尚无本地 TLS 集成验证。所有文件来源限制在应用规则目录，HTTPS 缓存统一放入配置状态文件。

`lib/features/settings/routing_settings_page.dart` 提供仅校园/校园优先模板、原文编辑、校验预览、应用和回退；必须成功预览才能应用，编辑后旧预览失效。桌面启动从应用支持目录加载同一规则管理器，新连接附加该管理器。规则测试不发起 DNS、不提供当前连接资源表，因此只显示用户配置的规则意图，不证明自动校园授权或实际连通性。

校园名称需要 IP 时使用获授权的校园 TCP DNS；失败不使用公共 DNS。DIRECT 尝试所有已解析地址；DNS 地址只接受对应查询名称或有界 CNAME 链中的相同地址族记录，一种地址族查询失败仍保留另一种成功结果。校园 DNS 的真实网关联调、解析来源/TTL/版本元数据及缓存仍待完成，不能用基础匹配或本地代理测试替代。

新增核心测试覆盖规则边界、展开预算、配置并发/失败/恢复、固定预览、真实独立出站、自身代理防回环、热更新及旧流保留。真实 Dart FFI 另验证独立物理端点和拒绝不拨号，编辑页测试验证模板到应用流程。第四批验证结果另见下表；前面 345/31 项是第三批基线。

| 验证 | 第四批结果 |
| --- | --- |
| Windows Flutter 全量测试 | 最终版本 348 项通过，原生测试加载 Release 目录中的 DLL |
| Windows/Linux Rust 全量测试与 Clippy | 两端各 51 项通过，包含 DNS 名称关联和多地址重试回归；Clippy 均无警告 |
| Linux Flutter 全量测试 | 最终版本 348 项通过，原生测试加载 release bundle 中的 SO |
| Windows/Linux release 构建 | 两端 x64 最终版本通过 |
| 打包库 smoke 与 FFI | Windows DLL/Linux SO 的 smoke 及打包库全量测试均通过，包含各 13 项真实原生测试 |
| Flutter 静态检查、Rust 格式与生成一致性 | 两端 Flutter analyze、cargo fmt 检查通过；FRB 再生成哈希不变 |

M2.6 高级规则、M3 平台接管、M4 桌面窗口/托盘与完整规则管理、M5 校园长期运行和分发仍属于完整目标。规则命中与 TUN 捕获范围不能混同，本批没有启用 TUN 或全流量捕获。

## 第五批：规则集刷新与异步预览一致性（2026-10-11）

`rust/src/routing/refresh.rs` 为当前生效配置的 file/HTTPS 规则集分别维护更新间隔。`interval` 为 1–31,536,000 秒，未设置时只手动刷新；inline 规则集不接受无意义的刷新间隔。描述符或间隔变化会重新建立对应定时任务。启动时从持久化有效载荷恢复，再重新计时；本批尚未保存跨进程的最后检查时间。

恢复当前及上一配置时，缓存描述符必须与配置档逐项一致，并重新校验来源、格式、行为和 interval 边界。损坏缓存中的零间隔、超大间隔或不一致描述符会在启动定时任务前被拒绝，避免绕过导入时的校验。

刷新只替换规则集载荷，保留配置原文、前置/后置规则和策略组选择。到期批次整体下载、校验并编译，失败保持旧快照；内容不变时不提升配置版本。有效变化仍使用版本比较、持久化成功后发布的同一提交入口，较旧刷新结果不能覆盖用户较新配置。下载时只持有数据副本与弱引用，最后 owner 销毁时通过 watch 关闭取消网络等待，不让定时器或下载永久保留目录锁。

FRB 增加当前刷新状态、手动刷新和更新流。Dart 同时处理 Future 结果与状态流，统一按递增版本接收新配置并使旧预览失效。编辑内容采用修订计数，校验期间被修改的输入不能发布迟到预览；模板加载期间禁用编辑操作。回退失败保留草稿与一致的预览。规则页显示当前有效缓存提示、更新状态、上次检查时间和失败原因；后台刷新不会改写编辑器草稿。

两端回归发现并修正了失败提示被无关成功检查清空的问题：运行时按规则集保留失败状态，其他规则集成功发布新版本也不能掩盖尚未恢复的失败。该规则集成功检查或用户应用新配置后才清除对应状态；这类运行时检查记录不写入持久化快照。

HTTPS 下载测试运行实际 TLS server，使用同一下载配置，仅在测试私有客户端中增加临时证书信任根。未信任证书仍失败；YAML/text 下载解析、UTF-8 BOM、损坏内容和超限响应均有真实网络验证。生产下载器继续校验证书、禁止重定向和系统代理，并限定 HTTPS。测试证书与私钥临时生成，不进入发布产物。

新增验证覆盖不同间隔及热更新、失败重试、手动更新不覆盖覆写、旧刷新提交冲突、下载中销毁并撤销 TCP 连接、目录锁释放、状态流使旧预览失效、迟到校验丢弃、损坏缓存恢复拒绝，以及磁盘写入失败时回退保留草稿。

| 验证 | 第五批最终结果 |
| --- | --- |
| Rust 全量测试 | Windows/Linux 各 59 项通过 |
| Clippy 与 Rust 格式 | 两端无警告，格式检查通过 |
| Flutter 全量测试 | 两端各 350 项通过，原生测试加载最终 release bundle 的 DLL/SO |
| Flutter 静态检查 | Windows/Linux 无问题 |
| release 构建 | Windows/Linux x64 均通过 |
| 打包库 smoke 与 FFI | 两端 smoke 通过；各 15 项真实原生测试包含在全量回归中 |
| FRB 再生成、diff 检查 | 生成文件 SHA-256 一致，diff 无空白错误 |

本批继续推进 M2.5/M4.4，未完成独立多档管理、完整 DNS 元数据、高级规则、TUN、窗口/托盘或校园真实长期联调。

## 第六批：DNS 来源、TTL、版本与真实通道验证（2026-10-11）

`rust/src/routing/dns.rs` 持有 DNS 响应校验、解析元数据与有界缓存。每次真实解析记录目标名称、地址、来源、响应 TTL、规则版本、引擎缓存命中，以及查询返回时的观察后时长和缓存剩余时间。校园来源标明所选授权 DNS 服务器；系统解析只标为 system，不编造 `lookup_host` 无法提供的 TTL、服务器身份或系统缓存命中信息，且不进入引擎缓存。cached 字段只表示 ShuVPN 自身缓存。

校园查询通过获授权的签名 TCP 隧道发送 A/AAAA 报文，继续校验证书固定、资源 appId 和端口。响应校验 ID、opcode、question、记录类和实际 CNAME 链；冲突、循环或超长链被拒绝。有效 TTL 取实际采用的 CNAME 各段与地址记录最小值，缓存最长一天且不延长响应截止时间。TTL 为零只供当次使用；未知 TTL、部分地址族失败和已过期结果不缓存。

双栈查询并发并设定查询预算；一种地址族失败或超时仍可采用另一种有效结果。等待期间正 TTL 记录过期时，受限重新查询该地址族；仍过期的数据不能进入规则匹配和拨号。缓存最多 512 个域名结果，来源与版本分开；更换规则管理器时，在同一锁范围取得/切换缓存 epoch，旧请求不能命中新管理器缓存或把迟到结果写回。会话关闭后清空映射。

真实网络回归修复了先前查询名称缺少末尾点、导致 wire 解码后的 question 不一致的问题。旧严格校园代理的目标解析也接入统一策略；校园 DNS 不可用时失败，不再经低层拨号暗中使用系统解析。DIRECT 明确采用系统地址解析与物理 socket，校园 DNS 数据不冒充系统数据。

测试网关现在解析真正的 IPv4/IPv6 TCP 目的记录，在授权 DNS 通道中收发真实 DNS 报文。验证 CNAME TTL、缓存命中/过期、规则版本变化、零 TTL、A/AAAA 失败与超时、异常 ID/无关回答、无授权 DNS 时不进行系统目标解析、no-resolve、IPv6 地址授权与隧道回显。Dart FFI 另验证最终元数据、缓存统计与真实代理字节。`dns_stats` 的 system_lookups 只统计目标解析接口调用，不代表 OS/控制通道的全部 DNS 网络活动。

| 验证 | 第六批最终结果 |
| --- | --- |
| Rust 全量测试 | Windows/Linux 各 70 项通过 |
| Clippy、Rust 格式与 Flutter 静态检查 | 两端通过，无警告或问题 |
| Flutter 全量测试 | 两端各 351 项通过，使用最终 release bundle 的 DLL/SO |
| release 构建 | Windows/Linux x64 均通过 |
| 打包库 smoke 与真实 FFI | 两端 smoke 通过，各 16 项原生测试包含在全量回归中 |
| FRB 生成一致性 | 再生成后的 Rust/Dart 文件 SHA-256 不变 |

本批没有接管操作系统 DNS，没有交付 UDP DNS 数据面、完整 CNAME 追问、负缓存、DNSSEC、fake-IP 或动态 TUN 路由。系统解析的 TTL/服务器身份仍未知，校园实际通道仍需现场验证；这些限制不会被测试网关结果替代。

## 第七批：桌面窗口、托盘与退出清理（2026-10-11）

桌面入口初始化 window_manager，设置 1024×760 初始尺寸、640×560 最小尺寸和隐藏的系统标题栏；`DesktopFrame` 在启动、错误页和正式应用中提供一致的拖动区、最小化、最大化/还原及关闭按钮。按钮带文字提示和键盘焦点，Ctrl+Q 明确退出；移动端不创建桌面运行时。

`lib/core/desktop/` 分离原生端口、插件适配与生命周期策略。托盘提供状态、显示窗口、连接、断开、协议及退出菜单；协议使用固定已知注册表，未实现/未启用或连接锁定时置灰。菜单只创建一次，后续修改标签、启用与选择状态，流量变化不会不断重建菜单。连接动作先显示窗口再进入原有连接/账号路径，验证码继续使用现有 shell 输入流程。

关闭窗口时，仅在托盘图标成功建立且宿主仍可用时隐藏；否则等待 disconnect 后退出。Linux 检查实际 StatusNotifierWatcher 服务，没面板时不创建不可找回的隐藏窗口。退出等待连接、初始化与菜单更新收尾，再撤托盘并销毁窗口；失败保留窗口、重新开启关闭拦截并允许重试。销毁期间的迟到托盘初始化及菜单更新有明确清理顺序。

原生探针发现 Windows 的 PostQuitMessage 路径会在成员析构中销毁 Flutter 视图，此时窗口消息重入仍把消息交给正在销毁的控制器，导致进程异常退出。`windows/runner/flutter_window.cpp` 在析构前显式 reset 控制器，并保护迟到字体消息；修改后探针以退出码 0 结束。

`tool/desktop_window_smoke.dart` 是独立测试入口，使用真实 Flutter 窗口、托盘插件和本地 TLS/Rust 代理，验证最小化、还原、最大化、关闭隐藏/显示、退出前代理端口撤销以及测试网关退出。它不使用校园凭据，发布构建恢复 `lib/main.dart`。Linux 探针用独立 DBus/Xvfb/X11/Openbox 会话；`tool/desktop_linux_smoke.sh --panel` 启动真实 Xfce systray/StatusNotifier 面板，配置写入独立临时目录，不改用户面板配置。

已通过 Windows 托盘环境、Linux 无托盘与真实 Xfce 面板两种环境的原生窗口/代理清理探针。Linux Flutter SDK 销毁 implicit view 时仍输出非致命上游警告，进程退出码为 0、代理与网关已关闭；本批没有以此证明所有桌面环境行为相同。真实校园验证码、GNOME/KDE/Wayland、单实例启动及系统网络恢复仍待验收。

Linux 联合构建新增 libayatana-appindicator3-dev；本机安装和 native-core CI 依赖声明已更新。托盘 ico 由现有品牌 PNG 派生，没有引入新品牌图案。

| 验证 | 第七批最终结果 |
| --- | --- |
| Flutter 全量测试 | Windows/Linux 各 363 项通过，包含 12 项桌面策略/菜单/标题栏回归 |
| Flutter 静态检查 | 两端无问题 |
| 正式入口 release 构建 | 两端 x64 通过，最终目标为 lib/main.dart |
| 打包库 smoke 与 FFI | 两端 smoke 通过，16 项原生测试包含在全量回归 |
| 原生桌面探针 | Windows 托盘、Linux X11 无面板与 Xfce 面板均通过，退出码 0；真实 TLS 代理监听和网关撤销 |
| Rust 核心 | 本批没有修改核心源码，上一批两端 70 项验证仍为核心基线 |

## 第八批：Linux TUN 与 Netlink 平台基础（2026-10-11）

`rust/src/platform/` 新增 Linux TUN 适配器，使用原生 `ifreq` 与 `TUNSETIFF` 独占创建非持久网卡，再通过 Netlink 配置 MTU、虚拟地址与校园分流路由。只操作本次创建的接口，不替换默认路由。接口名称、MTU、地址和捕获范围在任何系统修改前校验；保护地址不能落入捕获网段，也不能被配置为本机虚拟地址。现有同名接口、占用地址和同前缀主路由表条目会使创建失败。

异步读写持有文件描述符操作租约；关闭先取消等待、释放 owner，再等所有租约结束，避免关闭后复用描述符。内核包长度、版本、头长度与 MTU 在写入前校验。创建失败、取消和正常销毁均释放非持久网卡描述符；相应接口及其路由随内核接口删除，无需修改已有接口以回滚。配置前另用 TUNGETIFF 核对描述符实际接口名，拒绝已观察到的重命名。

FRB 暴露创建、信息、包读写与关闭接口；Windows 对有效计划明确返回尚未支持的平台错误。Linux 适配器尚未接入连接控制器和 VPN 数据面，界面没有启用 TUN 开关；调用者提供的保护列表也不能替代未来权限 helper 的可信控制面策略。此批没有实现系统代理、OS DNS 接管、动态捕获路由或全流量模式。

`tool/tun_linux_namespace_smoke.sh` 在独立 network namespace 中验证真实 IPv4/IPv6 UDP 包捕获与回复、取消等待和重复关闭、名称/路由冲突、无权限拒绝、IPv6 配置失败后撤销已配置的 IPv4 地址，以及 SIGKILL 后内核清理。失败探针对比原有地址与路由快照；正常探针检查默认路由保持原值。`tool/tun_bridge_smoke.dart` 另经真实 SO/FRB 捕获内核包，验证关闭取消在途读取并删除接口。

| 验证 | 第八批结果 |
| --- | --- |
| Rust 全量测试 | Windows/Linux 各 73 项通过 |
| Clippy、Rust 格式 | 两端 Clippy 无警告；格式检查通过 |
| Flutter 全量测试与静态检查 | 两端各 364 项通过，analyze 无问题 |
| 正式入口 release 构建及打包库 smoke | Windows/Linux x64 通过，入口为 lib/main.dart |
| Linux 内核隔离探针 | 包收发、冲突、权限、部分配置失败、强制终止六个场景通过 |
| Linux 真实 TUN FFI | 内核包跨桥接读取、在途取消和接口清理通过 |

native-core CI 新增隔离命名空间内核与 TUN FFI 探针；两端打包后的原生检查扩为整个 test/native 目录。CI 声明已更新，尚未据此宣称 GitHub 远端运行通过。M3.2 仍为部分完成，下一步连接统一规则与数据面，之后继续 Windows、权限 helper 和系统设置恢复。

接入调查确认 `connection/session.rs` 当前 control_loop 仅消费心跳及第二虚拟地址响应，尚未消费数据帧。固定上游版本的 `plane::DataPlane` 已提供节点效果、每流签名授权、connectToken 和 TCP 本地终止回退；不能将原始包直接写入控制 TLS stream。其 `packet::build_packet_meta` 和流授权目前使用 IPv4，虽然入站拆包支持 IPv6，也不能据此宣称 IPv6 出站已交付。后续集成必须统一通道所有权、流授权、规则决策与关闭顺序，并补齐或明确拒绝尚未支持的出站。

## 第九批：统一 UDP 包规则、校园 L3 授权与 Linux TUN 数据面（2026-10-11）

`rust/src/connection/packet/` 从原始 IPv4/IPv6 UDP 包提取统一 Flow，使用现有规则快照与独立校园资源授权。每个新五元组固定建立时的规则决策，配置更新只影响新流；不会因失败继续匹配其他规则或隐式直连。端口最多持有 128 个流，入口、每流和回包队列均有容量上限；空闲流回收、停止和对象销毁会取消在途 I/O。包版本、长度、MTU、IPv4 头与 UDP 校验和均校验，源地址必须是当前会话下发的虚拟地址。

校园 IPv4 UDP 为获授权资源建立 TLS L3 通道，发送带资源 appId、五元组、conntrackHash 和 HMAC 签名的流授权请求。响应必须具有匹配的流标识、成功状态及有效 connectToken，随后才释放排队原包；回包仍校验授权五元组和 IP/UDP 格式。没有资源、拒绝、错配授权、错误回包及尚未验证的校园 IPv6 原始包均明确失败。真实 TLS fixture 独立验证签名、token、帧边界和实际发送字节。

DIRECT 为每流创建真实 UDP socket，保留虚拟五元组构造回包。Linux TUN 必须选择明确出口，通过内核接口类型查询拒绝 TUN/无效接口，并用 SO_BINDTODEVICE 使 DIRECT 不回到捕获路由。当前接口出口支持以太网与回环，未交付 PPP 等接口；IPv6 link-local/mapped、分片、扩展头、TCP、ICMP 和零端口包均不属于本批能力。普通包端口供跨平台原生适配使用，尚不能当作 Android VpnService 已接入。

`VpnSession.start_tun/stop_tun` 把 Linux 网卡、包端口与会话生命周期连接起来。TUN 地址必须来自当前认证会话；启动前另从节点计划解析并核对保护地址，不能仅信任调用者 JSON 声明。停止等待包流和网卡租约结束再报告完成，并可在同一会话重建。独立审查发现停止 Future 被取消后提前移出任务槽的问题，现改为等待完成后移除，并补充取消重试/driver 异常测试。TUN FD 创建也不再使用可脱离等待屏障的后台创建工作。心跳采用 Delay 补发策略，调度停顿后的过期 tick 不会连续计作多个独立失败探测。

`tool/tun_data_plane_linux_smoke.py` 在独立 network namespace 创建 veth 和另一个隔离对端。真实内核 UDP 请求进入 TUN：校园目标经签名 TLS 回显；DIRECT 的 IPv4/IPv6 请求从指定 veth 发往对端，再回注内核 socket；拒绝及未授权目标不发送校园数据。探针验证 stop_tun、重建、session.stop 后网卡撤销，以及原有 IPv4/IPv6 默认路由保持不变。没有使用校园账号，也没有修改宿主网络。

Linux 首次全量回归出现一项旧代理测试连接拒绝，单独复跑通过；已保留失败记录、增加状态上下文并调整心跳逾期补发策略，修改后的全量回归通过。首次失败没有足够日志证明唯一根因，不能把复跑通过当作原失败未发生。

| 验证 | 第九批最终结果 |
| --- | --- |
| Rust 全量测试 | Windows 84 项、Linux 85 项通过；Linux 多一项 TUN 停止取消回归 |
| Clippy、Rust 格式、Flutter 静态检查 | 两端通过，无警告或问题 |
| Flutter 全量测试 | 两端各 365 项通过，使用最终 release bundle 的 DLL/SO |
| 正式入口 release 构建及打包库 smoke | Windows/Linux x64 通过，入口为 lib/main.dart |
| Linux 基础内核探针 | 原有包收发、冲突、权限、部分配置失败与 SIGKILL 六场景全部通过 |
| Linux 实际 TUN 分流 | veth 对端验证校园签名 UDP、DIRECT IPv4/IPv6、拒绝、未授权、停止/重建及默认路由保持 |
| 真实 FFI | 两端新增原始 UDP TLS/取消测试通过；Linux root 隔离探针验证 Dart start_tun→内核→校园 L3→内核回复，session.stop 返回后网卡已删除 |
| FRB 生成一致性与差异检查 | 重复生成后 Rust/Dart 文件 SHA-256 不变，git diff --check 通过 |

native-core CI 已加入实际 TUN/veth 探针，仍未宣称 GitHub 远端执行已通过。FRB API 包含跨平台包端口以及 Linux start_tun/stop_tun；Dart 测试和 Linux FFI 探针均使用生成的真实接口。

完整目标仍包括 TUN TCP、校园 IPv6、DNS 动态捕获、控制/SSO/下载等全部物理出口保护、Windows 驱动与权限 helper、UI 接线、高级规则、安装包和真实校园长期验证。此批不开放全流量模式。

## 第十批：双栈 TCP 终止、统一拨号与完整内核路径（2026-10-11）

采用计划中的 smoltcp，精确固定为 0.14.0（最低 Rust 1.91），处理每流原始 IPv4/IPv6 TCP 的序列、确认、窗口、重传及双方 FIN。每流接收/发送缓冲各 64 KiB，原始输入/输出各最多 64 个包，读写块为 16 KiB，读、写及结果通道各容量 1；TCP/UDP 总流数仍最多 128。测试注入回包阻塞并重复 10,000 次输入，验证拨号尚未结束时内部包队列也不会绕过上限。

第一个 SYN 捕获规则快照，统一 TCP 拨号入口接收该快照；现有 HTTP/SOCKS 仍使用同一入口。校园 TCP 复用已有资源匹配、appId、请求签名、证书固定及目标 IPv6 握手。DIRECT TCP 和 UDP 共用 `connection/physical.rs`，Linux TUN 的物理 TCP socket 同样绑定明确出口。选定出站实际建立后才发出 SYN-ACK，拒绝、未授权和拨号失败会重置 SYN，没有隐式直连。

读写分为受取消令牌控制的子任务。TCP 接收窗口和有界队列对停读/慢写施加背压；远端 EOF 只关闭本地发送方向，客户端 FIN 则在接收缓冲及已排队上传完成后执行远端半关闭。窗口重新开放后继续发送剩余数据；正常关闭和会话取消等待子任务终态后才完成。写入及半关闭有 300 秒等待预算；这不是普通空闲 TCP 连接的统一寿命上限。运行中错误会记录原快照的决策和实际失败原因。

真实 TLS fixture 增加双栈 IP 发布的 TCP 资源，正常 EOF 后发送 TLS close_notify。测试客户端独立构造原始 TCP 包，验证双栈校园 256 KiB 往返、DIRECT 双栈、双方各自先 FIN、512 KiB 数据的零窗口恢复、拒绝/未授权 SYN、规则热更新，以及回包无人读取时取消仍撤销所有通道。Linux 首次测试暴露了测试客户端的重传缺口：此前既不修剪重叠重传段，也不重发丢失的数据段。修正客户端并保留超时上下文后，修改后的完整回归通过；内核 socket 验证另外证明真实协议栈的行为。

HTTP/SOCKS/TUN 三入站测试使用相同目标 IP、端口与规则版本，分别访问同一个真实物理监听端点，验证三份 RouteDecision 相同及独立的实际字节。该证据只覆盖已知 IP 的 TCP 基础一致性，不替代 DNS 名称映射、进程身份或所有规则组合的验收。

Linux veth 探针增加校园和 DIRECT 的双栈内核 TCP，每个方向验证 256 KiB 原样数据与 FIN；拒绝和未授权目标不能完成内核握手。真实 Dart FFI 探针再通过 start_tun 建立内核 TCP/UDP 流，验证校园 TCP 前导、64 KiB 往返、UDP 回包，以及 session.stop 返回后接口删除。

| 验证 | 第十批最终结果 |
| --- | --- |
| Rust 全量测试 | Windows 95 项、Linux 96 项通过 |
| Clippy 与 Rust 格式 | 两端无警告，格式检查通过 |
| Flutter 静态检查及全量测试 | 两端 analyze 无问题，各 365 项通过 |
| 正式入口 release 构建与打包库 smoke | 两端 x64 通过，最终入口为 lib/main.dart |
| Linux 内核路径 | 原有 TUN 六场景全部通过；校园/DIRECT 双栈 TCP、IPv4 校园 UDP、DIRECT 双栈 UDP、拒绝、撤销及默认路由保持通过 |
| Linux 真实 FFI | 基础读写/取消及 Dart start_tun→内核 TCP/UDP→校园通道→内核回复通过，session.stop 后接口已删除 |
| 生成一致性与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，git diff --check 通过 |

两端最终库和 Linux 内核/FFI 探针都使用同一批当前核心源码。CI 中已有探针会执行新增 TCP 场景，但尚未以本机结果替代 GitHub 远端执行证据。

此批仍未开放桌面 TUN 开关；校园 IPv6 原始 UDP、分片/扩展头、TCP Fast Open、动态 DNS 捕获、所有控制/下载物理出口、Windows 网卡、权限 helper、系统代理和校园真实长期运行仍未完成。

## 第十一批：原生进程出口保护、绑定 DNS 与 HTTP 请求租约（2026-10-11）

`rust/src/network/` 统一原生出站 TCP/UDP 的建立、物理接口选择和 socket 注册。已建立的控制/出站连接使用实际 socket 副本作为保护操作租约，注册表仅持 Weak，不保留关闭后的连接；保护/恢复时升级租约，避免使用已经复用的裸 FD。Linux TUN 进程只允许一个生效出口，loopback 通道仍走 lo，其他原生出站走指定以太网接口。

启动先取得转换写锁，等待已开始的连接、HTTP 请求及系统解析工作收尾，再发布出口和配置网卡/捕获路由。系统解析即使调用方取消，阻塞工作也继续持有读租约，不能在捕获生效后突然发出旧的未绑定解析。已有 socket 记录原绑定，创建失败及取消会撤销本次保护；正常关闭先撤销 TUN 接口/路由，再恢复旧绑定。恢复失败保留可重试的 retired 状态，阻止下一次切换假装成功；`egress_status` 提供进程作用域、生效接口、解析器、socket 绑定及待清理状态。

TUN 期间的节点与 DIRECT 名称查询使用指定物理出口的 A/AAAA UDP 查询，复用严格 DNS 回答校验；截断回答核对响应 ID、question、opcode/type 后通过同一出口进行 TCP 查询。显式 `physical_dns_servers` 拒绝 loopback stub、unspecified 和 multicast；缺省查找系统明确上游，只有本地 stub 时不编造其上游，也不回落公共 DNS。IP 与 localhost 可直接使用；系统解析与该物理查询仍不向前端编造未接入的 TTL/缓存元数据。

原生认证与 HTTPS 规则集加载不再复用启动前的长期 HTTP 客户端。每个请求使用当前出口、固定解析结果和关闭空闲池的客户端，响应读取期间持有转换读租约；认证响应还保留客户端及租约到作用域结束。Cookie 来源/路径、CSRF、重定向限制、证书校验和字节上限保持原约束。HTTP 连接建立有 5 秒预算，请求原有 15 秒预算保留；转换等待仍受调用者的会话取消控制，不能把请求预算当作任意保留响应对象的寿命上限。

`egress-probe` 是默认关闭的测试 feature。隔离 veth 探针为捕获网段中的 DNS 服务器和 HTTPS 规则源提供双栈回答、截断回答及错误 ID；证明 UDP/TCP DNS 与 HTTPS 下载绕过 TUN 捕获走物理口，默认未信任证书失败，临时测试根证书仅由探针私有 loader 使用。探针也用 IPv6 部分配置失败验证旧控制 socket 的绑定回滚，随后成功启动并验证 stop_tun 恢复原绑定。首轮测试证书误用了 CA 扩展作为服务端证书，校正为服务端 leaf 后通过；没有放宽生产校验。

| 验证 | 第十一批最终结果 |
| --- | --- |
| Rust 全量回归 | Windows 97 项、Linux 98 项通过 |
| Clippy 与格式 | 两端含所有 features/targets 的 Clippy 无警告；Rust 格式检查通过 |
| Flutter 全量回归与静态检查 | 两端各 365 项通过，analyze 无问题 |
| 默认 features 正式构建 | Windows/Linux x64 release 与打包库 smoke 通过，入口为 lib/main.dart |
| Linux 原有内核场景 | 六项 TUN 生命周期场景、双栈 TCP/UDP 分流与拒绝、撤销全部通过 |
| 出口保护特权探针 | 捕获网段内的双栈 DNS、TCP 截断重试、错误 ID 拒绝、HTTPS 规则下载、未信任证书拒绝、部分配置失败及停止后的绑定恢复通过 |
| 真实 FFI | 出口状态跨桥接、内核 TCP/UDP 校园往返及 session.stop 后网卡/出口撤销通过 |
| 生成一致性与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，git diff --check、探针及工作流语法检查通过 |

CI 已接入 feature 专用探针，并在其后重新构建默认 features 的库；测试证书未安装系统信任根，测试 feature 不进入默认桌面库。远端 CI 尚未据本机结果宣称执行通过。

本批保护范围是 Rust 原生出站，包括认证及规则下载；没有接管操作系统 DNS，没有替代 Dart SDK/WebView 的网络迁移，没有完成 DNS 域名到捕获路由的动态映射，也尚未交付 Windows 网卡、权限 helper 或桌面 TUN 开关。完整目标继续保留。

## 第十二批：校园 DNS 入站、域名身份映射与动态捕获租约（2026-10-11）

实验 TUN 计划新增默认关闭的 `dns_capture`。启用时，仅为认证资源允许 TCP/53 的校园 DNS 地址提供 UDP DNS 入站并添加其主机捕获范围；没有获授权服务器或地址与保护范围冲突时拒绝启动。DNS 入站只接受单一 IN A/AAAA 问题，经现有签名 TCP DNS 通道查询并复用 ID、question、CNAME、TTL 等校验；不支持的类型返回 NotImp，错误/无可捕获 TTL 返回 ServFail，超出 MTU 的 UDP 回答标记 TC。它没有修改操作系统 DNS 配置。

`connection/capture.rs` 串行管理域名、版本/epoch、地址与流租约。仅校园来源、属于发布域名资源、尚未到期的正 TTL 结果建立映射，最多 512 个名字和 1024 个地址。IP 存在多个有效域名时明确报歧义，不按最后查询或端口猜测站点。配置更新/替换清除旧映射，新流取得开始时的快照及有效域名；旧流保留建立时身份及地址租约。

DNS 回复等待 IPv4 /32、IPv6 /128 路由安装、版本/epoch 复核及尚有效 TTL，再返回给内核客户端。动态路由只使用本次 TUN 的接口、main table、Link scope 和固定 metric，不替换已有同目标主机路由。更换采用逐条 Netlink 操作并维护实际安装集合，有失败回滚；这不是内核原子事务。操作和回滚有 2 秒预算，超时不能证明内核提交状态时关闭整个 TUN，而非声称安全恢复。关闭取消保留 FD 租约直至操作退出。

TCP 和 IPv4 UDP 从映射恢复域名执行同一规则入口，校园资源检查仍独立。TCP 签名目标保持原域名，同时绑定实际 DNS 回答 IP；UDP 授权带 domain 和原始五元组。DIRECT 仍按原始包目标 IP 走物理出口，不把校园解析结果冒充系统解析。TTL 清理由 100ms 轮询触发，不是硬实时截止；正在使用的地址租约使旧 TCP/UDP 流保持捕获，最后一个流结束后删除到期地址。

新增 `resolve_campus_dns` 与 `capture_status` 真实接口。前者沿用 DNS 来源/版本/缓存元数据并在活动 TUN 中等待捕获发布，后者显示逻辑捕获地址、有效映射、租约及错误。没有以这些接口替代实际路由验证。

独立 namespace 探针从内核 UDP 查询 DNS，得到回答后连接回答 IP，证明仅发布域名的资源可经签名校园通道双栈 TCP 往返及 IPv4 UDP L3 往返。未预置回答地址路由，直接检查内核主机路由；TTL 到期时未使用 IPv6 路由撤销，已建立 IPv4 TCP 保留并继续传输，流结束后撤销。配置替换、错误 ID、零 TTL 与外部同目标路由冲突也有真实验证。Dart FFI 探针另覆盖内核 DNS→动态捕获→域名 TCP→停止清理。

| 验证 | 第十二批最终结果 |
| --- | --- |
| Rust 全量回归 | Windows 100 项、Linux 101 项通过 |
| Clippy 与格式 | 两端所有 targets/features 的 Clippy 无警告，Rust 格式检查通过 |
| Flutter 全量与静态检查 | 两端各 365 项通过，analyze 无问题 |
| 默认正式入口构建 | Windows/Linux x64 release 与打包库 smoke 通过，入口 lib/main.dart |
| Linux 真实内核 DNS 场景 | 自动双栈路由、原域名 TCP 签名、IPv4 UDP 域名授权、TTL/流租约、版本撤销、错误 ID/零 TTL/路由冲突通过 |
| 旧内核场景回归 | 六项 TUN 生命周期场景及原 TCP/UDP 实际分流探针通过 |
| 真实 FFI | 新解析/捕获状态接口及内核 DNS→域名 TCP→清理探针通过 |
| 生成一致性与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，格式、工作流语法与 git diff --check 通过 |

CI 已加入原生与真实 FFI DNS 探针，尚未宣称 GitHub 远端执行通过。

OS DNS 设置、负缓存/DNSSEC、完整 CNAME 追问、fake-IP/域名嗅探、校园 IPv6 原始 UDP、分片、桌面接线、Windows 驱动、权限 helper 及真实校园长期验收仍未完成；实际不同客户端的 DNS 缓存/网络切换仍待现场验证。

## 第十三批：原生资源准备、anti-MITM 与桌面认证接线（2026-10-11）

新增原生 `prepare_tunnel`：串行完成现有认证链、刷新 authConfig、验证登录状态和最新 CSRF、请求 clientResource、解析 L3VPN 应用/节点/DNS，并生成 SessionPlan。资源响应轮换 SID 后，计划和返回会话都使用新 SID；Flutter 只在准备成功且代际未变化时同步共享 Cookie。取消覆盖认证、资源请求和排队操作，晚到的取消不会调用已释放的原生对象。

资源解析保留 WAN 优先、LAN 后备、`{{sdpcHost}}` 替换、默认 441 端口、裸 IPv6、输入顺序的主节点回退，以及旧 SDK 的默认协议字段。资源引用缺失或空节点组时仍保留资源，由已有隧道拨号回退主节点组；没有把节点缺失误判为网关未授权。无效资源/地址会报告警告，资源、节点、响应和签名解析均有预算。

anti-MITM 使用固定 SDK 的 AES-128-CBC/PKCS7、挑战派生、两轮 SHA-256/XOR、叶子字段 HMAC 与证书摘要盐。响应 JSON 保留原始字段顺序，以匹配重复叶子键的最后覆盖行为；最终字段排序采用 Dart 的 UTF-16 顺序。独立 Dart 工具生成确定向量并调用固定 SDK 验证，包含中文挑战、重复字段、布尔/null、非 BMP 键及证书摘要；Rust 消费同一向量并验证篡改拒绝。

桌面 RustConnectionAttempt 不再通过 Dart SDK 获取资源和生成计划，改为使用原生认证后端的准备能力。兼容上报模式沿用此前行为：挑战/签名失败记录警告，保留公布的节点证书固定；原生严格模式直接拒绝。没有放宽 HTTPS 认证证书验证，也没有实现 SDK 本身未发出的独立 antiMITM confirmation 请求。Android 旧路径仍待迁移。

| 验证 | 第十三批结果 |
| --- | --- |
| Rust 全量回归 | Windows 106 项、Linux 107 项通过 |
| 原生认证集成 | 14 项通过，覆盖资源请求/CSRF、SID 轮换、严格拒绝、兼容模式保留固定、会话失效和无效标识 |
| Clippy 与格式 | 两端所有 targets/features 的 Clippy 无警告；Rust 格式检查通过 |
| Flutter 静态检查与全量回归 | 两端 analyze 无问题，各 367 项通过 |
| 打包库真实 FFI | 两端各 9 项原生认证测试通过，含新资源准备和资源请求阶段取消；全量 Flutter 测试同样使用各端 release 库 |
| 正式入口构建与 smoke | Windows/Linux x64 release 通过，入口 lib/main.dart；两端打包库 smoke 通过 |
| 生成一致性与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，git diff --check 通过 |

日志分别保存在本机缓存的 `auth-windows-rust.log`、`auth-linux-build.log`、`auth-windows-clippy.log`、`auth-linux-release.log`、`auth-windows-flutter.log`、`auth-linux-flutter.log`。Linux 初次 Clippy 发现测试使用 unwrap，修正后重新检查通过；首次静态检查发现 Dart 接口类型未提升，显式能力转换后两端检查通过。

本批验证使用本地 HTTP/TLS fixture，未替代真实校园认证或长期连接验收。Windows 驱动、系统代理/OS DNS、权限 helper、桌面 TUN 接线、高级规则、Android Rust 接入和安装包等原计划项仍未完成，完整迁移目标继续保留。

## 第十四批：Windows/GNOME 系统代理租约与桌面开关（2026-10-11）

`platform/system_proxy/` 保存原设置和本次写入值，修改前提交恢复记录，并以目录文件锁约束本应用实例。记录限制 256 KiB、字段限制 64 KiB；写入前先验证原设置也能被恢复读取，超限时不修改系统。Linux 记录文件使用 0600 权限。记录覆盖本批验证的进程终止恢复，系统设置的多字段写入没有被宣称为 OS 原子事务。

Windows 通过 WinINet 的默认 LAN per-connection 接口读取 FLAGS_UI（必要时回退 FLAGS）、代理端点及排除项，写入 FLAGS、HTTP/HTTPS 端点并刷新设置。没有直接重写用户注册表，也没有覆盖 PAC URL；旧模式标志和端点用于恢复。GNOME 通过 GIO/GSettings 修改 dconf 中的 mode、HTTP/HTTPS 端点、HTTP 认证开关和排除项，保留用户值与未设置值的区别；没有修改已废弃的 enabled/use-same-proxy 字段或认证用户名/密码。进程私有 memory/keyfile 后端不能宣称为系统配置。KDE 支持仍未完成。

设置变化进入已有会话串行队列。启用需要已启动、IPv4 本机可访问的 HTTP 监听，并使用实际绑定端口。重复启用核验当前设置；调用方放弃结果时撤销本次设置，撤销失败保留租约到会话清理并报告失败。停止 HTTP 之前恢复，恢复失败保留监听；关闭整个会话时先恢复再关闭监听，失败结果不会被取消分支或 UI 的“已断开”覆盖。工作线程异常的对象销毁仍会尝试恢复，无法完成时留存记录。

恢复按字段比较当前值与原值/本次值。外部端点变更保留对应端点及全局模式，避免关闭其他应用的新代理；外部排除项变更仅保留该项，仍还原本应用的端点，避免留下指向已停止监听的代理。启动及每次桌面重连在认证前重放旧记录。

Flutter 网络设置加入默认关闭的“连接时启用系统代理”、实际状态和恢复错误。启用偏好会开启 HTTP 监听偏好，关闭 HTTP 偏好同时关闭系统代理偏好。运行期沿用整页锁定；不支持的桌面可以关闭旧偏好，不能重新启用。系统代理只覆盖采用系统设置的应用，仍遵循当前规则，不代表 TUN 或自动启用公网 DIRECT。

本批独立审查发现并修正缓存成功标志、取消分支吞掉清理错误和 GVariant 引用计数问题。真实空数组恢复探针复现了 `[]` 的类型推断失败，改为使用 schema 的 string/int32/bool/string-array 类型后验证。独立 Dart 探针改为直接使用纯 FRB API，以免独立 Dart 运行时引入 Flutter 的 dart:ui。

| 验证 | 第十四批最终结果 |
| --- | --- |
| Rust 全量回归 | Windows 110 项、Linux 111 项通过 |
| Clippy 与格式 | 两端所有 targets/features 的 Clippy 无警告，Rust 格式检查通过 |
| Flutter 静态检查与全量回归 | 两端 analyze 无问题，各 370 项通过，使用最新打包库 |
| 正式入口 release | Windows/Linux x64 构建通过，入口 lib/main.dart |
| Windows 实际设置与进程终止 | 默认 LAN 系统代理指向实际端口；正常停止、HTTP 停止、取消及 TerminateProcess 后恢复通过，原 ProxyEnable/Server/Override/PAC 值一致 |
| GNOME 实际 dconf | 私有 D-Bus/临时配置目录中验证空列表、未设置值、原 PAC/认证、正常停止、HTTP 停止、取消、外部排除项及 SIGKILL 后恢复；memory 后端明确拒绝 |
| 打包库真实 FFI | 两端读取实际系统代理端口、完成校园 TLS 代理字节往返，停止 HTTP 后恢复通过 |
| 验证源码一致性 | 工作树和 Linux 快照的 153 个 Rust/Dart 源文件 SHA-256 一致 |
| 生成与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，Rust 格式和 git diff --check 通过 |

最终日志为本机缓存中的 `system-proxy-windows-rust-current.log`、`system-proxy-linux-current.log`、`system-proxy-windows-clippy-current.log`、`system-proxy-analyze-current.log`、`system-proxy-windows-bundle-current.log`、`system-proxy-windows-current.log`、`system-proxy-windows-flutter-current.log` 和 `system-proxy-linux-flutter-current.log`。CI 已加入两端恢复与打包库探针，未据本机结果宣称远端 CI 已运行通过。

Windows 驱动、KDE、OS DNS、权限 helper、桌面 TUN、高级规则、Android Rust 接入、安装包和真实校园长期验收等原目标仍保留。

## 第十五批：KDE 5/6 系统代理与 KIO 应用验证（2026-10-12）

新增 KDE 适配器，复用现有租约、目录锁和恢复记录。通过固定官方 KF5/KF6 源码核验 `kioslaverc` / `Proxy Settings`、模式编号、手动代理的空格端口格式、例外列表以及 KIO 刷新 signal，再分别使用实际 KConfig 工具验证。工具优先使用 6，缺失时使用 5；配置值通过字面 argv 传递，操作有 5 秒预算和 64 KiB 输出上限。

实际 Dart AOT FFI 验证暴露了宿主回收原生直接子进程的问题：Dart 的退出线程会等待全部直接子进程，Rust 不能可靠取回其退出状态。改为固定监督者等待工具，在管道末尾写入随机标识和工具退出状态；不修改宿主的回收线程或全局信号处理。失败/超限/超时会终止监督者的进程组。字面 shell 特殊字符、非零退出、输出超限和超时后不能延迟写配置均有测试，纯 Rust 与真实 Dart 宿主都已验证。

用户文件读取保留缺失键与空字符串的区别。正常写入使用与 KIO 相同的相对文件名和级联不可变策略，恢复缺失键则删除本地项，使系统继承值重新可见；没有用删除标记屏蔽继承配置。PAC URL、FTP/SOCKS 代理及其他应用分组不作为本次写入字段。对代理字段的展开/删除标记明确报不支持，未静默改变其语义。

独立审查发现外部端点改回启用前值时，旧逻辑仍可能关闭该端点的当前模式。恢复记录新增可向后读取的 active/preserve 字段：成功启用和实际恢复阶段分别原子保存；检测到外部端点时先保存需保留的字段，再执行恢复。正常调用、崩溃重放和已保存恢复计划后中断都有回归向量。旧记录没有新增字段时仍可读取。

新增独立 C++ KIO 客户端，使用发行版提供的 Qt/KIO 调用 HTTP GET，而非由测试手动填写代理地址；校园 TLS fixture 验证签名后返回固定 HTTP 内容。测试同时观察 KIO 刷新 signal 的路径、接口、成员及空字符串参数。该证据针对 KIO 应用，不宣称所有程序都会采用 KDE 配置或已有连接立即重建。

KF6 工具和 KIO 客户端在独立 Debian trixie 根目录中运行，使用官方包索引和校验值建立依赖环境；测试采用独立 D-Bus、XDG_CONFIG_HOME/XDG_CONFIG_DIRS，不修改桌面用户真实代理设置。Dart AOT 所需的 `/proc/self` 仅在私有挂载空间中提供，工作目录固定到隔离工程目录。

| 验证 | 第十五批最终结果 |
| --- | --- |
| Rust 全量回归 | Windows 111 项、Linux 114 项通过 |
| Flutter 与静态检查 | 两端各 370 项通过，analyze 无问题 |
| Clippy 与正式构建 | 两端 all-targets/all-features 无警告，Windows/Linux x64 release 构建通过 |
| KDE5 实际工具与客户端 | KF5 5.92 / Qt5 5.15；正常停止、HTTP 停止、取消、SIGKILL、空/缺失/继承值、immutable、标记拒绝、外部变更及原端点回退验证通过 |
| KDE6 实际工具与客户端 | Debian KF6 6.13 / Qt6 6.8；同一套配置/恢复验证、真实 KIO HTTP 内容和 Dart AOT FFI 通过 |
| 配置生效 | 观察 KIO 刷新 signal 的实际参数；两套 KIO 客户端使用系统设置完成校园签名 TLS 上的 HTTP 内容读取 |
| 新恢复记录 | 外部端点改回原值仍保留其模式；正常、崩溃重放、恢复计划中断通过 |
| 旧平台回归 | GNOME dconf、Windows WinINet 的真实恢复与打包库 FFI 探针通过 |
| 生成与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，Rust 格式、探针语法、CI YAML 和 git diff --check 通过 |

最终日志在本机缓存中的 `kde-linux-regression.log`、`kde-windows-rust.log`、`kde-windows-clippy.log`、`kde-windows-flutter.log`、`kde5-supervised-ffi.log`、`kde6-verified-smoke.log`、`kde-windows-proxy.log`。CI 已增加 KF5/KIO 客户端和 AOT FFI 验证，本机独立 KF6 根目录验证不作为远端 CI 已执行的证据。

原目标中的 OS DNS、Windows 驱动、权限 helper、桌面 TUN、高级规则、Android Rust 接入、安装包与真实校园长期验收仍未完成。

## 最终目标修订：后端零 Dart（2026-10-12）

用户明确选择保留 Flutter 前端、应用后端业务全部移入 Rust。主计划新增完整账号/业务认证范围、M2.7 其余应用后端下沉和 M5.4 后端零 Dart 验收；具体残留入口和顺序见[迁移审计](backend_rust_migration_audit.md)。

此项包含桌面仍执行的统一登录、企微、业务 OAuth、OTP/教务、Cookie/凭据存储，以及更新/公告、配置持久化和 Android 旧数据面。Dart 最终仅保留前端、展示快照、生成绑定和薄平台接线；旧后台实现与 SDK 后备调用链必须删除。此前已验证的 Rust 阶段继续保留，不能替代新增范围的验收。

本次侧边对话已修改目标文档并完成入口定位；未变更生产源码、未继续主线程已启动的任务，也未把新增后端迁移标为完成。下一实施范围为账号会话/Cookie 与完整 SSO，再接业务授权。

## 第十六批：逻辑条件、受限域名正则与规则能力查询（2026-10-12）

将谓词表示、字段扫描、语法解析和预算移到 `routing/predicate.rs`，配置编译仍负责顺序、策略选择及规则集编译，运行匹配仍由同一个 engine 执行。新增 AND/OR/NOT 的括号组合，允许规则集叶子；classical 载荷支持逻辑/正则而不允许嵌套规则集或 MATCH。谓词子条件不得携带出站策略，未知语法仍明确失败。

IP 未解析保留第三种结果 NeedAddress。AND 的确定假、OR 的确定真可决定整组条件，NOT 对 NeedAddress 仍返回 NeedAddress；显式 no-resolve 的无地址 IP 条件仍不匹配，NOT 会按该明确结果取反。规则集的 no-resolve 向其内部逻辑条件递归继承。没有为布尔真/假编造 DNS 回答，首条命中、自动校园顺序和独立资源授权保持原边界。

DOMAIN-REGEX 采用精确版本 regex 1.13.1，匹配已归一化的 ASCII 域名。正则字段长度 1024 字节、编译数量 128，单个程序/DFA 缓存 256 KiB、正则嵌套 32；规则字段长度 4096 字节、逻辑深度 16、节点和展开预算 100,000。规则集整体编译也纳入成本限制。环视、回溯引用、无效表达式或预算超限在发布前拒绝，不改变有效快照。

字段扫描保留分组、字符类、量词和转义中的逗号。独立审查发现合法字符类首位 `]` 被误当作结束符，已根据固定 regex-syntax 源码修正并补上顶级/嵌套/否定字符类回归。域名正则不对 IP 字面量执行，逻辑与正则均使用相同 HTTP/SOCKS/TUN 流元数据。

新增 routing_capabilities 接口，Flutter 规则页从核心读取支持与未支持清单、正则和逻辑限制，而非继续显示“高级规则未启用”的静态文案。真实 FFI 新增组合规则预览/提交/命中、无效更新保留版本的测试。实际 HTTP 代理测试分别证明组合 DIRECT 的物理字节、OR 校园签名 TLS 和正则 REJECT 不建立目标连接；NOT 选择校园出站仍不能扩大授权或回落 DIRECT。

完整 Linux 并行回归暴露旧系统代理租约的锁释放窗口：工具 fork 暂时继承同一文件描述，父句柄关闭后子句柄仍可能短暂保持锁。租约销毁改为显式 unlock，并用复制描述符的确定性复现验证；相关测试连续并行运行 20 轮后再次完成全量回归，没有只靠重跑忽略失败。

| 验证 | 第十六批最终结果 |
| --- | --- |
| Rust 全量回归 | Windows 116 项、Linux 119 项通过 |
| 静态检查与 Clippy | 两端 analyze 无问题，all-targets/all-features Clippy 无警告 |
| Flutter 与真实 FFI | 两端使用最新打包库各 371 项通过；组合规则和能力查询跨真实桥接验证 |
| 正式入口构建 | Windows/Linux x64 release 通过，入口 lib/main.dart |
| 匹配与预算 | 嵌套、三值布尔、no-resolve、规则集、IPv4/IPv6、三入站、正则边界和预算失败通过 |
| 实际出站 | 组合 DIRECT、REJECT、OR 校园通道和 NOT 授权拒绝通过，失败无隐式直连 |
| 事务 | 正则/逻辑校验失败保持当前配置版本，预览、提交和回退旧行为保持通过 |
| 生成与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，Rust 格式和 git diff --check 通过 |

日志为本机缓存中的 `logic-windows-rust-current.log`、`logic-linux-lock-regression.log`、`logic-windows-clippy-current.log`、`logic-windows-flutter-current.log`、`logic-linux-current.log`、`logic-windows-ffi.log`。远端 CI 仍未据本机结果宣称执行通过。

GeoIP/GeoSite 数据集、进程身份、完整捕获、Windows TUN、OS DNS、权限 helper、Android Rust 接入、安装包和真实校园长期验收仍属原目标的未完成部分。

## 第十七批：GeoIP/GeoSite 数据格式、固定版本与索引（2026-10-12）

新增 file/HTTPS 的 geoip/geosite 规则源和 `data:{code,version,sha256}` 描述符，格式限定 V2Fly protobuf `.dat`。类别和属性选择器归一到大小写不敏感形式；多个属性采用键存在性 AND，bool=false/int 值不改变存在性。版本是用户提供的发布标签，实际原始字节 SHA-256 在解析前核验。相同版本/摘要的只读文件可供多个类别投影，其他路径重复仍拒绝。

流式借用消息切片，先扫描类别再解析选定组，不分配未选择组的完整对象树。限制原文件 64 MiB、组数 100,000、每消息字段 2,000,000、选定条目 500,000 和 32 MiB；编译所有 Geo 源另设相同的全局选定预算。非法 wire/长度/UTF-8/域名类型/地址族/掩码在编译前拒绝。ip_addr 可作文本 IP 扩展，数据中的 file_path 不读取文件或执行动作。

GeoIP 为 IPv4/IPv6 前缀集合索引，包含 inverse_match；多个已知地址先对整个集合 any 匹配再取反，未知地址仍返回 NeedAddress，显式 no-resolve 仍不查询。GeoSite Full/Plain 保留模式值，RootDomain 使用小写 IDNA 和点边界；Regex 共用上一批预算。Full/后缀使用集合查询，Plain 使用 Aho-Corasick，独立关键词限 50,000/4 MiB，构建后的自动机限制 32 MiB。大于普通规则集 20,000 条的索引已有 25,000 条实际文件验证。

派生缓存保存行为、选择器、版本、原始 SHA 及包括 inverse 的内容摘要，恢复时对照描述符重新核验；磁盘状态预算扩到 192 MiB，保留当前及上一版本。独立审查发现 inverse 原先不受摘要保护、Attribute 重复 singular key 原先错误累计，均已修正，并增加单字段损坏和最后一次 key 值回归。

规则支持 GEOIP/GEOSITE 及其逻辑组合，RULE-SET 可引用数据投影；classical 内仍不允许继续引用 Geo 源。能力查询已更新，当前及预览说明显示实际固定版本、摘要和条数。真实代理测试通过 GeoIP 选择物理 DIRECT、GeoSite 属性选择 REJECT/校园签名 TLS，独立授权不因此扩大。真实 Dart FFI 验证固定数据预览、提交、命中和失败后保留版本。

| 验证 | 第十七批最终结果 |
| --- | --- |
| Rust 全量回归 | Windows 123 项、Linux 126 项通过 |
| Flutter 与静态检查 | 两端各 372 项通过，analyze 无问题 |
| Clippy 与正式构建 | 两端 all-targets/all-features 无警告，Windows/Linux x64 release 构建通过 |
| 实际数据加载 | 独立 protobuf 编码 fixture，经文件和真实 HTTPS TLS 加载；不信任证书/错误摘要/坏消息拒绝 |
| 匹配 | 四域名类型、属性 presence/AND、双栈 CIDR、inverse、NeedAddress/no-resolve、逻辑组合与大类别通过 |
| 更新和缓存 | 坏文件、版本/摘要/类别不匹配保持当前快照；源文件删除后重启派生缓存可用，篡改被检测 |
| 实际出站与 FFI | Geo 数据选择真实物理/拒绝/校园通道；两端数据版本和命中跨打包库验证 |
| 生成与差异检查 | FRB 重复生成后 Rust/Dart 文件 SHA-256 不变，Rust 格式与 git diff --check 通过 |

日志位于本机缓存 `geo-windows-rust.log`、`geo-windows-clippy.log`、`geo-windows-build.log`、`geo-windows-flutter.log`、`geo-linux-final.log`。本批验证使用固定小型及大条目 fixture，不代替社区数据集的准确性或真实校园长期验收；远端 CI 未据本机结果宣称执行通过。

进程身份、OS DNS、Windows TUN、权限 helper、桌面 TUN 接线、Android Rust 接入、安装包和真实校园长期验收仍属原目标未完成部分。

## 第十八批续作：Windows 驱动状态桥接与两端回归（2026-10-12，部分完成）

修复 Windows IP Helper 四处 Clippy 初始化错误，生成 `tun_driver_info` 的 Rust/Dart 绑定。真实 FFI 测试改为验证固定 Wintun 版本/摘要、权限状态、校园分流范围和 `sessionDataPlaneReady=false`；普通权限创建请求必须返回 permission。管理员进程不尝试创建测试网卡，避免常规测试修改系统。Linux 补充 native-linux/wintun=false 的真实桥接断言。

CI 的开发库测试使用仓库驱动的绝对路径。正式 Windows bundle 测试清除 `SHUVPN_WINTUN_DLL`，通过模块旁的实际 DLL 加载并验证固定 SHA-256；许可证随包安装到 `data/licenses/wintun/LICENSE.txt`。没有用环境覆盖替代随包发现验证。

| 验证 | 续作已收取的结果 |
| --- | --- |
| Windows Rust | 定向四项及全量 127 项通过 |
| Linux Rust | 独立源码快照全量 126 项通过 |
| Clippy / 格式 / analyze | 两端 all-targets/all-features 无警告，Rust 格式通过，analyze 无问题 |
| 正式入口 release | 两端 x64 构建通过，入口 lib/main.dart |
| Flutter 与真实 FFI | 两端各 372 项通过，使用当批 bundle；Linux 最新驱动状态断言另行补跑通过 |
| 打包库 smoke | 两端 `rust_bridge_smoke.dart` 通过 |
| Linux 实际内核回归 | 当前 release/examples：原生包/撤销、名称/路由冲突、拒绝权限、部分初始化回滚、SIGKILL；双栈域名 DNS、TTL/流租约/版本/无效回答/冲突；实际校园/DIRECT TCP/UDP、REJECT 与清理均通过 |
| Windows 打包驱动 | 清除路径覆盖后加载通过，SHA-256 为 E5DA8447DC2C320EDC0FC52FA01885C103DE8C118481F683643CACC3220DAFCE，许可证存在 |
| FRB 稳定性 | 重复生成的 14 个 Rust/Dart 文件 SHA-256 相同 |

Windows 高层接入前的源码调查已抽查确认：`network/interface.rs::validate` 对非 Linux 显式拒绝物理接口；`network/socket.rs`、`network/mod.rs::Record` 和 `network/http.rs::Lease::builder` 的实际绑定仍是 Linux 专用；动态捕获 worker 接受 Linux Device 并依赖 `sync_dynamic`；`VpnSession.start_tun` 仍明确拒绝 Windows。下一步须补 Windows 的现有/新 socket 与 HTTP 出口保护、双栈物理 DNS、动态路由事务和取消清理，再接高层会话。不能只放宽 cfg。

Windows 特权网卡/路由/ring 收发、清理失败重试、权限 helper、OS DNS、桌面 TUN、Android 和后端零 Dart 范围尚未验收；本次结果不作为 Windows TUN 可用或全项目完成的证据。远端 CI 尚未执行验证。

Linux 内核探针先用当前源码重新执行 `cargo build --manifest-path rust/Cargo.toml --locked --release --lib --examples`，随后以 WSL root 运行 `tool/tun_linux_namespace_smoke.sh`、`tool/tun_dns_linux_smoke.sh` 和 `unshare --net python3 tool/tun_data_plane_linux_smoke.py`。全部系统改动限制在独立 network namespace 内，没有修改宿主路由。默认用户 `sudo -n` 要求密码的执行未进入探针；改用 WSL root 后收取成功结果。

回退本次桥接接线时，应同时恢复 Rust/Dart 生成文件、对应 native 测试与 CI 环境配置，避免两侧 ABI 不一致；本次没有修改用户持久化格式，没有创建 Windows 网卡或路由。

## 第十八批续作（二）：Windows 原生出口与可重试清理（2026-10-12，部分完成）

Windows 原生网络适配保存接口 LUID/索引，要求接口启用且为硬件 Ethernet/Wi-Fi 或回环，拒绝虚拟 TUN 作为物理出口。现有及新建 TCP/UDP socket 在连接前设置对应 Winsock 单播接口选项；退出、取消启动或重复关闭恢复原索引。状态查询读取实际 socket 选项，不再为 Windows 返回空占位。IPv4 映射的 IPv6 目的地在建 socket 前归一为 IPv4，避免只保护 IPv6 选项；当前绑定路径对组播/广播明确拒绝，不能据此启用完整捕获。

默认物理 DNS 只从所选 LUID 的 GetAdaptersAddresses 记录读取，不使用其他接口或默认公共上游；忽略回环 stub，IPv4 映射地址同样归一后校验。当前带作用域/link-local IPv6 DNS 明确报不支持。原生物理查询复用受保护 UDP/TCP，包括截断后 TCP 重试。现场 WLAN 的 DNS 配置全部是本机 stub，筛选后为零个可用上游；域名查询须显式配置物理 DNS，没有回落到系统 stub。

Windows 的锁定 reqwest 版本没有连接前接口绑定扩展点。HTTP Lease 因此持有请求专属的本机转接器，使用同一受保护 socket 拨号：监听仅在 127.0.0.1，32 字节随机代理凭据，目标 hostname/port/scheme 和请求体约束在拨号前校验。HTTPS 的 TLS 仍由 reqwest 与目标端协商及校验证书。普通 HTTP 每连接只处理一个请求，使用 Hyper 标准 codec 处理 body/framing，删除代理认证头；已有流水请求与 chunked 回归证明第二次请求和代理凭据不会进入源站。取消会撤销监听、relay 及 upstream task。

仅在 Windows 增加直接依赖 Hyper 1.12.0、hyper-util 0.1.21、http-body-util 0.1.5，版本沿用原锁文件中的 reqwest 依赖，没有升级这些包；server feature 新增 httpdate 1.0.3。采用标准 HTTP codec，避免维护另一套请求体边界解析。认证与规则下载的 no_proxy 策略集中到 HTTP Lease builder，保留请求专属转接器；正常无保护状态仍不采用系统代理。

Wintun 配置关闭保留删除失败的路由/地址记录，仅移除成功或已不存在的记录，重试保持逆安装顺序。配置由 Device 与资源租约共享，资源释放不再把错误固化为不可恢复的字符串；显式 close 等待操作结束后重试仍持有的配置，再查询实际接口撤销。失败删除、部分成功和重试有确定性测试；真实特权删除失败、初始化失败恢复及崩溃恢复仍待验证。

| 验证 | 本轮已收取的结果 |
| --- | --- |
| Windows Rust | all-features 全量 136 项通过；最终网络定向 10 项通过，包含映射地址、组播/广播拒绝、请求前校验、流水/chunked、取消 relay 和真实 TLS |
| Linux Rust | 独立快照全量 126 项通过，all-targets/all-features Clippy 无警告 |
| Windows Flutter / 打包库 | 373 项通过；新增指定回环接口的 DIRECT UDP 真实字节、实际 binding=1、无效接口拒绝及停止释放验证 |
| Linux Flutter / 打包库 | 372 项通过；最终 native 代理六项与源码同步后的静态检查补跑通过 |
| 正式产物 | Windows/Linux x64 release，入口 lib/main.dart；两端当批 bundle 的 rust_bridge_smoke 通过 |
| 生成 | 14 个 Rust/Dart 绑定文件在重复生成后 SHA-256 相同，没有新增 FRB API |
| Windows 物理接口探针 | debug/release 均通过：现有/新建 TCP、UDP、双栈 DNS 回答、TCP DNS 重试、坏回答拒绝、正式 provider 下载的 TLS 校验、未提交回滚与停止恢复；索引 0→实际 WLAN 11→0，release 探针前后系统路由快照一致 |
| Linux 当前例程/内核 | 重新构建当前 release/examples 后，包/冲突/权限/部分失败/SIGKILL、DNS TTL/租约/版本/冲突、实际 TCP/UDP/Campus/DIRECT/REJECT 与出口 DNS/HTTPS/恢复均通过 |
| 两端源码一致性 | 220 个 Rust/Dart 源码、测试、例程及相关锁定配置 SHA-256 完全一致，清单保存于 build/windows-egress-sources.json |

探针在本机所选物理接口的地址上启动自有 TCP/UDP/DNS/TLS fixture。它证明 socket 选项及本机真实字节，不证明外部校园路径、Wintun ring 或捕获路由上的防回环。IPv6 当前证据为真实回环 socket 选项与 DNS 双栈回答，不能替代物理 IPv6 长期出站验收。组播/广播、作用域 DNS、网络切换、失败恢复及所有尚未迁移的 Dart 请求覆盖仍是后续要求，未从完整目标删除。

结构映射没有新增位置、混合职责、重复实现或组件环问题；已有不同模块的 Lease 同名保持各自职责。仓库没有供自动边界工具读取的层次声明，按主计划人工核对了变更导入与平台条件。Windows 高层 start_tun 仍拒绝启用，sessionDataPlaneReady 保持 false。下一项工作为动态路由事务/捕获、受限权限 helper 与真实网卡验收；完整后端零 Dart、Android、OS DNS、分发和校园长期验证继续保留。

回退本轮时，一并撤回 Windows network 适配与 HTTP 转接接线、Cargo/锁文件、新增测试及相关构建接线，恢复认证/规则下载原有 no_proxy 配置，再重建原生库与 bundle。本批没有迁移用户持久化数据或创建 Windows TUN/路由。

日志：Windows 缓存 `windows-egress-rust.log`、`windows-egress-flutter.log`、`windows-egress-structure.log`；Linux 缓存 `windows-egress-linux-rust.log`、`windows-egress-linux-flutter.log`。本机结果不作为远端 CI 已执行的证据。

## 第十八批续作（三）：Windows 动态路由事务与共享捕获接口（2026-10-12，部分完成）

新增 Windows 动态主机路由事务：所选 TUN LUID/索引和接口名称先核验，控制地址、VIP、映射 IPv4、组播/广播和超限输入先拒绝，静态捕获范围内的地址不重复安装。同步使用实际 IP Helper 查询/创建/删除入口；新增前拒绝已有同前缀路由，已记录路由的丢失、修改或遮蔽导致 route_rollback，禁止继续使用不确定状态。

逐项成功才更新所有权账本，新增/删除失败、取消或期限到达会恢复上一有效集合；恢复失败保留仍需清理的记录，并要求撤销 TUN。关闭失败项保留供重试。删除前按 LUID、索引、前缀、next hop、IPv6 scope/flow、metric/protocol、lifetimes 与受控标志核对已观察到的状态；Age/Origin 由网络栈设置，CreateIpForwardEntry2 忽略它们，故不以初始化值作为所有权条件。

捕获策略 worker 不再直接持有 Linux Device，而是依赖策略模块拥有的 RouteCapture 接口；Linux/Windows 适配委派各自 sync_dynamic。共享逻辑继续保持正 TTL、授权域名、版本/epoch、歧义拒绝与已有流租约，安装返回后再检查有效期/版本再确认 DNS 映射。Windows 仍未注册高层 TUN 服务；相关 start_tun、driver stop/join 以及权限 helper 必须在真实验收后一起接通，未只放宽 cfg。

| 验证 | 本轮实际结果 |
| --- | --- |
| Windows Rust 全量 | all-features 147 项通过，all-targets/all-features Clippy 无警告 |
| Windows 事务故障注入 | 双栈 /32-/128、重复同步、部分新增/删除失败、写入后取消、旧路由恢复失败、外部冲突/改动、关闭重试通过 |
| 共享 worker | 安装后确认、TTL/已有流租约、安装失败保持旧身份、致命回滚结束 worker、安装期间 epoch/TTL 变化后恢复通过 |
| Linux Rust 全量 | all-features 130 项通过，all-targets/all-features Clippy 无警告 |
| Flutter / 静态检查 | Windows 373、Linux 372 项通过，analyze 无问题 |
| 正式入口与真实 FFI | 两端 x64 release，lib/main.dart；当批 bundle 的 rust_bridge_smoke 通过 |
| Linux 当前例程内核 | 原生包/冲突/权限/部分初始化/SIGKILL，DNS 双栈/TTL/租约/版本/无效回答/冲突，TCP/UDP/Campus/DIRECT/REJECT 与清理通过 |
| FRB | 14 个生成文件 SHA-256 未变，没有新增前端 API |
| 源码一致性 | 两端 221 个 Rust/Dart 源码、测试、例程及相关锁定配置 SHA-256 一致，清单为 build/dynamic-sources.json |

Windows 事务测试使用 RouteIo 的确定性内核替身；它们证明账本和失败处理，不证明真实 Wintun/IP Helper 的成功、默认值、删除原子性或特权清理。系统查询与删除不是原子的条件删除，完全相同属性的外部替代路由也没有可查询的实例代次；不将“已观察到的外部改动会拒绝删除”扩大为“任何管理员竞态都不会改变外部状态”。后续以专属临时接口、特权探针和恢复约束继续验收。

当前 Windows sessionDataPlaneReady 仍为 false。权限 helper/IPC、真实网卡/ring、Windows 高层注册与取消/收尾、实际捕获路由上的出口保护、作用域 DNS、网络切换、Android 和后端零 Dart 等原要求全部保留。回退本批需同时撤回共享 RouteCapture 接口、平台适配、公共动态地址校验与 Windows 事务接线，再重建两端产物；没有用户数据迁移。

日志位于 Windows 缓存 dynamic-windows-rust.log、dynamic-windows-flutter.log，以及 Linux 缓存 dynamic-linux-rust.log、dynamic-linux-flutter.log。本机结果不作为远端 CI 或真实校园验收证据。

## 第十八批续作（四）：Windows 特权 TUN 探针入口（2026-10-12，部分完成）

新增独立 `tun-probe` feature 与 `windows_tun_smoke` 例程，不进入默认应用或 FRB API。管理员运行时，仅创建随机名称的测试接口和文档保留地址的 /32、/128 主机路由；既有非默认路由覆盖测试地址时在创建前拒绝。具体地址和执行命令见 CONTRIBUTING.md。

成功路径要求实际查询 VIP/Preferred 状态、MTU、LUID/索引与静态/动态路由；真实双栈 UDP socket 发包后读取 Wintun ring，匹配完整源/目的地址、端口和载荷，重新计算回包 IP/UDP 校验和后注入，等待真实 socket 的返回字节。随后检查重复同步、动态路由撤销与预先取消的拒绝，并留一组动态路由验证 close 的清理。预先取消测试不证明写入途中取消的内核回滚，后者仍只有此前故障注入证据。

空 ring 读取在工作线程观察到等待边界后发送一次性信号，探针关闭设备并核对同一次 read 返回 closed；不再以操作计数替代这项观察。重复 close 后仍须查询接口撤销并核对系统路由稳定属性快照。工作有 30 秒预算、清理有 10 秒预算；例程的独立 60 秒 watchdog 覆盖初始化和运行期阻塞，运行时关闭也有期限。watchdog 超时退出 124，明确说明清理未经证明，不输出成功标记。

| 验证 | 本轮实际结果 |
| --- | --- |
| Windows Rust | all-features 全量 148 项通过；all-targets/all-features Clippy 无警告；新增双栈回包/IPv4 options/校验和/错误元数据与截断测试通过 |
| Linux Rust | 独立快照 all-features 全量 130 项通过，all-targets/all-features Clippy 无警告；Windows 例程的非 Windows 拒绝入口 release 构建通过 |
| Windows 正式产物 | 默认 features 的 Flutter x64 release 构建通过，373 项 Flutter/当批 bundle FFI 测试通过；rust_bridge_smoke 通过 |
| Windows 探针例程 | release 构建通过；当前普通权限进程退出 1，错误为 permission；前后 Get-NetAdapter/Get-NetRoute 快照相同，没有创建接口或路由 |
| FRB / 格式 | 14 个生成文件 SHA-256 不变；Rust format 和 git diff --check 通过 |
| 默认 features / 源码 | 两端 release/lib/examples 重新构建通过；225 个源码/测试/锁定配置及 CI 文件 SHA-256 一致，清单为 build/tun-probe-sources.json |
| 结构 | 平台探针与包 fixture 有明确归属，无新增位置/混合/重复/环问题；无可供自动检查的层次声明，按主计划人工检查导入与条件编译 |

以上没有执行管理员成功路径，不证明 Wintun ring、实际动态路由或特权清理已经可用。Windows 高层 start_tun 仍拒绝启用，sessionDataPlaneReady=false；helper/UAC/IPC、真实捕获路由上的出口保护、初始化失败与崩溃恢复、系统 DNS、Android、后端零 Dart 等完整要求保持不变。本轮没有重新构建 Linux Flutter bundle；Linux 生产代码行为未改变，上一轮对应证据仍按其原范围保留。CI 测试命令改为 all-features 以覆盖可选探针的单元测试，未宣称远端 CI 已运行。

回退时撤回 tun-probe feature、例程和探针模块，移除 Windows read 的等待观察回调及对应 lib 导出、CI/贡献说明接线，再构建默认 features 的原生库和正式 bundle；没有用户持久化迁移。日志位于 C:/Users/xiema/.cache/shuvpn/：tun-probe-windows-rust.log、tun-probe-linux-rust.log、tun-probe-linux-default-build.log、tun-probe-windows-build.log、tun-probe-windows-flutter.log、tun-probe-windows-permission.log、tun-probe-bridge.log、tun-probe-structure.log。

## 第十八批续作（五）：Windows helper 与受限 IPC（2026-10-12，部分完成）

新增独立 shuvpn_tun_helper.exe、Rust Endpoint/Client 和仅持有一个 Windows TUN 的服务会话。普通应用创建 32 字节随机名的本机单实例管道，禁止远端连接；helper 使用 identification 级连接，不能把提升的 token 交给管道服务端用于模拟。握手前双方用管道 API 校验指定 PID，持续持有进程句柄；启动参数还绑定父进程创建时间，以拒绝旧 PID 代次。初始 nonce/协议版本/id=0 校验后才进入会话，后续请求使用严格递增 id。

IPC 仅接受 Hello/Create/Info/Read/Write/Routes/Close。固定长度前缀先限制 256 KiB 才分配 payload，未知操作、字段、截断、错误序号/回复类型均拒绝。计划继续由现有 Rust 校验，包和动态路由分别限 9000 字节和 1024 项；没有任意命令、文件或驱动路径操作。客户端逐请求持有独占通道，取消/错误时丢弃整条连接，后续调用不能复用旧回复；只有收到 Close 的 Done 才记录确认，取消 Close 不冒充成功。

helper 使用容量为 1 的请求队列与独立读任务。正常串行客户端断开可在执行请求期间通知会话，父进程退出也触发收尾；协议错误、路由致命恢复错误和正常关闭均经过显式 TUN close，Drop 为取消后的兜底。IPC 有握手、空闲、请求、发送和清理预算；同步原生调用停滞仍不能靠异步 timeout 保证硬中断，尚需特权故障、进程 watchdog/恢复验收。客户端 Close 还观察 helper 退出。当前 IPC 的串行请求和 JSON 包编码不作为高吞吐数据面已完成的证据。

Endpoint.launch 已实现 runas：只从当前应用同目录寻找 helper，在 blocking 线程初始化 COM，保留 ShellExecuteExW 返回的进程句柄，并使用隐藏运行参数。UAC 仍由 Windows 请求用户同意；本轮没有调用这条提升路径，也没有接入正式 FRB TUN 或 Flutter 按钮。默认 Windows Cargo/Flutter 编排同时编译和安装原生 DLL 与 helper。额外 Debug 检查复现了原 CMake 条件参数展开为空字符串导致 Cargo 拒绝的问题；增加 COMMAND_EXPAND_LISTS 后 Debug 原生目标构建通过。

| 验证 | 本轮实际结果 |
| --- | --- |
| Windows Rust | all-features 全量 158 项通过，all-targets/all-features Clippy 无警告 |
| IPC 单元/真实管道 | framing/未知字段、父进程代次、nonce/PID、请求重放、回复序号/类型、取消 Info/Close、断开后失效均通过 |
| 实际普通子进程 | 三项集成通过：计划/包/路由预算、普通权限拒绝、无设备状态、重复 Close、断开退出、过期父进程代次拒绝；管理员测试环境不尝试有效创建 |
| Windows 随包 helper | release bundle 中 helper 存在，SHA-256 与同次 Cargo 产物一致；指定该随包 exe 的三项集成重跑通过 |
| Windows 默认产物/FFI | Flutter x64 release、373 项全量测试、当批 bundle rust_bridge_smoke 通过 |
| Debug 原生目标 | CMake shuvpn_core_build 的 Debug DLL/helper 构建通过，该 Debug helper 的三项进程集成也通过；这不是整个 Flutter Debug bundle 验收 |
| Linux | 独立快照 130 项 Rust 测试及全特性 Clippy 通过，release lib/bin/examples 构建通过；helper 的非 Windows入口只返回不支持 |
| FRB/格式 | 14 个生成文件 SHA-256 不变；format 与 git diff --check 通过 |
| 源码一致性 | 235 个 Rust/Dart 源码、测试、锁定配置、CI 与 Windows 构建文件摘要一致，清单为 build/helper-sources.json |

结构工具对 helper 目录的通用名称、两个同形的 typed RPC 包装及不同模块 Response 同名给出提示；人工核对后，该目录只拥有 Windows TUN 权限进程，包装已共用 exchange/call，HTTP 与 IPC 的 Response 分别私有，没有泛用工具或重复协议实现。仓库没有自动层次声明，按主计划人工检查了平台/交付入口的依赖，没有把 Win32 或 IPC DTO 引入规则策略。

这些测试没有创建 TUN，不证明提权 helper 持有真实网卡后的断开、父进程退出、执行中取消、初始化失败或清理失败恢复。UAC 同意/取消、保护的安装目录与发布签名、真实 Wintun 收发、捕获路由上的物理出口、Windows 高层会话接入、OS DNS、Android 和后端零 Dart 全部保留。sessionDataPlaneReady 仍为 false，远端 CI 未执行；CI all-features 测试会覆盖本轮新增用例。本轮没有重新构建 Linux Flutter bundle。

回退本轮时撤回 helper 的 Cargo bin、平台 IPC/launcher 和 root 导出、测试与 Windows CMake 打包项，重建默认原生库和新 bundle，避免旧 bundle 中留下不再使用的 helper；没有用户持久化迁移。日志位于 C:/Users/xiema/.cache/shuvpn/：helper-windows-rust.log、helper-linux-rust.log、helper-windows-build.log、helper-windows-flutter.log、helper-windows-packaged.log、helper-windows-debug-build.log、helper-windows-debug-ipc.log、helper-bridge.log、helper-structure.log。实际执行命令和随包测试变量见 CONTRIBUTING.md。

## 第十八批续作（六）：helper 真实 TUN 探针与接口撤销错误修复（2026-10-12，部分完成）

为现有 windows_tun_smoke 增加 --helper、--helper-disconnect、--helper-inherit。前两种模式在普通父进程中检查未提升状态，再走生产 Endpoint.launch/runas；继承模式不触发 UAC。默认特权直接路径保留原验收项，UDP fixture 和内核身份/MTU/地址/路由查询共用。helper 路径通过 Client 的真实 read/write/sync_routes 交换数据，成功仍要求真实 UDP socket 收到返回元组及载荷，并核对 helper 退出、接口消失和系统路由稳定属性恢复。最终重装动态路由后再次查询，避免只凭 IPC Done 认定关闭时确有路由待撤销。

统一收尾覆盖 create 后的 JSON/身份/LUID/数据验证失败，仍等待关闭或断开退出，并通过 GetIfTable2 查询随机接口名称是否残留；没有把 early-return 的 Client Drop 代替 cleanup 证据。helper 模式的例程 watchdog 为 180 秒，包含 UAC；超时仅报告清理未证明。直接模式继续为 60 秒。命令与准备步骤见 CONTRIBUTING.md。

本机真实 query-only GetIfEntry2 测试发现旧撤销判断错误：不存在的 u32::MAX 接口索引返回 ERROR_FILE_NOT_FOUND（2），旧实现只接受 ERROR_NOT_FOUND（1168），因此在已不存在的接口上仍报系统错误。先保留旧行为运行新增测试，实际失败为“系统找不到指定的文件，os error 2”；随后修复状态分类，再运行同一测试通过。成功查询仍表示接口存在，AccessDenied/其他错误仍不接受为已撤销。这是实际内核返回值证据，未创建或删除接口。

| 验证 | 本轮实际结果 |
| --- | --- |
| Windows Rust | all-features 全量 159 项通过，全目标全特性 Clippy 无警告；真实缺失接口 query 的失败复现/修复验证通过 |
| Linux Rust | 独立快照 all-features 130 项通过，全目标全特性 Clippy、release lib/bin/examples 构建通过 |
| Windows 正式产物 | 默认 features 的 Flutter x64 release、373 项 Flutter/当批 bundle FFI、rust_bridge_smoke 通过 |
| helper-inherit | 当批 release 例程与同目录 helper 构建/复制摘要检查通过；普通权限运行返回 permission，等待空 helper 关闭，不创建 TUN |
| UAC 实际请求 | --helper 确实进入 Windows consent，观察到普通 probe/consent 的运行进程；随后 ShellExecuteExW 返回用户取消（os error 1223），例程退出 1，未进入 create |
| 取消后系统状态 | Get-NetRoute 稳定列和 Get-NetAdapter 名称/索引/GUID/状态快照均与请求前一致；本轮没有重试取消的 UAC |
| FRB/源码/结构 | 14 个生成文件摘要不变，两端 235 个源码/配置摘要一致，清单为 build/helper-tun-sources.json；结构无新增位置/混合/环问题，两种 PacketIo 的同形转发是各自的适配；无自动层次声明，按主计划人工核对依赖 |

真实 TUN/ring、提升 helper 的数据与断开清理均尚未通过，因为本次 UAC 已取消；不把新增验证代码、普通权限拒绝或取消后快照当作成功路径。新的 Windows UAC 同意仍是执行特权验收的条件，后续不能自动反复重试本次取消。普通权限下仍可推进账号/Cookie/完整 SSO 等后端迁移；总体目标未缩减。Windows 高层 start_tun 和 sessionDataPlaneReady=false 保持原状态，物理捕获防回环、父进程退出/崩溃、原生停滞、OS DNS、Android、后端零 Dart、分发与真实校园验收继续保留。

缓存中的官方 CreateUnicastIpAddressEntry 文档说明 Windows 10 可按 Preferred 初始状态执行 optimistic DAD。因此探针只声明已查询 Preferred 状态并要求实际 UDP 字节，不用 Preferred 单独声称普通 DAD 已完成。无接口创建的回归不覆盖这一特权行为。回退本轮时同时撤回共用 probe/helper 模式、TunInfo Deserialize 和 probe-only 客户端断连等待、撤销状态修复/测试，再重建默认原生库和 bundle；没有用户数据迁移。

日志在 C:/Users/xiema/.cache/shuvpn/：helper-tun-windows-rust.log、helper-tun-linux-rust.log、helper-tun-windows-build.log、helper-tun-windows-flutter.log、helper-tun-permission.log、helper-tun-elevated.log、helper-tun-bridge.log。本机结果不是远端 CI 或实际校园证据。

## 第十九批：B1 密码登录与 SSO 会话基础（2026-10-12，部分完成）

在 Windows UAC 已取消后继续普通权限可完成的 B1。新增 auth/newsso、auth/cookies 和 FRB NewssoSession：Rust 持有 Cookie、服务器登录 params、两步验证用户名/密码密文、方法列表和代次。原始密码在提交前完成 RSA PKCS#1 v1.5/base64，并用 Zeroizing 清理 Rust 输入副本；公钥和移动 UA 沿用原 Dart 协议。明文密码、Cookie 值和挑战密文不进入输出 DTO。

请求从教务入口发现实际登录页，跟随至最终 SSO params；密码 POST 只在配置的 SSO origin，不自动跟随 POST redirect。HTTP 复用现有物理出口 Lease，生产只允许 HTTPS 的上海大学域与标准端口，拒绝 userinfo/fragment/外域；发现阶段可升级校园 HTTP 规范化跳转。响应逐次限制 1 MiB/15 秒、跳转最多 16 次，Cookie 有单条/条数/请求头预算；这是逐请求预算，不是整个操作的总 deadline，Lease 等待由现有出口层控制。

Cookie 使用 cookie_store 的 host-only/Domain/path/Secure/HttpOnly/expiry 语义，加锁定的静态 PSL 拒绝公共后缀，按长 path 和创建次序构造敏感请求头。保留旧空值 scoped deletion；支持合法父域共享，不把所有 Cookie 假定为仅 SSO 可见。避免使用会打印非 Secure Cookie 值的 store_response_cookies。默认库新增 cookie_store 0.22.1（关闭默认 public_suffix/serde 特性）、psl 2.1.241；直接声明已锁定的 rsa 0.9.10 和 zeroize 1.9.1，RSA 没升级。锁新增五项：cookie_store、psl、psl-types、document-features、litrs。cookie_store 最低 Rust 1.88，两端现有工具链满足。

网络结果先留在代次事务内。clear/new login 取消旧 token，旧排队方法和晚到响应不能提交；已知验证码错误可更新重试 Cookie，并保留原 params/密文/方法。回调、协议、网络失败不发布本轮 staged credentials。新密码登录的清空与 token 获取在同一 state 临界区，避免旧请求捕获到新 token。同期 legacy import 也检查提交代次，并在退出/新登录后关闭入口，防止首次导入跨 clear 恢复旧凭据。

导入接受原 auth.newsso.cookies JSON，只读 SHU_OAUTH2、path、UTC expires，绑定 newsso host-only/Secure/HttpOnly，跳过过期/坏记录。**只保证当前句柄中的入口关闭，不是跨重启持久化迁移**：Rust 凭据文件、迁移标记、磁盘注销墓碑与旧 SharedPreferences 删除尚未实现。snapshot.hasSession 仅为匹配的 Cookie 存在，不是账号/服务端有效性或业务授权。

| 验证 | 本轮实际结果 |
| --- | --- |
| Windows Rust | all-features 全量 169 项，all-targets/all-features Clippy 无警告 |
| Linux Rust | 独立快照全量 140 项，all-targets/all-features Clippy 无警告，release lib/bin/examples 构建通过 |
| 真实 HTTPS fixture | 两主机的登录跳转/最终 params、独立私钥解密 PKCS#1 v1.5 密文、Cookie 主机隔离/轮转、验证码发送/错误重试/成功消费/空目标、外域回调和跳转/循环/超大 body、TLS 不信任、过长密码均通过 |
| 取消/并发 | 退出中止网络结果、同句柄新登录可用、登录替换旧登录、活动登录拒绝 import、首次 import 与 clear 的受控线程交错、退出后晚到导入拒绝通过 |
| Cookie 单元 | host-only/父域/公共后缀/路径边界与顺序、Secure/前缀、过期/Max-Age/空值删除通过 |
| Windows Flutter/FFI | 正式 x64 release，375 项全量测试，打包库 smoke 通过；新增两个真实句柄测试覆盖构造/导入/快照/退出/再导入拒绝/本地错误，不发送学校请求 |
| Linux Flutter/FFI | 正式 x64 release，374 项全量测试及打包库 smoke 通过，同一新接口测试覆盖 |
| 静态/生成 | 两端 analyze 无问题；16 个 FRB Rust/Dart 文件在重复生成后摘要不变；format/diff check 通过 |
| 源码一致性 | 两端 245 个源码、测试、锁定配置与构建/CI 文件摘要一致，清单为 build/newsso-sources.json |
| 结构 | 新状态机/HTTP/密码/DTO 分开，Cookie 适配有独立归属，无新增位置/混合/重复/环问题；不同模块私有 Client/Response 同名属各自协议，无自动层次声明，人工按主计划核对依赖 |

Windows 一次高并发测试编译触发系统提交内存错误 1455（无法 mmap rlib），退出 101；降低 CARGO_BUILD_JOBS=1 后完整重跑成功。没有改系统分页配置，没有弱化测试或删除缓存。它不是密码业务逻辑失败，也不把首次失败当作通过。

正式 createRustAuthSession 仍构造 Dart ShuAuthSession，本批没有替换生产登录页面、持久化、QR、业务授权与账户中心，没有删除旧后端或让新旧 Cookie 数据库长期共存作为完成方案。新 API 是可调用的基础，网络 fixture 不证明真实学校协议或校园凭据成功；FFI 验证范围为内存状态/输入边界，未在 FFI 中用真实校园账号登录。完整 B1/B2、Rust 跨重启存储、QR/业务 OAuth/aTrust/OTP/教务共用会话、正式前端接线与旧实现删除仍是下一阶段；Windows TUN、Android、OS DNS、分发及长期验收要求保持。

回退本批须一并撤回 Newsso API/状态机/Cookie 适配、Cargo/锁定依赖、16 文件中的新增/修改绑定与 native 测试，再重建两端库和 bundle；没有写入真实用户凭据或修改旧 SharedPreferences 数据。日志：newsso-windows-rust.log、newsso-windows-analyze.log、newsso-windows-build.log、newsso-windows-flutter.log、newsso-linux-final.log、newsso-bridge.log、newsso-bridge-repeat.log、newsso-structure.log，位于 C:/Users/xiema/.cache/shuvpn/。

## 第二十批：B1 Rust 凭据持久化与跨进程注销标记（2026-10-12，部分完成）

新增 NewssoSession.open(directory) 与 close。open 使用专属绝对路径目录，保存/恢复 newsso-state.bin；newInstance 仍为内存句柄。close 取消当前工作、释放目录锁并关闭句柄，保留已提交凭据；clear 为注销，先取消旧代次并清内存，再提交空凭据与关闭旧数据入口。只有成功返回的 clear 才证明注销记录已提交，写盘失败返回 ssoStorage，调用方不能把 snapshot 内存为空当作跨重启注销成功。

Record 同时持有版本、SSO origin、legacyImported/importOpen 标记与仅 SHU_OAUTH2 的 Cookie 记录，导入先提交再发布内存状态。重开目录恢复标记，不能再次导入旧偏好；新登录开始也先提交空会话并关闭入口，避免账户切换恢复旧 token。当前目录唯一 writer 通过 File.try_lock 排他持有；显式 unlock 仅由创建进程执行。读写复核目录和锁文件身份，Linux 拒绝最终符号链接/硬链接/错误属主或开放权限；Windows 拒绝重解析文件并保留不共享删除的目录句柄。

Windows 使用当前用户 DPAPI，设置 UI_FORBIDDEN，不请求 UAC；Linux 为 0700 目录和 0600 文件，不声称加密。临时文件同步后在同目录原子替换，Linux 同步目录，Windows 使用 MoveFileExW REPLACE_EXISTING/WRITE_THROUGH。文件有 256 KiB、版本、来源、字段和 Cookie 范围限制；损坏/过大/未来版本、错误 origin 或 Cookie 范围均报错，不自动回到 fresh import。未提交 tmp 不成为读取来源。

持久化记录保留 host-only/Domain、有效 path、Secure/HttpOnly/SameSite 和绝对到期时刻；移除 Max-Age 后写 Expires，重启不延长 TTL。无过期时间的 SHU 会话 Cookie 继续保存，兼容原 persistSession 省略 expires 的行为。只保存会话 Cookie，不保存业务 Cookie、密码、params 或两步验证挑战。未完成挑战写空凭据，重开后必须重新登录。序列化和 DPAPI 复制缓冲区清理，输出 DTO 仍没有 Cookie 值。

| 验证 | 本轮实际结果 |
| --- | --- |
| Windows Rust | all-features 全量 181 项通过，全目标全特性 Clippy 无警告 |
| Linux Rust | 独立快照全量 154 项通过，含 symlink/目录替换/锁替换拒绝；Clippy 与 release lib/bin/examples 通过 |
| Cookie/文件 | scope/属性/绝对 expiry 保留、过期跳过/错误范围拒绝、重复替换、排他锁、损坏/过大/未来版本/残留 tmp、失败前旧记录保持与重试通过 |
| 会话/故障 | 导入→重开→禁用再导入、注销→重开、首次注销标记、写失败不发布 import、失败注销明确报错/重试、代次溢出无提交、HTTPS 登录/MFA 成功恢复、MFA 写失败保留挑战、挑战不落盘、晚到响应不恢复注销通过 |
| Windows Flutter/FFI | 正式 x64 release、377 项全量测试、打包库 smoke 通过；新增两个真实库测试验证 persistent open/排他/close/reopen/clear/迁移状态及损坏拒绝 |
| Linux Flutter/FFI | 正式 x64 release、376 项全量测试、打包库 smoke 通过，同一新接口测试覆盖 |
| 跨进程 | 两端 tool/newsso_storage_smoke.dart 通过：四个独立 Dart/Rust 进程依次 import/restore/logout/logged-out，每步同步完成后 exit(0)，证明使用磁盘和 OS 释放的锁，不复用同一对象 |
| 静态/生成 | 两端 analyze 无问题，16 个生成文件重复生成摘要不变；format/diff check 通过 |
| 源码一致性 | 两端 249 个源码、测试、锁定配置、构建/CI 与持久化探针文件摘要一致，清单为 build/newsso-storage-sources.json |

三个只读子调查分别定位旧 Dart 数据 owner/目录传递、文件与 DPAPI实现、会话事务顺序，主线程修复并验证：严格磁盘 Cookie 恢复、敏感 Linux复制缓冲区、锁身份/创建 PID、溢出提交顺序。结构无新位置/混合/环问题，私有 Store/Record 等同名按各自 actor 拥有；仓库没有自动层次声明，按主计划人工核对依赖。

证明边界：写失败测试分别覆盖替换前拒绝和调用方错误处理；目录 sync 在 replace 之后失败可能已经改变目标，不声称所有错误都保持旧文件。MoveFileExW 标志和普通退出测试不证明系统断电/磁盘故障时的 Windows crash durability。路径身份检查与真正文件操作不是同一个原子的目录相对操作，不保证抵御有权限改目录的同用户竞态。主动删除整个凭据目录会删除迁移标记，不能据此仍声称存在墓碑；前端删除旧偏好和完整 reset 协调尚未接通。

正式 auth factory 仍同步创建旧 ShuAuthSession，AccountCenter 五处 persistSession 和旧 restore/clear owner 仍执行 Dart；本轮没有自动读取/删除真实用户偏好。实际前端迁移必须改为异步初始化 Native 账号句柄，在 open 的 legacyImportAllowed 为 true 时导入、成功提交后删除 auth.newsso.cookies，同时停止旧持久化再写回。不能只追加一次键删除，不能把 persistent=true 当作服务器账号有效。QR、共用业务授权/aTrust/OTP/教务、正式接线、旧后端删除、Android、Windows TUN、OS DNS/分发/校园验收继续保留。

CI 两端 bundle 阶段已加入跨进程探针，但远端 CI 未执行。回退时撤回持久化模块、Cookie 记录转换、open/close 与 snapshot 字段、Windows Cryptography 特性、对应 FRB/测试/CLI/CI，再重建两端。若已有正式用户产生 newsso-state.bin，须先安排凭据兼容性与迁移状态处理，不能靠删除墓碑完成回退；当前仅测试临时目录，没有真实凭据迁移。

日志位于 C:/Users/xiema/.cache/shuvpn/：newsso-storage-windows-final.log、newsso-storage-linux-final.log、newsso-storage-windows-analyze.log、newsso-storage-windows-build.log、newsso-storage-windows-flutter.log、newsso-storage-windows-process.log、newsso-storage-bridge.log、newsso-storage-bridge-repeat.log。工具命令：`dart run tool/newsso_storage_smoke.dart <本平台正式原生库>`。

## 第二十一批：B1 企业微信 QR 状态机（2026-10-12，部分完成）

新增 Rust QR 创建、图片请求、单次/循环轮询与扫码换码，全部归属同一个 NewssoSession。startQr 返回图片字节、confirmUrl 和 wxwork scheme；pollQr/waitQr 返回 Waiting/ConfirmedPending/Succeeded/Expired，auth_code、二维码 key 和期限保留在 Rust，redeemQr 不接收外部授权码。cancelQr 使用同一账号 clear/代次取消路径。正式 UI 尚未替换 Image.network、Dart authCode/redeem 与 AccountCenter cookie adoption，未把两个独立会话拼成完成方案。

生产企微请求仅允许 open.work.weixin.qq.com 的 HTTPS origin 与对应 qrConnect、qrImg、长轮询精确路径；禁止自动跟随跳转，Referer/Origin、AJAX/Accept、appid/agentid/redirect_uri/lang/version/login_type 沿用原协议。密码/SSO callback 的校园来源检查没有放宽到企微。state 为教务注册信息的 base64url 无 padding；二维码 key 有字符/长度检查，图片字节仅接收受限大小的常见 raster 签名。JSONP 只解析指定 callback 包裹内的 JSON，不执行脚本。

等待最多 180 秒，poll 请求最多 45 秒且受 QR deadline 约束；手机确认没有 auth_code 时仍为 ConfirmedPending。网络超时可返回 Waiting 重试，取消/到期不采纳晚到确认。复核发现独立 poll 与 wait 超时的同代覆盖竞态，先写受控调度测试实际失败为“late independent poll republished auth code”，再在超时分支递增代次/取消旧 token，原测试通过；不会把单个 wait 的丢弃等同于其他 poll 已失效。

redeem 之前在共享状态取走授权码并标为已失效，网络、协议和写盘失败不再放回同一码。响应必须是有受信校园 Location 的 3xx，且 Cookie jar 中实际存在对 SSO authorize 可匹配的 SHU_OAUTH2；空值、过期、错误域/路径均不算成功。provider Cookie 只用于企微，不能进入 SSO host-only 请求；成功时清除 QR/挑战并按已有 store 只持久化 SSO Cookie。不完整 QR/key/code 不进入 Record；hasSession 仍只表示本地 Cookie 存在，不证明校园业务授权。

验证：Rust HTTPS fixture 覆盖注册 state、二维码页/PNG 字节、provider Cookie/非法单条 Cookie忽略、轮询请求头与状态、同会话换码/持久化重开、缺失 Cookie/外域回调拒绝、授权码不重放、长轮询/换码期间 clear、wait deadline 与独立 poll 竞态、缺失 key/错误图片/过期状态。新 FFI 测试只覆盖无 QR 状态错误、超时范围与 cancel，不连接真实企微；网络成功路径为 Rust 本地 fixture，非真实扫码账号证据。

| 验证 | 本轮实际结果 |
| --- | --- |
| Rust 全量 | Windows 188 项、Linux 161 项通过，全目标全特性 Clippy 无警告 |
| 正式 Flutter/FFI | 两端 x64 release；Windows 378、Linux 377 项测试通过，analyze 无问题，真实打包库 smoke 通过 |
| 存储回归 | 两端四进程导入/恢复/注销/旧数据抑制探针继续通过 |
| 生成与结构 | 16 个绑定文件重复生成摘要不变，format/diff check 通过；QR 模块无新增位置/混合/重复/环问题，按主计划人工核对依赖 |
| 源码一致性 | 两端 252 个源码、测试、锁定配置和构建/CI 文件摘要一致，清单为 build/newsso-qr-sources.json |

正式入口、QR UI 所有权与业务 OAuth/aTrust/OTP/教务共用会话仍需一起接通，旧 Dart 服务/SDK 尚未删除。当前 QR Succeeded 意味着收到手机确认并有内部授权码，尚未代表换到账号或业务凭据；未来 UI 应按这些阶段展示。wait 状态可由 snapshot 查询，不新增长期后台 Dart 网络轮询或保留两侧 auth_code 所有权。真实学校/企微、Android、Windows TUN、OS DNS、安装分发与长期验收仍保留。

回退本批时同时撤回 qr 模块、transport 的受限 provider 入口、State/Transaction QR 字段、API/DTO 与生成绑定、测试，再重建两端默认库/bundle。持久化格式没有增加 QR 秘密字段，没有改真实用户偏好。日志：newsso-qr-windows-rust.log、newsso-qr-windows-analyze.log、newsso-qr-windows-build.log、newsso-qr-windows-flutter.log、newsso-qr-linux-final.log、newsso-qr-bridge.log、newsso-qr-bridge-repeat.log、newsso-qr-structure.log（C:/Users/xiema/.cache/shuvpn/）。

## 第二十二批：B2 同会话业务 OAuth 与原生 aTrust（2026-10-12，部分完成）

NewssoSession 新增 authorizeSystem(jwxt/otp) 和 signInAtrust(deviceId)。业务请求共用账号 jar、操作锁和取消代次，成功经原持久化提交路径才发布内存状态；失败、注销后的迟到响应不采纳业务 Cookie。持久化格式仍只保存 SSO Cookie，不保存业务 Cookie、授权码、SID 或 ticket。

JWXT 注册 client/scope/callback 固定在 Rust，每次随机 state 并验证返回值；OTP 从 bootstrap Location/HTML 发现 state，保留缓存/空值兼容和注册的 //Callback.aspx。校验精确回调 origin/path、唯一非空 code、无 userinfo/fragment；后续业务跳转限制在该系统 origin，最多十次。请求和整次操作均有预算。回执只返回去除 query/fragment 的落点与实际 HTTP 状态，HTTP 成功还须符合系统标记。OTP Default.aspx 落点不是实际数字码，教务授权也不依赖可选档案成功。

aTrust 复用现有 authConfig、OAuth 换 ticket、reportEnv、authCheck 和 onlineInfo，回调也拒绝重复 code/外域/错误路径。新路径将完整账号 jar 直接交给原生运输层；旧协议字段需要的两个名值快照从同一 jar 派生，不用于独立 Set-Cookie 合并。复核发现先复制两套旧 Cookie 容器再合并会恢复 stale 父域 Cookie并丢失属性，已撤掉该方案；HTTPS fixture 验证网关轮换→SSO 看到新值→网关删除→后续请求及最终 jar 均不再有旧值。aTrust 回执用户名来自 onlineInfo；该链未暴露最终响应状态，因此 httpStatus 为 null，不伪造 200。

新增七项本地 HTTPS 测试覆盖随机 state、中间跳 Cookie、OTP 双斜线/Location/HTML/cache/空 state/Default followup、错误 state/path/登录页/外域/循环/落点失败、原生票据/CSRF/环境链、父域轮换删除、重复码提前拒绝、注销晚到结果与无会话/无效输入不发请求。真实 FFI 新测试覆盖 authorizeSystem/signInAtrust 的本地输入和会话边界，不连接真实校园。

| 验证 | 本轮实际结果 |
| --- | --- |
| Rust 全量 | Windows 195、Linux 168 项通过，两端全目标全特性 Clippy 无警告 |
| Flutter/产物 | 两端正式 x64 release；Windows 379、Linux 378 项全量测试通过，analyze 无问题 |
| 真实库 | 两端正式打包库桥接 smoke、四进程导入/恢复/注销/迁移标记探针通过 |
| 生成/源码 | 16 个 FRB 文件重复生成摘要稳定，两端 255 个源码/测试/锁定构建配置摘要一致 |

Linux 首轮 Flutter 命令错误设置了未使用的 SHUVPN_RUST_LIBRARY_DIR，实际加载旧默认库，FRB 检出 Dart/Rust 内容摘要不一致。该轮不作为通过证据；中断后改用 FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR 指向本批正式 bundle，完整重跑 378 项通过，日志为 newsso-oauth-linux-flutter-final.log。没有关闭内容摘要校验或调整测试断言。

结构扫描 OAuth 模块无新增位置/混合/重复/环问题；按主计划人工核对 auth 内部依赖。正式 auth factory、AccountCenter、扫码页和业务 owner 尚未切换，旧 Dart/SDK 未删除。下一步完成 OTP 实际取码/解析/网络重试、教务档案/课表身份，再统一接线和迁移旧偏好。aTrust onlineInfo 不等于实际隧道已验证；没有真实校园账号或 Android 运行证据，Windows TUN 特权、高层、OS DNS、分发及长期验收继续保留。

回退须一起撤回 OAuth 模块、State/Transaction state 缓存、aTrust owned jar 与 callback 校验、API/DTO、FRB 和新增测试，重新构建两端库/bundle。没有读取/删除真实用户旧偏好或发起 UAC。日志前缀 newsso-oauth-，目录 C:/Users/xiema/.cache/shuvpn/；源码清单 build/newsso-oauth-sources.json。

## 第二十三批：B2 OTP 实际取码与教务账户资料（2026-10-12，部分完成）

新增 NewssoSession.fetchOtp 和 fetchJwxtAccount。两个操作共用账号 jar/操作锁/代次，要求当前 SSO 会话，clear/close 取消队列及在途结果，提交仍检查代次并经过原持久化路径。数字码、档案及课表数据不写入 newsso-state.bin，也不记录原始页面或账号值。

OTP 先 GET Default.aspx，再尝试根页，逐跳保存 Cookie 后再请求下一跳；只接受同 OTP origin，识别回到 SSO 登录页，最多五跳。每跳带随机防缓存查询串及 Cache-Control/Pragma no-cache，请求十五秒、整次九十秒。network/timeout/5xx 最多三次，退避 250/500ms；只有 otpSessionExpired/otpParseFailed 才执行一次同会话 OAuth 后再读取，不因网络错误反复授权。没有真实数字码不能返回成功。

OTP 解析保留原有文本/嵌套标签、nbsp/numeric nbsp、value fallback、可选 account、script remainingSeconds/totalSeconds 及默认三十秒，Refresh 正整数优先。新增 ASCII 数字边界：旧文本正则可能将九位数字截成八位，旧 Refresh 也会截断超过五位的整数；新实现拒绝/忽略这些超长值。输入按 lossy UTF-8 处理，不在本地计算 TOTP。DTO 返回 code/account/remainingSeconds/periodSeconds，不把 OAuth 页面作为数字码证明。

教务数据接口在业务授权后调用。先以无 AJAX 头的 GET 取档案片段，再 GET 课表查询页取实际 xnm/xqm，POST URL 编码表单并只在此请求添加 X-Requested-With。缺失任一学期字段不提交空值。档案姓名、学院、年级、专业可选，角色“学生”不充当学号；课表 JSON xsxx 的字符串 XM/XH/BJMC 覆盖姓名/学号并补班级，数值 XH 不隐式转为身份。档案/课表分别限时十五/三十秒，失败各自降级，不撤销已经完成的业务授权。

独立复核发现 optional 降级可能将失败响应的 Cookie 或 warmup→POST 失败的部分 Cookie 作为成功提交。已给每个资料子请求及 OTP 重试保存 jar 基线，失败/缺失结果回滚；成功档案与成功身份分别采纳，避免通过返回空资料绕过事务规则。HTTPS fixture 包含失败档案 503 设置 JSESSIONID、失败 OTP 503 设置 Cookie、课表 POST 503 与缺失学期，验证失败 Cookie 和未完成 warmup 不进入最终 jar。

九项新增 Rust 测试覆盖 OTP 解析/时间/边界/非法 UTF-8，教务档案/角色/学期/身份，实际取码、网络重试与不重授权、会话失效后一次授权、无数字码/外域拒绝、真实 warmup/表单/合并、可选降级及 Cookie 回滚、取码/身份 POST 中注销。新增真实 FFI 测试验证两接口的无会话及 close 输入边界，不连接真实学校。

| 验证 | 本轮实际结果 |
| --- | --- |
| Rust 全量 | 最终源码 Windows 204、Linux 177 项通过，两端全目标全特性 Clippy 无警告 |
| Flutter/产物 | 两端正式 x64 release；Windows 380、Linux 379 项全量测试通过，analyze 无问题 |
| 真实库 | 两端本批打包库桥接 smoke、四进程导入/恢复/注销/迁移抑制探针通过 |
| 生成/源码 | 16 个绑定重复生成摘要稳定，259 个源码/测试/锁定构建配置两端摘要一致，format/diff check 通过 |

Windows 发布构建持续 748.8 秒，原 DLL 与 helper 编译进程持续工作后正常退出，未按观察等待重启。源码清单 build/newsso-data-sources.json，日志前缀 newsso-data-（C:/Users/xiema/.cache/shuvpn/）；Windows 最终全量为 newsso-data-windows-rust-final.log，Flutter 两端为 newsso-data-windows-flutter.log / newsso-data-linux-flutter.log。

正式 auth factory/AccountCenter 和扫码页尚未切换，旧 Dart 后台仍执行，B1/B2 未完整验收。接线还需让同一 NewssoSession 准备原生隧道，替换目前 Dart Seed/同步 Cookie 的 RustATrustAuthentication，传递应用支持目录并导入后停止旧偏好写入；二维码展示改用字节及状态，成功 redeem 后页面销毁不能误调用 clear 退出账号。当前 credential gate 的九十秒超时只结束界面、不取消后台，后续要协调 native 操作生命周期。没有真实校园、Android、Windows TUN 高层、OS DNS 或长期运行新增证据。

回退须同时撤回 otp/jwxt 模块、transport 的 Refresh/no-cache/Form 支持、API/DTO/生成绑定与测试，再重建两端库与 bundle；不删除持久化迁移标记。本批不读取或删除真实用户旧偏好，不发起 UAC。

## 第二十四批：同会话原生连接准备、不透明计划与 aTrust 可用性验证（2026-10-12，部分完成）

新增 NewssoSession.prepareTunnel(deviceId, strictAntiMitm)，使用账号完整 jar 执行现有登录、资源请求与 anti-MITM，成功经同一持久化/代次事务才采纳 Cookie。连接标识和签名密钥在 Rust 生成；原 sessionIdentity API 复用同一个内部 identity 实现，保持旧调用兼容。资源后重新取得实际 SID；账号路径按 onlineInfo 的 Cookie 域/路径/次序选择 sid 或 sid-legacy，无关路径的 sid 不再遮蔽合法凭证。

返回 PreparedATrustTunnel 不透明句柄，展示只读 warnings。计划 JSON、SID、签名密钥和 Cookie 没有新路径的 FFI 值字段；VpnConnectAttempt.connectPrepared 在 Rust 消费计划并建立连接。计划最多使用一次，输入 timeout 校验在消费前执行；消费后连接失败/取消也不能重用，须重新准备。计划绑定账号取消代次，换账号、注销、close/drop 使旧计划失效，连接中同时监控 attempt 与账号取消。

已建立的连接也绑定账号有效性：账号注销/close 触发核心的显式停止和既有清理流程，连接结束则释放监控任务。最初回归在 socket 已关闭但核心尚未发布清理后状态时读取到 Connected；测试现在等待状态完成，账号停止与 Engine.stop 复用 request_stop，仍严格要求最终 Disconnected，保留清理失败分支。

新增 verifyAtrust 在同会话准备后实际建立原生 L3 控制连接、取得真实 VIP/DNS，再等待 stop 完成才返回可用性 DTO。它用于替换现有 credential_service.exchangeATrust 的 Dart/SDK 临时隧道验证，单次总预算九十秒、控制握手十五秒，输出只有 VIP/DNS/warnings。没有将 onlineInfo 或资源计划直接当作完整业务可用性证明。

七项 Rust 回归使用真实 HTTPS/TLS fixture，覆盖资源更新 SID/作用域 fallback、随机连接标识与密钥、不透明计划单次使用、签名严格拒绝/兼容保留 pin、失败资源不发布 Cookie、真实计划→L3 VIP、资源中注销、握手中 attempt/账号取消、旧计划失效、建立后注销/close 的实际 socket 关闭，以及临时验证关闭后返回。新增真实 FFI 测试只验证准备/验证入口的无会话、设备号和 close 边界，不用真实校园账号建立连接。

| 验证 | 本轮实际结果 |
| --- | --- |
| Rust 全量 | Windows 211、Linux 184 项通过，全目标全特性 Clippy 无警告 |
| Flutter/FFI | 两端 analyze 无问题；Windows 381、Linux 380 项全量测试通过，显式加载本批 Debug 原生库 |
| 真实库/存储 | 两端本批 Debug 库桥接 smoke、四进程导入/恢复/注销/迁移抑制探针通过 |
| 生成/源码 | 16 绑定重复生成摘要稳定，262 个源码/测试/锁定配置两端摘要一致；format/diff check 通过 |

结构扫描无新位置/混合/重复/环问题；Prepared/Client/Record/Response 的私有同名类型按各自 actor 所有，不合并不同职责。连接 identity 移入 core，auth 没有反向依赖 API，按主计划人工核对依赖。

验证使用当前 Debug 原生库；第二十三批 release bundle 保持为上一完整发布验证产物。正式账户和连接 factory 尚未改为这个不透明路径，后续与账户 owner/持久化接线一起构建和验证发布产物，不能把当前新接口标为已接生产。日志前缀 newsso-plan-（C:/Users/xiema/.cache/shuvpn/），最终 Windows 全量为 newsso-plan-windows-rust-final.log；源码清单 build/newsso-plan-sources.json。开发 Windows FFI 显式设置官方 Wintun 绝对路径，仅状态查询，不创建网卡或调用 helper UAC。

后续存储扩展需保留 version1 文件、导入关闭标记和失败注销契约；当前 Store 测试用动态 Record 构造旧数据，升级时必须增加冻结的旧 JSON/密封记录，不仅修改构造器。设备号、展示快照和 SSO 的迁移采纳需同次提交，失败不得关闭旧导入或发布部分新状态。新登录/注销应清身份但保留稳定设备号；成功 native 提交后移除旧三个键并停止旧 writer。正式账户、QR、Android、旧后端删除、Windows TUN/OS DNS/分发/真实校园及长期验收目标保持。

回退须撤回 owned prepare/SID 选择、新 identity 内部实现、opaque API/生成绑定、账号绑定监控、probe 与测试，再重建库；未改变持久化 schema，没有操作真实用户旧偏好或触发 UAC。

## 第二十五批：Rust 账户 owner 与整份旧数据采纳（2026-10-12，部分完成）

新增 AccountSession，持有同一个 Client、稳定设备号、展示状态及操作取消代次。登录/MFA/QR、三系统核对、五分钟缓存判断、进度、可用数与退出进入 Rust；新账户登录结果只返回验证方式，回调不进入其展示 DTO。账户核对调用已有 JWXT、实际 OTP 与临时 native L3 probe，单系统失败不阻止其它系统，至少一项可用才报告成功。

newsso-state.bin 增加 version2 的 device_id/account 可选字段，旧 version1 保持可读取，无扩展字段的 primitive 写入仍使用 version1。记录继承 DPAPI/私有权限/独占锁/同目录替换。采纳旧 Cookie、稳定设备号、展示快照和关闭旧入口为一次 store.save，成功才发布内存；不保存 OTP 码、SID、签名密钥、密码或旧快照的未知 payload。退出清 Cookie/身份/状态但保留设备号，close 保留已提交记录。version1 冻结 JSON/密封测试覆盖 imported closed 和从未导入但已注销两种门状态，不以新 Record 构造器代替旧格式证据。

独立复核的两个问题先复现为失败，再修复：QR 创建阶段尚无 Pending 状态时 cancel 未生效；全失败核对被缓存时间误判为成功。现在 qr_active 覆盖创建到兑换，成功兑换后的页面销毁不注销；零可用数不视为新鲜成功且继续需要核对。替换后同步错误也有注入测试：错误明确返回且不发布内存，磁盘可能已是完整新 bundle，重开按其恢复而不重新导入。该测试不证明系统断电耐久性，也不声称每种写入错误都留下旧文件。

实际 HTTPS fixture 覆盖三系统可用性、无秘密展示缓存、退出期间迟到响应、QR 创建取消/成功后销毁。真实 FFI 覆盖 bundle 重开、稳定设备保留、注销抑制、无账号/关闭边界；新增 account_storage_smoke 四独立进程验证 import/restore/logout/logged-out，不复用对象。最终验收与后续生产接线共同记录于第二十六批。

## 第二十六批：桌面正式账户与不透明连接接线（2026-10-12，桌面本地验收通过）

Windows/Linux bootstrap 异步打开 RustAccountCenter，传应用支持目录与三个旧偏好原始值。native 完整采纳成功后才删除 auth.newsso.cookies、auth.account.snapshot、atrust_device_id；删除失败释放句柄、返回启动错误，下一次使用 native 已提交记录继续清理，不重新采纳旧数据。正式 Provider 注入已准备账户，桌面不再构造旧 ShuAuthSession。设置页读取 native 脱敏设备号，避免被旧 getter 再生成写回。公共 AccountAuthentication 只提供会话可用性，旧 ShuAuthSession 仍用于 Android/旧测试分支。

RustAccountCenter 是快照/命令适配：Timer 仅刷新展示快照，无 Dart HTTP、协议重试、Cookie 或业务结果判断。扫码页 native 分支显示图片字节，wait/redeem 在账户 owner 内完成，不收授权码/原始 Cookie；创建/等待销毁调用 native 取消，成功兑换销毁不会注销。过渡页超时/销毁取消后台命令，post-frame 回调先检查 mounted。

连接通过 VpnConnectAttempt.prepareAccount 读取 native 稳定设备号并在 Rust 校验网关 origin，再消费不透明计划。attempt 取消覆盖资源准备而不退出账号；实际 TLS fixture 验证 account→prepare→L3，以及资源中取消。Dart 超时后到达的计划会释放，stop 出错时仍释放会话，启动失败释放 routing/account。

| 验证 | 本轮实际结果 |
| --- | --- |
| Rust/静态 | Windows 224、Linux 197 项全量通过，两端全目标全特性 Clippy/analyze 无问题 |
| 正式产物 | 两端 x64 release；Windows 385、Linux 384 项全量 Flutter/实际打包库测试通过 |
| 真实库/存储 | 两端打包库桥接、账户与 SSO 两种四进程存储探针通过 |
| 生成/源码 | 18 个绑定文件重复生成稳定，275 个源码/测试/锁定构建配置两端摘要一致，清单 build/account-ui-sources.json |

执行句柄 17998/39973 均正常终止、退出零；Windows release 构建 624.2 秒，保留原进程等待完成。前一轮中途改签名导致的旧 Flutter 失败结果不计入通过证据，完整新签名回归已通过。account 模块结构检查无新位置/混合/重复/环问题，私有 Snapshot/Account 同名按 actor 拥有；按主计划人工核对依赖。

Android 仍走旧 SDK/后端，通用旧类和依赖尚未删除。B3 更新/公告、B4 配置/连接权威状态、Android fd/socket protect、Windows 高层 TUN/特权验收、OS DNS、分发/真实校园及长期验收目标保持。没有迁移实际用户旧偏好、用真实校园账号登录或再次请求 UAC；本机旧数据迁移验证仅用临时目录/mock preferences。发布回退不能只删除 native 目录/标记或只恢复旧 Dart writer，须处理 version2 与已删除旧键的兼容性。

## 第二十七批在途：B3 原生更新查询基础（2026-10-12，未完成）

新增 Rust update/version、release、Client 与 FRB UpdateClient。固定 GitHub releases/latest，复用原生出口 Lease，保留 User-Agent/Accept/API-Version 请求头，整次八秒预算、1 MiB 响应上限，不自动跟随 redirect。404 无发布返回 None，其它非 200、错误 JSON/非对象、超限响应返回明确失败；close/drop 取消在途请求并关闭后续调用。

版本比较保留 v/V 前缀、忽略构建号、缺段补零、数字 core/prerelease 比较与正式版优先；资产选择只认 apk 结尾，优先 arm64-v8a→arm64→universal，再回落第一个候选，缺 APK 使用发布页。新测试将 checksum 放在 APK 前、覆盖多 ABI/大写后缀，补旧 Dart 测试未独立证明的选择顺序。

Windows 四项 Rust 定向测试通过，含实际本地 HTTP 请求头/404/403/302/JSON/超限边界与在途 close 的 socket 关闭；全目标全特性 Clippy/analyze 通过，新 Debug 库真实 FFI close-before-network 测试通过。网络成功证据为 Rust 本地 fixture，FFI 范围为本地关闭状态，不宣称真实 GitHub 最新发布或下载成功。本批尚未完成 Linux 新代码验证、正式入口切换和旧 Dart 解析/HTTP 删除，公告请求/解析/缓存也仍待迁移。

前一完整 release 验收仍为第二十六批。第二十七批新增 API 后的 source/Debug 绑定不代表旧 release bundle已更新；续作应以实际构建的库测试。日志前缀 update-native-（C:/Users/xiema/.cache/shuvpn/）。没有新增依赖、运行安装或下载更新、触及真实用户账户。

## 第二十八批：B3 公告核验与桌面适配（2026-10-12，桌面本地验收通过）

接手时工作区已经存在 Rust 公告请求/HTML 解析/缓存/FRB 和 RustAnnouncementClient，以及按 native 账户 owner 选择公告 Provider 的接线，但没有完整批次验收记录。本轮补 RustUpdateClient 薄适配和更新通道 close 契约，Provider 销毁时取消/释放通道；旧 Android 更新 HTTP 通道同时关闭在途客户端。更新界面仍按原策略仅 Android 启用，不因迁移适配而新增桌面 APK 提示。

独立只读审查后新增回归，确认三个测试失败，再修复同源重定向后的最终页面基址、HTTP→HTTPS 规范缓存键和表格内容保留。详情缓存保持规范化的原条目身份，正文图片/链接/附件依据最终响应地址解析；不可请求的分页不返回给 UI。表格增加受限 rowspan 占位、嵌套文字与图片链接（单元格继续使用行内展示 DTO），不改变顶层图片呈现。补 Flutter native 适配关闭/初始化取消和同步缓存错误转换。

| 验证 | 本批实际结果 |
| --- | --- |
| Rust/静态 | Windows239项、Linux212项全量通过，两端全目标全特性Clippy/analyze无问题 |
| 正式产物/FFI | 两端x64 release，Windows实际打包库Flutter391项、Linux390项通过；新Debug库同样通过 |
| 存储 | 两端正式库账户和SSO两套四进程探针通过，均只用临时目录假数据 |
| 源码一致性 | 322个相关源码/测试/锁定配置两端摘要一致，清单build/b3-sources.json |

正式流水线29071/75499均退出零，没有在途任务，Windows构建256.3秒。Windows首轮沙箱155项通过、6项命名管道helper超时，沙箱外完整重跑239项全通过（55767退出零），未忽略失败。Linux首轮analyze因旧.dart_tool没有登记当前锁定的freezed_annotation而失败；flutter pub get --enforce-lockfile后analyze和全量测试通过。测试使用当批库：Debug显式指向rust/target/debug；正式指向各自bundle目录，Windows正式回归清除Wintun外部路径覆盖。日志位于Windows build/b3-windows-*与Linux快照build/b3-*。本轮未更改FRB公开签名，未宣称重新生成绑定的稳定性证明。

结构检查未发现新放置/环问题；重复提示为两个独立通道的薄生命周期模板，保留各自actor边界。仓库未声明.clean架构，自动边界检查返回无声明，按主计划人工核对：前端适配只发native命令/映射展示DTO；请求、解析、缓存及取消由Rust拥有。git diff --check通过。

没有证明真实 GitHub/学校网站查询或 Android Rust 加载。新公告HTTPS成功路径通过本地证书fixture；真实FFI覆盖origin/关闭/初始化取消及错误转换，不代表真实站点访问。旧 Dart 更新/公告和解析测试仍因 Android 过渡路径而存在，B3/后端零 Dart 总目标仍未完成。

本批不修改磁盘数据格式。若回退，恢复桌面两个 Provider 和对应 native 适配版本，并配套使用同版 FRB/原生库；不要借回退删除既有 Rust 账户存储或迁移标记。

## 第二十九批：B4 原生设置 owner 与桌面接线（2026-10-12–13，桌面本地验收通过）

Rust settings 模块拥有schema v0–v4、平台/debug默认值、已知键校验、监听兼容解码、协议能力、关联代理开关、批量保存和恢复连接默认。9项旧schema测试已冻结为JSON升级向量，另含未来版本/未知字段保留向量。设置落在独立目录settings-state.json，不包含auth/TOFU/设备号；文件存在后忽略旧导入输入，损坏/未来文件格式明确失败。设置格式version1与历史settings.schema.version分开，未来schema不降级。

复用SSO已有的目录/锁身份校验、Unix权限、同目录原子替换和目录同步，将文件原语抽到platform/files.rs，SSO错误适配和DPAPI保留原actor。settings保存先验证整批、复制记录、同步/替换后才发布；替换后同步不确定则拒绝继续写入，旧内存快照不冒充新提交，关闭重开恢复完整磁盘记录。Windows设置继承应用支持目录ACL，已知设置不含SSO秘密，未知settings键为兼容而保留，未新增私有DACL保证；Linux使用0700/0600。

桌面bootstrap在Rust初始化后打开RustSettingsStore，原子采纳成功后清旧settings.*，清理失败释放owner并报启动错误，下次按native记录继续清理。旧Android及测试SettingsStore仍保留；native适配阻止旧同步setter写回偏好。所有生产设置页面改为异步命令；引导完成/重走、地址/端口重绑均等保存成功。校验通过backend接口，错误显示给用户且停止后续动作；日志/主题/协议展示只读已提交快照并通知。legacy load也改为迁移后构造日志配置，避免先读后迁移。

首轮7项Rust设置测试通过。独立审查后，导入系统代理缺HTTP、磁盘非规范值被接受及超出读取条目预算三项回归先失败后修复；导入系统代理开启时补HTTP监听，已经保存的矛盾组合明确拒绝。4项页面测试证明延迟/失败保存不提前导航/重绑，原有页面/schema定向38项通过；analyze通过。FRB新增公共DTO使用SettingsSnapshot，避免生成器将同名Snapshot与账户类型随机合并。

异步命令发送另补3项确定性延迟测试，复现原适配同时发出两个命令，而仅按修订号接收不能保证用户输入顺序。现在前端仅串行发送命令，业务校验/提交仍在Rust；首条失败的错误仍交调用方，队列继续，销毁后排队命令不发送。三个测试先失败后通过，包括失败/成功前序和销毁。随后重新构建两端前端产物并执行完整实际库回归。

| 验证 | 本批最终结果 |
| --- | --- |
| Rust/静态 | Windows249项、Linux222项全量通过，全目标全特性Clippy无警告，两端最终analyze无问题 |
| 正式产物/FFI | 两端x64 release，Windows实际打包库Flutter402项、Linux401项通过 |
| 存储 | 两端设置、账户、SSO三套四进程探针通过；排序修复前后Rust库摘要一致，探针覆盖同一最终原生库 |
| 源码一致性 | 338个相关源码/测试/锁定配置两端摘要一致，清单build/b4-sources.json |

原生库摘要：Windows `7B8CEE75F272DE8A78386ABEB4EEA4C239E4407B80A555F0C95CED8340EBFFBB`；Linux `3758BF8C968B805E9D6128DB7E2A427BAF7FD462F1865837D351CF2CEECEBCB5`。初次Debug库399/398项及三套存储通过；排序修复后的最终完整验证使用正式打包库，不能把旧计数当作最终结果。最终流水线49657/35039均退出零，没有在途任务；前序75889/66188/40178/72553也已退出零。Windows首次正式构建228.2秒，排序修复后重建73.7秒。

Windows首轮全量出现依赖rlib缺失和rustc ICE，未计为测试通过；仅清shuvpn_core编译缓存后-j2完整重跑通过。FRB按固定2.11.1生成，公共SettingsSnapshot避开同名合并，真实FFI验证两端新库；本批未另宣称重复生成稳定性。结构映射无新放置/环问题，私有Record/Store/Session按actor保留；无.clean架构声明，按主计划人工核对依赖，git diff --check通过。日志为Windows build/b4-*与Linux快照build/b4-*。

Android工具链也已重新只读定位，工程声明和本机常规位置证据写入交接。Android旧Store/schema/数据面、旧类删除、连接权威状态及完整平台/校园验收均未完成，B4整体保持未完成。

回退需要保留settings-state.json及其导入门：旧settings.*清理后，不可只恢复旧Dart writer导致默认值覆盖已迁配置；应由同版native导出已提交值或保持新owner，并配套同版FRB/库。本批不改变账户version2数据格式。

## 第三十批：Android原生打包与真实FFI/存储（2026-10-13，本地验收通过）

建立专属Android工具链缓存：Temurin17.0.20.1、SDK35/36、NDK28.2.13676358、cmdline-tools19.0、emulator37.2.12和API36 AOSP x64镜像，安装arm64/x64 Rust target。来源为Adoptium官方API和Google SDK仓库，校验归档SHA256/SHA1并由sdkmanager登记；旧jni依赖还需要SDK35。原解包与官方索引目录冲突时，先核验版本/绝对路径均在缓存，保留备份后归位。只配置进程环境与专属Gradle缓存（heap3G/worker2/in-process Kotlin），未修改全局SDK环境。

Android ffiPlugin与Cargo→generated jniLibs接线完成，ABI限定项目支持的arm64/x64，使用API24、每variant独立Cargo缓存和2jobs，JNI输出按ABI集合隔离。实际AGP9.1 API通过官方jar/javap及独立只读核验；APK本身仍作最终证据。真实arm64检查先复现6处空Backend引用匹配E0004，改为值scrutinee/显式字段借用后通过。Android系统代理能力仍明确返回不支持。

独立APK检查发现初次Rust库缺GNU build-id，显式增加SONAME/build-id/16KiB链接参数后重建。tool/verify_android_core_apk.py检查实际ELF架构、45项当前绑定导出、PT_LOAD和未压缩ZIP对齐、SONAME以及剥离前后的Cargo build-id；真实APK删库/错误ABI负控制均拒绝。检查范围为Rust库，不以此证明所有旧SDK库或完整Android数据面已验收。

模拟器首次实际加载Rust并通过健康/Unicode/关闭DTO，随后在设置open失败。查固定Rust1.96源码发现标准File.try_lock在Android返回Unsupported，旧map_err将其误报locked。新增platform/files内核flock/显式unlock，设置、SSO、规则、系统代理统一使用，保留文件/Arc生命周期及原有恢复后解锁顺序；竞争与其它IO错误分别报告。Android直接libc边沿复用锁定的0.2.190，并接UID/O_NOFOLLOW/私有权限校验。新增真实内核排他、活句柄显式unlock/relock测试；记录格式未变。

API36/x86_64隔离AVD实际运行新APK，四次独立进程2606/2669/2727/2788均通过：Unicode桥接、关闭错误DTO、旧schema/账户恢复、注销和旧导入门、设置/账户双owner排他。阶段2先验证锁仍被持有，再将两个owner保留到强杀；下一进程重开成功，证明OS释放锁。用nonce独立缓存目录和假Cookie/身份，无真实校园请求。tool/android_native_smoke.dart是独立入口，tool/run_android_native_smoke.py仅操作显式身份匹配的模拟器；生产Android仍走旧SDK。

| 验证 | 本批最终结果 |
| --- | --- |
| Android arm64 | 当前release主入口APK约32.1MB，45导出/4个16384对齐LOAD段/SONAME/Cargo build-id检查通过 |
| Android x64 | 实际APK同样通过检查，API36模拟器四次不同PID的FFI/存储/强杀恢复通过 |
| Rust/静态 | Windows250项、Linux223项全量，全目标全特性Clippy/analyze通过 |
| 桌面正式产物 | 两端x64 release，实际打包库Flutter402/401项及设置/账户/SSO三套四进程存储通过；本批Debug库同样通过 |
| 源码一致 | 379个相关源码/测试/Android构建/CI配置两端摘要一致，build/android-sources.json |

当前arm64产物为build/android/artifacts/shuvpn-arm64-release-local.apk，Rust build-id f5061ac1034daa872c468edc5935ff5005a05bc5；本地签名校验通过，证书为Android Debug，未发布。x64探针为build/android/artifacts/shuvpn-x64-native-smoke.apk，nonce见build/android-smoke-nonce.txt。ARM Debug旧包为flock修复前历史产物，当前ARM验收使用新release包。发布工作流已加入Rust target安装与实际APK检查，未运行远端CI或发布。

最终51229/92782/24173/26275及两端Debug/全量流程均退出零，没有在途构建；Windows正式重建175秒。中间SDK35缺失、build-id缺失和首次Android锁失败均保留为失败证据，没有忽略。隔离AVD测试后按身份核验显式停止，原PID126632消失/adb设备列表为空，不按旧PID文件判断仍运行；后续fd测试再启动并核验。日志build/android-*，新工具链可复用。

未改FRB公开签名，不宣称重新生成稳定性；结构/依赖按主计划人工核对，仓库无.clean架构声明，git diff --check通过。本批为原生打包、离线FFI/存储及主机回归，Android正式后台切换、TUN fd/socket protect、ARM硬件/真实校园、旧后端删除和完整平台目标继续。回退需配套移除Android插件构建接线并恢复平台锁调用，保留已提交原生存储/迁移门，不删除账户/设置目录来制造重新导入。

## 第三十一批：Android低层TUN与共享Unix包I/O（2026-10-13，本地验收通过）

抽取Linux原有非阻塞read/write、取消token、active操作租约和close等待到platform/packet_fd，Netlink创建/冲突/路由仍在linux_tun。新增真实UnixDatagram内核字节/超时/非法包、挂起read取消及取消close后重试3项测试并通过；原packet_outbounds基线7项通过。Android x64真实交叉检查通过。

Android NativeTunSession建立真实VpnService描述符，由JNI同步dup/核验TUNGETIFF/实际MTU，Java原PFD在use退出即关，Rust先持Pending OwnedFd，再经openAndroid一次消费到AsyncFd。单调令牌防重复/迟到消费；撤销直达原生取消，FD等全部操作租约释放才确认closed。未消费watchdog回收待交接fd；生产仍走旧SDK，fdIoReady与sessionDataPlaneReady/socketProtectionReady分开，后两者false。

只读审查后修复旧engine detach关闭新lease、executor泄漏、native/legacy establish预占TOCTOU及Rust close后的Java状态滞留。服务不持锁跨Binder establish，回调按epoch失效，pending与active取消均不依赖Dart事件；平台消息送主Looper。代码生成采用独立build/codegen-target，避免污染主机构建缓存。

API36/x64独立模拟器实际跑新APK：第二UID辅助Java应用产生IPv4/IPv6 UDP，包进入内核TUN→Rust读取/写回→原UDP socket收到。探针验证令牌重复消费拒绝、未消费超时回收、active read lease后平台cancel、重复close及原生drained；第二provider.prepare触发实际OS onRevoke，无Dart vpnRevoked handler仍退出挂起read，root Netlink确认接口消失。普通adb查看接口权限失败和appops deny未触发当前VPN撤销已记录并纠正测试刺激，不计为通过；root仅在显式身份匹配的debug AVD观察，应用为普通UID。最终watchdog竞态修复后29235退出零，nonce为8652d44d788b4d0fbb1ecc0b780943a6、主应用PID2105，未消费lease1回收后lease2收发/取消、lease3系统撤销均通过。日志build/android-tun-runtime.log；之前94629/36449仅作前序成功记录。

Linux227项、Windows250项Rust全量和两端全目标全特性Clippy通过；Linux计数以最终日志各组passed求和，纠正先前226的估算。Linux首次Clippy测试op_ref已修正。Linux新release/examples的三个root隔离namespace探针通过，覆盖TUN/回滚/崩溃清理、DNS动态路由与双栈TCP/UDP数据面；sudo -n无权限后以WSL root运行，未计未执行为通过。Windows新Debug库Flutter402项及三套四进程存储通过；两端新正式打包库Flutter402/401项和三套四进程存储均通过（Linux首次漏参数与随后shell变量传递失败不计通过，78264以显式完整库路径重跑通过）。Windows正式流水线48873退出零，构建423.1秒，独立root Cargo release83155也退出零。新绑定已改变，验证均使用本批新库，不能将30批bundle算作此批。

第二次只读生命周期检查指出watchdog先清Java占用、后JNI撤销的空窗。已将撤销请求放在session锁内，并且只有JNI确认全部FD/操作lease drained后才清占用；JNI只请求取消，不等待且不回调Java，未跨Binder持锁。撤销期间native/legacy建立均继续视该session为占用，后续hasLease/start按真实closed回收。修复后的Android完整探针通过，ARM正式Kotlin字节码也确认revoke/closed检查均在monitor内、closed后才写current=0。

ARM64 release主入口APK为build/android/artifacts/shuvpn-arm64-native-tun-release-local.apk（33,752,953字节），x64独立探针APK为build/android/artifacts/shuvpn-x64-tun-smoke.apk。两者实际ELF的50项必需导出（45项FRB与5项JNI）、四个16KiB LOAD段、ZIP对齐、SONAME及Cargo剥离前后build-id通过。ARM build-id ed0796527cff221351a76fa827d4cc09932089c9；x64为8ad40003f1c8791cfcd2d82beead9753c4371de3。ARM签名校验通过、证书Android Debug，未发布。首轮ARM构建成功后检查命令误用Windows Python占位别名，退出9009；改用显式NDK Python完成实际校验，不将未执行算作通过。

389个相关源码/测试/Android与CI配置两端摘要一致，清单build/android-tun-sources.json。Rust格式与git diff --check通过，Python工具编译检查通过，最终analyze无问题。结构映射未发现新放置/环问题；JNI命名/短包装重复属于ABI约定，平台Device与错误构造按actor保留，未按静态同形提示合并不同职责。仓库无.clean架构声明，依赖按工程计划人工核对。高层Android数据面、socket protect/HTTP与DNS保护、正式入口和旧类删除继续。

| 验证 | 本批最终结果 |
| --- | --- |
| Rust / 静态 | Windows250项、Linux227项；两端全目标全特性Clippy和analyze通过，Rust格式通过 |
| 桌面正式产物 | 两端x64 release，实际打包库Flutter402/401项和设置/账户/SSO三套四进程存储通过 |
| Linux真实内核 | 三组隔离探针覆盖TUN、回滚/崩溃清理、DNS动态主机路由与双栈TCP/UDP数据面 |
| Android x64 | 修复后的真实双栈双UID收发、未消费回收、pending取消/重复close、系统撤销/接口消失通过 |
| Android ARM64 | release主入口APK的50导出/16KiB/SONAME/build-id及本地签名检查通过；未执行ARM硬件测试 |
| 源码 / 现场 | 389个相关源码摘要一致；所有本批验证已退出，AVD已unroot/停止，PID132476消失且adb列表为空 |

最终正式打包原生库SHA256：Windows `2196B0809E8CA173937962CA7B62CEF873C84D0804FF99E62EB3A30C9C81CE04`；Linux `eeb574e7d67cacc50fbcb50bb331f3b56fd7b9cda18553c1fe32eeee42c296e3`。日志build/android-fd-*、build/android-tun-*；Linux日志已同步到Windows工作区build。没有运行远端发布/CI，未创建提交。

回退须配套恢复FRB绑定与原生库，不能将新增openAndroid调用与30批旧.so混用。生产Android仍走旧SDK，撤掉新增native通道时保留原有legacy通道；Linux共享包I/O回退应同时恢复Device内的取消/操作租约/close等待，不能只改回裸fd。此批不改变设置/账户记录格式或导入门，保留已迁移存储。

## 第三十二批：Android高层会话与受保护出口（2026-10-13，本地验收通过）

Android TUN 接入同一 Rust VpnSession、packet port、TCP/UDP、规则、DNS capture 与 stop owner。新增会话配置/一次性 lease 接管接口，Java Builder 输入复制为不可变列表，以 SHA256 的四个 Long 和真实底层 Network handle 同步交接；Rust 同时核验 FD 类型、实际 MTU、地址/路由/DNS 摘要。相同 MTU、不同捕获范围的负控制实际返回 identity，避免报告未安装的路由。

物理出口通过 NDK android_setsocknetwork 绑定具体 Network，核验 netId、explicitlySelected、protectedFromVpn 标记；默认解析使用该 Network 的 NDK resolver，显式上游复用受保护 UDP/TCP DNS。HTTP/HTTPS 复用 Windows 请求级连接器，保留原 TLS 验证与目标/代理凭据限制。Java 监听底层网络丢失并直达 native cancel，JNI 不等待出口 IPC；Rust JoinSet 持有 socket 撤销任务，在 blocking worker 关闭 I/O，finish 取消等待后仍可重试并 join。

多轮只读复核与主线程修复覆盖：FD drain 早于出口恢复的 Java 清理窗口、restore/shutdown 互斥、原 network handle 与新 scope 相同的恢复误关、并发 finish 消费错误后的假成功、pending cleanup 被隐藏后走默认出口、失败 startup 清理 owner 丢失。恢复成功会从原 scope weak 成员集合移除 socket；JNI closed 联合 FD drain、恢复、撤销成功与 worker 集合为空，session.stop 另外等待 TUN 主任务。Linux 真实 SO_BINDTODEVICE 恢复失败/重试测试证明 pending 阶段拒绝默认出站。

实际系统 Private DNS 探测曾把校园 DNS:853 送入 DIRECT，已增加 DNS 捕获模式的 DoT/DoQ 明确拒绝。captured_dns_tls_cannot_fall_back_to_a_direct_outbound 在旧实现失败为 network，修复后通过；真实 Java TCP/UDP 853 场景及 native 事件同时校验 dns_protocol、目标与端口。未实现的加密 DNS 不伪造域名映射或回落物理网络；普通非捕获 DNS 地址和未开启捕获的连接不受此限制。

完整模拟器复验曾在 Port.close 等待捕获回复时超时。新增 closing_packet_port_cancels_a_queued_capture_reply：暂停 route sink、确认 Acquire 入队，再关闭 port；旧版稳定超时，修复后测试通过（整项日志0.02s）。现在 TCP/UDP acquire 同时监听 port 取消，advertise/acquire/status 回复监听 capture 取消，TUN 桥接退出先取消子任务再等待 port。不得将早先偶发成功当成该缺口已不存在；最终整组使用这一修复后的源码。

最终高层探针34183已退出零：nonce a3c8aecfa3bd4a0684a05160d3d33629、普通应用PID5408、独立UID辅助应用。验证配置摘要不一致拒绝、正常stopTun恢复后原会话继续、校园IPv4 UDP与双栈TCP、DIRECT TCP/UDP、真实内核DNS后的域名双栈TCP/UDP、规则拒绝/未授权、DoT/DoQ防回落、物理DNS UDP/TCP截断重试/坏ID拒绝、默认拒绝自签与probe显式信任HTTPS、NDK resolver ABI、同handle恢复后实际UDP收发。持socket锁的故障注入证明revoke不等IPC，取消finish保留worker，释放锁后可重试join；另一provider.prepare触发真实OS撤销后，挂起HTTPS以非timeout错误结束，完整stop、FD/出口/capture清理、内核接口删除及宿主连接归零通过。宿主上传524438字节，DNS查询10/TCP重试2、HTTPS1、stalledHello/Closed各1、errors为空。

TLS fixture 最初经 ADB reverse，在半关闭后出现缺 close_notify；改物理转发仍有偶发，最终直接把 TLS gateway 绑定宿主拥有的地址，并在正常 TLS shutdown 后限时排空 TCP FIN 再 drop。保留严格 EOF 检查和每条校园TCP128KiB回包/FIN校验。辅助 Activity 的 am start -W 曾拖过 TLS 连接预算，改用测试结果标记同步；日志中普通超时、capability未启用、关闭超时及失败轮次均保留，未计为通过。失败驱动可在force-stop前采集native栈，debug trace仅在显式feature内。

debug 专属开关为子进程 SHUVPN_RUST_PROBE=egress-probe；旧混合大小写 Gradle 环境属性未启用测试代码，已修正并要求部署前确认实际 APK marker。release 任务强制关闭该 feature；检查器对当前成功 marker 做保守拒绝，负控制已明确拒绝 debug 出口 fixture APK。正常出口诊断 API 保留 capability stub，未宣称正式产物没有任何桥接健康诊断。测试根不进入正常 TLS 配置；此静态检查仍仅覆盖 Rust 库，不等同整 APK 或真实16KiB设备运行验收。

主机最终 Rust 为 Windows253/Linux231，包含新增 port-close 回归；全目标全特性 Clippy 通过。两端新正式打包库Flutter402/401项、设置/账户/SSO三套四进程存储与analyze通过；最新三个root namespace真实内核探针全部通过。默认无egress-probe的x64 APK另通过低层TUN回归66630，nonce67ddf3cef40d490da2163b6c6286778a、PID5813，覆盖双栈UDP、未消费超时、读取取消/重复close、OS撤销，静态marker确认该库没有出口fixture字面量。ARM64正式APK、50导出/16KiB/SONAME/Cargo build-id及签名检查通过。Windows正式流水线5280退出零，构建761.3秒；原生release预编译80456也退出零。394个相关源码摘要已同步且最终复核一致，build/android-session-sources.json；必须使用本批绑定和原生库。

ARM64产物为build/android/artifacts/shuvpn-arm64-session-release-local.apk（34,477,777字节），build-id `9c226e9c23ebfc5ac9e15f3048df076e870345d3`，APK SHA256 `D1206F7854D5772B0EEDB82326186733672C22C8CD2188D70C57A69E796F064D`。构建进程故意设置SHUVPN_RUST_PROBE=egress-probe，实际日志仍为release=true egressProbe=false，检查器无probe检查通过；本地签名Android Debug，未发布/未执行ARM硬件测试。x64 session probe build-id `bd7252d9f3ff79481f637a8e3e9906312310a302`，默认低层probe为`95d761b82e3138c0bcd6ddab14cc20205765f8b7`。检查器识别当前fixture marker的负控制已拒绝，不能单凭字符串缺失证明任意手工构建都没有测试实现；Gradle配置、cfg、实际构建日志和产物检查共同作为本次证据。

源码结构映射未发现network范围新放置、职责混杂或环；Lease/Record/Response跨actor同名按原命名空间保留。Rust格式、Python工具编译及git diff --check通过。模拟器已身份核验后unroot并停止，PID137352消失、adb列表为空；不按旧PID文件判断存活。最终正式库SHA256：Linux `6b94ffabb1552401799957b132a021efa0a9c4fa8aa1a13fc4f988d10221f731`；Windows `26934DD5AE7686068C423B7A0778DFB25C208D1ABC36472E8F5CC6C91F7713E4`。34183/51221/33746/36689/66630/43446/80456/5280均已明确退出零，无本批在途构建。日志build/android-session-*，Linux最终日志已同步到工作区build。

| 验证范围 | 本批最终结果 |
| --- | --- |
| Rust/静态 | Windows253、Linux231，全量及全目标全特性Clippy通过，两端analyze无问题 |
| 桌面正式库 | 两端x64 release，实际打包库Flutter402/401和设置/账户/SSO三套四进程存储通过 |
| Android高层 | 修复后的完整session真实内核、签名校园、DIRECT、域名DNS、TLS与系统撤销/完整stop通过 |
| Android默认构建 | 未启用出口fixture的独立低层双栈TUN/超时/取消/OS撤销回归通过 |
| Android ARM64 | release主入口APK、50导出/16KiB/SONAME/build-id/签名通过，携带probe环境仍强制关闭feature |
| Linux内核 | TUN、回滚/崩溃清理、DNS TTL/域名捕获及双栈TCP/UDP校园/DIRECT/REJECT三组隔离探针通过 |
| 源码与现场 | 394文件摘要一致，格式/结构/diff检查通过，AVD已停止，无本批在途任务 |

能力明确为 captureMode=preinstalled-routes、dynamicRouteUpdates/fullCaptureReady=false；动态域名只接受已经安装的路由覆盖，超界 capture_scope。当前生产 Android 仍用旧 SDK；动态route重建、正式账户/设置/更新公告与连接入口切换、旧后端删除、ARM硬件及真实校园仍未完成。回退须配套恢复 FRB/原生库及同版 JNI 参数，保留账户/设置格式与迁移门；不删除已迁移数据制造重导入。没有运行远端发布或创建提交。

## 第三十三批：Android动态域名路由（2026-10-13，在途）

Android route capture 由预装范围扩展为同一 lease/session 的 Builder 重建。Rust `platform/tun_handover` 持有可替换 FD、读写暂停门、重建任务及4MiB交接队列；取消调用方等待不丢掉 blocking worker，撤销拒绝迟到FD。JNI持有JavaVM及global class ref，Java holder在配置SHA256和session身份校验后detach，Rust立即接管OwnedFd。Builder之后等待真实ConnectivityManager LinkProperties的接口名及路由更新，再恢复I/O；失败可安装恢复FD并返回原错误，恢复失败关闭数据面。新增runId与JNI revision/interfaceIndex用于验证同一会话和真实接口切换；FRB/JNI需配套新版原生库。

实际模拟器已覆盖：初始无域名A/AAAA主机route、DNS新增双栈route、TTL到期后由活动flow保留单族route、最后flow关闭回收，以及campus/domain4/domain6同一Socket/本地端口跨增删route连续echo。66009完整流程通过（nonce `b13b7f824f6548b686cd2d701048ec77`、PID4823），还包含原32批所有数据/TLS/受保护出口、真实OS撤销pending HTTPS、最终stop约43ms、内核接口删除及宿主连接归零，证据build/android-routes-first-complete-*。但此前59422最终stop超时、随后20279校园TCP TLS提前EOF及79183单次DNS丢包均保留；仅增加诊断后通过不视为间歇性缺陷已经消失。

辅助应用的持久Socket期间保持前台，避免API36冻结后台进程清理socket干扰；没有关闭OS策略或重连Socket。另修复旧UI finish回调晚于下一case启动的竞态，终态marker改在UI清理提交后发送。驱动支持同一构建连续独立进程验证，每轮清日志/授权并按宿主增量确认TLS ClientHello、关闭、DNS TCP retry及HTTPS；完整原生日志和每轮内核快照均落盘。未增加DNS重试或放宽停止预算。

新增真实Unix内核回归模拟Builder清空旧队列，在旧实现稳定得到None丢包（36002退出101）；排空提前到Builder前并在安装时再次排空迟到数据后，6项handover测试通过（76092退出0）。覆盖队列顺序/新FD写入、长read打断、取消等待保留worker/撤销拒绝迟到FD、恢复错误可用、遗留恢复错误不吞新请求以及Builder reset旧队列。Android交叉Clippy51573退出0。修复后的三轮完整模拟器流程80543正在执行，不能提前计通过。

Linux正式流水线8363及root隔离59198退出0：正式bundle401 Flutter、实际FFI、设置/账户/SSO三套四进程存储、TUN回滚/崩溃清理、DNS TTL与双栈TCP UDP校园/DIRECT/REJECT通过；正式.so SHA256为 `16ec40c7b8e5abce1ed98de2e1623b3563099bc1bb1bf0f88d654b8fc79df500`。最新399相关文件已再次同步，最终Rust/Clippy/analyze7822及Windows Rust/Clippy28988在途。Windows正式DLL、ARM release及默认低层TUN回归仍待执行；整体目标保持active，生产Android仍走旧SDK。

生产接线定位：`bootstrap.dart`仅Windows/Linux启用Rust账户、设置和规则；`app.dart`以account注入选择Rust公告/更新，Android目前为Dart实现。`NativeConnection`尚无Android TUN生命周期接口，controller的startVpn/撤销/running仍依赖SDK `_connector.tunnel`/`_vpnDevice`；native proxy也缺foreground与通知断开接线。后续迁移需一起接配置与lease交接、native running/stop/revoke和foreground，而不是只扩大平台工厂条件。

## 第三十四批：Windows/Linux 候选包与内核回归（2026-10-14，本批本地验证通过）

用户将本次交付明确收敛为 Windows 与 Linux，Android 移出本次验收前置。主计划、后端审计及交接已记录此范围，保留原移动端代码和历史验证，不把 Android 第三十三批收尾作为桌面工作的阻塞。

新增 `tool/package_desktop.py` 对完整桌面 bundle 生成 Windows ZIP / Linux tar.gz、逐文件 SHA256 清单及整包摘要；核验 Flutter manifests、字体、全部项目图像、Windows 运行库/helper/Wintun、Linux 插件与 x64 二进制头，Windows 的 AOT `data/app.so` 按 ELF x64 检查。Windows CMake 通过 `InstallRequiredSystemLibraries` 安装 MSVC 运行库。六项打包测试通过。`tool/verify_desktop.ps1` 与 `tool/verify_desktop.sh` 已完成两端 Rust/Flutter 检查、release、归档、解压后实际库 FFI 和设置/账户/SSO四进程恢复。CI 已加入打包、三套存储、实际随包 helper 和 Linux 包内库内核探针，未运行远端 CI。

Windows `VpnSession.start_tun/stop_tun` 已接入共享包桥和动态捕获；新 `platform/windows_tun/managed.rs` 使用管理员进程本地设备或同目录提升 helper。helper IPC 由独立任务完整执行，调用方取消读取时保留已收到的包；合法关闭失败保持管道和设备供重试，断管则需确认 helper 成功退出，不能用进程消失替代清理成功。启动任务从创建接口前保存在 Shared，stop 等待其结束；Service 保留设备到清理完成，出口 activation/cleanup 使用同一 Arc Lease。关闭的最终错误进入会话结果，不能被 driver 正常退出吞掉。新增取消/关闭重试测试，Windows特权网卡与高层真实收发尚未运行，能力就绪标记尚未放开。

第一轮两端 Clippy 通过，但 Rust 全量同样复现历史遗留的 FIN 回归：Windows 184 passed/1 failed，Linux 164 passed/1 failed；日志分别在 `build/desktop/20261014-020541-windows/logs/rust-tests.log` 与 Linux快照 `build/desktop/20261014-020816-linux/logs/rust-tests.log`。smoltcp 0.14.0 在右端裁剪 payload 时仍接受 FIN，造成剩余数据未收到即进入 CloseWait。适配层把带数据的 FIN 按原序列分为数据段和独立 FIN，保留 IP/TCP options 并重算校验和，由原 TCP 栈继续判断序列与窗口；未修改依赖缓存或锁定版本。Windows四项定向测试通过，包含原失败用例、双栈序列回绕/校验与队列预算，日志 `build/desktop-tcp-window-fixed.log`。修复后的两端全量和 release 均已通过。

最终候选包位于 `build/desktop/ba4f80583397/`：

- `ShuVPN-0.4.2+9-windows-x64.zip`：21,054,862字节，SHA256 `51bcf5a6a1774661c32a3ab00103268b66a56c234f9f2dd39fa41033374a1d29`。
- `ShuVPN-0.4.2+9-linux-x64.tar.gz`：18,275,177字节，SHA256 `2930679b8d73945db9b5a02b6b96d53366b680be415a1d1c876ee1db341be096`。

两包及当前仓库的409个来源文件摘要一致：`ba4f805833978d131a8f2cd76349fb9cd0d127646cb1e6bfeb3d582a2d61d29b`，HEAD仍为 `1e055d672754971bb7ac5bbadb923858d78ae777`、dirty=true。来源排序按POSIX相对路径，避免Windows/Linux路径比较规则差异。Linux缓存快照多出的旧 `packet/direct.rs` 未被当前模块引用，已先完整备份至 `build/desktop/obsolete-linux-direct.rs` 再移除；其他来源逐文件无差异。最终归档刷新了来源清单，运行文件的逐项摘要与已经通过实际FFI/Flutter测试的原归档完全相同。`verification.json` 保存当前来源、整包摘要、测试计数及剩余验收项。

| 最终验证范围 | Windows | Linux |
| --- | --- | --- |
| Rust全量 | 266项通过 | 242项通过 |
| 全目标/全特性Clippy、格式 | 通过 | 通过 |
| Flutter全量，加载解压包内库 | 402项通过 | 401项通过 |
| 实际FFI及设置/账户/SSO四进程存储 | 通过 | 通过 |
| 随包release helper | 4项真实进程IPC/权限拒绝/退出通过，未请求UAC | 不适用 |
| 原生内核探针 | 真实Windows特权TUN尚未执行 | TUN/回滚/崩溃、DNS TTL、双栈TCP/UDP校园/DIRECT/REJECT三组通过 |
| 包内库内核FFI | 尚未执行特权路径 | 明确加载解压包内 `.so` 的包收发/关闭/会话停止、DNS动态路由两组通过 |
| 归档与来源 | 完整性、SHA256及当前来源一致 | 完整性、SHA256及当前来源一致 |

两端日志分别在 `build/desktop/20261014-070352-windows/logs/` 与 `build/desktop/20261014-070546-linux/logs/`，Linux日志已复制到本工作区。实际随包helper的补充验证见 `packaged-helper.log`；Linux五组内核记录见 `kernel-*.log`。独立Cargo和Flutter/CMake使用不同target目录，两份 `.so` 字节不同，因此没有用它们互相替代：两个Dart内核探针增加可选库路径，已实际验证解压包内库。FRB 2.11.1重新生成后摘要稳定，`dart analyze tool`无问题；结构映射已执行。项目没有声明层文件，边界脚本退出2，改用现有职责与导入关系人工复核，不能把此项写成自动边界检查通过。

Windows92380、Linux99436、Linux内核30919、后续来源刷新及核验均已结束，无在途构建。遗留测试AVD已核验为 `emulator-5580/shuvpn_native_api36`，unroot后停止，原136636/140984进程消失且adb不再列出该模拟器。没有改动已迁移账户/设置数据。

桌面正式 UI 的 TUN 开关、Rust 默认计划/物理接口选择、权限流程、关闭失败时前端句柄保留，以及剩余桌面交互和真实网络/安装验收仍待继续；没有提交、发布或标记完整目标完成。

本批网络与并发改动按 HIGH 风险验证。回退需配套恢复 `api/vpn` 的平台接线、`packet/tun` 与 Shared 的启动/清理状态、managed/helper 协议及 `network::Activation/Lease`，重新生成绑定并重建同版原生库；FIN 适配须同时恢复 codec 与 Stack，单独撤掉其中一半会破坏队列与序列契约。账户和设置存储格式未改变，保留已迁移数据。Windows ZIP/Linux tar.gz 生成与 CMake 运行库安装可独立回退；当前改动未提交，不能用清理整个未跟踪目录替代配套回退。

## 第三十五批：桌面TUN正式接线与关闭确认（2026-10-14，现阶段候选包本地验收通过）

前端连接关闭失败后保留同一owner，阻止其清理失败时启动替代连接；RustConnection只在finish确认清理后释放opaque句柄，失败Future缓存可重试。桌面退出使用能报告失败的命令，等待取消中的连接任务与迟到结果，失败保持窗口。原生TUN通过可选NativeTunConnection/NativeTunPlatform接线，Rust返回网络选择、接管计划与实际状态；设置页沿用现有样式，显示权限/读取/失败、一次启动的网络草稿、实际MTU和初始网段，桌面隐藏Android授权与Android DNS输入。代理和TUN可独立停止。

Rust的接口枚举只读，Windows读取当前接口/默认路由及接口metric、单个blocking查询占有许可且请求有3秒预算；Linux用Netlink读取up的Ethernet接口和main table普通默认路由。建议可用IPv4默认优先，metric/index确定选择，配置别名消失时明确失败。节点预览复用带出口切换read gate的DNS解析；超时后不能让getaddrinfo后台线程越过捕获切换。

桌面计划在Rust使用同一授权区间解析（IPv4/IPv6、连字符/波浪号），按网关资源去重、排除控制节点、限制MTU至网关值，域名型资源必须有获授权校园DNS，不输出SID/签名/Cookie。仍执行既有捕获范围限制，不把全网授权当成完整捕获已实现。真实34批DLL的连字符区间UDP FFI回归先失败（3秒无回包，`build/desktop-range-old-library.log`），35批早期新DLL全量通过。

新增finish_stop将原stop诊断与清理完成分开：未证明结束的worker仍失败，TUN/出口/startup owner及包任务必须结束，系统代理失败Lease保留至restore成功。错误按code/message去重累积，代理/TUN并发失败不得覆盖；公开stop消费panic后仍保留失败事实。tun_status无Service时也检查出口owner。此前相邻竞态均已加确定性测试，Windows原生5项目标及Dart连接/权限控件/退出42项通过，浅/深色375px大字号页面无布局异常；完整当前版本仍在验收。

准备Windows验收程序 `desktop_windows_tun_smoke`，Cargo受tun-probe门控制；--plan只读，--run-privileged须新的用户明确同意。程序使用本地固定证书TLS fixture、IPv4校园UDP/双栈TCP及显式上游DIRECT UDP/TCP DNS，TCP先核验prelude再比较128KiB及FIN；只打印的3条host routes，停止8秒预算，接口消失/出口/fixture连接归零必须通过。未执行提权，生产Windows可用标记未放开。实测WLAN DNS仅本地stub，计划中的1.1.1.1是拟议显式验收上游；当前生产DNS上游配置/普通Linux权限与安装架构仍须继续处理。

最终当前候选包目录 `build/desktop/d4742d6af314/`：

| 项目 | Windows x64 | Linux x64 |
| --- | --- | --- |
| Rust全量 | 274通过 | 251通过 |
| Flutter全量，加载解压包内库 | 422通过 | 421通过 |
| Clippy全部目标/特性、格式、analyze | 通过 | 通过 |
| 实际FFI及设置/账户/SSO四进程恢复 | 通过 | 通过 |
| 随包release helper | 4项非提权IPC/拒绝/退出通过 | 不适用 |
| 实际内核与包内库 | 未运行需要UAC的实际TUN路径 | 三组内核及两组包内FFI通过 |

Windows ZIP为21,170,502字节，SHA256 `03f39ba653b53f6ebf72b690d6345e11a89b56c811237f361547ea1d8891cd91`；Linux tar.gz为18,395,541字节，SHA256 `0973f5cf381630045a1f75fa5cc79e654392e182b7da0faf863023d7ba143f1f`。两端及当前仓库418个来源文件完整摘要一致：`d4742d6af314593705ba92014179886b95f39c284a9b6103e457fd6a0dacb49d`。ABI已不同于34批，必须配套本批新绑定和库。验收清单保存原生与包内测试日志来源，状态明确candidate。

原生通过后两端完整脚本在7条Dart风格提示处停下，修复后从Flutter阶段续作，未重复已通过且源码未改的原生测试。Windows原生日志 `build/desktop/20261014-113101-windows/logs`、续作日志 `20261014-114924-windows/logs`；Linux分别为 `20261014-113330-linux/logs` 与 `20261014-115237-linux/logs`，均已保存工作区build/desktop。当前正式默认TUN FFI测试在隔离namespace创建唯一dummy底层出口与默认路由，使用网关IP型资源、原生自动选择/计划/实际MTU、签名TCP/UDP和finish确认，再检查真实接口消失；新增标记 `DESKTOP_TUN_DEFAULT_PLAN_KERNEL_AND_CLEANUP_PASSED`，不是只测试原始手写plan入口。

Linux安装权限研究核验了本机glibc `2.35-0ubuntu3.15`、GLib `2.72.4-0ubuntu2.10`、GTK `3.24.33-1ubuntu2.2`。官方glibc2.35 `elf/dl-load.c:_dl_dst_substitute`限制secure executable中的$ORIGIN到可信系统路径；内核file capability可设置secureexec，GLib2.72.4 `g_check_setuid`读取AT_SECURE。GTK3.24.33 `gtk/gtkmain.c:check_setugid`本身只比UID/GID，不据此宣称GTK必然拒绝所有filecap。当前GUI bundle使用$ORIGIN/lib，不能把给GUI直接setcap作为已验证的权限方案，继续独立helper/安装架构；没有执行setcap或安装系统权限。

Windows真实提权验收仍等待新的明确同意，默认可用标记保持false。验收程序/只读计划已准备，当前系统仅本地DNS stub，测试上游须明确选择，尚未把1.1.1.1当成生产默认。普通Linux用户权限helper、生产物理DNS上游配置、安装包/远端CI/真实校园和剩余桌面规划继续，未提交或发布。

回退需同时恢复生成绑定与原生库、NativeTun接口注入与Controller状态/退出命令；关闭失败不能改回finally释放。账户、设置持久化格式未改变，不清用户数据。整体目标继续。

## 第三十六批：Windows 实际验收与 Linux 独立权限边界（2026-10-14，进行中）

Windows 探针已实际获准执行。`build/desktop/windows-tun-probe/runtime-strict-luid.log` 完整通过真实 Wintun/host routes、固定 TLS fixture 的签名 IPv4 UDP/双栈 TCP、显式 1.1.1.1 DIRECT UDP/TCP DNS、停止清理与精确 LUID 网卡消失。随后独立稳定性复测 `runtime-stability.log` 在 helper UAC 启动阶段失败，未进入创建；未将失败计为成功，生产可用标记保持 false。测试不使用真实校园凭据，也不证明校园生产运行。

此前就绪与正常取消问题已补实际系统状态屏障和确定性回归；新增本次物理 DNS 上游草稿，由原生校验并传入预览/启动，不修改系统 DNS。FRB 2.11.1 已重生成。35批候选包保留为原快照，尚未包含这些变化，不与当前新绑定混用。

Linux 新增独立 helper，GUI 保持普通 UID/capabilities 为零。helper 从私有继承 socketpair 接受受限命令，以 SCM_RIGHTS 交付 TUN fd 和保护已有 socket，复用现有设备/包校验/动态路由。创建事务、接管确认、关闭排空及双进程崩溃撤销见 [权限设计](linux_desktop_permissions.md)。独立审查发现“Device 创建后复制 fd 失败仍保留 owner”，已将其纳入创建回滚，并通过诊断二进制故障注入证明网卡消失。

`tool/tun_linux_helper_smoke.sh` 完整退出零：普通 UID 的包收发/动态路由/保护拒绝/重复关闭、生成名称、缺权限、同名接口、部分创建失败、复制 fd 失败、未接管创建、丢弃 lease、helper SIGKILL 与父进程 SIGKILL 均通过。首轮 `/proc/1/ns/net` 普通用户访问被拒绝，改由 root harness 在 unshare 前记录 namespace 基线；崩溃读测试先耗尽已排队控制包，仍保持原3秒关闭期限并追加本地 closed 状态与内核消失断言；父进程测试中的 PID 回收脚本错误已修复。原始失败日志与最终 `kernel.log` 保留，不冒充首轮成功。

三项 SCM_RIGHTS/消息边界单测、两端全特性全目标 Clippy、Windows276/Linux256 Rust全量通过。Windows当前Debug库423项Flutter、Linux正式bundle库422项Flutter及两端analyze通过；Windows本轮没有新release包。Linux CMake正式构建已将 sibling helper 安装到bundle根目录，其ldd依赖完整，GUI/helper都没有预设file capabilities。实际不带诊断故障注入的随包helper也通过普通UID正常、拒绝、取消/丢弃和两种SIGKILL场景，`release-helper-kernel.log`退出零。真实FFI和SSO/账户/设置四进程存储通过。

打包器将Linux helper纳入必需文件、ELF x64及精确0755校验，新增缺失/权限篡改/架构回归先失败后修复，9项包测试通过。最新Linux候选包在 `build/desktop/linux-helper/release-artifact/`，20,112,815字节，SHA256 `77914b2d6b5cf6e2dbdc55620791950b4ea61f46c9350f072603c49caa5eef09`；424来源文件的完整摘要 `72e24a38e40777e2fed3aa49499d79e59be3da9cd6e97cc20dd8d4da2df1b6a8` 与当前仓库一致，`verification.json`明确为partial-desktop-candidate。临时capability副本改为root所有、仅测试用户组可执行后重跑通过并重新归档，先前 `artifacts/` 仅为该权限收紧前的历史快照。原始失败日志、最终内核/构建/存储/Flutter日志均已同步共享工作区。

当前仍需生产普通用户高层 TUN/物理出口/HTTP broker 接线、固定路径安装及授权用户组、Windows 稳定性与生产启用、最新两端配套包、真实校园/长期/远端 CI 和剩余桌面迁移验收。Android 不属于本次交付前置。回退须配套还原 Linux helper/Cargo process 特性/CMake/包校验，以及 Windows/DNS 绑定与库；不清账户或设置数据。

## 第三十七批：Linux 正式会话与物理出口接入 helper（2026-10-14，后端阶段验证通过）

Linux正式startDesktopTun通过managed device选择本地权限路径或同一capability broker；在publish前准备，Shared先登记清理owner再创建TUN。已有/新建TCP和UDP、物理DNS、HTTP/HTTPS都走可登记的真实socket出口；HTTP复用受限请求级连接器，替代无法交付fd的reqwest interface分支。退出退休该出口、取消并等待在途socket创建，重新收集已登记连接，恢复后确认helper和broker worker退出。drop清理任务及失败owner保留在retirement表，可明确等待和重试。

helper私有协议升为2，Shutdown只在设备关闭确认后成功，客户端随后等待实际子进程退出。版本/权限握手失败也等待清理。共享异步注册入口补充退休状态复核，Windows追加迟到保护不能重新绑定的回归，原同步恢复语义保持。

独立审查发现取消publish后，排队的保护尚未应用，getsockopt原值会导致恢复误判。`cancelled_protect_restores_after_the_already_queued_bind`在修复前稳定失败，改成始终在同一broker队列提交原绑定并等待确认后通过。原始证据 `build/desktop/linux-session/cancelled-protect-before.log`；隔离网络/HTTP12项及生命周期7项通过。没有仅靠日志或重复执行宣称竞态消失。

普通UID65534/CapEff0的正式API内核套件完整通过：自动选择shuphys和实际MTU1400、既有控制连接保护、校园IPv4 UDP及双栈TCP、DIRECT双栈UDP/TCP（256KiB+FIN）、DNS UDP/TC-TCP回落及错误ID拒绝、HTTPS未信任证书拒绝/受信载荷、规则拒绝和授权拒绝、原连接绑定恢复、重启/finishStop、父进程仍存活时确认两轮helper PID消失。物理peer实际记录来源为192.0.2.1、2001:db8:ffff::1，无VIP泄漏。首次仅因测试providers目录不存在失败，已修正并保留 `desktop-kernel-provider-directory.log`；最终 `desktop-kernel.log`退出零，HTTPS负例的传输错误日志为预期拒绝。helper低层权限拒绝、部分创建/复制fd失败、未接管、丢弃lease、helper/父进程崩溃及显式进程退出套件也通过。

本机主netns中最小绑定lo的TCP对照也超时，新netns相同UID1000可收发；根因没有定论。新增Linux HTTP真实传输测试保持原断言/期限，在普通UID独立netns运行。正式测试入口 `tool/rust_linux_namespace_tests.sh`已在本机执行并接CI；本地verify脚本需显式 `SHUVPN_RUST_TEST_NAMESPACE=1`启用，不静默跳过测试。

最终Windows277/Linux263 Rust全量、两端Clippy和默认all-targets check通过。所有本批句柄结束、日志已同步共享工作区 `build/desktop/linux-session/`，阶段清单为 `verification-progress.json`。当前诊断Rust例程使用tun-probe/egress-probe临时helper定位；固定路径安装、默认正式库普通UID FFI及GUI可用性未验收，Linux available仍按GUI CapEff，普通用户开关未放开。本批没有把36批包改称最新包，也未将此前Flutter423/422项冒充当前库已重跑。

后续继续安装/可用性、helper异常退出后独立存活TCP/HTTP恢复与broker重启实测、Windows UAC启动诊断/稳定性/生产启用，以及当前两端分发和其余桌面迁移/校园/长期/远端CI验收。Android不作为本次前置。回退时须同时恢复网络异步入口和调用点、Linux managed/broker/retirement、协议2两端及配套原生库；保留账号与设置数据，不只删除新文件。

## 第三十八批：Linux固定路径安装、默认FFI TUN与Windows错误诊断（2026-10-14，本地候选阶段）

新增随包`linux/install.py`与菜单入口，GUI安装至`/opt/shuvpn`且无file capabilities；root:shuvpn/0750 helper固定安装至`/usr/lib/shuvpn`，仅其获得NET_ADMIN/NET_RAW。安装核验复制后的清单字节，拒绝不安全路径/链接，通过私有备份和重命名事务同步升级GUI/helper/launcher，授权失败回滚；卸载保留账户/设置及组成员。提交后的备份删除异常明确报告残留路径。9项完整性、路径、发布/授权失败回滚、卸载与清理异常回归通过；两端10项包测试通过，bundle/归档包含安装文件，仍区分脚本文本与ELF校验。

`tun_driver_info`异步核对GUI权限或正式固定路径helper的实际协议/权限握手，并等待真实子进程退出，才报告available/helperReady；诊断helper覆盖变量不能让该检测虚报安装就绪。普通用户加载最终归档的**默认release库**在隔离mount/netns中通过真实安装/授权、错误权限/NoNewPrivs/无capabilities/未授权用户拒绝、升级/卸载及三次重复检测。宿主`/etc`、安装与菜单目录均被overlay/tmpfs隔离，结束时实际核对helper进程消失及宿主账户文件摘要不变。

增强已有DNS FFI探针的`--installed-helper`模式，以普通UID/CapEff0调用startDesktopTun自动选shuegress、MTU1400；校园DNS回包/元数据、动态路由先发布、真实校园TCP收发、原始域名决策、finishStop无诊断和接口消失通过。最终包日志`build/desktop/linux-install/installed-current-package.log`。默认FFI本轮没有重复完整DIRECT双栈/HTTPS矩阵，也未代替完整GUI操作。CI已接相同安装+数据验收，本地通过`SHUVPN_LINUX_INSTALL_NAMESPACE=1`显式启用；远端CI未运行。

Windows helper启动错误在`managed.rs::helper_error`只格式化最外层anyhow上下文，现保留完整原因链；1223取消分类保持。确定性回归修复前失败、修复后全量通过。更新并运行实际UAC探针后，helper/网卡创建成功（index51），但首次校园IPv4 UDP五秒内无回包且decisions为空；finishStop、stopped、pending=false、egress撤销和精确LUID消失均确认。原始日志`windows-tun-runtime.log`保留，后续数据场景未进入，原因未确定；Windows生产标记仍false。

验证收齐：Windows278/Linux263 Rust，两端全特性全目标Clippy/analyze；Windows423 Flutter用新默认Debug库、Linux422 Flutter用实际release归档库。Windows当前release bundle另通过47项native Flutter、真实FFI与三套跨进程恢复，最终ZIP解压库健康通过。Linux最终包与受测Flutter归档所有运行文件清单一致，再次通过安装/default FFI实际TUN。FRB2.11.1的28个生成文件重复摘要稳定。首次沙箱生成器停滞及旧绑定编译失败没有计为通过，沙箱外生成完成后才运行最终全量；遗留本轮子进程已核实关闭。

当前候选包和清单在`build/desktop/85a19cb14947/`：Windows21,191,175字节/SHA`f7c400bb116b07887548df0e7f6a55d971e7cc3d52e838baae1f7bc89b51cd07`，Linux20,663,340字节/SHA`688e9412ebeadcfa0516b0b78fb19252287b6c1a92b1cb33b0fc328cc743d9b0`。433个来源文件一致，完整来源SHA`85a19cb14947ad1face911af1230470d953617b5fd4f30b5b6c2b17f3fddc213`；status为partial-desktop-candidate，非最终交付。当前尚未提交/发布，goal active。下一步为Windows首包超时定位、Linux helper故障长连接恢复、完整GUI及剩余桌面迁移、校园和长期运行。

## 第三十九批：桌面正式 TUN、故障恢复与随包库加载（2026-10-15）

Windows 正式 `startDesktopTun` 与普通用户/UAC helper 已通过校园 UDP、双栈 TCP/FIN、DIRECT 双协议 DNS、finishStop、出口恢复及精确 LUID 接口消失；新的无诊断特性默认库/Dart AOT 另通过真实 FFI 与实际 helper PID 消失。可用性改为管理员或配套 helper 就绪，正式数据面标记启用。最初失败时 helper 收到的55包均为系统组播；WARP 接口状态随后变化后对照通过，但未证明环境因果，也没有修改 WARP/Clash/防火墙。证据在 `build/desktop/desktop-ready/windows-desktop-kernel.log`、`windows-default-ffi-kernel.log` 和 `build/desktop/windows-packet-diagnosis/`。

Linux 保持普通 UID/CapEff0，在隔离网络中持有真实 native TCP 与延迟 HTTPS 响应，实际 SIGKILL helper、broker 替换、原 TCP 绑定恢复并继续收发、HTTPS 在恢复确认后释放尾部、新会话与 TUN 重启、所有 helper PID 消失及物理源地址核验通过。普通与故障日志为 `build/desktop/helper-recovery/http-{normal,crash}.log`；CI 接入同一 crash 场景。

实际安装 GUI 截图暴露 FRB 从 cwd 优先选旧开发 so 的启动失败。先用旧包稳定复现并保留错误映射，改为桌面应用显式优先加载可执行文件旁的配套库，正式应用缺库时报告完整包错误，保留内容摘要校验。新 Linux 归档真实安装/default FFI/GTK 欢迎页/无托盘 Alt+F4 退出0通过，截图和映射已核对；未把窗口出现本身当作启动成功。两端 Rust278/263、Clippy/default check，Flutter423/422、analyze 和 release 构建通过。39批阶段包 `9dab21c86e7d` 保留为下批 UI 修复前的快照。

## 第四十批：修复 Windows 显示 Android 的引导与权限入口（2026-10-15）

用户指出 Windows 界面仍显示 Android。回归使用真实桌面 controller 接线和 available=true 的原生平台替身，在修复前确实找到 Android 副标题与 VPN 描述。根因是 `vpnSupported` 已包含桌面 TUN，却仍被引导页当作 Android 授权能力使用；桌面 `vpnPrepared` 不代表这种授权，还会造成继续按钮被阻止。新增 `vpnPermissionSupported`，桌面不显示/申请 Android VPN 或通知授权，不阻止后续登录；Windows/Linux 分别解释管理员确认与安装/用户授权。设置目录副标题及关于页权限说明同步按桌面语义修正。

新增桌面可用 VPN 的登录放行回归、Windows/Linux 权限说明和目录文案用例；原 Android 授权拒绝/重试/通知可选流程继续通过。两端完整 Flutter：Windows427、Linux426，analyze 无问题，release 均重新构建。最终 Windows 测试指定实际 Release DLL/Wintun，Linux 指定新 bundle/lib。早先未指定库路径的检查选旧开发库，Windows Debug 检查缺 Wintun 定位，失败日志保留且不计通过。本次没有 Rust API/原生实现变化；新包内核心/helper/驱动摘要与39批归档相同，未重新执行特权网络矩阵。

亲自检查两端实际发布程序的欢迎/权限页截图，确认无 Android 授权清单，并核对真实随包核心加载。Windows截图 `build/desktop/desktop-ready/windows-gui-platform-permission.png`；Linux为 `linux-platform-gui/permission.png`，Linux无托盘关闭实际退出0。Windows托盘退出自动化未找到退出项，失败清理只终止本次自有进程，不能计正常退出通过。测试工具先前 DPI/子窗口查找问题已修正；旧 UIA 仅 FLUTTERVIEW 的结果没有被用作启动通过。

候选在 `build/desktop/286a92e1ea47/`，434 来源文件两端摘要一致：`286a92e1ea47299966b5ce954cd2ce51597d3f784814fa5ceaff9b30559bebf3`。Windows ZIP 21,190,793 字节，SHA `184aade8dcc1f1b5244ee84565e7099393af3ac7fc2090a8ae1a959985e96cff`；Linux tar 20,667,909 字节，SHA `440044727d8f939661dddd96c3f1b2fc253480d3320d5afb8ad1897872a65078`。两包完整性通过，`verification.json` 记录当前检查与继承的原生证据边界，状态仍 partial-desktop-candidate。未提交/发布。

剩余为规则卡片/排序/单条开关/组选择、桌面后端与旧SDK审计、Windows托盘退出、真实校园/长期运行/远端CI。原生规则仍是有序字符串数组，没有单条禁用持久模型；接下来在 Rust 提供结构化编辑与校验，保留 prepare/commit 版本事务，不将 YAML 解析挪回 Dart。整体交付目标保持 active，Android 不作为本次前置。

## 第四十一批：Rust 草稿编辑与桌面规则卡片（2026-10-15，源码/默认 Debug FFI 阶段通过）

新增纯原生草稿编辑接口，Rust 负责 YAML/JSON、规则表达式与策略组结构，Dart 只解码展示 DTO、提交编辑命令。旧字符串规则兼容，新增 `{rule, enabled}` 供停用条目持久保存。编译时仍校验停用条目与预算，再排除匹配；最终 MATCH 必须唯一、启用且固定兜底。重排限于来源分区，provider/group 描述符保留，可视化编辑转换为 JSON。程序降级到不认识对象条目的旧二进制不兼容；本轮程序的保存/重开/版本回退已验证。

规则页增加配置档卡片/重命名、前置/配置档/后置卡片的新增/编辑/删除/排序/启停、策略组与自动校园开关、最终兜底编辑。每来源每页25条，拖动及上下按钮可用，上下按钮支持跨页；大字体/明暗主题/禁用状态与分页控件已有检查。导入/原文折叠为高级编辑，修改原文使模型和预览失效。页面进入与销毁清理旧预览，异步编辑有代次保护。规则测试器增加 TCP/UDP、HTTP/SOCKS/TUN 输入；真实 FFI 以 NETWORK+IN-TYPE 规则证明选择被原生匹配使用。

编辑不会发布配置，仍须 prepare/commit；界面区分未校验草稿与通过的预览。独立只读审查指出无效规则可留在草稿与 orphan group selection：前者是纯编辑与编译事务分离的预期，已增加“无效草稿不能prepare且当前版本不变”回归，后者改为复用编译器策略组校验直接报错。真实控件/FFI证明启停、排序、策略选择、预览失效以及释放旧配置owner后同进程重开实例保存状态；不冒充独立进程正式应用重启。

两端新增真实传输回归 `disabling_a_reject_rule_requires_commit_and_keeps_existing_gateway_tunnels`：停用REJECT后的预览尚未提交时新HTTP CONNECT仍403，独立物理listener未accept；commit后DIRECT端点实际回显 `real`，原校园TLS流继续回显 `kept`，网关目的地保持原一项、记录对应版本1/2及实际出站，finishStop无诊断。此证据比纯决策结果多覆盖实际传输与旧流保持，不代表真实校园或新的特权TUN矩阵。

最终 Windows286/Linux271 Rust 全特性全量与两端全目标全特性 Clippy通过；Windows432/Linux431 Flutter 全量、两端analyze及各8项规则真实FFI通过，均使用本批配套默认Debug库。Linux Rust在普通用户隔离netns执行。FRB2.11.1重复生成28文件摘要稳定，新桥接内容摘要 `-790204183`。439来源文件两端一致，SHA `4d45f5baff54722661a35ff6d035842f1bb20bf313776bd22dd8b6c10e170364`；日志与带原始证据摘要的清单在 `build/desktop/rule-editor/verification-progress.json`。

原对象解析红回归、类型转换遗漏、生成器缺powershell路径、Windows沙箱helper通道超时，以及UI测试的动画等待/未释放owner重开失败均保留。正确设置工具路径、沙箱外运行通道测试、等待真实FFI及释放原owner后全部通过，没有延长期限来冒充修复、放宽文件锁或删除断言。

本批没有重建release或新归档。40批候选 `286a92e1ea47` 保留为包含Android文案修复、尚无本批规则编辑的快照，不与新绑定混用。后续为配套release/实际规则页GUI/完整包、连接明细与遮蔽原因、桌面后端/旧SDK、Windows托盘退出、真实校园/长期/远端CI。goal保持active，未提交/发布，Android不作本次前置。

## 第四十二批：真实发布窗口、浮层修复与成对新包（2026-10-15，本地候选阶段）

实际Windows规则页面暴露标题栏提示的 `No Overlay widget found`。DesktopFrame由router.builder包在Navigator外，旧home:DesktopFrame测试位置不同，未触发悬停。新增同结构真实mouse hover回归先失败，使用当前SDK `Overlay.wrap` 给caption自有浮层；三按钮悬停、同实例路由/主题更新和卸载清理通过，原窗口/托盘目标13项继续通过。随后两端全量Windows433/Linux432 Flutter、analyze与release重建通过；Rust/FRB未变，286/271 Rust及Clippy/实际HTTP传输沿用41批，不冒充本批重新运行。

最终两端真实release均在无账号fixture完成策略组选DIRECT、校园域名规则停用、校验预览、应用版本2、正常退出，并由另一个原生Dart进程读取保存状态和确认未登录。两端规则应用与标题栏提示截图已亲自检查，加载配套库映射已核对。Windows通过真实托盘菜单退出0，Linux无托盘Alt+F4退出0；不以截图或命中日志替代学校连接验收。

Windows退出的自动化缺口已解决：原生popup的PID/线程/MenuOwner/flags确认属于本次进程，UIA没找到退出；强类型Accessibility桥读取真实菜单并执行“退出”默认动作，要求进程真实退出0。定向按键、PowerShell COM转换和程序集转发提示的先前失败保留，未更改生产退出实现或放宽校验。

规则页自动化最初有提前正常退出（日志menu选择11，但未走harness退出段，原因不确定）、消息点击未稳定触发、物理输入坐标被遮挡以及未等待预览导致v1未提交等失败，全部保留且不计通过。最终自动鼠标操作只在坐标PID匹配时执行，原生预览可见就绪后才应用，独立读回明确断言v2/选择/启停；也实测标题栏提示，无No Overlay。原配置采用同盘GUID备份，fixture有独立标记，测试后原文件摘要逐项一致，备份已移回，没有遗留待恢复数据或真实账号登录。

新包在 `build/desktop/310b8f2f9bb1/`，439来源文件两端SHA `310b8f2f9bb1fdc491d9c3c03b900b081b048b757376c6c7ac5b404de9a59a0c`。Windows ZIP21,331,681字节/SHA `15ce818f2d56f5e1224fc99edaf40919a8eba34d7f7529808a8a91efb122b3a4`；Linux tar20,790,606字节/SHA `9b9905af379f2e04576b9653d6d163d672c6220c62f77f2e58f8d087335ce7c5`。两包完整性、解压包内真实桥接与Newsso/账户/设置三套跨进程存储通过；实际随包Windows helper4项非提权进程检查通过。Windows35/Linux28个运行文件逐项等于GUI受测bundle，当前归档证据统一在verification.json。

本轮actual GUI报告 `build/desktop/rule-editor/windows-release-gui-ready/`、`linux-release-gui-overlay/`，最终Flutter/构建日志为 `*-overlay-*.log`，包内检查为 `*-packaged-*.log`。状态partial-desktop-candidate，未提交/发布，goal active。继续连接明细/遮蔽、后端旧SDK、最终特权/安装与真实校园/长期/远端CI；Android不作本次前置。

## 第四十三批：原生连接记录、重复条件说明与桌面入口保护（2026-10-16）

桌面连接页新增“连接记录”入口，薄适配读取已有原生decisions JSON，不计算路由/DNS。展示目标、入站/网络、命中规则/版本/策略、期望动作、成功出站、错误原因和DNS来源/缓存/TTL；空decision读顶层version，解析地址不冒充最终socket peer。界面明示这是最多128条的历史快照，不是活动连接/完整流量审计或请求完成证明，不虚构时间戳/连接ID。会话变更清空，迟到响应/错误和销毁后结果被丢弃，失败刷新保留并标注旧快照。

独立检查和回归覆盖期望SHU-VPN与实际unrouted、decision:null、DNS空server/TTL、明暗大字体滚动、真实router入口/返回、新会话先完成/旧失败晚到、销毁pending及controller身份检查。真实TLS/代理FFI从新reader读取校园成功、DIRECT、规则REJECT、资源表拒绝与校园DNS缓存。早先断言过度固定拒绝文案、把RawTooltip强转IconButton、多个Scrollable未限定，以及重复点击当前连接tab误触发toggle的测试问题已修正，未放宽核心拒绝/数据隔离断言。

desktop nativeFactory存在时非aTrust提前unsupported；两个强制启用未实现协议的红回归修复前进入旧服务器URI处理，修复后阻止旧SDK路径。该保护是desktop入口约束，旧Dart/SDK代码尚未删除，不能计后端零Dart完成。

Rust新增共享重复条件诊断，只比较已启用规则的kind/修剪条件文本/no-resolve，顺序按前置→配置档→后置，保留最早前驱；忽略MATCH/停用项，不修改执行列表/预算/campus_offset。卡片获得全部shadowed_by，编译重复notices预算32+汇总；UI不承诺正则/逻辑等价或子集分析。跨来源、不同解析标记、停用前驱、域名子集不误报及数量预算回归通过，真实FFI预览/决策验证仍首条REJECT。独立只读审查未发现执行行为改变；未为JSON元数据扩展重生成未变的FRB公开签名。

最终Windows289/Linux274 Rust全量、两端Clippy，Windows454/Linux453 Flutter全量/analyze与release构建通过。两端未登录真实GUI记录入口/空状态、正常退出已检查，Windows同时复测规则v2应用/读回和原数据摘要恢复，Linux无托盘退出0。实际GUI没有学校流量，丰富记录内容由真实网络FFI+控件回归分别证明。

新候选 `build/desktop/97486cf710e0/`：443来源文件完整SHA `97486cf710e00ed28f19e68ee21588446fa65f3686a38e47c0e13f95b61ed7d8` 两端一致。Windows21,357,852字节/SHA `aefc3665dcba4f6414103dc399a3d8d31e1b59d924abbf8b2b3ef4cd9cefcdd9`；Linux20,803,514字节/SHA `97ba8e3c785e686462489bcec14905d6adc374cf03d29eb61a72a96314ff46ac`。两包完整性、归档实际库FFI Windows19/Linux18、三套跨进程存储和Windows随包helper4项非提权进程验证通过；35/28运行文件匹配GUI受测bundle。清单verification.json与日志 `build/desktop/connection-records/`保存范围。

状态partial-desktop-candidate，未提交/发布，goal active。继续桌面旧后端/SDK删除与所有权审计、最终安装与特权TUN、真实校园/长期/远端CI；Android不作本次前置。

## 第四十四批：删除混合认证桥与明确原生账户边界（2026-10-16，本地源码/构建阶段）

有界生产链核对确认桌面bootstrap使用Rust账户/设置/公告/更新和native factory，初始化失败不转旧业务。发现生产未调用的createRustAuthSession/RustATrustAuthentication仍把Cookie、设备号、raw认证plan转回Dart。已真正删除该生产文件、两个兼容接口/DTO及RustConnectionAttempt的ShuAuthSession/raw planJson分支；仅接受NativeAccountAuthentication并在初始化/代理恢复前拒绝旧对象。连接改为只传opaque账户/准备句柄，取消和prepared/attempt释放路径保留。

协议测试直接使用原生ATrustAuthClient，不保留新的生产Dart仿制adapter。OAuth顺序、SSO/gateway隔离与轮换、path优先/删除、过期cookie导出及请求过滤、并发串行、取消、resource请求体/CSRF/plan字段保留验证；设备号改由正式Native账户导入、提交后删旧键和重开持久化。边界回归先失败后通过；新增独立Flutter文件检查FRB initialized仍false，并在缺少native库目录时通过旧账户/取消2项，不把预初始化测试当成初始化顺序证明。

一次standalone Dart尝试因Flutter日志依赖dart:ui不能编译，失败保留，删除该工具后采用独立Flutter binding检查。AOT原包及新构建均没有三个旧符号名，不能据负面字符串搜索声称整项SDK清理；前后app.so大小9,307,016字节但摘要不同，记录在windows-aot-before/after.json。

最终Windows457/Linux456 Flutter全量/analyze通过，两端本地release重建；核心/helper/driver与43批归档字节相同，Rust/FRB未变，原生全量/特权未冒充重跑。443来源文件SHA `7b2410bd3efb3bec9c09c8cb384c03a58cc05a1f3c46b961d85860a974a10a93` 两端一致；镜像中的已删除文件已按明确绝对路径移除，未遗漏tar不删除旧文件的差异。清单 `build/desktop/backend-boundary/verification-progress.json`保存source-and-local-release-validated-not-archived边界，本批没有新归档/新GUI，最近归档97486仍为43批快照。

本轮没有修改真实账号/用户配置或系统网络，3名只读代理均关闭、句柄全部收取，未提交/发布，goal active。控制器内旧SDK/代理/TUN/计量、native客户端启动/清理策略、preparation.timeout与LAN地址选择仍需逐项分离/迁移；Android保留不作本次前置。最终安装/特权、校园/长期及远端CI继续。

## 第四十五批（2026-10-16）：桌面清理收尾、原生统一连接期限及新候选

修复权限界面清理遗漏的端口输入和耗时格式化导入，以及设置页首次读取桌面权限/网卡信息。端口输入通过正式native设置校验，提交成功后才重绑；非法输入、保存失败与有效端点保持回归。展示格式化从旧数据包观测模块中拆出。

认证准备与控制握手现在通过Rust connect_account在一个deadline内执行，Dart准备计时器/opaque prepared编排已删除。成功会话携带兼容警告，超时取消attempt且保留登录。真实TLS fixture、取消/超时与FFI预检通过；测试fixture的证书固定错误已通过公布真实测试证书修正，生产校验不变。

成对新候选 `build/desktop/424ec5d01b42/`：417来源文件SHA `424ec5d01b42cfb65e0b26506f21a74bd34b1da3b4bb98fd70387902afa987ca`，FRB内容734160530。Windows294/Linux273 Rust、Windows441/Linux440 Flutter全量及两端Clippy/analyze/release通过。解压包原生FFI53/52、桥接及三套跨进程存储通过；打包器10项与两包完整性通过。Linux最后仅删除cfg(test)死方法；最终Windows运行文件与此前受测归档逐项相同，来源边界在verification.json明确记录。

Linux实际新窗口验证设置、权限和引导前进，正常关闭并独立native重开未登录。WindowsGUI因其他进程遮挡目标位置而安全失败，原测试配置已完整恢复；不能标本批WindowsGUI通过。候选与原始日志见交接文档和清单，旧归档保留历史。控制器旧后台、启动/清理所有权、最终GUI/安装/特权、校园/长期/CI尚未完成，目标保持active，未提交发布。

## 第四十六批（2026-10-16）：删除控制器旧后端与独立界面状态

ConnectionController的旧SDK认证/连接、TOFU、DNS/资源路由、Dart HTTP/SOCKS、计量/采样实现已删除；六个相关生产模块与五个专属测试文件同批退役。界面状态/错误类别改为自有类型，Dock验证码旧入口和协议SDK映射移除。缺原生factory直接失败，统计直接读取native快照，正式native账户验证码界面保留。

旧测试97项中的4项纯格式化断言完整迁移，93项随退役实现移除，新增3项原生入口回归。factory创建异常卡busy及连接按钮无法取消的两项原有缺陷先红后绿；认证并发fixture用首HTTP屏障修复任务派发顺序假设，原断言保留。Rust代理测试补Proxy-Connection过滤及非法请求行400，两个平台各10项实际传输通过。

Windows351/Linux350 Flutter全量、两端analyze/release/Clippy通过；407来源文件SHA `f42a47c4b0c0efc9628202495f0534b3f6df6a7e8549d493e4735020a40a1603`一致。Rust生产及FRB未变，随包核心/helper/driver与45批字节一致；全原生和特权历史证据仅按原范围继承。Windows AOT从9,274,248降为8,962,952字节，不以体积或字符串证明整个后端迁移。

Windows实际新窗口完成规则v2保存/独立native读回、记录空页/标题提示/托盘退出0，原测试配置完整恢复，补上上批受遮挡的GUI缺口。Linux实际新窗口完成连接/设置/网络权限、正常关闭0与native guest重开；未执行本批Linux规则提交或真实校园。四名只读代理与所有句柄已收取关闭。

清单 `build/desktop/native-controller/verification-progress.json` 保存本地源码、release和GUI验证；本批无新归档，424ec旧包不含本批清理。账户/设置/更新公告的旧实现与SDK依赖、native策略所有权、最终安装/特权/校园/长期/CI仍未完成，目标active，未提交发布。

## 第四十七批（2026-10-16）：账户/公告/更新旧后台删除与SDK退出

ShuAccountCenter、14个旧认证服务、账户快照存储、公告parser/isolate/HTTP和更新版本/资产/HTTP实现已删除。账户、验证码、凭据和档案为展示接口/模型；App显式注入账户，bootstrap仅走Rust健康检查和原生初始化，扫码仅QrAccount/Image.memory。密码/MFA/扫码成功取消等5项UI回归与五字段真实native迁移重开通过。三个Flutter SDK及其旧依赖/override已移除，两端实际归档不含旧插件；Rust继续使用选定的协议crate。

真实FFI红回归复现ready owner关闭后仍派发导致DroppableDisposedException；adapter在await后检查_live，并以finally保证释放。typed fake只用于映射/注入owner异常安全，不作为真实网络证据；13项目标回归通过。Rust新增公告边界和缺失release tag向量。旧实现专属测试退役、展示模型保留，数量变化清楚记录于交接文档。

最新候选0339059010a0包含46批清理：395来源文件SHA `0339059010a09aeb9bfc78b5d21bd95e1da6e2110cf8803a542966370289716b`两端一致。Windows296/Linux275 Rust全量、267/266 Flutter全量、Clippy/analyze/release通过；解压库55/54 Native FFI、桥接、三套跨进程存储、Windows随包helper4项、打包/安装事务19项与归档完整性通过。Rust生产与FRB签名未变，Windows核心/helper二进制摘要变化按新包实测记录；不声称本批特权矩阵通过。

两端当前实际窗口均通过对应未登录流程；Windows规则v2/读回/托盘退出与原测试配置恢复，Linuxguest账户/无托盘关闭/重开。GUI运行文件逐项匹配归档，截图已检查。清单 `build/desktop/0339059010a0/verification.json` 状态partial-desktop-candidate。设置旧后台、剩余策略所有权与最终安装/特权/校园/长期/CI未完成，目标active，未提交发布。

## 第四十八批（2026-10-16）：设置旧后台与地址解析退出

SettingsStore从455行实现变为75行展示接口，Dart偏好读写/schema/defaults/validator及旧settings_schema、presets已删除。同步写拒绝，原生save/restore队列与提交后通知保留；fromNative移除prefs参数，账户迁移直接收原启动preferences。代理地址模型只保留native规范化结果与展示属性。

旧迁移/解析测试退役，UI用简单内存stub，真实native默认值、监听规范化/拒绝、端口65535重开、恢复范围及非设置键保护得到加强；fake只证明UI提交与顺序，不替代后台验证。Rust新装/旧版本welcome、未来schema重开、依赖edit顺序3项加入，两端settings模块13项通过。

Windows250/Linux249 Flutter全量、analyze/release/Clippy通过，393来源文件SHA `4882577be236893de3a879d5e39b67eb4da3beb81b866da51ec95c5e9e8567c5`一致。两端实际新窗口对应流程通过；Windows规则v2/读回/托盘退出与原配置恢复、Linux网络设置/无托盘关闭与guest重开均有本批证据。原生公开签名未变，不冒充重跑全原生或特权矩阵。

清单 `build/desktop/native-settings/verification-progress.json`保存源码/本地release/GUI证据，本批无新归档。native策略所有权与最终归档/安装/特权/校园/长期/CI仍未完成，目标active，未提交发布。

## 第四十九批（2026-10-16）：验证码发送策略归原生账户

Rust Account统一拥有默认验证方式、每种方式60秒发送预留和senderror替代选择。Flutter只展示原生倒计时和提交选择，删除本地递减/默认方式/错误切换逻辑。预留在请求发出前建立，失败或取消后仍保留，切换和同owner重新登录不绕过；不跨进程持久化。已在Rust的QR/凭据超时和OTP重试经核验移出旧Dart待办，UI动画/状态采样仍保留。

新增5项原生单元/本地HTTPS测试验证期限、实际请求数、重登录、错误替代、取消和并发选择；界面新增原生快照倒计时与凭据失败返回两项回归，后者先复现失败再修复。真实FFI验证新增方法无挑战边界，正向MFA网络证据来自Rust HTTPS夹具，不能当作真实学校登录。

Windows305/Linux284 Rust全量、两端Clippy/analyze/release、Windows259/Linux258 Flutter全量通过。Linux原生使用现有隔离网络入口，宿主超时与错误FFI目录的失败日志保留，修正环境后通过。两端源清单396文件一致，SHA `18b91af88fee9a73e12ebf953495e8a652e9ce5ef9ad37db3a6d9215939a3545`；FRB内容1236410258。

最新成对归档为 `build/desktop/18b91af88fee/`，包括48批设置清理。解压后Native FFI58/57、桥接及三套跨进程存储通过；Windows随包helper4项普通权限测试、Windows打包10项和Linux打包/安装事务19项、两包完整性通过。清单为partial-desktop-candidate，未提交发布。本批没有实际GUI、安装或特权TUN复验。剩余服务启动/清理、输入/LAN所有权、最终GUI/安装/特权/校园/长期/CI仍未完成。

## 第五十批（2026-10-16）：原生网关与代理端点、停止竞态

网关原始输入由Rust校验/HTTPS规范化；代理地址提示由Windows/Linux原生单播枚举和同族网卡选择产生，HTTP/SOCKS各自返回端点、端口与诊断，IPv6保持方括号。Controller删除dart:io、旧URI解析、网卡枚举与共享LAN缓存。HTTP stop只发一次native命令，由原生负责先恢复系统代理再停止；SOCKS停止失败保留状态。

6项停止期间重开/迟到结果污染新owner回归先失败后通过，目标控制器/权限测试50项通过。两端新增通配双栈监听实测完成HTTP/SOCKS响应和停止端口拒绝，Linux另在隔离dummy网卡验证指定IPv4/IPv6提示；不宣称外部设备穿透防火墙。原生网关正向链路使用本地HTTPS账户fixture，FFI补非法输入拒绝。

Windows309/Linux288 Rust全量、268/267 Flutter全量及两端Clippy/analyze/release通过。397来源文件SHA `f12d997bc8ec12169fa7026ce456114994b216a570f189c4f4471b7692cfdc5d`一致，成对归档`build/desktop/f12d997bc8ec/`。解压库FFI58/57、桥接和三套跨进程存储、Windows helper普通权限4项、两包完整性通过。FRB ProxyBinding字段变化已生成新编解码，内置函数名称hash仍为1236410258，不能作为旧库兼容证明。

Linux新release欢迎页/随包so/无托盘正常退出通过并查看截图；没有本批Windows GUI、实际安装或特权TUN重验。当前候选仍partial-desktop-candidate，未提交发布。连接启动/失败回收等剩余Dart编排尚未迁完，原计划最终能力与验收要求继续保留。

## 第五十一批（2026-10-16）：原生服务启动、恢复与 Linux 实际安装链路

connect_desktop_account在原生恢复后建立账户连接，并在Engine创建时持有正式规则owner；不再由Dart恢复或建立后补挂规则。统一start_desktop_services持有自动服务顺序与常规部分失败策略，查询真实HTTP/SOCKS/系统代理状态；TUN权限、启动回收与停止状态也由Rust返回。Dart改为请求/展示适配，继续保留尚未迁走的会话采纳、替换及迟到owner收尾。

新增4项Rust实际TLS/监听测试、首请求规则注入和损坏恢复记录真实FFI；修复启动过早空闲、失败诊断被迟到cancel覆盖、TUN清理重试仍被旧错误阻挡及部分失败在连接页隐藏。红回归及首次FRB可选借用参数编译失败日志保留，改为强制原生规则owner后重新生成，未手改生成绑定。

Windows313/Linux292 Rust全量、272/271 Flutter全量、两端Clippy/analyze/release、实际解压库59/58 FFI、桥接及三套存储通过。Linux已在私有mount/net namespace使用新统一入口完成普通用户helper、真实内核DNS/动态域名/TCP往返、HTTP/SOCKS监听与会话清理；实际安装/升级/卸载及拒绝矩阵、宿主账户SHA保持通过。GNOME/dconf与KDE5私有环境的新统一系统代理启动、真实HTTP、HTTP停止恢复及独立SOCKS响应通过，原恢复/SIGKILL/KIO矩阵保持。

最终成对候选`build/desktop/0c5456adbf6e/`，401来源文件SHA `0c5456adbf6e81618fa9997c7b92d15b1f68a475f9b2819930fc8d4125dcd714`两端一致，FRB内容948380653。最后仅改系统代理验证探针，最终包Windows36/Linux29项runtime payload与受测c0d2归档逐项一致，来源清单单独更新验证。Linux已安装GUI欢迎/原生加载/无托盘退出0通过并检查截图，日志中的OpenGL/关闭警告保留，不算完整GUI。Windows本批只有普通权限helper等已有回归，没有新真实系统代理或特权TUN验收。

清单仍partial-desktop-candidate。下一步完成持久原生连接owner及剩余手动命令/失败收尾，随后完整GUI、Windows当前平台矩阵、校园/长期、原计划能力与远端CI。未提交发布，goal active。

## 第五十二批（2026-10-17）：持久原生连接所有者与退出屏障

Rust actor与进程registry持有会话至清理证明完成；替换前关闭旧资源，失败保留并阻止新拨号。前端控制代次、丢弃等待、并发关闭、退出join及关闭后registry重开均有原生实现与测试。Dart的待清理会话列表、退出排空循环和_discard删除，视图仅释放桥接引用。HTTP/SOCKS重绑在Rust串行完成，并按配置维护实际系统代理端口。

新增8项core owner、1项registry、1项重绑测试，账户真实TLS首请求规则断言改走正式owner。Windows323/Linux302 Rust全量、274/273 Flutter全量、两端Clippy/analyze/release和61/60解压FFI通过。Windows认证负例fixture一次network失败后补齐TLS shutdown，保留原严格断言；失败与修正证据均保留。

新包Linux普通用户通过持久owner完成隔离安装、helper、HTTP/SOCKS/TUN、内核DNS/动态路由/TCP和退出清理，安装升级卸载及拒绝矩阵通过。GNOME/KDE5真实系统代理端口、重绑、独立SOCKS、恢复和KIO通过。已安装GUI欢迎/核心加载/关闭通过；无插件、window_manager、system_tray、url_launcher逐项对照将关闭警告范围收敛到依赖释放链，未过滤日志，不代替完整GUI验收。

候选`build/desktop/ed998163d149/`，408来源文件SHA `ed998163d149b49987573a772716f7a3a10a5fe46fb34fd8b18b6d05d79f5b4f`两端一致，FRB内容-1969221116。状态partial-desktop-candidate，未提交发布。core有界检查没有发现独立Dart网络/存储；公告预取及页面阶段组合还需边界审查，原计划剩余功能和最终Windows/GUI/校园/长期/安装分发/CI验收继续保留。

## 第五十三批（2026-10-17）：公告预取与分页策略迁入原生

Rust统一公告批次目标筛选、预算、共享3并发、缓存命中、URL请求合并及取消排空。页面删除worker/cursor，只展示原生事件；刷新/销毁取消批次并拒绝迟到创建和分页结果。原生终止空页和直接自引用分页。详情10秒总预算包含请求锁等待，防止前台等待已有预取后重新获得完整网络预算。

新增8项原生回归，两端公告21项及全量Windows331/Linux310 Rust通过；UI新增4项刷新/迟到回归、adapter新增3项事件/关闭边界、真实FFI新增批次预算/站外/取消/父关闭验证。282/281 Flutter全量与analyze在最后Rust-only deadline修复前通过，最后两端Clippy/release及解压FFI62/61通过。首轮Windows内存不足、fixture定时调度、测试返回按钮定位、工具lint与打包文件名错误均保留证据，未弱化业务断言或修改系统分页配置。

候选 `build/desktop/e8e936136bb2/` 的410来源文件SHA为 `e8e936136bb2bf438708bcd03cb703ebfbe10838e46f756a4ea0fce3002e52b5`，FRB2.11.1内容-630543985。两端解压库通过桥接、SSO/账户/设置跨进程探针，以及公开公告站10条列表/3详情/原生缓存实测；实际归档SHA及完整性证明见verification.json。未将本批公告验收扩展为平台特权、校园账户或完整GUI验收。

扫码自动阶段组合已定位并抽查，仍需主线程设计收敛和整条组合测试。本批未修改扫码代码、未提交发布、未标记完整目标完成；下一步保留全仓零Dart、Windows当前平台矩阵、完整GUI、校园/长期、原计划能力、正式分发与远端CI。

## 第五十四批（2026-10-17）：统一扫码登录操作与真实流错误边界

原生单一操作持有账户锁完成二维码创建、扫码等待、兑换和既有系统凭据交换，Dart只展示阶段与状态；扫码成功不再发起第二轮completeSignIn。SSO提交与展示标记之间的取消保留已提交登录，完成事件在状态收尾锁内发送。页面仅取消仍在等待的操作，已完成页面退场不会取消下一次操作，红/绿回归覆盖。

新增5项原生测试，覆盖四阶段取消、单次兑换和OTP请求、部分成功/全部凭据失败、接收端关闭、提交标记窗口及完成事件锁。账户76项、UI/adapter21项目标测试，Windows336/Linux315 Rust全量及两端Clippy/analyze通过。真实FFI另揭示FRB2.11.1的流方法Result错误由unawaited调用泄漏：QR改为明确失败事件，公告批次改为流错误，诊断接收端关闭直接结束。新增真实FFI验证关闭账户和重复订阅空公告批次；未放宽错误断言或修改依赖。

最终Windows293/Linux292 Flutter全量及解压库64/63项FFI、桥接/SSO/账户/设置跨进程探针通过；两端公开二维码探针收到图片后立即取消并验证未登录和空活动状态，不代替真实扫码成功。源码413文件两端SHA `64e4acc0c701e09bc65ea57221285efac57fbb6efc220c3e060551592af15533`，证据native-qr，成对候选64e4acc0c701的完整性及来源校验通过，具体归档SHA见verification.json。FRB名称hash仍101081745但DTO已变化，不能混库。旧Windows管线仅因确认ABI过时而停止，不是因等待超时；错误/中间验证日志保留。所有本批最终构建与探针已结束，无持久化schema变化，仍是partial-desktop-candidate。

下一步仍需收敛密码/MFA成功后自动获取凭据的阶段组合，再完成全仓零Dart及原计划平台、完整GUI、校园、长期、能力、分发与CI验收。未提交发布，目标active。

## 第五十五批（2026-10-17）：密码/MFA完整登录组合与表单取消

Rust新增sign_in_password/sign_in_code，身份认证与系统凭据获取在一个operation锁内完成；密码需要验证码时返回明确VerificationRequired，凭据全失败返回credentialsUnavailable并保留SSO。原细粒度API复用身份helper，行为和底层测试保留。Dart正式接口删除completeSignIn和独立凭据过渡页，表单只显示原生进度并按结果完成；旧页面4项生命周期测试迁入7项表单测试，补齐密码/发码/验证码取消、reset与迟到结果、完成后不误取消。适配器dispose后的迟到成功经红回归修复。

账户79项与UI/adapter/宿主53项目标测试通过。最终Windows339/Linux318 Rust、300/299 Flutter、两端Clippy/analyze/release、解压库65/64 FFI、桥接与三套跨进程存储探针通过。公开二维码探针仅图片立即取消，不代替密码/MFA真实校园验证。初轮lint、测试替身未订阅控制器关闭超时、关闭验证码取消优先错误码预期、临时包装脚本CRLF等证据保留，未弱化业务断言。

成对候选ffc946346612，413来源文件两端SHA `ffc9463466127bfadd6bc343c9cf972389d2913ccc88259957139688abe2a6a6`，FRB2.11.1内容-724705019。归档完整性和source manifest核对通过，SHA与检查范围见verification.json。无持久化schema变更、无新平台特权/完整GUI验收，全部本批构建/探针与六名只读代理已结束。未提交发布，目标active。

主线程重读完整工程计划并复核候选边界：一次性旧偏好插件清理、UI请求/订阅和启动依赖装配属于明确薄适配，不能因为await而无限扩大迁移。接下来推进原计划明确缺口：进程身份规则、正式安装格式、当前平台/完整捕获/GUI/校园/长期与远端CI，以及M5.4最终逐项签收。ZIP/tar和自定义Linux安装器不是计划要求的正式安装格式；构建可重复执行不等于字节一致可复现。

## 第五十六批（2026-10-17）：Inno/DEB安装器与真实安装事务

新增基于已验证便携归档的安装器构建器，输出Inno EXE和DEB以及输入/构建来源、工具版本、SHA256。Inno 7.1.0便携工具固定摘要并验证供应商签名；ShuVPN安装器本身未签发布证书。Windows按用户安装，runner和安装/卸载双向检查运行状态，安装事务互斥；烟测校验注册归属及路径重解析点。DEB只授权独立helper，拒绝混装/路径链接/运行中移除，配置失败保持不可用并允许重试，卸载保留非包文件并支持重装。更新选择按安装方式提供对应格式，缺少格式转发布页。

Python工具Linux25项通过，Windows25项含12项skip；路径/版本逃逸红回归后修正。Windows340/Linux319 Rust、300/299 Flutter、Clippy/analyze/release、最终解压库65/64 FFI通过。实际Windows安装、合成旧控制版本升级、卸载、1618 guard、四套已安装DLL探针和原生账户保留通过。DEB最终安装/升级/remove/purge/重装及权限矩阵通过，3轮已安装helper内核DNS/动态路由/TCP和owner退出屏障通过；真实账户恢复、宿主账户文件和dpkg状态保持。已安装欢迎页和无托盘退出0通过，截图已检查；依赖关闭警告保留。

保留未决观察：首轮TUN cleanup报VpnFailure并掩盖主错误，改进验证探针诊断后共7轮通过，但未定位原始原因，未据此修改或声称修复生产网络代码。最终安装器测试仍为局部验收，不代替完整GUI、Windows新特权、校园或长期验证。初轮WSL暂存权限777错误改由POSIX临时目录解决，没有放宽dpkg检查。

最终目录c865f4db8667，421来源文件SHA `c865f4db8667553ff3869cbec49357d53179e6caf53842adc709f6115427d3fb`；四种产物及输入链均校验，当前EXE/DEB重新执行安装烟测，运行负载与中间受测包除来源清单外逐字节相同。详细摘要和范围见verification.json。README清理过时支持声明；CI增加安装器生成/烟测但未远端执行。全部本批任务已收取，未提交发布，目标active。

## 验证和回退

每批记录实际执行的命令及结果，不能用纯 Dart 测试代替真实 FFI 测试，也不能用 Windows 构建证明 Linux 已验证。

此前阶段保留持久化会话与设置格式，规则配置另存于应用支持目录的 `rules/routing-state.json`。后端零 Dart 阶段保持用户语义和升级兼容性，由 Rust 通过一次性导入接管原数据；旧 Android 路径只作为迁移过渡，最终删除。规则回退使用编辑页上一版本入口，整体回退时恢复本分支相对基线的代码与构建改动。
