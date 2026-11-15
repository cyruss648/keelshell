# 服务端归档工具检测 / Server archive tool inspection

PATH 观察的既有工程门禁及非作者源码复核已通过，原生平台交互仍需单独验收。设计见
[ADR 0133](../adr/0133-reviewed-server-archive-tool-inspection.md)，实际命令、
测试结果与预构建复核限制见
[集中验证记录](../testing/records/2026-11-15-archive-tools-pax-integration.md)。

## 使用

1. 在当前已连接 SSH 会话的文件面板展开「传输设置」。
2. 在「归档传输 → 服务端工具」选择「审核检测」。
3. 核对当前主机、完整固定命令和检测边界，再明确确认。
4. 查看结果摘要，按需展开「工具详情」。重新检测需再次审核。

检测观察 tar、gzip、zip、unzip、python3、rz、sz 在当前 SSH 命令环境的 PATH
解析。它不运行这些工具，也不会上传脚本、安装软件、压缩、解包或启动 Zmodem。
不是交互终端当前目录或手工修改过的环境快照。

## 状态与限制

- 「已找到」表示观察到绝对路径，不表示版本、权限、格式支持或互通已验证。
- 「未找到」仅表示本次 PATH 解析没有找到工具。
- 「待确认」表示解析结果不能确认是可用的绝对路径。
- shell/protocol 不支持、非法或过量输出、超时与取消属于检测未完成；不会据此
  显示七个「未找到」。重新检测失败后也不会继续展示旧的成功结果。

审核绑定当前真实 SSH 连接，连接替换或结束后必须重新审核。结果不进入持久配置，
不用于自动执行任何操作。主题、语言和详情开关不产生新的远端请求。

服务器实际打包和现有 SSH PTY 上的审核式 Zmodem 已实现，分别见
[服务器归档](SERVER_ARCHIVES.md)与 [Zmodem](ZMODEM.md)；严格 peer 互通、
用户中继和性能收益仍按各自记录验收。客户端 ZIP/tar.gz 工作流见
[归档传输](ARCHIVE_TRANSFERS.md)。

## 版本与流式自测

该入口的集中工程门禁、真实 OpenSSH 16/16、Linux arm64 helper/Python 专项及
限定非作者复核已通过；本批原生桌面仍未验收。设计见
[ADR 0139](../adr/0139-reviewed-archive-tool-validation.md)，实际计数、原始失败
及预构建复核限制见[本批记录](../testing/records/2026-11-15-archive-tool-validation.md)。

在同一服务端工具区选择「版本与自测」，审核当前主机和完整命令后再确认。
这次授权会运行七项工具的固定版本参数，需要远端 POSIX 和 Python 3.8+。
版本文字来自远端程序自身，仅作为观察值；找到路径的旧审核不能启动这项操作。

| 工具 | 本次功能检查 |
| --- | --- |
| tar | 从固定微小内存 TAR 向标准输出读取唯一指定成员，核对字节 |
| gzip | 解压固定微小内存 gzip 流，核对字节 |
| python3 | 标准库 ZIP/tar.gz 内存往返与完整内容核对 |
| zip、unzip、rz、sz | 只观察版本，功能显示「未测试」 |

程序不创建文件、不读取用户归档、不安装工具，也不启动 Zmodem。每项时间和输出
有界。各项结果区分通过、失败、不可用、未测试、超时及输出超限；整体协议失败
不会显示为七个工具缺失。即使样本通过，也需要在实际打包或传输前分别审核。

结果仅在当前面板内存中保留。连接替换会拒绝旧审核和迟到结果；重新核验开始即
撤下该类旧成功，失败后需重新审核。PATH 观察和版本自测分别保留各自来源与时间。
取消后不自动重试，关闭 SSH 通道不证明远端进程已经结束。主题、语言和详情
开关不产生远端请求。

## English

Engineering checks and independent source review passed. Native platform
interactions remain separately unverified; the integration record above preserves
the executed commands, exact counts and prebuilt-review limitation.

Expand Transfer settings, then choose **Review inspection** under Archive transfer
and Server tools. Review the current SSH host and complete fixed command before
confirming. Open Tool details to inspect all seven observations; every refresh
needs another explicit review.

The inspection observes PATH resolution for tar, gzip, zip, unzip, python3, rz and
sz in the SSH command environment. It does not run these programs, install tools,
upload scripts, pack archives or start Zmodem. A discovered absolute path does not
verify a version, executable access, format support or interoperability.

Missing and unverified resolutions remain distinct. Unsupported shell/protocol,
malformed or excessive output, timeout and cancellation leave an incomplete
inspection, rather than seven fabricated missing results. A failed refresh
retires its earlier success. Connection replacement rejects old approvals and
late replies. Observations remain in panel memory and never authorize other
operations; theme, language and detail changes make no remote requests.

## Versions and stream checks

The engineering gate, 16 real OpenSSH tests, Linux arm64 helper/Python checks and
scoped nonauthor reviews passed. This batch has no native desktop acceptance.
See ADR 0139 and the batch record above for counts, preserved failures and the
reviewers' prebuilt-binary limitation.

Choose **Versions and checks** in the same Server tools section. Review the
current host and complete fixed command, then confirm separately. This executes
fixed version arguments for all seven tools and requires remote POSIX Python
3.8+. Version text is a remote program's own observation, not an authenticity
certificate. A PATH-only approval does not authorize execution.

Fixed small in-memory samples check tar member output, gzip decompression and
Python ZIP/tar.gz round trips. zip, unzip, rz and sz receive version checks only;
their functional checks remain **Not tested**. Nothing creates a file, reads a
user archive, installs software or starts Zmodem.

Each check has time and output limits. Passed, failed, unavailable, not tested,
timed out and output limit results remain distinct. A whole-operation failure
does not fabricate seven missing tools. Results are ephemeral and belong to the
actual SSH connection; replacement rejects old approvals and late responses.
Starting a refresh retires that check's previous result. Cancellation requires a
new review and does not prove remote process termination. Passing a sample
authorizes no packing or transfer, and changing themes, language or details
never starts a check.
