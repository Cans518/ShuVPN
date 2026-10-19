# 发布流程

## 版本与记录

应用版本位于 `desktop/package.json`、`desktop/src-tauri/tauri.conf.json` 和 `desktop/src-tauri/Cargo.toml`，根 `Cargo.lock` 与 `desktop/package-lock.json` 必须同步。界面版本直接来自前端 manifest。Rust 核心的 crate 版本独立于应用发行版。

在 `CHANGELOG.md` 中维护 `## 版本号 - YYYY-MM-DD` 节。正式版本只接受 `主版本.次版本.修订号`；标签必须为 `v版本号`。

```bash
python3 tool/release.py check --tag v0.6.1
python3 tool/release.py notes --output build/release-notes.md
```

## 本地发行构建

先运行[开发验证](development.md)并提交源码。发布命令要求 Git 工作树干净；构建产生的文件应留在被忽略的 `build/`、`target/` 和 `dist/`。

```bash
python3 tool/release.py check --tag v0.6.1 --clean
npm ci --prefix desktop
npm run build --prefix desktop
cargo build --locked -p shuvpn_desktop --release --features tauri/custom-protocol --bin shuvpn
cargo build --locked -p shuvpn_core --release --bin shuvpn_tun_helper
python3 tool/stage_desktop.py --platform linux --binaries target/release --output build/release-staging --release
python3 tool/package_desktop.py release --platform linux --bundle build/release-staging --output build/packages
python3 tool/package_installers.py build/packages/ShuVPN-0.6.1-linux-x64.tar.gz --output build/packages --release
```

Windows 将平台改为 `windows`，安装器编译使用 `tool/install_inno_setup.ps1` 提供的固定 Inno Setup 编译器，并向 `package_installers.py` 传入 `--iscc`。

正式暂存读取主程序 `--build-info`，校验版本、平台、架构和 release 配置；正式归档只允许固定载荷，拒绝开发文件与 dirty 源码。安装器必须与便携包使用同一份源码标识。便携包包含文件 SHA-256 清单，最终安装器和归档各有校验文件。

## GitHub 发布

1. 将已验证的发布提交推送到目标分支。
2. 确认版本记录和用户说明正确，为该提交创建并推送相同版本的标签。
3. `packages.yml` 验证版本与标签，通过与日常 CI 相同的平台验证入口（包含 Linux 隔离网络场景），再构建两平台 release 程序和安装包。
4. 归档中的 release 程序完成实际窗口验收后，工作流上传安装包、便携包、校验文件与构建元数据，并创建对应 GitHub Release。

手动运行打包工作流会生成可下载的 Actions artifacts；只有版本标签触发对外 Release。发布 job 依赖两个平台都成功。

## 发布检查

- 版本、标签、锁文件、界面显示和更新记录一致。
- 源码已提交，工作树干净，包内元数据引用该提交。
- Windows EXE/ZIP、Linux DEB/TAR.GZ 均完成校验。
- 对最终程序执行平台实际窗口验收；安装和权限场景按对应平台单独验收。
- 分发包保留 AGPL、字体、图标、协议与驱动许可。

程序当前未配置发布者代码签名；不要把 SHA-256 描述为发布者身份认证。若添加签名，应先签名，再生成安装器、校验值与构建元数据。
