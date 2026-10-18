# Linux 安装版完整捕获验收

第67批复验（2026-10-17）：当前DEB SHA `119c03bf89af7d6251ad73244bd59e2f2f09866c0f631bb374b7a0ddfa584821` 已重新执行本页默认安装路径的全部数据/故障矩阵。core仍为 `d4732d4f1521809dab8f6ef0ab99817da4f19174cce7cde5352a4b719da3cc38`，helper已变为 `061c64a1058778b8a48f2564f45732b05c9a4ae2780f13ceaf64f199965f72c6`，因此未沿用旧helper的字节级验收。实际库/helper摘要匹配本次DEB，普通UID/CapEff0、固定helper身份/pidfd、十阶段完整性、独立对端拒绝负例和全路由恢复通过，外层安装生命周期及宿主状态保持均退出0。新证据在 `build/desktop/tray-protocol/installed-full/verification.json` 及本批总汇；以下第63批记录保留历史来源。

2026-10-17，第63批。验证对象是第62批 `d26c2827760b` 候选的原始 DEB；本批修改测试入口和 CI，没有重建或替换运行库、helper 或完整应用。

## 权限与来源

- Flutter 测试引擎实际 UID 为普通授权用户，四个 UID 一致且非0，`CapInh/CapPrm/CapEff/CapAmb` 为0，`NoNewPrivs=0`。
- 明确加载 `/opt/shuvpn/lib/libshuvpn_core.so`，在引擎和 root 观测端检查该进程的 maps。库和 helper 的运行前后 SHA256 必须等于本次 DEB 提取的原始 payload。
- 正式默认库使用 `/usr/lib/shuvpn/shuvpn_tun_helper`。环境中移除测试 locator，root 核对活跃 helper 的固定 exe、inode、SHA、直接父进程、UID、netns 与所需 capabilities。
- root 在观察前打开 helper 的 pidfd，并在身份读取前后确认仍存活；清理时以相同 pidfd 确认原身份退出，同时扫描替代进程、接口和监听。
- `/etc`、包数据库、安装目录和菜单路径均位于私有挂载空间；数据流和独立对端使用私有网络空间。普通测试进程的 net/mnt 必须等于准备环境、不同于外层环境。使用私有 PID 空间时不以 `/proc/1/ns/net` 作父空间基线。

这些测试使用真实 Flutter/RustConnection 与原生 owner 的服务启动，但认证来自本地协议 fixture 的计划入口，不使用学校账户，不代表真实登录或校园验收。

## 实际数据与故障

一个测试执行三个 Full 会话：

1. 原生 owner 自动启动 Full；验证预览/实际状态、校园 IPv4 UDP及双栈签名TCP，非校园双栈TCP/UDP、64 KiB TCP往返与FIN、HTTPS正确证书及不受信证书拒绝。DNS经TUN接管，覆盖A/AAAA、UDP/TCP、截断后物理上游TCP重试、错误ID拒绝、域名映射驱动规则及歧义拒绝。
2. root在负例前后比较物理对端的TCP接入记录，配合原生TUN决策/错误/无成功出站，验证REJECT、校园授权不足及DNS歧义没有产生新的对端TCP接入。root随后安装额外物理主机路由；引擎必须报告覆盖变化、自动清理且保留该外来路由，测试方只删除自己注入的路由。
3. 再次手动建立Full，完成双栈UDP后由root通过已观察pidfd强杀helper，核对session与owner故障信息、完整退出，再次重建Full并正常停止。正常停止TUN后、关闭会话前，核对仍存活控制连接的原生接口绑定与启动前一致。

每轮结束都核对TUN消失、helper原身份退出且无替代进程、HTTP/SOCKS监听拒绝连接、原生owner退出屏障、出口保护关闭。IPv4/IPv6**全部路由表及策略规则**必须与基线一致，物理对端只可看到预期物理源地址。

基线取于虚拟物理接口的IPv6 DAD完成后。初轮完整路由检查捕获到新增的物理链路本地local路由；独立无VPN对照证明这是接口初始化，现通过实际地址状态等待稳定，保留严格全表比较，没有过滤该路由。

root与普通进程通过固定十阶段协议握手。阶段名、顺序与完整性均校验；确认文件以已打开目录fd、`O_EXCL/O_NOFOLLOW`写入。绝对/穿越路径、符号链接、漏跑故障阶段和重复阶段有独立回归，不能仅凭早期成功标记签收。

## 复现

先按桌面构建说明准备固定Flutter工具链和完整DEB，以及普通用户构建的探针：

```bash
dart compile exe tool/linux_install_smoke.dart -o build/linux-install-ffi
dart compile exe tool/tun_dns_bridge_smoke.dart -o build/linux-installed-tun-ffi
cargo build --manifest-path rust/Cargo.toml --locked --release --example proxy_fixture

flutter_root="$(dirname "$(dirname "$(readlink -f "$(command -v flutter)")")")"
dart_runtime="$flutter_root/bin/cache/dart-sdk/bin/dart"
sudo env SHUVPN_DART_RUNTIME="$dart_runtime" \
  SHUVPN_DEB_FLUTTER_ROOT="$flutter_root" \
  SHUVPN_DEB_FULL_REPORT="$PWD/build/desktop/installed-full-new" \
  bash tool/linux_deb_smoke.sh /absolute/path/to/ShuVPN-version-linux-x64.deb \
    "$PWD/build/linux-install-ffi" "$(id -un)" \
    "$PWD/build/linux-installed-tun-ffi" "$PWD/rust/target/release/examples/proxy_fixture"
```

报告目录必须是新的绝对路径。源码依赖和工具链缓存应预先准备，实际测试引擎以指定普通用户运行。SDK位于`/opt`时，harness先保留底层目录，再在原路径挂私有overlay；普通用户缓存写入只进入私有层，专门的写入标记核验不出现在原SDK。相关本地模拟已运行，不能据此声明远端CI通过。

入口为 `tool/linux_deb_smoke.sh` → `tool/installed_full_capture_smoke.py` → `tool/full_capture_adapter_smoke.dart` 的安装模式，数据断言位于 `tool/installed_full_capture_case.dart`。root driver复用既有物理peer fixture，生产Dart中没有新增数据面。保留未指定安装模式时的原root bridge探针。

本批证据在 `build/desktop/installed-full/`，汇总为 `verification-current.json`。DEB安装/升级/remove/purge、账户与非包文件保留、授权/拒绝矩阵及宿主账户/包数据库不变仍由外层harness核对。Windows特权Full、真实校园、长期、完整GUI与最终M5.4审计仍独立保留。
