# API 参数与审阅界面：三平台源码 CI — 2026-10-06

状态：精确提交 `6770da0f2d6ca8d42191f33036066a8cf1660da4` 的
[Quality 37400347411](https://github.com/cyruss648/keelshell/actions/runs/37400347411)
attempt 1 及 macOS、Linux、Windows 三个 job 均为 completed/success。根重新
读取公开 Actions API 确认该提交、attempt 与终态；新的非作者已检查实际运行器、
命令日志和两份 OpenSSH receipt artifact。此结果不覆盖后来尚未提交的传输或
本地智能体修复，也不是桌面、发布包或安装更新验收。

## 实际检查范围

三平台都实际执行 x.y 依赖策略、格式检查、严格 workspace/all-targets Clippy、
workspace 普通与 rustdoc 测试、6 项工程脚本测试、57 项打包逻辑回归及额外
2 MiB 栈的本地 Ask 控制器。设置滚动条和长请求固定确认区回归在各平台测试
日志均有通过结果；这些属于 GPUI 测试框架，未启动平台桌面窗口。

| 平台 | 普通 Rust 通过 / ignored | rustdoc | 默认控制器阶段 / TCP / 秒 | 2 MiB 控制器阶段 / TCP / 秒 | future 字节 |
| --- | --- | --- | --- | --- | --- |
| macOS | 1211 / 11 | 8 | 626 / 38 / 10.746 | 626 / 38 / 10.852 | 5080 / 5080 |
| Linux | 1211 / 12 | 8 | 626 / 38 / 9.105 | 628 / 39 / 9.121 | 5080 / 5080 |
| Windows | 1192 / 11 | 8 | 626 / 38 / 67.060 | 626 / 38 / 71.047 | 5440 / 5440 |

Linux 小栈的额外两阶段与一次 TCP 来自原有有界 Abort 连接轮询：首次连接
成功，下一次拒绝连接，完整控制器在期限内结束。不能概括为三平台固定
626/38。Windows 最初报告解析器把 8 项 rustdoc 计入普通测试，1200/0doc
统计明确作废；修正后为 1192/8doc，原解析失败保留。

## 互通与证据边界

macOS/Linux 各实际执行并通过 9 项系统 OpenSSH 测试；包装器分别为
15.354 / 14.654 秒，receipt 记录 57 / 65 个已观察出生身份停止、私有临时
目录删除。两份 hosted artifact 为测试收据，不是应用安装包。Windows 的
OpenSSH 准备、运行和 artifact 上传依条件跳过，不计通过；审查者没有执行
已结束 hosted runner 的事后进程普查。

原始 GitHub API、相关日志原字节与 artifact、精确 Git blob 源码、解析与
差异派生记录在 ignored 证据包中分别保存。门禁运行中日志读取被拒绝、CLI
终端控制字节保护及首次 Windows 解析错误均保留为审查工具失败，不作为
产品故障或追溯旧失败原因。所有审查命令已直接等待终态。

该 Quality 工作流不构建完整分发包，不签名、公证或发布。Windows/Linux
桌面交互、最终主题/语言/最小窗口矩阵、六目标 Release、已安装目录的
更新/回滚和供应商智能体业务仍需各自的新证据。新的源码提交需要独立核验
自己的 CI，不能沿用本次绿色结果。
