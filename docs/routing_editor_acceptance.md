# 规则编辑弹窗的原生联动验收

后续补充：第83/85批已补Linux三来源真实窗口路径；第85批Windows驱动完成工具守卫，实际运行被系统安全弹窗阻挡，尚未签收。见 [规则控件与Linux窗口](rule_accessibility_acceptance.md)及 [Windows驱动状态](windows_rule_dialog_acceptance.md)。下表保留第81批当时证据界限，不覆盖后续记录。

第81批，2026-10-18。按原计划M4.4补齐新增、编辑、删除及配置档重命名的测试证据。本批只新增 `test/native/routing_rule_dialog_test.dart`，不改变生产页面、后端、字体、绑定或安装包。

Windows、Linux均运行新4项与既有 `routing_service_test.dart` 11项，共15项通过；两端 `flutter analyze` 无问题，生产Dart边界108文件通过。新测试使用真正的RoutingService/Rust规则manager、临时磁盘目录及Flutter页面，不mock规则后端。它们属于widget/FFI联动测试，不是操作系统桌面窗口、实际网络出站或校园连接验收。

工具/测试源树475文件SHA256 `7df2976a012c9d2fdf3ee135dafc62115b1a4cf9f6b639c7078730f5e786a390`，两树一致。77批字体产品仍为474文件来源9a9ebbf5…，本批没有构建或改写四个安装产物。总汇 `build/desktop/rule-dialogs/verification.json`，最终日志为 `dialog-{windows,linux}-03.log`、`analyze-{windows,linux}-03.log`。

## 本批断言

三项参数化测试分别走prepend、profile、append的实际“新增规则”按钮，不能只凭任一文本框出现文本就判定来源正确：新增和编辑保存后都由原生inspectDraft核对规则source。

- 新增取消：两份草稿、原预览对象和磁盘字节不变。
- 新增保存：原预览失效，应用按钮禁用，生效版本仍1、文件不变，原生匹配仍DIRECT。
- 编辑取消：弹窗初值、两份草稿、原预览对象和文件均保持。
- `DOMAIN`字段数错误在编辑时拒绝，原草稿及磁盘不变；`UNSUPPORTED,example,DIRECT`可以进入草稿，但“校验并预览”报原生不支持错误，不产生预览、不能应用，原生生效决策及文件不变。
- 保存有效编辑并显式预览/应用后版本2，新域名REJECT、旧域名DIRECT；实际持久文件中的profile/overrides与编辑器一致。这个阶段没有manager重开，不称版本2重启恢复。
- 删除取消保持草稿、预览、磁盘；确认删除只改草稿，旧生效规则仍REJECT；重新预览/应用至版本3才变DIRECT。
- 卸载页面并释放manager后重新打开，版本3、三个来源各自原规则、启用状态及固定在最后的MATCH均保留，已删域名DIRECT。

第四项测试检查重命名取消/保存、应用前不改生效名称及文件；应用版本2后释放并重开manager，名称和两份配置恢复。重新挂载页面后点击“回退上一版本”，版本增长至3，页面编辑器、原生当前配置及实际持久文件恢复原profile/overrides，预览清空、应用按钮禁用。回退后没有再重开第三个manager，不扩大这一恢复证据。

临时目录清理在创建后立即注册；manager清理在打开后立即注册，手动释放幂等。准备、应用或页面挂载失败也进入测试框架清理，不只覆盖正常结束路径。

## M4.4当前证据对应

| 原计划要求 | 实现与证据 | 当前界限 |
| --- | --- | --- |
| 配置档卡片、导入预览、重命名 | RoutingSettingsPage卡片/高级编辑；本批重命名及既有模板预览联动 | 当前配置档编辑，不据此宣称独立多档目录或导出功能 |
| 新增、编辑、删除，前置/后置 | 本批三来源4项新测试及真实文件/原生决策 | 正式桌面窗口的这些弹窗矩阵仍待补 |
| 排序、启停、手动策略组 | 本次重跑的既有native rule cards测试实际启停/上移/选择及重开 | 拖动排序、跨页实际窗口操作未由此签收 |
| 规则集更新、失败保留和预览失效 | 本次重跑的native refresh事件及GeoSite错误摘要测试 | 不替代真实生产订阅联调 |
| 来源、遮蔽、当前版本、平台能力 | 本次重跑的duplicate priority、process preview/capabilities和页面联动 | 文本重复提示只描述相同条件，不是完整覆盖关系分析 |
| 连接详情的规则/动作/出站/拒绝 | ConnectionRecordsPage有相应展示，既有connection_records_page_test | 本批未重跑其测试或正式连接详情窗口；不能仅凭代码存在签收 |
| 输入域名/IP/协议/端口/进程的规则测试器 | 本次重跑的process preview和network/inbound联动 | 模拟进程信息及原生意图，不证明实际流量归属或出站 |
| 校验失败保持旧配置 | 本批两种错误、磁盘/原生决策；另有79/80批真实窗口 | 不把widget证据和真实窗口证据混用，不证明连续无写入 |

本表是M4.4的增量对应，不签署整个M4.4或整体迁移完成。实际窗口的新增/编辑/删除/重命名/策略组/详情矩阵仍需继续；原计划的真实校园、Windows原始EXE/安装GUI、可恢复guest特权Full、长期、其它Linux桌面/Wayland、远端CI与最终审计全部保留。

## 失败与复现

首轮新测试错误假设“不支持类型必须在编辑保存时拒绝”，两端各三项超时失败、重命名通过。原生editor只进行字段/结构校验，完整类型能力由prepare编译检查；修订测试分别检查字段错误与未知类型的真实阶段，仍保留错误提示、应用禁用和原文件/决策不变断言，没有更改生产校验。首轮源码副本和日志保留。

中间15项已通过，但analyze发现一个缺大括号的info并返回1；该结果没有升格。修正后最终两端15项、analyze和边界通过；独立复核还促成来源、取消状态、回退持久文件及失败清理检查的补充。

```powershell
$env:FRB_DART_LOAD_EXTERNAL_LIBRARY_NATIVE_LIB_DIR = "$PWD/rust/target/release"
flutter test test/native/routing_rule_dialog_test.dart test/native/routing_service_test.dart --reporter expanded
flutter analyze
python tool/verify_dart_boundary.py
```

Linux使用同名环境变量和命令，库目录需有配套Linux release库。该路径是本次测试的开发库目录，不是安装GUI证明。完整Rust、Flutter全套、117项Python工具测试及生产构建本批未重跑；既有记录保持原批次来源。无提交或发布，整体goal active。
