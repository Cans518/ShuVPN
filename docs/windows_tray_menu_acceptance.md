# Windows 原生菜单状态验收

2026-10-18 第73批补充：Explorer 实际图标点击、原生菜单/协议子菜单、Up/Enter退出与本地原生代理清理已另行通过，见 [Explorer 托盘交互验收](windows_explorer_tray_acceptance.md)。该证据使用开发入口，仍不能签收正式 main、账户/控制器、TUN 或特权 Full。本文以下保留第68批原生状态测试的原始范围。

第68批，2026-10-17。验证对象是锁定的 `system_tray 2.0.3` Windows 菜单实现，以及仓库内的定点适配。测试创建真实 Win32 HMENU，通过插件公共 MethodCall handler 更新菜单，再用 `GetMenuItemInfo` 读取状态；不显示窗口、不触发 UAC、不改用户设置。

上游真实复现了三个问题：

- 创建同时带有 enabled、checked 的复选项时，后写的 checked 覆盖 enabled，禁用项变为可用。
- `SetEnable` 覆盖勾选位。
- `SetCheck` 覆盖禁用位。应用的协议菜单按 label、enable、check 顺序更新，因此最后一步会丢失禁用状态。

四组原生测试在原文件上三组失败，标签和身份保留一组通过。适配后四组全部通过，包括 enabled × checked 四组合、启用和勾选反复切换、保留独立 default 位、标签/ID/owner/主菜单与子菜单句柄。每组均覆盖顶层 ID 和子菜单 ID。

`windows/third_party/system_tray/menu.cpp` 只改三处：创建时组合状态；运行时读取当前状态，再只修改对应的 enabled 或 checked 位。原头文件、回调、参数解析、图片处理和资源所有权继续使用上游实现。CMake 核对原文件 SHA256 `6cf1efb4b7371f372eb278f9387b8dc9ad62776b134fcd275cbcfced0ed2c7cd`，替换单个编译单元，不修改共享 pubcache。生成的生产项目中只有仓库内这一份 `menu.cpp`。

测试和生产分别编译同一源文件。测试目标不参与普通构建、不安装；`tool/verify_desktop.ps1` 和 native CI 显式构建并执行它：

```powershell
flutter build windows --release
cmake --build build/windows/x64 --config Release --target shuvpn_tray_menu_test
& build/windows/x64/Release/shuvpn_tray_menu_test.exe
```

完整上游 MIT 许可安装到 `data/licenses/system_tray/LICENSE`，Windows 打包检查将其列为必需文件，缺失时拒绝生成包。

本批证据目录为 `build/desktop/windows-tray-state`，包含原生 red/green、Windows release、两端工具检查、配套归档与安装器及实际安装事务记录；最终汇总为其中的 `verification.json`。Windows 实际托盘 popup、鼠标选择、回调至应用的完整闭环，以及已连接 GUI 仍待单独验收。此处的原生状态测试不能签收这些场景，也不能替代 Windows 特权 Full、真实校园或远端 CI。
