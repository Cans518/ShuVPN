# 安装与升级

请从项目 [Releases](https://github.com/Cans518/ShuVPN/releases) 下载对应版本、平台的完整文件，并核对旁边的 SHA-256 校验文件。

## Windows

支持 Windows 10/11 x64，需要 Microsoft Edge WebView2 Runtime。若系统未提供，可从 [Microsoft 官方页面](https://developer.microsoft.com/microsoft-edge/webview2/)安装 Evergreen Runtime。

- **安装器**：运行 `ShuVPN-版本号-windows-x64-setup.exe`，按向导安装，然后从开始菜单启动。
- **便携包**：完整解压 ZIP 后运行 `shuvpn.exe`。保留 `shuvpn_tun_helper.exe`、`wintun.dll` 及许可文件。

安装器默认安装到当前用户的程序目录。普通代理使用不需要 TUN 授权；启用 TUN 时，程序按系统要求处理权限。

PowerShell 校验示例：

```powershell
Get-FileHash .\ShuVPN-0.6.3-windows-x64-setup.exe -Algorithm SHA256
Get-Content .\ShuVPN-0.6.3-windows-x64-setup.exe.sha256
```

## Debian / Ubuntu

面向 Ubuntu 22.04 及兼容的 x64 图形桌面。DEB 包会声明 WebKitGTK、GTK、AppIndicator 和安装工具依赖：

```bash
sudo apt install ./ShuVPN-0.6.3-linux-x64.deb
```

从应用菜单启动 ShuVPN，使用普通用户运行图形界面。需要 TUN 时，将当前用户加入授权组，然后注销并重新登录：

```bash
sudo usermod -aG shuvpn "$USER"
```

Linux 托盘显示依赖桌面环境的托盘服务。可通过应用菜单再次启动 ShuVPN 来恢复已有窗口。

### TAR.GZ

完整解压后，可直接运行 `./shuvpn` 使用本机代理。可选安装到系统目录：

```bash
sudo python3 ./install.py --user "$USER"
```

安装脚本将 GUI 放入 `/opt/shuvpn`、受限 helper 放入 `/usr/lib/shuvpn`。不要给 GUI 可执行文件授予文件能力；不要用便携安装脚本覆盖 DEB 管理的安装。

```bash
sha256sum -c ShuVPN-0.6.3-linux-x64.tar.gz.sha256
```

## 升级和移除

1. 先从托盘或设置页退出 ShuVPN，等待网络清理完成。点击窗口关闭按钮会保留后台进程。
2. 安装器版本运行新安装器；DEB 使用 `apt install` 安装新包；便携版本完整替换程序目录。
3. 重新启动程序。账户和设置保存在用户数据目录，升级时保留。

Windows 安装版本可从系统“已安装的应用”移除；DEB 使用 `sudo apt remove shuvpn`；便携脚本安装使用 `sudo python3 /opt/shuvpn/install.py --uninstall`。移除程序保留账户、设置及 Linux 授权组成员关系。

## 分发说明

程序和安装器当前未提供发布者代码签名，SHA-256 用于核对下载文件完整性。请核对项目发布来源。系统代理、TUN 和实验性完整捕获受操作系统权限与桌面环境影响，出现错误时请保留提示并按[安全说明](../SECURITY.md)处理敏感信息。
