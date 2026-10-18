# 无效规则草稿保留生效配置的 Windows GUI 验收

2026-10-18 第80批补充：Linux便携及实际DEB安装同类路径已完成，见 [Linux无效规则GUI验收](linux_invalid_rule_gui_acceptance.md)。Linux Apply实际disabled且零动作，没有物理点击；与本页Windows UIA仍enabled=true、另留真实点击收据的证据分开。

第79批，2026-10-18。原计划M4.4要求校验失败保持旧配置。本批在正式main的新字体私有版本资源副本上，增加实际输入、校验失败、点击无提交效果、独立native读回及重开的证据，没有修改生产代码。

总汇为 `build/desktop/rules-invalid-gui/verification.json`，最终运行 `run-03`，三个普通权限进程正常退出0。工具与测试474文件来源 `916160ce5f732e7fb131684e9be798ffdd1c2027191e141bced2bdcfed1fe2ee`，Windows/Linux两树一致；产品继续使用77批474文件来源 `9a9ebbf5575f7a63f174437a04a8a8340a6654439ad1b72404ff49fd1af05c90` 的原始字体ZIP，SHA256 `6e9c4503de857f08d1a98c3749f0b76deacfe01f3c63d230467733b67c29c679`。工具来源与产品来源分别保存，不冒充本批重建。

## 实际验证

延续既有欢迎表单、假密码遮罩/清空、主题、端口、捕获模式与正常规则提交流程。规则版本1先预置两个非空DIRECT前后置覆写，再由GUI应用仅校园模板为版本2。

第二进程在“导入与高级编辑”的实际字段输入并逐字核对：

```json
{"name":"invalid-gui","rules":["UNSUPPORTED,example,DIRECT"]}
```

点击“校验并预览”，UI显示原生“规则类型尚未支持”。页面所示“应用预览”为灰色，但Windows UIA `CurrentIsEnabled` 与Legacy状态没有正确给出禁用信息，实际仍报告enabled=true；该问题没有修复或签收。

本批保留这一原始观测，再执行实际鼠标点击“应用预览”，保存点击目标PID、类型、实时矩形、ClickablePoint及原物理点击步骤。经过0.812秒的实际观察并再次截图，原错误仍可见，原生路由文件前后字节一致：

```text
3b1e399cd4f2b3ae7d1e7316b05416bc9692053daaae171d66567d3b52ce4a89
```

随后退出第二进程才打开独立native session，核对版本2、仅校园profile、完整非空overrides、两个覆写域名DIRECT与公网测试REJECT。第三进程重开后再次核对有效版本与记录，并展开raw编辑器，要求其原生无障碍值逐字等于已提交profile；无效草稿未替换它。最终native读回仍为版本2，四个网络开关为false，路由文件摘要仍与点击前相同。截图不是所有页面内容的逐像素验证，也不宣称raw编辑器所有行都在截图视口内。

这证明本次输入路径、观察区间及退出/重开时点的功能性无提交效果，不证明任意未来延迟写入不可能，更不证明UIA禁用标志正确。

## 校验器与失败记录

无收据时仍要求唯一按钮明确报告disabled；所有原有缺错误、错误不可见、按钮缺失/重复、enabled真/未知、记录为空/变化等负例保留。新增路径仅接受带实际点击收据、有限且至少0.5秒观察时长、前后摘要匹配，并且点击后原生错误仍可见且完整记录未变的证据。NaN/Infinity、错误目标、无收据或摘要不符均拒绝。

当前GUI调用链的收据直接来自所属窗口/PID、实时目标和WindowFromPoint检查后的物理点击；独立纯函数不是离线收据认证器，手造JSON并不能独立证明点击真的发生。总汇另把收据PID与第二次原进程匹配，并关联实际点击步骤。

- `run-01` 保持失败：实际ExpansionTile在本Windows引擎上暴露为UIA Text/50020，驱动原先只找Button；现在仅对该已观察控件指定精确类型，仍核对唯一性和物理点击。
- `run-02` 保持失败：严格UIA enabled=false预期未获证实，而且当时没有物理点击收据；不能追溯升格为通过。
- Linux首轮工具日志缺少无Git源码镜像必须声明的来源环境，9项门禁错误保留；最终正确提供revision/dirty后通过，没有修改门禁或测试断言。

两端完整工具测试各114项通过（Windows19/Linux7平台跳过），Dart边界108文件通过；其中17项私有profile/规则验证守卫通过。只修改Windows开发驱动与对应测试两个Python文件，没有生产Dart/Rust、原生runner、绑定、AOT或安装候选变化。完整Rust/Flutter/analyze不冒充重跑，安装与Linux Full证据仍按78批独立记录。

三次准备的6个KnownFolder根均不存在，验收mutex不存在，最终无应用/fixture进程，宿主两应用目录和两代理键本批前后相同。失败、源码和报告均保留。

## 继续范围

本批没有执行Linux同类无效草稿GUI路径，不把Windows通过扩大为两端M4.4全部签收。后续可复用 `desktop_gui_acceptance.py`；78批实际Atspi树中“导入与高级编辑”为panel，当前通用control筛选没有包括它，不能直接照抄按钮类型假设。本机已确认Atspi `get_text(start_offset, end_offset)` 与 `get_state_set()`接口可用，但新的运行路径仍需实测。

一般规则遮蔽、全部来源/格式交叉组合、真实连接详情拒绝展示、长期与其它平台等证据仍需按原计划分别审计。真实校园、Windows原始EXE/安装GUI、独立可恢复guest特权Full、远端CI与最终逐项审计保持未完成；整体目标active，无新包/提交/发布。
