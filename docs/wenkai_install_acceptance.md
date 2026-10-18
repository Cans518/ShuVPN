# 新字体候选安装与安装版运行验收

第78批，2026-10-18。实际执行第77批字体候选的 Windows Inno 与 Linux DEB 安装验收，没有重建或替换产品。总汇为 `build/desktop/wenkai-install/verification.json`，原始日志、报告与当批运行验证器副本同目录保存。

## 来源

工具与输入产品仍为474文件来源 `9a9ebbf5575f7a63f174437a04a8a8340a6654439ad1b72404ff49fd1af05c90`。四种77批产物逐项重新计算摘要，保持不变：

- Windows Inno：`a96fac8cfc03bd9309bbbcca80cb18174369a0de8fd7e842837e4ca232376eda`
- Linux DEB：`317355b3900f19e0cde7fea952a07137d5f5dde443eaf2f428bf39db4ee2f9f1`

字体、OFL与产物其他来源见 [77批接入记录](wenkai_font_acceptance.md)。本批把77批“安装器只生成”的范围推进为实际安装/升级/卸载；不反写历史记录为当时已经验证。

## Windows

`windows_installer_smoke.ps1` 在新的报告目录安装合成旧版本，然后升级到当前候选、卸载。开始前拒绝已有正式安装注册与运行实例，最终两处安装注册、相关mutex及应用/fixture进程均不存在。

实际检查包括运行中拒绝安装、安装事务拒绝卸载、运行中拒绝卸载、runner在安装标记下返回1618、非包文件保留、原生账户在升级/卸载后恢复。已安装默认DLL的桥接、SSO存储、账户存储、设置存储四套探针通过。

固定来源的 `Verify-Payload` 在实际安装、升级和拒绝卸载后遍历39项清单比较SHA256；初装和升级日志均记录字体与OFL的实际目标文件。没有把卸载后的原始安装目录当作仍可重新独立读取的证据；卸载后仅测试方非包 sentinel 保留。脚本、输入清单和成功终态共同证明本次谓词执行，相关源文件副本与摘要已封存。

这里没有运行 Windows 正式主入口GUI：实际runner调用在Flutter启动前被1618保护拒绝，不能冒充已安装GUI通过。原有两KnownFolder应用目录及两个HKCU代理键本批前后摘要相同，观察不是全系统持续监控。

## Linux

普通用户从当前源码重新编译 readiness/TUN 探针。root只准备私有 mount/net/PID 空间及overlay/tmpfs，安装版程序、Flutter引擎、客户端为UID1000/CapEff0，只有固定独立helper具有网络capabilities。

新DEB实际通过：未管理目录/符号链接拒绝、capabilities配置失败与重试、授权/未授权用户、移除capabilities不可用、运行中拒绝移除、合成旧控制版本升级、remove/purge/重装、原生账户和非包数据保留。宿主账户及包数据库由外层脚本在明确文件集合上比较前后摘要。

三轮校园TUN分别通过真实默认库、内核DNS/动态域名路由/TCP、服务启动、持久owner及退出屏障。安装版Full进一步通过完整十阶段：

`automatic-ready → before-blocked → after-blocked → coverage-change → automatic-cleanup → manual-crash-ready → helper-crash → manual-crash-cleanup → manual-normal-ready → manual-normal-cleanup`。

实际默认库为 `/opt/shuvpn/lib/libshuvpn_core.so`，摘要 `d4732d4f1521809dab8f6ef0ab99817da4f19174cce7cde5352a4b719da3cc38`；固定helper `/usr/lib/shuvpn/shuvpn_tun_helper` 摘要 `061c64a1058778b8a48f2564f45732b05c9a4ae2780f13ceaf64f199965f72c6`，均与新包实际文件及清单匹配。Full覆盖双栈DIRECT TCP/UDP、校园协议、DNS/HTTPS、拒绝/授权负例、覆盖变化与外来路由、强杀helper后重建、控制绑定及退出恢复。三次helper身份由原pidfd见证，全部路由/规则基线与终态相同。认证仍来自本地fixture，不代表真实校园。

实际安装路径上的离线设置GUI与XFCE托盘各三个进程，全部UID1000/CapEff0、退出0。设置包含字体渲染、引导表单、深色、70000拒绝/18443保存、Full重开/Campus恢复、规则应用与重开；托盘包含隐藏/菜单恢复/退出、宿主丢失和同进程重启恢复。截图已查看。报告记录默认库路径，加载由运行验证器在原进程maps中核对；JSON本身没有保存完整原始maps。

GUI/托盘报告中的PID属于私有PID空间，不按host PID去查或终止。外层启动器/harness的host PID结束、固定安装路径进程扫描为空、宿主安装目录不存在，已独立核对。最终外层CLI退出0。预期拒绝中的dpkg错误、保留非包文件造成的非空目录警告及xfconf关闭提示仍保留。

## 界限与下一步

本批不重复77批完整Rust/Flutter/analyze矩阵，不生成新包、提交或发布。持续推进目标保持active。

Windows原始EXE/安装GUI仍缺已证实可用的独立账户token/交互桌面或可恢复guest。只读查询发现启用的普通CodexSandbox账户，但未验证凭据或登录入口；`HypervisorPresent`和WSL不等于已有可恢复Windows guest，当前无Sandbox可执行入口。没有登录这些账户或改变系统设置，也没有用APPDATA替换冒充KnownFolder隔离。

原计划中的规则失败GUI、完整离线设置剩余项、真实账户/校园、Windows特权Full、长期运行、其它Linux桌面/Wayland、远端CI及最终逐项审计继续保留。当前静态检索仅提供规则/界面实现与测试线索，不把测试定义或本地fixture扩大为最终签收。
