# 后端零 Dart 迁移审计

2026-10-17 第六十四批：建立108个生产Dart文件的逐文件角色/规范化SHA/例外清单，覆盖31个生成文件和77个手写文件；运行依赖11项及pubspec/锁文件同时封存。非生成core36文件全文复核，features/shell/widgets30文件枚举、搜索并核验关键候选，主线程复核app/main装配与展示、原生落点及生成物。未发现确证的独立Dart后台网络/凭据配置数据库/业务重试或旧SDK后台回落；分页展示、原生cache snapshot、提交后显式用户命令、快照计时和UI平台能力均有边界判定。详见[dart_backend_boundary_review.md](dart_backend_boundary_review.md)及清单。

固定FRB2.11.1重生成的31个Dart文件及Rust绑定逐字节一致。新增门禁检查完整覆盖、内容、依赖/锁、条件imports、平台IO例外与跨入测试后端；发现并用红/绿回归修正library metadata提前结束扫描及注释伪装JS缺失分支两个缺口。该门禁不是Dart完整语义证明，不能用“清单通过”替代人工复核/Flutter分析/真实FFI/平台验收，更不能据此标记整个M5.4或最终交付完成。Windows特权Full、完整GUI/真实校园/长期/远端CI继续保留。

2026-10-17 第六十批：59批Rust进程归属、未知身份拒绝及捕获代次修复已进入完整桌面与Inno/DEB候选ccb6beba4cba。没有新增生产实现；新归档304/303 Flutter和实际安装/跨进程存储通过，最终DEB安装库100轮内核进程规则放行/拒绝和退出屏障通过，每轮校验.so/helper摘要。无新的完整GUI/校园/长期/Windows特权或远端CI签收；完整后端零Dart及交付仍未最终完成。当前来源、摘要和限制以候选verification.json及交接为准。

2026-10-17 第五十九批：进程归属查询、工作槽/排队/预算、平台表解析、规则编译/匹配和未知身份拒绝全在Rust；Dart仅收集模拟输入、展示能力与已给出的PID/路径/置信依据，诊断工具执行原生命令和内核fixture。修复捕获actor等待期间旧代次误拒绝新DNS答案。最终357/339 Rust、304/303 Flutter及两端默认库、Root/普通helper真实进程规则内核路径通过，具体来源与限制见process-rules/verification.json。新分发归档尚未构建，旧候选不含该能力；全仓零Dart和完整交付仍未最终签收。

2026-10-17 第五十八批：第57批Rust helper修复已重建进成对完整桌面与Inno/DEB候选1f1a2715c619。没有新增生产代码改动；来源与已验7文件一致，实际归档/安装库通过300/299 Flutter、桥接与跨进程存储；新DEB实际安装库100轮TUN/FFI及账户保留通过，EXE实际安装/升级/卸载和账户保留通过。当前证据见候选verification.json和交接。GUI仅Linux欢迎/加载/退出，既有警告保留；无新的完整GUI/校园/长期、Windows特权、远端CI或发布验收。全仓零Dart和完整交付仍未最终签收。

2026-10-17 第五十七批：Linux helper退出确认竞态修复留在Rust平台适配器。pre_exec自取pidfd、进程退出/强杀确认、异步注册失败守卫及有界收尾均由Rust持有；Dart仅在诊断工具增加失败阶段和原生快照输出，未新增生产后台实现。341/323 Rust、Linux299 Flutter及100轮真实FFI/helper清理通过，来源与证据见helper-pidfd-verification.json和交接。新分发包尚未构建；本批不改变全仓零Dart及完整交付仍待最终签收的状态。

2026-10-17 第五十六批：补齐本地Windows Inno与Linux DEB安装格式，更新资产选择按原生安装标记决定格式，仍无Dart文件/网络策略。Windows普通权限安装/升级/卸载、双向运行标记与原子安装事务、1618启动拒绝及真实账户保留通过；DEB授权/拒绝/失败重试/升级/remove/purge/残留重装、实际helper数据面和已安装欢迎页/退出通过。新提取路径的安全缺口已红/绿验证修正。340/319 Rust、300/299 Flutter、65/64解压FFI通过，候选c865f4db8667。

安装格式不等于完整交付：Windows EXE未签项目发布证书，远端CI/发布未验；Linux首轮TUN cleanup有一次未定位失败，诊断改进后的7轮通过不能冒充已修复根因，继续保留压力复核。已安装GUI仅欢迎/加载/退出，既有依赖关闭警告保留。全项目最终审计、进程身份、完整平台/GUI/校园/长期仍未签收。

2026-10-17 第五十五批：密码/MFA成功后的自动凭据阶段进入原生sign_in_password/sign_in_code，一个operation锁贯穿身份与凭据工作，真实用户challenge处才返回。零可用凭据是明确失败并保留SSO。Dart正式接口删除completeSignIn和凭据过渡页，只展示native进度/结果；表单reset/dispose发送取消、展示代次过滤迟到结果，适配器关闭后拒绝迟到成功。原过渡页的行为断言已移至7项表单生命周期测试，没有以删除失败用例完成迁移。339/318 Rust、300/299 Flutter、Clippy/analyze/release、65/64实际解压库FFI及桥接/三套存储探针通过；候选ffc946346612。

本轮限定审查的边界判断：native提交后经Flutter插件删除旧SharedPreferences键是既定一次性迁移桥接；页面挂载/滚动请求和订阅取消、DI装配、单条native恢复命令不等于独立后台算法。账户过期/交换和公告预取策略已在Rust，未因关键词async/await继续制造迁移项。仍需完成M5.4逐文件/依赖/正式运行证据的最终签收。本批公开二维码取消不代表真实密码/MFA或校园验收，未重跑平台特权/完整GUI。

主线程已完整重读工程计划。确认的独立剩余项包括M2.6进程身份规则（当前明确unsupported，不能以此当作已实现）、M5.2正式Windows/Linux安装格式与远端CI/发布，以及完整捕获/平台恢复/校园/长期验收。Geo数据、逻辑与受限正则有实现和测试，不能统称“所有高级规则未实现”。完整交付仍未完成。

2026-10-17 第五十四批：扫码创建→等待→兑换→获取系统凭据合并为原生sign_in_qr操作，Dart删除四阶段组合和扫码成功后的重复completeSignIn，只映射图片/阶段/失败事件与展示快照。取消依据已提交的SSO状态保留或清理登录；最终成功/不可用事件在账户操作/状态锁内发送。完成页面退场不再发送全局取消，红/绿回归覆盖。真实FFI发现StreamSink方法返回Err会进入生成器unawaited Future，QR改为typed Failed事件，公告批次用sink.add_error，diagnostics接收端关闭正常结束；其他生产流已专项核验，未修改FRB依赖。

336/315 Rust全量、两端Clippy/analyze/release、账户76项及UI/adapter21项目标测试通过。最终应用Windows293/Linux292 Flutter全量、解压库64/63项FFI、桥接与三套存储探针通过；两端公开二维码图片实际跨桥接后立即取消且未登录/无活动二维码，不是成功扫码或校园凭据验收。候选64e4acc0c701，413来源文件SHA64e4acc0c701e09bc65ea57221285efac57fbb6efc220c3e060551592af15533，完整性与来源校验通过，清单仍partial-desktop-candidate。本批没有平台特权或完整GUI新验收。

剩余可定位边界：密码直接成功与验证码验证成功仍由login_form自动导航到ShuCredentialGate并调用completeSignIn。发送验证码、方式选择、输入验证码属于用户动作，不能与这些自动组合混为一谈；表单关闭的取消需结合宿主进一步检查。整个零Dart、完整平台/GUI/校园/长期/原计划能力/正式分发/CI验收仍未完成。

2026-10-17 第五十三批：公告预取的 worker、游标、站点过滤、共享并发上限、缓存命中与同 URL 请求合并已进入 Rust；页面只消费预览事件、取消批次和防止旧展示结果覆盖新列表。空列表及直接自引用分页在原生终止。原生详情的 10 秒总预算包含同 URL 队列等待；初版虚拟时钟测试立即检查任务完成存在调度竞态，改为有界等待，不放宽生产超时。最终两端各21项公告测试、331/310 Rust全量、Clippy/release、62/61解压FFI通过；282/281 Flutter全量与analyze在最后Rust-only deadline修复前通过，最终库另行验证。候选e8e936136bb2通过归档完整性、桥接及三套跨进程存储；两端实际公开站点10条列表/3篇详情和原生缓存通过。没有本批平台特权/完整GUI新验收，状态仍partial-desktop-candidate。

下一边界已定位并抽查：`wecom_scan_page.dart` 在 start 成功后自动 wait、确认后 redeem；`login_form.dart` 收到路由 true 后自动 completeSignIn。HTTP、扫码轮询/超时、单次码消费、取消代次、Cookie 提交及系统凭据交换已经在 Rust，但页面仍决定阶段组合。已有独立扫码和凭据页面测试不能替代整条组合测试；须由主线程阅读具体实现后决定如何收敛。此批未修改扫码流程，全仓零 Dart 与完整交付仍未签收。

2026-10-16 永久范围修订：**项目仅支持 Windows 与 Linux，不再开发或移植 Android。**
Android 工程、专用运行入口、原生适配与构建/发布工具已移除；下文 Android 内容只保留为历史记录，不属于待办或后续验收。
本次删除改变原生桥接接口，旧候选归档不代表当前源码，必须重新构建配套库、绑定与完整桌面包。

2026-10-17 第五十二批：持久Rust连接owner已接入正式bootstrap/backend。会话登记、替换前清理、失败保留、取消/丢弃等待、退出屏障、前端控制代次和registry释放均在Rust；Dart旧待清理列表/排空循环/_discard删除，重绑也成为原生命令。323/302 Rust、274/273 Flutter、Clippy/analyze/release、61/60实际包FFI及Linux owner安装/TUN/系统代理退出链路通过。候选ed998163d149，仍非完整交付。core有界复核未发现独立后台网络/存储/重试；页面公告预取worker、分页终止以及自动扫码阶段组合需进一步做边界判定/收敛。页面保存后派发native命令并不自动等于Dart资源所有权，特别是其运行分支受到控件锁定。依赖最小对照已复现关闭警告，不以SDK源码推断替代运行证据；没有消音或修改依赖。最终全仓零Dart、原计划能力、Windows/GUI/校园/长期/分发/CI仍待完成。

2026-10-16 第五十一批当前更新：正式桌面attempt的恢复及规则挂接进入connect_desktop_account，规则在Engine创建时注入；新增原生desktop_services统一自动HTTP/系统代理/SOCKS/TUN启动和实际状态报告，TUN权限与失败回收也进入Rust。Dart旧自动顺序及失败后二次stop删除。原生4项真实集成、首请求规则、损坏恢复记录FFI及UI状态红回归覆盖新边界；Windows313/Linux292 Rust、272/271 Flutter、Clippy/analyze/release、59/58解压FFI通过。Linux隔离安装/授权helper/新入口内核DNS/TCP及清理、GNOME和KDE5新组合系统代理与恢复通过，实际安装GUI仅欢迎/加载/退出。最终候选0c5456adbf6e与受测c0d2 runtime payload逐项一致，末尾差异仅验证探针源码及包的来源清单。连接采纳/替换、迟到owner及手动重绑等Dart编排仍在；全项目B6、Windows当前特权/安装、完整GUI、校园/长期、原计划其他能力与CI仍未完成。

2026-10-16 第五十批当前更新：网关解析/HTTPS规范化与LAN单播地址查询/同族选择进入Rust，ConnectionController移除dart:io、_serverUri/_localIPv4；原生ProxyBinding提供每监听独立展示端点/诊断。HTTP stop删除重复Dart恢复，原生已有恢复后关闭契约继续负责OS操作；SOCKS失败保留状态，6项停止竞态红回归后通过。Windows309/Linux288 Rust、268/267 Flutter、Clippy/analyze/release、两端新解压包58/57 FFI与存储探针通过，候选f12d997bc8ec。Windows真实双栈端点与Linux隔离dummy网卡连接/关闭已验证，仅本机可达；Linux实际GUI仅启动/无托盘退出。FRB字段变更但内置名称hash未变，不能混旧包。剩余重点为连接启动配置/恢复/路由挂接/失败回收的原生owner，以及原计划所有最终平台/安装/校园/长期/CI验收；B6仍未完成。

2026-10-16 第四十九批当前更新：登录表单的默认验证方式、60秒发送间隔及senderror替代选择进入Rust Account的Delivery；Dart只读取原生方式/剩余秒数并提交选择，不递减、不按错误码决定替代。请求发送前预留，失败/取消/同owner重登录仍保留；不持久化、不替代服务端限流。核验确认QR超时/轮询、凭据总预算、OTP重试已在Rust，旧“前端认证超时/网络重试”待办应细化为本批已迁的发送策略。Windows305/Linux284 Rust、259/258 Flutter、两端Clippy/analyze/release及新解压包58/57 FFI与存储探针通过；候选18b91af88fee。正向MFA为本地HTTPS fixture，真实校园与本批GUI未执行。剩余native服务生命周期/输入/LAN及最终验收继续保留，B6和完整目标仍未完成。

2026-10-16 第四十八批当前更新：旧Dart SettingsStore偏好/schema/defaults/validator和地址parser已删除，设置基类仅展示接口，同步写拒绝，native适配继续拥有真实提交结果。账户迁移原preferences输入保留。默认/地址/端口重开与恢复断言用真实native验证，UI/fake仅验证输入响应与队列；两端250/249 Flutter、settings单元13/13、Clippy/analyze/release及实际GUI对应流程通过。没有新归档。剩余为native启动清理/输入/LAN与页面超时重试策略、最终安装/特权/校园/长期/CI，整个目标未完成。

2026-10-16 第四十七批当前更新：B1/B2旧Dart请求/加密/Cookie/业务解析、ShuAccountCenter及旧快照存储已删除，账户仅展示接口和原生适配；B3旧HTTP/HTML/isolate/版本与资产算法已删除，目标URL由Rust结果提供。三个Flutter SDK、旧依赖及平台插件已退出实际分发包，当前候选0339059010a0。密码/MFA/扫码UI、真实native迁移/恢复及ready-owner关闭竞态分别验证，测试作用域和二进制边界见交接。SettingsStore/schema旧代码和剩余Dart超时/重试/启动清理/LAN策略仍在，B4/B6与整项目标未最终完成；下文已删除模块的实施叙述保留历史，不作为当前待办。

2026-10-16 第四十六批当前更新：ConnectionController中的旧SDK连接、TOFU、DNS/资源路由、HTTP/SOCKS和流量采样，以及六个相应旧生产模块已删除；界面/托盘使用自有ConnectionStatus与ConnectionErrorCode，缺factory直接失败。Rust已有代理/授权/双栈/统计证据，本批补两项HTTP断言并在两端运行10项proxy_session回归。Windows351/Linux350 Flutter、analyze/release/Clippy通过，两端实际GUI对应路径通过，Windows规则v2和托盘退出/原配置恢复已补验收。没有新归档；详情与证据见第四十六批交接。AccountCenter/SettingsStore基类、旧auth/credential、公告/更新及pubspec SDK/插件仍在，native启动/清理/输入规范化/LAN选择仍需收敛，B4/B5/B6尚未最终完成。

2026-10-16 第四十五批：桌面RustConnectionAttempt已改为单次原生connect_account；Rust持有认证准备和L3握手的统一deadline、attempt取消与准备句柄，Dart preparation.timeout已删除。session警告仅作为展示信息返回。真实TLS账户连接、来源限制、兼容警告、资源/控制取消、准备超时与账户保留、虚拟总预算及真实FFI预检通过。Dart仍持有控制器启动服务/teardown编排和LAN显示地址选择，旧SDK协议/代理/路由/计量仍在；本批不等于B4/B5最终完成。成对候选424ec5d01b42及验证范围见交接文档；Linux实际GUI通过对应权限/引导路径，Windows本批GUI因其他窗口遮挡安全失败，安装/特权及校园等后续验收仍未完成。


2026-10-14 本次交付范围更新：用户仅要求 Windows/Linux 两个桌面版，Android 迁移和旧移动端实现删除不作为本次交付前置。桌面生产入口仍须使用 Rust 后端并完成桌面状态、生命周期及网络恢复验收；共享源码和依赖不能仅凭移动端保留而宣称桌面运行期已经执行或已经移除，须以桌面实际调用与打包证据判断。下文跨平台零 Dart 范围作为长期规划保留。

2026-10-12 用户确认：保留 Flutter 前端，整个应用后端业务迁入 Rust。本文记录当前入口及后续验收，不表示已经完成；主计划中的平台、规则、安装包和校园测试范围保持不变。

## 历史接线证据（第25–32批，当前状态以文首及交接记录为准）

Windows/Linux bootstrap 现已改为在应用支持目录打开 Rust AccountSession，经一次 native 提交采纳旧 SSO、设备号和展示快照，然后清除三个旧键并注入 RustAccountCenter。正式桌面密码/MFA/QR/业务核对与连接准备使用该 owner；新连接经不透明计划，不经 Dart Cookie/签名密钥。两端正式 x64 release/打包库已通过本地验证：Windows 224 Rust/385 Flutter、Linux 197 Rust/384 Flutter，两套跨进程存储探针通过。Android 仍执行旧 ShuAuthSession/SDK 分支；更新公告、设置等残余后台也未删除，整个后端零 Dart 尚未完成。本文下列十九至二十四批段落中的“尚未切换”描述为对应批次的历史边界，当前桌面状态以本段及二十五/二十六批为准。

第二十八批已进一步验收桌面Rust公告请求/解析/缓存和RustUpdateClient薄接线，两端新正式x64 release/打包库通过：Windows239 Rust/391 Flutter、Linux212 Rust/390 Flutter，Clippy/analyze及两套四进程存储通过。更新UI仍按原策略仅Android启用；Android仍走GithubReleaseClient/NewitsAnnouncementClient，旧Dart业务/解析与依赖尚未删除，不能标B3最终完成。真实网络成功证据为本地HTTP/HTTPS fixture，FFI证明本批库的边界与适配；没有真实GitHub/学校站点或Android运行证据。

第二十九批进一步把桌面settings命名空间、schema、默认值、校验与持久化接到RustSettingsStore/SettingsSession；旧settings.*只在native提交成功后清理，auth/TOFU/设备号保留。页面等待保存成功后导航/重绑，薄适配按输入顺序发送命令，失败结果仍返回调用方。Android旧Store/schema/校验仍保留，ConnectionController的权威状态与最终删除也未完成；B4整体保持未完成。最终两端正式库结果见第二十九批实施记录。

第三十批已接Android ffiPlugin及Gradle/Cargo原生打包，arm64 release实际APK的45项导出/16KiB/SONAME/Cargo build-id检查通过，API36/x86_64模拟器四个不同进程完成真实FFI/设置账户恢复/注销/排他锁及强杀恢复。发现并修复Android std File.try_lock Unsupported误报竞争，所有原生文件锁统一Android flock入口。两端最新桌面release/实际库及存储回归同样通过。生产Android仍执行旧SDK/Dart分支，fd生命周期/socket protect、高层数据面、正式账户/设置/更新公告切换、ARM硬件与校园验证均未完成，B1–B6最终范围保持。

第三十一批已实现低层TUN描述符生命周期：Java同步交接后关闭原PFD，Rust持有独立dup，以一次性令牌接管；共享Unix包I/O在全部操作租约释放后关闭，服务撤销不依赖Dart消息。API36/x64普通应用与独立UID辅助应用完成真实IPv4/IPv6 UDP收发、未消费超时回收、重复消费拒绝、挂起read取消及第二VPN触发的OS撤销/内核接口消失。此项推进了第三十批遗留的fd生命周期，仍未接高层会话或socket protect；能力明确报告fdIoReady=true、sessionDataPlaneReady/socketProtectionReady=false，生产分支继续保留旧SDK。

第三十二批已完成高层原生session接线和Android按Network的socket保护的本地验收：修复后的完整模拟器探针、默认无probe低层回归、ARM64正式APK与两端正式打包库/全量/存储均通过，详见实施记录。Java安装配置与Rust计划以SHA256绑定，同源packet/capture/清理owner承接TCP/UDP和域名DNS；NDK绑定并核验显式/受保护mark，HTTP复用受保护连接器，JNI撤销只发送取消，Rust拥有并等待关闭任务。Private DNS的DoT/DoQ在校园DNS捕获时明确拒绝，不能DIRECT回落；捕获回复等待的取消缺口已确定性复现并修复。当前源码sessionDataPlaneReady/socketProtectionReady=true，但captureMode=preinstalled-routes、dynamicRouteUpdates/fullCaptureReady=false；超出已安装网段的域名不能假装捕获成功。Android动态route重建、正式前端业务切换与旧SDK/Dart删除均未完成，完整后端零Dart目标保持。

| 迁移批次 | 当前入口 | 最终 Rust 职责 | Dart 最终保留 |
| --- | --- | --- | --- |
| B1 会话与完整登录 | 旧Dart服务已于47批删除；当前`auth_exception.dart`、`login_challenge.dart`及Rust账户适配 | Rust持有会话/Cookie/密码/SSO/挑战/扫码，后续保留真实校园和最终恢复验收 | 输入、扫码/验证码展示、快照适配 |
| B2 业务系统认证与数据 | 旧authorize/credential/OTP/JWXT服务已于47批删除 | Rust拥有业务请求、解析与有效性；当前新增五字段native恢复回归 | 纯凭据状态与学生档案模型 |
| B3 更新与公告 | 旧实现已于47批删除，当前两个纯client接口及Rust适配 | Rust拥有请求/解析/缓存/版本与资产选择；ready-owner关闭竞态已修复 | 列表、详情、更新提示和用户操作 |
| B4 配置与前端适配 | 设置/账户基类仅展示与命令接口；正式登录自动组合已进入native | 配置/schema/持久化、认证及凭据组合由Rust负责；继续整体验收 | 展示快照、输入收集、发送命令、订阅管理 |
| B5 桌面策略边界 | 连接owner、恢复/重绑、输入规范化/LAN选择及公告预取均已迁入Rust | 限定审查未定位新的独立Dart后台实现；平台能力与完整运行证据继续验收 | 桌面事件、依赖装配、权限状态和诊断展示 |
| B6 删除与桌面验收 | 生产 Dart 源码、`pubspec.yaml`/锁文件、Windows/Linux平台分支 | 唯一后端实现和持久化来源、配套构建与最终验收 | Flutter 与 FRB 所需依赖 |

## B1 首轮实现（2026-10-12，部分完成）

Rust 已新增 `auth/newsso/`、受限 Cookie 容器和 FRB `NewssoSession`。句柄拥有 SSO Cookie、服务器下发的 params、两步验证用户名/密码密文和会话代次，前端接口只返回验证方式、回调 URI、Cookie 名称及状态。密码按原协议做 RSA PKCS#1 v1.5/base64，不把明文或 Cookie 值作为输出 DTO。HTTP 复用 Rust 出口保护，密码 POST 固定于发现的 SSO origin，不自动跟随 POST redirect。

第十九批首次内存导入接受 `auth.newsso.cookies` 的旧 JSON 数组，只采纳 SHU_OAUTH2，绑定 newsso host-only、Secure、HttpOnly；导入提交检查代次，clear/new login 关闭旧数据入口。当时只有单句柄抑制，不能据此声称持久化接管已完成。

后续新增 NewssoSession.open(directory)、Rust `newsso-state.bin`、目录独占锁和与 Cookie 同次提交的持久化迁移标记。open 恢复提交记录，成功 clear 写入空会话并永久关闭该目录的旧数据入口；close 只释放句柄，保留已提交 Cookie。Windows 使用用户 DPAPI，Linux 使用私有目录/文件权限。导入只有成功提交后才能由前端删除旧键；写入错误明确返回，不能把内存清空当作成功的跨重启注销。删除或替换整个授权目录、系统断电及恶意同用户目录竞态不在普通恢复测试证明范围内。

**生产前端迁移仍未完成**：bootstrap/auth factory 仍同步创建旧 ShuAuthSession，旧 SharedPreferences 保存/读取/清理入口及账户中心的 persistSession 调用仍存在。必须一起替换这些 owner，并在 native 确认提交后删除 `auth.newsso.cookies`，不能只追加一次键删除，因为旧持久化调用会再写回。Rust 文件与标记的实现不等于正式路径已停止双侧存储。

生产 `createRustAuthSession` 仍构造 Dart ShuAuthSession，旧密码/QR/业务服务及 Cookie 库仍在执行。本轮没有切换正式登录入口或删除旧实现；Rust NewssoSession 的 hasSession 只表示当前可匹配的 Cookie 存在，不证明服务端有效性、账号身份或业务授权。具体测试与真实 FFI 证据见实施记录“第十九批：B1 密码登录与 SSO 会话基础”。

后续继续：Rust 持久化及跨重启迁移墓碑、企业微信 QR 创建/轮询/换码和取消、共用此账号 Cookie 所有权的业务 OAuth/aTrust/OTP/教务、账户展示快照、正式前端接线，最后移除已替代的 Dart 与 SDK 后备链。原平台、恢复、分发、Android 和校园验收仍保持。

后续 QR native 路径也已实现：NewssoSession 提供 startQr/pollQr/waitQr/redeemQr/cancelQr，同一账号句柄持有 provider Cookie、二维码 key、auth_code 与代次。前端获得图片字节、唤起链接和状态，不再需要把授权码传回 Rust；兑换要求受信回调及实际匹配的 SHU_OAUTH2，成功才采纳并持久化。尚未接入现有 WeComScanPage：其仍把 authCode 与 sessionCookies 传给 Dart AccountCenter，后续业务交换仍走旧 session。正式替换须同时覆盖共享账户 owner/业务授权，不能只把 QR 页面切到另一个独立 Rust 句柄。详见第二十一批记录。

## B2 共享业务授权基础（2026-10-12，部分完成）

NewssoSession 新增 authorizeSystem(jwxt/otp) 与 signInAtrust(deviceId)，使用同一账号的 Cookie jar、操作锁、代次和提交路径。OAuth 注册信息固定在 Rust，JWXT 使用随机 state 并校验回调，OTP 保留原协议的 state 发现/缓存/空值兼容及双斜线 Callback.aspx。回调 origin/path、唯一非空 code、跳转范围和请求预算受限；业务成功仅返回去除 query/fragment 的落点与实际状态，不输出回调码或 HTML。

aTrust 新路径直接以账号 jar 发请求并接收 Set-Cookie，复用现有换票、reportEnv、认证链与 onlineInfo。旧协议字段所需的 Cookie 名值快照从 jar 派生，不再让两个旧容器分别合并覆盖账号 Cookie；父域 Cookie 的轮换/删除、HostOnly、路径和绝对到期语义由单一 jar 保持。原独立 ATrustClient 的兼容路径仍存在，正式 Flutter 账户尚未使用新接口。

该基础不是完整 B2：授权落点与凭据可用性分别判断。OTP 数字码获取/解析/重试、教务身份和资料解析仍在 Dart；aTrust onlineInfo 认证也不是实际隧道验收。需完成这些数据接口及账户展示快照，再一起切换 auth factory、持久化迁移和正式页面，删除旧服务。网络验证为本地 HTTPS fixture，没有真实校园账号或 Android 运行证据。

后续接线应保留当前业务语义：credential_service.exchangeOtp 先尝试取码，仅 otpSessionExpired/otpParseFailed 才重新授权并再取一次，网络失败不反复授权；exchangeJwxt 授权成功后分别尝试档案及课表身份，任一资料失败不撤销凭据，课表身份覆盖姓名/学号。AccountCenter 目前以 available 状态计数，五分钟缓存不检查 OTP expiresAt，完整迁移时应明确到期状态策略。旧快照只保存身份、系统状态和验证时刻，不保存 credential payload/数字码；不要将新 DTO 秘密写入展示快照。入口证据：credential_service.dart 的 exchangeOtp/_readOtpCode/exchangeJwxt、account_center.dart 的 completeSignIn/verifyIfStale/_persistSnapshot，以及 auth_protocol_test.dart 的 OTP/档案/课表解析用例。

## 首批实施边界

第二十四批进一步新增同一 NewssoSession 的原生资源准备、PreparedATrustTunnel 不透明计划和 connectPrepared。新路径不把计划/SID/签名密钥作为 Dart 值传递；计划单次使用并绑定账号代次，建立后的 native 连接也随注销/close 停止。verifyAtrust 实际建立并关闭原生 L3 控制连接后返回可用性结果，供替换旧 Dart/SDK 探针。生产 factory 仍未切换，Debug 原生库验证不能视为发布接线完成；需与账户 owner/设备号/展示快照一起完成生产切换。

本批两端实际 Debug 库和全量回归为 Windows 211 Rust / 381 Flutter、Linux 184 Rust / 380 Flutter，桥接/跨进程存储、Clippy/analyze 通过，绑定16文件稳定，262个源文件摘要一致。未改持久化 schema，也没有重建 release bundle；发布产物仍对应第二十三批。原子旧数据采纳、生产账户/QR/连接入口及 B1–B6 的最终删除/全平台验收继续保留。

后续数据实现已新增 NewssoSession.fetchOtp/fetchJwxtAccount：OTP 先真实取码，网络/5xx 有限重试，只有会话失效或无法解析才重新授权一次；教务读取档案、课表页真实学期及表单身份，课表姓名/学号覆盖档案，缺失资料返回可选字段。两者沿用同一账号的锁、代次、Cookie 与提交路径，退出后的迟到结果不采用。失败的资料子请求及 OTP 重试回滚其 Cookie，不能用 optional 降级绕开提交规则。第二十三批两端本地发布/真实 FFI/存储回归已通过：Windows 204 Rust / 380 Flutter、Linux 177 Rust / 379 Flutter；正式 Dart owner 尚未替换，不能据此声称已停止旧后台执行。

1. 先完整阅读 B1/B2 调用链和现有认证测试，复用已有 Rust HTTP 出口保护、认证失败类型和请求预算。
2. 以 Rust 账号会话持有 Cookie、登录挑战和持久化状态；GUI 通过不透明会话句柄发送登录/验证码/退出命令，接收展示 DTO。
3. 保留现有凭据/设置格式与账户语义，用一次性导入迁移旧 SharedPreferences 数据；导入结束后 Rust 成为唯一状态来源。
4. 使用独立 HTTP fixture 验证真实回调限制、Cookie 隔离、验证码错误、取消、并发与退出后晚到结果，再验证打包库真实 FFI 和 Windows/Linux/Android。
5. 登录与业务授权接线齐备后删除旧 Dart 服务、相应纯后端测试替身和 SDK 调用；测试保留等价行为证据，不能通过删掉失败用例完成迁移。

首轮只读协议定位已确认：`ShuNativeAuthService.login` 从教务入口跳转发现登录页 params，再以 RSA 加密密码请求 `/oauth/userLogin`；挑战状态由 Dart 暂存登录 URI、params、用户名和加密密码，`sendCode` 与 `verifyCode` 分别调用 `/oauth/twoStep/send`、`/oauth/twoStep/verify`。这些状态与提交策略进入 Rust 会话；前端只接收可选验证方式、输入验证码并提交命令。该定位只证明现有 Dart 行为，不表示真实学校协议已完成新实现验证。

## 审计方法与完成证据

2026-10-15 桌面协议链复核（范围有限，不是零Dart验收完成）：生产 bootstrap 注入 RustSettingsStore/RustAccountCenter/RustConnectionAttempt，原生 settings snapshot 只将 aTrust 纳入 enabledProtocols；EasyConnect/OpenVPN 的 UI、selector 和普通设置均拒绝启用。当前可达桌面连接和账户动作进入 Rust，native 失败只清理/报错，没有自动 SDK 回退。点验证据：`rust/src/settings/schema.rs` 的 enabled_protocols、`lib/core/rust/rust_settings_store.dart::isProtocolEnabled`、`ConnectionController.connect` 的 nativeFactory 分支。

2026-10-16 入口保护更新：nativeFactory存在时，controller对非aTrust直接unsupported，不再落入旧端点处理；强制放开未实现协议的回归已先失败后通过。现有controller仍含旧 `_buildConnector` 和EasyConnect SDK后台代码，不能据这道门宣称旧实现已删除；后续需明确桌面模块边界、所有权与零Dart审计。保留Android源码不作为付费桌面验收前置。规则编辑/重复条件诊断由Rust负责；连接记录的新Dart类型仅是JSON展示DTO、异步视图状态及已有原生decisions薄读取，未新增网络或路由算法。

2026-10-16 第44批进一步删除未被生产使用的RustATrustAuthentication/createRustAuthSession混合层，以及NativeATrustTunnelAuthentication/ShuPreparedATrustTunnel。RustConnectionAttempt只接受NativeAccountAuthentication并在core/恢复前拒绝旧对象；不再使用Dart Cookie或plain认证plan。相应协议回归改为直接原生客户端，设备号改正式账户owner。457/456 Flutter及独立初始化边界通过；负面AOT符号检查在删除前也已为空，不作为整体删除证据。ConnectionController内旧后端及native启动/超时/清理策略仍存在，完整desktop零Dart审计未完成。

B3 当前：桌面Provider已选Rust更新/公告适配，Rust拥有版本/资产解析、固定GitHub HTTP、公告请求/HTML解析/缓存及关闭取消；两端新正式库与全量本地回归通过，详见第二十八批。Android原生加载已有第三十批探针证据，正式业务分支未切换，旧Dart HTTP/解析/isolate/cache保留于Android分支，最终删除与真实发布/学校站点验收仍未完成；不能用桌面或局部FFI证明B3整批完成。

历史的2026-10-12 Android接线检查（工具链/打包现已由第三十批推进）：rust_builder/pubspec.yaml 仅声明 Windows/Linux FFI 插件，Android 没有 Cargo/NDK/.so 打包接线；bootstrap 仍只为桌面初始化 Rust/注入 RustConnectionAttempt。现有 Kotlin VpnService/plugin 将 TUN fd 返回 Dart，ShuAndroidVpnDevice/SangforTunnelRouter 仍执行旧数据面。当前可发现的本机环境只有 Windows Rust target，标准位置的 Android SDK/JDK、NDK linker 配置均缺失（未穷尽任意自定义安装目录）。这不阻止继续桌面账户迁移，但不能用两端桌面 bundle 宣称 Android 已接 Rust。后续需补 arm64 Android 编译/打包、真实桥接加载，以及 Rust fd 生命周期/socket protect 与 VPN 权限薄接线，再删除旧 SDK 路径。

以下搜索是入口定位，不是通过/失败判定。必须阅读调用链，区分生成绑定、前端 DTO、平台界面适配及测试 fixture。

```powershell
rg -n 'HttpClient|HttpServer|Socket|RawDatagramSocket|package:http|Dio\(' lib
rg -n 'Cookie|credential|encrypt|decrypt|retry|Future<void>\.delayed' lib/core
rg -n 'flutter_sangfor|ShuHttpProxy|SangforConnector|ATrustTunnel' lib pubspec.yaml
rg -n 'SharedPreferences|File\(|Directory\(' lib/core
```

完成需要同时满足：

- B1–B5 正式行为通过 Rust 与真实 FFI 验证，现有持久化升级与用户状态保持正确。
- 所有平台分支均使用 Rust 后端，不存在 Android 或停用协议的长期 Dart 后备实现。
- Dart 中没有后台网络、协议、业务解析、业务计算/策略、凭据/配置独立存储；生成绑定和薄前端适配按主计划边界逐项确认。
- 旧代码和运行期后台 SDK 依赖删除，正式产物证明生产路径不再加载它们。
- 原迁移计划的网络恢复、规则能力、安装包及真实校园长期验收仍须独立完成。

最初侧边对话只完成目标修订及入口审计；后续已有上述 B1 中间实现。B1–B6 均不能仅凭该基础模块标为完成。
