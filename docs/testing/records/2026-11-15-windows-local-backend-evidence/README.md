# Windows 本地文件边界与 Linux 专项证据

状态：`AUTHOR_GATE_PASS / NON_AUTHOR_SOURCE_APPROVED / WINDOWS_NATIVE_NOT_RUN`。
设计见 [ADR 0138](../../../adr/0138-windows-handle-local-files.md)，实际结果及
限制见[集中记录](../2026-11-15-windows-local-backend.md)。

本目录保留 **1301 份原件**，导出时逐字或无损压缩、读回和来源 SHA 全部一致；
[完整清单](archive-manifest.json)映射旧工作路径、持久 artifact、大小和两个摘要。
`raw/` 保留原件字节，大于 64 KiB 的大部分文件使用 `.gz`；读取时解压，不修改
原报告的行尾、空白、失败或签署时边界。报告内绝对工作路径是运行时位置，
工作目录回收后按清单读取对应 `raw/` 文件。

## 接续入口

| 证据 | 位置 |
| --- | --- |
| 最终 588+14 工程冻结 | [combined-inputs-r2.json](raw/combined-inputs-r2.json) |
| 最终门禁、suite、新用例逐名结果 | [gate-summary-r2.json](raw/gate-summary-r2.json) |
| 门禁实际退出/时间/日志摘要 | [full-gate-r2.json](raw/logs/full-gate-r2.json) |
| Windows 四个 PE 和 143 个描述符 | [windows-pe-final-r1.json](raw/windows-pe-final-r1.json) |
| 正常 macOS GUI/MCP 摘要 | [normal-build-artifacts-r1.json](raw/normal-build-artifacts-r1.json) |
| 最终 Linux 运行与清理 | [receipt.gz](raw/linux-final-r2/receipt.json.gz) / [cleanup](raw/linux-final-r2/cleanup.json) |
| 平台 r3 独立源码批准 | [report-r3.md](raw/review-platform/r3/report-r3.md) |
| Linux runner r2 独立批准 | [report-approve-r2.md](raw/review-linux/r2/report-approve-r2.md) |
| 最终 Linux 原件独立核验 | [report-approve-r3.md](raw/review-linux/r3/report-approve-r3.md) |
| Session r2 组合源码批准 | [session-combined-review-r2.md](raw/review-session-windows/session-combined-review-r2.md) |
| 六项非作者预构建专项 | [prebuilt-special-report-r2.md](raw/review-session-windows/prebuilt-special-report-r2.md) |
| SDK 来源与摘要限制 | [sdk-payloads.json](raw/windows-sdk/sdk-payloads.json) |
| 失败导出与既有 redactor 测试占位分类 | [export-attempt-r1.json](raw/export-attempt-r1.json) / [classification](raw/redaction-sentinel-classification.json) |
| 自有 agent 关闭 | [agents-closed.json](raw/agents-closed.json) |

旧平台 r1、runner r1、session r1 HOLD，已中止门禁与失败检查、所有 notice、
红回归和旧快照结果保留。notice-003 只限定旧 Linux 快照不能改签变化后的树，
最终 r2 有自己的新运行。预构建专项独立执行没有被称为独立重编译。
Windows PE 字节扫描不是 native `--list` 或执行，Windows 原生运行数为 0。

## Git 绑定与资源

提交前已从 Git index/blob 读回全部 **602 项工程输入**与 **1301 份原件**，
见 [index-input-binding.json](index-input-binding.json)；
提交后使用同一清单核验精确功能提交及当前文件。绑定回执与清理回执放在本目录
根部，不覆盖 `raw/` 或原始报告。可使用 [verify_binding.py](verify_binding.py)
复核 index 或一个精确 commit，输出路径必须尚不存在。

SDK/CRT 下载与解压、公开 registry payload、编译探针和重复测试可执行文件等
临时内容没有作为源码提交。其逐项大小、摘要及排除理由见
[omitted-artifacts.json](omitted-artifacts.json)，共 `2,550,695,050` 字节。
来源、编译、ELF/PE、完整库存及清理回执保留。
完成 index/blob 审计并重新核验原件后，已删除任务工作目录，
回收 `2,609,954,825` 字节临时内容；三个 reviewer 已关闭，六个自有容器
按完整 ID 再次确认 absent，见 [cleanup-final.json](cleanup-final.json)。
唯一有效 Cargo target 与其他任务资源保留。

真实 Windows 文件系统/桌面、三平台完整桌面、IME/辅助技术、strict peer
HOLD/PARTIAL、FILE-05/M4、完整产品与发行安装仍分别开放。
