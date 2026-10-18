# 持久连接 owner 的本地协议长跑

第84批补充（2026-10-18）：已新增测试网关侧心跳观测及独立5秒时序配置，新的Linux50毫秒复现通过即时原生快照定位到心跳判失活，末尾回复间隔0.585秒；原449秒记录仍不能反向归因。工具、负向检查、不同网络环境Rust回归与新配置结果见 [心跳诊断记录](heartbeat_diagnostics_acceptance.md)。下文第82批来源、失败与复验保持原样。

第82批，2026-10-18。针对M5.1长期运行与资源占用的证据缺口，新增有明确时长、轮数和增长预算的本地探针。**Windows正式600秒/100轮通过；Linux首轮约449秒提前EOF，带诊断的同配置复验600秒/100轮通过，首次失败仍未归因。** 总汇保留partial/未完成状态。这不替代原计划要求的真实校园站点长测。

生产入口、Rust实现、绑定、字体和安装包不变。新增开发用 `tool/connection_soak_probe.dart`、`tool/connection_soak_acceptance.py`、`tool/process_resources.py` 及7项Python测试；已有 `connected_desktop_acceptance.py` 的代理握手提取为context manager供复用，原echo仍在写完payload后半关闭、核对回显和EOF。开发探针不被正式应用导入。

工具/测试源479文件SHA256 `224734f6dc6d91753d56dacda9e79fb48693f093fe29c073680d5cd9e9b8740a`，两树一致。Dart探针独立编译为AOT可执行文件，实际加载77批字体产品的配套核心库，以创建时间/原进程句柄或pidfd、映像路径/摘要及模块表/maps检查绑定。编译日志和Dart源码/探针摘要单独记录；不凭二进制摘要反推源码。

## 正式运行结果

Windows `long-windows-01` 共615.375秒，四条持久流分别600.406、600.438、600.047、600.031秒，独立核对305,201,152字节payload上传及同量回显。包含预热10轮、持久1轮和重连100轮，共111次session、444流。原探针/fixture退出0、无强杀、owner closed=true，无cleanup_pending。最终断开后probe相对基线RSS +2,703,360字节、PrivateUsage +4,091,904字节、句柄+11、线程-4；最大采样值须见原始resources.jsonl，不能把最终值称作峰值。宿主两处代理键选定值摘要前后相同。

Linux `long-linux-01` 在449.938秒结束，原始错误“Proxy closed before the expected protocol bytes”。只完成10轮预热，持久阶段不能计作完成600秒，也没有执行后续100轮。最后448.889秒采样仍有4个成功拨号、owner/session connected，upload230,293,504、download230,293,532，所采资源没有超预算。路由decision无error仅说明初始拨号成功，不能据此否定随后的流读写/心跳失败。

Linux首轮失败时未额外抓取即时原生失败快照，因此根因未归因。EOF导致驱动关闭stdin后，probe经finally清理再以255退出，fixture退出0；没有强杀或observer清理错误，宿主link和IPv4/IPv6路由摘要相同。该255属于失败路径，不能写成两进程正常退出0。后续需补即时失败状态/每流进度诊断，保留原失败和原来源，不能直接重跑为绿色后抹去。

本轮等待长跑期间另完成[Linux前置规则真实窗口弹窗](linux_rule_dialog_gui_acceptance.md)，与长跑/资源证明分开。它不修复或签收Linux长跑EOF。

## 后续诊断来源与复验终态

原始运行已封存为 `verification-baseline.json`，passed=false、partial=true，旧探针/驱动源码另存baseline-source，不能用后续工具来源改写其结果。随后只增加开发诊断：持久流逐条记录已验证批次/字节、完整时长与EOF状态；错误含协议/地址/进度；关闭输入前额外请求即时native snapshot，保留原始错误及诊断失败原因。生产Rust库和预算未改。

诊断版源仍479文件，SHA256 `0c5618da0a8db9af036c8791b296eb19f58d0dabeba85e4dac476d59bbcaf395`。两端完整工具125项通过（19/7跳过）、analyze及边界108通过，诊断AOT重新编译；两端短跑和负向资源预算均验证了新诊断。原始124项与原始600秒运行仍归224734f6来源，不冒称新工具也重跑了Windows600秒。

Linux受控试验只暂停本次驱动的原始fixture子进程，先以exe、PPid、UID及pidfd绑定，再用pidfd发SIGSTOP/SIGCONT。暂停实际0.401秒；50ms心跳配置下，HTTP IPv4在0.198秒、0个完整64 KiB批次时提前EOF，错误快照抓到owner.phase=failed、error_message为“L3 节点未回应心跳”，清理后fixture/probe均已结束且无强杀。这证明新诊断能识别心跳失败；**不能反推原449秒故障也是同因**。

`long-linux-02` 使用新诊断AOT、相同字体核心库、fixture、50ms心跳、600秒/100轮/10预热及完全相同的资源预算复验，已经消费原始exec会话的退出0及终态报告。整体642.822秒，四条持久流分别600.088、600.086、600.212、600.590秒，payload310,509,568字节上传及同量回显；共111session/444流，各轮数据/决策/监听器屏障完整，最终owner closed=true，probe/fixture退出0且无强杀、无失败快照或清理错误。

Linux复验最大采样增长：probe RSS3,010,560字节、VmSize2,101,248字节、FD16、线程0；fixture RSS307,200字节、VmSize0、FD5、线程0，均在预设预算内。宿主link和IPv4/IPv6路由摘要前后相同。结果只覆盖本次采样，不证明不存在泄漏，也不消除第一次间歇性EOF。两次原始Linux记录及各自工具来源均保留，未声称修复生产故障。

当前总汇 `build/desktop/connection-soak/verification.json` 标明linux_second_soak.running=false及linux_diagnostic_soak_passed=true；整批仍passed=false/partial=true，原失败cause_attributed=false。当前终态重算使用 `record_terminal.py`，它只消费原有终态，不启动或重跑长跑；`record_current.py`仅用于当时的在途存活核对。源未冻结为新的生产候选，没有提交/发布。

## 运行约束与判据

探针只打开一次DesktopConnectionOwner，claim/awaitFrontend一次；每轮调用原生connectPlan，等待disconnect完成后释放该轮session，保留同一个owner。最终shutdown要求closed=true、无run_id/cleanup_pending、正常进程退出。会话run_id必须各不相同，不要求恰好连续。

使用已有proxy_fixture的合成TLS网关，证书固定且不接受任意证书。仅接受一个127.0.0.1节点与固定IPv4/IPv6合成资源，移除通配域名和DNS服务器；两种代理只绑定127.0.0.1临时端口。TUN为空、systemProxyEnabled=false且启动报告必须确认无TUN、系统代理或服务失败。HTTP/SOCKS与原生匹配均实际执行；plan入口取代校园认证，未调用connectAccount。

该fixture的heartbeatSeconds为0.05，控制循环连续三次未回应会失败。当前正式鉴权的plan构造源码 `rust/src/auth/atrust.rs:246` 设置5.0秒，因此本次是更紧的本地心跳压力配置，不能直接外推正式认证路径的超时表现。原50ms失败和同配置复验继续保留；如另测正式5秒profile，须单独记录。

正式配置在运行前固定为：

1. 预热10轮，每轮HTTP CONNECT/SOCKS5各访问IPv4/IPv6合成目标，共4流，各64 KiB、7字节网关prelude、写半关闭、完整回显与EOF。
2. 同一owner再建立4条代理连接，各保持至少600秒，每约0.5秒发送64 KiB并逐字节核对回显，最后半关闭并检查EOF。Python独立客户端持有真实socket，不以原生自报成功替代数据传输。持久流的EOF检查不等于FIN后尾部数据验证；后续短流单独覆盖先FIN再读取回显。
3. 再完成100轮相同4流的连接/断开；含预热和持久阶段共111次session、444条流。
4. 每轮精确核对原生upload、download（payload加每流7字节）、成功连接数及四种流的SHU-VPN意图/实际出站。断开必须是可复用owner、已断开session、TUN停止、egress无active/pending_cleanup/socket。
5. 下一轮绑定前，进程存活时读取Windows IPv4/IPv6 OS监听表，或Linux tcp/tcp6表并关联该进程socket inode，要求所属监听器为空；不能用超时或消失的/proc当作关闭成功。最终关闭以owner终态及原进程退出为屏障，不在其退出后读取/proc。

## 资源观测

第10轮断开后的probe及fixture样本作为固定基线。持久连接期间约每秒采样，重连阶段每次断开后采样；不是连续观测，不覆盖预热增长及所有轮间短暂峰值。

| 指标 | Windows | Linux | 相对预热基线的上限 |
| --- | --- | --- | --- |
| RSS / Working Set | 是 | 是 | 32 MiB |
| PrivateUsage | 是 | 否 | 32 MiB |
| VmSize | 否 | 是 | 128 MiB |
| 进程句柄 / FD | 句柄 | FD | 32个 |
| 线程数 | 是 | 是 | 8个 |

值/单位/身份/存活检查失败不能转成0。Windows保留原进程句柄并核对独立内存查询句柄的创建时间；Linux持有pidfd、核对启动tick、exe、UID1000、CapEff0。Windows检查未提升及所属父进程，不称已验证用户SID。两端都核对请求的核心库确实映射在原探针进程。

首次超预算仍写入完整原始probe/fixture样本、阶段、轮次及growth_error，再判失败。负向故障注入以故意不可能满足的RSS预算验证该路径，失败报告不能升格为正式长跑成功；同时要求原始子进程结束且没有强杀。Windows host代理键与Linuxlink/IPv4/IPv6路由分别另取前后摘要；这不是全时监控，Linux没有独立的桌面系统代理快照。

低于预算的持续增长、未采指标、短时峰值及更长时长的问题仍可能存在。结果至多证明**本次配置、时长及样本的增长没有超过预设预算**，不能写成“没有内存泄漏”。

## 检查、失败记录与复现

两端7项新增守卫已通过：实际字节/出站、完整断开、计数器缺失/类型/预算、plan约束、消息身份/顺序、退出不能当空监听表，以及真实子进程资源采样/错误映像/错误模块/退出拒绝。两端完整Python工具124项通过，Windows19项、Linux7项平台跳过；analyze和生产Dart边界108文件通过。

Linux最后同步三个Python文件后重跑124项（tools-linux-04.log）；Dart未变化，分析/编译03的源码摘要保持。首次Linux短跑完成全部数据/重连，但最终读已退出进程的/proc失败，报告passed=false保留；修正为每轮存活时查表，最终看原进程退出。中间analyze缺大括号的info返回1也保留，最终03无问题。没有改Rust、生产GUI或扩大预算来掩盖失败。

```powershell
dart compile exe tool/connection_soak_probe.dart -o build/desktop/soak-probe.exe
python tool/connection_soak_acceptance.py `
  --probe build/desktop/soak-probe.exe `
  --fixture rust/target/release/examples/proxy_fixture.exe `
  --library /absolute/path/to/shuvpn_core.dll `
  --report build/desktop/new-soak `
  --hold-seconds 600 --cycles 100 --warmup 10
```

Linux使用对应AOT、fixture及.so路径，仍以普通用户运行。所有报告目录须新建于build/desktop内；新的rules目录只保存合成配置。本次没有TUN、系统代理启用、真实账户/校园、正式GUI、长期OS崩溃恢复或远端CI验收。更长/多日、真实站点、Windows可恢复guest特权Full、其它桌面/Wayland及最终审计继续保留；整体goal active。
