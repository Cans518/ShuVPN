# Windows Full 验收驱动与当前证据

第70批，2026-10-18（承接10月17日）。已接入独立观察者与普通 probe 的协调，完成真实**两个普通进程的只读 Plan** 验证。提升权限观察者、UAC helper 和 Full 数据路径的组合仍未实测，不能从此处推导 Windows Full 已验收。

## 独立观察者接线

同一个探针可运行 `--observe-plan`、`--observe-helper`、`--observe-local`。观察者只查询状态并保留进程 handle，不创建适配器、不修改路由、不启动或终止 helper，也不自行请求提升权限。Helper/local 观察者必须预先提升；helper 模式的 probe 必须是普通 token，local 模式的 probe 必须已提升。

普通 probe 创建单实例、拒绝远程访问的随机命名管道，观察者以 `SECURITY_IDENTIFICATION` 连接。双方通过 Windows 的管道对端 PID 查询打开 query/wait handle，核对不同 PID、相同可执行映像和 SHA；begin 回复的完整身份还须与 probe 保留的观察者 handle 核对。身份包含原始创建 FILETIME、映像、摘要、SID、elevation，不把日志中的 PID 自报值当成唯一依据，也不把 UAC 的操作系统父进程链当作归属保证。

运行阶段严格为 `begin0 → ready0 → cleaned0 → ready1 → cleaned1 → finish0`；Plan 仅 `begin0 → cleaned0 → finish0`。每条消息核对 nonce、OS 绑定的 probe PID、序号、阶段及轮次。`tun-probe` 特性下，正式 helper 客户端从它已经持有的精确进程 handle 提供 PID/创建时间到 `helperIo.process`；观察者再独立打开该实例，核对创建时间、固定 companion 映像/SHA及提升状态。每轮必须看到原 helper 退出0，再检查 all-compartment 网络基线。此诊断字段不修改普通构建的 FRB API、helper wire 协议或业务决策。

最后一个 finish 回复允许 probe 正常退出；观察者随后通过原 handle 等待 probe 退出0，再取一次网络快照，才设置最终 `passed=true`。失败也可能输出 `WINDOWS_FULL_OBSERVER_COMPLETE`，因此监督脚本同时要求布尔结果、退出码、完整原生消息、nonce、观察者身份和正确 scope，不能只看标记。缺原生消息、其他 nonce、null 网络表均有拒绝回归。

快照逻辑已提取到 `tool/full_capture_windows_snapshot.ps1`，并以 `include_str!` 固定进观察者二进制：实际查询内容与探针 SHA 一起固定，运行时不执行可被客户端替换的外部脚本。观察者使用系统目录内的 Windows PowerShell，而非依赖 PATH；对子进程指定执行策略参数和 UTF-8 输出，不修改系统执行策略。代理项从 **probe SID** 对应的 `HKEY_USERS` 读取，避免误用提升后观察者自己的 HKCU。观察者只向 stdout/管道输出，不接受客户端的命令或特权文件写入路径。

## 已执行的双进程证据

`build/desktop/windows-full-observer/preview-embedded/verification.json` 为当前二进制的真实 Plan 结果。观察者 PID81120、probe PID81344，两个 token 均普通，分别正常退出0；观察者创建时间与启动方保留的 `System.Diagnostics.Process` 相同。网络快照前后相等，非空计数为路由46、地址18、IP接口14、适配器23、DNS14。双方身份、三条握手/清理/结束事件、原生正式 Full 预览及退出后的快照均落盘。此前 `preview-process-policy` 为脚本固定进二进制前的通过记录，不替代当前源/产物配对。

首轮因 PATH 不含 `powershell.exe` 返回 `program not found`；改为系统路径后又明确暴露系统 Windows PowerShell 的脚本执行策略限制。两次都以失败退出并保留 `preview/`、`preview-system-shell/` 的假设验证记录；最终对子进程指定执行参数后通过，没有把失败记录改为通过。普通 token 运行 `--observe-helper` 的拒绝也已实测，不会自行弹 UAC。

Windows 原生探针10项通过，包含真实子进程 handle/退出0与退出7负例；平台过滤测试默认49、`tun-probe`50项、普通权限实际 helper IPC4项通过。Linux 对应 example 契约3项、默认平台35项通过。两端工具各66项（Windows19、Linux5平台skip），针对 lib/tests/example 的特性构建 Clippy、Dart边界108通过。该批没有重跑 Rust/Flutter 全量、完整应用/安装器或远端CI。

汇总为 `build/desktop/windows-full-observer/verification.json`。当前457来源文件两树 SHA 为 `2ddaa84eec83e2cd8465216db2265baf0d1aa3918034dbdc73f86a7bc1170078`。新增 Windows SDK authorization 特性仅供 SID 查询；当前输出仍为开发探针，不是新的桌面发布候选。

## 两个终端的运行方式

仍须先准备独立可恢复 Windows guest、独立双栈 peer 及上文 fixture 配置。在该环境构建匹配的探针/helper，按第69批命令把 helper/Wintun 放到 examples 目录。在普通终端用 `$token = [Guid]::NewGuid().ToString('N')` 生成新值，并把同一个值赋给提升终端的 `$token`。先在**预先提升的终端**启动只读观察者：

```powershell
& rust/target/release/examples/desktop_windows_full_smoke.exe `
  --observe-helper $token *> build/desktop/full-observer.log
```

在普通终端、60秒连接期限内启动 probe：

```powershell
./tool/full_capture_windows_smoke.ps1 `
  -ProbePath rust/target/release/examples/desktop_windows_full_smoke.exe `
  -InterfaceAlias 'Ethernet' -PeerConfig peer.json `
  -ReportDirectory build/desktop/full-helper -Mode Helper `
  -ObservationToken $token -ObserverLog build/desktop/full-observer.log
```

`-Mode Helper` 现在要求观察者及其日志，不再让同一个普通监督器尝试完成特权快照。只读复现可将观察者改为 `--observe-plan`、监督器改为 `-Mode Plan`，两个终端都不需要提升；其 scope 明确为 Current，不能用于替代 All 验收。`-Mode Elevated` 可配 `--observe-local` 做独立见证。

尚待执行：提升观察者/普通 probe/真实 UAC helper 的组合、两轮 Full 数据和精确 helper 退出；后续的覆盖变化、外来路由保留、helper 崩溃/重建、DNS歧义/HTTPS、Flutter owner和安装默认库等范围继续按交接保留。

## 第69批历史记录

第69批，2026-10-17。本批建立了独立的 Windows Full 原生探针和双栈对端，不将旧校园探针的 `--plan` 当作 Full 验收。当前已执行的是正式 Full **只读预览**、驱动契约、真实 loopback peer 服务和监督脚本负例；Windows 特权 Full 数据与故障矩阵仍未通过验收。

## 正式预览

`rust/examples/desktop_windows_full_smoke.rs` 显式调用
`VpnSession.desktop_tun_plan(..., Some(CaptureMode::Full), Some(interface))`。
本地 TLS fixture 经过真实会话握手；生产 API 读取当前系统路由和本机地址，计算完整捕获分区。探针要求：

- 计划为 Full、DNS 接管开启，物理 DNS 指向独立 peer。
- 所声明的物理源地址属于计划的保护地址。
- 校园和两个独立 DIRECT 对端确实位于捕获分区，不能用本机/回环 peer 绕过测试。
- 预览之后 TUN 仍 stopped，cleanup_pending 为 false；会话仍跨越正式 finish_stop 屏障。

当前调试版探针已在普通 Windows 进程完成上述预览，没有调用 TUN 启动或触发 UAC。`tool/full_capture_windows_smoke.ps1 -Mode Plan` 另核对**当前 compartment** 的非空活动路由、地址、IP 接口、适配器、DNS 和 WinINet 代理配置；只比较稳定配置字段，不比较寿命/流量计数。该次前后 JSON 相同，包含路由46、地址18、IP接口14、适配器23、DNS条目14。证据为 `build/desktop/windows-full-driver/preview-current/`。

最初监督脚本在 `-IncludeAllCompartments` 查询被拒绝后拿到空表，错误报告“恢复通过”。已保留原日志及原 JSON，并在旧 `preview/verification.json` 标为 invalidated/未证明，不能计作通过。修复后所有查询显式捕获错误，并拒绝空关键表；两条真实 PowerShell fixture 回归验证在查询错误或空路由表时不会启动 probe 或生成通过报告。当前普通进程的 all-compartment 查询拒绝也已用不会修改网络的 Python 占位程序实测，程序未启动。

## 两轮正常运行入口

新探针的运行分支已经编译、Clippy 和契约检查，**尚未执行特权数据路径**。它准备了：

1. 两轮独立 VPN 会话与显式 Full 启动；校验安装的 MTU/路由和正在运行的 Full 预览，即使传入无效参数及 Campus 仍须返回实际运行配置。
2. 独立物理 IPv4/IPv6 DIRECT TCP/UDP、64 KiB TCP 与 FIN；校园 signed UDP 和双栈 TCP；同时要求实际 TUN 决策的 action/outbound/network/port。
3. REJECT 和未授权 SHU-VPN 的双栈 TCP 负例：本地必须有新增原生失败记录，独立 peer 的 TCP accept 账本不得增加；停止 TUN 后再取一次 peer 账本，拒绝迟到的 blocked 服务接入。
4. DNS A/AAAA、UDP/TCP、TC 后 TCP 重试、错误 ID 拒绝。peer 记录查询名、传输、类型、请求/响应 ID 与 TC；SERVFAIL 必须同时有本轮新错误 ID 对端记录，不能用普通上游失败冒充。DNS 映射用例位于拒绝/授权负例之后，避免把映射歧义误作这些负例的原因。
5. `stop_tun` 后 VPN 保持 connected，原 loopback 控制 socket 绑定前后精确相同；每轮任何数据错误仍执行 finish_stop，检查 stopped、cleanup_pending=false 和 egress inactive/无 pending cleanup，然后再开下一轮。

`tool/full_capture_peer.py` 运行在**独立的双栈测试主机**，提供真实 TCP/UDP 回显、DNS 与带随机 token 的观察通道。观察通道在 TUN 前连接并显式绑定物理接口，不作为被测数据流；账本不记录控制连接。TCP 日志说明 accept 已发生，不冒充“没有 SYN”的抓包证明。所有被测流的来源必须等于配置指定的物理源地址。DIRECT UDP 与 blocked TCP 的端口也明确避免相同，防止端口级规则拦截正常 UDP。

## 准备与命令

在可丢弃、可恢复且已配置独立双栈网络的环境中准备 peer；下面地址是示例，必须先实际配置到独立主机。DNS 监听53端口需要该测试环境提供相应权限：

```sh
python3 tool/full_capture_peer.py --ipv4 198.51.100.2 --ipv6 2001:db8:ffff::2
```

保存 stdout 首行 JSON，并添加被测 Windows 物理网卡的 `source4`、`source6` 字段。保留 peer 返回的随机端口/token，不能把 peer 换成本机地址。`dns_port` 必须为53。配置接受 UTF-8，也接受 PowerShell 写出的 UTF-8 BOM。

构建及只读预览：

```powershell
cargo build --manifest-path rust/Cargo.toml --locked --features tun-probe --example desktop_windows_full_smoke
./tool/full_capture_windows_smoke.ps1 `
  -ProbePath rust/target/debug/examples/desktop_windows_full_smoke.exe `
  -InterfaceAlias 'Ethernet' -PeerConfig peer.json `
  -ReportDirectory build/desktop/full-preview -Mode Plan
```

本机只读预览可使用文档地址的配置，不要求 peer 在线；它不证明 peer 连通性。

在已恢复好基线的独立 Windows guest 中，可为 elevated-local 正常两轮准备配套构建：

```powershell
cargo build --manifest-path rust/Cargo.toml --locked --release --features tun-probe `
  --example desktop_windows_full_smoke --bin shuvpn_tun_helper
Copy-Item rust/target/release/shuvpn_tun_helper.exe rust/target/release/examples/
Copy-Item windows/third_party/wintun/amd64/wintun.dll rust/target/release/examples/
./tool/full_capture_windows_smoke.ps1 `
  -ProbePath rust/target/release/examples/desktop_windows_full_smoke.exe `
  -InterfaceAlias 'Ethernet' -PeerConfig peer.json `
  -ReportDirectory build/desktop/full-normal -Mode Elevated
```

运行模式的监督脚本要求 all-compartment 快照，读取失败即拒绝，不能自动降级。普通用户 helper 模式还需要把特权观察者与普通 probe 分开接线：同一普通脚本当前可能无法读取 all compartments，而同一 elevated probe 又不能代表普通用户 helper 路径。此项仍待实现，不能以运行原生 `--run-privileged` 后缺少独立系统见证的日志签收。

## 本批验证与剩余范围

Windows/Linux 新探针契约分别4/3项通过，两端针对该 example 的 Clippy 通过；Python 工具各63项，Windows跳过19、Linux跳过2。peer 的三项检查使用真实 loopback TCP/UDP/IPv6、DNS截断/错误ID及观察通道；Linux Python3.10 与 Windows Python3.12 均通过。最初使用较新 asyncio UDP API 导致 Linux3.10测试失败，已改为线程内有超时的标准 socket 交换，失败日志保留。两端 verify/native CI 接入这些检查，未远端执行。

当前产物是 `tun-probe` 特性的调试探针，不是默认库/Flutter owner/正式安装包的 Full 签收。本批没有修改生产库 API 或 Dart，没有重建完整应用/安装器，也未执行 Rust/Flutter 全量矩阵。当前证据总汇为 `build/desktop/windows-full-driver/verification.json`。

后续还须完成：独立 Windows guest 中的正常数据运行、普通用户 helper 与精确进程句柄身份/退出见证、覆盖变化及外来路由保留、helper 崩溃后恢复重建、DNS歧义与HTTPS、生产 Flutter/owner 自动启动与安装版默认库；既有真实校园、已连接GUI、长期运行、其他Linux桌面/Wayland、远端CI和原计划最终逐项审计继续保留。本机只核验到 WSL2 Ubuntu；不能把 hypervisor 存在当作已拥有可恢复 Windows guest。
