# Windows OpenSSH 修复的精确 CI 终态

本封存只绑定工程 **`660e90d6f7c7111853db7b73f4ca2344f633700d`**。
真实Windows OpenSSH两次、直接API以及三平台Quality均为completed/success。
首轮工程41671be的OpenSSH43990824640继续FAIL，首次坏census的精确子条件UNKNOWN；
两次新成功没有观察到transient census，不能证明旧失败就是race。

`originals.tar.gz`为 **510成员／2,555,157字节**，SHA-256：
`106af79167ef5a942b45f9bd12a3a346bc7328635670fc2a3c0dbed3fdb5e9ce`。
逐成员摘要在`manifest.json`。原始ZIP不转码，展开副本不重复保存。
原首轮168成员及修复50成员archive保持原字节；本封存通过修复verifier接续其765输入Git绑定。

| 精确运行 | 结果及范围 |
| --- | --- |
| OpenSSH43997403565／artifact14290995772 | 46成员、54,256字节，GitHub digest匹配；公开Rust SSH／monitor路由、独立Win32对照、原对象自然终态及清理。 |
| 同源码独立复跑43998970900／artifact14291880367 | 46成员、53,967字节，GitHub digest匹配；workflow_dispatch前核对远端SHA，未修改工程。 |
| 直接API43997403484／artifact14289254506 | 34成员、46,925字节，GitHub digest匹配；直接API和认证capture replay，手动SSH前置job仍skipped。根receipt无source字段，由精确run／jobs／checkout与命令装配绑定。 |
| Quality43997403533 | 三平台完整日志、jobs与两个真实OpenSSH interop artifact均已下载并核对摘要。 |

两次SSH均21原对象、20非master自然终态、4个固定PowerShell命令自然exit0。
封接纳时仅master active1、完整累计／当前census；最终Job active0，
terminal_proven／closed true、原身份集合不变、pending／handle／debug连接／fallback detach均0，
事件全部继续，临时私钥目录删除。私钥和私有配置未上传。

| Quality runner | 普通Rust passed／failed／ignored | rustdoc通过 | Python主run／skipped |
| --- | --- | --- | --- |
| windows-2025 | 3394／0／58 | 14 | 501／38 |
| macos-26 | 3407／0／60 | 14 | 501／3 |
| ubuntu-24.04 | 3400／0／61 | 14 | 501／3 |

每平台94普通套件的声明、逐名结果和摘要一致；14 rustdoc测试来自6个子运行。
包装、内嵌helper与Windows前置专项不重复计入主discovery。
macOS ARM64／Linux X64的真实OpenSSH interop各18项通过，
两个7成员ZIP的GitHub digest相等；原件记录自有进程停止、无未知祖先和临时目录删除。
这仍是隔离回环互操作，不能扩成生产或完整原生验收。

新非作者Darwin批准两次SSH与新API的指定原件，P0–P3全0：
6 ZIP、126 artifact成员、36日志成员，**4018/4018离线检查**通过，
`exit0/reaped/约0.976s`。Quality不在该reviewer范围，独立Windows执行和产品测试重跑0。
Faraday的首版CI复核另存，限定批准只覆盖旧工程41671be，未代签本次修复。
两位CI reviewer及原生reviewer Hume均已实际关闭；合计8位本批自有agent均关闭，
没有新用户chat、worktree或容器。

保留首轮reviewer解析错误、过宽checkout cleanliness检查停止、
父级统计的stdout／stderr交错解析错误、先使用错误artifact前缀未选中ZIP的回执。
这些属于工装过程，未改写产品运行结果。全量765输入来源与本机最终完整门禁继续由
[修复原件](../repair/README.md)核验。

执行自包含离线复核，无需重新访问GitHub或重放产品动作：

```sh
python3 -B docs/testing/records/2026-11-17-windows-openssh-monitor-evidence/ci-completion/verify.py
```

脚本核对manifest／ZIP逐成员、GitHub digest、精确Git生产命令、两次原对象seal／cleanup、
三平台测试计数、interop回执及review范围；通过修复封存验证765工程输入。
最终完整离线复核r3为`0/reaped/2.472996s`。中间扩展interop日志校验时误用
workspace-only汇总器而停止，原失败回执保留；改用该专项实际声明／逐名／摘要核对后通过。
原生GUI、安装、签名、公证、六目标发行和完整产品均不在本CI签署范围。
英文工具栏原生子集从[单独证据](../../2026-11-17-native-toolbar-evidence/README.md)读取。
