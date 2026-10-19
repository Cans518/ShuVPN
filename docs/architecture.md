# 桌面架构

ShuVPN 使用 Tauri 2、TypeScript 和 Rust。Windows 与 Linux 共用同一套连接核心。

## 模块边界

| 目录 | 职责 |
| --- | --- |
| `desktop/src` | 页面、主题、键盘交互与状态展示；Vite 打包本地 CSS、字体和 SVG 图标 |
| `desktop/src-tauri` | 本地 commands、窗口与托盘、自启动、主题持久化和退出屏障 |
| `rust/src` | 账户、认证、凭据、连接 owner、TCP/UDP/DNS、路由、系统代理与 TUN |
| `rust/tests`、`rust/examples` | 核心集成测试、协议夹具和平台权限验收入口 |
| `third_party` | 固定版本协议模块，保留来源、许可和校验清单 |
| `packaging` | Windows/Linux 安装模板与安装事务 |
| `tool`、`test/tool` | 构建验证、真实窗口验收、载荷打包与工具测试 |

前端只调用宿主公开的 commands，不接触认证 Cookie、连接密钥或原始隧道计划。网络策略和认证规则由 Rust 核心决定。宿主将核心状态转换为展示所需的 JSON。

## 配置与数据

应用标识为 `org.shuosc.shuvpn.tauri`。默认数据目录由平台路径 API 解析，包含 `account`、`settings`、`routing`、`system-proxy`、`webview` 和 `appearance.json`。测试通过绝对路径 `--data-dir` 隔离账户与配置；启动项由操作系统按当前用户管理。

网络设置与连接启动共享配置锁。连接启动时使用核心校验后的同一份配置；已有连接、连接中或清理中的重复启动会被拒绝。主题配色独立存储，允许连接过程中切换。主题文件只接受固定字段与十六进制颜色，采用同目录临时文件写入并原子替换。

外观变更不改写账户和网络配置。需要回退外观实现时可恢复对应代码并保留 `appearance.json`；删除该单独文件会恢复内置默认配色。启动登记与程序文件分开管理，卸载 Windows 安装版本时移除当前用户的启动登记。

规则提交必须携带本次预览标识。编辑、模板选择和导航各自维护修订，迟到结果不得覆盖后续操作。丢弃旧预览不会删除较新的预览；无效规则不会替换当前生效配置。

## 生命周期

关闭主窗口只隐藏窗口，保留连接与托盘。托盘左键恢复窗口，右键提供连接、取消、断开、代理配置、账户、规则、记录、设置、自启动与退出。状态图标直接订阅原生连接 owner，在前端隐藏时继续更新。

再次启动时通过 single-instance 插件恢复已有窗口。`--data-dir` 配置使用独立实例标识，避免开发或验收进程影响正常运行的用户实例。

自启动登记当前可执行文件并携带 `--hidden`，在用户登录桌面后进入托盘；不自动建立 VPN 连接。Windows 以注册表事务写入当前用户启动项并为程序路径加引号，Linux 按 XDG 配置目录写入 Desktop Entry，转义路径中的空格与特殊字符。托盘创建失败时显示窗口，后续关闭改为最小化，保留任务栏恢复入口。

明确退出会先取消账户与连接工作，再排空在途 commands，等待 owner 清理网络并关闭存储。清理失败会显示窗口和错误，保留应用以便再次尝试。安装器在应用运行时拒绝覆盖安装目录。

## 验证

宿主 `tests/ipc.rs` 验证真实 command handler、输入约束、配置互斥、预览身份、关闭排空与跨进程恢复；`local_connection.rs` 以本地 TLS 网关验证 HTTP/SOCKS5 字节、真实原生状态、重连及监听器释放。

Windows WebView2 与 Linux WebKit 验收覆盖界面导航、配置持久化及窗口生命周期。核心 namespace 场景验证 TUN、DNS、egress 和 helper 清理。验证命令与平台验收边界见[开发指南](development.md)；发行门禁见[发布流程](releasing.md)。
