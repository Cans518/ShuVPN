# 参与 ShuVPN 贡献

提交前先查看已有 Issue 和 Pull Request。报告中请写明平台、复现步骤、预期结果及实际结果，不包含密码、Cookie、Token、私钥或真实个人信息。

项目支持 Windows/Linux 桌面。开发环境和运行方式见[开发指南](docs/development.md)。`desktop/src` 负责显示和用户操作，`desktop/src-tauri` 负责本地命令及应用生命周期，`rust` 负责账户、连接、设置、路由和系统集成。网络策略与认证规则应留在核心。

使用独立分支完成一个明确改动，并同步更新受影响的测试和文档。提交前检查：

```bash
npm run build --prefix desktop
cargo fmt --manifest-path rust/Cargo.toml --check
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check
cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets --all-features -- -D warnings
cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --target-dir rust/target --locked --all-targets -- -D warnings
cargo test --manifest-path third_party/sangfor/Cargo.toml --target-dir rust/target --workspace --locked
```

Windows 使用 `tool/verify_desktop.ps1`，它会设置随仓库提供的 Wintun 测试路径。Linux 使用 `tool/verify_desktop.sh`，核心测试在普通用户独立网络命名空间中运行。修改 TUN 时还需运行 `tool/tun_linux_namespace_smoke.sh`、`tool/tun_dns_linux_smoke.sh` 和 `tool/tun_data_plane_linux_smoke.py` 的相关场景。

UI 修改要检查实际 Tauri 窗口、键盘操作、明暗主题和关闭退出；IPC、账户取消、规则提交和网络清理需要真实原生集成验证。自动化不使用真实校园凭据，也不隐式更改宿主系统代理或 TUN。

在有 Explorer 托盘的交互式 Windows 桌面，可给 `tool/tauri_windows_smoke.mjs` 的两个路径参数追加 `--native-tray`，验证真实菜单、恢复、代理开关及退出。此模式会短暂使用鼠标；无交互桌面的 CI 使用默认 WebView 验收。测试报告必须区分两者。

打包改动需检查 `test/tool/test_package*.py`、`test_linux_install.py`，保留路径安全、文件完整性、helper 隔离及安装事务回滚约束。模板构建成功不代表已完成安装/升级/卸载验证。

`third_party/sangfor` 的协议快照与测试样本独立于其他语言工具链。更新时保留
上游许可、固定 revision 和原始文件摘要；固定协议样本的变化需要说明协议依据。

在 PR 中说明问题、最终行为、受影响平台、执行的验证及未验证范围。第三方代码和资源需要保留许可。
