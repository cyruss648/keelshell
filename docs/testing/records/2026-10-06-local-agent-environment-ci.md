# 本地智能体环境引用：精确提交三平台 CI

公开提交 `24a19b954b2634c3f54d9554e5ce9b04a0062e9b` 的
[Quality 37417055579](https://github.com/cyruss648/keelshell/actions/runs/37417055579)
attempt 1 已完成，实际 API 为 `completed/success`，更新时间
`2026-10-06T05:26:49Z`。三个 job 的 head、checkout 日志和两份 OpenSSH
artifact 的 workflow-run 身份均匹配该精确提交。

| Runner | 普通 Rust / doc 通过 | 普通 ignored | scripts Python | Packaging |
| --- | --- | --- | --- | --- |
| macOS 26 | 1238 / 8 | 13 | 6 通过 | 57 通过 |
| Ubuntu 24.04 | 1238 / 8 | 14 | 6 通过 | 57 通过 |
| Windows 2025 | 1219 / 8 | 13 | 6 通过 | 53 通过、4 平台条件跳过 |

逐条测试名与 libtest 摘要计数相等，0 failed。三个平台均实际执行 x.y 策略、
格式、严格 workspace/all-targets/locked Clippy 和锁定 workspace 测试，
工具链为精确 `1.98.1`。Windows 四项包装跳过检查涉及 Unix executable
permission bits 或合成 Unix staging 的执行权限，不能算作通过。

| Runner | 默认 / 2 MiB 控制器耗时 | 两次实际 future 大小 |
| --- | --- | --- |
| macOS | 11.843775 / 10.657437 秒 | 5176 字节 |
| Linux | 8.988926 / 8.975579 秒 | 5176 字节 |
| Windows | 72.473221 / 69.092185 秒 | 5536 字节 |

每个平台的两次控制器各 626 条连续阶段记录、38 对 TCP begin/returned，
12 connected / 26 refused；各 95 对 scratch 检查结束，没有超时或记录容量异常。
默认和 2 MiB 均真实执行，未提高 45 秒 Unix / 90 秒 Windows 控制器期限。
这些自有 native process fixture 不调用安装的供应商 CLI、模型或账户。

macOS 与 Linux 后续各执行 9 项真实 localhost OpenSSH 测试，外层分别
16.154 / 10.042 秒，收据确认 61 / 62 个已观察、带内核出生标识的自有
进程身份停止，未验证祖先集合为空，生成凭据目录删除。Windows 该步骤跳过。
没有独立停止后 TCP refusal 收据，也没有全机器或完全脱离观察的后代普查。
普通 ignored 不计入 workspace 实际执行；Mac/Linux 的九项在后续步骤单独运行。

新的非作者只读取证保留 35 份 job 日志、14 份 artifact payload、16 份精确
提交源码及全部 API/解析结果，118 个 payload。两个 archive 的实际长度和
SHA-256 与 API digest 相等。OpenSSH result.json 的 logs 字段绑定其中
12 份日志的长度；另外两个 result.json 正文由 archive/API 摘要和清单绑定。
最初解析器误要求 Windows 57 项全部通过，失败及原代码保留；修正跳过分类后
实际解析通过。根再次完整读取全部正文及副本，独立重现 Rust 摘要和六份控制器
trace，与保存 JSON 相等。

本轮只验证该已提交版本的源码工程、自有 process fixture、localhost OpenSSH
和打包回归。正在主副本整合的逐目标参数及传输候选不在此 CI 中。原生 GUI/AX、
真实 CLI 模型、六目标 Release、签名公证、安装和自动更新仍须分别验收。
既有 `block v0.1.6` future-incompatibility warning 保留，不把当前严格检查通过
解释为修复了该上游提示。本轮没有提交、推送、发布标签或安装应用。
