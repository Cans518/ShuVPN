# ShuVPN

[![Release](https://img.shields.io/github/v/release/shuosc/ShuVPN?color=1677ff&label=release)](https://github.com/shuosc/ShuVPN/releases)
[![Downloads](https://img.shields.io/github/downloads/shuosc/ShuVPN/total?color=1677ff&label=downloads)](https://github.com/shuosc/ShuVPN/releases)
[![License](https://img.shields.io/badge/license-AGPL--3.0-1677ff)](./LICENSE)
[![Flutter](https://img.shields.io/badge/Flutter-3.47.2-02569B?logo=flutter&logoColor=white)](https://flutter.dev)

专为 SHUer 设计的一站式校园VPN服务软件！

当前整理结果、已验证范围和未完成项见[项目状态](docs/project_status.md)。

- 校园账户 账密/企业微信登录支持
- 当前桌面正式入口支持 aTrust；EasyConnect、OpenVPN 尚未启用
- 按照各平台特性设计的多种接入方式,包含 HTTP 代理、SOCKS5 代理与桌面校园分流

## 安装

### Windows / Linux
项目仅支持 Windows 与 Linux 桌面版，不再开发或移植 Android 版本。
已有 x64 本地候选包，正在完成权限、安装和校园环境验收。请保留完整目录运行，不能只复制主程序。
Windows 可生成按当前用户安装的 Inno Setup 安装器；Linux 可生成面向 Ubuntu 22.04 或兼容 Debian 系统的 DEB。关闭应用后再安装、升级或卸载。Windows 从系统“已安装的应用”卸载，用户账户与设置保留。

Linux DEB 使用 `sudo apt install ./ShuVPN-版本-linux-x64.deb` 安装或升级，使用 `sudo apt remove shuvpn` 卸载。GUI 无需 root。需要 TUN 时，由管理员执行 `sudo usermod -aG shuvpn YOUR_LOGIN_NAME`，然后退出登录并重新登录；包不会自动授权所有用户。卸载和 purge 均保留用户数据及授权组成员关系。不要用便携版 `install.py` 覆盖 DEB 管理的安装；从旧自定义安装迁移时先用其卸载入口移除程序文件，账户数据会保留。
已验证范围和剩余项目见[实施记录](docs/desktop_migration_progress.md)。

如果您遇到了本应用中不符合预期的行为，欢迎先查看已有的 [Issues](https://github.com/shuosc/ShuVPN/issues)，也可以[新建 Issue](https://github.com/shuosc/ShuVPN/issues/new/choose) 反馈问题或提出建议。修复问题或新增功能时，欢迎提交 Pull Request。

## 编译说明

本应用使用 [Dart](https://dart.dev/) 和 [Flutter](https://flutter.dev/) 开发。

当前验证固定使用 Flutter 3.47.2；升级工具链后需重新执行验证矩阵：

```shell
Flutter 3.47.2 • channel stable • https://github.com/flutter/flutter.git
Framework • revision d3b14c8769 (5 weeks ago) • 2026-08-26 16:07:51 -0700
Engine • hash 1cf1c4773fb941c4c74a7f8bb144a8837596c0f4 (revision a804b26164) (1 months ago) • 2026-08-26
18:46:13.000Z
Tools • Dart 3.13.2 • DevTools 2.60.0
```

为了构建本应用，您需要[下载](https://flutter.cn/docs/get-started/install)并安装 `Flutter SDK`，将 `flutter` 加入 PATH；

Windows 构建需要 Visual Studio 桌面 C++ 工具链；Linux 构建需要 GTK、CMake、Ninja 和 Clang。两端都需要 Rust 工具链。

确定配置正确后，在项目根目录执行：

```shell
flutter pub get
flutter devices
flutter run -d <device-id>
```

完整测试包含真实 FFI，需要先构建本机 Rust 库：

```shell
cargo build --manifest-path rust/Cargo.toml --locked --release --lib --examples
flutter test
```

Windows 在 Cargo 库目录执行真实 FFI 测试时，还需将 `SHUVPN_WINTUN_DLL` 设为仓库中 `windows/third_party/wintun/amd64/wintun.dll` 的绝对路径。正式 Windows bundle 会将固定版本驱动安装到原生库旁；验证该 bundle 时应清除这个环境变量，以确认随包驱动可被发现。权限测试只在非管理员进程中尝试创建，以验证请求在修改系统前被拒绝；它不证明实际网卡和路由成功路径。

Windows bundle 已包含独立 `shuvpn_tun_helper.exe`，正式会话接入受限本机 IPC 与 runas 启动。管理员或配套 helper 就绪时开放桌面 TUN；普通用户的正式入口与默认库 FFI 已通过本地校园/直连 fixture 数据及停止清理验收。与其他 VPN 同时运行的稳定性、真实校园及完整 GUI 操作仍需独立验收，实际日志范围见[实施记录](docs/desktop_migration_progress.md)。

Linux 高层会话已接入独立 `shuvpn_tun_helper`，普通用户的正式 TUN 入口、校园 IPv4 UDP/双栈 TCP、DIRECT 双栈数据、DNS/HTTPS、停止恢复及重启已在隔离内核环境验证。随包安装器、固定路径授权组、正式包内默认库的权限检测已通过隔离安装/升级/卸载验收；完整 GUI 数据面和校园运行仍在验收。GUI 主程序保持普通用户权限，不直接对 GTK 可执行文件设置 capabilities。设计、复现命令与当前限制见[Linux 桌面权限边界](docs/linux_desktop_permissions.md)。

Linux 解压完整包、退出应用后，可执行 `sudo python3 ./install.py --user YOUR_LOGIN_NAME`，需要 Python 3 和 `libcap2-bin`。安装到 `/opt/shuvpn`，仅为独立 helper 授予网络权限并通过 `shuvpn` 组授权指定用户。重新登录后从应用菜单启动。卸载使用 `sudo python3 /opt/shuvpn/install.py --uninstall`，保留用户账户/设置数据与组成员关系。

### Rust 核心与桌面开发构建

Rust 核心和 Windows/Linux 构建正在按[迁移计划](docs/desktop_migration_rust_core_plan.md)实施，实际完成范围与验证记录见[实施记录](docs/desktop_migration_progress.md)。桌面 aTrust OAuth2、资源获取/解析、anti-MITM 校验、Rust L3 控制、HTTP/SOCKS5、规则分流、窗口/托盘与系统 TUN 已有实现，Inno/DEB安装格式也已接入；完整平台、GUI、校园和长期验收仍在进行。

最终目标为保留 Flutter 前端、后端业务全部由 Rust 实现，仅覆盖 Windows/Linux。账户、配置、公告、连接所有权及登录自动阶段已进入 Rust，旧 Dart 请求、解析、存储和 SDK 后台调用链已删除；Dart 保留界面、输入、展示状态与薄桥接。全项目的依赖、正式运行和最终平台验收仍在进行，具体范围见[后端迁移审计](docs/backend_rust_migration_audit.md)。

桌面正式账户使用 Rust AccountSession，统一拥有密码/两步验证、Cookie、取消代次、稳定设备号和账户展示快照。启动时原子采纳旧账户数据，提交成功后清理三个旧偏好键；退出清会话，close 保留已提交记录。底层 NewssoSession 仍供独立原生接口与探针使用。两端正式库、本地 HTTPS fixture 和跨进程存储已有验证，真实学校账号仍需单独验收。

验证码的默认方式、发送间隔和发送受限后的替代方式由同一 Rust 账户决定。每种方式在请求发出前预留 60 秒，失败或取消也保留等待，以避免请求已到达服务端时立即重复发送；切换方式、返回表单或重新登录不会清除该账户实例的等待。发送受限只选择另一种可用方式，仍须用户点击发送。页面定时读取原生剩余秒数；等待不写入磁盘，也不替代学校服务端的频次限制。

网关输入由 Rust 补全和校验 HTTPS 地址，保留显式端口；不接受其他协议、内嵌凭据或片段。代理监听所有接口时，原生核心按可用网卡及系统默认接口提供同地址族的地址提示，HTTP 与 SOCKS 分别显示实际绑定和端口。IPv6 使用 `[地址]:端口`；未找到合适地址时保留通配监听地址并记录原因。地址提示不改变监听范围或规则，也不证明其他设备已经能穿过本机防火墙访问。

桌面连接入口现在由原生完成代理恢复，并在创建会话时挂接规则所有者；连接后由一次原生命令编排 HTTP、系统代理、SOCKS 和可选 TUN。各服务返回实际状态与独立诊断，TUN 权限不足时保留已可用的代理，TUN 启动失败的回收也由原生处理。Linux 新入口已通过隔离安装环境中的普通用户 helper、DNS/动态路由/TCP 数据面和关闭清理验证；GNOME 与 KDE 5 私有配置中的系统代理和恢复也已验证。完整 GUI 操作、Windows 最新特权路径、真实校园及长期运行仍待验收。

会话现在由进程内的 Rust 连接所有者持有。释放 Flutter 视图不会丢失会话；重连必须先确认旧资源清理完成，失败会保留原生句柄并阻止替换。退出由原生等待在途建立和清理任务结束，新前端接管后旧前端的控制请求会被拒绝。代理重绑也由原生执行，只更新已运行的指定监听，并按保存的系统代理开关应用实际新端口。

桌面扫码、密码与验证码登录均由同一 Rust 账户完成认证和凭据获取，只有真实用户输入边界才返回验证挑战。前端只接收图片、唤起链接、进度和结果，授权码留在 Rust。页面退出取消未完成操作，已完成页面的退场不会取消随后开始的操作；凭据全部失败明确报错并保留已接管的 SSO。

教务/OTP OAuth、数字码和身份资料、aTrust 换票/环境上报/核对均共用 Rust 账户 Cookie jar，回调码和签名密钥不进入展示 DTO。桌面连接消费一次性不透明准备计划，跟随账户代次取消；业务授权、资料读取和实际隧道各有独立成功条件。

桌面公告的请求、HTML 解析、缓存与预取调度由 Rust 持有；更新查询也有 Rust 薄适配，正式桌面发布渠道验收前暂不启用自动更新。安装版查询更新时只选择对应安装格式，缺少该格式则转到发布页，不提供便携文件覆盖包管理器的安装。桌面设置使用独立 Rust 存储、schema 迁移和校验，成功采纳后只清理旧 `settings.*`；账户和证书偏好保留。页面等保存确认后才导航或重绑代理，失败保留已提交快照。

连接准备、签名密钥/SID及计划留在 Rust，连接与账户代次绑定。另有建立并停止临时 L3 连接的可用性验证。正式登录和连接使用同一账户句柄；旧设备号、SSO和展示快照经native提交后清理旧键，后续写入原生记录。当前验收及限制以实施记录最新批次和验证清单为准。

桌面连接由原生认证后端刷新会话、获取资源并生成隧道计划，再交给 Rust 建立连接。anti-MITM 沿用现有兼容上报策略：挑战或签名校验失败记录警告，但仍保留网关公布的节点证书固定；原生接口另支持严格失败模式。HTTPS 认证请求仍使用正常证书校验。

当前源码的桌面“规则分流”提供配置档与规则卡片、前置/后置规则新增、拖动或按钮排序、启停、策略组选择，以及 TCP/UDP、HTTP/SOCKS/TUN 入站的规则测试器。YAML/JSON 导入位于“导入与高级编辑”；可视化修改由 Rust 解析并转换为 JSON，保留规则集和策略组字段，不保留 YAML 注释与排版。大列表每页显示25条规则，可使用上移/下移跨页调整。重新进入页面会清除上一份草稿的预览。

编辑只修改草稿，必须“校验并预览 → 应用预览”才改变新连接的路径，已有连接保持原决策。已有配置保留严格校园代理语义；手动应用“校园优先”模板后，获授权校园目标走 SHU-VPN，其余目标走 DIRECT。规则选择校园隧道不能扩大网关授权，拒绝或隧道失败也不会自动改为直连。HTTP/SOCKS5 与桌面 TUN 共用规则，TUN 只能执行已进入捕获范围的流量；规则测试器只表示规则意图。

“网络连接 → VPN 捕获模式”默认保持校园分流，可在断开后选择实验性完整捕获。完整捕获将可路由 IPv4/IPv6 单播和 DNS 交给原生规则，保留本机、控制节点、回环、链路本地与组播等排除范围；进入 TUN 不代表送往校园。查看接管计划会显示原生模式、DNS 上游和排除地址，运行中返回当前计划。Linux 安装版的默认库与固定路径 helper 已在隔离环境完成普通 UID、CapEff=0 的完整捕获验收，包含双栈数据、DNS/HTTPS、拒绝/授权边界、覆盖变化、helper 崩溃与恢复；详见[安装版完整捕获验收](docs/linux_installed_full_capture_acceptance.md)。该验证使用独立测试对端，真实校园、长期运行与 Windows 特权完整捕获仍待验收。当前源码能力不代表旧归档包含相同实现，实际产物范围以实施记录为准。

旧字符串规则继续兼容；停用条目以对象保存，预览时仍校验其语法、策略和规则集引用。最终 `MATCH` 必须唯一且启用，不能删除或移动。启停对象属于新的配置扩展，旧程序无法读取该格式；配置版本回退在本轮程序内执行。示例：

```yaml
rules:
  - rule: DOMAIN,blocked.example,REJECT
    enabled: false
  - MATCH,DIRECT
```

file/HTTPS 规则集可设置 `interval`（秒）自动更新，也可在规则页手动更新当前生效配置。刷新保留用户覆写，失败时旧规则继续运行；内容不变不会产生新版本。生效版本发生变化后，旧编辑预览需重新校验。

规则卡片会指出被更早规则遮蔽的相同条件文本，预览最多列32条重复说明并附汇总。这只比较已启用规则的同文本条件与解析标记，不是完整的正则/逻辑等价或子集分析；不会自动删除、重排或改变任何规则。

桌面连接页右上方“连接记录”显示原生核心最近最多128条出站结果，包括命中规则/版本、期望动作、成功出站、失败或拒绝原因及DNS证据。它是会话历史快照，不是活动连接列表；同一流可能有多个结果，成功记录不代表请求已完成，也不覆盖所有捕获前失败。记录不带绝对时间、连接ID或最终选中peer，因此界面不会生成这些字段。断开/更换会话会清空旧快照，刷新失败保留并标注上次结果。

规则现支持 `AND` / `OR` / `NOT` 和 `DOMAIN-REGEX`，也可用于 classical 规则集；组合叶子可以引用已编译规则集，但 classical 载荷不允许继续嵌套规则集。示例：`AND,((DOMAIN-SUFFIX,shu.edu.cn),(NETWORK,tcp)),SHU-VPN`。未知 IP 条件保留“需要解析”状态，NOT 不把它当作不命中；已确定的 AND 假或 OR 真无需为其他未知 IP 条件查询。`no-resolve` 仍仅用于 IP 叶子和 RULE-SET，并向规则集内条件继承。

域名正则匹配归一化的小写 IDNA ASCII 名称，IP 字面量不作为域名。使用无回溯的 Rust regex，明确拒绝环视/回溯引用；表达式最多 1024 字节、每次编译最多 128 个正则、单个编译程序和 DFA 缓存各限制 256 KiB。逻辑表达式最多嵌套 16 层，配置/规则集编译及展开受节点预算约束。规则页从核心读取支持与未支持清单，任意脚本仍未启用。

开发源码已接入 `PROCESS-NAME` / `PROCESS-PATH`，需要配套重新构建，旧候选不因此自动获得支持。规则匹配可执行映像的文件名或绝对路径，不匹配脚本名称或命令行，也不展开通配符。Windows 忽略大小写并统一路径分隔符，Linux 区分大小写。名称或路径含逗号、括号等分隔字符时，用 JSON 字符串作为值，例如 `PROCESS-NAME,"client[1,work.exe",REJECT`；Windows 路径可使用 `C:/Apps/client.exe`。这些条件也可用于逻辑表达式和 classical 规则集。

实际流量由 Rust 查询本机套接字的当前可见进程快照，连接记录显示 PID、名称、映像路径及依据；预览中的进程输入只是模拟值。HTTP/SOCKS 使用入站客户端 TCP 地址，TUN 使用原始 TCP/UDP 地址，未进入捕获范围的流量仍无法执行规则。远端客户端、无可见归属、已退出进程、可见共享歧义及查询预算不足时，仍需要该身份的规则会明确失败，不会跳过后直连。IPv4 映射连接会查询双地址族表；跨地址族 UDP 通配绑定无法判断时也拒绝归属。

快照不能排除权限不可见的共享持有者，不是进程认证或历史发送者证明。查询使用两个工作线程和包含排队的500毫秒协作预算，调用者等待另有250毫秒余量；底层读操作被取消后可继续收尾，直到释放工作槽。专用校园 DNS 捕获请求走独立解析流程，不参与普通出站进程规则。应用新规则仍只影响新流；降级到旧程序前须先回退包含新语法的规则配置，不能只替换二进制。

`GEOIP` / `GEOSITE` 支持版本和 SHA-256 固定的 V2Fly protobuf `.dat` 文件，经 file 或 HTTPS 规则源加载。类别名不区分大小写；GeoSite 的 `@属性` 按键存在性筛选，多个属性为 AND，不按 bool/int 值比较。数据源中公布的 `file_path` 不触发额外文件读取。示例中的版本由用户标记，SHA-256 必须替换为实际文件摘要：

```yaml
rules:
  - GEOSITE,google@ads,REJECT
  - GEOIP,CN,DIRECT,no-resolve
  - MATCH,SHU-VPN
rule-providers:
  sites:
    type: file
    behavior: geosite
    format: dat
    path: geosite.dat
    data:
      code: google@ads
      version: "本地数据版本"
      sha256: "替换为文件的64位SHA-256"
  ips:
    type: file
    behavior: geoip
    format: dat
    path: geoip.dat
    data:
      code: CN
      version: "本地数据版本"
      sha256: "替换为文件的64位SHA-256"
```

GeoIP 使用双栈前缀索引，GeoSite 使用完整域名/后缀集合、关键词自动机和受限正则；一次配置最多编译 500,000 个数据条目及 32 MiB 选定内容，原文件最多 64 MiB。版本和摘要在规则页显示。源内容错误或不匹配时保持有效快照，重启可恢复经校验的派生缓存；版本更新仍经预览后应用。数据类别匹配不能扩大校园资源授权。

需要目标 IP 时，校园名称经获授权的 TCP DNS 隧道解析，失败不改用系统解析；DIRECT 使用系统解析。核心记录 DNS 来源、响应 TTL 和规则版本，按来源与版本隔离校园缓存。系统接口未提供的 TTL/解析器信息标为未知。桌面 TUN 开关已接入原生能力查询；规则命中不等于所有系统流量已经被接管。

Rust 原生接口已提供 Linux TUN 的创建/撤销及 TCP/UDP 数据面：TCP 采用双栈本地终止，复用校园签名 TCP 通道或绑定所选出口的真实 DIRECT socket；校园 IPv4 UDP 逐流签名授权后经 L3 转发，DIRECT UDP 也支持双栈。REJECT 不拨号。独立网络命名空间已验证实际内核路径、半关闭、数据往返和撤销，正式入口使用受限 helper。完整捕获及复杂数据面能力仍需独立验收，不能由局部探针结果推断全部支持。

Linux TUN 启动时，原生核心采用进程共享的出口保护：已有控制连接与新建出站 socket 绑定指定接口，原生认证及规则集下载按当前出口创建 HTTP 客户端。TUN 计划可提供 `physical_dns_servers`，只接受明确的非 loopback 上游；缺省读取系统上游配置，无法取得上游时域名查询明确失败。DNS 使用绑定出口的双栈 UDP 查询，截断后转 TCP，不回落到系统 stub。此保护不等于接管 OS DNS，也不覆盖尚在迁移的 Dart SDK/WebView 网络请求。

Windows 高层 TUN 与 runas helper 已有实现和正式接线；默认 DNS 仅读取所选物理接口的非 stub 上游。最新特权路径、复杂共存场景和完整捕获仍需独立验收。开发探针与两端回归证据见实施记录，不能把 socket 绑定测试作为全部网卡、路由和恢复路径已经通过的证明。

实验 TUN 计划设置 `dns_capture: true` 后，配置中的获授权校园 DNS 地址会提供 UDP DNS 入站，经校园签名 TCP DNS 查询取得有效回答。域名资源的正 TTL 地址在 DNS 回复前安装双栈主机捕获路由；后续流恢复域名以执行规则和资源授权。过期、配置替换和停止会撤销无流租约的路由，已有流保持捕获至结束；共享 IP 的多域名身份会明确拒绝猜测。该接口尚未修改 OS DNS 配置，客户端须自行使用校园 DNS 地址。

桌面有可用托盘时，关闭按钮隐藏窗口并保留连接；从托盘可显示窗口或退出，Ctrl+Q 也会退出。退出会先断开连接。Linux 没有可用托盘面板时，关闭窗口直接断开并退出。

Windows、GNOME/dconf 和 KDE 桌面的“网络连接”页提供默认关闭的“连接时启用系统代理”。它会启动本机 HTTP 监听，将系统 HTTP/HTTPS 代理指向实际端口；应用流量仍遵循当前规则，原有严格校园配置不会因此自动改成公网直连。需要 IPv4 本机可访问的 HTTP 监听。只有支持系统代理的应用会采用这些设置，这个开关不等于 TUN。

停止 HTTP、断开和退出前会恢复原代理设置；修改前保存的恢复记录也用于下次启动和重连时清理异常退出的配置。检测到外部端点变更时保留新端点及其模式；仅排除项被更改时，保留新排除项并恢复本应用端点。恢复失败会显示错误并保留记录。Windows 使用当前用户的默认 LAN WinINet 设置；GNOME 保留原 PAC 地址、认证字段和未设置值。KDE 使用 `kreadconfig6/5`、`kwriteconfig6/5` 和 `dbus-send`，保留 PAC URL、其他协议代理及未设置值，并向 KIO 发送配置刷新信号。管理员锁定项会报错；代理字段使用展开/删除标记时会明确拒绝，避免无法恢复。其他 Linux 桌面尚未接入。

桌面构建需要 Rust 1.91 或更新的稳定工具链。Windows 还需要 Visual Studio 的桌面 C++ 构建工具；Linux 需要 Clang、CMake、Ninja、pkg-config、GTK 3 和 libayatana-appindicator3-dev 开发包。Flutter 联合构建会调用 Cargo 并将原生库放入产物目录：

```shell
flutter pub get --enforce-lockfile
flutter build windows --release
# 在 Linux 主机上执行：
flutter build linux --release
```

本次交付范围为 Windows 与 Linux。完整桌面验证与打包入口如下，需要 Python 3.10 或更新版本以及已配置的 Flutter/Rust 工具链：

```powershell
# Windows；若 python 是 Microsoft Store 占位命令，通过 -PythonPath 指定实际解释器。
./tool/verify_desktop.ps1
```

```shell
# Linux，在原生 Linux 或 WSL 的独立源码快照中运行。
bash tool/verify_desktop.sh
# 同时执行隔离网络测试与隔离安装验收（需要 sudo、unshare 和 overlayfs）：
SHUVPN_RUST_TEST_NAMESPACE=1 SHUVPN_LINUX_INSTALL_NAMESPACE=1 bash tool/verify_desktop.sh
```

脚本将完整 bundle 归档到 `build/desktop/<时间>-<平台>/`，Windows 为 ZIP、Linux 为 tar.gz，同时输出逐文件清单与 SHA256。随后解压归档，使用解压目录内的 Rust 库运行真实 FFI、设置/账户/SSO存储与 Flutter 测试。脚本失败即停止，已有旧包或归档成功不代表本次功能验收通过。当前交付进度及剩余平台功能见实施记录。

在已验证的完整归档上生成正式安装格式：

```powershell
$compiler = ./tool/install_inno_setup.ps1
python tool/package_installers.py <windows-x64.zip> --output build/desktop --iscc $compiler
```

```shell
python3 tool/package_installers.py <linux-x64.tar.gz> --output build/desktop
```

Windows 使用固定摘要和签名核对的 Inno Setup 7.1.0 便携编译器；DEB 使用本机 `dpkg-deb`。安装器的 SHA-256、输入归档摘要、负载来源和构建工具来源写入同名 `.json`。Windows 安装器尚未加入项目发布证书签名。CI 已配置生成和安装烟测，不代表远端 CI 或正式发布已经通过。

独立验证核心和真实 FFI：

```shell
cargo test --manifest-path rust/Cargo.toml --locked
cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings
cargo build --manifest-path rust/Cargo.toml --locked --release
dart run tool/rust_bridge_smoke.dart
```

修改 `rust/src/api/` 后，需要重新生成并提交 Rust/Dart 两侧绑定。生成器与两侧运行库固定使用 `2.11.1`：

```shell
cargo install flutter_rust_bridge_codegen --version 2.11.1 --locked
flutter_rust_bridge_codegen generate --config-file flutter_rust_bridge.yaml
```

Windows 也可使用 `./tool/generate_rust_bridge.ps1` 检查生成器版本并生成绑定。

## 许可证

[AGPL-3.0](./LICENSE)

## 致谢

本项目参考研究了以下项目，感谢各位原作者。

- [LXGW WenKai Screen](https://github.com/lxgw/LxgwWenKai-Screen) —— 界面使用霞鹜文楷屏幕阅读版 v1.522，字体与原始 SIL OFL 1.1 许可证随应用打包；来源与校验摘要见 [字体说明](assets/fonts/README.md)。
- [zju-connect](https://github.com/Mythologyli/zju-connect) —— 提供了本项目的灵感，其 aTrust / EasyConnect 协议实现是登录与隧道链路的参考。
- [flutter_sangfor](https://github.com/TsinbeiLabs/flutter_sangfor) —— 本应用复用其 Rust 协议组件，不再加载旧 Dart SDK 后台。
- [shu-sso-poc](https://github.com/preca-hoshino/shu-sso-poc) —— 上海大学教务系统登录认证链路分析。
- [shu-otp-poc](https://github.com/preca-hoshino/shu-otp-poc) —— 上海大学 OTP 令牌系统定时获取的链路分析。
