# ShuVPN 迁移接手文档

2026-10-19 提交整理（优先于下文提交状态）：现有迁移源码、测试、资源与文档纳入Git快照，忽略缓存/构建/本地环境文件，保留锁文件、绑定、字体与驱动。Rust格式/109文件边界/Flutter静态分析及Windows344项Flutter通过；工具测试仍有1项缺清单错误。私有引擎生命周期、默认构建接入及原有最终验收缺口保持，详见[项目状态](project_status.md)。

2026-10-18 整理补充（优先于下文缓存现状）：两端再生的16个pyc及本轮5个空测试临时目录已删除，原14个缓存目录均不存在。当前全量工具测试有1项缺少engine source.json的错误，较新engine-tree生命周期诊断另复现ASan释放后访问；完整简明状态见[项目状态](project_status.md)。原迁移验收范围与未完成项保持，本轮仅完成整理和进度汇总。

第九十三批当前状态（2026-10-18，优先于92批，部分完成）：独立检出固定SDK d3b14c…并实际构建私有Linux引擎及正式main，以本地ATK子树解除self-call、组合语义几何。34检查7失败先red后修正为34通过，几何20、跨进程系统ATK8、SDK相关5组54项通过。两轮真实GUI原文超时查明是fixture遗漏Rust默认三字段；仅补完整空值，新增回归red/green，两端工具183（19/7跳过）、边界109通过。最终drag-local-engine-03 CLI0/passed=true，八GUI进程全0、三来源六次物理拖动/放弃恢复/预览失效/应用4→5→6/新PID完整原文及三来源DIRECT通过，宿主网络保持。

隔离应用495/f4a84895…、开发驱动两原树495/c451f0e8…，原90产品/四包及字体保持；实际engine SHA669f84b2…与系统bridge SHA13670b19…逐进程绑定，无私有ATK库。默认SDK/CI/分发尚未接补丁，provider已有children重排/移除parent静态疑点待运行回归，完整适用性未签收。92有限清理114.652GiB逻辑量保持，本批重新生成16个pyc共341027字节；Windows删除自动审批blocked by policy拒绝，未绕过，两小缓存保留，其余12目录不存在。工作引擎/失败证据保留，无在途。见 [隔离引擎与拖动](engine_source_build_acceptance.md)、build/desktop/engine-geometry/verification.json。整体goal active，Windows/校园/guest Full/长期/其它桌面/84故障/CI/审计范围保持，无提交发布。

第九十二批当前状态（2026-10-18，优先于91批，部分完成）：固定SDK原始C++原生夹具确认父rect原点重复累加、scale忽略及PARENT继续累加的问题；6场景20断言原版18失败，实验补丁20通过。新增通知同步查询实际发现补丁回归零尺寸，先red后调整为几何/全部parent先提交、再通知，最终通过。私有诊断runner只增加读取update-semantics的观察者，AOT/DSO/Rust/字体等与90正式bundle相同；capture01首进程Alt+F4退出超时，capture02显式Ctrl+Q前三基线+导入共4退出0，第5真实拖动仍失败。原文不变、未猜坐标、未放宽门禁。

capture02两只真实手柄AT-SPI分别[368,488,48,48]/[368,381,48,48]，捕获的两条链共16节点经原始C++回放逐项相同；补丁实际ATK返回[368,402,48,48]/[368,241,48,48]，与点击前截图对应，偏移86/140已形成输入→原生输出因果证据。只在build研究补丁，无新正式引擎/应用/包，物理拖动仍未签收；全部来源保持495/f4a84895…，正式产品与四包仍90批0ecc。实际编译输入/header/binary摘要与旧pinned原版来源分开记录，不能混写；清理后23项原生检查重新运行通过。

按用户追加要求完成有限缓存清理：Windows8、Linux6目录共120802文件、114.652GiB逻辑字节；D盘实测可用32.182→85.573GiB，WSL内部另增49.747GiB，未压缩VHDX，不能把逻辑量当宿主回收。删除前核路径/链接/构建进程，后核来源、全部正式Linux载荷/四包/字体保持。64个Dart test/tool及581引用有界审查未找到可确信废弃源码/测试，删除0，不以旧Android负断言当无用测试。完整Flutter/Rust/工具套件未重跑。

SDK库为GTK+内部核心单体，无公开核心导出/可链接缓存，不能adapter-only替换。私有depot_tools clone成功，C盘独立源码clone300秒超时定向-15，原SDK保持，未gclient sync/构建；删除该外部不完整检出被自动审批拒绝（仅blocked by policy），目录保留但无在途。下一步完整隔离引擎构建与正式GUI验证，详见 [语义几何与有限清理](semantics_geometry_acceptance.md)、build/desktop/accessibility-geometry/verification.json。整体goal active，旧全部校园/Windows/guest Full/长期/其它桌面/84故障/CI/审计范围保持，无提交发布。

第九十一批当前状态（2026-10-18，优先于90批，部分完成）：固定at-spi2-atk2.38.0构建原始/私有实验库，无系统安装。系统库与新构建原版的最小同连接GetExtents均卡住，抓到self-call并定向终止-15；v1完成20项但10失败，独立复核后v2修正路径规范校验与GetPosition/GetSize分派。同连接20项全过、跨进程原版/v2各7项全过，原版/v2上游droute-test/atk-test各两组过。v2 SHA92224c0b…，只是默认context、限定夹具实验；重入/其它线程与context/解析范围等未全面覆盖，不能发布通用修复库。

正式90产品仍0ecc5605…，本批没有新应用构建或包。私有LD库下drag-private01/02/03均前三基线+第四导入进程0，第五失败：01发出鼠标但原文不变；02新VISIBLE守卫停在搜索不足；03补范围/方形裁切/配对截图后7次probe，SHOWING/VISIBLE均true且48×48，鼠标中心(392,512)→(392,405)仍与点击前截图真实手柄位置不符，未改顺序。**同连接等待已在实验库越过，但又确认一层语义几何错误，物理拖动仍未签收**。失败和释放记录保持，不能用最小协议绿色替代实窗绿色。

生产只改开发driver及其测试：VISIBLE严格true、缺失/false/非bool拒绝、非方形拒绝、先上后下每向32步、逐probe/鼠标前后证据；先red后green，最终两树495/`f4a84895e16e9c1fb9998610826f2e5e480890e27ddf675aeba1d60df82321ad`，工具181（19/7跳过）、针对12项及边界109过。中间88599a83…/180项另存，主程序/Dart/Rust/绑定/依赖/字体及90四包保持；实验库未进入默认分发路径，默认GTK门禁仍未通过。

Windows90产品私有副本两进程0，实测ScrollItem pattern10017为空，未调用滚动、没有E_FAIL可归因；原文/宿主目录元数据/代理保持。静态COM map无IA2只是源码调查，不作binary支持断言。另两端独立重算217-byte engine输入Git blob hash/Python SHA1为1cf1c477…，匹配缓存artifact元数据；SDK Git d3b14c…、元数据engine git a804b261…分开记录，补上内容映射但未做二进制认证/复现构建。详情/总汇见 [运行库隔离与边界](accessibility_runtime_acceptance.md)、build/desktop/accessibility-runtime/verification.json。下一步对SDK rect/transform/父坐标叠加做最小修复验证，并完成运行库生产适用性评估；不猜坐标、不放宽规则原文检查。整体goal active/partial，其余校园/Windows安装详情与三来源/guest Full/长期/其它桌面/原84故障/远端CI/最终审计保留，无提交发布。

第九十批当前状态（2026-10-18，优先于89批，部分完成）：Linux runner公开API补FlSocketAccessible→AtkSocket缺失的parent几何；独立复核指出初版强引用环，实际无parent控制退出0/parent-only新增销毁测试134，最终destroy普通回调在GTK清理前断环，四项native测试通过（几何/幂等、保留既有parent、普通widget、两对象弱通知释放）。新target接本地及Linux CI，无private字段/vtable/系统库补丁。最终495文件两树`0ecc5605c0f6950ef23a14b532dab56c429612e1a711d85732dc2ce12557ee22`，两端工具178（19/7跳过）、边界109、shell语法、两端正式main release通过；无Dart/Rust/绑定/依赖/字体改动，完整Flutter/Rust未重跑。

新Linux main离线GUI三普通进程0/版本2与覆写/设置重开、宿主网络相同。新bus诊断PID492231仍同sender=destination=:1.0 self-GetExtents/dbind超时、无拖动、定向终止-15；parent修复不解除同步等待。所查固定新版GNOME桥仍同步调用，没有升级即修复的依据。四新包rule-geometry/package逐项含原字体/OFL/FontManifest/core/helper，Inno仅外层/输入链，新安装器未安装，旧89等保持。

Windows仍用89产品/615fa36a来源：run-01任务栏指针守卫失败；只读确认前台后来变同session未提升终端，再做run-02，三基线0后高级字段缺失；run-03取得四组点击前后配对截图/provider、四进程全0，但新增按钮画面已不在视口却provider/point仍同名可见，字段未出现，草稿/磁盘保持，diagnosis_passed=true而passed=false，不签三来源。两端各2个framework/Rust几何诊断通过，能区分裁切与完整布局、记录源代码AX父bounds原点投影，但未读取实际C++AXTree，根因仍需更强证明。前面错误probe假设/释放时机等失败保留。

**来源校正**：engine artifact a804b261…不是本轮已核实源码Git commit；SDK Git HEAD d3b14c876900e553bc736ca19295fc09e3853e8e六文件与git show逐字节对应，未独立建立artifact编译映射。新的缓存出处优先于旧raw引用，旧报告不覆盖。详情/总汇见 [原生无障碍几何与生命周期](native_accessibility_geometry_acceptance.md)、build/desktop/rule-geometry/verification.json。整体goal active/partial，Windows原始安装与详情/三来源、Linux物理拖动、真实校园、guest Full、长期/其它桌面、原84故障、远端CI/最终审计保留，无提交发布。

第八十九批当前状态（2026-10-18，优先于88批，部分完成）：修复实心底栏仍extendBody导致正文伸入56逻辑像素，列表尾距84→28；独立通知页补SafeArea，DPR1/2及系统底部34的真实red/green保留，公告追加后测试滚到新末尾。四处纯view/边界109，无Rust/绑定/依赖/字体变化。两端完整Flutter344/343、工具178（19/7跳过）、analyze/边界109及正式main release通过；493文件两树来源`615fa36abaa0d64eec01a8b0b3f9c066d59a1940974fd12caf05439834e3cbcd`。

Windows驱动补点击前最新控件/前台/point守卫、真实模态等待、写报告失败撤销成功及外部前台最小身份记录。旧产品run-01三进程基线0后新增字段失败、run-02前台丢失、run-03底栏point拒绝；run-04/05两只读诊断均两进程0，重复point落匿名pane。新产品run-06底栏点击及三进程基线0通过，第四进程新增前置按钮provider/point一致仍无弹窗字段，CLI1/passed=false、失败截图与UIA的滚动可见性不一致，原因未定。run-07同步证据诊断首进程任务栏指针请求/读回不符被拒绝，原probe强制清理，外部Chrome前台非安全弹窗，未取得配对证据；先复核指针/前台，再继续同一有界诊断，不机械重试或猜坐标。所有旧失败保留。

Linux新产品离线GUI三普通进程0、设置/版本2/覆写/重开及宿主网络比较通过。另用88产品私有总线直接捕获GetExtents sender=destination=:1.0，同app PID480816，补齐同进程同步调用证据；仍dbind超时、无拖动、诊断定向终止-15，不称正常退出或修复。没有安全公开runner替换plug入口的证据，不做全局ATK私有vtable补丁。新四包windows-rule-resume/package逐项核对字体/OFL/FontManifest/core/helper，Inno仅外层/输入链，两新安装器未安装，旧候选不动。详情及总汇见 [几何与原生点击](desktop_geometry_acceptance.md)；整体partial/goal active，其余校园/Windows原始安装与详情/guest Full/长期/其它桌面/原84故障/远端CI/最终审计保持，无提交发布。

第八十八批当前状态（2026-10-18，优先于87批，部分完成）：补M4.4真实鼠标手势，Flutter/native三来源各验证取消保持、拖动仅改草稿/撤销既有预览、旧生效REJECT/应用v2才DIRECT、释放重开完整顺序与原文；另上/下/第二页26→25/忙碌拖动，最终两端针对16项通过。生产一处纯view给现有48×48手柄独立具名image语义，旧合并卡片的red保留，不加Tap或改排序/Rust/绑定/依赖/字体。

正式main两端release、完整Flutter341/340、analyze/边界109通过；最终两端工具172（19/7跳过）通过。应用四包493/`e610553f66e39f2abf444067ec5fe500c706713333473f0315aef4597db1cb5f`，后续开发驱动/守卫493/`65ed60b0b078d93d1f5b488cf55e6214f010d1545e34ad24a9519677cdd0881e`分别两树一致。新四包rule-drag/package逐项含字体/OFL/FontManifest/core/helper核对，Inno仅外层/输入链；新安装器未安装，旧87/86/83/77保持。

新正式Linux物理拖动driver已接本地/CI/DEB且9守卫通过，但实际portable-01/02均失败，**正式窗口拖动尚未执行成功，新GUI门禁在当前GTK环境会失败**。每轮基线3进程+导入第4进程正常退出0，在第5进程拖动前失败：先纠正Tap+Focus误当两个Apply动作，再遇Component.GetExtents/dbind超时，没有放宽矩形或猜测截图坐标。两轮宿主link/路由/rule保持，失败GUI定向清理；不冒充8进程完整通过或新DEB实测。

geometry-04普通正式main PID480238主线程栈确认poll→dbus_pending_call_block→dbus_connection_send_with_reply_and_block→libatk-bridge→atk_component_get_extents/Flutter父链，查询后正常退出也超时。两端engine固定a804b261…；运行库bridge2.38.0-3/ATK2.36/AT-SPI2.44/GTK3.24.33，固定engine与原始at-spi2-atk2.38源码支持同进程plug/socket同步等待机制，消息目的地未单独观察，尚未修复。SCREEN/WINDOW/PARENT连续超时不是各自冷启动对照；子进程GetSize/Position因节点缺失未执行。GDB只APT下载解包到本批build，具体新GUI只授权具体debuggerPID，无全局安装/ptrace改动；原退出失败、原报告pointer字段过宽及更正、WSL初诊日志覆盖均保留。当前已验证PID/正式GUI与私有GDB映像清空，宿主无固定安装，无在途。

**Windows阻挡状态已改变**：最后前台为DSH窗口，旧安全弹窗HWND5639302已不存在，只读、无输入，不能再把87批同一弹窗当当前阻挡。下一轮先走现有Windows启动前守卫，用新私有报告继续Windows实窗（现有windows_rule_acceptance路径），不操作DSH；Linux从原生GTK几何桥接继续修复后再跑真实拖动。总汇build/desktop/rule-drag/verification.json与 [拖动及阻塞证据](rule_drag_acceptance.md)，widget/native子范围true、Linux物理拖动false、overall partial，goal active。校园/Windows安装与详情/guest Full/长期/其它桌面/原84故障/远端CI/最终审计全部保留，无提交发布。

第八十七批当前状态（2026-10-18，优先于86批，部分完成）：补齐Linux M4.4连接详情实窗四类真实Rust HTTP/TCP结果：成功SHU、成功DIRECT、规则REJECT和资源表外SHU拒绝，32个具名字段绑定同PID/轮次/run_id。生产仅三处Dart view复用已有具名图标按钮及只读字段语义，清单109，无业务/Rust/绑定/依赖/字体变化。

正式应用/四包来源488/`ecc18a75a5edf358ea411f0fadb9c5806cd4bdc518841d6e32762f406c0b4480`，后续夹具观测竞争修复/测试来源490/`0881aa9a16a094d38b33afee999173b7ccd3af787aad2547558ddededbb9ce67`，分别两树一致，不混写。正式两端main release通过，最终完整Flutter332/331、两端工具163（19/7跳过）、analyze/边界109通过；Linux新夹具后重建正式入口，其runner/AOT/core/helper/字体字节与正式tar相同。

gui-01第三轮重连触发夹具共享会话视图释放竞争，原passed=false/fatal保留；先red后green补全FRB请求首次await前发出、冻结帧轮次/端口及错误stack，非生产/心跳故障。gui-02一个普通UID1000/CapEff0 GUI完成三轮12条原有代理流、四类详情及DIRECT独立接收65536字节/两HTTP403/两拒绝目标无待接收连接；记录页打开时托盘断开清空并禁刷新，第三轮重连和宿主托盘退出0。原网关pidfd退出、owner closed/监听与TUN/egress屏障、私有及宿主网络前后相同，本轮GUI/fixture/面板/宿主无遗留。32字段为语义检查，截图部分裁切；停止会话仍可保留原生历史，不把UI清空称作原生擦除。

新四包connection-details/package逐项核对ZIP39/tar32/DEB清单33与字体/OFL/FontManifest/core/helper/来源，旧86/83/77保持。Inno只核外层/输入ZIP/固定编译器链，未独立解码内嵌载荷；本批两新安装器均未实际安装。Windows工具一次UDP绑定10013失败保留，端口未记录、原因未确认；附端口单项3过及未改源全套163过，不抹旧失败。前台仍同一安全中心PickerHost，未输入或启动WindowsGUI。

总汇build/desktop/connection-details/verification.json与 [连接详情验收](connection_details_acceptance.md)，Linux详情子范围true、overall passed=false/partial=true、goal active。M4.4实际拖动仍未验收，旧按钮move/无拖拽柄断言不冒充手势。Windows正式/安装/详情GUI、真实校园、可恢复guest Full、长期/其它桌面Wayland、原84心跳/WSL绑定、远端CI与最终审计继续；无在途/提交发布。

第八十六批当前状态（2026-10-18，优先于85批，部分完成）：补齐Linux规则排序/启停/实际被引用的Office手动组/当前配置测试器/回退重开实窗验收，新增常规driver与13守卫，接入本地/CI/DEB且保留三来源CRUD。实测发现下拉与菜单缺少enabled、结果SelectableText缺少label，生产仅一处Dart view补显式启用/合并语义及结果label，无业务/Rust/绑定/依赖/字体变化；原始red和测试自身失败均保留。基线子目录路径与外层WM清理失败不能留passed:true已回归修复。

两端正式main构建/完整Flutter326/325、analyze、边界109通过；最终两端158工具（19/7平台跳过）通过。应用/四包来源488/`440d034800354bd52d9dc202acac7b307536a493290bd3131a0c4b5f040847b5`，后续开发driver/守卫来源488/`49ba360e3fd4502f2a528ced7297fdfa7612408b398e5189222b468641cd97cc`；分别两树一致，不能混写来源。当前release目录为本批新AOT；旧83/77四包保留，新四包在rule-management/package-final，字体/OFL/FontManifest、core/helper摘要保持。中间package/47ec…也保留，新Inno只生成未安装。

portable-07与实际DEB installed-01/rule-management各9个UID1000/CapEff0进程退出0，11条新GUI HTTP/TCP决策、22次字段复制读回、6次关闭owner后native读回，版本3/4/5/6/7/7；停用保留索引，组规则DIRECT→REJECT，回退得到新版本7且原文恢复v5，重开保持。安装另保留session三来源CRUD九普通进程退出0/最终v8；安装权限/合成升级/账户及非包文件保留/卸载、宿主账户和包数据库比较通过。宿主无固定安装与本次映像进程，所有句柄已终态，无在途。

GTK输入Text接口在部分同页更新后返回标签/空值，缓存刷新等未消除；保留原始失败和层级读数，最终使用独占Xvfb会话内实际Ctrl+A/Ctrl+C/GTK回调逐字核对，错误复制值仍拒绝，AT-SPI读数另存；不冒充全部Text元数据已修复。SHOWING不等于矩形裁切/物理命中，部分图内结果在视口外，截图已抽看，最终重开可见v7组决策。Windows最终前台仍同一安全中心Shell_SystemDialog/PickerHost，未输入或启动正式app，待用户处理系统弹窗后继续。

总汇build/desktop/rule-management/verification.json及 [规则管理验收](rule_management_acceptance.md)，Linux子范围true，整体passed=false/partial=true、goal active。原84心跳压力/WSL绑定机制、M4.4拖动/连接详情、Windows原始/安装GUI与可恢复guest Full、真实校园/更长运行/其它桌面Wayland/远端CI/最终审计继续保留；未提交发布。

第八十五批当前状态（2026-10-18，优先于84批，部分完成）：Windows三来源真实窗口驱动tool/windows_rule_acceptance.py已实现，复用正式main资源名称私有副本、原3进程基线与扩展9进程检查；shared rule_gui_evidence统一两平台受控规则/独立native来源与版本检查。修复实际暴露的primary error被二次清理覆盖及成功标记早于清理问题，先red后green；系统安全弹窗在启动前明确拒绝。只改工具/测试，无生产代码、字体、绑定或83候选改动。最终486文件两树SHA `cd7cde4b3796cef0f51fb8a1061a0aff110729de317b1334744cd3bfe20c1a6f`，两端145工具（19/7跳过）及边界109通过，新增10守卫，未重复Flutter/Rust/安装矩阵或远端CI。

Windows run-01/02均在首个窗口激活失败、需要强杀本次GUI，未开始规则流程；launches空是失败发生在追加记录之前，不能说没启动过应用。只读确认当前前台Windows安全中心、Shell_SystemDialog/PickerHost.exe、显示更多/允许/取消；没有代选系统授权。run-03新守卫在同一弹窗前明确拒绝，无GUI启动/强杀，清理正常；Windows三来源仍未验收。需用户处理这个系统弹窗后再用新私有目录/报告继续，不机械重试或操作系统“允许/取消”。

Linux共享检查回归linux-01九普通进程退出0，三来源增改删取消/保存/预览不落盘/应用/编辑重开及6独立读回版本2→8通过，截图已看。此运行完整来源486/03fa4fc2…早于Windows守卫修订，实际4个Linux GUI输入文件与最终树逐字节相同，摘要另存；不冒充最终全树cd7来源或新的DEB安装GUI。三轮Windows六个KnownFolder根/mutex/本次映像及Linux九PID清空，宿主两目录/两代理键保持，无在途。总汇windows-rule-dialogs/verification.json仍partial，见 [窗口驱动与阻挡记录](windows_rule_dialog_acceptance.md)。整体goal active；Windows此项等待系统弹窗由用户处理，原84心跳/宿主绑定机制及M4.4/校园/原始安装GUI/guest Full/长期/其它桌面/远端CI/最终审计范围全部保留。

第八十四批当前状态（2026-10-18，优先于83批）：新增测试专用有界心跳trace，来源483文件两树SHA `b4a9d1f1f4570972abb336c93e1b0cf691b2f7d7cdcb216ce35e85ac521cf1d7`。固定0.05秒fixture原计划与独立5秒鉴权时序参数分离，默认仍0.05；两个入口都仅本地plan，不调用connectAccount。trace明确区分完整request、成功flush与客户端实际消费，逐连接编号、64尾事件/累计数、重复/缺失/无效日志拒绝及主错误/清理保护已验证。只改工具/fixture/测试，生产Rust/绑定/GUI/字体及83四包不变。

Linux新50毫秒600/100/10复现整体198.256秒EOF，只完成10预热；即时owner failure=connection/“L3 节点未回应心跳”，11控制连接trace完整，末连接3824请求/3824flush、回复最大间隔0.584616457秒、末三请求突发。明确本次直接心跳判失活，未定位延迟来源，不能反推82批449秒。受控只暂停原fixture pidfd 0.400909秒也验证新诊断；原probe255/fixture0及真实失败保持。Windows1预热短跑handles+33超过原32预算，另存失败；正式10预热短跑通过，未放宽预算。

两端独立5秒600秒/100重连/10预热均终态通过：各111session/444流、四流各≥600秒、原进程退出0、111控制连接trace及owner closed/监听屏障/预算完整。Windows626.843秒/payload303628288，Linux650.564秒/payload303497216；非真实校园或无泄漏证明。宿主Windows两目录/两代理键与Linux路由/link摘要相同，本次映像进程清空，无在途构建/长跑。总汇heartbeat-diagnostics/verification.json仍passed=false/partial=true，分别保留旧/新压力失败与独立5秒通过，见 [心跳观测与环境范围](heartbeat_diagnostics_acceptance.md)。

两端135工具（19/7跳过）、18相关Rust、选定Clippy/rustfmt及边界109通过；完整Windows365 Rust通过。Linux宿主242过/3个HTTP出口超时，单组串行仍失败；普通socket证实当前宿主SO_BINDTODEVICE lo后loopback连接超时。新私有loopback netns内同UID1000同源码/二进制，socket及5相关测试、完整349 Rust通过，宿主网络不变。明确环境范围，不称宿主全套通过，不修改宿主网络或放宽时限。下一步继续定位心跳延迟两端/调度/传输、原449秒证据缺口及当前WSL命名空间绑定机制，并推进Windows三来源窗口/其它M4.4、校园、可恢复guest Full、多日/其它桌面、远端CI及最终审计。goal active，无提交发布。

第八十三批当前状态（2026-10-18，优先于82批）：规则卡片图标的空AT-SPI名称已修复，新增纯view LabelledIconButton，显式label与原生按钮tap/focus/disabled合并，Tooltip保留视觉并排除重复语义；规则动作/开关带来源及绝对行号，新增按钮带来源唯一名称。规则页重命名/兜底图标同样包装，业务/Rust/绑定/依赖不改。生产Dart清单109/view37；旧实现空label的真实red及测试自身finder/焦点数据读取失败分别保留，最终语义点击+Tab/Space各触发一次通过。

已将build试跑整理为tool/desktop_rule_acceptance.py，切入本地、Linux bundle CI和DEB GUI入口。当前新正式main两端构建、完整Flutter324/323、工具132（19/7跳过）、analyze、边界109及两个脚本分别语法检查通过。482文件两树SHA `f8464b55b25cb45d4de1c3e0a372a66302117247602f9a17bb1a6bc6e9c78c27`。新四包位于build/desktop/rule-accessibility/package，版本0.4.2+9；77字体四包未覆盖，TTF/OFL/core/helper保持原摘要，新AOT含展示修复。ZIP/tar/DEB清单和产物/来源独立复核通过；新Inno仅生成，未实际安装。

Linux便携及同一DEB实际安装后均9个普通GUI进程退出0，prepend/profile/append增改删取消/保存/预览不落盘/显式应用/编辑重开通过，版本2→8；各41具名动作、3组实际AT-SPI按钮/开关状态、6次关闭owner后的独立原生readback。无名图标顺序回退已移除，只通过完整name/role/owner/enabled/showing定位；平台do_action不是物理鼠标命中。安装同时验证权限/升级/卸载/账户及非包数据保留，宿主账户/包数据库前后相同、固定路径和本次进程清空；GUI PID是私有namespace PID。Windows新ZIP私有EXE资源副本原离线/非法规则回归三个进程退出0，宿主两目录/两代理键相同、KnownFolder根/mutex清空；未签Windows三来源弹窗、原始EXE或安装GUI。

总汇build/desktop/rule-accessibility/verification.json，说明 [规则控件与三来源验收](rule_accessibility_acceptance.md)。远端CI只是配置未运行，本批未重跑Rust/真实网络/TUN/Full/托盘矩阵。82批449.938秒EOF仍未归因、原失败和642.822秒复验分别保留；无在途长跑/构建/GUI。下一步继续原EOF定位、Windows同类弹窗、策略组/详情等完整M4.4及原计划校园/原始安装GUI/可恢复guest特权Full/多日/其它Linux桌面/远端CI/最终审计，goal active，无提交发布。

第八十二批当前状态（2026-10-18，优先于81批）：新增同一DesktopConnectionOwner普通用户本地长跑探针及OS资源观测，原始479文件来源224734f6…两树一致。Windows四持久流各≥600秒、之后100轮重连，含预热共111session/444流、持久payload305201152字节、整体615.375秒通过；原probe/fixture退出0，逐轮native与OS监听器屏障及32MiB/32句柄/8线程等固定预算通过。Linux同配置449.938秒提前EOF失败，只完成10轮预热；最后快照connected/4成功拨号，不能靠decision无error否定运行期故障。probe经finally清理后255、fixture0、无强杀；未归因。总汇build/desktop/connection-soak/verification-baseline.json明确passed=false/partial=true，旧源码已另存，字体四包/生产Rust/绑定不变。

同时Linux正式字体bundle真实窗口的前置规则新增/编辑/删除取消/保存/预览不落盘/版本2→3/重开/版本4删除通过，rule-gui-03五普通进程退出0，独立native读回edited域名DIRECT→REJECT、原前后覆写DIRECT，截图已查看。图标无AT-SPI名称未修复；以唯一domain Text+严格四按钮卡片结构及亲读widget的顺序绑定动作，非物理鼠标点击。前两次视口定位失败保留；只签prepend，不签profile/append、安装/Windows同类窗口。脚本在build目录属试跑，尚未集成工具/CI，见 [窗口弹窗](linux_rule_dialog_gui_acceptance.md)。

原始两端124工具（19/7跳过）及analyze/边界108通过；随后补每流进度、错误即时native快照，诊断来源479文件0c5618da…两树一致，两端125工具/analyze/边界通过、两端诊断短跑/预算故障通过。Linux只暂停本次fixture的pidfd约0.401秒，成功抓到0.198秒提前EOF及owner“L3 节点未回应心跳”，这只证明诊断能力，不能归因原449秒。未修改生产逻辑或放宽预算。详情 [长跑记录](native_owner_soak_acceptance.md)。

诊断复验long-linux-02已完成（配置600秒/100轮/10预热/50ms均不变），原exec会话33782退出0，整体642.822秒、111session/444流、四流各≥600秒、payload310509568字节通过。probe/fixture 433596/433573退出0、无强杀，逐轮监听表/native清理/最终owner closed通过；Linux前后link/路由相同，最大采样增长probe RSS3010560/VmSize2101248/FD16/线程0，均在预算内。不是当前在途任务，不要重启。总汇connection-soak/verification.json已记running=false，整批仍passed=false/partial=true、原449秒未归因。旧runtime-linux-long-02.json只是历史观测，不代表仍存活。

下一步继续定位原Linux间歇性EOF；可另以正式鉴权源码5秒心跳作独立profile，但50ms旧失败必须保留，不能替代或放宽预算重签。并把build目录GUI试跑收敛进常规工具/CI，补profile/append/Windows同类弹窗、无名称图标的无障碍及正式详情/策略组矩阵。现有Windows长跑、Linux首轮失败、Linux复验、窗口成功各自保留来源，不重跑record_baseline.py覆盖。完整Rust/Flutter全套、真实校园、Windows原始EXE/安装GUI、可恢复guest特权Full、更长/多日、其它Linux桌面/Wayland、远端CI与原计划最终审计仍待，goal active。

第八十一批当前状态（2026-10-18，优先于80批）：亲读原计划及规则页面/适配/原生editor后，补齐M4.4三来源新增/编辑/删除弹窗取消/保存、实际source绑定、字段错误/未知类型的不同校验阶段、显式应用与持久文件、删除后manager重开，以及重命名重开和页面回退的4项真实Rust widget联动测试。两端新4+既有11=15项、analyze及边界108通过。只新增test/native/routing_rule_dialog_test.dart，生产/字体/绑定/四包未变；无新构建提交发布。源475文件两树SHA `7df2976a012c9d2fdf3ee135dafc62115b1a4cf9f6b639c7078730f5e786a390`，产品仍77批474来源9a9ebbf5…。

总汇build/desktop/rule-dialogs/verification.json，说明及M4.4逐项对应见 [规则编辑弹窗验收](routing_editor_acceptance.md)。首轮错误假设未知类型在编辑阶段拒绝、两端三项超时失败保留；改为分别验证字段错误/prepare拒绝，未更改生产逻辑。中间15项通过但analyze缺大括号info返回1亦保留，最终修正并补来源/取消/回退文件/失败清理断言后通过。版本2只核实际持久文件，版本3才manager重开；重命名回退核文件而未再重开第三manager。不是正式桌面窗口/真实出站证明，不将本批写成全量Flutter/Rust/117工具重跑。

M4.4实现已有卡片、排序/启停、策略组、连接详情，历史“尚待实现”表不能照搬为当前事实；正式窗口新增/编辑/删除/策略组/详情矩阵仍待补。原计划M5.1需持久owner长跑及真实数据/占用采样，现有三轮或watchdog上限不代表长期证据；可另补普通用户fixture长跑，但不替代真实校园。继续真实校园、Windows原始EXE/安装GUI、可恢复guest特权Full、长期/其它Linux桌面/Wayland、远端CI与原计划最终审计，goal active。

第八十批当前状态（2026-10-18，优先于79批）：Linux同类无效规则GUI已补齐便携run-03及实际新字体DEB安装installed-01，各三个UID1000/CapEff0进程退出0，默认库/三项产品摘要匹配。精确输入UNSUPPORTED JSON、显示原生错误，Apply实际disabled且动作0，无平台或物理Apply点击；分别观察0.539/0.535秒，原记录字节及两次独立native读回/新PID编辑器值保持版本2和非空前后置覆写。截图已查看，重开raw值由AT-SPI读回，未称视口外文本在截图中可见。

仅改Linux开发驱动/对应测试两文件，两端117工具（19/7跳过）、边界108通过。工具474文件两树SHA `fdfa78556f20e2211d12a2c3bfec93d70a52dc624a6048783c5253275af4a7df`，产品仍77批9a9ebbf5…，四包未变；无新生产构建/候选/提交发布。run-01几何DBind超时和run-02错误role假设失败保留；实际panel/Text/State/Action路径已实测。纯守卫不认证来源或强制disabled，最终总汇另核本次disabled/PID；历史apply-click截图名不是点击证明。总汇 `build/desktop/linux-invalid-rules/verification.json`，说明 [Linux无效规则GUI验收](linux_invalid_rule_gui_acceptance.md)。

实际DEB安装/升级/权限矩阵/卸载/原生账户保留及宿主账户/包数据库前后相同通过，无安装或作用域进程遗留；私有namespace GUI PID不当host PID。便携32项、DEB归档33项核对，实际安装本批仅测GUI三项产品；辅助观察误用便携README期望失败已单存，不冒称全量安装逐项核对。78批全量安装/Full/托盘证据保持；完整Rust/Flutter/analyze沿用77，不称本批重跑。下一步继续完整M4.4/设置及规则审计、真实校园、Windows原始EXE/安装GUI、独立guest特权Full、长期/其它Linux桌面/Wayland、远端CI与最终审计，goal active。

第七十九批当前状态（2026-10-18，优先于78批）：已完成Windows正式main字体资源副本的无效规则真实GUI路径。输入UNSUPPORTED JSON，原生错误可见；UIA对灰色Apply仍报告enabled=true，未称修复。新增独立物理点击收据与至少0.5秒观察，实际0.812秒无提交效果；关闭后独立native读回、重开raw编辑器值及最终原生文件均保留版本2、仅校园profile与非空前后置覆写。run-03三个普通进程退出0，截图已查看；run-01类型假设与run-02无UIA disabled/无点击收据的失败均保留。所有原无收据负例不放宽，新增手造/NaN/Inf/错目标/错SHA等收据拒绝；该纯函数不是离线点击认证器。

只改开发驱动及对应Python测试两个文件。两端工具114（19/7跳过）、边界108通过，17项定向守卫通过；工具474文件两树SHA `916160ce5f732e7fb131684e9be798ffdd1c2027191e141bced2bdcfed1fe2ee`，产品仍77批9a9ebbf5…/字体ZIP6e9c4503…，无新生产构建/候选/提交发布。6个KnownFolder根、mutex及应用fixture清理，宿主两目录/两代理键本批前后相同。总汇 `build/desktop/rules-invalid-gui/verification.json`，说明 [无效规则GUI验收](invalid_rule_gui_acceptance.md)。78批新字体安装/Full/安装GUI证据已先封存，不能因工具来源后续变化抹去或改写其产品来源。

下一步补Linux同类无效草稿GUI。已知Atspi该扩展区是panel（不是通用control现有push button/text/password text三类），`get_text(start,end)`、`get_state_set()`已检查接口，但动作/状态仍需实际核验；无Git镜像工具测试必须设置SHUVPN_SOURCE_REVISION=1e055d672754971bb7ac5bbadb923858d78ae777与SHUVPN_SOURCE_DIRTY=true，首轮缺失失败已保留。继续完整M4.4/规则证据审计，以及真实校园、Windows原始EXE/安装GUI、独立guest特权Full、长期/其它Linux桌面/Wayland、远端CI与最终审计，goal active。

第七十八批当前状态（2026-10-18，优先于77批）：新字体候选已实际完成Windows安装/合成旧版本升级/卸载、39项载荷核对、1618与事务/运行保护、四套安装DLL探针及账户/非包文件保留；没有运行Windows正式GUI。新DEB完成实际安装权限矩阵、3轮校园TUN、默认库+固定helper的Full十阶段数据/覆盖故障/helper强杀重建及路由恢复，以及安装路径离线设置/托盘各3个普通进程退出0。截图已查看；日志中的预期dpkg拒绝、非空非包目录和xfconf提示保留。

总汇 `build/desktop/wenkai-install/verification.json`，说明 [78批安装验收](wenkai_install_acceptance.md)。source仍474文件9a9ebbf5…，77四包摘要未变，无生产/开发代码改动、新包/提交/发布。Windows两处注册、安装/运行mutex与应用fixture清空，宿主两目录/两代理键前后相同；Linux外层host PID结束，固定安装路径进程0/宿主未留安装，账户及包数据库由harness前后比较保持。私有namespace的GUI PID不能当host PID。完整Rust/Flutter/analyze沿用77，未冒充重跑。

只读核实有启用的普通CodexSandbox账户，但没有已验证可用的凭据/token/交互桌面；仍未识别可恢复Windows guest或Sandbox入口，没有登录账户、修改系统设置或在开发宿主启用Full/UAC。该缺口不以环境变量替换或资源副本验收消除。下一步补原计划规则校验失败仍保持旧配置的真实GUI证据，再继续完整设置/真实校园/guest特权Full/长期/其它Linux桌面与最终审计；goal active。

第七十七批当前状态（2026-10-18，优先于下文76批）：用户指定霞鹜文楷屏幕阅读版，已接入未修改v1.522字体、原始OFL及来源摘要，全局明暗主题、独立样式与日志一致使用本地family。生产仅3个展示文件及pubspec资源变化，运行依赖/锁未变，边界108复核更新。两端完整Flutter317/316、工具111（19/7跳过）、analyze、正式main release及各3进程离线GUI通过，截图已查看。源474文件两树SHA `9a9ebbf5575f7a63f174437a04a8a8340a6654439ad1b72404ff49fd1af05c90`。本批字体要求已完成，整个迁移goal仍active。

总汇 `build/desktop/wenkai-font/verification.json`，说明 [字体接入验收](wenkai_font_acceptance.md)。新ZIP/Inno/tar/DEB在同目录package，旧72候选未覆盖；ZIP39/tar32/DEB opt目录33项载荷及字体/许可独立复核，Inno只核生成与输入/来源，未重跑新安装器实际安装。Windows GUI仍私有版本资源副本，不等于原始EXE默认profile/安装GUI。默认构建写反斜杠被准备器拒绝，显式main重建得到相同产品后GUI通过；失败日志保留。两KnownFolder根/mutex/应用fixture无遗留，两宿主目录/两代理键本批前后相同。无新Rust/网络/安装全矩阵、提交或发布。

下一步范围不缩减：新字体候选安装验证、原始EXE/安装WindowsGUI、完整离线设置剩余项、真实账户/校园、独立可恢复guest的提升观察者/普通probe/真实UAC helper Full故障恢复、长期、其它Linux桌面/Wayland、远端CI及原计划最终逐项审计。76批最初的宿主代理差异继续保留未归因，不能由77批快照相同推翻。

第七十六批恢复完成（2026-10-18，优先于下文暂停及历史状态）：用户已明确恢复持续推进，goal active。正式 main 私有版本资源副本 run-03/04 各三个普通进程退出0；70000拒绝且设置不变、18443保存、Full重开/Campus恢复、仅校园预览不落盘/版本1→2/非空前后置覆写原文与决策保留均通过。两端工具110（19/7跳过）、analyze、边界108及Linux正式bundle离线三进程回归通过；Windows旧74开发AOT已连接三轮12流回归第二次通过，第一次overflow定位超时保留。总汇 `build/desktop/windows-settings-gui/verification.json`，详细范围见 [76批恢复验收](windows_settings_gui_acceptance.md)。

工具471文件两树SHA `7afadab042e990b71fbcef2c79470404735b8c47ea34b88e5c917a303bc30011`；产品仍72批462文件7abf6237…，生产AOT/库/参考build/候选未变，无新包/提交发布。恢复时段宿主两目录/两代理键前后相同；最初暂停前后的代理差异仍未归因，不称整个76批宿主代理不变。8个测试KnownFolder根、mutex及应用/fixture无遗留。下文“目标paused”仅为当时历史记录，不再代表当前授权。

用户随后指定字体使用 `lxgw/LxgwWenKai-Screen`。该生产字体调整是下一项工作，必须另验证字体资产/许可、主题继承、构建与布局；不能使用本批旧AOT的GUI报告签收字体版。完整迁移仍保留真实校园、原始EXE/安装WindowsGUI、guest特权Full、长期、其它Linux桌面/Wayland、远端CI和最终审计。

第七十六批暂停交接（2026-10-18，优先于下文所有历史状态）：用户已明确暂停持续推进目标，本次仅落盘交接及只读核对，**目标保持 paused，76批未完成、未验收通过**。不要把下文历史“goal active”当成当前授权。最后完成的里程碑仍是75批；本轮没有新包、提交或发布，也没有改生产入口/AOT/原生库。

本轮已扩展4个开发工具/测试文件：`tool/gui_settings_probe.dart` 增加系统代理开关读回；`tool/windows_desktop_probe.py` 增加物理滚轮；`tool/windows_formal_gui_acceptance.py` 增加非法端口70000拒绝、18443保存、Full保存重开/Campus恢复、仅校园规则预览不落盘/应用版本递增/重开读回；`test/tool/test_windows_profile_desktop_acceptance.py` 增加离线状态严格校验测试。只有定向11项测试实际通过，Dart格式化无改动；新增GUI路径尚未实测成功，全量104项工具测试、analyze及Linux同步均未执行。Linux镜像仍停留75批，不能声称两树相同。

首次真实尝试位于 `build/desktop/windows-settings-gui/run-01/verification.json`，CLI退出1，`passed=false`。在 `01-welcome` 首次任务栏激活阶段，物理指针目标 `[1713,1872]` 与读回 `[957,1219]` 不符而失败；尚未进入新增设置操作，`launches=[]`、`steps=[]`，不代表未曾启动应用，也不能签收首进程正常退出。具体代码位置及恢复步骤见 [76批暂停记录](windows_settings_gui_paused.md)。运行时工具来源471文件 `f7d88a492ac77e8c22d8cf670f48119ed62cace3c1cfd1306e843c2490606c01`；原包仍72批462来源7abf6237…，不要混淆。

暂停核对：CLI已结束，CIM未发现 `shuvpn*.exe` / `proxy_fixture.exe`，两个本轮KnownFolder根以lexists确认不存在，验收互斥锁不存在，报告cleanup_error=null。`host-after-pause.json` 与 `host-before.json` **不相同**：原有两应用目录摘要保持，但HKCU代理键中的ProxyEnable摘要由对应1变为对应0，Connections下DefaultConnectionSettings/SavedLegacySettings摘要也变化。来源未归因，本轮不能宣称宿主代理保持；未擅自恢复这些用户系统设置。证据见 `build/desktop/windows-settings-gui/pause-state.json` 及两份宿主快照。

恢复须等待用户明确恢复目标。恢复后先核对桌面坐标/会话与代理差异，再处理首次激活失败，使用新的私有副本和报告目录重跑，不覆盖run-01。另需补规则overrides保留的独立校验，再完成两端同步、适当验证及新的来源/总汇；不能沿用75批总汇冒充76批通过。完整剩余迁移范围仍包括真实账户/校园、Windows原始EXE或安装GUI、独立可恢复guest特权Full、长期运行、其它Linux桌面/Wayland、远端CI和原计划最终逐项审计。

第七十五批当前状态（2026-10-18，优先于74批）：已重复运行正式lib/main.dart AOT的Windows私有版本资源副本，最终run-10/11各3个普通进程退出0。使用72批原始ZIP的37个载荷；AOT351535b4…等36项非EXE文件保持原摘要，EXE仅CompanyName/ProductName两个UTF16等长资源字符串改变12字节，范围外全部字节保持。实际原生存储落到新的Roaming KnownFolder/company/product，未依赖APPDATA覆盖；账户/设置/规则/连接backend/root/bootstrap全部正式接线，没有fixture入口替换。原包462来源7abf6237…与本轮工具471来源264c8d9ebfa008f9a3e5b5ab1c8cb0ca17ede6b05eff8a4134a7032f2dd1b2bf分开保存，两树工具来源相同。

真实欢迎→权限→账户说明→登录表单；仅向本轮密码字段输入局部假文本，原生无障碍值16字符遮罩，清空确认，未输入用户名或提交。UIA/MSAA保护标志仍false，不能冒充修复；验收依据是受控遮罩值及截图。首进程正常退出后，仅独立native预置welcomeCompleted=true再进入离线设置；第二进程GUI保存dark、关闭后native读回，第三进程重开dark/截图及再次native读回。主题自动渲染只核单点暗色，不称全页像素或UIA selected通过。任务栏Appid精确绑定复制EXE、真实鼠标/键盘、留存句柄与原CreateProcess创建时间、默认core路径及无自身监听均核对。

两端工具102（19/7平台跳过）、边界108通过，旧74开发bundle的已连接三轮12流回归通过；本批不重跑全量Rust/Flutter/安装矩阵或远端CI。原有两宿主应用目录及两代理键摘要前后相同，本轮所有测试KnownFolder根逐项无遗留；运行finally清理失败不升格，预检失败仍可能保留未用预留目录，不泛称全部失败零残留。早期图标/窗口/动画/缺保护标志失败保留。只加开发工具/测试，无生产代码/AOT/DLL/参考build变更；原候选未覆盖，无新包/提交/发布。总汇build/desktop/windows-formal-gui/verification.json，说明docs/windows_formal_gui_acceptance.md，goal active。

剩余范围保持：正式真实账户connectAccount/校园登录、原始EXE默认profile或实际安装Windows GUI、Windows完整离线设置矩阵、独立可恢复guest提升观察者/普通probe/真实UAC helper Full数据故障恢复、长期运行、其它Linux桌面/Wayland、远端CI及最终逐项审计。此次资源副本验证正式主入口代码运行，但不取代这些各自的证据。

第七十四批当前状态（2026-10-18，优先于73批）：Windows生产ShuVpnApp/ConnectionController/默认Rust库/持久DesktopConnectionOwner的私有存储本地协议GUI已重复通过，最终run-11/12，非上一批仅窗口/VpnSession。按钮连接/断开、原生托盘连接/断开、隐藏后四流、左击恢复、第三次连接后实际Up/Enter退出；每轮HTTP/SOCKS×IPv4/IPv6目标四条64KiB/prelude/FIN，共12流，各262144/262172统计与SHU-VPN决策匹配。普通断开owner可复用且session/TUN/egress/监听清理，最后owner.closed=true、原GUI/fixture句柄退出0、图标移除。端口timeout仍需真实双栈OS监听表为空，未冒充拒连；累计统计不扩大为UI实时速率验证。

共用开发入口Windows分支直接使用报告下新profile的原生auth/settings/rules及fromNative，不调用Windows KnownFolder/SharedPreferences迁移，不更改APPDATA假设隔离。账户仍用当前用户DPAPI，不是独立用户/网络沙箱；宿主两应用目录摘要及两处HKCU代理键摘要前后相同，非所有系统状态持续监控。UIA初始不暴露Flutter按钮，显式请求所属Flutter MSAA provider后可读真实语义；按钮ClickablePoint缺失/RuntimeID为空，采用唯一PID/名称/类型/实时矩形重复核对、WindowFromPoint与真实鼠标及后续状态屏障，不宣称原生hit-test或RuntimeID身份通过。早期显示/焦点/无障碍/hit-test失败保留。

仅开发入口/工具和测试改动，无生产Dart/Rust/原生runner变更。468文件来源eeed2e27796d99139f3a0283b7202d169d6c895feac3264396d4f13c52f5c9a0，两树一致；两端工具93（19/7跳过）、analyze、边界108通过。Windows原窗口探针当前驱动回归通过；Linux共用入口在私有net/X11/DBus/XDG与普通UID/CapEff0真实XFCE三轮12流复验通过，链路/路由/规则两时点相同。最后仅Windows报告范围字段拆分，再按最终来源构建Linux，关键产品与已实测字节相同，无须冒充重跑GUI。两端正式main构建已恢复，无新安装包/提交/发布；完整Rust/Flutter与安装矩阵不冒充重跑，沿用71/72批。总汇build/desktop/windows-connected-gui/verification.json，说明docs/windows_connected_gui_acceptance.md，goal active。

尚未签收正式bootstrap/connectAccount/真实校园、系统代理/TUN或Windows Full。继续独立可恢复guest提升观察者/普通probe/真实UAC helper及Full数据故障恢复、真实校园/长期、其它Linux桌面/Wayland、远端CI和原计划最终逐项审计；完整GUI生产入口与真实账户仍须各自证据，不能用fixture替代。

第七十三批当前状态（2026-10-18，优先于72批）：Windows真实Explorer托盘开发入口闭环已执行，最终run-21/22及第72批原生文件+仅开发AOT覆盖的run-23/24均通过。实际Alt+F4隐藏后64KiB代理仍工作，左击恢复、右击HMENU状态/协议子菜单、点击显示、实际Up/Enter退出均走原生回调，命令严格show/show/quit。两流统计131072/131086；GUI/fixture原句柄退出0，图标消失，系统IPv4/IPv6目标监听表为空。连接timeout本身不作为端口清理证据。身份包含nonce/PID/创建时间/映像SHA/普通权限与真实Explorer会话；不发合成托盘回调。截图已查看。

仅改开发窗口入口、三个Python开发工具和工具测试，生产runner/临时托盘焦点实验全部撤销。两端87工具（Windows19/Linux7跳过）、analyze、边界108、Flutter宿主/生命周期15及原生菜单4通过；自动模式也退出0。466文件两树来源d33a2b4dc29fd2b835b6bf1478d389e0ebeabaacd76966bbd127f67948027f1a。正式lib/main.dart构建已恢复、AOT351535b4…；重新链接EXE/tray字节与72批不同，不作字节可复现声明，故额外使用72批原始原生字节覆验。72批安装候选未覆盖，本批未重跑完整Rust/Flutter/安装矩阵。总汇build/desktop/windows-tray-input/verification.json，说明docs/windows_explorer_tray_acceptance.md。早期定位/End/timeout失败保留，End未签收，无提交发布，goal active。

此入口没有账户/正式main/ConnectionController/持久owner，使用真实Rust VpnSession及本地TLS fixture；不得签收Windows完整已连接应用、真实校园、TUN/系统代理或Full。下一步推进私有账户路径的生产界面/控制器GUI；不能只改APPDATA假设KnownFolder隔离。独立可恢复Windows guest提升组合、真实校园/长期、其他Linux桌面/Wayland、远端CI及最终逐项审计继续保留。

第七十二批当前状态（2026-10-18，优先于71批）：已将71批两处断开/重连UI修复集成到新的成对桌面安装候选，位于 build/desktop/connection-candidate/package。版本仍0.4.2+9、状态partial-desktop-candidate，使用71批已构建且已受测的正式lib/main.dart产品，未将fixture AOT装包。Windows ZIP/Inno、Linux tar/DEB的payload/builder来源与两树重算均为462文件SHA `7abf62372522eddd35b7279eddb82ea8b44604181422d616497eadd7d8b78f33`。本批不冒充重新执行完整Rust/Flutter矩阵；317/316 Flutter、analyze等沿用明确的71批证据，新包核心产品摘要逐项匹配。

新Windows实际临时安装/合成旧版本升级/卸载、运行及事务保护、runner安装中1618拒绝、4套安装DLL探针及原生账户恢复/非包文件保留均通过；安装后逐项核对37个manifest文件。Inno仍NotSigned。新DEB在私有mount/net/PID空间完成实际拒绝/配置失败重试/升级/remove/purge/重装、3轮普通用户校园TUN、当前安装默认库+固定helper的10阶段Full全数据/覆盖变化/崩溃重建/路由与控制绑定恢复，以及已安装离线设置GUI和托盘三进程各一组。6个GUI均UID1000/CapEff0、maps为/opt/shuvpn默认库、退出0，截图已查看；宿主账户/包数据库摘要保持。最终Windows注册与应用/helper0，Linux外层3个宿主PID已消失、固定路径进程0且宿主未留下安装。

产物SHA：ZIP `0dc3fc111b5afeac5bac83412e849b72c37155c03aa9293f26985d91fe706dfd`，Inno `619d6eea6b4dfd21e73c70c2a64227d1cfb9ebf7e07a47a8dafc03feed265871`，tar `a709efb1dec7a2d67d1737c42ac8b777de6299ebe3fe5e65b852f91015b61c45`，DEB `b3bdb6f7ab822dcd374dee12660fdab4da18e60a683b5a266e4318db38620395`。总汇 build/desktop/connection-candidate/verification.json，安装Full/GUI/tray报告均在同目录。独立复核确认归档37/29文件完整性、11项71批核心产品匹配、DEB实际核心及固定helper相同、旁路/输入/来源摘要匹配；Inno内部载荷由主线程实际安装后核验，不能只凭metadata替代。旧Windows68/Linux67目录未覆盖。

顺带修正当前贡献指南：移除已删除Android工具/NDK/模拟器的活动命令，保留仅Windows/Linux范围说明；改正“Windows helper未接正式会话”的过时描述并保留最新特权Full待验收限界。LLM政策平台示例同步Windows/Linux。文档不在来源工具的462代码文件指纹中，另封存当前文档SHA。没有生产代码/绑定变更、没有发布或提交，3名只读代理已结束，goal active。

下一步Windows真实Explorer托盘路径已定位，尚未实际执行：主窗类FLUTTER_RUNNER_WIN32_WINDOW、图标回调为注册SystemTrayNotify消息，实际菜单经TrackPopupMenu返回ID再回Dart，直接发WM_COMMAND或托盘消息不能代替真实图标点击。可先复用不打开账户的tool/desktop_window_smoke.dart开发入口，补外部等待/身份屏障后驱动Explorer真实鼠标与菜单，再推进已连接GUI。path_provider_windows2.3.0实际通过SHGetKnownFolderPath(RoamingAppData)创建support目录，不能未经验证就假设改APPDATA环境足以隔离正式入口账户；未具备私有profile前不要据此启动正式账户路径。独立可恢复Windows guest的提升观察者/普通probe/真实helper Full、真实校园/长期、其他桌面/Wayland、远端CI及最终逐项审计继续保留。本批未运行Windows GUI/Full/UAC，未新增guest证据。

第七十一批当前状态（2026-10-18，优先于70批）：已完成本地协议 fixture 的真实 Linux 已连接 GUI/托盘闭环，并修复两处生产 UI 接线问题。DesktopHost 原先把 disconnectForExit 同时用于托盘断开/退出；现分别使用普通 disconnect 与终止 shutdown。实际 GUI 又暴露控制器在命令 future 清除前发出最后通知，导致断开后托盘连接/协议仍禁用；现清除后再通知。host 组件及普通/终止两条监听回归均先失败再通过，生产3个 Dart文件亲读相应代码后更新108文件边界清单/说明，没有新增Dart后端实现。

开发入口 tool/connected_desktop_app.dart 使用生产 widgets/controller、默认随包Rust库、真实原生账户/设置/规则存储及持久owner；账户为测试导入，连接入口改为本地TLS fixture connectPlan，不能签 production bootstrap/connectAccount、真实登录、TUN或系统代理。root仅创建loopback私有net namespace，GUI/fixture/独立客户端均UID1000、CapEff0，另有私有X11/DBus/XDG与真实XFCE宿主。按钮连接断开→托盘连接断开→托盘再次连接后实际popup退出三轮、12条双栈HTTP/SOCKS 64KiB/prelude/FIN流通过，native decisions逐目标匹配SHU-VPN；最终每轮upload262144/download262172。普通断开closed=false/无run/无cleanup，退出closed=true/无run/无cleanup、TUN stopped、egress inactive/无socket、端口拒绝、原fixture pidfd退出0及GUI退出0。截图已查看。gui-01/02保留busy缺陷失败，gui-03是统计收紧前的成功，最终为gui-04；独立复核后补全统计及最终session检查、fixture身份和bundle前后SHA。只比较链路及双栈全部路由表/规则前后相同，不宣称所有宿主网络状态持续不变。

Windows/Linux正式入口release已重建；Linux正式AOT已恢复，并另通过原首启/guest/托盘宿主丢失热恢复三进程，均退出0。两端完整Flutter317/316、analyze、工具74（Windows19/Linux5平台skip）及Dart边界108通过；新Windows bundle实际FFI67、桥接、随包helper普通权限IPC4及Win32菜单4通过。Windows首次未设Wintun DLL、Linux无.git镜像未设source环境、首轮宿主监听单测以及一次PowerShell/WSL路径展开导致正常GUI找不到bundle的失败均保留；最终正确路径/环境通过，不把这些当成产品修复。完整Rust/Full/安装矩阵本批未重跑。CI接本地协议GUI并在EXIT恢复正式入口，两端verify接GUI guard选择器；只做本地语法/测试，未远端执行。

462文件两树SHA `7abf62372522eddd35b7279eddb82ea8b44604181422d616497eadd7d8b78f33`，汇总 build/desktop/connected-gui/verification.json，说明 docs/desktop_connected_gui_acceptance.md。Linuxcore/helper保持 d4732d4f… / 061c64a1…；Windows新bundle的core/helper为13a6d6e7… / 5b1d8eba…且已按随包路径复验，不能套用旧包摘要。没有新安装归档或发布，Windows68/Linux67候选仍各自旧来源，尚不含本批UI修复。3名只读代理结束；核对Linux25个本轮PID全部消失，Windows工作区应用/helper进程0，无提交，goal active。

下一步：继续Windows真实托盘/已连接GUI和当前修复的安装候选集成；Windows特权Full仍需独立可恢复guest的提升观察者/普通probe/真实UAC helper组合及两轮数据/故障/重建，不能用本批代理GUI或70批普通Plan替代。真实校园登录/长期、其它Linux桌面/Wayland、远端CI及原计划最终逐项审计仍未完成；本批没有新增Windows guest证据，没有在开发主机启用Full/UAC。停止并确认原生清理后才替换完整配套包；撤销两处UI修复会恢复已复现缺陷。

第七十批当前状态（2026-10-18，承接10月17日，优先于69批）：已实现普通probe与独立观察者的分离接线。普通probe创建单实例/拒绝远程的随机管道，观察者以SECURITY_IDENTIFICATION连接；双方通过OS管道对端PID打开query/wait handle，核对不同PID、相同映像/SHA及完整创建时间/SID/elevation。Helper观察者必须预先提升、probe必须普通；local双方按角色验证。没有让观察者代开TUN、终止helper或自行提升，也未把UAC操作系统父链当成归属保证。tun-probe特性下从正式Client实际持有的helper handle采集PID/原始创建FILETIME，沿helperIo.process返回；不改普通FRB API、helper wire或业务策略。

阶段严格为begin0/ready0/cleaned0/ready1/cleaned1/finish0；Plan仅begin0/cleaned0/finish0。观察者在ready核对精确helper实例/固定companion映像/SHA/提升、同compartment同接口的各条批准路由；cleaned等原helper退出0并比较All快照；最后先ack让probe退出，再通过原handle确认probe退出0并再读快照，才设置passed。失败即使后续清理成功也保持false。快照PowerShell从probe SID的HKU读代理配置，已固定到二进制中，实际查询内容随exe SHA固定；执行系统目录的PowerShell、对子进程指定执行参数/UTF8，不改系统执行策略、不接收客户端命令/特权文件写入路径。

已实测当前二进制的两个真实普通进程Plan：observer81120/probe81344，nonce/OS管道身份、创建时间与启动方实际Process对象、三阶段、正式Full预览/无TUN、原probe退出0后网络基线及observer退出0全部匹配。Current非空快照46路由/18地址/14IP接口/23适配器/14DNS前后相同，helpers为空；没有把它称作提升组合或All/Full数据实测。最初PATH缺powershell.exe、之后系统PowerShell禁止脚本两次均真实失败，日志保留；系统路径与仅子进程执行参数修正后通过，再将脚本编译固定后复验通过。普通token --observe-helper拒绝已验证，无UAC。

Windows example10（含真实退出7拒绝）、平台默认49/特性50、真实普通helper IPC4通过；Linux example3/默认平台35；两端工具66（Windows19/Linux5平台skip）、特性lib/tests/example Clippy与Dart边界108通过。新Windows监督5条负例覆盖查询错误/空表及缺原生记录/错误nonce/null表，现有verify/CI选择器会运行；未远端执行。独立复核核心协议未发现失败升格或最终循环等待，路径/策略及编译固定部分由主线程实测验证。457来源文件两树SHA `2ddaa84eec83e2cd8465216db2265baf0d1aa3918034dbdc73f86a7bc1170078`。汇总build/desktop/windows-full-observer/verification.json，当前匹配预演preview-embedded，说明docs/windows_full_capture_acceptance.md。SDK增加authorization仅供SID查询，无生产Dart改动；未做Rust/Flutter全量、完整应用/安装器、发布或提交，旧Windows68/Linux67包继续保持原来源。三名只读代理结束，当前进程清理检查0。

下一步：分离观察者的代码接线已具备，仍须在独立可恢复Windows guest执行提升观察者/普通probe/真实UAC helper组合及两轮数据，不能用两个普通进程的Current预览签收。继续接覆盖变化/外来路由保留、helper崩溃重建、DNS歧义/HTTPS，以及正式Flutter/owner/安装默认库路径；当前helper ready/退出校验仅编译与契约，不是已运行的提升helper证据。环境库存沿69批限界，本批没有新增Windows guest证据或启用虚拟化；没有在开发主机启动Full/UAC。真实tray/已连接GUI、校园/长期、其他桌面/Wayland、远端CI与最终逐项审计仍待完成，goal active。

第六十九批当前状态（2026-10-17，优先于68批）：新增独立desktop_windows_full_smoke原生example，明确以CaptureMode::Full调用正式desktop_tun_plan和start_desktop_tun，旧campus探针不变。已实际执行当前调试/tun-probe版的Full正式只读预览：本地TLS fixture真实握手，正式API读取当前Windows路由/地址并生成780分区、MTU1400、DNS capture=true；检查校园及独立双栈peer被捕获、物理源被保护，预览没有启动TUN，finish_stop完成。不是手写配置打印，也不是特权Full数据通过。

新增可移植独立双栈peer（真实TCP/UDP/DNS与token观察账本）及两轮正常运行分支，后者仅编译/静态核验、未运行：DIRECT双栈TCP/UDP、64KiB/FIN、校园signed UDP/双栈TCP、原生决策；REJECT/未授权SHU-VPN双栈失败与peer TCP accept不增；DNS A/AAAA/UDP/TCP/TC重试/坏ID的独立新账本；停止TUN后控制绑定原样恢复、再次读取peer拒绝迟到blocked接入，再会话退出及新轮重建。保留每个连接后错误到finish_stop的清理路径，未把它签成helper崩溃矩阵或生产Flutter/owner。

监督脚本full_capture_windows_smoke.ps1首轮暴露并已修复自身假通过：跨compartment读取拒绝后空表曾被当作相同，旧preview/verification.json已作废并保留原JSON/log。现所有查询显式捕获错误、拒绝空关键表，两条PowerShell真进程fixture回归通过。Plan明确仅当前compartment，最终非空快照路由46/地址18/IP接口14/适配器23/DNS14前后相同；运行模式仍要求all-compartment，不自动降级。当前普通进程all查询拒绝已用无害Python占位程序复现，未启动程序。独立复核另发现UDP/blocked端口碰撞和SERVFAIL不足以证明坏ID，均已补契约/独立DNS查询ID账本；重复规则更新也使用实际committed version。Linux3.10测试缺少asyncio新UDP API的失败已修正，保留日志。

453来源文件两树SHA `7247e1672020b3fcee2a916367c64098ff58cb689812d990472176af8686e737`。Windows/Linux example契约4/3、针对该example Clippy、两端工具各63（Windows19/Linux2平台skip）、Dart边界108、PowerShell解析及bash -n通过；peer3项真实loopback服务测试和Windows监督2条负例计在工具内。verify两端/native CI接入，未远端执行。汇总build/desktop/windows-full-driver/verification.json，复现步骤及限界docs/windows_full_capture_acceptance.md。没有生产Rust/Dart API变化、没有完整应用/安装包重建或Rust/Flutter全量重跑；Windows仍68批包、Linux仍67批包，各自保留旧来源；无提交发布、goal active。

下一步：先补普通probe与特权all-compartment观察者的分离接线、精确helper进程句柄/身份/退出见证；同一普通监督器不能读取all时应拒绝，同一elevated probe也不能冒充ordinary-helper。再在独立可恢复Windows guest完成当前两轮数据、覆盖变化/外来路由保留、helper崩溃后重建、DNS歧义/HTTPS，并补正式Flutter/owner自动启动及安装默认库验收。只读环境核验仍未确认可恢复Windows guest：命令/常见工具路径未发现，Hyper-V数据目录拒绝读取；hypervisor=true/WSL2 Ubuntu不能替代guest，不能断言全盘不存在VM。没有在开发主机启动Full或UAC；也没有更改系统虚拟化功能。真实Windows tray闭环/已连接GUI、校园/长期、其他桌面/Wayland、远端CI和最终逐项审计继续保留。本批三名只读代理结束，最终进程清理检查见证据。

第六十八批当前状态（2026-10-17，优先于67批）：已把Windows原生菜单线索推进为真实复现及修复。system_tray2.0.3创建checkbox时checked覆盖enabled，SetEnable/SetCheck也互相覆盖状态位；应用实际按enable→check更新协议项，禁用会被最后一步取消。无窗口HMENU回归在原上游四组中三组失败；定点适配后三组及身份/标签控制组共四组通过，覆盖顶层/子菜单ID、四种初态、反复更新及独立default位。只替换Windows menu.cpp的三处状态处理，原头文件/回调/其余实现保持；上游SHA门控、不改pubcache、MIT许可安装并纳入Windows打包必需项。独立复核未发现所查范围内新增实质问题，生成vcxproj确认生产恰好编译一份仓库内menu.cpp。测试不安装，verify_desktop.ps1/native CI显式执行，未远端执行。

当前448来源文件两树SHA `82ad3b92ec15542c393e55204b8a6b789a87a942edb8739881df0a842b214ae6`。新Windows release、针对性Flutter桌面/原生库82、两端工具58（Windows19平台skip）、新ZIP/Inno来源/载荷/旁路摘要、实际临时安装升级卸载/保护及账户保留均通过。core/helper逐字节等于67批，tray DLL已改变；不冒充重跑Rust/全量Flutter、Linux release或新成对候选。新包位于build/desktop/windows-tray-state/package，ZIP SHA019405e20dab4c4b2c163fdf1a6d4cc9705b9609bb5da3f438c4997d23c42ef7、Inno SHA7173ffd45598f043b5fe5e6698b627cfc0dd666ae9c90319d8977a012090701b，仍NotSigned。证据windows-tray-state/verification.json，详见docs/windows_tray_menu_acceptance.md。Linux继续使用67批完整复验产物的原来源，不能声称已按68批重建。

下一步仍优先Windows Full专用入口与可恢复独立环境。只读复核及主线程抽查再次确认desktop_windows_tun_smoke.rs:42入口只接受旧三种模式，:66为campus三条host route、dns_capture=false；--plan在:79返回，正式desktop_tun_plan在:129之后才调用，不代表正式Full计划。可复用Linux full_capture_adapter_smoke.dart的RustConnection previewTun/startTun显式CaptureMode.full与owner自动启动链，但其硬性Linux namespace/固定路径断言不能原样搬到Windows；需亲读拟改代码和API后实现完整驱动，保留独立DIRECT双栈TCP/UDP、DNS、REJECT/对端无接入及全系统恢复/重建证据。Windows实际托盘popup/指针选择/应用回调闭环、已连接GUI、真实校园/长期、其他Linux桌面/Wayland、远端CI与原计划逐项审计仍待完成。未提交发布，goal active；全部本批测试/安装进程和两名只读代理结束。回退应先停止并确认清理，再替换完整旧Windows包；撤菜单适配会恢复本轮已复现状态错误。

第六十七批当前状态（2026-10-17，优先于66批）：已复现并修复再次激活当前aTrust后原生勾选消失。GTK先toggle，ConnectionController同值返回不通知，原菜单快照去重导致界面留在未选中；实际red文件toggle-state=0。DesktopLifecycle以UI刷新标记确认最新快照，在await前消费而非事后清除，允许在途再次事件置位；拒绝选择仍重画状态，退出与菜单任务等待契约保持。两条捕获原生回调/在途拒绝回归先失败后通过，独立复核未发现新增实质问题；生产Dart该文件亲读后更新SHA/说明，仍为platform-ui，不是后台重试/协议迁移。

真实Linux首启、Guest和宿主热恢复三处原生Event重新选择后均勾选正确，安装版设置/托盘六个普通用户PID、库maps/SHA/pidfd匹配、宿主热恢复及退出0通过。另补Full双栈父子分区重叠（并集未变）拒绝与保持dns_capture=true时physical DNS缺失/null/空数组拒绝，均为纯契约检查。Windows/Linux过滤平台Rust49/35通过；不是Windows Full专用入口或特权验证，原--plan仍为campus fixture。

444来源文件两树SHA `458203f79ef0c63231b50d373c94387f37ffff44e2ef29333218267e33fdfd12`。两端完整release、Flutter314/313、analyze、Clippy lib/tests、工具57（Windows19平台skip）、Linux GTK4、Dart边界108通过。配套ZIP/Inno/tar/DEB在build/desktop/tray-protocol/package，源/载荷/旁路校验通过；Windows实际安装升级卸载/保护与原生账户保留、新随包helper普通权限真实IPC4通过。Linux实际安装生命周期、账户/非包文件/宿主状态保持通过，仍未做真实登录或已连接GUI。

重建Rust产物后发现core摘要仍为d4732d4f…，helper变为061c64a1058778b8a48f2564f45732b05c9a4ae2780f13ceaf64f199965f72c6（Rust修改仅cfg(test)，不据此假定字节相同）。已对当前DEB补跑安装版默认库/固定helper的普通UID/CapEff0 Full全数据/故障矩阵，十阶段、helper崩溃与重建、覆盖变化、REJECT/授权/歧义无物理接入、全部路由及控制绑定恢复均通过。新DEB SHA119c03bf89af7d6251ad73244bd59e2f2f09866c0f631bb374b7a0ddfa584821；Full证据installed-full/verification.json，总汇tray-protocol/verification.json。没有冒充重跑Rust全量/all-features、Windows特权Full或远端CI，候选仍partial-desktop-candidate、Inno仍NotSigned，未提交发布，goal active。

下一步：优先准备Windows Full专用真实计划/运行驱动与独立可恢复环境，不能用新纯契约测试替代完整出站/恢复证据。另有只读线索：system_tray2.0.3 Windows menu.cpp的SetEnable/SetCheck均以MIIM_STATE全量设置fState，应先用不显示窗口的HMENU原生回归核对启用/勾选位是否互相覆盖；当前Flutter mock与安装器验证不代表Windows菜单已签收。继续已连接GUI/托盘断开退出、物理协议子菜单、真实校园/长期、其他桌面/Wayland、远端CI及原计划逐项审计。更多限界与回退仍按66批及验收说明，停止并确认清理后完整替换配套包，撤UI确认逻辑会恢复本次已复现问题。

第六十六批当前状态（2026-10-17，优先于65批）：正式Linux托盘已完成真实XFCE宿主验收，并修复依赖中被mock掩盖的问题。system_tray2.0.3的Linux运行时label/enable/check均为空实现，bool参数也错误检查string；真实菜单在等待后仍保留连接/断开启用与aTrust未勾选，红日志保留。仓库内MIT定点适配只替换menu.cc，按上游SHA校验并使用原头文件，不改pubcache或其他平台；更新原GTK对象/ID、拒绝非法参数并抑制程序勾选产生用户选择回调。未采用反复重建菜单，未更改Dart/Rust后端。

新增独立tool/desktop_tray_acceptance.py，复用私有显示/profile/清理基础。验证普通UID1000/CapEff0、实际配套库、SNI应用PID、真实host注册与watcher的panel父链/可执行文件/摘要。三个app PID覆盖首启禁用状态、关闭隐藏、原生Event恢复/退出、Guest连接进入账户页和空表单、host丢失后关闭退出、重启app/host及同一app运行期间host热重启后重新注册。真实右键以所属XEmbed窗口定位，确认所属PID的popup后用Home/End+Return选择显示/退出，截图已检查；不把原生Event称作鼠标操作。工具初轮误将任务栏按钮当tray、遗漏tooltip排除和页面切换时AT-SPI空child均按实际失败修正，日志保留；未弱化业务状态/退出码断言。独立复核补上host注册/父链、完整菜单就绪与清理保留原异常；相关回归通过。

444来源文件两树SHA `2acc46e6a54034d49b0f0b8f3aa161d7b67dbac85df196364d28e09f49dbed4d`。Linux release/Flutter311/analyze、原生GTK4、两端工具57（Windows19平台skip）、Dart边界108通过。新Linux tar/DEB在build/desktop/tray-acceptance/package，tar SHA655f9faeffb9573f7d587f2c5b2e5f8cc5e0d2aaa03847c109d07e68f7b7c009、DEB SHA802f99a22ad68790735d6e87e6092078fa4394502f36b123217b4f1dc4ac074d。来源/载荷/许可证/旁路摘要核验；core/helper等于既有d26c受测字节，未新跑Rust/Full全矩阵，没有新的成对发布候选。

实际新DEB两轮隔离安装生命周期均0，包含离线设置3PID、托盘3PID、一轮校园TUN数据与退出屏障、账户和非包文件保留及宿主账户/dpkg不变。首个maps观察器启动晚仅5PID并返回失败；重跑时自driver启动附着，完整6PID的exe/core/tray maps、SHA、UID/cap与pidfd存活均核对，namespace PID与GUI报告逐项匹配；旧部分记录不计作完整通过。证据build/desktop/tray-acceptance/verification-final.json及docs/desktop_tray_acceptance.md。CI和本地verify接入native GTK、bundle/安装tray场景，上传排除profile；未远端执行。

下一步保持Windows Full为优先：只读调查确认现有Rust/Dart探针仍省略CaptureMode，原--plan甚至在正式API前返回；需建立Full专用入口与独立DIRECT双栈TCP/UDP、DNS、REJECT/无对端接入、全系统恢复和再启动证明，不能沿用campus三个host route断言签收。可先做固定输入计划/恢复判定器测试，特权运行仍需可恢复的独立Windows环境。托盘目前验证的是初始协议选中/禁用状态，下一步还应实测再次点击当前协议后的幂等显示，不能仅以GTK setter测试替代用户回调闭环；继续已连接GUI/托盘断开退出、其他桌面/Wayland、真实校园/长期、远端CI和原计划最终逐项审计。既有Flutter/私有XFCE警告保留；运行时菜单图片替换未用亦未修复。未提交发布，goal active；回退须停止并确认清理后使用完整旧包，撤适配会恢复已复现的错误托盘状态。

第六十五批续作当前状态（2026-10-17，优先于64批）：接手工作树已有离线GUI脚本、文案修正及80d802b1c576产物，但安装版最后仍SIGABRT、汇总缺失；未执行旧脚本将其冒充成功。便携版三次GUI操作/退出复跑通过；安装版在修正DBus私有目录后仍真实复现第一次关闭-6，保留红日志。

已修正两个独立问题：私有XDG profile/runtime须在xvfb/DBus启动前准备，新增回归先失败后通过；Linux runner在window destroy前保留view子树，等待engine dispose中的Shutdown返回后在GTK idle释放并解除application hold，避免raster仍在绘制时释放compositor mutex。没有修改Flutter SDK、Rust后端或退出断言。独立复核未发现所查引用链的新闭环；仍须保留额外engine引用延迟退出及更多GPU/联网GUI场景的验证边界，不以超时强行释放。

新Linux完整release/tar/DEB来源438文件SHA `74dbae1ac5981e8ddfa3c91d98d0fcfb65d9a495b9525357b47bf4b5d31aa645`，两工作树一致。实际新DEB在私有namespace完成UID1000/CapEff0、/opt配套库加载、三次GUI首启至表单/离线设置/重开与退出0；深色、Full/Campus保存、非法端口拒绝/合法端口18443、规则预览不生效/应用及独立native REJECT读回通过。welcomeCompleted仍由测试准备单键，不是登录成功。安装版一轮校园TUN DNS/动态路由/TCP与owner退出屏障、安装升级卸载/purge/重装、账户及非包文件保留、宿主账户与dpkg保持均通过。截图已检查，不声称所有动画、布局与完整GUI签收。

Linux Flutter311、analyze、两端Python工具51（Windows19平台skip）及Dart边界108通过。core/helper重建摘要等于既有d26c受测字节，无新Rust/Full全矩阵；既有implicit-view/messenger/OpenGL警告仍保留。CI已加安装版GUI及失败证据上传，未远端执行。证据`build/desktop/gui-acceptance/verification-resume.json`，新Linux验证产物`shutdown-package/`（tar SHA4370ee5b2272921185d052ed8c88166263939571ee0b81aaf93b258943482880；DEB SHA1e419956989d81af63f55ac0a2f22b12d52dfc50abe0b52a0e94a925015744d2）。没有新的成对发布候选、没有提交发布。

下一步仍优先补Windows特权Full专用驱动与可恢复的独立验收环境；现有Windows入口只证明campus定向捕获，LinuxFull驱动明确拒绝Windows，不可混用签收。可并行继续有托盘隐藏/恢复、联网GUI与更多关闭/渲染覆盖，随后真实校园/长期、远端CI及原计划逐项最终审计。目标保持active。回退须停止并确认原生清理，再替换完整旧包；撤销runner的暂存逻辑会重新暴露本轮已复现的关闭问题。所有本轮进程与只读代理均已结束。

第六十四批当前状态（2026-10-17，优先于63批）：推进M5.4源树边界复核并建立持续门禁。先只读核对Windows隔离条件：Get-VM/Hyper-V模块、VBoxManage、qemu、WindowsSandbox入口及常见安装位置未发现可直接使用的Windows隔离VM；HypervisorPresent=true不代表已有此类验收环境。本批没有在开发主机上启用Full或触发UAC，没有修改系统虚拟化功能。Windows特权Full仍是明确缺口，未用下述审计替代其签收。

当前生产Dart共108文件，31生成、77手写，逐文件清单`docs/dart_backend_boundary_manifest.json`记录规范化LF的SHA、角色与复核说明，运行直接依赖11项、pubspec与锁、生成输入及Rust绑定也封存。非生成core36文件全文复核，features/shell/widgets30文件枚举/导入异步搜索与关键候选代码核验，主线程复核app/main装配/视觉路由、原生落点和关键证据。无确证独立Dart后台网络、Cookie/凭据/配置数据库、业务重试或旧SDK回落；分页视图去重、原生cache snapshot、提交后用户服务命令、原生挑战快照计时、UI协议/登录预检及窗口/托盘平台操作均有边界判定，不因await/JSON/Timer无限追加迁移。五处手写dart:io仅为平台/目录/原生库/托盘宿主探测及引导文案，详见`docs/dart_backend_boundary_review.md`；本次不冒充30个UI文件逐行完整人工阅读或M5.4最终完成。

固定FRB2.11.1对当前API重生成31个Dart文件和Rust绑定，32输出逐字节一致，未改生产源码。生成物含Freezed输出；web:false仍有JS条件stub引用，只有实际该条件分支有明确缺失目标例外。新增`tool/verify_dart_boundary.py`及15项回归，检查完整覆盖/已复核内容、角色、运行依赖/锁、普通与条件imports、平台IO说明和生产代码跨入测试后端。只读复核发现metadata可提前结束扫描、注释可伪装JS缺失分支，主线程用2项失败实际复现后修正，新增metadata及泛型未支持形式明确拒绝的回归；代码/清单通过不是完整Dart语义证明。

两条CI与两端verify_desktop均已接门禁，文档清单变更纳入native流程触发；Flutter CI固定3.47.2并enforce-lockfile，与native矩阵一致，贡献指南取消了自动升latest的冲突说明。没有执行远端CI。最终Windows/Linux工具各45项通过，Windows17项平台skip；两端清单108覆盖通过，Flutter analyze通过，实际d26c归档默认库的启动/账号边界/命令取消退出/登录挑战/原生设置与恢复等针对性测试两端各99项通过。没有冒充重跑未变更的完整Flutter/Rust/安装/Full矩阵。

证据`build/desktop/dart-boundary/verification-current.json`，含源树清单、依赖/生成输入SHA、32生成物复现、红/绿日志及配套库摘要。产品继续为d26c2827760b；无新库/应用/安装归档，无提交发布。全局goal仍active，M5.4为当前静态复核与局部运行证据，最终仍需原计划逐项审计及Windows特权Full、完整GUI、真实校园、长期和远端CI。全部本批进程与三名只读代理结束，重生成没有遗留变化。下一步保持Windows平台验收为优先项，并继续可独立完成的GUI/最终运行覆盖；不能把源码清单检查器当作全部平台已验收。

第六十三批当前状态（2026-10-17，优先于62批）：已补齐Linux已安装固定路径默认库的普通用户Full验证。验证对象保持第62批`d26c2827760b`原始DEB，SHA `bc39bcfccd1347bf218b19deb0b839210088f6c5ad73f4afe29c19cc8bb8ae97`；本批只改测试/CI/文档，没有更改lib或Rust生产代码，没有重建应用与安装包。实际安装库SHA `d4732d4f1521809dab8f6ef0ab99817da4f19174cce7cde5352a4b719da3cc38`、固定helper SHA `1db5ef317845320747a50e6987ff884d47cc5fa5cce01705ccae7d0676456858`均在运行前后核对原始DEB payload，并与62批清单一致。

新增安装模式由`linux_deb_smoke.sh`调用root driver，使用私有mount/net/PID与独立物理peer，Flutter测试引擎保持UID1000、四类capabilities全0、NoNewPrivs0。明确加载`/opt/shuvpn/lib/libshuvpn_core.so`并从自身及root侧maps确认，不使用helper locator或probe feature例程。root观测三个真实Full阶段的`/usr/lib/shuvpn/shuvpn_tun_helper`，绑定实际Flutter父PID、exe/inode/SHA/netns/UID与capabilities，并在身份观测前后检查pidfd未退出；最终由原pidfd确认退出且无替代进程。GUI、Dart runtime与Flutter引擎均无file capabilities。

真实安装库通过原生owner自动Full、RustConnection手动Full、停止重建三个会话。数据包括校园IPv4 UDP本机源转换、双栈签名TCP、非校园DIRECT双栈TCP/UDP、64KiB TCP及FIN、DNS A/AAAA/UDP/TCP/物理上游TCP回退/错误ID、域名映射与歧义、HTTPS受信与不受信证书。TUN入站原生决策与字节收发、OS路由及独立对端物理源地址共同核对；REJECT、授权不足和歧义负例前后，对端TCP接入数没有增加。root注入额外物理主机路由后捕获变化自动停止且保留该路由；再次Full后通过原pidfd强杀helper，核验session/owner错误并重建；第三次正常停止时在会话仍connected、控制socket仍存活时核验原接口绑定恢复，随后确认owner退出屏障、监听拒绝、TUN/原helper消失以及IPv4/IPv6全部路由表和策略规则与基线相等。

初轮错误是测试假设shutdown必返诊断：owner已自动清理并保存故障，终态shutdown确认后可返回None。现于确认前严格核对session failed/message和owner failed/errorMessage，确认后检查closed/无run/无cleanup/出口恢复；未改生产错误语义。扩大路由核验后发现新增物理link-local local路由，无VPN对照与原始差异证明是veth的IPv6 DAD完成；现先等待实际地址状态稳定再取基线，保留全表严格比较，未过滤该路由。根因/失败日志均保留。

两次只读复核修复新root协议的路径与漏阶段问题：固定十阶段顺序与终态完整性必须满足，确认文件用已打开目录fd、O_EXCL/O_NOFOLLOW创建；绝对路径、穿越、符号链接、乱序/重复和漏故障有5项独立回归。SDK位于/opt时先保留底层再在原路径挂私有overlay，缓存写入标记不进入宿主SDK；本地模拟/opt SDK场景连同完整DEB生命周期通过。先前模拟外层wrapper未卸载SDK就清临时目录的EBUSY，以及findmnt同时返回隐藏旧挂载与新overlay的误判均已按实际证据修正；最终以当前路径statfs确认overlay，正常卸载后只rmdir自有空目录。

最终增强场景、完整DEB安装/升级/remove/purge/重装/授权拒绝/账户及非包文件保留、宿主账户与dpkg不变均退出0。原root Full/Campus adapter探针回归通过，Flutter analyze通过，Linux工具30项通过、Windows30项含17项平台skip；Bash语法与Python编译检查通过。没有冒充重跑未变更的两端Rust/完整Flutter/应用构建矩阵，沿用62批产品证据；CI已接新安装模式但没有远端运行。验收说明见`docs/linux_installed_full_capture_acceptance.md`，汇总`build/desktop/installed-full/verification-current.json`与候选新增的`verification-installed-full.json`，记录产品来源、工具SHA、真实进程、路由/绑定、日志与限制；原62批历史verification.json保留。

全部本批进程及四名只读代理结束，未提交发布。完整目标仍active；剩余优先为Windows当前特权Full与恢复、完整GUI/真实校园/长期运行/远端CI及M5.4最终逐项审计。不能将本批本地协议fixture与Flutter测试引擎扩大为真实账号或完整GUI签收；本批未修复或重验既有Linux GUI关闭警告。回退仅撤本批测试入口/CI接线，运行产物仍使用配套d26c包；生产库/绑定不混用。

第六十二批当前状态（2026-10-17，优先于61批）：正式“网络连接”页已提供默认校园分流和实验性完整捕获选择，保存仍由Rust提交。模式贯通自动连接、手动启动与预览，预览另传所选物理接口；运行状态及连接抽屉读取原生实际模式，不以已保存设置推断。选择器遵循原先连接/清理期间锁定，失败保存保留旧值，大字/明暗主题下可滚动选择。README、权限页与设置目录文案同步。

原生Service保留安全接管计划并在状态JSON中报告capture_mode；运行中预览返回实际初始计划，不重新遍历自身已安装路由，不受新的MTU/DNS/模式请求误导。启动/清理中的Service拒绝预览；保留计划不参与运行或清理成功判定，失败仍保留owner重试。前端开始新预览先清除旧结果，失败不能继续展示上一份。新增Rust回归与真实内核探针验证上述行为，独立只读复核未发现问题。

新增`tool/full_capture_adapter_smoke.dart`以Flutter测试引擎加载实际默认库，通过正式RustConnection验证完整捕获及校园分流的预览、启动、状态、校园UDP内核往返、停止和路由恢复。已对新归档实际so重跑通过，CI已接线但未远端执行。Flutter runner启动前须在独立net namespace启用lo；首轮因未启用lo导致测试通信失败，另一次PowerShell管道末尾CR造成路径错误，均保留日志并修正启动命令，没有修改断言。

普通UID、CapEff0的Full测试通过新归档默认特性helper，覆盖双栈TCP/UDP、校园UDP来源转换、DNS UDP/TCP及物理重试、域名规则/歧义拒绝、物理HTTPS、既有socket绑定恢复、停止重启及helper被杀后的新会话。对端审计没有虚拟源泄漏；低层默认helper的创建回滚、拒绝、放弃/丢弃及helper/父进程崩溃清理亦通过。这些普通用户Full场景仍由测试locator选择临时受控helper，核心探针为专用feature例程；不等于已安装固定路径默认so的普通用户Full数据面。后者本批仍未验收。

最终Windows365/Linux348 Rust全量、两端Clippy/analyze、从lib/main.dart的完整release与Windows312/Linux311 Flutter全量通过。新解压库Native FFI为67/66，桥接及SSO/账户/设置三套跨进程存储均通过；Windows归档helper4项真实子进程IPC通过。Python工具25项两端通过，Windows含12项平台skip。首轮Linux工具测试缺少源码快照来源环境而失败，显式声明已知revision/dirty后通过。全量曾发现7处界面断言失败，其中连接抽屉标题/状态重名已改为“模式 · 已启用”，另更新真实变化的目录/权限文案断言，新增原生实际模式优先于设置的回归；未删除或放宽功能断言。Clippy布尔式和新探针格式问题也已修正并保留相应日志。

新成对候选`build/desktop/d26c2827760b/`，430来源文件两端SHA `d26c2827760b196bfc21a256bc7c35e1c0f69c7a3359868454c9c05d692b78f1`。ZIP21,251,592字节/SHA `fbdb3157a4ddf36d2e9a9f39f0c0559e12f35b35329d87a5a4260c6477892651`；Inno17,179,869字节/SHA `6fd6a127f7bd86c1af3b05b4c8888e300f4ac29e41236747fe16bf0749410dc7`；tar20,850,532字节/SHA `94eb901a82f02bb331fd9efbba8587b9686b5b000fd755ce985ffef53bfb6226`；DEB16,448,336字节/SHA `bc39bcfccd1347bf218b19deb0b839210088f6c5ad73f4afe29c19cc8bb8ae97`。两归档、安装器payload/builder来源、输入/输出SHA与旁路文件、完整bundle/解压文件清单均核验；core/helper等于各自Flutter联合构建Cargo目标。DEB两份安装清单、每文件SHA256及完整dpkg md5sums通过。

最终Windows EXE在非管理员shell实际完成安装、合成旧版本升级、卸载、运行/事务拒绝、runner1618、已安装库探针及原生账户/非包文件保留；注册项已清理，安装器NotSigned。最终DEB完成外来/符号链接拒绝、setcap失败及configure重试、显式组授权/非组拒绝、运行中移除拒绝、升级/remove/purge/重装、额外文件与原生账户保留，宿主账户及dpkg状态不变。三轮已安装默认库/固定helper的校园DNS/动态路由/TCP及owner退出屏障通过，此三轮不是Full模式。安装版欢迎页、/opt实际库加载与无托盘退出0通过，截图已亲自检查；OpenGL首帧、implicit-view和messenger关闭警告仍保留，不冒充完整GUI无警告通过。

证据在候选`verification.json`与`build/desktop/capture-ui/verification-current.json`，包含日志、库/helper、来源及产物摘要。没有新持久化schema或FRB签名变更，名称hash仍-724705019，仍须完整配套替换。回退先停止并确认清理，必要时改回校园，再使用第60批成对完整包。下一步优先补上已安装固定路径默认库的普通用户Full链路、Windows当前特权Full与恢复，再完成完整GUI、真实校园、长期、远端CI及M5.4逐项签收。全部本批句柄与四名只读代理结束，未提交发布，候选仍partial-desktop-candidate，完整目标active。

下一批边界定位（只读，尚未实施）：`full_capture_adapter_smoke.dart`须增加显式安装模式，使用DEB harness导出的父/内net与mnt标识，不能以`/proc/1/ns/net`判断私有PID空间。root在`linux_deb_smoke.sh`安装授权后统一准备网络，实际Flutter测试引擎用授权普通用户运行并在进程内证明UID非0、CapEff0、NoNewPrivs0；明确打开/opt安装库并核对自身maps。库与固定helper运行前后SHA应等于本次DEB原始payload，root观测真实helper的exe/netns及清理，不使用测试locator或给GUI/引擎加capabilities。当前adapter probe的校园UDP不能独自证明非校园Full接管；必须同时扩展独立非校园TCP/UDP、DNS及实际路径/源地址/拒绝与恢复证据，不能只新增环境开关和学校UDP就签收。测试缓存目录/SDK应对该普通用户可读写，避免把环境权限故障解释成网络结果。

第六十一批当前状态（2026-10-17，优先于60批）：当前工作树已经包含原生完整捕获实现，60批顶部“四文件调查”不再代表实际代码。主线程重新读取原工程计划和拟改代码，验证现有双栈路由分区、物理出口保护、DNS接管及覆盖变化监控，并修复Linux路由前置校验。没有把完整捕获设置字段当成功能完成；正式Flutter启动/预览/手动TUN命令尚未传递所保存模式，界面仍只提供校园分流，后续必须贯通。

Linux原检查忽略优先于main的local表，又未要求local/main规则实际存在。基线隔离内核探针明确复现foreign-local-unicast与missing-main仍被错误接受，红日志保留。修复逐地址族要求默认local/main规则存在且不重复；local表每条目的前缀必须完全位于保留范围或本机地址集合，不按路由类型跳过。最终六场景foreign-local、foreign-local-unicast、foreign-local-blackhole、missing-main、missing-local、missing-main-v6全部在OS修改前以明确interface原因拒绝。missing-local改在fixture网关连通后删除，否则会先破坏loopback并产生无关超时，未将早期失败记为通过。

最终正常完整捕获、运行中新加local路由、仅IPv4网关和原校园分流均通过真实隔离内核测试。验证范围含校园UDP来源转换、DIRECT双栈TCP/UDP、任意捕获DNS的UDP/TCP与物理上游重试、域名规则/歧义拒绝、覆盖变化停止、外来路由保留、接口/出口清理及新会话重建；对端源地址审计确认物理流量未泄漏虚拟源。该轮为root隔离namespace原生入口，不是普通用户helper、正式GUI或Windows特权完整捕获验收。

同步FRB2.11.1生成绑定修复SettingsSnapshot新增字段导致的编译失败，Dart设置展示适配和测试替身接收原生CaptureMode；新增提交前不发布、非法值拒绝及持久化重开/切回campus证据。原生默认仍为campus，恢复连接默认仍保留原先范围。修复两处新捕获代码的Clippy问题；本地两端verify_desktop入口改为与CI一致的test_package*.py，Linux实际执行14项打包测试含DEB，Bash/PowerShell语法检查通过。

最终Windows364/Linux347 Rust全量、两端Clippy/analyze、默认release库/helper、Windows305/Linux304 Flutter全量（含真实FFI）通过。两端新默认库的桥接及SSO/账户/设置三套跨进程探针通过。429来源文件两端SHA `99b5192d9c40a46d661b32978d98fd8adaecae789b7d7a50848e27e87d59a328`一致，实际库/helper/例程及所有成功、失败日志摘要在`build/desktop/full-capture/verification-current.json`。FRB名称hash仍-724705019，但DTO字段已变，不能混用旧库/绑定。

本批没有重建完整Flutter应用或安装归档；最新配套分发候选仍是60批ccb6beba4cba，不能把新完整捕获能力标给旧包。下一步先将模式贯通正式设置/预览/自动及手动命令并明确排除范围，再验普通用户helper、Windows当前特权路径、完整应用与成对安装产物，继续原计划GUI/校园/长期/远端CI及M5.4签收。回退先停止并确认清理，使用60批成对完整包；默认校园语义保持，设置键不等于旧程序支持新模式。全部本批任务和三名只读代理已结束，未提交发布，完整目标仍active。

第六十批当前状态（2026-10-17，优先于59批）：进程规则与捕获代次修复已进入新的成对完整桌面/安装产物，候选`build/desktop/ccb6beba4cba/`。427文件来源SHA `ccb6beba4cbaff07b1588df5913bf5319b3c064ff1ef0062f11d309f4c82878d`与59批已验完整快照一致，本批没有新增产品代码变更。两端从lib/main.dart重建；归档core/helper等于对应Cargo目标、完整bundle和解压目录。两归档来源、安装器payload/builder来源、输入归档与输出SHA/size和旁路校验文件一致；DEB两份安装清单、每文件SHA256和完整dpkg md5sums通过。FRB名称hash仍-724705019。

Windows ZIP21,636,545字节/SHA `2864368511295ffd950c669871623c600b1d4c08e15a4a966472aef9ca93f130`；Inno EXE17,094,774字节/SHA `3cb3571cf256b87a7dd4db068aa5bbb23b566332b0431789433783f2df2953d2`。Linux tar20,670,522字节/SHA `490bba9c4dcffb7d73702647b8ac19c8c5df961b01f3c33c5128aff5a84f1089`；DEB16,346,656字节/SHA `6bdfa0a5a037eb5ed210baa075f2bcb120e15f59027abb43b6f6463c13f2ff8f`。新归档库Flutter304/303全量、analyze、桥接和SSO/账户/设置三套跨进程探针通过；新Windows helper四项真实子进程IPC通过。357/339 Rust与Clippy沿用59批完整相同源码的证据，不冒充本批新执行。

最终EXE普通权限真实安装/合成旧版本升级/卸载、运行/事务冲突拒绝、runner1618、已安装DLL探针、账户哈希/恢复与非包文件保留通过，临时注册项清理；安装器仍NotSigned。最终DEB完整安装生命周期、外来/符号链接拒绝、失败setcap重试、显式授权与非组拒绝、运行中移除拒绝、升级/remove/purge/重装、额外文件和原生账户保留、宿主账户/dpkg不变通过。

新DEB100轮普通UID/CapEff0测试直接打开/opt安装库并调用固定/usr/lib helper，每轮先验证这两个文件SHA等于新归档清单，再完成真实内核DNS/动态路由、PROCESS-PATH校园签名TCP、PROCESS-NAME拒绝且无成功出站、PID/路径/快照依据，以及接口/HTTP/SOCKS关闭和owner退出屏障。计数同时要求进程规则和全部清理标记及整个任务退出0，不仅凭早期成功标记。AOT包装器用exec透传参数，实际映像为专用AOT而非包装脚本；临时规则目录收敛至任务自有TMPDIR，100轮结束确认无残留。普通shuvpn组授权与默认安装路径这次是实际新DEB验证，已补上59批私有helper harness的范围差异。

新随包默认helper低层内核/回滚/崩溃场景通过；新安装版欢迎页、/opt随包核心实际加载和无托盘关闭退出0通过，截图已检查。已有OpenGL首帧、implicit-view和messenger关闭警告仍保留，不是完整GUI无警告签收。没有本批Windows特权TUN、真实校园、长期、Wayland全场景、远端CI或发布验收。证据`build/desktop/process-packages/`及候选verification.json。本批所有任务与两名只读代理结束，未提交发布，状态partial-desktop-candidate、goal active；使用新进程语法后降级须先恢复兼容规则快照。

后续完整捕获调查已压缩到四文件证据：platform/model.rs的最小前缀/总IPv4空间/保护地址限制及未知full_capture字段、desktop_tun.rs仅从校园资源生成计划、packet/tun.rs再次拒绝捕获真实节点。network/mod.rs已有已登记socket绑定、retired恢复门禁和同owner重试，不能据此推断系统DNS接管/默认路由/崩溃恢复已完成。下一步主线程须亲读原计划及拟改代码，选择贯通完整捕获、控制面/物理DNS绕行和恢复的真实方案；不能把只新增关闭的模式字段当成该目标完成，也不能仅删除路由保护限制。保留既有split默认语义并验证TCP/UDP/IPv6与恢复的实际路径。

第五十九批当前状态（2026-10-17，优先于58批）：开发源码已接入PROCESS-NAME/PATH、实际本机套接字进程快照、平台×入站矩阵、模拟输入及连接记录。Flow新增可选process（pid/name/path/source/confidence）；没有进程规则时不查询系统。未知身份在AND/OR/NOT/RULE-SET中保留未知状态，仍需身份时明确报process错误，不跳过后直连；与未知IP组合可先解析，再由确定的逻辑分支决定是否还需要身份。进程字面量含分隔字符时支持JSON字符串值；Windows忽略大小写并统一路径分隔符，Linux精确区分。源树427文件SHA `ccb6beba4cbaff07b1588df5913bf5319b3c064ff1ef0062f11d309f4c82878d`，两端全量文件摘要相同，26个文件相对58批变化，新增5个Rust模块/测试文件。

Windows以IP Helper表和保留的进程句柄查询，复核创建时间、存活及端点；Linux从当前网络空间的套接字表按inode扫描可见fd、固定proc目录并在返回前复核fd/image。持有者可能移动网络空间，不能仅按其当前ns/net过滤。IPv4查询同时检查IPv6映射表；IPv6通配UDP缺少V6ONLY信息时不臆测IPv4归属。两个工作槽、包含排队的500ms协作预算和调用者250ms余量避免突发立即失败及无限阻塞任务增长。只承诺可见归属快照：权限隐藏的共享持有者无法排除，不是进程认证或历史发送者证明；专用校园DNS走独立传输。UI与README均已说明范围。

最终Windows357/Linux339 Rust、两端Clippy/analyze、默认release库/helper与Flutter304/303全量通过。真实子进程覆盖TCP/UDP双栈、IPv4映射、共享UDP歧义、IPv6-only不能冒充IPv4、无归属和短时排队；纯匹配覆盖逻辑/规则集、缺失身份、JSON字面量及大小写/路径。真实HTTP/SOCKS证明名称规则DIRECT、路径规则REJECT，进程规则SHU-VPN仍不能扩大授权或回落DIRECT；不可归属TUN包不能越过规则。实际FFI及widget验证模拟输入和能力矩阵，连接记录保留真实PID/路径及置信依据，不为旧记录补造身份。

真实Linux内核探针暴露配置切换后首DNS误拒绝：actor等待命令前缓存旧代次，Register收到当前新答案时仍用旧book。linux-capture-version-red.log暂停时钟确定性复现，Register现读实时generation后reset，安装后仍复核，已有flow租约不变；最后只读独立核验无发现。随后内核TCP仍拒绝，新增决策诊断证明实际映像为dartvm而启动器路径为dart；探针改按/proc/self/exe配置实际映像，并保留严格PID/path/confidence和REJECT断言，没有改后端匹配或放宽判据。

Root真实TUN/FFI和普通UID、CapEff0、固定可信路径默认helper的内核路径均已通过：PROCESS-PATH选择校园签名TCP，PROCESS-NAME拒绝新连接、无成功出站，换规则后重做DNS映射，最后接口/监听和owner退出屏障通过，宿主账户/dpkg状态不变。普通用户harness私有覆盖/usr/lib并临时授权现有用户组，仅验证默认库/helper的权限与数据面，不是新DEB/shuvpn组安装验收。证据linux-process-kernel-image-fixed.log、linux-process-helper-kernel.log及`build/desktop/process-rules/verification.json`，库/helper/源码及日志摘要已记录。

原始失败完整保留：linux-review-red.log有映射及字面量2项；windows-v6-only-red.log有错误IPv4归属反例；早期编译参数/变量名、Clippy范围写法、两项初轮Flutter失败均已按实际代码修正，没有跳过或弱化测试。一次完整examples重链实际耗9m39s、进程保持活跃，未因观察超时重启；最终仅重建所需lib/helper/proxy_fixture。全部构建、测试、探针句柄与只读代理均已收取结束。

尚无包含进程规则的新完整应用/安装归档；当前正式候选仍为58批1f1a2715c619，不能把新源码能力标到旧包。新语法应用后，降级需先恢复旧程序支持的规则快照；FRB接口与持久化容器schema没有变更不代表旧程序能理解新规则。没有新增Windows特权TUN、完整GUI/校园/长期、远端CI或发布验收。下一步将最终源码带入配套完整桌面产物，再回到原计划完整捕获及余下平台/最终验收缺口。goal active，未提交发布。

第五十八批当前状态（2026-10-17，优先于57批）：第57批pidfd修复已进入新的Windows/Linux成对完整应用与安装包。候选`build/desktop/1f1a2715c619/`，422来源文件，两端SHA `1f1a2715c6190ea4c862ef09657c485088d75f9c952e601e3a40578ea1400164`。相对第56批完整源码归档仅有第57批已验证的7个代码/工具文件变化；没有另加业务改动。两端从`lib/main.dart`重新构建，归档中的core/helper逐字节等于各自Cargo目标输出和完整bundle。两归档、两安装器的payload/builder来源、输入SHA、主产物SHA/size及旁路校验文件一致；DEB两份安装清单、每文件SHA256及完整dpkg md5sums通过。FRB名称hash仍-724705019。

Windows ZIP21,584,527字节/SHA `bcf85f1e795da4f8edb9c3411ca260e782f9b1d62a46618524c4be2ae5ef6ff8`；Inno EXE17,064,961字节/SHA `a502a6474e97f2074f7f29f95ef5898c3baba27fd39b00f6a47ef4c41bdf6f02`。Linux tar20,619,160字节/SHA `42394e41fd8c0556a67de170711afa0d860f5a54e32585feec6e597557151f16`；DEB16,275,252字节/SHA `069ffebd5488654bfa8a5d83b4fff1f802ab35fb88ed6ed778506d5c49986e75`。两端analyze和归档内库的Flutter全量300/299项通过；桥接及SSO/账户/设置三套跨进程探针通过。Windows新随包helper的4项真实子进程IPC测试通过，不触发UAC；Linux新随包默认特性helper的内核包、路由、绑定恢复、失败回滚及两种进程崩溃场景通过。原生341/323和LinuxClippy回归沿用第57批结果，其全部变更源码与本批快照一致；本批没有把这些结果冒充新运行。

最终EXE已用普通权限执行真实安装、合成旧版本升级、卸载、运行/事务拒绝、runner1618及已安装DLL四套探针；账户fixture在升级/卸载后哈希及恢复保持，注册项已清理。EXE仍NotSigned，未使用项目发布证书。最终DEB在隔离mount/net/PID环境验证外来/符号链接拒绝、setcap配置失败与configure重试、授权/非组拒绝、运行中移除拒绝、升级、额外文件保留/重装/remove/purge及真实原生账户保留；宿主账户与dpkg状态不变。

这次100轮TUN压力直接加载新DEB安装的`/opt/shuvpn/lib/libshuvpn_core.so`及固定路径helper，每轮均完成HTTP/SOCKS/TUN原生启动、内核DNS/动态路由/签名校园TCP和退出屏障，不再是替换库诊断轮。最终已安装欢迎页、包内核心加载及无托盘退出0通过，截图已检查；OpenGL首帧等待、implicit-view和messenger关闭警告仍保留，不宣称完整GUI无警告通过。GUI未执行登录或连接操作，没有新增真实校园、Windows特权TUN或长期运行验收。

证据`build/desktop/pidfd-packages/`及候选`verification.json`；本批构建、探针、安装与两名只读代理全部结束。没有覆盖旧候选或真实用户安装，没有提交/发布，无数据schema变化，回退使用配套完整包。下一步回到原计划的进程身份规则/平台入站能力、完整捕获、剩余平台恢复与完整GUI/校园/长期/远端CI等缺口，避免反复重打相同候选。状态仍partial-desktop-candidate，完整goal active。

第五十七批当前状态（2026-10-17，优先于56批）：已重新捕获并修复 Linux helper 的退出确认竞态。旧候选 c865f4db8667 在30轮真实FFI压力中第30轮于session-shutdown失败；探针新增phase和原生snapshot后，具体原因是“权限程序工作线程异常，退出尚未确认”。独立基线源码仅增加wait/kill日志后，真实FFI第1轮又在就绪检测中捕获wait ECHILD（10 / No child processes）及kill ESRCH（3 / No such process）。这是退出状态被其他线程回收时的错误确认路径；不能把它解释为已定位的内核网卡/路由泄漏。证据linux-deb-stress-baseline.log、linux-deb-stress-snapshot.log和helper-exit-baseline-stress.log。

修复使用子进程pre_exec打开自身pidfd并在exec前通过私有socket传回，父进程在首次await之前同步接管；保留既有actor与协议2的Hello序号0。退出先等pidfd可读，再允许wait返回ECHILD；kill/析构按pidfd发送，避免数字PID复用。pidfd的Identity守卫覆盖异步注册失败，启动帧重试EINTR，正常等待与强杀后的等待各限3秒。需要Linux5.3+，代理功能不依赖该新接口。session finish_stop保留底层原因；诊断探针保留失败阶段、会话状态和主/清理错误。未改变FRB或持久化schema。

最终Windows341/Linux323 Rust通过，Linux Clippy全目标全特性、Flutter analyze、299项Flutter全量通过。退出原因保留有红/绿回归；三个pidfd测试证明其他线程先回收后的确认、活进程不误判/取消等待后强杀、丢弃owner后的同进程终止。最终默认特性release库与helper构建通过，正式helper的权限拒绝、双栈包/动态路由/绑定恢复、创建回滚、放弃接管、lease析构、helper及父进程崩溃场景通过。

最终修复库搭配既有DEB实际安装的helper，在隔离mount/net/PID环境完成100轮普通用户真实FFI；每轮均验证原生owner启动HTTP/SOCKS/TUN、内核DNS/动态路由/签名校园TCP、停止后的接口/监听消失及退出屏障。随后升级/remove/purge/重装与宿主账户及dpkg状态恢复通过。此压力轮替换的是运行库，不是重建后的DEB验收；未重新做账户跨进程保留或GUI。证据helper-exit-fixed-stress.log和helper-pidfd-kernel-smoke.log；7个代码/工具文件两端SHA一致，最终库/helper与日志SHA在build/desktop/installers/helper-pidfd-verification.json。

保留失败记录：首次宿主网络命名空间下的Rust全量有3项本地HTTP超时，改用现有rust_linux_namespace_tests.sh后全量通过，没有改变生产实现或断言；一次诊断库放在用户私有home，导致nobody加载拒绝，后改为root临时目录内0755库并成功。所有最终构建/测试/压力句柄已收取、只读代理均已完成。既有c865f4db8667候选包未被替换、未提交发布；回退用第56批配套完整包，数据格式不变。下一步将修复带入新的成对桌面分发包并重新核验安装产物，随后继续进程规则、完整捕获、平台/GUI/校园/长期与远端CI等完整工程缺口。完整目标仍active。

第五十六批当前状态（2026-10-17，优先于55批）：正式安装格式已实现并完成本批局部验收。`tool/package_installers.py` 接收已通过完整性校验的 ZIP/tar.gz，生成 Windows Inno Setup EXE 或 Linux DEB，并记录输入归档、负载来源、构建来源、工具版本/摘要。`tool/install_inno_setup.ps1` 固定 Inno 7.1.0 x64 的安装器及ISCC摘要、验证Pyrsys签名，以PORTABLE+CURRENTUSER放入build目录；没有改系统工具链。初次只传PORTABLE的提取退出2，补CURRENTUSER后成功，日志在installers/toolchain。

Windows `windows/installer/shuvpn.iss` 按当前用户安装，保留账户与非包文件，不自动关闭正在运行的程序。runner在Flutter启动前检查Installing标记，创建Running标记后再检查一次；阻止启动返回1618。安装/卸载共享原生原子Transaction mutex，且两者均检查Running；烟测另持全轮验证标记，正常安装器不得闯入烟测，安装前再次检查验证预约。烟测检查注册项归属及所有路径祖先重解析点，仅清理自己安装。最终EXE实际安装、合成旧控制版本升级、卸载、运行/事务拒绝、实际runner 1618及四套已安装DLL探针通过，原生账户fixture在升级/卸载后哈希及恢复保持；最终日志windows-installer-current.log，报告windows-smoke-current。验证shell非管理员，安装后临时卸载注册项已清理。未做新的正常Windows完整GUI或特权TUN验收。

Linux `linux/packaging/lifecycle.sh` 提供preinst/postinst/prerm/postrm；GUI保持普通权限，独立helper配置root:shuvpn 0750和net_admin/net_raw，仅显式加入组的用户可用。拒绝外来目录、符号链接和运行中移除；固定PATH，缺失菜单目录允许由dpkg创建。卸载/purge保留非包文件，并为残留目录保留归属标记以支持重装；便携install.py拒绝覆盖dpkg管理的目录。更新资产按当前exe旁标记选择Inno/DEB，便携版保留ZIP/tar；未知/损坏标记或缺少对应安装资产转发布页，不用便携包覆盖包管理安装。增加md5sums供dpkg核验，原SHA256清单仍保留。

`tool/linux_deb_smoke.sh` 使用mount/net/PID命名空间，隔离/etc、/usr/lib、/usr/sbin、/usr/share、/var/lib/dpkg与/opt；它只接受可信本地构建的shuvpn/amd64包，不是恶意包沙箱。测试包括外来/符号链接拒绝、注入setcap配置失败及configure重试、显式授权/非组拒绝、旧控制版本升级、运行中移除拒绝、额外文件保留/重装/remove/purge、真实native账户保留、可选已安装TUN与GUI。失败时也核对宿主账户文件和dpkg状态。首次DEB暂存放在/mnt/d导致权限777被dpkg-deb拒绝，已将POSIX暂存移至本机临时目录；没有放宽权限检查。

新增归档提取暴露旧路径校验缺口，红/绿回归后拒绝Windows驱动器/ADS/保留设备名/控制字符/歧义路径及带路径的版本；提取后再核对文件集合和SHA256，防止平台名称别名改变负载。Python工具Linux25项通过，Windows25项含12个Linux相关skip；Windows340/Linux319 Rust、两端Clippy/analyze/release与300/299 Flutter通过，最终解压库FFI65/64通过。CI已接安装格式和安装烟测，未查询远端结果。README清理了桌面多协议、旧Dart后台和TUN未接线等过时描述；完整目标仍未完成。

最终候选 `build/desktop/c865f4db8667/`，421来源文件两端SHA `c865f4db8667553ff3869cbec49357d53179e6caf53842adc709f6115427d3fb`。同时提供Windows ZIP、Inno EXE、Linux tar.gz、DEB；源manifest与安装器输入/构建来源均一致，除source manifest外，便携运行负载与已测中间0f3c0b4a2319逐文件字节一致。最终EXE/DEB已重新执行安装烟测，不仅继承中间结果。Windows安装器17,062,158字节/SHA `6ee8b131a9b86a2f6253bebbd285a9f738d5a11872544883e6e1ab09a64ee9f0`；DEB16,260,764字节/SHA `dcbc3d885f1cc3573798f3a3c8e6a84038ea7f9c95375fa50f5b4f93935ff796`。其余SHA和证据范围见verification.json。FRB未改接口，内容hash仍-724705019。Windows安装器检查为NotSigned，未使用项目发布证书，不能声称已签名发布。

最终DEB通过缺失菜单目录首次安装、外来/符号链接拒绝、setcap配置失败与configure重试、授权/非组拒绝、运行中移除拒绝、升级、保留额外文件后的purge与重装、最终remove/purge，以及原生账户哈希和恢复。3轮已安装helper完成HTTP/SOCKS、TUN/内核DNS/动态路由/签名TCP及owner退出屏障；宿主/etc账户文件与dpkg状态保持。最终已安装欢迎页、/opt随包核心加载与无托盘退出0通过，截图linux-deb-gui-current.png已检查；日志保留了既有OpenGL首帧等待、implicit-view和messenger关闭警告，未消音或称为完整GUI无警告通过。

必须保留的未决观察：真实DEB首轮在TUN probe cleanup报VpnFailure并遮住主错误；已改tool/tun_dns_bridge_smoke.dart打印并保留主错误和清理错误。生产网络代码未因该失败修改；随后一次诊断轮、两次各3轮测试（共7轮）通过，初次原因未复现。不能宣称已定位或修复原始数据面失败。日志linux-deb-smoke.log保留初次，linux-deb-smoke-diagnostics.log、linux-deb-smoke-r3-gui.log和最终linux-deb-current.log保留后续结果；下一步优先用诊断版做FFI/helper退出压力复核，捕获主错误后判断，再推进其他全量门槛。

本批全部构建/安装/GUI/探针任务及四名只读代理已结束。证据 `build/desktop/installers/` 和候选verification.json，状态partial-desktop-candidate，无持久化schema变更，回退使用配套完整包。M5.2本地安装格式已经补上，但远端CI/发布未验证；独立剩余目标包括上述清理观察、进程身份规则、完整平台/GUI/校园/长期，goal active、未提交发布。

第五十五批当前状态（2026-10-17，优先于下文54批）：密码登录与验证码通过后的凭据获取已合并为原生 `sign_in_password` / `sign_in_code`。一个账户operation锁贯穿认证与exchange_all；密码结果仅Completed或VerificationRequired，后者真正等待用户选方式/发码/提交验证码。全部系统凭据失败返回credentialsUnavailable，保留已接管SSO，不冒充登录完成。身份认证公共步骤抽为sign_in.rs内部helper，原细粒度原生API及其测试仍保留，不产生第二次网络认证。

正式Dart AccountCenter删除completeSignIn，表单删除_acquireCredentials和独立ShuCredentialGate页面；native进度直接显示在表单。成功结果才通知宿主完成；已消费的challenge不会在凭据失败后重新显示。密码在发起请求时清空输入框。表单dispose/reset取消仍在等待的账户命令，用展示代次拒绝迟到结果；旧finally不能清掉新命令引用，成功回调同步reset也不会发取消。RustAccountCenter增加await返回后的disposed检查，红/绿回归证明迟到成功不再通过已关闭适配器返回。

旧credential_gate_test的超时由native决定、迟到结果、失败不取消已完成任务、成功展示不持有取消权等断言已迁入7项login_lifecycle_test，并新增密码/发码/验证码取消与reset交错。3项新的Rust测试覆盖两种认证、真实TLS请求/挑战/单次凭据获取、全部凭据失败、四个阶段取消和互斥；账户79项、UI/adapter/宿主53项目标测试通过。最终Windows339/Linux318 Rust、300/299 Flutter、两端Clippy/analyze/release、解压FFI65/64、桥接和三套跨进程存储探针全部通过。真实FFI覆盖新入口非法输入和关闭边界；密码关闭返回closed，验证码关闭沿用客户端取消优先返回cancelled，测试精确核对而非接受任意失败。

失败记录保留：初轮BuildContext lint改为显式mounted检查；三个新adapter测试等待未订阅的测试用QR控制器关闭而超时，补齐fixture订阅后全部通过，没有改生产断言；真实FFI关闭验证码的错误码预期按已核对的Client::verify_code修正；临时Linux验证包装脚本从Windows复制为CRLF，执行任何打包前已改为LF。无本批残留构建或代理，六名只读代理均完成关闭。

最终成对候选 `build/desktop/ffc946346612/`，413来源文件两端SHA `ffc9463466127bfadd6bc343c9cf972389d2913ccc88259957139688abe2a6a6`，FRB2.11.1内容hash -724705019。Windows ZIP21,087,610字节/SHA `b8fcb977e4a949b9474c329a27ef746442b28e0c91050dba72d0015d01ec0d29`；Linux tar20,585,104字节/SHA `49cfb840acab27760cda4b0b6f9a02420f2e0c4fde95b702e2c21093b78f5b7f`。归档完整性与来源清单通过。两端解压库公开二维码图片立即取消探针通过（Windows1348/Linux1358字节），并确认未登录/无活动二维码；它不是实际密码/MFA/手机扫码成功证明。本批无新平台特权、安装或完整GUI验收，无持久化schema变化，回退使用配套完整包。证据native-password和候选verification.json；未提交发布，状态partial-desktop-candidate、goal active。

边界复核决定：旧SharedPreferences仅作一次性导入输入，native提交成功后通过Flutter插件删除旧键，原B1审计已明确允许该桥接；它不是两侧独立长期存储。页面挂载/滚动发起native命令、展示批次订阅取消、bootstrap依赖装配及单条native恢复调用不能仅因async/await而误报为Dart后台实现。账户过期判定仍在Rust verify_if_stale。此次限定扫描与抽查未定位新的独立Dart网络/认证/存储实现；M5.4逐文件、依赖与运行期最终签收仍未完成，不能用搜索无命中代替验收。

下一步回到完整工程计划的明确缺口，避免继续无依据拆分薄UI接线。主线程已重新完整阅读计划，并抽查只读证据：M2.6的GEOIP/GEOSITE、逻辑条件和受限正则已有实际编译/匹配及测试，PROCESS-NAME/PROCESS-PATH仍在unsupported、processIdentity=false，Flow也没有进程身份；需要平台×入站能力和真实身份获取。M5.2当前只有ZIP/tar.gz及Linux自定义install.py，不是Inno/MSIX或AppImage/deb/Flatpak；CI仅配置归档上传，未核验远端成功或发布。随后仍有当前Windows特权/完整GUI、M3.4完整捕获与M5.3一致性/恢复、校园与长期验收。构建脚本可重复执行与来源追踪不等于归档字节一致；CI还有浮动工具链和归档时间戳。原目标范围未缩减。

第五十四批当前状态（2026-10-17，优先于下文53批）：新增原生 `Account::sign_in_qr` 与同名桥接事件流，一次持有账户操作锁执行创建图片、180秒扫码等待、单次码兑换及既有90秒凭据交换。发出Waiting/Redeeming/Credentials与最终Completed/Unavailable阶段，失败由Failed事件携带AuthFailure；Dart只映射事件/250ms展示快照。扫码页面不再调用start/wait/redeem，登录表单不再在扫码路由成功后再次completeSignIn；凭据进度在扫码页展示。旧细粒度原生API保留供低层用途，正式Dart接口仅有统一操作与取消。

`qr_flow`与`qr_active`区分完整操作和未接管阶段。取消时在账户锁内检查原生已提交会话；兑换提交已完成即使展示标记未更新，也保留SSO并停止后续凭据工作；提交前取消清除二维码/未完成会话。run_notifying在busy/progress/error收尾后、账户状态及操作锁释放前发出最终事件，避免事件被排到后续sign_out之后；事件表示该操作已结束，不保证随后不会退出登录。通知回调禁止重入账户；正式回调只写FRB sink。各阶段事件接收失败及时取消，最后完成事件发送失败不回滚已成功账户。

四名只读调查/复核代理均已完成关闭；主线程按行抽查了复核问题并实现上述提交边界/事件顺序修正。新增5项Rust测试（其中一项覆盖创建/等待/兑换/凭据四个取消阶段），真实本地TLS证明单次兑换、OTP实际请求一次、部分成功/全部失败与SSO保留。首轮误把OTP直接读取成功期望成OAuth callback，被已有实现证据纠正为实际请求计数；失败日志保留。账户76项、界面/adapter21项目标测试、Windows336/Linux315 Rust全量及两端Clippy/analyze已通过。页面收尾另经红/绿回归：仅在等待本次操作时取消；完成后退场不再发送全局cancelQr，避免干扰后续新扫码。两端最终release及Windows293/Linux292 Flutter全量通过，最后页面修复已包含在实际应用内。

真实FFI随后发现生成器的流错误边界：StreamSink方法返回Err时，FRB2.11.1的Dart调用是unawaited executeNormal，事件流先结束、认证错误成为未处理异常。未放宽回归断言；QR改为显式Failed事件携带AuthFailure，Dart adapter映射为原认证异常。公告预取重复订阅同样可触发，改用sink.add_error发送AnyhowException到流错误通道（保留错误码与消息），新增真实FFI重复空批次回归；diagnostics仅sink关闭时失败，改为正常停止发送。其余生产流入口已专项核验，没有同类Result错误返回。修正后新Linux Debug库11项账户/公告真实FFI通过，相关生成器原文、失败日志与最后绑定均保留；未修改FRB依赖。

当前413来源文件两端SHA `64e4acc0c701e09bc65ea57221285efac57fbb6efc220c3e060551592af15533`，FRB2.11.1已重新生成，内容hash仍101081745；名称hash不涵盖DTO字段，必须配套新库。最终成对候选在 `build/desktop/64e4acc0c701/`，Windows ZIP21,028,537字节/SHA `5eb1bf2022b2ba85d76afd31fd9a0fc4d4dc139fb47ab8b843520dd5d55b0251`，Linux tar20,574,979字节/SHA `163b4a98e907f7c0575e5fce27b3c8654e1a5bcad3b3bc0da269db8d1d38a7c4`。两端解压库FFI64/63、桥接与SSO/账户/设置三套跨进程存储探针通过，归档完整性和来源manifest再次校验通过。

新可选`tool/qr_live_smoke.dart`使用临时空账户，只收第一张二维码即取消，不显示或记录二维码及链接；Windows/Linux最终解压库实际分别收到1358/1329字节图片后取消，确认未登录、busy=false、qrStatus=null，标记QR_LIVE_NATIVE_IMAGE_AND_CANCEL_PASSED。它证明真实二维码/失败事件跨桥接及取消收尾，不代表手机扫码成功、完整GUI或校园凭据验收。本批未重跑平台特权/TUN/系统代理/安装矩阵。

证据 `build/desktop/native-qr/` 与候选verification.json，状态仍为partial-desktop-candidate。Windows旧管线82319确认仍在运行，但事件ABI已过时，为防止接着用新绑定测试旧DLL而显式停止对应进程树66340；不是因轮询超时重启。旧Linux管线6828以真实FFI失败终态收取。全部最终构建/测试/打包/探针句柄均已收取，无持久化schema变化，回退必须使用匹配的完整包。完整目标保持active，未提交/发布。

后续边界已由只读调查定位并由主线程抽查：`login_form.dart`密码无challenge成功分支以及验证码ok分支仍自动调用_acquireCredentials→ShuCredentialGate→completeSignIn。方式选择、发送验证码和提交验证码是真实用户动作边界；逐系统交换、90秒预算和限发状态已在Rust。表单dispose/reset/back是否取消账户命令，还需结合宿主检查，不能只凭表单无cancelPending断言全应用漏取消。下一步收敛这两个自动阶段接缝，再完成全仓零Dart与原计划平台/完整GUI/校园/长期/高级能力/正式分发/远端CI验收。

第五十三批当前状态（2026-10-17，优先于下文52批）：公告预取已迁入 `rust/src/announcements/prefetch.rs`，持有客户端、子取消令牌和单次运行状态。每客户端所有批次共享3个预取槽；每批最多2048条/4MiB元数据，原生过滤站外目标、去重、读缓存、调度请求并逐项发出成功或错误。独立前台详情不占预取槽；按规范化URL的弱引用锁合并重叠请求，缓存仍为64项。前台先请求时预取可共用一次请求；预取先请求且取消时，前台重新发起请求是合法接管，测试明确期望两次，不宣称此场景始终只请求一次。详情总10秒预算涵盖URL锁等待、下载和解析。空列表和直接自引用的下一页在原生终止，不宣称检测任意分页环。

新增 opaque AnnouncementPrefetch 与 Preview 事件；父客户端关闭/释放取消子批次，批次取消不关闭父客户端。原生 JoinSet 在run退出前排空；取消是异步完成，不声称调用cancel瞬间停止所有包或立即中止spawn_blocking解析CPU。Dart adapter只映射事件和释放桥接引用；页面删除3 worker、cursor及请求筛选，保留展示缓存、刷新代次和旧结果过滤。刷新先取消已有批次；创建中迟到的批次也会释放。分页迟到结果不得污染刷新后的列表。

两端21项公告测试已通过，包括跨批次共享并发、仅取消一批后另一批完成、前台/预取两种占锁顺序、站外/预算/父关闭、缓存及分页终止。初版deadline测试在advance后立即检查is_finished，修复后仍因调度时机失败；已改为有界等待，生产10秒预算不变，相关失败日志保留。最终源码全量Windows331/Linux310 Rust、两端Clippy与release通过；本批282/281 Flutter全量和analyze在最后Rust-only deadline修复前完成，最终解压库另跑Native FFI62/61通过，不把中间构建冒充最后产物。Windows首轮无限并行构建发生os1455/内存不足，保留日志，使用CARGO_BUILD_JOBS=2重试通过；未删缓存或修改系统分页文件。

两端最终解压库均通过桥接、SSO/账户/设置三套跨进程探针，并实际访问公开公告站：每端10条列表、3篇详情、3个非空预览及原生缓存命中，标记ANNOUNCEMENTS_LIVE_NATIVE_LIST_PREFETCH_AND_CACHE_PASSED；不是校园账户/TUN验收。本批没有重跑平台特权或完整GUI矩阵。最终候选 `build/desktop/e8e936136bb2/`，两端410来源文件SHA `e8e936136bb2bf438708bcd03cb703ebfbe10838e46f756a4ea0fce3002e52b5`，FRB2.11.1内容hash -630543985。Windows ZIP20,996,427字节/SHA `3269ae275c62ccfd774a1f4617c601c487bd8e82d88ffb5778d46a26739be0b3`；Linux tar20,530,097字节/SHA `9bba4ab32be6fccf9a09b8887ae2f7cbf47593233e2780f687a5b4a0be995216`。完整性与manifest来源再次校验通过，清单verification.json仍为partial-desktop-candidate。无持久化schema变更；回退须使用配套完整包。证据 `build/desktop/native-previews/`，未提交/发布，goal active。

下一边界调查已完成并抽查：扫码页面start→wait→redeem及登录表单路由成功后completeSignIn仍由Dart组合；Rust持有网络、轮询、代次取消、单次授权码、Cookie提交和各系统凭据交换。已有widget分别覆盖扫码页与凭据页，缺整条组合、start/redeem迟到与异常、正式adapter回调联测。主线程尚未修改扫码代码，下一步须完整阅读拟修改实现，不能把只读调查当作设计签收。本批三名只读代理已完成关闭，最终构建/测试/打包/探针进程均已收取。完整GUI、Windows当前安装/特权矩阵、校园与长期、原计划高级能力/正式分发/远端CI、全仓零Dart最终验收仍未完成。

2026-10-16 永久范围修订：**项目仅支持 Windows 与 Linux，不再开发或移植 Android。**
Android 工程、专用运行入口、原生适配与构建/发布工具已移除；下文 Android 内容只保留为历史记录，不属于待办或后续验收。
本次删除改变原生桥接接口，旧候选归档不代表当前源码，必须重新构建配套库、绑定与完整桌面包。

第五十二批当前状态（2026-10-17，优先于下文51批）：新增Rust connection/owner actor与DesktopConnectionOwner。进程registry在清理成功前持有原生会话；Engine用Arc共享给展示句柄，AccountSession也用Arc交给原生建立任务，不复制账户状态。actor在启动任何服务前登记Engine，替换前执行finish_stop；失败保留current及失败代次，拒绝已排队的替换操作继续拨号。close flight合并并发关闭，取消、丢弃等待和退出均不丢失任务；shutdown完成全部清理后join worker。worker异常则唤醒等待者、保留登记并拒绝替换，不声称已清理；这种无法确认的fatal状态需要重启进程，不能误报为普通可恢复关闭。

正式RustConnectionBackend替代逐次RustConnectionAttempt。bootstrap在打开账户/规则存储前，通过同步claimFrontend与awaitFrontend接管上一前端并等待其退出；控制代次在原生锁内校验，旧前端dispose/cancel不能取消新前端。registry中的closed owner即使原shutdown等待被丢弃，也会在下一次open中join并替换。修复独立复核发现的Waiting Drop检查后再close的TOCTOU，检查与关闭已合并在同一锁内。原生连接建立、完整服务启动完成后才返回展示句柄与initialServices。

ConnectionController删除_nativeConnectJobs、_pendingNativeCleanup、_teardown、_closeUnadopted以及退出排空循环；RustConnectionBackend删除_discard等Dart资源释放策略。视图只disposeView释放桥接引用；disconnect/shutdown和迟到资源收尾由原生负责。Dart保留界面请求排序、展示缓存及迟到显示结果过滤。手动重绑已迁入native rebind_proxy，与单项代理/系统代理操作共享desktop_operations；停止的监听不被重绑启动，HTTP重绑后按配置重新启用系统代理到实际新端口，独立SOCKS/TUN不受影响。已删除无调用者的Dart stopProxy组合方法。

新增8项core owner测试，使用真实TLS/监听及受控清理故障，覆盖视图丢弃、旧资源清理失败阻止新拨号、故障恢复后不残留旧诊断、迟到建立、丢弃等待、退出介入替换清理、旧控制代次、worker异常唤醒/拒绝替换。API另测丢弃shutdown后重新open及旧shutdown不能删除新registry；新增原生重绑IPv4→IPv6、端口占用回退、独立SOCKS与停止后不重启测试。原账户TLS测试改为正式owner入口并保留首请求规则v1/REJECT证明。真实FFI新增原生会话在视图释放后仍可用、前端接管后旧句柄stale拒绝、关闭监听及正式Dart facade账户/规则/非法输入边界。

UI原有契约保留为backend响应模拟，不作为原生生命周期证明。一次“迟到关闭失败”注入调整为关闭仍在途时注入；原生disconnect现在是清理屏障，不能在其成功之后再模拟新资源出现。原先一次性Dart attempt的预取消测试改为持久backend disposed-before-init边界，原生一次性attempt预取消测试仍保留。64项界面/控制器/记录回归通过。全量Windows323/Linux302 Rust、两端Clippy/analyze/release、Windows274/Linux273 Flutter通过；解压库Native FFI61/60、桥接、SSO/账户/设置三套跨进程探针及Windows随包helper普通权限4项通过。

Windows首次全量有一项认证负例fixture返回network而非预期错误；fixture此前写完TLS响应直接drop。本批补shutdown以完成TLS记录并输出具体scenario/期望code，严格错误码/禁止发布凭据断言不变；定点及后续全量通过，失败日志保留，生产认证逻辑未改。初轮Dart前缀遮蔽/格式lint和fixture表现差异日志也保留。

Linux本批新解压包再次通过隔离mount/net安装、授权/拒绝、升级、卸载、无残留helper及宿主账户SHA保持。installed-helper探针已改走持久owner connectPlan与shutdown：普通UID/CapEff=0完成HTTP/SOCKS、TUN、内核DNS/动态域名捕获/签名TCP、监听与网卡清理，标记LINUX_PERSISTENT_OWNER_TUN_AND_EXIT_BARRIER_PASSED。GNOME/dconf和KDE5私有配置中，owner建立与退出、HTTP重绑后的实际系统代理端口、HTTP停止恢复及独立SOCKS通过；既有外部修改/SIGKILL恢复与KIO消费者矩阵通过。Windows本批没有真实WinINet/UAC/TUN新验收。

Linux已安装应用欢迎页、/opt随包核心加载和无托盘正常退出0通过，截图已检查。针对关闭警告增加同一SDK、同一libflutter_linux_gtk.so字节的最小对照：无插件正常关闭没有相关警告；window_manager 0.5.2的单次destroy出现implicit-view警告；再注册system_tray 2.0.3无新增messenger警告；加入与本项目一致的url_launcher 6.3.3/url_launcher_linux 3.2.3后复现两条messenger警告。最小程序没有ShuVPN/Rust业务代码，runner与本项目仅名称差别；本地源码对应fl_view_dispose→RemoveView，以及url_launcher dispose清理两条handler。未采运行时backtrace，不把每条栈归属当作已捕获；没有过滤日志或修改SDK/依赖源码。证据minimal-*-gui及fixture源文件均保留，完整GUI数据面仍未验收。

最新成对候选 `build/desktop/ed998163d149/`，408来源文件两端SHA `ed998163d149b49987573a772716f7a3a10a5fe46fb34fd8b18b6d05d79f5b4f`，FRB2.11.1重新生成、内容-1969221116。Windows ZIP20,950,378字节/SHA `e63a74dd7fbfb8fc77196833453b730a1761f4664e2f7d6369eade919ade9e45`；Linux tar20,459,026字节/SHA `44105e16a15b7b6018a78e940eb314c6c67d97ca8ccdd74fd9c2c92b61195dd9`。本批验证直接使用此归档，不继承不同payload的验收。无持久化schema变更；回退仍需配套完整包。证据 `build/desktop/native-owner/` 与候选verification.json，状态partial-desktop-candidate，未提交/发布，goal active。

后续边界复核：core（排除生成绑定）未发现独立Dart网络/认证/配置存储及业务重试循环；旧SharedPreferences只作为一次性输入，原生快照计时、桥接健康检查和托盘主机探测属于展示/平台适配。页面审计定位到notifications_page._prefetch中的3 worker、游标/站点/缓存过滤和分页终止，应下一步亲自审查并收敛请求调度；扫码start/wait/redeem/completeSignIn属于阶段组合，需按交互与业务边界判定，不能凭方法名宣称残留。网络设置页保存后派发native命令，正常入口受tunnelUp/cleanupPending锁定，不能把不可达的运行分支误报为后台实现。整个零Dart审计尚未最终签收。

下一关键路径不再重复实现连接owner：完成上述页面调度/阶段边界与全仓验收，然后推进Windows当前安装/系统代理/特权矩阵、完整GUI、真实校园与长期恢复、原计划高级规则/平台能力和正式安装分发格式、远端CI、历史文档口径清理。已证实的依赖关闭警告与项目网络资源清理分开记录。五名本批只读代理均完成，构建/安装/GUI/对照进程已收取；不得据普通用户fixture或无校园凭据的欢迎页宣布完整交付。

第五十一批当前状态（2026-10-16，优先于下文50批）：新增原生connect_desktop_account与desktop_services。桌面attempt的系统代理恢复、规则挂接移出Dart；正式入口必须提供RoutingManager，Engine创建Shared时就持有该规则store，不再先发布默认规则会话后补挂。恢复事务等待完成，不因取消而丢弃阻塞OS恢复任务；恢复耗时仍不计入既有认证/握手deadline。NativeAccount准备、origin检查、同一deadline、警告与账户代次取消保留。

自动连接后的HTTP→系统代理→SOCKS→可选TUN顺序、常规部分失败继续、实际服务状态查询统一进入原生start_desktop_services。driver新增只读ProxyStatus命令返回实际监听/系统代理状态。TUN权限检查、启动失败后的stop与状态读回、单独stop结果均在Rust desktop_tun服务操作中；共享desktop_operations将这些启动/停止服务命令串行化，已有底层TUN startup/service/egress owner与finish_stop保留重试资源。Dart只发送偏好快照/每次启动的网卡DNS输入并映射报告；不再逐项自动启动或在TUN失败后补做stop。低层探针API仍保留，其职责不同于正式服务操作。

独立复核及红回归修复：自动服务完成前不得发出connected且busy=false的中间通知；迟到启动结果不得以cancel事件抹掉原始失败；TUN单独清理重试成功后清除本项错误，但不能清除其他会话清理错误。另从实际消费界面发现部分服务失败仅有message却保持connected会隐藏错误，新增回归先失败后修复为可见错误状态，同时保持可用隧道/SOCKS，重试成功恢复connected。原有TUN迟到/回收与系统代理取消断言保留；fixture区分内部恢复与公开方法调用，以状态及恢复次数证明新原生契约，不把fake当网络证据。

Rust新增4项真实TLS/监听集成测试，覆盖端口回退、缺系统代理目录保留HTTP/SOCKS、HTTP失败阻止系统修改但继续SOCKS、重试、禁用/失败TUN保留代理、并发重复启动及关闭后拒绝。原账户TLS连接测试现直接走新桌面入口，并以首个代理请求的REJECT/version1记录证明规则已预装。新增真实FFI preflight用损坏的私有恢复记录证明预算/网关/预取消在恢复前拒绝、有效尝试先报告恢复错误而非继续认证，记录原文保留；恢复记录损坏在选择平台backend前拒绝，不修改实际系统代理。

Windows313/Linux292 Rust全特性全量、两端Clippy/analyze/release、Windows272/Linux271 Flutter全量通过。解压实际库Native FFI59/58、桥接和三套跨进程存储、Windows随包helper4项普通权限测试通过。52项目标控制器/权限测试先通过，随后增加部分失败显示回归，由最终全量覆盖；不要把目标数误报为完整交付覆盖。

本批实际补验收Linux：用新默认库和普通UID/CapEff=0，在私有mount/net namespace安装授权helper；新版tun_dns_bridge_smoke的installed-helper分支直接调用统一服务启动，HTTP/SOCKS实际监听、TUN、内核DNS、动态域名捕获、签名TCP往返及finishStop后的网卡/监听清理全部通过。安装/升级/卸载、nobody拒绝、可写helper拒绝、no-new-privs拒绝、无capabilities拒绝、卸载后不可用、无残留helper与宿主账户文件SHA保持通过。随后实际已安装/opt/shuvpn应用欢迎页、随包so加载与无托盘退出0通过，截图已检查；它不是完整GUI数据面验收。Xvfb日志含OpenGL首帧等待及关闭时FlutterEngineRemoveView/FlBinaryMessenger警告，未将此次运行称为无警告完整GUI通过。

GNOME/dconf与KDE 5均在私有配置和D-Bus中通过现有stop/http-stop/cancel、外部变更及SIGKILL恢复矩阵；KDE同时通过KIO真实消费者。system_proxy_bridge_smoke已改为统一原生服务启动：OS代理匹配实际HTTP端口、校园HTTP字节往返、HTTP停止恢复OS、独立SOCKS仍响应、最后会话关闭。新标记NATIVE_SERVICE_STARTUP_SYSTEM_PROXY_HTTP_SOCKS_RESTORE_PASSED两端桌面环境均出现。本批没有重跑Windows真实WinINet或特权TUN，也没有KDE 6新验收。

最终候选 `build/desktop/0c5456adbf6e/`，401来源文件两端SHA `0c5456adbf6e81618fa9997c7b92d15b1f68a475f9b2819930fc8d4125dcd714`，FRB2.11.1重新生成、内容948380653。Windows ZIP20,743,549字节/SHA `25271f2465c6e3ea249e5db74f4c21e58272f775364351a1e38f30a306684a26`；Linux tar20,267,985字节/SHA `4d6e82ab32b7e2a466abaeb0f30409b5cc3f6801e784ab021d94c1b28b6509c1`。受测中间归档c0d2ea892ee1对应最终生产代码；随后仅升级system_proxy_bridge_smoke验证探针，未改生产代码或产物。最终两包除package.json来源清单外，Windows36/Linux29项payload与受测中间归档逐项字节一致；完整性与最终source均再次验证。因此安装/GUI/FFI证据明确基于c0d2实际库和同字节最终payload，不冒充执行了新文件名的安装。

原始证据 `build/desktop/native-startup/`，最终清单 `build/desktop/0c5456adbf6e/verification.json` 仍为partial-desktop-candidate。编译初轮发现FRB2.11.1不能生成Option<&RoutingManager>参数，改为正式入口强制规则owner后重新生成并编译；原始arity、lint、状态红回归日志保留。没有持久化schema变化；回退必须使用配套应用/原生库/绑定完整包。四名只读代理已完成关闭，所有构建/探针/安装/GUI句柄已收取。未提交/发布，goal active。

下一关键路径仍不是重新做这些已迁操作：RustConnectionAttempt的迟到会话_discard、ConnectionController的会话采纳/替换、_pendingNativeCleanup/_teardown/_finishExit、手动重绑及迟到手动启用的收尾仍有Dart资源编排；需收敛到持久原生连接owner，保留失败可重试与退出排空契约。bootstrap还在依赖构造时调用native recovery，不能声称全仓所有恢复调用已删除。之后是完整GUI（包括上述关闭警告复核）、Windows当前安装/特权矩阵、真实校园/长期、原计划高级规则/平台能力、远端CI与历史文档口径清理；不能以本批Linux隔离成功替代这些要求。

第五十批当前状态（2026-10-16，优先于下文49批）：ConnectionController已删除`dart:io`、`_serverUri`、`_localIPv4`及共享LAN提示。网关原始字符串直接进入原生connect_account，由connection/gateway完成HTTPS补全/规范化、显式端口保留及非法scheme/控制字符/凭据/fragment拒绝，继续接受账户配置的origin授权检查。账户本地HTTPS连接测试改用无scheme输入证明真实路径，FFI补非法网关在认证前拒绝。

ProxyBinding新增display_endpoint/display_warning；原生在监听成功后读取单播地址表，按现有可用网卡及推荐接口选择同地址族提示，过滤回环/未指定/组播/link-local/IPv4广播及mapped IPv6。Windows用GetUnicastIpAddressTable，Linux用rtnetlink地址表并排除暂定/DAD失败/过期地址，Local优先于Address。查询预算3秒，找不到或读取失败时保留通配端点并给出诊断；不改变监听/路由，不证明远端LAN穿透防火墙。HTTP/SOCKS分别持有展示端点，IPv6用标准SocketAddr方括号，不再被另一监听的IPv4提示覆盖。

HTTP stop直接委派native单命令，由已有Rust driver先恢复系统代理再停止监听；删除Dart重复restore。SOCKS停止失败保留运行标志和绑定供重试。独立只读复核指出停止期间开启被吞、旧stop结果污染新owner两类竞态；HTTP/SOCKS共6项新增回归先失败再通过。控制器仅为在途命令/展示结果保留停止future与代次检查，旧重绑/批量stop不得操作替换后的owner；**启动配置、会话替换及TUN失败回收编排仍在Dart，本批不是整个生命周期归Rust完成**。

Windows309/Linux288 Rust全特性全量、两端Clippy/analyze/release、Windows268/Linux267 Flutter全量通过；50项目标控制器/权限测试通过。新增Windows真实通配双栈代理端点完成本机HTTP/SOCKS响应及停止后端口拒绝。Linux现有隔离全量测试及专属dummy网卡namespace测试通过，实际提示192.0.2.10与2001:db8:50::10，普通用户完成两族连接，父namespace不变；不代表不同设备可达。真实新库FFI验证display字段、网关拒绝，原有代理/清理回归保留。

最新成对候选 `build/desktop/f12d997bc8ec/`，397来源文件两端SHA `f12d997bc8ec12169fa7026ce456114994b216a570f189c4f4471b7692cfdc5d`。Windows ZIP20,657,627字节/SHA `f3cd20a1fa908d3f39235a7cc071bd78e18b1c922a99b681e5bc8d6d6f2773fa`；Linux tar20,167,493字节/SHA `7f5b9e1740202afa638fda89f7d7bf4577c14bfb20a74a2f59044a78daf17d80`。两端解压库Native FFI58/57、桥接及三套跨进程存储、Windows随包helper4项普通权限测试和归档完整性通过。打包器/安装事务代码未改，未重跑上批19项工具单元测试，不冒充本批安装验收。

FRB2.11.1已重新生成ProxyBinding四字段编解码，但内置content hash仍为1236410258：已查固定生成器generate_content_hash只对函数名称求哈希，DTO字段变化不纳入。**hash相同不证明ABI兼容**；以新配套完整包、来源清单和实际字段FFI验证为准，不混用49批库/绑定。

Linux本批正式release在私有Xvfb/XDG完成欢迎页启动、实际随包so加载、无托盘关闭退出0，截图已检查；只证明启动/退出，不是完整界面/登录/安装/TUN验收。报告 `build/desktop/native-endpoints/linux-gui.png`，原始日志/进程库映射在Linux快照同名目录。Windows本批未跑GUI。候选清单partial-desktop-candidate；无持久化格式变更，回退使用匹配的完整包。三名本批只读代理已完成关闭，上批三名完成代理的线程槽也已回收，全部本批测试/构建/GUI/打包句柄已收取。未提交/发布，goal active。

下一关键路径：将`ConnectionController._connectNativeOperation/startProxy/_startDesktopVpn`及`RustConnectionAttempt`内的恢复、路由挂接、启动配置、失败回收/迟到owner清理编排收敛到原生owner，同时保留部分服务失败不破坏可用代理、清理失败保留owner并阻止替换、退出排空在途资源的现有契约。最后仍需完整GUI、实际安装/特权TUN、真实校园/长期运行、远端CI和README历史口径清理；原计划的高级规则与平台能力验收也不得仅凭本批端点迁移勾选。

第四十九批当前状态（2026-10-16，优先于下文48批）：验证码默认方式、每方式发送间隔和senderror后的替代选择已迁入Rust Account的Delivery；Flutter表单不再持有60秒常量、递减计时、默认方式或按错误码切换策略。账户提供verification_state/select_verification_method，前端每秒读取展示快照。发送前预留60秒，失败/取消也保留，以覆盖远端可能已接收请求的情况；切换、cancel_work和同owner重新登录不能清除预留，senderror只选择替代而不自动重发。预留只在Account实例内，不持久化、不宣称跨应用重启限流。

只读核验确认旧待办中的“认证超时/网络重试”已在Rust：QR的180秒预算及轮询、凭据90秒总预算、OTP有限重试不应继续列为Dart残留。页面450ms成功动画、原生状态采样和销毁取消属于界面职责。本次未处理ConnectionController服务启动/清理、输入规范化与LAN选择。

原生新增5项验证：虚拟时钟精确期限/方式切换、错误替代不清预留，以及真实本地HTTPS的发送计数、重新登录、限流、并发选择拒绝和在途取消。独立复核另发现“验证码已成功、凭据交换失败返回旧挑战”路径；新增界面回归先失败，修复后18项目标UI/模型测试通过。UI替身只给出明确原生响应，不作为真实MFA网络证据；真实FFI覆盖新增方法的无挑战/非法选择边界，正向MFA网络由Rust本地HTTPS夹具验证，未登录学校。

成对候选 `build/desktop/18b91af88fee/` 已生成，396来源文件两端SHA `18b91af88fee9a73e12ebf953495e8a652e9ce5ef9ad37db3a6d9215939a3545`。FRB2.11.1重新生成，内容1236410258，不能混用旧库。Windows ZIP20,624,454字节/SHA `f46be5d45e9307f139c21bb99f5f5fc170af2655a5a164adf2e47ed96248e4ef`；Linux tar20,142,626字节/SHA `5d4cf5f7948a6f957ef428678677890f72851a8edafb89c03eeb8145e6faa27d`。本包包括48批设置清理。

Windows305/Linux284 Rust全特性全量、两端Clippy/analyze/release、Windows259/Linux258 Flutter全量通过；两端解压库Native FFI58/57、桥接与SSO/账户/设置跨进程探针通过。Windows随包helper4项普通权限测试通过；Windows打包测试10项通过/9项Linux专属跳过，Linux打包/安装事务19项全通过；两包完整性验证通过。实际本批数量按日志记录，不沿用48批数字推算。

失败日志保留：Linux直接在宿主网络跑Rust时三个http_tunnel测试超时，改用现有普通UID隔离network namespace入口后全量通过；首轮WSL命令PATH含空格未正确引用，在测试前失败，后改固定PATH。Linux首轮Flutter的`$PWD`被WSL启动层提前展开成/mnt/d目录，导致14项FFI加载失败并留下等待；确认错误路径后停止该次运行，用明确/home/mzee绝对库路径重跑通过，旧三个进程PID均已不存在。没有修改断言或以超时当通过。

证据位于 `build/desktop/native-verification/`，候选清单 `build/desktop/18b91af88fee/verification.json` 状态partial-desktop-candidate。三名只读代理完成，测试/构建/打包句柄已收取。本批没有实际GUI或安装/特权TUN重验；上一批GUI不能代表新验证码界面实际校园运行。无持久化schema变更，回退需使用配套旧应用/原生库/绑定整包，不能单换DLL；内存预留随owner关闭结束。下一关键路径仍为native服务生命周期、输入/LAN所有权，之后最新GUI/安装/特权TUN、真实校园、长期运行与远端CI。未提交/发布，goal active。

第四十八批当前状态（2026-10-16，优先于下文47批）：已删除SettingsStore中的Dart偏好读写、schema迁移、默认值和校验实现，从455行降为75行展示接口；同步写统一拒绝，save/restore/validator均由RustSettingsStore承接。settings_schema.dart与旧presets.dart已删除。ShuProxyListen仅保留已由native规范化的地址与展示属性，旧InternetAddress/parser/fromStored入口删除。RustSettingsStore.fromNative仅接native owner，bootstrap将启动时原preferences直接给账户迁移；一次性旧键导入/提交成功后清理语义保留。

UI测试显式使用无持久化/迁移/生产校验的内存stub；端口拒绝提示是测试注入响应，不冒充Rust实际错误文案。原默认值检查转为真实native设置测试；新增地址IPv4/IPv6规范化、非法地址拒绝、65535实际保存重开、恢复默认后domain/timeout/protocol与账户/设备/TOFU键保留断言。save与restore交错两种顺序由typed native fake验证，已有成功/失败队列和dispose拒绝分发保留。Rust新增新装/v0/v1/v2 welcome快照、未来schema未知值编辑重开、同批代理依赖顺序3项；两端settings模块13项通过。

最终Windows250/Linux249 Flutter全量、analyze/release、全目标全特性Clippy通过，393来源文件两端SHA `4882577be236893de3a879d5e39b67eb4da3beb81b866da51ec95c5e9e8567c5`。旧schema9项、proxy旧实现13项和旧默认值UI1项退役；保留proxy展示2项，新增真实native默认值/监听2项及队列顺序2项，267−23+6=250（Linux同差）。Rust生产/FRB公开签名未改，内容734160530，当前只重跑新增settings模块与Clippy，不冒充全部原生或特权矩阵重新执行。

Windows当前release实际规则v2/读回、记录空页、标题提示与托盘正常退出通过，原测试配置字节一致恢复、无待恢复备份；Linux当前release私有Xvfb/XDG连接/设置/网络页、无托盘退出0与native guest重开通过，截图已检查。报告 `windows-native-settings-gui/`、`linux-native-settings-gui/`。三名只读代理已关闭，全部测试/构建/GUI句柄已收取。证据 `build/desktop/native-settings/verification-progress.json` 状态source-local-release-and-gui-validated-not-archived；**本批未生成新归档**，最新033905归档不含本次设置清理。

下一关键路径为native服务启动/清理、输入/LAN策略和剩余前端认证超时/自动重试策略，随后最新归档/安装/特权TUN及真实校园/长期/远端CI验收。Dart的旧网络/账户/公告/更新/设置具体后台均已删除，但剩余策略所有权与最终验收仍未完成；goal active，未提交/发布。原有::1展示仍采用保守曝光确认，不是本次parser删除引入的问题；未据此声称所有UI策略已归Rust。

第四十七批历史状态（2026-10-16）：已删除ShuAccountCenter具体后台及14个旧auth服务文件、旧账户快照存储、公告HTML/parser/isolate和更新HTTP/版本比较/资产选择。AccountCenter从646行变为149行的展示接口；验证码/凭据/学生档案移入纯模型，凭据DTO不再承载payload、maskedSecret或本地expiry计算。App要求显式账户注入，默认公告/更新均为Rust，bootstrap始终完成Rust健康检查和原生账户/设置初始化；扫码页仅接受QrAccount并渲染原生图片字节。旧cookie/profile/device三个键仍经Rust提交后一次性清理，迁移失败保留输入。

flutter_sangfor、flutter_sangfor_atrust、flutter_sangfor_easy_connect与Dart的HTTP/HTML/RSA/SOCKS后台依赖及Git override已移除，crypto仅为测试依赖。旧Dart anti-MITM生成工具随SDK退役，冻结向量继续由Rust验证。包配置和两端生成插件注册表已无旧SDK，增量构建残留的Windows DLL/Linux so已按明确构建路径移除，实际包清单验证不存在。Rust协议栈继续使用既定sangfor-core/tls crate。

新增验证码/扫码成功、过期、取消后迟到结果、无扫码能力拒绝共5项界面回归；native账户真实导入/重开验证姓名、学号、年级、学院、专业及注销清空。纯DTO映射测试明确使用typed fake，不冒充HTTP/FFI；真实FRB回归复现公告/更新ready owner在await期间关闭后的DroppableDisposedException，派发前_live检查修复。注入owner的close异常也经finally释放，5项红回归后13项目标测试通过。Rust补末页、无日期、不可用链接文字、浏览次数作者误判、图片/表格preview与缺/null/非字符串tag边界。旧UI/模型测试保留，旧实现专属测试退役；351−106+22=267，Linux350−106+22=266。

最新成对候选 `build/desktop/0339059010a0/`，395来源文件SHA `0339059010a09aeb9bfc78b5d21bd95e1da6e2110cf8803a542966370289716b`，两端一致。Windows ZIP20,612,683字节/SHA `3ec508f9f63455cfa51d538ef2e545dde654076c99bf5edac9a0cd8fb415e5ff`；Linux tar20,141,915字节/SHA `7985ac90c891229ddb1bc59619ac9867d0a2be1fdc44d629076af5ee4c964af5`。verification.json为partial-desktop-candidate，未提交/发布。46批仅本地构建的清理已包含在本批归档。

Windows296/Linux275 Rust全特性全量及两端Clippy、Windows267/Linux266 Flutter全量、analyze/release通过；解压实际库Native FFI55/54、桥接和SSO/账户/设置三套跨进程探针通过，随包Windows helper4项非提权进程验证、打包/安装事务19项及两包完整性通过。Rust生产原文与46批快照逐文件比较一致，仅cfg(test)向量增加，FRB公开签名/内容734160530未变；Linux核心/helper和Windows驱动与45批字节相同，Windows核心/helper摘要有变化，已按新解压包实际验证，不推断原因或冒充最新特权矩阵。

Windows当前release已实际完成规则v2/独立读回、记录空页、标题提示、托盘正常退出及原测试配置字节一致恢复；Linux当前release在私有Xvfb/XDG完成连接/设置/原生guest账户页、无托盘正常关闭与独立重开。GUI受测bundle所有运行文件逐项匹配新归档，截图已亲自检查。两端均无真实校园登录，本批Linux规则仍fixture v1。报告为 `windows-native-services-gui/`、`linux-native-services-gui/`，原始证据在 `build/desktop/native-services/`。测试内异步File读取导致QR spinner未结束已修正为同步读取固定图片夹具；一次Linux无.git快照运行打包单测漏传来源环境，补正确revision/dirty后19项通过，失败日志保留。

四名只读代理和全部测试/构建/GUI句柄已收取关闭，无已知在途app或待恢复配置。下一步：删除SettingsStore/settings_schema中的旧后台，实现/收敛剩余前端超时重试与native服务启动清理、输入/LAN策略；随后完成最新安装/特权TUN、真实校园/长期/远端CI验收。整个后端零Dart目标仍active，不能仅凭SDK删除或当前候选包标完成。

第四十六批历史状态（2026-10-16）：ConnectionController已真正删除旧SDK认证连接、TOFU、DNS/资源路由、本机HTTP/SOCKS、计量与定时采样代码，从2167行降为1289行；缺少native factory时明确失败，不再执行旧后端。界面和托盘改用项目自身ConnectionStatus/ConnectionErrorCode，协议的SDK product映射、Dock旧验证码弹窗/注入已删除；正式账户MFA仍经RustAccountCenter和独立登录表单。统计直接展示原生快照，取消/迟到结果/清理重试保留。

六个生产模块shu_http_proxy、shu_traffic、vpn_routes、vpn_packet_log、atrust_channel_probe、atrust_l3_framing及五个专属测试文件已删除，未移到其他生产目录。旧测试共97项，其中4项格式化测试完整迁入connection_telemetry_test，其余93项随退役实现移除；新增缺factory预初始化边界、factory创建异常和连接按钮取消3项。Rust已有实际CONNECT/SOCKS/HTTP/半关闭/端口回退/停止监听/授权/DNS/双栈测试，另补Proxy-Connection剔除及非法请求行400断言。旧逐包日志文案/200条预算不冒充已迁移，日志页已移除该承诺及observer依赖。

独立复核发现两处原有缺陷：factory()在try外导致异常后busy卡住，toggle()被busy挡住无法取消。新增两项红回归均复现，修复后通过。全量还暴露旧并发认证测试把Dart调用顺序当成FRB/Tokio锁获取顺序；fixture增加首个HTTP请求的明确屏障，保留首调用ticket/次调用复用/仅一次OAuth的严格断言。未改生产认证或放宽断言。

最终Windows351/Linux350 Flutter全量、两端analyze/release及全目标全特性Clippy通过；Rust新增断言所在proxy_session各10项真实传输回归通过。测试数量变化为441−93+3=351与440−93+3=350，不与历史总数混报。407来源文件两端SHA `f42a47c4b0c0efc9628202495f0534b3f6df6a7e8549d493e4735020a40a1603`。Rust生产实现/FRB签名未改，内容734160530；Windows核心/helper/Wintun、Linux核心/helper逐项等于45批归档，不冒充重跑全部原生或特权矩阵。Windows app.so从9,274,248变为8,962,952字节，摘要已记录，仅作为源码删除的辅助证据。

Windows新release实际GUI已通过规则version2、DIRECT选择、域名规则停用、连接记录空页、caption提示和真实托盘退出0，独立native重开状态一致且未登录；此前遮挡缺口在本批补上。原测试配置before/after字节一致、备份不存在，journal为gui-accepted-and-original-restored。报告 `build/desktop/rule-editor/windows-native-controller-gui/`。Linux新release在私有Xvfb/XDG中通过连接/设置/网络权限页、无托盘正常关闭0与独立native guest重开，截图已亲自检查；规则保持fixture version1，未声称本批Linux规则提交或实际校园连接。

清单 `build/desktop/native-controller/verification-progress.json` 状态source-local-release-and-gui-validated-not-archived；**本批没有新归档**，最近成对归档仍424ec，不含本批清理。所有构建/GUI/测试句柄及四名只读代理已收取关闭，未提交/发布，goal active。下一步处理AccountCenter/SettingsStore基类与默认provider中的旧后台、auth/credential服务、公告/更新遗留及SDK依赖/插件；随后收敛native启动/清理策略、输入规范化与LAN地址选择。最新安装/特权TUN、真实校园/长期/远端CI仍未完成，不能宣称整个SDK或后端零Dart完成。

第四十五批历史状态（2026-10-16）：当前桌面范围清理后的源码已修复两处编译缺失及设置页首次权限/网卡读取遗漏。端口编辑复用现有输入框，调用SettingsStore.validationMessage（正式桌面委派Rust），保存提交成功后再重绑；增加空串/非法整数/负数/超限及native端点0、65535验证。纯流量/耗时格式化进入connection_telemetry，旧包观测模块保留兼容导出，桌面展示格式化不再依赖该后端模块。Android工程和专用入口的移除以本文永久范围修订和当前工作区为准，44批“保留Android”仅为历史状态。

Rust VpnConnectAttempt新增connect_account，认证准备和L3握手由同一原生deadline包裹，超时取消该attempt并返回typed timeout；正式RustConnectionAttempt直接调用connectAccount，已删除Dart preparation.timeout与prepared句柄编排。成功session保留preparation_warnings。真实TLS账户fixture验证来源拒绝、VIP成功、兼容警告、资源/控制握手取消、准备超时及登录保留；虚拟时钟验证600ms+600ms不能越过1000ms总预算，FFI验证非法预算/来源/预取消。最初warning fixture公布的是静态另一张证书，连接正确被pin拒绝；随后使用本次TLS证书构造独立ResourceWarning场景，未放宽生产校验。不是严格墙钟期限或远端登录撤销证明。

最新成对候选为 `build/desktop/424ec5d01b42/`，417来源文件SHA `424ec5d01b42cfb65e0b26506f21a74bd34b1da3b4bb98fd70387902afa987ca`。Windows ZIP21,305,694字节/SHA `99034d583ef103e25da2d6b2fc4e5e0c6e05c75bbee74c16668c406d4391ee3d`；Linux tar20,788,092字节/SHA `eba3a4b4dea516e33ec60a9bc4928f5f66bdc4f47f303962fd4ad23f0d8cd38f`。FRB2.11.1已重新生成，当前内容摘要734160530，不能混用旧包或旧绑定。verification.json状态partial-desktop-candidate，未提交/发布。

Windows294/Linux273 Rust全特性全量、两端Clippy、Windows441/Linux440 Flutter全量、两端analyze/release通过；实际解压库native目录Windows53/Linux52 Flutter FFI、桥接及SSO/账户/设置跨进程探针通过，打包器10项及两包完整性通过。Linux最后移除三个无调用cfg(test)方法以修复Clippy；Windows全量及实际库测试在42c55来源完成，此差异不进入Windows编译或任何release，最终Windows归档除package.json来源清单外37项payload逐项与受测归档字节一致。两端来源均已核对，Linux缓存旧源码按固定目录校验后同步删除。

Linux新release实际GUI已进入设置/网络权限状态、重新引导、桌面权限说明和登录介绍，Alt+F4退出0后独立native重开确认未登录；截图已亲自检查，报告 `build/desktop/rule-editor/linux-deadline-gui/`，规则仍fixture version1，本批未操作规则提交。Windows实际GUI本批未验收：第一次seed仍传已移除android参数，修正测试工具后第二次因目标坐标属于DSH NEXT而拒绝注入；原配置before/after清单字节一致，journal为original-restored、无待恢复备份/已知在途app。不关闭或修改遮挡进程，不把启动截图当完整GUI成功。

所有构建/验证/GUI句柄已收取，四名只读代理已收取关闭；证据在 `build/desktop/desktop-finalization/`。下一关键路径是ConnectionController中旧SDK/Dart连接、代理、路由和计量的实际删除及native启动/清理所有权；LAN地址选择仍是Dart。Windows最终GUI、最新安装/特权TUN矩阵、真实校园、长期运行与远端CI保留未完成。goal active，不能以本批候选包替代整项后端迁移完成。


2026-10-14 交付范围更新（优先于下文历史现场）：用户明确本次只需要 Windows 与 Linux 两个桌面版。Android 不属于本次交付或验收前置；停止把 Android 第三十三批作为当前关键路径，转向桌面正式功能、打包和验证。完整桌面验收条件见主计划开头。

第四十四批历史状态（2026-10-16）：已删除生产混合认证层 `lib/core/rust/rust_atrust_authentication.dart`、createRustAuthSession、NativeATrustTunnelAuthentication、ShuPreparedATrustTunnel，以及RustConnectionAttempt接受ShuAuthSession/明文planJson的兼容分支。桌面attempt现在在core初始化/代理恢复前仅接受NativeAccountAuthentication，后续prepareAccount→opaque prepared→connectPrepared，保留取消/迟到prepared释放及finally dispose。Android旧账户与协议实现未迁移或删除，不作为本次前置；整个控制器/SDK清理仍未完成。

旧适配的协议回归已直接调用native.ATrustAuthClient，保留OAuth七请求顺序、Cookie作用域/同名path删除/过期发送及导出过滤、并发串行、active/queued取消、resource sid/节点/DNS/routes/CSRF/body和资源取消断言。设备号改由正式RustAccountCenter采纳旧值、删旧键、重开保留及native新生成。新增边界红回归原先进入旧来源准备分支，现提前拒绝；独立 `test/native_account_boundary_test.dart`不先初始化core，使用FRB公开initialized状态证明旧账户/取消后仍false，故意缺少native目录也2项通过。未把旧Dart backend driver移到其他生产目录。

最终Windows457/Linux456 Flutter全量与analyze通过，两端本地release已重建；Rust及生成公开签名未变，Windows核心/helper/Wintun、Linux核心/helper逐项等于43批归档，不重跑或重标原生/特权矩阵。最后增加的独立测试不改变已重建runtime源码。443来源文件两端完整SHA `7b2410bd3efb3bec9c09c8cb384c03a58cc05a1f3c46b961d85860a974a10a93`。清单 `build/desktop/backend-boundary/verification-progress.json`状态source-and-local-release-validated-not-archived；**本批没有新归档或新GUI运行**，最近归档仍97486，不包含这批来源清理。

AOT检查前后都没发现三个旧类型/工厂名称，不能据此证明运行期曾执行或整体后端已删除。Windows app.so大小前后同为9,307,016字节，SHA从d69c...变为7afa...；名称扫描只是辅助，核心证据是生产调用/删除、原生账户类型边界和实际协议回归。一次standalone Dart检查因adapter导入Flutter日志而无法使用dart:ui，失败日志保留，已删除该工具并改为独立Flutter测试，没有为了检查拆坏生产日志。

3名本批只读代理已收取关闭：账户/设置/公告/更新有界生产映射未发现额外可达旧业务；外围旧连接依赖map因PowerShell路径语法错误提前返回partial，缺口不作“全仓无残留”证明；删除范围独立复核未发现新增运行缺陷，提出初始化/expiry覆盖边界后已补严格检查。所有测试、构建句柄已收取，无已知在途任务，没有修改真实账户配置或OS网络。

下一关键路径仍是控制器native/legacy真正分离及Rust owner策略：ConnectionController依然包含旧SDK连接、代理/TUN/流量实现；native分支调用共同teardown/客户端启动顺序，RustConnectionAttempt仍有Dart preparation.timeout，desktop all-interface代理仍经Dart NetworkInterface.list选展示LAN IPv4。端口AddrInUse回退已在Rust session.rs，不能误报为Dart残留。后续亲自读将修改的controller/native代码，逐项迁移所有权/策略，不以目录移动或条件封闭宣称完成。最终安装/特权TUN、真实校园/长期/远端CI保留。未提交/发布，goal active。

第四十三批历史状态（2026-10-16）：连接记录、保守重复条件遮蔽说明及桌面协议保护已进入成对候选 `build/desktop/97486cf710e0/`，partial-desktop-candidate、未提交/发布。443来源文件两端完整SHA `97486cf710e00ed28f19e68ee21588446fa65f3686a38e47c0e13f95b61ed7d8`。Windows ZIP21,357,852字节/SHA `aefc3665dcba4f6414103dc399a3d8d31e1b59d924abbf8b2b3ef4cd9cefcdd9`；Linux tar20,803,514字节/SHA `97ba8e3c785e686462489bcec14905d6adc374cf03d29eb61a72a96314ff46ac`。verification.json保存本轮边界与证据摘要；旧归档保留历史。

连接页桌面右上“连接记录”通过新可选NativeConnectionObservations薄接口读取原生VpnSession.decisions，展示目标、协议/入站、规则版本/ID/来源/策略、期望动作、成功出站、拒绝/失败原因与DNS来源/缓存/TTL。纯Dart DTO不计算路由或DNS。记录是最多128条结果历史，不是活动连接或交付成功证明；无时间戳/连接ID/实际selected peer，不虚构这些字段，原生捕获前失败/特殊DNS路径也不保证完整。decision:null的顶层version单独保留，DNS解析地址与已知Flow.ips分开显示。页面以owner代次/可用性和请求revision丢弃迟到数据，断开清空，刷新失败明确保留旧快照，退出后忽略旧future。

nativeFactory存在时controller现在对非aTrust直接unsupported，不再越过native分支进入旧端点/SDK；两项强制允许未实现协议+无效地址的回归修复前确实进入旧URI处理，修复后提前拒绝。生产原生settings只启用aTrust的约束不变。旧Dart/SDK源码仍在，**不是后端零Dart完成**，后续要处理明确的桌面模块边界与删除审计。

Rust共享shadowed_rules按prepend→profile→append比较已启用规则的(kind,修剪后的条件文本,no_resolve)，忽略MATCH/停用前项，只记录第一个前驱，不删除/重排/优化规则。编辑模型给每条卡片shadowed_by，编译preview最多32条重复说明+汇总；其他原有notices不受该32条重复说明预算约束。UI明确不穷举正则/逻辑等价或子集覆盖。跨来源、停用/no-resolve差异、非同文本域名子集不误报及200重复保留201有效规则的预算测试通过；真实FFI预览与决策仍选第一条。仅JSON元数据和私有实现变动，公开FRB签名/内容摘要仍-790204183，没有无谓重新生成。

最终Windows289/Linux274 Rust全特性全量、两端全目标全特性Clippy；Windows454/Linux453 Flutter全量与analyze通过。归档解压库的原生/规则FFI Windows19/Linux18通过，包括真实TLS校园echo、DIRECT、规则REJECT、资源表unrouted和DNS缓存元数据经新reader读回；Newsso/账户/设置三套跨进程存储、随包Windows helper4项非提权进程检查与两包完整性通过。Windows35/Linux28个运行文件匹配实际GUI受测bundle。日志在 `build/desktop/connection-records/`。

实际GUI只验证未登录会话的记录入口/空状态、标题栏与正常退出，不冒充有真实校园流量的记录页。Windows `build/desktop/rule-editor/windows-connection-records-checked/`也复测规则v2保存/独立读回与真实菜单退出0；Linux `linux-connection-records-gui/`无托盘关闭0。两端记录页截图已亲自检查。Windows前两轮因其他进程遮挡坐标/鼠标API暂时拒绝而安全失败，独立原地设置诊断恢复后复测通过；每轮都同盘备份/恢复原配置，最终journal为gui-accepted-and-original-restored，before/after摘要相同，无待恢复备份。没有真实账号登录、宿主路由或WARP/Clash修改。

3名本批只读代理均完成关闭；schema/历史含义核对、UI异步独立复核与Rust重复条件审查已消费。所有本轮构建/GUI/包/FFI句柄已收取，无已知在途任务。下一关键路径：桌面旧后端/SDK真正删除与所有权审计、最终特权TUN和安装分发验收，再补真实校园/长期/远端CI。Android不作为本次前置，不能把当前保护或历史页当作整项验收完成。

第四十二批历史状态（2026-10-15）：规则编辑与标题栏浮层修复已进入两端候选包 `build/desktop/310b8f2f9bb1/`，status为partial-desktop-candidate，未提交/发布。439来源文件两端一致，完整SHA `310b8f2f9bb1fdc491d9c3c03b900b081b048b757376c6c7ac5b404de9a59a0c`。Windows ZIP21,331,681字节/SHA `15ce818f2d56f5e1224fc99edaf40919a8eba34d7f7529808a8a91efb122b3a4`；Linux tar20,790,606字节/SHA `9b9905af379f2e04576b9653d6d163d672c6220c62f77f2e58f8d087335ce7c5`。清单verification.json区分本轮与继承证据，旧现场不改称最新。

最终Windows433/Linux432 Flutter全量、两端analyze与缓存release构建通过；本轮生产代码只改DesktopFrame及Dart回归，Rust/FRB不变，继承41批Rust286/271和Clippy/真实HTTP传输证据，没有声称又跑过特权TUN。新归档解压后的真实桥接、Newsso/账户/设置三套跨进程存储通过，实际随包Windows helper4项非提权进程检查通过（不会请求UAC/创建网卡）。两包完整性通过，Windows35/Linux28个运行文件逐项与GUI受测bundle摘要一致。正式新绑定内容摘要仍-790204183，不能与40批旧库/绑定混用。

两端**最终release**都完成实际规则页进入、策略组改DIRECT、校园域名规则停用、预览2/应用版本2、正常退出和独立原生进程读回version2/选择/启停，确认无账号登录。Windows本轮最终报告 `build/desktop/rule-editor/windows-release-gui-ready/`，Linux `linux-release-gui-overlay/`；规则应用及标题栏“最大化或还原”提示截图均亲自检查，实际加载配套库映射已核对。Windows真实菜单退出0，Linux无托盘Alt+F4退出0。只有未登录UI fixture，不冒充学校登录/连接或最新安装/特权矩阵。

Windows托盘退出已实际通过：旧UIA根没有找到退出，原生#32768菜单的PID、线程、GUIThreadInfo.MenuOwner及flags20确认归属；定向按键没退出，PowerShell COM接口转换也失败。最终用引用PowerShell自带Accessibility的强类型C#辅助功能桥，枚举真实菜单并调用“退出”默认动作，进程退出0。C#仅针对已验证自有菜单；局部处理Accessibility的System.Runtime4→当前runtime转发警告，不改生产校验。成功日志 `windows-release-gui-typed-check.log`，菜单证据在 `build/desktop/desktop-ready/windows-gui-release-typed-{menu,msaa}.json`。这解决的是验收工具，不是生产退出实现修改。

Windows早期规则页stdout有真实 `No Overlay widget found` / `_OverlayPortalState` 错误；后来在自有关闭命令前进程已退出，stdout记录MenuItemBase选择11，harness未到退出段、没有本次menu.json，不能归因为崩溃或猜测是谁关闭。失败证据保留 `windows-release-gui/`；之后消息点击/临时禁用窗口的方案未稳定触发，`windows-release-gui-overlay/`的提前退出码实测0，也未计通过。最终转为短时自动实际鼠标输入，每点核验坐标属于自有PID、拒绝点击被其他进程遮挡的位置，并恢复鼠标。一次点击被遮挡、一次未等待原生预览导致未提交版本2的失败保留于physical/input报告；最终增加可见“应用预览”就绪检查后才通过独立数据断言，未放宽版本/选择/启停检查。

已亲自定位并修复标题栏浮层缺口：DesktopFrame在MaterialApp.router.builder中位于Navigator Overlay外，之前home:DesktopFrame的测试掩盖了它。新增真实鼠标hover/router.builder回归，修复前稳定失败；DesktopFrame用当前SDK的 `Overlay.wrap` 自带生命周期/child更新，在标题栏提供浮层。悬停三按钮、同实例切路由/明暗主题、浮层显示时卸载及原窗口/托盘13项目标用例通过（`windows-overlay-{red,green}.log`），最终两端真实窗口也显示正确提示，没有No Overlay错误。已同步、全量及重建release，最终日志为 `{windows,linux}-overlay-{full-tests,analyze,release}.log`。

Windows原配置每轮均先同盘原子移动到带GUID的备份，再创建标记一致的独立fixture；结束验证绝对路径/标记后仅移除该fixture并恢复原目录。最终 `windows-profile-transaction.json`为gui-accepted-and-original-restored，before/after逐文件摘要一致，Roaming/ShuVPN下只有恢复后的ShuVPN，没有在途备份。helper脚本均在build目录，不是生产后台；没有真实账号登录、宿主网络/TUN或WARP/Clash改动。

下一关键路径：连接明细/规则遮蔽解释、桌面后端旧SDK边界与删除审计，再完成最终特权TUN/安装包、真实校园/长期及远端CI。原生observations已有VpnSession.decisions JSON，可先接薄展示；nativeFactory仍只在aTrust条件下返回，未实现协议当前由原生settings/UI封住，但旧SDK分支源码仍存在，不可声称零Dart完成。本批所有构建/GUI/FFI/包验证句柄及2名只读代理均已收取，无已知在途任务或待恢复配置。

第四十一批历史状态（2026-10-15）：规则卡片/排序/启停/策略组与测试器已接线，通过源码及默认 Debug 库真实 FFI 阶段验证，**该批没有重建 release 或新归档**。40批 `286a92e1ea47` 仍是最近归档候选，含 Windows Android 文案修复，不包含这批规则编辑。该批439来源文件两端摘要一致：`4d45f5baff54722661a35ff6d035842f1bb20bf313776bd22dd8b6c10e170364`。详细证据与日志摘要为 `build/desktop/rule-editor/verification-progress.json`，status 为 source-and-debug-ffi-validated；未提交/发布，goal active。

`rust/src/routing/editor.rs` 负责纯草稿解析/转换，公开 `routing_editor` / `edit_routing` 两个 JSON 桥接接口；Dart DTO 不解析 YAML/规则语法。Profile/前置/后置接受旧字符串或 `{rule, enabled}`，编译器仍校验停用条目后才排除匹配；唯一启用 MATCH 不可删除/停用/重排。可视化操作保留 provider/group 字段并序列化为 JSON，不保留注释和排版。纯草稿允许暂存尚未编译的规则，必须 prepare/commit 才能发布；界面明确区分“请校验并预览”与“已通过校验”。共享策略组校验已避免 orphan selection 被编辑器隐藏。旧二进制不能读取新增对象条目；应用内版本回退已验证，不声称任意程序降级兼容。

页面提供配置档卡片/重命名、前置/配置档/后置规则新增/编辑/删除、拖动及上下按钮排序、启停、手动策略组与自动校园开关、固定最终兜底。每来源25条分页，上下按钮可跨页；高级导入折叠，原始文档修改会使可视化模型和预览失效。页面进入/销毁亦丢弃旧预览，异步编辑检查代次。规则测试器可选 TCP/UDP、HTTP/SOCKS/TUN，进程规则仍按原生能力列表报告不支持。真实UI控件通过 FFI 证明草稿不改变当前版本、编辑使旧预览失效、排序/选择保存及释放旧owner后重开配置实例；这是同进程新实例，不冒充跨进程正式应用重启。

最新验证：Windows286/Linux271 Rust 全特性全量、两端全目标全特性 Clippy；Windows432/Linux431 Flutter 全量、两端 analyze，及各8项规则真实 FFI 套件均通过。Linux Rust 在普通用户隔离 netns 中执行。`routing_outbounds::disabling_a_reject_rule_requires_commit_and_keeps_existing_gateway_tunnels` 在两端通过：预览前/提交前403且物理端点无accept，commit后真实DIRECT端点回显 `real`，原校园TLS流继续回显 `kept`，只见原校园目的地、决策版本1/2及finishStop无诊断。28个FRB2.11.1文件重复生成摘要 `336b40f8f95e725fb0e973712dfcd4453a1d4a1d6bba4e5b058342fa829547a5` 稳定，新内容摘要 `-790204183`，必须配套新Dart与库；40批包继续使用其旧配套绑定。

失败证据保留：启停对象原红回归解析失败；一次测试类型转换遗漏已修正；生成器缺 powershell 路径先失败，补齐后成功；Windows 沙箱内7项helper通道超时，沙箱外完整通过；新增UI用例先因 pumpAndSettle 等待持续进度动画超时，改为等待真实FFI结果，后因原配置owner未释放而重开失败，正确释放后通过，没有放宽文件锁或校验。日志 `build/desktop/rule-editor/*-current.log` 代表最终检查，早先 final/fixed 命名文件只对应相应中间现场，不按文件名猜最新状态。

下一关键路径：本批 release 配套构建与实际规则页 GUI/完整包验证，再补连接明细/规则遮蔽说明、桌面后端残留/旧SDK边界、Windows托盘退出及校园/长期/远端CI。只读协议链核对并已点验：生产 Rust settings 只启用 aTrust，桌面 native factory 将它送入 Rust；未实现协议 UI/设置都不可用。但 controller 的 native 条件仍只覆盖 aTrust，源码中旧 `_buildConnector`/EasyConnect SDK 分支仍在；不得据当前不可达就声称后端零Dart完成。全部本轮句柄及4名只读代理已收取关闭，无已知在途任务。Android不作为本次前置。

第四十批历史状态（2026-10-15）：用户指出 Windows 显示 Android，已用真实 native desktop factory 和 available=true 的 widget 回归复现。根因是引导页把 `vpnSupported` 当作 Android 授权能力，而桌面可用后它也为 true；`vpnPrepared` 又不代表桌面权限，可能把用户卡住。新增 `vpnPermissionSupported` 区分 Android 授权，桌面不列/不申请 Android VPN 与通知权限，不阻止登录。Windows 显示按需管理员确认，Linux 显示安装/用户授权说明；设置目录和关于页权限说明也已修正。

40批归档候选包在 `build/desktop/286a92e1ea47/`，`verification.json` 明确为 partial-desktop-candidate，未提交/发布。434 个来源文件在 Windows/WSL 摘要一致：`286a92e1ea47299966b5ce954cd2ce51597d3f784814fa5ceaff9b30559bebf3`。Windows ZIP 21,190,793 字节/SHA `184aade8dcc1f1b5244ee84565e7099393af3ac7fc2090a8ae1a959985e96cff`；Linux tar 20,667,909 字节/SHA `440044727d8f939661dddd96c3f1b2fc253480d3320d5afb8ad1897872a65078`。两包完整性通过；原生库/helper（Windows 含 Wintun）逐项摘要与39批 `9dab21c86e7d` 相同，本次只改 Dart 展示/权限判断与测试，没有重新执行特权网络矩阵。

Windows 427/Linux 426 项 Flutter 全量和两端 analyze 通过，Windows 测试显式使用新 Release DLL/Wintun，Linux使用新 bundle/lib。最初未指定库路径的两端测试实际选中旧开发库，失败并结束；Windows 随后的 Debug 库检查缺 Wintun 定位失败，也未计通过。最终日志 `build/desktop/desktop-ready/*-platform-ui-full-tests-{bundle,fixed-library}.log`；两端 release 已重建。Windows 与 Linux 实际 welcome/权限页截图均已亲自检查：`windows-gui-platform-permission.png`、`linux-platform-gui/permission.png`，并核对实际包内原生库映射。Linux 无托盘 Alt+F4 退出0。Windows 托盘退出自动化未找到退出项，finally 只终止本次自有进程，不计正常退出通过。UIA 曾只有 FLUTTERVIEW，不再用它宣称界面初始化成功；截图工具已修正 DPI 和 PowerShell null 字符串的子窗口查找问题。

后续关键路径仍为规则卡片/排序/单条开关/策略组选择、桌面后端所有权及旧 SDK 审计、Windows 实际托盘退出、真实校园/长期运行/远端 CI。只读定位结果：原生 `Profile.rules`、`Overrides.prepend/append` 当前都是 `Vec<String>`，没有 disabled 持久字段；`ConfigurationInfo` 也没有结构化编辑模型。主线程已读完整 `routing_settings_page.dart`（目前仅原始文档编辑），下一步须亲自读 `rust/src/routing/model.rs`、`compiler.rs`、`api/routing.rs` 和 `lib/core/rust/routing_service.dart` 后设计 native-owned 编辑接口，保留 prepare/commit 与 stale-preview 保护，不在 Dart 解析 YAML/规则。全部本轮构建/测试句柄和两名只读代理已收取，无已知在途任务；goal active。

第三十九批后端记录（2026-10-15，优先于下文38批Windows关闭标记）：Windows正式TUN能力已接线，available按管理员身份或配套helper文件就绪判断，缺少helper时返回完整目录恢复提示；sessionDataPlaneReady=true。helper启动与检测共用同一规范化路径。普通用户/UAC helper的正式startDesktopTun已通过计划范围核对、MTU裁剪、校园IPv4 UDP/双栈TCP及FIN、DIRECT UDP/TCP DNS、finishStop/出口恢复/精确LUID接口消失；日志`build/desktop/desktop-ready/windows-desktop-kernel.log`。新的独立Dart AOT程序放在实际release DLL/helper旁，使用默认加载和无tun-probe库，也完成校园UDP/双栈TCP字节、DIRECT双协议DNS、实际helper PID消失，`windows-default-ffi-kernel.log`退出0。Dart入口为`tool/windows_desktop_tun_bridge_smoke.dart`，仅用于明确获准的三个主机路由验收，不随应用分发。

Windows早先失败的观测已保留于`build/desktop/windows-packet-diagnosis/`：helper读写无错，收到的55个包均为系统组播，没有测试单播包。最初网卡快照有Up的CloudflareWARP；随后只读CLI返回Disconnected，接口快照变为空，提升权限本地和普通/helper对照都完整通过。我们没有请求断开或修改WARP/Clash/防火墙；这些是环境关联证据，尚未证明WARP共存的具体因果，不应声称修复了共享包解析器。诊断计数/包头采样仅在tun-probe构建启用。

Linux独立TCP和延迟HTTPS响应跨helper故障的内核验收已补齐，`build/desktop/helper-recovery/http-normal.log`和`http-crash.log`均退出0。普通UID/CapEff0持有native Record；SIGKILL后broker实际替换helper，原TCP恢复绑定后继续收发，HTTPS的尾部必须在恢复确认之后由独立peer释放，原响应完整结束。故障诊断Some保留、新会话启动、所有原/替代/重启helper PID消失及物理源地址核对均通过。CI增加同一crash模式，独立只读审查未发现该测试范围中的伪通过路径。

**安装GUI验收发现并已修复实际启动缺陷**：旧包从源码目录运行时，FRB先选`rust/target/release`的旧so，截图为Dart/Rust内容摘要不一致的“无法启动应用”。先前窗口显示/退出检查不足，不再当作初始化通过。新红回归`linux-gui-cwd-red.log`退出预期失败，`native-library-all.txt`实际映射到开发目录，不能以重建或删除旧开发库掩盖它。`RustCore`现在优先加载当前桌面可执行文件旁的配套库，缺失时不让正式应用退回开发搜索路径；显式库参数及非桌面入口保留。Windows默认FFI已验证该选择；Linux 新 AOT 实际安装/default FFI/GTK/no-tray Alt+F4 全部通过，截图 `linux-gui-fixed/screenshot.png` 已检查为正常欢迎页，实际库为 `/opt/shuvpn/lib/libshuvpn_core.so`，日志 `linux-installed-gui-ffi.log`。GUI harness加入实际加载路径核对、私有DBus运行目录及Alt+F4正常关闭，避免直接销毁窗口伪造退出。

39批 Windows278/Linux263 Rust及两端全目标全特性Clippy/default check通过；加载修复后的 Windows423/Linux422 Flutter/analyze/AOT 均已收取通过，旧句柄64397/63026不再在途。阶段包 `9dab21c86e7d` 已生成并保留为40批界面修改前的历史快照；38批 `85a19cb14947` 同样保持历史，不代表新Windows能力或GUI加载修复。原始日志在`build/desktop/desktop-ready/`，当前候选与后续缺口以上文40批为准。未提交/发布，goal active。

第三十八批历史本地阶段（优先于下文37批安装缺口）：新增随包Linux安装器/菜单入口，GUI在`/opt/shuvpn`保持普通用户，固定helper在`/usr/lib/shuvpn/shuvpn_tun_helper`为root:shuvpn/0750及NET_ADMIN/NET_RAW。复制后逐文件校验、可信安装路径、组授权、升级事务与失败回滚、卸载保留用户数据已实现。`tun_driver_info`改为异步：正式可信路径的实际协议/权限握手及子进程退出确认后才报告可用，不接受诊断helper定位作为安装就绪依据。9项安装事务、两端10项包测试通过，CI和本地显式隔离安装验证已接线，远端CI未运行。

普通用户/CapEff0加载**最终Linux归档中的默认正式库**，经固定安装路径完成安装前拒绝、授权后可用、未授权用户/组可写/NoNewPrivs/无capabilities拒绝、升级/卸载与反复检测；新mount/netns中的overlay/tmpfs隔离宿主安装/账户，最终核对helper PID归零和宿主账户文件摘要不变。增强既有`tool/tun_dns_bridge_smoke.dart --installed-helper`，同一环境中实际`startDesktopTun`自动选shuegress/MTU1400、校园DNS与元数据、动态域名路由、校园TCP字节及域名决策、finishStop无诊断/接口消失均通过。最终日志`build/desktop/linux-install/installed-current-package.log`。这不代表完整GUI操作，也不把37批诊断构建的完整DIRECT双栈/HTTPS矩阵冒充本轮默认FFI已重跑。

Windows UAC错误详情丢失已定位到`managed.rs::helper_error`的`error.to_string()`，改为保留原因链；1223取消映射保持。新增确定性回归先失败后通过。新特权探针本轮**成功启动UAC helper并创建index51接口**，但第一个校园IPv4 UDP回包超时、decisions为空；其后finishStop、状态stopped/pending=false、出口撤销及精确LUID接口消失全部确认。日志`build/desktop/linux-install/windows-tun-runtime.log`；校园TCP和DIRECT DNS尚未进入，不能计为成功。根因未确定，生产Windows available/sessionDataPlaneReady保持false，不把上次36批完整成功作为本轮稳定性通过。实际UDP期限源码为5秒，不是3秒。

当前成对候选包在`build/desktop/85a19cb14947/`，**partial-desktop-candidate，未提交或发布**。433个来源文件两端/当前仓库一致，完整来源SHA`85a19cb14947ad1face911af1230470d953617b5fd4f30b5b6c2b17f3fddc213`；Windows ZIP 21,191,175字节，SHA`f7c400bb116b07887548df0e7f6a55d971e7cc3d52e838baae1f7bc89b51cd07`；Linux tar 20,663,340字节，SHA`688e9412ebeadcfa0516b0b78fb19252287b6c1a92b1cb33b0fc328cc743d9b0`。清单`verification.json`与`build/desktop/linux-install/verification-progress.json`保存本轮边界。

验证：Windows278/Linux263 Rust全量与两端全特性全目标Clippy通过；Windows423 Flutter加载当时新构建的默认Debug DLL，Linux422 Flutter加载实际release归档库，两端analyze通过。Windows新release bundle另通过47项native Flutter和真实FFI/三套四进程存储；最终ZIP解压库FFI健康通过，归档逐文件与受测bundle一致。最终Linux归档与受测Flutter归档的全部运行文件清单一致，并再次执行固定安装/default FFI实际TUN。28个FRB文件重复生成摘要稳定。原始失败/成功日志均在`build/desktop/linux-install/`；所有本批构建/测试句柄与只读子代理已收取，沙箱内失败生成器的遗留子进程树也已按PID核实并结束。

下一关键路径是Windows首次校园UDP不稳定的证据定位（先区分TUN未收到包与下游分派问题），以及Linux独立TCP/HTTP连接存活时helper异常退出后的恢复实测、完整安装GUI操作、桌面Rust所有权/旧SDK与剩余交互审计、校园/长期运行。只读审查另提示Windows managed read在oneshot成功投递后、调用方未轮询即取消可能丢包；现有用例只覆盖投递前取消，需先写确定性复现。当前bridge使用持续独立读循环，该线索**未证明是本轮首包超时原因**，不应直接据此宣称修复稳定性。整体goal保持active。

第三十七批历史阶段（优先于36批源码/包描述）：Linux高层会话现已使用 `linux_tun::managed::Prepared/Device`，物理TCP/UDP、既有socket保护/恢复、DNS及HTTP均接入同一capability broker。HTTP改用已有的受限请求级连接器，复用真实socket保护与恢复；不再由reqwest隐藏的interface选项绕过登记。退出等待在途socket创建，取消后仍显式排队恢复原绑定，drop清理任务和失败owner保留在retirement表。helper协议已升为2，增加显式Shutdown与实际子进程退出确认；36批旧包不含这些变化，不能与当前库/helper混用。

普通UID65534、CapEff=0的正式 `startDesktopTun` 已完成一轮隔离内核验收：自动选shuphys/MTU1400、既有控制连接保护、校园IPv4 UDP及双栈TCP、DIRECT双栈UDP/TCP（256KiB+FIN）、物理DNS/TC回落/错误ID拒绝、HTTPS证书拒绝与受信载荷、REJECT/校园授权拒绝、停止恢复/重启/finishStop。父进程保持存活时确认两次helper PID均消失；对端实际观察到的源地址为192.0.2.1和2001:db8:ffff::1，没有VIP泄漏。`build/desktop/linux-session/desktop-kernel.log`退出0；首轮仅因探针providers目录未创建失败，原日志 `desktop-kernel-provider-directory.log`保留。测试使用tun-probe的临时helper定位入口，**固定路径安装、默认正式库的普通用户FFI/界面可用性仍未验收**。

独立审查发现publish取消后“旧绑定快照看似未变，跳过恢复，但队列中保护稍后生效”的真实竞态。确定性用例 `cancelled_protect_restores_after_the_already_queued_bind`修复前稳定失败，改为始终将恢复提交同一broker后，隔离空间内12项网络/HTTP和7项TUN生命周期通过。共享注册入口另补退休状态复核及Windows迟到保护回归。WSL主网络中最小绑定lo TCP也超时，而新netns中同UID1000成功；没有据此宣称根因，Linux实际HTTP测试保持原断言和期限，在普通UID的隔离空间执行。正式入口为 `tool/rust_linux_namespace_tests.sh <user> <absolute-cargo> --locked --all-features`，本地verify脚本通过显式 `SHUVPN_RUST_TEST_NAMESPACE=1`选择，CI已接线。

本批后端阶段验证已收齐：Windows277/Linux263 Rust全量、两端全特性全目标Clippy和默认不带诊断特性的all-targets check通过。最终50244的低层helper拒绝/回滚/崩溃与正式会话内核套件均退出0；所有本批句柄（包括42582/74922/70377）已收取，无在途构建或子代理。原始失败、确定性回归、全量、默认构建和内核日志在 `build/desktop/linux-session/`，阶段清单为 `verification-progress.json`。本批未重新执行当前库的Flutter/FFI或生成新的最终分发包，36批旧包继续只代表历史快照。

下一步：固定路径安装/授权组与可信helper可用性、默认包内库普通用户FFI/界面、helper异常退出后仍存活独立TCP/HTTP连接的恢复与broker重启实测，再处理Windows UAC启动诊断/稳定性、生产启用和两端最新包。Linux已实现的broker重启不能仅凭静态审查当成该故障路径验收。当前Linux界面available仍只按CapEff判断，普通用户开关未放开；整体goal active，未提交或发布。

第三十六批当前状态（优先于下文35批“未执行提权”和旧句柄描述）：Windows 已实际执行获准的特权探针，一轮 `runtime-strict-luid.log` 完整通过校园 fixture UDP/双栈 TCP、显式上游 DIRECT UDP/TCP DNS、finishStop、精确 LUID 接口消失、出口恢复与 fixture 连接归零。随后 `runtime-stability.log` 在启动 UAC helper 阶段失败，未创建网卡；日志只有 `launch TUN helper with UAC`，不能断言具体原因或把复测计为通过。生产 Windows available/sessionDataPlaneReady 仍 false。原始日志均在 `build/desktop/windows-tun-probe/`。

35批候选包保留不变，但已不对应当前源码或绑定：后续加入了 Windows 接口实际就绪检查、停止正常取消归一、每次启动的显式物理 DNS 上游参数及其 FRB 绑定，必须重建配套库/包。下文35批的418文件一致性只对当时快照成立，不能将旧包作为当前源码交付。

Linux 独立 capability helper 已实现并通过普通 UID 的隔离内核场景，细节与生产接线缺口见 [Linux 桌面权限边界](linux_desktop_permissions.md)。私有 SOCK_SEQPACKET/SCM_RIGHTS、TUN fd 双 owner 关闭、动态路由、已有 socket 保护/恢复、失败创建回滚、未接管结果/丢弃 lease、helper 与父进程崩溃均有实际证据；三项 IPC 单测与两端全特性 Clippy 通过。Windows276/Linux256 Rust、Windows423/Linux422 Flutter及两端analyze通过；Windows本轮Flutter加载当前Debug库，Linux加载正式bundle库，不冒充新的Windows发布包。Linux正式bundle构建、实际随包helper全部正常/崩溃内核场景、真实FFI及账户/设置/SSO四进程恢复通过。打包新增 Linux helper 必需文件、执行权限及 ELF 校验，9项包测试通过，3项新增回归先失败后修复。正式普通用户高层 TUN/出口/HTTP 接线与固定路径安装仍未完成，GUI 未授予 capabilities。

本轮最新Linux候选包为 `build/desktop/linux-helper/release-artifact/ShuVPN-0.4.2+9-linux-x64.tar.gz`，20,112,815字节，SHA256 `77914b2d6b5cf6e2dbdc55620791950b4ea61f46c9350f072603c49caa5eef09`。424个来源文件与当前仓库一致，来源摘要 `72e24a38e40777e2fed3aa49499d79e59be3da9cd6e97cc20dd8d4da2df1b6a8`；完整清单为 `build/desktop/linux-helper/verification.json`，状态为partial-desktop-candidate。本批日志已复制本工作区同目录，所有本轮构建/测试句柄已收取，无在途子代理；未创建提交或发布，整体goal active。下一项实施是正式会话与物理出口broker接线，不能以这些探针通过代替普通用户正式启动、安装或真实校园验收。

35批最新候选包（优先于下文34批）：完整现阶段本地验收结束，两个当前包及SHA256、verification.json在 `build/desktop/d4742d6af314/`。Windows274 Rust/422 Flutter，Linux251 Rust/421 Flutter，原生全目标全特性Clippy/格式、两端analyze、解压包内真实FFI及三套四进程存储均通过。随包Windows helper4项非提权进程测试通过。Linux三组内核和两组实际包内库FFI通过，其中已实际执行正式 `startDesktopTun` 自动底层网络选择、网关计划/MTU、tunStatus及finishStop：`DESKTOP_TUN_DEFAULT_PLAN_KERNEL_AND_CLEANUP_PASSED`。418个来源文件两端及当前仓库一致，完整SHA `d4742d6af314593705ba92014179886b95f39c284a9b6103e457fd6a0dacb49d`。原生日志在windows113101/linux113330目录，包/Flutter/内核日志在windows114924/linux115237目录（均build/desktop/20261014-前缀），Linux日志已同步本工作区。当前包与34批ABI不同，必须配套新绑定/库。

正式桌面设置页已接原生网络枚举、按网关资源准备的接管计划、TUN状态/独立停止；桌面不显示Android授权/DNS项，底层网络是明确标注的本次启动草稿。Controller关闭失败保留owner、迟到连接失败保留重试、退出等待连接任务、TUN迟到回滚复核generation/identity、同一连接停止合并等均有失败前后回归；42项连接/控件/退出目标测试通过。旧34批实际DLL上的连字符区间UDP授权用例稳定失败，当前完整新包已通过相同用例。

Rust已增加desktop_network_interfaces、desktop_tun_plan/start_desktop_tun、tun_status、finish_stop；finish保留原stop诊断并只在资源owner清理确认后释放，失败系统代理Lease由Shared保留可重试，worker panic记账不可被公开stop消费，独立清理错误累积保存。No Service但有出口lease的状态也报待清理。绑定固定2.11.1多次稳定；生产Windows available仍false，Linux根据实际CapEff的NET_ADMIN/NET_RAW只读判断，不伪造权限。

38683/49975在Dart风格提示阶段退出1，原生部分通过；仅修复7条Dart lint后从Flutter阶段续作10572/80041均退出0，未冒充首轮完整通过。Linux内核58961退出0。当前只读验收程序与最新源码配套刷新句柄 **43539**（先收取），其余构建已结束。已编译但未执行提权的程序 `build/desktop/windows-tun-probe/desktop_windows_tun_smoke.exe`，配helper/Wintun；只读`--plan WLAN 1.1.1.1`通过，plan.log明确3条host routes。当前WLAN index11/IP192.168.33.106，OS DNS只有127.0.2.2/3本地stub（含mapped表示）；1.1.1.1只是验收拟显式上游，不是声称OS配置。`--run-privileged`会请求UAC和短暂捕获该上游/32，**未经用户新的明确同意不得执行**，此前UAC取消要求仍有效。实际Windows特权成功/普通Linux用户权限架构、安装与真实校园继续；goal active。Linux已确认glibc2.35、GLib2.72.4、GTK3.24.33；不要直接给使用$ORIGIN/lib的GUI主程序setcap，独立权限helper方案仍需实现与验证。

已核对上一会话最后一轮状态为 failed，平台错误为 “This content was flagged for possible cybersecurity risk.”，不是迁移完成。第33批最新模拟器日志 `build/android-routes-runtime.log:171` 仍为 `direct-tcp_FAILED ... Kernel TCP bytes/FIN changed`，不计通过。第32批仍为最近完成整批本地验收的基线。现有未提交改动须保留；旧日志中的执行句柄和PID不是当前运行证明。

34批最新现场（优先于下文历史在途记录）：本批本地验证结束，整体桌面交付目标仍active。候选包及SHA256、`verification.json`在 `build/desktop/ba4f80583397/`；Windows ZIP与Linux tar.gz均为0.4.2+9/x64。409个来源文件两端及当前仓库摘要一致，完整SHA为 `ba4f805833978d131a8f2cd76349fb9cd0d127646cb1e6bfeb3d582a2d61d29b`，未提交。最终归档运行文件与实际FFI/Flutter验收归档逐项一致。

Windows266 Rust/402 Flutter、Linux242 Rust/401 Flutter，两端全特性Clippy/analyze、真实FFI与设置/账户/SSO四进程恢复通过。随包Windows release helper的4项非提权IPC测试通过；Linux三组Cargo内核与两组明确加载解压包内.so的真实FFI内核探针通过。日志已保存本工作区 `build/desktop/20261014-070352-windows/logs` 与 `build/desktop/20261014-070546-linux/logs`。遗留FIN回归在两端先失败后修复，未放宽检查或改依赖缓存；FRB2.11.1重生成摘要稳定。所有本批句柄均结束，无在途构建或活子代理。测试AVD已核验、unroot并停止，旧PID136636/140984已消失。

下一关键路径为桌面正式TUN界面/配置与物理接口选择、权限/生命周期接线、前端关闭失败保留句柄和状态权威收敛、桌面SDK后备路径审计及剩余桌面交互、安装/真实校园验收。Windows特权TUN成功路径尚未验收，driver就绪标记仍false，不自动重试此前被取消的UAC。Linux快照已同步；移除了唯一多余的旧 `rust/src/connection/packet/direct.rs`，内容完整备份在 `build/desktop/obsolete-linux-direct.rs`。快照无.git，打包使用显式revision/dirty和实际来源摘要；不能把独立Cargo的.so当作Flutter包内库，现有探针接受库路径。详细实现、证据和回退见实施记录第三十四批。

更新日期：2026-10-14，Asia/Shanghai。工作目录：`D:\Desktop\ShuVPN`。以下保留各批历史现场。

33批更新（2026-10-13 10:55，优先于以下现场）：66009退出0，nonce `b13b7f824f6548b686cd2d701048ec77`、PID4823，完整动态内核routes/持久TCP/原32批数据与物理出口/OS撤销/stop通过，stop约43ms，接口消失、宿主connections0；证据 `build/android-routes-first-complete-{pass,fixture,native}.log`。仅加诊断后成功不能证明此前stop间歇性问题修复。后续20279退出1（nonce0cfefa...，campus TCP4 TLS unexpected EOF/reset，build/android-routes-campus-tls-*），79183退出1（nonce0d9753...，domain-udp前的首次A查询单包超时、无flow_start，build/android-routes-late-dns-*）。三轮都通过完整动态阶段。独立源码审查未找到packet JoinSet drain的直接取消缺口，fixture也已正确TLS shutdown；根因不能提前归为fixture。匹配build-id `3d57e523d76e2620465096945b83833b73244b88`的243个native frames已符号化，build/android-routes-late-dns-symbols.log，显示线程池/runtime停车，没有业务锁链。47323结束0，全部scout已关闭。

辅助Activity发现并修复旧UI finish回调可晚于下一个case启动的真实竞态：activeCases计数、终态marker在UI清理提交后发出。尚未重跑修复后完整模拟器。driver新增`--runs 1..10`（默认1）可一次构建连续独立进程验证，每轮清logcat/重置peer授权、按宿主hello/closed/DNS retry/HTTPS增量验收，保存每轮native/kernel记录；最后finally保存完整native日志。未添加DNS重试或放宽任何预算。另新增TunIo测试模拟Builder清空旧内核队列，36002退出1、稳定得到None丢包，日志build/android-routes-pre-drain-before.log；主线程已将旧队列排空提前到Builder前，安装后再排空迟到数据，维持4MiB预算/失败关闭，当前6项测试句柄 **76092** 正在执行（先收取），尚未跑新版Android。原5测试结果不能代替此改动验收。

Linux正式流水线8363与release examples/root隔离三组59198均退出0：正式bundle401 Flutter/实际FFI/设置账户SSO三套四进程存储，TUN rollback/crash、DNS TTL、双栈TCP UDP校园/DIRECT/REJECT全通过；日志build/android-routes-linux-*已同步。其生产Linux代码与最新相同（TunIo只Android生产/Linux test编译），但后续新增test需单独补验证。399相关sourcehash已同步一次，此后Activity/driver/TunIo新增改动使清单再次过时，最终需重同步。Android交叉Clippy42053退出0（pre-drain改动前）。新runId绑定已配Linux正式.so，Windows正式DLL仍32批；33批ARM release、默认raw回归及Windows正式流水线仍待做。AVD继续存活PID136636/emulator-5580/shuvpn_native_api36，未unroot或kill。

33批续作现场（2026-10-13 10:33，优先于以下早期在途记录）：动态路由实际内核验证已覆盖初始无A/AAAA主机route、DNS新增双栈route、TTL后由活动flow保留单族route、最后flow关闭后回收，以及同一Socket/本地端口的campus/domain4/domain6跨重建echo。保留runId=2、lease=3，未重连会话。JNI Builder返回后新增awaitInstalled，等待ConnectivityManager当前LinkProperties的实际接口及routes完成更新，解决首次DNS回包丢失。辅助Activity在持久Socket打开期间保持前台；API36的冻结后台进程socket清理曾干扰IPv6，未修改全局OS策略或重建Socket。

最新完成句柄59422退出1，nonce `3b17ba1c8f344add9b10a934491b3c51`、主PID4036：上述全部动态阶段及原32批数据/TLS/真实OS撤销pending HTTPS已通过，但最后stop超过原8秒预算。日志保留 `build/android-routes-final-stop-failure.log`，native stacks只能看到Tokio等待，尚不能确定未退出任务。主线程为egress-probe增加flow_start/flow_done与TCP读写子任务完成标识，当前完整重跑句柄 **66009**，日志 `build/android-routes-runtime.log`、APK构建 `build/android-session-apk-build.log`；先收取此句柄，不启动重复fixture或APK构建。本轮未放宽关闭预算或资源断言。

5项tun_handover真实Unix内核测试已通过（16099退出0，build/android-routes-handover-test.log，包含最新guard drop顺序）；早期Windows15435/Linux20138全量Rust与Clippy退出0，Android交叉Clippy60153日志成功，均不能替代后续修改的最终回归。source manifest `build/android-routes-sources.json`已过时，Linux快照还需最终同步，正式33批产物及配套FFI尚未验收。AVD仍存活emulator-5580/shuvpn_native_api36，启动PID136636；本轮所有只读代理已关闭。整体goal仍active，生产Android仍旧SDK。

第三十三批在途（优先于下文32批已完成状态）：正在实现Android动态域名route重建，同一lease/network/session不关闭，JNI0.22.4按实际缓存源码接JavaVM/global class ref，spawn_blocking调用@Keep NativeTunBridge.rebuild；Java holder持PFD，take校验实际Builder SHA256后detach，Rust立即OwnedFd。NativeTunSession配置拆成不可变NativeTunConfiguration，rebuild与legacy/native共用establishing/epoch预留，revoke关闭pending holder并取消Rust。新增platform/tun_handover持有替换任务/read-write gate，先打断长read/暂停I/O，Builder后排空旧fd队列，切新AsyncFd再关旧fd；4MiB预算、迟到fd拒绝、取消等待保留任务，失败回滚可安装恢复fd并返回原错误。初期3项真实Unix内核test已通过；后续新增成功回滚/遗留错误不吞新请求等共5项，完整回归正在执行。主线程最新调整guard drop顺序（pause更新在gate解锁前），此一行尚未重同步Linux；最终sourcehash目前过时。

33批当前活句柄：8297（完整动态模拟器driver，首次JNI依赖/新APK正在编译，日志build/android-routes-runtime.log，实际APK构建日志仍用build/android-session-apk-build.log）；20138（Linux全量Rust/Clippy，首次下载jni source）；15435（Windows全量Rust/Clippy/Debug库）。先收取，不重复启动；41060 codegen、77495交叉Clippy、47675 analyze已成功，运行时尚未验收。32批旧DLL/APK不能配33批新runId绑定；JNI导出新增revision/interfaceIndex，checker现在52必需符号。本批尚未build正式release或进行最终配套FFI，不能按32批绿色推断完成。

动态测试：初始_tun去掉10.99.0.7/32、2001:db8::7/128，Native runId跨重建不变，Dart输出ROUTE_STATE/revision/ifindex；驱动root ip -j route table all实际检查INITIAL_ABSENT、DNS_ADDED_BOTH、V4_LEASE_AFTER_TTL/V4_RELEASED、再双栈与V6对称。PersistentTcpProbe同一Socket/本地端口在campus/domain4/domain6跨增删route重复echo，close后等lease释放；再完整32批数据/TLS/OS撤销测试。已采用按结果marker同步，不用peer -W。CNAME TTL3/地址RR10，最终按有效缓存/系统route变化观察，不只盲等。JavaFixture活动app普通UID，adbd root仅观测实际路由。

33批AVD已启动并boot：emulator-5580/shuvpn_native_api36，启动PID136636，build/android-routes-emulator*。宿主当前WLAN192.168.33.106（已Get-NetIPAddress核验），driver用不可变build/android/session-fixture/android_network_fixture.exe（32批直接物理TLS fixture协议不变，无需重编）；最后身份核验后unroot/kill。目前全部只读探索代理已完成；两组复核返回问题已改：Builder前取消不额外回滚；取消等待后的历史restore错误不吞掉新请求；pending清理保守关闭；legacy晚engine关闭fd在service guard内复位handoff标记。仍有两名已completed复核代理需close：Goodall 01a132d8-12c8-7bb2-a95b-ff0f4d07abfc、Leibniz 01a132d8-df27-70e3-97e7-aa9c74767fb6（不要继续追派）。生产Android仍旧SDK，整体goal active。

最新第三十二批已完成本地验收，完整迁移目标保持active：Android高层TUN接到同一Rust VpnSession、规则、TCP/UDP与DNS capture，Java实际Builder输入与Rust计划用SHA256绑定；NDK按具体Network绑定socket并核验explicit/protected mark。HTTP复用受保护连接器，JNI只cancel，Rust拥有并join撤销任务；恢复成功从scope成员移除socket，避免同handle误关。pending cleanup拒绝默认出口，失败owner保留供重试。Private DNS的DoT/DoQ在捕获模式明确拒绝，不能DIRECT回落。

最终完整session探针34183退出0，nonce a3c8aecfa3bd4a0684a05160d3d33629、PID5408：同MTU scope不一致拒绝、正常stopTun恢复后session继续、独立UID校园UDP/双栈TCP、DIRECT TCP/UDP、内核DNS后的域名双栈TCP/UDP、拒绝/未授权、DoT/DoQ、物理DNS/TCP重试/坏ID、严格TLS/测试根、NDK resolver、同handle恢复UDP、阻塞锁下revoke不等IPC/取消finish保留worker/重试join均通过；第二provider.prepare真实撤销后pending HTTPS以非timeout结束，完整stop、FD/出口/capture清理、接口删除和宿主connections0通过。上传524438，DNS queries10/retries2、HTTPS1、stalledHello/Closed各1、errors[]，日志build/android-session-runtime.log。早先79466/25462成功不替代最后修复验收；中间89443关闭超时通过确定性回归closing_packet_port_cancels_a_queued_capture_reply先失败后修复。TCP/UDP acquire及capture回复等待现在响应取消，桥接退出先cancel子任务再Port.close。

最终Windows253/Linux231 Rust、两端全目标全特性Clippy/analyze、两端x64正式打包库Flutter402/401和设置/账户/SSO三套四进程存储通过；最新Linux三组root namespace覆盖TUN/回滚/崩溃、DNS TTL/域名与双栈TCP UDP校园/DIRECT/REJECT通过。Windows5280正式流水线退出0，构建761.3s，80456原生release预编译也退出0；51221/33746/36689均退出0。Windows最终DLL SHA256 26934DD5AE7686068C423B7A0778DFB25C208D1ABC36472E8F5CC6C91F7713E4；Linux正式.so为6b94ffabb1552401799957b132a021efa0a9c4fa8aa1a13fc4f988d10221f731。

默认无egress-probe的低层x64回归66630也退出0（nonce67ddf3cef40d490da2163b6c6286778a、PID5813），双栈UDP/未消费10s/取消/OS撤销均通过。ARM64 release主入口build/android/artifacts/shuvpn-arm64-session-release-local.apk，34,477,777字节，build-id9c226e9c23ebfc5ac9e15f3048df076e870345d3，50导出/16KiB/SONAME/Cargo build-id与本地Android Debug签名检查通过；没有ARM硬件运行/远端发布。构建故意携带SHUVPN_RUST_PROBE=egress-probe，release仍强制false，checker拒绝debug fixture的负控制通过。正常出口probe API是capability stub，不能说没有所有诊断。marker是当前实现的保守静态门，结合Gradle/cfg/日志/实际产物证明本批隔离，不夸大为任意手工构建或整APK/16KiB设备已验收。

394相关源码/测试/Android/CI配置摘要两端一致，build/android-session-sources.json；格式/Python工具编译/diff/结构检查通过。新FRB由固定2.11.1生成，API已不同于31批，必须配套本批库。日志build/android-session-*；Linux日志已同步本工作区build。所有本批句柄已结束，没有在途构建。AVD emulator-5580/shuvpn_native_api36已身份核验、unroot、停止，启动PID137352消失且adb列表空；别按旧PID文件重判存活。没有活代理、未创建提交。

当前能力：sessionDataPlaneReady/socketProtectionReady=true，captureMode=preinstalled-routes，dynamicRouteUpdates/fullCaptureReady=false。动态域名超出已安装route返回capture_scope，不能假装捕获。生产Android仍旧SDK；下一步继续Android动态路由/配置重建与正式业务接线、后端状态权威收敛和旧SDK/Dart删除，完整平台/分发/ARM硬件/真实校园目标都保持。回退配套恢复同版FRB/JNI/.so及清理owner，不删除已迁账户/设置或导入门。完整证据见实施记录第三十二批。

复用工具：SDK/JDK/Gradle缓存沿用31批，显式NDK Python避免Windows Store占位。debug session driver自动设置全大写SHUVPN_RUST_PROBE；宿主fixture直接绑定本机拥有的IPv4（本次WLAN192.168.33.106）作TLS网关，不用ADB reverse/额外转发，正常TLS shutdown后排空TCP FIN再drop；半关闭/close_notify检查没有弱化。peer用结果marker同步，避免am start -W拖过TLS预算。Linux命令用wsl --exec bash -lc，root为wsl -u root --exec bash -lc，正确传递退出码。

以下保留前批历史，不能用其中的在途句覆盖本段最终状态。

最新第三十一批已收尾，Android低层TUN与两端正式bundle本地验收通过：platform/packet_fd共享Linux/Android取消/操作租约/FD关闭；JNI同步dup真实PFD、Java立即关原描述符、Rust以一次性令牌消费。两轮独立审查修复engine晚detach、worker泄漏、native/legacy建立TOCTOU、Rust关闭后Java占用，以及watchdog先清占用后cancel的空窗；占用保留到原生全部drained。最终29235退出零，API36/x64普通应用+独立UID辅助应用完成双栈UDP往返、重复消费拒绝、未接管10s回收、持lease取消/重复close、第二provider.prepare系统撤销与内核接口消失；nonce8652d44d788b4d0fbb1ecc0b780943a6、PID2105。root adbd只观察显式身份匹配的debug AVD，应用普通UID；失败的appops deny刺激不计成功。测试后已unroot并停止AVD，PID132476消失且adb列表为空。48873正式Windows流水线及83155独立root Cargo release均退出零，本批没有在途构建。

第三十一批已完成Windows250/Linux227 Rust、两端Clippy/analyze、两端正式x64 release实际打包库402/401 Flutter及设置/账户/SSO三套四进程存储，以及Linux三个root namespace的TUN/DNS/双栈TCP UDP/回滚崩溃清理探针。Linux计数已按最终日志各组passed纠正。ARM release主入口新APK约33.8MB和x64独立探针APK均通过50必需导出/16KiB/SONAME/build-id，ARM本地Android Debug签名有效、未发布；ARM正式Kotlin字节码包含最后watchdog修复。产物build/android/artifacts/shuvpn-arm64-native-tun-release-local.apk与shuvpn-x64-tun-smoke.apk，日志build/android-tun-*、build/android-fd-*。389个相关源码摘要两端一致，build/android-tun-sources.json；新FRB API已变，必须配套本批新库，30批旧APK/bundle不适用。当前fdIoReady=true，高层sessionDataPlaneReady/socketProtectionReady仍false；生产Android仍走旧SDK，完整目标保持，下一步继续高层会话/socket protect、正式业务接线和旧后端删除。

最新第三十批已收尾：Android工具链、ffiPlugin/Cargo→generated jniLibs、16KiB/SONAME/build-id接线完成，修复6处空Backend匹配和Android std File.try_lock Unsupported被误报locked。内核flock及锁错误分类已覆盖设置/SSO/规则/系统代理，Android UID/O_NOFOLLOW和私有权限生效。arm64 release APK约32.1MB，45导出/16KiB/剥离前后build-id及本地debug签名校验通过；x64 APK在API36隔离模拟器四个进程2606/2669/2727/2788通过桥接/存储/注销/双owner排他/强杀恢复。Windows250 Rust/402 Flutter、Linux223 Rust/401 Flutter，两端正式x64 release/打包库/Clippy/analyze/三套四进程存储通过，379个源码/测试/Android构建与CI配置摘要一致（build/android-sources.json）。51229/92782/24173/26275均退出零，无在途构建；模拟器已明确停止（原PID126632消失，adb列表空）。生产Android仍走旧SDK，fd/protect/正式切换、ARM硬件/校园验收和旧后端删除继续，完整目标保持。产物build/android/artifacts/shuvpn-arm64-release-local.apk及shuvpn-x64-native-smoke.apk；ARM Debug旧包为锁修复前，不用它验收新代码。工具缓存C:/Users/xiema/.cache/shuvpn/android（Temurin17.0.20.1/SDK35+36/NDK28.2.13676358/cmdline19/emulator37.2.12/API36 AOSP x64），Gradle专属heap3G/worker2，Cargo每variant独立缓存/2jobs，日志build/android-*。

前一完整基线第二十九批已收尾（10月12日开始、13日结束）：Rust settings owner/schema/校验/原子存储、FRB及桌面RustSettingsStore/bootstrap已接线，页面await保存后导航/重绑。最终两端x64正式release/实际打包库Flutter402/401项、Rust249/222项、全特性Clippy/analyze和设置/账户/SSO三套四进程存储通过；338个相关源码摘要一致，build/b4-sources.json。连续命令排序另经3项确定性测试先失败后修复，失败返回调用方且队列继续，销毁后不再发送排队命令。最终发布49657/35039均退出零，没有在途任务。Windows首轮rlib缺失/编译器ICE经仅清shuvpn_core编译缓存、-j2完整重跑后通过。settings采用专属settings-state.json与锁，成功采纳后只清旧settings.*，auth/TOFU/设备号不属于该清理范围；共享文件原语已提取，SSO DPAPI不变，未新增Windows私有ACL。Android、连接权威状态、旧后端删除和完整平台/校园目标继续，不能把桌面设置批次标成整个B4完成。

前一完整基线第二十八批：Windows/Linux正式x64 release、实际打包库Flutter391/390项、Rust239/212项、Clippy/analyze和账户/SSO存储通过；29071/75499均退出零。桌面公告使用Rust请求/解析/缓存，更新薄适配已接Provider（更新UI仍仅Android启用）。322个相关源文件摘要一致（build/b3-sources.json）。

前一完整基线第二十六批：两个流水线17998/39973已退出零。Windows/Linux实际打包库Flutter385/384项、Rust224/197项；18个绑定稳定、275个源文件摘要一致（build/account-ui-sources.json）。正式桌面账户接线已完成本地验收。

第二十七批历史边界：当时只有原生更新基础及Windows局部验证，正式更新仍走Dart、公告未验收；已由第二十八批推进，不按该历史段判断当前源码。第二十八批Windows首轮沙箱6项命名管道超时在沙箱外全量重跑通过（55767退出零）。Linux首轮analyze因旧依赖登记缺失而失败，按锁文件pub get后通过。相关日志在两端build/b3-*，不要把被修正的首轮结果冒充通过结果。真实FFI使用各自本批新库；新公告HTTPS成功为本地fixture，未证明真实站点/校园运行。

## 1. 接手时的结论

完整迁移尚未完成。第十七批 GeoIP/GeoSite 是最近一次完成整批验收的批次；第十八批 Windows TUN 正在实现。续作已有原生出口、路由事务、捕获、低层/IPC 探针及随包 helper。最新 --helper 实际请求了 Windows UAC，但返回取消（1223），未进入网卡创建；不自动重复该请求。真实网卡、路由与高层数据面仍未证明。

随后推进第十九批 B1：Rust NewssoSession 已实现密码/两步验证、Cookie 和退出代次，生成 FRB 并通过两端真实 FFI 状态测试与 HTTPS fixture。正式前端仍执行旧 Dart 登录/账户服务；持久化跨重启迁移、QR、业务授权共享和旧代码删除尚未完成。最新回归为 Windows 169 Rust / 375 Flutter、Linux 140 Rust / 374 Flutter，16 绑定文件重复生成稳定。

第二十批已新增 Rust 凭据文件、open/close、跨重启导入/注销标记，Windows DPAPI 与 Linux 私有权限，四进程恢复探针通过。最新回归 Windows 181 Rust / 377 Flutter、Linux 154 Rust / 376 Flutter。正式前端没有接 persistent open 或删除旧键，QR/业务授权共享等仍未完成；请区分第十九批历史内存限制与第二十批的新 native 能力。

第二十一批进一步实现同一 NewssoSession 的企微 QR 创建/图片/轮询/换码，授权码不进入 FFI DTO，成功要求实际 SSO Cookie。已复现并修复 wait 超时后独立 poll 迟到覆盖 Expired 的代次竞态。正式扫码页和共享业务账户 owner 尚未切换；单独 QR 成功不等于账户迁移完成。接手优先补业务授权共享，再整体验证/切换账户 UI 与旧后端删除，不反复请求已取消的 Windows UAC。

最初交接只整理状态。用户随后要求持续推进，当前验证结果见实施记录中的“第十八批续作”；下文中断现场保留为历史证据，并注明已处理项。

最新第二十五/二十六批已落盘 Rust AccountSession/version2 原子旧数据采纳和桌面正式接线。Windows/Linux bootstrap 打开 native owner后清理旧三个键，Provider/扫码/核对/设备号展示/连接走同一账户；新连接不透明计划。Windows 最终 224 Rust/385 Debug Flutter、Clippy/analyze、两套四进程探针通过；正式 Windows release 流水线 17998 与 Linux 全量至 release/FFI 流水线 39973 正在执行，下一步先收取现有句柄结果，不重启。binding/source final清单尚待更新。当前 version1 文件读兼容、version2 设备/展示字段已实现；失败注销和 post-replace sync 错误契约仍保持，不能称所有写失败都保持旧磁盘。生产发布尚未验收，不能只按前面第24批“未接线”历史段判断源码。Android/B3/B4/旧类删除与完整平台目标均未完成。

第二十四批继续完成同会话 prepareTunnel、PreparedATrustTunnel 不透明计划和 connectPrepared，sid/签名密钥/计划不作为新路径的 Dart 值传递。计划一次使用并绑定账号代次，已连接的 native 传输也随账号注销/close 显式停止。verifyAtrust 实际建立和关闭原生 L3 控制后返回 VIP/DNS/warnings，用于替换旧 credential_service 的 SDK 探针。Windows 211 Rust / 381 Flutter、Linux 184 Rust / 380 Flutter通过；本批为 Debug 原生库 FFI/存储验证，16 绑定稳定、262 源文件匹配，没有重建 release bundle，上一完整发布验证仍为第二十三批。下一步优先做 Rust 账户 owner/稳定设备号/展示快照的原子迁移和生产接线，再一起验证发布产物。现有 Store 的 version1 兼容、失败注销和 marker 契约不能丢；升级必须冻结旧 JSON fixture，不能仅改动态 Record 构造器。完整目标尚未完成。

第二十三批进一步完成 native OTP 实际取码/重试和教务档案/课表身份读取，新增 fetchOtp/fetchJwxtAccount。请求共用账号 owner，失败资料及重试回滚其 Cookie，退出不采纳迟到数字码/身份；缺失学期不发空表单，资料失败不撤销授权。最终 Windows 204 Rust / 380 Flutter、Linux 177 Rust / 379 Flutter，发布/真实 FFI/四进程存储通过，16 绑定稳定、259 个源文件摘要一致。正式账户和扫码页仍在 Dart，下一步接同一 NewssoSession 的连接准备，避免 Dart Seed/Cookie 往返，再迁移稳定设备号/账户快照与生产 factory/UI、删除旧 owner。完整目标保持；Android 当前也没有 Rust .so 打包及 VPN fd 接线，工具链准备证据见审计。

第二十二批继续 B2：同一 NewssoSession 已实现教务/OTP OAuth 与原生 aTrust 换票/环境上报/onlineInfo，aTrust 请求也直接使用账号 jar，避免父域 Cookie 被两套旧容器合并回滚。授权回执不输出码、SID、ticket 或 HTML。最新两端发布/真实 FFI/存储探针通过，Windows 195 Rust / 379 Flutter、Linux 168 Rust / 378 Flutter，16 个绑定稳定、255 个源文件摘要一致。OTP 数字码、教务身份/资料及正式账户接线仍待完成；新的授权落点不能直接标为业务凭据可用。Linux 首轮错误加载旧默认库的测试已排除，显式 bundle 重跑通过，详见第二十二批实施记录。

最终目标以 [工程计划](desktop_migration_rust_core_plan.md) 为准：保留 Flutter 前端，Windows、Linux、Android 的后端业务全部迁入 Rust，最终删除已替代的 Dart 后端和旧 SDK 后台调用链。桌面 VPN 主链路接入 Rust 不代表整体完成。账号、Cookie、完整 SSO、业务授权、更新公告、配置和持久化的残留入口见 [后端迁移审计](backend_rust_migration_audit.md)。

历史实现和逐批证据见 [实施记录](desktop_migration_progress.md)。其前部验收表存在尚未随后续批次更新的描述，判断最新能力时需同时阅读对应批次正文；不要仅按旧表认定 GeoIP、系统代理等仍完全未实施。

## 2. 仓库与现场

- 当前分支：`refactor/architecture-cleanup`。
- HEAD / 基线：`1e055d6`，原版本 0.4.2。
- 大量修改和新增文件尚未提交，包括整个 `rust/`、`windows/`、`linux/`、`tool/`、`docs/` 及部分 Flutter 接线。`git diff` 不会展示未跟踪文件内容，必须结合 `git status --short` 和直接读取。
- 不要 reset、clean 或覆盖未提交产物。本轮没有创建提交。
- 若后续创建提交，作者和提交者必须为 `Cans518 <798061533@qq.com>`，先纠正仓库本地身份覆盖。
- 续作普通测试进程没有管理员权限。历史阶段未触发 UAC；最新 helper TUN 例程实际请求 UAC 后返回取消 1223，取消前后网卡/路由快照一致，没有执行 Wintun 创建成功测试。
- 原交接收尾时没有发现名为 cargo、rustc、flutter、dart 的运行进程。这只是历史快照；续作启动了新验证进程，应按实际执行句柄判断状态。

用户提供的 AGENTS.md 要求主代理亲自阅读基础文档和即将修改的代码；独立调查可尽早派发只读子代理。小范围文档整理可以直接处理。修改代码时应用 clean-code 技能；小规模手工编辑优先 `apply_patch`，避免 PowerShell 对 JS/TS 模板文本做临时字符串替换。

## 3. 最近完整验证的基线：第十七批

此前已经实现并在对应批次验证：

- Rust 工作区、FRB、Windows/Linux Cargo 联合构建、正式入口和真实原生库调用。
- 桌面 aTrust OAuth、换票、环境上报、认证链、资源准备、anti-MITM、TLS L3 控制和签名 TCP 隧道；并非完整应用登录业务已迁完。
- HTTP/CONNECT/SOCKS5 的实际 DIRECT、REJECT、SHU-VPN 出口；规则决策与校园资源授权分别校验，失败不会隐式回落直连。
- 规则编译、策略选择、配置预览/提交/回滚、规则集定时与手动更新、域名/IP/端口/入站、AND/OR/NOT、DOMAIN-REGEX 和受限预算。
- 校园 TCP DNS、TTL 缓存与域名身份；Linux TUN/Netlink、TCP/UDP 数据面、物理出口保护、DNS 动态主机路由及清理。
- 桌面窗口/托盘生命周期，Windows WinINet、GNOME/dconf、KDE 5/6 系统代理及崩溃恢复。
- 固定版本和 SHA-256 的 V2Fly protobuf GeoIP/GeoSite 文件/HTTPS 数据源、索引、派生缓存、GEOIP/GEOSITE 规则及界面能力查询。

第十七批最终证据：

| 项目 | 已记录结果 |
| --- | --- |
| Rust 全量 | Windows 123 项、Linux 126 项通过 |
| Flutter / 真实 FFI | Windows、Linux 各 372 项通过 |
| 静态检查 | 两端 analyze 无问题，all-targets/all-features Clippy 无警告 |
| 构建 | 两端 x64 release，入口 `lib/main.dart` |
| 数据与实际出口 | 独立 protobuf fixture、真实 HTTPS、本地实际代理字节及打包库 FFI |
| 生成稳定性 | FRB 重复生成，Rust/Dart 文件 SHA-256 不变 |

日志目录为 `C:/Users/xiema/.cache/shuvpn/`，相关文件包括 `geo-windows-rust.log`、`geo-windows-clippy.log`、`geo-windows-build.log`、`geo-windows-flutter.log`、`geo-linux-final.log`。

这些是本机固定 fixture 和隔离测试结果，不证明远端 CI、社区数据集准确性或真实校园长期运行已经通过。续作已重新构建两端正式产物并通过回归，最新打包库包含第十八批低层 Windows TUN 和驱动状态绑定；第十七批仍是最近完成整批验收的批次。

## 4. 第十八批在途实现：Windows TUN

### 已落盘的代码

| 位置 | 当前改动 |
| --- | --- |
| `rust/Cargo.toml`、`Cargo.lock` | Windows 直接依赖 windows-sys 精确版本 0.61.2，并启用 IP Helper、WinSock、Security、加载器和线程等特性 |
| `windows/third_party/wintun/` | 官方 Wintun 0.14.1 amd64 DLL、许可证和来源说明 |
| `windows/CMakeLists.txt` | 安装 Wintun DLL 到应用目录，许可证安装到 `data/licenses/wintun/`；续作已验证 release 打包、DLL 固定摘要、许可证字节一致及无路径覆盖的真实 FFI 加载 |
| `rust/src/platform/windows_tun/api.rs` | Wintun C API 动态加载、固定摘要检查、模块和文件句柄生命周期 |
| `rust/src/platform/windows_tun/ip.rs` | IP Helper 地址/路由校验、MTU/接口设置、独占地址和分流路由安装与撤销 |
| `rust/src/platform/windows_tun/mod.rs` | 网卡和 ring session 生命周期、读写、取消、操作租约、关闭等待和驱动状态查询 |
| `rust/src/platform/model.rs`、`linux_tun.rs` | TunInfo 扩展 Windows，包合法性校验抽成共享函数 |
| `rust/src/platform/mod.rs` | 注册 Windows 平台模块 |
| `rust/src/api/tun.rs` | 低层 TunInterface 支持 Windows；析构请求取消；新增 `tun_driver_info` |

驱动固定 SHA-256：

```text
E5DA8447DC2C320EDC0FC52FA01885C103DE8C118481F683643CACC3220DAFCE
```

来源和签名检查已记录在 `windows/third_party/wintun/README.md`：官方 0.14.1 压缩包，Authenticode 状态 Valid，签名者 WireGuard LLC。缓存位于 `C:/Users/xiema/.cache/shuvpn/wintun/`，包含头文件和 IP Helper 官方资料。API 参考固定 Wintun 提交为 `bfef136abfa1665c2592be09a7e383d646cdbe6e`。

加载器仅接受固定摘要的 DLL，默认从应用/原生模块目录加载，也接受绝对路径环境变量 `SHUVPN_WINTUN_DLL`。目前安装路径只覆盖 amd64，不构成 arm64 支持。

### 中断后已收取的实际结果

执行句柄 `81641` 已结束，退出码 1，无需继续轮询或按“仍在运行”处理。该命令依次执行 Rust format、Windows TUN 定向测试和全目标全特性 Clippy：

```powershell
cargo fmt --manifest-path rust/Cargo.toml
cargo test --manifest-path rust/Cargo.toml --locked platform::windows_tun --lib
cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets --all-features -- -D warnings
```

四项测试均通过：

1. 固定摘要的官方驱动和导出函数加载，不创建网卡。
2. IP Helper 行中的 LUID、前缀和 on-link next hop 编码。
3. 操作租约阻止提前释放；取消关闭 future 后可以再次关闭。
4. 非管理员创建请求在系统修改前返回权限错误。

Clippy 随后失败，全部是 `clippy::field_reassign_with_default`，位置均在 `rust/src/platform/windows_tun/ip.rs`：

| 报错位置 | 涉及结构 |
| --- | --- |
| 105 行 | `MIB_IFROW` 的 dwIndex / dwAdminStatus 初始化 |
| 198 行 | `SOCKADDR_IN` 的 sin_family 初始化 |
| 204 行 | `SOCKADDR_IN6` 的 sin6_family 初始化 |
| 255 行 | `MIB_IF_ROW2` 的 InterfaceLuid 初始化 |

续作已亲自阅读并修复这四处结构体初始化。Windows 四项定向测试、全量 127 项 Rust 测试及 all-targets/all-features Clippy 均通过；新绑定生成后也重新运行了格式和 Clippy 检查。以上仍不证明特权网卡成功路径。

### 尚未接通或验证的部分

- 新增 `tun_driver_info` 已生成 FRB 绑定，重复生成的 14 个文件 SHA-256 一致；两端真实打包库调用通过，Windows 返回固定驱动版本/摘要、权限及未就绪状态，Linux 返回 native-linux。
- `rust/src/api/vpn.rs` 的 `VpnSession.start_tun` 仍只支持 Linux；Windows 高层会话 TUN 数据面未接入。
- 包桥接/高层 TUN 服务仍有 Linux 专用边界；capture worker 已依赖 RouteCapture，共享 Linux/Windows 适配。Windows 动态路由新增/删除/恢复已实现并通过故障注入；真实特权成功路径未验证。原生出口已有本机证据，但没有 Wintun 捕获路由上的防回环证明，也不覆盖 Dart 请求；组播/广播、作用域 DNS 和完整捕获仍未支持。
- 低层读写的资源租约已有测试，但尚无真实 Wintun ring 收发、实际网卡、路由、包字节或特权清理验收。
- `Configuration.close` 已保留失败记录供后续 close 重试，Device/资源共享配置，确定性失败/部分成功/重试测试通过；尚无真实特权删除失败、初始化恢复或崩溃恢复证明。
- CloseAdapter 返回 void，必须以实际接口消失等证据验证清理；仅调用关闭函数不够。
- ReadWaitEvent 是借用句柄，不应主动 CloseHandle；EndSession 必须等持有资源的操作结束后执行。
- IPv4 SetIpInterfaceEntry 的 SitePrefixLength 要为 0；地址/路由结构必须按官方初始化流程设置。DAD 的真实系统行为仍待验证。
- 当前驱动状态明确标识 `sessionDataPlaneReady=false`，不要提前展示 Windows TUN 已可用。
- 续作已完成两端全量 Rust/Flutter、Clippy、analyze、release、真实打包库 FFI 及绑定重复生成验证；Linux 当前 release/examples 的隔离内核包、DNS 动态路由和清理探针也全部通过。Windows 特权路径、高层会话接入和完整第十八批验收仍未完成。

## 5. 推荐接手顺序

1. 阅读主计划、后端迁移审计、本交接及第十七批记录，确认完整目标；保留现有未提交文件。
2. 清理失败记录保留与重试代码已实施；补实际特权删除失败、初始化失败、接口撤销与崩溃恢复证据。检查未跟踪源码，不能只看 diff。
3. `tun_driver_info` 绑定、重复生成、analyze 和真实桥接驱动/权限状态已验证。开发 Cargo 库需显式驱动绝对路径，正式 bundle 验证清除环境覆盖。
4. 动态路由事务和共享捕获接口已实施。接入高层前完成特权探针、helper/IPC、Windows Service 字段/初始化及 driver stop/join，补网络切换/失败恢复；审计域名/映射 IP 与真实运输 IP 的排除一致性，并验证控制/认证/DNS/规则下载在实际捕获路由下仍走物理接口。IPv6、组播及作用域 DNS 等剩余范围继续保留。
5. 在管理员环境执行 windows_tun_smoke 低层探针；helper/受限 IPC/runas 入口现已落盘并随包，但仍需验证真实 UAC 同意/取消、helper 持有 TUN 的断开/父进程退出、原生停滞与失败恢复，再接入正式高层。当前非管理员现场只通过权限拒绝/前后网络快照及无网卡 IPC 证据。探针只操作随机名称测试接口与文档保留地址路由，必须输出完整成功标记才可验收；命令、范围、预算和限制见 CONTRIBUTING.md。
6. Linux 独立快照已同步并通过两端常规回归；以后修改出口保护、动态路由或数据面后，重新运行相应平台及隔离内核验证。
7. 证据齐备后再向实施记录追加第十八批，分别记录通过项和残余限制。

后续 B4 的只读定位已完成并抽查：设置不是只迁移 schema 类。应一起迁入 settings 命名空间、v0–v4 迁移、默认值/兼容解码、输入校验、写入完成/失败及恢复连接默认。现有 `settings_schema.dart:71–94` 保持新装 `from=0` 不自动完成引导、v1/v2 完成引导、未来版本不降级；`settings_store.dart:332–339` 的恢复只删服务器/登录域/超时/协议键。普通 setter 当前 unawaited 写后立即通知，不能将其直接当作 native 已提交快照。`preferences` 还被 auth/设备号/TOFU 使用，尤其 `connection_controller.dart:575–613`，不能整库清空。Android/debug 相关默认值需由平台输入传给 Rust；现有9项 schema测试需冻结为 native 升级向量，最终正式接线与清除旧 settings writer 一起验收。后续修改前主代理须完整读即将修改的 Store/schema 与消费代码。

上述设置定位为第二十九批前的历史入口，桌面当前已用native owner；旧Store保留于Android/测试，连接权威状态和最终旧类删除仍未完成。Android续作环境也已重新定位：工程声明AGP9.1.0/Kotlin2.4.0/Gradle9.3.1，源码/字节码目标Java17；当前Flutter3.47.2的FlutterExtension.kt给出compile/target SDK36、min SDK24、NDK28.2.13676358。JAVA_HOME/ANDROID_HOME/ANDROID_SDK_ROOT在进程/用户/机器三层均未设置，PATH无java/javac/adb/sdkmanager；已查常规目录未发现JDK/SDK/NDK（未扫描全盘自定义目录），Rust仅安装Windows target。rust_builder仍仅声明Windows/Linux FFI，工程尚无Android .so/fd接线。不能以Java字节码17推断AGP运行时JDK要求，后续搭建时还需核对固定版本兼容性。

普通权限下已推进 B1 基础及 native 持久化。后续接 QR 与共用此会话的业务 OAuth/aTrust/OTP/教务，改造同步 auth factory/目录传递，替换正式前端后删除旧 owner 与偏好。内存 newInstance 仍无跨重启 marker；open(directory) 才恢复持久化并抑制旧数据，clear 写失败会返回错误，close 仅保留已提交记录并释放锁。旧 Dart 五处 persistSession 尚在写旧键，不能孤立删除一次便宣称接管。详见第十九/二十批与后端审计。Windows 已取消的 UAC 不自动重试。

后续仍必须完成：完整账号/SSO/Cookie/凭据与业务服务迁移、Dart 后端和 SDK 后备调用删除、Android Rust 接入、OS DNS、桌面 TUN 控制器与界面、进程身份规则、多配置档和更完整规则/连接管理、完整捕获、安装分发、远端 CI、真实校园和长期恢复验收。具体以主计划和审计为准，不用 Windows TUN 单批成果替代整个目标。

## 6. 工具、构建与隔离环境

Windows 工具：

- Flutter 3.47.2 / Dart 3.13.2：`C:/Users/xiema/.cache/shuvpn/flutter/bin/`。
- Cargo / rustc 1.96.0，VS 2022 / CMake。
- FRB 运行库与生成器必须同为精确版本 2.11.1。
- Python：`D:/miniconda3/python.exe`；WindowsApps 的 Python 别名不可作为可用解释器。

FRB 生成：

```powershell
$env:PATH = 'C:/Users/xiema/.cache/shuvpn/flutter/bin;C:/Windows/System32/WindowsPowerShell/v1.0;' + $env:PATH
./tool/generate_rust_bridge.ps1
```

Flutter 正式构建始终使用 `lib/main.dart`。真实 FFI 必须使用当批构建的打包库，不能误测旧库：

```powershell
$env:FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR = 'D:/Desktop/ShuVPN/build/windows/x64/runner/Release'
```

Linux 使用 WSL Ubuntu-22.04，用户 mzee，Rust 1.92.0，Flutter 位于 `/home/mzee/.cache/shuvpn/flutter/bin`。独立源码快照为 `/home/mzee/.cache/shuvpn/linux-worktree`；续作已同步第十八批源码与新绑定，并补同步了最新 Linux 驱动状态测试。

Linux PATH：

```text
/home/mzee/.cache/shuvpn/flutter/bin:/home/mzee/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
```

快照同步沿用以下排除项，避免覆盖构建缓存或带入 Windows ephemeral：

```powershell
wsl -d Ubuntu-22.04 -- bash -lc 'set -e; set -o pipefail; tar -C /mnt/d/Desktop/ShuVPN --exclude=.git --exclude=build --exclude=.dart_tool --exclude=rust/target --exclude=windows/flutter/ephemeral --exclude=linux/flutter/ephemeral --exclude=.flutter-plugins-dependencies --exclude=.idea --exclude=*.iml -cf - . | tar -C /home/mzee/.cache/shuvpn/linux-worktree -xf -'
```

Linux FFI 库目录为 `/home/mzee/.cache/shuvpn/linux-worktree/build/linux/x64/release/bundle/lib`。特权 TUN 探针在隔离 network namespace 内执行，不修改真实用户网络。root 跑纯 Dart 探针时使用实际 SDK 可执行文件 `/home/mzee/.cache/shuvpn/flutter/bin/cache/dart-sdk/bin/dart`，避免 Flutter wrapper 的 Git ownership 问题。

已有探针入口包括 `tool/tun_linux_namespace_smoke.sh`、`tun_data_plane_linux_smoke.py`、`tun_dns_linux_smoke.sh`、`tun_bridge_smoke.dart`、`tun_dns_bridge_smoke.dart`、`rust_bridge_smoke.dart` 和各平台 `system_proxy_*_smoke`。执行前读取脚本所需环境，避免凭名称猜测参数。

Windows 新增 `cargo run --manifest-path rust/Cargo.toml --locked --release --features egress-probe --example windows_egress_smoke -- <接口别名> <该接口IPv4地址>`。地址必须先从系统读取，不猜测固定网段；探针只建立自有 socket fixture，不创建网卡或路由。生成绑定仍为 14 文件 SHA-256 稳定；当前常规回归为 Windows 147 Rust / 373 Flutter、Linux 130 Rust / 372 Flutter。事务故障注入和通用策略 sink 测试不能作为真实 Windows 路由验收证据。

最新低层 TUN 探针续作的回归为 Windows 148 Rust / 373 Flutter、Linux 130 Rust；两端全特性 Clippy、Windows 正式 bundle/FFI 和 14 文件绑定稳定性通过。Windows 可选 `tun-probe` feature 的 windows_tun_smoke release 例程已构建；设置官方 Wintun 绝对路径后在管理员终端运行，无参数、不会自行触发 UAC。成功标记为 WINDOWS_TUN_DUAL_STACK_RING_DYNAMIC_ROUTES_AND_CLEANUP_PASSED；watchdog 超时退出 124 且清理未证明。当前现场只通过非管理员拒绝路径，没有创建 TUN，也没有新增 Linux Flutter bundle 验收结果。详情及日志见实施记录“第十八批续作（四）”。

最新 helper 续作回归为 Windows 158 Rust / 373 Flutter、Linux 130 Rust，两端全特性 Clippy 和 14 文件绑定稳定性通过。正式 Windows bundle 已携带 shuvpn_tun_helper.exe，摘要与 Cargo 产物一致，随包 exe 的三项真实进程集成通过。Debug 原生 CMake 目标的空参数问题已修复并构建通过，不等于整个 Flutter Debug bundle 已验收。生产 Endpoint.launch 只接受同目录 helper，通过 Windows runas 进行用户同意的提升；本轮未调用 UAC。当前 VpnSession.start_tun 与 TunInterface 仍未改用 helper，完整 Windows 高层保持未就绪；详情见“第十八批续作（五）”。

最新 helper TUN 续作为 Windows 159 Rust / 373 Flutter、Linux 130 Rust，两端全特性 Clippy 与 Windows 正式 bundle/FFI 通过。真实 GetIfEntry2 查询复现并修复了缺失接口返回 2 却被当作撤销失败的错误。例程新增 --helper / --helper-disconnect / --helper-inherit，命令见 CONTRIBUTING.md；普通继承模式拒绝通过，实际 UAC 请求返回取消 1223、退出 1，前后系统快照一致。特权成功路径仍未运行，不反复重试取消；普通权限的账号/SSO/Cookie 等迁移仍可继续。详情见“第十八批续作（六）”。

KDE 6 隔离 rootfs 在 `/home/mzee/.cache/shuvpn/kde6-rootfs`，用于 KF6/Qt6 实际消费者测试。此前代理测试已恢复 Windows 原配置；Linux KDE/GNOME 测试使用独立配置和 DBus 环境。

## 7. 已踩过的坑

- Dart VM 会通过全局 wait 回收未注册的原生直接子进程。KDE 工具调用已使用固定 shell supervisor 和随机状态尾标，不要恢复成直接依赖子进程退出码的旧实现。
- fork 子进程短暂继承同一文件描述可能延长系统代理租约锁；租约 Drop 已显式 unlock，有确定性回归证据。
- 子进程在对话中断后可能仍运行。先收取已知句柄或检查现场，再决定是否重启；本次 `81641` 已结束。
- 纯 Dart 命令不能直接导入依赖 `dart:ui` 的 Flutter 模块。真实 FFI 和 Flutter 测试需分别使用合适入口。
- 不用纯 Dart 替身测试代替真实打包库 FFI，不用 Windows 构建代替 Linux 验证，不用本机测试代替 GitHub CI。
- 不为每批通过就宣称总目标完成。进度记录应明确实际范围、证据和剩余项。
