# 参与 ShuVPN 贡献

感谢你愿意为 ShuVPN 做出贡献！我们欢迎认真提交的 Bug 报告、功能建议、文档改进和代码贡献。

我们希望贡献流程保持友好、简单。即使这是你第一次参与 Flutter 项目，也不必担心；请描述你尝试过的内容，我们会尽力提供帮助。

## 开始之前

- 创建新 Issue 前，请先搜索已有的 [Issues](https://github.com/shuosc/ShuVPN/issues) 和 Pull Request，避免重复提交。
- 报告 Bug 或提出功能想法时，请使用合适的 [Issue 模板](https://github.com/shuosc/ShuVPN/issues/new/choose)。
- 对于较大的改动，可以先创建 Issue 讨论方案，再开始编写代码。
- 请勿在 Issue、Pull Request、截图、日志或测试数据中包含密码、Cookie、Token、API Key、私钥或真实个人信息。

## 开发环境

ShuVPN 当前构建与验收固定使用 Flutter 3.47.2。请先参考 [README](README.md) 配置同一工具链；升级 SDK 或依赖时，作为单独变更更新验证矩阵和边界复核记录：

```shell
flutter --version
```

然后在项目根目录执行：

```shell
flutter pub get --enforce-lockfile
flutter devices
flutter run -d <device-id>
```

项目仅支持 Windows 与 Linux 桌面版。Windows 需要 Visual Studio 的桌面 C++ 工具链；Linux 需要 GTK、CMake、Ninja 与 Clang。两端均需要 Rust 工具链。

## 项目方向与功能建议

ShuVPN 正在快速迭代补齐功能中，欢迎你在下列预备开发计划中贡献。
 - Easyconnect 协议隧道支持
 - OpenVPN 协议隧道支持
 - SHU Speedtest 内网测速支持
 - 添加内网实用服务链接
 - (远期) SHU校内 锐捷网络portal认证支持

如果你有新的功能想法，请先创建 [Issue](https://github.com/shuosc/ShuVPN/issues)，说明使用场景、要解决的问题和预期方案，等待维护者与社区充分讨论后再决定是否加入。未经讨论和确认的新功能 Pull Request 可能不会被采纳，因此建议先沟通方案，以免投入时间编写的代码最终无法合并。

我们尤其欢迎以下贡献：

- 报告能够稳定复现的 Bug，并协助验证修复结果；
- 修复已确认的 Bug；
- 改善现有功能的交互和可用性；
- 补充测试、文档和跨平台兼容性改进。

## 开发流程

1. 从 `main` 创建一个专注于单项任务的分支。
2. 以解决问题所需的最小改动为目标。
3. 保持生产代码、测试和文档与改动内容一致。
4. 创建 Pull Request 前，格式化并验证代码：

   ```shell
   dart format .
   python -m unittest discover -s test/tool -p test_dart_boundary.py
   python tool/verify_dart_boundary.py
   flutter analyze
   cargo build --manifest-path rust/Cargo.toml --locked --release --lib --examples
   flutter test
   ```

5. 如果改动涉及平台侧代码，请在受影响的平台上使用 Simulator 或真机测试，并在 Pull Request 中说明结果。
6. 使用仓库的 [Pull Request 模板](.github/pull_request_template.md) 创建 PR，说明改动内容、关联的 Issue、受影响的平台以及已经执行的检查。

生产 Dart 的文件集合、职责、内容、运行依赖与锁由[边界清单](docs/dart_backend_boundary_manifest.json)封存，说明见[边界复核](docs/dart_backend_boundary_review.md)。变更失败时先审阅实际代码及调用链，再更新相应条目；不能只改SHA绕过门禁。生成绑定须用固定生成器重新生成并核对，页面状态与薄适配不等于后台业务实现；平台/运行期验收仍需独立执行。

Linux TUN 改动还需在独立网络命名空间验证内核包、实际出站与回滚。先执行上面的 release/examples 构建，再在 Linux 项目根目录运行：

```shell
sudo bash tool/tun_linux_namespace_smoke.sh
sudo bash tool/tun_dns_linux_smoke.sh
sudo unshare --net python3 tool/tun_data_plane_linux_smoke.py
```

验证实际桌面包中的原生库时，两个 Dart 内核探针接受可选的库路径：

```shell
# 使用实际 Dart SDK 可执行文件，避免 root 经 Flutter wrapper 触发 Git ownership 检查。
sudo unshare --net /path/to/dart-sdk/bin/dart run tool/tun_bridge_smoke.dart build/linux/x64/release/bundle/lib/libshuvpn_core.so
sudo unshare --net /path/to/dart-sdk/bin/dart run tool/tun_dns_bridge_smoke.dart build/linux/x64/release/bundle/lib/libshuvpn_core.so
```

独立 Cargo 与 Flutter 联合构建使用不同的 target 目录，不能要求两份动态库字节一致来代替包内验证。应明确加载待分发包或其解压目录中的库，分别记录真实内核收发、动态域名 DNS 与清理结果。

`tool/tun_data_plane_linux_smoke.py` 创建独立的 veth 对端，验证实际校园 TLS、DIRECT 双栈 TCP/UDP、256 KiB TCP 往返及 FIN、拒绝、停止/重建与默认路由保持不变。探针拒绝在宿主网络命名空间直接运行；不需要校园凭据。

`tool/verify_desktop.sh` 可在正常 Git checkout 中直接运行。若使用不带 `.git` 的源码快照，需显式设置 `SHUVPN_SOURCE_REVISION` 为原仓库的完整40位提交号，并用 `SHUVPN_SOURCE_DIRTY=true` 或 `false` 记录快照是否包含未提交修改。打包清单还会计算实际源文件摘要，不能用提交号掩盖未提交源码。

Linux 安装/权限修改还需执行9项安装事务回归，以及解压包内默认库的真实FFI安装验收：

```shell
python3 -m unittest discover -s test/tool -p test_linux_install.py
dart compile exe tool/linux_install_smoke.dart -o build/linux-install-ffi
dart compile exe tool/tun_dns_bridge_smoke.dart -o build/linux-installed-tun-ffi
sudo bash tool/linux_install_smoke.sh /absolute/path/to/extracted-package "$PWD/build/linux-install-ffi" "$(id -un)" \
  "$PWD/build/linux-installed-tun-ffi" "$PWD/rust/target/release/examples/proxy_fixture"
```

该harness通过独立mount/net namespace及overlay/tmpfs运行真实安装器，隔离账户文件和固定安装目录；普通用户检测安装前、授权后、未授权用户、错误权限、NoNewPrivs、缺少capabilities、升级及卸载后的状态，并执行默认库的startDesktopTun、校园DNS/动态域名路由/TCP和finishStop，核验helper进程退出与宿主账户文件摘要不变。需要root准备空间，不向GUI授予capabilities。省略最后两个可选参数时仅检测安装权限，不执行TUN数据场景。`SHUVPN_LINUX_INSTALL_NAMESPACE=1 bash tool/verify_desktop.sh`可在完整构建流程中启用同一验收；GitHub流程已接线，远端运行证据仍需单独记录。

出口保护改动还需执行专用 DNS/HTTPS 探针。该 feature 只构建测试例程，正式桌面库保持默认 features：

```shell
cargo build --manifest-path rust/Cargo.toml --locked --release --features egress-probe --example egress_namespace_smoke
sudo unshare --net python3 tool/tun_data_plane_linux_smoke.py --egress
cargo build --manifest-path rust/Cargo.toml --locked --release --lib --examples
```

探针把物理 DNS/HTTPS 服务放进 TUN 捕获网段，验证出口绑定、DNS TCP 重试、证书验证和失败回滚。TLS 测试证书只写入独立临时目录，不安装系统信任根。

`tun_dns_linux_smoke.sh` 使用真实内核 DNS/套接字和仅按域名授权的资源，验证动态 IPv4/IPv6 主机路由、原域名签名目标、TTL、已有流租约、配置替换、错误回答、零 TTL 和外部路由冲突；不预置 DNS 回答地址的捕获路由。

Windows 原生出口改动还需验证实际 socket 索引、取消和回退。下面的探针不创建 TUN 或修改路由；接口必须为启用的物理 Ethernet/Wi-Fi，IPv4 地址从系统读取。将 `WLAN` 改为本机接口别名：

```powershell
$egressAlias = 'WLAN'
$egressAddress = Get-NetIPAddress -InterfaceAlias $egressAlias -AddressFamily IPv4 |
    Where-Object AddressState -eq Preferred |
    Select-Object -First 1 -ExpandProperty IPAddress
cargo run --manifest-path rust/Cargo.toml --locked --release --features egress-probe --example windows_egress_smoke -- $egressAlias $egressAddress
```

探针读取所选接口 DNS，另外使用自有 DNS/TLS fixture 验证保护路径；这不代表现场上游 DNS 或真实校园已通过。Wintun 与捕获路由上的成功/失败/清理路径仍需单独做特权验收。

Windows 低层 TUN 特权验收入口如下。在管理员 PowerShell 中运行，工作目录为项目根目录；探针本身不会触发 UAC：

```powershell
$env:SHUVPN_WINTUN_DLL = (Resolve-Path windows/third_party/wintun/amd64/wintun.dll).Path
cargo run --manifest-path rust/Cargo.toml --locked --release --features tun-probe --example windows_tun_smoke
```

同一例程还支持普通父进程启动提升 helper 的模式。先构建并把测试 helper 和固定驱动放在 probe 同目录，再从**普通权限** PowerShell 运行：

```powershell
cargo build --manifest-path rust/Cargo.toml --locked --release --features tun-probe --example windows_tun_smoke --bin shuvpn_tun_helper
Copy-Item rust/target/release/shuvpn_tun_helper.exe rust/target/release/examples/shuvpn_tun_helper.exe
Copy-Item windows/third_party/wintun/amd64/wintun.dll rust/target/release/examples/wintun.dll
rust/target/release/examples/windows_tun_smoke.exe --helper
```

`--helper` 调用生产 Endpoint.launch，会请求 Windows UAC 同意；父进程必须未提升。它经真实 IPC 完成创建、双栈 UDP ring 往返、动态路由安装/删除/重装，再确认 helper 退出、接口消失和路由稳定属性快照恢复，成功标记为 `WINDOWS_TUN_HELPER_DUAL_STACK_RING_DYNAMIC_ROUTES_AND_CLEANUP_PASSED`。`--helper-disconnect` 改为断开管道后核对拥有真实网卡和动态路由的 helper 收尾，成功标记为 `WINDOWS_TUN_HELPER_DISCONNECT_WITH_OWNED_ADAPTER_CLEANUP_PASSED`；它不宣称打断了已进入内核等待的 read。

`--helper-inherit` 启动继承当前权限的普通子进程，不触发 UAC；当前非管理员环境应返回 permission 并等待空会话关闭。这三种 helper 模式的进程 watchdog 为 180 秒，包含 UAC 等待；超时退出 124 仍表示清理未经证明。无参数模式继续要求管理员环境并使用 60 秒 watchdog。UAC 取消返回系统错误 1223；不要把取消或超时当作成功，也不要自动反复重试用户已取消的提升。

探针创建随机 `shuvpn` 前缀的接口，使用 `192.0.2.2` / `2001:db8:518::2` 虚拟地址，静态捕获 `192.0.2.3/32` / `2001:db8:518::3/128`，动态捕获 `198.51.100.3/32` / `2001:db8:519::3/128`。已有非默认路由覆盖这些测试地址时，在创建前拒绝运行。它验证双栈内核 UDP 包进入 ring、注入回包到真实 socket、动态路由重复同步/删除、预先取消更新的拒绝、空 ring 读取关闭、重复 close、接口消失及系统路由快照恢复。预先取消检查不证明写入途中取消的内核回滚。

无参数模式只有输出 `WINDOWS_TUN_DUAL_STACK_RING_DYNAMIC_ROUTES_AND_CLEANUP_PASSED` 才表示本次直接低层成功路径通过。超时应读取系统接口与路由状态定位失败，不能作为通过结果。helper 模式新增 IPC 路径核验，仍不提供高层会话、物理出口防回环、系统 DNS 或崩溃恢复验收。`tun-probe` 为独立可选 feature，正式应用保持默认 features。

Windows bundle 还携带 `shuvpn_tun_helper.exe`。它通过受限的本机管道接收 TUN 操作；Rust 的 Endpoint 负责生成随机管道、绑定进程身份和启动参数，不能把这个内部进程当作通用命令行执行器。正式桌面 TUN 会话已接入同目录 helper 的 Windows runas 启动，UAC 仍由 Windows 请求用户同意。接线和普通权限 IPC 回归不代表最新特权 Full 路径已验收；独立观察者、可恢复 guest 及当前限制见[Windows Full 验收说明](docs/windows_full_capture_acceptance.md)。

下面的集成测试启动普通权限子进程，不触发 UAC；在管理员测试环境也不创建网卡。它验证实际跨进程握手、计划/包/路由预算、普通权限拒绝、父进程代次错误、重复关闭与客户端断开退出：

```powershell
cargo test --manifest-path rust/Cargo.toml --locked --all-features --test windows_tun_helper
```

构建 bundle 后，可针对实际随包 helper 重跑同一组测试：

```powershell
$env:SHUVPN_TEST_TUN_HELPER = (Resolve-Path build/windows/x64/runner/Release/shuvpn_tun_helper.exe).Path
cargo test --manifest-path rust/Cargo.toml --locked --all-features --test windows_tun_helper
Remove-Item Env:SHUVPN_TEST_TUN_HELPER
```

此环境变量只由测试读取，生产 launcher 不接受外部 helper 路径。当前证据没有覆盖真实 UAC 同意/取消、提权后持有 TUN 的断开/父进程退出、原生调用停滞和清理失败，不能以无网卡 IPC 测试替代特权验收。

`Flutter CI` 工作流会对推送到 `main` 的提交，以及目标分支为 `main` 的 Pull Request，运行 `flutter analyze` 和 `flutter test`。

Rust SSO 存储改动还需运行跨进程探针，使用当批正式库：

```powershell
dart run tool/newsso_storage_smoke.dart build/windows/x64/runner/Release/shuvpn_core.dll
```

Linux 使用 `build/linux/x64/release/bundle/lib/libshuvpn_core.so`。探针只在独立临时目录使用假凭据，不连接学校；四个进程分别提交导入、恢复、注销和验证已注销，成功标记为 `NEWSso_CROSS_PROCESS_IMPORT_RESTORE_LOGOUT_AND_MIGRATION_GATE_PASSED`。当前 persistent API 需要专属绝对路径目录；`clear` 注销，`close` 保留已提交会话并释放锁。写盘失败必须处理，不能以空内存快照宣称注销已持久化。该探针不替代断电、真实用户偏好迁移或正式登录接线验收。

设置存储或共用文件原语改动还需运行设置跨进程探针，使用当批库：

```powershell
dart run tool/settings_storage_smoke.dart build/windows/x64/runner/Release/shuvpn_core.dll
```

Linux 使用 `build/linux/x64/release/bundle/lib/libshuvpn_core.so`。四个独立进程验证旧 schema 导入、恢复、恢复连接默认及旧数据门，成功标记为 `SETTINGS_CROSS_PROCESS_MIGRATION_RESTORE_RESET_AND_IMPORT_GATE_PASSED`；只用临时目录假设置，不连接学校。设置 owner 需要专属绝对目录，保存成功后才发布快照；替换后同步无法确认时明确报错并拒绝续写，重开恢复磁盘记录。调整 `rust/src/platform/files.rs` 时还需运行账户和 SSO 探针，避免破坏共享锁/替换语义。桌面清除旧 `settings.*` 前必须得到 native 提交成功，auth/TOFU/设备号不属于设置清理范围。

Windows 请确认 `python` 指向实际解释器，避免 Microsoft Store 占位命令导致脚本未执行；同时检查退出码与探针成功标记。

项目自2026-10-16起永久移除 Android 工程、构建与验证入口，仅维护 Windows/Linux。旧 Android 验证记录保留在历史实施文档中，不作为当前开发命令或验收要求。

## 代码风格

- 遵循现有的 Dart 和 Flutter 代码习惯，并使用 `dart format` 格式化代码。
- 使用清晰的名称，保持方法和类的职责单一。
- 避免重复代码和不必要的依赖。
- 将平台特定的改动保持在适当范围内，并为不明显的设计决策补充说明。
- 行为发生变化时，添加或更新相应的测试。

## 提交信息

请使用简短且能够说明内容的提交标题，格式如下：

```text
<type>: <description>
```

常用的类型包括：

- `feat`：新增功能；
- `fix`：修复 Bug；
- `docs`：更新文档；
- `refactor`：在不改变行为的情况下调整代码结构；
- `test`：添加或更新测试；
- `chore`：维护构建工具或仓库配置；
- `style`：进行格式化或其他不影响功能的代码风格调整。

例如：

```text
docs: add contribution guidelines
```

## 其他项目规范

- 请遵守 [ShuVPN 社区行为准则](CODE_OF_CONDUCT.md)。
- 如果在开发过程中使用了 LLM，请遵守 [LLM 使用政策](LLM_POLICY.md)。

再次感谢你帮助 ShuVPN 变得更好！
