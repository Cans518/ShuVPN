# 连接详情实际窗口验收

第87批，2026-10-18。Linux 的 M4.4 连接详情子范围通过，整体迁移仍为 active。总汇为 `build/desktop/connection-details/verification.json`，其中 `linux_connection_details_scope_passed=true`，整体 `passed=false/partial=true`。本页不将合成协议网关扩大为真实校园登录。

## 改动与来源

生产仅修改三个 Dart 展示文件：连接页历史入口和详情页刷新入口复用已有 `LabelledIconButton`，共用顶栏返回/通知按钮使用同样的明确名称；只读详情的 `SelectableText` 带“字段：值”语义，排除重复的标签语义。布局、点击回调、原生决策解码和业务所有权不变。逐项复核并更新 Dart 展示清单，边界仍为109文件。未改生产 Rust、生成绑定、依赖、字体或许可证。

正式应用、初次夹具与四包来源为488文件，SHA256 `ecc18a75a5edf358ea411f0fadb9c5806cd4bdc518841d6e32762f406c0b4480`；Windows/Linux 两树一致。后续夹具观测竞争修复及新增回归的来源为490文件，SHA256 `0881aa9a16a094d38b33afee999173b7ccd3af787aad2547558ddededbb9ce67`，两树及最终实窗输入一致。两者不能混写。

正式入口为 `lib/main.dart`，夹具为 `tool/connected_desktop_app.dart`。Linux 两次夹具构建均在 finally 中重建正式入口；最终正式 runner、AOT、core/helper、字体/OFL/FontManifest 摘要与本批正式 tar 一致，见 `linux-final-state.json`。正式入口没有导入夹具及新的观测模块。

## 实际窗口与网络结果

`gui-02` 在全新、只有 loopback 的私有网络命名空间中运行，一个真实 Flutter GUI 进程 PID476012、UID1000、CapEff0，使用默认原生库。独立本地协议网关 PID476093 同 UID、同命名空间、为 GUI 子进程，并验证可执行文件摘要及原始 pidfd。账户、计划及私有数据均为本轮合成输入。

通过真实界面历史入口、刷新、展开和返回，逐项核对以下四种实际 Rust HTTP/TCP 结果：

| 情况 | 原生规则 | 期望动作 | 实际出站或错误 |
| --- | --- | --- | --- |
| 合成校园目标 `10.21.0.5:443` | `profile:3`，版本1 | SHU-VPN | 成功 SHU-VPN |
| 私有 loopback 接收端 | `profile:2`，版本1 | DIRECT | 成功 DIRECT |
| `127.0.0.1:19444` | `profile:0`，版本1 | REJECT | 无成功出站，`rejected` |
| 网关资源表外 `127.0.0.1:19445` | `profile:1`，版本1 | SHU-VPN | 无成功出站，`unrouted` |

32次字段核对包含版本、规则ID、来源、策略、期望动作、实际出站、命中原因，以及两类失败的错误代码和原生原因。每个字段来自本次 PID 的唯一 AT-SPI“字段：值”，绑定轮次1及 run_id1。原生数据没有被替换为界面模拟值。详情的成功出站仍只是历史结果，不等同于活跃连接或完整请求完成证明。

DIRECT 独立目标实际接收65536字节，摘要与客户端载荷相同，客户端确认 prelude、完整回显及半关闭 EOF。两个拒绝请求均要求 HTTP403；两个独立监听器在检查窗口内均无待接收连接。这个检查只证明两个受控目标的结果，不是全部流量永不直连的全局证明。

同一 GUI 完成三轮连接、断开、重连，原有 HTTP/SOCKS × IPv4/IPv6 共12条合成校园代理流保持通过。第二轮记录页保持打开时从真实托盘断开，界面显示没有可读原生会话，旧展开字段与目标行消失，刷新 disabled 且无可执行动作，再通过返回按钮离开；第三轮可正常重连。断开后的 Rust 历史仍可能保留在旧会话对象，本项验证的是界面失效与清空，不声称原生历史被擦除。

最后通过宿主托盘菜单退出：GUI、网关均退出0，原始 pidfd 表明网关已退出；owner closed、run_id为空、cleanup_pending=false，TUN stopped、egress inactive且无残留socket，HTTP/SOCKS监听关闭。私有网络及宿主 link/路由/rule 前后相同，GUI/网关/面板/托盘宿主四个本轮 PID 均无遗留，宿主没有 `/opt/shuvpn` 固定安装。

截图已抽看成功SHU-VPN、规则拒绝及断开空记录页；有些详情顶部被滚动裁切，32个语义字段检查不代表所有字段同时处于截图视口，也不代表每个物理指针命中区域均已验证。

## 失败与修复

第一次 `gui-01` 已完成四类详情和断开清空，但第三轮重连在开发夹具报告处失败，`fatal-474062.json` 为 `DroppableDisposedException`，明确保留 `passed=false`。夹具与控制器持有同一 Dart 会话视图；原先逐个 await 读取，第一次读取与后续调用编码之间，控制器可能释放该视图。没有将本次失败归因于原心跳压力问题。

新增开发专用 `connected_desktop_observation.dart`，在第一次 await 前发出完整四个 FRB 读取。当前固定 FRB2.11.1 的 handler 同步编码参数并克隆 Arc，后续视图释放不影响已经发出的调用；报告同时在异步前固定 round/ports，保留错误及 stack。它不保证四个 Rust 观察值在同一瞬间原子读取。错误不会转换成成功的空观察。

回归用可控未完成 Future，启动读取后立刻 dispose，再完成响应：旧顺序读稳定失败，修复后保留完整字段；另验证 null/已释放不读与原错误身份透传。red/green日志和实际旧失败报告都保留，最终 `gui-02` 没有 fatal 文件并完整退出0。独立只读复核未发现本范围新的具体问题，但最终方案、修改与实窗验证由主线程完成。

Windows 后续完整工具测试曾出现一次 DNS UDP 绑定 `WinError10013`。该次未记录实际端口，原因未确认；只读观察的 UDP 排除段为50000–50059，不能据此断言失败端口落在其中。单项附绑定参数的诊断3项通过，未改源码的全套重跑163项通过。原失败保留为 `tools-windows-02.log`；不把重跑绿色抹掉最初错误。

## 验证与候选

初次两端正式 main release、Flutter329/328、analyze、边界109及163项工具测试通过。夹具修复后的两端完整 Flutter 为 Windows332/Linux331，通过；两端163项工具测试通过，平台跳过19/7，analyze及边界通过。Linux 新夹具 release 与恢复正式入口构建通过。未重跑完整 Rust、TUN/Full、长跑或远端 CI。

本批候选保存在 `build/desktop/connection-details/package/`，旧86/83/77候选保持原样：

| 产物 | bytes | SHA256 |
| --- | ---: | --- |
| Windows ZIP | 35407993 | `a7626451143f922605dcc195822420e6dfb3a480aa6b1afb0e2a4732f70eded7` |
| Windows Inno | 28407161 | `a8c6a441a97ed8a62b9737a41d256f090303dd4e1c2ea8a07622b00a0940d637` |
| Linux tar.gz | 34991712 | `f47e5ce7e23c46cd9a7a42629d41c65db94c4dac736ddb405068eb8be77b4773` |
| Linux DEB | 28056400 | `8d086b85afe5879cc199836ac7213757dbbe81848257c280356f5eb9d091a895` |

ZIP39、tar32、DEB安装清单33个文件均逐项核对，清单文件本身另计；DEB的 README、distribution清单及dpkg标记按安装格式单独验证，所有应用载荷与tar一致。core/helper摘要与86批相同。字体仍为未修改的霞鹜文楷屏幕阅读版，TTF/OFL/FontManifest字节与77批一致。Windows Python不能直接读DEB的zstd数据，改用WSL `dpkg-deb --fsys-tarfile` 只读解析；首轮误把DEB的分发元数据要求与tar完全相等，核对打包器后分别验证，没有删掉实际应用载荷校验。

Inno由已验证ZIP、固定编译器及同来源生成，外层摘要与输入链核对；未独立解码其内嵌载荷，本批也未实际安装新Inno/DEB，不能套用86批安装GUI证据为87批新包签收。详见 `package-audit.json` 和 `audit_packages.py`。

Windows只读确认前台仍为 HWND5639302、Windows安全中心、`Shell_SystemDialog`、PickerHost.exe PID63020；未输入、未代选系统允许/取消、未重试启动GUI。Windows连接详情实窗仍待系统弹窗由用户处理后继续。

M4.4真正的指针拖动尚未完成：既有测试中的跨页移动按钮、原生move与禁用拖拽柄断言不是实际拖动手势。本页不签收该项。真实校园、Windows正式/安装GUI、可恢复guest Full、更长运行、其它桌面/Wayland、原84心跳及WSL绑定机制、远端CI和最终审计继续保留；无提交、签名或发布。
