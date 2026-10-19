# 参与 ShuVPN 贡献

提交前先查看已有 Issue 和 Pull Request。报告中请写明平台、复现步骤、预期结果及实际结果，不包含密码、Cookie、Token、私钥或真实个人信息。

项目支持 Windows/Linux 桌面。开发环境、模块边界和验证命令见[开发指南](docs/development.md)，发行构建见[发布指南](docs/releasing.md)。

使用独立分支完成一个明确改动，并同步更新受影响的测试和文档。提交前运行开发指南中的平台验证入口；修改 TUN 时还需运行 `tool/verify_network_linux.sh`，覆盖隔离网络与 helper 清理场景。

UI 修改要检查实际 Tauri 窗口、键盘操作、明暗主题和关闭退出；IPC、账户取消、规则提交和网络清理需要真实原生集成验证。自动化不使用真实校园凭据，也不隐式更改宿主系统代理或 TUN。

在有 Explorer 托盘的交互式 Windows 桌面，可给 `tool/tauri_windows_smoke.mjs` 的两个路径参数追加 `--native-tray`，验证真实菜单、恢复、代理开关及退出。此模式会短暂使用鼠标；无交互桌面的 CI 使用默认 WebView 验收。测试报告必须区分两者。

打包改动需检查 `tests/tool/test_package*.py`、`tests/tool/test_linux_install.py`，保留路径安全、文件完整性、helper 隔离及安装事务回滚约束。模板构建成功不代表已完成安装/升级/卸载验证。

`third_party/sangfor` 的协议快照与测试样本独立于其他语言工具链。更新时保留
上游许可、固定 revision 和原始文件摘要；固定协议样本的变化需要说明协议依据。
该目录保留自己的 Cargo workspace 和锁文件；第一方格式检查使用 `cargo fmt --check`，
避免 `--all` 沿本地依赖重排协议快照。

在 PR 中说明问题、最终行为、受影响平台、执行的验证及未验证范围。第三方代码和资源需要保留许可。
