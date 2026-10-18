# 无障碍运行库隔离实验

第93批补充（2026-10-18）：实际构建独立Flutter引擎，以本地ATK子树避免同进程plug/socket同步调用，系统bridge保持。20几何/34生命周期/8跨进程/54相关SDK项及隔离main三来源六次真实拖动通过，八GUI进程全0；前两轮原文超时源于fixture漏Rust默认字段，已补完整空值而保持所有门禁。默认SDK/CI/发布包尚未接入，91通用桥库适用性不因此获签收，见 [隔离构建范围](engine_source_build_acceptance.md)。

第92批补充（2026-10-18）：已从私有诊断runner取得正式AOT的两条真实几何链，经固定原始C++实际ATK输出重现86/140像素偏移，实验源码补丁及通知回归修复合计23项通过。尚未进入正式引擎或通过真实拖动；91批运行库的通用适用性限制保持。用户要求的有限缓存清理与完整边界见 [语义几何与清理](semantics_geometry_acceptance.md)。

第91批，2026-10-18，部分完成。本批确认同连接同步 D-Bus 调用的最小复现，验证私有库中的局部分派修复，并加强开发期拖动守卫。正式窗口拖动仍失败；实验库未进入应用分发包，整体迁移目标保持 active。

## 同连接最小复现与隔离修复

以固定 `at-spi2-atk 2.38.0` 原始归档构建原版和实验版，源归档 SHA256 为 `cfa008a5af822b36ae6287f18182c40c91dd699c55faa38605881ed175ca464f`。缺少的 Meson0.61.2由 Ubuntu 包0.61.2-1下载后仅解包到本轮 build；未执行系统安装。两个库分别装到本轮独立 prefix，原系统库保持。

实验仅改变 `socket-adaptor.c`。同连接目标经桥接服务自身 registry 解析，临时持有实际 AtkSocket 引用，直接调用对应 geometry API；非本连接继续原 D-Bus 路径。v2 增加规范路径往返检查，GetPosition/GetSize分别分派，失败本地目标不再回退到同步 self-call。仅在默认 main context 被持有时走本地路径，另有线程局部重入保护。

原生测试夹具只创建 ATK对象，不启动 Flutter/Dart，不访问账户。自定义 socket 对 extents、position、size 故意返回不同值，并覆盖三种坐标类型。所有会话拥有私有 Xvfb/DBus/XDG，逐进程读取 `/proc/PID/maps` 核对实际库及摘要。

| 对照 | 实际结果 |
| --- | --- |
| 系统2.38.0-3，同连接 | 首次GetExtents卡住，捕获 sender=destination，控制器定向终止-15 |
| 新构建原始2.38.0，同连接 | 同样卡住并捕获self-call，定向终止-15 |
| 实验v1，同连接 | 程序完成但20项中10项失败，暴露方法分派及路径别名错误 |
| 实验v2，同连接 | 20项、0失败，原生进程退出0 |
| 原始2.38.0，跨进程 | 7项、0失败，两个原生进程退出0 |
| 实验v2，跨进程 | 7项、0失败，两个原生进程退出0 |
| 两版上游测试 | 各droute-test/atk-test两个可执行测试通过 |

v2私有库 SHA256 为 `92224c0b0abdd5de94a69487930fa4ead97385a90d885b3703fb1f442e980f5a`。20项包括错误路径、路径别名、根/自身目标、一个循环探针、恢复正常查询及 context未持有时的拒绝；并非20项均为正常查询。

这些结果证明限定夹具中的修复，不证明通用运行库可发布。尚未覆盖自定义 vfunc 的合法嵌套查询、真正的自定义重入分支、工作线程/其它 main context、并发 registry销毁、超范围数字路径及远端错误路径。默认 context ownership本身也不等于任意线程安全。v2的线程局部布尔保护会拒绝其它合法嵌套本地查询。未将 upstream原始包等同于已逐补丁复核的 Ubuntu `-3` 源码。

## 正式 main 的三次拖动实验

保留90批正式 main/配套原生库，运行时仅在本轮私有会话中指定上述库路径。每个GUI进程都核对实际映射；没有修改系统库、AOT、Rust、绑定、依赖或字体。各次前三个基线进程与第四个导入进程退出0，完整规则导入版本3；第五进程失败，不能写成八进程通过。

`drag-private-01` 解除了旧GetExtents超时，取得48×48矩形后发出一次真实 mousedown/12次移动/mouseup，但没有改变规则原文；`passed=false`。之后发现旧驱动只检查SHOWING，未检查由隐藏语义控制的VISIBLE。

新增守卫先red后green：必须VISIBLE=true，缺失/false/非bool均拒绝。`drag-private-02` 没有发出拖动，在32次旧范围内查找后停在“两只手柄尚未同时可见”；截图显示目标段仍部分在视口上方。没有把该拒绝改为成功。

随后补查找范围和裁切测试：先向上搜索，每方向最多32步，拒绝非正方形手柄，逐次记录probe，并在真实拖动前后截图。`drag-private-03` 最终7次probe找到两只声明可见的完整手柄，发出一次真实拖动；仍等不到前置规则顺序变化，`passed=false`。

第三轮返回中心 `(392,512)` / `(392,405)`，SHOWING及VISIBLE均true，矩形均48×48，但点击前截图中对应手柄并不在这些位置。由此确认：解除同连接等待后，仍有可见节点几何与画面的第二层问题，不能把VISIBLE或完整矩形本身当成功证明。前后截图、鼠标释放记录、原文及失败均保留，没有猜测替代坐标或放宽原文检查。

只改 `tool/desktop_rule_drag_acceptance.py` 及其测试。最终源码495文件、SHA256 `f4a84895e16e9c1fb9998610826f2e5e480890e27ddf675aeba1d60df82321ad`，两树一致；两端工具181（跳过19/7）、边界109、针对12项通过。中间88599a83…及180项通过另存。正式产品/四包仍是90批0ecc5605…，本批没有重建应用、生成新包或重跑完整Flutter/Rust。

下一步应对语义坐标链建立最小修复验证，重点是 SDK 的节点 rect/transform 与父节点坐标叠加；不要继续把取得矩形当作可点击证明。运行库补丁仍需单独完成重入、线程及分发评估，再决定如何进入默认产品路径。

## Windows 原生滚动接口

本批使用90批正式 Windows 包的私有资源副本，两个普通进程退出0。对所属窗口、PID、唯一启用的“新增前置规则”调用 `GetCurrentPattern(10017)`，实测返回空指针；supported=false，scroll_requested=false。未调用ScrollIntoView，也没有HRESULT失败值可据此归因为E_FAIL。前后规则文件、宿主应用目录元数据及代理键摘要保持。

SDK源码中该UIA方法暴露provider但缺少action赋值，是源码调查结果，不等同于本次实际provider调用了该方法。限定源码的Windows COM map也没有IAccessible2接口；没有取得可替代的IA2滚动调用链，不将未执行的QI写成runtime不支持证明。

## SDK 来源补强与边界

本轮找到并在两端独立重算 engine内容指纹：HEAD中 `DEPS`、`engine`、release marker的217字节`git ls-tree`输出，经Git blob hash和独立Python SHA-1均得到 `1cf1c4773fb941c4c74a7f8bb144a8837596c0f4`，与engine_stamp.json声明完全相同。SDK HEAD为d3b14c876900e553bc736ca19295fc09e3853e8e，元数据声明engine git revision a804b261645ef8c13eb3d5c44a5c2fb0340c5539。它们不是同一个值，也不能把a804说成本轮计算出的内容哈希。

这补上“SDK输入内容与artifact元数据一致”的独立证据；未验证二进制签名、可复现构建或真实供应链来源。未执行会更新缓存的Flutter版本脚本，没有改SDK。

总汇 `build/desktop/accessibility-runtime/verification.json` 仍为 partial。当前默认产品的GTK问题、Windows三来源/原始安装与详情GUI、真实校园、guest Full、长期及其它桌面、84批故障、远端CI和最终审计保持未完成。没有提交、发布或新安装器安装。
