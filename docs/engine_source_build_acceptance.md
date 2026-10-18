# 隔离 Linux 引擎构建与真实拖动验收

第93批，2026-10-18，部分完成。独立构建修补后的 Flutter Linux 引擎，并用正式 `lib/main.dart` release 完成三来源物理拖动闭环。结果限定在私有引擎与 X11/GTK 环境；默认 SDK、CI和现有发布包尚未接入补丁，迁移目标保持 active。

## 构建来源

从原始 SDK Git `d3b14c876900e553bc736ca19295fc09e3853e8e` 创建独立、无 hardlink/共享 Git common directory 的检出 `/home/mzee/.cache/shuvpn/engine-geometry/flutter`。私有 depot_tools Git 为 `d38909eb68d85b91259f4bf20fe5ebcd38461bc2`。按固定 DEPS 同步依赖，仅执行 Dart 配置/版本、离线 pub、x64 sysroot 所需 hooks；系统库未安装/替换，原 SDK保持。工具更新、metrics、交互凭据及Git hooks禁用，工具缓存/XDG/TMP均在私有根。DEPS仍下载JDK和带Android名称的工具，不能据custom_vars声称完全没有这些下载。

GN参数为 `--runtime-mode=release --no-lto --no-build-glfw-shell --no-build-embedder-examples`，保留原生测试；固定检出内GN/Ninja及私有sysroot，以并行度4实际构建GTK引擎、headers、Dart工具、sky_engine、字体/着色器工具和Linux测试。首次构建因审查发现回归主动停止、退出2；修复后增量构建退出0，两份记录保留。

应用原始源码复制到独立 `engine-geometry/app`，重新生成pub/CMake状态；Cargo缓存按值复制，仍实际执行locked/release构建，无预构建旁路。私有SDK以local-engine三参数构建正式main。首次release误选depot_tools Ninja wrapper失败，随后只修正子进程PATH及该次失败的私有CMakeCache，重试成功。

| 对象 | 来源 |
| --- | --- |
| 原发布产品与四包 | 90批，495文件，`0ecc5605c0f6950ef23a14b532dab56c429612e1a711d85732dc2ce12557ee22` |
| 本批隔离main | 495文件，`f4a84895e16e9c1fb9998610826f2e5e480890e27ddf675aeba1d60df82321ad` |
| 修复夹具后两原树的开发驱动 | 495文件，`c451f0e8e239c4c6f76a011cde7a46fbf266b49289f550d759cb776f29196acc` |
| 实际引擎DSO SHA256 | `669f84b268fa2f6dae352943c9ac38f339c685ac307067a335b603bcd25b6b06` |
| 实际系统ATK bridge SHA256 | `13670b19f87dbf32b697104cb5019c91ad886d8122e8d645c27162c97e41e877` |

缓存元数据engine Git `a804b261…`、输入内容hash `1cf1c477…`、SDK Git及带补丁DSO分开记录；不声称原缓存二进制已认证或完成可复现构建。

## 源码修复及原生验证

FlViewAccessible由AtkPlug改为本地AtkObject/AtkComponent，经FlSocketAccessible直接子对象暴露，使用弱parent及GTK原生坐标边界，避免同连接plug/socket同步D-Bus调用。语义树组合transform/rect并缓存部分更新；几何/parent先提交，再通知。改变8文件，另冻结未变的fl_value.cc，共9文件摘要见build-fixed-runtime.json。

独立审查实际复现空根state set、首次root add过早、重入clear丢失、销毁时重新挂接等回归：provider-unit-05的34检查中7失败，修复后06的34检查全部通过。最初夹具借用wrapper生命周期/运行名变量错误另保留失败日志，不归因于应用。

| 验证 | 实际结果 |
| --- | --- |
| source-only几何矩阵 | 20断言通过 |
| 本地provider生命周期/通知/重入 | 34检查通过，退出0 |
| 原始provider，跨进程AT-SPI | 首次几何超时并捕获self-call，定向终止−15，作为失败对照 |
| 修补provider，系统ATK，跨进程AT-SPI | 8检查通过，6次Component RPC、0 self-call，退出0 |
| 实际构建的SDK单元测试 | 5组54项通过，0失败/错误 |

54项限FlAccessibleNode/FlAccessibleTextField/FlViewAccessible/FlView/FlAccessibilityHandler，未跑完整引擎套件。

## 正式入口真实窗口结果

会话均使用私有Xvfb/D-Bus/XDG、普通用户、零有效capabilities，无LD_PRELOAD、私有ATK库或vtable hook。逐GUI进程maps绑定引擎和系统bridge路径/SHA。

drag-local-engine-01/02在首次拖动后完整JSON等式超时。02只读原文记录确认拖动已把prepend的DIRECT移到REJECT前；Rust编辑器会序列化夹具遗漏的空proxy-groups/rule-providers/selections，因此整文等式失败。截图捕获时还在短暂编辑状态，不能仅据截图认定手势无效。

仅在开发driver fixture显式写出三个空字段，保留实际原文/完整JSON/首匹配/所有权/可见性门禁。独立字面量回归先red后green；另证明三个非规则字段若被篡改，即使同步伪造committed原文，仍被拒绝。两端工具各183项通过（Windows跳过19/Linux跳过7）、边界各109文件通过。Linux首次漏设镜像来源环境的9错误保留，补环境后通过。

最终drag-local-engine-03：CLI0、passed=true、八GUI进程全部正常退出0：

- 三基线、导入、三来源编辑及最终重开均完成。
- 六次真实鼠标拖动，prepend/profile/append各两次；每来源先放弃恢复，再重拖并显式应用。
- 从当前具名手柄实际AT-SPI rect取点，如prepend中心 `(392,426)→(392,265)`，没有猜截图坐标。
- 拖动撤销旧预览；未应用时原生生效REJECT保持，应用后版本4/5/6、已应用来源首匹配DIRECT。
- 新PID重开与独立native probe核对完整原文、三来源顺序及首匹配，最终版本6，前后置哨兵保持。
- 应用/驱动/引擎源码、全部私有载荷、宿主link/route/rule前后保持。

所有收据位于 `build/desktop/engine-geometry/`：verification.json、build-fixed-runtime.json、local-provider.patch、provider-unit-05/06.json、provider-remote-original/local-02.json、sdk-tests-runtime.json、gui-product.json、drag-local-engine-01/02/03、linux/windows-final-state.json和cleanup-retention.json。02/03的raw-observations.jsonl均归档。完整补丁对当前私有源码的 `git apply --reverse --check` 通过。早期manifest的formal_gui_fix_verified=false为当时状态，本批最终GUI收据补充了隔离main成功结果。

## 清理及剩余范围

92批有限清理14目录/120802文件/114.652GiB逻辑量保持；当时D实际可用32.182→85.573GiB，WSL内部另增49.747GiB、未压缩VHDX，不等同宿主回收量。未找到可确信废弃源码/测试，删除0。本批两端各重新生成8个pyc，共341027字节；Windows tool/__pycache__删除被自动审批以blocked by policy拒绝，未执行/未绕过，两小缓存保留，其余12已清理目录仍不存在。前批C盘半成品仍因原拒绝保留；当前需要的引擎工作产物和失败证据保留。

收尾核对原始SDK、原Linux bundle全部载荷、90四包、字体/OFL保持。本次私有bundle的TTF/OFL/FontManifest与原产品相同。Windows SDK的CRLF源码与冻结LF输入规范化比较，实际摘要分别记录，不称原始字节相同。已知GUI/构建/probe PID均消失，无工作构建目录残留进程。

无新发布包、候选安装或提交发布；补丁尚未接入默认SDK/本地/CI，默认门禁未转绿。原fl_accessible_node_set_children已有children重排/移除parent清理的静态疑点仍需专门运行回归，不能签收完整provider适用性。下一步先固化该回归，再接入默认流程复验。

完整Flutter/Rust本批未重跑。Windows三来源/原安装与连接详情、真实校园、guest提升权限Full/TUN、更长运行/其它桌面/Wayland、84批原故障、远端CI及最终需求审计保持未完成，整体goal不关闭。
