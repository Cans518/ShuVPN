# Windows 三来源规则窗口驱动与系统弹窗阻挡记录

2026-10-18，第85批，**部分完成**。已实现常规Windows三来源规则窗口验收驱动并通过工具守卫，Linux共享检查回归通过；Windows真实三来源弹窗尚未验收。当前前台是“Windows 安全中心”，其“允许/取消”选择须由用户处理。本批不绕过该选择，也不将Linux或单元测试结果代替Windows窗口证据。

## 改动与已运行检查

新增 `tool/windows_rule_acceptance.py`，继承正式main私有EXE资源副本的离线验收。计划先完成原3进程基线，再分别对prepend/profile/append执行编辑应用和重开删除，共9个原进程。完整来源名称、角色、本次PID、可点击点/控件矩形及页面区域都参与检查，实际输入仍由现有物理鼠标/键盘适配器执行。这个流程目前是已实现、待实测的工具，不能描述成9个Windows窗口已经通过。

新增 `tool/rule_gui_evidence.py` 供两平台共用受控JSON fixture及原生读回检查；真实文档解析和规则业务仍归Rust。每次独立读回要求GUI已退出，测试域名参数只允许read模式，核对来源、版本、动作、原前后置覆写及四个网络开关严格为false。Windows原生probe复用既有 `gui_settings_probe.dart`，未新增Dart后端或探针副本。

FormalAcceptance新增可扩展的离线基线后续阶段和3/9数量守卫；创建时间必须唯一、每个记录正常退出。原进程身份的实际证据仍来自启动时Popen句柄/路径/摘要和停止时保留句柄，数量函数单测不单独证明进程身份。

本批还修复了实际暴露的失败收尾问题：原错误与二次进程/profile清理错误分别保存；整体通过在清理期间撤销，最终清理全部成功后才写true并输出PASSED。`cleanup-red.log` 确认原实现把primary error覆盖为profile清理异常；新增回归通过后保留原错误。未把导入新模块时的red误称产品语义缺陷。

最终新增10项工具守卫，覆盖数量/退出、完整命名与几何、三来源及删除回落读回、独立probe参数与失败输出、基线及扩展失败、双重清理失败、系统弹窗启动前拒绝。两端全量工具各145项通过，Windows19/Linux7平台跳过；Dart边界109文件两端通过。没有生产Dart、Rust、字体、绑定或安装载荷变化，因此本批未重新构建产品，也未冒充重跑Flutter/Rust/安装/Full/托盘矩阵。

最终工具/测试来源486文件，两树SHA256 `cd7cde4b3796cef0f51fb8a1061a0aff110729de317b1334744cd3bfe20c1a6f`。产品保持第83批482文件来源 `f8464b55…`，四包摘要不变。总汇位于 `build/desktop/windows-rule-dialogs/verification.json`，保持passed=false、partial=true及goal active。

## 实际窗口证据与边界

第83批Windows UIA快照确实导出“新增前置规则”“新增配置档规则”“新增后置规则”和来源/行号/表达式完整的编辑、上移、下移、删除名称；两个raw编辑器均为type50004。首次实现据此绑定名称，不使用匿名图标顺序。

Windows快照也确认首尾移动按钮依旧enabled=true，不能声称禁用元数据已修复。新驱动计划对这些边界按钮进行受控实际点击，核对草稿与文件在有限观察期间不变；此Windows行为检查尚未执行成功，不能凭Linux或单测签收。

`run-01` 与 `run-02` 均停在首次窗口激活，规则路径没有开始。它们已启动本次私有GUI进程，但失败发生在完整launch记录追加前，报告launches为空不能解释为未启动应用。两次原进程最终需要强制终止，failed/cleanup_error及日志完整保留。第二次验证了清理错误不再掩盖原激活异常；这不是正常退出通过。

随后只读检查得到前台HWND5639302、类`Shell_SystemDialog`、标题“Windows 安全中心”，进程为`C:\Windows\System32\PickerHost.exe`，同一交互session且未提升。可见按钮为“显示更多”“允许”“取消”；thread/input desktop均为Default。这里只读取窗口身份和控件名称，没有读取安全请求正文或替用户作选择，也不反推该弹窗覆盖此前整个失败时段。

因此新增启动前检查。`run-03` 对同一实际系统弹窗立即返回明确错误；没有启动GUI、没有应用日志或规则操作，未强杀，cleanup_error为null，两套已预留KnownFolder根安全清理。其passed=false是正确阻挡结果，不是Windows规则通过。

## Linux 共享检查回归

同一第83批正式字体bundle的 `linux-01` 共9个不同PID，均UID1000、CapEff0、默认核心库映射正确且退出0。三来源新增/编辑/删除取消与保存、预览不提交、显式应用、编辑重开及6次独立原生读回通过，版本2→8；最终原前后置域名仍DIRECT、已删域名回落REJECT、网络开关均false。主线程查看了配置档删除确认和后置规则重开截图。

该GUI运行的完整工具树为486文件、SHA `03fa4fc21c8e775b5b0523b77e6928deeaedee4d68c160c51aeb7ce5deeffe5f`，早于随后Windows激活/清理守卫修改。它实际使用的 `desktop_gui_acceptance.py`、`desktop_rule_acceptance.py`、`rule_gui_evidence.py`、`gui_settings_probe.dart` 与最终两树逐字节相同，四项摘要保存为 `linux-gui-01-inputs.json`。不能将最终全树cd7cde4b来源冒充为这次GUI启动时的完整树。

本批没有重新执行Linux DEB安装GUI，沿用83批明确范围；当前Linux便携回归不扩展成新的安装证据。

## 清理、复现与继续条件

三次Windows尝试的六个KnownFolder根、输入互斥锁及本次应用映像进程均无遗留，宿主两应用目录/两代理键前后相同。Linux记录中的9个PID及相同bundle映像均已结束。当前没有在途GUI/构建；系统安全弹窗仍保留由用户处理。

处理完系统弹窗后，使用新的output/report目录继续，不能重用本批失败目录：

```powershell
python tool/windows_private_profile.py `
  --archive build/desktop/rule-accessibility/package/ShuVPN-0.4.2+9-windows-x64.zip `
  --output build/desktop/new-rule-private
python tool/windows_rule_acceptance.py `
  --bundle build/desktop/new-rule-private/bundle `
  --report build/desktop/new-rule-gui `
  --dart /absolute/path/to/flutter/bin/cache/dart-sdk/bin/dart.exe
```

实际环境应使用可用Python路径；本机为`D:\miniconda3\python.exe`，PATH中的python不可用。正式构建配置仍须是显式`lib/main.dart`，准备器不接受开发AOT或未配套载荷。

新增测试名包含`desktop`与`acceptance`，符合现有两平台CI守卫发现模式。Windows真实交互GUI没有接入或执行远端CI，不宣称hosted runner拥有可用桌面。本地驱动准备不替代正式原始EXE/安装GUI、真实账户/校园、guest特权Full、长期、多桌面或整个M4.4验收。

用户处理Windows安全弹窗后继续实际三来源窗口。第84批心跳延迟来源、原449秒及当前宿主绑定机制仍按各自证据待查；原迁移其余范围保持，未提交或发布。
