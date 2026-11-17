# 精确协议增量的 CI 终态

工程 `de9768c183c22f1051baabd529600667697247a1`，三个工作流均为
attempt 1 的自然 `completed/success`。终态观察时间为
2026-11-17T04:25:45Z；不是从当前文档 HEAD 或后续隧道候选推定成功。

| 工作流 | run | 终态与边界 |
| --- | --- | --- |
| Quality | 44013808155 | Windows、macOS、Ubuntu 全部 success |
| Windows monitor integration | 44013808184 | API job success，独立 isolated-SSH 前置 job 仍 skipped |
| Windows OpenSSH monitor integration | 44013808220 | 真实 SSH job success |

| Quality 平台 | 普通 Rust passed / failed / ignored | rustdoc passed | Python主 run / skipped | 真实 OpenSSH |
| --- | --- | --- | --- | --- |
| Windows | 3406 / 0 / 60 | 14 | 523 / 50 | 此 Quality 步骤 skipped |
| macOS | 3419 / 0 / 62 | 14 | 523 / 3 | 18 passed |
| Ubuntu | 3412 / 0 / 63 | 14 | 523 / 3 | 18 passed |

每平台94个普通套件的声明、逐名结果与摘要相等；没有独立执行 `--list`。
两个 `harness=false` controller 的实际执行和三个额外controller gate独立检查，
不将它们伪装为普通库存。

原件包含7个未改写ZIP、156个成员，CRC全部通过；4个artifact的ZIP摘要均
匹配GitHub API digest。56份命令输出摘要已复核，子进程全部wait/reap、
无超时；首次日志下载的转义保护失败与后续成功重试均保留。
新非作者Feynman已独立解析API、ZIP和精确Git输入，限定批准，
P0=0/P1=0/P2=0/P3=0；主复算5,091断言和交付16检查全部通过，已实际关闭。
765输入均读取指定旧提交的完整Git blob后核对，不以当前HEAD代签。
56份Collector回执为55次exit0和首次转义下载exit1，均自然退出且reaped；
SSH原件的20个非master自然终止、master清理exit125分别保留，
不声称全部原对象自然成功。API的`ssh=UNRUN`也继续保留。

`originals.tar.gz`保存207成员／5,368,560逻辑字节，
压缩后1,814,889字节，SHA256
`a8202205d13b283ee24e605f23891db000780bdc336c220e00bab27dbbaddc85`。
[manifest.json](manifest.json)逐件映射Collector及非作者原件；
归档内`independent-review/REVIEW.md`、`results.json`与`review.py`
保留范围、独立复算结果和工具。它们是离线检查，不是新的产品测试。

此结果不覆盖本地转发的新候选、GUI、IME、辅助技术、安装、签名、公证或
六目标发行；独立peer的空文件／缺OO失败继续保持。前批in_progress快照
保留于[交付原件](../delivery/README.md)，未改写成当时已经成功。
