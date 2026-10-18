# Linux 前置规则真实窗口弹窗验收

当前补充（2026-10-18）：第83批已修复规则控件的显式名称，并把驱动纳入常规工具/CI；新正式构建的三来源窗口路径已通过。见 [规则控件与三来源验收](rule_accessibility_acceptance.md)。下文保留第82批旧产品、无名按钮和仅prepend试跑的历史范围。

2026-10-18，第82批补充。使用77批原始字体Linux bundle的正式main，在私有Xvfb/DBus/Openbox和新XDG profile中补做前置规则新增、编辑、删除的实际窗口路径。报告 `build/desktop/connection-soak/rule-gui-03/verification.json`，五个GUI进程均UID1000、CapEff0、默认库maps正确、退出0；主线程已查看重开、删除确认和最终配置截图。

运行工具源为479文件224734f6…，产品仍为77批474文件9a9ebbf5…，不是新生产构建。此次驱动及独立Dart读回器保存在构建报告目录，属于开发验收试跑，尚未集成常规工具/CI。

先完整重跑已有离线设置与非法规则路径，版本2仅校园profile和两个非空前后置DIRECT覆写保持。第四进程实际通过前置区“新增规则”按钮输入：

```text
DOMAIN,created.gui.invalid,DIRECT
```

新增取消保持两份草稿和原文件；保存后原生编辑器回读确认在prepend末尾，原生文件仍版本2且字节不变。编辑取消同样保持草稿和文件；保存为 `DOMAIN,edited.gui.invalid,DIRECT`，再次精确回读prepend，预览仍不落盘，显式应用才变版本3。

第四进程关闭后，独立Dart进程加载同一Rust库读取规则并实际匹配：created域名REJECT、edited域名DIRECT，原两个覆写域名仍DIRECT。第五进程重开，实际raw字段与版本3持久文件中的profile/overrides完整相同。

实际打开删除确认框，截图可见“删除这条规则？”和仅修改草稿的说明。取消保持草稿及文件；确认删除只改变草稿，版本3字节不变；再次预览仍不写入，应用才变版本4。关闭后独立原生读回确认edited域名恢复REJECT，前后置原覆写仍DIRECT。版本4没有再启动第六个GUI进程，不扩大重开范围。

输入通过真实xdotool键盘并经Text接口完整核对；控件通过本次PID的AT-SPI动作操作，不称物理鼠标点击。Linux provider没有为规则卡片的编辑/上移/下移/删除图标给出名称；本次以唯一domain Text绑定卡片，要求恰好一个toggle与四个push button，再按亲读widget的edit/up/down/delete顺序选择并记录索引。运行效果由弹窗、精确草稿、文件不变、版本及独立匹配交叉检查。这不是图标无障碍标签已修复或语义名称完整的证明。

原run-01在视口外的覆写字段定位失败；run-02已完成新增/编辑/取消，但预览按钮仍在视口外而失败。均保留passed=false、截图和日志。最终run-03用ListView左侧padding中的真实滚轮定位，避免滚动多行编辑器内部；没有变更生产页面或把缺失节点当作成功。

本次只签前置规则真实窗口路径；profile/append仍只有81批widget/FFI三来源证据。安装GUI、Windows同类弹窗、排序/策略组/连接详情窗口矩阵和完整M4.4继续保留。五个GUI及外层启动器PID已退出；这是窗口及原生规则验收，没有真实校园连接、代理数据流、TUN或系统代理启用。Linux长跑提前EOF另行调查，不能由本页成功消除。
