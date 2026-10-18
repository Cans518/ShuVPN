# 无效规则草稿保留生效配置的 Linux GUI 验收

第80批，2026-10-18。使用77批霞鹜文楷屏幕阅读版的正式 main 产品，补齐规则校验失败的 Linux 便携、实际 DEB 安装路径。仅修改 `tool/desktop_gui_acceptance.py` 和对应 Python 测试；没有生产代码、字体资源、原生库或包变更。

总汇为 `build/desktop/linux-invalid-rules/verification.json`。便携最终运行 `run-03`，实际安装运行 `installed-01/session`（Windows 留存副本为 `installed-01`），两组各三个普通用户进程，UID1000、CapEff0、正常退出0。安装组实际 maps 为 `/opt/shuvpn/lib/libshuvpn_core.so`；namespace 内 PID 891、1146、1419 不用于宿主 PID 清理检查。

工具来源474文件 SHA256 `fdfa78556f20e2211d12a2c3bfec93d70a52dc624a6048783c5253275af4a7df`，Windows 与 Linux 镜像相同。产品继续使用77批474文件来源 `9a9ebbf5575f7a63f174437a04a8a8340a6654439ad1b72404ff49fd1af05c90`。tar SHA256 `57e63552302f2334682f3310e87d2ea702be428463e965a503e9fa4414b1d356`，DEB SHA256 `317355b3900f19e0cde7fea952a07137d5f5dde443eaf2f428bf39db4ee2f9f1`；四个77批安装产物摘要均未变。字体接入及原始许可见 [字体验收](wenkai_font_acceptance.md)，之前完整安装/Full/托盘证据见 [78批安装验收](wenkai_install_acceptance.md)。

## 实际路径

延续已有欢迎表单、离线设置准备、深色主题、非法70000端口拒绝、18443保存、Full设置重开及切回Campus。离线准备预置版本1和两个非空前后置DIRECT覆写，GUI正常预览、应用仅校园模板为版本2，然后在同一第二进程中展开高级编辑器。

展开区和原生错误的实际 AT-SPI role 均为 `panel`。raw 字段通过当前应用 PID 子树中唯一且内容精确匹配的 `text` 节点定位，平台动作聚焦后用 xdotool 键盘输入，再以 Text 接口完整回读：

```json
{"name":"invalid-gui","rules":["UNSUPPORTED,example,DIRECT"]}
```

点击“校验并预览”后，截图与辅助功能状态均确认“规则类型尚未支持”显示。“应用预览”实际 `enabled=false`、`showing=true`、动作数量0，因此**没有调用 Apply 平台动作，也没有物理鼠标点击 Apply**。便携与安装组分别继续观察0.539、0.535秒，错误仍显示，原生规则文件在前后采样点逐字节一致，SHA256均为：

```text
3b1e399cd4f2b3ae7d1e7316b05416bc9692053daaae171d66567d3b52ce4a89
```

版本2、current/previous、profile、非空prepend/append、providers、warnings及has_previous全部保持。关闭第二进程后，用独立 Dart 进程加载默认 Rust 库读回；第三个新 PID 重开，AT-SPI 完整读取编辑器中的合法仅校园 profile，打开编辑器前后文件不变；最后再次关闭并独立读回。两次原生读取均保留版本2、原profile和覆写原文，两个覆盖域名决策DIRECT，公网测试REJECT，HTTP/SOCKS/VPN/系统代理开关均false。这里没有建立真实校园连接。

主线程已查看两组非法草稿截图和便携重开截图。重开截图中 raw 编辑器在视口外；完整恢复文本由 AT-SPI 读取验证，不称全部文本已在截图中显示。沿用的截图文件名 `22-invalid-rule-apply-click.png` 不表示发生了点击，实际动作以记录中的 `actions:0, result:null` 为准。

## 检查和失败记录

两端完整 Python 工具测试117项通过，Windows跳过19项、Linux跳过7项；生产Dart边界108文件通过。本批没有重跑完整Rust/Flutter/analyze或生产构建，不能把77批结果称为本批新跑。

新增纯校验守卫要求原记录非空且前后字节相同、精确输入与显示的原生错误、有限且至少0.5秒的观察、正整数PID、完整动作结果及匹配摘要；回归拒绝空记录、版本/覆写变化、错误目标、缺失动作结果、NaN/Inf等。调用链还要求目标PID等于当前应用PID。纯函数本身不认证观察来源，也不强制disabled状态；最终总汇单独核对本次实际disabled/showing、零动作及对应进程，不扩大为所有未来运行均验证了disabled。记录相等证明采样点相同，不证明期间从未写入。

`run-01` 因 Component.get_extents 的 DBind 超时失败，失败截图的辅助功能读取又掩盖了原异常；现诊断失败保留原异常。`run-02` 已成功输入并显示原生错误，但错误控件被假设为text而定位失败。两次退出1、截图及日志保留；最终不再调用几何接口，不将接口签名存在当作运行可用。修正panel定位后 `run-03` 通过，未改生产界面绕过错误。

另一次辅助安装载荷观察误将便携tar的README清单用于DEB安装，README大小/摘要不同而失败；后续观察时GUI已结束，未生成全量安装文件逐项证明。该失败另存 `payload-observation-failed.json`。本批实际测量了安装GUI的exe、core与app三项摘要；独立检查了tar32项及DEB归档内33项清单，明确区分归档核验与安装后核验。78批全量安装证据保持独立。

DEB仍通过既有私有mount/net/PID harness安装，完成拒绝外来目录/符号链接、配置失败重试、权限矩阵、合成旧版本升级、运行保护、remove/purge/重装、原生账户保留及卸载后不可用检查。日志中的预期dpkg拒绝和非空非包目录提示保留。外层比较宿主账户及dpkg数据库摘要相同，宿主没有遗留 `/opt/shuvpn`；记录的宿主启动器/便携进程PID均已消失，固定安装与便携路径进程为空。未重跑TUN、Full、托盘或Windows GUI。

真实校园、Windows原始EXE/安装GUI、独立可恢复Windows guest特权Full、长期、其它Linux桌面/Wayland、远端CI及原计划最终审计继续保留，整体迁移目标仍active，没有提交或发布。
