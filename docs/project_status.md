# 项目整理与当前进展

核查日期：2026-10-19。当前版本为 `0.4.2+9`，仅维护 Windows/Linux。项目处于本地桌面候选与验收阶段，尚未完成正式发布验收。

## Git 提交整理

本次提交保存现有桌面迁移源码、测试、构建工具、资源与文档。根 `.gitignore` 补齐 Dart/Python 缓存、CMake/Ninja 状态、编辑器文件、本地环境文件及系统元数据；`.env.example`、`.env.sample`、`.env.template` 允许作为示例提交。24个忽略路径与19个源码保留路径均验证符合预期。

Rust与Dart锁文件、生成绑定、平台插件注册文件、字体/OFL、固定Wintun驱动及许可证纳入版本管理。`.gitattributes` 标记生成绑定，并对生成绑定及三份上游许可证明确保留原始空白格式，避免手动重写生成内容或改变许可证摘要。缓存、构建产物和本地测试日志保留在忽略目录中。

2026-10-19 提交前复核：Rust格式检查、109文件Dart边界清单、Flutter静态分析均通过；Windows完整Flutter测试344项通过，使用当前release Rust库与仓库固定驱动。Python工具测试191项，171通过、19项平台跳过、1项仍因缺少引擎校验清单而错误。日志位于 `build/desktop/project-commit/`。本次没有重跑完整Rust套件、Linux平台矩阵或真实校园验收。

提交使用作者与提交者 `Cans518 <798061533@qq.com>`。这次Git快照不代表私有引擎、最终平台验收或发布已经完成。

## 整理结果

此前第92批已删除两端14个可重建缓存目录中的120,802个文件，共123,106,819,665逻辑字节，约114.652 GiB。原记录中，D盘可用空间从32.182 GiB增至85.573 GiB；WSL内部另增加49.747 GiB，未压缩VHDX。这些数字来自此前清理回执，不能当作本轮新增回收量，也不能将逻辑删除量等同于宿主磁盘回收量。详细范围见[有限清理记录](semantics_geometry_acceptance.md)。

本轮复核并完成：

- 删除 Windows/Linux 后续再生的16个 Python 字节码文件，共341,027字节。
- 删除本轮沙箱测试失败后留下的5个空测试临时目录。
- 复核此前清理清单：Windows的8个目录、Linux的6个目录均已不存在。
- 在根 `.gitignore` 加入 `__pycache__/` 和 `*.py[cod]`，防止测试缓存进入 Git；后续工具检查使用 `python -B`，减少字节码再生。
- 重新审查现有测试、stub及验证工具，未确认有可以删除的过时测试源码，本轮删除测试源码0个。当前工作树中此前已删除的旧 Dart/Android 测试属于原迁移改动，不重复计算成本轮清理成果。

保留账户、凭据、设置、规则配置、依赖下载缓存、真实FFI所需原生库、现存候选包、历史验收日志和当前私有引擎研究产物。旧候选及失败证据仍被迁移记录引用；本轮不将这些内容认定为无用数据。

可机器核查的本轮回执位于：

- `build/desktop/project-cleanup/windows-cleanup.json`
- `build/desktop/project-cleanup/linux-cleanup.json`
- `build/desktop/project-cleanup/retained-packages.json`

这些回执随本地构建资料保存，不属于需要发布的应用源码。

## 2026-10-18 清理验证

| 检查 | 实际结果 |
| --- | --- |
| 全量 Python 工具测试 `python -B -m unittest discover -s test/tool -p 'test_*.py'` | 191项：171通过、19项平台跳过、1项错误 |
| 清理后 Dart 边界工具回归 | 15项全部通过 |
| `python -B tool/verify_dart_boundary.py` | 109个生产 Dart 文件的审阅清单及导入边界通过 |
| Python 缓存 Git 忽略规则 | `tool/`、`test/tool/`、`linux/` 示例路径均被忽略 |
| 第90批保留的 ZIP、tar.gz、Inno EXE、DEB | 四份文件重新计算 SHA-256，均与各自摘要文件一致 |

全量工具测试的唯一错误是 `test_linux_engine.EngineManifestTest.test_committed_patch_matches_manifest`：`tool/verify_linux_engine.py` 要求的 `tool/flutter_engine/source.json` 不存在。目录中也没有配套补丁。该入口属于尚未收尾的私有引擎校验工作，保留测试与错误，不通过删测、跳过或构造无依据的清单使其转绿。

沙箱内首次测试因新建临时目录不可访问而出现权限错误；上表工具测试与边界回归采用随后获准的沙箱外执行结果。清理未修改 Dart/Rust 生产代码，本轮没有重跑完整 Flutter/Rust 套件或重建两端应用，也没有重新验收安装、特权TUN或真实校园连接。

## 已实现与已有验收

以下是当前代码及既有验收记录的范围，不代表本轮重新执行全部验收：

| 方面 | 当前进展 |
| --- | --- |
| 架构 | Flutter保留界面和薄适配；账户、配置、公告、认证、规则、代理和连接所有权已迁入Rust。109文件边界清单本轮复核通过，运行期验收仍单独计算。 |
| 网络能力 | aTrust桌面入口、HTTP/SOCKS5、校园分流/实验性完整捕获、规则预览/应用/回退、DNS和连接记录已有实现；EasyConnect/OpenVPN未启用。 |
| Linux | 既有隔离环境记录覆盖普通用户helper、双栈数据、DNS、安装升级卸载、完整捕获、规则管理和连接详情GUI。不能扩大到所有桌面或真实校园环境。 |
| Windows | 已有桌面候选、Inno安装和部分GUI/helper/代理验证；当前规则三来源编辑、原始安装及连接详情GUI等矩阵仍有缺口。 |
| 分发 | 四种候选文件保留在 `build/desktop/rule-geometry/package/`，本轮摘要复核通过；它们不包含后续私有引擎修复，也不表示已正式发布。 |
| Linux拖动 | 第93批私有引擎的正式main在隔离X11/GTK环境完成三来源六次物理拖动、取消/应用/重开，八个GUI进程退出0。默认SDK、CI及现有分发包尚未接入该修复。 |

私有引擎成功范围见[隔离引擎与真实拖动记录](engine_source_build_acceptance.md)。

## 当前未完成项

1. **私有 Linux 引擎仍未具备默认接入条件。** 较新的 `build/desktop/engine-tree/sdk-tests-runtime.json` 记录选定56项测试通过；但随后 `lifetime-red.json` 的AddressSanitizer诊断退出1，在 `fl_view_accessible_handle_update_semantics` 的冻结实验源码第265行复现 `heap-use-after-free`。这份更新晚于第93批汇总，不能用旧GUI成功或56项绿色结果覆盖它，也不能据此认定正式发布包含有同一实验缺陷。
2. **引擎校验资料未齐。** 上述 `source.json` 与配套补丁缺失，使当前全量工具测试保持1项错误；应在确认引擎修复和固定来源后补齐真实输入并复验。
3. **最终平台验收未完成。** Windows剩余GUI、可恢复guest中的特权Full/TUN、真实校园账号与网络、更长运行、其它桌面/Wayland、既有未归因故障、远端CI及最终迁移逐项审计仍需完成。
4. **尚未正式发布。** 迁移改动与整理记录在本次Git提交中保存；未完成的平台和引擎验收继续保留，本次不生成安装器或发布版本。

后续开发应先解决私有引擎生命周期回归和校验输入缺失，再接入默认构建并执行受影响的GUI/平台矩阵。详细历史、方案和证据分别保留在[迁移计划](desktop_migration_rust_core_plan.md)、[实施记录](desktop_migration_progress.md)和[交接文档](desktop_migration_handoff.md)中；清理任务完成与整项桌面迁移最终完成分别判断。
