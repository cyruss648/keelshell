# 文件审核展开与真实编辑焦点 — 2026-10-07

当前更新：新的非作者逐项核对源码、原始证据和实际程序，并独立通过四项新增 GPUI 行为及 92 项文件工作区回归。结论为限定范围无阻断，主线组合及新原生窗口仍待验证。后台差异检查可经固定“返回文件”到原取消入口；这两步可达的源码事实不代替键盘或 VoiceOver 验收。以下保留作者原时点。

状态：独立作者工作树基于精确 `b54b83089bbf2166f1a8351a5535e220f4946163`，
仅修改 files 呈现、meaningful 回归与本文档。作者 scoped files 92 项实际通过，
包括四项新增受控 GPUI 测试；格式、x.y 策略与 workspace/all-targets 严格 Clippy
实际退出 0。751 份最终 gate 输入前后和当前源码逐字相同，随后仅更新 ADR 与本文档
的结果状态。未提交、未安装、未启动原生应用；新的非作者复核、主树整合与原生
验收尚待完成，不继承其它候选的门禁通过。

## 原始依据与范围

作者此前作为非作者已逐字节核对受控 macOS 合并/patch 业务记录与全部 16 张
2880×1866 原始截图和 AX。原证据封包 SEAL SHA-256 为
`924f5a87e3c828694ec9f02eb38789d9b0abf918fabb1e6b4c262827fbfb6cbf`，
manifest SHA-256 为 `f5f14d5b033735b04246ab6437fdc178ed31de69ea7928a27649cf9e051b7448`。
它证明独立远端更改、采用合并不写远端、人工确认保存与严格 patch 到草稿后另行保存，
仍仅限 macOS 中文 System 深色、自有隔离 SSH/SFTP 服务，无客户/MCP/模型。
原输入、原失败、原截图与字节收据均保留，未修改或删除。

07 的三行完整合并同时可见。保存审核 AX 保留全文，但约 112 physical-pixel 正文只
分段显示；14 虽命名 middle，实际只有 tail，不能宣称 middle 已可见。03 的首次输入
误落目录字段后被恢复，这是操作失误，未证明产品焦点缺陷。17 的 patch 使用真正 AX
setValue，但输入区当时在屏幕之外，未证明键盘可达。原检查没有关闭长横向、重叠冲突、
英语/明暗、最小窗口、VoiceOver 或 Windows/Linux 验收。

## 候选与测试设计

新增展开视图使用文件面板空间，保留默认 48 logical-pixel 紧凑区、逐行原文、两轴、
固定确认/取消；视图切换不替换 operation/snapshot/目标/字节。新提案回到紧凑/顶部。
固定草稿/差异入口复用原 TextareaState，显式展开与 focus；状态和返回操作保持可见。
无 transport、worker、权限或预算变更。

新增四个受控 GPUI 测试保留现有 Harness 的有界 idle 等待，旧断言和期限均不变：

- 两尺寸与中英/三主题、真实助手可见：逐原行 Label 相等，展开后容量增长，三个短结果行
  同时在正文 bounds 内，实际方向键/End/wheel 两轴可达，正文 Enter 不批准，固定动作
  不越界；收起/展开与新审核重置不改目标和字节，取消后 WRITE 为零且远端基线不变。
- 最小文件面板的固定入口：真实 scoped keyboard 输入草稿与严格单文件 patch，断言
  Textarea 焦点及精确内容，目录字段逐字不变；patch 仅改草稿，人工审核后才实际保存。
- 展开审核中草稿变化仍拒绝旧确认；新审核从紧凑开始，暂停丢弃授权与展开状态并保留
  草稿，远端内容与 WRITE 计数不变。
- 目录镜像的全部完整路径和哈希逐行保留，展开后的固定动作可达；取消不改变审核
  目标、源文件或目标文件，并且不发出删除请求。

测试与源码、每次前后输入映射、raw stdout/stderr、实际 Popen.wait 结果、私有 TMP 和
所属进程组终结收据保存于忽略的作者证据目录。编译或测试失败完整保留；不扩
原期限或削弱旧断言。最终精确源码冻结后另由新的非作者复核。

## 实际作者门禁与保留失败

以下命令由各自独立 Popen 进程组在私有 TMP 执行，单次外层 watchdog 保持 300 秒；
均取得实际 wait 返回值并 reap。每次 751 输入前后相等，全部 raw 与 SOURCE_BEFORE
已逐字节核对。所有已知 PGID 与私有 TMP 在终态核对和封包前再次实际确认不存在。

| 执行 | 实际 wait | 秒 | 结果 |
| --- | ---: | ---: | --- |
| compile-v1 | 101 | 125.592 | 作者初次代码/测试的借用、Button 参数与 Focusable 导入错误，保留完整 raw |
| compile-v2 | 101 | 9.952 | 测试 String::as_ref 类型推断错误，保留完整 raw |
| compile-v3 | 0 | 27.428 | scoped app test executable 构建成功；随后去除无效 clone |
| viewport-v1 | 101 | 9.536 | 初版 Harness 的独立 FilesPanel 布局未进入真实 workspace 容器，三个展开入口不可达，保留 raw |
| viewport-v2 | 0 | 22.275 | 四项新增 GPUI 测试通过，12 种尺寸/语言/主题组合保留逐行与双轴记录 |
| files-v1 | 101 | 42.131 | 91 passed/1 failed；原最小布局断言实际观察 21px 工具区，要求至少 28px，保留 raw |
| files-v2 | 0 | 64.693 | 保持原断言，将固定编辑入口仅放在没有 pending 的浏览布局；92 files 测试全部通过 |
| fmt-v1 | 0 | 1.349 | cargo fmt --all --check |
| policy-v1 | 0 | 0.093 | scripts/check.py --policy-only，直接依赖 x.y |
| clippy-v1 | 0 | 88.411 | cargo clippy --workspace --all-targets --locked -- -D warnings |

files-v2、fmt、policy 与严格 Clippy 使用完全相同的 751 输入（15,243,924 字节）。
files-v2 包含四项新回归；不把早期 viewport-v2 的源码时点代作最终 gate。最终测试
可执行文件已实际全文读回：246,311,616 字节，SHA-256
`df1bdfb7d02ab9c7a84d2a7ff2620aa6cc53b8de92a792e02b9d0a9f8a44cf49`。
作者目标目录来自已有构建缓存的私有 CoW 副本；缓存准备实际 0/112.083 秒，不是
构建成功证据。首次证据目录创建的 FileNotFoundError 也保留；当时未启动 Cargo/GPUI。

作者仅完成 scoped files 测试及上述工程检查，未运行完整 workspace 测试或新原生
GUI。expanded editor 在后台 patch 检查时显示状态，可通过固定「返回文件」回到原
取消入口；该交互仍需独立复核和原生操作，不概括为完整键盘/辅助技术验收。

## 验收边界

受控 GPUI 与自有 TCP SSH/SFTP 的上述 scoped 通过不等于原生像素、
VoiceOver、完整辅助技术操作、客户环境或 Windows/Linux 桌面验收。新的 macOS 原生
布局与键盘复验需要单独协调；本次不接管既有应用或其它 owner 的句柄。
