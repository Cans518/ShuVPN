# 开发与验证

## 工具

- Node.js 24。
- Rust 1.96.0，版本与组件由 `rust-toolchain.toml` 固定。
- Python 3.10 以上，用于验证和打包。
- Windows：MSVC C++ 工具链和 WebView2 Runtime。
- Linux：WebKitGTK 4.1、GTK 3、Ayatana AppIndicator 开发包。

```bash
sudo apt-get install --no-install-recommends libwebkit2gtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev pkg-config
npm ci --prefix desktop
cd desktop
npm run tauri -- dev
```

## 模块

| 目录 | 职责 |
| --- | --- |
| `desktop/src` | 页面、主题与交互；`main.ts` 协调启动、导航、轮询和全局反馈 |
| `desktop/src-tauri` | 本地 commands、窗口、托盘、自启动与退出清理 |
| `crates/core` | 账户、认证、连接、路由、系统代理与 TUN；`tests` 和 `examples` 放核心测试及平台验收入口 |
| `packaging` | 平台安装模板与安装事务 |
| `tool`、`tests/tool` | 构建、验证、打包工具及其测试 |
| `third_party/sangfor` | 固定协议快照、上游来源、摘要与许可 |

前端通过宿主公开的 commands 访问核心，不接触认证 Cookie、连接密钥或原始隧道计划。网络策略与认证规则由核心决定，核心不依赖 Tauri。页面逻辑由各自的 `*-page.ts` 管理，账户请求和二维码资源由账户控制器管理；导航离开账户页时须等待取消工作完成。

设置提交与连接启动共用配置锁。规则提交必须携带预览标识，迟到结果不得覆盖较新的编辑或预览。明确退出时先取消账户与连接工作，再排空 commands、等待网络清理并关闭存储；清理失败时保留应用及错误提示，允许重试。

在仓库根目录运行 Rust 命令。第一方包共用根 `Cargo.lock` 与 `target/`：

```bash
cargo fmt --check
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
cargo build --locked -p shuvpn_core --bin shuvpn_tun_helper
```

用 `-p shuvpn_core` 或 `-p shuvpn_desktop` 选择包。Tauri CLI 仍从 `desktop` 启动，编译产物也进入根 `target/`。第三方协议快照保留独立 workspace，不能把它的锁文件合并到第一方锁文件，也不在第一方格式整理时重排上游源码。

提交要求见[贡献指南](../CONTRIBUTING.md)。

## 验证

Windows：

```powershell
./tool/verify_desktop.ps1 -GuiSmoke
```

Windows 缺少 WebView2 Runtime 时可运行 `tool/install_webview2.ps1`，脚本会验证 Microsoft 安装程序签名后安装。CI 在窗口验收前自动检查运行时。

Linux 需额外安装 `libcap2-bin`、`iproute2`、`python3-gi`、`gir1.2-atspi-2.0`、`at-spi2-core`、`xvfb`、`xdotool`、`xauth`、`x11-utils`、`dbus-x11`：

```bash
SHUVPN_GUI_SMOKE=1 SHUVPN_NETWORK_SMOKE=1 bash tool/verify_desktop.sh
```

脚本覆盖前端构建、页面控制器测试（`npm test --prefix desktop`）、格式、Clippy、Rust tests 和打包工具测试。控制器测试在 Node.js 24 中直接加载 TypeScript，仅替换 IPC 和 DOM 边界；实际 DOM、布局及宿主交互仍由窗口验收覆盖。核心 Linux 测试在独立网络命名空间运行；实际窗口验收使用 WebView2 CDP 或 WebKit/AT-SPI。`SHUVPN_NETWORK_SMOKE=1` 额外调用 `tool/verify_network_linux.sh`，执行隔离的 TUN、DNS、data plane、egress 和 helper 清理测试；也可单独运行该脚本。普通验证可省略两个 smoke 环境变量。CI 与发布复用同一套平台入口和网络验收脚本。

验收使用临时账户目录和本地 TLS 网关，不使用真实校园凭据。报告写入被 Git 忽略的 `build/`。在有交互式 Explorer 托盘的 Windows 桌面，可以为 `tool/tauri_windows_smoke.mjs` 追加 `--native-tray`，验证实际托盘菜单；该模式短暂使用鼠标。

真实校园认证、宿主系统代理、特权 TUN 与安装器安装/升级/卸载属于平台验收，应在报告中与离线测试分开列出。修改对应模块时执行 `tool/tun_*` 及平台安装测试，不能用构建成功代替运行验证。

## 独立配置

正常数据目录由应用标识 `org.shuosc.shuvpn.tauri` 决定。调试或验收时可以使用独立绝对路径：

```bash
shuvpn --data-dir /absolute/path/to/private-profile
```

该选项为不同目录提供独立实例标识，避免影响正常用户实例。不要把真实账户数据放入测试目录或版本库。

## 正式构建

正式构建、版本一致性和分发包检查见[发布指南](releasing.md)。普通本地打包命令可生成包含源码状态的开发快照；对外发行必须使用带正式门禁的发布命令。
