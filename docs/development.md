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

`desktop/src` 是界面，`desktop/src-tauri` 管理桌面宿主，`rust` 提供网络核心，`packaging` 放平台安装模板。`tool` 与 `test/tool` 管理开发验证和打包契约。协议依赖的来源、摘要和许可见 `third_party/sangfor`。

架构约束见[架构](architecture.md)，提交要求见[贡献指南](../CONTRIBUTING.md)。

## 验证

Windows：

```powershell
./tool/verify_desktop.ps1 -GuiSmoke
```

Linux 需额外安装 `libcap2-bin`、`iproute2`、`python3-gi`、`gir1.2-atspi-2.0`、`xvfb`、`xdotool`、`xauth`、`x11-utils`、`dbus-x11`：

```bash
SHUVPN_GUI_SMOKE=1 bash tool/verify_desktop.sh
```

脚本覆盖前端构建、格式、Clippy、Rust tests 和打包工具测试。核心 Linux 测试在独立网络命名空间运行；实际窗口验收使用 WebView2 CDP 或 WebKit/AT-SPI。

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
