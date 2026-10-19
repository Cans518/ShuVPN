<p align="center">
  <img src="desktop/src-tauri/icons/icon.png" alt="ShuVPN 盾牌图标" width="88" height="88">
</p>

<h1 align="center">ShuVPN</h1>

<p align="center">
  面向上海大学校园网络的开源桌面客户端<br>
  在 Windows 与 Linux 上连接校园资源，自由选择代理、分流与界面配色。
</p>

<p align="center">
  <a href="CHANGELOG.md"><img src="https://img.shields.io/badge/version-0.6.3-2864ad?style=flat-square" alt="版本 0.6.3"></a>
  <a href="#下载安装"><img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-526e64?style=flat-square" alt="支持 Windows 与 Linux"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-AGPL--3.0-7853ad?style=flat-square" alt="AGPL-3.0 许可证"></a>
</p>

<p align="center">
  <a href="https://github.com/Cans518/ShuVPN/releases"><strong>下载客户端</strong></a> ·
  <a href="docs/user-guide.md">使用指南</a> ·
  <a href="docs/themes.md">自定义主题</a> ·
  <a href="CHANGELOG.md">更新记录</a> ·
  <a href="https://github.com/Cans518/ShuVPN/issues">反馈问题</a>
</p>

---

ShuVPN 将账户登录、连接状态、代理设置与规则分流放在一个紧凑的桌面窗口中，也可以在系统托盘里管理连接。使用前需要具备上海大学校园网络的账户与相应资源访问权限。

## 界面预览

<table align="center">
  <tr>
    <th align="center">浅色外观</th>
    <th align="center">深色外观</th>
  </tr>
  <tr>
    <td align="center"><img src="docs/screenshots/home-light.png" alt="ShuVPN 0.6.3 浅色连接页，展示连接按钮、流量和 HTTP/SOCKS5 开关" width="320"></td>
    <td align="center"><img src="docs/screenshots/home-dark.png" alt="ShuVPN 0.6.3 深色连接页，展示连接按钮、流量和 HTTP/SOCKS5 开关" width="320"></td>
  </tr>
</table>

<p align="center">
  暮紫配色 · 支持浅色、深色与跟随系统 · 内嵌霞鹜文楷 GB 字体
</p>

## 功能概览

| 功能 | 说明 |
| --- | --- |
| 校园账户登录 | 使用统一身份认证账户，按提示完成验证码或企业微信扫码 |
| 连接状态与流量 | 查看连接阶段、累计上传/下载及本机代理监听地址，支持取消连接和主动断开 |
| 多种接入方式 | 按需开启 HTTP、SOCKS5、系统代理或 TUN |
| 规则分流 | 修改规则后先校验、预览，再确认应用；支持回退上一份配置 |
| 托盘与启动 | 从托盘连接、断开、切换代理配置或打开页面，可选登录系统时启动 |
| 主题定制 | 晴海、青竹、暮紫、暖砂与奶龙五套配色，支持自定义明暗主题、预览和 JSON 导入导出 |

## 下载安装

前往 **[GitHub Releases](https://github.com/Cans518/ShuVPN/releases)**，按系统选择完整安装包。当前仓库版本为 **0.6.3**，分发目标为 **x64**。

| 系统 | 推荐安装包 | 便携包 |
| --- | --- | --- |
| Windows 10/11 x64 | `ShuVPN-版本号-windows-x64-setup.exe` | `ShuVPN-版本号-windows-x64.zip` |
| Debian/Ubuntu x64 | `ShuVPN-版本号-linux-x64.deb` | `ShuVPN-版本号-linux-x64.tar.gz` |

- **Windows**：需要 Microsoft Edge WebView2 Runtime；安装器提供向导，便携包解压后运行 `shuvpn.exe`。
- **Linux**：面向 Ubuntu 22.04 及兼容的图形桌面，需要 WebKitGTK 4.1、GTK 3 与 Ayatana AppIndicator 运行库；DEB 包声明所需依赖。
- **便携包**：完整解压，保留主程序、独立 helper、驱动和许可证的目录关系。

下载文件旁提供 `.sha256` 校验文件。安装、权限配置、校验命令与升级步骤见[安装指南](docs/installation.md)。

> 程序和安装器当前未提供发布者代码签名。请核对下载来源；SHA-256 用于核对文件完整性。

## 开始使用

1. **登录账户**：打开“账户”，使用上海大学统一身份认证账户，按提示完成身份核验。
2. **选择接入方式**：按需开启首页的 HTTP/SOCKS5，或在设置中配置系统代理/TUN。
3. **建立连接**：返回“连接”并点击连接按钮，查看当前状态、流量和代理地址。

HTTP/SOCKS5 供配置了相应代理的应用使用；需要系统代理或 TUN 时，请在设置中选择。代理配置修改在下一次连接时生效，TUN 可能请求系统授权。完整捕获模式为实验性功能，需要显式选择。

> **关闭窗口会继续在后台运行。** 已有连接和托盘保留，再次启动程序会恢复窗口。退出或升级前，请从托盘选择“退出 ShuVPN”，或使用设置页的退出按钮，等待网络清理完成。

更多操作见[使用指南](docs/user-guide.md)。

## 常见问题

<details>
<summary><strong>开启 HTTP/SOCKS5 后，为什么某个应用仍无法访问校园资源？</strong></summary>

该应用需要配置使用首页显示的代理地址；本机代理只服务于使用它的应用。也可以按需选择系统代理或 TUN。资源能否访问仍取决于校园账户的授权范围。

</details>

<details>
<summary><strong>Linux 如何使用 TUN？</strong></summary>

使用 DEB 安装后，将当前用户加入授权组，然后注销并重新登录：

```bash
sudo usermod -aG shuvpn "$USER"
```

TAR.GZ 的 helper 安装与授权步骤见[安装指南](docs/installation.md#targz)。使用普通用户运行图形界面。

</details>

<details>
<summary><strong>开机自启动后会自动连接吗？</strong></summary>

不会。此选项在当前用户登录系统时启动 ShuVPN，并停留在托盘。便携版本移动程序后，需要关闭并重新开启自启动。

</details>

<details>
<summary><strong>账户与设置保存在哪里？</strong></summary>

数据保存在当前用户的应用数据目录中，开发和验收可通过 `--data-dir` 指定独立目录。账户会话在 Windows 上使用当前用户的 DPAPI 保护；Linux 上依赖用户目录与文件权限，未进行内容加密。请勿公开上传完整数据目录。

</details>

<details>
<summary><strong>断开或退出时提示清理失败，该怎么办？</strong></summary>

保留程序，按错误提示重试。反馈时提供系统、应用版本、复现步骤和错误文字，并移除密码、验证码、Cookie、Token 与个人信息。安全问题请按 [SECURITY.md](SECURITY.md) 私密报告。

</details>

## 从源码运行

桌面应用使用 **Tauri 2 + TypeScript + Rust**。准备 Node.js 24、Rust 1.96.0 和 Python 3.10 以上版本；Rust 工具链由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定。

先安装对应平台的原生依赖：

- **Windows**：MSVC C++ 工具链和 WebView2 Runtime。
- **Linux**：WebKitGTK 4.1、GTK 3 与 Ayatana AppIndicator 开发包，具体安装命令见[开发指南](docs/development.md#工具)。

```bash
git clone https://github.com/Cans518/ShuVPN.git
cd ShuVPN
npm ci --prefix desktop
cd desktop
npm run tauri -- dev
```

前端构建与检查可在仓库根目录执行：

```bash
npm run build --prefix desktop
npm test --prefix desktop
cargo fmt --check
```

原生测试、平台窗口验收与网络测试使用[开发指南](docs/development.md#验证)中的统一入口。生成对外分发包时，请按[发布指南](docs/releasing.md)执行版本检查、正式构建及打包验证。

## 项目结构

```text
ShuVPN/
├── desktop/
│   ├── src/             # 页面、主题与交互
│   └── src-tauri/       # 桌面宿主、窗口、托盘与本地命令
├── crates/core/         # 认证、连接、路由、系统代理与 TUN 核心
├── third_party/sangfor/ # 固定的协议快照与独立许可
├── packaging/           # 平台安装模板
├── tool/                # 构建、验证与打包工具
├── tests/tool/          # 工具测试
└── docs/                # 用户与开发文档、界面截图
```

第一方 Rust 包由根 Cargo workspace 统一构建；协议快照保留独立 workspace、锁文件与来源信息。核心不依赖 Tauri，前端通过桌面宿主公开的 commands 访问核心。模块职责与修改约束见[开发指南](docs/development.md#模块)。

## 文档导航

| 文档 | 内容 |
| --- | --- |
| [安装指南](docs/installation.md) | 平台依赖、安装、校验、权限、升级与卸载 |
| [使用指南](docs/user-guide.md) | 账户、连接、托盘、自启动与规则操作 |
| [自定义主题](docs/themes.md) | 主题编辑、导入导出及 JSON 格式 |
| [开发指南](docs/development.md) | 开发环境、模块边界与平台验证 |
| [发布指南](docs/releasing.md) | 版本检查、正式构建与分发流程 |
| [更新记录](CHANGELOG.md) | 各版本的功能与修复 |

## 参与贡献

欢迎提交问题反馈、文档改进、主题和代码贡献。

- **报告问题或提出建议**：先查看已有 [Issues](https://github.com/Cans518/ShuVPN/issues)，提供平台、应用版本、复现步骤、预期与实际结果。
- **提交 Pull Request**：阅读[贡献指南](CONTRIBUTING.md)与[行为准则](CODE_OF_CONDUCT.md)，保持改动聚焦，并说明验证结果及未验证范围。
- **报告安全问题**：按[安全政策](SECURITY.md)使用私密渠道，避免在公开 Issue 中披露敏感信息。

## 许可与致谢

项目采用 **[GNU AGPL-3.0](LICENSE)** 许可证。

感谢以下开源项目提供的基础能力与资源：

- **Tauri / Rust / TypeScript**：桌面宿主、连接核心与界面开发。
- **Pico CSS / Lucide / Feather**：界面样式与图标。
- **霞鹜文楷 GB**：随应用打包的界面字体。
- **flutter_sangfor 的 Rust 协议模块**：固定的 aTrust 协议与 TLS 快照。
- **Wintun**：Windows TUN 驱动。

第三方资源各自保留其许可证，详情见 [THIRD_PARTY_NOTICES.md](desktop/THIRD_PARTY_NOTICES.md)与 [Wintun 许可](desktop/resources/wintun/LICENSE.txt)，也可在应用内查看开源许可。
