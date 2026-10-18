# 规则拖动回归与 Linux 几何查询阻塞

第88批，2026-10-18。Flutter 指针手势与真实 Rust 规则持久化的子范围通过，正式 Linux 窗口的物理拖动仍未通过。总汇为 `build/desktop/rule-drag/verification.json`：`widget_native_pointer_drag_scope_passed=true`，`linux_physical_pointer_drag_scope_passed=false`，整体 `passed=false/partial=true`，迁移目标仍 active。

## 改动和验证边界

生产只修改 `routing_rule_list.dart` 的展示：为现有48×48拖动手柄增加独立 Semantics image 节点，名称包含来源、绝对位置及规则表达式；保留原指针监听器、上下移替代按钮和所有回调，没有增加语义 Tap。原先手柄名称与卡片标题/启用状态合并，无法独立取得手柄范围；新增测试先复现旧名称，修复后明确核对唯一名称、image、无Tap及48×48范围。边界清单亲读后更新，仍109文件。没有修改排序业务、Rust、生成绑定、依赖或字体。

新 `test/native/routing_rule_drag_test.dart` 对 prepend/profile/append 各自使用两个同目标、动作相反且都启用的规则：

- 真正鼠标指针按下、分步跨过前一条规则，然后取消：两份编辑器原文、既有预览对象和持久文件字节保持。
- 再次拖动并松开：原生 inspectDraft 读出倒序，MATCH仍固定最后，所有规则仍启用；旧预览失效、Apply禁用，版本1及旧文件不变，真实生效决策仍为REJECT。
- 显式校验/应用后版本2变为DIRECT；卸载页面、释放manager并重新打开后，逐项核对三个来源的完整顺序、未编辑来源不变、两份配置和实际文件一致，原生决策及页面重挂载都保持。

`routing_rule_list_test.dart` 另补真实手势的向上、向下、第二页26→25绝对索引、取消、忙碌后在原手柄位置拖动不发命令和独立手柄语义。不是仅调用 `onReorderItem`，也不是把上移按钮算作拖动。

两端最终针对这两个文件单并发运行16项通过，日志 `gesture-{windows,linux}-final.log` 逐项保留名称。完整 Flutter Windows341/Linux340通过，analyze、边界109通过，两端最终工具172项通过（平台跳过19/7），两端正式 `--target lib/main.dart` release通过。最初测试本身的重复 cancel 与未推进松开动画时钟失败保留；修正测试后排序手势通过。独立语义修复后的第一次 finder 仍取得父节点，原失败也保留，最终定位独立导出节点。

## 正式窗口驱动及失败

新增 `tool/desktop_rule_drag_acceptance.py`，复用既有离线基线与管理辅助流程，计划以正式main程序验证三来源拖动、离开页面丢弃草稿、预览失效、显式应用和重开。按实际当前PID的唯一 image 名称查找手柄；两个完整矩形必须位于当前已校验归属的窗口内容范围，窗口PID、位置和激活状态在输入前再次核对。实际输入使用 mousedown、12次mousemove、mouseup，主错误和释放失败分别处理；不使用 AT-SPI Tap 代替拖动。9项新守卫覆盖所有权、矩形、操作序列、失败释放、完整原生文档/三来源决策和拖动前预览能力。

独立复核指出初稿没有先创建有效预览，会让“拖动后Apply禁用”检查空过；已补每次拖动前生成预览、确认唯一Tap和未落盘，拖动后再核对失效。该复核是验收器漏洞，不是发现生产拖动没有失效。三个原生widget测试也验证了既有预览失效。

流程已接入本地 opt-in、Linux bundle CI与DEB GUI入口，artifact仅收集顶层及baseline的PNG/JSON/log，不递归上传私有profile。**当前该新GUI门禁在本环境会失败**，不能将接线存在写成CI或安装验收通过；远端CI未执行，本批新DEB也未安装。

`portable-01` 与 `portable-02` 各自完成三进程离线基线、版本3夹具导入及关闭owner后的独立原生读回，再在第5个GUI进程中失败。前4个进程均正常退出0，失败进程由清理回收；两轮宿主link/路由/rule前后相同。

首次误将 Apply 的全部辅助操作数要求为1，故在拖动前失败。第二轮实际记录其动作列表为 `["Tap","Focus"]`，修正为“唯一Tap”，不把Focus算作第二次Apply，也不接受无Tap/重复Tap或禁用按钮。

第二轮随后在手柄 `Component.GetExtents` 处触发 `atspi_error: timeout from dbind`，尚未发送任何按下/移动/松开的拖动序列，保持 `passed=false`。本轮没有以截图猜测坐标，也没有放宽手柄矩形守卫。只有widget/FFI手势范围通过，正式窗口物理拖动没有签收。

## 运行时定位

`geometry-01/02/04` 是本轮合成配置的独立诊断，只复制本轮自行创建的 settings/routing 两个记录并设600权限，不复制账户，不接触真实校园凭据。诊断不计入完整GUI验收。第一份外层WSL日志没有合并stdout/stderr，部分输出发生覆盖；后续显式dup2，使用后续完整日志和调用栈作为定位证据。

同进程诊断可先取得独立手柄名称，再连续尝试SCREEN/WINDOW/PARENT坐标，均超时；后两种尝试发生在首次失败之后，不是各自冷启动隔离对照。后续子进程未重新取得手柄，因此GetSize/GetPosition没有直接运行时结果，不能把“节点缺失”称作这些方法的测试结论。几何请求后连普通退出也超时，需要定向清理。

`geometry-04` 在普通UID的真实正式main进程 PID480238上复现，并抓取主线程调用栈。关键顺序为：

```text
poll
dbus_pending_call_block
dbus_connection_send_with_reply_and_block
libatk-bridge-2.0.so.0
atk_component_get_extents
libflutter_linux_gtk.so / atk_component_get_extents 的父链
```

原始栈在 `geometry-04/geometry-stack.log`，Thread1从880行起。它证明几何查询时GUI主线程阻塞在ATK桥的同步D-Bus等待中。它不证明消息的实际目的地；“同进程plug/socket重入等待”是与源码及栈一致的机制判断，尚未单独抓取目的地或实施修复。没有归因于Rust排序、原心跳压力失败或另一个已确定的WSL绑定问题。

两端实际engine revision均为 `a804b261645ef8c13eb3d5c44a5c2fb0340c5539`。本机Linux包为 libatk-bridge2.0-0 2.38.0-3、libatk1.0-0 2.36.0-3build1、libatspi2.0-0 2.44.0-3、GTK3.24.33-1ubuntu2.2。固定engine源码的 `fl_accessible_node.cc` 先递归取得父范围，`fl_socket_accessible.cc` 使用AtkSocket嵌入；Ubuntu镜像的at-spi2-atk2.38.0原始源码中，plug范围经 `dbus_connection_send_with_reply_and_block(...,-1,...)` 取得。原始文件、源码包及SHA/来源在 `upstream/`，不把最初404的GitHub tag当作取得源码。未核对发行版-3全部补丁，不声称原始源码与每个运行时字节完全一致。

GDB通过APT只下载并解包到 `build/desktop/rule-drag/gdb-toolchain/`，未全局安装。调试子进程使用自己的库目录和空debuginfod地址；只为本轮新GUI子进程授权预先创建的具体debugger PID，再核对实际exe/PID存活后附加，读取栈并detach，未全局修改ptrace设置。抓栈与退出失败的记录保留，没有把清理成功算作正常GUI退出。

原 `geometry-04/geometry-diagnostic.json` 中 `pointer_input_sent=false` 过宽：诊断发送了滚轮，未发送拖动序列。原文件不改写，另有 `geometry-04-scope.json` 明确更正；诊断脚本后续已改为分别记录drag/scroll。最终本轮已验证GUI PID和正式程序/私有GDB映像均无遗留，宿主无固定app/helper安装。

## 来源与候选

正式构建及四包来源493文件，SHA256 `e610553f66e39f2abf444067ec5fe500c706713333473f0315aef4597db1cb5f`；后续只修改开发驱动/守卫，最终工具493文件来源 `65ed60b0b078d93d1f5b488cf55e6214f010d1545e34ad24a9519677cdd0881e`。各自Windows/Linux两树一致。正式release目录是本批main，不是夹具AOT；旧87/86/83/77候选保持原样。

新候选在 `build/desktop/rule-drag/package/`：

| 产物 | bytes | SHA256 |
| --- | ---: | --- |
| Windows ZIP | 35408077 | `57cb6146739309b0a355717290bce5d1c00a0783466348f98fae8499df21ad61` |
| Windows Inno | 28410531 | `fbad8aa2627db981448200821775a13074883457ba78e5b8c8f1ba41aaabc548` |
| Linux tar | 34991799 | `cbcc30d16bd56e40ee5a497d08e8dc41015a370b0947973b49de60444bcc2658` |
| Linux DEB | 28057394 | `eb06036cec1691ad607d37aebba187bd9310df03c642f4365ce7200f5f953b16` |

ZIP39、tar32、DEB声明33项及实际载荷逐项核对，manifest另计；DEB分发README/标记/清单单列，其余应用与tar一致。霞鹜文楷TTF/OFL/FontManifest和core/helper摘要保持。Inno只核对外层、已验证ZIP输入和固定编译器链，没有独立解码内嵌载荷。本批两新安装器都未实际安装，也未重跑完整Rust、网络/TUN/Full、长跑或远端CI。

## 下一步

本轮最后只读检查发现Windows前台已变为DSH窗口，旧安全弹窗 HWND5639302 **已不存在**，没有代选允许/取消。因此87批“仍被同一前台弹窗阻挡”只描述当时，不能继续作为当前阻挡理由。下一轮先重新执行Windows启动前守卫，再用全新私有目录推进现有Windows实窗路径；不要假设所有系统窗口状态永远不变，也不要操作DSH窗口。

Linux须在Flutter/GTK plug/socket几何边界继续定位并修复，再重新执行正式窗口物理拖动；当前新GUI门禁的失败保持，不用窄测试代替。真实校园、Windows正式/安装/连接详情及可恢复guest Full、长期/其它桌面Wayland、原84故障、远端CI和最终逐项审计范围继续，无提交、签名或发布。
