# 服务端归档工具检测 / Server archive tool inspection

本片工程门禁及非作者源码复核已通过，原生平台交互仍需单独验收。设计见
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

服务端实际打包、版本识别、Zmodem 和性能收益验收仍开放；客户端 ZIP/tar.gz
工作流见[归档传输](ARCHIVE_TRANSFERS.md)。

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
