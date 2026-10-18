# Linux 桌面权限边界

2026-10-14，当前交付范围为 Windows 与 Linux。本文记录 Linux 普通用户 TUN 的权限设计和实际验证边界；整体交付验收仍以 `desktop_migration_rust_core_plan.md` 为准。

2026-10-17 第63批补充：第62批正式DEB的安装库与固定helper已通过普通UID/CapEff0的Full自动及手动入口、DIRECT双栈TCP/UDP、DNS/HTTPS、校园签名通道、拒绝与授权、外来路由冲突、helper强杀、重建和正常关闭验证。原helper退出用pidfd确认，全部IPv4/IPv6路由表和策略规则恢复基线；控制连接在会话仍存活时恢复原绑定。测试加载`/opt/shuvpn/lib/libshuvpn_core.so`，不使用测试helper路径变量，主进程/Flutter引擎无capabilities。复现及证据范围见[安装版完整捕获验收](linux_installed_full_capture_acceptance.md)。下文较早批次的“尚未在默认库重跑”是历史状态；本批仍不代表真实校园、完整GUI或长期验收完成。

## 权限归属

Flutter/GTK 主程序保持普通用户身份，不给使用 `$ORIGIN/lib` 的 GUI 可执行文件设置 capabilities。独立 `shuvpn_tun_helper` 只承担创建非持久 TUN、配置其地址及分流路由、同步动态主机路由，以及对传入的 IPv4/IPv6 TCP/UDP socket 设置或恢复物理接口绑定。协议不接受 shell 命令、任意文件路径或进程内整数 fd。

正式客户端只从 `/usr/lib/shuvpn/shuvpn_tun_helper` 启动程序，检查固定路径各级目录和文件为 root 所有、非符号链接、不能被组或其他用户写入。随包 `install.py` 将 GUI 安装到 `/opt/shuvpn`，安装 root:shuvpn、0750 的独立 helper，授予 `CAP_NET_ADMIN` 与 `CAP_NET_RAW`，并用专用组限制可执行用户。GUI 不以 root 运行。构建默认不包含 `tun-probe`；只有诊断构建能指定临时 helper 路径或请求故障注入。

## IPC 与所有权

客户端创建私有 `AF_UNIX/SOCK_SEQPACKET` socketpair，通过继承的 fd 3 启动 helper。helper 核对内核 `SO_PEERCRED` 中的父进程身份，在核验前设置父进程死亡信号。通道无文件系统监听路径，也没有网络监听端口。请求有版本、递增序号和大小上限，未知字段/操作、截断消息及多余描述符会被拒绝；收到的 fd 使用原子 `CLOEXEC`，在解析前纳入 `OwnedFd`。

helper 完成 TUN 创建后，用 `SCM_RIGHTS` 将同一内核设备的 fd 引用交给普通用户进程。包收发继续复用 `PacketFd`，不通过 JSON 复制每个 IP 包。helper 保留配置 owner；关闭时客户端先取消并排空本地 fd 操作，再请求 helper 关闭其引用，回读接口 index 消失后才确认。关闭 TUN 后 helper 仍可恢复物理 socket，直到出口 owner 也结束。

客户端的有界 actor 持有已经入队的请求，调用 future 取消不会截断一次协议交换。创建结果还需要接收方明确接管：接收方取消、校验失败或不接管时，actor 关闭 helper 的设备 owner。失败复制 fd 的创建事务也先回滚，不把“返回错误”当成已清理。重复创建被拒绝，避免旧 lease 的析构误关后来创建的设备。

actor 监视实际 helper 子进程退出，并通过弱引用登记撤销本地 `PacketFd`。helper 崩溃、运行时取消或通道失败时，应用端不能继续持有一个无人管理的捕获接口。正常关闭仍由会话 owner 显式等待；析构回收是补充路径。

Linux helper 启动与退出确认使用内核 pidfd，需要 Linux 5.3 或更新内核。子进程在 exec 前为自身打开 pidfd，通过现有私有 socket 传回父进程；父进程同步接管后才开始异步握手。退出以该 pidfd 可读为依据，允许宿主运行时已经回收子进程的 wait 状态；不能仅凭 `ECHILD` 判定退出。强杀及析构也使用 pidfd，避免向复用后的数字 PID 发信号。正常退出等待与强杀后的等待各有 3 秒期限，无法确认时仍返回失败。该启动帧不占用 helper 协议 2 的命令编号。

## 已验证范围

`tool/tun_linux_helper_smoke.sh` 在 root harness 中为临时 helper 设置 capabilities；每个场景另建网络命名空间，再以 UID 65534、有效 capabilities 为零的进程调用。namespace 基线由 root 在 `unshare` 前读取，普通用户只读自己的 namespace 与该基线比较，不要求其读取 `/proc/1/ns/net`。

已通过的诊断 helper 内核场景：

- 普通用户通过传递的 TUN fd 收取 IPv4 UDP 包并注入回包，动态主机路由新增/撤销、保护地址拒绝、取消读、重复关闭和接口消失。
- 对普通用户拥有的真实 UDP socket 进行接口绑定、解除绑定，并在恢复后实际接收字节；该用户直接解除绑定被内核拒绝。
- 未指定网卡名称时，两个进程使用同一生成名称并正确撤销。
- 缺少 capabilities 时拒绝启动；已有同名接口保持；部分 Netlink 创建失败回滚。
- 创建成功后强制 fd 传递失败、创建结果未被接收、保留 broker 时丢弃 lease，均回收网卡。
- SIGKILL helper 后撤销应用端 fd；SIGKILL 普通用户父进程后，helper 进程及接口消失。测试 harness 自行回收故意孤立的子进程，避免依赖 WSL PID 1 的回收行为。

原始失败和修正后日志在 `build/desktop/linux-helper/`；以 `kernel.log` 的完整退出结果为准。已对正式 bundle 中不带故障注入的 helper 用同一脚本的第二个 helper 路径参数复核，`release-helper-kernel.log`完整退出零。故障注入场景只对诊断二进制执行，其余场景在正式随包二进制上全部执行。

复现命令（Linux checkout 内）：

```shell
cargo build --manifest-path rust/Cargo.toml --locked --features tun-probe \
  --bin shuvpn_tun_helper --example linux_tun_helper_smoke
sudo bash tool/tun_linux_helper_smoke.sh rust/target/debug
flutter build linux --release
sudo bash tool/tun_linux_helper_smoke.sh rust/target/debug \
  build/linux/x64/release/bundle/shuvpn_tun_helper
```

临时目录和可执行文件属于 root，仅测试 UID 65534 所在组可读取/执行；普通用户不能修改 helper。脚本退出会删除临时副本，不在宿主安装 helper 或给 GUI 授权。系统需要 `libcap2-bin` 提供 `setcap/getcap`。

## 正式会话接线与剩余安装验收

第三十七批已将高层 Linux TUN 接入 managed device，按GUI实际权限选择本地设备或独立helper。启动在publish前准备同一broker，将其交给Egress与清理lease；物理TCP/UDP/DNS、已有连接的保护/恢复，以及HTTP请求级连接器都使用同一登记路径。退出先取消并等待在途socket创建，重新收集已登记连接，恢复后才确认helper进程结束。drop触发的清理任务和owner有明确登记，失败可重试。

取消保护future不代表broker队列中的操作被撤销，因此恢复始终提交原接口并等待队列确认，不能仅凭getsockopt当前值相等就跳过。这一竞态已有确定性失败前后回归。helper协议2增加Shutdown；成功关闭明确等待真实子进程退出和broker worker join。正常权限不足或版本不匹配的握手也等待进程结束。

`tool/tun_data_plane_linux_smoke.py --desktop-helper <profile> <helper>`由root在新netns配置双栈veth与物理peer，以UID65534/CapEff0执行正式startDesktopTun。已通过校园IPv4 UDP及双栈TCP、DIRECT双栈UDP/TCP、DNS/TCP回落/错误ID、HTTPS信任、拒绝与授权、停止/重启与原socket绑定恢复；helper PID消失在父进程仍存活时核验，对端观察到的源地址也核对为物理地址。该探针需要同时构建tun-probe与egress-probe，并使用诊断构建的临时helper定位。它没有代替固定路径安装或默认正式FFI库的证明。

第三十八批将 `tun_driver_info` 改为异步：GUI 自身已具备权限时使用本地设备；否则核验固定可信路径、执行实际协议2握手，确认 helper 获得所需权限，再等待其真实退出，才报告 `available/helperReady`。仅有可执行文件、旧协议或不能提升权限不会被当成就绪。该检测不创建 TUN；诊断环境变量不能改变安装可用性检测。

Linux 正式包现在包含 `install.py` 与 `shuvpn.desktop`。退出应用后，在解压目录执行：

```shell
sudo python3 ./install.py --user YOUR_LOGIN_NAME
```

需要 Python 3 与 `libcap2-bin`。安装按包内清单校验复制后的每个文件，拒绝不安全路径及符号链接；GUI、helper 与菜单入口通过备份/重命名事务一同升级，授权失败回滚已发布路径。提交后的备份删除失败会明确输出残留路径，不冒充已经回滚。新增组或卸载后的组成员关系保留，用户需重新登录让新组生效。账户与设置目录不由安装程序读取或删除。

```shell
sudo python3 /opt/shuvpn/install.py --uninstall
```

`test/tool/test_linux_install.py` 的9项测试覆盖完整性、路径、链接、发布/授权失败回滚、卸载及提交后备份删除失败。`tool/linux_install_smoke.sh <解压包> <编译后的FFI探针> <普通用户>` 在新挂载/netns内使用 overlay/tmpfs 隔离 `/etc`、`/usr/lib`、`/opt` 与菜单目录，实际运行安装、升级、卸载。普通用户加载包内**默认正式库**已验证：授权用户可用；未授权用户、组可写helper、NoNewPrivs、移除file capabilities均不可用；每种状态重复检测三次，最终核对无残留helper，宿主账户文件摘要不变。探针为 `dart compile exe tool/linux_install_smoke.dart -o <输出>`。CI已接线；本地完整验证可显式设置 `SHUVPN_LINUX_INSTALL_NAMESPACE=1`，实际远端CI尚未运行。

同一harness还接受额外的 `<编译后的tun_dns_bridge_smoke> <proxy_fixture>` 参数。增强的既有DNS探针以普通UID/CapEff0加载安装后的默认正式库，执行 `startDesktopTun` 自动选择shuegress、MTU1400，已通过校园DNS查询/元数据、动态域名路由发布、真实校园TCP收发、规则中的原始域名身份、finishStop无诊断及网卡消失；全部helper PID消失和宿主账户文件摘要不变也通过。测试使用固定安装路径，不使用临时helper覆盖变量。当前日志为 `build/desktop/linux-install/installed-tun-ffi.log`。

这补齐固定安装路径与默认FFI的权限及实际TUN证明，尚未代替完整GUI操作、helper崩溃后独立长连接恢复、真实校园和长期运行验收；默认FFI本轮数据场景为校园DNS/TCP，不能冒充完整DIRECT双栈/HTTPS矩阵已在此默认库上重跑。

本机主netns的最小SO_BINDTODEVICE回环TCP对照超时，隔离netns同UID可收发；尚未确定该环境行为的根因。Linux完整网络/HTTP单测保持断言和期限，通过 `rust_linux_namespace_tests.sh`在普通UID的隔离netns运行；本地完整验证可显式设置 `SHUVPN_RUST_TEST_NAMESPACE=1`，不会静默跳过测试。
