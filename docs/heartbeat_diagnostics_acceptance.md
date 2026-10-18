# 本地长跑的心跳观测与独立时序配置

2026-10-18，第84批。第82批 Linux 449.938秒提前 EOF 仍未归因。本批补齐测试网关侧的控制心跳记录，分别运行50毫秒压力配置和5秒鉴权计划时序配置。生产 Rust 心跳循环、超时门槛、原始失败记录及第83批四包保持；本批不声称已修复生产故障或完成整个迁移。

## 从旧记录能确定的事实

独立只读复核定位首轮终态前59个资源样本，主线程抽查最后样本与原生快照：`resources.jsonl:452` 为448.889秒，probe RSS30,957,568 bytes、VMS3,151,032,320、FD30、线程43，fixture RSS6,455,296、VMS1,461,092,352、FD15、线程22。相对预热基线的增量未超过预算；这不能排除更短暂停或未采指标。

首轮最后原生快照仍 connected，upload230,293,504、download230,293,532、4个连接。没有带时间戳的第一次 EOF 事件或即时失败快照；最后59个资源间隔约1.004至1.157秒，不能从秒级采样证明不存在150毫秒量级的调度或网络停顿。

首轮 probe stderr 的“Probe input ended without explicit shutdown”没有时间戳；主线程亲读驱动确认流异常后关闭stdin会走该失败清理路径，但原记录无法单凭stderr确定首次故障因果。首/第二轮fixture及core摘要相同，probe二进制及诊断源码不同；第二轮绿色不能反向消除第一次失败。原报告及总汇未重写。

当前鉴权构造 `rust/src/auth/atrust.rs` 的 heartbeatSeconds 为5.0；测试网关原始计划固定0.05。两种配置都通过本地合成plan入口，不调用connectAccount，也不证明真实鉴权、校园站点或生产网络的表现。

## 本批改动

新增 `rust/tests/support/gateway_trace.rs`，仅测试网关在显式 `SHUVPN_FIXTURE_HEARTBEAT_TRACE=1` 时启用。每条L3连接分配本进程单调编号，累计完整心跳请求/成功flush回复数，保留最后64个事件以及相对连接开始的单调时长、最大回复间隔、请求解析至flush的最大耗时、最后回复年龄及pending状态。

普通模式不查询额外peer信息，不输出trace。记录走stderr，stdout第一行仍是原plan。网关example在stdin结束后显式停止并等待连接任务，再输出已收集的连接错误；默认测试对象Drop路径保持。trace的Drop写入不再次panic，验证器拒绝缺失或截断的所请求记录。

`tool/connection_soak_acceptance.py` 保持默认50毫秒，新增显式 `--heartbeat-seconds 5.0` 独立配置及 `--trace-heartbeats`。原始fixture计划必须仍为固定0.05，所选时序只覆盖发给探针的私有plan；不能混称同一运行配置。驱动清除fixture继承的trace环境变量，再按显式参数启用。probe仍为未改动的82批诊断AOT，核心库仍为既有正式配套库。

读取器检查连接编号不重复、请求/回复及pending计数、有限非负时长、事件上限/顺序、已知事件名称和末尾累计计数；成功运行要求编号覆盖本轮完整连接数，且实际记录过心跳。缺记录不当作未发送心跳或绿色通过。额外fixture连接错误阻止成功；stderr读取、产品重核或观察者关闭失败保留原始故障并写失败报告。

新增工具守卫先观察到旧读取器接受重复/不完整记录，以及后续读取错误覆盖primary error的失败，再完成修正。配置分离、日志解析和完整失败收尾均有独立用例。`guards-red-02.log`、`guards-green-02.log` 保留对应红绿记录。

当前源码483文件，两树SHA256 `b4a9d1f1f4570972abb336c93e1b0cf691b2f7d7cdcb216ce35e85ac521cf1d7`。两端各135项工具测试（Windows19/Linux7跳过）、Rust相关18项、对应Clippy严格检查及rustfmt通过。完整Rust的不同网络环境结果见下文，不用私有环境绿色抹去宿主失败。

Linux可按以下入口重新建立独立报告；Windows使用对应`.exe`、DLL和路径。SDK、依赖与原生库准备沿用原有构建说明。报告目录必须尚不存在，不能重用本批或第82批目录。

```sh
cargo build --manifest-path rust/Cargo.toml --locked --release --example proxy_fixture
dart compile exe tool/connection_soak_probe.dart -o build/desktop/soak-probe
python3 tool/connection_soak_acceptance.py \
  --probe build/desktop/soak-probe \
  --fixture rust/target/release/examples/proxy_fixture \
  --library build/linux/x64/release/bundle/lib/libshuvpn_core.so \
  --report build/desktop/new-auth-timing \
  --hold-seconds 600 --cycles 100 --warmup 10 \
  --heartbeat-seconds 5.0 --trace-heartbeats
```

另测原压力配置时显式使用`--heartbeat-seconds 0.05`及不同报告目录。新的5秒通过不能改写0.05秒失败。

## 已完成的受控检查

报告目录 `build/desktop/heartbeat-diagnostics/`，所有case使用新目录。旧fixture二进制另存baseline-products；新fixture实际编译、摘要及来源记录与旧probe/core分别保存。第82批原文件不覆盖。

两端 `compatible-default` 使用旧fixture、默认时序且不请求trace，通过，验证原入口仍可用。两端 `missing-trace` 完成全部3轮、数据及owner closed、原子进程退出0后，因请求的trace不存在而严格返回失败；该负向结果不是网络失败或长跑通过。

Linux `stress-smoke` 的8秒四流/1预热/2重连通过，4条控制连接记录齐全，保留事件达到64上限且累计请求超过64，证明事件裁剪不重置累计计数。Linux `auth-timing-smoke` 的12秒、5秒心跳独立通过，确实记录两次以上请求，不凭计划数值推断实际节拍。

Windows 首次 `stress-smoke-01` 使用1轮预热，在4.922秒触发probe句柄增量33、固定上限32，原probe255/fixture0退出，无强杀。即时native仍connected，trace2共记录70次请求/70次成功flush，最大回复间隔约0.065秒。故障是实际预算拒绝，不能改写为心跳失败或通过。随后使用正式10轮预热和同一固定预算另开报告：`stress-smoke-02` 共13轮10.625秒、`auth-timing-smoke-01` 共13轮15.953秒通过。首个失败完整保留，不以改变预热抹去。

Linux `controlled-pause-01` 仅对本驱动的原始fixture子进程注入：亲核exe/SHA、PPid、UID1000、CapEff0及pidfd，然后暂停0.400909秒并恢复。HTTP IPv4在2个完整批次、1.194秒提前EOF；即时owner phase=failed、error_code=connection、error_message为“L3 节点未回应心跳”。两条控制连接的trace齐全，受影响连接23次请求/23次flush，最大回复间隔0.431995秒，原probe255/fixture0结束，无强杀、无清理错误。

该暂停实验只证明新记录能在已知注入下关联fixture回复停顿与原生心跳失败；它不归因第82批故障。所有计数相等也不能证明客户端在超时前消费回复。

## 自发压力复现与完整 Rust 回归

`linux-stress-long-01` 保持50毫秒/600秒/100重连/10预热及原预算，实际整体198.256252秒失败，只完成10轮预热；HTTP IPv4在持久阶段367批、193.886秒EOF，其它三流各366批，均未完成持有时长或半关闭EOF判据。原probe443890以255退出、fixture443867以0退出，无强杀、无清理错误；采样资源未越预算。

这次即时owner明确记录 `phase=failed`、`error_code=connection`、`error_message="L3 节点未回应心跳"`，可以将**本次复现的直接会话失败原因**定位为心跳判失活。代码为三次请求发出后尚未处理到回复、下一次timer分支才失败，且使用Delay，不是严格墙钟150毫秒门槛。

11条控制连接trace齐全。第11连接累计3824请求/3824次flush、pending=false、最大回复间隔0.584616457秒。末次前回复3821位于193.359142838秒，随后三次请求3822/3823/3824在193.943733秒附近连续读出并flush，之后read_end。最大请求解析至flush仅0.000965秒；这只能缩小延迟区间，不能区分客户端发送、网关调度/读取、传输积压或其它来源。此时并行Rust构建/回归也是本实验负载的一部分，不能仅因同时存在就宣称它造成超时。

该结果不同于无诊断的第82批449.938秒故障；旧故障仍未归因，本批也没有修改生产心跳阈值或宣称修复。报告、四流进度、错误快照、资源及stderr均封存，独立只读复核确认上述关键值与代码门槛。

完整Rust `--locked --lib --tests`：Windows365项通过；Linux宿主首轮242通过、3个`network::http_tunnel`测试超时，不能签宿主全套通过。将该组单独串行运行仍2通过/3失败，故不能归因于测试组间并行。

普通UID1000的独立socket实验可执行SO_BINDTODEVICE，却在绑定lo后连接本实验loopback listener超时。新的loopback-only网络命名空间内仍用同一普通用户、同一源码/二进制，原socket实验成功，HTTP出口5项通过，完整Rust349项通过。宿主link/IPv4/IPv6路由在隔离验证前后相同。这把失败缩小到当前宿主网络命名空间相关行为，尚未定位具体机制；没有修改系统代理、宿主网络设置或放宽测试时限。

Windows独立5秒时序 `auth-timing-long-01` 已通过：四流各≥600秒，之后100重连，111session/444流，持久payload303,628,288 bytes，整体626.843秒；111条控制连接trace完整，最大fixture回复间隔5.020914秒。原probe/fixture退出0、owner closed及采样预算通过。它是不同配置的通过记录，不抵消50毫秒压力失败。

Linux独立5秒时序同样已消费原exec与终态：111session/444流，四持久流各≥600秒，持久payload303,497,216 bytes，整体650.564410秒；111条控制连接trace完整，最大fixture回复间隔5.043589秒，原probe446554/fixture446531均退出0、无强杀、owner closed、监听屏障及原采样预算通过。两个5秒profile均无失败快照、trace读取错误或fixture连接错误。

本批总汇 `build/desktop/heartbeat-diagnostics/verification.json` 保持 `passed=false, partial=true`，并单列diagnostic_changes_verified及两个5秒profile通过。不把两个通过profile提升为整批全部压力通过。Windows两应用目录/两代理键、Linuxlink/IPv4/IPv6路由前后相同；隔离Rust验证也有独立前后网络摘要。最终同映像的本次fixture/probe均不存在，无在途长跑或构建。第83批四包逐项摘要保持，没有新生产构建、安装、提交或发布。

## 证据边界与后续

trace时长为fixture本地观测，`response_flushed` 只证明TLS写入及flush返回，不证明客户端读取、解码或更新心跳状态。最大gap包含第一次请求前的等待，最大write包含调度等待；没有完成的最后一次回复不会更新这两个max，须同时看pending、最后回复年龄和尾部事件。

只保留每连接尾部64事件，不是全程抓包；部分帧和排队未读取请求不计数。L3 header读取错误进入read_end记录，TLS accept失败和部分任务panic仍有观测边界；没有连接错误日志不证明全程无错误。日志本身也有运行开销，强制终止不能保证Drop记录。

Linux50毫秒复现与Windows5秒时序均有并行构建/回归负载，不能当作空闲宿主测量。Linux独立5秒时序另开报告，不以它替代原50毫秒配置；运行期间也有其它本地验收任务，不能据此构造等负载的跨平台性能排名。

保持原32MiB RSS/Private、128MiB VMS、32句柄/FD、8线程预算。成功结果最多覆盖本次配置、时长和采样，不证明没有泄漏；失败仍各自保留。真实校园长测、Windows原始/安装GUI与guest特权Full、更多窗口矩阵、更长/多日、其它桌面/Wayland、远端CI和最终迁移审计继续，goal active。
