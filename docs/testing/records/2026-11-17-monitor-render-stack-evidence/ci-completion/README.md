# 监控渲染栈修复的精确 CI 终态

Quality `43955379234`，源码
`bbd4eaccfa8bba2feb487ff43e0423567428bc82`，
Windows `133608443190`、macOS `133608443592`、
Ubuntu `133608443616` 全部 `completed/success`。
本地已有快照的 `updatedAt` 为 `2026-11-16T21:31:17Z`
（北京时间 `2026-11-17 05:31:17`），不将其当作本次封存时间或原下载时间。
本次仅从既存本地原件派生，没有网络请求、下载、CI 重跑或测试重放。

| 平台 | 普通 Rust passed / failed / ignored | app passed / failed / ignored | rustdoc passed / failed / ignored | Python 主 discovery run / passed / skipped |
| --- | --- | --- | --- | --- |
| Windows | 3312 / 0 / 56 | 1364 / 0 / 2 | 14 / 0 / 0 | 293 / 259 / 34 |
| macOS | 3325 / 0 / 58 | 1375 / 0 / 2 | 14 / 0 / 0 | 293 / 291 / 2 |
| Ubuntu | 3318 / 0 / 59 | 1376 / 0 / 2 | 14 / 0 / 0 | 293 / 291 / 2 |

每平台 92 个普通 suite 和 6 个 rustdoc subrun 的运行声明数、
唯一逐名结果和摘要相等；app 分别声明 1366／1377／1378 名。
普通 Rust 已包含 app；rustdoc、Python 与两个自定义 harness 分别记录。
rustdoc 的五个提前到达的 header 不强行分配到六个 subrun。
CI 没有独立 app `--list` 库存，Python 主 discovery 没有逐名 `-v` 库存。

Python 包装回归另为 57 run：Windows `53 passed / 4 skipped`，
macOS／Ubuntu 各 `57 passed / 0 skipped`。
macOS／Ubuntu 的 workspace 内嵌 Python helper 各 `30 run / 30 passed / 0 skipped`；
Windows 原日志未报告该 helper subrun。它们不重复计入主 discovery 或 Rust 数量。

三个 Rust quality gate、包装步骤均成功。Windows 三项预检全部成功；
macOS／Ubuntu 的标准 OpenSSH 及回执保存步骤成功，Windows 对应步骤 skipped。
Linux OpenSSH server 准备仅 Ubuntu 成功，其余两平台 skipped。
全量 42 个步骤原样保存在派生 JSON 中：Windows `11 success / 3 skipped`，
macOS `10 / 4`，Ubuntu `11 / 3`，共 `32 success / 10 skipped`。

原 `43945988599` 栈溢出的两个测试名在三个 app 中均逐名 `ok`，共六项核对：

- `monitor::tests::independent_disk_monitor::independent_disk_samples_survive_production_theme_buttons_and_focused_input`
- `monitor::tests::disk_monitor_minimum_workspace_languages_themes_scroll_and_selection_are_stable`

`collect-terminal-r1` 仍为 exit 1：gh 拒绝 ANSI，三份零字节输出、
完整 stderr、外层失败及当时脚本保留。
`collect-terminal-r2` 也仍为 exit 1：三个完整日志及 suites JSON 已保存，
仅外层选择 Windows `src\main.rs` 时抛出 `StopIteration`。
原 parser 未改；三份 JSON 均与本次从原日志重算的完整结果完全相等。
新增 `derive_terminal_offline.py` 只在 app 选择处使用
`suite.replace('\\', '/')`，没有调用旧下载工具。
原 r2 没有成功完成的聚合摘要或独立逐 job 下载退出／耗时回执，
本次不倒填这些字段；现有零 stderr 和可完整重算的日志按原证据强度记载。

`originals.tar.gz` 的 37 个成员保存原件、原失败、未改 parser、
纯派生 helper／结果／成功回执、原 731 输入冻结、精确 Git 绑定及验证器副本。
`manifest.json` 保存 archive 与每个成员的字节数／SHA-256。
archive 为 `800,304` 字节，SHA-256
`f5c06c21bbe58879d25e9e0066ee97b952d4d65f1cacfb401bf256328fdd4737`。
既存 29 项输入均先记录摘要，28 项文本原件入 archive；
唯一已有 Python bytecode 只记录摘要、保留原位，不执行、不重新生成、不入 archive。
封存后的验证回执与最终报告留在 `work/monitor-stack-ci-20261117/`。

731 个工程路径、字节数和 SHA-256 均从上述精确提交的真实 Git blob 核对，
aggregate 为 `2ea283af87af7e0af2fa912ebc28f9360e0c5113cd40e1111502f8cdd6d8551e`。
冻结内 `head=262a642…` 是原采集基线元数据；精确 CI 绑定另以 `bbd4eacc…` 完成。
验证器只读 archive 与本地 Git，禁止 Git 懒取对象，不解包、不访问 target：

```sh
python3 -B docs/testing/records/2026-11-17-monitor-render-stack-evidence/ci-completion/verify.py
```

本地纯派生为 `0/reaped/2.126181s`；
封存后只读重算为 `0/reaped/2.147902s`，37 成员／731 输入零 mismatch。
两个实际成功回执及封存命令回执留在上述 work 目录，原两次失败不改写。

本记录不签署当前 HEAD `4209e272…` 的后续文档或当前 dirty Windows 实现，
不扩大为原生桌面、正常应用包、安装、签名、公证或发行验收。
旧 CI 失败及依赖最新版本在线复查的 UNKNOWN 保持；本次未运行 Cargo 或改动工程输入。
