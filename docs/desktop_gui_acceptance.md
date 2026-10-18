# 正式 Linux GUI 离线设置验收

2026-10-18 第80批补充：新字体便携及实际DEB安装各三个普通GUI进程退出0；新增非法规则精确输入、原生错误、Apply禁用/零动作观察、原生记录不变及重开编辑器恢复，见 [Linux无效规则GUI验收](linux_invalid_rule_gui_acceptance.md)。实际未点击Apply。扩展区与错误role均为panel，Text/State/聚焦动作已实测；Component几何读取再次DBind超时，最终路径不使用它。该批只改开发驱动/测试，产品保持77批来源，详细范围与失败记录以新文档为准。

2026-10-18 第75批补充：Windows 已运行真实正式 main AOT 的私有版本资源副本，重复通过首次表单可达、主题 GUI 保存及重启恢复，见 [Windows 正式主入口离线验收](windows_formal_gui_acceptance.md)。生产 AOT/DLL保持原始包字节，EXE仅两个存储命名资源字符串改变；这不是原始 EXE 默认 profile 或真实登录签收，也尚未覆盖本页 Linux 的完整端口/规则设置矩阵。

第65批，2026-10-17。使用由 `lib/main.dart` 构建的完整release应用，经真实X11窗口和平台辅助功能操作；不替换账户/设置/规则后端，不使用widget测试替代窗口验收。

## 本批续作实测

接手时只有部分成功的便携 GUI 记录，安装版最后仍以 SIGABRT（-6）退出，不能按旧汇总脚本预期生成成功清单。先复跑便携版三次正常退出，随后在原 `80d802b1c576` DEB 上独立复现安装版第一次关闭失败。

安装 harness 经 root 切回普通用户时，DBus 曾继承 `/run/user/0`。现将私有 XDG profile/runtime 的创建移到 DBus 启动前，runtime 上下文覆盖整个子进程。新增回归先失败后通过。修正目录后，权限错误消失，但安装版仍以 -6 退出，证明它与渲染释放失败独立。

旧 GDB 栈显示主线程在 Flutter Linux 库的 compositor mutex 清理处 abort，同时 raster 线程仍在绘制。runner 现于窗口销毁前保留 view 子树的 widget 引用，等待 engine dispose 中的 Shutdown 返回，再通过默认 GTK context 的 idle 释放；application hold 保证最后窗口关闭后仍能处理该回调。不修改 Flutter SDK、Rust 网络清理或退出码断言。独立复核未在已检查的引用链中发现新闭环；额外的 engine 长期引用仍可能延迟退出，不能通过超时强行释放仍被使用的 renderer。

新 Linux release、tar 和 DEB 对应438文件来源 `74dbae1ac5981e8ddfa3c91d98d0fcfb65d9a495b9525357b47bf4b5d31aa645`，Windows 工作区与 Linux 快照一致。安装版三次启动均为 UID1000/CapEff0，实际加载 `/opt/shuvpn/lib/libshuvpn_core.so`，完整执行上述离线设置与重开场景，三次退出均0；另完成一轮安装版校园 TUN 数据/退出屏障，以及安装、升级、卸载、账户保留和宿主状态不变检查。本轮关闭序列未出现 mutex abort，不据此宣称全部窗口、GPU、联网 GUI 或长期场景已验收。

Linux Flutter全量311、analyze、两端工具各51项通过（Windows19项平台skip），生产Dart边界108文件通过。重建 core/helper SHA 与既有受测版本一致；未冒充新跑 Rust 全量或 Full 故障矩阵。CI新增安装版 GUI 场景并在失败时保留非 profile 截图/JSON/日志，尚未远端执行。

证据为 `build/desktop/gui-acceptance/verification-resume.json`，红/绿日志保留；新 Linux 验证产物在同目录 `shutdown-package/`，tar SHA `4370ee5b2272921185d052ed8c88166263939571ee0b81aaf93b258943482880`、DEB SHA `1e419956989d81af63f55ac0a2f22b12d52dfc50abe0b52a0e94a925015744d2`。它们不是新的成对桌面发布候选，没有提交或发布。

## 范围

- 创建独立Xvfb、DBus会话与Openbox，拒绝继承的显示/会话；新建XDG配置、数据、缓存与runtime目录。只识别本次应用PID所属窗口，检查标题、实际exe与配套Rust库maps，进程保持普通用户、CapEff0。
- 首次启动操作三页欢迎、权限说明、登录介绍与登录表单；不提交账号密码。旧“本应用不保存账户信息”与Rust持久化会话/设备号/展示状态不一致，已改为说明本机保存登录会话和账户展示状态。
- 当前引导必须登录成功才完成。脚本在首个窗口正常关闭后，以独立Rust设置命令**仅准备welcomeCompleted=true**再启动Guest场景。这是测试准备，不是登录成功或完整首启认证签收。
- 正式窗口操作深色主题、Full捕获模式、HTTP端口；非法70000显示原生校验错误且提交记录不变，合法18443保存成功。不开启HTTP/SOCKS/VPN，也不建立校园连接。
- 规则选择“仅校园模板”后预览不会改生效版本，应用后版本增长。关闭应用后独立Rust会话读回配置名称与版本，并对非校园输入执行原生规则测试，要求REJECT。
- 同profile以新PID重开：检查实际渲染的深色选中指示、GUI端口/Full行值与规则名称/生效版本/REJECT兜底，然后从GUI切回Campus保存。三次正常退出均要求0，覆盖Alt+F4和Ctrl+Q。

状态提交由实际Rust文件及关闭后的原生API分别核对；截图和辅助功能树保留。真实控件动作按本次PID的标签定位，输入用聚焦后的键盘操作，固定1120×960窗口中的少数像素位置用于返回、输入焦点和主题选中指示。该尺寸与布局是测试前提；不声称覆盖全部缩放/桌面环境。组件几何接口和EditableText在本环境的调用失败保留日志，最终操作路径与验收断言明确区分。

## 运行

需 `python3-gi`、GTK/Atspi typelib、Xvfb、Openbox、xdotool、xauth和dbus-run-session，以及当前Flutter工具链。

```bash
python3 tool/desktop_gui_acceptance.py \
  --bundle "$PWD/build/linux/x64/release/bundle" \
  --report "$PWD/build/desktop/gui-acceptance-new" \
  --dart /absolute/path/to/flutter/bin/cache/dart-sdk/bin/dart
```

报告目录必须是新目录。脚本自行建立私有显示/会话，以普通用户运行，不能用Python优化模式关闭断言。内部入口的token/显示/DBus与UID限制有独立回归。不要传入真实用户XDG目录。

`tool/gui_settings_probe.dart`只接收库路径、测试settings目录及操作模式，在GUI owner关闭后使用原生会话读回。自第76批起，离线准备还通过原生路由API提交两个非空前后置DIRECT测试覆写，供Windows独立校验保留；Linux共用路径也已重新完成三进程设置回归。该准备不建立目标连接，settings/rules写入仍由Rust负责。

CI的Linux bundle步骤已接该场景并保存顶层截图/辅助功能JSON/日志；不上传profile内账户/会话数据。本地完整验证可使用 `SHUVPN_DESKTOP_GUI_ACCEPTANCE=1 bash tool/verify_desktop.sh`。DEB harness提供 `SHUVPN_DEB_GUI_REPORT` 和 `SHUVPN_DEB_GUI_ACCEPTANCE=1`，在实际安装后的 `/opt/shuvpn` 上执行相同场景，需同时指定 `SHUVPN_DART_RUNTIME`。

## 限制

第66批另以真实 XFCE 宿主完成独立的[托盘验收](desktop_tray_acceptance.md)，包括 native 菜单状态、隐藏/恢复、真实右键与键盘选择、宿主丢失及同进程热重启。它不改变下述离线设置场景的范围，也不等于联网 GUI 签收。

第71批另完成[本地协议已连接 GUI](desktop_connected_gui_acceptance.md)：真实默认库、生产界面/控制器、三轮双栈代理数据、普通断开重连及连接中退出。该开发入口替换账户/连接计划，不属于本页正式入口场景，也不签收真实校园登录。正式入口 release 已恢复，并另重验首启/guest/托盘宿主恢复。

这不是校园登录、网络数据面、托盘可用时隐藏/恢复、Wayland、长期运行或全部GUI签收。系统代理/权限不可用的提示在无桌面host环境中按实际返回展示。既有OpenGL首帧及implicit-view/messenger关闭警告不因进程正常退出而视为修复。平台网络与安装Full证据继续按各批次独立保存。
