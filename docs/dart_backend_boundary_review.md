# 生产 Dart 后端边界复核

第83批增量复核（2026-10-18）：亲读规则列表、规则页按钮接线及新LabelledIconButton。旧Tooltip只提供tooltip字段，未形成Linux按钮名称；新组件合并显式label与既有按钮语义，视觉Tooltip排除重复语义，保留默认tap/focus/disabled状态，不覆盖回调。规则列表给动作/开关增加来源和行号，新增按钮采用来源唯一的label，仅排除其可见Text子节点的重复语义；规则页仅替换重命名及最终兜底图标包装。新增文件为纯view，清单从108增至109、view从36增至37。规则编辑/解析/校验/存储仍由原生负责，无新运行依赖、绑定或pubspec变化；其余封存条目未重新宣称全量审阅。运行和无障碍导出仍需独立验证。

第77批增量复核（2026-10-18）：用户指定霞鹜文楷屏幕阅读版，亲读并修改 `theme.dart`、`shuyo_text_styles.dart`、`log_page.dart` 的字体展示接线。全局/独立样式选择本地字体，日志继承同一字体；pubspec只新增字体与许可资源，无新增运行依赖或锁变化。按实际代码复核刷新三项 view 文件及 pubspec 的清单摘要，108文件分类保持。字体资产、布局和运行验证另记，不以边界检查证明渲染成功。

第67批增量复核：`desktop_lifecycle.dart` 的原生协议点击会使 GTK 勾选先变化；即使协议选择无变化或被拒绝，也需将最新 UI 快照重新发给原生菜单。刷新标记在原生更新前消费，允许在途事件再次置位；它不执行认证、隧道或业务重试，退出仍委派原生 owner。本次亲读该文件并更新对应 SHA/说明，新增幂等与在途回归先失败后通过；不将这一个文件的复核扩展为全仓重新签收。

2026-10-17，第64批。范围为仓库拥有的 `lib/**/*.dart` 和 `rust_builder/lib/**/*.dart`，不把测试、探针、第三方 Flutter SDK 或构建缓存混入生产源码。当前 `rust_builder` 只有原生构建接线，没有 Dart lib 文件。

本次建立覆盖当前源树的复核基线，不单凭检索或检查器签署整个 M5.4。平台、实际产物、真实校园和完整 GUI 的剩余验收仍按原工程计划保留。

## 清单与生成物

当前增量更新后的 `dart_backend_boundary_manifest.json` 逐文件记录路径、规范化 LF 的 SHA256、职责和复核说明，共109文件：

| 职责 | 文件数 |
| --- | ---: |
| Flutter/原生初始化装配 | 4 |
| 页面、控件与视觉表现 | 37 |
| 窗口/托盘适配 | 5 |
| 展示接口/DTO/格式化 | 22 |
| 手写原生命令/快照适配 | 10 |
| 生成的 FRB/Freezed DTO 与编解码 | 31 |

固定 `flutter_rust_bridge_codegen 2.11.1` 对当前 Rust API 重新生成后，31个 Dart 输出及 `rust/src/frb_generated.rs` 与重生成前逐字节一致。生成目录包含 Freezed 输出，不仅依据目录名或注释将其豁免。`web: false` 的原生生成器仍保留 `dart.library.js_interop` 条件分支的 web stub 引用；这是唯一缺失目标例外，仅适用于该实际条件 URI，不能用注释或无条件 import 触发。

生产直接依赖为11项，清单同时封存 `pubspec.yaml`、`pubspec.lock`、生成配置和 Rust 绑定。旧 Sangfor Dart SDK、HTTP/HTML/加密后端包不在生产导入中；crypto仅用于开发/测试。

## 责任判定

core的36个非生成文件做了全文复核。features/shell/widgets的30文件完成逐文件枚举和调用/导入/异步搜索，对认证、分页、预取、设置应用、规则编辑及更新等候选执行代码核验；这不冒充30文件逐行人工完整阅读。主线程核验app/main的装配、视觉/路由代码、依赖与生成物，并按关键file:line抽查子任务证据。

在已检查范围没有发现独立 Dart HTTP/Socket 数据面、Cookie/凭据加密与解析、配置/凭据数据库、业务重试或旧SDK后台回落。以下判定不能因出现await、JSON或Timer而无限追加迁移：

- `connection_controller.dart` 的协议/登录快照预检、busy、generation、重复命令合并与释放订阅是UI命令接线；正式bootstrap只装配Rust backend，连接、替换、清理与退出由原生owner执行。原生仍负责授权，UI预检不替代授权。
- 连接设置页等待原生设置提交后，根据当前界面运行状态派发服务启停/重绑，是显式用户操作；它不执行OS恢复或代理/TUN业务步骤。保存失败不继续后续动作。
- 通知页的滚动游标、可见列表URL去重、按月份分组和触底再次请求属于列表交互。请求、分页目标校验/终止、缓存、预取并发、取消及预算在Rust。
- 详情页读原生cache snapshot后直接展示或发原生命令；不在Dart制定TTL、淘汰或网络退避。
- 登录表单每秒刷新原生挑战的剩余秒数，Rust账户适配每250ms采样展示快照；没有Dart冷却递减、二维码网络轮询或认证阶段重试。
- URI/日期/JSON解码用于展示DTO与命令封装，规则解析/编译/匹配、DNS及更新版本/资产判定仍在Rust。
- 候选渠道的 `ShuUpdatePolicy.enabled=false` 是显式UI产品开关，不能视为仍有Dart更新协议实现。

五处 `dart:io` 例外有单独复核说明：

| 文件 | 实际用途 |
| --- | --- |
| `lib/main.dart` | Windows/Linux桌面平台选择 |
| `lib/app/bootstrap.dart` | 应用支持目录、路径分隔符及原生owner装配 |
| `lib/core/rust/rust_core.dart` | 只读定位并加载配套原生库，FRB健康检查 |
| `lib/core/desktop/plugin_desktop_port.dart` | 托盘资产及gdbus UI宿主能力探测 |
| `lib/features/onboarding/welcome_page.dart` | 按平台选择权限说明文案 |

旧偏好例外仅在 `RustAccountCenter.open` 与 `RustSettingsStore.open`：把旧键作为输入交给原生，原生提交成功后删除旧键；不维持第二份长期Cookie/凭据/设置数据库。浏览器/企业微信唤起、剪贴板、窗口/托盘操作属于必要UI平台接口。

## 自动复核门禁

```shell
python -m unittest discover -s test/tool -p test_dart_boundary.py
python tool/verify_dart_boundary.py
```

工具检查完整文件集合、已复核内容、职责、运行依赖与锁、生成输入、条件导入中的后台依赖、越界引用测试后端，以及手写平台IO是否有明确说明。所有源文件变更都要求重新审阅并更新清单；不提供自动批准新内容的命令。LF规范化只消除Git跨平台换行差异，不忽略代码或空白之外的内容变化。

头部扫描支持普通metadata、注释和条件URI，不是完整Dart语义分析器；泛型header metadata等未支持形式明确拒绝，不能提前停止后悄悄放过imports。检查器也不能证明获批IO例外内部从未做业务决策，因此内容SHA、源代码复核、Flutter分析与运行证据缺一不可。不得只改清单SHA使CI变绿。

两条CI及Windows/Linux本地 `verify_desktop` 已接该门禁；Flutter CI与原生矩阵使用同一固定Flutter 3.47.2及锁文件。远端运行结果尚未签收。

第71批增量复核（2026-10-18）：完整阅读 `desktop_host.dart` 与 `desktop_lifecycle.dart`，将托盘普通断开接到 `ConnectionController.disconnect`，退出接到 `disconnectForExit`，分别转发原生 owner 的非终止断开与终止清理。窗口销毁仍等待原生退出清理完成。亲自复核 `connection_controller.dart` 的 disconnect/exit 命令与状态通知代码：命令 future 清除后再通知 UI，防止最后一次通知仍暴露 busy=true 导致托盘连接无法恢复。新增 host 组件和两条监听回归均先复现旧行为；修改只调整 UI 命令接线及通知时机，不增加后端业务实现。开发工具中的本地协议 GUI 入口不属于生产导入清单；它使用真实默认 Rust 库和生产界面，但替换账户/连接计划入口，不能作为正式登录或 production bootstrap 验收。

本批已有实际配套库的启动、账号边界、命令取消/退出、登录挑战、原生设置提交/迁移/恢复等针对性运行证据。全量产品测试和平台Full/GUI证据继续引用对应批次，不能把此源码清单当作全部平台/真实校园已完成的证明。`lib/main.dart`中保留的旧Sangfor错误类型注释是文档历史，不是可执行SDK调用。
