# 精确监听源码的三平台 CI 终态

源码 `bd198a8ef2578ea9b96e0a603cd705bb2eadca13` 的 Quality `43916079699`
已 **completed/success**。三个 job 均成功，没有取消或重启。
watch 已实际 `0/reaped/100.277s`，没有活动 watcher。

| 平台 / job | 普通 Rust passed / failed / ignored | app passed / failed / ignored | rustdoc passed | Python run / skipped | 标准 OpenSSH step |
| --- | --- | --- | --- | --- | --- |
| Windows `133470464623` | `3259 / 0 / 49` | `1356 / 0 / 2` | 14 | `293 / 34` | skipped |
| macOS `133470464468` | `3272 / 0 / 51` | `1367 / 0 / 2` | 14 | `293 / 2` | success |
| Ubuntu `133470463980` | `3265 / 0 / 52` | `1368 / 0 / 2` | 14 | `293 / 2` | success |

各平台 92 个普通套件与全部 rustdoc 子运行逐项核对声明、唯一结果及摘要，
零 mismatch。CI app 没有独立 `--list`，Python discovery 没有逐名 `-v`；
不补写这些不存在的证据。Windows 的三个前置检查和完整质量 gate 成功；
macOS／Ubuntu 的标准 OpenSSH step 成功，不扩大为图形桌面或完整协议矩阵。

本封存 31 成员、726,323 字节压缩包，SHA-256
`af6d5661d932de0d7a509963b68c5b0eb097bb6751eb5e3b762304fe1c3cfad0`。
包括运行中快照、终态、三个原始 job log、逐名派生、复用的 r3 解析器、
已退出的 watch 和精确源码绑定。初次两次日志获取被 gh 的终端转义输出保护
拒绝，exit 1 原件保留；后续明确保存为原始文件，exit 0，不向终端渲染其内容。
这不是测试失败，也没有为此重放 CI。

`ci-previous-source-binding-r1` 从准确提交的真实 Git blob 核对原 724 输入及
255 成员，`0/reaped/7.872s`，零 mismatch、测试重放 0。
该输入 aggregate 为
`baae8aaab5903ad60509c23341b91a23553fac47ea7d1b0ff9ac993031c2e6ef`。
本目录 `verify.py` 独立解包校验、重算逐名结果与平台计数，实际 exit 0。
它不运行 Cargo 或测试，也不代签后续编辑器备份批。

此前主封存和交付补充中的 in_progress 保留为原捕获时点。
本终态只签上述监听源码；完整产品、FILE-05/M4、三平台桌面与六目标发行仍开放。
