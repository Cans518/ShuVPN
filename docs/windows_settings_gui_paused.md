# Windows 离线设置 GUI 第76批暂停记录

恢复更新（2026-10-18）：用户随后明确恢复持续推进，已完成本批两次真实GUI验收与回归，目标active。当前结论见 [76批恢复验收](windows_settings_gui_acceptance.md)。下文保留暂停时的失败与现场，不代表当前目标仍暂停；最初宿主代理差异仍未归因。

日期：2026-10-18。用户暂停持续推进目标后要求落盘交接；目标保持 paused。本文记录未完成工作，不是验收通过记录。上一完成里程碑是 [75批正式main离线GUI](windows_formal_gui_acceptance.md)。

## 已保留的改动

本轮只修改开发工具与测试，没有修改生产Dart/Rust、runner、正式AOT、DLL或候选包。

| 文件 | 本轮改动 | 验证边界 |
| --- | --- | --- |
| `tool/gui_settings_probe.dart` | native结果增加 `system_proxy_enabled` | Dart格式化运行、0处变化；尚未通过本轮GUI闭环读回 |
| `tool/windows_desktop_probe.py` | 增加有限非零notches的物理滚轮输入 | 新滚轮在本轮GUI中尚未执行到 |
| `tool/windows_formal_gui_acceptance.py` | 增加滚动定位、端口编辑、捕获模式、规则预览/应用/重开及严格离线状态检查 | 首次启动激活即失败，新增操作均未获实际通过证据 |
| `test/tool/test_windows_profile_desktop_acceptance.py` | 增加端口/模式/规则及四开关严格状态校验测试 | `guards-green.log`：11项通过；此前缺函数ImportError保留在red日志 |

预期流程是：保留75批欢迎/受控假密码清空/主题流程；在第二进程保存Full，输入70000检查“设置值无效”且存储不变，改18443保存；选择仅校园模板，校验预览时规则文件字节不变，应用后版本只加1；关闭后native独立读回；第三进程确认Full/端口/规则恢复，再改回Campus并关闭读回。所有HTTP、SOCKS、VPN和系统代理开关须严格为false。此段描述实现意图，不能当作已运行结果。

## 实际失败与现场

证据目录是 `build/desktop/windows-settings-gui/`，属于本地忽略产物，不能假定其他checkout已有这些文件。

- `private-01/verification.json`：准备器marker及载荷摘要。
- `prepare-01.log`、`run-01.log`：准备与真实运行日志。
- `run-01/verification.json`：`passed=false`，`cleanup_error=null`，`private_knownfolder_directories_removed=true`，`launches=[]`、`steps=[]`。
- `host-before.json`、`host-after-pause.json`、`pause-state.json`：暂停核对证据。

CLI实际退出1，调用链为 `run → start_launch("01-welcome") → activate_formal → pointed_target → WindowsDesktop.move`。实际错误：

```text
Physical pointer did not reach its target: [1713, 1872] vs [957, 1219]
```

尚未完成第一次激活，也未进入端口/捕获模式/规则操作。空launches只是激活成功后记录尚未追加，不能解读为从未启动应用。失败不能证明坐标缩放、显示器布局或外部输入中的哪一个是原因；恢复后须核实，而不是删掉物理指针读回断言绕过。

本次marker与verifier运行时工具来源均为471文件：

```text
revision  1e055d672754971bb7ac5bbadb923858d78ae777
dirty     true
tree_sha256 f7d88a492ac77e8c22d8cf670f48119ed62cace3c1cfd1306e843c2490606c01
```

这是运行时来源，不包含随后交接文档变化的保证。Linux镜像 `/home/mzee/.cache/shuvpn/linux-worktree` 尚未同步本轮4个文件，仍是75批工具来源 `264c8d9ebfa008f9a3e5b5ab1c8cb0ca17ede6b05eff8a4134a7032f2dd1b2bf`。没有新的两树一致证明，也没有76批总汇通过标记。

使用的原始ZIP仍为72批 `connection-candidate/package/ShuVPN-0.4.2+9-windows-x64.zip`，SHA256 `0dc3fc111b5afeac5bac83412e849b72c37155c03aa9293f26985d91fe706dfd`；产品来源462文件7abf6237…，正式AOT SHA256 `351535b462f105a5a9274187ba39692dca213626491deac95c7d35dcc8b23981`。只改复制EXE的两个等长版本资源字符串来选择新KnownFolder目录，不是原始EXE默认profile或实际安装版GUI验收，也不是独立用户/网络隔离。

## 暂停后的清理与宿主差异

CLI会话已结束；只读CIM查询未发现名称匹配 `shuvpn*.exe` 或 `proxy_fixture.exe` 的进程。报告finally清理完成，实际 `os.path.lexists` 对以下两根均为false：

```text
C:\Users\xiema\AppData\Roaming\43ee0d
C:\Users\xiema\AppData\Local\43ee0d
```

`Local\ShuVPN.Desktop.WindowAcceptance` 互斥锁OpenMutex返回不存在，错误码2。没有进一步启动GUI或执行新验收，也未为交接删除或回退代码。

宿主快照只涵盖原有Roaming/Local `ShuVPN/ShuVPN` 两目录及两个HKCU Internet Settings键，非连续监控。两应用目录存在状态与文件摘要相同，代理键快照不相同：

| 路径/值 | 前后差异 |
| --- | --- |
| `Internet Settings/ProxyEnable` | 值摘要从对应JSON数字1变为对应JSON数字0 |
| `Internet Settings\Connections/DefaultConnectionSettings` | 摘要由cdaeb61f…变为b5bc11e0… |
| `Internet Settings\Connections/SavedLegacySettings` | 摘要由cdaeb61f…变为b5bc11e0… |

本轮未归因这些变化，不能说是验收造成，也不能说宿主代理保持不变。未擅自回写或恢复用户系统代理。75批自己的快照相同结论仅适用于75批。

## 明确恢复后继续

1. 先读交接顶段和本文，确认用户已恢复目标，再核对桌面会话、实际屏幕/任务栏坐标、指针移动环境及代理差异。不要自动启用系统代理、TUN或UAC。
2. 主线程阅读将修改的驱动代码，定位首次激活失败。保留run-01、原marker、日志和快照；修复后使用新的private/report目录。旧private-01对应的预留根已清理，不能直接复用作新验收。
3. 补充规则overrides保留的实际校验。当前仅校园模板只替换profile，页面保留既有 `_overrides.text`；驱动目前只核对提交profile/providers，未独立比较原overrides。native API配置键是 `profile_document` / `overrides_document`，磁盘键是 `current.profile` / `current.overrides`，不可混用。当前native probe尚未输出这两个文档。
4. 查验滚动定位、同名选项、弹窗与长页在Windows实际视口的行为。捕获模式选择即保存，无独立保存按钮；非法端口的真实Rust错误是“设置值无效”。规则预览/提交/重开必须保留原始磁盘与native证据，不能只靠UI文本。应用前后按钮禁用、自动校园开关等额外语义也尚未签收。
5. 完成适当测试、analyze、Linux同步和共同工具路径回归后，记录新的来源并执行真实GUI。全量工具104项是预期数量，本轮只实际运行定向11项；不要写成104通过。未重跑全量Flutter/Rust、安装、远端CI，也没有新产品构建。

恢复后的命令示意（本次交接未执行；应先修复并确认环境，目录名须未占用）：

```powershell
& D:\miniconda3\python.exe -X utf8 tool/windows_private_profile.py --archive build/desktop/connection-candidate/package/ShuVPN-0.4.2+9-windows-x64.zip --output build/desktop/windows-settings-gui/private-02
& D:\miniconda3\python.exe -X utf8 tool/windows_formal_gui_acceptance.py --bundle build/desktop/windows-settings-gui/private-02/bundle --dart C:\Users\xiema\.cache\shuvpn\flutter\bin\cache\dart-sdk\bin\dart.exe --report build/desktop/windows-settings-gui/run-02
```

完整迁移尚有真实账户/校园、Windows原始EXE默认profile或安装GUI、独立可恢复guest特权Full及故障恢复、长期运行、其它Linux桌面/Wayland、远端CI和原计划最终逐项审计。保留现有全部未提交迁移工作，不因本轮暂停撤销历史修改。
