# 编辑器备份完整读回 — 2026-11-17

当前状态：专项、真实 OpenSSH、两位非作者复核及集中完整门禁通过。基线为
`7058925330b96161a25142fb5bee94fa24aa5117`。
完整产品、FILE-05/M4、三平台桌面和发行仍开放。

本批新增生产备份完整读回，再启动目标替换；失败状态与目标发布后读回失败分开。
原始草稿、旧基线、UNKNOWN/quarantine、当前授权、目标二次复核与较新草稿保留合同
继续有效。范围见[ADR 0153](../../adr/0153-editor-backup-readback.md)。

当前活动原件位于 `work/editor-backup-verification-20261117/`。
唯一 Cargo 槽继续归属同一 checkout，未切换、未 clean：
`work/recovery-batch-integration-20261115/target`，jobs 2，macOS minos 15.0。
作者不得使用 Cargo，父级持有集成和验证槽。

## 已收取的专项

`inputs-freeze-r3.json` 为 727 工程输入，aggregate
`bbe90528db8e2d0fced73848f585d149a425253bc6595a9939397643e7b9e970`。
`build-test-inventory-r4` 实际重新编译当前 app；session 测试修订在 r3 重新编译。
准确二进制、摘要与各轮冻结由 `test-binaries-r*.json` 绑定。

- session 独立编译库存 343 名，新增 30 名与 `author-handoff.json` 一致；
  `session-backup-r3` 为 `30/0/0`，`0/reaped/2.279s`。
- app 独立库存 1373 名，新 4 名逐项相等；`app-backup-r3` 为 `4/0/0`，
  `0/reaped/0.799s`。含真实按钮、备份 CLOSE ACK 前阻止目标、UTF-16 较新草稿、
  取消、目标发布后读回失败、三主题 × 明暗 × 双语的已打开状态详情及零新增请求。
- OpenSSH 独立库存 18 名，`openssh-r1` 为 `18/0/0`，
  `0/reaped/12.518s`，runner 内部 12.419s。
  新例通过标准系统 sshd 对 Unicode／UTF-16 BE／GBK／空内容各连续保存两次，
  独立重读八份原字节备份、目标和私有 mode，并确认 SSH 仍可用。
  这不能单独证明故障拒绝路径；这些由受控 SFTP public API 回归支持。
  自有服务与 89 个累计观察身份已停止，临时密钥目录已删除。

## 初始失败与修订

初始编译 r1 为 exit 101：新 app 测试的短暂 `find()` 返回值借用跨语句；
引入显式绑定后 r2 编译通过。
session 专项 r1 为 `29/1/0`：二次目标复核的 fixture 把八字节原内容换成更长
内容，因此先触发 read budget，而非例中预期的字节审核差异。
修为同长独立内容，断言精确 `Invalid("reviewed file content or metadata changed")`；
生产逻辑不变，r2、r3 的 30 项都通过。

app r2 为 `2/2/0`。一例的 writable staging 前缀少了第二个 `.`，未暂停期望
阶段；修为实际 staging 前缀。另一例揭示真实取消反馈竞态：editor authority
可能先于外层 10ms 取消计时器返回 `Closed`。保存 worker 仅在 `Closed` 且
自身 stop 已设置时映射为取消；UNKNOWN 等其他 typed 结果保持。
修复后 r3 四项全部通过。所有旧日志和退出事实保留，不追溯改签。

## 独立评审发现及当前修订

新非作者 Arendt 对 r3 严格只读源码审查，Pasteur 独立使用已核对的父级
prebuilt 执行 session/editor `66/0/0`、app/editor `87/0/0`，
各自前后重算 r3 的 727 输入和二进制 SHA，所有命令已实际 wait。
二者均为 `REQUEST_CHANGES，P0/P1/P2/P3 = 0/0/1/0`：
普通 `.tooltip()` 不支持指针移入，却提供了 220px 的可滚动长消息区域。
源码审不含独立编译／执行，功能审不含独立编译，不能把父级结果移记为独立运行。

父级新增第五个真实指针／滚轮用例。初始长消息先暴露状态触发行被多行内容
撑高，尚未到达移入阶段；`truncate`／line-clamp 单独也未约束布局，
现按现有 theme chrome 的状态栏高度设上限，保留完整 AX 和详情。
`tooltip-negative-r5` 才准确执行完溢出检查和指针移入，再因详情消失失败；
实际一例、exit 101，回溯与原件保留。
改为 `.hoverable_tooltip()` 后，新例通过实际滚轮改变内容位置、读到尾部，
切换语言仍显示当前完整消息，并可滚回开头；没有新增 SFTP 请求或写入。

一次手工负例命令错取旧 r3 feature-unification binary，exit 0 但实际运行
0 例，父级 guard 失败；保留为执行诊断，不视为负例复现或通过。随后准确
Cargo Running 路径及独立库存确认真实新例，后续二进制均显式绑定。

r4 为 727 输入，aggregate
`78d8a0c0000ce0afb661d8af0c6712a266cd44fa168b801d37c1672c39ddc4c2`。
当前精确编译库存 session 343、app 1374、OpenSSH 18；新增 session30/app5
均逐名通过，分别 `0/reaped/2.323s`、`0/reaped/0.913s`。
标准 OpenSSH 的代码与二进制 SHA 未变，不为 UI 差分重放旧服务测试。
两位独立复核的 r4 follow-up 均为 APPROVE，P0/P1/P2/P3 全 0，
原 REQUEST_CHANGES 保留。Arendt 的追加签署仅覆盖 r4 静态差分，独立编译和
测试执行均为 0。Pasteur 独立核对 1374 名 app 库存，执行宽 editor 专项
`88/0/0`，五个新例逐名 ok；owned `0/reaped/80.554407s`。
各命令前后及最终 727 工程输入与三份测试二进制摘要相等。该功能复核使用
父级 prebuilt，不能写成独立编译，也不覆盖并行变更的文档。
功能 reviewer 的外层收尾最初因 docs 变化退出 1；实际测试与 owned 回执为 0，
后续 finalizer 重新派生并保留该诊断，没有重放测试。
作者及两位 reviewer 三个自有 agent 已实际关闭；没有新建用户 chat、worktree、
分支或容器。

## 最终集中门禁

727 输入 r4 冻结后只执行一次完整 `python3 -B scripts/check.py`；
`full-gate-r1` 实际 `0/reaped/701.468731s`，无超时或中断，
active_processes=0、pipes_drained=true、errors=[]。
门禁前后与正常构建后的工程输入逐项相等，aggregate 保持上述 r4。

| 检查 | 实际结果 |
| --- | --- |
| 直接 registry 依赖策略、fmt、严格 Clippy | passed |
| 普通 Rust，92 个套件 | `3307 passed / 0 failed / 52 ignored` |
| app，包含于普通 Rust | `1372 passed / 0 failed / 2 ignored` |
| rustdoc | `14 / 0 / 0` |
| Python discovery | `293 run / 2 skipped` |
| archive helper 的另一次 Python 运行 | `30 run / 0 skipped` |
| workspace 自定义 AI controller、readiness、observer I/O、small-stack | 全部退出 0 |

从此次完整门禁真实 Running 路径选择 app 二进制，再独立 `--list` 得到 1374 名；
路径、字节数和 SHA 与 r4 专项、reviewer 使用的二进制相等，逐名库存与完整执行
结果集合相等。新 session30/app5 全部逐名 ok。OpenSSH 默认 18 项保持 ignored；
其中新用例的真实服务执行结果由上述 `openssh-r1` 单独签署，不能累加为默认运行。
`gate-summary-r1.json`、`gate-suites-r1.json` 从原日志重新派生，测试重放 0。

原件和复算入口见[封存说明](2026-11-17-editor-backup-readback-evidence/README.md)。
正常提交／推送、精确 Git 输入绑定与提交来源包另记交付结果；本批门禁不代签
上一提交的 CI 或新提交尚未完成的 CI。

## 当前边界

依赖官方查询 r1 达外层预算，`124/reaped`；r2 返回部分 registry 查询失败。
降低并发后的 r3 `0/reaped/32.890s`，在 `2026-11-16T17:48:25Z` 官方复查
37 包／113 声明全部通过；旧失败不改写，直接依赖声明和 lock 未改。
当前环境 Rust/Cargo 1.100.0；manifest 不 pin toolchain、不设 rust-version。

本批 native inventory 仍报告 Mac 锁屏、自动解锁失败，GUI 操作、截图、启动和
安装均为 0；不扩大 headless 和真实 SSH 的签署范围。
唯一 Cargo 槽继续归属当前 checkout，未切换、未 clean；全部父级验证命令均已实际 wait。
正常 release build r1 在 r3 上 `0/reaped/149.155s`，随后 UI 修订使其不再
代表最终输入，未打包或启动。r4 正常 build r2 `0/reaped/137.448s`，
打包及本机结构检查通过，GUI/MCP 均为 arm64、minos 15.0。
precommit 包的 build.commit 为 null，不伪造精确提交来源；六文件摘要已保存。
GUI SHA-256 `e5afb6f779725cefbe15e1564ab484934aeb01aae2983a3b73bb223cf484f847`，
MCP `bb415f3ae3ae7ca62bdb78abe752b50f5a3a796dc65ec99490f54ee28cb3f693`。
正常构建、包结构和未启动／未安装事实不构成本批原生桌面验收。
